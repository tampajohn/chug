//! Interactive chat mode (`chug chat`): the user drives turn by turn.
//!
//! The UI thread parses input; this module owns the worker-side session:
//! the Idle → Working → Idle state machine, the slash-command parser, and
//! the turn lifecycle (objective in, `driver::run_turn`, turn-end out).

use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::mpsc::Receiver;
use std::time::Duration;

use crate::api::{Client, ContentBlock, Llm, Message};
use crate::attach;
use crate::driver::{self, Controls, SlashUpdate, TurnKnobs};
use crate::events::{Event, EventSink};
use crate::ledger;
use crate::observ;
use crate::riskgate::{LayaJudge, RiskGate};
use crate::transcript;

/// Chat state machine:
///
/// ```text
/// Idle ──submit request──▶ Working ──turn ends──▶ Idle
/// Working ──Esc/q (interrupt)──▶ (Interrupting) ──▶ Idle
/// Working ──budget exceeded──▶ Idle
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatState {
    /// Input dock focused; waiting for the next objective.
    Idle,
    /// A turn is running.
    Working,
    /// Abort flag set; the driver stops at the next iteration boundary.
    Interrupting,
}

impl ChatState {
    pub fn label(self) -> &'static str {
        match self {
            ChatState::Idle => "idle",
            ChatState::Working => "working",
            ChatState::Interrupting => "interrupting…",
        }
    }
}

/// A parsed slash command. Slash lines are never sent to the LLM; unknown
/// commands and bad arguments become activity-stream lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlashCommand {
    /// `/spec <path>` loads/replaces the spec; `/spec` alone clears it.
    Spec(Option<String>),
    /// `/goal <text>` sets a persistent goal; `/goal` alone clears it.
    Goal(Option<String>),
    /// `/check <cmd>` sets the goal_complete gate; `/check` alone clears it.
    Check(Option<String>),
    /// `/ledger` refocuses the ledger pane.
    Ledger,
    /// `/model <id>` switches model; `/model` alone shows the current one.
    Model(Option<String>),
    /// `/budget <iters> <minutes>` changes per-turn budgets; `/budget` alone
    /// shows the current budgets.
    Budget(Option<(u32, u64)>),
    /// `/quit` exits (same as `q` in Idle).
    Quit,
    /// `/help` lists the commands.
    Help,
    /// Unknown `/x`.
    Unknown(String),
    /// Known command with malformed arguments.
    Usage(&'static str),
}

/// Parse a line into a slash command. Returns `None` when the line is not a
/// slash command (after trimming leading whitespace). Never fails: unknown
/// commands yield [`SlashCommand::Unknown`], bad arguments
/// [`SlashCommand::Usage`].
pub fn parse_slash(line: &str) -> Option<SlashCommand> {
    let rest = line.trim_start().strip_prefix('/')?;
    let mut parts = rest.splitn(2, char::is_whitespace);
    let name = parts.next().unwrap_or_default();
    let arg = parts
        .next()
        .map(str::trim)
        .filter(|a| !a.is_empty())
        .map(str::to_string);
    Some(match name {
        "spec" => SlashCommand::Spec(arg),
        "goal" => SlashCommand::Goal(arg),
        "check" => SlashCommand::Check(arg),
        "ledger" => SlashCommand::Ledger,
        "model" => SlashCommand::Model(arg),
        "budget" => match arg {
            None => SlashCommand::Budget(None),
            Some(text) => {
                let mut words = text.split_whitespace();
                let (Some(iters), Some(minutes), None) =
                    (words.next(), words.next(), words.next())
                else {
                    return Some(SlashCommand::Usage("/budget <iters> <minutes>"));
                };
                match (iters.parse::<u32>(), minutes.parse::<u64>()) {
                    (Ok(i), Ok(m)) if i >= 1 && m >= 1 => SlashCommand::Budget(Some((i, m))),
                    _ => SlashCommand::Usage("/budget <iters> <minutes>"),
                }
            }
        },
        "quit" => SlashCommand::Quit,
        "help" => SlashCommand::Help,
        other => SlashCommand::Unknown(other.to_string()),
    })
}

