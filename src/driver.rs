use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use anyhow::{Context, bail};
use serde_json::{Value, json};

use crate::api::{Client, ContentBlock, KnownBlock, Llm, Message, ObsCtx};
use crate::archive;
use crate::events::{Event, EventSink, TurnEndReason};
use crate::ledger;
use crate::mcp::McpRegistry;
use crate::observ;
use crate::riskgate::{GateDecision, LayaJudge, RiskGate};
use crate::tools::{self, ToolCtx, ToolResult};
use crate::transcript;

pub const DEFAULT_MODEL: &str = "claude-sonnet-4-6";

const PREAMBLE: &str = "You are chug, an autonomous coding agent driven by a code loop, not a conversation. Work in small, verified steps. After each step, update LEDGER.md with the update_ledger tool (what is done, what is next, any blockers). Verify your work by running builds/tests before claiming success. Never declare the goal complete without running the relevant checks. When the goal is fully met and verified, call the goal_complete tool with a short summary.";

/// Chat-mode harness preamble: the user is present and drives turn by turn.
const CHAT_PREAMBLE: &str = "You are chug in an interactive session; work the user's current objective; when it is done, stop — the user will give the next objective. Work in small, verified steps. After each step, update LEDGER.md with the update_ledger tool (what is done, what is next, any blockers). Verify your work by running builds/tests before claiming success. When the objective is fully met and verified, call the goal_complete tool with a short summary; otherwise simply stop.";

const KICK: &str = "Ledger and goal are above. You have not called goal_complete. Continue with the next ledger item, or update the ledger if the plan changed.";

const TRIM_ABOVE_TOKENS: usize = 120_000;
const TRIM_TARGET_TOKENS: usize = 80_000;
const KEEP_LAST_MESSAGES: usize = 20;
const STUCK_WINDOW: usize = 3;

pub struct RunConfig {
    pub cwd: PathBuf,
    pub spec_path: PathBuf,
    pub goal: String,
    pub model: String,
    pub max_iters: u32,
    pub max_minutes: u64,
    pub resume: bool,
    /// Shared controls checked at every iteration boundary.
    pub controls: Controls,
    /// When true, every bash command is classified by the laya risk gate
    /// before execution.
    pub risk_gate: bool,
    /// Per-command wall-clock budget for the `bash` tool.
    pub bash_timeout: Duration,
    /// Path to an MCP config JSON. Overrides discovery. None → discovery.
    pub mcp_config: Option<PathBuf>,
    /// Force MCP servers off even when a config exists.
    pub mcp_off: bool,
}

/// Operator controls the driver honors at each iteration boundary:
/// `q` sets the abort flag; steering notes are drained into the transcript.
pub struct Controls {
    pub abort: Arc<AtomicBool>,
    pub steering_rx: Receiver<String>,
}

impl Controls {
    /// Controls nothing can ever trigger (headless default): a never-set flag
    /// and a steering channel whose sender has been dropped.
    pub fn detached() -> Self {
        let (tx, rx) = mpsc::channel();
        drop(tx);
        Controls {
            abort: Arc::new(AtomicBool::new(false)),
            steering_rx: rx,
        }
    }
}

impl Default for Controls {
    fn default() -> Self {
        Self::detached()
    }
}

/// The two loop personalities: `run` (process exits on completion) and `chat`
/// (a turn ends, control returns to the user).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Autonomous,
    Chat,
}

/// Live-tunable session knobs a chat turn re-reads at every iteration
/// boundary; slash commands mutate these mid-turn. In autonomous mode they
/// are fixed for the whole run.
pub struct TurnKnobs {
    /// Spec file injected into the system prompt (re-read every iteration).
    pub spec_path: Option<PathBuf>,
    /// Persistent goal injected into the system prompt (chat: `/goal`).
    pub goal: Option<String>,
    /// Verification command gating `goal_complete` (chat: `/check`; run mode
    /// parses the spec's `check:` line instead).
    pub check_cmd: Option<String>,
    /// Per-turn (or per-run) iteration budget.
    pub max_iters: u32,
    /// Per-turn (or per-run) wall-clock budget in minutes.
    pub max_minutes: u64,
}

/// Mid-turn session updates, parsed from slash commands UI-side and applied
/// by the loop at the next iteration boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlashUpdate {
    Spec(Option<PathBuf>),
    Goal(Option<String>),
    Check(Option<String>),
    Model(String),
    Budget { iters: u32, minutes: u64 },
}

/// Apply one slash-command update to the session knobs (and the API client
/// for model switches, which affect subsequent calls only).
pub fn apply_slash_update(knobs: &mut TurnKnobs, client: &mut dyn Llm, update: SlashUpdate) {
    match update {
        SlashUpdate::Spec(path) => knobs.spec_path = path,
        SlashUpdate::Goal(goal) => knobs.goal = goal,
        SlashUpdate::Check(cmd) => knobs.check_cmd = cmd,
        SlashUpdate::Model(model) => client.set_model(&model),
        SlashUpdate::Budget { iters, minutes } => {
            knobs.max_iters = iters;
            knobs.max_minutes = minutes;
        }
    }
}

/// What a finished drive_loop invocation produced.
enum DriveOutcome {
    /// Autonomous run finished; value is the process exit code.
    RunFinished(i32),
    /// Chat turn ended; the chat loop returns to idle.
    TurnEnded(TurnEndReason),
}

/// Shared, mode-independent context for one drive_loop invocation.
struct LoopCtx<'a> {
    cwd: &'a Path,
    mode: Mode,
    controls: &'a Controls,
    updates: &'a Receiver<SlashUpdate>,
    bash_timeout: Duration,
    /// Langfuse trace for the enclosing run / chat session (`None` when
    /// observability is off): gates all per-event emission in the loop.
    trace: Option<&'a str>,
    /// The process observability sink (Noop when off → every call is a no-op).
    obs: &'a observ::Sink,
}

enum VerifyOutcome {
    Accepted,
    Failed(String),
    NoCheck,
}

pub fn run(cfg: RunConfig, sink: &mut dyn EventSink) -> anyhow::Result<i32> {
    let client = Client::new(&cfg.model)?;
    let gate = if cfg.risk_gate {
        Some(RiskGate::new(Box::new(LayaJudge::from_env()?), &cfg.cwd))
    } else {
        None
    };
    run_loop(cfg, client, gate, sink, observ::global())
}

