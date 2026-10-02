//! T190: completion notifications — opt-in, fail-open, fire-and-forget.
//!
//! chug is async-first, but the operator only learns outcomes by polling.
//! When `.chug/notify.json` enables it, the autonomous run's sink chain
//! (driver::run_loop) gains a notify leg that fires a macOS banner on the
//! run-level outcomes an operator must not miss: goal accepted, abort
//! (budget/cause named), validation verdicts (the spec `check:` PASS/FAIL).
//! A `release` subscription is accepted by the config (and filtered like
//! the others) but has no in-repo emit point — the release tag push happens
//! in the orchestrator's own release flow, not in a child chug process.
//!
//! OFF unless `.chug/notify.json` exists and says `enabled: true` — absent,
//! disabled, or malformed config is ZERO behavior change (malformed earns
//! exactly one stderr warn + one events.jsonl note, the T90 fail-open
//! pattern). Delivery mirrors the SPEC-8 observability shape: a bounded
//! channel (drop-on-full, never blocks the run) plus one flusher thread
//! that executes each notice. Any delivery failure is counted and recorded
//! exactly once per run as a `notify_error` line in events.jsonl; the run
//! itself is NEVER affected.
//!
//! Sinks: `osascript` shells `display notification "<msg>" with title
//! "chug"`; `layad` POSTs `{LAYA_URL}/hook/notification` and reuses layad's
//! push-vs-silent judgment (same default host as the risk gate's judge).
//! loopd forwards nothing — notifications fire from the child chug
//! processes themselves, per-run config.

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, SyncSender, sync_channel};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use anyhow::{Context, bail};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::eventlog;
use crate::events::{Event, EventSink};
use crate::observ::now_rfc3339;
use crate::riskgate::DEFAULT_LAYA_URL;

/// Bounded queue: a notification storm can never back-pressure the run
/// (drop-on-full, the SPEC-8 pattern).
const CHANNEL_CAP: usize = 16;
/// Per-notification delivery budget (process wait / HTTP timeout).
const DELIVER_TIMEOUT: Duration = Duration::from_secs(5);
/// Drop-side drain cap: the terminal notification still lands on a normal
/// exit, but a hung sink cannot stall process exit.
const DRAIN_CAP: Duration = Duration::from_secs(3);
/// One banner line; summaries are flattened and truncated to this.
const MESSAGE_MAX_CHARS: usize = 280;

/// Subscribable notification kinds — the `events` list in the config.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Goal,
    Abort,
    Validation,
    Release,
}

impl Kind {
    fn from_config_name(name: &str) -> Option<Kind> {
        match name {
            "goal" => Some(Kind::Goal),
            "abort" => Some(Kind::Abort),
            "validation" => Some(Kind::Validation),
            "release" => Some(Kind::Release),
            _ => None,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Kind::Goal => "goal",
            Kind::Abort => "abort",
            Kind::Validation => "validation",
            Kind::Release => "release",
        }
    }
}

/// Where a notice goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SinkKind {
    /// `osascript -e 'display notification …'` — the local macOS banner.
    Osascript,
    /// POST `{url}/hook/notification` — layad judges push-vs-silent.
    Layad,
}

/// Parsed `.chug/notify.json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotifyConfig {
    pub enabled: bool,
    pub sink: SinkKind,
    /// Kinds this run notifies on; a kind absent from the list never fires.
    pub events: Vec<Kind>,
    /// Goal notifications are suppressed for runs shorter than this (a
    /// just-started run finishing is not news). Aborts and validation
    /// verdicts are never suppressed — an early death is exactly the news.
    pub min_duration_secs: u64,
}

/// The `events` default (field absent): every kind.
fn notify_event_names() -> Vec<String> {
    ["goal", "abort", "validation", "release"]
        .into_iter()
        .map(str::to_string)
        .collect()
}

/// Config resolution. `warning` is the ONE warn a malformed/partial config
/// earns (T90 fail-open pattern); `config: None` means off — zero behavior.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Loaded {
    pub config: Option<NotifyConfig>,
    pub warning: Option<String>,
}

/// Loose config shape: every field defaults, unknown fields are ignored
/// (forward-compatible), so only genuinely broken files are malformed.
/// `events: None` means the field is absent → all kinds; an explicit empty
/// list is the operator asking for silence (valid, honored).
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawConfig {
    enabled: bool,
    sink: String,
    events: Option<Vec<String>>,
    min_duration_secs: u64,
}

