//! Interactive chat mode (`chug chat`): the user drives turn by turn.
//!
//! The UI thread parses input; this module owns the worker-side session:
//! the Idle → Working → Idle state machine, the slash-command parser, and
//! the turn lifecycle (objective in, `driver::run_turn`, turn-end out).

use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, RecvTimeoutError, TryRecvError};
use std::time::Duration;

use crate::api::{Client, ContentBlock, Llm, Message};
use crate::attach;
use crate::autospec;
use crate::driver::{self, Controls, SlashUpdate, TurnKnobs};
use crate::eventlog;
use crate::events::{Event, EventSink};
use crate::ledger;
use crate::mcp::McpRegistry;
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
    /// T188: `/auto-spec <request>` — draft a spec for the request (worker
    /// calls the LLM once, tool-less), show it, park it pending approval.
    /// `/auto-spec` alone prints the usage line.
    AutoSpec(Option<String>),
    /// T188: `/auto-spec-approve` — gate the drafted spec (structure +
    /// non-vacuous check + dry-run; never loosened) and start the pending
    /// request as a turn with the draft loaded as the spec.
    AutoSpecApprove,
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
    /// Not a built-in: the chat dispatch first tries it as a
    /// `.chug/commands/` pack (F9) before rendering the unknown-command
    /// line. Carries the name and the post-command arguments separately
    /// so the pack's `$ARGUMENTS` expansion gets only the args.
    Unknown {
        name: String,
        args: Option<String>,
    },
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
        "auto-spec" => SlashCommand::AutoSpec(arg),
        "auto-spec-approve" => SlashCommand::AutoSpecApprove,
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
        other => SlashCommand::Unknown {
            name: other.to_string(),
            args: arg,
        },
    })
}

/// Worker-side session configuration for one `chug chat` process.
pub struct ChatConfig {
    pub cwd: PathBuf,
    pub model: String,
    pub max_iters: u32,
    pub max_minutes: u64,
    /// Per-turn token budget: cumulative input+output tokens. `0` = unlimited.
    pub max_tokens: u64,
    /// T143: the per-request output-token cap sent as `max_tokens` on every
    /// API call (default 32768; `--max-tokens-per-request`/`$CHUG_MAX_TOKENS`
    /// override). Distinct from the cumulative per-turn `max_tokens` above.
    pub max_tokens_per_request: u32,
    pub resume: bool,
    pub risk_gate: bool,
    /// Per-command wall-clock budget for the `bash` tool.
    pub bash_timeout: Duration,
    /// Path to an MCP config JSON. Overrides discovery. None → discovery.
    pub mcp_config: Option<PathBuf>,
    /// Force MCP servers off even when a config exists.
    pub mcp_off: bool,
    /// Abort flag + steering channel shared with the UI.
    pub controls: Controls,
    /// User objectives submitted while idle.
    pub objective_rx: Receiver<String>,
    /// Slash-command session updates (drained at every iteration boundary).
    pub update_rx: Receiver<SlashUpdate>,
    /// T188: auto-spec requests from the UI (its own channel — see
    /// [`AutoSpecRequest`]).
    pub autospec_rx: Receiver<AutoSpecRequest>,
}

/// T188: a chat-side auto-spec request from the UI. Rides its OWN channel
/// (`ChatConfig::autospec_rx`) — never the `SlashUpdate` channel — so the
/// worker can act on it while idle without consuming (and losing) regular
/// session updates, which stay buffered until the next turn boundary.
#[derive(Debug, Clone, PartialEq)]
pub enum AutoSpecRequest {
    /// `/auto-spec <request>`: draft a spec for the request (worker-side one
    /// LLM call, no tools), show it, park it pending approval.
    Draft(String),
    /// `/auto-spec-approve`: gate the (possibly operator-edited) draft at
    /// `.chug/auto-spec.md` — same validation + vacuous + dry-run rules, the
    /// check is never loosened — and start the pending request as a turn.
    Approve,
}

/// Production entry point: build the API client + risk gate + MCP registry,
/// then run the chat session. Returns the process exit code. The MCP registry
/// lives for the whole session (spawned at start, killed when the session
/// ends — including on error return).
pub fn run_chat(cfg: ChatConfig, sink: &mut dyn EventSink) -> anyhow::Result<i32> {
    let mut client = Client::new(&cfg.model, cfg.max_tokens_per_request)?;
    let gate = if cfg.risk_gate {
        Some(RiskGate::new(Box::new(LayaJudge::from_env()?), &cfg.cwd))
    } else {
        None
    };
    let mut mcp = McpRegistry::new(&cfg.cwd, cfg.mcp_off, cfg.mcp_config.clone())?;
    run_chat_with(cfg, &mut client, gate, &mut mcp, sink, observ::global())
}

/// The idle poll's tick: how often the idle session wakes to check for an
/// auto-spec request sent while idle (T188).
const IDLE_POLL_TICK: Duration = Duration::from_millis(100);

/// One decision of the idle poll ([`poll_step`]). The session loop in
/// [`run_chat_with`] executes the serving arms on [`PollStep::Serve`] —
/// byte-identical to the inline arms this seam replaced; the seam owns
/// only the POLL decisions.
#[derive(Debug, PartialEq)]
enum PollStep {
    /// A queued auto-spec request — serve it (T188: actionable exactly
    /// while idle), then poll again.
    Serve(AutoSpecRequest),
    /// An objective arrived — start the turn with it.
    Objective(String),
    /// The queue is empty and the UI has quit — the drain is complete;
    /// end the session gracefully.
    Drained,
    /// Nothing actionable this tick — poll again.
    Idle,
}

