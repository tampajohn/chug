mod api;
mod archive;
mod attach;
mod auth;
mod build_info;
mod chat;
mod complete;
mod driver;
mod eventlog;
mod events;
mod riskgate;
mod ledger;
mod observ;
mod tools;
mod transcript;
mod tui;
mod mcp;
mod mcp_http;
mod sse;

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, mpsc};
use std::thread;

use anyhow::{Context, anyhow};
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
        #[arg(long)]
        spec: PathBuf,
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
    },
    /// Print the current LEDGER.md.
    Ledger {
        /// Working directory. Defaults to `.`.
        #[arg(long)]
        cwd: Option<PathBuf>,
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

fn main() -> ExitCode {
    let cli = Cli::parse();
    // SPEC-8: the observability sink must drain on EVERY exit path —
    // including a panic unwinding out of command dispatch, caught here so
    // the queued events still flush before the process exits.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match cli.command {
        CliCommand::Ledger { cwd } => cmd_ledger(cwd),
        CliCommand::Chat {
            cwd,
            model,
            max_iters,
            max_minutes,
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
            resume,
            risk_gate,
            bash_timeout,
            mcp_config,
            mcp_off,
        ),
        CliCommand::Run {
            spec,
            goal,
            cwd,
            model,
            max_iters,
            max_minutes,
            resume,
            tui,
            risk_gate,
            bash_timeout,
            mcp_config,
            mcp_off,
        } => cmd_run(
            spec,
            goal,
            cwd,
            model,
            max_iters,
            max_minutes,
            resume,
            tui,
            risk_gate,
            bash_timeout,
            mcp_config,
            mcp_off,
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

#[allow(clippy::too_many_arguments)]
fn cmd_run(
    spec: PathBuf,
    goal: String,
    cwd: Option<PathBuf>,
    model: Option<String>,
    max_iters: u32,
    max_minutes: u64,
    resume: bool,
    tui: bool,
    risk_gate: bool,
    bash_timeout: Option<u64>,
    mcp_config: Option<PathBuf>,
    mcp_off: bool,
) -> anyhow::Result<i32> {
    let cwd = resolve_cwd(cwd)?;
    let bash_timeout = resolve_bash_timeout(bash_timeout)?;
    let spec = spec
        .canonicalize()
        .with_context(|| format!("spec file {} not found", spec.display()))?;
    let model = model
        .filter(|m| !m.trim().is_empty())
        .or_else(|| std::env::var("CHUG_MODEL").ok().filter(|m| !m.trim().is_empty()))
        .unwrap_or_else(|| driver::DEFAULT_MODEL.to_string());
    // T11: one stderr line naming the build so a stale binary is obvious.
    build_info::print_startup_banner(&cwd, Some(&spec), &model);

    if tui {
        run_with_tui(
            spec, goal, cwd, model, max_iters, max_minutes, resume, risk_gate,
            bash_timeout, mcp_config, mcp_off,
        )
    } else {
        let cfg = driver::RunConfig {
            cwd,
            spec_path: spec,
            goal,
            model,
            max_iters,
            max_minutes,
            resume,
            controls: driver::Controls::detached(),
            risk_gate,
            bash_timeout,
            mcp_config,
            mcp_off,
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
    cwd: PathBuf,
    model: String,
    max_iters: u32,
    max_minutes: u64,
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
        resume,
        risk_gate,
        bash_timeout,
        mcp_config,
        mcp_off,
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
    resume: bool,
    risk_gate: bool,
    bash_timeout: Option<u64>,
    mcp_config: Option<PathBuf>,
    mcp_off: bool,
) -> anyhow::Result<i32> {
    let cwd = resolve_cwd(cwd)?;
    let bash_timeout = resolve_bash_timeout(bash_timeout)?;
    let model = model
        .filter(|m| !m.trim().is_empty())
        .or_else(|| std::env::var("CHUG_MODEL").ok().filter(|m| !m.trim().is_empty()))
        .unwrap_or_else(|| driver::DEFAULT_MODEL.to_string());
    // T11: same banner as `run` (chat has no spec yet — one may arrive via /spec).
    build_info::print_startup_banner(&cwd, None, &model);

    let (event_tx, event_rx) = mpsc::channel::<events::Event>();
    let (steer_tx, steer_rx) = mpsc::channel::<String>();
    let (objective_tx, objective_rx) = mpsc::channel::<String>();
    let (update_tx, update_rx) = mpsc::channel::<driver::SlashUpdate>();
    let abort = Arc::new(AtomicBool::new(false));
    let driver_done = Arc::new(AtomicBool::new(false));

    let cfg = chat::ChatConfig {
        cwd: cwd.clone(),
        model: model.clone(),
        max_iters,
        max_minutes,
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
}
