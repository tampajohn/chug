//! Streamable-HTTP MCP transport (SPEC-9): POST-per-message client with
//! incremental SSE response parsing, session-id propagation, bounded
//! timeouts, and connection-level retry. No async: a worker thread performs
//! the blocking POST and streams the response head and body lines back over
//! a channel, so the caller enforces the first-byte (30s) and per-call total
//! (60s) deadlines itself — a server holding its SSE stream open after
//! answering never hangs the call.
//!
//! Round 3 adds the GET listen stream: after a successful handshake a
//! manager thread opens `GET {url}` with `Accept: text/event-stream` on a
//! dedicated connection, answers server→client requests with `-32601`
//! POSTs, drops notifications, and reconnects a dropped stream with
//! jittered backoff (1s→30s cap, Last-Event-ID replay) until Drop. A 405
//! means the server offers no listen stream: log once, never retry.

use anyhow::{Context, bail};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, mpsc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime};

use crate::mcp::{McpBackend, McpTool};
use crate::sse::{PostRetrySchedule, SseParser, SseReconnectBackoff};
use crate::tools::ToolResult;

const PROTOCOL_VERSION: &str = "2025-06-18";
const INIT_TIMEOUT: Duration = Duration::from_secs(10);
const LIST_TIMEOUT: Duration = Duration::from_secs(10);
const CALL_TIMEOUT: Duration = Duration::from_secs(60);
/// Waiting for the response head (status line + headers) once the request
/// has left the process. The 10s connect timeout nests inside this budget.
const FIRST_BYTE_TIMEOUT: Duration = Duration::from_secs(30);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Bounded TOTAL timeout for fire-and-forget `-32601` reply POSTs — the one
/// client that keeps a reqwest-level timeout, so a hung reply target cannot
/// park a helper thread forever.
const REPLY_TIMEOUT: Duration = Duration::from_secs(30);
/// Defensive cap on registered tools per server (a broken server must not
/// flood the model's tool list).
const MAX_MCP_TOOLS: usize = 200;
/// How often the listen manager wakes from its channel wait to check the
/// shutdown flag — the bound on Drop latency, and the chunk size of
/// interruptible backoff sleeps.
const LISTEN_POLL: Duration = Duration::from_millis(100);
/// Grace for joining the listen manager thread on Drop. The manager wakes
/// every LISTEN_POLL, so this only matters when something has gone wrong —
/// mirror of stdio's READER_JOIN_GRACE: leak rather than block Drop forever.
const LISTEN_JOIN_GRACE: Duration = Duration::from_secs(5);

/// A remote MCP server spoken to over the streamable-HTTP transport: every
/// JSON-RPC message is its own POST; responses may arrive as a plain JSON
/// body or framed as SSE events.
pub struct HttpMcpServer {
    name: String,
    url: String,
    headers: Vec<(String, String)>,
    /// One shared client for the server's whole life: connection pooling
    /// across POSTs, connect timeout only — no blanket request timeout,
    /// which would kill legitimate long SSE reads.
    client: reqwest::blocking::Client,
    /// Dedicated client for the GET listen stream: its own pool, so the
    /// listen connection never shares (or starves) the POST pool's
    /// keep-alive connections.
    listen_client: reqwest::blocking::Client,
    /// Bounded-timeout client used ONLY for fire-and-forget `-32601` reply
    /// POSTs (the main clients deliberately carry no blanket timeout).
    reply_client: reqwest::blocking::Client,
    /// First-byte deadline for POSTs (default FIRST_BYTE_TIMEOUT); a field
    /// so tests can shrink it instead of burning real time.
    first_byte_timeout: Duration,
    /// Total tools/call budget (default CALL_TIMEOUT); overridable in tests
    /// for the same reason.
    call_timeout: Duration,
    session_id: Arc<Mutex<Option<String>>>,
    next_id: u64,
    tools: Vec<McpTool>,
    alive: Arc<Mutex<bool>>,
    /// Sleep between POST retries and listen-stream backoff chunks;
    /// injectable so tests never really sleep.
    sleeper: Arc<dyn Fn(Duration) + Send + Sync>,
    /// Per-server log file (.chug/mcp-<name>.log when spawned via the
    /// registry); falls back to stderr when unset.
    log_path: Option<PathBuf>,
    /// Set when the server is being dropped; the listen manager polls it.
    shutdown: Arc<AtomicBool>,
    /// Listen manager thread (spawned after a successful handshake).
    listen_handle: Option<JoinHandle<()>>,
}

/// Lock a mutex without panicking on poisoning (same discipline as stdio).
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Messages from the POST worker thread to the caller.
enum WireMsg {
    /// Response head arrived: status + flattened (lowercased) headers.
    Head(u16, HashMap<String, String>),
    /// One body line, CR/LF stripped.
    Line(String),
    /// Body finished (also fires immediately for empty bodies, e.g. 202).
    Eof,
    /// The request failed before any HTTP response head; the reqwest error
    /// is kept for is_connect()/is_timeout() classification — retry
    /// decisions are classifier-based, never string matching.
    ConnectFailed(reqwest::Error),
    /// A body read failed after the head arrived. Always an immediate error.
    ReadFailed(String),
}

/// Outcome of one POST attempt; retry policy lives in the caller.
enum Attempt {
    /// The matching JSON-RPC response arrived.
    Done(Value),
    /// A notification/response-only message was acknowledged (202 or 200).
    Acked,
    /// No HTTP response at all and reqwest classified the failure as
    /// connect-level or connect-phase timeout → eligible for the 1s/2s/4s
    /// retry schedule.
    Retryable(reqwest::Error),
    /// Everything else: HTTP status, parse failure, deadline, read error.
    Failed(anyhow::Error),
}

/// Result of consuming one SSE event's data payload.
enum SseMatch {
    /// The awaited response arrived.
    Response(Value),
    /// Event consumed without a response: server→client request (replied
    /// method-not-found), notification, or foreign id — all dropped.
    Consumed,
    /// Malformed event data: fail the call.
    Bad(anyhow::Error),
}

impl HttpMcpServer {
    pub fn new(name: String, url: String, headers: Vec<(String, String)>) -> anyhow::Result<Self> {
        Self::with_sleeper(name, url, headers, Arc::new(std::thread::sleep))
    }