/// Resolve `.chug/notify.json`. Absent → off, silent (the default; zero
/// behavior change). Malformed → off + one warn. Unknown event names are
/// skipped while valid siblings load (the T90 sibling rule); a config left
/// with no known events can never fire, so it is off + one warn too.
pub fn load(cwd: &Path) -> Loaded {
    let path = cwd.join(".chug").join("notify.json");
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        // Absent is the default state: silently off. Unreadable-but-present
        // is a config problem: off + one warn.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Loaded {
                config: None,
                warning: None,
            };
        }
        Err(e) => {
            return Loaded {
                config: None,
                warning: Some(format!("notify config unreadable ({}): {e}", path.display())),
            };
        }
    };
    let raw: RawConfig = match serde_json::from_str(&text) {
        Ok(raw) => raw,
        Err(e) => {
            return Loaded {
                config: None,
                warning: Some(format!("ignoring malformed {}: {e}", path.display())),
            };
        }
    };
    if !raw.enabled {
        // Explicitly disabled: a valid config the operator turned off —
        // silent, like absent.
        return Loaded {
            config: None,
            warning: None,
        };
    }
    let sink = match raw.sink.as_str() {
        "" | "osascript" => SinkKind::Osascript,
        "layad" => SinkKind::Layad,
        other => {
            return Loaded {
                config: None,
                warning: Some(format!(
                    "ignoring {}: unknown notify sink \"{other}\" (osascript|layad)",
                    path.display()
                )),
            };
        }
    };
    let mut warning: Option<String> = None;
    let mut events = Vec::new();
    let mut unknown: Vec<String> = Vec::new();
    // Absent `events` subscribes to everything — the operator opted in, the
    // list only narrows. Unknown names are skipped while valid siblings
    // load (the T90 sibling rule), with the one warn.
    for name in raw.events.unwrap_or_else(notify_event_names) {
        match Kind::from_config_name(&name) {
            Some(kind) => {
                if !events.contains(&kind) {
                    events.push(kind);
                }
            }
            None => unknown.push(name),
        }
    }
    if !unknown.is_empty() {
        warning = Some(format!(
            "notify config: skipped unknown event names: {} (known: goal|abort|validation|release)",
            unknown.join(", ")
        ));
    }
    Loaded {
        config: Some(NotifyConfig {
            enabled: true,
            sink,
            events,
            min_duration_secs: raw.min_duration_secs,
        }),
        warning,
    }
}

// ---------------------------------------------------------------------------
// Notices: the unit of delivery. One human line per outcome.
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct Notice {
    kind: Kind,
    /// The single banner line (flattened, truncated) — the osascript message
    /// text and the layad body's `message` field.
    message: String,
}

/// Collapse a multi-line summary into one banner line, truncated.
fn flatten(text: &str) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    flat.chars().take(MESSAGE_MAX_CHARS).collect()
}

fn goal_message(summary: &str) -> String {
    format!("goal complete: {}", flatten(summary))
}

fn abort_message(reason: &str, model: &str, budget: Option<&crate::events::BudgetExceeded>) -> String {
    // The cause is the news; the model + exhausted budget are what the
    // operator needs to resume (the T12 abort block names the same trio).
    let mut message = format!("run aborted: {}", flatten(reason));
    message.push_str(&format!(" ({model}"));
    if let Some(budget) = budget {
        message.push_str(&format!("; budget: {})", budget.label()));
    } else {
        message.push(')');
    }
    message
}

fn validation_message(item: &str, passed: bool) -> String {
    let verdict = if passed { "PASS" } else { "FAIL" };
    format!("validation {verdict}: {}", flatten(item))
}

/// The osascript script for one notice: the banner text is the message, the
/// banner's title is `chug` (spec shape). Quotes and backslashes escaped so
/// a summary cannot break out of the AppleScript string literal.
fn osascript_script(message: &str) -> String {
    let escaped = message.replace('\\', "\\\\").replace('"', "\\\"");
    format!("display notification \"{escaped}\" with title \"chug\"")
}

/// The layad `/hook/notification` body. layad judges push-vs-silent; chug
/// only describes the outcome.
fn layad_body(notice: &Notice) -> Value {
    json!({
        "source": "chug",
        "kind": notice.kind.name(),
        "title": "chug",
        "message": notice.message,
    })
}

// ---------------------------------------------------------------------------
// Transport: one delivery. A trait so tests count notices without any real
// banner or network.
// ---------------------------------------------------------------------------

trait Transport: Send + Sync {
    fn deliver(&self, notice: &Notice) -> anyhow::Result<()>;
}

/// `osascript -e …` — waits for the (fast) banner call; a non-zero exit is
/// a delivery failure.
struct OsascriptTransport;

