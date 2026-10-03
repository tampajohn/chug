use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Context;
use serde_json::{Value, json};

use crate::events::{Event, EventSink};

pub const DEFAULT_LAYA_URL: &str = "http://127.0.0.1:8420";
/// Judge calls must fail fast so the agent loop is never stalled by layad.
const JUDGE_TIMEOUT_SECS: u64 = 2;
const COMMAND_PREVIEW_CHARS: usize = 200;
/// Block threshold on `probabilities.destructive`.
const BLOCK_THRESHOLD: f64 = 0.5;

/// One laya classification.
#[derive(Debug, Clone, PartialEq)]
pub struct Verdict {
    pub choice: String,
    pub p_destructive: f64,
}

/// Injectable judge (trait object) so tests can supply canned verdicts without
/// any network.
pub trait Judge: Send {
    /// Classify a bash command. `Err` means the judge failed; the gate fails
    /// open.
    fn judge(&mut self, command: &str) -> Result<Verdict, String>;
}

/// Production judge: POSTs to a laya judge server (`POST {LAYA_URL}/judge`).
pub struct LayaJudge {
    url: String,
    http: reqwest::blocking::Client,
}

impl LayaJudge {
    pub fn from_env() -> anyhow::Result<Self> {
        // One env read at the one call site; the selection logic is the pure
        // [`laya_url`] (the mcp_serve.rs env rule), pinned by table tests.
        let url = laya_url(std::env::var("LAYA_URL").ok().as_deref());
        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(JUDGE_TIMEOUT_SECS))
            .build()
            .context("building risk-gate HTTP client")?;
        Ok(Self { url, http })
    }
}

/// The layad base URL, parsed PURELY from an optional `LAYA_URL` value
/// ([`LayaJudge::from_env`] is the only env reader): a set, non-blank value
/// wins VERBATIM (trimming would silently alter the URL the operator
/// configured); unset or blank falls back to [`DEFAULT_LAYA_URL`].
pub(crate) fn laya_url(raw: Option<&str>) -> String {
    raw.filter(|s| !s.trim().is_empty())
        .unwrap_or(DEFAULT_LAYA_URL)
        .to_string()
}

impl Judge for LayaJudge {
    fn judge(&mut self, command: &str) -> Result<Verdict, String> {
        let url = format!("{}/judge", self.url.trim_end_matches('/'));
        let body = judge_request_body(command);
        let resp = self
            .http
            .post(&url)
            .json(&body)
            .send()
            .map_err(|e| format!("layad request failed: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("layad returned HTTP {}", resp.status()));
        }
        let value: Value = resp
            .json()
            .map_err(|e| format!("layad response is not valid JSON: {e}"))?;
        parse_verdict(&value)
    }
}

/// `CHUG_JUDGE` — the judge client selection (T204 spec req 5):
/// `daemon` | `http` | `off`. Unset (or empty) defaults to `daemon` — the
/// baked-in judge daemon over its 0600 unix socket (auto-spawned on the
/// first judge call); `http` is the external layad at `LAYA_URL`
/// byte-for-byte today's path (the escape hatch); `off` disables the judge.
pub const JUDGE_ENV: &str = "CHUG_JUDGE";

/// Select the judge client per `CHUG_JUDGE` (see [`JUDGE_ENV`]). The risk
/// gate owns fail-open; every mode's judge errors degrade exactly as the
/// HTTP path's do today (logged `gate_failure`, command allowed).
pub fn judge_from_env() -> anyhow::Result<Box<dyn Judge>> {
    // One env read at the one call site; the selection table itself is the
    // pure [`judge_mode`] (the mcp_serve.rs env rule), pinned branch-by-
    // branch by table tests below.
    let raw = std::env::var(JUDGE_ENV).ok();
    match judge_mode(raw.as_deref()).map_err(|e| anyhow::anyhow!(e))? {
        JudgeMode::Daemon => Ok(Box::new(crate::daemon::DaemonJudge::from_env()?)),
        JudgeMode::Http => Ok(Box::new(LayaJudge::from_env()?)),
        JudgeMode::Off => Ok(Box::new(OffJudge)),
    }
}

/// Which judge client [`judge_mode`] selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum JudgeMode {
    /// The baked-in judge daemon over its 0600 unix socket (auto-spawned).
    Daemon,
    /// The external layad at `LAYA_URL` — today's reqwest TCP path.
    Http,
    /// The judge is disabled — every classification fails open, logged.
    Off,
}

