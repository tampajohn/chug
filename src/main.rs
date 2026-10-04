mod api;
mod archive;
mod attach;
mod auth;
mod autospec;
mod build_info;
mod chat;
mod commands;
mod complete;
/// T204 phase 1 (F15): the baked-in judge daemon — the `chug daemon`
/// subcommand (serve/stop/status/ensure), the 0600 unix-socket HTTP/1.1
/// transport, the single-instance lock, and the auto-spawn lifecycle. The
/// inference core it hosts is `judge_model` (feature `daemon`); the client
/// half (`DaemonJudge`, `CHUG_JUDGE=daemon`) compiles in EVERY build so the
/// default (feature-off) binary still fails open with a clear message.
mod daemon;
mod delegate;
mod decisions;
mod driver;
mod driver_lock;
mod eventlog;
mod events;
mod fork;
mod fsatomic;
mod hooks;
mod riskgate;
mod ledger;
mod live_ctx;
mod observ;
mod plan;
mod tools;
mod transcript;
mod trim;
mod tui;
mod mcp;
mod mcp_http;
mod mcp_serve;
mod notify;
mod permissions;
mod sse;
mod tgrep;
mod todos;
mod valroute;
mod webfetch;
mod judge_pack;
/// T204 phase 1 (F15): the baked-in judge's inference core (RLAgent
/// checkpoint loader + candle ModernBERT + decision head). Compiled only
/// under the `daemon` feature — off by default, so the hot `chug run` path
/// never compiles candle (pinned by tests/daemon_feature_off.rs). The
/// socket server + lifecycle hosting this core live in `daemon.rs`.
#[cfg(feature = "daemon")]
mod judge_model;
mod websearch;
/// T205: the chug-side HF consumption path for laya checkpoints (revision
/// parsing, endpoint/token passthroughs, the auth-failure honesty gate).
/// Compiled in EVERY build — its pins run in the plain `cargo test` gate;
/// the feature-gated `judge_model` is its only production caller.
mod hf_hosting;
/// T222: the kev checkpoint's pure consumption logic — the pins for BOTH
/// revisions (adapter repo + base), the PEFT LoRA config parse, the merge
/// scale and the base-architecture classification. Compiled in EVERY build
/// (the `hf_hosting` precedent) so its pins run in the plain `cargo test`
/// gate; the daemon-feature-gated loader that consumes it is `kev_model`.
mod kev_config;
/// T222: the kev-layout checkpoint loader (adapter + head + provenance,
/// resolved through the same T205 machinery) and the base-architecture
/// gate. Compiled only under the `daemon` feature, like `judge_model` —
/// the feature-off build stays candle-free (tests/daemon_feature_off.rs).
#[cfg(feature = "daemon")]
mod kev_model;

/// T151: the ONE shared serialization domain for wall-clock/spawn-timing
/// tests (see the module doc). Compiled only under `cargo test` — a
/// release build carries no lock and no test code.
#[cfg(test)]
mod testsupport;

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, mpsc};
use std::thread;

use anyhow::{Context, anyhow, bail};
use clap::{Parser, Subcommand};

/// chug: autonomous coding harness — the loop is code, not conversation.
#[derive(Parser)]
#[command(name = "chug", version, about)]
struct Cli {
    #[command(subcommand)]
    command: CliCommand,
}

