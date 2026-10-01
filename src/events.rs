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
        /// T184: cumulative cache-read input tokens (the API layer already
        /// parses `cache_read_input_tokens`); 0 until a response reports one.
        /// Rides the serialized iteration line; the console/TUI token lines
        /// stay input/output only.
        cache_read: u64,
        /// T184: cumulative cache-creation input tokens (the API layer
        /// already parses `cache_creation_input_tokens`); 0 until a response
        /// reports one. Same ride-along as `cache_read`.
        cache_creation: u64,
    },
    /// T184 telemetry: one transcript-trim pass (T77) collapsed frozen 16k
    /// segments. The advisory lives in the transcript as `[trimmed: …]`
    /// markers; this event exists only so `.chug/events.jsonl` records that
    /// the pass fired — estimated tokens before/after, how many segments
    /// THIS pass collapsed (frozen markers from earlier passes don't count),
    /// and the total marker count after. Console/TUI stay silent
    /// (BudgetLow/OutputTruncated precedent: telemetry-only).
    Trim {
        /// Estimated tokens in the transcript before the pass.
        before_tokens: u64,
        /// Estimated tokens after the pass (strictly less than before).
        after_tokens: u64,
        /// Segments collapsed in this pass (= the marker-count delta).
        segments_collapsed: u32,
        /// Total `[trimmed: …]` markers in the transcript after the pass.
        marker_count: u32,
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
    /// T91 telemetry: the endpoint rejected a request carrying image blocks
    /// (400 + image/content in the body), so the driver retried once with
    /// every image replaced by a placeholder text block and latched for the
    /// rest of the run — later image results are downgraded at wrap time and
    /// never sent. The placeholder text already reached the model; this event
    /// records that the run is image-free from here on.
    ImageDegraded,
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
    /// T83 telemetry: one hook execution fired (PreToolUse veto-gate /
    /// PostToolUse advisory). `exit` is `None` when the hook died without
    /// an exit code; a timed-out hook emits a HookError line instead of a
    /// fire line.
    HookFired {
        /// `PreToolUse` or `PostToolUse`.
        event: String,
        tool: String,
        command: String,
        exit: Option<i32>,
        veto: bool,
        duration_ms: u64,
    },
    /// T83: a hooks problem (config load, spawn failure, timeout). Hooks
    /// fail open, so this is telemetry only: the run continues with the
    /// call allowed (a config error continues with zero hooks).
    HookError {
        detail: String,
    },
    /// T90 telemetry: one permission deny — a `.chug/permissions.json` deny
    /// rule matched and the call was refused before execution (fail-closed:
    /// the tool never ran; the model received a `[permission denied]` tool
    /// error).
    PermissionDenied {
        tool: String,
        /// The matched rule, in its config shape (e.g.
        /// `deny bash command "*rm -rf*"`).
        rule: String,
    },
    /// T90: a permissions config problem (unreadable/malformed config, or a
    /// malformed rule skipped while valid siblings load). Permissions fail
    /// open on config problems, so this is telemetry only: the run
    /// continues with zero (or fewer) rules.
    PermissionError {
        detail: String,
    },
    /// Chat mode: the user submitted a new objective and a turn is starting.
    TurnStart {
        objective: String,
    },
    /// Chat mode: the active turn ended; the app returns to idle.
    TurnEnd {
        reason: TurnEndReason,
    },
    /// F7 phase 1: one incremental piece of model text, emitted by the driver's
    /// text-delta hook while the streamed response is still arriving. Console
    /// cosmetics ONLY — the sink renders deltas live (headless `chug run` /
    /// `delegate` logs gain liveness during minutes-long generations); the
    /// events.jsonl log stays silent (ModelText precedent: model text never
    /// enters the event log), and the transcript only ever sees the final
    /// accumulated Response. Emitted between `ToolStart` boundaries; the
    /// completing `Event::ModelText` still carries the full text (a streamed
    /// response suppresses the console preview so text never double-prints).
    ModelTextDelta(String),
    /// F7 phase 1 telemetry: a streaming request was answered with a plain
    /// JSON body (content-type not `text/event-stream` — typically a proxy
    /// downgrade), so the driver parsed it exactly as a non-streaming
    /// response. First occurrence per session latched (bounded telemetry for
    /// the tools-proxy compatibility question; no per-response spam).
    StreamFallback,
    /// T188: an auto-spec status line from the chat worker / headless draft
    /// phase (drafting, draft written, gate refusal, approval) — harness
    /// text, never model output (model output rides `ModelText`).
    AutoSpecNote(String),
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
    /// F7 phase 1: live model-text streaming is mid-line — at least one
    /// `ModelTextDelta` was printed (after the one-time `[chug] model: `
    /// prefix) and no response boundary has closed the line yet.
    stream_open: bool,
}