/// The PURE `CHUG_JUDGE` selection table ([`judge_from_env`] is its only
/// caller — the process-global env is read exactly once, there). Unset,
/// empty, or blank defaults to [`JudgeMode::Daemon`] (T204 spec req 5: the
/// baked-in daemon is the default judge); `daemon` (case/whitespace
/// tolerant, matching today's trim+lowercase) is explicit Daemon, `http` is
/// the external layad escape hatch, `off` disables the judge. Any other
/// value is a HARD ERROR naming the valid modes: an unknown mode must
/// refuse the run (driver.rs/chat.rs propagate the error) rather than
/// silently picking a client the operator did not ask for.
pub(crate) fn judge_mode(raw: Option<&str>) -> Result<JudgeMode, String> {
    let normalized = raw
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_ascii_lowercase());
    match normalized.as_deref() {
        None | Some("daemon") => Ok(JudgeMode::Daemon),
        Some("http") => Ok(JudgeMode::Http),
        Some("off") => Ok(JudgeMode::Off),
        Some(other) => Err(format!(
            "unknown {JUDGE_ENV} mode {other:?} (expected daemon|http|off)"
        )),
    }
}

/// `CHUG_JUDGE=off`: the judge is disabled — every classification fails
/// open, logged as a `gate_failure` in `.chug/risk_verdicts.jsonl` (the same
/// degrade shape as an unreachable judge, so "the gate was off" is visible
/// in the verdict log rather than silent).
struct OffJudge;

impl Judge for OffJudge {
    fn judge(&mut self, _command: &str) -> Result<Verdict, String> {
        Err("judge disabled (CHUG_JUDGE=off)".into())
    }
}

/// Request body per SPEC-3-tools-riskgate.md. Fixed context + criteria: laya
/// does text classification ONLY, so the command string is the only variable
/// input. `pub(crate)`: the daemon client (`CHUG_JUDGE=daemon`) POSTs the
/// exact same body over the unix socket.
pub(crate) fn judge_request_body(command: &str) -> Value {
    json!({
        "state": {
            "context": "An autonomous coding agent working toward a goal in a project directory wants to execute a bash command.",
            "command": command,
        },
        "questions": {
            "risk": {
                "type": "choice",
                "instructions": "Judge whether the bash command is destructive or hard to reverse: permanently deletes files or state that cannot be regenerated, force-pushes, drops data, kills processes it does not own, modifies things outside the project directory.",
                "criteria": {
                    "destructive": "permanently destroys work or state that cannot be regenerated, or escapes the project directory",
                    "risky": "reversible but disruptive, or destructive only to regenerable artifacts (build outputs, caches, node_modules)",
                    "safe": "read-only or fully reversible within the project",
                },
            },
        },
    })
}

pub(crate) fn parse_verdict(value: &Value) -> Result<Verdict, String> {
    let choice = value
        .pointer("/answers/risk/choice")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing answers.risk.choice".to_string())?
        .to_string();
    let p = value
        .pointer("/answers/risk/probabilities/destructive")
        .and_then(Value::as_f64)
        .ok_or_else(|| "missing answers.risk.probabilities.destructive".to_string())?;
    Ok(Verdict {
        choice,
        p_destructive: p,
    })
}

/// Outcome of a gate check on a bash command.
pub enum GateDecision {
    /// The command may be executed.
    Allowed,
    /// The command was blocked; carries the error tool_result content.
    Blocked(String),
}