#[derive(Subcommand)]
enum CliCommand {
    /// Run the agent loop against a spec + goal until verified done or a budget/tripwire fires.
    Run {
        /// Path to the spec file (re-read every iteration; may contain a `check:` line).
        /// Required unless `--auto-spec` drafts the spec first (T188).
        #[arg(long, required_unless_present = "auto_spec")]
        spec: Option<PathBuf>,
        /// T188: draft the spec from the goal with one read-only plan
        /// session, gate it (structure + non-vacuous check + dry-run), and
        /// run against the drafted spec at <cwd>/.chug/auto-spec.md. With
        /// `--resume` the existing draft is reused, never redrafted.
        #[arg(long, default_value_t = false)]
        auto_spec: bool,
        /// T189: force FULL adversarial validation (the kimi child) for
        /// every item on this auto-spec'd run — the low-stakes lane
        /// predicate is off. Operator override, recorded via decision_log.
        /// Belongs to the --auto-spec group.
        #[arg(long, requires = "auto_spec", conflicts_with = "no_validate")]
        validate: bool,
        /// T189: force the gates-only lane for every item on this
        /// auto-spec'd run, regardless of the diff — the kimi validation
        /// child is skipped, never the gates (build + clippy + the full
        /// suite in the worktree, byte-clean review, and the scope check
        /// stay REQUIRED). Operator override, recorded via decision_log.
        /// Belongs to the --auto-spec group.
        #[arg(long, requires = "auto_spec", conflicts_with = "validate")]
        no_validate: bool,
        /// Goal text.
        #[arg(long)]
        goal: String,
        /// Working directory; all file/bash tools are sandboxed here. Defaults to `.`.
        #[arg(long)]
        cwd: Option<PathBuf>,
        /// Model id. Order: --model, $CHUG_MODEL, claude-sonnet-4-6.
        #[arg(long)]
        model: Option<String>,
        /// Iteration budget.
        #[arg(long, default_value_t = 40)]
        max_iters: u32,
        /// Wall-clock budget in minutes.
        #[arg(long, default_value_t = 120)]
        max_minutes: u64,
        /// Token budget: cumulative input+output tokens across the run.
        /// 0 = unlimited.
        #[arg(long, default_value_t = 0)]
        max_tokens: u64,
        /// Per-request output-token cap sent as `max_tokens` on every API
        /// call. GLM thinking blocks share this budget with the response
        /// content, so low caps truncate large tool calls (T143). Order:
        /// --max-tokens-per-request, $CHUG_MAX_TOKENS, 32768.
        #[arg(long)]
        max_tokens_per_request: Option<u32>,
        /// T192: one-shot context-occupancy warning, in estimated tokens.
        /// When the pre-call context reaches it, the driver injects a
        /// one-shot advisory naming .chug/LIVE_CTX.md editing as the remedy
        /// (an accepted edit-only turn is free, cap 3 consecutive). 0 = off.
        #[arg(long, default_value_t = 0)]
        ctx_warn_at_tokens: u64,
        /// Resume from <cwd>/.chug/transcript.jsonl.
        #[arg(long)]
        resume: bool,
        /// Run the live dashboard UI instead of headless logging.
        #[arg(long)]
        tui: bool,
        /// Classify every bash command with the laya risk judge before executing
        /// (blocks destructive commands; fails open when the judge is down).
        #[arg(long)]
        risk_gate: bool,
        /// Per-command bash timeout in seconds. Overrides $CHUG_BASH_TIMEOUT
        /// (default 120).
        #[arg(long)]
        bash_timeout: Option<u64>,
        /// Path to MCP config JSON. Overrides discovery.
        #[arg(long)]
        mcp_config: Option<PathBuf>,
        /// Disable MCP servers even if config exists.
        #[arg(long, default_value_t = false)]
        mcp_off: bool,
        /// Path to an operator-approved implementation plan (F2 phase 2a).
        /// The plan becomes the run's execution contract: its text is
        /// prepended to the first message ("implement it, then satisfy your
        /// goal's check"), and `run_start` records the path + file hash. The
        /// file must exist, be a readable regular file, and be non-empty —
        /// otherwise the run refuses to start before any `.chug/` write.
        /// The path must stay inside the working directory.
        #[arg(long)]
        approve: Option<PathBuf>,
    },
    /// Read-only planning session: explore the repo, draft an implementation
    /// plan, end with `submit_plan`. Exactly six tools (T146 added web_fetch); no other write path.
    Plan {
        /// Goal text: what the plan should accomplish.
        #[arg(long)]
        goal: String,
        /// Optional spec file (same resolution as `run`).
        #[arg(long)]
        spec: Option<PathBuf>,
        /// Where to write the plan (cwd-sandboxed path; parent dirs created).
        /// Absent → the plan prints to stdout.
        #[arg(long)]
        out: Option<PathBuf>,
        /// Working directory; plan mode is read-only here. Defaults to `.`.
        #[arg(long)]
        cwd: Option<PathBuf>,
        /// Model id. Order: --model, $CHUG_MODEL, claude-sonnet-4-6.
        #[arg(long)]
        model: Option<String>,
        /// Iteration budget.
        #[arg(long, default_value_t = 30)]
        max_iters: u32,
        /// Wall-clock budget in minutes.
        #[arg(long, default_value_t = 20)]
        max_minutes: u64,
        /// Token budget: cumulative input+output tokens across the session.
        /// 0 = unlimited.
        #[arg(long, default_value_t = 0)]
        max_tokens: u64,
        /// Per-request output-token cap sent as `max_tokens` on every API
        /// call (T143). Order: --max-tokens-per-request, $CHUG_MAX_TOKENS,
        /// 32768.
        #[arg(long)]
        max_tokens_per_request: Option<u32>,
    },
    /// T188: the zero-setup alias — `chug quick --goal "<task>"` drafts the
    /// spec (one read-only session), gates it (structure + non-vacuous
    /// check + dry-run), and runs the task against it. Headless only.
    Quick {
        /// Goal text: the task, in one sentence or ten.
        #[arg(long)]
        goal: String,
        /// T189: force FULL adversarial validation for every item on this
        /// run (the low-stakes lane predicate is off). Operator override,
        /// recorded via decision_log. A quick run IS an auto-spec run.
        #[arg(long, conflicts_with = "no_validate")]
        validate: bool,
        /// T189: force the gates-only lane for every item on this run,
        /// regardless of the diff — the kimi validation child is skipped,
        /// never the gates. Operator override, recorded via decision_log.
        #[arg(long, conflicts_with = "validate")]
        no_validate: bool,
        /// Working directory; all file/bash tools are sandboxed here. Defaults to `.`.
        #[arg(long)]
        cwd: Option<PathBuf>,
        /// Model id. Order: --model, $CHUG_MODEL, claude-sonnet-4-6.
        #[arg(long)]
        model: Option<String>,
        /// Iteration budget.
        #[arg(long, default_value_t = 40)]
        max_iters: u32,
        /// Wall-clock budget in minutes.
        #[arg(long, default_value_t = 120)]
        max_minutes: u64,
        /// Token budget: cumulative input+output tokens across the run.
        /// 0 = unlimited.
        #[arg(long, default_value_t = 0)]
        max_tokens: u64,
        /// Per-request output-token cap sent as `max_tokens` on every API call.
        #[arg(long)]
        max_tokens_per_request: Option<u32>,
        /// T192: one-shot context-occupancy warning, in estimated tokens
        /// (same knob and remedy as `chug run --ctx-warn-at-tokens`). 0 = off.
        #[arg(long, default_value_t = 0)]
        ctx_warn_at_tokens: u64,
        /// Classify every bash command with the laya risk judge before executing.
        #[arg(long)]
        risk_gate: bool,
        /// Per-command bash timeout in seconds. Overrides $CHUG_BASH_TIMEOUT
        /// (default 120).
        #[arg(long)]
        bash_timeout: Option<u64>,
        /// Path to MCP config JSON. Overrides discovery.
        #[arg(long)]
        mcp_config: Option<PathBuf>,
        /// Disable MCP servers even if config exists.
        #[arg(long, default_value_t = false)]
        mcp_off: bool,
    },
    /// Save/list/restore named session fork slots over the live transcript
    /// + LEDGER.md (F6 phase 1): explore two approaches from one state.
    Fork {
        #[command(subcommand)]
        action: ForkAction,
    },
    /// T204 phase 1 (F15): the baked-in Laya judge daemon — hosts the /judge
    /// inference server over a 0600 unix domain socket (no TCP listener, no
    /// port). Serve mode loads the checkpoint once and serves until killed;
    /// `--stop`/`--status`/`--ensure` are the lifecycle verbs (the
    /// single-instance flock on the daemon lock admits one server per host).
    Daemon {
        /// Stop a running daemon (TERM the lock holder, remove the socket).
        #[arg(long, conflicts_with_all = ["status", "ensure"])]
        stop: bool,
        /// Report status: exit 0 running, 1 starting or absent.
        #[arg(long, conflicts_with_all = ["stop", "ensure"])]
        status: bool,
        /// Ensure a healthy daemon: spawn + bounded wait (the loopd
        /// cycle-start step; best-effort — a nonzero exit never blocks).
        #[arg(long, conflicts_with_all = ["stop", "status"])]
        ensure: bool,
    },
    /// Print the current LEDGER.md.
    Ledger {
        /// Working directory. Defaults to `.`.
        #[arg(long)]
        cwd: Option<PathBuf>,
    },
    /// F10 phase 1 (T124): serve chug TO other agents as a stdio MCP server
    /// (read-only tools, `chug_status` + `chug_collect`) until stdin EOF,
    /// then exit 0. Stdout carries ONLY protocol messages — never run it
    /// expecting chatty output (a stdio server's stdout IS the wire).
    McpServe {
        /// F10 phase 2b (T129) + phase 3a (T153): advertise and serve the
        /// write tools — `chug_launch` (launch a bounded detached `chug
        /// run` in a chug cwd) and `chug_cancel` (stop one by pid). The
        /// flag gates the write SURFACE, not individual tools. Default
        /// OFF: without it the server is byte-identical to the read-only
        /// phase-1/2a server — neither write tool is advertised in
        /// `tools/list` and a call for either gets the unknown-tool error.
        /// The operator who starts the server decides whether writes exist.
        #[arg(long, default_value_t = false)]
        allow_launch: bool,
        /// F10 phase 3 (T157): advertise and serve the CONTROL verbs —
        /// `chug_abort` (cancel a launched run by id, run-level: the abort
        /// is recorded in the child's events stream and the terminal state
        /// is reported) and `chug_steer` (inject an `[operator]` steering
        /// note into a running child). Same policy family as
        /// `--allow-launch`: default-deny, advertised ⇔ callable, and
        /// INDEPENDENT of it — the operator who grants launching need not
        /// grant control of running children.
        #[arg(long, default_value_t = false)]
        allow_control: bool,
    },
    /// Start an interactive chat session in the TUI: type a request, chug
    /// works it with tools, returns to idle, repeat.
    Chat {
        /// Working directory; all file/bash tools are sandboxed here. Defaults to `.`.
        #[arg(long)]
        cwd: Option<PathBuf>,
        /// Model id. Order: --model, $CHUG_MODEL, claude-sonnet-4-6.
        #[arg(long)]
        model: Option<String>,
        /// Per-turn iteration budget.
        #[arg(long, default_value_t = 40)]
        max_iters: u32,
        /// Per-turn wall-clock budget in minutes.
        #[arg(long, default_value_t = 120)]
        max_minutes: u64,
        /// Per-turn token budget: cumulative input+output tokens. 0 = unlimited.
        #[arg(long, default_value_t = 0)]
        max_tokens: u64,
        /// Per-request output-token cap sent as `max_tokens` on every API
        /// call (T143). Order: --max-tokens-per-request, $CHUG_MAX_TOKENS,
        /// 32768.
        #[arg(long)]
        max_tokens_per_request: Option<u32>,
        /// Resume from <cwd>/.chug/transcript.jsonl.
        #[arg(long)]
        resume: bool,
        /// Classify every bash command with the laya risk judge before executing
        /// (blocks destructive commands; fails open when the judge is down).
        #[arg(long)]
        risk_gate: bool,
        /// Per-command bash timeout in seconds. Overrides $CHUG_BASH_TIMEOUT
        /// (default 120).
        #[arg(long)]
        bash_timeout: Option<u64>,
        /// Path to MCP config JSON. Overrides discovery.
        #[arg(long)]
        mcp_config: Option<PathBuf>,
        /// Disable MCP servers even if config exists.
        #[arg(long, default_value_t = false)]
        mcp_off: bool,
    },
}

