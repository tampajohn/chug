use std::io::Read;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime};

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const MAX_TOKENS: u32 = 8192;
const READ_TIMEOUT_SECS: u64 = 600;
/// Abort an attempt when the response body delivers no bytes for this long
/// (T2: a stalled connection is distinct from the 600s total read timeout).
/// The attempt then fails as a connection error and T1's retry applies.
const ACTIVITY_TIMEOUT_SECS: u64 = 180;
/// Connect-phase timeout (T49): reqwest has no default connect timeout, so a
/// blackholed endpoint (SYNs dropped — firewall rule, wedged NAT, a host that
/// is up but not refusing) blocks the connect until the OS TCP stack gives up
/// (~75s on macOS, 2min+ on Linux) — and T1's retry loop multiplies that stall
/// per attempt, while T2's activity watchdog cannot help because it arms only
/// after the connection exists. Failing the connect fast gets the retry to a
/// recovered endpoint sooner. Mirrors the sibling clients:
/// `mcp_http::CONNECT_TIMEOUT` and `webfetch::WEB_FETCH_CONNECT_TIMEOUT`.
const CONNECT_TIMEOUT_SECS: u64 = 10;
const ANTHROPIC_VERSION: &str = "2023-06-01";
/// Connection-level retry backoff (T1): 1s..240s — 9 retries (10 attempts),
/// ~8 min total, enough to outlive a real endpoint restart (30-60s+).
const RETRY_DELAYS_SECS: [u64; 9] = [1, 2, 4, 8, 16, 32, 64, 120, 240];
/// A `retry-after` header is honored but never longer than this.
const RETRY_AFTER_CAP_SECS: u64 = 120;

/// One content block of a message.
///
/// Known block types are deserialized with `#[serde(tag = "type")]`; anything
/// else (or a known type carrying unexpected extra fields) falls through to
/// [`ContentBlock::Other`], which preserves the raw JSON verbatim. This keeps
/// multi-turn echo valid for providers that interleave `thinking` or
/// `redacted_thinking` blocks with `text` and `tool_use`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ContentBlock {
    Known(KnownBlock),
    Other(Value),
}

/// Internally tagged ("type") variants we understand structurally.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum KnownBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "thinking")]
    Thinking {
        thinking: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        signature: Option<String>,
    },
    #[serde(rename = "redacted_thinking")]
    RedactedThinking {
        #[serde(default)]
        data: String,
    },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        #[serde(default)]
        input: Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        #[serde(default)]
        content: Value,
        #[serde(default)]
        is_error: bool,
    },
    /// T91: an image content block — `{"type":"image","source":{"type":
    /// "base64","media_type":…,"data":…}}` — as produced by `read_file`'s
    /// image leg and accepted by vision endpoints inside user messages (and
    /// tool_result content arrays).
    #[serde(rename = "image")]
    Image { source: ImageSource },
}

/// The `source` object of a [`KnownBlock::Image`]. Only the base64 source
/// shape is produced/consumed today; a URL source stays round-tripped through
/// [`ContentBlock::Other`] until needed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ImageSource {
    Base64 {
        media_type: String,
        data: String,
    },
}

/// T91: an image payload riding a tool result — media type plus base64 data.
/// Built by `tools::read_file`'s image leg; serialized into the tool_result
/// content array by [`ContentBlock::tool_result_block_with_images`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageBlock {
    pub media_type: String,
    pub data: String,
}

impl ContentBlock {
    /// Returns `(tool_use_id, name, input)` if this block requests a tool call.
    /// All other block types (text, thinking, unknown) are ignored.
    pub fn tool_use(&self) -> Option<(&str, &str, &Value)> {
        match self {
            ContentBlock::Known(KnownBlock::ToolUse { id, name, input }) => {
                Some((id, name, input))
            }
            _ => None,
        }
    }

    /// Returns the text payload if this is a plain text block.
    pub fn text(&self) -> Option<&str> {
        match self {
            ContentBlock::Known(KnownBlock::Text { text }) => Some(text),
            _ => None,
        }
    }

    /// Convenience constructor for a plain text block.
    pub fn text_block(text: impl Into<String>) -> Self {
        ContentBlock::Known(KnownBlock::Text { text: text.into() })
    }

    /// Convenience constructor for a tool_result block.
    pub fn tool_result_block(tool_use_id: &str, content: String, is_error: bool) -> Self {
        ContentBlock::Known(KnownBlock::ToolResult {
            tool_use_id: tool_use_id.to_string(),
            content: Value::String(content),
            is_error,
        })
    }

    /// T91: tool_result constructor for results carrying images. The
    /// `content` becomes an ARRAY — one image block per payload (first),
    /// then the text note — per the Messages API's array-content shape.
    /// String-content results stay byte-identical: with no images this
    /// delegates to [`Self::tool_result_block`] (`Value::String`, never a
    /// one-element array).
    pub fn tool_result_block_with_images(
        tool_use_id: &str,
        content: String,
        is_error: bool,
        images: &[ImageBlock],
    ) -> Self {
        if images.is_empty() {
            return Self::tool_result_block(tool_use_id, content, is_error);
        }
        let mut blocks: Vec<Value> = images
            .iter()
            .map(|img| {
                json!({
                    "type": "image",
                    "source": {
                        "type": "base64",
                        "media_type": img.media_type,
                        "data": img.data,
                    },
                })
            })
            .collect();
        blocks.push(json!({ "type": "text", "text": content }));
        ContentBlock::Known(KnownBlock::ToolResult {
            tool_use_id: tool_use_id.to_string(),
            content: Value::Array(blocks),
            is_error,
        })
    }
}

/// T91 degrade: the text that replaces an image block when the endpoint has
/// rejected image content.
pub const IMAGE_REMOVED_PLACEHOLDER: &str = "[image removed: endpoint rejected image content]";

/// T91 degrade leg: rebuild `messages` with EVERY image block replaced by the
/// placeholder text — both standalone image blocks and image entries inside a
/// tool_result's array content. Used (a) once, when the endpoint rejects a
/// request containing images, so the immediate retry can succeed, and (b) to
/// keep already-sent images out of every later request. Everything else —
/// text, thinking, tool_use, unknown blocks — is preserved verbatim.
pub fn replace_images_with_placeholder(messages: &[Message]) -> Vec<Message> {
    messages
        .iter()
        .map(|m| Message {
            role: m.role.clone(),
            content: m.content.iter().map(replace_block_images).collect(),
        })
        .collect()
}

