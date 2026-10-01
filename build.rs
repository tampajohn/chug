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
/// (new commits). In a worktree `.git` is a FILE pointing at the main
/// repo's worktree gitdir, so `.git/HEAD` never exists as a path — and
/// cargo treats a missing rerun-if-changed target as ALWAYS stale, which
/// made the pre-T179 shape (hint emitted before the read) rebuild this
/// build script and all its dependents on EVERY cargo invocation in any
/// worktree (the cycle-81 discovery, confirmed with
/// `CARGO_LOG=cargo::core::compiler::fingerprint=trace cargo build`,
/// which logs `StaleItem(MissingFile { path: ".../.git/HEAD" })`; the tax
/// was the full crate, ~40-95s release per invocation, behind slow
/// worktree gates, eaten gate windows, and child budget deaths). So the
/// read comes FIRST and the hints are emitted ONLY when it succeeded: a
/// worktree build watches nothing and stays cache-clean, while the main
/// tree — where `.git/HEAD` exists — still watches HEAD and the ref, so
/// commit moves still rebuild the binary and the banner hash stays honest
/// where it worked before. Known gap (T179 out of scope, follow-up if
/// wanted): a worktree commit no longer retriggers the build script, so
/// its baked hash can go stale. Cycle 81 also found that exporting
/// CHUG_GIT_HASH in a gate env breaks the delegate-launch stub tests
/// (via the rerun-if-env-changed above) — documented, not changed here.
fn emit_git_rerun_hints() {
    let Ok(head) = fs::read_to_string(".git/HEAD") else {
        return;
    };
    println!("cargo:rerun-if-changed=.git/HEAD");
    if let Some(reference) = head.trim().strip_prefix("ref: ") {
        println!("cargo:rerun-if-changed=.git/{}", reference.trim());
    }
}
