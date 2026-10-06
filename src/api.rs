use std::io::Read;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime};

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// T143: the per-request output-token ceiling sent as `max_tokens` on every
/// Messages API call. Default 32768 — the operator-proven value from the
/// 2026-09-28 glm-5-3-flash session, where the old hardcoded 8192 let GLM
/// thinking blocks (which consume the SAME budget as the response content)
/// push a ~7KB `write_file` JSON past the cap, truncating it mid-stream.
/// Configurable per invocation: `$CHUG_MAX_TOKENS` env, then the
/// `--max-tokens-per-request` CLI flag (resolution in main.rs — flag > env >
/// this default). NOT the T15 cumulative budget (`--max-tokens`), which sums
/// usage across every response of a run.
pub(crate) const DEFAULT_MAX_TOKENS: u32 = 32768;
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

    /// F7 phase 1: like [`Transport::send`], but invokes `on_chunk` with each
    /// raw body chunk as it arrives so the caller can parse incrementally.
    /// Default: ignore the hook and delegate to `send` — existing fakes (and
    /// every non-streaming leg) compile unchanged and behave byte-identically.
    /// The hook fires on the CALLER's thread inside this call (the body reader
    /// hands progress to a channel drained right here), so no cross-thread
    /// synchronization is needed to touch caller state.
    fn send_with_progress(
        &self,
        url: &str,
        headers: &[(String, String)],
        body: &str,
        _on_chunk: &mut dyn FnMut(&[u8]),
    ) -> Result<RawResponse, TransportError> {
        self.send(url, headers, body)
    }
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

    /// F7 phase 1: the streaming leg — same request, same watchdog margins,
    /// but each body chunk is handed to `on_chunk` as it arrives so the SSE
    /// accumulator can emit text deltas while the generation is still running.
    fn send_with_progress(
        &self,
        url: &str,
        headers: &[(String, String)],
        body: &str,
        on_chunk: &mut dyn FnMut(&[u8]),
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
        let body = read_body_with_watchdog_progress(resp, self.activity_timeout, Some(on_chunk))?;
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
    reader: R,
    activity_timeout: Duration,
) -> Result<String, TransportError> {
    read_body_with_watchdog_progress(reader, activity_timeout, None)
}

