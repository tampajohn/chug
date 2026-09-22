//! T11: bake the git short hash into the binary for the startup banner
//! (`src/build_info.rs`). Never fails the build: no git binary, no `.git`,
//! or an unreadable `HEAD` all fall back to `"unknown"`.
//!
//! Override: `CHUG_GIT_HASH=<value>` in the build environment skips
//! detection (reproducible builds; `CHUG_GIT_HASH=unknown` simulates a
//! git-less build).

use std::fs;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=CHUG_GIT_HASH");
    emit_git_rerun_hints();
    let hash = std::env::var("CHUG_GIT_HASH")
        .ok()
        .and_then(|v| v.lines().next().map(str::trim).map(str::to_string))
        .filter(|v| !v.is_empty())
        .or_else(git_short_hash)
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=CHUG_GIT_HASH={hash}");
}

/// `git rev-parse --short HEAD`, best-effort. Any failure (git missing, not
/// a repo, odd output) → None.
fn git_short_hash() -> Option<String> {
    let out = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let hash = String::from_utf8(out.stdout).ok()?.trim().to_string();
    (!hash.is_empty()).then_some(hash)
}

/// Rebuild when the checked-out commit moves. In a plain checkout `.git` is
/// a directory: watch HEAD (branch switches) and the loose ref it points at
/// (new commits). In a worktree `.git` is a file — nothing useful to watch,
/// so skip silently (the hash is still captured correctly at build time).
fn emit_git_rerun_hints() {
    println!("cargo:rerun-if-changed=.git/HEAD");
    let Ok(head) = fs::read_to_string(".git/HEAD") else {
        return;
    };
    if let Some(reference) = head.trim().strip_prefix("ref: ") {
        println!("cargo:rerun-if-changed=.git/{}", reference.trim());
    }
}