/// `chug fork` actions (T105). Each carries its own `--cwd` like `ledger`.
#[derive(Subcommand)]
enum ForkAction {
    /// Snapshot the live transcript + LEDGER.md into .chug/sessions/<name>/.
    Save {
        /// Slot name: 1+ chars of [A-Za-z0-9._-] (no path separators).
        name: String,
        /// Overwrite an existing slot.
        #[arg(long)]
        force: bool,
        /// Working directory. Defaults to `.`.
        #[arg(long)]
        cwd: Option<PathBuf>,
    },
    /// List saved fork slots (name, transcript bytes, mtime, first-message
    /// preview).
    List {
        /// Working directory. Defaults to `.`.
        #[arg(long)]
        cwd: Option<PathBuf>,
    },
    /// Restore a slot over the live state — archives the live session first
    /// (nothing is lost); refuses while a live run holds .chug/driver.lock.
    Restore {
        /// Slot name: 1+ chars of [A-Za-z0-9._-] (no path separators).
        name: String,
        /// Working directory. Defaults to `.`.
        #[arg(long)]
        cwd: Option<PathBuf>,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    // SPEC-8: the observability sink must drain on EVERY exit path —
    // including a panic unwinding out of command dispatch, caught here so
    // the queued events still flush before the process exits.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match cli.command {
        CliCommand::Ledger { cwd } => cmd_ledger(cwd),
        // Stdout purity by construction (T124): the mcp-serve subcommand
        // dispatches STRAIGHT to the server loop — no banner, no ledger, no
        // driver lock, no events write on this path; stdout carries only
        // protocol messages.
        CliCommand::McpServe { allow_launch, allow_control } => {
            cmd_mcp_serve(allow_launch, allow_control)
        }
        CliCommand::Fork { action } => cmd_fork(action),
        CliCommand::Daemon { stop, status, ensure } => cmd_daemon(stop, status, ensure),
        CliCommand::Plan {
            goal,
            spec,
            out,
            cwd,
            model,
            max_iters,
            max_minutes,
            max_tokens,
            max_tokens_per_request,
        } => cmd_plan(
            goal, spec, out, cwd, model, max_iters, max_minutes, max_tokens,
            max_tokens_per_request,
        ),
        CliCommand::Chat {
            cwd,
            model,
            max_iters,
            max_minutes,
            max_tokens,
            max_tokens_per_request,
            resume,
            risk_gate,
            bash_timeout,
            mcp_config,
            mcp_off,
        } => cmd_chat(
            cwd,
            model,
            max_iters,
            max_minutes,
            max_tokens,
            max_tokens_per_request,
            resume,
            risk_gate,
            bash_timeout,
            mcp_config,
            mcp_off,
        ),
        CliCommand::Run {
            spec,
            auto_spec,
            validate,
            no_validate,
            goal,
            cwd,
            model,
            max_iters,
            max_minutes,
            max_tokens,
            max_tokens_per_request,
            ctx_warn_at_tokens,
            resume,
            tui,
            risk_gate,
            bash_timeout,
            mcp_config,
            mcp_off,
            approve,
        } => cmd_run(
            spec,
            auto_spec,
            // T189: clap enforces the pair's exclusivity and the auto-spec
            // group; here the bools fold into the lane override.
            if validate {
                Some(valroute::LaneOverride::Full)
            } else if no_validate {
                Some(valroute::LaneOverride::GatesOnly)
            } else {
                None
            },
            goal,
            cwd,
            model,
            max_iters,
            max_minutes,
            max_tokens,
            max_tokens_per_request,
            ctx_warn_at_tokens,
            resume,
            tui,
            risk_gate,
            bash_timeout,
            mcp_config,
            mcp_off,
            approve,
        ),
        CliCommand::Quick {
            goal,
            validate,
            no_validate,
            cwd,
            model,
            max_iters,
            max_minutes,
            max_tokens,
            max_tokens_per_request,
            ctx_warn_at_tokens,
            risk_gate,
            bash_timeout,
            mcp_config,
            mcp_off,
        } => cmd_run(
            None,      // no --spec: the draft phase writes it
            true,      // auto_spec
            if validate {
                Some(valroute::LaneOverride::Full)
            } else if no_validate {
                Some(valroute::LaneOverride::GatesOnly)
            } else {
                None
            },
            goal,
            cwd,
            model,
            max_iters,
            max_minutes,
            max_tokens,
            max_tokens_per_request,
            ctx_warn_at_tokens,
            false,     // no --resume on quick
            false,     // headless
            risk_gate,
            bash_timeout,
            mcp_config,
            mcp_off,
            None,      // no --approve
        ),
    }));
    observ::shutdown_global();
    match result {
        Ok(Ok(code)) => ExitCode::from(code as u8),
        Ok(Err(e)) => {
            eprintln!("chug: error: {e:#}");
            ExitCode::FAILURE
        }
        Err(panic) => {
            eprintln!("chug: panicked: {panic:?}");
            ExitCode::FAILURE
        }
    }
}

/// Bash timeout precedence: `--bash-timeout` flag > `$CHUG_BASH_TIMEOUT` env
/// > the 120s tool default. Rejects zero and unparseable values.
fn bash_timeout_secs(flag: Option<u64>, env: Option<&str>) -> anyhow::Result<u64> {
    fn parse(source: &str, raw: &str) -> anyhow::Result<u64> {
        let secs: u64 = raw
            .parse()
            .map_err(|_| anyhow!("{source} must be a number of seconds, got {raw:?}"))?;
        if secs == 0 {
            anyhow::bail!("{source} must be a positive number of seconds");
        }
        Ok(secs)
    }
    match flag {
        Some(secs) => {
            if secs == 0 {
                anyhow::bail!("--bash-timeout must be a positive number of seconds");
            }
            Ok(secs)
        }
        None => match env.map(str::trim).filter(|v| !v.is_empty()) {
            Some(raw) => parse("$CHUG_BASH_TIMEOUT", raw),
            None => Ok(tools::BASH_TIMEOUT_SECS),
        },
    }
}

fn resolve_bash_timeout(flag: Option<u64>) -> anyhow::Result<std::time::Duration> {
    let env = std::env::var("CHUG_BASH_TIMEOUT").ok();
    Ok(std::time::Duration::from_secs(bash_timeout_secs(flag, env.as_deref())?))
}

/// T143 per-request cap precedence: `--max-tokens-per-request` flag >
/// `$CHUG_MAX_TOKENS` env > the 32768 default ([`api::DEFAULT_MAX_TOKENS`],
/// the operator-proven value after the 8192 hardcoded cap let GLM thinking
/// blocks truncate large tool-call JSON). Pure so the precedence is
/// unit-testable without touching process env. Rejects zero and unparseable
/// values from either source (the API needs a positive cap).
fn max_tokens_per_request(flag: Option<u32>, env: Option<&str>) -> anyhow::Result<u32> {
    fn parse(source: &str, raw: &str) -> anyhow::Result<u32> {
        let cap: u32 = raw
            .parse()
            .map_err(|_| anyhow!("{source} must be a number of tokens, got {raw:?}"))?;
        if cap == 0 {
            anyhow::bail!("{source} must be a positive number of tokens");
        }
        Ok(cap)
    }
    match flag {
        Some(cap) => {
            if cap == 0 {
                anyhow::bail!("--max-tokens-per-request must be a positive number of tokens");
            }
            Ok(cap)
        }
        None => match env.map(str::trim).filter(|v| !v.is_empty()) {
            Some(raw) => parse("$CHUG_MAX_TOKENS", raw),
            None => Ok(api::DEFAULT_MAX_TOKENS),
        },
    }
}

fn resolve_max_tokens(flag: Option<u32>) -> anyhow::Result<u32> {
    let env = std::env::var("CHUG_MAX_TOKENS").ok();
    max_tokens_per_request(flag, env.as_deref())
}

#[allow(clippy::too_many_arguments)]
/// T188: where an auto-spec'd run's draft lands (under the resolved cwd).
fn auto_spec_path(cwd: &Path) -> PathBuf {
    cwd.join(autospec::AUTO_SPEC_REL)
}

