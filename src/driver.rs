use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use anyhow::{Context, bail};
use serde_json::{Value, json};

use crate::api::{Client, ContentBlock, Llm, Message, ObsCtx};
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
    // the configured budget ceilings (T17) and the cwd's checkout HEAD
    // (T20, best-effort: a non-repo cwd just leaves both fields null).
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
pub fn resume_messages(cwd: &Path) -> anyhow::Result<Vec<Message>> {
    let mut messages = transcript::load(cwd)?;
    if !messages.is_empty() && trim::transcript_trim(&mut messages) {
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
        plan_out: None,
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
                client.complete(&system, messages, &tool_schemas, &obs_ctx)?
            }
            Err(e) => return Err(e),
        };

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
            if name == "goal_complete" && ctx.mode != Mode::Plan {
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

pub fn build_system_prompt(spec: &str, goal: &str, ledger_text: &str) -> String {
    format!("{PREAMBLE}\n\n## Spec\n\n{spec}\n\n## Goal\n\n{goal}\n\n## Ledger\n\n{ledger_text}")
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
pub(crate) mod tests {
    use super::*;
    use crate::api::{KnownBlock, ScriptedLlm};
    use serde_json::json;

    #[test]
    fn tripwire_fires_on_three_identical_errors() {
        let recent: Vec<ToolResult> = (0..3)
            .map(|_| ToolResult {
                content: "boom".to_string(),
                is_error: true,
                images: Vec::new(),
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
                images: Vec::new(),
            },
            ToolResult {
                content: "boom".to_string(),
                is_error: true,
                images: Vec::new(),
            },
            ToolResult {
                content: "different failure".to_string(),
                is_error: true,
                images: Vec::new(),
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
                images: Vec::new(),
            },
            ToolResult {
                content: "boom".to_string(),
                is_error: true,
                images: Vec::new(),
            },
            ToolResult {
                content: "boom".to_string(),
                is_error: false,
                images: Vec::new(),
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
                images: Vec::new(),
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
                images: Vec::new(),
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
    pub(crate) struct RecordingSink(Vec<Event>);

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
            max_tokens: 0, // no token budget: pre-T15 behavior
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
            Some(Event::Aborted { reason, .. }) if reason == "operator abort"
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

        // T11: the run's events log opens with the startup banner fields.
        let first: Value = serde_json::from_str(
            std::fs::read_to_string(tmp.path().join(".chug/events.jsonl"))
                .expect("events.jsonl written")
                .lines()
                .next()
                .expect("run_start line"),
        )
        .expect("first line parses");
        assert_eq!(first["type"], "run_start");
        assert_eq!(first["mode"], "run");
        assert_eq!(first["model"], "test-model");
        assert_eq!(first["version"], crate::build_info::VERSION);
        assert_eq!(first["commit"], crate::build_info::GIT_COMMIT);
        // T17: the run's configured ceilings ride the banner.
        assert_eq!(first["max_iters"], 5);
        assert_eq!(first["max_minutes"], 10);
        assert!(first["max_tokens"].is_null(), "no token budget → null");
        // T20: the cwd's checkout HEAD rides along; this tempdir is not a
        // repo, so both stay null (and the run itself was never touched by
        // the failed resolution).
        assert!(first["head_branch"].is_null(), "{first}");
        assert!(first["head_commit"].is_null(), "{first}");
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
            max_tokens: 0, // no token budget: pre-T15 behavior
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

    // ---------- T7: fresh-run transcript rotation ----------

    fn transcript_archives(tmp: &tempfile::TempDir) -> Vec<PathBuf> {
        let dir = tmp.path().join(".chug");
        if !dir.exists() {
            return Vec::new();
        }
        let mut out: Vec<PathBuf> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| {
                let path = e.unwrap().path();
                let name = path.file_name().unwrap().to_string_lossy().to_string();
                (name.starts_with("transcript-") && name.ends_with(".jsonl")).then_some(path)
            })
            .collect();
        out.sort();
        out
    }

    #[test]
    fn fresh_run_rotates_previous_transcript() {
        let tmp = tempfile::tempdir().unwrap();
        let spec = write_spec(&tmp);
        let old = Message::user(vec![ContentBlock::text_block("Goal: OLD SESSION")]);
        transcript::append(tmp.path(), &old).unwrap();

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

        // The old session was archived with its content intact.
        let archives = transcript_archives(&tmp);
        assert_eq!(archives.len(), 1, "exactly one transcript archive: {archives:?}");
        assert!(
            std::fs::read_to_string(&archives[0])
                .unwrap()
                .contains("Goal: OLD SESSION")
        );
        // The new transcript begins with this run's goal message and nothing
        // else (the abort fires before any LLM call).
        let messages = transcript::load(tmp.path()).unwrap();
        assert_eq!(messages.len(), 1);
        assert!(
            messages[0].content[0]
                .text()
                .unwrap()
                .starts_with("Goal: x\n"),
            "{:?}",
            messages[0].content[0].text()
        );
    }

    #[test]
    fn resume_run_loads_transcript_without_rotating() {
        let tmp = tempfile::tempdir().unwrap();
        let spec = write_spec(&tmp);
        let old = Message::user(vec![ContentBlock::text_block("Goal: OLD SESSION")]);
        transcript::append(tmp.path(), &old).unwrap();

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

        assert!(transcript_archives(&tmp).is_empty(), "--resume never archives");
        let messages = transcript::load(tmp.path()).unwrap();
        assert_eq!(
            messages,
            vec![old],
            "transcript untouched: the old session loads as-is"
        );
    }

    // ---------- T9: goal-rejection environment honesty ----------

    #[test]
    fn goal_rejected_message_states_check_environment() {
        let msg = goal_rejected_message("$ cargo test\nexit code: 127\nsh: cargo: command not found");
        // The three key facts (T9): the check shares the bash tool's
        // run_shell wrapper, the cargo PATH prepend, and the out-of-cwd
        // prohibition. The literals are derived from the shell wrapper's own
        // constants, so drift between the message and reality breaks here.
        assert!(msg.contains("same shell wrapper as your bash tool"), "{msg}");
        assert!(msg.contains(&format!("~/{}", tools::CARGO_BIN_REL)), "{msg}");
        assert!(
            msg.contains(&format!("{}s timeout", tools::CHECK_TIMEOUT_SECS)),
            "{msg}"
        );
        assert!(
            msg.contains("do NOT create or modify files outside the run cwd"),
            "{msg}"
        );
        // The original guidance and the failing output are preserved.
        assert!(msg.contains("Fix the failure and try again"), "{msg}");
        assert!(msg.contains("sh: cargo: command not found"), "{msg}");
    }

    #[test]
    fn goal_rejection_includes_environment_note_in_tool_result() {
        // End to end through drive_loop: a goal_complete with a failing
        // check puts the T9 environment note in the user message the model
        // sees next iteration.
        let tmp = tempfile::tempdir().unwrap();
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for(&tmp, Mode::Autonomous, &controls, &urx, None, &observ::Sink::Noop);
        let mut knobs = knobs_with(1); // abort right after the first iteration
        let mut llm = ScriptedLlm::new(vec![tool_use_response(
            "goal_complete",
            json!({"summary": "claim done"}),
        )]);
        let mut gate = None;
        let mut messages = Vec::new();
        let outcome = drive_loop(
            &ctx,
            &mut knobs,
            &mut llm,
            &mut gate,
            &mut messages,
            Some("check: false".to_string()),
            &mut RecordingSink::default(),
            &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
        )
        .unwrap();
        assert!(matches!(outcome, DriveOutcome::RunFinished(1)));
        let rejection: String = messages
            .last()
            .expect("rejection user message")
            .content
            .iter()
            .filter_map(|b| b.text())
            .collect();
        assert!(rejection.contains("goal_complete rejected"), "{rejection}");
        assert!(
            rejection.contains("same shell wrapper as your bash tool"),
            "{rejection}"
        );
        assert!(
            rejection.contains("do NOT create or modify files outside the run cwd"),
            "{rejection}"
        );
    }

    // ---------- T10: .chug/events.jsonl ----------

    /// Read the events log of a run in `tmp`, asserting every line is JSON.
    fn events_jsonl(tmp: &tempfile::TempDir) -> Vec<Value> {
        let path = tmp.path().join(".chug").join("events.jsonl");
        std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("events.jsonl readable: {e}"))
            .lines()
            .map(|l| serde_json::from_str(l).expect("every events.jsonl line parses as JSON"))
            .collect()
    }

    /// A scripted autonomous run leaves a jq-mineable events log: iteration
    /// lines carry cumulative tokens, tool results carry ok/is_error/
    /// duration_ms and a ≤200-char preview, and the terminal goal event is
    /// recorded. The tee is transparent to the real sink.
    #[test]
    fn drive_loop_writes_events_jsonl() {
        let tmp = tempfile::tempdir().unwrap();
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for(&tmp, Mode::Autonomous, &controls, &urx, None, &observ::Sink::Noop);
        let mut knobs = knobs_with(5);
        let loud = format!("printf '{}'", "x".repeat(500));
        let mut llm = ScriptedLlm::new(vec![
            tool_use_response("bash", json!({"command": "false"})),
            tool_use_response("bash", json!({"command": loud})),
            tool_use_response("goal_complete", json!({"summary": "all done"})),
        ]);
        let mut gate = None;
        let mut messages = Vec::new();
        let mut sink = RecordingSink::default();
        let outcome = drive_loop(
            &ctx,
            &mut knobs,
            &mut llm,
            &mut gate,
            &mut messages,
            Some("check: true".to_string()),
            &mut sink,
            &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
        )
        .unwrap();
        assert!(matches!(outcome, DriveOutcome::RunFinished(0)));
        // Transparent tee: the real sink still saw the terminal event.
        assert!(
            sink.0.iter().any(|e| matches!(e, Event::GoalAccepted { .. })),
            "inner sink receives events through the tee"
        );

        let lines = events_jsonl(&tmp);
        // Iteration lines: n + cumulative tokens from the Usage merge.
        let iters: Vec<&Value> = lines.iter().filter(|l| l["type"] == "iteration").collect();
        assert_eq!(iters.len(), 3, "one per iteration: {lines:?}");
        assert_eq!(iters[0]["n"], 1);
        assert_eq!(iters[0]["input_tokens"], 10);
        assert_eq!(iters[2]["input_tokens"], 30);
        // Tool results: the failing bash is an error; previews cap at 200.
        let tools: Vec<&Value> = lines.iter().filter(|l| l["type"] == "tool_result").collect();
        assert_eq!(tools.len(), 3, "{lines:?}");
        assert_eq!(tools[0]["name"], "bash");
        assert_eq!(tools[0]["ok"], false);
        assert_eq!(tools[0]["is_error"], true);
        assert!(tools[0]["duration_ms"].is_u64(), "duration recorded");
        assert_eq!(tools[1]["is_error"], false);
        for t in &tools {
            let p = t["preview"].as_str().unwrap();
            assert!(p.chars().count() <= 200, "preview ≤200 chars, got {}", p.chars().count());
        }
        assert_eq!(tools[1]["preview"].as_str().unwrap().chars().count(), 200);
        // The check ran and the terminal goal verdict is on record.
        assert!(lines.iter().any(|l| l["type"] == "verifying"));
        let goal = lines.iter().find(|l| l["type"] == "goal").expect("goal line");
        assert_eq!(goal["outcome"], "accepted");
        assert_eq!(goal["summary"], "all done");
        // The goal line is the last event of the run.
        assert_eq!(lines.last().unwrap()["type"], "goal");
    }

    /// The terminal abort event lands in the log too (budget death here).
    #[test]
    fn drive_loop_events_jsonl_records_abort() {
        let tmp = tempfile::tempdir().unwrap();
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for(&tmp, Mode::Autonomous, &controls, &urx, None, &observ::Sink::Noop);
        let mut knobs = knobs_with(1); // die on the iteration budget after one pass
        let mut llm = ScriptedLlm::new(vec![text_only_response("thinking")]);
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
            &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
        )
        .unwrap();
        assert!(matches!(outcome, DriveOutcome::RunFinished(1)));
        let lines = events_jsonl(&tmp);
        assert_eq!(lines.last().unwrap()["type"], "abort");
        assert_eq!(
            lines.last().unwrap()["reason"],
            "iteration budget exceeded"
        );
        // T12: the abort line names the model and the exhausted budget.
        assert_eq!(lines.last().unwrap()["model"], "scripted-model");
        assert_eq!(lines.last().unwrap()["budget_kind"], "iterations");
        assert_eq!(lines.last().unwrap()["budget_max"], 1);
    }

    /// T12: the Aborted event itself carries the dying model + the exhausted
    /// budget, so sinks can render the resume-with-fallback hint.
    #[test]
    fn budget_abort_event_names_model_and_exhausted_budget() {
        let tmp = tempfile::tempdir().unwrap();
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for(&tmp, Mode::Autonomous, &controls, &urx, None, &observ::Sink::Noop);
        let mut knobs = knobs_with(1); // die on the iteration budget after one pass
        let mut llm = ScriptedLlm::new(vec![text_only_response("thinking")]);
        let mut gate = None;
        let mut messages = Vec::new();
        let mut sink = RecordingSink::default();
        let outcome = drive_loop(
            &ctx,
            &mut knobs,
            &mut llm,
            &mut gate,
            &mut messages,
            None,
            &mut sink,
            &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
        )
        .unwrap();
        assert!(matches!(outcome, DriveOutcome::RunFinished(1)));
        let abort = sink
            .0
            .iter()
            .find_map(|e| match e {
                Event::Aborted {
                    reason,
                    model,
                    budget,
                } => Some((reason.clone(), model.clone(), *budget)),
                _ => None,
            })
            .expect("abort event emitted");
        assert_eq!(abort.0, "iteration budget exceeded");
        assert_eq!(abort.1, "scripted-model", "the dying model is named");
        assert_eq!(
            abort.2,
            Some(BudgetExceeded::Iterations { max: 1 }),
            "the exhausted iteration budget is named"
        );
    }

    /// T12: operator aborts share the model line but carry no budget (the
    /// fallback hint is a budget-death feature).
    #[test]
    fn operator_abort_event_has_model_but_no_budget() {
        let tmp = tempfile::tempdir().unwrap();
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls {
            abort: Arc::new(AtomicBool::new(true)),
            steering_rx: mpsc::channel().1,
        };
        let ctx = ctx_for(&tmp, Mode::Autonomous, &controls, &urx, None, &observ::Sink::Noop);
        let mut knobs = knobs_with(5);
        let mut llm = ScriptedLlm::new(vec![]);
        let mut gate = None;
        let mut messages = Vec::new();
        let mut sink = RecordingSink::default();
        let outcome = drive_loop(
            &ctx,
            &mut knobs,
            &mut llm,
            &mut gate,
            &mut messages,
            None,
            &mut sink,
            &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
        )
        .unwrap();
        assert!(matches!(outcome, DriveOutcome::RunFinished(1)));
        let abort = sink
            .0
            .iter()
            .find_map(|e| match e {
                Event::Aborted { model, budget, .. } => Some((model.clone(), *budget)),
                _ => None,
            })
            .expect("abort event emitted");
        assert_eq!(abort.0, "scripted-model");
        assert_eq!(abort.1, None);
    }

    // ---------- T13: budget-low warning before abort ----------

    #[test]
    fn budget_low_notice_iteration_boundary() {
        // 9 remaining is above the threshold; 8 fires (T18: WARN_REMAINING_ITERS = 8).
        assert_eq!(budget_low_notice(9, u64::MAX, None, false, false, false), None);
        assert!(budget_low_notice(8, u64::MAX, None, false, false, false).is_some());
    }

    #[test]
    fn budget_low_notice_time_boundary() {
        // One second above the 5-minute threshold stays silent; at the
        // threshold the time half fires on its own (iters far from low).
        assert_eq!(
            budget_low_notice(u32::MAX, WARN_REMAINING_SECS + 1, None, false, false, false),
            None
        );
        assert!(
            budget_low_notice(u32::MAX, WARN_REMAINING_SECS, None, false, false, false).is_some()
        );
    }

    #[test]
    fn budget_low_notice_one_shot_flags_suppress_repeats() {
        // Both kinds latched: silent forever after, even deep in the low zone.
        assert_eq!(budget_low_notice(1, 30, None, true, true, false), None);
        // A latched kind never re-fires; the other still gets its one shot.
        assert!(budget_low_notice(1, 30, None, true, false, false).is_some());
        assert!(budget_low_notice(1, 30, None, false, true, false).is_some());
    }

    #[test]
    fn budget_low_notice_interpolates_actual_counts() {
        let msg = budget_low_notice(3, 150, None, false, false, false)
            .expect("fires below both thresholds");
        assert!(msg.contains("3 iteration(s)"), "{msg}");
        assert!(msg.contains("2 minute(s)"), "{msg}");
        assert!(msg.contains("commit what is done"), "{msg}");
        // The counts are the ones at fire time, not the thresholds.
        let msg = budget_low_notice(1, 60, None, false, true, false).expect("iter half still armed");
        assert!(msg.contains("1 iteration(s)"), "{msg}");
        assert!(msg.contains("1 minute(s)"), "{msg}");
    }

    /// T13, scripted run with an 8-iteration budget and WARN=8 (T18): the
    /// whole budget is warn-zone from the very first boundary, so exactly one
    /// budget-low user message — naming the 8 iterations that remain — goes
    /// out before the first call, and never a second one. The run itself
    /// ends exactly as before (accepted goal, exit 0).
    #[test]
    fn budget_low_warning_fires_once_at_threshold() {
        let tmp = tempfile::tempdir().unwrap();
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for(&tmp, Mode::Autonomous, &controls, &urx, None, &observ::Sink::Noop);
        let mut knobs = knobs_with(8);
        let mut responses = vec![text_only_response("working"); 7];
        responses.push(tool_use_response("goal_complete", json!({"summary": "wrapped up"})));
        let mut llm = ScriptedLlm::new(responses);
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
            &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
        )
        .unwrap();
        assert!(matches!(outcome, DriveOutcome::RunFinished(0)));

        let is_notice = |m: &Message| {
            m.role == "user"
                && m.content
                    .iter()
                    .any(|b| b.text().is_some_and(|t| t.starts_with("chug: budget low")))
        };

        // Exactly one notice in the transcript the driver holds…
        let notices: Vec<&Message> = messages.iter().filter(|m| is_notice(m)).collect();
        assert_eq!(notices.len(), 1, "exactly one budget-low warning");
        let text = notices[0].content[0].text().expect("notice is text");
        assert!(text.contains("8 iteration(s)"), "{text}");
        assert!(text.contains("commit what is done"), "{text}");
        // …and exactly one on disk.
        let on_disk = transcript::load(tmp.path()).unwrap();
        assert_eq!(on_disk.iter().filter(|m| is_notice(m)).count(), 1);

        // The model sees the single notice from the 1st call on (remaining
        // == 8: with max_iters == WARN the whole budget is warn-zone at the
        // first boundary) and no call ever sees more than that one message:
        // later calls still carry it as conversation history, never a second.
        assert_eq!(llm.calls.len(), 8);
        for (i, (_, seen)) in llm.calls.iter().enumerate() {
            let count = seen.iter().filter(|m| is_notice(m)).count();
            assert_eq!(
                count, 1,
                "call {} (1-based) carries {count} notice(s), expected the one",
                i + 1
            );
        }
    }

    /// T17: the injection lands in `.chug/events.jsonl` as exactly one
    /// `budget_low` line — the remaining counts at fire time (T18: with an
    /// 8-iteration budget and WARN=8 the iteration leg fires on the first
    /// boundary, with 8 remaining) and `remaining_tokens` null when no token
    /// budget is configured — recorded before the run ends.
    #[test]
    fn budget_low_injection_lands_in_events_jsonl() {
        let tmp = tempfile::tempdir().unwrap();
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for(&tmp, Mode::Autonomous, &controls, &urx, None, &observ::Sink::Noop);
        let mut knobs = knobs_with(8);
        let mut responses = vec![text_only_response("working"); 7];
        responses.push(tool_use_response("goal_complete", json!({"summary": "wrapped up"})));
        let mut llm = ScriptedLlm::new(responses);
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
            &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
        )
        .unwrap();
        assert!(matches!(outcome, DriveOutcome::RunFinished(0)));

        let lines = events_jsonl(&tmp);
        let lows: Vec<&Value> = lines
            .iter()
            .filter(|l| l["type"] == "budget_low")
            .collect();
        assert_eq!(lows.len(), 1, "one injection → one budget_low line: {lines:?}");
        let remaining = lows[0]["remaining_iters"].as_u64().unwrap();
        assert!(
            remaining <= u64::from(WARN_REMAINING_ITERS),
            "remaining at fire time is in the warn zone, got {remaining}"
        );
        assert_eq!(remaining, 8, "the fire-time count, not the threshold");
        assert!(
            lows[0]["remaining_tokens"].is_null(),
            "no token budget → null, never a phantom number"
        );
        // Telemetry of a mid-run injection: before the terminal goal line.
        let low_idx = lines
            .iter()
            .position(|l| l["type"] == "budget_low")
            .unwrap();
        let goal_idx = lines.iter().position(|l| l["type"] == "goal").unwrap();
        assert!(low_idx < goal_idx);
    }

    /// T17 control: a run that never approaches any budget writes no
    /// `budget_low` line at all.
    #[test]
    fn no_budget_low_line_when_far_from_budget() {
        let tmp = tempfile::tempdir().unwrap();
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for(&tmp, Mode::Autonomous, &controls, &urx, None, &observ::Sink::Noop);
        let mut knobs = knobs_with(50);
        let mut llm = ScriptedLlm::new(vec![
            text_only_response("working"),
            tool_use_response("goal_complete", json!({"summary": "wrapped up"})),
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
            &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
        )
        .unwrap();
        assert!(matches!(outcome, DriveOutcome::RunFinished(0)));

        let lines = events_jsonl(&tmp);
        assert!(
            lines.iter().all(|l| l["type"] != "budget_low"),
            "no budget_low when budgets stay far away: {lines:?}"
        );
        // And no notice reached the transcript either.
        let on_disk = transcript::load(tmp.path()).unwrap();
        assert!(!on_disk.iter().any(|m| m.content.iter().any(|b| b
            .text()
            .is_some_and(|t| t.starts_with("chug: budget low")))));
    }

    // ---------- T15: token-denominated budget ----------

    #[test]
    fn budget_low_notice_tokens_boundary() {
        // One token above the threshold stays silent; at the threshold the
        // tokens leg fires on its own (iters/time far from low).
        assert_eq!(
            budget_low_notice(
                u32::MAX,
                u64::MAX,
                Some(WARN_REMAINING_TOKENS + 1),
                false,
                false,
                false
            ),
            None
        );
        assert!(
            budget_low_notice(u32::MAX, u64::MAX, Some(WARN_REMAINING_TOKENS), false, false, false)
                .is_some()
        );
    }

    #[test]
    fn budget_low_notice_tokens_none_means_unlimited() {
        // No token budget: the tokens leg never fires…
        assert_eq!(budget_low_notice(u32::MAX, u64::MAX, None, false, false, false), None);
        // …and the message never mentions tokens, even when another leg fires.
        let msg = budget_low_notice(3, 150, None, false, false, false).unwrap();
        assert!(!msg.contains("token"), "{msg}");
    }

    #[test]
    fn budget_low_notice_tokens_one_shot_latch() {
        // A latched tokens leg stays silent even deeper in the low zone…
        assert_eq!(budget_low_notice(u32::MAX, u64::MAX, Some(1), false, false, true), None);
        // …while the other legs still get their one shot (and vice versa).
        assert!(
            budget_low_notice(1, 30, Some(1), true, true, false).is_some(),
            "iters/time legs armed"
        );
        assert!(
            budget_low_notice(u32::MAX, u64::MAX, Some(1), false, false, false).is_some(),
            "tokens leg armed"
        );
    }

    #[test]
    fn budget_low_notice_tokens_interpolates_remaining() {
        let msg = budget_low_notice(u32::MAX, u64::MAX, Some(12_345), false, false, false)
            .expect("tokens leg fires");
        assert!(msg.contains("12345 token(s)"), "{msg}");
        assert!(msg.contains("commit what is done"), "{msg}");
        // The message names the remaining counts of every budget kind at
        // fire time, not the thresholds.
        let msg = budget_low_notice(2, 60, Some(100), false, false, false)
            .expect("tokens leg fires with iters low too");
        assert!(msg.contains("2 iteration(s)"), "{msg}");
        assert!(msg.contains("1 minute(s)"), "{msg}");
        assert!(msg.contains("100 token(s)"), "{msg}");
    }

    /// T15, scripted run: a tiny token budget aborts at the top of the
    /// iteration where cumulative usage (input+output) crosses it, naming
    /// the exhausted budget; the loop stops with script responses left.
    #[test]
    fn token_budget_aborts_when_cumulative_usage_crosses_max() {
        let tmp = tempfile::tempdir().unwrap();
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for(&tmp, Mode::Autonomous, &controls, &urx, None, &observ::Sink::Noop);
        // --max-tokens 25 against responses costing 10 in + 5 out = 15 each:
        // after two responses the cumulative 30 has crossed 25.
        let mut knobs = knobs_with_tokens(50, 25);
        let mut llm = ScriptedLlm::new(vec![
            text_only_response("working"),
            text_only_response("working"),
            text_only_response("never reached"),
        ]);
        let mut gate = None;
        let mut messages = Vec::new();
        let mut sink = RecordingSink::default();
        let outcome = drive_loop(
            &ctx,
            &mut knobs,
            &mut llm,
            &mut gate,
            &mut messages,
            None,
            &mut sink,
            &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
        )
        .unwrap();
        assert!(matches!(outcome, DriveOutcome::RunFinished(1)));
        // The loop stopped at the boundary with a scripted response unused.
        assert_eq!(llm.calls.len(), 2);
        let abort = sink
            .0
            .iter()
            .find_map(|e| match e {
                Event::Aborted { reason, budget, .. } => Some((reason.clone(), *budget)),
                _ => None,
            })
            .expect("abort event emitted");
        assert_eq!(abort.0, "token budget exceeded");
        assert_eq!(
            abort.1,
            Some(BudgetExceeded::Tokens { max: 25 }),
            "the exhausted token budget is named"
        );
        // The events log's abort line picks the new variant up unchanged.
        let lines = events_jsonl(&tmp);
        assert_eq!(lines.last().unwrap()["type"], "abort");
        assert_eq!(lines.last().unwrap()["reason"], "token budget exceeded");
        assert_eq!(lines.last().unwrap()["budget_kind"], "tokens");
        assert_eq!(lines.last().unwrap()["budget_max"], 25);
    }

    /// T15 control: the same shape of run with the knob unset (`0` =
    /// unlimited) completes naturally no matter how many tokens it burns —
    /// pre-T15 behavior exactly (no abort, no token warning leg).
    #[test]
    fn no_token_budget_runs_to_natural_completion() {
        let tmp = tempfile::tempdir().unwrap();
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for(&tmp, Mode::Autonomous, &controls, &urx, None, &observ::Sink::Noop);
        let mut knobs = knobs_with_tokens(50, 0);
        let mut llm = ScriptedLlm::new(vec![
            big_usage_text_response("working", (20_000, 5_000)),
            big_usage_text_response("working", (20_000, 5_000)),
            big_usage_text_response("working", (20_000, 5_000)),
            tool_use_response("goal_complete", json!({"summary": "wrapped up"})),
        ]);
        let mut gate = None;
        let mut messages = Vec::new();
        let mut sink = RecordingSink::default();
        let outcome = drive_loop(
            &ctx,
            &mut knobs,
            &mut llm,
            &mut gate,
            &mut messages,
            Some("check: true".to_string()),
            &mut sink,
            &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
        )
        .unwrap();
        assert!(matches!(outcome, DriveOutcome::RunFinished(0)));
        assert!(!sink.0.iter().any(|e| matches!(e, Event::Aborted { .. })));
        // 60,010 cumulative input tokens burned with no ceiling, no warning leg.
        assert!(sink.0.iter().any(|e| matches!(
            e,
            Event::Usage {
                input: 60_010,
                ..
            }
        )));
        let on_disk = transcript::load(tmp.path()).unwrap();
        assert!(!on_disk.iter().any(|m| m.content.iter().any(
            |b| b.text().is_some_and(|t| t.starts_with("chug: budget low"))
        )));
    }

    /// T15, scripted run with a 120k-token budget and 25k-token responses:
    /// exactly one budget-low user message naming the remaining tokens, first
    /// seen by the model on the call after remaining drops to 45k, never a
    /// second one. The run itself ends exactly as before (accepted goal).
    #[test]
    fn token_budget_low_warning_fires_once_mid_run() {
        let tmp = tempfile::tempdir().unwrap();
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for(&tmp, Mode::Autonomous, &controls, &urx, None, &observ::Sink::Noop);
        let mut knobs = knobs_with_tokens(50, 120_000);
        let mut llm = ScriptedLlm::new(vec![
            big_usage_text_response("working", (20_000, 5_000)),
            big_usage_text_response("working", (20_000, 5_000)),
            big_usage_text_response("working", (20_000, 5_000)),
            tool_use_response("goal_complete", json!({"summary": "wrapped up"})),
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
            &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
        )
        .unwrap();
        assert!(matches!(outcome, DriveOutcome::RunFinished(0)));

        let is_notice = |m: &Message| {
            m.role == "user"
                && m.content
                    .iter()
                    .any(|b| b.text().is_some_and(|t| t.starts_with("chug: budget low")))
        };

        // Exactly one notice, naming the 45k tokens remaining at fire time.
        let notices: Vec<&Message> = messages.iter().filter(|m| is_notice(m)).collect();
        assert_eq!(notices.len(), 1, "exactly one token budget-low warning");
        let text = notices[0].content[0].text().expect("notice is text");
        assert!(text.contains("45000 token(s)"), "{text}");
        assert!(text.contains("commit what is done"), "{text}");
        // The model first sees it on the 4th call (remaining crossed the 50k
        // threshold after the third response) and no call ever sees more
        // than that single message.
        assert_eq!(llm.calls.len(), 4);
        for (i, (_, seen)) in llm.calls.iter().enumerate() {
            let count = seen.iter().filter(|m| is_notice(m)).count();
            assert!(
                count == usize::from(i >= 3),
                "call {} (1-based) carries {count} notice(s)",
                i + 1
            );
        }
    }

    // ---------- T38: truncated-response advisory ----------

    /// The advisory pinned as a LITERAL (not the const), so corrupting any
    /// load-bearing token of [`crate::driver::OUTPUT_TRUNCATED_ADVISORY`] —
    /// `max_tokens`, `stop_reason`, `write_file`, `edit_file` — fails here.
    const T38_ADVISORY: &str = "chug: output truncated — the previous response hit the API output-token ceiling (stop_reason=max_tokens). If you were writing a file, split it: write_file the first chunk, then append with edit_file (or bash heredoc) in smaller pieces.";

    /// A truncated response carrying one tool call — the T37 shape: a big
    /// `write_file` cut off by the output-token ceiling.
    fn truncated_tool_use_response(name: &str, input: Value) -> Value {
        json!({
            "stop_reason": "max_tokens",
            "usage": {"input_tokens": 10, "output_tokens": 8192},
            "content": [{"type": "tool_use", "id": "tu_1", "name": name, "input": input}],
        })
    }

    /// A truncated pure-text response (no tool calls at all).
    fn truncated_text_response(text: &str) -> Value {
        json!({
            "stop_reason": "max_tokens",
            "usage": {"input_tokens": 10, "output_tokens": 8192},
            "content": [{"type": "text", "text": text}],
        })
    }

    /// Drive one autonomous scripted run to an accepted goal, returning the
    /// LLM double, the recorded events, and the run's parsed events log.
    fn run_t38(
        tmp: &tempfile::TempDir,
        responses: Vec<Value>,
    ) -> (ScriptedLlm, Vec<Event>, Vec<Value>) {
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for(tmp, Mode::Autonomous, &controls, &urx, None, &observ::Sink::Noop);
        // 20 iterations: comfortably clear of WARN_REMAINING_ITERS (8), so
        // the T13 budget-low notice never fires inside these 2-3-call runs —
        // it would land after the advisory and break the last-message
        // position assertions (the advisory must be observable as the most
        // recent injection before the next call).
        let mut knobs = knobs_with(20);
        let mut llm = ScriptedLlm::new(responses);
        let mut gate = None;
        let mut messages = Vec::new();
        let mut sink = RecordingSink::default();
        let outcome = drive_loop(
            &ctx,
            &mut knobs,
            &mut llm,
            &mut gate,
            &mut messages,
            Some("check: true".to_string()),
            &mut sink,
            &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
        )
        .unwrap();
        assert!(matches!(outcome, DriveOutcome::RunFinished(0)));
        (llm, sink.0, events_jsonl(tmp))
    }

    fn advisory_count(messages: &[Message]) -> usize {
        messages
            .iter()
            .filter(|m| m.content.iter().any(|b| b.text() == Some(T38_ADVISORY)))
            .count()
    }

    /// T38: no advisory message, no event, no log line — the control for
    /// every non-truncated stop reason (`tool_use`, `end_turn`, absent).
    fn assert_untouched(llm: &ScriptedLlm, events: &[Event], lines: &[Value]) {
        for (_, seen) in &llm.calls {
            assert_eq!(
                advisory_count(seen),
                0,
                "no truncation advisory may reach the model"
            );
        }
        assert!(
            !events.iter().any(|e| matches!(e, Event::OutputTruncated)),
            "no OutputTruncated event expected"
        );
        assert!(
            !lines.iter().any(|l| l["type"] == "output_truncated"),
            "no output_truncated log line expected: {lines:?}"
        );
    }

    /// T76 integration: a scripted driver run where the model calls `tgrep`
    /// on a 300-hit corpus. The tool result the NEXT LLM call sees must stay
    /// under the requested budget and carry the omission marker.
    #[test]
    fn tgrep_scripted_run_stays_under_budget_and_marks_omissions() {
        let tmp = tempfile::tempdir().unwrap();
        let mut body = String::new();
        // 300 hits, one every 10 lines, windows never touching: 300 clusters.
        for i in 1..=3000 {
            let line = if i % 10 == 0 {
                format!("needle line {i}\n")
            } else {
                format!("filler line {i} padding padding padding\n")
            };
            body.push_str(&line);
        }
        fs::write(tmp.path().join("hay.rs"), body).unwrap();
        // Round-3 sweep (blocking class 1, driver leg): the <=budget claim
        // must EXERCISE the omitted-marker band the reserve guards, so the
        // budget is calibrated at runtime from measured cluster sizes (the
        // fixed 600-token budget left ~200 chars of headroom — a
        // reserve-deletion mutant ran green). band::calibrate picks the
        // budget where the shipped reserve stays under budget but a
        // delete/shrink mutant (reserve → 0/8/16/32) re-packs one more
        // cluster and overflows.
        // Round-4 timing sweep (T72 family): this leg has NO wall-clock
        // asserts — every pin here is SIZE-based (chars / budgets / cluster
        // counts) and band::calibrate measures rendered cluster sizes,
        // never time — so the leg is load-immune by construction. The only
        // timing assert on the tgrep surface is deterministic_and_fast's
        // SPEED leg (src/tgrep.rs, median-of-5 vs a load-robust bound).
        let measure_ctx = ToolCtx {
            cwd: tmp.path().to_path_buf(),
            bash_timeout: Duration::from_secs(crate::tools::BASH_TIMEOUT_SECS),
        };
        let measured = crate::tools::dispatch(
            &measure_ctx,
            "tgrep",
            &json!({"query": "needle", "budget": 8000u64}),
        );
        assert!(!measured.is_error, "{}", measured.content);
        // The 300-cluster measure pass at the 8000-token ceiling itself
        // truncates (300 x ~300 chars > 32000); calibrate only needs the
        // header's total plus the first ~100 rendered sizes — packing is
        // prefix-based, and the picked budgets show < 20 clusters.
        let (budget, shown96, body96, over0, header_len) =
            crate::tgrep::band::calibrate(&measured.content, 3);
        let (_llm, _events, _lines) = run_t38(
            &tmp,
            vec![
                tool_use_response("tgrep", json!({"query": "needle", "budget": budget})),
                tool_use_response("goal_complete", json!({"summary": "done"})),
            ],
        );
        // The second LLM call is the first to see the tgrep tool result.
        assert_eq!(_llm.calls.len(), 2);
        let (content, is_error) = tool_result_text(&_llm.calls[1].1).unwrap();
        assert!(!is_error, "{content}");
        // Marker fires; shown + omitted = 300 with at least one cluster shown.
        let marker_at = content
            .find("[more: ")
            .unwrap_or_else(|| panic!("no omission marker: {content}"));
        let marker_end = content[marker_at..].find(']').unwrap() + marker_at;
        let omitted: usize = content[marker_at..marker_end]
            .trim_start_matches("[more: ")
            .trim_end_matches(" clusters omitted")
            .parse()
            .unwrap();
        let shown = content.matches(" (exact-phrase)").count();
        assert_eq!(shown + omitted, 300, "{content}");
        assert!(shown > 0, "{content}");
        assert_eq!(
            shown, shown96,
            "reserve pushed out exactly one cluster: {content}"
        );
        assert!(
            content.contains("300 clusters in 1 file"),
            "header names the corpus: {content}"
        );
        // The output is EXACTLY header + reserve-limited body + marker, and
        // the band pins hold: without the reserve the packing takes one more
        // cluster and overflows by `over0` chars — the fixture straddles the
        // marker band, so the invariant below is genuinely exercised.
        assert_eq!(
            content.chars().count(),
            header_len + 2 + body96 + 59 + omitted.to_string().len(), // line1\n + marker\n
            "packing drifted: {content}"
        );
        assert!(over0 > 0, "fixture drifted out of the reserve band (over0 = {over0})");
        assert!(over0 <= 24, "band too loose: over0 = {over0}");
        assert!(
            content.chars().count() <= budget * 4,
            "tgrep output {} chars exceeds the {}-token budget: {}",
            content.chars().count(),
            budget,
            content
        );
    }

    /// T38: a truncated response (`stop_reason=max_tokens`) injects the
    /// pinned advisory as the last user message before the next LLM call —
    /// transcript + memory — and records exactly one `output_truncated`
    /// event and log line.
    #[test]
    fn truncated_response_injects_pinned_advisory_and_event() {
        let tmp = tempfile::tempdir().unwrap();
        let (llm, events, lines) = run_t38(
            &tmp,
            vec![
                truncated_tool_use_response(
                    "write_file",
                    json!({"path": "src/big.rs", "content": "…"}),
                ),
                tool_use_response("goal_complete", json!({"summary": "done"})),
            ],
        );
        assert_eq!(llm.calls.len(), 2);
        // The first call predates any response: no advisory anywhere in it.
        assert_eq!(advisory_count(&llm.calls[0].1), 0);

        // The next call ends with the pinned advisory, right after the tool
        // result of the truncated response.
        let seen = &llm.calls[1].1;
        let last = seen.last().unwrap();
        assert_eq!(last.role, "user");
        assert_eq!(last.content.len(), 1);
        assert_eq!(last.content[0].text(), Some(T38_ADVISORY));
        let second_last = &seen[seen.len() - 2];
        assert_eq!(second_last.role, "user");
        assert!(matches!(
            second_last.content[0],
            ContentBlock::Known(KnownBlock::ToolResult { .. })
        ));

        // Transcript on disk carries the same advisory.
        let on_disk = transcript::load(tmp.path()).unwrap();
        assert_eq!(advisory_count(&on_disk), 1);

        // Exactly one event on the recorded stream, exactly one
        // `output_truncated` line on the log, both before the terminal goal.
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, Event::OutputTruncated))
                .count(),
            1
        );
        let truncs: Vec<&Value> = lines
            .iter()
            .filter(|l| l["type"] == "output_truncated")
            .collect();
        assert_eq!(truncs.len(), 1, "{lines:?}");
        assert!(truncs[0]["ts"].as_str().unwrap().ends_with('Z'));
        let trunc_idx = lines.iter().position(|l| l["type"] == "output_truncated").unwrap();
        let goal_idx = lines.iter().position(|l| l["type"] == "goal").unwrap();
        assert!(trunc_idx < goal_idx);
    }

    /// T38 control: `tool_use`, `end_turn`, and an absent stop_reason are
    /// byte-identical to pre-T38 — no advisory message, no event, no line.
    #[test]
    fn non_truncated_responses_inject_nothing() {
        // stop_reason: "tool_use"
        let tmp = tempfile::tempdir().unwrap();
        let (llm, events, lines) = run_t38(
            &tmp,
            vec![
                tool_use_response("bash", json!({"command": "true"})),
                tool_use_response("goal_complete", json!({"summary": "done"})),
            ],
        );
        assert_untouched(&llm, &events, &lines);

        // stop_reason: "end_turn" (natural stop → anti-stall kick → retry)
        let tmp = tempfile::tempdir().unwrap();
        let (llm, events, lines) = run_t38(
            &tmp,
            vec![
                text_only_response("all done"),
                tool_use_response("goal_complete", json!({"summary": "done"})),
            ],
        );
        assert_untouched(&llm, &events, &lines);

        // stop_reason absent entirely.
        let tmp = tempfile::tempdir().unwrap();
        let (llm, events, lines) = run_t38(
            &tmp,
            vec![
                json!({
                    "usage": {"input_tokens": 10, "output_tokens": 5},
                    "content": [{"type": "text", "text": "working"}],
                }),
                tool_use_response("goal_complete", json!({"summary": "done"})),
            ],
        );
        assert_untouched(&llm, &events, &lines);
    }

    /// T38: two consecutive truncated responses → two advisories — no
    /// one-shot latch (contrast: the T13 budget-low warning). History
    /// accumulates, so the third call carries both.
    #[test]
    fn two_truncated_responses_inject_two_advisories() {
        let tmp = tempfile::tempdir().unwrap();
        let (llm, events, lines) = run_t38(
            &tmp,
            vec![
                truncated_tool_use_response("bash", json!({"command": "echo one"})),
                truncated_tool_use_response("bash", json!({"command": "echo two"})),
                tool_use_response("goal_complete", json!({"summary": "done"})),
            ],
        );
        assert_eq!(llm.calls.len(), 3);
        assert_eq!(advisory_count(&llm.calls[0].1), 0);
        assert_eq!(advisory_count(&llm.calls[1].1), 1);
        assert_eq!(advisory_count(&llm.calls[2].1), 2);
        // Each truncated turn's advisory is the LAST message of the
        // following call.
        assert_eq!(llm.calls[1].1.last().unwrap().content[0].text(), Some(T38_ADVISORY));
        assert_eq!(llm.calls[2].1.last().unwrap().content[0].text(), Some(T38_ADVISORY));
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, Event::OutputTruncated))
                .count(),
            2
        );
        assert_eq!(
            lines
                .iter()
                .filter(|l| l["type"] == "output_truncated")
                .count(),
            2
        );
    }

    /// T38: a truncation with no tool calls (pure text cut short) still
    /// injects the same advisory — the chunking remedy sentence applies
    /// regardless of what was cut.
    #[test]
    fn pure_text_truncation_still_injects_advisory() {
        let tmp = tempfile::tempdir().unwrap();
        let (llm, events, lines) = run_t38(
            &tmp,
            vec![
                truncated_text_response("I'll write the file star"),
                tool_use_response("goal_complete", json!({"summary": "done"})),
            ],
        );
        // Autonomous: the text-only truncated response also takes the
        // anti-stall kick; the advisory lands after it, last before the
        // next call.
        let seen = &llm.calls[1].1;
        let last = seen.last().unwrap();
        assert_eq!(last.role, "user");
        assert_eq!(last.content[0].text(), Some(T38_ADVISORY));
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, Event::OutputTruncated))
                .count(),
            1
        );
        assert_eq!(
            lines
                .iter()
                .filter(|l| l["type"] == "output_truncated")
                .count(),
            1
        );
    }

    /// An unwritable events log never aborts the run: poison the path with
    /// a directory so every append fails, and the scripted run still
    /// completes through the real sink.
    #[test]
    fn drive_loop_survives_unwritable_events_log() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".chug").join("events.jsonl")).unwrap();
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for(&tmp, Mode::Autonomous, &controls, &urx, None, &observ::Sink::Noop);
        let mut knobs = knobs_with(5);
        let mut llm = ScriptedLlm::new(vec![
            tool_use_response("bash", json!({"command": "true"})),
            tool_use_response("goal_complete", json!({"summary": "finished anyway"})),
        ]);
        let mut gate = None;
        let mut messages = Vec::new();
        let mut sink = RecordingSink::default();
        let outcome = drive_loop(
            &ctx,
            &mut knobs,
            &mut llm,
            &mut gate,
            &mut messages,
            Some("check: true".to_string()),
            &mut sink,
            &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
        )
        .unwrap();
        assert!(matches!(outcome, DriveOutcome::RunFinished(0)));
        assert!(
            sink.0.iter().any(|e| matches!(e, Event::GoalAccepted { .. })),
            "run completes with the events log failing underneath"
        );
    }

    // ---------- T25: failure-aware event previews ----------

    #[test]
    fn error_preview_is_tail_anchored_long_content() {
        let marker = "failures:\n    tests::the_flaky_one";
        let content = format!("{}{marker}", "x".repeat(3000));
        let preview = tool_result_preview(&content, true);
        assert_eq!(preview.chars().count(), ERROR_PREVIEW_TAIL_CHARS);
        assert!(
            preview.ends_with(marker),
            "the failing test's name at the end survives: ...{}",
            preview.chars().skip(ERROR_PREVIEW_TAIL_CHARS - 40).collect::<String>()
        );
        // Exactly the last 2000 chars of the content, nothing else.
        let expected: String = content
            .chars()
            .skip(content.chars().count() - ERROR_PREVIEW_TAIL_CHARS)
            .collect();
        assert_eq!(preview, expected);
    }

    #[test]
    fn error_preview_short_content_kept_whole() {
        let content = "FAILED tests::small_failure".to_string();
        assert_eq!(tool_result_preview(&content, true), content);
    }

    #[test]
    fn ok_preview_keeps_500_char_head_byte_identical() {
        // Head/tail distinguishable: a head-take(500) is all 'a', a tail
        // window would end in 'b'.
        let content = format!("{}{}", "a".repeat(800), "b".repeat(200));
        let preview = tool_result_preview(&content, false);
        assert_eq!(preview, "a".repeat(500));
    }

    #[test]
    fn ok_preview_short_content_kept_whole() {
        let content = "wrote 5 bytes".to_string();
        assert_eq!(tool_result_preview(&content, false), content);
    }

    /// T25, end to end through drive_loop: a failing bash command whose
    /// output exceeds the window carries its unique end-of-output marker
    /// into the emitted `Event::ToolResult` preview (pre-T25 the flat
    /// 500-char head dropped it, which is what hid the flaky test's name).
    #[test]
    fn error_tool_result_preview_ends_with_output_tail() {
        let tmp = tempfile::tempdir().unwrap();
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for(&tmp, Mode::Autonomous, &controls, &urx, None, &observ::Sink::Noop);
        let mut knobs = knobs_with(5);
        let marker = "T25-TAIL-MARKER-the-flaky-test";
        let cmd = format!("printf '%s' '{}{marker}'; exit 7", "x".repeat(2200));
        let mut llm = ScriptedLlm::new(vec![
            tool_use_response("bash", json!({"command": cmd})),
            tool_use_response("goal_complete", json!({"summary": "done"})),
        ]);
        let mut gate = None;
        let mut messages = Vec::new();
        let mut sink = RecordingSink::default();
        let outcome = drive_loop(
            &ctx,
            &mut knobs,
            &mut llm,
            &mut gate,
            &mut messages,
            Some("check: true".to_string()),
            &mut sink,
            &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
        )
        .unwrap();
        assert!(matches!(outcome, DriveOutcome::RunFinished(0)));

        let (ok, preview) = sink
            .0
            .iter()
            .find_map(|e| match e {
                Event::ToolResult { name, ok, preview, .. } if name == "bash" => {
                    Some((*ok, preview.clone()))
                }
                _ => None,
            })
            .expect("bash tool_result event");
        assert!(!ok, "the failing command is an error result");
        assert_eq!(preview.chars().count(), ERROR_PREVIEW_TAIL_CHARS);
        assert!(
            preview.ends_with(&format!("{marker}\n[exit code: 7]")),
            "the emitted preview ends with the output tail"
        );
    }

    /// T25 control: a successful tool result keeps the pre-T25 500-char
    /// head preview, byte-identical.
    #[test]
    fn ok_tool_result_preview_keeps_500_char_head() {
        let tmp = tempfile::tempdir().unwrap();
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for(&tmp, Mode::Autonomous, &controls, &urx, None, &observ::Sink::Noop);
        let mut knobs = knobs_with(5);
        let cmd = format!("printf '%s' '{}'", "y".repeat(800));
        let mut llm = ScriptedLlm::new(vec![
            tool_use_response("bash", json!({"command": cmd})),
            tool_use_response("goal_complete", json!({"summary": "done"})),
        ]);
        let mut gate = None;
        let mut messages = Vec::new();
        let mut sink = RecordingSink::default();
        let outcome = drive_loop(
            &ctx,
            &mut knobs,
            &mut llm,
            &mut gate,
            &mut messages,
            Some("check: true".to_string()),
            &mut sink,
            &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
        )
        .unwrap();
        assert!(matches!(outcome, DriveOutcome::RunFinished(0)));

        let (ok, preview) = sink
            .0
            .iter()
            .find_map(|e| match e {
                Event::ToolResult { name, ok, preview, .. } if name == "bash" => {
                    Some((*ok, preview.clone()))
                }
                _ => None,
            })
            .expect("bash tool_result event");
        assert!(ok, "the succeeding command is not an error result");
        assert_eq!(preview, "y".repeat(500), "ok preview is the 500-char head");
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

        fn model(&self) -> &str {
            "tool-recording-model"
        }
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

    pub(crate) fn tool_use_response(name: &str, input: Value) -> Value {
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

    /// T15: a text-only response with an explicit (input, output) usage, so
    /// scripted runs can move the cumulative token counters in big steps.
    fn big_usage_text_response(text: &str, usage: (u64, u64)) -> Value {
        json!({
            "stop_reason": "end_turn",
            "usage": {"input_tokens": usage.0, "output_tokens": usage.1},
            "content": [{"type": "text", "text": text}],
        })
    }

    pub(crate) fn ctx_for<'a>(
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
            plan_out: None,
        }
    }

    pub(crate) fn knobs_with(max_iters: u32) -> TurnKnobs {
        TurnKnobs {
            spec_path: None,
            goal: None,
            check_cmd: None,
            max_iters,
            max_minutes: 120,
            max_tokens: 0,
        }
    }

    /// T15: knobs with a token budget (`0` = unlimited, like [`knobs_with`]).
    fn knobs_with_tokens(max_iters: u32, max_tokens: u64) -> TurnKnobs {
        TurnKnobs {
            max_tokens,
            ..knobs_with(max_iters)
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

    // ---------- T55: .chug/driver.lock — run-path acquire/release ----------

    /// Test transport feeding scripted response bodies through the real
    /// Client, so tests can run the FULL `run_loop` (lock acquire at startup,
    /// release at exit) to goal-acceptance without network.
    struct ScriptedTransport(std::sync::Mutex<std::collections::VecDeque<Value>>);

    impl crate::api::Transport for ScriptedTransport {
        fn send(
            &self,
            _url: &str,
            _headers: &[(String, String)],
            _body: &str,
        ) -> Result<crate::api::RawResponse, crate::api::TransportError> {
            let body = self
                .0
                .lock()
                .unwrap()
                .pop_front()
                .expect("scripted transport exhausted");
            Ok(crate::api::RawResponse {
                status: 200,
                headers: Vec::new(),
                body: body.to_string(),
            })
        }
    }

    fn scripted_client(responses: Vec<Value>) -> Client {
        Client::with_transport_for_tests(
            Arc::new(ScriptedTransport(std::sync::Mutex::new(responses.into()))),
            "test-model",
        )
    }

    fn scripted_accepting_run_config(tmp: &tempfile::TempDir, spec: &Path, resume: bool) -> RunConfig {
        // A run that would accept immediately: the LLM answers goal_complete
        // on its first (and only) call; the spec's `check: true` accepts it.
        let (stx, srx) = mpsc::channel();
        drop(stx);
        RunConfig {
            cwd: tmp.path().to_path_buf(),
            spec_path: spec.to_path_buf(),
            goal: "x".to_string(),
            model: "test-model".to_string(),
            max_iters: 5,
            max_minutes: 10,
            max_tokens: 0,
            resume,
            controls: Controls {
                abort: Arc::new(AtomicBool::new(false)),
                steering_rx: srx,
            },
            risk_gate: false,
            bash_timeout: Duration::from_secs(tools::BASH_TIMEOUT_SECS),
            mcp_config: None,
            mcp_off: true,
        }
    }

    /// Release leg (T55 req 5): a scripted run to goal-acceptance leaves NO
    /// lock behind, and a second scripted run in the same cwd acquires
    /// cleanly — the same-cwd successor is never blocked.
    #[test]
    fn run_releases_lock_on_goal_acceptance_and_successor_acquires() {
        let tmp = tempfile::tempdir().unwrap();
        let spec = write_spec(&tmp);
        let responses = || {
            vec![tool_use_response("goal_complete", json!({"summary": "wrapped up"}))]
        };

        let mut sink = RecordingSink::default();
        let code = run_loop(
            scripted_accepting_run_config(&tmp, &spec, false),
            scripted_client(responses()),
            None,
            &mut sink,
            &observ::Sink::Noop,
        )
        .unwrap();
        assert_eq!(code, 0, "goal accepted");
        assert!(
            !driver_lock::lock_path(tmp.path()).exists(),
            "the lock is released on the goal-acceptance exit path"
        );

        // Second scripted run, same cwd: acquires cleanly (nothing stale).
        let mut sink2 = RecordingSink::default();
        let code2 = run_loop(
            scripted_accepting_run_config(&tmp, &spec, true),
            scripted_client(responses()),
            None,
            &mut sink2,
            &observ::Sink::Noop,
        )
        .unwrap();
        assert_eq!(code2, 0);
        assert!(!driver_lock::lock_path(tmp.path()).exists());
    }

    /// Run-path integration pin (T55): a run that finds a lock held by a
    /// LIVE non-chug process (a real `sleep`, argv lacking chug) RECLAIMS it
    /// and proceeds — its events/transcript writes land — proving the check
    /// sits before the appends without blocking them. A stale (dead-holder)
    /// lock never blocks the next run either.
    #[test]
    fn run_reclaims_lock_held_by_live_non_chug_process_and_writes_proceed() {
        let tmp = tempfile::tempdir().unwrap();
        let spec = write_spec(&tmp);
        let mut holder = std::process::Command::new("sleep")
            .arg("37")
            .spawn()
            .expect("spawning sleep");
        // Hand-write the lock as if the sleep held it (alive, argv ≠ chug).
        fs::create_dir_all(tmp.path().join(".chug")).unwrap();
        fs::write(driver_lock::lock_path(tmp.path()), format!("{}\n", holder.id()))
            .expect("seeding the foreign lock");

        let mut sink = RecordingSink::default();
        let code = run_loop(
            scripted_accepting_run_config(&tmp, &spec, false),
            scripted_client(vec![tool_use_response(
                "goal_complete",
                json!({"summary": "wrapped up"}),
            )]),
            None,
            &mut sink,
            &observ::Sink::Noop,
        )
        .unwrap();
        assert_eq!(code, 0, "the live-but-not-chug holder never blocks a run");

        // The run's own writes proceeded: the transcript holds this run's
        // goal message (the reclaim happened before any append, and nothing
        // was blocked by the pre-existing file).
        let messages = transcript::load(tmp.path()).unwrap();
        assert!(
            messages
                .iter()
                .any(|m| m.content[0].text().unwrap_or_default().starts_with("Goal: x\n")),
            "run wrote its transcript despite the pre-existing lock"
        );
        let events = fs::read_to_string(tmp.path().join(".chug/events.jsonl")).unwrap();
        assert!(events.contains("\"run_start\""), "events log written");
        assert!(!driver_lock::lock_path(tmp.path()).exists(), "released at exit");

        holder.kill().unwrap();
        holder.wait().unwrap();
    }

    /// A SIGKILLed holder leaves a stale lock BY DESIGN; the next run must
    /// reclaim it transparently (T55 req 4/5, acceptance row 2).
    #[test]
    fn run_reclaims_stale_lock_left_by_killed_holder() {
        let tmp = tempfile::tempdir().unwrap();
        let spec = write_spec(&tmp);
        let mut holder = std::process::Command::new("sleep")
            .arg("37")
            .spawn()
            .expect("spawning sleep");
        fs::create_dir_all(tmp.path().join(".chug")).unwrap();
        fs::write(driver_lock::lock_path(tmp.path()), format!("{}\n", holder.id()))
            .expect("seeding the doomed lock");
        // SIGKILL-class death: no destructor runs, the lock file survives.
        holder.kill().unwrap();
        holder.wait().unwrap();
        assert!(driver_lock::lock_path(tmp.path()).exists(), "stale by design");

        let mut sink = RecordingSink::default();
        let code = run_loop(
            scripted_accepting_run_config(&tmp, &spec, false),
            scripted_client(vec![tool_use_response(
                "goal_complete",
                json!({"summary": "wrapped up"}),
            )]),
            None,
            &mut sink,
            &observ::Sink::Noop,
        )
        .unwrap();
        assert_eq!(code, 0, "a stale lock never blocks the next run");
        assert!(!driver_lock::lock_path(tmp.path()).exists());
    }

    /// Chat exemption (T55 req 6): chat's turn path drives the SAME
    /// drive_loop but never the run startup path, so a chat turn in a cwd
    /// leaves any lock file exactly as it found it — never created, never
    /// removed. drive_loop is the shared iteration loop; run_loop is the
    /// only acquire site, and run_chat never calls it.
    #[test]
    fn chat_turn_never_creates_or_removes_the_driver_lock() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join(".chug")).unwrap();
        // A pre-existing lock file (as a live run would leave it) that the
        // chat turn must neither consume nor delete.
        fs::write(driver_lock::lock_path(tmp.path()), format!("{}\n", i32::MAX)).unwrap();

        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for(&tmp, Mode::Chat, &controls, &urx, None, &observ::Sink::Noop);
        let mut knobs = knobs_with(3);
        let mut llm = ScriptedLlm::new(vec![text_only_response("done, idle")]);
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
            &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
        )
        .unwrap();
        assert!(matches!(outcome, DriveOutcome::TurnEnded(_)));
        let text = fs::read_to_string(driver_lock::lock_path(tmp.path())).unwrap();
        assert_eq!(
            text.lines().next().unwrap().trim(),
            i32::MAX.to_string(),
            "chat neither acquired (rewrote) nor removed the existing lock"
        );
        // And no run started here, so no lock was created either.
        assert_eq!(fs::read_to_string(driver_lock::lock_path(tmp.path())).unwrap(), text);
    }

    // ===================== T73: plan mode =====================

    /// Like `ctx_for` but with Mode::Plan and plan_out — the `--out` path
    /// submit_plan writes.
    fn ctx_for_plan<'a>(
        tmp: &'a tempfile::TempDir,
        plan_out: Option<&'a Path>,
        controls: &'a Controls,
        urx: &'a Receiver<SlashUpdate>,
    ) -> LoopCtx<'a> {
        LoopCtx {
            cwd: tmp.path(),
            mode: Mode::Plan,
            controls,
            updates: urx,
            bash_timeout: Duration::from_secs(1),
            trace: None,
            obs: &observ::Sink::Noop,
            plan_out,
        }
    }

    /// Schema-filter pin: the tool list a plan-mode run sends to the API is
    /// EXACTLY the five names (set compare with exact cardinality — a sixth
    /// added or one dropped turns this RED).
    #[test]
    fn plan_mode_advertises_exactly_the_five_tool_schemas() {
        let tmp = tempfile::tempdir().unwrap();
        let out = tmp.path().join("plan.md");
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for_plan(&tmp, Some(&out), &controls, &urx);
        let mut knobs = knobs_with(5);
        let mut llm = ToolRecordingLlm::new(vec![tool_use_response(
            "submit_plan",
            json!({"plan": "# Plan\n\n- step one\n- step two\n"}),
        )]);
        let mut gate = None;
        let mut messages = Vec::new();
        let mut sink = RecordingSink::default();
        let outcome = drive_loop(
            &ctx,
            &mut knobs,
            &mut llm,
            &mut gate,
            &mut messages,
            None,
            &mut sink,
            &mut McpRegistry::new(ctx.cwd, true, None).unwrap(),
        )
        .unwrap();
        assert!(matches!(outcome, DriveOutcome::RunFinished(0)), "{outcome:?}");
        let mut names: Vec<String> = llm.recorded_tools[0]
            .iter()
            .filter_map(|t| t.get("name").and_then(Value::as_str))
            .map(String::from)
            .collect();
        names.sort();
        assert_eq!(names.len(), 5, "exact cardinality five: {names:?}");
        assert_eq!(
            names,
            vec!["glob", "grep", "list_dir", "read_file", "submit_plan"],
            "the plan surface is exactly the five read-only tools + submit_plan"
        );
    }

    /// Rejection sweep through the LOOP (T72 sweep-the-family doctrine): one
    /// leg per excluded registered tool. Scripted tool_use of the excluded
    /// name in plan mode → tool error naming the allowed set; the loop
    /// CONTINUES (the scripted follow-up submit_plan runs); the tool never
    /// executed (per-leg side-effect pin).
    #[test]
    fn plan_mode_rejects_every_excluded_tool_and_the_loop_continues() {
        let excluded = [
            "write_file",
            "edit_file",
            "bash",
            "delegate",
            "web_fetch",
            "update_ledger",
            "goal_complete",
            "decision_log",
        ];
        let plan_text = "# Plan\n\nthe plan body\n";
        for name in excluded {
            let tmp = tempfile::tempdir().unwrap();
            // edit_file leg: a target the scripted edit would change.
            fs::write(tmp.path().join("target.txt"), "original").unwrap();
            // delegate leg: a would-be child dir whose delegate.log proves
            // whether a child was spawned.
            let child_dir = tempfile::tempdir().unwrap();
            let input = match name {
                "write_file" => json!({"path": "escape.md", "content": "mutated"}),
                "edit_file" => json!({"path": "target.txt", "old": "original", "new": "mutated"}),
                "bash" => json!({"command": "touch pwned-by-bash.txt"}),
                "delegate" => json!({
                    "action": "launch",
                    "cwd": child_dir.path().display().to_string(),
                    "spec": child_dir.path().join("s.md").display().to_string(),
                    "goal": "g",
                    "model": "m"
                }),
                "web_fetch" => json!({"url": "http://127.0.0.1:1/x"}),
                "update_ledger" => json!({"content": "MUTATED LEDGER"}),
                "goal_complete" => json!({"summary": "claim done"}),
                "decision_log" => json!({
                    "class": "outcome", "subject": "T73", "inputs": "i",
                    "options": "o", "choice": "landed-clean", "confidence": 0.5
                }),
                other => unreachable!("{other}"),
            };
            let out = tmp.path().join("plan.md");
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for_plan(&tmp, Some(&out), &controls, &urx);
            let mut knobs = knobs_with(5);
            let mut llm = ScriptedLlm::new(vec![
                tool_use_response(name, input),
                tool_use_response("submit_plan", json!({"plan": plan_text})),
            ]);
            let mut gate = None;
            let mut messages = Vec::new();
            let mut sink = RecordingSink::default();
            let outcome = drive_loop(
                &ctx,
                &mut knobs,
                &mut llm,
                &mut gate,
                &mut messages,
                None,
                &mut sink,
                &mut McpRegistry::new(ctx.cwd, true, None).unwrap(),
            )
            .unwrap();
            assert!(
                matches!(outcome, DriveOutcome::RunFinished(0)),
                "{name}: the loop must continue past the rejection and exit on submit_plan, got {outcome:?}"
            );
            // The loop continued: BOTH scripted calls were consumed.
            assert_eq!(llm.calls.len(), 2, "{name}: two model calls expected");
            // The rejection error reached the transcript, naming the allowed set.
            let transcript = fs::read_to_string(tmp.path().join(".chug/transcript.jsonl")).unwrap();
            assert!(
                transcript.contains("plan mode") && transcript.contains("submit_plan"),
                "{name}: transcript must carry the plan-gate rejection naming the allowed set: {transcript}"
            );
            // The plan still landed (the follow-up submit_plan executed).
            assert_eq!(
                fs::read_to_string(tmp.path().join("plan.md")).unwrap(),
                plan_text,
                "{name}: leg must not disturb the plan exit"
            );
            // Per-leg side-effect pins: the excluded tool never executed.
            match name {
                "write_file" => assert!(!tmp.path().join("escape.md").exists()),
                "edit_file" => assert_eq!(
                    fs::read_to_string(tmp.path().join("target.txt")).unwrap(),
                    "original"
                ),
                "bash" => assert!(!tmp.path().join("pwned-by-bash.txt").exists()),
                "delegate" => assert!(!child_dir.path().join(".chug/delegate.log").exists()),
                "web_fetch" => {} // the plan-gate message assert above is the leg
                "update_ledger" => assert!(!tmp.path().join("LEDGER.md").exists()),
                // goal_complete: rejected as a TOOL, not honored as the exit —
                // the second scripted call proves the loop moved past it.
                "goal_complete" => {}
                "decision_log" => {
                    assert!(!tmp.path().join(".chug/decisions.jsonl").exists())
                }
                other => unreachable!("{other}"),
            }
        }
    }

    /// Helpers for the LOOP-level plan tests: these drive the REAL
    /// `run_plan_loop` (driver lock, fresh rotation, the loop's own
    /// `run_start` event, budget enforcement, the submit_plan exit) with a
    /// scripted `&mut dyn Llm` — the run_turn seam pattern — so the plan
    /// startup/exit guarantees are pinned on the lines the loop actually
    /// executes, not on hand-written mirrors.
    fn plan_cfg(tmp: &tempfile::TempDir, out: Option<PathBuf>, max_iters: u32) -> PlanConfig {
        PlanConfig {
            cwd: tmp.path().to_path_buf(),
            spec_path: None,
            goal: "draft a plan".to_string(),
            model: "scripted-model".to_string(),
            max_iters,
            max_minutes: 20,
            max_tokens: 0,
            out_path: out,
        }
    }

    fn empty_plan_registry(tmp: &tempfile::TempDir) -> McpRegistry {
        // The prod shape: plan mode never initializes MCP servers.
        McpRegistry::new(tmp.path(), true, None).unwrap()
    }

    fn events_lines(cwd: &Path) -> Vec<Value> {
        fs::read_to_string(cwd.join(".chug/events.jsonl"))
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    /// (a) The run_start event the REAL loop emits carries mode "plan" —
    /// read from the events.jsonl the loop itself wrote. The pre-fix tests
    /// hand-wrote this line, so a mode drift inside run_plan_loop was
    /// invisible; here the loop's own line is the pin.
    #[test]
    fn plan_loop_run_start_event_names_the_plan_mode() {
        let tmp = tempfile::tempdir().unwrap();
        let mut llm = ScriptedLlm::new(vec![tool_use_response(
            "submit_plan",
            json!({"plan": "# Plan\n\nloop-level run_start\n"}),
        )]);
        let mut sink = RecordingSink::default();
        let code = run_plan_loop(
            plan_cfg(&tmp, None, 5),
            &mut llm,
            &mut sink,
            &observ::Sink::Noop,
            empty_plan_registry(&tmp),
        )
        .unwrap();
        assert_eq!(code, 0);
        let lines = events_lines(tmp.path());
        assert_eq!(lines[0]["type"], "run_start", "{lines:?}");
        assert_eq!(
            lines[0]["mode"], "plan",
            "the loop's own run_start must name mode \"plan\""
        );
    }

    /// (b, ensure_seeded mutant) A plan loop run in a cwd with NO ledger
    /// leaves it absent: plan mode never seeds LEDGER.md (the run path's
    /// `ledger::ensure_seeded` is deliberately not in the plan startup).
    #[test]
    fn plan_loop_run_leaves_an_absent_ledger_absent() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(!tmp.path().join("LEDGER.md").exists());
        let mut llm = ScriptedLlm::new(vec![tool_use_response(
            "submit_plan",
            json!({"plan": "# Plan\n\nno ledger writes\n"}),
        )]);
        let mut sink = RecordingSink::default();
        let code = run_plan_loop(
            plan_cfg(&tmp, None, 5),
            &mut llm,
            &mut sink,
            &observ::Sink::Noop,
            empty_plan_registry(&tmp),
        )
        .unwrap();
        assert_eq!(code, 0);
        assert!(
            !tmp.path().join("LEDGER.md").exists(),
            "a plan run must never seed LEDGER.md"
        );
    }

    /// (b, archive_stale mutant) A plan loop run leaves a PRE-EXISTING
    /// ledger and TODO.md byte-untouched: no archive rotation into .chug,
    /// no seeding, no rewrite. The run path archives any non-seed ledger at
    /// startup; plan mode must not.
    #[test]
    fn plan_loop_run_leaves_preexisting_ledger_and_todo_bytes_untouched() {
        let tmp = tempfile::tempdir().unwrap();
        let ledger = "# Ledger\n\n## Done\n- custom content\n";
        let todo = "# TODO\n\n| T1 | something | pending |\n";
        fs::write(tmp.path().join("LEDGER.md"), ledger).unwrap();
        fs::write(tmp.path().join("TODO.md"), todo).unwrap();
        let mut llm = ScriptedLlm::new(vec![tool_use_response(
            "submit_plan",
            json!({"plan": "# Plan\n\nread-only bookkeeping\n"}),
        )]);
        let mut sink = RecordingSink::default();
        let code = run_plan_loop(
            plan_cfg(&tmp, None, 5),
            &mut llm,
            &mut sink,
            &observ::Sink::Noop,
            empty_plan_registry(&tmp),
        )
        .unwrap();
        assert_eq!(code, 0);
        assert_eq!(
            fs::read_to_string(tmp.path().join("LEDGER.md")).unwrap(),
            ledger,
            "a plan run must never touch LEDGER.md"
        );
        assert_eq!(
            fs::read_to_string(tmp.path().join("TODO.md")).unwrap(),
            todo,
            "a plan run must never touch TODO.md"
        );
        let archived: Vec<String> = fs::read_dir(tmp.path().join(".chug"))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with("LEDGER"))
            .collect();
        assert!(
            archived.is_empty(),
            "a plan run must never archive the ledger: {archived:?}"
        );
    }

    /// (c) The tool list the REAL loop sends to the API is exactly the five
    /// plan tools EVEN WITH a live non-empty MCP registry present — the
    /// plan branch must never extend the advertised list with mcp schemas
    /// (an empty prod registry would make that extension invisible).
    #[test]
    fn plan_loop_advertises_exactly_five_tools_with_a_live_mcp_registry() {
        let tmp = tempfile::tempdir().unwrap();
        write_echo_server(tmp.path());
        let mcp = McpRegistry::new(tmp.path(), false, None).expect("live fake registry");
        assert!(
            !mcp.tool_schemas().is_empty(),
            "leg premise: the registry must be non-empty (fake server must be up)"
        );
        let mut llm = ToolRecordingLlm::new(vec![tool_use_response(
            "submit_plan",
            json!({"plan": "# Plan\n\nfive tools only\n"}),
        )]);
        let mut sink = RecordingSink::default();
        let code = run_plan_loop(
            plan_cfg(&tmp, None, 5),
            &mut llm,
            &mut sink,
            &observ::Sink::Noop,
            mcp,
        )
        .unwrap();
        assert_eq!(code, 0);
        let mut names: Vec<String> = llm.recorded_tools[0]
            .iter()
            .filter_map(|t| t.get("name").and_then(Value::as_str))
            .map(String::from)
            .collect();
        names.sort();
        assert_eq!(
            names,
            vec!["glob", "grep", "list_dir", "read_file", "submit_plan"],
            "the live MCP schemas must never reach a plan-mode API call: {names:?}"
        );
    }

    /// (d) A REJECTED submit_plan keeps the loop UP — no goal/accepted
    /// event, no exit 0, the conversation continues to the next model call.
    /// Two legs: an empty plan, and a sandbox-escaping --out. Killing the
    /// is_error-guard mutant: without the guard a rejected submit_plan
    /// latches, exits 0, and records a goal/accepted with a bogus summary.
    #[test]
    fn plan_loop_rejected_submit_plan_keeps_the_loop_up() {
        let plan_text = "# Plan\n\nthe real plan\n";
        let legs: Vec<(&str, Value, Option<PathBuf>, String)> = vec![
            (
                "empty plan",
                json!({"plan": ""}),
                None,
                "must not be empty".to_string(),
            ),
            (
                "sandbox escape",
                json!({"plan": plan_text}),
                Some(PathBuf::from("../plan-escape.md")),
                "escapes the cwd sandbox".to_string(),
            ),
        ];
        for (leg, input, out, err_needle) in legs {
            let tmp = tempfile::tempdir().unwrap();
            let mut llm = ScriptedLlm::new(vec![
                tool_use_response("submit_plan", input),
                // The conversation continues: the model gets the tool error
                // and answers again.
                text_only_response("let me fix that"),
            ]);
            let mut sink = RecordingSink::default();
            let code = run_plan_loop(
                plan_cfg(&tmp, out.clone(), 2),
                &mut llm,
                &mut sink,
                &observ::Sink::Noop,
                empty_plan_registry(&tmp),
            )
            .unwrap();
            // NOT exit 0: with the rejection, the run dies on the iteration
            // budget via the existing abort path.
            assert_ne!(
                code, 0,
                "{leg}: a rejected submit_plan must not exit 0 (got {code})"
            );
            // The conversation continued past the rejection: the model was
            // called again after the tool error.
            assert_eq!(llm.calls.len(), 2, "{leg}: the loop must continue");
            // The rejection reached the transcript with its specific wording.
            let transcript =
                fs::read_to_string(tmp.path().join(".chug/transcript.jsonl")).unwrap();
            assert!(
                transcript.contains(&err_needle),
                "{leg}: rejection must reach the transcript: {err_needle}"
            );
            // The events stream has NO goal/accepted — the rejection is not
            // a completion — and does record the abort.
            let lines = events_lines(tmp.path());
            assert!(
                !lines
                    .iter()
                    .any(|l| l["type"] == "goal" && l["outcome"] == "accepted"),
                "{leg}: a rejected submit_plan must not record goal/accepted: {lines:?}"
            );
            assert!(
                lines
                    .iter()
                    .any(|l| l["type"] == "abort" && l["reason"] == "iteration budget exceeded"),
                "{leg}: the run must end through the existing abort path: {lines:?}"
            );
            // And nothing was written: not inside cwd…
            assert!(
                !tmp.path().join("plan.md").exists()
                    && !tmp.path().join("plan-escape.md").exists(),
                "{leg}: no plan file may be written on a rejected submit"
            );
            // … nor outside it (the ../ traversal stays lexical-only).
            if out.is_some() {
                assert!(
                    !tmp.path().parent().unwrap().join("plan-escape.md").exists(),
                    "{leg}: the plan must never land outside the sandbox"
                );
            }
        }
    }

    /// submit_plan end-to-end through the REAL loop with `--out`: the file's
    /// bytes are exactly the plan string; exit 0; the loop's own events
    /// stream carries the plan-completed outcome (run_start mode "plan", a
    /// goal line accepted with the plan as summary) — the same machinery a
    /// run's goal acceptance uses.
    #[test]
    fn submit_plan_end_to_end_writes_out_file_and_records_the_outcome() {
        let tmp = tempfile::tempdir().unwrap();
        let out = tmp.path().join("scratch/plan.md");
        let plan_text = "# Plan\n\n1. add the flag\n2. pin the parse\n";
        let mut llm = ScriptedLlm::new(vec![tool_use_response(
            "submit_plan",
            json!({"plan": plan_text}),
        )]);
        let mut sink = RecordingSink::default();
        let code = run_plan_loop(
            plan_cfg(&tmp, Some(out.clone()), 5),
            &mut llm,
            &mut sink,
            &observ::Sink::Noop,
            empty_plan_registry(&tmp),
        )
        .unwrap();
        assert_eq!(code, 0, "an accepted submit_plan exits 0");
        // Verbatim bytes, parent dirs created.
        assert_eq!(fs::read(&out).unwrap(), plan_text.as_bytes());
        // The events stream carries the completion outcome.
        let lines = events_lines(tmp.path());
        assert_eq!(lines[0]["type"], "run_start");
        assert_eq!(
            lines[0]["mode"], "plan",
            "the loop's own run_start names the plan mode"
        );
        let goal = lines
            .iter()
            .find(|l| l["type"] == "goal" && l["outcome"] == "accepted")
            .expect("events stream must record the plan-completed outcome");
        assert_eq!(goal["summary"], plan_text, "the plan rides the goal event");
        // And the submit_plan tool call itself is on the stream as ok.
        assert!(
            lines
                .iter()
                .any(|l| l["type"] == "tool_result" && l["name"] == "submit_plan" && l["ok"] == true),
            "submit_plan tool_result missing: {lines:?}"
        );
    }

    /// stdout leg: without `--out`, the plan surfaces on stdout (via the
    /// GoalAccepted summary the ConsoleSink prints) and NO file is created.
    #[test]
    fn submit_plan_without_out_prints_the_plan_and_creates_no_file() {
        let tmp = tempfile::tempdir().unwrap();
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for_plan(&tmp, None, &controls, &urx);
        let mut knobs = knobs_with(5);
        let plan_text = "# Plan\n\nprinted, not written\n";
        let mut llm = ScriptedLlm::new(vec![tool_use_response(
            "submit_plan",
            json!({"plan": plan_text}),
        )]);
        let mut gate = None;
        let mut messages = Vec::new();
        let out_buf: std::sync::Arc<std::sync::Mutex<Vec<u8>>> = Default::default();
        let err_buf: std::sync::Arc<std::sync::Mutex<Vec<u8>>> = Default::default();
        struct SharedWriter(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);
        impl std::io::Write for SharedWriter {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(buf);
                Ok(buf.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let mut sink = crate::events::ConsoleSink::with_writers(
            Box::new(SharedWriter(out_buf.clone())),
            Box::new(SharedWriter(err_buf.clone())),
            ctx.cwd.to_path_buf(),
        );
        let outcome = drive_loop(
            &ctx,
            &mut knobs,
            &mut llm,
            &mut gate,
            &mut messages,
            None,
            &mut sink,
            &mut McpRegistry::new(ctx.cwd, true, None).unwrap(),
        )
        .unwrap();
        assert!(matches!(outcome, DriveOutcome::RunFinished(0)), "{outcome:?}");
        let stdout = String::from_utf8(out_buf.lock().unwrap().clone()).unwrap();
        assert!(
            stdout.contains("printed, not written"),
            "the plan must surface on stdout: {stdout:?}"
        );
        // No-file pin: nothing but .chug/ exists in the cwd.
        let created: Vec<String> = fs::read_dir(tmp.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(created, vec![".chug"], "no plan file may be created: {created:?}");
    }

    /// Budget exhaustion without submit_plan uses the existing abort path
    /// unchanged: nonzero exit, Aborted event naming model + budget.
    #[test]
    fn plan_mode_budget_death_uses_the_existing_abort_path() {
        let tmp = tempfile::tempdir().unwrap();
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for_plan(&tmp, None, &controls, &urx);
        let mut knobs = knobs_with(1);
        let mut llm = ScriptedLlm::new(vec![text_only_response("still thinking…")]);
        let mut gate = None;
        let mut messages = Vec::new();
        let mut sink = RecordingSink::default();
        let outcome = drive_loop(
            &ctx,
            &mut knobs,
            &mut llm,
            &mut gate,
            &mut messages,
            None,
            &mut sink,
            &mut McpRegistry::new(ctx.cwd, true, None).unwrap(),
        )
        .unwrap();
        assert!(matches!(outcome, DriveOutcome::RunFinished(1)), "{outcome:?}");
        let events = fs::read_to_string(tmp.path().join(".chug/events.jsonl")).unwrap();
        assert!(events.contains("\"type\":\"abort\""), "{events}");
        assert!(events.contains("iteration budget exceeded"), "{events}");
        assert!(events.contains("scripted-model"), "{events}");
        assert!(events.contains("\"budget_kind\":\"iterations\""), "{events}");
    }

    /// Regression pin: the run-mode advertised tool list equals the exact
    /// pre-change set — thirteen tools today (T73 added plan mode's surface
    /// without touching this list; T76 added tgrep and updated this pin in
    /// the same diff). If a rebase changes the set, update this pin in the
    /// same diff and say so.
    #[test]
    fn run_mode_advertised_tool_list_is_exactly_the_pre_change_set() {
        let schemas = crate::tools::tool_schemas();
        let mut names: Vec<&str> = schemas
            .iter()
            .filter_map(|t| t.get("name").and_then(Value::as_str))
            .collect();
        names.sort_unstable();
        let mut expected = [
            "read_file",
            "write_file",
            "edit_file",
            "bash",
            "grep",
            "tgrep",
            "glob",
            "list_dir",
            "update_ledger",
            "goal_complete",
            "delegate",
            "web_fetch",
            "decision_log",
        ];
        expected.sort_unstable();
        assert_eq!(names, expected, "run-mode tool list changed — update this pin in the same diff and say so");
    }

    /// submit_plan absence pin: the run-mode and chat-mode advertised tool
    /// lists do NOT contain submit_plan; the plan-mode list does.
    /// (One test, three assertions.)
    #[test]
    fn submit_plan_is_absent_from_run_and_chat_lists_present_in_plan() {
        // Run surface: the builtin registry (drive_loop extends it with MCP
        // only, never submit_plan).
        let run_names: Vec<String> = crate::tools::tool_schemas()
            .iter()
            .filter_map(|t| t.get("name").and_then(Value::as_str))
            .map(String::from)
            .collect();
        assert!(
            !run_names.contains(&"submit_plan".to_string()),
            "run-mode list must not advertise submit_plan: {run_names:?}"
        );
        // Chat surface: drive a real run_turn (the chat path) with a
        // tool-recording LLM and inspect the advertised array.
        let tmp = tempfile::tempdir().unwrap();
        let mut client = ToolRecordingLlm::new(vec![text_only_response("hi")]);
        let mut messages = vec![Message::user(vec![ContentBlock::text_block(
            "hello".to_string(),
        )])];
        let mut gate = None;
        run_turn(
            tmp.path(),
            &mut client,
            &mut gate,
            &mut messages,
            &Controls::detached(),
            &mpsc::channel().1,
            &mut knobs_with(5),
            Duration::from_secs(tools::BASH_TIMEOUT_SECS),
            &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
            None,
            &observ::Sink::Noop,
            &mut RecordingSink::default(),
        )
        .unwrap();
        let chat_names: Vec<String> = client.recorded_tools[0]
            .iter()
            .filter_map(|t| t.get("name").and_then(Value::as_str))
            .map(String::from)
            .collect();
        assert!(
            !chat_names.contains(&"submit_plan".to_string()),
            "chat-mode list must not advertise submit_plan: {chat_names:?}"
        );
        // Plan surface: the five, submit_plan included.
        let plan_names: Vec<String> = crate::plan::tool_schemas()
            .iter()
            .filter_map(|t| t.get("name").and_then(Value::as_str))
            .map(String::from)
            .collect();
        assert!(
            plan_names.contains(&"submit_plan".to_string()),
            "plan-mode list must advertise submit_plan: {plan_names:?}"
        );
    }

    // ---------- T83: .chug/hooks.json (PreToolUse veto + PostToolUse advisory) ----------

    /// Write a hooks.json configuring one PreToolUse entry and one
    /// PostToolUse entry into `cwd/.chug/`.
    fn write_hooks_json(cwd: &Path, pre: Value, post: Value) {
        let dir = cwd.join(".chug");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            hooks::hooks_path(cwd),
            json!({"hooks": {"PreToolUse": pre, "PostToolUse": post}}).to_string(),
        )
        .unwrap();
    }

    fn hook_entry(glob: &str, command: &str) -> Value {
        json!({"match": glob, "command": command})
    }

    /// THE VETO LEG (kills the allow-by-default mutant): a PreToolUse hook
    /// vetoes a `bash` call that WOULD have created a file — the file does
    /// not exist, the model receives a tool error carrying `[hook veto]` +
    /// the hook's stderr, and the loop stays alive (the next turn proceeds
    /// to completion).
    #[test]
    fn hook_veto_blocks_bash_execution_and_loop_continues() {
        let tmp = tempfile::tempdir().unwrap();
        write_hooks_json(
            tmp.path(),
            json!([hook_entry("bash", "echo vetoing-hook-stderr >&2; exit 2")]),
            json!([]),
        );
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for(&tmp, Mode::Chat, &controls, &urx, None, &observ::Sink::Noop);
        let mut knobs = knobs_with(5);
        let mut llm = ScriptedLlm::new(vec![
            tool_use_response("bash", json!({"command": "echo created > veto-marker.txt"})),
            text_only_response("routed around the veto"),
        ]);
        let mut gate = None;
        let mut messages = Vec::new();
        let mut sink = RecordingSink::default();
        let outcome = drive_loop(
            &ctx,
            &mut knobs,
            &mut llm,
            &mut gate,
            &mut messages,
            None,
            &mut sink,
            &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
        )
        .unwrap();
        // The loop is alive: the next turn ran to natural completion.
        assert!(matches!(outcome, DriveOutcome::TurnEnded(TurnEndReason::Completed)), "{outcome:?}");
        // The tool did NOT execute.
        assert!(
            !tmp.path().join("veto-marker.txt").exists(),
            "a vetoed bash call must never execute"
        );
        // The model received a tool error with the veto text.
        let (content, is_error) = tool_result_text(&messages).expect("a tool result exists");
        assert!(is_error, "the veto is a tool error: {content:?}");
        assert!(content.starts_with("[hook veto] "), "{content:?}");
        assert!(content.contains("vetoing-hook-stderr"), "carries the hook stderr: {content:?}");
        // The veto is NOT in the transcript as executed output.
        let transcript = fs::read_to_string(tmp.path().join(".chug/transcript.jsonl")).unwrap();
        assert!(!transcript.contains("created\n"), "no execution output: {transcript:?}");
        // The events log carries the veto fire line.
        let lines = events_jsonl(&tmp);
        let hook_lines: Vec<&Value> = lines.iter().filter(|l| l["type"] == "hook").collect();
        assert_eq!(hook_lines.len(), 1, "{lines:?}");
        assert_eq!(hook_lines[0]["event"], "PreToolUse");
        assert_eq!(hook_lines[0]["tool"], "bash");
        assert_eq!(hook_lines[0]["exit"], 2);
        assert_eq!(hook_lines[0]["veto"], true);
        assert!(hook_lines[0]["duration_ms"].is_u64(), "{:?}", hook_lines[0]);
        assert!(hook_lines[0]["command"].as_str().unwrap().contains("vetoing-hook-stderr"));
    }

    /// CLASS-SWEEP killing test (T83 fix-up, kills the
    /// post-fires-on-vetoed-call mutant — the validator round-1 blocking
    /// finding): with BOTH a PreToolUse vetoing hook AND a PostToolUse hook
    /// configured, a vetoed call never executed, so NO PostToolUse hook
    /// fires — the vetoed result is EXACTLY `[hook veto] <stderr>` (req 2's
    /// shape, no `[hook]` advisory riding it) and events.jsonl carries no
    /// PostToolUse fire line for the call. Revert the driver's `!blocked`
    /// guard and this test fails (a phantom PostToolUse line appears and
    /// the advisory mutates the veto text).
    #[test]
    fn hook_veto_result_is_exact_and_fires_no_post_hook() {
        let tmp = tempfile::tempdir().unwrap();
        write_hooks_json(
            tmp.path(),
            json!([hook_entry("bash", "echo vetoing-hook-stderr >&2; exit 2")]),
            json!([hook_entry("bash", "echo POST-MUST-NOT-FIRE-ON-VETO")]),
        );
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for(&tmp, Mode::Chat, &controls, &urx, None, &observ::Sink::Noop);
        let mut knobs = knobs_with(5);
        let mut llm = ScriptedLlm::new(vec![
            tool_use_response("bash", json!({"command": "echo created > veto-marker.txt"})),
            text_only_response("routed around the veto"),
        ]);
        let mut gate = None;
        let mut messages = Vec::new();
        let mut sink = RecordingSink::default();
        drive_loop(
            &ctx,
            &mut knobs,
            &mut llm,
            &mut gate,
            &mut messages,
            None,
            &mut sink,
            &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
        )
        .unwrap();
        // The tool did NOT execute.
        assert!(!tmp.path().join("veto-marker.txt").exists());
        // The vetoed result is EXACTLY req 2's shape: `[hook veto] ` + the
        // hook's trimmed stderr — no PostToolUse advisory appended.
        let (content, is_error) = tool_result_text(&messages).expect("a tool result exists");
        assert!(is_error, "{content:?}");
        assert_eq!(content, "[hook veto] vetoing-hook-stderr", "{content:?}");
        // Exactly ONE hook fire line, and it is the PreToolUse veto — no
        // phantom PostToolUse line for a call whose tool never ran.
        let lines = events_jsonl(&tmp);
        let hook_lines: Vec<&Value> = lines.iter().filter(|l| l["type"] == "hook").collect();
        assert_eq!(hook_lines.len(), 1, "no PostToolUse fire on a vetoed call: {lines:?}");
        assert_eq!(hook_lines[0]["event"], "PreToolUse");
        assert_eq!(hook_lines[0]["veto"], true);
    }

    /// CLASS-SWEEP killing test (T83 fix-up, kills the
    /// post-fires-on-gate-blocked-call mutant): a risk-gate-blocked bash
    /// call never executed either (the gate returns the block error before
    /// dispatch), so NO PostToolUse hook fires — the blocked result carries
    /// no `[hook]` advisory and events.jsonl carries no PostToolUse fire
    /// line. Revert the driver's `!blocked` guard and this test fails.
    #[test]
    fn risk_gate_block_result_fires_no_post_hook() {
        let tmp = tempfile::tempdir().unwrap();
        write_hooks_json(
            tmp.path(),
            json!([]),
            json!([hook_entry("bash", "echo POST-MUST-NOT-FIRE-ON-GATE-BLOCK")]),
        );
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for(&tmp, Mode::Chat, &controls, &urx, None, &observ::Sink::Noop);
        let mut knobs = knobs_with(5);
        // A judge that always blocks (p_destructive 0.9 >= threshold).
        let mut gate = Some(RiskGate::new(Box::new(CannedJudge("destructive", 0.9)), tmp.path()));
        let mut llm = ScriptedLlm::new(vec![
            tool_use_response("bash", json!({"command": "echo created > gate-marker.txt"})),
            text_only_response("routed around the block"),
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
            &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
        )
        .unwrap();
        // The tool did NOT execute.
        assert!(!tmp.path().join("gate-marker.txt").exists());
        // The block result is the gate's message — no PostToolUse advisory
        // appended, and the result is still an error.
        let (content, is_error) = tool_result_text(&messages).expect("a tool result exists");
        assert!(is_error, "{content:?}");
        assert!(
            !content.contains("[hook]"),
            "a gate-blocked call never executed; no advisory may ride it: {content:?}"
        );
        // No hook fired at all: the only configured hook is PostToolUse, and
        // a never-executed call must not produce a fire line.
        let lines = events_jsonl(&tmp);
        let hook_lines: Vec<&Value> = lines.iter().filter(|l| l["type"] == "hook").collect();
        assert_eq!(hook_lines.len(), 0, "no PostToolUse fire on a gate-blocked call: {lines:?}");
    }

    /// The allow + advisory legs (kills the post-never-fires and
    /// post-blocks-result mutants): an exit-0 PreToolUse hook lets the tool
    /// execute, and the PostToolUse echo hook's note lands IN the tool
    /// result the model receives, without changing ok/is_error.
    #[test]
    fn hook_allow_executes_tool_and_post_note_lands_in_result() {
        let tmp = tempfile::tempdir().unwrap();
        write_hooks_json(
            tmp.path(),
            json!([hook_entry("bash", "exit 0")]),
            json!([hook_entry("bash", "echo post-note-from-hook")]),
        );
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for(&tmp, Mode::Autonomous, &controls, &urx, None, &observ::Sink::Noop);
        let mut knobs = knobs_with(5);
        let mut llm = ScriptedLlm::new(vec![
            tool_use_response("bash", json!({"command": "echo created > allow-marker.txt"})),
            tool_use_response("goal_complete", json!({"summary": "done"})),
        ]);
        let mut gate = None;
        let mut messages = Vec::new();
        let mut sink = RecordingSink::default();
        let outcome = drive_loop(
            &ctx,
            &mut knobs,
            &mut llm,
            &mut gate,
            &mut messages,
            None,
            &mut sink,
            &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
        )
        .unwrap();
        assert!(matches!(outcome, DriveOutcome::RunFinished(0)), "{outcome:?}");
        // The tool executed.
        assert!(tmp.path().join("allow-marker.txt").exists(), "exit-0 hook must allow");
        // The advisory note rides the tool result, ok unchanged. (The bash
        // tool result itself is the `[exit code: n]` shape, not stdout.)
        let (content, is_error) = tool_result_text(&messages).expect("a tool result exists");
        assert!(!is_error);
        assert!(content.contains("\n\n[hook] post-note-from-hook"), "{content:?}");
        // Both fire lines, in order: Pre then Post, neither a veto.
        let lines = events_jsonl(&tmp);
        let hook_lines: Vec<&Value> = lines.iter().filter(|l| l["type"] == "hook").collect();
        assert_eq!(hook_lines.len(), 2, "{lines:?}");
        assert_eq!(hook_lines[0]["event"], "PreToolUse");
        assert_eq!(hook_lines[0]["veto"], false);
        assert_eq!(hook_lines[1]["event"], "PostToolUse");
        assert_eq!(hook_lines[1]["veto"], false);
        assert_eq!(hook_lines[1]["exit"], 0);
    }

    /// The once-per-run leg: a malformed config warns and records exactly
    /// ONE error line even when two tool calls happen — the load happens
    /// once per drive_loop invocation, not per tool call.
    #[test]
    fn malformed_hooks_config_error_line_exactly_once_despite_two_calls() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join(".chug")).unwrap();
        fs::write(hooks::hooks_path(tmp.path()), "{ not json !!!").unwrap();
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for(&tmp, Mode::Chat, &controls, &urx, None, &observ::Sink::Noop);
        let mut knobs = knobs_with(5);
        // Two tool calls in ONE response, then a clean finish.
        let mut llm = ScriptedLlm::new(vec![
            json!({
                "stop_reason": "tool_use",
                "usage": {"input_tokens": 1, "output_tokens": 1},
                "content": [
                    {"type": "tool_use", "id": "tu_1", "name": "bash", "input": {"command": "echo one"}},
                    {"type": "tool_use", "id": "tu_2", "name": "bash", "input": {"command": "echo two"}}
                ]
            }),
            text_only_response("done"),
        ]);
        let mut gate = None;
        let mut messages = Vec::new();
        let mut sink = RecordingSink::default();
        drive_loop(
            &ctx,
            &mut knobs,
            &mut llm,
            &mut gate,
            &mut messages,
            None,
            &mut sink,
            &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
        )
        .unwrap();
        // Both tools executed (fail-open), and exactly one error line.
        let lines = events_jsonl(&tmp);
        let errors: Vec<&Value> = lines.iter().filter(|l| l["type"] == "hook_error").collect();
        assert_eq!(errors.len(), 1, "{lines:?}");
        assert!(errors[0]["detail"].as_str().unwrap().contains("malformed"));
        assert!(
            !lines.iter().any(|l| l["type"] == "hook"),
            "zero hooks configured → zero fire lines: {lines:?}"
        );
    }

    /// Plan mode NEVER fires hooks: a plan session with a veto-everything
    /// hooks.json present still executes its read-only tools and exits via
    /// submit_plan (the structural exclusion in drive_loop's load site).
    #[test]
    fn plan_mode_fires_no_hooks_even_with_config_present() {
        let tmp = tempfile::tempdir().unwrap();
        write_hooks_json(
            tmp.path(),
            json!([hook_entry("*", "exit 2")]),
            json!([hook_entry("*", "echo plan-post")]),
        );
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for_plan(&tmp, None, &controls, &urx);
        let mut knobs = knobs_with(5);
        let mut llm = ScriptedLlm::new(vec![
            tool_use_response("read_file", json!({"path": "note.txt"})),
            tool_use_response("submit_plan", json!({"plan": "the plan"})),
        ]);
        fs::write(tmp.path().join("note.txt"), "planning input").unwrap();
        let mut gate = None;
        let mut messages = Vec::new();
        let mut sink = RecordingSink::default();
        let outcome = drive_loop(
            &ctx,
            &mut knobs,
            &mut llm,
            &mut gate,
            &mut messages,
            None,
            &mut sink,
            &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
        )
        .unwrap();
        assert!(matches!(outcome, DriveOutcome::RunFinished(0)), "{outcome:?}");
        // The read_file executed (the veto-everything hook did not fire).
        let (content, is_error) = tool_result_text(&messages).expect("tool results exist");
        assert!(!is_error, "read_file executed in plan mode: {content:?}");
        assert!(content.contains("planning input"), "{content:?}");
        assert!(!content.contains("[hook]"), "no advisory in plan mode: {content:?}");
        let lines = events_jsonl(&tmp);
        assert!(
            !lines.iter().any(|l| l["type"] == "hook" || l["type"] == "hook_error"),
            "plan mode fires NOTHING: {lines:?}"
        );
    }

    // ---------- T90: .chug/permissions.json deny-list ----------

    /// Write a permissions.json deny config into `cwd/.chug/`.
    fn write_permissions_json(cwd: &Path, deny: Value) {
        let dir = cwd.join(".chug");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            permissions::permissions_path(cwd),
            json!({"permissions": {"deny": deny}}).to_string(),
        )
        .unwrap();
    }

    fn write_permissions_raw(cwd: &Path, text: &str) {
        let dir = cwd.join(".chug");
        fs::create_dir_all(&dir).unwrap();
        fs::write(permissions::permissions_path(cwd), text).unwrap();
    }

    /// Every tool result in the transcript, in order (the single-result
    /// `tool_result_text` only sees the last one).
    fn all_tool_results(messages: &[Message]) -> Vec<(String, bool)> {
        messages
            .iter()
            .filter_map(|m| match &m.content[0] {
                ContentBlock::Known(KnownBlock::ToolResult { content, is_error, .. }) => {
                    Some((content.as_str().unwrap_or_default().to_string(), *is_error))
                }
                _ => None,
            })
            .collect()
    }

    /// THE DENY LEG (kills the allow-by-default mutant): a permissions.json
    /// command-glob rule denies a bash call that WOULD have created a file —
    /// the file does not exist, the model receives a `[permission denied]`
    /// tool error, and the loop stays alive.
    #[test]
    fn permission_deny_blocks_bash_execution_and_loop_continues() {
        let tmp = tempfile::tempdir().unwrap();
        // One malformed sibling (command matcher on read_file) + the valid
        // rule: the sibling must be skipped with a permission_error line
        // while the valid sibling still denies (fail-open per rule,
        // fail-closed on match).
        write_permissions_json(
            tmp.path(),
            json!([
                {"tool": "read_file", "command": "*evil*"},
                {"tool": "bash", "command": "*deny-marker*"}
            ]),
        );
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for(&tmp, Mode::Chat, &controls, &urx, None, &observ::Sink::Noop);
        let mut knobs = knobs_with(5);
        let mut llm = ScriptedLlm::new(vec![
            tool_use_response("bash", json!({"command": "echo created > deny-marker.txt"})),
            text_only_response("routed around the deny"),
        ]);
        let mut gate = None;
        let mut messages = Vec::new();
        let mut sink = RecordingSink::default();
        let outcome = drive_loop(
            &ctx,
            &mut knobs,
            &mut llm,
            &mut gate,
            &mut messages,
            None,
            &mut sink,
            &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
        )
        .unwrap();
        // The loop is alive: the next turn ran to natural completion.
        assert!(matches!(outcome, DriveOutcome::TurnEnded(TurnEndReason::Completed)), "{outcome:?}");
        // The tool did NOT execute.
        assert!(
            !tmp.path().join("deny-marker.txt").exists(),
            "a denied bash call must never execute"
        );
        // The model received a tool error with the deny text naming the rule.
        let (content, is_error) = tool_result_text(&messages).expect("a tool result exists");
        assert!(is_error, "the deny is a tool error: {content:?}");
        assert!(
            content.starts_with("[permission denied] deny bash command \"*deny-marker*\""),
            "{content:?}"
        );
        // The events log carries one deny line per deny + one error line for
        // the skipped malformed sibling — and NO hook lines of any kind.
        let lines = events_jsonl(&tmp);
        let denies: Vec<&Value> = lines.iter().filter(|l| l["type"] == "permission_denied").collect();
        assert_eq!(denies.len(), 1, "{lines:?}");
        assert_eq!(denies[0]["tool"], "bash");
        assert_eq!(denies[0]["rule"], "deny bash command \"*deny-marker*\"");
        let errors: Vec<&Value> = lines.iter().filter(|l| l["type"] == "permission_error").collect();
        assert_eq!(errors.len(), 1, "{lines:?}");
        assert!(errors[0]["detail"].as_str().unwrap().contains("deny[0]"));
        assert!(
            !lines.iter().any(|l| l["type"] == "hook"),
            "a denied call fires no hooks: {lines:?}"
        );
    }

    /// POLICY ORDER, pinned (spec req 3): with a PreToolUse veto hook AND a
    /// PostToolUse hook both matching the same call, a permission deny fires
    /// NO hook — the result text is exactly the deny text (no `[hook veto]`,
    /// no `[hook]` advisory), and events.jsonl carries zero hook lines.
    /// Flip the dispatch order and the veto text wins instead; drop the
    /// `blocked` guard and a phantom PostToolUse line appears.
    #[test]
    fn permission_deny_fires_no_hooks_before_or_after() {
        let tmp = tempfile::tempdir().unwrap();
        write_permissions_json(
            tmp.path(),
            json!([{"tool": "bash", "command": "*deny-marker*"}]),
        );
        write_hooks_json(
            tmp.path(),
            json!([hook_entry("bash", "echo vetoing-hook-stderr >&2; exit 2")]),
            json!([hook_entry("bash", "echo POST-MUST-NOT-FIRE-ON-DENY")]),
        );
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for(&tmp, Mode::Chat, &controls, &urx, None, &observ::Sink::Noop);
        let mut knobs = knobs_with(5);
        let mut llm = ScriptedLlm::new(vec![
            tool_use_response("bash", json!({"command": "echo created > deny-marker.txt"})),
            text_only_response("routed around the deny"),
        ]);
        let mut gate = None;
        let mut messages = Vec::new();
        let mut sink = RecordingSink::default();
        drive_loop(
            &ctx,
            &mut knobs,
            &mut llm,
            &mut gate,
            &mut messages,
            None,
            &mut sink,
            &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
        )
        .unwrap();
        assert!(!tmp.path().join("deny-marker.txt").exists());
        // The deny result is EXACTLY the deny text — no hook veto text, no
        // PostToolUse advisory riding it.
        let (content, is_error) = tool_result_text(&messages).expect("a tool result exists");
        assert!(is_error, "{content:?}");
        assert!(content.starts_with("[permission denied] "), "{content:?}");
        assert!(!content.contains("[hook"), "{content:?}");
        // Zero hook fire lines: not PreToolUse, not PostToolUse.
        let lines = events_jsonl(&tmp);
        assert!(
            !lines.iter().any(|l| l["type"] == "hook"),
            "a permission deny must fire NO hook: {lines:?}"
        );
    }

    /// NON-VACUOUSNESS (kills the deny-everything / matcher-inverted
    /// mutants): a non-matching command under a command rule executes
    /// normally, hooks still fire around it, and no deny line lands.
    #[test]
    fn non_matching_command_executes_and_hooks_still_fire() {
        let tmp = tempfile::tempdir().unwrap();
        write_permissions_json(
            tmp.path(),
            json!([{"tool": "bash", "command": "*forbidden*"}]),
        );
        write_hooks_json(
            tmp.path(),
            json!([hook_entry("bash", "exit 0")]),
            json!([hook_entry("bash", "echo post-note")]),
        );
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for(&tmp, Mode::Chat, &controls, &urx, None, &observ::Sink::Noop);
        let mut knobs = knobs_with(5);
        let mut llm = ScriptedLlm::new(vec![
            tool_use_response("bash", json!({"command": "echo fine > allow-marker.txt"})),
            text_only_response("done"),
        ]);
        let mut gate = None;
        let mut messages = Vec::new();
        let mut sink = RecordingSink::default();
        drive_loop(
            &ctx,
            &mut knobs,
            &mut llm,
            &mut gate,
            &mut messages,
            None,
            &mut sink,
            &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
        )
        .unwrap();
        // The tool executed.
        assert!(tmp.path().join("allow-marker.txt").exists(), "non-matching command must run");
        let (content, is_error) = tool_result_text(&messages).expect("a tool result exists");
        assert!(!is_error, "{content:?}");
        assert!(content.contains("\n\n[hook] post-note"), "{content:?}");
        let lines = events_jsonl(&tmp);
        assert!(
            !lines.iter().any(|l| l["type"] == "permission_denied"),
            "no deny on the allow path: {lines:?}"
        );
        let hook_lines: Vec<&Value> = lines.iter().filter(|l| l["type"] == "hook").collect();
        assert_eq!(hook_lines.len(), 2, "hooks fire on an allowed call: {lines:?}");
    }

    /// A whole-tool deny keeps the run alive too: the denied web_fetch call
    /// errors with the deny text (nothing is fetched) and the next turn
    /// completes.
    #[test]
    fn whole_tool_deny_blocks_web_fetch_and_loop_continues() {
        let tmp = tempfile::tempdir().unwrap();
        write_permissions_json(tmp.path(), json!([{"tool": "web_fetch"}]));
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for(&tmp, Mode::Chat, &controls, &urx, None, &observ::Sink::Noop);
        let mut knobs = knobs_with(5);
        let mut llm = ScriptedLlm::new(vec![
            tool_use_response("web_fetch", json!({"url": "https://example.com/x"})),
            text_only_response("routed around the deny"),
        ]);
        let mut gate = None;
        let mut messages = Vec::new();
        let mut sink = RecordingSink::default();
        let outcome = drive_loop(
            &ctx,
            &mut knobs,
            &mut llm,
            &mut gate,
            &mut messages,
            None,
            &mut sink,
            &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
        )
        .unwrap();
        assert!(matches!(outcome, DriveOutcome::TurnEnded(TurnEndReason::Completed)), "{outcome:?}");
        let (content, is_error) = tool_result_text(&messages).expect("a tool result exists");
        assert!(is_error, "{content:?}");
        assert_eq!(content, "[permission denied] deny web_fetch", "{content:?}");
    }

    /// PLAN MODE surfaces the deny (spec req 6): an in-process policy can
    /// only restrict further, so a `read_file *.key` rule denies that read
    /// inside a plan session while the other read-only tools keep working
    /// and the session still ends via submit_plan (the five-tool contract
    /// is unchanged).
    #[test]
    fn plan_mode_permission_deny_restricts_read_file_others_work() {
        let tmp = tempfile::tempdir().unwrap();
        write_permissions_json(
            tmp.path(),
            json!([{"tool": "read_file", "path": "*.key"}]),
        );
        fs::write(tmp.path().join("secret.key"), "PRIVATE").unwrap();
        fs::write(tmp.path().join("notes.md"), "planning input").unwrap();
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for_plan(&tmp, None, &controls, &urx);
        let mut knobs = knobs_with(5);
        let mut llm = ScriptedLlm::new(vec![
            tool_use_response("read_file", json!({"path": "secret.key"})),
            tool_use_response("read_file", json!({"path": "notes.md"})),
            tool_use_response("submit_plan", json!({"plan": "the plan"})),
        ]);
        let mut gate = None;
        let mut messages = Vec::new();
        let mut sink = RecordingSink::default();
        let outcome = drive_loop(
            &ctx,
            &mut knobs,
            &mut llm,
            &mut gate,
            &mut messages,
            None,
            &mut sink,
            &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
        )
        .unwrap();
        assert!(matches!(outcome, DriveOutcome::RunFinished(0)), "{outcome:?}");
        let results = all_tool_results(&messages);
        // submit_plan ends the session (its result need not land in the
        // transcript); the two reads are what this leg pins.
        assert!(results.len() >= 2, "{results:?}");
        // The denied read: tool error naming the rule; the key's contents
        // never reached the model.
        assert!(results[0].1, "the denied read is a tool error: {:?}", results[0]);
        assert!(
            results[0].0.starts_with("[permission denied] deny read_file path \"*.key\""),
            "{:?}",
            results[0]
        );
        // The allowed read executed and returned the file contents.
        assert!(!results[1].1, "notes.md read works in plan mode: {:?}", results[1]);
        assert!(results[1].0.contains("planning input"), "{:?}", results[1]);
        // One deny line, no hook lines (plan mode never fires hooks anyway).
        let lines = events_jsonl(&tmp);
        let denies: Vec<&Value> = lines.iter().filter(|l| l["type"] == "permission_denied").collect();
        assert_eq!(denies.len(), 1, "{lines:?}");
        assert_eq!(denies[0]["tool"], "read_file");
        assert!(!lines.iter().any(|l| l["type"] == "hook"), "{lines:?}");
    }

    /// The config fail-open leg at driver level: a malformed permissions
    /// config warns + records exactly ONE error line even with two tool
    /// calls in the run (the load happens once per drive_loop invocation),
    /// both calls execute, and the loop behaves exactly as with no config.
    #[test]
    fn malformed_permissions_config_fails_open_once_and_run_continues() {
        let tmp = tempfile::tempdir().unwrap();
        write_permissions_raw(tmp.path(), "{ not json !!!");
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for(&tmp, Mode::Chat, &controls, &urx, None, &observ::Sink::Noop);
        let mut knobs = knobs_with(5);
        // Two tool calls in ONE response, then a clean finish.
        let mut llm = ScriptedLlm::new(vec![
            json!({
                "stop_reason": "tool_use",
                "usage": {"input_tokens": 1, "output_tokens": 1},
                "content": [
                    {"type": "tool_use", "id": "tu_1", "name": "bash", "input": {"command": "echo one > pm-one.txt"}},
                    {"type": "tool_use", "id": "tu_2", "name": "bash", "input": {"command": "echo two > pm-two.txt"}}
                ]
            }),
            text_only_response("done"),
        ]);
        let mut gate = None;
        let mut messages = Vec::new();
        let mut sink = RecordingSink::default();
        drive_loop(
            &ctx,
            &mut knobs,
            &mut llm,
            &mut gate,
            &mut messages,
            None,
            &mut sink,
            &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
        )
        .unwrap();
        // Both tools executed (fail-open), and exactly one error line.
        assert!(tmp.path().join("pm-one.txt").exists() && tmp.path().join("pm-two.txt").exists());
        let lines = events_jsonl(&tmp);
        let errors: Vec<&Value> = lines.iter().filter(|l| l["type"] == "permission_error").collect();
        assert_eq!(errors.len(), 1, "{lines:?}");
        assert!(errors[0]["detail"].as_str().unwrap().contains("malformed"));
        assert!(
            !lines.iter().any(|l| l["type"] == "permission_denied"),
            "zero rules → zero denies: {lines:?}"
        );
    }

    /// Absent config = zero cost: a clean run produces no permission lines
    /// at all (the load leg never emits, and no per-call check runs).
    #[test]
    fn absent_permissions_config_produces_no_permission_lines() {
        let tmp = tempfile::tempdir().unwrap();
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for(&tmp, Mode::Chat, &controls, &urx, None, &observ::Sink::Noop);
        let mut knobs = knobs_with(5);
        let mut llm = ScriptedLlm::new(vec![
            tool_use_response("bash", json!({"command": "echo clean"})),
            text_only_response("done"),
        ]);
        let mut gate = None;
        let mut messages = Vec::new();
        let mut sink = RecordingSink::default();
        drive_loop(
            &ctx,
            &mut knobs,
            &mut llm,
            &mut gate,
            &mut messages,
            None,
            &mut sink,
            &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
        )
        .unwrap();
        let lines = events_jsonl(&tmp);
        assert!(
            !lines.iter().any(|l| l["type"].as_str().unwrap_or_default().starts_with("permission")),
            "absent config: zero permission lines: {lines:?}"
        );
    }