    pub fn with_sleeper(
        name: String,
        url: String,
        headers: Vec<(String, String)>,
        sleeper: Arc<dyn Fn(Duration) + Send + Sync>,
    ) -> anyhow::Result<Self> {
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            // No blanket timeout: reqwest's default (30s per body read)
            // would kill silent listen streams and late-arriving SSE call
            // answers — the explicit deadlines (connect 10s, first-byte
            // 30s, per-call total 60s) are enforced by the caller via
            // channel recv_timeout, so they alone govern.
            .timeout(None)
            .build()
            .context("building http client")?;
        let listen_client = reqwest::blocking::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            // Same: a healthy listen stream may stay silent for hours.
            .timeout(None)
            .build()
            .context("building http listen client")?;
        // Fire-and-forget method-not-found replies ride a dedicated client
        // with a BOUNDED total timeout: the main clients have none, and a
        // hung reply target must not park a helper thread forever.
        let reply_client = reqwest::blocking::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REPLY_TIMEOUT)
            .build()
            .context("building http reply client")?;
        Ok(Self {
            name,
            url,
            headers,
            client,
            listen_client,
            reply_client,
            first_byte_timeout: FIRST_BYTE_TIMEOUT,
            call_timeout: CALL_TIMEOUT,
            session_id: Arc::new(Mutex::new(None)),
            next_id: 0,
            tools: Vec::new(),
            alive: Arc::new(Mutex::new(true)),
            sleeper,
            log_path: None,
            shutdown: Arc::new(AtomicBool::new(false)),
            listen_handle: None,
        })
    }

    /// Route `log` to a per-server file (registry-spawned servers) instead
    /// of stderr. Must be called before `initialize` (the listen thread
    /// inherits the path).
    pub fn set_log_path(&mut self, path: PathBuf) {
        self.log_path = Some(path);
    }

    /// One line to the per-server log (best effort — logging must never
    /// break the protocol path).
    fn log(&self, line: &str) {
        listen_log(&self.log_path, &self.name, line);
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn is_alive(&self) -> bool {
        *lock(&self.alive)
    }

    pub fn tools(&self) -> &[McpTool] {
        &self.tools
    }

    /// `initialize` → capture Mcp-Session-Id → `notifications/initialized`
    /// (expecting 202) → `tools/list`. Any failure aborts the handshake; the
    /// caller skips the server (fail-soft).
    pub fn initialize(&mut self) -> anyhow::Result<()> {
        let params = json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": { "name": "chug", "version": "0.1.0" }
        });
        self.send_request("initialize", params, INIT_TIMEOUT)
            .context("initialize handshake")?;
        self.send_notification("notifications/initialized", json!({}), INIT_TIMEOUT)
            .context("initialized notification")?;
        let list_resp = self
            .send_request("tools/list", json!({}), LIST_TIMEOUT)
            .context("tools/list")?;
        let tools = list_resp
            .get("result")
            .and_then(|r| r.get("tools"))
            .and_then(Value::as_array)
            .with_context(|| format!("mcp server {}: tools/list returned no tools array", self.name))?;
        if tools.len() > MAX_MCP_TOOLS {
            self.log(&format!(
                "tools/list advertised {} tools; capped at {MAX_MCP_TOOLS}",
                tools.len()
            ));
        }
        self.tools = tools
            .iter()
            .take(MAX_MCP_TOOLS)
            .filter_map(|t| {
                let name = t.get("name")?.as_str()?.to_string();
                let description = t
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let input_schema = t.get("inputSchema").cloned().unwrap_or(json!({}));
                Some(McpTool {
                    name,
                    description,
                    input_schema,
                })
            })
            .collect();
        // Handshake complete: open the GET listen stream (server→client
        // requests/notifications) on its own manager thread.
        self.start_listen_stream();
        Ok(())
    }

    /// Call an MCP tool (`call_timeout` budget, 60s by default). Errors are
    /// returned as `is_error` tool results, never as driver-level failures —
    /// same contract as stdio.
    pub fn call(&mut self, tool_name: &str, arguments: Value) -> anyhow::Result<ToolResult> {
        if !self.is_alive() {
            return Ok(ToolResult {
                content: format!("mcp server {} is down", self.name),
                is_error: true,
            });
        }
        let params = json!({ "name": tool_name, "arguments": arguments });
        match self.send_request("tools/call", params, self.call_timeout) {
            Ok(resp) => Ok(parse_call_response(&resp)),
            Err(e) => Ok(ToolResult {
                content: format!("mcp tool {tool_name} failed: {e:#}"),
                is_error: true,
            }),
        }
    }

    /// Send one JSON-RPC request and await its response, retrying only
    /// connection-level failures (no HTTP response) on the 1s/2s/4s schedule.
    fn send_request(&mut self, method: &str, params: Value, timeout: Duration) -> anyhow::Result<Value> {
        self.next_id += 1;
        let id = self.next_id;
        let body = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params
        });
        let mut retry = PostRetrySchedule::with_sleeper(Arc::clone(&self.sleeper));
        loop {
            match self.post_attempt(&body, Some(id), timeout) {
                Attempt::Done(v) => return Ok(v),
                // Unreachable by contract: post_attempt returns Acked only
                // when expect_id is None (notifications), and send_request
                // always passes Some(id). Kept as a defensive bail rather
                // than unreachable!() so a future post_attempt change
                // degrades to an error instead of a panic.
                Attempt::Acked => bail!(
                    "mcp server {}: request {method} accepted (202) but no response returned",
                    self.name
                ),
                Attempt::Retryable(e) => {
                    if retry.sleep_next() {
                        continue;
                    }
                    // Same contract as stdio's dead server: mark down so later
                    // calls short-circuit instead of retry-storming.
                    *lock(&self.alive) = false;
                    bail!("mcp server {} POST failed after retries: {e}", self.name);
                }
                Attempt::Failed(e) => return Err(e),
            }
        }
    }

    /// POST a notification (no id): a 202/200 ack is the whole contract.
    fn send_notification(&mut self, method: &str, params: Value, timeout: Duration) -> anyhow::Result<()> {
        let body = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params
        });
        let mut retry = PostRetrySchedule::with_sleeper(Arc::clone(&self.sleeper));
        loop {
            match self.post_attempt(&body, None, timeout) {
                Attempt::Acked | Attempt::Done(_) => return Ok(()),
                Attempt::Retryable(e) => {
                    if retry.sleep_next() {
                        continue;
                    }
                    *lock(&self.alive) = false;
                    bail!("mcp server {} POST failed after retries: {e}", self.name);
                }
                Attempt::Failed(e) => return Err(e),
            }
        }
    }

    /// One POST attempt: worker thread performs the blocking request and
    /// streams head + body lines back; this thread enforces the first-byte
    /// (30s) and total (`timeout`) deadlines via channel recv_timeout.
    fn post_attempt(&self, body: &Value, expect_id: Option<u64>, timeout: Duration) -> Attempt {
        let deadline = Instant::now() + timeout;
        let cancel = Arc::new(AtomicBool::new(false));
        let rx = spawn_post_worker(
            self.client.clone(),
            self.url.clone(),
            self.all_headers(),
            body.to_string(),
            Arc::clone(&cancel),
        );
        // Phase 1: response head within the first-byte budget.
        let first_byte = self.first_byte_timeout.min(timeout);
        let (status, headers) = match rx.recv_timeout(first_byte) {
            Ok(WireMsg::Head(status, headers)) => (status, headers),
            Ok(WireMsg::ConnectFailed(e)) => {
                return if e.is_connect() || e.is_timeout() {
                    Attempt::Retryable(e)
                } else {
                    Attempt::Failed(anyhow::anyhow!("mcp server {}: POST failed: {e}", self.name))
                };
            }
            Ok(_) => {
                return Attempt::Failed(anyhow::anyhow!(
                    "mcp server {}: protocol error (body before head)",
                    self.name
                ));
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                cancel.store(true, Ordering::Relaxed);
                return Attempt::Failed(anyhow::anyhow!(
                    "mcp server {}: no response head within {}s",
                    self.name,
                    first_byte.as_secs()
                ));
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Attempt::Failed(anyhow::anyhow!(
                    "mcp server {}: POST worker exited unexpectedly",
                    self.name
                ));
            }
        };
        // Session header: once issued, it rides every subsequent request.
        if let Some(sid) = headers.get("mcp-session-id") {
            *lock(&self.session_id) = Some(sid.clone());
        }
        let Some(want) = expect_id else {
            // Notification / response-only message: 202 (or 200) acks it.
            return if status == 202 || status == 200 {
                Attempt::Acked
            } else {
                Attempt::Failed(anyhow::anyhow!(
                    "mcp server {} POST returned HTTP {status}",
                    self.name
                ))
            };
        };
        // HTTP statuses are immediate errors — only connection-level
        // failures (no response at all) are retryable.
        if status != 200 {
            return Attempt::Failed(anyhow::anyhow!(
                "mcp server {} POST returned HTTP {status}",
                self.name
            ));
        }
        let is_sse = headers
            .get("content-type")
            .map(|v| v.to_ascii_lowercase().contains("text/event-stream"))
            .unwrap_or(false);
        // Phase 2: body, line by line, within the remaining budget. SSE
        // events are consumed incrementally: the call returns the moment the
        // matching-id response arrives, even if the server holds the stream
        // open.
        let mut parser = SseParser::new();
        let mut json_body = String::new();
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                cancel.store(true, Ordering::Relaxed);
                return Attempt::Failed(anyhow::anyhow!(
                    "mcp request timed out after {}s",
                    timeout.as_secs()
                ));
            }
            match rx.recv_timeout(remaining) {
                Ok(WireMsg::Line(line)) => {
                    if is_sse {
                        if let Some(ev) = parser.feed_line(&line) {
                            match self.handle_sse_data(&ev.data, want) {
                                SseMatch::Response(v) => {
                                    cancel.store(true, Ordering::Relaxed);
                                    return Attempt::Done(v);
                                }
                                SseMatch::Bad(e) => {
                                    cancel.store(true, Ordering::Relaxed);
                                    return Attempt::Failed(e);
                                }
                                SseMatch::Consumed => {}
                            }
                        }
                    } else {
                        json_body.push_str(&line);
                        json_body.push('\n');
                    }
                }
                Ok(WireMsg::Eof) => {
                    if is_sse {
                        return Attempt::Failed(anyhow::anyhow!(
                            "mcp server {}: SSE stream ended without a response for id {want}",
                            self.name
                        ));
                    }
                    return match serde_json::from_str::<Value>(&json_body) {
                        Ok(v) if v.get("id").and_then(Value::as_u64) == Some(want) => Attempt::Done(v),
                        Ok(_) => Attempt::Failed(anyhow::anyhow!(
                            "mcp server {}: response id mismatch (want {want})",
                            self.name
                        )),
                        Err(e) => Attempt::Failed(anyhow::anyhow!(
                            "mcp server {}: invalid JSON response: {e}",
                            self.name
                        )),
                    };
                }
                Ok(WireMsg::ReadFailed(e)) => {
                    return Attempt::Failed(anyhow::anyhow!(
                        "mcp server {}: response read failed: {e}",
                        self.name
                    ));
                }
                Ok(WireMsg::Head(..)) | Ok(WireMsg::ConnectFailed(_)) => {
                    return Attempt::Failed(anyhow::anyhow!(
                        "mcp server {}: protocol error (unexpected wire message)",
                        self.name
                    ));
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    cancel.store(true, Ordering::Relaxed);
                    return Attempt::Failed(anyhow::anyhow!(
                        "mcp request timed out after {}s",
                        timeout.as_secs()
                    ));
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Attempt::Failed(anyhow::anyhow!(
                        "mcp server {}: response stream dropped",
                        self.name
                    ));
                }
            }
        }
    }

    /// Consume one SSE event's data payload while awaiting response `want`.
    fn handle_sse_data(&self, data: &str, want: u64) -> SseMatch {
        if data.is_empty() {
            return SseMatch::Consumed;
        }
        let v: Value = match serde_json::from_str(data) {
            Ok(v) => v,
            Err(e) => {
                return SseMatch::Bad(anyhow::anyhow!(
                    "mcp server {}: invalid SSE JSON data: {e}",
                    self.name
                ));
            }
        };
        // v1 limitation: a STRING id fails as_u64 and is classified as a
        // notification (dropped, no -32601) — parity with stdio, by design.
        let id = v.get("id").and_then(Value::as_u64);
        let is_request = v.get("method").is_some();
        match (id, is_request) {
            // Server→client request over the SSE stream: v1 implements
            // nothing on that surface, but the server MUST get a JSON-RPC
            // reply — same rule as stdio.
            (Some(req_id), true) => {
                self.reply_method_not_found(req_id);
                SseMatch::Consumed
            }
            (Some(resp_id), false) if resp_id == want => SseMatch::Response(v),
            // Notification or a response for another id: drop.
            _ => SseMatch::Consumed,
        }
    }

    /// Best-effort JSON-RPC `method not found` reply to a server→client
    /// request, as its own POST (fire-and-forget on a helper thread so a
    /// slow server cannot stall the in-flight call). Rides the bounded-
    /// timeout reply client: a hung reply target must not park the helper.
    fn reply_method_not_found(&self, req_id: u64) {
        post_method_not_found_reply(self.reply_client.clone(), self.url.clone(), self.all_headers(), req_id);
    }

    /// Spawn the listen manager thread: one GET listen stream per remote
    /// server, reconnecting for the life of the run (round 3). Called once,
    /// after a successful handshake.
    fn start_listen_stream(&mut self) {
        let cfg = ListenConfig {
            name: self.name.clone(),
            url: self.url.clone(),
            headers: self.headers.clone(),
            post_client: self.reply_client.clone(),
            get_client: self.listen_client.clone(),
            session_id: Arc::clone(&self.session_id),
            shutdown: Arc::clone(&self.shutdown),
            sleeper: Arc::clone(&self.sleeper),
            log_path: self.log_path.clone(),
        };
        self.listen_handle = Some(thread::spawn(move || listen_manager(cfg)));
    }

    /// Content-Type/Accept + configured headers + session id (once the
    /// server has issued one). Built per request: the session id can arrive
    /// mid-run.
    fn all_headers(&self) -> Vec<(String, String)> {
        let mut out = vec![
            ("content-type".to_string(), "application/json".to_string()),
            ("accept".to_string(), "application/json, text/event-stream".to_string()),
        ];
        for (k, v) in &self.headers {
            out.push((k.clone(), v.clone()));
        }
        if let Some(sid) = lock(&self.session_id).clone() {
            out.push(("mcp-session-id".to_string(), sid));
        }
        out
    }
}

/// Perform one HTTP request on a worker thread, reporting the response head
/// and then each body line over the channel. All deadlines live in the
/// caller: the worker never blocks the call path. When the caller leaves
/// early (answer received or deadline hit) the dropped channel stops the
/// worker at its next line; a server holding the stream open only keeps
/// this worker — not the call — waiting, and the `cancel` flag lets it exit
/// between lines. `body: Some` issues a POST; `None` a GET (listen stream).
fn spawn_wire_worker(
    client: reqwest::blocking::Client,
    url: String,
    headers: Vec<(String, String)>,
    body: Option<String>,
    cancel: Arc<AtomicBool>,
) -> mpsc::Receiver<WireMsg> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        macro_rules! send {
            ($msg:expr) => {
                if tx.send($msg).is_err() {
                    return;
                }
            };
        }
        let mut req = match &body {
            Some(b) => client.post(&url).body(b.clone()),
            None => client.get(&url),
        };
        for (k, v) in &headers {
            req = req.header(k.as_str(), v.as_str());
        }
        let resp = match req.send() {
            Ok(r) => r,
            Err(e) => {
                send!(WireMsg::ConnectFailed(e));
                return;
            }
        };
        let status = resp.status().as_u16();
        let headers: HashMap<String, String> = resp
            .headers()
            .iter()
            .map(|(k, v)| {
                (
                    k.as_str().to_ascii_lowercase(),
                    v.to_str().unwrap_or("").to_string(),
                )
            })
            .collect();
        send!(WireMsg::Head(status, headers));
        let mut reader = BufReader::new(resp);
        loop {
            if cancel.load(Ordering::Relaxed) {
                return;
            }
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => {
                    send!(WireMsg::Eof);
                    return;
                }
                Ok(_) => {
                    while line.ends_with('\n') || line.ends_with('\r') {
                        line.pop();
                    }
                    send!(WireMsg::Line(line));
                }
                Err(e) => {
                    send!(WireMsg::ReadFailed(e.to_string()));
                    return;
                }
            }
        }
    });
    rx
}