impl Transport for OsascriptTransport {
    fn deliver(&self, notice: &Notice) -> anyhow::Result<()> {
        let script = osascript_script(&notice.message);
        let status = std::process::Command::new("osascript")
            .arg("-e")
            .arg(&script)
            .status()
            .context("spawning osascript")?;
        if !status.success() {
            bail!("osascript exited {status}");
        }
        Ok(())
    }
}

/// `POST {url}/hook/notification` — layad's push-vs-silent judgment decides
/// how the banner surfaces; any non-success status is a delivery failure.
struct LayadTransport {
    client: reqwest::blocking::Client,
    url: String,
}

impl LayadTransport {
    fn new(url: String) -> anyhow::Result<Self> {
        let client = reqwest::blocking::Client::builder()
            .timeout(DELIVER_TIMEOUT)
            .use_rustls_tls()
            .build()
            .context("building layad HTTP client")?;
        Ok(Self { client, url })
    }
}

impl Transport for LayadTransport {
    fn deliver(&self, notice: &Notice) -> anyhow::Result<()> {
        let url = format!("{}/hook/notification", self.url.trim_end_matches('/'));
        let resp = self
            .client
            .post(&url)
            .json(&layad_body(notice))
            .send()
            .context("POSTing layad /hook/notification")?;
        let status = resp.status();
        if !status.is_success() {
            bail!("layad returned HTTP {status}");
        }
        Ok(())
    }
}

fn transport_for(sink: SinkKind) -> anyhow::Result<Arc<dyn Transport>> {
    match sink {
        SinkKind::Osascript => Ok(Arc::new(OsascriptTransport)),
        SinkKind::Layad => {
            let url = std::env::var("LAYA_URL")
                .ok()
                .filter(|u| !u.trim().is_empty())
                .unwrap_or_else(|| DEFAULT_LAYA_URL.to_string());
            Ok(Arc::new(LayadTransport::new(url)?))
        }
    }
}

// ---------------------------------------------------------------------------
// Flusher thread: deliver each notice as it arrives; on channel disconnect
// drain what remains (a run's terminal notification must not be lost), then
// signal done so the drop-side wait is bounded.
// ---------------------------------------------------------------------------

#[derive(Default)]
struct Stats {
    dropped: AtomicU64,
    send_errors: AtomicU64,
    /// One events.jsonl note per run, however many deliveries fail.
    error_logged: AtomicBool,
}

/// The ONE events.jsonl note per run for notify failures (plus the stderr
/// warn, house style). Best-effort: the note itself never changes behavior.
fn note_failure_once(cwd: &Path, stats: &Stats, detail: &str) {
    stats.send_errors.fetch_add(1, Ordering::SeqCst);
    if !stats.error_logged.swap(true, Ordering::SeqCst) {
        eprintln!("chug: notification delivery failing (run continues): {detail}");
        eventlog::append_line(
            cwd,
            json!({
                "type": "notify_error",
                "ts": now_rfc3339(),
                "detail": detail,
            }),
        );
    }
}

fn run_flusher(
    rx: Receiver<Notice>,
    transport: &dyn Transport,
    cwd: &Path,
    stats: &Stats,
    done: Sender<()>,
) {
    while let Ok(notice) = rx.recv() {
        deliver(transport, cwd, stats, &notice);
    }
    // Disconnected: drain the terminal notices, capped so a hung sink cannot
    // stall exit — the deadline is enforced by the drop-side wait; here we
    // just finish what is queued.
    while let Ok(notice) = rx.try_recv() {
        deliver(transport, cwd, stats, &notice);
    }
    let _ = done.send(());
}

fn deliver(transport: &dyn Transport, cwd: &Path, stats: &Stats, notice: &Notice) {
    if let Err(err) = transport.deliver(notice) {
        note_failure_once(cwd, stats, &format!("{} notification: {err:#}", notice.kind.name()));
    }
}

// ---------------------------------------------------------------------------
// Machine: the live machinery (bounded channel + flusher). None of this
// exists when notifications are off.
// ---------------------------------------------------------------------------

struct Machine {
    wants: Vec<Kind>,
    min_duration: Duration,
    run_started: Instant,
    /// `None` once the drop path disconnects the channel (the flusher then
    /// drains and exits). A taken/dead sender bumps the dropped counter.
    tx: Mutex<Option<SyncSender<Notice>>>,
    stats: Arc<Stats>,
    done_rx: Receiver<()>,
    flusher: Mutex<Option<JoinHandle<()>>>,
}

