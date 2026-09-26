use std::io::Write;
use std::path::PathBuf;

/// Events emitted by the driver loop. The driver never writes to stdout/stderr
/// directly; every observable output flows through an [`EventSink`].
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Iteration {
        n: u32,
        max: u32,
        /// Messages in the transcript at this boundary (part of the console line).
        messages: u32,
    },
    ModelText(String),
    ToolStart {
        name: String,
    },
    ToolResult {
        name: String,
        ok: bool,
        /// Wall-clock time the tool call took (logged by the T10 events
        /// log; the console/TUI sinks ignore it).
        duration_ms: u64,
        /// First 500 chars of the tool result content.
        preview: String,
    },
    LedgerChanged(String),
    Verifying {
        cmd: String,
    },
    GoalRejected {
        reason: String,
    },
    GoalAccepted {
        summary: String,
    },
    Aborted {
        reason: String,
        /// Model in use when the loop died (T12: named so the operator can
        /// resume with a different one).
        model: String,
        /// Which budget ran out — `Some` only on budget deaths (T12);
        /// operator/stuck aborts carry `None`.
        budget: Option<BudgetExceeded>,
    },
    Usage {
        input: u64,
        output: u64,
    },
    /// T17 telemetry: the one-shot budget-low warning was injected into the
    /// transcript. The notice itself already reached the user as a message;
    /// this event exists only so `.chug/events.jsonl` records that it fired,
    /// with the remaining counts at fire time (a session whose transcript
    /// didn't survive still shows whether — and how close to the ceiling —
    /// the warning fired).
    BudgetLow {
        remaining_iters: u32,
        remaining_secs: u64,
        remaining_tokens: Option<u64>,
    },
    /// T38 telemetry: a response came back truncated at the API output-token
    /// ceiling (`stop_reason=max_tokens`) and the driver injected the chunking
    /// advisory into the transcript. The advisory itself already reached the
    /// model as a message; this event exists only so `.chug/events.jsonl`
    /// records that it fired — one line per injected advisory, no latch, so a
    /// later `jq` pass can count truncations per run.
    OutputTruncated,
    SteeringQueued(String),
    /// One risk-gate judgment on a bash command (only when --risk-gate is on).
    RiskVerdict {
        blocked: bool,
        choice: String,
        p: f64,
        preview: String,
    },
    /// An `allow destructive` operator note disabled the gate for this run.
    RiskGateDisabled,
    /// Chat mode: the user submitted a new objective and a turn is starting.
    TurnStart {
        objective: String,
    },
    /// Chat mode: the active turn ended; the app returns to idle.
    TurnEnd {
        reason: TurnEndReason,
    },
}

/// Which budget killed the loop (T12). `Some` on [`Event::Aborted`] only for
/// budget deaths, so the abort output can name the exhausted budget next to
/// the resume-with-different-model hint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetExceeded {
    /// Iteration budget (`--max-iters`).
    Iterations { max: u32 },
    /// Wall-clock budget (`--max-minutes`).
    Minutes { max: u64 },
    /// Token budget (`--max-tokens`): cumulative input+output tokens (T15).
    Tokens { max: u64 },
}

impl BudgetExceeded {
    /// Human label for the abort block: `40 iterations` / `120 minutes` /
    /// `250000 tokens`.
    pub fn label(self) -> String {
        match self {
            BudgetExceeded::Iterations { max } => format!("{max} iterations"),
            BudgetExceeded::Minutes { max } => format!("{max} minutes"),
            BudgetExceeded::Tokens { max } => format!("{max} tokens"),
        }
    }
}

/// Why a chat-mode turn ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnEndReason {
    /// Natural stop: the assistant answered without any tool call.
    Completed,
    /// `goal_complete` was accepted (verified by `/check` when configured).
    GoalAccepted,
    /// Operator interrupt (Esc/q) or the stuck tripwire fired.
    Interrupted,
    /// A per-turn budget (iterations or wall-clock) was exceeded.
    BudgetExceeded,
}