/// POST worker for JSON-RPC calls/notifications (see spawn_wire_worker).
fn spawn_post_worker(
    client: reqwest::blocking::Client,
    url: String,
    headers: Vec<(String, String)>,
    body: String,
    cancel: Arc<AtomicBool>,
) -> mpsc::Receiver<WireMsg> {
    spawn_wire_worker(client, url, headers, Some(body), cancel)
}

/// GET worker for the listen stream (see spawn_wire_worker).
fn spawn_get_worker(
    client: reqwest::blocking::Client,
    url: String,
    headers: Vec<(String, String)>,
    cancel: Arc<AtomicBool>,
) -> mpsc::Receiver<WireMsg> {
    spawn_wire_worker(client, url, headers, None, cancel)
}

// ---------- GET listen stream (SPEC-9 round 3) ----------

/// Everything the listen manager thread needs, cloned out of the server at
/// spawn time (the manager never touches `HttpMcpServer` itself).
struct ListenConfig {
    name: String,
    url: String,
    /// Configured headers from mcp.json (e.g. Authorization).
    headers: Vec<(String, String)>,
    /// POST pool client — used for method-not-found replies.
    post_client: reqwest::blocking::Client,
    /// Dedicated client/pool for the GET stream itself.
    get_client: reqwest::blocking::Client,
    session_id: Arc<Mutex<Option<String>>>,
    shutdown: Arc<AtomicBool>,
    sleeper: Arc<dyn Fn(Duration) + Send + Sync>,
    log_path: Option<PathBuf>,
}

/// How one listen connection ended; retry policy lives in listen_manager.
enum ListenOutcome {
    /// The stream opened (200 SSE) and later ended or errored → the backoff
    /// schedule resets before sleeping.
    DroppedAfterOpen,
    /// No stream was established (connect failure, HTTP status, missing
    /// head deadline, non-SSE 200) → advance the backoff schedule.
    FailedToOpen,
    /// GET answered 405: the server offers no listen stream — stop trying.
    NotOffered,
    /// Shutdown was requested mid-connection.
    Quit,
}

/// Append one line to the per-server log, or stderr when no file is set.
/// Best effort: logging must never break the protocol path.
fn listen_log(log_path: &Option<PathBuf>, name: &str, line: &str) {
    let msg = format!("chug: mcp server {name}: {line}");
    match log_path {
        Some(p) => crate::mcp::log_line(p, &msg),
        None => eprintln!("{msg}"),
    }
}

/// Jitter factor in [0.5, 1.0) from the system clock — decorrelates
/// reconnect storms without a rand dependency. Tests never observe the
/// resulting durations (the injected sleeper swallows them), so this needs
/// no injection point.
fn jitter_factor() -> f64 {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    0.5 + f64::from(nanos % 500) / 1000.0
}

/// Sleep `delay` in ≤LISTEN_POLL chunks through the (possibly injected)
/// sleeper so a shutdown during backoff is prompt. Returns false when
/// shutdown fired before the delay elapsed.
fn interruptible_sleep(delay: Duration, cfg: &ListenConfig) -> bool {
    let mut remaining = delay;
    while !remaining.is_zero() {
        if cfg.shutdown.load(Ordering::Relaxed) {
            return false;
        }
        let chunk = remaining.min(LISTEN_POLL);
        (cfg.sleeper)(chunk);
        remaining -= chunk;
    }
    !cfg.shutdown.load(Ordering::Relaxed)
}

/// The listen manager: owns the GET listen stream for the life of the run.
/// Reconnects a dropped/errored stream with SseReconnectBackoff (1s→30s,
/// jittered); a 405 means the server offers no listen stream (log once,
/// exit); shutdown exits promptly (the manager never blocks on I/O for
/// longer than LISTEN_POLL — all blocking reads live in the GET worker).
fn listen_manager(cfg: ListenConfig) {
    let mut backoff = SseReconnectBackoff::new();
    let mut last_event_id: Option<String> = None;
    loop {
        if cfg.shutdown.load(Ordering::Relaxed) {
            return;
        }
        match listen_once(&cfg, &mut last_event_id) {
            ListenOutcome::Quit => return,
            ListenOutcome::NotOffered => {
                listen_log(
                    &cfg.log_path,
                    &cfg.name,
                    "server offers no listen stream (GET returned 405); continuing without one",
                );
                return;
            }
            ListenOutcome::DroppedAfterOpen => {
                // A real stream existed: reset the schedule, then back off.
                backoff = SseReconnectBackoff::new();
                let delay = backoff.next(jitter_factor());
                if !interruptible_sleep(delay, &cfg) {
                    return;
                }
            }
            ListenOutcome::FailedToOpen => {
                let delay = backoff.next(jitter_factor());
                if !interruptible_sleep(delay, &cfg) {
                    return;
                }
            }
        }
    }
}

/// One listen connection: GET with Accept: text/event-stream (plus config
/// headers, Mcp-Session-Id when known, Last-Event-ID on reconnect when the
/// server supplied event ids), then consume SSE events until EOF, error, or
/// shutdown. Updates `last_event_id` as id-bearing events arrive.
fn listen_once(cfg: &ListenConfig, last_event_id: &mut Option<String>) -> ListenOutcome {
    let mut headers = vec![("accept".to_string(), "text/event-stream".to_string())];
    for (k, v) in &cfg.headers {
        headers.push((k.clone(), v.clone()));
    }
    if let Some(sid) = lock(&cfg.session_id).clone() {
        headers.push(("mcp-session-id".to_string(), sid));
    }
    if let Some(id) = last_event_id.as_deref() {
        headers.push(("last-event-id".to_string(), id.to_string()));
    }
    let cancel = Arc::new(AtomicBool::new(false));
    let rx = spawn_get_worker(
        cfg.get_client.clone(),
        cfg.url.clone(),
        headers,
        Arc::clone(&cancel),
    );
    // Phase 1: response head within the first-byte budget, polling shutdown
    // every LISTEN_POLL instead of one long recv_timeout.
    let head_deadline = Instant::now() + FIRST_BYTE_TIMEOUT;
    let (status, head) = loop {
        if cfg.shutdown.load(Ordering::Relaxed) {
            cancel.store(true, Ordering::Relaxed);
            return ListenOutcome::Quit;
        }
        let remaining = head_deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            cancel.store(true, Ordering::Relaxed);
            listen_log(
                &cfg.log_path,
                &cfg.name,
                "listen stream: no response head within 30s; reconnecting",
            );
            return ListenOutcome::FailedToOpen;
        }
        match rx.recv_timeout(remaining.min(LISTEN_POLL)) {
            Ok(WireMsg::Head(status, head)) => break (status, head),
            Ok(WireMsg::ConnectFailed(e)) => {
                listen_log(
                    &cfg.log_path,
                    &cfg.name,
                    &format!("listen stream connect failed: {e}; reconnecting"),
                );
                return ListenOutcome::FailedToOpen;
            }
            Ok(_) => {
                listen_log(&cfg.log_path, &cfg.name, "listen stream: protocol error before head");
                return ListenOutcome::FailedToOpen;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                listen_log(&cfg.log_path, &cfg.name, "listen worker exited unexpectedly");
                return ListenOutcome::FailedToOpen;
            }
        }
    };
    // A session id may ride the GET response too; replay it from then on.
    if let Some(sid) = head.get("mcp-session-id") {
        *lock(&cfg.session_id) = Some(sid.clone());
    }
    if status == 405 {
        return ListenOutcome::NotOffered;
    }
    if status != 200 {
        listen_log(
            &cfg.log_path,
            &cfg.name,
            &format!("listen stream GET returned HTTP {status}; reconnecting"),
        );
        return ListenOutcome::FailedToOpen;
    }
    let is_sse = head
        .get("content-type")
        .map(|v| v.to_ascii_lowercase().contains("text/event-stream"))
        .unwrap_or(false);
    if !is_sse {
        listen_log(
            &cfg.log_path,
            &cfg.name,
            "listen stream GET did not return text/event-stream; reconnecting",
        );
        return ListenOutcome::FailedToOpen;
    }
    // Phase 2: consume events until the stream ends or shutdown fires.
    let mut parser = SseParser::new();
    loop {
        if cfg.shutdown.load(Ordering::Relaxed) {
            cancel.store(true, Ordering::Relaxed);
            return ListenOutcome::Quit;
        }
        match rx.recv_timeout(LISTEN_POLL) {
            Ok(WireMsg::Line(line)) => {
                if let Some(ev) = parser.feed_line(&line) {
                    if let Some(id) = ev.id {
                        *last_event_id = Some(id);
                    }
                    handle_listen_message(cfg, &ev.data);
                }
            }
            Ok(WireMsg::Eof) => {
                listen_log(&cfg.log_path, &cfg.name, "listen stream closed by server; reconnecting");
                return ListenOutcome::DroppedAfterOpen;
            }
            Ok(WireMsg::ReadFailed(e)) => {
                listen_log(
                    &cfg.log_path,
                    &cfg.name,
                    &format!("listen stream read failed: {e}; reconnecting"),
                );
                return ListenOutcome::DroppedAfterOpen;
            }
            Ok(WireMsg::Head(..)) | Ok(WireMsg::ConnectFailed(_)) => {}
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                listen_log(&cfg.log_path, &cfg.name, "listen worker exited; reconnecting");
                return ListenOutcome::DroppedAfterOpen;
            }
        }
    }
}

/// Handle one JSON-RPC message arriving on the listen stream. Requests get
/// a `method not found` reply via the normal POST path (same rule as
/// stdio); notifications are dropped; responses are ignored (v1 routes no
/// client→server calls over the listen stream).
fn handle_listen_message(cfg: &ListenConfig, data: &str) {
    if data.is_empty() {
        return;
    }
    let v: Value = match serde_json::from_str(data) {
        Ok(v) => v,
        Err(_) => {
            listen_log(&cfg.log_path, &cfg.name, "listen stream: dropping malformed JSON data");
            return;
        }
    };
    // v1 limitation: a STRING id fails as_u64 and is classified as a
    // notification (dropped, no -32601) — parity with stdio, by design.
    let id = v.get("id").and_then(Value::as_u64);
    let is_request = v.get("method").is_some();
    match (id, is_request) {
        (Some(req_id), true) => {
            let mut headers = vec![
                ("content-type".to_string(), "application/json".to_string()),
                ("accept".to_string(), "application/json, text/event-stream".to_string()),
            ];
            for (k, v) in &cfg.headers {
                headers.push((k.clone(), v.clone()));
            }
            if let Some(sid) = lock(&cfg.session_id).clone() {
                headers.push(("mcp-session-id".to_string(), sid));
            }
            post_method_not_found_reply(cfg.post_client.clone(), cfg.url.clone(), headers, req_id);
        }
        (None, true) => {
            listen_log(&cfg.log_path, &cfg.name, "listen stream: dropped server notification")
        }
        (_, false) => listen_log(
            &cfg.log_path,
            &cfg.name,
            "listen stream: ignoring response message (no in-flight call matches)",
        ),
    }
}