/// The idle poll's two inputs, abstracted so [`poll_step`]'s drain
/// decision is unit-testable (T188 round-4 pin): production wires the
/// session's channels ([`ChannelSources`]); the seam tests wire a
/// scripted source that controls WHEN a queued request becomes visible
/// relative to the observed disconnect — the en-route race that the
/// session-level tests hit only by wall-clock luck.
trait IdlePollSources {
    /// Non-blocking poll of the auto-spec request queue.
    fn try_recv_autospec(&mut self) -> Result<AutoSpecRequest, TryRecvError>;
    /// Bounded wait for the next objective.
    fn recv_objective(&mut self, tick: Duration) -> Result<String, RecvTimeoutError>;
}

/// The production wiring of [`IdlePollSources`]: the session's own
/// channels. Shared borrows — `Receiver::try_recv`/`recv_timeout` take
/// `&self` — so the poll never needs the config mutably.
struct ChannelSources<'a> {
    autospec_rx: &'a Receiver<AutoSpecRequest>,
    objective_rx: &'a Receiver<String>,
}

impl IdlePollSources for ChannelSources<'_> {
    fn try_recv_autospec(&mut self) -> Result<AutoSpecRequest, TryRecvError> {
        self.autospec_rx.try_recv()
    }

    fn recv_objective(&mut self, tick: Duration) -> Result<String, RecvTimeoutError> {
        self.objective_rx.recv_timeout(tick)
    }
}

/// The idle-poll decision, extracted as a seam (T188 round-4 pin): the
/// serve-arm ordering (auto-spec requests first), the disconnect latch,
/// the one-more-drain-pass, and exit-on-empty. `run_chat_with` executes
/// the serving arms on [`PollStep::Serve`] exactly as before; THIS
/// function owns the poll decisions, so the drain fix — a request queued
/// around the UI quit is still served before the exit — is pinned by unit
/// tests against a scripted source instead of the racy session-level
/// timing the suite relied on before (under the pre-fix behavior — return
/// immediately on disconnect — the en-route request is dropped, and the
/// deterministic drain test below fails where the session-level tests
/// raced and passed either way).
fn poll_step<S: IdlePollSources>(src: &mut S, ui_gone: &mut bool, tick: Duration) -> PollStep {
    // Serve auto-spec requests first (T188 — they are actionable exactly
    // while idle), then wait for the next objective.
    match src.try_recv_autospec() {
        Ok(request) => return PollStep::Serve(request),
        // Empty queue. If the UI already quit, the drain is complete:
        // nothing actionable remains — exit the session gracefully.
        Err(_) if *ui_gone => return PollStep::Drained,
        Err(_) => {}
    }
    match src.recv_objective(tick) {
        Ok(objective) => PollStep::Objective(objective),
        Err(RecvTimeoutError::Timeout) => PollStep::Idle,
        // The UI has quit. Latch (never spin) and take one more drain pass
        // through the serving arm above: a request queued before the quit
        // is still actionable (T188 — served exactly while idle) and must
        // never be dropped by the exit. The pass re-enters the try_recv
        // match; the `Err(_) if ui_gone` arm exits once the queue is empty.
        Err(RecvTimeoutError::Disconnected) => {
            *ui_gone = true;
            PollStep::Idle
        }
    }
}

/// The chat session loop. Idle: block for the next objective. Working: run
/// one turn via the shared driver loop. The UI quitting (dropping its
/// senders) ends the session gracefully. Split from [`run_chat`] so tests
/// can inject a scripted LLM, no gate, and an MCP registry.
///
/// SPEC-8: one trace per chat SESSION (not per turn) — created here, finished
/// with the outcome metadata when the session ends gracefully. Individual
/// turns attach their generations / spans / events to the same trace.
fn run_chat_with(
    cfg: ChatConfig,
    client: &mut dyn Llm,
    mut gate: Option<RiskGate>,
    mcp: &mut McpRegistry,
    sink: &mut dyn EventSink,
    obs: &observ::Sink,
) -> anyhow::Result<i32> {
    ledger::ensure_seeded(&cfg.cwd)?;
    // T11: the session's events log opens with the banner fields (mode
    // "chat"; a spec, if any, arrives later via /spec) plus the configured
    // per-turn budget ceilings (T17) and the cwd's checkout HEAD (T20,
    // best-effort: unresolvable → null fields). T115: `goal_sha256` is null
    // here — chat sessions open goal-less (objectives arrive turn by turn).
    // T117: `goal_pack` is null too — chat-side pack expansion is per-turn
    // (T113), so a session never opens with a pack-expanded goal.
    let head = crate::build_info::resolve_head(&cfg.cwd);
    eventlog::run_start(
        &cfg.cwd,
        "chat",
        None,
        &cfg.model,
        cfg.max_iters,
        cfg.max_minutes,
        cfg.max_tokens,
        cfg.max_tokens_per_request,
        crate::build_info::as_pair(&head),
        None,
        None,
        // T146: chat carries no approved plan — the fields stay present-null.
        None,
        None,
    );
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
        max_tokens: cfg.max_tokens,
    };
    // T188: the request parked by `/auto-spec` awaiting
    // `/auto-spec-approve`. Session-scoped: a restarted session re-drafts.
    let mut pending_autospec: Option<String> = None;
    // The UI has quit (the objective channel disconnected). Latched: one
    // more drain pass serves anything still queued, then the session exits.
    let mut ui_gone = false;

    loop {
        // Idle: serve auto-spec requests first (T188 — they are actionable
        // exactly while idle), then wait for the next objective. The poll
        // wakes every 100 ms so a request sent while idle is served without
        // waiting for an objective. The poll DECISION is [`poll_step`] —
        // extracted as a seam so the drain-after-disconnect fix is pinned
        // deterministically (T188 round-4); the serving arms below are the
        // byte-identical arms the inline poll ran before it.
        let objective = loop {
            let step = poll_step(
                &mut ChannelSources {
                    autospec_rx: &cfg.autospec_rx,
                    objective_rx: &cfg.objective_rx,
                },
                &mut ui_gone,
                IDLE_POLL_TICK,
            );
            match step {
                PollStep::Serve(AutoSpecRequest::Draft(request)) => {
                    pending_autospec =
                        handle_auto_spec_draft(&cfg, client, trace.as_deref(), sink, request);
                }
                PollStep::Serve(AutoSpecRequest::Approve) => {
                    match handle_auto_spec_approve(
                        &cfg,
                        sink,
                        pending_autospec.as_deref(),
                        &mut knobs,
                    ) {
                        // Approved: start the turn with the pending request —
                        // and CONSUME it (F4): the draft has been gated and
                        // its file loaded as the spec, so the parked request
                        // is spent. Clearing here is what keeps a stale
                        // draft from being re-approved into a second turn
                        // (a refusal does NOT clear — the operator edits the
                        // draft and re-approves the same request).
                        Ok(request) => {
                            pending_autospec = None;
                            break request;
                        }
                        // The gate refused: stay idle (the notice landed).
                        Err(notice) => sink.emit(Event::AutoSpecNote(notice)),
                    }
                }
                // An objective arrived — start the turn with it.
                PollStep::Objective(objective) => break objective,
                // Empty queue. If the UI already quit, the drain is
                // complete: nothing actionable remains — exit the session
                // gracefully.
                PollStep::Drained => {
                    if let Some(trace) = &trace {
                        obs.trace_finished(trace, observ::outcome::COMPLETED, turns);
                    }
                    return Ok(0);
                }
                // Nothing actionable this tick — poll again.
                PollStep::Idle => {}
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
            mcp,
            trace.as_deref(),
            obs,
            sink,
        )?;
        turns += 1;
        sink.emit(Event::TurnEnd { reason });
    }
}