// ---- T91: read_file image leg through the loop ----

/// Count image blocks in an outgoing request: standalone image blocks plus
/// image entries inside tool_result array content.
fn count_image_blocks(messages: &[Message]) -> usize {
    messages
        .iter()
        .flat_map(|m| m.content.iter())
        .filter(|b| match b {
            ContentBlock::Known(KnownBlock::Image { .. }) => true,
            ContentBlock::Known(KnownBlock::ToolResult {
                content: Value::Array(items),
                ..
            }) => items
                .iter()
                .any(|i| i.get("type").and_then(Value::as_str) == Some("image")),
            _ => false,
        })
        .count()
}

/// LLM double for the T91 degrade legs: call 1 replays scripted responses;
/// call 2 fails with an HTTP 400 whose body rejects image content (the
/// endpoint-rejection shape); later calls replay the remaining responses.
/// Every call's messages are recorded so the test can assert what the
/// retried request actually carried.
struct ImageDegradeLlm {
    responses: std::collections::VecDeque<Value>,
    calls: Vec<Vec<Message>>,
}

impl ImageDegradeLlm {
    fn new(responses: Vec<Value>) -> Self {
        ImageDegradeLlm {
            responses: responses.into(),
            calls: Vec::new(),
        }
    }
}

impl Llm for ImageDegradeLlm {
    fn complete(
        &mut self,
        _system: &str,
        messages: &[Message],
        _tools: &[Value],
        _obs: &crate::api::ObsCtx<'_>,
    ) -> anyhow::Result<crate::api::Response> {
        self.calls.push(messages.to_vec());
        if self.calls.len() == 2 {
            return Err(anyhow::anyhow!(
                "LLM request failed: HTTP 400: {{\"type\":\"error\",\"error\":{{\"type\":\"invalid_request_error\",\"message\":\"Requests must not contain image content blocks\"}}}}"
            ));
        }
        self.responses
            .pop_front()
            .map(|body| crate::api::Response { body })
            .ok_or_else(|| anyhow::anyhow!("no scripted response left"))
    }

