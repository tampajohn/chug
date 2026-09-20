use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use anyhow::{Context, bail};
use serde_json::{Value, json};

use crate::api::{Client, ContentBlock, KnownBlock, Llm, Message};
use crate::events::{Event, EventSink, TurnEndReason};
use crate::ledger;
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
    run_loop(cfg, client, gate, sink)
}

fn run_loop(
    cfg: RunConfig,
    client: Client,
    mut gate: Option<RiskGate>,
    sink: &mut dyn EventSink,
) -> anyhow::Result<i32> {
    let mut client = client;
    ledger::ensure_seeded(&cfg.cwd)?;

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
    };
    match drive_loop(
        &ctx,
        &mut knobs,
        &mut client,
        &mut gate,
        &mut messages,
        Some(initial_spec),
        sink,
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
    sink: &mut dyn EventSink,
) -> anyhow::Result<TurnEndReason> {
    let ctx = LoopCtx {
        cwd,
        mode: Mode::Chat,
        controls,
        updates,
    };
    match drive_loop(&ctx, knobs, client, gate, messages, None, sink)? {
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
) -> anyhow::Result<DriveOutcome> {
    let tool_schemas = tools::tool_schemas();
    let tool_ctx = ToolCtx {
        cwd: ctx.cwd.to_path_buf(),
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
                sink,
            );
        }
        if start.elapsed() >= Duration::from_secs(knobs.max_minutes.saturating_mul(60)) {
            return abort_exit(
                ctx,
                "time budget exceeded",
                TurnEndReason::BudgetExceeded,
                1,
                sink,
            );
        }
        if ctx.controls.abort.load(Ordering::SeqCst) {
            let reason = match ctx.mode {
                Mode::Autonomous => "operator abort",
                Mode::Chat => "operator interrupt",
            };
            return abort_exit(ctx, reason, TurnEndReason::Interrupted, 1, sink);
        }

        // Steering notes queued by the operator are consumed here, at the
        // iteration boundary, before the next LLM call.
        let notes = drain_steering(&ctx.controls.steering_rx);
        if !notes.is_empty() {
            append_steering_notes(ctx.cwd, messages, &notes, sink)?;
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
        let resp = client.complete(&system, messages, &tool_schemas)?;

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
            let result = if name == "bash" {
                if let Some(gate) = gate.as_mut() {
                    match input
                        .get("command")
                        .and_then(Value::as_str)
                        .map(|command| gate.check(command, sink))
                    {
                        Some(GateDecision::Blocked(msg)) => ToolResult {
                            content: msg,
                            is_error: true,
                        },
                        // Allowed, no command field, or no gate: execute.
                        _ => tools::dispatch(&tool_ctx, name, input),
                    }
                } else {
                    tools::dispatch(&tool_ctx, name, input)
                }
            } else {
                tools::dispatch(&tool_ctx, name, input)
            };
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
                    sink.emit(Event::GoalAccepted { summary });
                    if ctx.mode == Mode::Chat {
                        // Keep full history so the next turn continues the
                        // same conversation.
                        messages.push(assistant);
                        messages.push(user_msg);
                        return Ok(DriveOutcome::TurnEnded(TurnEndReason::GoalAccepted));
                    }
                    return Ok(DriveOutcome::RunFinished(0));
                }
                VerifyOutcome::Failed(output) => {
                    sink.emit(Event::GoalRejected {
                        reason: "check command failed".to_string(),
                    });
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
            return abort_exit(ctx, "stuck: repeated error", TurnEndReason::Interrupted, 2, sink);
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

/// Shared abort tail: push the freshest ledger, then the Aborted event, then
/// map to the mode-appropriate outcome (exit code for `run`, turn-end for
/// chat). `turn_reason` is only used in chat mode.
fn abort_exit(
    ctx: &LoopCtx,
    reason: &str,
    turn_reason: TurnEndReason,
    code: i32,
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
    Ok(match ctx.mode {
        Mode::Autonomous => DriveOutcome::RunFinished(code),
        Mode::Chat => DriveOutcome::TurnEnded(turn_reason),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
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
        };
        let client = Client::new_without_credentials("test-model").unwrap();
        let mut sink = RecordingSink::default();
        let code = run_loop(cfg, client, None, &mut sink).unwrap();

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
}