impl ConsoleSink {
    pub fn new(cwd: PathBuf) -> Self {
        ConsoleSink {
            out: Box::new(std::io::stdout()),
            err: Box::new(std::io::stderr()),
            cwd,
            last_ledger: String::new(),
            last_usage: None,
            stream_open: false,
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
            stream_open: false,
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
            Event::ModelTextDelta(delta) => {
                // F7 phase 1: raw incremental writes — the model's text as it
                // arrives, under a one-time prefix. No preview/truncation:
                // these are the live bytes of the generation.
                if !self.stream_open {
                    let _ = write!(self.err, "[chug] model: ");
                    self.stream_open = true;
                }
                let _ = write!(self.err, "{delta}");
            }
            Event::ModelText(text) => {
                if self.stream_open {
                    // F7 phase 1: a streamed response already printed its text
                    // live as deltas. Terminate the line and SUPPRESS the
                    // preview so the text never double-prints.
                    self.stream_open = false;
                    if !text.is_empty() {
                        let _ = writeln!(self.err);
                    }
                    return;
                }
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
            // F7 phase 1: telemetry only — the response was parsed exactly as
            // a non-streaming one; events.jsonl carries the single latched
            // note, the console stays silent.
            Event::StreamFallback => {}
            Event::Aborted {
                reason,
                model,
                budget,
            } => {
                // F7 phase 1: if a streamed response was still open (the loop
                // died mid-generation), terminate the line so abort output
                // never runs into partial model text.
                if self.stream_open {
                    let _ = writeln!(self.err);
                    self.stream_open = false;
                }
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
            Event::Usage {
                input,
                output,
                ..
            } => {
                // Cumulative run totals: latest wins. Printed at the
                // goal-complete/abort boundaries, not per event. The cache
                // counters ride the events log (T184); the console tokens
                // line stays input/output only.
                self.last_usage = Some((input, output));
            }
            // T184: telemetry only — the collapse is already recorded in the
            // transcript as `[trimmed: …]` markers; no console output.
            Event::Trim { .. } => {}
            // T17: telemetry only — the notice already reached the user as a
            // transcript message; no console output.
            Event::BudgetLow { .. } => {}
            // T38: telemetry only — the advisory already reached the model as
            // a transcript message; no console output.
            Event::OutputTruncated => {}
            // T91: telemetry only — the placeholder text already reached the
            // model inside the retried request; no console output.
            Event::ImageDegraded => {}
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
            // T83: telemetry only — hook vetoes/notes reach the model inside
            // the tool result; no console output.
            Event::HookFired { .. } | Event::HookError { .. } => {}
            // T90: telemetry only — the deny text reaches the model inside
            // the tool result; the config-problem warn already went to
            // stderr at load time. No console output.
            Event::PermissionDenied { .. } | Event::PermissionError { .. } => {}
            // No headless chat: turn-boundary events are TUI-only.
            Event::TurnStart { .. } | Event::TurnEnd { .. } => {}
            // T188: auto-spec status lines are for the operator — stderr,
            // matching the other [chug] console lines.
            Event::AutoSpecNote(note) => {
                let _ = writeln!(self.err, "[chug] auto-spec: {note}");
            }
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

    // ---- F7 phase 1: console streaming ----

    #[test]
    fn streamed_response_prints_deltas_live_and_suppresses_preview() {
        let (mut sink, _out, err) = sink("/work/dir");
        sink.emit(Event::ModelTextDelta("run".into()));
        sink.emit(Event::ModelTextDelta("ning now".into()));
        sink.emit(Event::ModelText("running now and much more than two hundred characters would be here to prove the preview truncation does not apply to the streamed leg which already printed everything".into()));
        // The live deltas are the text ("run" + "ning now"); the completing
        // ModelText (long enough to truncate a preview) only terminates the
        // line — its preview is suppressed.
        assert_eq!(
            out_bytes(&err),
            "[chug] model: running now\n",
            "one-time prefix + raw deltas + terminating newline; NO truncated preview"
        );
    }

    #[test]
    fn non_streamed_response_keeps_the_preview_line_byte_identically() {
        let (mut sink, _out, err) = sink("/work/dir");
        sink.emit(Event::ModelText("short answer".into()));
        assert_eq!(out_bytes(&err), "[chug] model: short answer\n");
    }

    /// F7 family sweep (kimi round): the streaming prefix /
    /// preview-suppression latch is PER-RESPONSE by design. It opens on the
    /// first delta of a response (prefix printed exactly ONCE no matter how
    /// many deltas follow), closes at the response boundary, and RE-ARMS: a
    /// second streamed response prints its own prefix, and a following
    /// NON-streamed response prints today's preview line. Kills the
    /// stick-across-responses mutant (the close on ModelText removed) and the
    /// re-fire-within-one-response mutant (the prefix printed per delta).
    #[test]
    fn streaming_prefix_latch_opens_once_per_response_and_rearms() {
        let (mut sink, _out, err) = sink("/work/dir");
        // Response 1: streamed — one prefix, raw deltas, terminating newline.
        sink.emit(Event::ModelTextDelta("one ".into()));
        sink.emit(Event::ModelTextDelta("two".into()));
        sink.emit(Event::ModelText("one two".into()));
        // Response 2: streamed again — the latch re-arms (a latch stuck
        // across responses would print neither the prefix nor the close).
        sink.emit(Event::ModelTextDelta("three".into()));
        sink.emit(Event::ModelText("three".into()));
        // Response 3: NON-streamed — today's preview line, byte-identical.
        sink.emit(Event::ModelText("preview line".into()));
        assert_eq!(
            out_bytes(&err),
            "[chug] model: one two\n[chug] model: three\n[chug] model: preview line\n",
            "prefix once per response, re-armed for the next, preview kept for non-streamed"
        );
    }

    #[test]
    fn empty_model_text_stays_silent_in_both_legs() {
        let (mut sink, _out, err) = sink("/work/dir");
        sink.emit(Event::ModelText("".into()));
        assert_eq!(out_bytes(&err), "");
        // A streamed response whose text is empty still terminates the line.
        sink.emit(Event::ModelTextDelta("x".into()));
        sink.emit(Event::ModelText("".into()));
        assert_eq!(out_bytes(&err), "[chug] model: x");
    }

    #[test]
    fn abort_terminates_an_open_stream_line() {
        let (mut sink, _out, err) = sink("/work/dir");
        sink.emit(Event::ModelTextDelta("partial".into()));
        sink.emit(Event::Aborted {
            reason: "budget".into(),
            model: "m".into(),
            budget: None,
        });
        assert_eq!(
            out_bytes(&err),
            "[chug] model: partial\nchug: abort: budget\n",
            "the abort output never runs into partial model text"
        );
    }

    #[test]
    fn stream_fallback_is_console_silent() {
        let (mut sink, out, err) = sink("/work/dir");
        sink.emit(Event::StreamFallback);
        assert_eq!(out_bytes(&out), "");
        assert_eq!(out_bytes(&err), "");
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
            cache_read: 0,
            cache_creation: 0,
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
            cache_read: 0,
            cache_creation: 0,
        });
        sink.emit(Event::Usage {
            input: 8_683_323,
            output: 1_243_749,
            cache_read: 0,
            cache_creation: 0,
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
            cache_read: 0,
            cache_creation: 0,
        });
        sink.emit(Event::SteeringQueued("note".into()));
        // T38: the truncation advisory is telemetry-only here too.
        sink.emit(Event::OutputTruncated);
        assert_eq!(out_bytes(&err), "");
        assert_eq!(out_bytes(&out), "");
    }

    /// T184: the Trim event is telemetry for `.chug/events.jsonl` only —
    /// the console (and TUI, pinned in tui.rs) stays silent, BudgetLow
    /// precedent.
    #[test]
    fn console_sink_trim_is_silent() {
        let (mut sink, out, err) = sink("/w");
        sink.emit(Event::Trim {
            before_tokens: 130_000,
            after_tokens: 88_000,
            segments_collapsed: 2,
            marker_count: 5,
        });
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
