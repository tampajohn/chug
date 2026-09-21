//! Langfuse observability (SPEC-8): optional, fail-open, zero-cost when off.
//!
//! One trace per run/chat session, one generation per LLM call, one span per
//! tool call, discrete events (goal verdicts, aborts, risk-gate verdicts,
//! steering notes) and outcome scores, delivered to a self-hosted Langfuse
//! v3 via batched ingestion:
//!
//! ```text
//! POST {LANGFUSE_HOST}/api/public/ingestion   (Basic auth pk:sk)
//! {"batch": [ {id, timestamp, type, body}, ... ], "metadata": {}}
//! ```
//!
//! OFF unless `LANGFUSE_HOST`, `LANGFUSE_PUBLIC_KEY` and
//! `LANGFUSE_SECRET_KEY` are all resolvable (process env first, then
//! `~/.langfuse-keys-chug` / `~/.langfuse-keys`; env wins per key). When off,
//! [`Sink::Noop`] does nothing: no thread, no I/O, no output — unless a
//! partial or malformed config was found, which earns exactly one stderr
//! note. Delivery is fire-and-forget: a bounded channel (drop-on-full) plus
//! a flusher thread batching every 2s or 50 events. Any HTTP error is
//! counted and logged once per run; observability NEVER changes run
//! behavior. Keys are never logged.

// R1 lands this module without its driver/api call sites (R2 wires those);
// until then most of the surface is reachable only from tests.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, bail};
use serde_json::{Value, json};

pub const HOST_VAR: &str = "LANGFUSE_HOST";
pub const PUBLIC_KEY_VAR: &str = "LANGFUSE_PUBLIC_KEY";
pub const SECRET_KEY_VAR: &str = "LANGFUSE_SECRET_KEY";

const CHANNEL_CAP: usize = 1000;
const BATCH_SIZE: usize = 50;
const FLUSH_INTERVAL: Duration = Duration::from_secs(2);
const HTTP_TIMEOUT: Duration = Duration::from_secs(5);
const DRAIN_CAP: Duration = Duration::from_secs(5);
const TRACE_NAME_MAX_CHARS: usize = 80;

/// Score names emitted at run end.
pub const SCORE_OUTCOME: &str = "outcome";
pub const SCORE_ITERATIONS: &str = "iterations";

/// Allowed values of the categorical `outcome` score.
pub mod outcome {
    pub const COMPLETED: &str = "completed";
    pub const ABORTED: &str = "aborted";
    pub const BUDGET: &str = "budget";
    pub const STUCK: &str = "stuck";
}

// ---------------------------------------------------------------------------
// Config resolution. The pure core takes an injected env getter + home dir,
// so tests need no process-env mutation (edition 2024 makes that unsafe).
// ---------------------------------------------------------------------------

/// Everything needed to reach one Langfuse project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LangfuseConfig {
    pub host: String,
    pub public_key: String,
    pub secret_key: String,
}

/// Result of one config resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    /// All three pieces found: observability on.
    On(LangfuseConfig),
    /// Nothing configured anywhere: silently off (the default; zero output).
    OffSilent,
    /// Something was configured but is unusable: off, with a one-line stderr
    /// note. Never contains key material.
    OffNoted(String),
}