impl Machine {
    fn observe(&self, e: &Event) {
        let (kind, message) = match e {
            Event::GoalAccepted { summary } => {
                // Short-run suppression: a run that finished almost as soon
                // as it started is not news the operator asked to be nudged
                // for. Aborts and verdicts are never suppressed.
                if self.run_started.elapsed() < self.min_duration {
                    return;
                }
                (Kind::Goal, goal_message(summary))
            }
            Event::Aborted {
                reason,
                model,
                budget,
            } => (Kind::Abort, abort_message(reason, model, budget.as_ref())),
            Event::ValidationVerdict { item, passed } => {
                (Kind::Validation, validation_message(item, *passed))
            }
            _ => return,
        };
        if !self.wants.contains(&kind) {
            return;
        }
        self.fire(Notice { kind, message });
    }

    /// Enqueue without EVER blocking: a full queue (or a dead flusher) bumps
    /// the dropped counter instead — the run never waits on a notification.
    fn fire(&self, notice: Notice) {
        let sent = match self.tx.lock() {
            Ok(guard) => match guard.as_ref() {
                Some(tx) => tx.try_send(notice).is_ok(),
                None => false,
            },
            Err(_) => false,
        };
        if !sent {
            self.stats.dropped.fetch_add(1, Ordering::SeqCst);
        }
    }
}

