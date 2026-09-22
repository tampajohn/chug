//! T10: append-only local event log at `<cwd>/.chug/events.jsonl`.
//!
//! Postmortems and meta-evaluations mine the transcript, which is lossy
//! (`[trimmed]` lines) and session-spliced; this log persists the useful
//! slice of the driver's structured event stream — run start, one line per
//! iteration with cumulative tokens, tool results with bounded previews,
//! verifications, goal verdicts, aborts — so a later session has a cheap,
//! untrimmed source of truth (`jq`-mineable, one JSON object per line).
//!
//! Hard rule: best-effort telemetry. Any open/write failure warns ONCE on
//! stderr and is otherwise ignored — the log must never change run
//! behavior. Previews are of tool results only; never request bodies, auth
//! headers, or ledger contents.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use serde_json::{Value, json};

use crate::archive;
use crate::events::{BudgetExceeded, Event, EventSink};
use crate::observ::now_rfc3339;

pub fn events_path(cwd: &Path) -> PathBuf {
    cwd.join(".chug").join("events.jsonl")
}

/// Fresh-run housekeeping: rotate a non-empty events log left by a previous
/// session to `.chug/events-<timestamp>.jsonl` BEFORE the new run's first
/// append — same lifecycle as the transcript (T7), called from the same
/// site so both files rotate together. `--resume` and chat keep appending.
pub fn rotate_fresh(cwd: &Path) -> archive::Outcome {
    let path = events_path(cwd);
    match fs::metadata(&path) {
        Ok(meta) if meta.len() > 0 => {
            archive::rotate(&path, &cwd.join(".chug"), "events", ".jsonl")
        }
        _ => archive::Outcome::Skipped,
    }
}

/// The run-start record: version, commit, model, spec path, cwd, mode —
/// the same fields as the T11 startup banner. Written once per autonomous
/// run and once per chat session.
pub fn run_start(cwd: &Path, mode: &str, spec: Option<&Path>, model: &str) {
    append_line(
        cwd,
        json!({
            "type": "run_start",
            "ts": now_rfc3339(),
            "mode": mode,
            "model": model,
            "spec": spec.map(|p| p.display().to_string()),
            "cwd": cwd.display().to_string(),
            "version": crate::build_info::VERSION,
            "commit": crate::build_info::GIT_COMMIT,
        }),
    );
}

static WARNED: AtomicBool = AtomicBool::new(false);

/// Append one JSON object as a line, creating `.chug/` on demand. Failures
/// warn once and are dropped: telemetry never aborts a run.
fn append_line(cwd: &Path, line: Value) {
    if let Err(e) = append_line_inner(cwd, &line)
        && !WARNED.swap(true, Ordering::Relaxed)
    {
        eprintln!("chug: warning: events log write failed ({e}); continuing without it");
    }
}

fn append_line_inner(cwd: &Path, line: &Value) -> std::io::Result<()> {
    let dir = cwd.join(".chug");
    fs::create_dir_all(&dir)?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(events_path(cwd))?;
    writeln!(file, "{line}")?;
    Ok(())
}

/// An [`EventSink`] tee: mirrors the logged slice of the event stream to
/// `.chug/events.jsonl`, then forwards the event untouched to the inner
/// sink. Installed inside `drive_loop` so every mode (autonomous runs and
/// chat turns, scripted test runs included) is logged with zero config.
pub struct EventLogSink<'a> {
    cwd: PathBuf,
    inner: &'a mut dyn EventSink,
    /// The iteration number announced by the last `Iteration` event; merged
    /// into the iteration line written at the following `Usage` event.
    pending_iter: Option<u32>,
}

impl<'a> EventLogSink<'a> {
    pub fn new(cwd: &Path, inner: &'a mut dyn EventSink) -> Self {
        EventLogSink {
            cwd: cwd.to_path_buf(),
            inner,
            pending_iter: None,
        }
    }
}

