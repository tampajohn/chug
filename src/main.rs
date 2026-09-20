mod api;
mod driver;
mod ledger;
mod tools;
mod transcript;

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Context;
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
        } => cmd_run(spec, goal, cwd, model, max_iters, max_minutes, resume),
    };
    match result {
        Ok(code) => ExitCode::from(code as u8),
        Err(e) => {
            eprintln!("chug: error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn cmd_run(
    spec: PathBuf,
    goal: String,
    cwd: Option<PathBuf>,
    model: Option<String>,
    max_iters: u32,
    max_minutes: u64,
    resume: bool,
) -> anyhow::Result<i32> {
    let cwd = resolve_cwd(cwd)?;
    let spec = spec
        .canonicalize()
        .with_context(|| format!("spec file {} not found", spec.display()))?;
    let model = model
        .filter(|m| !m.trim().is_empty())
        .or_else(|| std::env::var("CHUG_MODEL").ok().filter(|m| !m.trim().is_empty()))
        .unwrap_or_else(|| driver::DEFAULT_MODEL.to_string());
    let cfg = driver::RunConfig {
        cwd,
        spec_path: spec,
        goal,
        model,
        max_iters,
        max_minutes,
        resume,
    };
    driver::run(cfg)
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