#[allow(clippy::too_many_arguments)]
fn cmd_run(
    spec: Option<PathBuf>,
    auto_spec: bool,
    // T189: the auto-spec lane override — Some(Full) = --validate, Some(
    // GatesOnly) = --no-validate, None = the lane predicate governs.
    validation_override: Option<valroute::LaneOverride>,
    goal: String,
    cwd: Option<PathBuf>,
    model: Option<String>,
    max_iters: u32,
    max_minutes: u64,
    max_tokens: u64,
    max_tokens_per_request: Option<u32>,
    ctx_warn_at_tokens: u64,
    resume: bool,
    tui: bool,
    risk_gate: bool,
    bash_timeout: Option<u64>,
    mcp_config: Option<PathBuf>,
    mcp_off: bool,
    approve: Option<PathBuf>,
) -> anyhow::Result<i32> {
    let cwd = resolve_cwd(cwd)?;
    let bash_timeout = resolve_bash_timeout(bash_timeout)?;
    // T143: resolve the per-request cap here — flag > $CHUG_MAX_TOKENS > 32768.
    let max_tokens_per_request = resolve_max_tokens(max_tokens_per_request)?;
    // T117: pack expansion happens first — before the spec resolution and
    // every `.chug/` write, so a bad `/name` exits clean.
    let (goal, goal_pack) = expand_goal(&cwd, goal)?;
    // T146: the approved plan is validated at the CLI boundary too — same
    // ordering discipline (before the spec resolution and every `.chug/`
    // write), so a bad `--approve` path refuses the run without touching
    // the working directory's session state.
    let approve = match approve {
        Some(path) => Some(driver::load_approved_plan(&cwd, &path)?),
        None => None,
    };
    let model = model
        .filter(|m| !m.trim().is_empty())
        .or_else(|| std::env::var("CHUG_MODEL").ok().filter(|m| !m.trim().is_empty()))
        .unwrap_or_else(|| driver::DEFAULT_MODEL.to_string());
    // T188: --spec and --auto-spec are mutually exclusive — a hand-written
    // spec is the operator's word; the draft phase only ever runs bare.
    if auto_spec && spec.is_some() {
        bail!("--spec and --auto-spec are mutually exclusive: drop --spec to let the draft phase write the spec");
    }
    let spec = if auto_spec {
        let drafted = auto_spec_path(&cwd);
        if resume {
            // Resumable: the draft lives in .chug (gitignored) — a resumed
            // --auto-spec run reuses it, never redrafts.
            if !drafted.is_file() {
                bail!(
                    "--auto-spec --resume: no drafted spec at {} — run without --resume to draft one",
                    drafted.display()
                );
            }
            drafted.canonicalize()?
        } else {
            let mut sink = events::ConsoleSink::new(cwd.clone());
            let mut client = api::Client::new(&model, max_tokens_per_request)?;
            autospec::draft_and_gate(
                &cwd,
                &goal,
                &model,
                max_tokens_per_request,
                &mut client,
                &mut sink,
            )?
            .canonicalize()?
        }
    } else {
        spec.expect("--spec is required without --auto-spec (clap enforces)")
            .canonicalize()
            .with_context(|| "spec file not found")?
    };
    // T189: the auto-spec validation lane. The lane predicate is the
    // default (LOOP-SPEC §2 step 4); --validate/--no-validate are operator
    // overrides. An override is RECORDED via decision_log
    // (validation-routing) and carried to the orchestrator on the goal
    // (re-read every iteration); the default carries the self-contained
    // predicate directive the same way. Hand-written specs are untouched —
    // the operator's spec is the contract there.
    let goal = if auto_spec {
        match validation_override {
            Some(lane) => {
                if let Err(e) = valroute::record_override(&cwd, lane) {
                    // The decision_log contract is best-effort: a record
                    // failure warns and never aborts the run.
                    eprintln!("chug: warning: lane override record failed: {e:#}");
                }
                format!("{goal}{}", lane.directive())
            }
            None => format!("{goal}{}", valroute::predicate_directive()),
        }
    } else {
        goal
    };
    // T11: one stderr line naming the build so a stale binary is obvious.
    // T20: resolve the cwd's checkout identity at runtime so the same line
    // also names the branch@commit this process actually runs in — children
    // run the main-tree binary inside a worktree, so `head=` and the baked
    // commit legitimately differ. Unresolvable → no field, never a failure.
    let head = build_info::resolve_head(&cwd);
    build_info::print_startup_banner(&cwd, Some(&spec), &model, build_info::as_pair(&head));

    if tui {
        run_with_tui(
            spec, goal, goal_pack, approve, cwd, model, max_iters, max_minutes, max_tokens,
            max_tokens_per_request, ctx_warn_at_tokens, resume, risk_gate, bash_timeout, mcp_config,
            mcp_off,
        )
    } else {
        let cfg = driver::RunConfig {
            cwd,
            spec_path: spec,
            goal,
            model,
            max_iters,
            max_minutes,
            max_tokens,
            max_tokens_per_request,
            ctx_warn_at_tokens,
            resume,
            controls: driver::Controls::detached(),
            risk_gate,
            bash_timeout,
            mcp_config,
            mcp_off,
            goal_pack,
            approve,
        };
        let mut sink = events::ConsoleSink::new(cfg.cwd.clone());
        driver::run(cfg, &mut sink)
    }
}

/// `--tui` mode: worker thread runs the driver, main thread runs the UI.
#[allow(clippy::too_many_arguments)]
fn run_with_tui(
    spec: PathBuf,
    goal: String,
    goal_pack: Option<String>,
    approve: Option<driver::ApprovedPlan>,
    cwd: PathBuf,
    model: String,
    max_iters: u32,
    max_minutes: u64,
    max_tokens: u64,
    max_tokens_per_request: u32,
    ctx_warn_at_tokens: u64,
    resume: bool,
    risk_gate: bool,
    bash_timeout: std::time::Duration,
    mcp_config: Option<PathBuf>,
    mcp_off: bool,
) -> anyhow::Result<i32> {
    let (event_tx, event_rx) = mpsc::channel::<events::Event>();
    let (steer_tx, steer_rx) = mpsc::channel::<String>();
    let abort = Arc::new(AtomicBool::new(false));
    let driver_done = Arc::new(AtomicBool::new(false));

    let cfg = driver::RunConfig {
        cwd: cwd.clone(),
        spec_path: spec,
        goal: goal.clone(),
        model: model.clone(),
        max_iters,
        max_minutes,
        max_tokens,
        max_tokens_per_request,
        ctx_warn_at_tokens,
        resume,
        risk_gate,
        bash_timeout,
        mcp_config,
        mcp_off,
        goal_pack,
        approve,
        controls: driver::Controls {
            abort: Arc::clone(&abort),
            steering_rx: steer_rx,
        },
    };

    let worker = {
        let done = Arc::clone(&driver_done);
        thread::Builder::new()
            .name("chug-driver".into())
            .spawn(move || {
                let mut sink = tui::TuiSink { tx: event_tx };
                let result = driver::run(cfg, &mut sink);
                done.store(true, std::sync::atomic::Ordering::SeqCst);
                result
            })
            .context("spawning driver thread")?
    };

    let ui = tui::run_tui(tui::TuiConfig {
        goal,
        model,
        abort: Arc::clone(&abort),
        events: event_rx,
        steering_tx: steer_tx,
        driver_done,
        chat: None,
    });

    let code = worker
        .join()
        .map_err(|e| anyhow!("driver thread panicked: {e:?}"))??;
    ui?;
    Ok(code)
}