impl EventSink for EventLogSink<'_> {
    fn emit(&mut self, e: Event) {
        let line = match &e {
            // Held, not logged: the iteration line is written when Usage
            // arrives with the cumulative token counts.
            Event::Iteration { n, .. } => {
                self.pending_iter = Some(*n);
                None
            }
            Event::Usage { input, output } => Some(match self.pending_iter.take() {
                Some(n) => json!({
                    "type": "iteration",
                    "ts": now_rfc3339(),
                    "n": n,
                    "input_tokens": input,
                    "output_tokens": output,
                }),
                None => json!({
                    "type": "usage",
                    "ts": now_rfc3339(),
                    "input_tokens": input,
                    "output_tokens": output,
                }),
            }),
            Event::ToolResult {
                name,
                ok,
                duration_ms,
                preview,
            } => Some(json!({
                "type": "tool_result",
                "ts": now_rfc3339(),
                "name": name,
                "ok": ok,
                "is_error": !ok,
                "duration_ms": duration_ms,
                "preview": preview.chars().take(200).collect::<String>(),
            })),
            Event::Verifying { cmd } => Some(json!({
                "type": "verifying",
                "ts": now_rfc3339(),
                "cmd": cmd,
            })),
            Event::GoalAccepted { summary } => Some(json!({
                "type": "goal",
                "ts": now_rfc3339(),
                "outcome": "accepted",
                "summary": summary,
            })),
            Event::GoalRejected { reason } => Some(json!({
                "type": "goal",
                "ts": now_rfc3339(),
                "outcome": "rejected",
                "reason": reason,
            })),
            Event::Aborted {
                reason,
                model,
                budget,
            } => {
                // T12: the abort line names the model that died, plus the
                // exhausted budget on budget deaths.
                let mut line = json!({
                    "type": "abort",
                    "ts": now_rfc3339(),
                    "reason": reason,
                    "model": model,
                });
                if let Some(budget) = budget {
                    let (kind, max) = match budget {
                        BudgetExceeded::Iterations { max } => ("iterations", u64::from(*max)),
                        BudgetExceeded::Minutes { max } => ("minutes", *max),
                    };
                    line["budget_kind"] = json!(kind);
                    line["budget_max"] = json!(max);
                }
                Some(line)
            }
            // Everything else (model text, tool starts, ledger snapshots,
            // steering, risk verdicts, chat turn boundaries) stays out of
            // the log per the T10 spec.
            _ => None,
        };
        if let Some(line) = line {
            append_line(&self.cwd, line);
        }
        self.inner.emit(e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::Outcome;

    struct NullSink;
    impl EventSink for NullSink {
        fn emit(&mut self, _e: Event) {}
    }

    fn read_lines(cwd: &Path) -> Vec<Value> {
        fs::read_to_string(events_path(cwd))
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).expect("every line parses as JSON"))
            .collect()
    }

    #[test]
    fn run_start_line_has_model_spec_cwd_mode() {
        let tmp = tempfile::tempdir().unwrap();
        run_start(
            tmp.path(),
            "run",
            Some(Path::new("/repo/SPEC.md")),
            "test-model",
        );
        let lines = read_lines(tmp.path());
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0]["type"], "run_start");
        assert_eq!(lines[0]["mode"], "run");
        assert_eq!(lines[0]["model"], "test-model");
        assert_eq!(lines[0]["spec"], "/repo/SPEC.md");
        assert_eq!(lines[0]["cwd"], tmp.path().display().to_string());
        // T11: the banner's build identification rides along.
        assert_eq!(lines[0]["version"], crate::build_info::VERSION);
        assert_eq!(lines[0]["commit"], crate::build_info::GIT_COMMIT);
        assert!(lines[0]["ts"].as_str().unwrap().ends_with('Z'));
    }

    #[test]
    fn rotate_fresh_archives_non_empty_events_log() {
        let tmp = tempfile::tempdir().unwrap();
        run_start(tmp.path(), "run", None, "m");

        let out = rotate_fresh(tmp.path());
        let Outcome::Archived(dst) = out else {
            panic!("expected Archived, got {out:?}");
        };
        let name = dst.file_name().unwrap().to_string_lossy().to_string();
        assert!(name.starts_with("events-") && name.ends_with(".jsonl"), "{name}");
        assert!(fs::read_to_string(&dst).unwrap().contains("run_start"));
        assert!(!events_path(tmp.path()).exists(), "log moved away");

        // Absent now → skipped; empty file → skipped and left alone.
        assert_eq!(rotate_fresh(tmp.path()), Outcome::Skipped);
        fs::write(events_path(tmp.path()), "").unwrap();
        assert_eq!(rotate_fresh(tmp.path()), Outcome::Skipped);
        assert!(events_path(tmp.path()).exists());
    }

    #[test]
    fn sink_merges_iteration_and_usage_into_one_line() {
        let tmp = tempfile::tempdir().unwrap();
        let mut inner = NullSink;
        let mut sink = EventLogSink::new(tmp.path(), &mut inner);
        sink.emit(Event::Iteration {
            n: 3,
            max: 40,
            messages: 12,
        });
        sink.emit(Event::Usage {
            input: 1234,
            output: 56,
        });
        let lines = read_lines(tmp.path());
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0]["type"], "iteration");
        assert_eq!(lines[0]["n"], 3);
        assert_eq!(lines[0]["input_tokens"], 1234);
        assert_eq!(lines[0]["output_tokens"], 56);
    }

    #[test]
    fn sink_truncates_tool_preview_at_200_chars() {
        let tmp = tempfile::tempdir().unwrap();
        let mut inner = NullSink;
        let mut sink = EventLogSink::new(tmp.path(), &mut inner);
        sink.emit(Event::ToolResult {
            name: "bash".into(),
            ok: false,
            duration_ms: 7,
            preview: "x".repeat(500),
        });
        let lines = read_lines(tmp.path());
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0]["type"], "tool_result");
        assert_eq!(lines[0]["ok"], false);
        assert_eq!(lines[0]["is_error"], true);
        assert_eq!(lines[0]["duration_ms"], 7);
        let preview = lines[0]["preview"].as_str().unwrap();
        assert_eq!(preview.chars().count(), 200);
    }

    #[test]
    fn sink_logs_goal_abort_and_verifying_but_not_other_events() {
        let tmp = tempfile::tempdir().unwrap();
        let mut inner = NullSink;
        let mut sink = EventLogSink::new(tmp.path(), &mut inner);
        sink.emit(Event::ModelText("hello".into()));
        sink.emit(Event::ToolStart { name: "bash".into() });
        sink.emit(Event::LedgerChanged("# Ledger".into()));
        sink.emit(Event::SteeringQueued("note".into()));
        sink.emit(Event::Verifying {
            cmd: "cargo test".into(),
        });
        sink.emit(Event::GoalAccepted {
            summary: "did it".into(),
        });
        sink.emit(Event::GoalRejected {
            reason: "check command failed".into(),
        });
        sink.emit(Event::Aborted {
            reason: "iteration budget exceeded".into(),
            model: "test-model".into(),
            budget: Some(BudgetExceeded::Iterations { max: 40 }),
        });
        let lines = read_lines(tmp.path());
        let types: Vec<&str> = lines.iter().map(|l| l["type"].as_str().unwrap()).collect();
        assert_eq!(types, ["verifying", "goal", "goal", "abort"]);
        assert_eq!(lines[0]["cmd"], "cargo test");
        assert_eq!(lines[1]["outcome"], "accepted");
        assert_eq!(lines[1]["summary"], "did it");
        assert_eq!(lines[2]["outcome"], "rejected");
        assert_eq!(lines[2]["reason"], "check command failed");
        assert_eq!(lines[3]["reason"], "iteration budget exceeded");
        // T12: model + exhausted budget ride the abort line.
        assert_eq!(lines[3]["model"], "test-model");
        assert_eq!(lines[3]["budget_kind"], "iterations");
        assert_eq!(lines[3]["budget_max"], 40);
    }

    #[test]
    fn unwritable_events_path_is_silently_ignored() {
        let tmp = tempfile::tempdir().unwrap();
        // Poison the log path: a directory where the file should be makes
        // every append open fail. Nothing may panic.
        fs::create_dir_all(events_path(tmp.path())).unwrap();
        let mut inner = NullSink;
        let mut sink = EventLogSink::new(tmp.path(), &mut inner);
        sink.emit(Event::Iteration {
            n: 1,
            max: 1,
            messages: 1,
        });
        sink.emit(Event::Usage { input: 1, output: 1 });
        sink.emit(Event::Aborted {
            reason: "operator abort".into(),
            model: "m".into(),
            budget: None,
        });
        run_start(tmp.path(), "run", None, "m");
    }
}