/// Worker-side session configuration for one `chug chat` process.
pub struct ChatConfig {
    pub cwd: PathBuf,
    pub model: String,
    pub max_iters: u32,
    pub max_minutes: u64,
    pub resume: bool,
    pub risk_gate: bool,
    /// Per-command wall-clock budget for the `bash` tool.
    pub bash_timeout: Duration,
    /// Abort flag + steering channel shared with the UI.
    pub controls: Controls,
    /// User objectives submitted while idle.
    pub objective_rx: Receiver<String>,
    /// Slash-command session updates (drained at every iteration boundary).
    pub update_rx: Receiver<SlashUpdate>,
}

/// Production entry point: build the API client + risk gate, then run the
/// chat session. Returns the process exit code.
pub fn run_chat(cfg: ChatConfig, sink: &mut dyn EventSink) -> anyhow::Result<i32> {
    let mut client = Client::new(&cfg.model)?;
    let gate = if cfg.risk_gate {
        Some(RiskGate::new(Box::new(LayaJudge::from_env()?), &cfg.cwd))
    } else {
        None
    };
    run_chat_with(cfg, &mut client, gate, sink, observ::global())
}

/// The chat session loop. Idle: block for the next objective. Working: run
/// one turn via the shared driver loop. The UI quitting (dropping its
/// senders) ends the session gracefully. Split from [`run_chat`] so tests
/// can inject a scripted LLM and no gate.
///
/// SPEC-8: one trace per chat SESSION (not per turn) — created here, finished
/// with the outcome metadata when the session ends gracefully. Individual
/// turns attach their generations / spans / events to the same trace.
fn run_chat_with(
    cfg: ChatConfig,
    client: &mut dyn Llm,
    mut gate: Option<RiskGate>,
    sink: &mut dyn EventSink,
    obs: &observ::Sink,
) -> anyhow::Result<i32> {
    ledger::ensure_seeded(&cfg.cwd)?;
    // No goal text at session start (objectives arrive turn by turn); the
    // trace is identified by its id, mode metadata, and tags.
    let trace = obs.trace_started("", &cfg.model, &cfg.cwd.display().to_string(), "chat", None);
    let mut turns: u64 = 0;
    let mut messages: Vec<Message> = if cfg.resume {
        driver::resume_messages(&cfg.cwd)?
    } else {
        Vec::new()
    };
    let mut knobs = TurnKnobs {
        spec_path: None,
        goal: None,
        check_cmd: None,
        max_iters: cfg.max_iters,
        max_minutes: cfg.max_minutes,
    };

    loop {
        // Idle: wait for the next objective. A closed channel means the UI
        // has quit — exit the session (and finish its trace) gracefully.
        let objective = match cfg.objective_rx.recv() {
            Ok(objective) => objective,
            Err(_) => {
                if let Some(trace) = &trace {
                    obs.trace_finished(trace, observ::outcome::COMPLETED, turns);
                }
                return Ok(0);
            }
        };
        // Clear any abort flag left over from keys pressed between turns so
        // the new turn starts fresh. A genuine interrupt arrives only while
        // Working, after this point.
        cfg.controls.abort.store(false, Ordering::SeqCst);

        // SPEC-5 §1: expand `@file` mentions before the message reaches the
        // driver. The model and the transcript (resume-safe history) get the
        // expanded message; the activity stream, via TurnStart, gets the
        // typed text with any `[file not found: ...]` notes inline — never
        // the expanded file contents.
        let expanded = attach::expand_message(&cfg.cwd, &objective);
        sink.emit(Event::TurnStart {
            objective: expanded.text_with_notes,
        });
        let msg = Message::user(vec![ContentBlock::text_block(expanded.llm_message)]);
        transcript::append(&cfg.cwd, &msg)?;
        messages.push(msg);

        let reason = driver::run_turn(
            &cfg.cwd,
            client,
            &mut gate,
            &mut messages,
            &cfg.controls,
            &cfg.update_rx,
            &mut knobs,
            cfg.bash_timeout,
            trace.as_deref(),
            obs,
            sink,
        )?;
        turns += 1;
        sink.emit(Event::TurnEnd { reason });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::ScriptedLlm;
    use crate::events::TurnEndReason;
    use serde_json::{Value, json};
    use std::sync::atomic::AtomicBool;
    use std::sync::{Arc, mpsc};

    // ---------- slash parser ----------

    #[test]
    fn parse_each_command() {
        assert_eq!(
            parse_slash("/spec SPEC.md"),
            Some(SlashCommand::Spec(Some("SPEC.md".into())))
        );
        assert_eq!(parse_slash("/spec"), Some(SlashCommand::Spec(None)));
        assert_eq!(
            parse_slash("/goal ship it"),
            Some(SlashCommand::Goal(Some("ship it".into())))
        );
        assert_eq!(parse_slash("/goal"), Some(SlashCommand::Goal(None)));
        assert_eq!(
            parse_slash("/check cargo test"),
            Some(SlashCommand::Check(Some("cargo test".into())))
        );
        assert_eq!(parse_slash("/check"), Some(SlashCommand::Check(None)));
        assert_eq!(parse_slash("/ledger"), Some(SlashCommand::Ledger));
        assert_eq!(
            parse_slash("/model opus-4"),
            Some(SlashCommand::Model(Some("opus-4".into())))
        );
        assert_eq!(parse_slash("/model"), Some(SlashCommand::Model(None)));
        assert_eq!(
            parse_slash("/budget 10 30"),
            Some(SlashCommand::Budget(Some((10, 30))))
        );
        assert_eq!(parse_slash("/budget"), Some(SlashCommand::Budget(None)));
        assert_eq!(parse_slash("/quit"), Some(SlashCommand::Quit));
        assert_eq!(parse_slash("/help"), Some(SlashCommand::Help));
    }

    #[test]
    fn parse_unknown_and_malformed() {
        assert_eq!(
            parse_slash("/xyzzy"),
            Some(SlashCommand::Unknown("xyzzy".into()))
        );
        assert_eq!(
            parse_slash("/budget 5"),
            Some(SlashCommand::Usage("/budget <iters> <minutes>"))
        );
        assert_eq!(
            parse_slash("/budget a b"),
            Some(SlashCommand::Usage("/budget <iters> <minutes>"))
        );
        assert_eq!(
            parse_slash("/budget 0 5"),
            Some(SlashCommand::Usage("/budget <iters> <minutes>"))
        );
        assert_eq!(
            parse_slash("/budget 5 10 extra"),
            Some(SlashCommand::Usage("/budget <iters> <minutes>"))
        );
        // Bare "/" is an (empty) unknown command, never an LLM round-trip.
        assert_eq!(parse_slash("/"), Some(SlashCommand::Unknown("".into())));
    }

    #[test]
    fn parse_handles_whitespace_and_non_slash_lines() {
        // Leading whitespace still parses.
        assert_eq!(
            parse_slash("   /goal  keep going "),
            Some(SlashCommand::Goal(Some("keep going".into())))
        );
        assert_eq!(parse_slash("\t/help"), Some(SlashCommand::Help));
        // Plain text is not a command.
        assert_eq!(parse_slash("fix the parser"), None);
        assert_eq!(parse_slash(""), None);
        // Multi-word arguments keep their inner spacing.
        assert_eq!(
            parse_slash("/check cargo test -- --nocapture"),
            Some(SlashCommand::Check(Some("cargo test -- --nocapture".into())))
        );
    }

    // ---------- chat session state machine (scripted LLM, no network) ----------

    #[derive(Default)]
    struct RecordingSink(Vec<Event>);
    impl EventSink for RecordingSink {
        fn emit(&mut self, e: Event) {
            self.0.push(e);
        }
    }

    struct Harness {
        cfg: ChatConfig,
        llm: ScriptedLlm,
        sink: RecordingSink,
        abort: Arc<AtomicBool>,
        objective_tx: mpsc::Sender<String>,
        update_tx: mpsc::Sender<SlashUpdate>,
    }

    fn harness(tmp: &tempfile::TempDir, responses: Vec<Value>) -> Harness {
        let (objective_tx, objective_rx) = mpsc::channel();
        let (update_tx, update_rx) = mpsc::channel();
        let (_steer_tx, steering_rx) = mpsc::channel();
        let abort = Arc::new(AtomicBool::new(false));
        let cfg = ChatConfig {
            cwd: tmp.path().to_path_buf(),
            model: "scripted-model".into(),
            max_iters: 40,
            max_minutes: 120,
            resume: false,
            risk_gate: false,
            bash_timeout: Duration::from_secs(crate::tools::BASH_TIMEOUT_SECS),
            controls: Controls {
                abort: Arc::clone(&abort),
                steering_rx,
            },
            objective_rx,
            update_rx,
        };
        Harness {
            cfg,
            llm: ScriptedLlm::new(responses),
            sink: RecordingSink::default(),
            abort,
            objective_tx,
            update_tx,
        }
    }

    fn text_response(text: &str) -> Value {
        json!({
            "stop_reason": "end_turn",
            "usage": {"input_tokens": 10, "output_tokens": 5},
            "content": [{"type": "text", "text": text}],
        })
    }

    fn tool_response(name: &str, input: Value) -> Value {
        json!({
            "stop_reason": "tool_use",
            "usage": {"input_tokens": 10, "output_tokens": 5},
            "content": [{"type": "tool_use", "id": "tu_1", "name": name, "input": input}],
        })
    }

    /// Run the session on a thread; send objectives; close; join.
    fn run_session(
        mut h: Harness,
        script: impl FnOnce(&mpsc::Sender<String>, &mpsc::Sender<SlashUpdate>) + Send + 'static,
    ) -> (i32, Vec<Event>, ScriptedLlm, PathBuf) {
        let cwd = h.cfg.cwd.clone();
        let worker = std::thread::spawn(move || {
            // Noop observability sink: tests that assert on Langfuse events
            // build their own live test sink (see the trace-lifecycle test).
            let code =
                run_chat_with(h.cfg, &mut h.llm, None, &mut h.sink, &crate::observ::Sink::Noop)
                    .expect("chat session failed");
            (code, h.sink.0, h.llm)
        });
        script(&h.objective_tx, &h.update_tx);
        drop(h.objective_tx);
        drop(h.update_tx);
        let (code, events, llm) = worker.join().expect("worker panicked");
        (code, events, llm, cwd)
    }

    fn turn_ends(events: &[Event]) -> Vec<TurnEndReason> {
        events
            .iter()
            .filter_map(|e| match e {
                Event::TurnEnd { reason } => Some(*reason),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn idle_working_idle_on_natural_stop() {
        let tmp = tempfile::tempdir().unwrap();
        let h = harness(&tmp, vec![text_response("all done")]);
        let (code, events, llm, cwd) = run_session(h, |objective_tx, _| {
            objective_tx.send("say hi".into()).unwrap();
        });
        assert_eq!(code, 0);
        // Turn started, then ended Completed — no anti-stall kick.
        assert!(matches!(
            events.iter().find(|e| matches!(e, Event::TurnStart { .. })),
            Some(Event::TurnStart { objective }) if objective == "say hi"
        ));
        assert_eq!(turn_ends(&events), vec![TurnEndReason::Completed]);
        // Exactly one LLM call; the objective is the final user message.
        assert_eq!(llm.calls.len(), 1);
        let messages = &llm.calls[0].1;
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[0].content[0].text(), Some("say hi"));
        // Transcript continuity on disk: user objective + assistant reply.
        let saved = transcript::load(&cwd).unwrap();
        assert_eq!(saved.len(), 2);
        assert_eq!(saved[1].role, "assistant");
    }

    #[test]
    fn goal_complete_ends_turn_unverified_without_check() {
        let tmp = tempfile::tempdir().unwrap();
        let h = harness(
            &tmp,
            vec![tool_response("goal_complete", json!({"summary": "did it"}))],
        );
        let (code, events, _, _) = run_session(h, |objective_tx, _| {
            objective_tx.send("finish it".into()).unwrap();
        });
        assert_eq!(code, 0);
        assert_eq!(turn_ends(&events), vec![TurnEndReason::GoalAccepted]);
        assert!(events.iter().any(|e| matches!(
            e,
            Event::GoalAccepted { summary } if summary == "did it"
        )));
        // No check configured: never a Verifying event.
        assert!(!events.iter().any(|e| matches!(e, Event::Verifying { .. })));
    }

    #[test]
    fn goal_complete_with_failing_check_rejects_then_natural_stop() {
        let tmp = tempfile::tempdir().unwrap();
        let h = harness(
            &tmp,
            vec![
                tool_response("goal_complete", json!({"summary": "premature"})),
                text_response("ok, continuing"),
            ],
        );
        let (code, events, _, _) = run_session(h, |objective_tx, update_tx| {
            update_tx
                .send(SlashUpdate::Check(Some("exit 1".into())))
                .unwrap();
            objective_tx.send("try to finish".into()).unwrap();
        });
        assert_eq!(code, 0);
        assert!(events.iter().any(|e| matches!(
            e,
            Event::Verifying { cmd } if cmd == "exit 1"
        )));
        assert!(events.iter().any(|e| matches!(e, Event::GoalRejected { .. })));
        assert!(!events.iter().any(|e| matches!(e, Event::GoalAccepted { .. })));
        // Turn continues after rejection, ends on the natural stop.
        assert_eq!(turn_ends(&events), vec![TurnEndReason::Completed]);
    }

    #[test]
    fn goal_complete_with_passing_check_is_accepted() {
        let tmp = tempfile::tempdir().unwrap();
        let h = harness(
            &tmp,
            vec![tool_response("goal_complete", json!({"summary": "verified"}))],
        );
        let (code, events, _, _) = run_session(h, |objective_tx, update_tx| {
            update_tx
                .send(SlashUpdate::Check(Some("true".into())))
                .unwrap();
            objective_tx.send("finish verified".into()).unwrap();
        });
        assert_eq!(code, 0);
        assert!(events.iter().any(|e| matches!(
            e,
            Event::GoalAccepted { summary } if summary == "verified"
        )));
        assert_eq!(turn_ends(&events), vec![TurnEndReason::GoalAccepted]);
    }

    #[test]
    fn esc_interrupt_aborts_turn_and_session_continues() {
        let tmp = tempfile::tempdir().unwrap();
        // One tool call so the loop reaches a second iteration boundary; the
        // scripted LLM raises the abort flag during the call (Esc mid-turn).
        let mut h = harness(
            &tmp,
            vec![tool_response("read_file", json!({"path": "missing.txt"}))],
        );
        h.llm.abort_on_call = Some(Arc::clone(&h.abort));
        let (code, events, llm, cwd) = run_session(h, |objective_tx, _| {
            objective_tx.send("do work".into()).unwrap();
        });
        assert_eq!(code, 0);
        assert!(events.iter().any(|e| matches!(
            e,
            Event::Aborted { reason } if reason == "operator interrupt"
        )));
        assert_eq!(turn_ends(&events), vec![TurnEndReason::Interrupted]);
        // The interrupt hit at the boundary after exactly one LLM call, and
        // the session stayed alive (it exited via closed channel, code 0).
        assert_eq!(llm.calls.len(), 1);
        // The tool result of the interrupted turn is the last transcript entry.
        let saved = transcript::load(&cwd).unwrap();
        assert_eq!(saved.last().map(|m| m.role.as_str()), Some("user"));
    }

    #[test]
    fn per_turn_budget_ends_turn_app_continues() {
        let tmp = tempfile::tempdir().unwrap();
        let mut h = harness(&tmp, vec![]);
        h.cfg.max_iters = 0; // budget exhausted immediately
        let (code, events, _, _) = run_session(h, |objective_tx, _| {
            objective_tx.send("first".into()).unwrap();
        });
        assert_eq!(code, 0);
        assert!(events.iter().any(|e| matches!(
            e,
            Event::Aborted { reason } if reason == "iteration budget exceeded"
        )));
        assert_eq!(turn_ends(&events), vec![TurnEndReason::BudgetExceeded]);
    }

    #[test]
    fn second_request_after_completed_turn_continues_transcript() {
        let tmp = tempfile::tempdir().unwrap();
        let h = harness(
            &tmp,
            vec![text_response("reply one"), text_response("reply two")],
        );
        let (code, events, llm, cwd) = run_session(h, |objective_tx, _| {
            objective_tx.send("first request".into()).unwrap();
            // Wait for the first turn to drain its scripted response before
            // sending the second objective (turns are sequential anyway).
            std::thread::sleep(std::time::Duration::from_millis(200));
            objective_tx.send("second request".into()).unwrap();
        });
        assert_eq!(code, 0);
        assert_eq!(
            turn_ends(&events),
            vec![TurnEndReason::Completed, TurnEndReason::Completed]
        );
        // Second call saw the full conversation: objective 1, reply 1, objective 2.
        assert_eq!(llm.calls.len(), 2);
        let seen = &llm.calls[1].1;
        assert_eq!(seen.len(), 3);
        assert_eq!(seen[0].content[0].text(), Some("first request"));
        assert_eq!(seen[1].content[0].text(), Some("reply one"));
        assert_eq!(seen[2].content[0].text(), Some("second request"));
        // And the transcript on disk matches.
        assert_eq!(transcript::load(&cwd).unwrap().len(), 4);
    }

    #[test]
    fn resume_loads_existing_transcript() {
        let tmp = tempfile::tempdir().unwrap();
        transcript::append(
            tmp.path(),
            &Message::user(vec![ContentBlock::text_block("earlier objective")]),
        )
        .unwrap();
        transcript::append(
            tmp.path(),
            &Message::assistant(vec![ContentBlock::text_block("earlier reply")]),
        )
        .unwrap();

        let mut h = harness(&tmp, vec![text_response("next reply")]);
        h.cfg.resume = true;
        let (code, _, llm, _) = run_session(h, |objective_tx, _| {
            objective_tx.send("next objective".into()).unwrap();
        });
        assert_eq!(code, 0);
        let seen = &llm.calls[0].1;
        assert_eq!(seen.len(), 3);
        assert_eq!(seen[0].content[0].text(), Some("earlier objective"));
        assert_eq!(seen[2].content[0].text(), Some("next objective"));
    }

    #[test]
    fn slash_updates_apply_at_turn_boundary() {
        let tmp = tempfile::tempdir().unwrap();
        let spec = tmp.path().join("CHATSPEC.md");
        std::fs::write(&spec, "chat spec body").unwrap();
        let h = harness(&tmp, vec![text_response("done")]);
        let (code, _, llm, _) = run_session(h, move |objective_tx, update_tx| {
            update_tx
                .send(SlashUpdate::Spec(Some(spec.clone())))
                .unwrap();
            update_tx
                .send(SlashUpdate::Goal(Some("persistent goal".into())))
                .unwrap();
            update_tx
                .send(SlashUpdate::Model("new-model".into()))
                .unwrap();
            update_tx
                .send(SlashUpdate::Budget {
                    iters: 7,
                    minutes: 9,
                })
                .unwrap();
            objective_tx.send("work".into()).unwrap();
        });
        assert_eq!(code, 0);
        let (system, _) = &llm.calls[0];
        assert!(system.contains("## Spec\n\nchat spec body"));
        assert!(system.contains("## Goal\n\npersistent goal"));
        assert!(system.contains("interactive session"));
        assert_eq!(llm.model, "new-model");
    }

    #[test]
    fn closed_channel_exits_gracefully_without_objectives() {
        let tmp = tempfile::tempdir().unwrap();
        let h = harness(&tmp, vec![]);
        let (code, events, llm, _) = run_session(h, |_, _| {});
        assert_eq!(code, 0);
        assert!(events.is_empty());
        assert!(llm.calls.is_empty());
    }

    // ---------- session trace lifecycle (SPEC-8) ----------

    #[test]
    fn one_trace_per_session_finished_on_quit_not_per_turn() {
        let tmp = tempfile::tempdir().unwrap();
        let (objective_tx, objective_rx) = mpsc::channel();
        let (update_tx, update_rx) = mpsc::channel();
        let (_steer_tx, steering_rx) = mpsc::channel();
        let cfg = ChatConfig {
            cwd: tmp.path().to_path_buf(),
            model: "scripted-model".into(),
            max_iters: 40,
            max_minutes: 120,
            resume: false,
            risk_gate: false,
            bash_timeout: Duration::from_secs(crate::tools::BASH_TIMEOUT_SECS),
            controls: Controls {
                abort: Arc::new(AtomicBool::new(false)),
                steering_rx,
            },
            objective_rx,
            update_rx,
        };
        let mut llm = ScriptedLlm::new(vec![text_response("one"), text_response("two")]);
        let transport = crate::observ::testing::CountingTransport::new();
        let obs = crate::observ::testing::test_sink(transport.clone());
        let mut sink = RecordingSink::default();
        let worker = std::thread::spawn(move || {
            run_chat_with(cfg, &mut llm, None, &mut sink, &obs).expect("chat session failed")
        });
        objective_tx.send("first".into()).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(200));
        objective_tx.send("second".into()).unwrap();
        drop(objective_tx);
        drop(update_tx);
        assert_eq!(worker.join().unwrap(), 0);
        // The sink moved into the worker thread; its Drop drains on join.

        let events = transport.events();
        // Exactly two trace-create upserts — session start + graceful finish —
        // sharing one id. NOT one trace per turn.
        let trace_creates: Vec<&Value> = events
            .iter()
            .filter(|e| e["type"] == "trace-create")
            .collect();
        assert_eq!(trace_creates.len(), 2);
        assert_eq!(trace_creates[0]["body"]["id"], trace_creates[1]["body"]["id"]);
        assert_eq!(trace_creates[0]["body"]["metadata"]["mode"], "chat");
        assert_eq!(trace_creates[1]["body"]["metadata"]["outcome"], "completed");
        assert_eq!(trace_creates[1]["body"]["metadata"]["iterations"], 2);
        // A session is not a run: no categorical outcome score is emitted.
        assert!(events.iter().all(|e| e["type"] != "score-create"));
        // Everything ties back to the same trace id.
        let trace_id = trace_creates[0]["body"]["id"].as_str().unwrap();
        assert!(events
            .iter()
            .all(|e| e["type"] == "trace-create" || e["body"]["traceId"] == trace_id));
    }

    // ---------- @file expansion on the submit path (SPEC-5 §1) ----------

    #[test]
    fn objective_with_file_mention_expands_for_model_and_transcript() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("a.txt"), "hello\nworld\n").unwrap();
        let h = harness(&tmp, vec![text_response("done")]);
        let (code, events, llm, cwd) = run_session(h, |objective_tx, _| {
            objective_tx.send("look at @a.txt please".into()).unwrap();
        });
        assert_eq!(code, 0);
        assert_eq!(turn_ends(&events), vec![TurnEndReason::Completed]);

        // The model-bound message carries the typed text plus the file block.
        assert_eq!(llm.calls.len(), 1);
        let sent = llm.calls[0].1[0].content[0].text().unwrap();
        assert!(
            sent.starts_with(
                "look at @a.txt please\n\n<file path=\"a.txt\">\nhello\nworld\n</file>"
            ),
            "{sent}"
        );

        // The activity stream (TurnStart) shows the typed text only — never
        // the expanded contents.
        assert!(matches!(
            events.iter().find(|e| matches!(e, Event::TurnStart { .. })),
            Some(Event::TurnStart { objective }) if objective == "look at @a.txt please"
        ));
        assert!(!events.iter().any(
            |e| matches!(e, Event::TurnStart { objective } if objective.contains("hello\nworld"))
        ));

        // The transcript stores the EXPANDED message (resume-safe history).
        let saved = transcript::load(&cwd).unwrap();
        let stored = saved[0].content[0].text().unwrap();
        assert!(stored.contains("<file path=\"a.txt\">\nhello\nworld\n</file>"));
    }

    #[test]
    fn objective_with_missing_file_gets_inline_note_not_error() {
        let tmp = tempfile::tempdir().unwrap();
        let h = harness(&tmp, vec![text_response("ok")]);
        let (code, events, llm, _) = run_session(h, |objective_tx, _| {
            objective_tx.send("read @nope.txt please".into()).unwrap();
        });
        assert_eq!(code, 0);
        // No error, no extra round-trip: exactly one LLM call, natural stop.
        assert_eq!(llm.calls.len(), 1);
        assert_eq!(turn_ends(&events), vec![TurnEndReason::Completed]);
        // The note is inline in both the model-bound message and TurnStart.
        let sent = llm.calls[0].1[0].content[0].text().unwrap();
        assert_eq!(sent, "read [file not found: nope.txt] please");
        assert!(matches!(
            events.iter().find(|e| matches!(e, Event::TurnStart { .. })),
            Some(Event::TurnStart { objective })
                if objective == "read [file not found: nope.txt] please"
        ));
    }

    #[test]
    fn objective_with_multiple_mentions_expands_in_order_and_escape_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("a.txt"), "AAA").unwrap();
        std::fs::write(tmp.path().join("b.txt"), "BBB").unwrap();
        let h = harness(&tmp, vec![text_response("done")]);
        let (code, _, llm, _) = run_session(h, |objective_tx, _| {
            objective_tx
                .send("compare @a.txt and @b.txt, then @../escape.txt".into())
                .unwrap();
        });
        assert_eq!(code, 0);
        let sent = llm.calls[0].1[0].content[0].text().unwrap();
        assert_eq!(
            sent,
            "compare @a.txt and @b.txt, then [file not found: ../escape.txt]\
             \n\n<file path=\"a.txt\">\nAAA\n</file>\
             \n\n<file path=\"b.txt\">\nBBB\n</file>"
        );
    }
}