/// The semantic pre-execution safety layer for `bash` tool calls.
pub struct RiskGate {
    judge: Box<dyn Judge>,
    disabled: bool,
    log_path: PathBuf,
}

impl RiskGate {
    pub fn new(judge: Box<dyn Judge>, cwd: &Path) -> Self {
        RiskGate {
            judge,
            disabled: false,
            log_path: cwd.join(".chug").join("risk_verdicts.jsonl"),
        }
    }

    /// Operator override: after an `allow destructive` steering note the gate
    /// stops consulting the judge for the rest of the run.
    pub fn disable(&mut self, sink: &mut dyn EventSink) {
        if !self.disabled {
            self.disabled = true;
            sink.emit(Event::RiskGateDisabled);
        }
    }

    /// True once `disable` has been called (operator override active);
    /// used to skip observability gate events that would just be noise.
    pub fn is_disabled(&self) -> bool {
        self.disabled
    }

    /// Check a bash command before execution. Never hard-fails: judge failures
    /// are logged and the command is allowed (fail-open).
    pub fn check(&mut self, command: &str, sink: &mut dyn EventSink) -> GateDecision {
        if self.disabled {
            return GateDecision::Allowed;
        }
        let preview: String = command.chars().take(COMMAND_PREVIEW_CHARS).collect();

        let judgment = self.judge.judge(command);
        match &judgment {
            Ok(verdict) => {
                let blocked = verdict.p_destructive >= BLOCK_THRESHOLD;
                self.log_verdict(&preview, &verdict.choice, verdict.p_destructive, blocked, None);
                sink.emit(Event::RiskVerdict {
                    blocked,
                    choice: verdict.choice.clone(),
                    p: verdict.p_destructive,
                    preview: preview.clone(),
                });
                if blocked {
                    GateDecision::Blocked(blocked_message(command, verdict.p_destructive))
                } else {
                    GateDecision::Allowed
                }
            }
            Err(failure) => {
                // Fail open: record the failure, let the command run.
                self.log_verdict(&preview, "gate_failure", 0.0, false, Some(failure));
                GateDecision::Allowed
            }
        }
    }

    fn log_verdict(
        &self,
        preview: &str,
        choice: &str,
        p_destructive: f64,
        blocked: bool,
        gate_failure: Option<&str>,
    ) {
        let mut entry = json!({
            "ts": unix_ts(),
            "command_preview": preview,
            "choice": choice,
            "p_destructive": p_destructive,
            "blocked": blocked,
        });
        if let Some(failure) = gate_failure {
            entry["gate_failure"] = json!(failure);
        }
        if let Some(parent) = self.log_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let append = || -> std::io::Result<()> {
            let mut f = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.log_path)?;
            writeln!(f, "{entry}")
        };
        let _ = append();
    }
}

fn blocked_message(command: &str, p: f64) -> String {
    format!(
        "BLOCKED by risk gate (destructive p={p:.2}): {command}. Choose a safer \
         alternative, or wait for an operator steering note: 'allow destructive' \
         disables the gate for the rest of this run."
    )
}