fn run_loop(
    cfg: RunConfig,
    client: Client,
    mut gate: Option<RiskGate>,
    sink: &mut dyn EventSink,
    obs: &observ::Sink,
) -> anyhow::Result<i32> {
    let mut client = client;
    if cfg.resume {
        ledger::ensure_seeded(&cfg.cwd)?;
    } else {
        // T3: a fresh run never inherits a previous session's ledger — it is
        // archived (never deleted) and the seed re-installed. Continuation
        // across a crash is what --resume is for. Best-effort: housekeeping
        // failures warn on stderr, never abort the run.
        match ledger::archive_stale(&cfg.cwd) {
            archive::Outcome::Archived(path) => {
                eprintln!("chug: archived previous ledger to {}", path.display());
            }
            archive::Outcome::Failed(why) => {
                eprintln!("chug: warning: could not archive previous ledger ({why}); keeping it");
            }
            archive::Outcome::Skipped => {}
        }
        ledger::ensure_seeded(&cfg.cwd)?;
    }
    // MCP servers spawn lazily here, at run start; an empty registry (no
    // config anywhere, or --mcp-off) is a strict no-op. Dropped on every exit
    // path — normal, budget/abort, or panic unwind — killing every server.
    let mut mcp = McpRegistry::new(&cfg.cwd, cfg.mcp_off, cfg.mcp_config.clone())?;

    // SPEC-8: one trace per run, created up front; finished with the outcome
    // + iterations scores at the end of the loop (or left open on a hard
    // error — the process-exit drain still flushes what was emitted).
    let trace = obs.trace_started(
        &cfg.goal,
        &cfg.model,
        &cfg.cwd.display().to_string(),
        "run",
        Some(cfg.spec_path.to_string_lossy().as_ref()),
    );

    let mut messages: Vec<Message> = if cfg.resume {
        resume_messages(&cfg.cwd)?
    } else {
        Vec::new()
    };
    if messages.is_empty() {
        let first = Message::user(vec![ContentBlock::text_block(format!(
            "Goal: {}\n\nThe spec, goal, and ledger are in your system prompt. Start working.",
            cfg.goal
        ))]);
        transcript::append(&cfg.cwd, &first)?;
        messages.push(first);
    }

    // The spec must be readable at startup; afterwards the loop keeps the last
    // good copy if it becomes unreadable mid-run.
    let initial_spec = fs::read_to_string(&cfg.spec_path)
        .with_context(|| format!("reading spec {}", cfg.spec_path.display()))?;

    let mut knobs = TurnKnobs {
        spec_path: Some(cfg.spec_path.clone()),
        goal: Some(cfg.goal.clone()),
        check_cmd: None, // autonomous mode parses the spec's `check:` line
        max_iters: cfg.max_iters,
        max_minutes: cfg.max_minutes,
    };
    let (_update_tx, update_rx) = mpsc::channel::<SlashUpdate>();
    let ctx = LoopCtx {
        cwd: &cfg.cwd,
        mode: Mode::Autonomous,
        controls: &cfg.controls,
        updates: &update_rx,
        bash_timeout: cfg.bash_timeout,
        trace: trace.as_deref(),
        obs,
    };
    match drive_loop(
        &ctx,
        &mut knobs,
        &mut client,
        &mut gate,
        &mut messages,
        Some(initial_spec),
        sink,
        &mut mcp,
    )? {
        DriveOutcome::RunFinished(code) => Ok(code),
        DriveOutcome::TurnEnded(_) => bail!("chat turn outcome in autonomous mode"),
    }
}

/// Load the transcript for a resumed session, trimming it first if it is over
/// the token budget so an over-large transcript starts compact.
pub fn resume_messages(cwd: &Path) -> anyhow::Result<Vec<Message>> {
    let mut messages = transcript::load(cwd)?;
    if !messages.is_empty() && transcript_trim(&mut messages) {
        transcript::rewrite(cwd, &messages)?;
    }
    Ok(messages)
}

/// Run one chat turn: iterate until a natural stop, an accepted
/// `goal_complete`, an operator interrupt, or an exhausted per-turn budget.
/// Budgets reset on every call; `messages` (and the transcript on disk)
/// persist across turns.
#[allow(clippy::too_many_arguments)]
pub fn run_turn(
    cwd: &Path,
    client: &mut dyn Llm,
    gate: &mut Option<RiskGate>,
    messages: &mut Vec<Message>,
    controls: &Controls,
    updates: &Receiver<SlashUpdate>,
    knobs: &mut TurnKnobs,
    bash_timeout: Duration,
    mcp: &mut McpRegistry,
    trace: Option<&str>,
    obs: &observ::Sink,
    sink: &mut dyn EventSink,
) -> anyhow::Result<TurnEndReason> {
    let ctx = LoopCtx {
        cwd,
        mode: Mode::Chat,
        controls,
        updates,
        bash_timeout,
        trace,
        obs,
    };
    match drive_loop(&ctx, knobs, client, gate, messages, None, sink, mcp)? {
        DriveOutcome::TurnEnded(reason) => Ok(reason),
        DriveOutcome::RunFinished(_) => bail!("autonomous run outcome in chat mode"),
    }
}