/// Resolve config from an injected env getter and home directory. Per key,
/// env wins over the fallback files; `~/.langfuse-keys-chug` is preferred
/// over `~/.langfuse-keys` (first found wins). ANY missing piece → off.
pub fn resolve_with(
    env: &dyn Fn(&str) -> Option<String>,
    home: &Option<PathBuf>,
) -> Resolution {
    let mut malformed: Option<PathBuf> = None;
    let file = match home.as_deref() {
        Some(dir) => match load_key_file(dir) {
            FileOutcome::Parsed(keys) => keys,
            FileOutcome::Malformed(path) => {
                malformed = Some(path);
                FileKeys::default()
            }
            FileOutcome::Absent => FileKeys::default(),
        },
        None => FileKeys::default(),
    };

    let pick = |var: &str, from_file: Option<&str>| -> Option<String> {
        env(var)
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .or_else(|| from_file.map(str::to_string))
    };
    let host = pick(HOST_VAR, file.host.as_deref());
    let public_key = pick(PUBLIC_KEY_VAR, file.public_key.as_deref());
    let secret_key = pick(SECRET_KEY_VAR, file.secret_key.as_deref());

    match (host, public_key, secret_key) {
        (Some(host), Some(public_key), Some(secret_key)) => Resolution::On(LangfuseConfig {
            host,
            public_key,
            secret_key,
        }),
        (None, None, None) => match malformed {
            Some(path) => Resolution::OffNoted(format!(
                "ignoring malformed {} (no pk-lf-/sk-lf- tokens found)",
                path.display()
            )),
            None => Resolution::OffSilent,
        },
        (host, public_key, secret_key) => {
            let mut missing = Vec::new();
            if host.is_none() {
                missing.push(HOST_VAR);
            }
            if public_key.is_none() {
                missing.push(PUBLIC_KEY_VAR);
            }
            if secret_key.is_none() {
                missing.push(SECRET_KEY_VAR);
            }
            Resolution::OffNoted(format!(
                "incomplete Langfuse config: missing {}; need all of \
                 {HOST_VAR}, {PUBLIC_KEY_VAR}, {SECRET_KEY_VAR} \
                 (env or ~/.langfuse-keys-chug)",
                missing.join(", ")
            ))
        }
    }
}

/// Production resolution: real process env + the real home directory.
pub fn resolve() -> Resolution {
    resolve_with(&|name| std::env::var(name).ok(), &home_dir())
}

fn home_dir() -> Option<PathBuf> {
    #[cfg(unix)]
    let var = "HOME";
    #[cfg(windows)]
    let var = "USERPROFILE";
    std::env::var_os(var)
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
}

/// Keys parsed out of a fallback file. All fields optional; env can still
/// supply any piece the file lacks.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct FileKeys {
    host: Option<String>,
    public_key: Option<String>,
    secret_key: Option<String>,
}

impl FileKeys {
    fn is_empty(&self) -> bool {
        self.host.is_none() && self.public_key.is_none() && self.secret_key.is_none()
    }
}

enum FileOutcome {
    Absent,
    Parsed(FileKeys),
    /// File exists but contains nothing usable.
    Malformed(PathBuf),
}

/// First found of `~/.langfuse-keys-chug`, `~/.langfuse-keys`.
fn load_key_file(home: &Path) -> FileOutcome {
    for name in [".langfuse-keys-chug", ".langfuse-keys"] {
        let path = home.join(name);
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                let keys = parse_key_file(&text);
                return if keys.is_empty() {
                    FileOutcome::Malformed(path)
                } else {
                    FileOutcome::Parsed(keys)
                };
            }
            // Missing or unreadable: fall through to the next candidate.
            Err(_) => continue,
        }
    }
    FileOutcome::Absent
}

/// Loose line-oriented parse: `pk-lf-…` / `sk-lf-…` tokens anywhere (bare,
/// `KEY=token`, `export KEY="token"`), plus a `LANGFUSE_HOST=` line. First
/// occurrence of each piece wins; `#` lines are comments.
fn parse_key_file(text: &str) -> FileKeys {
    let mut keys = FileKeys::default();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line);
        if let Some(rest) = line.strip_prefix("LANGFUSE_HOST=") {
            let host = rest.trim().trim_matches('"').trim_matches('\'');
            if !host.is_empty() && keys.host.is_none() {
                keys.host = Some(host.to_string());
            }
        }
        for token in
            line.split(|c: char| c.is_whitespace() || c == '=' || c == '"' || c == '\'' || c == ',')
        {
            if token.starts_with("pk-lf-") && keys.public_key.is_none() {
                keys.public_key = Some(token.to_string());
            } else if token.starts_with("sk-lf-") && keys.secret_key.is_none() {
                keys.secret_key = Some(token.to_string());
            }
        }
    }
    keys
}