fn replace_block_images(block: &ContentBlock) -> ContentBlock {
    match block {
        ContentBlock::Known(KnownBlock::Image { .. }) => {
            ContentBlock::text_block(IMAGE_REMOVED_PLACEHOLDER)
        }
        ContentBlock::Known(KnownBlock::ToolResult {
            tool_use_id,
            content: Value::Array(items),
            is_error,
        }) => {
            let items = items
                .iter()
                .map(|item| {
                    if item.get("type").and_then(Value::as_str) == Some("image") {
                        json!({ "type": "text", "text": IMAGE_REMOVED_PLACEHOLDER })
                    } else {
                        item.clone()
                    }
                })
                .collect();
            ContentBlock::Known(KnownBlock::ToolResult {
                tool_use_id: tool_use_id.clone(),
                content: Value::Array(items),
                is_error: *is_error,
            })
        }
        other => other.clone(),
    }
}

/// T91 degrade detection: does this `complete` failure look like the endpoint
/// rejecting image content? Narrow on purpose — HTTP 400 whose body mentions
/// image or content — so unrelated 400s (auth, malformed request) keep
/// today's fail-fast behavior. The client's error text embeds the status and
/// a bounded body preview (`LLM request failed: HTTP 400: <body>`).
pub fn is_image_rejection(err: &anyhow::Error) -> bool {
    let msg = err.to_string();
    let Some(body) = msg.strip_prefix("LLM request failed: HTTP 400: ") else {
        return false;
    };
    let body = body.to_ascii_lowercase();
    body.contains("image") || body.contains("content")
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: Vec<ContentBlock>,
}

impl Message {
    pub fn user(content: Vec<ContentBlock>) -> Self {
        Message {
            role: "user".to_string(),
            content,
        }
    }

    pub fn assistant(content: Vec<ContentBlock>) -> Self {
        Message {
            role: "assistant".to_string(),
            content,
        }
    }
}

/// A raw HTTP response handed back by the transport layer, before any status
/// handling. `headers` preserves (name, value) pairs as received.
pub(crate) struct RawResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl RawResponse {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

/// Why a transport attempt failed (T1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TransportError {
    /// Connection-level failure: connect error, connection reset/broken pipe,
    /// read or activity timeout. Safe to retry.
    Connection(String),
    /// Anything else (TLS misuse, unsupported scheme, ...). Fail fast.
    Fatal(String),
}

/// The HTTP boundary of [`Client`], split out so tests can inject a fake
/// transport that scripts connection failures, statuses and stalled bodies.
pub(crate) trait Transport: Send + Sync {
    fn send(
        &self,
        url: &str,
        headers: &[(String, String)],
        body: &str,
    ) -> Result<RawResponse, TransportError>;
}

/// Production transport: reqwest blocking, with a read-side activity watchdog.
struct ReqwestTransport {
    http: reqwest::blocking::Client,
    activity_timeout: Duration,
}

impl Transport for ReqwestTransport {
    fn send(
        &self,
        url: &str,
        headers: &[(String, String)],
        body: &str,
    ) -> Result<RawResponse, TransportError> {
        let mut req = self.http.post(url);
        for (name, value) in headers {
            req = req.header(name, value);
        }
        let resp = req
            .body(body.to_string())
            .send()
            .map_err(classify_reqwest)?;
        let status = resp.status().as_u16();
        let resp_headers: Vec<(String, String)> = resp
            .headers()
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();
        let body = read_body_with_watchdog(resp, self.activity_timeout)?;
        Ok(RawResponse {
            status,
            headers: resp_headers,
            body,
        })
    }
}

/// Classify a reqwest failure: timeouts, connect errors and anything wrapping
/// an io error of the connection-reset family are connection-level (retryable
/// under T1); everything else fails fast.
fn classify_reqwest(e: reqwest::Error) -> TransportError {
    if e.is_timeout() || e.is_connect() {
        return TransportError::Connection(e.to_string());
    }
    let mut source: Option<&(dyn std::error::Error + 'static)> = Some(&e);
    while let Some(err) = source {
        if let Some(io) = err.downcast_ref::<std::io::Error>()
            && is_connection_io_error(io.kind())
        {
            return TransportError::Connection(io.to_string());
        }
        source = err.source();
    }
    // Belt and braces: hyper surfaces some resets as plain display strings
    // without an io error anywhere in the chain.
    let text = e.to_string().to_ascii_lowercase();
    if text.contains("connection reset")
        || text.contains("broken pipe")
        || text.contains("connection closed")
    {
        return TransportError::Connection(e.to_string());
    }
    TransportError::Fatal(e.to_string())
}

/// Classify a raw io failure from the body reader (connection-reset family is
/// retryable under T1; anything else fails fast).
fn classify_io(e: &std::io::Error) -> TransportError {
    if is_connection_io_error(e.kind()) {
        TransportError::Connection(e.to_string())
    } else {
        TransportError::Fatal(e.to_string())
    }
}

fn is_connection_io_error(kind: std::io::ErrorKind) -> bool {
    use std::io::ErrorKind as K;
    matches!(
        kind,
        K::ConnectionReset
            | K::ConnectionAborted
            | K::BrokenPipe
            | K::TimedOut
            | K::UnexpectedEof
    )
}

/// Read the response body with a per-chunk activity watchdog (T2): if no bytes
/// arrive for `activity_timeout`, the attempt is aborted with a connection
/// error so T1's retry schedule applies. Distinct from the client-wide 600s
/// total read timeout.
///
/// Blocking `Read` offers no deadlines, so the read runs on a worker thread
/// that signals progress per chunk. If the watchdog fires, that thread is left
/// blocked until the client's total read timeout errs it out — a bounded
/// leak, never a hung caller.
fn read_body_with_watchdog<R: Read + Send + 'static>(
    mut reader: R,
    activity_timeout: Duration,
) -> Result<String, TransportError> {
    enum BodyMsg {
        Progress,
        Done(Result<String, TransportError>),
    }
    let (tx, rx) = std::sync::mpsc::channel::<BodyMsg>();
    thread::spawn(move || {
        let mut buf = Vec::new();
        let mut chunk = [0u8; 8192];
        loop {
            match reader.read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => {
                    buf.extend_from_slice(&chunk[..n]);
                    if tx.send(BodyMsg::Progress).is_err() {
                        // Consumer gone (activity timeout): stop reading.
                        return;
                    }
                }
                Err(e) => {
                    let _ = tx.send(BodyMsg::Done(Err(classify_io(&e))));
                    return;
                }
            }
        }
        let _ = tx.send(BodyMsg::Done(Ok(
            String::from_utf8_lossy(&buf).into_owned()
        )));
    });

    loop {
        match rx.recv_timeout(activity_timeout) {
            Ok(BodyMsg::Progress) => continue,
            Ok(BodyMsg::Done(result)) => return result,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                return Err(TransportError::Connection(format!(
                    "no response bytes for {}s (activity timeout)",
                    activity_timeout.as_secs()
                )));
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                return Err(TransportError::Fatal(
                    "body reader thread died unexpectedly".to_string(),
                ));
            }
        }
    }
}

