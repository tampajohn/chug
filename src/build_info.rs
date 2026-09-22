//! T11: build identification for the startup banner.
//!
//! Stale-binary confusion burned two sessions (EVALUATION.md I11: a pty
//! smoke ran an old `target/debug/chug`; an orchestrator ran a pre-T4
//! binary so its own session never got the fix it was testing), so every
//! `chug run` / `chug chat` prints one stderr line at start carrying the
//! version, baked-in commit, cwd, spec, and model — and the same fields
//! open the run's `.chug/events.jsonl` (see [`crate::eventlog::run_start`]).
//!
//! [`GIT_COMMIT`] is captured at build time by `build.rs`
//! (`git rev-parse --short HEAD`) and falls back to `"unknown"` when git or
//! the repo is unavailable — the build never fails over the banner.

use std::path::Path;

/// Package version, baked in by cargo.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Git short hash of the commit this binary was built from, baked in by
/// `build.rs` (`"unknown"` when it could not be determined).
pub const GIT_COMMIT: &str = env!("CHUG_GIT_HASH");

/// The one-line startup banner:
/// `chug <version> (<commit>) cwd=<cwd> spec=<path|-> model=<model>`.
pub fn banner(commit: &str, cwd: &Path, spec: Option<&Path>, model: &str) -> String {
    let spec = spec.map_or_else(|| "-".to_string(), |p| p.display().to_string());
    format!(
        "chug {VERSION} ({commit}) cwd={} spec={spec} model={model}",
        cwd.display()
    )
}

/// Print the banner (with this build's commit) to stderr. Called once at
/// `chug run` / `chug chat` start, before the driver or the TUI takes over.
pub fn print_startup_banner(cwd: &Path, spec: Option<&Path>, model: &str) {
    eprintln!("{}", banner(GIT_COMMIT, cwd, spec, model));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn banner_contains_version_commit_cwd_spec_and_model() {
        let line = banner(
            "abc1234",
            Path::new("/repo/work"),
            Some(Path::new("/repo/SPEC.md")),
            "claude-sonnet-4-6",
        );
        assert_eq!(
            line,
            "chug 0.1.0 (abc1234) cwd=/repo/work spec=/repo/SPEC.md model=claude-sonnet-4-6"
        );
        assert!(line.contains(VERSION), "package version present: {line}");
    }

    #[test]
    fn banner_without_spec_uses_dash_placeholder() {
        // Chat starts without a spec (one may arrive later via /spec).
        let line = banner("abc1234", Path::new("/w"), None, "m");
        assert!(line.contains(" spec=- "), "{line}");
        assert!(line.ends_with(" model=m"), "{line}");
    }

    #[test]
    fn banner_renders_unknown_commit_fallback() {
        // The git-less build path: build.rs could not resolve a hash (or the
        // CHUG_GIT_HASH=unknown override simulated that) → "(unknown)".
        let line = banner("unknown", Path::new("/w"), None, "m");
        assert!(
            line.starts_with(&format!("chug {VERSION} (unknown) ")),
            "{line}"
        );
    }

    #[test]
    fn baked_commit_is_a_single_non_empty_token() {
        assert!(!GIT_COMMIT.is_empty());
        assert!(
            GIT_COMMIT.chars().all(|c| !c.is_whitespace()),
            "commit must be one banner-safe token: {GIT_COMMIT:?}"
        );
    }
}