// ---------------------------------------------------------------------------
// Transport: one POST of a finished batch. A trait so tests can count sends
// without any network.
// ---------------------------------------------------------------------------

trait Transport: Send + Sync {
    fn send_batch(&self, events: &[Value]) -> anyhow::Result<()>;
}

/// `POST {host}/api/public/ingestion` with Basic auth `pk:sk`, 5s timeout.
struct HttpTransport {
    client: reqwest::blocking::Client,
    url: String,
    public_key: String,
    secret_key: String,
}

impl HttpTransport {
    fn new(cfg: &LangfuseConfig) -> anyhow::Result<Self> {
        let client = reqwest::blocking::Client::builder()
            .timeout(HTTP_TIMEOUT)
            .use_rustls_tls()
            .build()
            .context("building observability HTTP client")?;
        Ok(Self {
            client,
            url: format!("{}/api/public/ingestion", cfg.host.trim_end_matches('/')),
            public_key: cfg.public_key.clone(),
            secret_key: cfg.secret_key.clone(),
        })
    }
}

impl Transport for HttpTransport {
    fn send_batch(&self, events: &[Value]) -> anyhow::Result<()> {
        let body = json!({ "batch": events, "metadata": {} });
        let resp = self
            .client
            .post(&self.url)
            .basic_auth(&self.public_key, Some(&self.secret_key))
            .json(&body)
            .send()
            .context("POSTing Langfuse ingestion batch")?;
        let status = resp.status();
        if !status.is_success() {
            // Status only — credentials are never included in errors.
            bail!("Langfuse ingestion returned HTTP {status}");
        }
        Ok(())
    }
}

/// Used when the HTTP client itself cannot be built: every send fails
/// (counted, logged once) so the run is never disturbed.
struct BrokenTransport;

impl Transport for BrokenTransport {
    fn send_batch(&self, _events: &[Value]) -> anyhow::Result<()> {
        bail!("observability HTTP transport unavailable")
    }
}

// ---------------------------------------------------------------------------
// Flusher thread: batch every `interval` or `batch_size` events; on shutdown
// drain what remains, capped so a hung endpoint cannot stall process exit.
// ---------------------------------------------------------------------------

struct FlushOpts {
    batch_size: usize,
    interval: Duration,
    drain_cap: Duration,
}

impl Default for FlushOpts {
    fn default() -> Self {
        FlushOpts {
            batch_size: BATCH_SIZE,
            interval: FLUSH_INTERVAL,
            drain_cap: DRAIN_CAP,
        }
    }
}

#[derive(Default)]
struct SinkStats {
    dropped: AtomicU64,
    send_errors: AtomicU64,
    error_logged: AtomicBool,
}

fn run_flusher(
    rx: Receiver<Value>,
    transport: &dyn Transport,
    opts: &FlushOpts,
    closing: &AtomicBool,
    stats: &SinkStats,
) {
    let mut batch: Vec<Value> = Vec::with_capacity(opts.batch_size);
    let mut deadline = Instant::now() + opts.interval;
    loop {
        if closing.load(Ordering::SeqCst) {
            drain_and_exit(&rx, transport, opts, stats, &mut batch);
            return;
        }
        let timeout = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(timeout) {
            Ok(event) => {
                batch.push(event);
                if batch.len() >= opts.batch_size {
                    post_batch(transport, &mut batch, stats);
                    deadline = Instant::now() + opts.interval;
                }
            }
            Err(RecvTimeoutError::Timeout) => {
                if !batch.is_empty() {
                    post_batch(transport, &mut batch, stats);
                }
                deadline = Instant::now() + opts.interval;
            }
            Err(RecvTimeoutError::Disconnected) => {
                drain_and_exit(&rx, transport, opts, stats, &mut batch);
                return;
            }
        }
    }
}