#[derive(Clone)]
pub struct Client {
    transport: Arc<dyn Transport>,
    retry_delays: Vec<Duration>,
    base_url: String,
    api_key: Option<String>,
    auth_token: Option<String>,
    model: String,
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client")
            .field("base_url", &self.base_url)
            .field("model", &self.model)
            .finish_non_exhaustive()
    }
}

/// The driver-facing slice of the Messages API client. A trait so driver and
/// chat tests can script responses without any network. `&mut self` so test
/// doubles can consume scripted responses.
///
/// `obs` carries the SPEC-8 observability context for this call: when
/// `trace_id` is set, every successful response emits one Langfuse
/// generation (model, usage incl. cache reads, latency, stop reason,
/// iteration) via the global sink. `trace_id: None` → no emission, and the
/// call behaves exactly as before.
pub trait Llm {
    fn complete(
        &mut self,
        system: &str,
        messages: &[Message],
        tools: &[Value],
        obs: &ObsCtx<'_>,
    ) -> anyhow::Result<Response>;
    fn set_model(&mut self, model: &str);
    /// The model id subsequent `complete` calls will use (T12: abort output
    /// names it, so chat `/model` switches are reflected immediately).
    fn model(&self) -> &str;
}

/// Observability context for one `complete` call, supplied by the driver.
#[derive(Debug, Clone, Copy, Default)]
pub struct ObsCtx<'a> {
    /// Langfuse trace id for the enclosing run/chat session (`None` when
    /// observability is off or no trace exists).
    pub trace_id: Option<&'a str>,
    /// Zero-based iteration the call belongs to.
    pub iteration: u32,
}

impl Llm for Client {
    fn complete(
        &mut self,
        system: &str,
        messages: &[Message],
        tools: &[Value],
        obs: &ObsCtx<'_>,
    ) -> anyhow::Result<Response> {
        let start = SystemTime::now();
        let result = Client::complete(self, system, messages, tools);
        if let (Some(trace_id), Ok(resp)) = (&obs.trace_id, &result) {
            emit_generation(
                crate::observ::global(),
                trace_id,
                &self.model,
                resp,
                start,
                SystemTime::now(),
                obs.iteration,
            );
        }
        result
    }

    fn set_model(&mut self, model: &str) {
        Client::set_model(self, model);
    }

    fn model(&self) -> &str {
        &self.model
    }
}

#[derive(Debug)]
pub struct Response {
    pub body: Value,
}

impl Response {
    /// Parsed content blocks. Unknown block types come back as
    /// [`ContentBlock::Other`] with their raw JSON intact.
    pub fn content_blocks(&self) -> Vec<ContentBlock> {
        match self.body.get("content") {
            Some(v) => serde_json::from_value::<Vec<ContentBlock>>(v.clone())
                .unwrap_or_default(),
            None => Vec::new(),
        }
    }

    /// Stop reason (`end_turn`, `tool_use`, ...). Used by generation events,
    /// tests and diagnostics; the driver loop itself keys off tool_use blocks
    /// rather than this field.
    pub fn stop_reason(&self) -> Option<String> {
        self.body
            .get("stop_reason")
            .and_then(Value::as_str)
            .map(str::to_string)
    }

    /// Token usage mapped from the response body (SPEC-8 generation events).
    pub fn usage(&self) -> crate::observ::Usage {
        usage_from(&self.body)
    }