/// T188 worker side of `/auto-spec <request>`: one direct LLM call (no
/// tools — a nested plan session would rotate the live chat transcript),
/// the same validation + vacuous gate as the headless draft, the draft
/// written to `.chug/auto-spec.md` and shown in the activity stream.
/// Returns `Some(request)` to park pending `/auto-spec-approve`.
fn handle_auto_spec_draft(
    cfg: &ChatConfig,
    client: &mut dyn Llm,
    trace: Option<&str>,
    sink: &mut dyn EventSink,
    request: String,
) -> Option<String> {
    sink.emit(Event::AutoSpecNote(format!(
        "drafting spec for: {} (one tool-less call…)",
        crate::events::preview(&request, 120)
    )));
    match autospec::chat_draft(&cfg.cwd, &request, client, trace) {
        Ok(draft) => {
            sink.emit(Event::ModelText(draft.clone()));
            sink.emit(Event::AutoSpecNote(format!(
                "draft written to {} — edit it, then /auto-spec-approve to run",
                autospec::AUTO_SPEC_REL
            )));
            Some(request)
        }
        Err(why) => {
            sink.emit(Event::AutoSpecNote(format!("auto-spec draft refused: {why}")));
            // Nothing parks: the operator re-runs /auto-spec.
            None
        }
    }
}