/// Shutdown path: empty whatever is left, honoring `drain_cap` so a hung
/// endpoint cannot stall exit. Excess remainder counts as dropped.
fn drain_and_exit(
    rx: &Receiver<Value>,
    transport: &dyn Transport,
    opts: &FlushOpts,
    stats: &SinkStats,
    batch: &mut Vec<Value>,
) {
    let deadline = Instant::now() + opts.drain_cap;
    loop {
        match rx.try_recv() {
            Ok(event) => {
                batch.push(event);
                if batch.len() >= opts.batch_size && !flush_within(transport, stats, batch, deadline)
                {
                    return;
                }
            }
            Err(_) => break,
        }
    }
    if !batch.is_empty() {
        flush_within(transport, stats, batch, deadline);
    }
}

/// Send one batch unless the drain deadline has passed (then the events
/// count as dropped). Returns false when the deadline fired.
fn flush_within(
    transport: &dyn Transport,
    stats: &SinkStats,
    batch: &mut Vec<Value>,
    deadline: Instant,
) -> bool {
    if Instant::now() >= deadline {
        stats
            .dropped
            .fetch_add(batch.len() as u64, Ordering::SeqCst);
        batch.clear();
        return false;
    }
    post_batch(transport, batch, stats);
    true
}

/// Fail-open always: an HTTP error is counted and logged exactly once per
/// run; the run itself is never affected.
fn post_batch(transport: &dyn Transport, batch: &mut Vec<Value>, stats: &SinkStats) {
    if !batch.is_empty()
        && let Err(err) = transport.send_batch(batch)
    {
        stats.send_errors.fetch_add(1, Ordering::SeqCst);
        if !stats.error_logged.swap(true, Ordering::SeqCst) {
            eprintln!("chug: observability delivery failing (run continues): {err:#}");
        }
    }
    batch.clear();
}

// ---------------------------------------------------------------------------
// Sink: the driver's handle. `Noop` when off → zero cost, zero behavior
// change. `Live` owns the bounded channel + flusher thread.
// ---------------------------------------------------------------------------

pub enum Sink {
    Live(LiveSink),
    Noop,
}

pub struct LiveSink {
    tx: Mutex<Option<SyncSender<Value>>>,
    closing: Arc<AtomicBool>,
    stats: Arc<SinkStats>,
    flusher: Mutex<Option<JoinHandle<()>>>,
}

impl LiveSink {
    fn spawn(cfg: &LangfuseConfig) -> Self {
        let transport: Arc<dyn Transport> = match HttpTransport::new(cfg) {
            Ok(t) => Arc::new(t),
            Err(err) => {
                eprintln!(
                    "chug: observability transport unavailable ({err:#}); run continues"
                );
                Arc::new(BrokenTransport)
            }
        };
        Self::with_transport(transport, CHANNEL_CAP, FlushOpts::default())
    }

    fn with_transport(transport: Arc<dyn Transport>, cap: usize, opts: FlushOpts) -> Self {
        let (tx, rx) = std::sync::mpsc::sync_channel::<Value>(cap);
        let closing = Arc::new(AtomicBool::new(false));
        let stats = Arc::new(SinkStats::default());
        let spawned = {
            let closing = Arc::clone(&closing);
            let stats = Arc::clone(&stats);
            std::thread::Builder::new()
                .name("chug-observ-flusher".into())
                .spawn(move || {
                    run_flusher(rx, transport.as_ref(), &opts, &closing, &stats);
                })
        };
        match spawned {
            Ok(handle) => LiveSink {
                tx: Mutex::new(Some(tx)),
                closing,
                stats,
                flusher: Mutex::new(Some(handle)),
            },
            Err(err) => {
                eprintln!("chug: observability off: cannot spawn flusher thread: {err}");
                LiveSink {
                    tx: Mutex::new(None),
                    closing,
                    stats,
                    flusher: Mutex::new(None),
                }
            }
        }
    }

