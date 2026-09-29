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
use crate::driver_lock;
use crate::eventlog;
use crate::events::{BudgetExceeded, Event, EventSink, TurnEndReason};
use crate::hooks;
use crate::ledger;
use crate::mcp::McpRegistry;
use crate::observ;
use crate::permissions;
use crate::riskgate::{GateDecision, LayaJudge, RiskGate};
use crate::tools::{self, ToolCtx, ToolResult};
use crate::transcript;
use crate::trim;

pub const DEFAULT_MODEL: &str = "claude-sonnet-4-6";

const PREAMBLE: &str = "You are chug, an autonomous coding agent driven by a code loop, not a conversation. Work in small, verified steps. After each step, update LEDGER.md with the update_ledger tool (what is done, what is next, any blockers). Verify your work by running builds/tests before claiming success. Never declare the goal complete without running the relevant checks. When the goal is fully met and verified, call the goal_complete tool with a short summary.";

/// Chat-mode harness preamble: the user is present and drives turn by turn.
const CHAT_PREAMBLE: &str = "You are chug in an interactive session; work the user's current objective; when it is done, stop — the user will give the next objective. Work in small, verified steps. After each step, update LEDGER.md with the update_ledger tool (what is done, what is next, any blockers). Verify your work by running builds/tests before claiming success. When the objective is fully met and verified, call the goal_complete tool with a short summary; otherwise simply stop.";

const KICK: &str = "Ledger and goal are above. You have not called goal_complete. Continue with the next ledger item, or update the ledger if the plan changed.";

const STUCK_WINDOW: usize = 3;

/// T25: the error-leg preview window. Failure bytes cluster at the END of
/// command output (cargo's `failures:` list, rustc's `error[Exxxx]` blocks),
/// and the pre-T25 flat 500-char head window lost the failing test's name
/// (cycle-9 eval N1), so error tool results keep the LAST
/// [`ERROR_PREVIEW_TAIL_CHARS`] chars of their content in the emitted
/// `Event::ToolResult` preview. Ok results keep the T10-era 500-char head,
/// byte-identical.
const ERROR_PREVIEW_TAIL_CHARS: usize = 2000;

/// T13: the one-shot budget-low warning fires when this many iterations (or
/// this many wall-clock seconds — [`WARN_REMAINING_SECS`]) remain, giving the
/// model a chance to commit and wrap up before the abort at the loop top.
/// T18: 5→8 on cycle-4 J6 evidence — 3 of the last 4 child runs died at the
/// iteration ceiling with the wrap unfinished (final gates + commit + verdict
/// need 4–6 iterations of runway; a 5-iteration warning left none for a slow
/// gate run or a self-inflicted rework loop).
const WARN_REMAINING_ITERS: u32 = 8;
/// T13: wall-clock seconds remaining that trigger the one-shot warning
/// (5 minutes).
const WARN_REMAINING_SECS: u64 = 300;
/// T15: remaining token budget (cumulative input+output) that triggers the
/// one-shot warning, same shape as the iteration/seconds legs.
const WARN_REMAINING_TOKENS: u64 = 50_000;

/// T38: the advisory injected as a user message whenever a response comes
/// back truncated at the API output-token ceiling (`stop_reason=max_tokens`).
/// Origin: cycle-16, the T37 glm impl child died 50/50 on the truncated-write
/// saga — a 987-line `write_file` in one call is impossible under the
/// 8192-token ceiling — and the model never got told why its file kept
/// arriving short. Fires on every truncated response (no one-shot latch);
/// the load-bearing tokens (`max_tokens`, `stop_reason`, and the chunking
/// remedy naming `write_file` + `edit_file`) are pinned by tests.
pub(crate) const OUTPUT_TRUNCATED_ADVISORY: &str = "chug: output truncated — the previous response hit the API output-token ceiling (stop_reason=max_tokens). If you were writing a file, split it: write_file the first chunk, then append with edit_file (or bash heredoc) in smaller pieces.";

