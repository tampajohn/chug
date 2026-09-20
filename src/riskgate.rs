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
        let url = std::env::var("LAYA_URL")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_LAYA_URL.to_string());
        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(JUDGE_TIMEOUT_SECS))
            .build()
            .context("building risk-gate HTTP client")?;
        Ok(Self { url, http })
    }
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

/// Request body per SPEC-3-tools-riskgate.md. Fixed context + criteria: laya
/// does text classification ONLY, so the command string is the only variable
/// input.
fn judge_request_body(command: &str) -> Value {
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

fn parse_verdict(value: &Value) -> Result<Verdict, String> {
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
}