/// Best-effort JSON-RPC `method not found` reply to a server→client
/// request, as its own POST on a helper thread (fire-and-forget: failures
/// are deliberately swallowed — a slow server must not stall the stream
/// that delivered the request).
fn post_method_not_found_reply(
    client: reqwest::blocking::Client,
    url: String,
    headers: Vec<(String, String)>,
    req_id: u64,
) {
    let reply = json!({
        "jsonrpc": "2.0",
        "id": req_id,
        "error": {"code": -32601, "message": "method not found"}
    });
    thread::spawn(move || {
        let mut req = client.post(&url);
        for (k, v) in &headers {
            req = req.header(k.as_str(), v.as_str());
        }
        let _ = req.body(reply.to_string()).send();
    });
}

impl McpBackend for HttpMcpServer {
    fn name(&self) -> &str {
        HttpMcpServer::name(self)
    }
    fn is_alive(&self) -> bool {
        HttpMcpServer::is_alive(self)
    }
    fn tools(&self) -> &[McpTool] {
        HttpMcpServer::tools(self)
    }
    fn call(&mut self, tool_name: &str, arguments: Value) -> anyhow::Result<ToolResult> {
        HttpMcpServer::call(self, tool_name, arguments)
    }
}

impl Drop for HttpMcpServer {
    fn drop(&mut self) {
        // Signal the listen manager; it polls the flag every LISTEN_POLL
        // (mid-stream, mid-backoff, mid-head-wait) so it wakes on its own —
        // the socket itself needs no closing.
        self.shutdown.store(true, Ordering::Relaxed);
        // Bounded join, mirroring stdio McpServer::drop: a manager that
        // somehow still has not exited after the grace period is leaked
        // (with its waiter) rather than allowed to block Drop forever. The
        // per-connection GET worker is NOT joined: it may be wedged on a
        // silent stream, so it is abandoned like the POST workers (channel
        // closed + cancel flag; it exits at the next line or socket close).
        if let Some(handle) = self.listen_handle.take() {
            let (tx, rx) = mpsc::channel::<()>();
            let waiter = thread::spawn(move || {
                let _ = handle.join();
                let _ = tx.send(());
            });
            if rx.recv_timeout(LISTEN_JOIN_GRACE).is_err() {
                std::mem::forget(waiter);
            }
        }
    }
}