pub struct RunConfig {
    pub cwd: PathBuf,
    pub spec_path: PathBuf,
    pub goal: String,
    pub model: String,
    pub max_iters: u32,
    pub max_minutes: u64,
    /// Cumulative token budget across the run: input+output tokens summed
    /// over every API response. `0` = unlimited (the default; no ceiling,
    /// no warning leg — pre-T15 behavior exactly).
    pub max_tokens: u64,
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
    /// F9 phase 2a (T117): the `.chug/commands/` pack the goal was expanded
    /// from — `Some` when the CLI boundary resolved `--goal "/name args"`
    /// through a pack, `None` for a literal goal. Recorded on the run's
    /// `run_start` line (`goal_pack`, always present) so a harvested stream
    /// shows the goal's provenance alongside its hash.
    pub goal_pack: Option<String>,
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

/// The three loop personalities: `run` (process exits on completion), `chat`
/// (a turn ends, control returns to the user), and `plan` (T73: read-only
/// planning — process exits 0 on `submit_plan`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    Autonomous,
    Chat,
    Plan,
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
    /// Per-turn (or per-run) cumulative token budget (input+output across
    /// the invocation's API responses); `0` = unlimited.
    pub max_tokens: u64,
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
#[derive(Debug)]
pub(crate) enum DriveOutcome {
    /// Autonomous run finished; value is the process exit code.
    RunFinished(i32),
    /// Chat turn ended; the chat loop returns to idle.
    TurnEnded(TurnEndReason),
}

/// Shared, mode-independent context for one drive_loop invocation.
pub(crate) struct LoopCtx<'a> {
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
    /// T73 plan mode only: where `submit_plan` writes the plan (`None` → the
    /// plan surfaces on stdout). Always `None` in run/chat modes.
    plan_out: Option<&'a Path>,
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
    // T55: same-cwd mutual exclusion. Acquired BEFORE the resume/fresh
    // housekeeping below, after ensuring `.chug/` exists (acquire creates
    // it), so the first transcript/ledger rotation is already serialized.
    // The guard lives for the whole run and releases on every normal exit
    // path — goal acceptance, abort, budget death, error, panic unwind;
    // SIGKILL leaves a stale lock by design and the next acquirer reclaims
    // it (see driver_lock's module comment). A held lock returns an error
    // here, which surfaces as the startup-error exit in main.rs. Chat mode
    // never reaches this function, so it never touches the lock.
    let _driver_lock: driver_lock::Guard = match driver_lock::acquire(&cfg.cwd) {
        Ok(guard) => guard,
        Err(msg) => bail!("{msg}"),
    };
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
        // T7: same treatment for the transcript, before the first append —
        // a later --resume must never splice foreign sessions into context.
        match transcript::rotate_fresh(&cfg.cwd) {
            archive::Outcome::Archived(path) => {
                eprintln!("chug: archived previous transcript to {}", path.display());
            }
            archive::Outcome::Failed(why) => {
                eprintln!(
                    "chug: warning: could not archive previous transcript ({why}); appending to it"
                );
            }
            archive::Outcome::Skipped => {}
        }
        // T10: the events log shares the transcript's lifecycle — fresh
        // runs start a new file; --resume keeps appending.
        match eventlog::rotate_fresh(&cfg.cwd) {
            archive::Outcome::Archived(path) => {
                eprintln!("chug: archived previous events log to {}", path.display());
            }
            archive::Outcome::Failed(why) => {
                eprintln!(
                    "chug: warning: could not archive previous events log ({why}); appending to it"
                );
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
        max_tokens: cfg.max_tokens,
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
        plan_out: None,
    };
    // T10: first line of the run's events log (model/spec/cwd/mode), plus
    // the configured budget ceilings (T17), the cwd's checkout HEAD (T20,
    // best-effort: a non-repo cwd just leaves both fields null), and the
    // goal's SHA-256 (T115, the child-side half of the delegate integrity
    // comparison). T117: `goal_pack` names the pack the goal was expanded
    // from (the CLI boundary rewrote `--goal "/name args"` into the pack
    // body); `goal_sha256` above hashes that expanded text.
    let head = crate::build_info::resolve_head(&cfg.cwd);
    eventlog::run_start(
        &cfg.cwd,
        "run",
        Some(&cfg.spec_path),
        &cfg.model,
        cfg.max_iters,
        cfg.max_minutes,
        cfg.max_tokens,
        crate::build_info::as_pair(&head),
        Some(&cfg.goal),
        cfg.goal_pack.as_deref(),
    );
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

/// T73: plan-mode config — a read-only planning session in `cwd` that ends
/// when the model calls `submit_plan` (exit 0), or on a budget abort.
pub struct PlanConfig {
    pub cwd: PathBuf,
    /// Optional spec file, same resolution as `run` (absent = no spec section).
    pub spec_path: Option<PathBuf>,
    pub goal: String,
    pub model: String,
    pub max_iters: u32,
    pub max_minutes: u64,
    /// Cumulative token budget across the session (`0` = unlimited), same
    /// shape as run/chat (T15 parity).
    pub max_tokens: u64,
    /// Where `submit_plan` writes the plan; `None` → the plan surfaces on
    /// stdout. Resolved through the cwd sandbox at write time.
    pub out_path: Option<PathBuf>,
    /// F9 phase 2a (T117): the `.chug/commands/` pack the goal was expanded
    /// from — same shape as [`RunConfig::goal_pack`], recorded on the plan
    /// session's `run_start` line.
    pub goal_pack: Option<String>,
}

/// Plan mode entry point: the same shape as `run`, with the plan-mode
/// startup differences — no ledger archiving or seeding (a plan run never
/// writes LEDGER.md), no MCP, no risk gate (there is no bash), and a
/// `run_start` event naming mode "plan".
pub fn run_plan(cfg: PlanConfig, sink: &mut dyn EventSink) -> anyhow::Result<i32> {
    let mut client = Client::new(&cfg.model)?;
    // Plan mode never initializes MCP servers: the registry is the empty one
    // (mcp_off), and drive_loop's plan branch never extends the advertised
    // five-tool list with it. The registry rides the signature (like `&mut
    // dyn Llm`) so tests can drive the REAL loop with a non-empty registry
    // and pin the no-extension guarantee at the loop level.
    let mcp = McpRegistry::new(&cfg.cwd, true, None)?;
    run_plan_loop(cfg, &mut client, sink, observ::global(), mcp)
}

/// The plan loop proper. Drivable in tests with a [`ScriptedLlm`] (the
/// `run_turn` seam pattern): everything a real plan session does — driver
/// lock, fresh rotation, the `run_start` event naming mode "plan", budget
/// enforcement, the submit_plan exit — except the concrete `Client` and the
/// (prod: empty) MCP registry, which arrive as parameters.
fn run_plan_loop(
    cfg: PlanConfig,
    client: &mut dyn Llm,
    sink: &mut dyn EventSink,
    obs: &observ::Sink,
    mut mcp: McpRegistry,
) -> anyhow::Result<i32> {
    // Same same-cwd mutual exclusion as a run: the session appends to the
    // shared transcript/events files, so a concurrent chug in this cwd must
    // not interleave with it (released on every exit path).
    let _driver_lock: driver_lock::Guard = match driver_lock::acquire(&cfg.cwd) {
        Ok(guard) => guard,
        Err(msg) => bail!("{msg}"),
    };
    // Fresh-session rotation, same lifecycle as a fresh run (transcript +
    // events log). Deliberately NO ledger::archive_stale / ensure_seeded:
    // plan mode never touches LEDGER.md (requirement: no bookkeeping writes).
    match transcript::rotate_fresh(&cfg.cwd) {
        archive::Outcome::Archived(path) => {
            eprintln!("chug: archived previous transcript to {}", path.display());
        }
        archive::Outcome::Failed(why) => {
            eprintln!(
                "chug: warning: could not archive previous transcript ({why}); appending to it"
            );
        }
        archive::Outcome::Skipped => {}
    }
    match eventlog::rotate_fresh(&cfg.cwd) {
        archive::Outcome::Archived(path) => {
            eprintln!("chug: archived previous events log to {}", path.display());
        }
        archive::Outcome::Failed(why) => {
            eprintln!(
                "chug: warning: could not archive previous events log ({why}); appending to it"
            );
        }
        archive::Outcome::Skipped => {}
    }
    // The spec must be readable at startup when given (same resolution as
    // run); a plan session may run without one.
    let initial_spec = match &cfg.spec_path {
        Some(path) => Some(
            fs::read_to_string(path)
                .with_context(|| format!("reading spec {}", path.display()))?,
        ),
        None => None,
    };
    let first = Message::user(vec![ContentBlock::text_block(format!(
        "Goal: {}\n\nThe goal (and spec, when given) are in your system prompt. \
         Explore read-only, then call submit_plan with the complete plan.",
        cfg.goal
    ))]);
    transcript::append(&cfg.cwd, &first)?;
    let mut messages = vec![first];

    let head = crate::build_info::resolve_head(&cfg.cwd);
    eventlog::run_start(
        &cfg.cwd,
        "plan",
        cfg.spec_path.as_deref(),
        &cfg.model,
        cfg.max_iters,
        cfg.max_minutes,
        cfg.max_tokens,
        crate::build_info::as_pair(&head),
        Some(&cfg.goal),
        cfg.goal_pack.as_deref(),
    );
    let trace = obs.trace_started(
        &cfg.goal,
        &cfg.model,
        &cfg.cwd.display().to_string(),
        "plan",
        cfg.spec_path
            .as_deref()
            .map(|p| p.to_string_lossy().into_owned())
            .as_deref(),
    );
    let mut knobs = TurnKnobs {
        spec_path: cfg.spec_path.clone(),
        goal: Some(cfg.goal.clone()),
        check_cmd: None, // a plan is not check-verifiable; submit_plan is the exit
        max_iters: cfg.max_iters,
        max_minutes: cfg.max_minutes,
        max_tokens: cfg.max_tokens,
    };
    let (_update_tx, update_rx) = mpsc::channel::<SlashUpdate>();
    let controls = Controls::detached();
    let mut gate: Option<RiskGate> = None;
    let ctx = LoopCtx {
        cwd: &cfg.cwd,
        mode: Mode::Plan,
        controls: &controls,
        updates: &update_rx,
        bash_timeout: Duration::from_secs(tools::BASH_TIMEOUT_SECS),
        trace: trace.as_deref(),
        obs,
        plan_out: cfg.out_path.as_deref(),
    };
    match drive_loop(
        &ctx,
        &mut knobs,
        client,
        &mut gate,
        &mut messages,
        initial_spec,
        sink,
        &mut mcp,
    )? {
        DriveOutcome::RunFinished(code) => Ok(code),
        DriveOutcome::TurnEnded(_) => bail!("chat turn outcome in plan mode"),
    }
}

/// Load the transcript for a resumed session, trimming it first if it is over
/// the token budget so an over-large transcript starts compact.
///
/// T136 crash recovery: a kill mid-tool-batch leaves the transcript's last
/// assistant message holding unanswered `tool_use` blocks (the batch's
/// tool_result user message is appended only after the whole batch
/// finishes), and a crash mid-append can tear the final JSONL line. Both
/// shapes made the transcript unresumable — the endpoint rejects a request
/// whose tool_use blocks carry no tool_result, and a torn line aborted
/// loading entirely. Here: a torn tail is dropped and physically truncated
/// (so later appends cannot merge into the torn bytes), and every
/// unanswered tool_use gets an is_error "interrupted" tool_result, in
/// memory and on disk. The repaired history is the same shape the loop
/// itself would have written had the process survived.
pub fn resume_messages(cwd: &Path) -> anyhow::Result<Vec<Message>> {
    let (mut messages, torn) = transcript::load_with_torn(cwd)?;
    if torn {
        // Physically drop the torn bytes: the next transcript::append would
        // otherwise write onto the same (newline-less) line, merging two
        // messages into one unparsable line.
        transcript::rewrite(cwd, &messages)?;
    }
    repair_interrupted_tools(cwd, &mut messages)?;
    if !messages.is_empty() && trim::transcript_trim(&mut messages) {
        transcript::rewrite(cwd, &messages)?;
    }
    Ok(messages)
}

/// The tool_result content a resumed run receives for a tool whose
/// execution was interrupted by a crash or kill (T136). The tool may have
/// changed files before the interrupt — its real result is gone — so the
/// note routes the model at re-verifying state instead of trusting either
/// a success or a failure that never landed.
pub(crate) const INTERRUPTED_TOOL_NOTE: &str = "[interrupted] chug was killed or crashed while \
this tool was running; its result was never recorded, and any side effects it had already made \
are unknown. Re-check actual state (files, git status, logs) before continuing.";

/// T136: repair a transcript whose last assistant message carries
/// `tool_use` blocks that no later message answers (a kill landed between
/// the assistant append and the post-batch tool_result append). Appends ONE
/// user message with an is_error tool_result per unanswered id — the exact
/// shape the loop writes after an intact batch, so the next request never
/// carries missing tool_results — and persists it to the transcript.
/// Returns whether a repair was appended. Idempotent: a repaired transcript
/// has no unanswered ids and is left byte-identical.
///
/// Invariant the repair relies on (loop-written transcripts): an assistant
/// message with tool_use is always immediately followed by the batch's
/// tool_result user message unless the process died in between — so any
/// unanswered tool_use sits in the LAST assistant message, and appending at
/// the end IS the immediately-following user message the endpoint requires.
pub(crate) fn repair_interrupted_tools(
    cwd: &Path,
    messages: &mut Vec<Message>,
) -> anyhow::Result<bool> {
    let Some((assistant_idx, use_ids)) = messages.iter().enumerate().rev().find_map(|(i, m)| {
        if m.role != "assistant" {
            return None;
        }
        let ids: Vec<&str> = m
            .content
            .iter()
            .filter_map(ContentBlock::tool_use)
            .map(|(id, _, _)| id)
            .collect();
        (!ids.is_empty()).then_some((i, ids))
    }) else {
        return Ok(false);
    };
    // Answered = a tool_result with that id appears in ANY later message.
    let answered: std::collections::HashSet<&str> = messages[assistant_idx + 1..]
        .iter()
        .flat_map(|m| m.content.iter())
        .filter_map(|b| match b {
            ContentBlock::Known(KnownBlock::ToolResult { tool_use_id, .. }) => {
                Some(tool_use_id.as_str())
            }
            _ => None,
        })
        .collect();
    let missing: Vec<&str> = use_ids
        .iter()
        .copied()
        .filter(|id| !answered.contains(id))
        .collect();
    if missing.is_empty() {
        return Ok(false);
    }
    let repair = Message::user(
        missing
            .iter()
            .map(|id| ContentBlock::tool_result_block(id, INTERRUPTED_TOOL_NOTE.to_string(), true))
            .collect(),
    );
    transcript::append(cwd, &repair)?;
    messages.push(repair);
    Ok(true)
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
        plan_out: None,
    };
    match drive_loop(&ctx, knobs, client, gate, messages, None, sink, mcp)? {
        DriveOutcome::TurnEnded(reason) => Ok(reason),
        DriveOutcome::RunFinished(_) => bail!("autonomous run outcome in chat mode"),
    }
}

/// F7 phase 1: the text-delta hook handed to the LLM client for one
/// `complete` call — emits [`Event::ModelTextDelta`] into the driver's sink
/// as the model's text arrives.
///
/// SAFETY (the `Send` façade): the raw sink pointer is dereferenced ONLY
/// inside the hook, and the hook runs synchronously on THIS thread — the
/// caller's thread inside `Client::complete` (the chunk hook fires in
/// `read_body_with_watchdog_progress`'s receive loop, which runs on the thread
/// blocked in `complete()`; no other thread can reach it). The driver clears
/// the hook as soon as the call returns, before any other use of the sink, so
/// the pointer never outlives the exclusive borrow it was taken from and no
/// aliasing ever escapes the call.
struct SinkPtr(*mut (dyn EventSink + 'static));
// SAFETY: the wrapper exists precisely to move the pointer to the hook that
// runs on the installing thread only — see the safety note above.
unsafe impl Send for SinkPtr {}

impl SinkPtr {
    fn emit(&self, delta: &str) {
        // SAFETY: same thread as the install (see the safety note above), and
        // the hook is cleared before the aliased borrow ends.
        let sink: &mut dyn EventSink = unsafe { &mut *(self.0) };
        sink.emit(Event::ModelTextDelta(delta.to_string()));
    }
}

fn model_text_delta_hook(sink: &mut dyn EventSink) -> Box<dyn FnMut(&str) + Send> {
    let ptr: *mut dyn EventSink = sink;
    // SAFETY: erases the borrow's lifetime from the fat pointer. The pointee
    // is only ever touched on the installing thread inside the hook, and the
    // driver clears the hook before the aliased `&mut dyn EventSink` borrow
    // ends — the pointer never escapes that window.
    let ptr: *mut (dyn EventSink + 'static) = unsafe { std::mem::transmute(ptr) };
    let ptr = SinkPtr(ptr);
    // Method-call capture: the closure captures the WHOLE Send wrapper, not
    // its raw-pointer field.
    Box::new(move |delta: &str| {
        ptr.emit(delta);
    })
}

/// The iteration loop shared by `run` and chat turns. Behavior differences:
/// - Autonomous: natural stop triggers the anti-stall kick; aborts return a
///   process exit code. Chat: natural stop ends the turn; aborts return a
///   `TurnEndReason`.
/// - `goal_complete` verification: autonomous parses the spec's `check:` line,
///   chat uses the `/check` command configured in the knobs.
#[allow(clippy::too_many_arguments)]
pub(crate) fn drive_loop(
    ctx: &LoopCtx,
    knobs: &mut TurnKnobs,
    client: &mut dyn Llm,
    gate: &mut Option<RiskGate>,
    messages: &mut Vec<Message>,
    initial_spec: Option<String>,
    sink: &mut dyn EventSink,
    mcp: &mut McpRegistry,
) -> anyhow::Result<DriveOutcome> {
    // T10: tee every event into `.chug/events.jsonl` (best-effort, never
    // aborts) before it reaches the console/TUI sink.
    let mut event_log = eventlog::EventLogSink::new(ctx.cwd, sink);
    let sink = &mut event_log as &mut dyn EventSink;
    // T83: `.chug/hooks.json` loads once per invocation, before the loop —
    // that is what makes the config-error warn+error-line once-per-run. Plan
    // mode is structurally excluded (its tool contract is exactly the five
    // read-only tools — hooks would be a sixth behavior), so it runs with
    // zero hooks and zero cost.
    let mut hooks = match ctx.mode {
        Mode::Plan => hooks::Hooks::empty(),
        _ => hooks::Hooks::load(ctx.cwd, sink),
    };
    // T90: `.chug/permissions.json` loads once per invocation, in EVERY mode
    // (run + chat + plan): an in-process deny can only restrict further, so
    // plan mode is not structurally excluded the way hooks are — denying
    // `read_file *.key` inside a plan session is exactly the point.
    let permissions = permissions::Permissions::load(ctx.cwd, sink);
    // T73 plan mode: EXACTLY the five-tool read-only surface, and never an
    // MCP extension (an empty registry would be a no-op anyway, but the plan
    // branch makes the "no other schema advertised" guarantee structural).
    let tool_schemas = match ctx.mode {
        Mode::Plan => crate::plan::tool_schemas(),
        _ => {
            // An empty registry (no MCP config) extends with nothing:
            // byte-identical tools array to before.
            let mut schemas = tools::tool_schemas();
            schemas.extend(mcp.tool_schemas());
            schemas
        }
    };
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
    // T13: one-shot latches for the budget-low warning, one per budget kind.
    // Per invocation: a chat turn (or a --resume) gets fresh warnings.
    let (mut warned_iter, mut warned_time, mut warned_tokens) = (false, false, false);
    // T91: latched when the endpoint rejected image content — from then on
    // every image result is downgraded at WRAP time (never sent), and the
    // degrade retry itself happens at most once per invocation.
    let mut images_degraded = false;

    loop {
        if iteration >= knobs.max_iters {
            return abort_exit(
                ctx,
                "iteration budget exceeded",
                Some(BudgetExceeded::Iterations {
                    max: knobs.max_iters,
                }),
                client.model(),
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
                Some(BudgetExceeded::Minutes {
                    max: knobs.max_minutes,
                }),
                client.model(),
                TurnEndReason::BudgetExceeded,
                1,
                iteration,
                sink,
            );
        }
        // T15: token budget — cumulative input+output across every API
        // response so far (0 before the first response), checked at the same
        // top-of-iteration point as the other budgets. `0` = unlimited.
        if knobs.max_tokens > 0 && usage_in.saturating_add(usage_out) >= knobs.max_tokens {
            return abort_exit(
                ctx,
                "token budget exceeded",
                Some(BudgetExceeded::Tokens {
                    max: knobs.max_tokens,
                }),
                client.model(),
                TurnEndReason::BudgetExceeded,
                1,
                iteration,
                sink,
            );
        }
        if ctx.controls.abort.load(Ordering::SeqCst) {
            let reason = match ctx.mode {
                Mode::Autonomous | Mode::Plan => "operator abort",
                Mode::Chat => "operator interrupt",
            };
            return abort_exit(
                ctx,
                reason,
                None,
                client.model(),
                TurnEndReason::Interrupted,
                1,
                iteration,
                sink,
            );
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

        // T13: one-shot budget-low warning, injected before the next LLM call
        // so the model reprioritizes toward committing, gates, and
        // bookkeeping while budget remains (EVALUATION.md J1: wrap-phase
        // budget deaths). The message is a plain user message in the
        // transcript, like steering notes — no event, no abort-behavior
        // change. Flags latch the underlying conditions, so each kind fires
        // at most once even when the message was suppressed by the other
        // kind's latch.
        let remaining_iters = knobs.max_iters.saturating_sub(iteration);
        let remaining_secs = Duration::from_secs(knobs.max_minutes.saturating_mul(60))
            .saturating_sub(start.elapsed())
            .as_secs();
        // T15: tokens left under the cumulative budget; `None` when no token
        // budget is set (the notice then never mentions tokens). Cumulative
        // usage only grows, so this only ever drops.
        let remaining_tokens = (knobs.max_tokens > 0)
            .then(|| knobs.max_tokens.saturating_sub(usage_in.saturating_add(usage_out)));
        if let Some(notice) = budget_low_notice(
            remaining_iters,
            remaining_secs,
            remaining_tokens,
            warned_iter,
            warned_time,
            warned_tokens,
        ) {
            let msg = Message::user(vec![ContentBlock::text_block(notice)]);
            transcript::append(ctx.cwd, &msg)?;
            messages.push(msg);
            // T17: put the injection on the events record, with the remaining
            // counts at fire time — one event per actual injection (the
            // one-shot latches cap a run at three). Telemetry only: the
            // notice itself already reached the user as the message above.
            sink.emit(Event::BudgetLow {
                remaining_iters,
                remaining_secs,
                remaining_tokens,
            });
        }
        warned_iter |= remaining_iters <= WARN_REMAINING_ITERS;
        warned_time |= remaining_secs <= WARN_REMAINING_SECS;
        warned_tokens |= remaining_tokens.is_some_and(|r| r <= WARN_REMAINING_TOKENS);

        // Re-read spec every iteration: the user may edit it mid-run. Keep the
        // last good copy if it becomes unreadable.
        if let Some(spec_path) = &knobs.spec_path
            && let Ok(text) = fs::read_to_string(spec_path)
        {
            spec_text = Some(text);
        }
        let ledger_text = ledger::read(ctx.cwd)?;
        // T111: the todo store rides the prompt next to the ledger in run
        // and chat modes (empty/corrupt store ⇒ empty text ⇒ no section).
        // Plan mode renders nothing about todos (no todo tools there).
        let todos_text = crate::todos::prompt_text(ctx.cwd);
        let system = match ctx.mode {
            Mode::Autonomous => build_system_prompt(
                spec_text.as_deref().unwrap_or_default(),
                knobs.goal.as_deref().unwrap_or_default(),
                &ledger_text,
                &todos_text,
            ),
            Mode::Chat => build_chat_system_prompt(
                spec_text.as_deref(),
                knobs.goal.as_deref(),
                &ledger_text,
                &todos_text,
            ),
            Mode::Plan => build_plan_system_prompt(
                spec_text.as_deref(),
                knobs.goal.as_deref().unwrap_or_default(),
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
        // F7 phase 1: stream the model's text to the sink AS IT ARRIVES. The
        // hook emits `Event::ModelTextDelta` per text piece; the console sink
        // renders them live (headless logs gain liveness during minutes-long
        // generations), the TUI ignores them (phase 2), the event log stays
        // silent. The hook is armed around each `complete` call and cleared
        // immediately after — the deltas are console cosmetics only, never
        // transcript state.
        client.set_text_delta_hook(Some(model_text_delta_hook(sink)));
        let resp = match client.complete(&system, messages, &tool_schemas, &obs_ctx) {
            Ok(resp) => resp,
            // T91: the endpoint rejected a request carrying images (400 +
            // image/content in the body). Retry ONCE with every image block
            // in the outgoing messages replaced by placeholder text, record
            // one events note, and latch: later image results are downgraded
            // at wrap time for the rest of the run. A non-vision endpoint
            // degrades instead of poisoning every subsequent request.
            Err(e) if !images_degraded && crate::api::is_image_rejection(&e) => {
                *messages = crate::api::replace_images_with_placeholder(messages);
                images_degraded = true;
                sink.emit(Event::ImageDegraded);
                // F7 hook hygiene (kimi finding 2): the hook is cleared on
                // EVERY exit path of the LLM call — this `?` included.
                // Propagating directly would leave the raw-pointer hook
                // armed into the dropped sink borrow.
                let retried = client.complete(&system, messages, &tool_schemas, &obs_ctx);
                client.set_text_delta_hook(None);
                retried?
            }
            Err(e) => {
                client.set_text_delta_hook(None);
                return Err(e);
            }
        };
        client.set_text_delta_hook(None);
        // F7 phase 1 telemetry (req 4): a streaming request answered with a
        // plain JSON body (proxy downgrade). Latched in the client to fire at
        // most ONCE per session; events.jsonl carries the single line.
        if client.take_stream_fallback() {
            sink.emit(Event::StreamFallback);
        }

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

        // T38: a response cut off at the output-token ceiling is fresh,
        // actionable information for the very next call. Remember it here;
        // the advisory is injected below, after this turn's messages are in
        // place, immediately before the next LLM call — no latch, one
        // advisory per truncated response.
        let truncated = resp.stop_reason().as_deref() == Some("max_tokens");

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
        // T73: the plan text of an accepted submit_plan call (set only when
        // the dispatch succeeded, so an empty/invalid plan keeps the loop up).
        let mut plan_submitted: Option<String> = None;

        for (id, name, input) in assistant.content.iter().filter_map(ContentBlock::tool_use) {
            sink.emit(Event::ToolStart {
                name: name.to_string(),
            });
            let tool_start = std::time::SystemTime::now();
            let tool_t0 = Instant::now();
            // T73 plan mode: every call goes through the plan gate — the
            // read-only four execute; submit_plan is the write/exit path;
            // ANY other name (all the run/chat tools, and any mcp__ import)
            // is rejected with a tool error naming the allowed set, never
            // executed, and the loop continues.
            // T83: a driver-level block (PreToolUse veto, risk-gate block)
            // means the tool NEVER executed, so no PostToolUse hook may fire
            // on the block result (reqs 2+3: PostToolUse follows execution
            // only, and no advisory may ride a block's exact error text).
            let mut blocked = false;
            // T90: the permission check is the FIRST gate in the dispatch
            // chain — before the PreToolUse hook check, the plan gate, MCP
            // dispatch, the risk gate, and the tool itself. A denied call
            // fires no hooks, reaches none of the later gates, and never
            // executes; the deny result is the T83 veto shape (a tool error
            // the model routes around, loop continues), riding the same
            // `blocked` flag so no PostToolUse hook may fire on a deny
            // (T83's reqs 2+3 semantics cover both block classes).
            let mut result = if !permissions.is_empty()
                && let Some(deny_message) = permissions.check(name, input, sink)
            {
                blocked = true;
                ToolResult {
                    content: deny_message,
                    is_error: true,
                    images: Vec::new(),
                }
            } else if !hooks.is_empty() && ctx.mode != Mode::Plan
                && let Err(veto_message) = hooks.pre_tool_use(ctx.cwd, name, input, sink)
            {
                // T83 PreToolUse veto: the tool does NOT execute (the gate,
                // MCP dispatch, and the tool itself are all skipped); the
                // model receives a tool error it routes around and the loop
                // continues — the risk-gate block shape.
                blocked = true;
                ToolResult {
                    content: veto_message,
                    is_error: true,
                    images: Vec::new(),
                }
            } else if ctx.mode == Mode::Plan {
                crate::plan::dispatch(&tool_ctx, name, input, ctx.plan_out)
            } else if name.starts_with("mcp__") {
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
                            // The gate blocked before dispatch: the tool
                            // never executed, so PostToolUse must not fire.
                            blocked = true;
                            ToolResult {
                                content: msg,
                                is_error: true,
                                images: Vec::new(),
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
            // Capture the tool-only end time + duration BEFORE the
            // PostToolUse hooks run: the hook advisory rides the result but
            // is not tool time (SPEC-8 span + ToolResult measure the call).
            let tool_end = std::time::SystemTime::now();
            let tool_duration = tool_t0.elapsed().as_millis() as u64;
            // T83 PostToolUse: after the tool executed (ok or error), fire
            // every matching hook; non-empty stdout+stderr is APPENDED to
            // the result content as `\n\n[hook] <text>` — advisory only,
            // never changes ok/is_error, never re-fires. A driver-level
            // block (PreToolUse veto, risk-gate block) means the tool never
            // executed: no hook fires, no advisory rides the block text,
            // and no phantom fire line lands in events.jsonl.
            if !blocked && !hooks.is_empty() && ctx.mode != Mode::Plan {
                hooks.post_tool_use(
                    ctx.cwd,
                    name,
                    input,
                    result.is_error,
                    &mut result.content,
                    sink,
                );
            }
            // SPEC-8: one span per tool call with ok / is_error metadata.
            if let Some(trace) = ctx.trace {
                ctx.obs.span(
                    trace,
                    name,
                    tool_start,
                    tool_end,
                    !result.is_error,
                    result.is_error,
                );
            }
            sink.emit(Event::ToolResult {
                name: name.to_string(),
                ok: !result.is_error,
                duration_ms: tool_duration,
                preview: tool_result_preview(&result.content, result.is_error),
            });
            // Plan mode has no goal_complete exit: the call is rejected by
            // the plan gate above and the loop continues (the plan exit is
            // submit_plan below).
            // T140: the summary latch requires the call to have SUCCEEDED —
            // a blocked call (permission deny, PreToolUse veto, risk-gate
            // block) produces an is_error result without ever executing the
            // tool, and accepting its summary would let the block bypass
            // the only exit gate: the denied goal_complete would complete
            // the run through verification. Same shape submit_plan checks.
            if name == "goal_complete" && ctx.mode != Mode::Plan && !result.is_error {
                goal_summary = Some(
                    input
                        .get("summary")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                );
            }
            if name == "submit_plan" && ctx.mode == Mode::Plan && !result.is_error {
                plan_submitted = Some(
                    input
                        .get("plan")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                );
            }
            // T91: image blocks ride the tool_result as an array (image
            // blocks first, then the text note) — unless the endpoint has
            // rejected image content, in which case the latch downgrades the
            // result at wrap time: text note only, images never sent.
            let images: &[crate::api::ImageBlock] = if images_degraded {
                &[]
            } else {
                &result.images
            };
            user_blocks.push(ContentBlock::tool_result_block_with_images(
                id,
                result.content.clone(),
                result.is_error,
                images,
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
                // completeness. The assistant message stays the last entry —
                // except after a truncated response, where the advisory
                // follows it so the next turn's model knows the output was
                // cut short (there is no next call this turn).
                messages.push(assistant);
                if truncated {
                    inject_truncation_advisory(ctx.cwd, messages, sink)?;
                }
                return Ok(DriveOutcome::TurnEnded(TurnEndReason::Completed));
            }
            // Model stopped talking without finishing — the anti-stall kick
            // (plan mode names its own exit: submit_plan, not goal_complete).
            user_blocks.push(ContentBlock::text_block(match ctx.mode {
                Mode::Plan => crate::plan::PLAN_KICK,
                _ => KICK,
            }));
        } else if let Some(plan) = plan_submitted {
            // T73 plan exit: submit_plan succeeded — end the session with
            // exit 0. The plan rides the existing goal/verdict machinery:
            // GoalAccepted (the events log records it exactly like a run's
            // goal acceptance, with the plan as the summary), and the
            // ConsoleSink prints it to stdout (the no---out surface).
            let user_msg = Message::user(user_blocks);
            transcript::append(ctx.cwd, &user_msg)?;
            sink.emit(Event::GoalAccepted { summary: plan.clone() });
            if let Some(trace) = ctx.trace {
                ctx.obs.event(trace, "goal_accepted", json!({ "summary": plan }));
                finish_run(
                    ctx.obs,
                    trace,
                    observ::outcome::COMPLETED,
                    u64::from(iteration) + 1,
                );
            }
            return Ok(DriveOutcome::RunFinished(0));
        } else if let Some(summary) = goal_summary {
            // Autonomous: the spec's `check:` line. Chat: the `/check` command.
            let check_cmd = match ctx.mode {
                Mode::Autonomous => spec_text.as_deref().and_then(parse_check_command),
                // Plan mode has no check gate: submit_plan is the exit.
                Mode::Plan => None,
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
                    user_blocks.push(ContentBlock::text_block(goal_rejected_message(&output)));
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
                None,
                client.model(),
                TurnEndReason::Interrupted,
                2,
                iteration,
                sink,
            );
        }

        if trim::transcript_trim(messages) {
            transcript::rewrite(ctx.cwd, messages)?;
        }

        // T38: a truncated response injects its advisory here — after this
        // turn's messages, immediately before the next LLM call. One advisory
        // per truncated response, no latch: every truncation is fresh,
        // actionable information. Responses that instead end the run (an
        // accepted goal above, an abort) have no next call to advise and get
        // none.
        if truncated {
            inject_truncation_advisory(ctx.cwd, messages, sink)?;
        }

        iteration += 1;
    }
}

/// T25: the preview string a tool result contributes to [`Event::ToolResult`].
/// Ok results keep the T10-era 500-char head of the content — byte-identical
/// to pre-T25. Error results get a tail-anchored window, the LAST
/// [`ERROR_PREVIEW_TAIL_CHARS`] chars, because failure bytes cluster at the
/// end of command output (cargo's `failures:` list, rustc's `error[Exxxx]`
/// blocks) and a flat head window drops the failing test's name (cycle-9
/// eval N1). Char-boundary safe: always `chars()`, never byte slicing.
fn tool_result_preview(content: &str, is_error: bool) -> String {
    if !is_error {
        return content.chars().take(500).collect();
    }
    let total = content.chars().count();
    let skip = total.saturating_sub(ERROR_PREVIEW_TAIL_CHARS);
    content.chars().skip(skip).collect()
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

/// T38: append the truncation advisory as a user message — transcript +
/// memory, the same steering-note mechanism as the T13/T17 notices — and put
/// the injection on the events record (one [`Event::OutputTruncated`] per
/// call, no latch). Telemetry only: console/TUI sinks render nothing (T17
/// precedent).
fn inject_truncation_advisory(
    cwd: &Path,
    messages: &mut Vec<Message>,
    sink: &mut dyn EventSink,
) -> anyhow::Result<()> {
    let msg = Message::user(vec![ContentBlock::text_block(OUTPUT_TRUNCATED_ADVISORY)]);
    transcript::append(cwd, &msg)?;
    messages.push(msg);
    sink.emit(Event::OutputTruncated);
    Ok(())
}

/// T13: the one-shot budget-low warning text, or `None` when nothing needs
/// firing. Pure so the time half is unit-testable without sleeping.
///
/// Fires when the remaining iteration budget first drops to
/// `<= WARN_REMAINING_ITERS`, the remaining wall-clock budget first drops
/// to `<= WARN_REMAINING_SECS`, or — T15 — the remaining token budget first
/// drops to `<= WARN_REMAINING_TOKENS` (`None` = no token budget: the leg
/// never fires and the message never mentions tokens), one shot per budget
/// kind: the caller passes the three latch flags and mirrors this function's
/// threshold comparisons when latching them (all directions are pinned by
/// unit + scripted-loop tests). The message always states the actual
/// remaining counts at fire time.
fn budget_low_notice(
    remaining_iters: u32,
    remaining_secs: u64,
    remaining_tokens: Option<u64>,
    already_warned_iter: bool,
    already_warned_time: bool,
    already_warned_tokens: bool,
) -> Option<String> {
    let iters_low = remaining_iters <= WARN_REMAINING_ITERS;
    let time_low = remaining_secs <= WARN_REMAINING_SECS;
    let tokens_low = remaining_tokens.is_some_and(|r| r <= WARN_REMAINING_TOKENS);
    if (iters_low && !already_warned_iter)
        || (time_low && !already_warned_time)
        || (tokens_low && !already_warned_tokens)
    {
        // Without a token budget this stays byte-identical to the pre-T15
        // message; with one, the remaining tokens join the count.
        let counts = match remaining_tokens {
            None => format!(
                "{remaining_iters} iteration(s) and {} minute(s)",
                remaining_secs / 60
            ),
            Some(tokens) => format!(
                "{remaining_iters} iteration(s), {} minute(s), and {tokens} token(s)",
                remaining_secs / 60
            ),
        };
        Some(format!(
            "chug: budget low — {counts} remain. \
             Stop starting new work: commit what is done, run the gates, and finish \
             bookkeeping now."
        ))
    } else {
        None
    }
}

/// The rejection text for a failed goal check (T9). Beyond the failure
/// output, the model is told the check shares the bash tool's environment —
/// the same `sh -c` wrapper in the run cwd with the same PATH prepend — and
/// is explicitly warned against "fixing" the check by mutating state outside
/// the run cwd (EVALUATION.md I7: an agent once created a global cargo
/// symlink to make cargo resolvable). Environment literals are derived from
/// the same constants the shell wrapper uses, so the note cannot drift from
/// reality: [`tools::CARGO_BIN_REL`] and [`tools::CHECK_TIMEOUT_SECS`].
fn goal_rejected_message(output: &str) -> String {
    format!(
        "goal_complete rejected: the spec check command failed. Output:\n\n{output}\n\n\
         Fix the failure and try again. Update the ledger to reflect the current state.\n\n\
         Environment note: the check ran via the same shell wrapper as your bash tool \
         (`sh -c` in the run cwd, `~/{CARGO_BIN_REL}` prepended to PATH when that \
         directory exists, {CHECK_TIMEOUT_SECS}s timeout). If the check fails on a \
         missing tool that works in your bash tool, suspect the check command itself — \
         do NOT create or modify files outside the run cwd to make the check pass.",
        CARGO_BIN_REL = tools::CARGO_BIN_REL,
        CHECK_TIMEOUT_SECS = tools::CHECK_TIMEOUT_SECS,
    )
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

/// T111: the `## Todos` section appended after `## Ledger` — ONLY when the
/// todo list is non-empty (`todos_text` empty ⇒ no heading is emitted). The
/// text is pre-rendered by `todos::prompt_text` (`t3 [in_progress] title`
/// lines). Plan mode takes no todos parameter: it has no write tools, so it
/// renders nothing about todos.
fn append_todos_section(prompt: &mut String, todos_text: &str) {
    if !todos_text.is_empty() {
        prompt.push_str(&format!("\n\n## Todos\n\n{todos_text}"));
    }
}

pub fn build_system_prompt(spec: &str, goal: &str, ledger_text: &str, todos_text: &str) -> String {
    let mut prompt = format!(
        "{PREAMBLE}\n\n## Spec\n\n{spec}\n\n## Goal\n\n{goal}\n\n## Ledger\n\n{ledger_text}"
    );
    append_todos_section(&mut prompt, todos_text);
    prompt
}

/// T73 plan-mode system prompt: the read-only contract preamble, optional
/// spec section, the goal, and the ledger as READ-ONLY context (plan mode
/// never writes it — the ledger is read with the usual seed fallback, so a
/// worktree without one still gets a prompt).
pub fn build_plan_system_prompt(spec: Option<&str>, goal: &str, ledger_text: &str) -> String {
    let mut prompt = crate::plan::PLAN_PREAMBLE.to_string();
    if let Some(spec) = spec {
        prompt.push_str(&format!("\n\n## Spec\n\n{spec}"));
    }
    prompt.push_str(&format!("\n\n## Goal\n\n{goal}"));
    prompt.push_str(&format!(
        "\n\n## Ledger (read-only context; plan mode never writes it)\n\n{ledger_text}"
    ));
    prompt
}

/// Chat-mode system prompt: interactive preamble, optional spec and goal
/// sections (only when configured), ledger as today. The current objective is
/// the final user message, never part of the system prompt.
pub fn build_chat_system_prompt(
    spec: Option<&str>,
    goal: Option<&str>,
    ledger_text: &str,
    todos_text: &str,
) -> String {
    let mut prompt = CHAT_PREAMBLE.to_string();
    if let Some(spec) = spec {
        prompt.push_str(&format!("\n\n## Spec\n\n{spec}"));
    }
    if let Some(goal) = goal {
        prompt.push_str(&format!("\n\n## Goal\n\n{goal}"));
    }
    prompt.push_str(&format!("\n\n## Ledger\n\n{ledger_text}"));
    append_todos_section(&mut prompt, todos_text);
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
/// T12: `model` names the model that died (read from the client at the abort
/// site, so chat `/model` switches are reflected); `budget` is `Some` only
/// for budget deaths, driving the sink's fallback-resume hint.
#[allow(clippy::too_many_arguments)]
fn abort_exit(
    ctx: &LoopCtx,
    reason: &str,
    budget: Option<BudgetExceeded>,
    model: &str,
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
        model: model.to_string(),
        budget,
    });
    if let Some(trace) = ctx.trace {
        ctx.obs.event(trace, "abort", json!({ "reason": reason }));
        // SPEC-8: finish the run/plan trace with the classified outcome. Chat
        // turns keep the session trace open — the session continues.
        if ctx.mode != Mode::Chat {
            finish_run(ctx.obs, trace, abort_outcome(reason), u64::from(iteration));
        }
    }
    Ok(match ctx.mode {
        Mode::Chat => DriveOutcome::TurnEnded(turn_reason),
        Mode::Autonomous | Mode::Plan => DriveOutcome::RunFinished(code),
    })
}

#[cfg(test)]
// `pub(crate)` (test-only module): T84's trim.rs tests reuse the drive-loop
// harness (ctx_for/knobs_with/tool_use_response/RecordingSink) instead of
// duplicating it — the mcp_http.rs T37 precedent.
pub(crate) mod tests;