    fn set_model(&mut self, _model: &str) {}

    fn model(&self) -> &str {
        "image-degrade-model"
    }
}

/// LLM double that always fails with an UNRELATED 400 body (no image/content
/// mention) — the control leg proving the degrade never fires on it.
struct UnrelatedErrLlm {
    calls: usize,
}

impl Llm for UnrelatedErrLlm {
    fn complete(
        &mut self,
        _system: &str,
        _messages: &[Message],
        _tools: &[Value],
        _obs: &crate::api::ObsCtx<'_>,
    ) -> anyhow::Result<crate::api::Response> {
        self.calls += 1;
        Err(anyhow::anyhow!(
            "LLM request failed: HTTP 400: {{\"error\":{{\"message\":\"max_tokens: field required\"}}}}"
        ))
    }

    fn set_model(&mut self, _model: &str) {}

    fn model(&self) -> &str {
        "unrelated-err-model"
    }
}

/// The full degrade arc: the request carrying a.png's image block is rejected
/// with a 400 image-rejection body → ONE retry whose request carries ZERO
/// image blocks and the placeholder text; ONE events note; and the latch
/// downgrades the SECOND image result at wrap time (never sent).
#[test]
fn image_endpoint_400_rejection_retries_once_with_placeholder_and_latches() {
    let tmp = tempfile::tempdir().unwrap();
    let a_bytes: &[u8] = b"\x89PNG\r\n\x1a\nAAA";
    let b_bytes: &[u8] = b"\x89PNG\r\n\x1a\nBBB";
    fs::write(tmp.path().join("a.png"), a_bytes).unwrap();
    fs::write(tmp.path().join("b.png"), b_bytes).unwrap();

    let (_utx, urx) = mpsc::channel::<SlashUpdate>();
    let controls = Controls::detached();
    let ctx = ctx_for(&tmp, Mode::Autonomous, &controls, &urx, None, &observ::Sink::Noop);
    let mut knobs = knobs_with(10);
    let mut llm = ImageDegradeLlm::new(vec![
        tool_use_response("read_file", json!({"path": "a.png"})),
        tool_use_response("read_file", json!({"path": "b.png"})),
        tool_use_response("goal_complete", json!({"summary": "did it"})),
    ]);
    let mut gate = None;
    let mut messages = Vec::new();
    let mut sink = RecordingSink::default();
    let outcome = drive_loop(
        &ctx,
        &mut knobs,
        &mut llm,
        &mut gate,
        &mut messages,
        Some("check: true".to_string()),
        &mut sink,
        &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
    )
    .unwrap();
    assert!(matches!(outcome, DriveOutcome::RunFinished(0)), "{outcome:?}");

    // Call 2 — the first request carrying a.png's image block — failed; the
    // retry is call 3; the final call is 4.
    assert_eq!(llm.calls.len(), 4, "failed call + retry + one per scripted turn");
    assert_eq!(count_image_blocks(&llm.calls[0]), 0, "kick request has no images");
    assert_eq!(
        count_image_blocks(&llm.calls[1]),
        1,
        "a.png's image block rode the request the endpoint rejected"
    );
    // Retry and every later request: zero image blocks, placeholder present.
    for (i, call) in llm.calls.iter().enumerate().skip(2) {
        assert_eq!(
            count_image_blocks(call),
            0,
            "call {i} must carry zero image blocks"
        );
        let wire = serde_json::to_string(call).unwrap();
        assert!(
            wire.contains(crate::api::IMAGE_REMOVED_PLACEHOLDER),
            "call {i} carries the placeholder text: {wire}"
        );
        assert!(!wire.contains(base64_of(a_bytes).as_str()), "call {i} leaks no a.png base64");
    }
    // The latch downgraded b.png's result at WRAP time: its tool_result is
    // the short note STRING, never an array with an image block.
    let b_result = llm.calls[3]
        .iter()
        .flat_map(|m| m.content.iter())
        .find_map(|b| match b {
            ContentBlock::Known(KnownBlock::ToolResult { content, is_error, .. }) => content
                .as_str()
                .filter(|s| s.contains("b.png"))
                .map(|s| (s, *is_error)),
            _ => None,
        })
        .expect("b.png tool result rides the final request as string content");
    assert!(!b_result.1, "the image read is not an error");
    assert!(
        b_result.0.starts_with("[image: ") && b_result.0.ends_with("image/png)]"),
        "the note rides: {}",
        b_result.0
    );

    // Exactly one events note for the whole run (the latch caps it).
    let degraded = sink
        .0
        .iter()
        .filter(|e| matches!(e, Event::ImageDegraded))
        .count();
    assert_eq!(degraded, 1, "exactly one image_degraded note, not per request");
}

