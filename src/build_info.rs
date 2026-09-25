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
//!
//! T20: the banner also names the **checkout's** worktree HEAD at runtime
//! (`head=<branch>@<short>`, see [`resolve_head`]). That is a second,
//! deliberately distinct identity: children run the main-tree binary with a
//! worktree cwd (LOOP-SPEC §2), so the baked commit names the binary's build
//! tree while `head=` names the cwd the child actually sits in — their
//! difference is the wrong-HEAD confusion signal this row exists for.

use std::path::Path;
use std::process::Command;

/// Package version, baked in by cargo.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Git short hash of the commit this binary was built from, baked in by
/// `build.rs` (`"unknown"` when it could not be determined).
pub const GIT_COMMIT: &str = env!("CHUG_GIT_HASH");

/// Best-effort runtime resolution of the **cwd's** worktree HEAD: the pair
/// `(branch, short_commit)` from `git rev-parse --abbrev-ref HEAD` (the
/// branch is literally `"HEAD"` when detached) and `git rev-parse --short
/// HEAD`. Plain `Command` spawn with captured output; ANY failure — not a
/// repo, git missing, git erroring — yields `None`. Never blocks, never
/// fails the run: callers just omit the checkout identity. This is the
/// first runtime git call in the binary (build.rs only bakes
/// [`GIT_COMMIT`] at build time).
pub fn resolve_head(cwd: &Path) -> Option<(String, String)> {
    let branch = git_out("git", cwd, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    let commit = git_out("git", cwd, &["rev-parse", "--short", "HEAD"])?;
    Some((branch, commit))
}

/// Borrow a resolved head as the plain `(&branch, &commit)` pair the pure
/// render seams ([`banner`] and [`crate::eventlog::run_start`]) take.
pub fn as_pair(head: &Option<(String, String)>) -> Option<(&str, &str)> {
    head.as_ref().map(|(b, c)| (b.as_str(), c.as_str()))
}

/// One best-effort `git <args>` in `cwd`: trimmed stdout on success, else
/// `None` (spawn failure, non-zero exit, empty output). Both stdout and
/// stderr are captured, so a failing git never leaks onto the terminal.
fn git_out(program: &str, cwd: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new(program).args(args).current_dir(cwd).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if stdout.is_empty() { None } else { Some(stdout) }
}

/// The one-line startup banner:
/// `chug <version> (<commit>) cwd=<cwd> spec=<path|-> model=<model>`, plus
/// ` head=<branch>@<short>` when the cwd's checkout HEAD resolved (T20).
/// `commit` is the binary's build-time hash; `head` is the *checkout's*
/// branch@short — both are meaningful and their difference is the signal.
/// `head` is `None` whenever the checkout identity couldn't be resolved,
/// and then the line is byte-identical to the pre-T20 banner.
pub fn banner(
    commit: &str,
    head: Option<(&str, &str)>,
    cwd: &Path,
    spec: Option<&Path>,
    model: &str,
) -> String {
    let spec = spec.map_or_else(|| "-".to_string(), |p| p.display().to_string());
    let head = head.map_or_else(String::new, |(b, c)| format!(" head={b}@{c}"));
    format!(
        "chug {VERSION} ({commit}) cwd={} spec={spec} model={model}{head}",
        cwd.display()
    )
}

/// Print the banner (with this build's commit and the caller-resolved
/// checkout identity) to stderr. Called once at `chug run` / `chug chat`
/// start, before the driver or the TUI takes over. The banner never fails
/// the run: an unresolved `head` simply omits the field.
pub fn print_startup_banner(
    cwd: &Path,
    spec: Option<&Path>,
    model: &str,
    head: Option<(&str, &str)>,
) {
    eprintln!("{}", banner(GIT_COMMIT, head, cwd, spec, model));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn banner_contains_version_commit_cwd_spec_and_model() {
        let line = banner(
            "abc1234",
            None,
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
        let line = banner("abc1234", None, Path::new("/w"), None, "m");
        assert!(line.contains(" spec=- "), "{line}");
        assert!(line.ends_with(" model=m"), "{line}");
    }

    #[test]
    fn banner_renders_unknown_commit_fallback() {
        // The git-less build path: build.rs could not resolve a hash (or the
        // CHUG_GIT_HASH=unknown override simulated that) → "(unknown)".
        let line = banner("unknown", None, Path::new("/w"), None, "m");
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

    // ---------- T20: the checkout's head=<branch>@<short> ----------

    #[test]
    fn banner_appends_head_field_when_checkout_resolves() {
        let line = banner(
            "abc1234",
            Some(("loop-t18", "9056c78")),
            Path::new("/repo/work"),
            Some(Path::new("/repo/SPEC.md")),
            "claude-sonnet-4-6",
        );
        assert!(
            line.contains("head=loop-t18@9056c78"),
            "checkout identity present: {line}"
        );
        assert_eq!(
            line,
            "chug 0.1.0 (abc1234) cwd=/repo/work spec=/repo/SPEC.md model=claude-sonnet-4-6 head=loop-t18@9056c78"
        );
    }

    #[test]
    fn banner_without_head_is_the_pre_t20_bytes_exactly() {
        // The unresolvable leg must stay byte-identical to the pre-T20
        // banner: no head= field, model still last.
        let line = banner("abc1234", None, Path::new("/w"), None, "m");
        assert_eq!(line, "chug 0.1.0 (abc1234) cwd=/w spec=- model=m");
        assert!(!line.contains("head="), "no checkout field: {line}");
    }

    #[test]
    fn head_leg_differs_from_fallback_by_the_appended_field_only() {
        // Non-vacuousness seam: the resolved leg is exactly the fallback
        // bytes plus ` head=<branch>@<short>`. Reverting the head= render
        // (mutation) fails `banner_appends_head_field_when_checkout_resolves`;
        // mangling the base line fails both it and this pin.
        let with = banner("abc1234", Some(("loop-t20", "deadbeef")), Path::new("/w"), None, "m");
        let without = banner("abc1234", None, Path::new("/w"), None, "m");
        assert_eq!(with, format!("{without} head=loop-t20@deadbeef"));
    }

    #[test]
    fn resolve_head_in_the_real_repo_yields_branch_and_hex_short_hash() {
        // Integration: the chug checkout itself (whatever worktree the test
        // binary was built in) resolves to a non-empty branch and a 7+ char
        // lowercase-hex short hash.
        let (branch, commit) =
            resolve_head(Path::new(env!("CARGO_MANIFEST_DIR"))).expect("checkout resolves");
        assert!(!branch.is_empty(), "branch: {branch:?}");
        assert!(
            commit.len() >= 7 && commit.chars().all(|c| c.is_ascii_hexdigit()),
            "short hash is 7+ hex chars: {commit:?}"
        );
    }

    #[test]
    fn resolve_head_outside_a_repo_is_none() {
        // A tempdir is not a repo: no panic, no output, plain None.
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(resolve_head(tmp.path()), None);
    }

    #[test]
    fn broken_git_repository_fails_soft_to_none() {
        // Simulated git failure: a repo-shaped cwd whose .git is garbage.
        // git rev-parse exits non-zero → None; nothing may panic or print.
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join(".git")).unwrap();
        fs::write(tmp.path().join(".git/HEAD"), "not a real ref\n").unwrap();
        assert_eq!(resolve_head(tmp.path()), None);
    }

    #[test]
    fn missing_git_binary_fails_soft_to_none() {
        // The spawn-error leg (git not on PATH): spawn failure → None.
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(
            git_out("definitely-not-git-xyz", tmp.path(), &["rev-parse", "HEAD"]),
            None
        );
    }

    #[test]
    fn resolve_head_names_a_detached_head_literally() {
        // Integration: detached HEAD → branch is literally "HEAD" (the
        // spec'd detached semantics), commit still a short hash.
        let tmp = tempfile::tempdir().unwrap();
        let git = |args: &[&str]| {
            let out = Command::new("git")
                .args(args)
                .current_dir(tmp.path())
                .output()
                .expect("git available for integration pin");
            assert!(
                out.status.success(),
                "git {args:?} failed: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        };
        git(&["init", "-q"]);
        fs::write(tmp.path().join("f.txt"), "x").unwrap();
        git(&["add", "f.txt"]);
        git(&["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-qm", "x"]);
        git(&["checkout", "-q", "--detach", "HEAD"]);
        let (branch, commit) = resolve_head(tmp.path()).expect("detached repo resolves");
        assert_eq!(branch, "HEAD");
        assert!(commit.len() >= 7, "{commit:?}");
    }
}