/// `chat` mode: worker thread runs the chat session, main thread runs the UI.
/// The TUI is the interface; there is no headless chat.
#[allow(clippy::too_many_arguments)]
fn cmd_chat(
    cwd: Option<PathBuf>,
    model: Option<String>,
    max_iters: u32,
    max_minutes: u64,
    max_tokens: u64,
    max_tokens_per_request: Option<u32>,
    resume: bool,
    risk_gate: bool,
    bash_timeout: Option<u64>,
    mcp_config: Option<PathBuf>,
    mcp_off: bool,
) -> anyhow::Result<i32> {
    let cwd = resolve_cwd(cwd)?;
    let bash_timeout = resolve_bash_timeout(bash_timeout)?;
    // T143: flag > $CHUG_MAX_TOKENS > 32768, same resolution as run/plan.
    let max_tokens_per_request = resolve_max_tokens(max_tokens_per_request)?;
    let model = model
        .filter(|m| !m.trim().is_empty())
        .or_else(|| std::env::var("CHUG_MODEL").ok().filter(|m| !m.trim().is_empty()))
        .unwrap_or_else(|| driver::DEFAULT_MODEL.to_string());
    // T11: same banner as `run` (chat has no spec yet — one may arrive via /spec).
    // T20: the cwd's checkout identity rides it, same as run mode.
    let head = build_info::resolve_head(&cwd);
    build_info::print_startup_banner(&cwd, None, &model, build_info::as_pair(&head));

    let (event_tx, event_rx) = mpsc::channel::<events::Event>();
    let (steer_tx, steer_rx) = mpsc::channel::<String>();
    let (objective_tx, objective_rx) = mpsc::channel::<String>();
    let (update_tx, update_rx) = mpsc::channel::<driver::SlashUpdate>();
    // T188: auto-spec requests ride their own channel (the worker serves
    // them while idle without consuming buffered session updates).
    let (autospec_tx, autospec_rx) = mpsc::channel::<chat::AutoSpecRequest>();
    let abort = Arc::new(AtomicBool::new(false));
    let driver_done = Arc::new(AtomicBool::new(false));

    let cfg = chat::ChatConfig {
        cwd: cwd.clone(),
        model: model.clone(),
        max_iters,
        max_minutes,
        max_tokens,
        max_tokens_per_request,
        resume,
        risk_gate,
        bash_timeout,
        mcp_config,
        mcp_off,
        controls: driver::Controls {
            abort: Arc::clone(&abort),
            steering_rx: steer_rx,
        },
        objective_rx,
        update_rx,
        autospec_rx,
    };

    let worker = {
        let done = Arc::clone(&driver_done);
        thread::Builder::new()
            .name("chug-chat".into())
            .spawn(move || {
                let mut sink = tui::TuiSink { tx: event_tx };
                let result = chat::run_chat(cfg, &mut sink);
                done.store(true, std::sync::atomic::Ordering::SeqCst);
                result
            })
            .context("spawning chat thread")?
    };

    let ui = tui::run_tui(tui::TuiConfig {
        goal: String::new(),
        model,
        abort: Arc::clone(&abort),
        events: event_rx,
        steering_tx: steer_tx,
        driver_done,
        chat: Some(tui::ChatWiring {
            cwd,
            budget: (max_iters, max_minutes),
            objective_tx,
            update_tx,
            autospec_tx,
        }),
    });

    let code = worker
        .join()
        .map_err(|e| anyhow!("chat thread panicked: {e:?}"))??;
    ui?;
    Ok(code)
}

fn cmd_ledger(cwd: Option<PathBuf>) -> anyhow::Result<i32> {
    let cwd = resolve_cwd(cwd)?;
    print!("{}", ledger::read(&cwd)?);
    Ok(0)
}

/// F10 phase 1 (T124): run the stdio MCP server until stdin EOF, then exit 0.
/// T129 + T153: `allow_launch` gates the write tools (`chug_launch`,
/// `chug_cancel`). T157: `allow_control` gates the control verbs
/// (`chug_abort`, `chug_steer`) — both default OFF; the flagless server is
/// byte-identical to pre-T129.
fn cmd_mcp_serve(allow_launch: bool, allow_control: bool) -> anyhow::Result<i32> {
    mcp_serve::serve(mcp_serve::Gates {
        allow_launch,
        allow_control,
    })?;
    Ok(0)
}

/// `chug daemon` (T204 phase 1): serve mode (no flags) takes the
/// single-instance lock, loads the checkpoint, binds the 0600 socket, and
/// serves /healthz + /judge until killed — like mcp-serve, it dispatches
/// STRAIGHT to the server loop (no banner, no ledger, no driver lock, no
/// events write: a long-lived host process). The lifecycle verbs are pure
/// CLI ops in the cmd_fork shape. On a default (feature-off) build serve
/// refuses with a clear message; clients fail open meanwhile.
fn cmd_daemon(stop: bool, status: bool, ensure: bool) -> anyhow::Result<i32> {
    if stop {
        return daemon::stop();
    }
    if status {
        return daemon::status();
    }
    if ensure {
        return daemon::ensure_cmd();
    }
    daemon::serve()
}

/// `chug fork` (T105): a pure CLI op over the two session files — no driver,
/// no events, no banner (the resumed run's own `run_start` records the
/// continuation). Every failure leg is an anyhow error, so `main` prints the
/// one-line stderr message and exits non-zero; success prints one line.
fn cmd_fork(action: ForkAction) -> anyhow::Result<i32> {
    let summary = match action {
        ForkAction::Save { name, force, cwd } => {
            let cwd = resolve_cwd(cwd)?;
            fork::save(&cwd, &name, force)?
        }
        ForkAction::List { cwd } => fork::list(&resolve_cwd(cwd)?)?,
        ForkAction::Restore { name, cwd } => {
            let cwd = resolve_cwd(cwd)?;
            fork::restore(&cwd, &name)?
        }
    };
    println!("{summary}");
    Ok(0)
}

/// T73: `chug plan` — the read-only planning session. Same model resolution
/// and spec resolution as `run`; `--out` is passed through raw and resolved
/// through the cwd sandbox when submit_plan writes.
#[allow(clippy::too_many_arguments)] // one arg per clap flag, same shape as cmd_run
fn cmd_plan(
    goal: String,
    spec: Option<PathBuf>,
    out: Option<PathBuf>,
    cwd: Option<PathBuf>,
    model: Option<String>,
    max_iters: u32,
    max_minutes: u64,
    max_tokens: u64,
    max_tokens_per_request: Option<u32>,
) -> anyhow::Result<i32> {
    let cwd = resolve_cwd(cwd)?;
    // T143: flag > $CHUG_MAX_TOKENS > 32768, same resolution as run/chat.
    let max_tokens_per_request = resolve_max_tokens(max_tokens_per_request)?;
    // T117: same CLI-boundary expansion as run — before anything writes.
    let (goal, goal_pack) = expand_goal(&cwd, goal)?;
    let spec = match spec {
        Some(path) => Some(
            path.canonicalize()
                .with_context(|| format!("spec file {} not found", path.display()))?,
        ),
        None => None,
    };
    let model = model
        .filter(|m| !m.trim().is_empty())
        .or_else(|| std::env::var("CHUG_MODEL").ok().filter(|m| !m.trim().is_empty()))
        .unwrap_or_else(|| driver::DEFAULT_MODEL.to_string());
    // T11: same banner as run (the spec field is null when no --spec).
    let head = build_info::resolve_head(&cwd);
    build_info::print_startup_banner(&cwd, spec.as_deref(), &model, build_info::as_pair(&head));

    let cfg = driver::PlanConfig {
        cwd,
        kind: plan::PlanKind::Plan,
        spec_path: spec,
        goal,
        model,
        max_iters,
        max_minutes,
        max_tokens,
        max_tokens_per_request,
        out_path: out,
        goal_pack,
    };
    let mut sink = events::ConsoleSink::new(cfg.cwd.clone());
    driver::run_plan(cfg, &mut sink)
}