    pub fn text(&self) -> String {
        self.content_blocks()
            .iter()
            .filter_map(ContentBlock::text)
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Map a Messages API `usage` object onto the Langfuse [`Usage`] shape.
/// `total_tokens` wins when the provider sends one; otherwise input+output.
/// Absent fields default to zero; `cache_read_input_tokens` maps through
/// only when present.
fn usage_from(body: &Value) -> crate::observ::Usage {
    let usage = body.get("usage").cloned().unwrap_or(Value::Null);
    let input = usage
        .get("input_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let output = usage
        .get("output_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    crate::observ::Usage {
        input,
        output,
        total: usage
            .get("total_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(input + output),
        cache_read_input_tokens: usage
            .get("cache_read_input_tokens")
            .and_then(Value::as_u64),
    }
}

/// SPEC-8: hand one LLM response to the sink as a generation — model,
/// usage (incl. `cache_read_input_tokens`), start/end latency, stop reason,
/// iteration. A no-op on a disabled sink.
pub(crate) fn emit_generation(
    sink: &crate::observ::Sink,
    trace_id: &str,
    model: &str,
    resp: &Response,
    start: SystemTime,
    end: SystemTime,
    iteration: u32,
) {
    sink.generation(
        trace_id,
        model,
        MAX_TOKENS,
        &resp.usage(),
        start,
        end,
        iteration,
        resp.stop_reason().as_deref(),
    );
}

impl Client {
    pub fn new(model: &str) -> anyhow::Result<Self> {
        // Endpoint + credentials: process env first, then the `env` block of
        // ~/.claude/settings.json, then the api.anthropic.com default
        // (base URL only). Resolution lives in auth.rs.
        let ep = crate::auth::resolve_endpoint()?;
        Ok(Self {
            transport: Arc::new(ReqwestTransport {
                http: build_http_client()?,
                activity_timeout: Duration::from_secs(ACTIVITY_TIMEOUT_SECS),
            }),
            retry_delays: default_retry_delays(),
            base_url: ep.base_url,
            api_key: ep.api_key,
            auth_token: ep.auth_token,
            model: model.to_string(),
        })
    }

    /// Test-only constructor that skips credential checks (no network is ever
    /// attempted by the constructor itself).
    #[cfg(test)]
    pub fn new_without_credentials(model: &str) -> anyhow::Result<Self> {
        Ok(Self {
            transport: Arc::new(ReqwestTransport {
                http: build_http_client()?,
                activity_timeout: Duration::from_secs(ACTIVITY_TIMEOUT_SECS),
            }),
            retry_delays: default_retry_delays(),
            base_url: crate::auth::DEFAULT_BASE_URL.to_string(),
            api_key: None,
            auth_token: None,
            model: model.to_string(),
        })
    }

    /// Test-only constructor wiring a caller-built transport with zero retry
    /// delays, so driver tests can run the FULL `run_loop` (T55 lock
    /// acquire/release legs) to goal-acceptance without network.
    #[cfg(test)]
    pub(crate) fn with_transport_for_tests(
        transport: Arc<dyn Transport>,
        model: &str,
    ) -> Self {
        Self {
            transport,
            retry_delays: Vec::new(),
            base_url: "http://fake.local".to_string(),
            api_key: None,
            auth_token: None,
            model: model.to_string(),
        }
    }

    /// Swap the model id used by subsequent `complete` calls (chat `/model`).
    pub fn set_model(&mut self, model: &str) {
        self.model = model.to_string();
    }

    /// One non-streaming Messages API call with T1 retry semantics: connection
    /// failures and the gateway/overload statuses retry with exponential
    /// backoff (honoring `retry-after`, capped); other HTTP errors fail fast.
    pub fn complete(
        &self,
        system: &str,
        messages: &[Message],
        tools: &[Value],
    ) -> anyhow::Result<Response> {
        let url = format!("{}/v1/messages", self.base_url.trim_end_matches('/'));
        let body = json!({
            "model": self.model,
            "max_tokens": MAX_TOKENS,
            "system": system,
            "messages": messages,
            "tools": tools,
            "tool_choice": { "type": "auto" },
        });
        let body = serde_json::to_string(&body).context("serializing request body")?;

        let mut headers = vec![(
            "anthropic-version".to_string(),
            ANTHROPIC_VERSION.to_string(),
        )];
        if let Some(key) = &self.api_key {
            headers.push(("x-api-key".to_string(), key.clone()));
        }
        if let Some(token) = &self.auth_token {
            headers.push(("Authorization".to_string(), format!("Bearer {token}")));
        }

        let total_attempts = self.retry_delays.len() + 1;
        let mut last_error = String::new();
        let mut attempts = 0;
        for attempt in 0..total_attempts {
            attempts += 1;
            match self.transport.send(&url, &headers, &body) {
                Ok(raw) => {
                    if (200..300).contains(&raw.status) {
                        let parsed: Value = serde_json::from_str(&raw.body).with_context(|| {
                            format!("parsing response JSON: {}", preview(&raw.body, 500))
                        })?;
                        return Ok(Response { body: parsed });
                    }
                    let status = raw.status;
                    last_error = format!("HTTP {status}: {}", preview(&raw.body, 1000));
                    // Only the overload/gateway classes retry (T1); 4xx auth,
                    // model and request errors stay fail-fast.
                    if !is_retryable_status(status) {
                        bail!("LLM request failed: {last_error}");
                    }
                    if attempt + 1 == total_attempts {
                        break;
                    }
                    let retry_after = raw.header("retry-after");
                    thread::sleep(self.delay_for(attempt, retry_after));
                }
                Err(TransportError::Connection(msg)) => {
                    last_error = format!("connection error: {msg}");
                    if attempt + 1 == total_attempts {
                        break;
                    }
                    thread::sleep(self.retry_delays[attempt]);
                }
                Err(TransportError::Fatal(msg)) => {
                    bail!("LLM request failed: {msg}");
                }
            }
        }
        bail!(
            "LLM request failed after {attempts} attempts; last error: {last_error}"
        );
    }

    /// Delay before the retry following `attempt`: an explicit `retry-after`
    /// header wins, capped at [`RETRY_AFTER_CAP_SECS`]; otherwise the
    /// scheduled backoff for that attempt.
    fn delay_for(&self, attempt: usize, retry_after: Option<&str>) -> Duration {
        retry_after
            .and_then(|s| s.trim().parse::<u64>().ok())
            .map(|secs| Duration::from_secs(secs.min(RETRY_AFTER_CAP_SECS)))
            .unwrap_or(self.retry_delays[attempt])
    }
}

fn build_http_client() -> anyhow::Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(CONNECT_TIMEOUT_SECS))
        .timeout(Duration::from_secs(READ_TIMEOUT_SECS))
        .use_rustls_tls()
        .build()
        .context("building HTTP client")
}

fn default_retry_delays() -> Vec<Duration> {
    RETRY_DELAYS_SECS
        .iter()
        .map(|&secs| Duration::from_secs(secs))
        .collect()
}

/// T1: the only HTTP statuses worth retrying — overload and gateway classes.
/// Auth/model errors (401/403/404...), bad requests and unexpected server
/// errors fail fast.
fn is_retryable_status(status: u16) -> bool {
    matches!(status, 429 | 502 | 503 | 504 | 529)
}

fn preview(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        let head: String = s.chars().take(max_chars).collect();
        format!("{head}...")
    }
}

/// Test double: replays scripted response bodies and records every call, so
/// driver/chat state-machine tests run without network.
#[cfg(test)]
pub struct ScriptedLlm {
    pub responses: std::collections::VecDeque<Value>,
    pub calls: Vec<(String, Vec<Message>)>,
    pub model: String,
    /// When set, every `complete` call raises this flag before returning —
    /// lets tests trigger a deterministic mid-turn operator interrupt.
    pub abort_on_call: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
}

#[cfg(test)]
impl ScriptedLlm {
    pub fn new(responses: Vec<Value>) -> Self {
        ScriptedLlm {
            responses: responses.into(),
            calls: Vec::new(),
            model: "scripted-model".to_string(),
            abort_on_call: None,
        }
    }
}

#[cfg(test)]
impl Llm for ScriptedLlm {
    fn complete(
        &mut self,
        system: &str,
        messages: &[Message],
        _tools: &[Value],
        _obs: &ObsCtx<'_>,
    ) -> anyhow::Result<Response> {
        self.calls.push((system.to_string(), messages.to_vec()));
        if let Some(flag) = &self.abort_on_call {
            flag.store(true, std::sync::atomic::Ordering::SeqCst);
        }
        let body = self
            .responses
            .pop_front()
            .ok_or_else(|| anyhow::anyhow!("ScriptedLlm: no scripted response left"))?;
        Ok(Response { body })
    }