impl TurnEndReason {
    /// Human label used by the UI for the turn-boundary banner.
    pub fn label(self) -> &'static str {
        match self {
            TurnEndReason::Completed => "completed",
            TurnEndReason::GoalAccepted => "goal accepted",
            TurnEndReason::Interrupted => "interrupted",
            TurnEndReason::BudgetExceeded => "budget exceeded",
        }
    }
}

pub trait EventSink {
    fn emit(&mut self, e: Event);
}

/// Headless sink. Reproduces the pre-TUI log format byte-for-byte:
/// progress lines on stderr, goal-complete/abort blocks on stdout.
pub struct ConsoleSink {
    out: Box<dyn Write>,
    err: Box<dyn Write>,
    cwd: PathBuf,
    last_ledger: String,
    /// Latest cumulative token totals (T14): the driver emits `Usage` after
    /// every response with run-to-date totals, so the last one wins. `None`
    /// until the first response (an early abort then prints no tokens line).
    last_usage: Option<(u64, u64)>,
}

impl ConsoleSink {
    pub fn new(cwd: PathBuf) -> Self {
        ConsoleSink {
            out: Box::new(std::io::stdout()),
            err: Box::new(std::io::stderr()),
            cwd,
            last_ledger: String::new(),
            last_usage: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_writers(out: Box<dyn Write>, err: Box<dyn Write>, cwd: PathBuf) -> Self {
        ConsoleSink {
            out,
            err,
            cwd,
            last_ledger: String::new(),
            last_usage: None,
        }
    }

    /// `tokens: <input> in / <output> out (cumulative)` — omitted entirely
    /// when the loop died before any API response.
    fn tokens_line(&self) -> Option<String> {
        self.last_usage
            .map(|(input, output)| format!("tokens: {input} in / {output} out (cumulative)"))
    }
}

impl EventSink for ConsoleSink {
    fn emit(&mut self, e: Event) {
        match e {
            Event::Iteration { n, max, messages } => {
                let _ = writeln!(self.err, "[chug] iteration {n} / {max} ({messages} messages)");
            }
            Event::ModelText(text) => {
                if !text.is_empty() {
                    let _ = writeln!(self.err, "[chug] model: {}", preview(&text, 200));
                }
            }
            Event::ToolStart { .. } => {}
            Event::ToolResult { name, ok, .. } => {
                let label = if ok { "ok" } else { "error" };
                let _ = writeln!(self.err, "[chug] tool {name} -> {label}");
            }
            Event::LedgerChanged(text) => self.last_ledger = text,
            Event::Verifying { .. } => {}
            Event::GoalRejected { reason } => {
                let _ = writeln!(self.err, "[chug] goal_complete rejected: {reason}");
            }
            Event::GoalAccepted { summary } => {
                let _ = writeln!(self.out, "chug: goal complete");
                let _ = writeln!(self.out, "summary: {summary}");
                // T14: what the run cost, right next to the summary.
                if let Some(line) = self.tokens_line() {
                    let _ = writeln!(self.out, "{line}");
                }
                let _ = writeln!(self.out, "\n--- LEDGER.md ---");
                let _ = writeln!(self.out, "{}", self.last_ledger);
            }
            Event::Aborted {
                reason,
                model,
                budget,
            } => {
                let _ = writeln!(self.err, "chug: abort: {reason}");
                let _ = writeln!(self.out, "--- LEDGER.md ---");
                let _ = writeln!(self.out, "{}", self.last_ledger);
                let _ = writeln!(self.out, "---");
                // T12: name the model that died (and the exhausted budget on
                // budget deaths) so the operator can resume with a fallback.
                let _ = writeln!(self.out, "model: {model}");
                if let Some(budget) = budget {
                    let _ = writeln!(self.out, "budget: {}", budget.label());
                }
                // T14: cumulative tokens for the run, so a wrapped run's cost
                // is visible without mining `.chug/events.jsonl`.
                if let Some(line) = self.tokens_line() {
                    let _ = writeln!(self.out, "{line}");
                }
                let _ = writeln!(
                    self.out,
                    "resume: chug run --spec <spec> --goal \"<goal>\" --cwd {} --resume [--model <other>]  (current model: {model})",
                    self.cwd.display()
                );
            }
            Event::Usage { input, output } => {
                // Cumulative run totals: latest wins. Printed at the
                // goal-complete/abort boundaries, not per event.
                self.last_usage = Some((input, output));
            }
            // T17: telemetry only — the notice already reached the user as a
            // transcript message; no console output.
            Event::BudgetLow { .. } => {}
            // T38: telemetry only — the advisory already reached the model as
            // a transcript message; no console output.
            Event::OutputTruncated => {}
            Event::SteeringQueued(_) => {}
            Event::RiskVerdict {
                blocked,
                choice,
                p,
                ..
            } => {
                let suffix = if blocked { " BLOCKED" } else { "" };
                let _ = writeln!(
                    self.err,
                    "[chug] risk gate: {choice} (p={p:.2}){suffix}"
                );
            }
            Event::RiskGateDisabled => {
                let _ = writeln!(self.err, "[chug] risk gate disabled by operator note");
            }
            // No headless chat: turn-boundary events are TUI-only.
            Event::TurnStart { .. } | Event::TurnEnd { .. } => {}
        }
    }
}

/// Truncate to `max_chars` with an ellipsis suffix (shared with the driver so
/// console bytes stay identical to the pre-TUI `preview()` helper).
pub fn preview(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        let head: String = s.chars().take(max_chars).collect();
        format!("{head}...")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type Shared = std::sync::Arc<std::sync::Mutex<Vec<u8>>>;

    fn shared() -> Shared {
        std::sync::Arc::new(std::sync::Mutex::new(Vec::new()))
    }

    fn sink(cwd: &str) -> (ConsoleSink, Shared, Shared) {
        let out = shared();
        let err = shared();
        let s = ConsoleSink::with_writers(
            Box::new(SharedWriter(out.clone())),
            Box::new(SharedWriter(err.clone())),
            PathBuf::from(cwd),
        );
        (s, out, err)
    }

    struct SharedWriter(Shared);
    impl Write for SharedWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn out_bytes(shared: &Shared) -> String {
        String::from_utf8(shared.lock().unwrap().clone()).unwrap()
    }

    #[test]
    fn console_sink_golden_matches_pre_tui_format() {
        let (mut sink, out, err) = sink("/work/dir");

        sink.emit(Event::Iteration {
            n: 1,
            max: 40,
            messages: 3,
        });
        sink.emit(Event::ModelText("Now I'll write the parser.".into()));
        sink.emit(Event::ToolStart {
            name: "bash".into(),
        });
        sink.emit(Event::ToolResult {
            name: "bash".into(),
            ok: false,
            duration_ms: 3,
            preview: "boom".into(),
        });
        sink.emit(Event::ToolStart {
            name: "write_file".into(),
        });
        sink.emit(Event::ToolResult {
            name: "write_file".into(),
            ok: true,
            duration_ms: 1,
            preview: "wrote 5 bytes".into(),
        });
        sink.emit(Event::LedgerChanged("# Ledger\n".into()));
        sink.emit(Event::GoalAccepted {
            summary: "did it".into(),
        });

        assert_eq!(
            out_bytes(&err),
            "[chug] iteration 1 / 40 (3 messages)\n\
             [chug] model: Now I'll write the parser.\n\
             [chug] tool bash -> error\n\
             [chug] tool write_file -> ok\n"
        );
        assert_eq!(
            out_bytes(&out),
            "chug: goal complete\n\
             summary: did it\n\
             \n--- LEDGER.md ---\n\
             # Ledger\n\n"
        );
    }

    /// T12: a budget death names the model, the exhausted budget, and the
    /// resume-with-fallback hint (with the current model named).
    #[test]
    fn console_sink_budget_abort_names_model_budget_and_fallback_hint() {
        let (mut sink, out, err) = sink("/work/dir");
        sink.emit(Event::LedgerChanged("# Ledger\n\n## Next\n- x\n".into()));
        sink.emit(Event::Aborted {
            reason: "iteration budget exceeded".into(),
            model: "muse-glimmer-30b".into(),
            budget: Some(BudgetExceeded::Iterations { max: 40 }),
        });

        assert_eq!(out_bytes(&err), "chug: abort: iteration budget exceeded\n");
        assert_eq!(
            out_bytes(&out),
            "--- LEDGER.md ---\n\
             # Ledger\n\n## Next\n- x\n\n\
             ---\n\
             model: muse-glimmer-30b\n\
             budget: 40 iterations\n\
             resume: chug run --spec <spec> --goal \"<goal>\" --cwd /work/dir --resume [--model <other>]  (current model: muse-glimmer-30b)\n"
        );
    }

    /// T12: non-budget aborts print the ledger exactly as before, share the
    /// model line, and skip only the budget line.
    #[test]
    fn console_sink_non_budget_abort_has_model_but_no_budget_line() {
        let (mut sink, out, err) = sink("/work/dir");
        sink.emit(Event::LedgerChanged("# Ledger\n\n## Next\n- x\n".into()));
        sink.emit(Event::Aborted {
            reason: "stuck: repeated error".into(),
            model: "claude-sonnet-4-6".into(),
            budget: None,
        });

        assert_eq!(out_bytes(&err), "chug: abort: stuck: repeated error\n");
        assert_eq!(
            out_bytes(&out),
            "--- LEDGER.md ---\n\
             # Ledger\n\n## Next\n- x\n\n\
             ---\n\
             model: claude-sonnet-4-6\n\
             resume: chug run --spec <spec> --goal \"<goal>\" --cwd /work/dir --resume [--model <other>]  (current model: claude-sonnet-4-6)\n"
        );
        assert!(!out_bytes(&out).contains("budget:"));
    }

    /// T14: goal-complete output names the cumulative token totals.
    #[test]
    fn console_sink_goal_accepted_prints_cumulative_tokens() {
        let (mut sink, out, _err) = sink("/work/dir");
        sink.emit(Event::Usage {
            input: 8_683_323,
            output: 1_243_749,
        });
        sink.emit(Event::GoalAccepted {
            summary: "did it".into(),
        });

        let stdout = out_bytes(&out);
        assert!(stdout.contains("tokens: 8683323 in / 1243749 out (cumulative)"));
        // Spec: the tokens line goes after the summary line.
        let summary = stdout.find("summary: did it").unwrap();
        let tokens = stdout.find("tokens: 8683323").unwrap();
        let ledger = stdout.find("--- LEDGER.md ---").unwrap();
        assert!(summary < tokens && tokens < ledger);
    }

    /// T14: `Usage` values are cumulative, so the last one seen is what the
    /// abort block prints.
    #[test]
    fn console_sink_abort_prints_latest_usage_totals() {
        let (mut sink, out, _err) = sink("/work/dir");
        sink.emit(Event::Usage {
            input: 1_000,
            output: 100,
        });
        sink.emit(Event::Usage {
            input: 8_683_323,
            output: 1_243_749,
        });
        sink.emit(Event::Aborted {
            reason: "iteration budget exceeded".into(),
            model: "muse-glimmer-30b".into(),
            budget: Some(BudgetExceeded::Iterations { max: 40 }),
        });

        let stdout = out_bytes(&out);
        assert!(stdout.contains("tokens: 8683323 in / 1243749 out (cumulative)"));
        assert!(!stdout.contains("tokens: 1000"));
        // Sits alongside the model/budget lines, before the resume hint.
        let budget = stdout.find("budget: 40 iterations").unwrap();
        let tokens = stdout.find("tokens: 8683323").unwrap();
        let resume = stdout.find("resume:").unwrap();
        assert!(budget < tokens && tokens < resume);
    }

    /// T14: an abort before the first API response prints no tokens line.
    #[test]
    fn console_sink_abort_without_usage_has_no_tokens_line() {
        let (mut sink, out, _err) = sink("/work/dir");
        sink.emit(Event::Aborted {
            reason: "interrupted".into(),
            model: "muse-glimmer-30b".into(),
            budget: None,
        });

        let stdout = out_bytes(&out);
        assert!(!stdout.contains("tokens:"));
        assert!(stdout.contains("model: muse-glimmer-30b"));
    }

    #[test]
    fn budget_exceeded_labels() {
        assert_eq!(
            BudgetExceeded::Iterations { max: 40 }.label(),
            "40 iterations"
        );
        assert_eq!(BudgetExceeded::Minutes { max: 120 }.label(), "120 minutes");
        // T15: the token budget names its ceiling the same way.
        assert_eq!(BudgetExceeded::Tokens { max: 50_000 }.label(), "50000 tokens");
    }

    #[test]
    fn console_sink_model_text_truncates_at_200_chars() {
        let (mut sink, _out, err) = sink("/w");
        let long = "x".repeat(300);
        sink.emit(Event::ModelText(long));
        let line = out_bytes(&err);
        assert!(line.starts_with("[chug] model: "));
        assert_eq!(line, format!("[chug] model: {}...\n", "x".repeat(200)));
    }

    #[test]
    fn console_sink_silent_events() {
        let (mut sink, out, err) = sink("/w");
        sink.emit(Event::ToolStart { name: "bash".into() });
        sink.emit(Event::Verifying {
            cmd: "cargo test".into(),
        });
        sink.emit(Event::Usage {
            input: 10,
            output: 20,
        });
        sink.emit(Event::SteeringQueued("note".into()));
        // T38: the truncation advisory is telemetry-only here too.
        sink.emit(Event::OutputTruncated);
        assert_eq!(out_bytes(&err), "");
        assert_eq!(out_bytes(&out), "");
    }

    #[test]
    fn console_sink_risk_gate_lines() {
        let (mut sink, out, err) = sink("/w");
        sink.emit(Event::RiskVerdict {
            blocked: true,
            choice: "destructive".into(),
            p: 0.7,
            preview: "rm -rf x".into(),
        });
        sink.emit(Event::RiskVerdict {
            blocked: false,
            choice: "risky".into(),
            p: 0.3,
            preview: "cargo clean".into(),
        });
        sink.emit(Event::RiskGateDisabled);
        assert_eq!(
            out_bytes(&err),
            "[chug] risk gate: destructive (p=0.70) BLOCKED\n\
             [chug] risk gate: risky (p=0.30)\n\
             [chug] risk gate disabled by operator note\n"
        );
        assert_eq!(out_bytes(&out), "");
    }

    #[test]
    fn console_sink_turn_events_are_silent() {
        let (mut sink, out, err) = sink("/w");
        sink.emit(Event::TurnStart {
            objective: "do the thing".into(),
        });
        sink.emit(Event::TurnEnd {
            reason: TurnEndReason::Completed,
        });
        assert_eq!(out_bytes(&err), "");
        assert_eq!(out_bytes(&out), "");
    }

    #[test]
    fn turn_end_reason_labels() {
        assert_eq!(TurnEndReason::Completed.label(), "completed");
        assert_eq!(TurnEndReason::GoalAccepted.label(), "goal accepted");
        assert_eq!(TurnEndReason::Interrupted.label(), "interrupted");
        assert_eq!(TurnEndReason::BudgetExceeded.label(), "budget exceeded");
    }

    #[test]
    fn preview_truncates_on_char_boundary() {
        assert_eq!(preview("short", 10), "short");
        let s: String = "é".repeat(300);
        let p = preview(&s, 200);
        assert_eq!(p.chars().count(), 203);
        assert!(p.ends_with("..."));
    }
}