/// F7 phase 1 variant of [`read_body_with_watchdog`]: `on_chunk`, when
/// present, is invoked with every raw body chunk ON THE CALLER'S THREAD (the
/// `recv_timeout` loop below runs on the thread blocked in this function — no
/// cross-thread synchronization needed). The activity-timeout/retry
/// classification is unchanged: a stalled stream aborts at the SAME activity
/// timeout (T74's margins untouched — the pins call the 2-arg wrapper).
fn read_body_with_watchdog_progress<R: Read + Send + 'static>(
    mut reader: R,
    activity_timeout: Duration,
    mut on_chunk: Option<ChunkHook<'_>>,
) -> Result<String, TransportError> {
    enum BodyMsg {
        Progress(Vec<u8>),
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
                    let bytes = chunk[..n].to_vec();
                    buf.extend_from_slice(&bytes);
                    if tx.send(BodyMsg::Progress(bytes)).is_err() {
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
            Ok(BodyMsg::Progress(bytes)) => {
                if let Some(hook) = on_chunk.as_deref_mut() {
                    hook(&bytes);
                }
            }
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

pub struct Client {
    transport: Arc<dyn Transport>,
    retry_delays: Vec<Duration>,
    base_url: String,
    api_key: Option<String>,
    auth_token: Option<String>,
    model: String,
    /// T143: the per-request output-token cap baked into every request body
    /// (`max_tokens`) and the generation events. Fixed per client; see
    /// [`DEFAULT_MAX_TOKENS`] for the default and the GLM truncation story.
    max_tokens: u32,
    /// F7 phase 1: the live model-text delta hook (see
    /// [`Llm::set_text_delta_hook`]). `RefCell` because `complete` takes
    /// `&self` and the hook fires during the body read.
    text_delta_hook: std::cell::RefCell<Option<TextDeltaHook>>,
    /// F7 phase 1 fallback latch: set on the FIRST proxy downgrade this
    /// session (req 4: one latched telemetry line, no per-response spam);
    /// consumed by the driver via [`Llm::take_stream_fallback`]. Three-state
    /// ([`FallbackLatch`]): after the one line fires, later downgrades never
    /// re-latch.
    fallback_latch: std::cell::Cell<FallbackLatch>,
}

impl Clone for Client {
    fn clone(&self) -> Self {
        Client {
            transport: self.transport.clone(),
            retry_delays: self.retry_delays.clone(),
            base_url: self.base_url.clone(),
            api_key: self.api_key.clone(),
            auth_token: self.auth_token.clone(),
            model: self.model.clone(),
            max_tokens: self.max_tokens,
            // The hook is per-call transient state (the driver arms it around
            // each `complete`); clones start clean.
            text_delta_hook: std::cell::RefCell::new(None),
            fallback_latch: std::cell::Cell::new(FallbackLatch::Fresh),
        }
    }
}

/// F7 phase 1: the driver-installed live model-text hook (see
/// [`Llm::set_text_delta_hook`]).
pub(crate) type TextDeltaHook = Box<dyn FnMut(&str) + Send>;

/// F7 phase 1 fallback-latch state machine (kimi fix-up round: the
/// validator's two-response probe caught the naive `Cell<bool>` re-latching
/// on EVERY downgraded response — per-response `stream_fallback` spam,
/// violating req 4's first-per-run latch).
/// `Fresh` → (first downgrade) → `Pending` → (driver consumes) → `Consumed`;
/// a downgrade never re-latches out of `Consumed`, so exactly ONE
/// `stream_fallback` line is emitted per run no matter how many responses
/// downgrade. A false take on a clean response leaves `Fresh` untouched, so
/// the run's first downgrade still emits its one line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FallbackLatch {
    /// No downgrade yet; the first one latches.
    Fresh,
    /// A downgrade is latched, not yet consumed by the driver.
    Pending,
    /// The one telemetry line has fired; later downgrades stay silent.
    Consumed,
}
/// F7 phase 1: the transport's per-chunk callback (fires on the caller's
/// thread inside the body-read receive loop).
type ChunkHook<'a> = &'a mut dyn FnMut(&[u8]);

/// Why a streamed attempt failed to accumulate.
#[derive(Debug, Clone, PartialEq, Eq)]
enum StreamError {
    /// The server sent an SSE `error` event: T1-RETRYABLE, classified exactly
    /// like a connection failure (req 6).
    Retryable(TransportError),
    /// A malformed payload (unparseable `data:` JSON, unparseable tool_use
    /// input). Parity (named): today's unparseable-2xx-body leg `bail!`s
    /// WITHOUT retrying — so does this.
    Malformed(String),
}

/// F7 phase 1: incremental accumulator for the Anthropic Messages SSE stream.
/// Fed raw body chunks (via the transport's chunk hook when the transport
/// supports it; once over the full body otherwise — both legs tested to
/// identical results), it drives [`crate::sse::SseParser`] and at
/// `message_stop` synthesizes the NON-STREAMING response body shape, so the
/// existing `Response` parse path produces a Response whose
/// `content_blocks()` / `stop_reason()` / `usage()` equal the non-streaming
/// values for the same exchange (usage equality pinned explicitly, incl.
/// cache fields — T15 budget enforcement is untouched).
/// T141: a body that ENDS before its terminal events (`content_block_stop`
/// for every open block, then `message_stop`) is a TRUNCATED response, never
/// a complete one — `finish()` rejects it instead of closing the open block
/// into an executable `tool_use`.
struct StreamAccumulator<'a> {
    parser: crate::sse::SseParser,
    /// Byte carry for UTF-8 boundary safety: a chunk may split a multi-byte
    /// character, so undecodable tails stay here until more bytes arrive.
    carry: Vec<u8>,
    /// `message_start` skeleton fields, surfaced into the synthesized body.
    message_id: Option<String>,
    model: Option<String>,
    /// Synthesized content blocks, in `content_block_start` order.
    blocks: Vec<Value>,
    /// The open block (between `content_block_start` and `content_block_stop`).
    open: Option<OpenBlock>,
    /// T141: the terminal `message_stop` was seen. A well-formed Anthropic
    /// SSE stream always ends with it; its absence at end-of-body means the
    /// response was cut short (proxy truncation, clean EOF).
    message_stop_seen: bool,
    /// `message_delta`'s stop_reason, passed through.
    stop_reason: Option<String>,
    /// `message_start`'s usage object, with `message_delta`'s usage merged
    /// over it (the non-streaming shape: input + final output). T112: the
    /// delta's input-side fields merge too when it carries them — some
    /// proxies defer the real counts (input included) to `message_delta`.
    usage: Value,
    /// First latched error, if any. Later feeds are ignored once latched.
    error: Option<StreamError>,
    /// The [`Llm::set_text_delta_hook`] hook — invoked per `text_delta` as it
    /// arrives (console cosmetics only; never transcript state).
    delta_hook: Option<&'a mut dyn FnMut(&str)>,
}

/// The block currently being accumulated between start/stop events.
enum OpenBlock {
    Text { index: u64, text: String },
    ToolUse {
        index: u64,
        id: String,
        name: String,
        /// Concatenated `input_json_delta` partials, parsed at block stop.
        input_json: String,
    },
    /// Kept for content parity with the non-streaming body (thinking-enabled
    /// models re-serialize thinking blocks verbatim into the transcript).
    Thinking {
        index: u64,
        thinking: String,
        signature: String,
    },
}

impl<'a> StreamAccumulator<'a> {
    fn new(delta_hook: Option<&'a mut dyn FnMut(&str)>) -> Self {
        StreamAccumulator {
            parser: crate::sse::SseParser::new(),
            carry: Vec::new(),
            message_id: None,
            model: None,
            blocks: Vec::new(),
            open: None,
            message_stop_seen: false,
            stop_reason: None,
            usage: Value::Null,
            error: None,
            delta_hook,
        }
    }

    /// Feed raw body bytes. UTF-8-boundary safe: complete characters are
    /// parsed immediately, a split tail is carried for the next chunk.
    fn feed_bytes(&mut self, bytes: &[u8]) {
        if self.error.is_some() {
            return; // latched: a stream that errored stays dead
        }
        self.carry.extend_from_slice(bytes);
        // Decode the longest valid UTF-8 prefix. The prefix is COPIED out so
        // the parser call can take &mut self (the borrow of `carry` must not
        // span it).
        let valid = match std::str::from_utf8(&self.carry) {
            Ok(_) => self.carry.len(),
            Err(e) => e.valid_up_to(),
        };
        if valid > 0 {
            let text = std::str::from_utf8(&self.carry[..valid])
                .expect("validated prefix")
                .to_string();
            self.feed_text(&text);
        }
        self.carry.drain(..valid);
        // A genuinely invalid byte (never a split character): drop it. SSE
        // framing is ASCII; the non-streaming reader's lossy conversion has
        // no meaningful parity here.
        if let Some(len) = std::str::from_utf8(&self.carry)
            .err()
            .and_then(|e| e.error_len())
        {
            self.carry.drain(..len);
        }
    }

    /// Feed decoded text through the SSE parser and apply every event.
    fn feed_text(&mut self, text: &str) {
        if self.error.is_some() {
            return;
        }
        for event in self.parser.feed(text) {
            if let Err(e) = self.apply_event(&event) {
                self.error = Some(e);
                return;
            }
        }
    }

    /// Append one `text_delta` to the open text block, invoking the delta
    /// hook (console cosmetics only) with the raw piece as it arrived.
    fn append_text(&mut self, index: u64, text: &str) -> Result<(), StreamError> {
        let matches =
            matches!(&self.open, Some(OpenBlock::Text { index: i, .. }) if *i == index);
        if !matches {
            return Ok(()); // defensive: a delta with no matching open block
        }
        if let Some(OpenBlock::Text { text: buf, .. }) = &mut self.open {
            buf.push_str(text);
        }
        if let Some(hook) = self.delta_hook.as_deref_mut() {
            hook(text);
        }
        Ok(())
    }

    /// Concatenate one `input_json_delta` partial onto the open tool_use
    /// block; the full input is parsed once at `content_block_stop`.
    fn append_tool_json(&mut self, index: u64, partial: &str) -> Result<(), StreamError> {
        match &mut self.open {
            Some(OpenBlock::ToolUse {
                index: open_idx,
                input_json,
                ..
            }) if *open_idx == index => {
                input_json.push_str(partial);
                Ok(())
            }
            // A delta with no matching open block: ignored (defensive).
            _ => Ok(()),
        }
    }

    fn append_thinking(&mut self, index: u64, piece: &str) -> Result<(), StreamError> {
        match &mut self.open {
            Some(OpenBlock::Thinking {
                index: open_idx,
                thinking,
                ..
            }) if *open_idx == index => {
                thinking.push_str(piece);
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn append_signature(&mut self, index: u64, piece: &str) -> Result<(), StreamError> {
        match &mut self.open {
            Some(OpenBlock::Thinking {
                index: open_idx,
                signature,
                ..
            }) if *open_idx == index => {
                signature.push_str(piece);
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// Finalize the open block (if any) into `blocks`.
    fn close_open_block(&mut self) -> Result<(), StreamError> {
        match self.open.take() {
            None => Ok(()),
            Some(OpenBlock::Text { text, .. }) => {
                self.blocks.push(json!({"type": "text", "text": text}));
                Ok(())
            }
            Some(OpenBlock::ToolUse {
                id,
                name,
                input_json,
                ..
            }) => {
                // Zero-arg tools arrive as an empty partial stream; the
                // non-streaming body carries `input: {}` for the same call.
                let input: Value = if input_json.is_empty() {
                    json!({})
                } else {
                    serde_json::from_str(&input_json).map_err(|e| {
                        StreamError::Malformed(format!(
                            "malformed tool_use input JSON: {e}; partial: {}",
                            preview(&input_json, 200)
                        ))
                    })?
                };
                self.blocks
                    .push(json!({"type": "tool_use", "id": id, "name": name, "input": input}));
                Ok(())
            }
            Some(OpenBlock::Thinking {
                thinking,
                signature,
                ..
            }) => {
                let mut block = json!({"type": "thinking", "thinking": thinking});
                if !signature.is_empty() {
                    block["signature"] = json!(signature);
                }
                self.blocks.push(block);
                Ok(())
            }
        }
    }

    /// Synthesize the NON-STREAMING response body shape from the accumulated
    /// SSE events. Consumes the accumulator: a failed attempt discards it and
    /// no half-accumulated Response ever escapes (req 6 — deltas already
    /// printed are console cosmetics only, never transcript state).
    fn finish(mut self) -> Result<Value, StreamError> {
        if let Some(e) = self.error.take() {
            return Err(e);
        }
        // T141: a body that ends before its terminal events is a TRUNCATED
        // response, not a complete one. Closing the open block here would
        // synthesize a fully-formed `tool_use` out of a half-delivered call
        // (`input: {}` when the stream died right after the block started) —
        // and the driver would EXECUTE it; an unfinished `goal_complete`
        // could end a run without a check. Classified T1-retryable: a real
        // transport surfaces the same mid-body EOF as a connection error,
        // and `message_stop`-less truncation is the same failure shape.
        if let Some(open) = &self.open {
            let (kind, index) = match open {
                OpenBlock::Text { index, .. } => ("text", *index),
                OpenBlock::ToolUse { index, .. } => ("tool_use", *index),
                OpenBlock::Thinking { index, .. } => ("thinking", *index),
            };
            return Err(StreamError::Retryable(TransportError::Connection(
                format!("truncated SSE stream: body ended before content_block_stop (open {kind} block at index {index})"),
            )));
        }
        if !self.message_stop_seen {
            return Err(StreamError::Retryable(TransportError::Connection(
                "truncated SSE stream: body ended before message_stop".to_string(),
            )));
        }
        self.close_open_block()?;
        let mut body = serde_json::Map::new();
        if let Some(id) = &self.message_id {
            body.insert("id".to_string(), json!(id));
        }
        body.insert("type".to_string(), json!("message"));
        body.insert("role".to_string(), json!("assistant"));
        if let Some(model) = &self.model {
            body.insert("model".to_string(), json!(model));
        }
        body.insert("content".to_string(), Value::Array(self.blocks.clone()));
        body.insert(
            "stop_reason".to_string(),
            self.stop_reason
                .clone()
                .map(Value::String)
                .unwrap_or(Value::Null),
        );
        body.insert("stop_sequence".to_string(), Value::Null);
        body.insert("usage".to_string(), self.usage.clone());
        Ok(Value::Object(body))
    }

    /// Apply one parsed SSE event to the accumulator state.
    fn apply_event(&mut self, event: &crate::sse::SseEvent) -> Result<(), StreamError> {
        let data: Value = serde_json::from_str(&event.data).map_err(|e| {
            StreamError::Malformed(format!(
                "malformed SSE data payload: {e}; data: {}",
                preview(&event.data, 200)
            ))
        })?;
        match data.get("type").and_then(Value::as_str).unwrap_or("") {
            "message_start" => {
                let message = data.get("message").cloned().unwrap_or(Value::Null);
                self.message_id = message.get("id").and_then(Value::as_str).map(str::to_string);
                self.model = message
                    .get("model")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                if let Some(usage) = message.get("usage") {
                    self.usage = usage.clone();
                }
                Ok(())
            }
            "content_block_start" => {
                let index = data.get("index").and_then(Value::as_u64).unwrap_or(0);
                let block = data.get("content_block").cloned().unwrap_or(Value::Null);
                match block.get("type").and_then(Value::as_str).unwrap_or("") {
                    "text" => {
                        self.open = Some(OpenBlock::Text {
                            index,
                            text: String::new(),
                        })
                    }
                    "tool_use" => {
                        self.open = Some(OpenBlock::ToolUse {
                            index,
                            id: block
                                .get("id")
                                .and_then(Value::as_str)
                                .unwrap_or_default()
                                .to_string(),
                            name: block
                                .get("name")
                                .and_then(Value::as_str)
                                .unwrap_or_default()
                                .to_string(),
                            input_json: String::new(),
                        });
                    }
                    "thinking" => {
                        self.open = Some(OpenBlock::Thinking {
                            index,
                            thinking: String::new(),
                            signature: String::new(),
                        });
                    }
                    // Unknown block types cannot be reconstructed from their
                    // deltas; the parity pins cover text/tool_use/usage/
                    // stop_reason only.
                    _ => self.open = None,
                }
                Ok(())
            }
            "content_block_delta" => {
                let index = data.get("index").and_then(Value::as_u64).unwrap_or(0);
                let delta = data.get("delta").cloned().unwrap_or(Value::Null);
                match delta.get("type").and_then(Value::as_str).unwrap_or("") {
                    "text_delta" => {
                        let text = delta.get("text").and_then(Value::as_str).unwrap_or("");
                        self.append_text(index, text)
                    }
                    "input_json_delta" => {
                        let partial = delta
                            .get("partial_json")
                            .and_then(Value::as_str)
                            .unwrap_or("");
                        self.append_tool_json(index, partial)
                    }
                    "thinking_delta" => {
                        let t = delta.get("thinking").and_then(Value::as_str).unwrap_or("");
                        self.append_thinking(index, t)
                    }
                    "signature_delta" => {
                        let s = delta.get("signature").and_then(Value::as_str).unwrap_or("");
                        self.append_signature(index, s)
                    }
                    _ => Ok(()),
                }
            }
            "content_block_stop" => self.close_open_block(),
            "message_delta" => {
                let delta = data.get("delta").cloned().unwrap_or(Value::Null);
                if let Some(reason) = delta.get("stop_reason").and_then(Value::as_str) {
                    self.stop_reason = Some(reason.to_string());
                }
                // T112: merge the delta's usage over the `message_start`
                // skeleton. `output_tokens` keeps today's shape — the delta
                // always carries the FINAL count and it overwrites
                // unconditionally. The input-side fields merge only when the
                // delta carries them (delta wins — latest is freshest); on
                // the real Anthropic API `message_delta.usage` carries only
                // `output_tokens`, so that merge is a no-op there and the
                // T108 parity pins stay green. But some proxies send
                // placeholder zeros in `message_start` and the real counts —
                // input included — ONLY in `message_delta`; observed live on
                // the internal LLM proxy endpoint (cycle-61 probe):
                //   message_start: "usage":{"input_tokens":0,"output_tokens":0}
                //   message_delta: "usage":{"input_tokens":257,"output_tokens":1}
                // Discarding that input made every streamed run report
                // `input_tokens: 0` — ~95% of agentic-loop tokens invisible
                // to T15 budget enforcement.
                if let Some(usage) = data.get("usage").and_then(Value::as_object) {
                    if let Some(output) = usage.get("output_tokens").and_then(Value::as_u64) {
                        if self.usage.is_null() {
                            self.usage = json!({"output_tokens": output});
                        } else {
                            self.usage["output_tokens"] = json!(output);
                        }
                    }
                    for field in [
                        "input_tokens",
                        "cache_read_input_tokens",
                        "cache_creation_input_tokens",
                    ] {
                        if let Some(count) = usage.get(field).and_then(Value::as_u64) {
                            if self.usage.is_null() {
                                self.usage = json!({});
                            }
                            self.usage[field] = json!(count);
                        }
                    }
                }
                Ok(())
            }
            "message_stop" => {
                // T141: tracked — its absence at end-of-body means the
                // response was cut short before the message completed.
                self.message_stop_seen = true;
                Ok(())
            }
            "ping" => Ok(()),
            "error" => {
                let err = data.get("error").cloned().unwrap_or(Value::Null);
                let kind = err.get("type").and_then(Value::as_str).unwrap_or("unknown");
                let message = err
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("(no message)");
                Err(StreamError::Retryable(TransportError::Connection(
                    format!("stream error event: {kind}: {message}"),
                )))
            }
            _ => Ok(()),
        }
    }
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
    /// T143: the per-request output-token cap this client sends as
    /// `max_tokens` on every request. Default: [`DEFAULT_MAX_TOKENS`] —
    /// test doubles that never build a request inherit it, so the T38
    /// truncation advisory (which keys off this cap, req 3) behaves exactly
    /// as in production unless a test lowers it deliberately.
    fn max_tokens_per_request(&self) -> u32 {
        DEFAULT_MAX_TOKENS
    }
    /// F7 phase 1: install (`Some`) or clear (`None`) the live model-text
    /// delta hook. The hook receives each text piece as it arrives DURING a
    /// streamed `complete` call, synchronously on the thread that called
    /// `complete` (the body reader's chunk hook fires in its caller-thread
    /// receive loop). Default NO-OP: scripted doubles never stream, and the
    /// driver only arms the hook around each `complete` call.
    fn set_text_delta_hook(&mut self, _hook: Option<TextDeltaHook>) {}
    /// F7 phase 1: whether a `complete` fell back from a streaming request to
    /// a plain JSON response (proxy downgrade, req 4). Read-and-clear, FIRST-
    /// PER-RUN latched in the client: the first downgrade fires once and
    /// consumes the latch — later downgrades never re-fire — so the driver
    /// emits exactly ONE `stream_fallback` telemetry line per run no matter
    /// how many responses downgrade.
    fn take_stream_fallback(&mut self) -> bool {
        false
    }
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
                self.max_tokens,
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

    fn max_tokens_per_request(&self) -> u32 {
        self.max_tokens
    }

    fn set_text_delta_hook(&mut self, hook: Option<TextDeltaHook>) {
        *self.text_delta_hook.borrow_mut() = hook;
    }

    fn take_stream_fallback(&mut self) -> bool {
        // First-per-run latch (req 4): only a PENDING downgrade fires, and
        // firing consumes it — `Consumed` never re-latches. A false take on
        // a clean response leaves `Fresh` untouched, so the run's first
        // downgrade still emits its one line.
        if self.fallback_latch.get() == FallbackLatch::Pending {
            self.fallback_latch.set(FallbackLatch::Consumed);
            true
        } else {
            false
        }
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
#[allow(clippy::too_many_arguments)] // one line per field, sink-generation shape
pub(crate) fn emit_generation(
    sink: &crate::observ::Sink,
    trace_id: &str,
    model: &str,
    max_tokens: u32,
    resp: &Response,
    start: SystemTime,
    end: SystemTime,
    iteration: u32,
) {
    sink.generation(
        trace_id,
        model,
        max_tokens,
        &resp.usage(),
        start,
        end,
        iteration,
        resp.stop_reason().as_deref(),
    );
}

impl Client {
    pub fn new(model: &str, max_tokens_per_request: u32) -> anyhow::Result<Self> {
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
            max_tokens: max_tokens_per_request,
            text_delta_hook: std::cell::RefCell::new(None),
            fallback_latch: std::cell::Cell::new(FallbackLatch::Fresh),
        })
    }

    /// Test-only constructor that skips credential checks (no network is ever
    /// attempted by the constructor itself).
    #[cfg(test)]
    pub fn new_without_credentials(model: &str, max_tokens_per_request: u32) -> anyhow::Result<Self> {
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
            max_tokens: max_tokens_per_request,
            text_delta_hook: std::cell::RefCell::new(None),
            fallback_latch: std::cell::Cell::new(FallbackLatch::Fresh),
        })
    }

    /// Test-only constructor wiring a caller-built transport with zero retry
    /// delays, so driver tests can run the FULL `run_loop` (T55 lock
    /// acquire/release legs) to goal-acceptance without network.
    #[cfg(test)]
    pub(crate) fn with_transport_for_tests(
        transport: Arc<dyn Transport>,
        model: &str,
        max_tokens_per_request: u32,
    ) -> Self {
        Self {
            transport,
            retry_delays: Vec::new(),
            base_url: "http://fake.local".to_string(),
            api_key: None,
            auth_token: None,
            model: model.to_string(),
            max_tokens: max_tokens_per_request,
            text_delta_hook: std::cell::RefCell::new(None),
            fallback_latch: std::cell::Cell::new(FallbackLatch::Fresh),
        }
    }

    /// Swap the model id used by subsequent `complete` calls (chat `/model`).
    pub fn set_model(&mut self, model: &str) {
        self.model = model.to_string();
    }

    /// One Messages API call with T1 retry semantics: connection failures and
    /// the gateway/overload statuses retry with exponential backoff (honoring
    /// `retry-after`, capped); other HTTP errors fail fast.
    ///
    /// F7 phase 1: the request carries `"stream": true` by default (kill
    /// switch `CHUG_STREAM=0` restores the byte-identical non-streaming
    /// request body AND parse path — both legs pinned). On the streaming leg
    /// each body chunk feeds a fresh [`StreamAccumulator`], which emits text
    /// deltas to the installed hook as they arrive and synthesizes the
    /// non-streaming body shape at the end; a proxy downgrade (2xx body whose
    /// content-type is not `text/event-stream`) parses byte-identically to
    /// today and latches ONE telemetry note.
    pub fn complete(
        &self,
        system: &str,
        messages: &[Message],
        tools: &[Value],
    ) -> anyhow::Result<Response> {
        let url = format!("{}/v1/messages", self.base_url.trim_end_matches('/'));
        let streaming = streaming_enabled();
        let mut body = json!({
            "model": self.model,
            "max_tokens": self.max_tokens,
            "system": system,
            "messages": messages,
            "tools": tools,
            "tool_choice": { "type": "auto" },
        });
        if streaming {
            body["stream"] = json!(true);
        }
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
            // F7 phase 1: a FRESH accumulator per attempt (req 6: a failed
            // attempt discards it — no half-accumulated Response ever escapes;
            // deltas already printed are console cosmetics only).
            let mut hook_borrow = streaming.then(|| self.text_delta_hook.borrow_mut());
            // RefMut -> Option -> (auto-deref chain) -> &mut dyn FnMut(&str).
            // The accumulator exists whenever STREAMING — hook or not (an
            // unarmed hook only means no delta cosmetics; the SSE parse leg
            // still runs).
            let mut acc = match hook_borrow.as_mut().and_then(|hook| hook.as_deref_mut()) {
                Some(hook_dyn) => {
                    let hook: &mut dyn FnMut(&str) = hook_dyn;
                    Some(StreamAccumulator::new(Some(hook)))
                }
                None if streaming => Some(StreamAccumulator::new(None)),
                None => None,
            };
            let send_result = match acc.as_mut() {
                Some(acc) => {
                    let mut on_chunk = |bytes: &[u8]| acc.feed_bytes(bytes);
                    self.transport
                        .send_with_progress(&url, &headers, &body, &mut on_chunk)
                }
                None => self.transport.send(&url, &headers, &body),
            };
            match send_result {
                Ok(raw) => {
                    if (200..300).contains(&raw.status) {
                        if let Some(acc) = acc {
                            let content_type = raw
                                .header("content-type")
                                .unwrap_or("")
                                .to_ascii_lowercase();
                            if content_type.contains("text/event-stream") {
                                // SSE leg: synthesize the non-streaming body
                                // shape. An SSE `error` event retries like a
                                // connection failure (T1); a malformed payload
                                // fails fast, exactly like today's
                                // unparseable-body leg.
                                return match acc.finish() {
                                    Ok(parsed) => Ok(Response { body: parsed }),
                                    Err(StreamError::Malformed(msg)) => {
                                        bail!("LLM request failed: {msg}")
                                    }
                                    Err(StreamError::Retryable(TransportError::Connection(
                                        msg,
                                    ))) => {
                                        last_error = format!("connection error: {msg}");
                                        if attempt + 1 == total_attempts {
                                            break;
                                        }
                                        thread::sleep(self.retry_delays[attempt]);
                                        continue;
                                    }
                                    Err(StreamError::Retryable(TransportError::Fatal(msg))) => {
                                        bail!("LLM request failed: {msg}")
                                    }
                                };
                            }
                            // Fallback leg (req 4): streaming requested, but the
                            // response is not SSE (proxy downgrade) — parse
                            // byte-identically to today and latch ONE note PER
                            // RUN: only a Fresh client latches, so a second
                            // downgrade (Pending or Consumed) stays silent.
                            if self.fallback_latch.get() == FallbackLatch::Fresh {
                                self.fallback_latch.set(FallbackLatch::Pending);
                            }
                        }
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

/// F7 phase 1 kill switch: `CHUG_STREAM=0` (process env) restores the
/// byte-identical non-streaming request body AND parse path — both legs
/// pinned against a request-body-capturing fake transport. Any other value
/// (unset included) streams.
fn streaming_enabled() -> bool {
    std::env::var("CHUG_STREAM").ok().as_deref() != Some("0")
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
    /// T143: the per-request cap this double reports. Defaults to
    /// [`DEFAULT_MAX_TOKENS`] (production parity — the T38 remedy line stays
    /// silent at/above 32768); truncation-remedy tests lower it to reproduce
    /// the GLM 8192 shape.
    pub max_tokens_per_request: u32,
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
            max_tokens_per_request: DEFAULT_MAX_TOKENS,
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

    fn max_tokens_per_request(&self) -> u32 {
        self.max_tokens_per_request
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

    /// T143: the per-request cap default is value-pinned to the
    /// operator-proven 32768 (2026-09-28 glm-5-3-flash session: 8192 let a
    /// thinking block plus a ~7KB write_file truncate the tool-call JSON).
    /// A const-only edit must fail here.
    #[test]
    fn max_tokens_default_is_value_pinned() {
        assert_eq!(
            DEFAULT_MAX_TOKENS, 32768u32,
            "DEFAULT_MAX_TOKENS drifted from the operator-proven 32768"
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
            DEFAULT_MAX_TOKENS,
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
        assert_eq!(event["body"]["modelParameters"]["maxTokens"], DEFAULT_MAX_TOKENS);
        assert_eq!(event["body"]["usage"]["input"], 10);
        assert_eq!(event["body"]["usage"]["output"], 5);
        assert_eq!(event["body"]["usage"]["total"], 15);
        assert_eq!(event["body"]["usage"]["cache_read_input_tokens"], 3);
        assert_eq!(event["body"]["metadata"]["iteration"], 7);
        assert_eq!(event["body"]["metadata"]["stop_reason"], "tool_use");
    }

    #[test]
    fn set_model_swaps_the_model_id() {
        let mut client =
            Client::new_without_credentials("first-model", DEFAULT_MAX_TOKENS).unwrap();
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
            max_tokens: DEFAULT_MAX_TOKENS,
            text_delta_hook: std::cell::RefCell::new(None),
            fallback_latch: std::cell::Cell::new(FallbackLatch::Fresh),
        }
    }

    fn run_complete(client: &Client) -> anyhow::Result<Response> {
        client.complete(
            "sys",
            &[Message::user(vec![ContentBlock::text_block("hi")])],
            &[],
        )
    }

    /// Test transport that records every request body handed to it — the
    /// no-network seam for request-shape assertions (T143).
    struct BodyCapturingTransport {
        responses: std::sync::Mutex<std::collections::VecDeque<Result<RawResponse, TransportError>>>,
        bodies: std::sync::Mutex<Vec<String>>,
    }

    impl BodyCapturingTransport {
        fn new(responses: Vec<Result<RawResponse, TransportError>>) -> Arc<Self> {
            Arc::new(BodyCapturingTransport {
                responses: std::sync::Mutex::new(responses.into()),
                bodies: std::sync::Mutex::new(Vec::new()),
            })
        }

        fn bodies(&self) -> Vec<String> {
            self.bodies.lock().unwrap().clone()
        }
    }

    impl Transport for BodyCapturingTransport {
        fn send(
            &self,
            _url: &str,
            _headers: &[(String, String)],
            body: &str,
        ) -> Result<RawResponse, TransportError> {
            self.bodies.lock().unwrap().push(body.to_string());
            self.responses
                .lock()
                .unwrap()
                .pop_front()
                .expect("capturing transport script exhausted")
        }
    }

    /// T143: the per-request `max_tokens` cap lands in the request body.
    /// RED against the old hardcoded 8192: GLM thinking blocks consume the
    /// SAME budget as the response content, so a ~7KB `write_file` plus a
    /// thinking block overflowed the 8192 cap and truncated the tool-call
    /// JSON mid-stream (operator session 2026-09-28, glm child died at
    /// iteration 9 on an unparseable write_file). The default is the
    /// operator-proven 32768; a configured value (env/CLI) replaces it.
    #[test]
    fn request_body_carries_the_configured_max_tokens() {
        let transport = BodyCapturingTransport::new(vec![ok_raw(json!({
            "stop_reason": "end_turn",
            "usage": {"input_tokens": 1, "output_tokens": 1},
            "content": [{"type": "text", "text": "ok"}],
        }))]);

        // Default: the operator-proven 32768.
        let client = Client::with_transport_for_tests(
            transport.clone(),
            "test-model",
            DEFAULT_MAX_TOKENS,
        );
        run_complete(&client).unwrap();
        let bodies = transport.bodies();
        assert_eq!(bodies.len(), 1);
        let body: Value = serde_json::from_str(&bodies[0]).unwrap();
        assert_eq!(
            body["max_tokens"], 32768,
            "the old hardcoded 8192 truncated GLM thinking + ~7KB writes"
        );

        // A configured cap (what main.rs resolves from flag/env) replaces the
        // default in the request body.
        let transport2 = BodyCapturingTransport::new(vec![ok_raw(json!({
            "stop_reason": "end_turn",
            "usage": {"input_tokens": 1, "output_tokens": 1},
            "content": [{"type": "text", "text": "ok"}],
        }))]);
        let client2 =
            Client::with_transport_for_tests(transport2.clone(), "test-model", 4096);
        run_complete(&client2).unwrap();
        let body2: Value = serde_json::from_str(&transport2.bodies()[0]).unwrap();
        assert_eq!(body2["max_tokens"], 4096);
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

    // ---- F7 phase 1: streaming responses + text deltas ----

    /// Build an SSE stream body from typed events (`event:` + `data:` lines).
    fn sse_stream(events: &[Value]) -> String {
        let mut out = String::new();
        for e in events {
            out.push_str(&format!(
                "event: {}\ndata: {}\n\n",
                e["type"].as_str().unwrap_or(""),
                e
            ));
        }
        out
    }

    fn msg_start() -> Value {
        json!({
            "type": "message_start",
            "message": {
                "id": "msg_1", "type": "message", "role": "assistant",
                "model": "test-model", "content": [],
                "usage": {"input_tokens": 100, "output_tokens": 1,
                          "cache_read_input_tokens": 64},
            }
        })
    }

    /// A transport that records every request body and, per scripted response,
    /// either feeds SSE chunks through `send_with_progress` or returns a plain
    /// (non-SSE) body — the harness for both request legs + the fallback leg.
    struct StreamingFake {
        /// One entry per scripted response: SSE chunks (fed via the progress
        /// hook) or a plain body. `Err` entries script transport failures.
        responses: std::sync::Mutex<std::collections::VecDeque<Result<Vec<String>, TransportError>>>,
        /// Response headers for the NEXT 2xx (content-type controls the leg).
        content_type: String,
        /// Per-response content-type overrides (FIFO, one per scripted 2xx);
        /// when exhausted, `content_type` applies. Lets one fake script MIXED
        /// legs (SSE response, then downgraded JSON responses).
        content_type_overrides: std::sync::Mutex<std::collections::VecDeque<String>>,
        bodies: std::sync::Mutex<Vec<String>>,
        calls: std::sync::Mutex<usize>,
    }

    /// One scripted 2xx: its chunks plus (optionally) its content-type.
    type Scripted2xx = (Option<&'static str>, Vec<String>);

    impl StreamingFake {
        fn sse(chunks: Vec<String>) -> Arc<Self> {
            Arc::new(StreamingFake {
                responses: std::sync::Mutex::new(vec![Ok(chunks)].into()),
                content_type: "text/event-stream".to_string(),
                content_type_overrides: std::sync::Mutex::new(Default::default()),
                bodies: std::sync::Mutex::new(Vec::new()),
                calls: std::sync::Mutex::new(0),
            })
        }

        fn plain(body: Value) -> Arc<Self> {
            Arc::new(StreamingFake {
                responses: std::sync::Mutex::new(vec![Ok(vec![body.to_string()])].into()),
                content_type: "application/json".to_string(),
                content_type_overrides: std::sync::Mutex::new(Default::default()),
                bodies: std::sync::Mutex::new(Vec::new()),
                calls: std::sync::Mutex::new(0),
            })
        }

        /// A multi-response fake with per-response content types: one entry
        /// per scripted 2xx, `Some(ct)` overriding the default for that
        /// response (mixed SSE/JSON legs — the fallback-latch cardinality
        /// probes).
        fn scripted(script: Vec<Scripted2xx>, default_ct: &str) -> Arc<Self> {
            let (responses, overrides): (Vec<_>, Vec<_>) = script
                .into_iter()
                .map(|(ct, chunks)| {
                    (
                        Ok(chunks),
                        ct.map(str::to_string).unwrap_or_else(|| default_ct.to_string()),
                    )
                })
                .unzip();
            Arc::new(StreamingFake {
                responses: std::sync::Mutex::new(responses.into()),
                content_type: default_ct.to_string(),
                content_type_overrides: std::sync::Mutex::new(overrides.into()),
                bodies: std::sync::Mutex::new(Vec::new()),
                calls: std::sync::Mutex::new(0),
            })
        }

        fn bodies(&self) -> Vec<String> {
            self.bodies.lock().unwrap().clone()
        }

        fn calls(&self) -> usize {
            *self.calls.lock().unwrap()
        }
        /// The content-type header for the next scripted 2xx: a per-response
        /// override if one is queued, else the fake's default.
        fn next_content_type(&self) -> String {
            self.content_type_overrides
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_else(|| self.content_type.clone())
        }
    }

    impl Transport for StreamingFake {
        fn send(
            &self,
            _url: &str,
            _headers: &[(String, String)],
            body: &str,
        ) -> Result<RawResponse, TransportError> {
            *self.calls.lock().unwrap() += 1;
            self.bodies.lock().unwrap().push(body.to_string());
            match self
                .responses
                .lock()
                .unwrap()
                .pop_front()
                .expect("script exhausted")
            {
                Ok(chunks) => Ok(RawResponse {
                    status: 200,
                    headers: vec![("content-type".to_string(), self.next_content_type())],
                    body: chunks.concat(),
                }),
                Err(e) => Err(e),
            }
        }

        fn send_with_progress(
            &self,
            url: &str,
            headers: &[(String, String)],
            body: &str,
            on_chunk: &mut dyn FnMut(&[u8]),
        ) -> Result<RawResponse, TransportError> {
            // Same request, chunk-fed: the harness for the live leg.
            *self.calls.lock().unwrap() += 1;
            self.bodies.lock().unwrap().push(body.to_string());
            match self
                .responses
                .lock()
                .unwrap()
                .pop_front()
                .expect("script exhausted")
            {
                Ok(chunks) => {
                    for c in &chunks {
                        on_chunk(c.as_bytes());
                    }
                    Ok(RawResponse {
                        status: 200,
                        headers: vec![("content-type".to_string(), self.next_content_type())],
                        body: chunks.concat(),
                    })
                }
                Err(e) => Err(e),
            }
            .inspect(|raw| {
                let _ = (url, headers);
                let _ = raw;
            })
        }
    }

    fn run(client: &Client) -> anyhow::Result<Response> {
        client.complete(
            "sys",
            &[Message::user(vec![ContentBlock::text_block("hi")])],
            &[],
        )
    }


    /// Serializes every test that calls `complete()` against the
    /// CHUG_STREAM kill-switch flip (the env is process-global; the flip
    /// window must not overlap another streaming-leg test).
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    // ---- F7 phase 1: accumulator unit tests ----

    fn acc_feed_all(acc: &mut StreamAccumulator<'_>, chunks: &[&str]) {
        for c in chunks {
            acc.feed_bytes(c.as_bytes());
        }
    }

    #[test]
    fn accumulator_text_only_multi_chunk_with_deltas_in_order() {
        let mut deltas: Vec<String> = Vec::new();
        {
            let mut hook = |d: &str| deltas.push(d.to_string());
            let mut acc = StreamAccumulator::new(Some(&mut hook));
            acc_feed_all(
                &mut acc,
                &[
                    &sse_stream(&[msg_start()]),
                    "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n",
                    "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hel\"}}\n\nevent: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"lo w\"}}\n\n",
                    "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"orld\"}}\n\n",
                    "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\nevent: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":5}}\n\nevent: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
                ],
            );
            let body = acc.finish().unwrap();
            assert_eq!(deltas, ["Hel", "lo w", "orld"]);
            assert_eq!(body["content"], json!([{"type": "text", "text": "Hello world"}]));
            assert_eq!(body["stop_reason"], "end_turn");
            assert_eq!(body["role"], "assistant");
        }
        // A latched error survives to finish(): the half-accumulated body is
        // never unwrapped into a Response (req 6) — the caller discards it.
        let mut acc = StreamAccumulator::new(None);
        acc.feed_bytes(
            b"event: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"slow down\"}}\n\n",
        );
        let err = acc.finish().unwrap_err();
        assert!(matches!(
            err,
            StreamError::Retryable(TransportError::Connection(_))
        ));
    }

    #[test]
    fn accumulator_tool_use_via_split_input_json_partials() {
        let mut acc = StreamAccumulator::new(None);
        acc_feed_all(
            &mut acc,
            &[
                &sse_stream(&[json!({
                    "type": "message_start",
                    "message": {"id": "msg_2", "model": "m", "usage": {"input_tokens": 7, "output_tokens": 1}}
                })]),
                "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"tool_use\",\"id\":\"tu_1\",\"name\":\"bash\",\"input\":{}}}\n\n",
                // The tool input JSON split MID-OBJECT across partials.
                "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"command\\\":\\\"ls\\\",\"}}\n\n",
                "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"\\\"flag\\\":true}\"}}\n\n",
                "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
                "event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"tool_use\"},\"usage\":{\"output_tokens\":9}}\n\nevent: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
            ],
        );
        let body = acc.finish().unwrap();
        let resp = Response { body };
        let blocks = resp.content_blocks();
        let uses: Vec<_> = blocks.iter().filter_map(ContentBlock::tool_use).collect();
        assert_eq!(uses.len(), 1);
        assert_eq!(uses[0].0, "tu_1");
        assert_eq!(uses[0].1, "bash");
        assert_eq!(uses[0].2, &json!({"command": "ls", "flag": true}));
        assert_eq!(resp.stop_reason().as_deref(), Some("tool_use"));
    }

    #[test]
    fn accumulator_text_plus_tool_interleaved_and_thinking_kept() {
        let mut acc = StreamAccumulator::new(None);
        acc_feed_all(
            &mut acc,
            &[
                &sse_stream(&[msg_start()]),
                "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"thinking\",\"thinking\":\"\"}}\n\n",
                "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"look\"}}\n\n",
                "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"signature_delta\",\"signature\":\"sig1\"}}\n\n",
                "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
                "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":1,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n",
                "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"text_delta\",\"text\":\"running ls\"}}\n\n",
                "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":1}\n\n",
                "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":2,\"content_block\":{\"type\":\"tool_use\",\"id\":\"tu_9\",\"name\":\"bash\",\"input\":{}}}\n\n",
                "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":2,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"command\\\":\\\"ls\\\"}\"}}\n\n",
                "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":2}\n\n",
                "event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"tool_use\"},\"usage\":{\"output_tokens\":12}}\n\nevent: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
            ],
        );
        let body = acc.finish().unwrap();
        let resp = Response { body };
        let blocks = resp.content_blocks();
        assert_eq!(blocks.len(), 3);
        match &blocks[0] {
            ContentBlock::Known(KnownBlock::Thinking { thinking, signature }) => {
                assert_eq!(thinking, "look");
                assert_eq!(signature.as_deref(), Some("sig1"));
            }
            other => panic!("expected thinking block, got {other:?}"),
        }
        assert_eq!(blocks[1].text().map(str::to_string), Some("running ls".to_string()));
        assert_eq!(resp.text(), "running ls");
    }

    /// T15 budget enforcement rides `usage()` — the streaming synthesis must
    /// equal the non-streaming values for the same exchange, cache fields
    /// included.
    #[test]
    fn accumulator_usage_matches_non_streaming_incl_cache_fields() {
        let mut acc = StreamAccumulator::new(None);
        acc_feed_all(
            &mut acc,
            &[
                &sse_stream(&[msg_start()]),
                "event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":20}}\n\nevent: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
            ],
        );
        let resp = Response { body: acc.finish().unwrap() };
        assert_eq!(
            resp.usage(),
            crate::observ::Usage {
                input: 100,
                output: 20,
                total: 120,
                cache_read_input_tokens: Some(64),
            }
        );
        assert_eq!(resp.stop_reason().as_deref(), Some("end_turn"));
    }

    /// T112 regression pin — the EXACT observed production shape
    /// (the internal LLM proxy endpoint, cycle-61 live probe): the proxy
    /// sends placeholder zeros in `message_start` and the real counts — input
    /// included — only in `message_delta`. The delta's usage must merge over
    /// the skeleton so `usage()` reports the real run totals (pre-fix every
    /// streamed run read `input_tokens: 0`, ~95% of agentic-loop tokens
    /// invisible to T15 budget enforcement).
    #[test]
    fn accumulator_message_delta_usage_merges_input_over_zero_skeleton() {
        let mut acc = StreamAccumulator::new(None);
        acc_feed_all(
            &mut acc,
            &[
                &sse_stream(&[json!({
                    "type": "message_start",
                    "message": {"id": "msg_1", "model": "m",
                                "usage": {"input_tokens": 0, "output_tokens": 0}}
                })]),
                "event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"input_tokens\":257,\"output_tokens\":1}}\n\nevent: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
            ],
        );
        let resp = Response { body: acc.finish().unwrap() };
        let usage = resp.usage();
        assert_eq!(usage.input, 257);
        assert_eq!(usage.output, 1);
        assert_eq!(usage.total, 258);
    }

    /// T112: a `message_delta` carrying the input-side cache fields merges
    /// them over the `message_start` skeleton (delta wins — latest is
    /// freshest).
    #[test]
    fn accumulator_message_delta_usage_merges_cache_fields() {
        let mut acc = StreamAccumulator::new(None);
        acc_feed_all(
            &mut acc,
            &[
                &sse_stream(&[json!({
                    "type": "message_start",
                    "message": {"id": "msg_1", "model": "m",
                                "usage": {"input_tokens": 100, "output_tokens": 1}}
                })]),
                "event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"input_tokens\":180,\"output_tokens\":20,\"cache_read_input_tokens\":64,\"cache_creation_input_tokens\":7}}\n\nevent: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
            ],
        );
        let body = acc.finish().unwrap();
        let usage = Response { body: body.clone() }.usage();
        assert_eq!(usage.input, 180);
        assert_eq!(usage.output, 20);
        assert_eq!(usage.total, 200);
        assert_eq!(usage.cache_read_input_tokens, Some(64));
        // `cache_creation_input_tokens` rides the synthesized body's usage
        // object (observ::Usage has no slot for it — same as non-streaming).
        assert_eq!(body["usage"]["cache_creation_input_tokens"], 7);
    }

    /// T112 no-op leg — today's real-API shape: `message_start` carries the
    /// real input count and `message_delta.usage` carries ONLY
    /// `output_tokens`. The input-side merge must be a no-op there, keeping
    /// the `message_start` input (T108 parity pins stay green untouched).
    #[test]
    fn accumulator_message_delta_without_input_fields_keeps_message_start_usage() {
        let mut acc = StreamAccumulator::new(None);
        acc_feed_all(
            &mut acc,
            &[
                &sse_stream(&[msg_start()]),
                "event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":20}}\n\nevent: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
            ],
        );
        let resp = Response { body: acc.finish().unwrap() };
        assert_eq!(
            resp.usage(),
            crate::observ::Usage {
                input: 100,
                output: 20,
                total: 120,
                cache_read_input_tokens: Some(64),
            }
        );
    }

    /// The sse.rs parser-feed_split_across_chunks pattern, at full strength:
    /// the SAME fixture split at EVERY byte produces the identical body.
    #[test]
    fn accumulator_chunk_boundary_splits_at_every_byte() {
        let fixture = sse_stream(&[
            msg_start(),
            json!({"type": "content_block_start", "index": 0, "content_block": {"type": "text", "text": ""}}),
            json!({"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": "héllo — wörld"}}),
            json!({"type": "content_block_stop", "index": 0}),
            json!({"type": "message_delta", "delta": {"stop_reason": "end_turn"}, "usage": {"output_tokens": 3}}),
            json!({"type": "message_stop"}),
        ]);
        let single = {
            let mut acc = StreamAccumulator::new(None);
            acc.feed_bytes(fixture.as_bytes());
            acc.finish().unwrap()
        };
        let bytes = fixture.as_bytes();
        for split in 0..=bytes.len() {
            let mut acc = StreamAccumulator::new(None);
            acc.feed_bytes(&bytes[..split]);
            acc.feed_bytes(&bytes[split..]);
            assert_eq!(acc.finish().unwrap(), single, "split at byte {split}");
        }
    }

    /// T141 (codex review, HIGH — truncated SSE accepted): a clean HTTP EOF
    /// BEFORE `content_block_stop` is a truncated response, not a complete
    /// one. `finish()` used to close the open block for it — a stream cut
    /// right after a tool block started synthesized a fully-formed
    /// `tool_use` (`input: {}` when no deltas arrived), and the driver would
    /// execute it; an unfinished `goal_complete` could end a run without a
    /// check. The truncated stream must ERROR, never synthesize a body.
    #[test]
    fn accumulator_truncated_mid_tool_block_is_rejected() {
        let mut acc = StreamAccumulator::new(None);
        acc_feed_all(
            &mut acc,
            &[
                &sse_stream(&[msg_start()]),
                // The tool block opens and receives a SYNTACTICALLY COMPLETE
                // input JSON — then the HTTP body ends. No content_block_stop.
                "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"tool_use\",\"id\":\"tu_1\",\"name\":\"goal_complete\",\"input\":{}}}\n\n",
                "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"summary\\\":\\\"all checks green\\\"}\"}}\n\n",
            ],
        );
        let err = acc.finish().unwrap_err();
        assert!(
            matches!(err, StreamError::Retryable(TransportError::Connection(ref m)) if m.contains("truncated")),
            "{err:?}"
        );
    }

    /// T147 (T141-M1 kill): the open-block check is the ONLY rejector for a
    /// stream that opens a tool_use block, delivers a VALID but partial
    /// input, and then ends with `message_stop` WITHOUT `content_block_stop`.
    /// The `message_stop` guard alone accepts this shape (the terminal event
    /// arrived; the old close-on-message_stop path parses the partial JSON
    /// fine and synthesizes an executable `goal_complete`) — only the
    /// open-block check rejects it. Removing that check must flip exactly
    /// this test RED (every other truncation test ends WITHOUT message_stop
    /// and stays green under the mutant — the M1 mask).
    #[test]
    fn accumulator_open_tool_block_with_message_stop_is_rejected() {
        let mut acc = StreamAccumulator::new(None);
        acc_feed_all(
            &mut acc,
            &[&sse_stream(&[
                msg_start(),
                json!({"type": "content_block_start", "index": 0, "content_block": {"type": "tool_use", "id": "tu_goal", "name": "goal_complete", "input": {}}}),
                // A VALID but partial input: the proxy cut the stream after
                // the first delta — a block closed here would parse fine and
                // execute.
                json!({"type": "content_block_delta", "index": 0, "delta": {"type": "input_json_delta", "partial_json": "{\"summary\":\"all checks green\"}"}}),
                json!({"type": "message_delta", "delta": {"stop_reason": "tool_use"}, "usage": {"output_tokens": 9}}),
                // The terminal event arrives; the block was never closed.
                json!({"type": "message_stop"}),
            ])],
        );
        let err = acc.finish().unwrap_err();
        assert!(
            matches!(err, StreamError::Retryable(TransportError::Connection(ref m))
                if m.contains("truncated") && m.contains("content_block_stop") && m.contains("tool_use")),
            "{err:?}"
        );
    }

    /// T141 (codex review, HIGH): `message_stop` is tracked, not ignored. A
    /// stream that ended after every block closed — `message_delta` even
    /// carried a stop_reason — but before the terminal `message_stop` is
    /// still a truncated response and must not synthesize a success body.
    #[test]
    fn accumulator_truncated_before_message_stop_is_rejected() {
        let mut acc = StreamAccumulator::new(None);
        acc_feed_all(
            &mut acc,
            &[&sse_stream(&[
                msg_start(),
                json!({"type": "content_block_start", "index": 0, "content_block": {"type": "text", "text": ""}}),
                json!({"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": "done"}}),
                json!({"type": "content_block_stop", "index": 0}),
                json!({"type": "message_delta", "delta": {"stop_reason": "end_turn"}, "usage": {"output_tokens": 2}}),
                // EOF here: message_stop never arrived.
            ])],
        );
        let err = acc.finish().unwrap_err();
        assert!(
            matches!(err, StreamError::Retryable(TransportError::Connection(ref m)) if m.contains("truncated")),
            "{err:?}"
        );
    }

    /// T141: an SSE 2xx body with no events at all (or only `message_start`)
    /// is truncated, not an empty-but-valid assistant message.
    #[test]
    fn accumulator_empty_sse_body_is_rejected() {
        let mut acc = StreamAccumulator::new(None);
        acc.feed_bytes(b"");
        assert!(acc.finish().is_err());
    }

    // ---- T141: the review's trigger, end to end through the client ----

    /// The review's exact trigger: a proxy answers HTTP 200 `text/event-stream`
    /// carrying a tool block whose input JSON is syntactically complete, then
    /// ends the HTTP body before `content_block_stop`/`message_stop`. The
    /// client must return an ERROR — never a Response whose tool_use blocks
    /// the driver would execute (an unfinished `goal_complete` must not
    /// terminate a run without a check).
    #[test]
    fn truncated_sse_tool_block_is_rejected_not_executed() {
        let _env = ENV_LOCK.lock().unwrap();
        let body = sse_stream(&[
            msg_start(),
            json!({"type": "content_block_start", "index": 0, "content_block": {"type": "tool_use", "id": "tu_goal", "name": "goal_complete", "input": {}}}),
            json!({"type": "content_block_delta", "index": 0, "delta": {"type": "input_json_delta", "partial_json": "{\"summary\":\"all checks green\"}"}}),
            // The HTTP body ends here: no content_block_stop, no message_stop.
        ]);
        let ft = StreamingFake::sse(vec![body]);
        let client = client_with(ft.clone(), &[]);
        let err = run(&client).unwrap_err();
        assert!(err.to_string().contains("truncated"), "{err}");
    }

    /// T141 + T1 parity: a mid-body EOF is the connection-failure class (a
    /// real transport surfaces the same truncation as a connection error), so
    /// it consumes the retry schedule instead of failing the iteration fast.
    #[test]
    fn truncated_sse_retries_like_a_connection_failure() {
        let _env = ENV_LOCK.lock().unwrap();
        let truncated = sse_stream(&[
            msg_start(),
            json!({"type": "content_block_start", "index": 0, "content_block": {"type": "text", "text": ""}}),
            json!({"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": "partial"}}),
            json!({"type": "content_block_stop", "index": 0}),
            // EOF before message_stop — twice: the retry re-truncates.
        ]);
        let ft = Arc::new(StreamingFake {
            responses: std::sync::Mutex::new(
                vec![Ok(vec![truncated.clone()]), Ok(vec![truncated])].into(),
            ),
            content_type: "text/event-stream".to_string(),
            content_type_overrides: std::sync::Mutex::new(Default::default()),
            bodies: std::sync::Mutex::new(Vec::new()),
            calls: std::sync::Mutex::new(0),
        });
        let client = client_with(ft.clone(), &[0]);
        let err = run(&client).unwrap_err();
        assert!(err.to_string().contains("truncated"), "{err}");
        assert_eq!(ft.calls(), 2, "the truncation retried once, then gave up");
    }

    #[test]
    fn sse_error_event_is_t1_retryable_then_success() {
        let _env = ENV_LOCK.lock().unwrap();
        let error_stream = sse_stream(&[json!({
            "type": "error",
            "error": {"type": "overloaded_error", "message": "overloaded"}
        })]);
        let ok_stream = sse_stream(&[
            msg_start(),
            json!({"type": "content_block_start", "index": 0, "content_block": {"type": "text", "text": ""}}),
            json!({"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": "ok"}}),
            json!({"type": "content_block_stop", "index": 0}),
            json!({"type": "message_delta", "delta": {"stop_reason": "end_turn"}, "usage": {"output_tokens": 1}}),
            json!({"type": "message_stop"}),
        ]);
        let ft = Arc::new(StreamingFake {
            responses: std::sync::Mutex::new(
                vec![Ok(vec![error_stream]), Ok(vec![ok_stream])].into(),
            ),
            content_type: "text/event-stream".to_string(),
            content_type_overrides: std::sync::Mutex::new(Default::default()),
            bodies: std::sync::Mutex::new(Vec::new()),
            calls: std::sync::Mutex::new(0),
        });
        let client = client_with(ft.clone(), &[0]);
        let resp = run(&client).expect("the SSE error event retries (T1) then succeeds");
        assert_eq!(resp.text(), "ok");
        assert_eq!(ft.calls(), 2);
    }

    /// Req 4: streaming requested, plain JSON + `application/json` answered —
    /// byte-identical parse, and ONE latched fallback note (take semantics).
    #[test]
    fn fallback_leg_parses_identically_and_latches_once() {
        let _env = ENV_LOCK.lock().unwrap();
        let ft = StreamingFake::plain(json!({
            "stop_reason": "end_turn",
            "content": [{"type": "text", "text": "plain"}],
            "usage": {"input_tokens": 3, "output_tokens": 4},
        }));
        let mut client = client_with(ft.clone(), &[]);
        let resp = run(&client).unwrap();
        assert_eq!(resp.text(), "plain");
        assert_eq!(resp.usage().input, 3);
        assert!(client.take_stream_fallback(), "the fallback latched");
        assert!(!client.take_stream_fallback(), "take semantics: fires once");
        // The fallback leg does not switch the REQUEST leg: still streamed.
        assert!(ft.bodies()[0].contains("\"stream\":true"));
    }

    // ---- F7 phase 1: request legs ----

    #[test]
    fn streaming_request_body_carries_stream_true_and_deltas_flow() {
        let _env = ENV_LOCK.lock().unwrap();
        let ft = StreamingFake::sse(vec![sse_stream(&[
            msg_start(),
            json!({"type": "content_block_start", "index": 0, "content_block": {"type": "text", "text": ""}}),
            json!({"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": "hi"}}),
            json!({"type": "content_block_stop", "index": 0}),
            json!({"type": "message_delta", "delta": {"stop_reason": "end_turn"}, "usage": {"output_tokens": 1}}),
            json!({"type": "message_stop"}),
        ])]);
        let mut client = client_with(ft.clone(), &[]);
        let deltas: std::sync::Arc<std::sync::Mutex<Vec<String>>> = Default::default();
        client.set_text_delta_hook(Some(Box::new({
            let log = deltas.clone();
            move |d: &str| log.lock().unwrap().push(d.to_string())
        })));
        let resp = run(&client).unwrap();
        client.set_text_delta_hook(None);
        assert_eq!(resp.text(), "hi");
        assert!(ft.bodies()[0].contains("\"stream\":true"));
        assert_eq!(*deltas.lock().unwrap(), ["hi"], "the hook received the text delta");
    }

    /// Kill switch (req 1): CHUG_STREAM=0 restores the byte-identical
    /// non-streaming request body AND parse path. The env is process-global;
    /// the flip window is this test only (no other test pins a request body,
    /// and the fallback leg keeps every other fake-transport test's JSON
    /// parse byte-identical either way).
    #[test]
    fn kill_switch_restores_non_streaming_body_and_parse() {
        let _env = ENV_LOCK.lock().unwrap();
        let ft = StreamingFake::plain(json!({
            "stop_reason": "end_turn",
            "content": [{"type": "text", "text": "plain"}],
            "usage": {"input_tokens": 1, "output_tokens": 2},
        }));
        let mut client = client_with(ft.clone(), &[]);
        // SAFETY: single-threaded test process semantics are not guaranteed
        // under the default harness, but no other test reads CHUG_STREAM and
        // none pins a request body — the flip window is benign (see above).
        unsafe { std::env::set_var("CHUG_STREAM", "0") };
        let resp = run(&client).unwrap();
        unsafe { std::env::remove_var("CHUG_STREAM") };
        assert_eq!(resp.text(), "plain");
        assert!(!ft.bodies()[0].contains("\"stream\""), "body has no stream field");
        assert_eq!(ft.calls(), 1);
        assert!(!client.take_stream_fallback(), "the non-streaming leg never latches");
    }

    /// Req 6: a mid-stream connection failure retries exactly like a mid-body
    /// failure (T1), and the discarded accumulator never yields a Response.
    #[test]
    fn mid_stream_connection_failure_retries() {
        let _env = ENV_LOCK.lock().unwrap();
        let ok_stream = sse_stream(&[
            msg_start(),
            json!({"type": "content_block_start", "index": 0, "content_block": {"type": "text", "text": ""}}),
            json!({"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": "ok"}}),
            json!({"type": "content_block_stop", "index": 0}),
            json!({"type": "message_delta", "delta": {"stop_reason": "end_turn"}, "usage": {"output_tokens": 1}}),
            json!({"type": "message_stop"}),
        ]);
        // Script: attempt 1 dies mid-stream AFTER a chunk was fed; attempt 2 succeeds.
        let fake = Arc::new(StreamingFake {
            responses: std::sync::Mutex::new(
                vec![
                    Err(TransportError::Connection("connection reset by peer".into())),
                    Ok(vec![ok_stream]),
                ]
                .into(),
            ),
            content_type: "text/event-stream".to_string(),
            content_type_overrides: std::sync::Mutex::new(Default::default()),
            bodies: std::sync::Mutex::new(Vec::new()),
            calls: std::sync::Mutex::new(0),
        });
        let client = client_with(fake.clone(), &[0]);
        let resp = run(&client).unwrap();
        assert_eq!(resp.text(), "ok");
        assert_eq!(fake.calls(), 2);
    }

    /// The streamed-stall leg: a stream that goes silent aborts at the SAME
    /// activity timeout through the progress variant (T74 margins untouched).
    #[test]
    fn streamed_stall_aborts_at_activity_timeout() {
        let err = read_body_with_watchdog_progress(
            SilentAfterFirst { sent_first: false },
            Duration::from_secs(1),
            Some(&mut |_bytes: &[u8]| {}),
        )
        .unwrap_err();
        match err {
            TransportError::Connection(msg) => assert!(msg.contains("activity timeout"), "{msg}"),
            other => panic!("expected connection error, got {other:?}"),
        }
    }

    /// Hook hygiene (kimi finding 3, un-vacuumed): the hook is ARMED first and
    /// proven live (call 1's deltas arrive), THEN cleared — and call 2 (also a
    /// streamed call) delivers nothing to it. Kills a mutant whose
    /// `set_text_delta_hook` ignores the `None` clear (the hook stays armed
    /// and call 2's deltas leak into it).
    #[test]
    fn cleared_hook_receives_no_deltas() {
        let _env = ENV_LOCK.lock().unwrap();
        let streamed = || {
            sse_stream(&[
                msg_start(),
                json!({"type": "content_block_start", "index": 0, "content_block": {"type": "text", "text": ""}}),
                json!({"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": "hi"}}),
                json!({"type": "content_block_stop", "index": 0}),
                json!({"type": "message_delta", "delta": {"stop_reason": "end_turn"}, "usage": {"output_tokens": 1}}),
                json!({"type": "message_stop"}),
            ])
        };
        let ft = Arc::new(StreamingFake {
            responses: std::sync::Mutex::new(
                vec![Ok(vec![streamed()]), Ok(vec![streamed()])].into(),
            ),
            content_type: "text/event-stream".to_string(),
            content_type_overrides: std::sync::Mutex::new(Default::default()),
            bodies: std::sync::Mutex::new(Vec::new()),
            calls: std::sync::Mutex::new(0),
        });
        let mut client = client_with(ft, &[]);
        let deltas: std::sync::Arc<std::sync::Mutex<Vec<String>>> = Default::default();
        {
            let sink = deltas.clone();
            client.set_text_delta_hook(Some(Box::new(move |d: &str| {
                sink.lock().unwrap().push(d.to_string())
            })));
        }
        // Call 1: the hook IS armed — deltas arrive (proves the hook works).
        let resp = run(&client).unwrap();
        assert_eq!(resp.text(), "hi");
        assert_eq!(
            deltas.lock().unwrap().as_slice(),
            ["hi"],
            "the armed hook received call 1's deltas"
        );
        // THE CLEAR under test.
        client.set_text_delta_hook(None);
        // Call 2: also streamed — the cleared hook must receive NOTHING.
        let resp = run(&client).unwrap();
        assert_eq!(resp.text(), "hi");
        assert_eq!(
            deltas.lock().unwrap().as_slice(),
            ["hi"],
            "a cleared hook receives no deltas from a later streamed call"
        );
    }

    // ---- kimi FAIL findings on 53e4aed: the fix-up round's killing tests ----

    /// Finding 1 (req 4, BLOCKING — the validator's two-response probe,
    /// promoted verbatim): the fallback telemetry is FIRST-PER-RUN latched.
    /// Two consecutive proxy-downgrade responses produce exactly ONE latched
    /// note — the second `take_stream_fallback` is false. RED on 53e4aed
    /// (the per-downgrade `set(true)` re-latched: true, true).
    #[test]
    fn stream_fallback_latch_fires_exactly_once_across_consecutive_downgrades() {
        let _env = ENV_LOCK.lock().unwrap();
        let plain_body = || {
            json!({
                "stop_reason": "end_turn",
                "content": [{"type": "text", "text": "downgraded"}],
                "usage": {"input_tokens": 3, "output_tokens": 4},
            })
            .to_string()
        };
        let ft: Arc<dyn Transport> = Arc::new(StreamingFake {
            responses: std::sync::Mutex::new(
                vec![Ok(vec![plain_body()]), Ok(vec![plain_body()])].into(),
            ),
            content_type: "application/json".to_string(),
            content_type_overrides: std::sync::Mutex::new(Default::default()),
            bodies: std::sync::Mutex::new(Vec::new()),
            calls: std::sync::Mutex::new(0),
        });
        let mut client = client_with(ft, &[]);
        run(&client).unwrap();
        assert!(
            client.take_stream_fallback(),
            "the first downgrade latches"
        );
        run(&client).unwrap();
        assert!(
            !client.take_stream_fallback(),
            "the SECOND downgrade must not re-latch: one stream_fallback line per run"
        );
    }

    /// Fallback-latch family sweep, other cardinality edge: a take on a CLEAN
    /// (SSE) response must not burn the latch — the run's first downgrade
    /// still emits its one line, and only that one. Kills a mutant whose
    /// take moves Fresh→Consumed on a false read (the driver takes after
    /// EVERY complete, so the downgrade's line would be lost).
    #[test]
    fn stream_fallback_latch_survives_clean_responses_then_fires_once() {
        let _env = ENV_LOCK.lock().unwrap();
        let ok_sse = sse_stream(&[
            msg_start(),
            json!({"type": "content_block_start", "index": 0, "content_block": {"type": "text", "text": ""}}),
            json!({"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": "clean"}}),
            json!({"type": "content_block_stop", "index": 0}),
            json!({"type": "message_delta", "delta": {"stop_reason": "end_turn"}, "usage": {"output_tokens": 1}}),
            json!({"type": "message_stop"}),
        ]);
        let downgraded = json!({
            "stop_reason": "end_turn",
            "content": [{"type": "text", "text": "downgraded"}],
            "usage": {"input_tokens": 3, "output_tokens": 4},
        })
        .to_string();
        let ft = StreamingFake::scripted(
            vec![
                (Some("text/event-stream"), vec![ok_sse]),
                (Some("application/json"), vec![downgraded.clone()]),
                (Some("application/json"), vec![downgraded]),
            ],
            "text/event-stream",
        );
        let mut client = client_with(ft, &[]);
        // Response 1: clean SSE — take is false and does NOT consume the
        // right to latch later.
        run(&client).unwrap();
        assert!(!client.take_stream_fallback(), "no downgrade, no line");
        // Response 2: the run's FIRST downgrade — exactly one line.
        run(&client).unwrap();
        assert!(
            client.take_stream_fallback(),
            "a downgrade after clean responses still latches once"
        );
        // Response 3: another downgrade — never re-latches.
        run(&client).unwrap();
        assert!(
            !client.take_stream_fallback(),
            "one line per run, no matter how many responses downgrade"
        );
    }

    /// StreamAccumulator error-latch family sweep (per-attempt cardinality,
    /// by design): the FIRST error wins — once latched, later feeds are
    /// ignored, so a second, different error event can never overwrite the
    /// first and the stream stays dead (finish still errors). Kills a mutant
    /// that drops the `error.is_some()` feed guard (the second error event
    /// would overwrite the first).
    #[test]
    fn accumulator_error_latch_first_error_wins_later_feeds_ignored() {
        let mut acc = StreamAccumulator::new(None::<&mut dyn FnMut(&str)>);
        acc.feed_bytes(
            sse_stream(&[json!({
                "type": "error",
                "error": {"type": "overloaded_error", "message": "first failure"},
            })])
            .as_bytes(),
        );
        // Later feeds after the latch: a valid tail AND a second error event
        // with a different message — neither may resurrect or overwrite.
        acc.feed_bytes(
            sse_stream(&[
                json!({"type": "message_stop"}),
                json!({
                    "type": "error",
                    "error": {"type": "api_error", "message": "second failure"},
                }),
            ])
            .as_bytes(),
        );
        match acc.finish().unwrap_err() {
            StreamError::Retryable(TransportError::Connection(msg)) => {
                assert!(
                    msg.contains("first failure") && !msg.contains("second failure"),
                    "the FIRST error is latched, later feeds ignored: {msg}"
                );
            }
            other => panic!("expected the latched retryable error, got {other:?}"),
        }
    }

}
