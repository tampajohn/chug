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
    },
    Usage {
        input: u64,
        output: u64,
    },
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
}

impl ConsoleSink {
    pub fn new(cwd: PathBuf) -> Self {
        ConsoleSink {
            out: Box::new(std::io::stdout()),
            err: Box::new(std::io::stderr()),
            cwd,
            last_ledger: String::new(),
        }
    }

    #[cfg(test)]
    fn with_writers(out: Box<dyn Write>, err: Box<dyn Write>, cwd: PathBuf) -> Self {
        ConsoleSink {
            out,
            err,
            cwd,
            last_ledger: String::new(),
        }
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
                let _ = writeln!(self.out, "\n--- LEDGER.md ---");
                let _ = writeln!(self.out, "{}", self.last_ledger);
            }
            Event::Aborted { reason } => {
                let _ = writeln!(self.err, "chug: abort: {reason}");
                let _ = writeln!(self.out, "--- LEDGER.md ---");
                let _ = writeln!(self.out, "{}", self.last_ledger);
                let _ = writeln!(self.out, "---");
                let _ = writeln!(
                    self.out,
                    "resume with: chug run --spec <spec> --goal \"<goal>\" --cwd {} --resume",
                    self.cwd.display()
                );
            }
            Event::Usage { .. } => {}
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

    #[test]
    fn console_sink_abort_block_matches_pre_tui_format() {
        let (mut sink, out, err) = sink("/work/dir");
        sink.emit(Event::LedgerChanged("# Ledger\n\n## Next\n- x\n".into()));
        sink.emit(Event::Aborted {
            reason: "iteration budget exceeded".into(),
        });

        assert_eq!(out_bytes(&err), "chug: abort: iteration budget exceeded\n");
        assert_eq!(
            out_bytes(&out),
            "--- LEDGER.md ---\n\
             # Ledger\n\n## Next\n- x\n\n\
             ---\n\
             resume with: chug run --spec <spec> --goal \"<goal>\" --cwd /work/dir --resume\n"
        );
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