impl Drop for Machine {
    fn drop(&mut self) {
        // Disconnect the channel; the flusher drains what is queued (the
        // run's terminal notification), signals done, and exits. The wait is
        // capped and a hung sink is DETACHED, never joined — process exit
        // cannot be stalled by a notification.
        if let Ok(mut guard) = self.tx.lock() {
            guard.take(); // disconnect
        }
        let deadline = Instant::now() + DRAIN_CAP;
        match self
            .done_rx
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        {
            // Drained: the flusher is at its last statement — safe to join.
            Ok(()) => {
                if let Ok(mut guard) = self.flusher.lock()
                    && let Some(handle) = guard.take()
                {
                    let _ = handle.join();
                }
            }
            // Timed out (hung sink) or the thread is already gone: detach.
            Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Sink: the driver's notify leg. Pass-through when off.
// ---------------------------------------------------------------------------

/// Wraps the run's sink; on run-level events it fires a notification
/// (fire-and-forget) and then forwards the event untouched. When off this is
/// a pure pass-through — zero behavior change.
pub struct NotifySink<'a> {
    inner: &'a mut dyn EventSink,
    machine: Option<Machine>,
}

impl<'a> NotifySink<'a> {
    /// Load `.chug/notify.json` and arm the machinery when enabled. A
    /// malformed config is off + ONE warn (stderr + events.jsonl note);
    /// absent/disabled is silent.
    pub fn new(cwd: &Path, inner: &'a mut dyn EventSink) -> Self {
        let loaded = load(cwd);
        if let Some(warning) = &loaded.warning {
            eprintln!("chug: warning: {warning}");
            eventlog::append_line(
                cwd,
                json!({
                    "type": "notify_error",
                    "ts": now_rfc3339(),
                    "detail": warning,
                }),
            );
        }
        let machine = loaded.config.map(|config| {
            // A transport that cannot be built (e.g. a bad LAYA_URL) is a
            // delivery failure like any other: counted, noted once, never a
            // run killer — so the machinery still arms.
            let transport = transport_for(config.sink).unwrap_or_else(|err| {
                eprintln!("chug: notification transport unavailable ({err:#}); run continues");
                Arc::new(BrokenTransport)
            });
            Self::with_parts(cwd, config, Instant::now(), transport)
        });
        Self { inner, machine }
    }

    /// The seam tests use: an injected transport + run-start instant, so no
    /// test ever fires a real banner or touches the network.
    fn with_parts(
        cwd: &Path,
        config: NotifyConfig,
        run_started: Instant,
        transport: Arc<dyn Transport>,
    ) -> Machine {
        let (tx, rx) = sync_channel::<Notice>(CHANNEL_CAP);
        let (done_tx, done_rx) = std::sync::mpsc::channel::<()>();
        let stats = Arc::new(Stats::default());
        let thread_cwd = cwd.to_path_buf();
        let spawned = {
            let stats = Arc::clone(&stats);
            std::thread::Builder::new()
                .name("chug-notify-flusher".into())
                .spawn(move || {
                    run_flusher(rx, transport.as_ref(), &thread_cwd, &stats, done_tx);
                })
        };
        match spawned {
            Ok(handle) => Machine {
                wants: config.events,
                min_duration: Duration::from_secs(config.min_duration_secs),
                run_started,
                tx: Mutex::new(Some(tx)),
                stats,
                done_rx,
                flusher: Mutex::new(Some(handle)),
            },
            Err(err) => {
                eprintln!("chug: notifications off: cannot spawn flusher thread: {err}");
                Machine {
                    wants: Vec::new(),
                    min_duration: Duration::ZERO,
                    run_started,
                    tx: Mutex::new(None),
                    stats,
                    done_rx: std::sync::mpsc::channel().1,
                    flusher: Mutex::new(None),
                }
            }
        }
    }
}

/// Used when the transport itself cannot be built: every delivery fails
/// (counted, noted once) so the run is never disturbed.
struct BrokenTransport;

impl Transport for BrokenTransport {
    fn deliver(&self, _notice: &Notice) -> anyhow::Result<()> {
        bail!("notification transport unavailable")
    }
}

impl EventSink for NotifySink<'_> {
    fn emit(&mut self, e: Event) {
        // Forward first: the console/TUI render and the events-log record
        // (upstream) are the run's behavior; the notification is the extra.
        self.inner.emit(e.clone());
        if let Some(machine) = &self.machine {
            machine.observe(&e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::BudgetExceeded;
    use std::sync::mpsc;

    fn tmp() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    fn write_config(cwd: &Path, text: &str) {
        std::fs::create_dir_all(cwd.join(".chug")).unwrap();
        std::fs::write(cwd.join(".chug").join("notify.json"), text).unwrap();
    }

    fn config(events: &[Kind], min_duration_secs: u64) -> NotifyConfig {
        NotifyConfig {
            enabled: true,
            sink: SinkKind::Osascript,
            events: events.to_vec(),
            min_duration_secs,
        }
    }

    // A transport that records delivered messages — no real banner, no
    // network, deterministic assertions after the Machine drains on Drop.
    #[derive(Default)]
    struct RecordingTransport(Mutex<Vec<String>>);

    impl Transport for RecordingTransport {
        fn deliver(&self, notice: &Notice) -> anyhow::Result<()> {
            self.0.lock().unwrap().push(notice.message.clone());
            Ok(())
        }
    }

    struct FailingTransport;

    impl Transport for FailingTransport {
        fn deliver(&self, _notice: &Notice) -> anyhow::Result<()> {
            bail!("sink exploded")
        }
    }

    fn machine_for(
        cwd: &Path,
        cfg: NotifyConfig,
        transport: Arc<dyn Transport>,
    ) -> Machine {
        NotifySink::with_parts(cwd, cfg, Instant::now(), transport)
    }

    // --- config parse legs -------------------------------------------------

    /// Absent file = off, silent: the default state earns no output at all.
    #[test]
    fn absent_config_is_off_and_silent() {
        let tmp = tmp();
        let loaded = load(tmp.path());
        assert!(loaded.config.is_none());
        assert!(loaded.warning.is_none(), "absent is the default: no warn");
    }

    /// Disabled = off, silent (a valid config the operator turned off).
    #[test]
    fn disabled_config_is_off_and_silent() {
        let tmp = tmp();
        write_config(tmp.path(), r#"{"enabled": false, "sink": "osascript"}"#);
        let loaded = load(tmp.path());
        assert!(loaded.config.is_none());
        assert!(loaded.warning.is_none());
    }

    /// Malformed JSON = off + ONE warn (the T90 fail-open pattern).
    #[test]
    fn malformed_config_is_off_with_one_warning() {
        let tmp = tmp();
        write_config(tmp.path(), "{not json");
        let loaded = load(tmp.path());
        assert!(loaded.config.is_none(), "malformed never enables");
        let warning = loaded.warning.expect("malformed earns one warn");
        assert!(warning.contains("malformed"), "{warning}");
    }

    /// Wrong types are malformed the same way (enabled: "yes" is not a bool).
    #[test]
    fn mistyped_config_is_off_with_a_warning() {
        let tmp = tmp();
        write_config(tmp.path(), r#"{"enabled": "yes"}"#);
        let loaded = load(tmp.path());
        assert!(loaded.config.is_none());
        assert!(loaded.warning.is_some());
    }

    /// Unknown sink = off + one warn naming the bad value.
    #[test]
    fn unknown_sink_is_off_with_a_warning() {
        let tmp = tmp();
        write_config(tmp.path(), r#"{"enabled": true, "sink": "slack"}"#);
        let loaded = load(tmp.path());
        assert!(loaded.config.is_none());
        assert!(loaded.warning.unwrap().contains("slack"));
    }

    /// Unknown event names are skipped while valid siblings load (the T90
    /// sibling rule): the config stays ON for the known kinds, one warn.
    #[test]
    fn unknown_event_names_are_skipped_with_a_warning() {
        let tmp = tmp();
        write_config(
            tmp.path(),
            r#"{"enabled": true, "events": ["goal", "bogus"]}"#,
        );
        let loaded = load(tmp.path());
        let config = loaded.config.expect("valid siblings still load");
        assert_eq!(config.events, vec![Kind::Goal]);
        assert!(loaded.warning.unwrap().contains("bogus"));
    }

    /// A full config parses field-for-field; an absent `events` subscribes
    /// to everything and an absent sink defaults to osascript.
    #[test]
    fn valid_config_parses_with_defaults() {
        let tmp = tmp();
        write_config(
            tmp.path(),
            r#"{"enabled": true, "sink": "layad", "min_duration_secs": 300}"#,
        );
        let loaded = load(tmp.path());
        let config = loaded.config.unwrap();
        assert_eq!(config.sink, SinkKind::Layad);
        assert_eq!(config.min_duration_secs, 300);
        assert_eq!(config.events, vec![
            Kind::Goal,
            Kind::Abort,
            Kind::Validation,
            Kind::Release,
        ]);
        assert!(loaded.warning.is_none());

        write_config(
            tmp.path(),
            r#"{"enabled": true, "events": ["abort", "goal"]}"#,
        );
        let config = load(tmp.path()).config.unwrap();
        assert_eq!(config.sink, SinkKind::Osascript, "sink defaults to osascript");
        assert_eq!(config.events, vec![Kind::Abort, Kind::Goal]);
    }

    // --- fire legs ----------------------------------------------------------

    /// Goal accepted → ONE notification carrying the summary line.
    #[test]
    fn goal_fires_one_notification_with_the_summary() {
        let tmp = tmp();
        let transport = Arc::new(RecordingTransport::default());
        let machine = machine_for(tmp.path(), config(&[Kind::Goal], 0), transport.clone());
        machine.observe(&Event::GoalAccepted {
            summary: "did it\nacross two lines".into(),
        });
        drop(machine); // drains + joins: delivery is finished now
        let messages = transport.0.lock().unwrap().clone();
        assert_eq!(messages.len(), 1, "one notice per event, one event here");
        assert_eq!(messages[0], "goal complete: did it across two lines");
    }

    /// Abort → the cause is named, with the exhausted budget on budget
    /// deaths and none on operator/stuck aborts.
    #[test]
    fn abort_names_the_cause_and_budget() {
        let tmp = tmp();
        let transport = Arc::new(RecordingTransport::default());
        let machine = machine_for(tmp.path(), config(&[Kind::Abort], 0), transport.clone());
        machine.observe(&Event::Aborted {
            reason: "iteration budget exceeded".into(),
            model: "muse-glimmer-30b".into(),
            budget: Some(BudgetExceeded::Iterations { max: 40 }),
        });
        machine.observe(&Event::Aborted {
            reason: "stuck: repeated error".into(),
            model: "claude-sonnet-4-6".into(),
            budget: None,
        });
        drop(machine);
        let messages = transport.0.lock().unwrap().clone();
        assert_eq!(messages.len(), 2);
        assert!(
            messages[0].contains("iteration budget exceeded")
                && messages[0].contains("budget: 40 iterations")
                && messages[0].contains("muse-glimmer-30b"),
            "budget death names cause, budget, model: {}",
            messages[0]
        );
        assert!(
            messages[1].contains("stuck: repeated error")
                && !messages[1].contains("budget:"),
            "non-budget abort names the cause without a budget segment: {}",
            messages[1]
        );
    }

    /// Validation verdicts fire with the item + PASS/FAIL verdict.
    #[test]
    fn validation_verdicts_fire_with_item_and_verdict() {
        let tmp = tmp();
        let transport = Arc::new(RecordingTransport::default());
        let machine =
            machine_for(tmp.path(), config(&[Kind::Validation], 0), transport.clone());
        machine.observe(&Event::ValidationVerdict {
            item: "cargo test".into(),
            passed: true,
        });
        machine.observe(&Event::ValidationVerdict {
            item: "cargo clippy --all-targets".into(),
            passed: false,
        });
        drop(machine);
        let messages = transport.0.lock().unwrap().clone();
        assert_eq!(messages, vec![
            "validation PASS: cargo test",
            "validation FAIL: cargo clippy --all-targets",
        ]);
    }

    /// Short-run suppression: a goal inside the min-duration window does
    /// NOT notify — but an abort in the same window still does (an early
    /// death is exactly the news).
    #[test]
    fn short_run_goal_is_suppressed_but_abort_is_not() {
        let tmp = tmp();
        let transport = Arc::new(RecordingTransport::default());
        let machine =
            machine_for(tmp.path(), config(&[Kind::Goal, Kind::Abort], 600), transport.clone());
        machine.observe(&Event::GoalAccepted {
            summary: "too fast to be news".into(),
        });
        machine.observe(&Event::Aborted {
            reason: "interrupted".into(),
            model: "m".into(),
            budget: None,
        });
        drop(machine);
        let messages = transport.0.lock().unwrap().clone();
        assert_eq!(messages.len(), 1, "goal suppressed, abort fires");
        assert!(messages[0].starts_with("run aborted:"));
    }

    /// An old-enough run's goal still notifies (the suppression window is
    /// measured from run start, not from process start).
    #[test]
    fn long_run_goal_fires() {
        let tmp = tmp();
        let transport = Arc::new(RecordingTransport::default());
        let started = Instant::now()
            .checked_sub(Duration::from_secs(601))
            .expect("600s before now is representable");
        let machine = NotifySink::with_parts(
            tmp.path(),
            config(&[Kind::Goal], 600),
            started,
            transport.clone(),
        );
        machine.observe(&Event::GoalAccepted {
            summary: "ran long enough".into(),
        });
        drop(machine);
        assert_eq!(transport.0.lock().unwrap().len(), 1);
    }

    /// The events filter is respected: an unsubscribed kind never fires.
    #[test]
    fn unsubscribed_kinds_do_not_fire() {
        let tmp = tmp();
        let transport = Arc::new(RecordingTransport::default());
        let machine = machine_for(tmp.path(), config(&[Kind::Goal], 0), transport.clone());
        machine.observe(&Event::ValidationVerdict {
            item: "cargo test".into(),
            passed: false,
        });
        machine.observe(&Event::Aborted {
            reason: "interrupted".into(),
            model: "m".into(),
            budget: None,
        });
        machine.observe(&Event::Iteration {
            n: 1,
            max: 40,
            messages: 3,
        });
        drop(machine);
        assert!(transport.0.lock().unwrap().is_empty());
    }

    /// The sink is a pass-through first: the inner sink sees every event
    /// with or without machinery — the run's behavior never changes.
    #[test]
    fn notify_sink_forwards_every_event_to_the_inner_sink() {
        let tmp = tmp();
        let inner_events: Arc<Mutex<Vec<Event>>> = Arc::new(Mutex::new(Vec::new()));
        struct RecordingSink(Arc<Mutex<Vec<Event>>>);
        impl EventSink for RecordingSink {
            fn emit(&mut self, e: Event) {
                self.0.lock().unwrap().push(e);
            }
        }
        let mut inner = RecordingSink(Arc::clone(&inner_events));
        let transport = Arc::new(RecordingTransport::default());
        {
            let mut sink = NotifySink {
                inner: &mut inner,
                machine: Some(NotifySink::with_parts(
                    tmp.path(),
                    config(&[Kind::Goal], 0),
                    Instant::now(),
                    transport,
                )),
            };
            sink.emit(Event::Iteration {
                n: 1,
                max: 40,
                messages: 2,
            });
            sink.emit(Event::GoalAccepted {
                summary: "did it".into(),
            });
        } // machinery drained here
        let seen = inner_events.lock().unwrap();
        assert_eq!(seen.len(), 2, "every event still reaches the inner sink");
        assert!(matches!(seen[0], Event::Iteration { .. }));
        assert!(matches!(seen[1], Event::GoalAccepted { .. }));
    }

    // --- sink failure legs ---------------------------------------------------

    /// A failing sink changes nothing about the run: events flow, and the
    /// failure is noted EXACTLY ONCE in events.jsonl no matter how many
    /// deliveries fail.
    #[test]
    fn sink_failure_is_noted_once_and_never_affects_the_run() {
        let tmp = tmp();
        let machine = machine_for(
            tmp.path(),
            config(&[Kind::Goal, Kind::Abort], 0),
            Arc::new(FailingTransport),
        );
        machine.observe(&Event::GoalAccepted {
            summary: "did it".into(),
        });
        machine.observe(&Event::Aborted {
            reason: "interrupted".into(),
            model: "m".into(),
            budget: None,
        });
        let stats = Arc::clone(&machine.stats);
        drop(machine); // both deliveries fail during the drain; drain + join first
        assert_eq!(
            stats.dropped.load(Ordering::SeqCst),
            0,
            "the queue never dropped: both notices were delivered (and failed)"
        );
        assert_eq!(
            stats.send_errors.load(Ordering::SeqCst),
            2,
            "both deliveries failed"
        );

        let log = std::fs::read_to_string(eventlog::events_path(tmp.path())).unwrap();
        let notes: Vec<&str> = log
            .lines()
            .filter(|l| l.contains("\"notify_error\""))
            .collect();
        assert_eq!(notes.len(), 1, "ONE note per run, never repeated: {log}");
        let note: Value = serde_json::from_str(notes[0]).unwrap();
        assert_eq!(note["type"], "notify_error");
        assert!(
            note["detail"]
                .as_str()
                .unwrap()
                .contains("goal notification"),
            "the first failure's detail is the note: {note}"
        );
    }

    /// A full queue drops instead of blocking: the run never waits on a
    /// notification (the bounded-channel contract). The exact drop count
    /// races with the flusher's pickup of the first notice, so the pinned
    /// invariants are: at least one drop (the queue really is bounded), no
    /// notice lost other than drops, and the burst loop completed while the
    /// flusher was parked (try_send never blocks — this test finishing is
    /// that pin).
    #[test]
    fn full_queue_drops_without_blocking() {
        let tmp = tmp();
        // The flusher parks in deliver(): nothing drains while the test
        // fires a burst.
        struct BlockingTransport {
            parked: Mutex<mpsc::Receiver<()>>,
            delivered: AtomicU64,
        }
        impl Transport for BlockingTransport {
            fn deliver(&self, _notice: &Notice) -> anyhow::Result<()> {
                let _ = self.parked.lock().unwrap().recv();
                self.delivered.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        }
        let (release_tx, release_rx) = mpsc::channel::<()>();
        let transport = Arc::new(BlockingTransport {
            parked: Mutex::new(release_rx),
            delivered: AtomicU64::new(0),
        });
        let machine = machine_for(tmp.path(), config(&[Kind::Goal], 0), transport.clone());
        for i in 0..(CHANNEL_CAP + 5) {
            machine.observe(&Event::GoalAccepted {
                summary: format!("notice {i}"),
            });
        }
        let dropped = machine.stats.dropped.load(Ordering::SeqCst);
        assert!(
            (1..=5).contains(&dropped),
            "the burst overflowed a parked flusher: some notices dropped, got {dropped}"
        );
        // Unblock the parked delivery, then close the release channel so the
        // drained notices fail fast and the flusher exits (drop joins it).
        let _ = release_tx.send(());
        drop(release_tx);
        drop(machine);
        assert_eq!(
            transport.delivered.load(Ordering::SeqCst) + dropped,
            (CHANNEL_CAP + 5) as u64,
            "nothing is lost other than the drops"
        );
    }

    // --- sink shapes ----------------------------------------------------------

    /// The osascript script is the spec shape; quotes/backslashes in a
    /// summary cannot break out of the AppleScript string literal.
    #[test]
    fn osascript_script_matches_spec_shape_and_escapes() {
        assert_eq!(
            osascript_script("goal complete: did it"),
            "display notification \"goal complete: did it\" with title \"chug\""
        );
        assert_eq!(
            osascript_script("say \"hi\" \\ now"),
            "display notification \"say \\\"hi\\\" \\\\ now\" with title \"chug\""
        );
    }

    /// The layad body names the source, kind, and message (layad judges
    /// push-vs-silent from there).
    #[test]
    fn layad_body_carries_source_kind_and_message() {
        let body = layad_body(&Notice {
            kind: Kind::Abort,
            message: "run aborted: interrupted (m)".into(),
        });
        assert_eq!(body["source"], "chug");
        assert_eq!(body["kind"], "abort");
        assert_eq!(body["title"], "chug");
        assert_eq!(body["message"], "run aborted: interrupted (m)");
    }

    /// The real layad transport fails cleanly against a dead port (this is
    /// the production failure shape the note-once latch records).
    #[test]
    fn layad_transport_fails_on_a_dead_port() {
        let transport =
            LayadTransport::new("http://127.0.0.1:1".into()).expect("client builds");
        let err = transport
            .deliver(&Notice {
                kind: Kind::Goal,
                message: "x".into(),
            })
            .expect_err("a closed port cannot deliver");
        assert!(!err.to_string().is_empty());
    }

    /// Summaries flatten to one banner line, truncated to the cap.
    #[test]
    fn long_multiline_summaries_flatten_and_truncate() {
        let flat = flatten("a\n\nb\tc");
        assert_eq!(flat, "a b c");
        let long = flatten(&"x".repeat(MESSAGE_MAX_CHARS + 50));
        assert_eq!(long.chars().count(), MESSAGE_MAX_CHARS);
    }
}
