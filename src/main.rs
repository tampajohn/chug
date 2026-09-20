mod api;
mod driver;
mod events;
mod riskgate;
mod ledger;
mod tools;
mod transcript;
mod tui;

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
    },
    /// Print the current LEDGER.md.
    Ledger {
        /// Working directory. Defaults to `.`.
        #[arg(long)]
        cwd: Option<PathBuf>,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        CliCommand::Ledger { cwd } => cmd_ledger(cwd),
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
        ),
    };
    match result {
        Ok(code) => ExitCode::from(code as u8),
        Err(e) => {
            eprintln!("chug: error: {e:#}");
            ExitCode::FAILURE
        }
    }
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
) -> anyhow::Result<i32> {
    let cwd = resolve_cwd(cwd)?;
    let spec = spec
        .canonicalize()
        .with_context(|| format!("spec file {} not found", spec.display()))?;
    let model = model
        .filter(|m| !m.trim().is_empty())
        .or_else(|| std::env::var("CHUG_MODEL").ok().filter(|m| !m.trim().is_empty()))
        .unwrap_or_else(|| driver::DEFAULT_MODEL.to_string());

    if tui {
        run_with_tui(
            spec, goal, cwd, model, max_iters, max_minutes, resume, risk_gate,
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
    });

    let code = worker
        .join()
        .map_err(|e| anyhow!("driver thread panicked: {e:?}"))??;
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