    fn set_model(&mut self, model: &str) {
        self.model = model.to_string();
    }

    fn model(&self) -> &str {
        &self.model
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T49: the three api.rs timeout constants are value-pinned (T42 pattern —
    /// a const-only edit must fail the suite). Each assert names its const so
    /// a mutation points at the right one.
    #[test]
    fn timeout_constants_are_value_pinned() {
        assert_eq!(
            CONNECT_TIMEOUT_SECS, 10u64,
            "CONNECT_TIMEOUT_SECS drifted from 10 (mcp_http/webfetch parity)"
        );
        assert_eq!(
            READ_TIMEOUT_SECS, 600u64,
            "READ_TIMEOUT_SECS drifted from 600 (total read timeout)"
        );
        assert_eq!(
            ACTIVITY_TIMEOUT_SECS, 180u64,
            "ACTIVITY_TIMEOUT_SECS drifted from 180 (T2 body watchdog)"
        );
    }

    #[test]
    fn response_parses_tool_use_ignoring_other_blocks() {
        let resp = Response {
            body: json!({
                "stop_reason": "tool_use",
                "content": [
                    {"type": "thinking", "thinking": "let me look", "signature": "sig1"},
                    {"type": "text", "text": "hello"},
                    {"type": "tool_use", "id": "tu_1", "name": "bash", "input": {"command": "ls"}},
                    {"type": "brand_new_future_block", "payload": {"a": 1}}
                ]
            }),
        };
        let blocks = resp.content_blocks();
        assert_eq!(blocks.len(), 4);
        // thinking block round-trips verbatim enough to re-serialize with its signature
        match &blocks[0] {
            ContentBlock::Known(KnownBlock::Thinking { thinking, signature }) => {
                assert_eq!(thinking, "let me look");
                assert_eq!(signature.as_deref(), Some("sig1"));
            }
            other => panic!("expected thinking block, got {other:?}"),
        }
        // unknown block type falls back to raw JSON, never errors
        match &blocks[3] {
            ContentBlock::Other(v) => {
                assert_eq!(v["payload"]["a"], 1);
            }
            other => panic!("expected raw fallback, got {other:?}"),
        }
        // only the tool_use block is surfaced for dispatch
        let tool_uses: Vec<_> = blocks.iter().filter_map(ContentBlock::tool_use).collect();
        assert_eq!(tool_uses.len(), 1);
        assert_eq!(tool_uses[0].0, "tu_1");
        assert_eq!(tool_uses[0].1, "bash");
        assert_eq!(tool_uses[0].2["command"], "ls");
        assert_eq!(resp.text(), "hello");
        assert_eq!(resp.stop_reason().as_deref(), Some("tool_use"));
    }

    #[test]
    fn usage_maps_input_output_total_and_cache_read() {
        let resp = Response {
            body: json!({
                "stop_reason": "tool_use",
                "usage": {
                    "input_tokens": 100,
                    "output_tokens": 20,
                    "cache_read_input_tokens": 64,
                },
            }),
        };
        assert_eq!(
            resp.usage(),
            crate::observ::Usage {
                input: 100,
                output: 20,
                total: 120,
                cache_read_input_tokens: Some(64),
            }
        );

        // total_tokens wins when the provider sends one; cache read absent.
        let resp = Response {
            body: json!({
                "usage": {"input_tokens": 5, "output_tokens": 6, "total_tokens": 11},
            }),
        };
        assert_eq!(
            resp.usage(),
            crate::observ::Usage {
                input: 5,
                output: 6,
                total: 11,
                cache_read_input_tokens: None,
            }
        );

        // No usage object at all: zeros, never a panic.
        let resp = Response { body: json!({"content": []}) };
        assert_eq!(
            resp.usage(),
            crate::observ::Usage {
                input: 0,
                output: 0,
                total: 0,
                cache_read_input_tokens: None,
            }
        );
    }

    #[test]
    fn emit_generation_hands_the_response_to_the_sink() {
        let transport = crate::observ::testing::CountingTransport::new();
        let sink = crate::observ::testing::test_sink(transport.clone());
        let resp = Response {
            body: json!({
                "stop_reason": "tool_use",
                "usage": {"input_tokens": 10, "output_tokens": 5, "cache_read_input_tokens": 3},
            }),
        };
        emit_generation(
            &sink,
            "chug-test0001",
            "scripted-model",
            &resp,
            SystemTime::UNIX_EPOCH,
            SystemTime::now(),
            7,
        );
        sink.shutdown();

        let events = transport.events();
        assert_eq!(events.len(), 1);
        let event = &events[0];
        assert_eq!(event["type"], "generation-create");
        assert_eq!(event["body"]["traceId"], "chug-test0001");
        assert_eq!(event["body"]["model"], "scripted-model");
        assert_eq!(event["body"]["modelParameters"]["maxTokens"], MAX_TOKENS);
        assert_eq!(event["body"]["usage"]["input"], 10);
        assert_eq!(event["body"]["usage"]["output"], 5);
        assert_eq!(event["body"]["usage"]["total"], 15);
        assert_eq!(event["body"]["usage"]["cache_read_input_tokens"], 3);
        assert_eq!(event["body"]["metadata"]["iteration"], 7);
        assert_eq!(event["body"]["metadata"]["stop_reason"], "tool_use");
    }

    #[test]
    fn set_model_swaps_the_model_id() {
        let mut client = Client::new_without_credentials("first-model").unwrap();
        assert_eq!(client.model(), "first-model");
        client.set_model("second-model");
        assert_eq!(client.model(), "second-model");
    }

    #[test]
    fn message_serialization_round_trips() {
        let msg = Message::user(vec![
            ContentBlock::text_block("kick"),
            ContentBlock::tool_result_block("tu_1", "output".into(), true),
            ContentBlock::Known(KnownBlock::RedactedThinking {
                data: "xyz".into(),
            }),
            ContentBlock::Other(json!({"type": "brand_new_block", "payload": [1]})),
        ]);
        let text = serde_json::to_string(&msg).unwrap();
        assert_eq!(serde_json::from_str::<Message>(&text).unwrap(), msg);
        assert!(text.contains("\"type\":\"tool_result\""));
        assert!(text.contains("\"tool_use_id\":\"tu_1\""));
        assert!(text.contains("\"is_error\":true"));
    }

    // ---- T1/T2: retry + watchdog regression tests on a fake transport ----

    struct FakeTransport {
        responses: std::sync::Mutex<std::collections::VecDeque<Result<RawResponse, TransportError>>>,
        calls: std::sync::Mutex<usize>,
    }

    impl FakeTransport {
        fn new(responses: Vec<Result<RawResponse, TransportError>>) -> Arc<Self> {
            Arc::new(FakeTransport {
                responses: std::sync::Mutex::new(responses.into()),
                calls: std::sync::Mutex::new(0),
            })
        }

        fn calls(&self) -> usize {
            *self.calls.lock().unwrap()
        }
    }

    impl Transport for FakeTransport {
        fn send(
            &self,
            _url: &str,
            _headers: &[(String, String)],
            _body: &str,
        ) -> Result<RawResponse, TransportError> {
            *self.calls.lock().unwrap() += 1;
            self.responses
                .lock()
                .unwrap()
                .pop_front()
                .expect("fake transport script exhausted")
        }
    }

    fn ok_raw(body: Value) -> Result<RawResponse, TransportError> {
        Ok(RawResponse {
            status: 200,
            headers: Vec::new(),
            body: body.to_string(),
        })
    }

    fn status_raw(status: u16, headers: Vec<(String, String)>) -> Result<RawResponse, TransportError> {
        Ok(RawResponse {
            status,
            headers,
            body: json!({"error": {"message": "synthetic"}}).to_string(),
        })
    }

    fn conn_err(msg: &str) -> Result<RawResponse, TransportError> {
        Err(TransportError::Connection(msg.to_string()))
    }

    fn client_with(transport: Arc<dyn Transport>, delays: &[u64]) -> Client {
        Client {
            transport,
            retry_delays: delays.iter().map(|&s| Duration::from_secs(s)).collect(),
            base_url: "http://fake.local".to_string(),
            api_key: None,
            auth_token: None,
            model: "test-model".to_string(),
        }
    }

    fn run_complete(client: &Client) -> anyhow::Result<Response> {
        client.complete(
            "sys",
            &[Message::user(vec![ContentBlock::text_block("hi")])],
            &[],
        )
    }

    /// T1: connection resets retry; the run survives once the endpoint comes
    /// back (here: on attempt 3, as after a real endpoint restart).
    #[test]
    fn connection_error_retries_and_succeeds_on_attempt_3() {
        let ft = FakeTransport::new(vec![
            conn_err("connection reset by peer"),
            conn_err("connection reset by peer"),
            ok_raw(json!({"content": [{"type": "text", "text": "back"}]})),
        ]);
        let client = client_with(ft.clone(), &[0, 0, 0, 0, 0, 0, 0, 0, 0]);
        let resp = run_complete(&client).unwrap();
        assert_eq!(resp.text(), "back");
        assert_eq!(ft.calls(), 3);
    }

    /// T1: HTTP 400 (auth/model/request errors) never retries.
    #[test]
    fn http_400_fails_immediately() {
        let ft = FakeTransport::new(vec![status_raw(400, Vec::new())]);
        let client = client_with(ft.clone(), &[0, 0, 0, 0, 0, 0, 0, 0, 0]);
        let err = run_complete(&client).unwrap_err();
        assert!(err.to_string().contains("HTTP 400"), "{err}");
        assert_eq!(ft.calls(), 1);
    }

    /// T1: retryable statuses (429/502/503/504/529) follow the schedule; a
    /// plain 500 does not.
    #[test]
    fn retryable_status_follows_schedule_and_500_fails_fast() {
        let ft = FakeTransport::new((0..50).map(|_| status_raw(503, Vec::new())).collect());
        let client = client_with(ft.clone(), &[0, 0, 0]);
        let err = run_complete(&client).unwrap_err();
        assert_eq!(ft.calls(), 4, "schedule of 3 delays = 4 attempts: {err}");
        assert!(err.to_string().contains("after 4 attempts"), "{err}");

        let ft = FakeTransport::new(vec![status_raw(500, Vec::new())]);
        let client = client_with(ft.clone(), &[0, 0, 0]);
        let err = run_complete(&client).unwrap_err();
        assert!(err.to_string().contains("HTTP 500"), "{err}");
        assert_eq!(ft.calls(), 1);
    }

    /// T1: the schedule bounds the retries on persistent connection failures.
    #[test]
    fn connection_retries_stop_after_schedule() {
        let ft = FakeTransport::new(
            (0..50)
                .map(|_| conn_err("connection reset"))
                .collect(),
        );
        let client = client_with(ft.clone(), &[0, 0]);
        let err = run_complete(&client).unwrap_err();
        assert_eq!(ft.calls(), 3, "schedule of 2 delays = 3 attempts");
        assert!(err.to_string().contains("after 3 attempts"), "{err}");
    }

    /// T1: `retry-after` wins over the scheduled delay, capped at 120s; a
    /// non-numeric header falls back to the schedule.
    #[test]
    fn retry_after_is_honored_and_capped() {
        let client = client_with(FakeTransport::new(Vec::new()), &[1, 2, 4]);
        assert_eq!(client.delay_for(0, Some("5")), Duration::from_secs(5));
        assert_eq!(
            client.delay_for(0, Some("300")),
            Duration::from_secs(RETRY_AFTER_CAP_SECS)
        );
        assert_eq!(client.delay_for(1, None), Duration::from_secs(2));
        assert_eq!(
            client.delay_for(0, Some("not-a-number")),
            Duration::from_secs(1)
        );
        // End-to-end: a 429 with retry-after: 0 retries without stalling.
        let ft = FakeTransport::new(vec![
            status_raw(429, vec![("retry-after".to_string(), "0".to_string())]),
            ok_raw(json!({"content": []})),
        ]);
        let client = client_with(ft.clone(), &[0, 0, 0, 0, 0, 0, 0, 0, 0]);
        run_complete(&client).unwrap();
        assert_eq!(ft.calls(), 2);
    }

    /// A reader that delivers one byte then goes silent for the rest of the
    /// test (simulating a stalled connection mid-body).
    struct SilentAfterFirst {
        sent_first: bool,
    }

    impl Read for SilentAfterFirst {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            if !self.sent_first {
                self.sent_first = true;
                buf[0] = b'a';
                return Ok(1);
            }
            // Stall well past the watchdog, then report EOF so the worker
            // thread (which outlives the abandoned attempt) also exits.
            thread::sleep(Duration::from_secs(2));
            Ok(0)
        }
    }

    /// T2: a body stream that goes silent aborts the attempt at the activity
    /// timeout (injected: 1s instead of the production 180s) as a connection
    /// error — which T1's retry then treats as retryable.
    #[test]
    fn body_watchdog_aborts_silent_stream() {
        let start = std::time::Instant::now();
        let err = read_body_with_watchdog(
            SilentAfterFirst { sent_first: false },
            Duration::from_secs(1),
        )
        .unwrap_err();
        let elapsed = start.elapsed();
        match err {
            TransportError::Connection(msg) => {
                assert!(msg.contains("activity timeout"), "{msg}")
            }
            other => panic!("expected connection error, got {other:?}"),
        }
        // Not instant: the injected deadline must actually be awaited, not
        // skipped (recv_timeout never returns Timeout early, so this lower
        // bound only bites if the watchdog fired immediately).
        assert!(elapsed >= Duration::from_millis(900), "{elapsed:?}");
        // T74 (cold-parallel flake, 2 sightings 2026-09-26): the old upper
        // bound (< 1900ms) assumed this thread wakes within ~900ms of the 1s
        // deadline expiring, but under a cold default-parallel full-suite
        // build (all cores on rustc) the recv_timeout wakeup can be delayed
        // past that, false-redding the gate. The bound's only job is to
        // discriminate the ACTIVITY timeout (~1s) from the client-wide 600s
        // TOTAL read timeout, and that discrimination survives a much wider
        // bound: 15s is still 40x under 600s (T31 comment convention: name
        // the discrimination the margin preserves).
        assert!(elapsed < Duration::from_secs(15), "{elapsed:?}");
    }

    /// A reader that dribbles chunks at a steady pace: every inter-chunk gap
    /// far under the activity timeout while the TOTAL stream time crosses it
    /// — the shape that separates per-chunk activity from a total deadline.
    struct SteadyReader {
        interval: Duration,
        chunks: usize,
        sent: usize,
        next_at: std::time::Instant,
    }

    impl Read for SteadyReader {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            if self.sent >= self.chunks {
                return Ok(0);
            }
            let now = std::time::Instant::now();
            if now < self.next_at {
                thread::sleep(self.next_at - now);
            }
            self.next_at = std::time::Instant::now() + self.interval;
            buf[0] = b'x';
            self.sent += 1;
            Ok(1)
        }
    }