    /// Enqueue without EVER blocking: a full channel (or a dead flusher)
    /// bumps the dropped counter instead.
    fn emit(&self, event: Value) {
        let sent = match self.tx.lock() {
            Ok(guard) => match guard.as_ref() {
                Some(tx) => tx.try_send(event).is_ok(),
                None => false,
            },
            Err(_) => false,
        };
        if !sent {
            self.stats.dropped.fetch_add(1, Ordering::SeqCst);
        }
    }

    /// Signal the flusher to drain + exit, then join it. Idempotent; the
    /// drain is capped (see `drain_cap`), so shutdown cannot hang exit.
    fn shutdown(&self) {
        self.closing.store(true, Ordering::SeqCst);
        if let Ok(mut guard) = self.tx.lock() {
            guard.take(); // disconnect the channel
        }
        if let Ok(mut guard) = self.flusher.lock() {
            if let Some(handle) = guard.take() {
                let _ = handle.join();
            }
        }
    }
}

impl Drop for LiveSink {
    fn drop(&mut self) {
        self.shutdown();
    }
}

impl Sink {
    /// True when observability is on. Cheap gate for callers that want to
    /// skip building event fields entirely when off.
    pub fn enabled(&self) -> bool {
        matches!(self, Sink::Live(_))
    }

    /// Raw enqueue of one prebuilt ingestion event; never blocks.
    pub fn emit(&self, event: Value) {
        if let Sink::Live(live) = self {
            live.emit(event);
        }
    }

    /// Emit the run/chat trace; returns its `chug-{shortid}` id, or `None`
    /// when off (callers then skip all per-event emission).
    pub fn trace_started(
        &self,
        goal: &str,
        model: &str,
        cwd: &str,
        mode: &str,
        spec_file: Option<&str>,
    ) -> Option<String> {
        let Sink::Live(live) = self else {
            return None;
        };
        let trace_id = new_trace_id();
        let ts = now_rfc3339();
        live.emit(envelope(
            &new_id(),
            &ts,
            "trace-create",
            trace_body(&trace_id, goal, model, cwd, mode, spec_file),
        ));
        Some(trace_id)
    }

    /// One generation per LLM response.
    #[allow(clippy::too_many_arguments)]
    pub fn generation(
        &self,
        trace_id: &str,
        model: &str,
        max_tokens: u32,
        usage: &Usage,
        start: SystemTime,
        end: SystemTime,
        iteration: u32,
        stop_reason: Option<&str>,
    ) {
        let Sink::Live(live) = self else {
            return;
        };
        let id = new_id();
        live.emit(envelope(
            &id,
            &now_rfc3339(),
            "generation-create",
            generation_body(
                &id,
                trace_id,
                model,
                max_tokens,
                usage,
                &rfc3339_millis(start),
                &rfc3339_millis(end),
                iteration,
                stop_reason,
            ),
        ));
    }

    /// One span per tool call.
    pub fn span(
        &self,
        trace_id: &str,
        name: &str,
        start: SystemTime,
        end: SystemTime,
        ok: bool,
        is_error: bool,
    ) {
        let Sink::Live(live) = self else {
            return;
        };
        let id = new_id();
        live.emit(envelope(
            &id,
            &now_rfc3339(),
            "span-create",
            span_body(
                &id,
                trace_id,
                name,
                &rfc3339_millis(start),
                &rfc3339_millis(end),
                ok,
                is_error,
            ),
        ));
    }

    /// One discrete event (goal accepted/rejected, abort, risk-gate verdict,
    /// operator steering note).
    pub fn event(&self, trace_id: &str, name: &str, metadata: Value) {
        let Sink::Live(live) = self else {
            return;
        };
        let id = new_id();
        live.emit(envelope(
            &id,
            &now_rfc3339(),
            "event-create",
            event_body(&id, trace_id, name, metadata),
        ));
    }

    /// The categorical `outcome` score (`completed|aborted|budget|stuck`).
    pub fn score_outcome(&self, trace_id: &str, outcome: &str) {
        let Sink::Live(live) = self else {
            return;
        };
        let id = new_id();
        live.emit(envelope(
            &id,
            &now_rfc3339(),
            "score-create",
            categorical_score_body(&id, trace_id, SCORE_OUTCOME, outcome),
        ));
    }