fn unix_ts() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{Event, EventSink};

    /// Records emitted events so tests can assert on them.
    #[derive(Default)]
    struct RecordingSink(Vec<Event>);
    impl EventSink for RecordingSink {
        fn emit(&mut self, e: Event) {
            self.0.push(e);
        }
    }

    #[derive(Clone)]
    struct CannedJudge(Result<Verdict, String>);
    impl Judge for CannedJudge {
        fn judge(&mut self, _command: &str) -> Result<Verdict, String> {
            self.0.clone()
        }
    }

    fn gate_with(judge: CannedJudge) -> (RiskGate, PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let g = RiskGate::new(Box::new(judge), tmp.path());
        let log = tmp.path().join(".chug/risk_verdicts.jsonl");
        (g, log)
    }

    fn read_log(log: &Path) -> Vec<Value> {
        let text = fs::read_to_string(log).unwrap();
        text.lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    #[test]
    fn destructive_high_p_blocks() {
        let (mut gate, log) = gate_with(CannedJudge(Ok(Verdict {
            choice: "destructive".into(),
            p_destructive: 0.7,
        })));
        let mut sink = RecordingSink::default();
        let decision = gate.check("rm -rf build/", &mut sink);
        match decision {
            GateDecision::Blocked(msg) => {
                assert!(msg.contains("BLOCKED"));
                assert!(msg.contains("rm -rf build/"));
                assert!(msg.contains("allow destructive"));
            }
            GateDecision::Allowed => panic!("expected Blocked"),
        }
        // Command NOT executed is a driver-level behavior; the gate only decides.
        let entries = read_log(&log);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["choice"], "destructive");
        assert_eq!(entries[0]["p_destructive"], 0.7);
        assert_eq!(entries[0]["blocked"], true);
        assert!(entries[0].get("gate_failure").is_none());
        assert_eq!(
            sink.0,
            vec![Event::RiskVerdict {
                blocked: true,
                choice: "destructive".into(),
                p: 0.7,
                preview: "rm -rf build/".into(),
            }]
        );
    }

    #[test]
    fn low_p_allows() {
        let (mut gate, log) = gate_with(CannedJudge(Ok(Verdict {
            choice: "risky".into(),
            p_destructive: 0.4,
        })));
        let mut sink = RecordingSink::default();
        assert!(matches!(gate.check("cargo clean", &mut sink), GateDecision::Allowed));
        let entries = read_log(&log);
        assert_eq!(entries[0]["blocked"], false);
        assert_eq!(entries[0]["choice"], "risky");
    }

    #[test]
    fn judge_error_fails_open_and_logs_failure() {
        let (mut gate, log) = gate_with(CannedJudge(Err("layad unreachable".into())));
        let mut sink = RecordingSink::default();
        assert!(matches!(
            gate.check("rm -rf build/", &mut sink),
            GateDecision::Allowed
        ));
        let entries = read_log(&log);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["blocked"], false);
        assert_eq!(entries[0]["choice"], "gate_failure");
        assert_eq!(entries[0]["gate_failure"], "layad unreachable");
        // No RiskVerdict event for a failed judgment.
        assert!(sink.0.is_empty());
    }

    #[test]
    fn allow_destructive_disables_gate_for_subsequent_commands() {
        // Judge that would always block.
        let (mut gate, log) = gate_with(CannedJudge(Ok(Verdict {
            choice: "destructive".into(),
            p_destructive: 0.9,
        })));
        let mut sink = RecordingSink::default();
        assert!(matches!(
            gate.check("rm -rf a", &mut sink),
            GateDecision::Blocked(_)
        ));
        gate.disable(&mut sink);
        assert!(gate.disabled);
        // After the override the judge is never consulted.
        assert!(matches!(
            gate.check("rm -rf b", &mut sink),
            GateDecision::Allowed
        ));
        // disable() is idempotent; only one RiskGateDisabled event.
        gate.disable(&mut sink);
        let disables = sink
            .0
            .iter()
            .filter(|e| matches!(e, Event::RiskGateDisabled))
            .count();
        assert_eq!(disables, 1);
        // Only the pre-disable judgment is in the log.
        assert_eq!(read_log(&log).len(), 1);
    }

    #[test]
    fn parses_laya_response_shape() {
        let value = json!({
            "answers": {
                "risk": {
                    "choice": "destructive",
                    "probabilities": {"destructive": 0.83, "risky": 0.1, "safe": 0.07}
                }
            }
        });
        let v = parse_verdict(&value).unwrap();
        assert_eq!(v.choice, "destructive");
        assert!((v.p_destructive - 0.83).abs() < 1e-9);
    }

    #[test]
    fn malformed_response_is_an_error() {
        assert!(parse_verdict(&json!({})).is_err());
        assert!(parse_verdict(&json!({"answers": {"risk": {"choice": "safe"}}})).is_err());
    }

    // ------------------------------------------------------------------
    // T204 spec req 5 — the CHUG_JUDGE + LAYA_URL selection tables. The
    // tables are PURE helpers (`judge_mode` / `laya_url`) so every branch
    // pins without touching process-global env (the mcp_serve.rs env rule);
    // `judge_from_env` is the single env-reading call site and its wiring
    // is pinned separately under the crate's one env lock below.
    // ------------------------------------------------------------------

    /// The full `CHUG_JUDGE` selection table, branch by branch. The
    /// validator's M3 mutant (unset/empty -> http) and any default drift
    /// die here: the spec'd default is the DAEMON for unset, empty, AND
    /// blank values.
    #[test]
    fn chug_judge_mode_table_is_pinned() {
        // Unset / empty / blank / explicit daemon, case- and
        // whitespace-tolerant (today's trim + to_ascii_lowercase) — all
        // select the baked-in daemon.
        for raw in [
            None,
            Some(""),
            Some("   "),
            Some("daemon"),
            Some("DAEMON"),
            Some("  Daemon  "),
        ] {
            assert_eq!(judge_mode(raw), Ok(JudgeMode::Daemon), "raw {raw:?}");
        }
        // The external layad escape hatch.
        for raw in [Some("http"), Some("HTTP"), Some(" http ")] {
            assert_eq!(judge_mode(raw), Ok(JudgeMode::Http), "raw {raw:?}");
        }
        // The disabled judge.
        for raw in [Some("off"), Some("OFF"), Some("Off ")] {
            assert_eq!(judge_mode(raw), Ok(JudgeMode::Off), "raw {raw:?}");
        }
        // Unknown/garbage -> a HARD ERROR naming the valid modes (the
        // documented fallback: driver.rs/chat.rs propagate the error and
        // the run refuses to start — it never silently picks a client the
        // operator did not ask for).
        for garbage in ["garbage", "daemons", "0", "daemon,http", "GARBAGE"] {
            let err = judge_mode(Some(garbage)).expect_err(garbage);
            assert!(err.contains("unknown CHUG_JUDGE mode"), "{garbage}: {err}");
            assert!(err.contains("expected daemon|http|off"), "{garbage}: {err}");
        }
        // The error names the offending (trimmed, lowercased) value.
        let err = judge_mode(Some("  Bogus  ")).expect_err("bogus");
        assert!(err.contains("bogus"), "the error names the value: {err}");
    }

    /// The `LAYA_URL` selection table: unset or blank -> the default layad
    /// address; a set value is used VERBATIM (blankness is judged on the
    /// trim, the value is never trimmed — the operator's URL is not
    /// silently altered).
    #[test]
    fn laya_url_table_is_pinned() {
        assert_eq!(laya_url(None), DEFAULT_LAYA_URL);
        assert_eq!(laya_url(Some("")), DEFAULT_LAYA_URL);
        assert_eq!(laya_url(Some("   ")), DEFAULT_LAYA_URL);
        assert_eq!(laya_url(Some("http://10.0.0.5:9000")), "http://10.0.0.5:9000");
        assert_eq!(
            laya_url(Some("  http://10.0.0.5:9000  ")),
            "  http://10.0.0.5:9000  "
        );
    }

    /// SAFETY: set/remove one env var — serialized by [`DELEGATE_ENV_LOCK`]
    /// (the crate's ONE process-global env lock, T129), saved value
    /// restored by the caller.
    fn set_env(var: &str, value: Option<&std::ffi::OsStr>) {
        unsafe {
            match value {
                Some(v) => std::env::set_var(var, v),
                None => std::env::remove_var(var),
            }
        }
    }

    /// The env WIRING behind the pure table: `judge_from_env` really reads
    /// `CHUG_JUDGE` once and builds the matching client. Observable per
    /// leg through each client's error FLAVOR (no type introspection on
    /// the `Box<dyn Judge>`): `off` says "judge disabled", `http` against a
    /// dead layad says "layad request failed", and the daemon client says
    /// "judge daemon" — so the UNSET and EMPTY default legs prove the
    /// daemon client is what the default builds (an http or off mutant
    /// flips the flavor and fails here).
    #[test]
    fn judge_from_env_wiring_selects_per_chug_judge() {
        let _timing = crate::testsupport::timing_guard();
        let _guard = crate::delegate::tests::DELEGATE_ENV_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let saved: Vec<(&'static str, Option<std::ffi::OsString>)> = vec![
            (
                JUDGE_ENV,
                std::env::var_os(JUDGE_ENV),
            ),
            ("LAYA_URL", std::env::var_os("LAYA_URL")),
            (
                crate::daemon::BINARY_ENV,
                std::env::var_os(crate::daemon::BINARY_ENV),
            ),
            (
                crate::daemon::HOME_ENV,
                std::env::var_os(crate::daemon::HOME_ENV),
            ),
            (
                crate::daemon::SOCK_ENV,
                std::env::var_os(crate::daemon::SOCK_ENV),
            ),
        ];
        struct Restore(Vec<(&'static str, Option<std::ffi::OsString>)>);
        impl Drop for Restore {
            fn drop(&mut self) {
                for (var, value) in &self.0 {
                    set_env(var, value.as_deref());
                }
            }
        }
        let _restore = Restore(saved);

        // off: the judge is disabled — every classification fails open.
        set_env(JUDGE_ENV, Some("off".as_ref()));
        let mut judge = judge_from_env().expect("off constructs");
        assert_eq!(
            judge.judge("rm -rf /"),
            Err("judge disabled (CHUG_JUDGE=off)".to_string())
        );

        // garbage: the run refuses to start (the pure table's error, live).
        set_env(JUDGE_ENV, Some("garbage".as_ref()));
        let err = match judge_from_env() {
            Ok(_) => panic!("garbage refuses to construct a judge"),
            Err(e) => e,
        };
        assert!(err.to_string().contains("unknown CHUG_JUDGE mode"), "{err}");

        // http: the LAYA_URL client — against a guaranteed-dead address the
        // error flavor is layad's (never a spawn, never "judge disabled").
        set_env(JUDGE_ENV, Some("http".as_ref()));
        set_env("LAYA_URL", Some("http://127.0.0.1:1".as_ref()));
        let mut judge = judge_from_env().expect("http constructs");
        let err = match judge.judge("rm -rf /") {
            Ok(_) => panic!("a dead layad must not judge"),
            Err(e) => e,
        };
        assert!(err.contains("layad request failed"), "{err}");

        // unset + empty: the DEFAULT is the daemon client (spec req 5). With
        // the delegate seam pointed at /bin/false the daemon client's ensure
        // fails with the SPAWN flavor — proof the default built the daemon
        // client (http would say "layad request failed", off "judge
        // disabled"). CHUG_HOME is scoped to a tempdir so the lock/log
        // touch nothing real.
        let tmp = tempfile::tempdir().expect("tempdir");
        for mode in [None, Some("".as_ref())] {
            set_env(JUDGE_ENV, mode);
            set_env(crate::daemon::BINARY_ENV, Some("/bin/false".as_ref()));
            set_env(crate::daemon::HOME_ENV, Some(tmp.path().as_os_str()));
            set_env(
                crate::daemon::SOCK_ENV,
                Some(tmp.path().join("wiring.sock").as_os_str()),
            );
            let mut judge = judge_from_env().expect("default constructs");
            let err = match judge.judge("rm -rf /") {
                Ok(_) => panic!("a daemon that cannot come up must not judge"),
                Err(e) => e,
            };
            assert!(err.contains("judge daemon"), "mode {mode:?}: {err}");
        }
    }
}