/// The iteration loop shared by `run` and chat turns. Behavior differences:
/// - Autonomous: natural stop triggers the anti-stall kick; aborts return a
///   process exit code. Chat: natural stop ends the turn; aborts return a
///   `TurnEndReason`.
/// - `goal_complete` verification: autonomous parses the spec's `check:` line,
///   chat uses the `/check` command configured in the knobs.
#[allow(clippy::too_many_arguments)]
fn drive_loop(
    ctx: &LoopCtx,
    knobs: &mut TurnKnobs,
    client: &mut dyn Llm,
    gate: &mut Option<RiskGate>,
    messages: &mut Vec<Message>,
    initial_spec: Option<String>,
    sink: &mut dyn EventSink,
    mcp: &mut McpRegistry,
) -> anyhow::Result<DriveOutcome> {
    // An empty registry (no MCP config) extends with nothing: byte-identical
    // tools array to before.
    let mut tool_schemas = tools::tool_schemas();
    tool_schemas.extend(mcp.tool_schemas());
    let tool_ctx = ToolCtx {
        cwd: ctx.cwd.to_path_buf(),
        bash_timeout: ctx.bash_timeout,
    };
    // Budgets are per invocation: per run in autonomous mode, per turn in chat.
    let start = Instant::now();
    let mut iteration: u32 = 0;
    let mut spec_text = initial_spec;
    let mut recent: VecDeque<ToolResult> = VecDeque::with_capacity(STUCK_WINDOW);
    let (mut usage_in, mut usage_out) = (0u64, 0u64);

    loop {
        if iteration >= knobs.max_iters {
            return abort_exit(
                ctx,
                "iteration budget exceeded",
                TurnEndReason::BudgetExceeded,
                1,
                iteration,
                sink,
            );
        }
        if start.elapsed() >= Duration::from_secs(knobs.max_minutes.saturating_mul(60)) {
            return abort_exit(
                ctx,
                "time budget exceeded",
                TurnEndReason::BudgetExceeded,
                1,
                iteration,
                sink,
            );
        }
        if ctx.controls.abort.load(Ordering::SeqCst) {
            let reason = match ctx.mode {
                Mode::Autonomous => "operator abort",
                Mode::Chat => "operator interrupt",
            };
            return abort_exit(ctx, reason, TurnEndReason::Interrupted, 1, iteration, sink);
        }

        // Steering notes queued by the operator are consumed here, at the
        // iteration boundary, before the next LLM call.
        let notes = drain_steering(&ctx.controls.steering_rx);
        if !notes.is_empty() {
            append_steering_notes(ctx.cwd, messages, &notes, sink)?;
            if let Some(trace) = ctx.trace {
                for note in &notes {
                    ctx.obs.event(trace, "steering", json!({ "note": note }));
                }
            }
            // Operator override: `allow destructive` disables the risk gate
            // for the remainder of the run.
            if notes.iter().any(|n| is_allow_destructive(n))
                && let Some(gate) = gate.as_mut()
            {
                gate.disable(sink);
            }
        }

        // Slash-command updates queued by the UI are applied here, so model /
        // budget / spec / goal / check changes affect subsequent API calls.
        while let Ok(update) = ctx.updates.try_recv() {
            apply_slash_update(knobs, client, update);
        }

        // Re-read spec every iteration: the user may edit it mid-run. Keep the
        // last good copy if it becomes unreadable.
        if let Some(spec_path) = &knobs.spec_path
            && let Ok(text) = fs::read_to_string(spec_path)
        {
            spec_text = Some(text);
        }
        let ledger_text = ledger::read(ctx.cwd)?;
        let system = match ctx.mode {
            Mode::Autonomous => build_system_prompt(
                spec_text.as_deref().unwrap_or_default(),
                knobs.goal.as_deref().unwrap_or_default(),
                &ledger_text,
            ),
            Mode::Chat => build_chat_system_prompt(
                spec_text.as_deref(),
                knobs.goal.as_deref(),
                &ledger_text,
            ),
        };

        sink.emit(Event::LedgerChanged(ledger_text.clone()));
        sink.emit(Event::Iteration {
            n: iteration + 1,
            max: knobs.max_iters,
            messages: messages.len() as u32,
        });
        // SPEC-8: the api layer emits one generation per response (model,
        // usage incl. cache reads, latency, stop reason, iteration) when a
        // trace exists for this loop.
        let obs_ctx = ObsCtx {
            trace_id: ctx.trace,
            iteration,
        };
        let resp = client.complete(&system, messages, &tool_schemas, &obs_ctx)?;

        let usage = resp.body.get("usage").cloned().unwrap_or(Value::Null);
        usage_in += usage
            .get("input_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        usage_out += usage
            .get("output_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        sink.emit(Event::Usage {
            input: usage_in,
            output: usage_out,
        });

        // Store the assistant message verbatim (text, thinking, tool_use, and
        // any unknown block types) so multi-turn echo stays valid.
        let assistant = Message::assistant(resp.content_blocks());
        transcript::append(ctx.cwd, &assistant)?;
        let model_text = resp.text();
        if !model_text.is_empty() {
            sink.emit(Event::ModelText(model_text));
        }

        let mut user_blocks: Vec<ContentBlock> = Vec::new();
        let mut tool_count = 0usize;
        let mut goal_summary: Option<String> = None;

        for (id, name, input) in assistant.content.iter().filter_map(ContentBlock::tool_use) {
            sink.emit(Event::ToolStart {
                name: name.to_string(),
            });
            let tool_start = std::time::SystemTime::now();
            let result = if name.starts_with("mcp__") {
                // MCP tools bypass the laya risk gate (it judges bash only).
                mcp.dispatch(name, input.clone())
            } else if name == "bash" {
                if let Some(gate) = gate.as_mut()
                    && let Some(command) = input.get("command").and_then(Value::as_str)
                    && !gate.is_disabled()
                {
                    let command_preview: String = command.chars().take(200).collect();
                    match gate.check(command, sink) {
                        GateDecision::Blocked(msg) => {
                            if let Some(trace) = ctx.trace {
                                ctx.obs.event(
                                    trace,
                                    "risk_gate",
                                    json!({ "verdict": "blocked", "command": command_preview }),
                                );
                            }
                            ToolResult {
                                content: msg,
                                is_error: true,
                            }
                        }
                        GateDecision::Allowed => {
                            if let Some(trace) = ctx.trace {
                                ctx.obs.event(
                                    trace,
                                    "risk_gate",
                                    json!({ "verdict": "allowed", "command": command_preview }),
                                );
                            }
                            tools::dispatch(&tool_ctx, name, input)
                        }
                    }
                } else {
                    // Disabled gate, no command field, or no gate: execute.
                    tools::dispatch(&tool_ctx, name, input)
                }
            } else {
                tools::dispatch(&tool_ctx, name, input)
            };
            // SPEC-8: one span per tool call with ok / is_error metadata.
            if let Some(trace) = ctx.trace {
                ctx.obs.span(
                    trace,
                    name,
                    tool_start,
                    std::time::SystemTime::now(),
                    !result.is_error,
                    result.is_error,
                );
            }
            sink.emit(Event::ToolResult {
                name: name.to_string(),
                ok: !result.is_error,
                preview: result.content.chars().take(500).collect(),
            });
            if name == "goal_complete" {
                goal_summary = Some(
                    input
                        .get("summary")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                );
            }
            user_blocks.push(ContentBlock::tool_result_block(
                id,
                result.content.clone(),
                result.is_error,
            ));
            recent.push_back(result);
            while recent.len() > STUCK_WINDOW {
                recent.pop_front();
            }
            tool_count += 1;
        }

        if tool_count == 0 {
            if ctx.mode == Mode::Chat {
                // Natural stop ends the turn: the user is present and judges
                // completeness. The assistant message stays the last entry.
                messages.push(assistant);
                return Ok(DriveOutcome::TurnEnded(TurnEndReason::Completed));
            }
            // Model stopped talking without finishing — the anti-stall kick.
            user_blocks.push(ContentBlock::text_block(KICK));
        } else if let Some(summary) = goal_summary {
            // Autonomous: the spec's `check:` line. Chat: the `/check` command.
            let check_cmd = match ctx.mode {
                Mode::Autonomous => spec_text.as_deref().and_then(parse_check_command),
                Mode::Chat => knobs.check_cmd.clone(),
            };
            match verify(ctx.cwd, check_cmd.as_deref(), sink)? {
                VerifyOutcome::NoCheck | VerifyOutcome::Accepted => {
                    let user_msg = Message::user(user_blocks);
                    transcript::append(ctx.cwd, &user_msg)?;
                    sink.emit(Event::GoalAccepted { summary: summary.clone() });
                    if let Some(trace) = ctx.trace {
                        ctx.obs.event(trace, "goal_accepted", json!({ "summary": summary }));
                    }
                    if ctx.mode == Mode::Chat {
                        // Keep full history so the next turn continues the
                        // same conversation.
                        messages.push(assistant);
                        messages.push(user_msg);
                        return Ok(DriveOutcome::TurnEnded(TurnEndReason::GoalAccepted));
                    }
                    // SPEC-8: the run's outcome scores + trace finish.
                    if let Some(trace) = ctx.trace {
                        finish_run(
                            ctx.obs,
                            trace,
                            observ::outcome::COMPLETED,
                            u64::from(iteration) + 1,
                        );
                    }
                    return Ok(DriveOutcome::RunFinished(0));
                }
                VerifyOutcome::Failed(output) => {
                    sink.emit(Event::GoalRejected {
                        reason: "check command failed".to_string(),
                    });
                    if let Some(trace) = ctx.trace {
                        ctx.obs.event(
                            trace,
                            "goal_rejected",
                            json!({ "reason": "check command failed" }),
                        );
                    }
                    user_blocks.push(ContentBlock::text_block(format!(
                        "goal_complete rejected: the spec check command failed. Output:\n\n{output}\n\nFix the failure and try again. Update the ledger to reflect the current state."
                    )));
                }
            }
        }

        messages.push(assistant);
        let user_msg = Message::user(user_blocks);
        transcript::append(ctx.cwd, &user_msg)?;
        messages.push(user_msg);

        if is_stuck(recent.make_contiguous()) {
            return abort_exit(
                ctx,
                "stuck: repeated error",
                TurnEndReason::Interrupted,
                2,
                iteration,
                sink,
            );
        }

        if transcript_trim(messages) {
            transcript::rewrite(ctx.cwd, messages)?;
        }

        iteration += 1;
    }
}

/// Drain ALL pending steering notes, FIFO.
fn drain_steering(rx: &Receiver<String>) -> Vec<String> {
    let mut notes = Vec::new();
    while let Ok(note) = rx.try_recv() {
        notes.push(note);
    }
    notes
}

/// The exact operator override phrase recognized by the risk gate.
fn is_allow_destructive(note: &str) -> bool {
    note.trim().to_lowercase() == "allow destructive"
}

/// Append each note as a user message `[operator] <note>` to the transcript.
fn append_steering_notes(
    cwd: &Path,
    messages: &mut Vec<Message>,
    notes: &[String],
    sink: &mut dyn EventSink,
) -> anyhow::Result<()> {
    for note in notes {
        let msg = Message::user(vec![ContentBlock::text_block(format!("[operator] {note}"))]);
        transcript::append(cwd, &msg)?;
        messages.push(msg);
        sink.emit(Event::SteeringQueued(note.clone()));
    }
    Ok(())
}

/// Verification on `goal_complete`: run the configured check command, if any.
/// Autonomous mode parses the command from the spec's `check:` line; chat mode
/// uses the `/check` setting. No configured command means unverified accept.
fn verify(
    cwd: &Path,
    check_cmd: Option<&str>,
    sink: &mut dyn EventSink,
) -> anyhow::Result<VerifyOutcome> {
    let Some(command) = check_cmd else {
        return Ok(VerifyOutcome::NoCheck);
    };
    sink.emit(Event::Verifying {
        cmd: command.to_string(),
    });
    let outcome = tools::run_shell(cwd, command, Duration::from_secs(tools::CHECK_TIMEOUT_SECS))?;
    if !outcome.timed_out && outcome.exit_code == Some(0) {
        return Ok(VerifyOutcome::Accepted);
    }
    let output = tools::truncate_middle(&outcome.output, 5_000, 5_000);
    let exit_label = match outcome.exit_code {
        Some(code) => code.to_string(),
        None => "timeout".to_string(),
    };
    Ok(VerifyOutcome::Failed(format!(
        "$ {command}\nexit code: {exit_label}\n{output}"
    )))
}