/// Control leg: an unrelated 400 body keeps today's fail-fast error path —
/// no retry, no degrade note.
#[test]
fn image_unrelated_400_keeps_fail_fast_error_path() {
    let tmp = tempfile::tempdir().unwrap();
    let (_utx, urx) = mpsc::channel::<SlashUpdate>();
    let controls = Controls::detached();
    let ctx = ctx_for(&tmp, Mode::Autonomous, &controls, &urx, None, &observ::Sink::Noop);
    let mut knobs = knobs_with(10);
    let mut llm = UnrelatedErrLlm { calls: 0 };
    let mut gate = None;
    let mut messages = Vec::new();
    let mut sink = RecordingSink::default();
    let err = drive_loop(
        &ctx,
        &mut knobs,
        &mut llm,
        &mut gate,
        &mut messages,
        Some("check: true".to_string()),
        &mut sink,
        &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
    )
    .unwrap_err();
    assert!(err.to_string().contains("HTTP 400"), "{err}");
    assert_eq!(llm.calls, 1, "fail fast: no degrade retry on an unrelated 400");
    assert!(
        !sink.0.iter().any(|e| matches!(e, Event::ImageDegraded)),
        "no degrade note on an unrelated 400"
    );
}

/// Events preview hygiene: an image tool result's events line carries the
/// short note — never more than a sliver of base64, in the event stream or
/// the `.chug/events.jsonl` line.
#[test]
fn image_result_events_preview_carries_short_note_no_base64() {
    let tmp = tempfile::tempdir().unwrap();
    let png: &[u8] = b"\x89PNG\r\n\x1a\nPREVIEWHYGIENE";
    fs::write(tmp.path().join("a.png"), png).unwrap();

    let (_utx, urx) = mpsc::channel::<SlashUpdate>();
    let controls = Controls::detached();
    let ctx = ctx_for(&tmp, Mode::Autonomous, &controls, &urx, None, &observ::Sink::Noop);
    let mut knobs = knobs_with(10);
    let mut llm = ScriptedLlm::new(vec![
        tool_use_response("read_file", json!({"path": "a.png"})),
        tool_use_response("goal_complete", json!({"summary": "did it"})),
    ]);
    let mut gate = None;
    let mut messages = Vec::new();
    let mut sink = RecordingSink::default();
    let outcome = drive_loop(
        &ctx,
        &mut knobs,
        &mut llm,
        &mut gate,
        &mut messages,
        Some("check: true".to_string()),
        &mut sink,
        &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
    )
    .unwrap();
    assert!(matches!(outcome, DriveOutcome::RunFinished(0)), "{outcome:?}");

    let note = format!(
        "[image: {} ({} bytes, image/png)]",
        tmp.path().join("a.png").display(),
        png.len()
    );
    let (ok, preview) = sink
        .0
        .iter()
        .find_map(|e| match e {
            Event::ToolResult { name, ok, preview, .. } if name == "read_file" => {
                Some((*ok, preview.clone()))
            }
            _ => None,
        })
        .expect("read_file tool_result event");
    assert!(ok, "the image read is ok");
    assert_eq!(preview, note, "the preview IS the short note");
    assert!(preview.len() < 100, "preview stays short: {preview}");
    assert!(!preview.contains(base64_of(png).as_str()), "no base64 in the preview");
    // The `.chug/events.jsonl` tool_result line rides the same short note.
    let lines: Vec<Value> = fs::read_to_string(tmp.path().join(".chug/events.jsonl"))
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let tr = lines
        .iter()
        .find(|l| l["type"] == "tool_result" && l["name"] == "read_file")
        .expect("events.jsonl records the read_file result");
    assert_eq!(tr["preview"], note, "events line carries the short note");
}