    /// The numeric `iterations` score.
    pub fn score_iterations(&self, trace_id: &str, iterations: u64) {
        let Sink::Live(live) = self else {
            return;
        };
        let id = new_id();
        live.emit(envelope(
            &id,
            &now_rfc3339(),
            "score-create",
            numeric_score_body(&id, trace_id, SCORE_ITERATIONS, iterations),
        ));
    }

    /// Events dropped because the channel was full or the flusher is gone.
    pub fn dropped_count(&self) -> u64 {
        match self {
            Sink::Live(live) => live.stats.dropped.load(Ordering::SeqCst),
            Sink::Noop => 0,
        }
    }

    /// Batches whose delivery failed (each failure logged only once per run).
    pub fn send_error_count(&self) -> u64 {
        match self {
            Sink::Live(live) => live.stats.send_errors.load(Ordering::SeqCst),
            Sink::Noop => 0,
        }
    }

    /// Drain the queue and stop the flusher (best effort, ~5s cap). Safe to
    /// call more than once; a no-op when off.
    pub fn shutdown(&self) {
        if let Sink::Live(live) = self {
            live.shutdown();
        }
    }
}

// ---------------------------------------------------------------------------
// Global accessor (mirrors auth.rs's process-wide OnceLock pattern).
// ---------------------------------------------------------------------------

static GLOBAL: OnceLock<Sink> = OnceLock::new();

/// The process-wide sink, resolved once. Off (the default) → `Sink::Noop`:
/// no thread, no I/O, no output. A partial/malformed config → one stderr
/// note, then Noop.
pub fn global() -> &'static Sink {
    GLOBAL.get_or_init(|| match resolve() {
        Resolution::On(cfg) => Sink::Live(LiveSink::spawn(&cfg)),
        Resolution::OffSilent => Sink::Noop,
        Resolution::OffNoted(reason) => {
            eprintln!("chug: observability off: {reason}");
            Sink::Noop
        }
    })
}

/// Flush + stop the global sink's queue before process exit (normal end,
/// abort, panic-guard path). Never initializes the sink just to stop it.
pub fn shutdown_global() {
    if let Some(sink) = GLOBAL.get() {
        sink.shutdown();
    }
}

// ---------------------------------------------------------------------------
// Event builders: pure functions producing the exact ingestion JSON shapes.
// Every envelope item is `{id, timestamp, type, body}`.
// ---------------------------------------------------------------------------

pub fn envelope(id: &str, timestamp: &str, kind: &str, body: Value) -> Value {
    json!({
        "id": id,
        "timestamp": timestamp,
        "type": kind,
        "body": body,
    })
}

/// One trace per run/chat session: id `chug-{shortid}`, name = goal
/// (truncated to 80 chars), tags `["chug", model]`, sessionId = trace id.
pub fn trace_body(
    trace_id: &str,
    goal: &str,
    model: &str,
    cwd: &str,
    mode: &str,
    spec_file: Option<&str>,
) -> Value {
    json!({
        "id": trace_id,
        "name": truncate_chars(goal, TRACE_NAME_MAX_CHARS),
        "metadata": {
            "model": model,
            "cwd": cwd,
            "mode": mode,
            "spec_file": spec_file,
        },
        "tags": ["chug", model],
        "sessionId": trace_id,
    })
}

/// Token usage from one Messages API response `usage` object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Usage {
    pub input: u64,
    pub output: u64,
    pub total: u64,
    /// Mapped through from the API response when present.
    pub cache_read_input_tokens: Option<u64>,
}