/// Map a tools/call JSON-RPC response to a ToolResult — same semantics as
/// the stdio transport: a JSON-RPC error becomes an error result carrying
/// the message; text content blocks join with newlines; the result's
/// `isError` flag propagates.
fn parse_call_response(resp: &Value) -> ToolResult {
    let json_rpc_error = resp.get("error").map(|e| {
        e.get("message")
            .and_then(Value::as_str)
            .unwrap_or("unknown mcp error")
            .to_string()
    });
    let is_error_flag = resp
        .get("result")
        .and_then(|r| r.get("isError"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let text = resp
        .get("result")
        .and_then(|r| r.get("content"))
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|b| {
                    if b.get("type").and_then(Value::as_str) == Some("text") {
                        b.get("text").and_then(Value::as_str).map(str::to_string)
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();
    match json_rpc_error {
        Some(err) => ToolResult {
            content: err,
            is_error: true,
        },
        None => ToolResult {
            content: text,
            is_error: is_error_flag,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::McpRegistry;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use tempfile::TempDir;

    // ---------- hand-rolled HTTP/1.1 stub (no crates, 127.0.0.1 only) ----------

    /// One observed request: request line, headers (original case), body.
    struct Observed {
        request_line: String,
        headers: Vec<(String, String)>,
        body: String,
    }

    impl Observed {
        fn header(&self, name: &str) -> Option<&str> {
            self.headers
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(name))
                .map(|(_, v)| v.as_str())
        }
        fn json(&self) -> Value {
            serde_json::from_str(&self.body).expect("request body is json")
        }
        /// The listen stream's GET (no body) vs a JSON-RPC POST.
        fn is_get(&self) -> bool {
            self.request_line.starts_with("GET ")
        }
    }

    /// Read one request: head up to \r\n\r\n, then exactly Content-Length
    /// body bytes. Panics (loudly, failing the test via join) on any
    /// protocol surprise; STUB_IO_TIMEOUT read timeout keeps a broken
    /// client from hanging the suite (T6).
    fn read_request(stream: &mut TcpStream) -> Observed {
        read_request_within(stream, STUB_IO_TIMEOUT)
    }

    /// `read_request` with an explicit read timeout — short-timeout variant
    /// for tests that exercise the stub's own fail-fast behavior.
    fn read_request_within(stream: &mut TcpStream, timeout: Duration) -> Observed {
        stream.set_read_timeout(Some(timeout)).unwrap();
        let mut head = Vec::new();
        let mut byte = [0u8; 1];
        loop {
            let n = stream.read(&mut byte).expect("read request head");
            assert!(n == 1, "connection closed mid-request");
            head.push(byte[0]);
            assert!(head.len() < 64 * 1024, "request head too large");
            if head.ends_with(b"\r\n\r\n") {
                break;
            }
        }
        let head = String::from_utf8(head).unwrap();
        let mut lines = head.split("\r\n");
        let request_line = lines.next().unwrap().to_string();
        let mut headers = Vec::new();
        let mut content_len = 0usize;
        for line in lines {
            if line.is_empty() {
                continue;
            }
            let (k, v) = line.split_once(':').expect("header line");
            let v = v.trim().to_string();
            if k.eq_ignore_ascii_case("content-length") {
                content_len = v.parse().expect("content-length number");
            }
            headers.push((k.to_string(), v));
        }
        let mut body = vec![0u8; content_len];
        stream.read_exact(&mut body).expect("read request body");
        Observed {
            request_line,
            headers,
            body: String::from_utf8(body).unwrap(),
        }
    }

    /// Content-Length-delimited response; keep-alive so the pooled client
    /// reuses this one connection for the whole test.
    fn write_response(stream: &mut TcpStream, status: u16, headers: &[(&str, &str)], body: &[u8]) {
        write_response_impl(stream, status, headers, body, "keep-alive");
    }

    /// Same, but `connection: close` — forces the client's next request onto
    /// a FRESH connection, which makes the stub's accept() order
    /// deterministic when a listen stream is also open.
    fn write_response_close(stream: &mut TcpStream, status: u16, headers: &[(&str, &str)], body: &[u8]) {
        write_response_impl(stream, status, headers, body, "close");
    }

    fn write_response_impl(
        stream: &mut TcpStream,
        status: u16,
        headers: &[(&str, &str)],
        body: &[u8],
        connection: &str,
    ) {
        let reason = match status {
            200 => "OK",
            202 => "Accepted",
            _ => "Status",
        };
        let mut resp = format!("HTTP/1.1 {status} {reason}\r\n");
        for (k, v) in headers {
            resp.push_str(k);
            resp.push_str(": ");
            resp.push_str(v);
            resp.push_str("\r\n");
        }
        resp.push_str(&format!(
            "content-length: {}\r\nconnection: {connection}\r\n\r\n",
            body.len()
        ));
        stream.write_all(resp.as_bytes()).unwrap();
        stream.write_all(body).unwrap();
        stream.flush().unwrap();
    }

    /// SSE response head: no Content-Length, `connection: close` — the body
    /// is delimited by the connection closing, so the stub can hold the
    /// stream open mid-test (proving the client reads incrementally).
    fn write_sse_head(stream: &mut TcpStream) {
        stream
            .write_all(b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n")
            .unwrap();
        stream.flush().unwrap();
    }

    fn write_sse_event(stream: &mut TcpStream, data: &str) {
        stream
            .write_all(format!("data: {data}\r\n\r\n").as_bytes())
            .unwrap();
        stream.flush().unwrap();
    }

    fn bind_stub() -> (TcpListener, String) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        (listener, url)
    }

    /// Stub-side I/O ceiling (T6): every blocking operation a stub thread
    /// performs is bounded so a broken client fails the test in seconds
    /// instead of hanging the whole suite on `accept()`/`read()`/`write()`.
    const STUB_IO_TIMEOUT: Duration = Duration::from_secs(10);
    const STUB_ACCEPT_TIMEOUT: Duration = Duration::from_secs(10);

    /// Accept one connection with a deadline (nonblocking poll): a client
    /// that never connects panics the stub thread — failing the test via
    /// `join` — instead of blocking it forever. Accepted streams carry
    /// read+write timeouts so a wedged peer fails fast mid-exchange too.
    fn accept_conn(listener: &TcpListener) -> TcpStream {
        accept_conn_within(listener, STUB_ACCEPT_TIMEOUT)
    }

    fn accept_conn_within(listener: &TcpListener, deadline: Duration) -> TcpStream {
        listener.set_nonblocking(true).unwrap();
        let t0 = Instant::now();
        loop {
            match listener.accept() {
                Ok((stream, _)) => {
                    // macOS accepted sockets inherit the listener's
                    // nonblocking flag — force blocking mode so all stub
                    // I/O is governed by the explicit timeouts below
                    // (SO_RCVTIMEO fires as WouldBlock/TimedOut).
                    stream.set_nonblocking(false).unwrap();
                    stream.set_read_timeout(Some(STUB_IO_TIMEOUT)).unwrap();
                    stream.set_write_timeout(Some(STUB_IO_TIMEOUT)).unwrap();
                    return stream;
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(
                        t0.elapsed() < deadline,
                        "stub accept timed out after {deadline:?}: client never connected"
                    );
                    thread::sleep(Duration::from_millis(5));
                }
                Err(e) => panic!("accept failed: {e}"),
            }
        }
    }

    /// Serve the three handshake exchanges on an accepted connection:
    /// initialize (200 JSON, issuing `session`), notifications/initialized
    /// (202, no body), tools/list (200 JSON, one `echo` tool). Asserts the
    /// session id is replayed on every request after initialize.
    fn serve_handshake(stream: &mut TcpStream, session: Option<&str>) {
        serve_handshake_impl(stream, session, false);
    }

    /// Handshake variant whose tools/list response closes the connection:
    /// every later POST lands on a FRESH connection, so the stub's accept()
    /// order stays deterministic once the listen stream is also open.
    fn serve_handshake_close(stream: &mut TcpStream, session: Option<&str>) {
        serve_handshake_impl(stream, session, true);
    }

    fn serve_handshake_impl(stream: &mut TcpStream, session: Option<&str>, close_last: bool) {
        let req = read_request(stream);
        assert_eq!(req.json()["method"], "initialize");
        assert_eq!(req.json()["id"], 1);
        let session_headers: Vec<(&str, &str)> = match session {
            Some(s) => vec![("content-type", "application/json"), ("mcp-session-id", s)],
            None => vec![("content-type", "application/json")],
        };
        write_response(
            stream,
            200,
            &session_headers,
            br#"{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-06-18","capabilities":{"tools":{}},"serverInfo":{"name":"stub","version":"0.0.1"}}}"#,
        );
        // notifications/initialized → 202 Accepted with no body
        let req = read_request(stream);
        assert_eq!(req.json()["method"], "notifications/initialized");
        assert!(req.json().get("id").is_none(), "notification must carry no id");
        if let Some(s) = session {
            assert_eq!(req.header("mcp-session-id"), Some(s), "session id on notification");
        }
        write_response(stream, 202, &[], b"");
        // tools/list → one echo tool
        let req = read_request(stream);
        assert_eq!(req.json()["method"], "tools/list");
        assert_eq!(req.json()["id"], 2);
        if let Some(s) = session {
            assert_eq!(req.header("mcp-session-id"), Some(s), "session id on tools/list");
        }
        let body = br#"{"jsonrpc":"2.0","id":2,"result":{"tools":[{"name":"echo","description":"echo the arguments","inputSchema":{"type":"object","properties":{"text":{"type":"string"}}}}]}}"#;
        if close_last {
            write_response_close(stream, 200, &[("content-type", "application/json")], body);
        } else {
            write_response(stream, 200, &[("content-type", "application/json")], body);
        }
    }

    fn new_server(name: &str, url: String) -> HttpMcpServer {
        HttpMcpServer::new(name.to_string(), url, vec![]).unwrap()
    }

    type Sleeper = Arc<dyn Fn(Duration) + Send + Sync>;

    fn no_sleep() -> (Sleeper, Arc<Mutex<Vec<Duration>>>) {
        let slept = Arc::new(Mutex::new(Vec::new()));
        let record = Arc::clone(&slept);
        let sleeper: Sleeper = Arc::new(move |d| record.lock().unwrap().push(d));
        (sleeper, slept)
    }

    // ---------- (a)+(b)+(c): handshake via registry, session replay, 202 ----------

    #[test]
    fn registry_handshake_session_replay_and_call() {
        let (listener, url) = bind_stub();
        let stub = thread::spawn(move || {
            let mut stream = accept_conn(&listener);
            serve_handshake(&mut stream, Some("sess-abc"));
            let req = read_request(&mut stream);
            assert_eq!(req.json()["method"], "tools/call");
            assert_eq!(req.json()["id"], 3);
            assert_eq!(req.json()["params"]["name"], "echo");
            assert_eq!(req.header("mcp-session-id"), Some("sess-abc"), "session id on tools/call");
            write_response(
                &mut stream,
                200,
                &[("content-type", "application/json")],
                br#"{"jsonrpc":"2.0","id":3,"result":{"content":[{"type":"text","text":"stub says hi"}],"isError":false}}"#,
            );
        });
        let tmp = TempDir::new().unwrap();
        let cfg = json!({"mcpServers": {"remote": {"url": url, "transport": "http"}}});
        std::fs::write(tmp.path().join("mcp.json"), cfg.to_string()).unwrap();

        let mut reg = McpRegistry::new(tmp.path(), false, None).unwrap();
        let schemas = reg.tool_schemas();
        assert_eq!(schemas.len(), 1);
        assert_eq!(schemas[0]["name"], "mcp__remote__echo");
        assert_eq!(schemas[0]["description"], "echo the arguments");

        let res = reg.dispatch("mcp__remote__echo", json!({"text": "yo"}));
        assert!(!res.is_error, "{}", res.content);
        assert_eq!(res.content, "stub says hi");
        stub.join().unwrap();
    }

    /// Configured headers (e.g. Authorization with an expanded ${VAR}) ride
    /// every request.
    #[test]
    fn configured_headers_sent_with_expanded_values() {
        let (listener, url) = bind_stub();
        let stub = thread::spawn(move || {
            let mut stream = accept_conn(&listener);
            for _ in 0..3 {
                let req = read_request(&mut stream);
                assert_eq!(req.header("authorization"), Some("Bearer tok123"));
                let id = req.json()["id"].as_u64();
                match req.json()["method"].as_str().unwrap() {
                    "initialize" => write_response(
                        &mut stream,
                        200,
                        &[("content-type", "application/json")],
                        br#"{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-06-18","capabilities":{},"serverInfo":{"name":"s","version":"0"}}}"#,
                    ),
                    "notifications/initialized" => write_response(&mut stream, 202, &[], b""),
                    "tools/list" => write_response(
                        &mut stream,
                        200,
                        &[("content-type", "application/json")],
                        br#"{"jsonrpc":"2.0","id":2,"result":{"tools":[]}}"#,
                    ),
                    m => panic!("unexpected method {m} (id {id:?})"),
                }
            }
        });
        unsafe { std::env::set_var("CHUG_TEST_TOKEN", "tok123") };
        let tmp = TempDir::new().unwrap();
        let cfg = json!({"mcpServers": {"remote": {
            "url": url,
            "headers": {"Authorization": "Bearer ${CHUG_TEST_TOKEN}"}
        }}});
        std::fs::write(tmp.path().join("mcp.json"), cfg.to_string()).unwrap();
        let reg = McpRegistry::new(tmp.path(), false, None).unwrap();
        assert!(reg.tool_schemas().is_empty());
        stub.join().unwrap();
    }

    // ---------- (d): SSE-framed response, multi-line data, held-open stream ----------

    #[test]
    fn sse_framed_call_response_multiline_data_stream_held_open() {
        let (listener, url) = bind_stub();
        let (release_tx, release_rx) = mpsc::channel::<()>();
        let stub = thread::spawn(move || {
            let mut stream = accept_conn(&listener);
            serve_handshake(&mut stream, None);
            let req = read_request(&mut stream);
            assert_eq!(req.json()["method"], "tools/call");
            write_sse_head(&mut stream);
            // comment line, event line, and the JSON response split across
            // two data: lines (joined with \n by the parser) — still valid
            // JSON — framed CRLF.
            stream
                .write_all(
                    b": keepalive\r\nevent: message\r\ndata: {\"jsonrpc\":\"2.0\",\"id\":3,\r\ndata: \"result\":{\"content\":[{\"type\":\"text\",\"text\":\"sse ok\"}],\"isError\":false}}\r\n\r\n",
                )
                .unwrap();
            stream.flush().unwrap();
            // Hold the stream open: the client must have its answer already.
            release_rx.recv_timeout(Duration::from_secs(10)).unwrap();
        });
        let mut srv = new_server("remote", url);
        srv.initialize().unwrap();
        let res = srv.call("echo", json!({})).unwrap();
        assert!(!res.is_error, "{}", res.content);
        assert_eq!(res.content, "sse ok");
        // The call returned while the server still held the SSE stream open.
        release_tx.send(()).unwrap();
        stub.join().unwrap();
    }

    // ---------- server→client request over SSE: method-not-found reply ----------

    #[test]
    fn server_request_over_sse_gets_method_not_found_reply() {
        let (listener, url) = bind_stub();
        let stub = thread::spawn(move || {
            let mut c1 = accept_conn(&listener);
            serve_handshake(&mut c1, Some("sess-9"));
            let req = read_request(&mut c1);
            assert_eq!(req.json()["method"], "tools/call");
            write_sse_head(&mut c1);
            // A server notification (dropped) and a server→client request.
            write_sse_event(&mut c1, r#"{"jsonrpc":"2.0","method":"notifications/progress","params":{"progress":1}}"#);
            write_sse_event(&mut c1, r#"{"jsonrpc":"2.0","id":99,"method":"sampling/createMessage","params":{}}"#);
            // The client must answer the request with its own POST — on a
            // fresh connection, since c1 is busy streaming. The listen
            // stream's GET (opened right after the handshake, on its own
            // connection) races that POST at accept(): answer GETs 405 so
            // the listen manager stops trying, and keep accepting until the
            // reply POST arrives.
            let (mut c2, reply) = loop {
                let mut s = accept_conn(&listener);
                let req = read_request(&mut s);
                if req.is_get() {
                    write_response(&mut s, 405, &[("content-type", "text/plain")], b"no listen stream");
                    continue;
                }
                break (s, req);
            };
            let j = reply.json();
            assert_eq!(j["id"], 99);
            assert_eq!(j["error"]["code"], -32601);
            assert!(j["error"]["message"].as_str().unwrap().contains("method not found"));
            assert_eq!(reply.header("mcp-session-id"), Some("sess-9"));
            write_response(&mut c2, 202, &[], b"");
            // …and only then does the real tools/call response arrive.
            write_sse_event(&mut c1, r#"{"jsonrpc":"2.0","id":3,"result":{"content":[{"type":"text","text":"done"}],"isError":false}}"#);
        });
        let mut srv = new_server("remote", url);
        srv.initialize().unwrap();
        let res = srv.call("echo", json!({})).unwrap();
        assert!(!res.is_error, "{}", res.content);
        assert_eq!(res.content, "done");
        stub.join().unwrap();
    }

    // ---------- (e): dead server fail-soft, retries without real sleeping ----------

    #[test]
    fn dead_server_retries_then_tool_error_without_sleeping() {
        // Bind + drop to get a port nothing listens on (connection refused).
        let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        let (sleeper, slept) = no_sleep();
        let mut srv = HttpMcpServer::with_sleeper(
            "remote".to_string(),
            format!("http://127.0.0.1:{port}"),
            vec![],
            sleeper,
        )
        .unwrap();

        let res = srv.call("echo", json!({})).unwrap();
        assert!(res.is_error);
        assert!(
            res.content.contains("POST failed after retries"),
            "{}",
            res.content
        );
        // Exactly the 1s/2s/4s schedule ran — through the injected sleeper,
        // so the test never really slept.
        assert_eq!(
            slept.lock().unwrap().as_slice(),
            &[Duration::from_secs(1), Duration::from_secs(2), Duration::from_secs(4)]
        );
        // Marked down: the next call short-circuits (no new retries).
        assert!(!srv.is_alive());
        let res = srv.call("echo", json!({})).unwrap();
        assert!(res.is_error);
        assert_eq!(res.content, "mcp server remote is down");
        assert_eq!(slept.lock().unwrap().len(), 3, "no further retries once down");

        // The handshake against a dead server also fails (registry skips it).
        let (sleeper2, _) = no_sleep();
        let mut srv2 = HttpMcpServer::with_sleeper(
            "remote".to_string(),
            format!("http://127.0.0.1:{port}"),
            vec![],
            sleeper2,
        )
        .unwrap();
        assert!(srv2.initialize().is_err());
    }

    /// HTTP statuses are immediate errors — no retry schedule, no sleep.
    #[test]
    fn http_status_is_immediate_error_without_retry() {
        let (listener, url) = bind_stub();
        let stub = thread::spawn(move || {
            let mut stream = accept_conn(&listener);
            let _req = read_request(&mut stream);
            write_response(&mut stream, 500, &[("content-type", "text/plain")], b"nope");
        });
        let (sleeper, slept) = no_sleep();
        let mut srv =
            HttpMcpServer::with_sleeper("remote".to_string(), url, vec![], sleeper).unwrap();
        let err = srv.initialize().unwrap_err();
        assert!(format!("{err:#}").contains("HTTP 500"), "{err:#}");
        assert!(slept.lock().unwrap().is_empty(), "HTTP statuses never retry");
        stub.join().unwrap();
    }

    /// The registry skips a remote whose handshake fails and continues the
    /// run — never aborts.
    #[test]
    fn registry_skips_remote_on_handshake_failure() {
        let (listener, url) = bind_stub();
        let stub = thread::spawn(move || {
            let mut stream = accept_conn(&listener);
            let _req = read_request(&mut stream);
            write_response(&mut stream, 500, &[("content-type", "text/plain")], b"nope");
        });
        let tmp = TempDir::new().unwrap();
        let cfg = json!({"mcpServers": {"remote": {"url": url}}});
        std::fs::write(tmp.path().join("mcp.json"), cfg.to_string()).unwrap();
        let reg = McpRegistry::new(tmp.path(), false, None).unwrap();
        assert!(reg.tool_schemas().is_empty());
        stub.join().unwrap();
    }

    // ---------- response mapping ----------

    #[test]
    fn parse_call_response_maps_error_and_text_like_stdio() {
        let resp = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "error": {"code": -32602, "message": "unknown tool: nope"}
        });
        let res = parse_call_response(&resp);
        assert!(res.is_error);
        assert_eq!(res.content, "unknown tool: nope");

        let resp = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "content": [
                    {"type": "text", "text": "line one"},
                    {"type": "image", "data": "..."},
                    {"type": "text", "text": "line two"}
                ],
                "isError": true
            }
        });
        let res = parse_call_response(&resp);
        assert!(res.is_error);
        assert_eq!(res.content, "line one\nline two");
    }

    // ---------- round 3: GET listen stream ----------

    /// A server→client request pushed over the GET listen stream is answered
    /// with a JSON-RPC -32601 reply POST carrying the pushed id and the
    /// session header — same rule as stdio.
    #[test]
    fn listen_stream_request_gets_method_not_found_reply() {
        let (listener, url) = bind_stub();
        let stub = thread::spawn(move || {
            let mut c1 = accept_conn(&listener);
            serve_handshake_close(&mut c1, Some("sess-L"));
            // The listen GET arrives next, carrying the SSE accept header
            // and the session id captured during the handshake.
            let mut c2 = accept_conn(&listener);
            let get = read_request(&mut c2);
            assert!(get.is_get(), "expected listen GET, got {}", get.request_line);
            assert!(
                get.header("accept").unwrap().contains("text/event-stream"),
                "listen GET accept header"
            );
            assert_eq!(get.header("mcp-session-id"), Some("sess-L"));
            write_sse_head(&mut c2);
            write_sse_event(&mut c2, r#"{"jsonrpc":"2.0","id":77,"method":"sampling/createMessage","params":{}}"#);
            // The client answers with its own POST on a fresh connection.
            let mut c3 = accept_conn(&listener);
            let reply = read_request(&mut c3);
            assert!(!reply.is_get());
            let j = reply.json();
            assert_eq!(j["id"], 77);
            assert_eq!(j["error"]["code"], -32601);
            assert!(j["error"]["message"].as_str().unwrap().contains("method not found"));
            assert_eq!(reply.header("mcp-session-id"), Some("sess-L"));
            write_response(&mut c3, 202, &[], b"");
        });
        let mut srv = new_server("remote", url);
        srv.initialize().unwrap();
        stub.join().unwrap();
        drop(srv);
    }

    /// A notification pushed over the GET listen stream is dropped: no
    /// reply POST, and a subsequent tools/call works normally.
    #[test]
    fn listen_stream_notification_is_dropped_without_reply() {
        let (listener, url) = bind_stub();
        let stub = thread::spawn(move || {
            let mut c1 = accept_conn(&listener);
            // Keep-alive handshake: the later tools/call reuses c1 through
            // the connection pool, so the listen GET is the only NEW
            // connection after the handshake (no accept-order race).
            serve_handshake(&mut c1, None);
            let mut c2 = accept_conn(&listener);
            let get = read_request(&mut c2);
            assert!(get.is_get(), "expected listen GET, got {}", get.request_line);
            write_sse_head(&mut c2);
            write_sse_event(&mut c2, r#"{"jsonrpc":"2.0","method":"notifications/progress","params":{"progress":1}}"#);
            // The test now makes a tools/call, on the pooled c1.
            let req = read_request(&mut c1);
            assert_eq!(req.json()["method"], "tools/call");
            write_response(
                &mut c1,
                200,
                &[("content-type", "application/json")],
                br#"{"jsonrpc":"2.0","id":3,"result":{"content":[{"type":"text","text":"ok"}],"isError":false}}"#,
            );
            // Settle window: any (buggy) reply to the notification would
            // arrive as a new connection here.
            listener.set_nonblocking(true).unwrap();
            let mut unexpected = Vec::new();
            let t0 = Instant::now();
            while t0.elapsed() < Duration::from_millis(400) {
                match listener.accept() {
                    Ok((mut s, _)) => {
                        s.set_nonblocking(false).unwrap();
                        let req = read_request(&mut s);
                        unexpected.push(req.request_line.clone());
                        write_response(&mut s, 202, &[], b"");
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(e) => panic!("accept failed: {e}"),
                }
            }
            assert!(unexpected.is_empty(), "notification must not be answered: {unexpected:?}");
        });
        let mut srv = new_server("remote", url);
        srv.initialize().unwrap();
        let res = srv.call("echo", json!({})).unwrap();
        assert!(!res.is_error, "{}", res.content);
        assert_eq!(res.content, "ok");
        stub.join().unwrap();
        drop(srv);
    }

    /// A dropped listen stream is reconnected (fast, through the injected
    /// sleeper), and the reconnect GET carries Last-Event-ID from the last
    /// id-bearing event plus the session header. Across two consecutive
    /// drop→reconnect cycles the backoff schedule RESETS after each
    /// successfully opened stream: the second wait starts at ~1s again
    /// instead of climbing to 2s.
    #[test]
    fn listen_stream_reconnects_with_last_event_id_after_drop() {
        let (listener, url) = bind_stub();
        let (seen_tx, seen_rx) = mpsc::channel::<()>();
        let (ack_tx, ack_rx) = mpsc::channel::<()>();
        let (release_tx, release_rx) = mpsc::channel::<()>();
        let stub = thread::spawn(move || {
            let mut c1 = accept_conn(&listener);
            serve_handshake_close(&mut c1, Some("sess-R"));
            // First listen stream: one id-bearing event, then the server
            // drops the connection.
            let mut c2 = accept_conn(&listener);
            let get1 = read_request(&mut c2);
            assert!(get1.is_get());
            assert!(get1.header("last-event-id").is_none(), "no Last-Event-ID on first connect");
            write_sse_head(&mut c2);
            c2.write_all(b"id: evt-42\r\ndata: {\"jsonrpc\":\"2.0\",\"method\":\"notifications/progress\"}\r\n\r\n").unwrap();
            c2.flush().unwrap();
            drop(c2);
            // Reconnect: Last-Event-ID + session ride the second GET.
            let mut c3 = accept_conn(&listener);
            let get2 = read_request(&mut c3);
            assert!(get2.is_get());
            assert_eq!(get2.header("last-event-id"), Some("evt-42"));
            assert_eq!(get2.header("mcp-session-id"), Some("sess-R"));
            write_sse_head(&mut c3);
            seen_tx.send(()).unwrap();
            // Wait for the test's ack so it can snapshot the sleeper
            // recording BEFORE the second cycle adds to it.
            ack_rx.recv_timeout(Duration::from_secs(10)).unwrap();
            // Second cycle: another id-bearing event, then dropped again.
            c3.write_all(b"id: evt-43\r\ndata: {\"jsonrpc\":\"2.0\",\"method\":\"notifications/progress\"}\r\n\r\n").unwrap();
            c3.flush().unwrap();
            drop(c3);
            // Third GET: Last-Event-ID advanced to the second stream's id.
            let mut c4 = accept_conn(&listener);
            let get3 = read_request(&mut c4);
            assert!(get3.is_get());
            assert_eq!(get3.header("last-event-id"), Some("evt-43"));
            assert_eq!(get3.header("mcp-session-id"), Some("sess-R"));
            write_sse_head(&mut c4);
            seen_tx.send(()).unwrap();
            // Hold the third stream open until the server is dropped (a
            // reconnect loop against a closed listener would spin on the
            // no-op sleeper).
            release_rx.recv_timeout(Duration::from_secs(10)).unwrap();
        });
        let (sleeper, slept) = no_sleep();
        let mut srv = HttpMcpServer::with_sleeper("remote".to_string(), url, vec![], sleeper).unwrap();
        srv.initialize().unwrap();
        // Backoff #1 ran (in ≤100ms chunks through the injected sleeper)
        // before GET#2 was opened.
        seen_rx.recv_timeout(Duration::from_secs(10)).unwrap();
        let first: Duration = slept.lock().unwrap().iter().sum();
        ack_tx.send(()).unwrap();
        // Backoff #2 ran between GET#2's drop and GET#3.
        seen_rx.recv_timeout(Duration::from_secs(10)).unwrap();
        let total: Duration = slept.lock().unwrap().iter().sum();
        let second = total - first;
        // Both waits started at the jittered 1s floor ([0.5s, 1s)) — a
        // schedule that kept climbing would make the second wait ≥ 1s
        // (2s × jitter).
        for (label, d) in [("first", first), ("second", second)] {
            assert!(
                d >= Duration::from_millis(500) && d < Duration::from_secs(1),
                "{label} reconnect backoff must restart at ~1s after an open stream, got {d:?}"
            );
        }
        drop(srv);
        release_tx.send(()).unwrap();
        stub.join().unwrap();
    }

    /// GET answered 405: the server offers no listen stream — one log note,
    /// and no reconnect storm.
    #[test]
    fn listen_get_405_disables_listen_stream_without_reconnect_storm() {
        let (listener, url) = bind_stub();
        let stub = thread::spawn(move || {
            let mut c1 = accept_conn(&listener);
            serve_handshake_close(&mut c1, None);
            let mut c2 = accept_conn(&listener);
            let get = read_request(&mut c2);
            assert!(get.is_get());
            write_response_close(&mut c2, 405, &[("content-type", "text/plain")], b"no listen stream");
            // Storm watch: after a 405 the client must not retry the GET.
            listener.set_nonblocking(true).unwrap();
            let mut retries = 0;
            let t0 = Instant::now();
            while t0.elapsed() < Duration::from_millis(400) {
                match listener.accept() {
                    Ok((mut s, _)) => {
                        retries += 1;
                        s.set_nonblocking(false).unwrap();
                        let _req = read_request(&mut s);
                        write_response_close(&mut s, 405, &[], b"");
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(e) => panic!("accept failed: {e}"),
                }
            }
            assert_eq!(retries, 0, "405 must stop the listen stream, not retry it");
        });
        let tmp = TempDir::new().unwrap();
        let log_path = tmp.path().join("mcp-remote.log");
        let (sleeper, _slept) = no_sleep();
        let mut srv = HttpMcpServer::with_sleeper("remote".to_string(), url, vec![], sleeper).unwrap();
        srv.set_log_path(log_path.clone());
        srv.initialize().unwrap();
        stub.join().unwrap();
        // Drop joins the listen manager first, so the 405 note is on disk.
        drop(srv);
        let log = std::fs::read_to_string(&log_path).expect("405 note logged");
        assert!(log.contains("405"), "log should note the missing listen stream: {log}");
    }

    /// Drop with a wedged (established but silent) listen stream: the
    /// shutdown flag wakes the manager within one poll and the bounded join
    /// returns fast — no hang, no unbounded wait on the wedged GET worker.
    #[test]
    fn drop_with_wedged_listen_stream_completes_fast() {
        let (listener, url) = bind_stub();
        let (seen_tx, seen_rx) = mpsc::channel::<()>();
        let (release_tx, release_rx) = mpsc::channel::<()>();
        let stub = thread::spawn(move || {
            let mut c1 = accept_conn(&listener);
            serve_handshake(&mut c1, None);
            let mut c2 = accept_conn(&listener);
            let _get = read_request(&mut c2);
            write_sse_head(&mut c2);
            seen_tx.send(()).unwrap();
            // Wedged: 200 SSE head sent, then silence until released.
            release_rx.recv_timeout(Duration::from_secs(10)).unwrap();
        });
        let mut srv = new_server("remote", url);
        srv.initialize().unwrap();
        // The listen stream is established and silent.
        seen_rx.recv_timeout(Duration::from_secs(10)).unwrap();
        let t0 = Instant::now();
        drop(srv);
        let elapsed = t0.elapsed();
        assert!(
            elapsed < Duration::from_secs(3),
            "Drop blocked for {elapsed:?} on a silent listen stream"
        );
        release_tx.send(()).unwrap();
        stub.join().unwrap();
    }

    /// A healthy but SILENT listen stream must sit open without churn: no
    /// reconnect (no second GET) and no backoff sleeps while no events
    /// arrive. Guards the `.timeout(None)` fix — a reqwest-level read
    /// timeout would kill the stream and trigger spurious reconnects.
    #[test]
    fn silent_listen_stream_is_not_reconnected() {
        let (listener, url) = bind_stub();
        let (seen_tx, seen_rx) = mpsc::channel::<()>();
        let (watch_done_tx, watch_done_rx) = mpsc::channel::<()>();
        let (release_tx, release_rx) = mpsc::channel::<()>();
        let stub = thread::spawn(move || {
            let mut c1 = accept_conn(&listener);
            serve_handshake_close(&mut c1, None);
            let mut c2 = accept_conn(&listener);
            let get = read_request(&mut c2);
            assert!(get.is_get(), "expected listen GET, got {}", get.request_line);
            write_sse_head(&mut c2);
            seen_tx.send(()).unwrap();
            // Storm watch: while the established stream sits silent, no
            // second connection may arrive.
            listener.set_nonblocking(true).unwrap();
            let mut extra = 0;
            let t0 = Instant::now();
            while t0.elapsed() < Duration::from_millis(1500) {
                match listener.accept() {
                    Ok((mut s, _)) => {
                        extra += 1;
                        s.set_nonblocking(false).unwrap();
                        let _ = read_request(&mut s);
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(e) => panic!("accept failed: {e}"),
                }
            }
            assert_eq!(extra, 0, "silent listen stream must not be reconnected");
            watch_done_tx.send(()).unwrap();
            release_rx.recv_timeout(Duration::from_secs(10)).unwrap();
        });
        let (sleeper, slept) = no_sleep();
        let mut srv = HttpMcpServer::with_sleeper("remote".to_string(), url, vec![], sleeper).unwrap();
        srv.initialize().unwrap();
        // The listen stream is established; the stub watches for extra
        // connections during a 1.5s silent window.
        seen_rx.recv_timeout(Duration::from_secs(10)).unwrap();
        watch_done_rx.recv_timeout(Duration::from_secs(10)).unwrap();
        // No reconnect cycle ran: the injected sleeper never saw a backoff.
        assert!(slept.lock().unwrap().is_empty(), "silent stream must not back off");
        drop(srv);
        release_tx.send(()).unwrap();
        stub.join().unwrap();
    }

    /// An SSE-framed tools/call response arriving after a multi-second
    /// silence still succeeds: only the explicit per-call total deadline
    /// (60s) governs — no reqwest-level read timeout may fire first.
    #[test]
    fn sse_call_response_after_delayed_gap_succeeds() {
        let (listener, url) = bind_stub();
        let stub = thread::spawn(move || {
            let mut stream = accept_conn(&listener);
            serve_handshake(&mut stream, None);
            let req = read_request(&mut stream);
            assert_eq!(req.json()["method"], "tools/call");
            write_sse_head(&mut stream);
            // Multi-second gap before the answer: legal under the 60s
            // per-call budget.
            thread::sleep(Duration::from_secs(2));
            write_sse_event(&mut stream, r#"{"jsonrpc":"2.0","id":3,"result":{"content":[{"type":"text","text":"late ok"}],"isError":false}}"#);
        });
        let mut srv = new_server("remote", url);
        srv.initialize().unwrap();
        let res = srv.call("echo", json!({})).unwrap();
        assert!(!res.is_error, "{}", res.content);
        assert_eq!(res.content, "late ok");
        stub.join().unwrap();
        drop(srv);
    }

    /// A fully-stalled SSE response (head arrived, body never) still fails
    /// the call at the per-call total deadline: the caller-side deadline
    /// governs even while the worker thread stays parked on the stalled
    /// read (the clients carry no reqwest-level timeout to rely on).
    #[test]
    fn stalled_call_fails_at_overridden_total_deadline() {
        let (listener, url) = bind_stub();
        let (release_tx, release_rx) = mpsc::channel::<()>();
        let stub = thread::spawn(move || {
            let mut stream = accept_conn(&listener);
            serve_handshake(&mut stream, None);
            let req = read_request(&mut stream);
            assert_eq!(req.json()["method"], "tools/call");
            write_sse_head(&mut stream);
            // Fully stalled body until released.
            release_rx.recv_timeout(Duration::from_secs(30)).unwrap();
        });
        let mut srv = new_server("remote", url);
        srv.initialize().unwrap();
        srv.call_timeout = Duration::from_millis(600);
        let t0 = Instant::now();
        let res = srv.call("echo", json!({})).unwrap();
        let elapsed = t0.elapsed();
        assert!(res.is_error, "stalled call must fail: {}", res.content);
        assert!(res.content.contains("timed out"), "{}", res.content);
        assert!(
            elapsed < Duration::from_secs(5),
            "call must fail near the overridden 600ms deadline, took {elapsed:?}"
        );
        release_tx.send(()).unwrap();
        stub.join().unwrap();
        drop(srv);
    }

    /// Drop while the listen manager is mid-backoff must be prompt: the
    /// interruptible sleep notices the shutdown flag between chunks instead
    /// of sleeping out the remaining delay (which would eat the join grace).
    #[test]
    fn drop_during_listen_backoff_is_prompt() {
        let (listener, url) = bind_stub();
        let stub = thread::spawn(move || {
            let mut c1 = accept_conn(&listener);
            serve_handshake_close(&mut c1, None);
            let mut c2 = accept_conn(&listener);
            let get = read_request(&mut c2);
            assert!(get.is_get());
            // Refuse the listen stream: FailedToOpen → backoff sleep.
            write_response_close(&mut c2, 500, &[("content-type", "text/plain")], b"nope");
            // Nothing more: the manager must now be parked in its backoff.
        });
        // Gated sleeper: reports each chunk, then parks until the test
        // releases it — so the manager is provably INSIDE a backoff sleep.
        let (calls_tx, calls_rx) = mpsc::channel::<Duration>();
        let (release_tx, release_rx) = mpsc::channel::<()>();
        let release_rx = Arc::new(Mutex::new(release_rx));
        let sleeper: Sleeper = Arc::new(move |d| {
            let _ = calls_tx.send(d);
            let _ = release_rx.lock().unwrap().recv();
        });
        let mut srv = HttpMcpServer::with_sleeper("remote".to_string(), url, vec![], sleeper).unwrap();
        srv.initialize().unwrap();
        stub.join().unwrap();
        // The manager is parked inside the first backoff chunk.
        calls_rx.recv_timeout(Duration::from_secs(10)).unwrap();
        let shutdown = Arc::clone(&srv.shutdown);
        let t0 = Instant::now();
        let dropper = thread::spawn(move || drop(srv));
        // Wait until Drop has signalled shutdown, then let the in-flight
        // chunk return — the manager must exit right after it.
        for _ in 0..1000 {
            if shutdown.load(Ordering::Relaxed) {
                break;
            }
            thread::sleep(Duration::from_millis(1));
        }
        release_tx.send(()).unwrap();
        dropper.join().unwrap();
        let elapsed = t0.elapsed();
        assert!(
            elapsed < Duration::from_secs(3),
            "Drop during backoff took {elapsed:?} (interruptible_sleep missed shutdown)"
        );
    }

    /// SSE call-response demux: events carrying a WRONG id (a foreign
    /// response, even an error) are ignored; the call completes only when
    /// the matching-id response arrives.
    #[test]
    fn sse_call_response_ignores_wrong_id_until_matching() {
        let (listener, url) = bind_stub();
        let stub = thread::spawn(move || {
            let mut stream = accept_conn(&listener);
            serve_handshake(&mut stream, None);
            let req = read_request(&mut stream);
            assert_eq!(req.json()["method"], "tools/call");
            write_sse_head(&mut stream);
            // Foreign-id responses first: the client must keep waiting.
            write_sse_event(&mut stream, r#"{"jsonrpc":"2.0","id":998,"error":{"code":-32603,"message":"not ours"}}"#);
            write_sse_event(&mut stream, r#"{"jsonrpc":"2.0","id":999,"result":{"content":[{"type":"text","text":"wrong one"}],"isError":false}}"#);
            write_sse_event(&mut stream, r#"{"jsonrpc":"2.0","id":3,"result":{"content":[{"type":"text","text":"right one"}],"isError":false}}"#);
        });
        let mut srv = new_server("remote", url);
        srv.initialize().unwrap();
        let res = srv.call("echo", json!({})).unwrap();
        assert!(!res.is_error, "{}", res.content);
        assert_eq!(res.content, "right one");
        stub.join().unwrap();
        drop(srv);
    }

    /// tools/list is capped at MAX_MCP_TOOLS (200), with a log note.
    #[test]
    fn tools_list_capped_at_max_with_log_note() {
        let (listener, url) = bind_stub();
        let stub = thread::spawn(move || {
            let mut stream = accept_conn(&listener);
            let req = read_request(&mut stream);
            assert_eq!(req.json()["method"], "initialize");
            write_response(
                &mut stream,
                200,
                &[("content-type", "application/json")],
                br#"{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-06-18","capabilities":{},"serverInfo":{"name":"s","version":"0"}}}"#,
            );
            let req = read_request(&mut stream);
            assert_eq!(req.json()["method"], "notifications/initialized");
            write_response(&mut stream, 202, &[], b"");
            let req = read_request(&mut stream);
            assert_eq!(req.json()["method"], "tools/list");
            // Five over the cap.
            let tools: Vec<Value> = (0..(MAX_MCP_TOOLS + 5))
                .map(|i| json!({"name": format!("t{i}"), "description": "d", "inputSchema": {"type": "object"}}))
                .collect();
            let body = json!({"jsonrpc": "2.0", "id": 2, "result": {"tools": tools}});
            write_response_close(
                &mut stream,
                200,
                &[("content-type", "application/json")],
                body.to_string().as_bytes(),
            );
        });
        let tmp = TempDir::new().unwrap();
        let log_path = tmp.path().join("mcp-remote.log");
        let mut srv = new_server("remote", url);
        srv.set_log_path(log_path.clone());
        srv.initialize().unwrap();
        assert_eq!(srv.tools().len(), MAX_MCP_TOOLS);
        assert_eq!(srv.tools()[0].name, "t0");
        assert_eq!(srv.tools()[MAX_MCP_TOOLS - 1].name, format!("t{}", MAX_MCP_TOOLS - 1));
        stub.join().unwrap();
        drop(srv);
        let log = std::fs::read_to_string(&log_path).expect("cap note logged");
        assert!(log.contains("capped at 200"), "log should note the cap: {log}");
    }

    // ---------- T16: pins for the SPEC-9 R4 carried validator gaps ----------

    /// PIN (T16 #1, Accept headers): the exact Accept header the client
    /// sends — `application/json, text/event-stream` on every JSON-RPC POST
    /// and `text/event-stream` ALONE on the GET listen stream. Asserted with
    /// equality, not `contains`: servers key content negotiation off this
    /// exact value, so a "harmless" rewording must fail here.
    #[test]
    fn accept_headers_pinned_exactly_on_posts_and_listen_get() {
        let (listener, url) = bind_stub();
        let stub = thread::spawn(move || {
            let mut c1 = accept_conn(&listener);
            // All handshake POSTs ride all_headers(); one representative POST
            // would suffice per the spec, but all three take the same wire
            // path — assert each so a partial regression can't hide.
            let req = read_request(&mut c1);
            assert_eq!(req.json()["method"], "initialize");
            assert_eq!(
                req.header("accept"),
                Some("application/json, text/event-stream"),
                "POST accept header must be exactly the streamable-HTTP pair"
            );
            assert_eq!(req.header("content-type"), Some("application/json"));
            write_response(
                &mut c1,
                200,
                &[("content-type", "application/json")],
                br#"{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-06-18","capabilities":{},"serverInfo":{"name":"stub","version":"0.0.1"}}}"#,
            );
            let req = read_request(&mut c1);
            assert_eq!(req.json()["method"], "notifications/initialized");
            assert_eq!(
                req.header("accept"),
                Some("application/json, text/event-stream"),
                "notification POST accept header"
            );
            write_response(&mut c1, 202, &[], b"");
            let req = read_request(&mut c1);
            assert_eq!(req.json()["method"], "tools/list");
            assert_eq!(
                req.header("accept"),
                Some("application/json, text/event-stream"),
                "tools/list POST accept header"
            );
            write_response(
                &mut c1,
                200,
                &[("content-type", "application/json")],
                br#"{"jsonrpc":"2.0","id":2,"result":{"tools":[{"name":"echo","description":"d","inputSchema":{"type":"object"}}]}}"#,
            );
            // GET listen stream (fresh connection from the dedicated listen
            // client): SSE-only accept, pinned exactly.
            let mut c2 = accept_conn(&listener);
            let get = read_request(&mut c2);
            assert!(get.is_get(), "expected listen GET, got {}", get.request_line);
            assert_eq!(
                get.header("accept"),
                Some("text/event-stream"),
                "GET listen stream must ask for SSE only"
            );
            write_response_close(&mut c2, 405, &[("content-type", "text/plain")], b"no listen stream");
        });
        let mut srv = new_server("remote", url);
        srv.initialize().unwrap();
        assert_eq!(srv.tools()[0].name, "echo");
        stub.join().unwrap();
        drop(srv);
    }

    /// PIN (T16 #2, SSE-framed tools/list): a tools/list reply delivered as
    /// SSE (`Content-Type: text/event-stream`, `data: {...}` events) instead
    /// of a plain JSON body must parse through the shared incremental SSE
    /// response path and yield the payload's tool list. Limitation note:
    /// there is no list-specific SSE branch — one framing path serves every
    /// `send_request` — so this pins the narrowest *previously uncovered*
    /// path, the tools/list leg (the tools/call leg is covered by
    /// `sse_framed_call_response_multiline_data_stream_held_open`).
    #[test]
    fn tools_list_reply_framed_as_sse_parses_to_tool_list() {
        let (listener, url) = bind_stub();
        let stub = thread::spawn(move || {
            let mut c1 = accept_conn(&listener);
            let req = read_request(&mut c1);
            assert_eq!(req.json()["method"], "initialize");
            write_response(
                &mut c1,
                200,
                &[("content-type", "application/json")],
                br#"{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-06-18","capabilities":{},"serverInfo":{"name":"stub","version":"0.0.1"}}}"#,
            );
            let req = read_request(&mut c1);
            assert_eq!(req.json()["method"], "notifications/initialized");
            write_response(&mut c1, 202, &[], b"");
            let req = read_request(&mut c1);
            assert_eq!(req.json()["method"], "tools/list");
            assert_eq!(req.json()["id"], 2);
            // The whole reply arrives SSE-framed, not as a JSON body.
            write_sse_head(&mut c1);
            write_sse_event(
                &mut c1,
                r#"{"jsonrpc":"2.0","id":2,"result":{"tools":[{"name":"sse-echo","description":"echo over SSE","inputSchema":{"type":"object","properties":{"n":{"type":"number"}}}}]}}"#,
            );
            // initialize() returns the moment the matching-id event lands;
            // the listen GET then arrives on a fresh connection — refuse it
            // (405) so the manager exits and Drop stays trivial.
            let mut c2 = accept_conn(&listener);
            let get = read_request(&mut c2);
            assert!(get.is_get(), "expected listen GET, got {}", get.request_line);
            write_response_close(&mut c2, 405, &[("content-type", "text/plain")], b"no listen stream");
        });
        let mut srv = new_server("remote", url);
        srv.initialize().unwrap();
        let tools = srv.tools();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "sse-echo");
        assert_eq!(tools[0].description, "echo over SSE");
        assert_eq!(
            tools[0].input_schema,
            json!({"type":"object","properties":{"n":{"type":"number"}}})
        );
        stub.join().unwrap();
        drop(srv);
    }

    /// PIN (T16 #3, deadlines): the explicit caller-side deadline surface.
    /// The R4 blocker was a reqwest-level body-read timeout killing silent
    /// SSE streams; the fix is these explicit deadlines (connect 10s inside
    /// first-byte 30s inside per-call total 60s, plus the reply/listen
    /// bounds). A drive-by "bump the timeout" edit must fail this test —
    /// and deleting a constant must fail to compile — so the change is
    /// deliberate.
    #[test]
    fn deadline_constants_and_constructor_defaults_are_pinned() {
        assert_eq!(INIT_TIMEOUT, Duration::from_secs(10));
        assert_eq!(LIST_TIMEOUT, Duration::from_secs(10));
        assert_eq!(CALL_TIMEOUT, Duration::from_secs(60));
        assert_eq!(FIRST_BYTE_TIMEOUT, Duration::from_secs(30));
        assert_eq!(CONNECT_TIMEOUT, Duration::from_secs(10));
        assert_eq!(REPLY_TIMEOUT, Duration::from_secs(30));
        assert_eq!(LISTEN_POLL, Duration::from_millis(100));
        assert_eq!(LISTEN_JOIN_GRACE, Duration::from_secs(5));
        // The wire is governed by the FIELDS, not the constants directly:
        // pin the constructor defaults to literal seconds so a drive-by edit
        // to either the constants or the initializer breaks here.
        // (Construction performs no I/O — port 9 is never dialed.)
        let srv = new_server("remote", "http://127.0.0.1:9".to_string());
        assert_eq!(srv.first_byte_timeout, Duration::from_secs(30));
        assert_eq!(srv.call_timeout, Duration::from_secs(60));
    }

    /// PIN (T16 #3, behavioral half): `first_byte_timeout` (default 30s,
    /// shrunk here) really is the response-head deadline — a stub that
    /// accepts and reads the request but never answers fails the call at
    /// the shrunk deadline, not at some reqwest-level default. Bounded:
    /// fails in well under a second.
    #[test]
    fn first_byte_timeout_governs_head_deadline() {
        let (listener, url) = bind_stub();
        let (release_tx, release_rx) = mpsc::channel::<()>();
        let stub = thread::spawn(move || {
            let mut stream = accept_conn(&listener);
            // Read the request, then answer NOTHING until released.
            let req = read_request(&mut stream);
            assert_eq!(req.json()["method"], "initialize");
            release_rx.recv_timeout(Duration::from_secs(10)).unwrap();
        });
        let mut srv = new_server("remote", url);
        srv.first_byte_timeout = Duration::from_millis(300);
        let t0 = Instant::now();
        let err = srv.initialize().unwrap_err();
        let elapsed = t0.elapsed();
        let msg = format!("{err:#}");
        assert!(
            msg.contains("no response head within"),
            "unexpected failure mode: {msg}"
        );
        assert!(
            elapsed < Duration::from_secs(5),
            "first-byte deadline must fire near 300ms, took {elapsed:?}"
        );
        drop(srv);
        release_tx.send(()).unwrap();
        stub.join().unwrap();
    }

    // ---------- T6: stub-internal timeouts keep a broken stub from hanging ----------

    /// A stub whose client never connects must panic at the accept deadline
    /// instead of blocking the suite forever on `accept()`.
    #[test]
    fn stub_accept_deadline_fails_fast_when_client_never_connects() {
        let (listener, _url) = bind_stub();
        let t0 = Instant::now();
        let stub = thread::spawn(move || {
            let _ = accept_conn_within(&listener, Duration::from_millis(300));
        });
        stub.join().expect_err("accept must fail at the deadline");
        assert!(
            t0.elapsed() < Duration::from_secs(5),
            "accept outlived its deadline"
        );
    }

    /// A stub reading from a silent client must panic at its read timeout
    /// instead of hanging the suite on `read()`.
    #[test]
    fn stub_read_timeout_fails_fast_on_silent_client() {
        let (listener, url) = bind_stub();
        let stub = thread::spawn(move || {
            let mut stream = accept_conn(&listener);
            let _ = read_request_within(&mut stream, Duration::from_millis(300));
        });
        // Connect and say nothing.
        let addr = url.strip_prefix("http://").unwrap();
        let _client = TcpStream::connect(addr).unwrap();
        let t0 = Instant::now();
        stub.join().expect_err("stub must fail on a silent client");
        assert!(
            t0.elapsed() < Duration::from_secs(5),
            "stub read outlived its timeout"
        );
    }

    /// A deliberately unresponsive stub (accepts, never replies) makes the
    /// affected call FAIL within a small bounded time — the suite degrades
    /// to a failure, never a freeze.
    #[test]
    fn unresponsive_stub_fails_call_bounded() {
        let (listener, url) = bind_stub();
        let (release_tx, release_rx) = mpsc::channel::<()>();
        let stub = thread::spawn(move || {
            let mut stream = accept_conn(&listener);
            serve_handshake(&mut stream, None);
            let req = read_request(&mut stream);
            assert_eq!(req.json()["method"], "tools/call");
            // Unresponsive: the answer never comes.
            release_rx.recv_timeout(Duration::from_secs(30)).unwrap();
        });
        let mut srv = new_server("remote", url);
        srv.initialize().unwrap();
        srv.call_timeout = Duration::from_millis(600);
        let t0 = Instant::now();
        let res = srv.call("echo", json!({}));
        let elapsed = t0.elapsed();
        // The failure surfaces as an Err (no response head ever arrived) or
        // an error ToolResult — either way it must be an error, bounded.
        match res {
            Ok(r) => assert!(r.is_error, "unresponsive stub must fail the call: {}", r.content),
            Err(e) => {
                let msg = format!("{e:#}");
                assert!(
                    msg.contains("no response head") || msg.contains("timed out"),
                    "unexpected failure mode: {msg}"
                );
            }
        }
        assert!(
            elapsed < Duration::from_secs(5),
            "call must fail near the overridden 600ms deadline, took {elapsed:?}"
        );
        release_tx.send(()).unwrap();
        stub.join().unwrap();
        drop(srv);
    }
}