    /// T2: the watchdog is per-chunk activity, not a total read deadline — a
    /// slow-but-steady stream whose TOTAL time (50ms × 60 chunks ≈ 3s)
    /// exceeds the 1s injected activity timeout by 3x still completes.
    ///
    /// T74 margin audit (cold-parallel flake class, 2 sightings 2026-09-26):
    /// the old shape (250ms × 5 = 1.25s total, only 1.25x the activity
    /// timeout, gaps a bare 4x under it) false-redded under a cold
    /// default-parallel build — one scheduler-starved stretch of the reader
    /// thread pushed a 250ms sleep past the 1s deadline and the watchdog
    /// fired on the green leg. New margins, each naming the discrimination
    /// it preserves (T31 convention):
    /// - Per-gap headroom: 50ms gaps sit 20x under the 1s activity timeout,
    ///   so a false-fire needs ~a full second of continuous starvation of
    ///   the reader thread, not one late wakeup. Only the READER's delay
    ///   can arm the deadline (progress is channel-buffered, so main-thread
    ///   starvation cannot), and sleep never undershoots, so load stretches
    ///   the stream without ever firing the watchdog.
    /// - Not-a-total-deadline: nominal total ~3s = 3x the activity timeout,
    ///   and load can only grow it — if the watchdog were a TOTAL 1s
    ///   deadline this stream could never complete (RED-proof b in the
    ///   commit message).
    #[test]
    fn body_watchdog_allows_slow_steady_stream() {
        let reader = SteadyReader {
            interval: Duration::from_millis(50),
            chunks: 60,
            sent: 0,
            next_at: std::time::Instant::now(),
        };
        let body = read_body_with_watchdog(reader, Duration::from_secs(1)).unwrap();
        assert_eq!(body, "x".repeat(60));
    }