/// One generation per LLM call: trace-linked, model + parameters, usage
/// (with `cache_read_input_tokens` when present), start/end latency,
/// iteration + stop_reason metadata.
#[allow(clippy::too_many_arguments)]
pub fn generation_body(
    id: &str,
    trace_id: &str,
    model: &str,
    max_tokens: u32,
    usage: &Usage,
    start_time: &str,
    end_time: &str,
    iteration: u32,
    stop_reason: Option<&str>,
) -> Value {
    let mut usage_json = json!({
        "input": usage.input,
        "output": usage.output,
        "total": usage.total,
    });
    if let Some(cache_read) = usage.cache_read_input_tokens {
        usage_json["cache_read_input_tokens"] = json!(cache_read);
    }
    json!({
        "id": id,
        "traceId": trace_id,
        "model": model,
        "modelParameters": { "maxTokens": max_tokens },
        "usage": usage_json,
        "startTime": start_time,
        "endTime": end_time,
        "metadata": {
            "iteration": iteration,
            "stop_reason": stop_reason,
        },
    })
}

/// One span per tool call.
pub fn span_body(
    id: &str,
    trace_id: &str,
    name: &str,
    start_time: &str,
    end_time: &str,
    ok: bool,
    is_error: bool,
) -> Value {
    json!({
        "id": id,
        "traceId": trace_id,
        "name": name,
        "startTime": start_time,
        "endTime": end_time,
        "metadata": { "ok": ok, "is_error": is_error },
    })
}

/// One discrete event (goal accepted/rejected, abort, risk-gate verdict,
/// operator steering note).
pub fn event_body(id: &str, trace_id: &str, name: &str, metadata: Value) -> Value {
    json!({
        "id": id,
        "traceId": trace_id,
        "name": name,
        "metadata": metadata,
    })
}

/// CATEGORICAL score (e.g. `outcome` = completed|aborted|budget|stuck).
pub fn categorical_score_body(id: &str, trace_id: &str, name: &str, string_value: &str) -> Value {
    json!({
        "id": id,
        "traceId": trace_id,
        "name": name,
        "stringValue": string_value,
        "dataType": "CATEGORICAL",
    })
}

/// NUMERIC score (e.g. `iterations`).
pub fn numeric_score_body(id: &str, trace_id: &str, name: &str, value: u64) -> Value {
    json!({
        "id": id,
        "traceId": trace_id,
        "name": name,
        "value": value,
        "dataType": "NUMERIC",
    })
}

// ---------------------------------------------------------------------------
// Ids + timestamps (no new dependencies).
// ---------------------------------------------------------------------------

/// 16 lowercase hex chars from time ⊕ pid ⊕ a process-wide counter
/// (splitmix64-mixed). Not cryptographic; unique enough within a process.
fn new_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let mut seed = nanos
        ^ (u64::from(std::process::id()) << 32)
        ^ COUNTER
            .fetch_add(1, Ordering::SeqCst)
            .wrapping_mul(0x9E37_79B9_7F4A_7C15);
    format!("{:016x}", splitmix64(&mut seed))
}

/// `chug-{shortid}` (shortid = 8 hex chars).
fn new_trace_id() -> String {
    let id = new_id();
    format!("chug-{}", &id[..8])
}

fn splitmix64(x: &mut u64) -> u64 {
    *x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn now_rfc3339() -> String {
    rfc3339_millis(SystemTime::now())
}

/// RFC 3339 / ISO 8601 UTC with millisecond precision, e.g.
/// `2024-01-02T03:04:05.006Z`. Pre-epoch times clamp to the epoch.
fn rfc3339_millis(t: SystemTime) -> String {
    let dur = t.duration_since(UNIX_EPOCH).unwrap_or_default();
    let secs = dur.as_secs();
    let millis = dur.subsec_millis();
    let days = (secs / 86_400) as i64;
    let secs_of_day = secs % 86_400;
    let (hour, min, sec) = (secs_of_day / 3_600, (secs_of_day % 3_600) / 60, secs_of_day % 60);
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{min:02}:{sec:02}.{millis:03}Z")
}

/// Howard Hinnant's civil-from-days algorithm.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64; // day of era: [0, 146096]
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        s.chars().take(max).collect()
    }
}