/// F9 phase 2a (T117): resolve a run/plan `--goal` through the slash-command
/// packs at the CLI boundary — before the driver (or plan loop) starts, so a
/// failed resolution never triggers a `.chug/` write and the driver never
/// sees the `/name` literal.
///
/// - Not an invocation → the goal passes through byte-identical, `goal_pack`
///   `None`.
/// - A pack hit → the goal is REPLACED with the expanded body (one stderr
///   note names the pack) and the pack name rides `run_start` as
///   `goal_pack`.
/// - Unknown `/name` or an empty expansion → a hard error (non-zero exit)
///   naming the remedy; a typo'd pack name is never silently run as a
///   literal goal.
fn expand_goal(cwd: &std::path::Path, goal: String) -> anyhow::Result<(String, Option<String>)> {
    match commands::resolve_goal(cwd, &goal) {
        Ok(commands::GoalResolution::Passthrough) => Ok((goal, None)),
        Ok(commands::GoalResolution::Expanded { body, pack }) => {
            eprintln!("chug: goal expanded from pack '{pack}'");
            Ok((body, Some(pack)))
        }
        Err(message) => Err(anyhow!(message)),
    }
}

fn resolve_cwd(cwd: Option<PathBuf>) -> anyhow::Result<PathBuf> {
    let given = cwd.unwrap_or_else(|| PathBuf::from("."));
    given
        .canonicalize()
        .with_context(|| format!("resolving --cwd {}", given.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn bash_timeout_precedence_flag_over_env_over_default() {
        // Flag wins over env; env wins over the 120s default.
        assert_eq!(bash_timeout_secs(Some(7), Some("5")).unwrap(), 7);
        assert_eq!(bash_timeout_secs(None, Some("5")).unwrap(), 5);
        assert_eq!(
            bash_timeout_secs(None, None).unwrap(),
            tools::BASH_TIMEOUT_SECS
        );
        // Env whitespace is trimmed; a blank env falls through to the default.
        assert_eq!(bash_timeout_secs(None, Some(" 9 ")).unwrap(), 9);
        assert_eq!(
            bash_timeout_secs(None, Some("   ")).unwrap(),
            tools::BASH_TIMEOUT_SECS
        );
    }

    #[test]
    fn bash_timeout_rejects_zero_and_garbage() {
        assert!(bash_timeout_secs(Some(0), None).is_err());
        assert!(bash_timeout_secs(None, Some("0")).is_err());
        assert!(bash_timeout_secs(None, Some("abc")).is_err());
        assert!(bash_timeout_secs(None, Some("-3")).is_err());
        // Zero from the flag beats a valid env (flag is consulted first).
        assert!(bash_timeout_secs(Some(0), Some("5")).is_err());
    }

    #[test]
    fn resolve_bash_timeout_wraps_secs_in_duration() {
        assert_eq!(resolve_bash_timeout(Some(1)).unwrap(), Duration::from_secs(1));
    }

    /// T143: per-request cap precedence — the 32768 default, `$CHUG_MAX_TOKENS`
    /// wins over the default, the flag wins over the env (bash-timeout shape);
    /// zero/garbage from either source is rejected; a blank env falls through
    /// to the default. Pinned in the SAME test: T15's cumulative-budget
    /// `--max-tokens` is unchanged — still present on run/chat/plan with its
    /// `0` (= unlimited) clap default — and the new
    /// `--max-tokens-per-request` carries no clap default (its default is the
    /// resolution const) so the two flags can never collide.
    #[test]
    fn max_tokens_per_request_precedence_and_t15_budget_flag_pinned() {
        use clap::CommandFactory;

        // Precedence (env passed as a parameter — no process-env mutation).
        assert_eq!(
            max_tokens_per_request(None, None).unwrap(),
            32768,
            "the operator-proven default"
        );
        assert_eq!(
            max_tokens_per_request(None, Some("8192")).unwrap(),
            8192,
            "env wins over the default"
        );
        assert_eq!(
            max_tokens_per_request(Some(65536), Some("8192")).unwrap(),
            65536,
            "flag wins over the env"
        );
        // Blank env falls through to the default (bash-timeout parity).
        assert_eq!(max_tokens_per_request(None, Some("  ")).unwrap(), 32768);
        // Zero and garbage are rejected from either source.
        assert!(max_tokens_per_request(Some(0), Some("8192")).is_err());
        assert!(max_tokens_per_request(None, Some("0")).is_err());
        assert!(max_tokens_per_request(None, Some("abc")).is_err());

        // T15's cumulative-budget flag unchanged, in the same breath.
        let cli = Cli::command();
        for sub in ["run", "chat", "plan"] {
            let cmd = cli.find_subcommand(sub).unwrap_or_else(|| panic!("{sub} subcommand"));
            let t15 = cmd
                .get_arguments()
                .find(|a| a.get_id() == "max_tokens")
                .unwrap_or_else(|| panic!("{sub}: T15 --max-tokens present"));
            let defaults = t15.get_default_values();
            assert_eq!(defaults.len(), 1, "{sub}: --max-tokens default present");
            assert_eq!(
                defaults[0].to_str(),
                Some("0"),
                "{sub}: T15 cumulative budget default (0 = unlimited) unchanged"
            );
            let t143 = cmd
                .get_arguments()
                .find(|a| a.get_id() == "max_tokens_per_request")
                .unwrap_or_else(|| panic!("{sub}: T143 --max-tokens-per-request present"));
            assert!(
                t143.get_default_values().is_empty(),
                "{sub}: the per-request cap is Option — its default comes from resolution, not clap"
            );
            assert_ne!(t15.get_id(), t143.get_id(), "{sub}: no id collision");
        }
    }

    /// T117: the CLI-boundary seam both `run` and `plan` go through. Hit →
    /// expanded body + `Some(pack)`; unknown/empty → the error legs; a
    /// non-invocation goal → byte-identical passthrough with `None`.
    #[test]
    fn expand_goal_resolves_packs_at_the_cli_boundary() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = commands::commands_dir(tmp.path());
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("smoke.md"), "Say hello to $ARGUMENTS.").unwrap();
        std::fs::write(dir.join("note.md"), "").unwrap();

        // Hit: the goal is replaced with the expanded body; the pack is named.
        let (goal, pack) = expand_goal(tmp.path(), "/smoke hello".into()).unwrap();
        assert_eq!(goal, "Say hello to hello.");
        assert_eq!(pack.as_deref(), Some("smoke"));

        // Non-invocation: byte-identical passthrough, no pack.
        let (goal, pack) = expand_goal(tmp.path(), "fix the login bug".into()).unwrap();
        assert_eq!(goal, "fix the login bug");
        assert_eq!(pack, None);

        // Unknown pack: a hard error naming the remedy (never a literal run).
        let err = expand_goal(tmp.path(), "/nosuch hello".into()).unwrap_err();
        assert!(err.to_string().contains("'/nosuch'"), "{err}");
        assert!(err.to_string().contains("smoke"), "{err}");

        // Empty expansion: a hard error naming the pack.
        let err = expand_goal(tmp.path(), "/note".into()).unwrap_err();
        assert!(err.to_string().contains("'note'"), "{err}");

        // `/` and `/ x` are not invocations (passthrough, zero-cost leg).
        for goal in ["/", "/ smoke"] {
            let (goal, pack) = expand_goal(tmp.path(), goal.to_string()).unwrap();
            assert!(goal.starts_with('/'));
            assert_eq!(pack, None);
        }
    }

    /// T15: `--max-tokens` parses into the knob (u64) and defaults to 0
    /// (unlimited) on both subcommands that carry budgets.
    #[test]
    fn cli_max_tokens_parses_and_defaults_to_unlimited() {
        let cli = Cli::try_parse_from([
            "chug",
            "run",
            "--spec",
            "s.md",
            "--goal",
            "g",
            "--max-tokens",
            "250000",
        ])
        .expect("run parses");
        let CliCommand::Run { max_tokens, .. } = cli.command else {
            panic!("expected the run subcommand");
        };
        assert_eq!(max_tokens, 250_000);

        let cli = Cli::try_parse_from(["chug", "run", "--spec", "s.md", "--goal", "g"])
            .expect("run without the flag parses");
        let CliCommand::Run { max_tokens, .. } = cli.command else {
            panic!("expected the run subcommand");
        };
        assert_eq!(max_tokens, 0, "unset = unlimited");

        let cli = Cli::try_parse_from([
            "chug",
            "chat",
            "--max-tokens",
            "1000",
        ])
        .expect("chat parses");
        let CliCommand::Chat { max_tokens, .. } = cli.command else {
            panic!("expected the chat subcommand");
        };
        assert_eq!(max_tokens, 1_000);
    }

    /// T105 clap pins: `chug fork` parses all three actions; `save` carries
    /// the positional name + `--force`; `restore` has no `--force`.
    #[test]
    fn cli_fork_parses_save_list_restore() {
        let cli = Cli::try_parse_from([
            "chug",
            "fork",
            "save",
            "approach-a",
            "--force",
            "--cwd",
            "/tmp/x",
        ])
        .expect("fork save parses");
        let CliCommand::Fork { action } = cli.command else {
            panic!("expected the fork subcommand");
        };
        let ForkAction::Save { name, force, cwd } = action else {
            panic!("expected the save action");
        };
        assert_eq!(name, "approach-a");
        assert!(force);
        assert_eq!(cwd.as_deref(), Some(std::path::Path::new("/tmp/x")));

        let cli = Cli::try_parse_from(["chug", "fork", "list"]).expect("fork list parses");
        assert!(matches!(
            cli.command,
            CliCommand::Fork { action: ForkAction::List { .. } }
        ));

        let cli = Cli::try_parse_from(["chug", "fork", "restore", "approach-b"])
            .expect("fork restore parses");
        let CliCommand::Fork { action } = cli.command else {
            panic!("expected the fork subcommand");
        };
        let ForkAction::Restore { name, cwd } = action else {
            panic!("expected the restore action");
        };
        assert_eq!(name, "approach-b");
        assert!(cwd.is_none(), "cwd defaults to `.`");

        // A name is required for save and restore.
        assert!(Cli::try_parse_from(["chug", "fork", "save"]).is_err());
        assert!(Cli::try_parse_from(["chug", "fork", "restore"]).is_err());
    }

    /// T73 clap pins: `chug plan` parses with the 30/20 defaults, carries the
    /// full flag set, and rejects a missing --goal.
    #[test]
    fn cli_plan_parses_with_defaults_and_requires_goal() {
        let cli = Cli::try_parse_from(["chug", "plan", "--goal", "x"]).expect("plan parses");
        let CliCommand::Plan {
            goal,
            spec,
            out,
            max_iters,
            max_minutes,
            max_tokens,
            ..
        } = cli.command
        else {
            panic!("expected the plan subcommand");
        };
        assert_eq!(goal, "x");
        assert!(spec.is_none() && out.is_none());
        assert_eq!(max_iters, 30, "default iteration budget");
        assert_eq!(max_minutes, 20, "default wall-clock budget");
        assert_eq!(max_tokens, 0, "unset token budget = unlimited");

        let cli = Cli::try_parse_from([
            "chug",
            "plan",
            "--goal",
            "draft it",
            "--spec",
            "SPEC.md",
            "--out",
            "p.md",
            "--max-iters",
            "5",
            "--max-minutes",
            "3",
            "--max-tokens",
            "1000",
        ])
        .expect("full flag set parses");
        let CliCommand::Plan {
            goal,
            spec,
            out,
            max_iters,
            max_minutes,
            max_tokens,
            ..
        } = cli.command
        else {
            panic!("expected the plan subcommand");
        };
        assert_eq!(goal, "draft it");
        assert_eq!(spec.as_deref(), Some(std::path::Path::new("SPEC.md")));
        assert_eq!(out.as_deref(), Some(std::path::Path::new("p.md")));
        assert_eq!((max_iters, max_minutes, max_tokens), (5, 3, 1_000));

        // Missing --goal is a hard clap error (never silently accepted).
        let err = Cli::try_parse_from(["chug", "plan"]).map(|_| ()).unwrap_err();
        assert!(
            err.to_string().contains("--goal"),
            "missing --goal must be rejected: {err}"
        );
    }

    // ---------- T146: `chug run --approve <plan.md>` ----------

    /// clap legs: `--approve` parses on `run` ONLY — `chug plan` produces
    /// plans and `chug chat` is interactive, so either receiving the flag is
    /// a clap error (an approve-accepted-on-plan/chat mutant dies here).
    #[test]
    fn approve_flag_parses_on_run_and_is_a_clap_error_on_plan_and_chat() {
        let cli = Cli::try_parse_from([
            "chug",
            "run",
            "--spec",
            "s.md",
            "--goal",
            "g",
            "--approve",
            "plan.md",
        ])
        .expect("--approve parses on run");
        let CliCommand::Run { approve, .. } = cli.command else {
            panic!("expected the run subcommand");
        };
        assert_eq!(approve, Some(PathBuf::from("plan.md")));

        for args in [
            vec!["chug", "plan", "--goal", "g", "--approve", "plan.md"],
            vec!["chug", "chat", "--approve", "plan.md"],
        ] {
            let err = Cli::try_parse_from(args).map(|_| ()).unwrap_err();
            assert!(
                err.to_string().contains("--approve"),
                "plan/chat must reject --approve: {err}"
            );
        }
    }

    /// Refusal legs (T146): a missing file, an unreadable (directory) path,
    /// an empty file, and a path escaping the cwd — each leg refuses at the
    /// CLI boundary BEFORE any `.chug/` write, exits nonzero with a message
    /// naming the path and the leg that failed.
    #[test]
    fn approve_refusals_name_the_leg_and_never_touch_chug() {
        let tmp = tempfile::tempdir().unwrap();
        let spec = tmp.path().join("s.md");
        std::fs::write(&spec, "spec text\ncheck: true\n").unwrap();
        std::fs::write(tmp.path().join("empty.md"), "   \n\t\n").unwrap();
        std::fs::create_dir(tmp.path().join("dir.md")).unwrap();

        // (path, expected-leg substrings)
        let legs: Vec<(PathBuf, Vec<&str>)> = vec![
            (
                PathBuf::from("missing.md"),
                vec!["--approve missing.md", "not found"],
            ),
            (
                PathBuf::from("dir.md"),
                vec!["--approve dir.md", "not a readable regular file"],
            ),
            (
                PathBuf::from("empty.md"),
                vec!["--approve empty.md", "empty"],
            ),
            // Relative `..` traversal …
            (
                PathBuf::from("../approved-outside.md"),
                vec!["--approve ../approved-outside.md", "path escapes cwd"],
            ),
            // … and an absolute path outside cwd (with the file EXISTING
            // there, so the sandbox leg fires before any missing-file leg).
            (
                tmp.path().parent().unwrap().join("approved-outside.md"),
                vec!["path escapes cwd", "inside the working directory"],
            ),
        ];
        std::fs::write(
            tmp.path().parent().unwrap().join("approved-outside.md"),
            "# real plan outside\n",
        )
        .unwrap();

        for (path, needles) in legs {
            let err = cmd_run(
                Some(spec.clone()),
                false,
                None, // T189: no lane override (hand-written spec)
                "g".into(),
                Some(tmp.path().to_path_buf()),
                Some("test-model".into()),
                5,
                10,
                0,
                None,
                0, // ctx_warn_at_tokens
                false,
                false,
                false,
                None,
                None,
                true,
                Some(path.clone()),
            )
            .unwrap_err();
            let message = err.to_string();
            for needle in needles {
                assert!(
                    message.contains(needle),
                    "{path:?}: refusal must name the leg ({needle}): {message}"
                );
            }
            // The refusal happened BEFORE any `.chug/` write: the run cwd is
            // untouched (no events log, no driver lock, no ledger seed).
            assert!(
                !tmp.path().join(".chug").exists(),
                "{path:?}: a refused run must not create .chug/"
            );
        }
    }

    /// The loader's success shape: the path is recorded as typed, the hash
    /// is over the file bytes (the same hex `shasum -a 256` gives), and the
    /// text is verbatim.
    #[test]
    fn approve_loader_returns_path_hash_and_verbatim_text() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("plan.md");
        std::fs::write(&path, "# Plan\n\n1. add the flag\n").unwrap();
        let plan = driver::load_approved_plan(tmp.path(), &path).unwrap();
        assert_eq!(plan.path, path.display().to_string());
        // The hash is over the file's bytes — the same hex `shasum -a 256`
        // gives (the shared sha256_hex primitive behind goal_sha256).
        assert_eq!(
            plan.sha256,
            crate::eventlog::goal_sha256("# Plan\n\n1. add the flag\n")
        );
        assert_eq!(plan.text, "# Plan\n\n1. add the flag\n");
    }
    // ---------- T188 (F7): the auto-spec CLI entry points ----------

    /// `chug quick --goal "<task>"` (the zero-setup alias) and
    /// `chug run --auto-spec --goal "<task>"` parse: the goal is required on
    /// both, `--spec` is NOT required under `--auto-spec`
    /// (required_unless_present — the draft phase writes it), and a bare
    /// `run`/`quick` without a goal is a clap error. A dropped flag or a
    /// flipped required-unless wiring is RED here.
    #[test]
    fn quick_and_auto_spec_cli_parse_pins() {
        // The quick alias parses with its documented defaults.
        let cli = Cli::try_parse_from(["chug", "quick", "--goal", "fix the flaky test"])
            .expect("quick parses");
        let CliCommand::Quick {
            goal,
            max_iters,
            max_minutes,
            ..
        } = cli.command
        else {
            panic!("expected the quick subcommand");
        };
        assert_eq!(goal, "fix the flaky test");
        assert_eq!(max_iters, 40, "the quick iteration default");
        assert_eq!(max_minutes, 120, "the quick wall-clock default");

        // The goal is required on quick.
        assert!(
            Cli::try_parse_from(["chug", "quick"]).is_err(),
            "bare quick is a clap error (the goal is required)"
        );

        // run --auto-spec needs no --spec: the draft phase writes it.
        let cli = Cli::try_parse_from(["chug", "run", "--auto-spec", "--goal", "g"])
            .expect("run --auto-spec parses without --spec");
        let CliCommand::Run {
            spec, auto_spec, ..
        } = cli.command
        else {
            panic!("expected the run subcommand");
        };
        assert_eq!(spec, None, "--spec stays absent under --auto-spec");
        assert!(auto_spec);

        // Bare `run` (no --spec, no --auto-spec) stays a clap error.
        assert!(
            Cli::try_parse_from(["chug", "run", "--goal", "g"]).is_err(),
            "run still requires --spec without --auto-spec"
        );
    }

    // ---------- T189: the validation-lane CLI flags ----------

    /// `--validate` / `--no-validate` parse on both auto-spec entry points
    /// (`run --auto-spec` and `quick`), default false, are mutually
    /// exclusive, and belong to the --auto-spec group: on a hand-written
    /// `--spec` run either flag is a clap error (the lane override is an
    /// auto-spec-path knob — a hand-written spec is the operator's
    /// contract, and per-item routing stays the orchestrator's per
    /// LOOP-SPEC §2 step 4). A dropped flag or flipped requires/conflicts
    /// wiring is RED here.
    #[test]
    fn validation_lane_cli_parse_pins() {
        // Defaults: both flags absent → false/false.
        let cli = Cli::try_parse_from(["chug", "run", "--spec", "s.md", "--goal", "g"])
            .expect("run --spec parses");
        let CliCommand::Run {
            validate, no_validate, ..
        } = cli.command
        else {
            panic!("expected the run subcommand");
        };
        assert!(!validate, "--validate defaults false");
        assert!(!no_validate, "--no-validate defaults false");

        // Each override parses under --auto-spec.
        let cli = Cli::try_parse_from(["chug", "run", "--auto-spec", "--validate", "--goal", "g"])
            .expect("--validate parses with --auto-spec");
        let CliCommand::Run {
            validate, no_validate, ..
        } = cli.command
        else {
            panic!("expected the run subcommand");
        };
        assert!(validate && !no_validate);

        let cli = Cli::try_parse_from([
            "chug", "run", "--auto-spec", "--no-validate", "--goal", "g",
        ])
        .expect("--no-validate parses with --auto-spec");
        let CliCommand::Run {
            validate, no_validate, ..
        } = cli.command
        else {
            panic!("expected the run subcommand");
        };
        assert!(!validate && no_validate);

        // The pair is mutually exclusive.
        assert!(
            Cli::try_parse_from([
                "chug", "run", "--auto-spec", "--validate", "--no-validate", "--goal", "g",
            ])
            .is_err(),
            "--validate and --no-validate together are a clap error"
        );

        // The overrides belong to the --auto-spec group: a hand-written
        // --spec run refuses either flag.
        assert!(
            Cli::try_parse_from(["chug", "run", "--spec", "s.md", "--validate", "--goal", "g"])
                .is_err(),
            "--validate without --auto-spec is a clap error"
        );
        assert!(
            Cli::try_parse_from(["chug", "run", "--spec", "s.md", "--no-validate", "--goal", "g"])
                .is_err(),
            "--no-validate without --auto-spec is a clap error"
        );

        // The quick alias (an auto-spec run) carries the same pair.
        let cli = Cli::try_parse_from(["chug", "quick", "--goal", "g", "--no-validate"])
            .expect("quick --no-validate parses");
        let CliCommand::Quick {
            validate, no_validate, ..
        } = cli.command
        else {
            panic!("expected the quick subcommand");
        };
        assert!(!validate && no_validate);
        let cli = Cli::try_parse_from(["chug", "quick", "--goal", "g", "--validate"])
            .expect("quick --validate parses");
        let CliCommand::Quick {
            validate, no_validate, ..
        } = cli.command
        else {
            panic!("expected the quick subcommand");
        };
        assert!(validate && !no_validate);
        assert!(
            Cli::try_parse_from(["chug", "quick", "--goal", "g", "--validate", "--no-validate"])
                .is_err(),
            "quick --validate --no-validate together are a clap error"
        );
    }

    /// The cmd_run refusals the CLI boundary enforces BEFORE any draft or
    /// LLM call: a hand-written `--spec` is mutually exclusive with
    /// `--auto-spec` (the operator's word is never overwritten), and
    /// `--auto-spec --resume` without a drafted spec on disk refuses with
    /// the remedy — never a silent fresh draft. Neither leg touches the
    /// network (both bail before the client is built) and neither leaves a
    /// draft behind.
    #[test]
    fn cmd_run_auto_spec_refusals_before_any_draft() {
        let tmp = tempfile::tempdir().unwrap();
        let spec = tmp.path().join("s.md");
        std::fs::write(&spec, "spec text\ncheck: true\n").unwrap();

        // --spec + --auto-spec: mutually exclusive.
        let err = cmd_run(
            Some(spec.clone()),
            true,
            None, // T189: no lane override (the refusal fires before the lane)
            "g".into(),
            Some(tmp.path().to_path_buf()),
            Some("test-model".into()),
            5,
            10,
            0,
            None,
            0, // ctx_warn_at_tokens
            false,
            false,
            false,
            None,
            None,
            true,
            None,
        )
        .expect_err("--spec and --auto-spec refuse");
        assert!(
            err.to_string().contains("mutually exclusive"),
            "the refusal names the exclusivity: {err}"
        );

        // --auto-spec --resume with no drafted spec: refuses, names the
        // remedy, drafts nothing.
        let err = cmd_run(
            None,
            true,
            None, // T189: no lane override (the refusal fires before the lane)
            "g".into(),
            Some(tmp.path().to_path_buf()),
            Some("test-model".into()),
            5,
            10,
            0,
            None,
            0, // ctx_warn_at_tokens
            true,
            false,
            false,
            None,
            None,
            true,
            None,
        )
        .expect_err("resume without a draft refuses");
        assert!(
            err.to_string().contains("no drafted spec"),
            "the refusal names the missing draft: {err}"
        );
        assert!(
            !tmp.path().join(".chug/auto-spec.md").exists(),
            "a refused auto-spec run drafts nothing"
        );
    }
}