/// First `check: <shell command>` line in the spec text, if any.
pub fn parse_check_command(spec: &str) -> Option<String> {
    spec.lines().find_map(|line| {
        let rest = line.trim().strip_prefix("check:")?;
        let command = rest.trim();
        if command.is_empty() {
            None
        } else {
            Some(command.to_string())
        }
    })
}

/// Stuck tripwire: the last 3 tool results are all errors with identical
/// content (compared on the first 500 chars).
pub fn is_stuck(recent: &[ToolResult]) -> bool {
    if recent.len() < STUCK_WINDOW {
        return false;
    }
    let last: Vec<&ToolResult> = recent.iter().rev().take(STUCK_WINDOW).collect();
    last.iter().all(|r| r.is_error)
        && error_marker(&last[0].content) == error_marker(&last[1].content)
        && error_marker(&last[1].content) == error_marker(&last[2].content)
}

fn error_marker(content: &str) -> String {
    content.chars().take(500).collect()
}

pub fn estimate_tokens(messages: &[Message]) -> usize {
    let chars: usize = messages
        .iter()
        .filter_map(|m| serde_json::to_string(m).ok())
        .map(|s| s.len())
        .sum();
    chars / 4
}

/// Transcript trimming: above 120k estimated tokens, replace tool_result /
/// tool_use payloads older than the last 20 messages with `"[trimmed]"` until
/// under 80k. Message 0 and the last 20 messages are never touched.
pub fn transcript_trim(messages: &mut [Message]) -> bool {
    if estimate_tokens(messages) <= TRIM_ABOVE_TOKENS {
        return false;
    }
    let trim_end = messages.len().saturating_sub(KEEP_LAST_MESSAGES);
    let mut changed = false;
    let mut i = 1; // never trim message 0
    while i < trim_end {
        let mut replaced_any = false;
        for block in messages[i].content.iter_mut() {
            match block {
                ContentBlock::Known(KnownBlock::ToolResult { content, .. }) => {
                    *content = Value::String("[trimmed]".to_string());
                    replaced_any = true;
                }
                ContentBlock::Known(KnownBlock::ToolUse { input, .. }) => {
                    *input = json!({});
                    replaced_any = true;
                }
                _ => {}
            }
        }
        if replaced_any {
            changed = true;
            if estimate_tokens(messages) <= TRIM_TARGET_TOKENS {
                break;
            }
        }
        i += 1;
    }
    changed
}

pub fn build_system_prompt(spec: &str, goal: &str, ledger_text: &str) -> String {
    format!("{PREAMBLE}\n\n## Spec\n\n{spec}\n\n## Goal\n\n{goal}\n\n## Ledger\n\n{ledger_text}")
}

/// Chat-mode system prompt: interactive preamble, optional spec and goal
/// sections (only when configured), ledger as today. The current objective is
/// the final user message, never part of the system prompt.
pub fn build_chat_system_prompt(
    spec: Option<&str>,
    goal: Option<&str>,
    ledger_text: &str,
) -> String {
    let mut prompt = CHAT_PREAMBLE.to_string();
    if let Some(spec) = spec {
        prompt.push_str(&format!("\n\n## Spec\n\n{spec}"));
    }
    if let Some(goal) = goal {
        prompt.push_str(&format!("\n\n## Goal\n\n{goal}"));
    }
    prompt.push_str(&format!("\n\n## Ledger\n\n{ledger_text}"));
    prompt
}

/// End-of-run observability tail: outcome + iterations scores, then the
/// trace-finish upsert (Langfuse merges trace-create events by id).
fn finish_run(obs: &observ::Sink, trace: &str, outcome: &str, iterations: u64) {
    obs.score_outcome(trace, outcome);
    obs.score_iterations(trace, iterations);
    obs.trace_finished(trace, outcome, iterations);
}

/// Map an abort reason onto the categorical outcome score.
fn abort_outcome(reason: &str) -> &'static str {
    if reason.contains("budget") {
        observ::outcome::BUDGET
    } else if reason.contains("stuck") {
        observ::outcome::STUCK
    } else {
        observ::outcome::ABORTED
    }
}