/// T188 worker side of `/auto-spec-approve`: gate the draft at
/// `.chug/auto-spec.md` through the SAME rules the headless gate uses
/// (structure, non-vacuous check, dry-run — the check is never loosened),
/// then load the spec + check knobs and hand back the pending request as
/// the turn's objective. `Err` is the operator-facing refusal; the session
/// stays idle.
fn handle_auto_spec_approve(
    cfg: &ChatConfig,
    sink: &mut dyn EventSink,
    pending: Option<&str>,
    knobs: &mut TurnKnobs,
) -> Result<String, String> {
    let request = pending.ok_or_else(|| {
        "auto-spec: nothing pending — /auto-spec <request> first, then approve".to_string()
    })?;
    let spec_path = cfg.cwd.join(autospec::AUTO_SPEC_REL);
    let text = std::fs::read_to_string(&spec_path).map_err(|e| {
        format!(
            "auto-spec: no draft at {} ({e}) — /auto-spec <request> first",
            autospec::AUTO_SPEC_REL
        )
    })?;
    let check = autospec::approve_gate(&cfg.cwd, &text)?;
    // Load the drafted spec as the session spec and its check as the
    // goal_complete gate — the T146 pattern: the operator approved the file,
    // the turn runs against exactly that file.
    knobs.spec_path = Some(spec_path);
    knobs.check_cmd = Some(check);
    sink.emit(Event::AutoSpecNote(format!(
        "auto-spec approved — running with {} as the spec; its check gates goal_complete",
        autospec::AUTO_SPEC_REL
    )));
    Ok(request.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::ScriptedLlm;
    use crate::events::TurnEndReason;
    use serde_json::{Value, json};
    use std::collections::VecDeque;
    use std::fs;
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

    /// T188 (F7): the auto-spec entry points parse — both slash commands,
    /// with and without an argument, whitespace-tolerant like every other
    /// command. A dropped or renamed arm (or an arg swallowed whole) is RED
    /// here.
    #[test]
    fn parse_slash_pins_the_auto_spec_commands() {
        assert_eq!(
            parse_slash("/auto-spec fix the flaky test"),
            Some(SlashCommand::AutoSpec(Some("fix the flaky test".into()))),
            "/auto-spec carries its request text"
        );
        assert_eq!(
            parse_slash("/auto-spec"),
            Some(SlashCommand::AutoSpec(None)),
            "bare /auto-spec parses (the usage leg)"
        );
        assert_eq!(
            parse_slash("  /auto-spec-approve"),
            Some(SlashCommand::AutoSpecApprove),
            "the approve command parses (whitespace-tolerant)"
        );
        assert_eq!(
            parse_slash("/auto-spec-approve"),
            Some(SlashCommand::AutoSpecApprove)
        );
        // The two commands are distinct names — an -approve arm that
        // swallowed the bare command's arg shape (or vice versa) is RED.
        assert_ne!(
            parse_slash("/auto-spec x"),
            parse_slash("/auto-spec-approve")
        );
    }

    #[test]
    fn parse_unknown_and_malformed() {
        assert_eq!(
            parse_slash("/xyzzy"),
            Some(SlashCommand::Unknown {
                name: "xyzzy".into(),
                args: None
            })
        );
        // An unknown name keeps its arguments for the pack dispatcher (F9).
        assert_eq!(
            parse_slash("/review the diff"),
            Some(SlashCommand::Unknown {
                name: "review".into(),
                args: Some("the diff".into())
            })
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
        assert_eq!(
            parse_slash("/"),
            Some(SlashCommand::Unknown {
                name: "".into(),
                args: None
            })
        );
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
        let (_autospec_tx, autospec_rx) = mpsc::channel();
        let (_steer_tx, steering_rx) = mpsc::channel();
        let abort = Arc::new(AtomicBool::new(false));
        let cfg = ChatConfig {
            cwd: tmp.path().to_path_buf(),
            model: "scripted-model".into(),
            max_iters: 40,
            max_minutes: 120,
            max_tokens: 0, // no token budget: pre-T15 behavior
            max_tokens_per_request: crate::api::DEFAULT_MAX_TOKENS,
            resume: false,
            risk_gate: false,
            bash_timeout: Duration::from_secs(crate::tools::BASH_TIMEOUT_SECS),
            // mcp_off: tests must never pick up the developer's
            // ~/.config/chug/mcp.json.
            mcp_config: None,
            mcp_off: true,
            controls: Controls {
                abort: Arc::clone(&abort),
                steering_rx,
            },
            objective_rx,
            update_rx,
            autospec_rx,
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
        let cwd_for_worker = cwd.clone();
        let worker = std::thread::spawn(move || {
            // Forced-off registry: MCP is a strict no-op in these tests.
            let mut mcp = crate::mcp::McpRegistry::new(&cwd_for_worker, true, None)
                .expect("empty mcp registry");
            // Noop observability sink: tests that assert on Langfuse events
            // build their own live test sink (see the trace-lifecycle test).
            let code = run_chat_with(
                h.cfg,
                &mut h.llm,
                None,
                &mut mcp,
                &mut h.sink,
                &crate::observ::Sink::Noop,
            )
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

    /// T38: a chat turn whose pure-text reply comes back truncated still
    /// ends the turn (natural-stop semantics unchanged), but the advisory
    /// follows the truncated assistant message in the transcript — the next
    /// turn's model learns its output was cut short — and the events record
    /// notes the injection.
    #[test]
    fn truncated_reply_injects_advisory_and_still_ends_turn() {
        let tmp = tempfile::tempdir().unwrap();
        let truncated = json!({
            "stop_reason": "max_tokens",
            "usage": {"input_tokens": 10, "output_tokens": 8192},
            "content": [{"type": "text", "text": "partial ans"}],
        });
        let h = harness(&tmp, vec![truncated]);
        let (code, events, _llm, cwd) = run_session(h, |objective_tx, _| {
            objective_tx.send("answer me".into()).unwrap();
        });
        assert_eq!(code, 0);
        // The advisory never changes turn semantics: still a natural stop.
        assert_eq!(turn_ends(&events), vec![TurnEndReason::Completed]);
        // Transcript: objective, truncated assistant, then the advisory.
        let saved = transcript::load(&cwd).unwrap();
        assert_eq!(saved.len(), 3);
        assert_eq!(saved[1].role, "assistant");
        let last = saved.last().unwrap();
        assert_eq!(last.role, "user");
        assert_eq!(
            last.content[0].text(),
            Some(driver::OUTPUT_TRUNCATED_ADVISORY)
        );
        // Telemetry: exactly one OutputTruncated event for the injection.
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, Event::OutputTruncated))
                .count(),
            1
        );
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

    /// T11: a chat session's events log opens with the banner fields, mode
    /// "chat" (mirrors the run-mode run_start line).
    #[test]
    fn chat_session_opens_events_log_with_run_start() {
        let tmp = tempfile::tempdir().unwrap();
        let h = harness(&tmp, vec![text_response("hi")]);
        let (code, _, _, cwd) = run_session(h, |objective_tx, _| {
            objective_tx.send("hello".into()).unwrap();
        });
        assert_eq!(code, 0);
        let first: serde_json::Value = serde_json::from_str(
            std::fs::read_to_string(cwd.join(".chug/events.jsonl"))
                .expect("events.jsonl written")
                .lines()
                .next()
                .expect("run_start line"),
        )
        .expect("first line parses");
        assert_eq!(first["type"], "run_start");
        assert_eq!(first["mode"], "chat");
        assert_eq!(first["model"], "scripted-model");
        assert_eq!(first["version"], crate::build_info::VERSION);
        assert_eq!(first["commit"], crate::build_info::GIT_COMMIT);
        assert!(first["spec"].is_null(), "chat starts spec-less: {first}");
        // T17: the session's per-turn budget ceilings ride the banner too.
        assert_eq!(first["max_iters"], 40);
        assert_eq!(first["max_minutes"], 120);
        assert!(first["max_tokens"].is_null(), "no token budget → null");
        // T20: the cwd's checkout HEAD rides the chat session's opening line
        // too; a tempdir cwd is not a repo, so both stay null.
        assert!(first["head_branch"].is_null(), "{first}");
        assert!(first["head_commit"].is_null(), "{first}");
    }

    /// T117 wiring leg: chat opens its session pack-less — `goal_pack` is
    /// null but the field is always present (chat-side pack expansion is
    /// per-turn, T113; a session never opens with an expanded goal).
    #[test]
    fn chat_run_start_goal_pack_null_but_always_present() {
        let tmp = tempfile::tempdir().unwrap();
        let h = harness(&tmp, vec![text_response("hi")]);
        let (code, _, _, cwd) = run_session(h, |objective_tx, _| {
            objective_tx.send("hello".into()).unwrap();
        });
        assert_eq!(code, 0);
        let first: serde_json::Value = serde_json::from_str(
            std::fs::read_to_string(cwd.join(".chug/events.jsonl"))
                .expect("events.jsonl written")
                .lines()
                .next()
                .expect("run_start line"),
        )
        .expect("first line parses");
        assert!(
            first["goal_pack"].is_null(),
            "chat never expands a session goal → null: {first}"
        );
        assert!(
            first
                .as_object()
                .expect("run_start is an object")
                .contains_key("goal_pack"),
            "the field must be PRESENT even when null: {first}"
        );
    }

    /// T143: chat mode carries the per-request cap both ways — the session's
    /// run_start line records the configured cap, and a truncated turn's
    /// advisory gains the raise-the-cap remedy line when the cap is below
    /// 32768 (the client double reports the lowered cap exactly as the real
    /// `Client::new(model, cap)` wiring would send it).
    #[test]
    fn chat_records_cap_and_adds_remedy_on_truncation_below_default() {
        let tmp = tempfile::tempdir().unwrap();
        let mut h = harness(
            &tmp,
            vec![
                json!({
                    "stop_reason": "max_tokens",
                    "usage": {"input_tokens": 10, "output_tokens": 8192},
                    "content": [
                        {"type": "tool_use", "id": "tu_1", "name": "bash", "input": {"command": "true"}}
                    ],
                }),
                text_response("all done"),
            ],
        );
        h.cfg.max_tokens_per_request = 8192;
        h.llm.max_tokens_per_request = 8192;
        let (code, _, _, cwd) = run_session(h, |objective_tx, _| {
            objective_tx.send("write a big file".into()).unwrap();
        });
        assert_eq!(code, 0);

        // The session's opening line records the configured per-request cap.
        let first: serde_json::Value = serde_json::from_str(
            std::fs::read_to_string(cwd.join(".chug/events.jsonl"))
                .expect("events.jsonl written")
                .lines()
                .next()
                .expect("run_start line"),
        )
        .expect("first line parses");
        assert_eq!(first["max_tokens_per_request"], 8192);

        // The truncated turn's advisory carries the remedy line.
        let on_disk = transcript::load(&cwd).unwrap();
        let remedy = on_disk
            .iter()
            .filter_map(|m| {
                m.content
                    .iter()
                    .filter_map(|b| b.text())
                    .find(|t| t.contains("raise CHUG_MAX_TOKENS"))
            })
            .next()
            .expect("remedy line in the chat transcript");
        assert!(
            remedy.starts_with("chug: output truncated"),
            "the remedy extends the T38 advisory, never replaces it: {remedy}"
        );
        assert!(remedy.contains("8192"), "names the cap in effect: {remedy}");
    }

    /// T119 wiring leg (the chat call-site `None→Some` mutant): chat
    /// sessions open goal-less (objectives arrive turn by turn, T113), so
    /// the session's `run_start` records `goal_sha256: null` — with the key
    /// PRESENT (the T115 always-present pattern), so jq can distinguish
    /// "no goal" from a truncated line. The T115 eventlog-level legs pin
    /// the field's shape GIVEN `None`; this leg runs the REAL chat session
    /// (the scripted-provider shape), so a call-site mutant that passes
    /// `Some(...)` renders a phantom hash and fails the null assert.
    #[test]
    fn chat_session_run_start_goal_sha256_null_but_always_present() {
        let tmp = tempfile::tempdir().unwrap();
        let h = harness(&tmp, vec![text_response("hi")]);
        let (code, _, _, cwd) = run_session(h, |objective_tx, _| {
            objective_tx.send("hello".into()).unwrap();
        });
        assert_eq!(code, 0);
        let first: serde_json::Value = serde_json::from_str(
            std::fs::read_to_string(cwd.join(".chug/events.jsonl"))
                .expect("events.jsonl written")
                .lines()
                .next()
                .expect("run_start line"),
        )
        .expect("first line parses");
        assert!(
            first["goal_sha256"].is_null(),
            "a goal-less chat session records null, never a phantom hash: {first}"
        );
        assert!(
            first
                .as_object()
                .expect("run_start is an object")
                .contains_key("goal_sha256"),
            "the field must be PRESENT even when null: {first}"
        );
    }

    /// T17: a chat session with configured budgets (token budget set) opens
    /// its events log with those ceilings as numbers, so jq can distinguish
    /// unset from set.
    #[test]
    fn chat_run_start_records_configured_budgets() {
        let tmp = tempfile::tempdir().unwrap();
        let mut h = harness(&tmp, vec![text_response("hi")]);
        h.cfg.max_iters = 7;
        h.cfg.max_minutes = 9;
        h.cfg.max_tokens = 250_000;
        let (code, _, _, cwd) = run_session(h, |objective_tx, _| {
            objective_tx.send("hello".into()).unwrap();
        });
        assert_eq!(code, 0);
        let first: serde_json::Value = serde_json::from_str(
            std::fs::read_to_string(cwd.join(".chug/events.jsonl"))
                .expect("events.jsonl written")
                .lines()
                .next()
                .expect("run_start line"),
        )
        .expect("first line parses");
        assert_eq!(first["type"], "run_start");
        assert_eq!(first["max_iters"], 7);
        assert_eq!(first["max_minutes"], 9);
        assert_eq!(first["max_tokens"], 250_000);
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
            Event::Aborted { reason, .. } if reason == "operator interrupt"
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
            Event::Aborted { reason, .. } if reason == "iteration budget exceeded"
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

    // ---------- T188 auto-spec: the session-level draft/approve loop ----------

    /// The chat_draft fixture: structurally complete, non-vacuous, and
    /// green-dry-run in a cwd containing `marker.txt`.
    const AUTOSPEC_DRAFT: &str = "\
# T-fix — the approved request

check: test -f marker.txt

estimate: ~5 changed lines

## Concern

The request needs a spec.

## Requirements

- The marker file stays present.

## Tests

- `test -f marker.txt` exits 0.

## Acceptance

- The check exits 0 on the current tree.
";

    /// Drive the session with access to ALL three senders (the harness's
    /// run_session only forwards objective+update) so a script can park and
    /// approve auto-spec requests.
    fn run_autospec_session(
        h: Harness,
        script: impl FnOnce(
            &mpsc::Sender<String>,
            &mpsc::Sender<SlashUpdate>,
            &mpsc::Sender<AutoSpecRequest>,
        ) + Send
        + 'static,
    ) -> (i32, Vec<Event>, ScriptedLlm) {
        let (autospec_tx, autospec_rx) = mpsc::channel();
        let cfg = ChatConfig {
            autospec_rx,
            ..h.cfg
        };
        let cwd = cfg.cwd.clone();
        let cwd_for_worker = cwd.clone();
        let mut h = Harness { cfg, ..h };
        let worker = std::thread::spawn(move || {
            let mut mcp = crate::mcp::McpRegistry::new(&cwd_for_worker, true, None)
                .expect("empty mcp registry");
            let code = run_chat_with(
                h.cfg,
                &mut h.llm,
                None,
                &mut mcp,
                &mut h.sink,
                &crate::observ::Sink::Noop,
            )
            .expect("chat session failed");
            (code, h.sink.0, h.llm)
        });
        script(&h.objective_tx, &h.update_tx, &autospec_tx);
        drop(h.objective_tx);
        drop(h.update_tx);
        drop(autospec_tx);
        let (code, events, llm) = worker.join().expect("worker panicked");
        (code, events, llm)
    }

    fn autospec_notes(events: &[Event]) -> Vec<String> {
        events
            .iter()
            .filter_map(|e| match e {
                Event::AutoSpecNote(note) => Some(note.clone()),
                _ => None,
            })
            .collect()
    }

    /// F4: an approved auto-spec request is CONSUMED — the session clears
    /// `pending_autospec` when the gate passes, so a stale draft can never
    /// be re-approved into a second turn. Pre-fix the second
    /// /auto-spec-approve re-gated the still-on-disk draft and started a
    /// second (stale) turn; the pinned refusal ("nothing pending") never
    /// fired and the scripted LLM ran dry.
    #[test]
    fn auto_spec_approve_consumes_the_pending_request() {
        let tmp = tempfile::tempdir().unwrap();
        // The drafted check dry-runs green in the session cwd.
        fs::write(tmp.path().join("marker.txt"), "x").unwrap();
        let h = harness(
            &tmp,
            vec![
                // The tool-less draft call returns the complete spec.
                json!({
                    "stop_reason": "end_turn",
                    "usage": {"input_tokens": 10, "output_tokens": 5},
                    "content": [{"type": "text", "text": AUTOSPEC_DRAFT}]
                }),
                // The approved turn completes in one reply.
                text_response("did the approved work"),
            ],
        );
        let (code, events, llm) = run_autospec_session(h, |objective_tx, _, autospec_tx| {
            autospec_tx
                .send(AutoSpecRequest::Draft("fix the login bug".into()))
                .unwrap();
            // Approve #1: gates the draft and starts the turn.
            autospec_tx.send(AutoSpecRequest::Approve).unwrap();
            // Approve #2 (the stale one): must refuse — nothing is pending
            // anymore — and start NO second turn.
            autospec_tx.send(AutoSpecRequest::Approve).unwrap();
            let _ = objective_tx;
        });
        assert_eq!(code, 0);
        // Exactly ONE turn started, carrying the approved request.
        let starts: Vec<&String> = events
            .iter()
            .filter_map(|e| match e {
                Event::TurnStart { objective } => Some(objective),
                _ => None,
            })
            .collect();
        assert_eq!(
            starts,
            vec!["fix the login bug"],
            "exactly the approved turn may start: {starts:?}"
        );
        // The stale approve refused with the nothing-pending notice.
        let notes = autospec_notes(&events);
        assert!(
            notes
                .iter()
                .any(|n| n.contains("nothing pending")),
            "the stale re-approve must be refused as nothing pending: {notes:?}"
        );
        // Two LLM calls total: the draft + the single approved turn.
        assert_eq!(llm.calls.len(), 2, "no second (stale) turn: {notes:?}");
    }

    /// The refusal side of the same class: a gate refusal NEVER consumes
    /// the pending request — the operator edits the draft and re-approves
    /// the same request. (Over-clearing would strand the parked request.)
    #[test]
    fn auto_spec_approve_refusal_keeps_the_request_pending() {
        let tmp = tempfile::tempdir().unwrap();
        // The draft's check is non-vacuous but FAILS the dry-run (no
        // marker.txt) — chat_draft writes it, the approve gate refuses it.
        let failing = AUTOSPEC_DRAFT.replace("test -f marker.txt", "test -f absent-marker.txt");
        let h = harness(
            &tmp,
            vec![
                json!({
                    "stop_reason": "end_turn",
                    "usage": {"input_tokens": 10, "output_tokens": 5},
                    "content": [{"type": "text", "text": failing}]
                }),
            ],
        );
        let (code, events, llm) = run_autospec_session(h, |_, _, autospec_tx| {
            autospec_tx
                .send(AutoSpecRequest::Draft("fix the login bug".into()))
                .unwrap();
            // Two approvals of the SAME (still-failing) draft: both refuse
            // on the dry-run — the request stays parked, never "nothing
            // pending". The brief hold-open matters: a refusal does NOT
            // break the idle poll (only an approval starts a turn), so the
            // session would otherwise see the closed objective channel and
            // exit before draining the second queued approve.
            autospec_tx.send(AutoSpecRequest::Approve).unwrap();
            autospec_tx.send(AutoSpecRequest::Approve).unwrap();
            std::thread::sleep(Duration::from_millis(2000));
        });
        assert_eq!(code, 0);
        let notes = autospec_notes(&events);
        assert_eq!(
            notes.iter().filter(|n| n.contains("dry-run")).count(),
            2,
            "both approvals gate the same parked draft: {notes:?}"
        );
        assert!(
            !notes.iter().any(|n| n.contains("nothing pending")),
            "a refusal must keep the request pending: {notes:?}"
        );
        // No turn ever started; only the draft call was made.
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, Event::TurnStart { .. })),
            "a refused approve never starts a turn: {notes:?}"
        );
        assert_eq!(llm.calls.len(), 1, "only the draft call ran");
    }

    // ---------- T188 round-4: the idle-poll drain seam (deterministic) ----------

    /// A scripted [`IdlePollSources`] for the poll seam tests: each side's
    /// results are queued in order; a queued `Ok` is CONSUMED like a real
    /// channel pops it, while an `Err` (empty queue / disconnect) is the
    /// channel STATE and persists for the next poll; an exhausted script
    /// settles on empty/disconnected. Deterministic by construction — no
    /// threads, no wall-clock.
    struct ScriptedSources {
        autospec: VecDeque<Result<AutoSpecRequest, TryRecvError>>,
        objectives: VecDeque<Result<String, RecvTimeoutError>>,
        /// Poll counts, for asserting what the exit does after the latch.
        autospec_polls: u32,
        objective_waits: u32,
    }

    impl IdlePollSources for ScriptedSources {
        fn try_recv_autospec(&mut self) -> Result<AutoSpecRequest, TryRecvError> {
            self.autospec_polls += 1;
            match self.autospec.pop_front() {
                Some(Err(why)) => {
                    // Only the LAST entry persists (the channel state an
                    // empty/disconnected queue keeps returning); a scripted
                    // sequence still advances.
                    if self.autospec.is_empty() {
                        self.autospec.push_front(Err(why));
                    }
                    Err(why)
                }
                other => other.unwrap_or(Err(TryRecvError::Empty)),
            }
        }

        fn recv_objective(&mut self, _tick: Duration) -> Result<String, RecvTimeoutError> {
            self.objective_waits += 1;
            match self.objectives.pop_front() {
                Some(Err(why)) => {
                    if self.objectives.is_empty() {
                        self.objectives.push_front(Err(why));
                    }
                    Err(why)
                }
                other => other.unwrap_or(Err(RecvTimeoutError::Disconnected)),
            }
        }
    }

    /// Drive [`poll_step`] exactly like the session's idle loop: record
    /// every served request, stop at the first Objective/Drained.
    fn run_poll_steps(src: &mut ScriptedSources) -> (Vec<AutoSpecRequest>, PollStep) {
        let mut ui_gone = false;
        let mut served = Vec::new();
        for _ in 0..100 {
            match poll_step(src, &mut ui_gone, IDLE_POLL_TICK) {
                PollStep::Serve(request) => served.push(request),
                PollStep::Objective(objective) => return (served, PollStep::Objective(objective)),
                PollStep::Drained => return (served, PollStep::Drained),
                PollStep::Idle => {}
            }
        }
        panic!("the idle poll never resolved (script never yields Objective/Drained)");
    }

    /// T188 round-4, THE m8 killer: the en-route drain. The UI quits (the
    /// objective channel disconnects) and the /auto-spec request lands in
    /// the queue only on a poll AFTER that disconnect was observed — the
    /// race the session-level tests above hit only by wall-clock luck
    /// (they drop the senders with the queue already populated, which the
    /// pre-fix code drained anyway). The fix latches the disconnect and
    /// takes one more drain pass; under m8 (return immediately on
    /// disconnect) the request is dropped and this test fails
    /// deterministically, not racily.
    #[test]
    fn idle_poll_drains_an_en_route_request_after_the_disconnect() {
        let mut src = ScriptedSources {
            // Poll 1: the queue is momentarily empty (the request is in
            // flight); poll 2 — after the disconnect was observed — it
            // lands.
            autospec: VecDeque::from(vec![
                Err(TryRecvError::Empty),
                Ok(AutoSpecRequest::Draft("en-route request".into())),
            ]),
            // The objective channel reports the quit on the first wait.
            objectives: VecDeque::from(vec![Err(RecvTimeoutError::Disconnected)]),
            autospec_polls: 0,
            objective_waits: 0,
        };
        let (served, end) = run_poll_steps(&mut src);
        assert_eq!(
            served,
            vec![AutoSpecRequest::Draft("en-route request".into())],
            "a request queued around the UI quit must be served by the drain pass"
        );
        assert_eq!(
            end,
            PollStep::Drained,
            "the session still exits gracefully once the queue is empty"
        );
        // Three polls: empty (the disconnect was observed), the drain pass
        // serving the request, then the latched exit on the empty queue.
        assert_eq!(src.autospec_polls, 3);
        // The exit is the LATCHED one: after the disconnect the empty queue
        // ends the session without another blocking wait on the dead
        // channel.
        assert_eq!(src.objective_waits, 1);
    }

    /// The serve-first ordering: a queued auto-spec request is served
    /// BEFORE the session takes the next objective (T188 — actionable
    /// exactly while idle; the objective's turn starts only after the
    /// queue is drained).
    #[test]
    fn idle_poll_serves_a_queued_request_before_the_next_objective() {
        let mut src = ScriptedSources {
            autospec: VecDeque::from(vec![Ok(AutoSpecRequest::Approve)]),
            objectives: VecDeque::from(vec![Ok("the objective".into())]),
            autospec_polls: 0,
            objective_waits: 0,
        };
        let (served, end) = run_poll_steps(&mut src);
        assert_eq!(
            served,
            vec![AutoSpecRequest::Approve],
            "the queued request is served before any objective"
        );
        assert_eq!(
            end,
            PollStep::Objective("the objective".into()),
            "the objective still starts the turn once the queue is drained"
        );
        assert_eq!(src.autospec_polls, 2);
        assert_eq!(src.objective_waits, 1);
    }

    /// Exit-on-empty: nothing ever queued and the UI already quit — the
    /// poll latches the disconnect and ends the session on the next poll
    /// without serving anything and without re-waiting on the dead
    /// channel.
    #[test]
    fn idle_poll_exits_when_the_drain_completes_with_nothing_queued() {
        let mut src = ScriptedSources {
            autospec: VecDeque::new(),
            objectives: VecDeque::from(vec![Err(RecvTimeoutError::Disconnected)]),
            autospec_polls: 0,
            objective_waits: 0,
        };
        let (served, end) = run_poll_steps(&mut src);
        assert!(served.is_empty());
        assert_eq!(end, PollStep::Drained);
        assert_eq!(src.autospec_polls, 2);
        assert_eq!(
            src.objective_waits, 1,
            "a latched disconnect exits without re-waiting"
        );
    }

    /// The tick timeout is NOT a disconnect: an idle session with a live
    /// UI keeps polling (Timeout → Idle) until the objective (or a queued
    /// request) arrives; latching on Timeout would strand both.
    #[test]
    fn idle_poll_keeps_polling_through_the_tick_timeout() {
        let mut src = ScriptedSources {
            autospec: VecDeque::new(),
            objectives: VecDeque::from(vec![
                Err(RecvTimeoutError::Timeout),
                Ok("late objective".into()),
            ]),
            autospec_polls: 0,
            objective_waits: 0,
        };
        let (served, end) = run_poll_steps(&mut src);
        assert!(served.is_empty());
        assert_eq!(
            end,
            PollStep::Objective("late objective".into()),
            "a tick timeout never latches the disconnect"
        );
        assert_eq!(src.autospec_polls, 2);
        assert_eq!(src.objective_waits, 2);
    }

    // ---------- session trace lifecycle (SPEC-8) ----------

    #[test]
    fn one_trace_per_session_finished_on_quit_not_per_turn() {
        let tmp = tempfile::tempdir().unwrap();
        let (objective_tx, objective_rx) = mpsc::channel();
        let (update_tx, update_rx) = mpsc::channel();
        let (_steer_tx, steering_rx) = mpsc::channel();
        let (_autospec_tx, autospec_rx) = mpsc::channel();
        let cfg = ChatConfig {
            cwd: tmp.path().to_path_buf(),
            model: "scripted-model".into(),
            max_iters: 40,
            max_minutes: 120,
            max_tokens: 0, // no token budget: pre-T15 behavior
            max_tokens_per_request: crate::api::DEFAULT_MAX_TOKENS,
            resume: false,
            risk_gate: false,
            bash_timeout: Duration::from_secs(crate::tools::BASH_TIMEOUT_SECS),
            controls: Controls {
                abort: Arc::new(AtomicBool::new(false)),
                steering_rx,
            },
            objective_rx,
            update_rx,
            autospec_rx,
            // Tests must never pick up the developer's ~/.config/chug/mcp.json.
            mcp_config: None,
            mcp_off: true,
        };
        let mut llm = ScriptedLlm::new(vec![text_response("one"), text_response("two")]);
        let transport = crate::observ::testing::CountingTransport::new();
        let obs = crate::observ::testing::test_sink(transport.clone());
        let mut sink = RecordingSink::default();
        let cwd_for_worker = cfg.cwd.clone();
        let worker = std::thread::spawn(move || {
            // Forced-off registry: MCP is a strict no-op in these tests.
            let mut mcp = crate::mcp::McpRegistry::new(&cwd_for_worker, true, None)
                .expect("empty mcp registry");
            run_chat_with(cfg, &mut llm, None, &mut mcp, &mut sink, &obs)
                .expect("chat session failed")
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