/// Plan-mode leg: read_file is one of the five plan tools — an image read
/// works there, riding the same array-content shape (five-tool contract is
/// pinned separately by `plan_mode_advertises_exactly_the_five_tool_schemas`).
#[test]
fn image_read_works_in_plan_mode() {
    let tmp = tempfile::tempdir().unwrap();
    let png: &[u8] = b"\x89PNG\r\n\x1a\nPLANIMG";
    fs::write(tmp.path().join("a.png"), png).unwrap();
    let out = tmp.path().join("plan.md");
    let (_utx, urx) = mpsc::channel::<SlashUpdate>();
    let controls = Controls::detached();
    let ctx = ctx_for_plan(&tmp, Some(&out), &controls, &urx);
    let mut knobs = knobs_with(5);
    let mut llm = ScriptedLlm::new(vec![
        tool_use_response("read_file", json!({"path": "a.png"})),
        tool_use_response("submit_plan", json!({"plan": "# Plan\n\nread the image\n"})),
    ]);
    let mut gate = None;
    let mut messages = Vec::new();
    let mut sink = RecordingSink::default();
    let outcome = drive_loop(
        &ctx,
        &mut knobs,
        &mut llm,
        &mut gate,
        &mut messages,
        None,
        &mut sink,
        &mut McpRegistry::new(ctx.cwd, true, None).unwrap(),
    )
    .unwrap();
    assert!(matches!(outcome, DriveOutcome::RunFinished(0)), "{outcome:?}");
    assert_eq!(count_image_blocks(&messages), 1, "no degrade in plan mode: the image rides");
    let block = messages
        .iter()
        .flat_map(|m| m.content.iter())
        .find_map(|b| match b {
            ContentBlock::Known(KnownBlock::ToolResult { content, is_error, .. }) => {
                content.as_array().map(|items| (items, *is_error))
            }
            _ => None,
        })
        .expect("the image tool result uses array content");
    assert!(!block.1, "not an error");
    let items = block.0;
    assert_eq!(items.len(), 2, "image block first, then the text note");
    assert_eq!(items[0]["type"], "image");
    assert_eq!(items[0]["source"]["media_type"], "image/png");
    assert_eq!(items[0]["source"]["data"], base64_of(png));
    assert_eq!(items[1]["type"], "text");
    assert_eq!(
        items[1]["text"],
        format!(
            "[image: {} ({} bytes, image/png)]",
            tmp.path().join("a.png").display(),
            png.len()
        )
    );
}

/// Minimal PNG bytes for tests above need their base64; re-derive it from
/// the tool layer's encoder (private) via a dispatch round-trip is overkill —
/// compute it inline with the same standard alphabet.
fn base64_of(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b0 = u32::from(chunk[0]);
        let b1 = u32::from(chunk.get(1).copied().unwrap_or(0));
        let b2 = u32::from(chunk.get(2).copied().unwrap_or(0));
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[(n >> 18 & 63) as usize] as char);
        out.push(TABLE[(n >> 12 & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            TABLE[(n >> 6 & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}
}