/// Shared abort tail: push the freshest ledger, then the Aborted event, then
/// map to the mode-appropriate outcome (exit code for `run`, turn-end for
/// chat). `turn_reason` is only used in chat mode. `iteration` feeds the
/// `iterations` score on autonomous exits (the completed full iterations).
fn abort_exit(
    ctx: &LoopCtx,
    reason: &str,
    turn_reason: TurnEndReason,
    code: i32,
    iteration: u32,
    sink: &mut dyn EventSink,
) -> anyhow::Result<DriveOutcome> {
    // Push the freshest ledger before the abort event so the sink prints the
    // same contents a direct read would (ConsoleSink caches from events).
    let ledger_text =
        ledger::read(ctx.cwd).unwrap_or_else(|_| "(ledger unavailable)".to_string());
    sink.emit(Event::LedgerChanged(ledger_text));
    sink.emit(Event::Aborted {
        reason: reason.to_string(),
    });
    if let Some(trace) = ctx.trace {
        ctx.obs.event(trace, "abort", json!({ "reason": reason }));
        if ctx.mode == Mode::Autonomous {
            // SPEC-8: finish the run trace with the classified outcome. Chat
            // turns keep the session trace open — the session continues.
            finish_run(ctx.obs, trace, abort_outcome(reason), u64::from(iteration));
        }
    }
    Ok(match ctx.mode {
        Mode::Autonomous => DriveOutcome::RunFinished(code),
        Mode::Chat => DriveOutcome::TurnEnded(turn_reason),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::ScriptedLlm;
    use serde_json::json;

    fn tool_result_msg(text: &str) -> Message {
        Message::user(vec![ContentBlock::tool_result_block("t1", text.to_string(), false)])
    }

    #[test]
    fn tripwire_fires_on_three_identical_errors() {
        let recent: Vec<ToolResult> = (0..3)
            .map(|_| ToolResult {
                content: "boom".to_string(),
                is_error: true,
            })
            .collect();
        assert!(is_stuck(&recent));
    }

    #[test]
    fn tripwire_ignores_different_errors() {
        let recent = vec![
            ToolResult {
                content: "boom".to_string(),
                is_error: true,
            },
            ToolResult {
                content: "boom".to_string(),
                is_error: true,
            },
            ToolResult {
                content: "different failure".to_string(),
                is_error: true,
            },
        ];
        assert!(!is_stuck(&recent));
    }

    #[test]
    fn tripwire_requires_all_errors() {
        let recent = vec![
            ToolResult {
                content: "boom".to_string(),
                is_error: true,
            },
            ToolResult {
                content: "boom".to_string(),
                is_error: true,
            },
            ToolResult {
                content: "boom".to_string(),
                is_error: false,
            },
        ];
        assert!(!is_stuck(&recent));
    }

    #[test]
    fn tripwire_needs_full_window() {
        let recent: Vec<ToolResult> = (0..2)
            .map(|_| ToolResult {
                content: "boom".to_string(),
                is_error: true,
            })
            .collect();
        assert!(!is_stuck(&recent));
    }

    #[test]
    fn tripwire_compares_first_500_chars() {
        let long_err = format!("{}{}", "e".repeat(600), "A");
        let long_err2 = format!("{}{}", "e".repeat(600), "B");
        let mut recent = Vec::new();
        for content in [long_err.clone(), long_err, long_err2] {
            recent.push(ToolResult {
                content,
                is_error: true,
            });
        }
        // identical in the first 500 chars despite differing tails
        assert!(is_stuck(&recent));
    }

    #[test]
    fn check_line_parsing() {
        assert_eq!(
            parse_check_command("goal: x\ncheck: cargo test\n"),
            Some("cargo test".to_string())
        );
        assert_eq!(parse_check_command("no check here"), None);
        assert_eq!(
            parse_check_command("check:   echo hi  "),
            Some("echo hi".to_string())
        );
        assert_eq!(
            parse_check_command("check: first\ncheck: second"),
            Some("first".to_string())
        );
        assert_eq!(parse_check_command("check:"), None);
        assert_eq!(parse_check_command("  check: go build ./..."), Some("go build ./...".to_string()));
    }

    #[test]
    fn trimming_preserves_last_20_and_first_message() {
        let long = "x".repeat(25_000);
        let mut messages = vec![tool_result_msg("first message")];
        for _ in 0..30 {
            messages.push(tool_result_msg(&long));
        }
        assert!(estimate_tokens(&messages) > TRIM_ABOVE_TOKENS);
        assert!(transcript_trim(&mut messages));

        for (i, msg) in messages.iter().enumerate() {
            let ContentBlock::Known(KnownBlock::ToolResult { content, .. }) = &msg.content[0]
            else {
                panic!("expected tool_result at index {i}");
            };
            if i == 0 {
                assert_eq!(content, "first message", "message 0 must never be trimmed");
            } else if i < messages.len() - KEEP_LAST_MESSAGES {
                assert_eq!(content, "[trimmed]", "index {i} should be trimmed");
            } else {
                assert_eq!(content, &long, "index {i} must stay intact");
            }
        }
    }

    #[test]
    fn trimming_noop_under_threshold() {
        let mut messages = vec![tool_result_msg("small"), tool_result_msg("tiny")];
        assert!(!transcript_trim(&mut messages));
        let ContentBlock::Known(KnownBlock::ToolResult { content, .. }) = &messages[0].content[0]
        else {
            panic!("expected tool_result");
        };
        assert_eq!(content, "small");
    }

    #[test]
    fn trimming_respects_target_or_exhausts() {
        let long = "y".repeat(10_000);
        let mut messages = vec![tool_result_msg("first")];
        for _ in 0..60 {
            messages.push(tool_result_msg(&long));
        }
        transcript_trim(&mut messages);
        // 61 messages of ~10k chars start above the 120k-token threshold; the
        // trimmable pool (everything older than the last 20) is big enough to
        // reach the 80k-token target.
        assert!(estimate_tokens(&messages) <= TRIM_TARGET_TOKENS);
        // The last 20 are untouched.
        for msg in &messages[messages.len() - KEEP_LAST_MESSAGES..] {
            let ContentBlock::Known(KnownBlock::ToolResult { content, .. }) = &msg.content[0]
            else {
                panic!("expected tool_result");
            };
            assert_eq!(content, &long);
        }
    }

    #[test]
    fn system_prompt_contains_all_sections() {
        let prompt = build_system_prompt("SPEC TEXT", "do the thing", "# Ledger\n...");
        assert!(prompt.contains("## Spec"));
        assert!(prompt.contains("SPEC TEXT"));
        assert!(prompt.contains("## Goal"));
        assert!(prompt.contains("do the thing"));
        assert!(prompt.contains("## Ledger"));
        assert!(prompt.contains("goal_complete"));
    }

    #[test]
    fn tool_use_blocks_extracted_from_assistant_content() {
        let msg = Message::assistant(vec![
            ContentBlock::text_block("thinking out loud"),
            ContentBlock::Known(KnownBlock::ToolUse {
                id: "tu_1".into(),
                name: "bash".into(),
                input: json!({"command": "ls"}),
            }),
            ContentBlock::Other(json!({"type": "mystery"})),
        ]);
        let uses: Vec<_> = msg.content.iter().filter_map(ContentBlock::tool_use).collect();
        assert_eq!(uses.len(), 1);
        assert_eq!(uses[0].0, "tu_1");
        assert_eq!(uses[0].1, "bash");
    }

    #[derive(Default)]
    struct RecordingSink(Vec<Event>);

    impl EventSink for RecordingSink {
        fn emit(&mut self, e: Event) {
            self.0.push(e);
        }
    }

    #[test]
    fn steering_drains_fifo_into_transcript() {
        let tmp = tempfile::tempdir().unwrap();
        let (tx, rx) = mpsc::channel();
        tx.send("note one".to_string()).unwrap();
        tx.send("note two".to_string()).unwrap();
        drop(tx);

        let notes = drain_steering(&rx);
        assert_eq!(notes, vec!["note one".to_string(), "note two".to_string()]);

        let mut messages = vec![Message::user(vec![ContentBlock::text_block("start")])];
        let mut sink = RecordingSink::default();
        append_steering_notes(tmp.path(), &mut messages, &notes, &mut sink).unwrap();

        assert_eq!(messages.len(), 3);
        for (i, expected) in ["[operator] note one", "[operator] note two"]
            .into_iter()
            .enumerate()
        {
            assert_eq!(messages[1 + i].role, "user");
            assert_eq!(messages[1 + i].content[0].text(), Some(expected));
        }
        // transcript file round-trips exactly what the driver holds
        assert_eq!(transcript::load(tmp.path()).unwrap(), messages[1..]);
        // SteeringQueued emitted in FIFO order
        let queued: Vec<String> = sink
            .0
            .iter()
            .filter_map(|e| match e {
                Event::SteeringQueued(n) => Some(n.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(queued, notes);
    }

    #[test]
    fn steering_drain_ignores_disconnected_channel() {
        let (tx, rx) = mpsc::channel::<String>();
        drop(tx);
        assert!(drain_steering(&rx).is_empty());
    }

    #[test]
    fn allow_destructive_note_matching() {
        assert!(is_allow_destructive("allow destructive"));
        assert!(is_allow_destructive("  ALLOW DESTRUCTIVE  "));
        assert!(!is_allow_destructive("allow destructively"));
        assert!(!is_allow_destructive("please allow destructive commands"));
    }

    #[test]
    fn abort_flag_aborts_at_boundary_like_budget_abort() {
        let tmp = tempfile::tempdir().unwrap();
        let spec = tmp.path().join("s.md");
        std::fs::write(&spec, "spec text\ncheck: true\n").unwrap();

        let (stx, srx) = mpsc::channel();
        drop(stx);
        let controls = Controls {
            abort: Arc::new(AtomicBool::new(true)),
            steering_rx: srx,
        };
        let cfg = RunConfig {
            cwd: tmp.path().to_path_buf(),
            spec_path: spec,
            goal: "x".to_string(),
            model: "test-model".to_string(),
            max_iters: 5,
            max_minutes: 10,
            resume: false,
            controls,
            risk_gate: false,
            bash_timeout: Duration::from_secs(tools::BASH_TIMEOUT_SECS),
            mcp_config: None,
            // Tests must never pick up the developer's ~/.config/chug/mcp.json.
            mcp_off: true,
        };
        let client = Client::new_without_credentials("test-model").unwrap();
        let mut sink = RecordingSink::default();
        // Noop sink: observability off → the run path must be untouched.
        let code = run_loop(cfg, client, None, &mut sink, &observ::Sink::Noop).unwrap();

        assert_eq!(code, 1);
        assert!(matches!(
            sink.0.iter().find(|e| matches!(e, Event::Aborted { .. })),
            Some(Event::Aborted { reason }) if reason == "operator abort"
        ));
        // identical abort path to budgets: freshest ledger pushed, then abort
        let aborted_idx = sink
            .0
            .iter()
            .position(|e| matches!(e, Event::Aborted { .. }))
            .unwrap();
        assert!(matches!(&sink.0[aborted_idx - 1], Event::LedgerChanged(_)));
        // aborted at the boundary before any LLM call
        assert!(!sink.0.iter().any(|e| matches!(e, Event::ModelText(_))));
        assert!(!sink.0.iter().any(|e| matches!(e, Event::Iteration { .. })));
    }

    // ---------- T3: fresh-run ledger archiving ----------

    /// A RunConfig that aborts at the first iteration boundary: the whole
    /// startup path (seeding, archiving, first-message append) runs, but no
    /// LLM call is ever made.
    fn aborted_run_config(tmp: &tempfile::TempDir, spec: &Path, resume: bool) -> RunConfig {
        let (stx, srx) = mpsc::channel();
        drop(stx);
        RunConfig {
            cwd: tmp.path().to_path_buf(),
            spec_path: spec.to_path_buf(),
            goal: "x".to_string(),
            model: "test-model".to_string(),
            max_iters: 5,
            max_minutes: 10,
            resume,
            controls: Controls {
                abort: Arc::new(AtomicBool::new(true)),
                steering_rx: srx,
            },
            risk_gate: false,
            bash_timeout: Duration::from_secs(tools::BASH_TIMEOUT_SECS),
            mcp_config: None,
            mcp_off: true,
        }
    }

    fn write_spec(tmp: &tempfile::TempDir) -> PathBuf {
        let spec = tmp.path().join("s.md");
        std::fs::write(&spec, "spec text\ncheck: true\n").unwrap();
        spec
    }

    fn ledger_archives(tmp: &tempfile::TempDir) -> Vec<PathBuf> {
        let dir = tmp.path().join(".chug");
        if !dir.exists() {
            return Vec::new();
        }
        let mut out: Vec<PathBuf> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| {
                let path = e.unwrap().path();
                let name = path.file_name().unwrap().to_string_lossy().to_string();
                (name.starts_with("LEDGER-") && name.ends_with(".md")).then_some(path)
            })
            .collect();
        out.sort();
        out
    }

    #[test]
    fn fresh_run_archives_foreign_ledger_and_reseeds() {
        let tmp = tempfile::tempdir().unwrap();
        let spec = write_spec(&tmp);
        std::fs::write(
            tmp.path().join("LEDGER.md"),
            "# Ledger\n\n## Done\n- OLD PROJECT goal met\n",
        )
        .unwrap();

        let client = Client::new_without_credentials("test-model").unwrap();
        let mut sink = RecordingSink::default();
        let code = run_loop(
            aborted_run_config(&tmp, &spec, false),
            client,
            None,
            &mut sink,
            &observ::Sink::Noop,
        )
        .unwrap();
        assert_eq!(code, 1, "aborted at the first boundary");

        let archives = ledger_archives(&tmp);
        assert_eq!(archives.len(), 1, "exactly one ledger archive: {archives:?}");
        assert!(
            std::fs::read_to_string(&archives[0])
                .unwrap()
                .contains("OLD PROJECT goal met")
        );
        assert_eq!(
            ledger::read(tmp.path()).unwrap(),
            ledger::SEED,
            "fresh run starts on the pristine seed"
        );
    }

    #[test]
    fn fresh_run_leaves_pristine_seed_ledger_alone() {
        let tmp = tempfile::tempdir().unwrap();
        let spec = write_spec(&tmp);
        std::fs::write(tmp.path().join("LEDGER.md"), ledger::SEED).unwrap();

        let client = Client::new_without_credentials("test-model").unwrap();
        let mut sink = RecordingSink::default();
        run_loop(
            aborted_run_config(&tmp, &spec, false),
            client,
            None,
            &mut sink,
            &observ::Sink::Noop,
        )
        .unwrap();

        assert!(ledger_archives(&tmp).is_empty(), "no archive for the seed");
        assert_eq!(ledger::read(tmp.path()).unwrap(), ledger::SEED);
    }

    #[test]
    fn resume_run_never_archives_ledger() {
        let tmp = tempfile::tempdir().unwrap();
        let spec = write_spec(&tmp);
        let foreign = "# Ledger\n\n## Done\n- previous run state\n";
        std::fs::write(tmp.path().join("LEDGER.md"), foreign).unwrap();

        let client = Client::new_without_credentials("test-model").unwrap();
        let mut sink = RecordingSink::default();
        run_loop(
            aborted_run_config(&tmp, &spec, true),
            client,
            None,
            &mut sink,
            &observ::Sink::Noop,
        )
        .unwrap();

        assert!(ledger_archives(&tmp).is_empty(), "--resume never archives");
        assert_eq!(ledger::read(tmp.path()).unwrap(), foreign, "ledger kept");
    }

    #[cfg(unix)]
    #[test]
    fn fresh_run_warns_and_proceeds_when_ledger_archive_fails() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let spec = write_spec(&tmp);
        let foreign = "# Ledger\n\n## Done\n- stuck foreign ledger\n";
        std::fs::write(tmp.path().join("LEDGER.md"), foreign).unwrap();
        // The rename needs write permission on the source dir (cwd); with cwd
        // read-only but .chug/ writable the archive fails while the run can
        // still proceed and append its transcript.
        std::fs::create_dir(tmp.path().join(".chug")).unwrap();
        std::fs::set_permissions(tmp.path(), std::fs::Permissions::from_mode(0o555)).unwrap();

        let client = Client::new_without_credentials("test-model").unwrap();
        let mut sink = RecordingSink::default();
        let result = run_loop(
            aborted_run_config(&tmp, &spec, false),
            client,
            None,
            &mut sink,
            &observ::Sink::Noop,
        );
        std::fs::set_permissions(tmp.path(), std::fs::Permissions::from_mode(0o755)).unwrap();

        assert_eq!(result.unwrap(), 1, "run proceeds despite the failed archive");
        assert_eq!(ledger::read(tmp.path()).unwrap(), foreign, "ledger kept as-is");
    }

    // ---------- MCP integration (fake echo server, no network) ----------

    /// LLM double that records the tools array it was offered, like
    /// ScriptedLlm but for the tool schemas (which the driver composes).
    struct ToolRecordingLlm {
        responses: std::collections::VecDeque<Value>,
        recorded_tools: Vec<Vec<Value>>,
    }

    impl ToolRecordingLlm {
        fn new(responses: Vec<Value>) -> Self {
            ToolRecordingLlm {
                responses: responses.into(),
                recorded_tools: Vec::new(),
            }
        }
    }

    impl Llm for ToolRecordingLlm {
        fn complete(
            &mut self,
            _system: &str,
            _messages: &[Message],
            tools: &[Value],
            _obs: &crate::api::ObsCtx<'_>,
        ) -> anyhow::Result<crate::api::Response> {
            self.recorded_tools.push(tools.to_vec());
            self.responses
                .pop_front()
                .map(|body| crate::api::Response { body })
                .ok_or_else(|| anyhow::anyhow!("no scripted response left"))
        }

        fn set_model(&mut self, _model: &str) {}
    }

    /// Fake MCP echo server (same script family as mcp.rs's tests) configured
    /// via mcp.json in the cwd.
    fn write_echo_server(dir: &Path) {
        let py = dir.join("fake_srv.py");
        std::fs::write(
            &py,
            r#"
import sys, json
def send(o):
    sys.stdout.write(json.dumps(o) + "\n"); sys.stdout.flush()
for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    req = json.loads(line)
    if "method" not in req or "id" not in req:
        continue
    m, i = req["method"], req["id"]
    if m == "initialize":
        send({"jsonrpc": "2.0", "id": i, "result": {"protocolVersion": "2025-06-18", "capabilities": {"tools": {}}, "serverInfo": {"name": "fake", "version": "0"}}})
    elif m == "tools/list":
        send({"jsonrpc": "2.0", "id": i, "result": {"tools": [{"name": "echo", "description": "Echo the arguments back", "inputSchema": {"type": "object", "properties": {"text": {"type": "string"}}}}]}})
    elif m == "tools/call":
        send({"jsonrpc": "2.0", "id": i, "result": {"content": [{"type": "text", "text": "echo: " + json.dumps(req["params"]["arguments"])}], "isError": False}})
"#,
        )
        .unwrap();
        let cfg = json!({
            "mcpServers": {
                "fake": {"command": "python3", "args": [py.to_string_lossy()]}
            }
        });
        std::fs::write(dir.join("mcp.json"), cfg.to_string()).unwrap();
    }

    // ---------- observability wiring (SPEC-8) ----------

    /// A live observability sink wired to a counting transport: `sink.shutdown()`
    /// flushes everything, then `transport.events()` returns what was emitted.
    fn test_obs() -> (std::sync::Arc<observ::testing::CountingTransport>, observ::Sink) {
        let transport = observ::testing::CountingTransport::new();
        let sink = observ::testing::test_sink(transport.clone());
        (transport, sink)
    }

    fn events_of_kind(
        transport: &observ::testing::CountingTransport,
        kind: &str,
    ) -> Vec<Value> {
        transport
            .events()
            .into_iter()
            .filter(|e| e["type"] == kind)
            .collect()
    }

    fn tool_use_response(name: &str, input: Value) -> Value {
        json!({
            "stop_reason": "tool_use",
            "usage": {"input_tokens": 10, "output_tokens": 5},
            "content": [{"type": "tool_use", "id": "tu_1", "name": name, "input": input}],
        })
    }

    fn text_only_response(text: &str) -> Value {
        json!({
            "stop_reason": "end_turn",
            "usage": {"input_tokens": 10, "output_tokens": 5},
            "content": [{"type": "text", "text": text}],
        })
    }

    fn ctx_for<'a>(
        tmp: &'a tempfile::TempDir,
        mode: Mode,
        controls: &'a Controls,
        update_rx: &'a Receiver<SlashUpdate>,
        trace: Option<&'a str>,
        obs: &'a observ::Sink,
    ) -> LoopCtx<'a> {
        LoopCtx {
            cwd: tmp.path(),
            mode,
            controls,
            updates: update_rx,
            bash_timeout: Duration::from_secs(1),
            trace,
            obs,
        }
    }

    fn knobs_with(max_iters: u32) -> TurnKnobs {
        TurnKnobs {
            spec_path: None,
            goal: None,
            check_cmd: None,
            max_iters,
            max_minutes: 120,
        }
    }

    fn tool_result_text(messages: &[Message]) -> Option<(String, bool)> {
        messages.iter().rev().find_map(|m| match &m.content[0] {
            ContentBlock::Known(KnownBlock::ToolResult { content, is_error, .. }) => {
                Some((content.as_str().unwrap_or_default().to_string(), *is_error))
            }
            _ => None,
        })
    }

    #[test]
    fn mcp_schemas_merged_and_mcp_tool_use_routed_to_registry() {
        let tmp = tempfile::tempdir().unwrap();
        write_echo_server(tmp.path());
        let mut mcp =
            McpRegistry::new(tmp.path(), false, None).expect("registry with fake server");
        assert!(!mcp.tool_schemas().is_empty(), "fake server must register tools");

        let mut client = ToolRecordingLlm::new(vec![
            json!({
                "stop_reason": "tool_use",
                "usage": {"input_tokens": 1, "output_tokens": 1},
                "content": [{"type": "tool_use", "id": "tu_1", "name": "mcp__fake__echo", "input": {"text": "hello mcp"}}]
            }),
            json!({
                "stop_reason": "end_turn",
                "usage": {"input_tokens": 1, "output_tokens": 1},
                "content": [{"type": "text", "text": "done"}]
            }),
        ]);
        let mut messages = vec![Message::user(vec![ContentBlock::text_block(
            "call the echo tool".to_string(),
        )])];
        let mut sink = RecordingSink::default();
        let reason = run_turn(
            tmp.path(),
            &mut client,
            &mut None,
            &mut messages,
            &Controls::detached(),
            &mpsc::channel().1,
            &mut knobs_with(5),
            Duration::from_secs(tools::BASH_TIMEOUT_SECS),
            &mut mcp,
            None,
            &observ::Sink::Noop,
            &mut sink,
        )
        .unwrap();
        assert_eq!(reason, TurnEndReason::Completed);

        // The tools array the model saw merges MCP schemas with the built-ins.
        let offered = &client.recorded_tools[0];
        assert!(
            offered.iter().any(|t| t["name"] == "mcp__fake__echo"),
            "mcp schema missing: {offered:?}"
        );
        assert!(offered.iter().any(|t| t["name"] == "bash"));

        // The mcp__-prefixed tool_use was routed to the registry and the
        // model saw the echoed content.
        let (content, is_error) = tool_result_text(&messages).expect("tool result in transcript");
        assert!(!is_error, "{content}");
        assert!(content.contains(r#""text": "hello mcp""#), "{content}");
    }

    #[test]
    fn empty_registry_is_a_noop_on_the_tools_array() {
        let tmp = tempfile::tempdir().unwrap();
        // mcp_off: guaranteed-empty registry even if the developer's machine
        // has ~/.config/chug/mcp.json.
        let mut mcp = McpRegistry::new(tmp.path(), true, None).unwrap();
        let mut client = ToolRecordingLlm::new(vec![json!({
            "stop_reason": "end_turn",
            "usage": {"input_tokens": 1, "output_tokens": 1},
            "content": [{"type": "text", "text": "done"}]
        })]);
        let mut messages = vec![Message::user(vec![ContentBlock::text_block("hi")])];
        let mut sink = RecordingSink::default();
        run_turn(
            tmp.path(),
            &mut client,
            &mut None,
            &mut messages,
            &Controls::detached(),
            &mpsc::channel().1,
            &mut knobs_with(5),
            Duration::from_secs(tools::BASH_TIMEOUT_SECS),
            &mut mcp,
            None,
            &observ::Sink::Noop,
            &mut sink,
        )
        .unwrap();

        let offered = &client.recorded_tools[0];
        let names: Vec<&str> = offered
            .iter()
            .filter_map(|t| t["name"].as_str())
            .collect();
        let baseline: Vec<String> = tools::tool_schemas()
            .into_iter()
            .filter_map(|t| t["name"].as_str().map(str::to_string))
            .collect();
        // Byte-identical tool list: nothing added, nothing removed.
        assert_eq!(names, baseline);
        assert!(names.iter().all(|n| !n.starts_with("mcp__")));
    }

    // ---------- observability tests (SPEC-8) ----------

    #[test]
    fn observability_run_emits_span_goal_event_and_completion_scores() {
        let tmp = tempfile::tempdir().unwrap();
        let (transport, sink) = test_obs();
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for(&tmp, Mode::Autonomous, &controls, &urx, Some("chug-test0001"), &sink);
        let mut knobs = knobs_with(40);
        let mut llm = ScriptedLlm::new(vec![
            tool_use_response("read_file", json!({"path": "missing.txt"})),
            tool_use_response("goal_complete", json!({"summary": "did it"})),
        ]);
        let mut gate = None;
        let mut messages = Vec::new();
        let outcome = drive_loop(
            &ctx,
            &mut knobs,
            &mut llm,
            &mut gate,
            &mut messages,
            Some("check: true".to_string()),
            &mut RecordingSink::default(),
            // Forced-off registry: MCP is a strict no-op in these tests.
            &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
        )
        .unwrap();
        assert!(matches!(outcome, DriveOutcome::RunFinished(0)));
        sink.shutdown();

        // One span per tool call — read_file (missing file → error result)
        // and goal_complete (ok) — with ok / is_error metadata.
        let spans = events_of_kind(&transport, "span-create");
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0]["body"]["name"], "read_file");
        assert_eq!(spans[0]["body"]["metadata"]["ok"], false);
        assert_eq!(spans[0]["body"]["metadata"]["is_error"], true);
        assert_eq!(spans[1]["body"]["name"], "goal_complete");
        assert_eq!(spans[1]["body"]["metadata"]["ok"], true);
        assert_eq!(spans[1]["body"]["metadata"]["is_error"], false);

        // Goal accepted event carrying the summary.
        let accepted = events_of_kind(&transport, "event-create");
        assert_eq!(accepted.len(), 1);
        assert_eq!(accepted[0]["body"]["name"], "goal_accepted");
        assert_eq!(accepted[0]["body"]["metadata"]["summary"], "did it");

        // Outcome (completed) + iterations (2 LLM iterations) scores.
        let scores = events_of_kind(&transport, "score-create");
        assert_eq!(scores.len(), 2);
        assert_eq!(scores[0]["body"]["name"], "outcome");
        assert_eq!(scores[0]["body"]["stringValue"], "completed");
        assert_eq!(scores[0]["body"]["dataType"], "CATEGORICAL");
        assert_eq!(scores[1]["body"]["name"], "iterations");
        assert_eq!(scores[1]["body"]["value"], 2);
        assert_eq!(scores[1]["body"]["dataType"], "NUMERIC");

        // The run trace is finished via the trace-create upsert.
        let finishes = events_of_kind(&transport, "trace-create");
        assert_eq!(finishes.len(), 1);
        assert_eq!(finishes[0]["body"]["id"], "chug-test0001");
        assert_eq!(finishes[0]["body"]["metadata"]["outcome"], "completed");
        assert_eq!(finishes[0]["body"]["metadata"]["iterations"], 2);
    }

    #[test]
    fn observability_budget_abort_scores_budget_outcome_and_finishes_trace() {
        let tmp = tempfile::tempdir().unwrap();
        let (transport, sink) = test_obs();
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for(&tmp, Mode::Autonomous, &controls, &urx, Some("chug-test0002"), &sink);
        let mut knobs = knobs_with(0); // budget exhausted before iteration 1
        let mut llm = ScriptedLlm::new(vec![]);
        let mut gate = None;
        let mut messages = Vec::new();
        let outcome = drive_loop(
            &ctx,
            &mut knobs,
            &mut llm,
            &mut gate,
            &mut messages,
            None,
            &mut RecordingSink::default(),
            // Forced-off registry: MCP is a strict no-op in these tests.
            &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
        )
        .unwrap();
        assert!(matches!(outcome, DriveOutcome::RunFinished(1)));
        sink.shutdown();

        let aborts = events_of_kind(&transport, "event-create");
        assert_eq!(aborts.len(), 1);
        assert_eq!(aborts[0]["body"]["name"], "abort");
        assert_eq!(
            aborts[0]["body"]["metadata"]["reason"],
            "iteration budget exceeded"
        );
        let scores = events_of_kind(&transport, "score-create");
        assert_eq!(scores[0]["body"]["stringValue"], "budget");
        assert_eq!(scores[1]["body"]["value"], 0);
        let finishes = events_of_kind(&transport, "trace-create");
        assert_eq!(finishes[0]["body"]["metadata"]["outcome"], "budget");
    }

    #[test]
    fn observability_chat_turns_neither_score_nor_finish_the_session_trace() {
        let tmp = tempfile::tempdir().unwrap();
        let (transport, sink) = test_obs();
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for(&tmp, Mode::Chat, &controls, &urx, Some("chug-test0003"), &sink);
        // Budget exhausted immediately: the turn aborts but the SESSION
        // trace stays open (per-session lifecycle, not per-turn).
        let mut knobs = knobs_with(0);
        let mut llm = ScriptedLlm::new(vec![]);
        let mut gate = None;
        let mut messages = Vec::new();
        let outcome = drive_loop(
            &ctx,
            &mut knobs,
            &mut llm,
            &mut gate,
            &mut messages,
            None,
            &mut RecordingSink::default(),
            // Forced-off registry: MCP is a strict no-op in these tests.
            &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
        )
        .unwrap();
        assert!(matches!(
            outcome,
            DriveOutcome::TurnEnded(TurnEndReason::BudgetExceeded)
        ));
        sink.shutdown();

        // The abort event is emitted, but no scores and no trace finish —
        // the session continues and finishing is the chat loop's job.
        let aborts = events_of_kind(&transport, "event-create");
        assert_eq!(aborts[0]["body"]["name"], "abort");
        assert!(events_of_kind(&transport, "score-create").is_empty());
        assert!(events_of_kind(&transport, "trace-create").is_empty());
    }

    #[test]
    fn observability_risk_gate_verdicts_become_events() {
        let tmp = tempfile::tempdir().unwrap();
        let (transport, sink) = test_obs();
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();

        // Phase 1: judge blocks → verdict "blocked" event.
        let ctx = ctx_for(&tmp, Mode::Chat, &controls, &urx, Some("chug-test0004"), &sink);
        let mut knobs = knobs_with(40);
        let mut gate = Some(RiskGate::new(
            Box::new(CannedJudge("destructive", 0.9)),
            tmp.path(),
        ));
        let mut llm = ScriptedLlm::new(vec![
            tool_use_response("bash", json!({"command": "rm -rf site"})),
            text_only_response("understood"),
        ]);
        let mut messages = Vec::new();
        let outcome = drive_loop(
            &ctx,
            &mut knobs,
            &mut llm,
            &mut gate,
            &mut messages,
            None,
            &mut RecordingSink::default(),
            // Forced-off registry: MCP is a strict no-op in these tests.
            &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
        )
        .unwrap();
        assert!(matches!(outcome, DriveOutcome::TurnEnded(TurnEndReason::Completed)));

        // Phase 2: judge allows → verdict "allowed" event (same trace).
        let ctx = ctx_for(&tmp, Mode::Chat, &controls, &urx, Some("chug-test0004"), &sink);
        let mut knobs = knobs_with(40);
        let mut gate = Some(RiskGate::new(
            Box::new(CannedJudge("safe", 0.1)),
            tmp.path(),
        ));
        let mut llm = ScriptedLlm::new(vec![
            tool_use_response("bash", json!({"command": "ls"})),
            text_only_response("listed"),
        ]);
        let mut messages = Vec::new();
        drive_loop(
            &ctx,
            &mut knobs,
            &mut llm,
            &mut gate,
            &mut messages,
            None,
            &mut RecordingSink::default(),
            // Forced-off registry: MCP is a strict no-op in these tests.
            &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
        )
        .unwrap();
        sink.shutdown();

        let gate_events: Vec<Value> = transport
            .events()
            .into_iter()
            .filter(|e| e["type"] == "event-create" && e["body"]["name"] == "risk_gate")
            .collect();
        assert_eq!(gate_events.len(), 2);
        assert_eq!(gate_events[0]["body"]["metadata"]["verdict"], "blocked");
        assert_eq!(gate_events[0]["body"]["metadata"]["command"], "rm -rf site");
        assert_eq!(gate_events[1]["body"]["metadata"]["verdict"], "allowed");
        assert_eq!(gate_events[1]["body"]["metadata"]["command"], "ls");
    }

    #[test]
    fn observability_steering_notes_become_events() {
        let tmp = tempfile::tempdir().unwrap();
        let (transport, sink) = test_obs();
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let (steer_tx, steer_rx) = mpsc::channel();
        steer_tx.send("focus on tests".to_string()).unwrap();
        let controls = Controls {
            abort: Arc::new(AtomicBool::new(false)),
            steering_rx: steer_rx,
        };
        let ctx = ctx_for(&tmp, Mode::Chat, &controls, &urx, Some("chug-test0005"), &sink);
        let mut knobs = knobs_with(40);
        let mut llm = ScriptedLlm::new(vec![text_only_response("ok")]);
        let mut gate = None;
        let mut messages = Vec::new();
        drive_loop(
            &ctx,
            &mut knobs,
            &mut llm,
            &mut gate,
            &mut messages,
            None,
            &mut RecordingSink::default(),
            // Forced-off registry: MCP is a strict no-op in these tests.
            &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
        )
        .unwrap();
        sink.shutdown();

        let steering: Vec<Value> = transport
            .events()
            .into_iter()
            .filter(|e| e["type"] == "event-create" && e["body"]["name"] == "steering")
            .collect();
        assert_eq!(steering.len(), 1);
        assert_eq!(steering[0]["body"]["metadata"]["note"], "focus on tests");
    }

    /// Minimal judge double: always returns the same canned verdict.
    struct CannedJudge(&'static str, f64);

    impl crate::riskgate::Judge for CannedJudge {
        fn judge(&mut self, _command: &str) -> Result<crate::riskgate::Verdict, String> {
            Ok(crate::riskgate::Verdict {
                choice: self.0.to_string(),
                p_destructive: self.1,
            })
        }
    }
}