    /// A mid-body connection reset is connection-level (retryable under T1);
    /// an unexpected io error is fatal (fail fast).
    ///
    /// T74 family sweep (cold-parallel timing-margin class): this test
    /// injects the 1s activity timeout but asserts ERROR CLASS only — both
    /// readers error on their very first `read`, so the deadline never arms
    /// and there is no wall-clock bound to be load-fragile. No margin to
    /// widen; the only other `read_body_with_watchdog` callers (the two
    /// watchdog tests above) carry this row's reworked margins.
    #[test]
    fn body_read_error_classification() {
        struct ResetReader;
        impl Read for ResetReader {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::new(
                    std::io::ErrorKind::ConnectionReset,
                    "connection reset by peer",
                ))
            }
        }
        let err = read_body_with_watchdog(ResetReader, Duration::from_secs(1)).unwrap_err();
        assert_eq!(err, TransportError::Connection("connection reset by peer".to_string()));

        struct DeniedReader;
        impl Read for DeniedReader {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "denied",
                ))
            }
        }
        let err = read_body_with_watchdog(DeniedReader, Duration::from_secs(1)).unwrap_err();
        assert_eq!(err, TransportError::Fatal("denied".to_string()));
    }

    // ---- T91: image blocks ----

    /// KnownBlock::Image round-trips the base64 source shape: `type: "image"`
    /// with `source: {type: "base64", media_type, data}`, in both directions.
    #[test]
    fn image_known_block_round_trips_base64_source() {
        let block = ContentBlock::Known(KnownBlock::Image {
            source: ImageSource::Base64 {
                media_type: "image/png".to_string(),
                data: "aGk=".to_string(),
            },
        });
        let text = serde_json::to_string(&block).unwrap();
        assert_eq!(
            text,
            r#"{"type":"image","source":{"type":"base64","media_type":"image/png","data":"aGk="}}"#
        );
        let back: ContentBlock = serde_json::from_str(&text).unwrap();
        assert_eq!(back, block, "serialization round-trips");
        // An endpoint's image block parses into the known variant, not Other.
        let parsed: ContentBlock =
            serde_json::from_str(r#"{"type":"image","source":{"type":"base64","media_type":"image/jpeg","data":"Zg=="}}"#).unwrap();
        match parsed {
            ContentBlock::Known(KnownBlock::Image {
                source: ImageSource::Base64 { media_type, data },
            }) => {
                assert_eq!(media_type, "image/jpeg");
                assert_eq!(data, "Zg==");
            }
            other => panic!("expected known image block, got {other:?}"),
        }
    }

    /// A tool result carrying images serializes with content as an ARRAY:
    /// image block(s) first, then the text note.
    #[test]
    fn image_tool_result_serializes_content_array_image_first_then_text() {
        let block = ContentBlock::tool_result_block_with_images(
            "tu_1",
            "[image: a.png (12 bytes, image/png)]".to_string(),
            false,
            &[ImageBlock {
                media_type: "image/png".to_string(),
                data: "aGk=".to_string(),
            }],
        );
        let text = serde_json::to_string(&block).unwrap();
        let v: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["type"], "tool_result");
        assert_eq!(v["tool_use_id"], "tu_1");
        assert!(v["content"].is_array(), "image results use array content");
        let items = v["content"].as_array().unwrap();
        assert_eq!(items.len(), 2, "image block first, then the text note");
        assert_eq!(items[0]["type"], "image");
        assert_eq!(items[0]["source"]["type"], "base64");
        assert_eq!(items[0]["source"]["media_type"], "image/png");
        assert_eq!(items[0]["source"]["data"], "aGk=");
        assert_eq!(items[1]["type"], "text");
        assert_eq!(items[1]["text"], "[image: a.png (12 bytes, image/png)]");
        // The serialized form round-trips back through the block parser.
        let parsed: ContentBlock = serde_json::from_str(&text).unwrap();
        assert_eq!(parsed, block);
    }

    /// Non-regression pin: a result with NO images stays byte-identical to
    /// the pre-T91 shape — string content via the plain constructor, never a
    /// one-element array.
    #[test]
    fn image_plain_tool_result_stays_string_content_byte_identical() {
        let with_empty = ContentBlock::tool_result_block_with_images(
            "tu_1",
            "output".to_string(),
            true,
            &[],
        );
        let plain = ContentBlock::tool_result_block("tu_1", "output".to_string(), true);
        assert_eq!(with_empty, plain, "empty images delegates to the plain shape");
        let text = serde_json::to_string(&with_empty).unwrap();
        assert_eq!(
            text,
            r#"{"type":"tool_result","tool_use_id":"tu_1","content":"output","is_error":true}"#,
            "byte-identical pre-T91 serialization"
        );
    }

    /// The degrade rewrite: standalone image blocks AND image entries inside
    /// tool_result array content become the placeholder text; everything else
    /// is preserved verbatim.
    #[test]
    fn image_replacement_swaps_blocks_and_array_entries_for_placeholder() {
        let messages = vec![
            Message::user(vec![
                ContentBlock::text_block("kick"),
                ContentBlock::Known(KnownBlock::Image {
                    source: ImageSource::Base64 {
                        media_type: "image/png".to_string(),
                        data: "aGk=".to_string(),
                    },
                }),
                ContentBlock::tool_result_block_with_images(
                    "tu_1",
                    "[image: a.png (2 bytes, image/png)]".to_string(),
                    false,
                    &[ImageBlock {
                        media_type: "image/png".to_string(),
                        data: "aGk=".to_string(),
                    }],
                ),
            ]),
        ];
        let replaced = replace_images_with_placeholder(&messages);
        assert_eq!(replaced.len(), 1);
        let blocks = &replaced[0].content;
        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[0], ContentBlock::text_block("kick"));
        assert_eq!(
            blocks[1],
            ContentBlock::text_block(IMAGE_REMOVED_PLACEHOLDER),
            "standalone image block becomes placeholder text"
        );
        match &blocks[2] {
            ContentBlock::Known(KnownBlock::ToolResult { content, is_error, .. }) => {
                assert!(!is_error);
                let items = content.as_array().expect("array content preserved");
                assert_eq!(items.len(), 2);
                assert_eq!(
                    items[0],
                    json!({ "type": "text", "text": IMAGE_REMOVED_PLACEHOLDER }),
                    "image entry in the array becomes placeholder text"
                );
                assert_eq!(items[1]["type"], "text");
            }
            other => panic!("expected tool_result block, got {other:?}"),
        }
        // No images anywhere in the rewrite.
        let text = serde_json::to_string(&replaced).unwrap();
        assert!(!text.contains(r#""type":"image""#), "{text}");
    }

    /// Degrade detection is narrow: a 400 whose body mentions image/content
    /// matches; unrelated 400s, other statuses and other errors do not.
    #[test]
    fn image_rejection_detection_is_narrow_to_400_image_content_bodies() {
        let image_err = anyhow::anyhow!(
            "LLM request failed: HTTP 400: {{\"type\":\"error\",\"error\":{{\"type\":\"invalid_request_error\",\"message\":\"Requests must not contain image content blocks\"}}}}"
        );
        assert!(is_image_rejection(&image_err));

        let content_err = anyhow::anyhow!(
            "LLM request failed: HTTP 400: {{\"error\":{{\"message\":\"messages: content field required\"}}}}"
        );
        assert!(is_image_rejection(&content_err));

        // Unrelated 400: no image/content in the body.
        let auth_err = anyhow::anyhow!(
            "LLM request failed: HTTP 400: {{\"error\":{{\"message\":\"max_tokens: field required\"}}}}"
        );
        assert!(!is_image_rejection(&auth_err));

        // Wrong status class.
        let five_xx = anyhow::anyhow!("LLM request failed: HTTP 500: server image trouble");
        assert!(!is_image_rejection(&five_xx));

        // Not an HTTP error at all.
        let conn = anyhow::anyhow!("LLM request failed after 3 attempts; last error: connection error: reset");
        assert!(!is_image_rejection(&conn));
    }
}
