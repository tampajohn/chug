//! T47 — shared CARGO_TARGET_DIR for worktree builds.
//!
//! Doctrine pins, not behavior: every round's worktree build (impl child,
//! validator's mutate→test→revert→re-test, orchestrator gates) must land in
//! the shared `target-shared/` cache — a NEW dir, never the repo's own
//! `target/` (the operator's build cache stays separate so a wedge can't
//! poison daily builds). These tests pin the templates and supervisor wiring
//! so a later edit cannot silently drop the export or blur the separation.

use std::path::PathBuf;

const SHARED: &str = "CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared";
/// The repo's OWN target dir — no template may ever point CARGO_TARGET_DIR at
/// it (that is the separation the T47 review is required to check).
const REPO_TARGET: &str = "CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target";

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(rel: &str) -> String {
    std::fs::read_to_string(repo_root().join(rel))
        .unwrap_or_else(|e| panic!("reading {rel}: {e}"))
}

fn assert_contains(haystack: &str, needle: &str, what: &str) {
    assert!(
        haystack.contains(needle),
        "{what} must contain {needle:?} (T47)"
    );
}

#[test]
fn gitignore_ignores_target_shared() {
    let gitignore = read(".gitignore");
    assert!(
        gitignore.lines().any(|l| l.trim() == "target-shared/"),
        ".gitignore must gain a `target-shared/` line (T47); got:\n{gitignore}"
    );
}

#[test]
fn loopd_creates_target_shared_at_supervisor_start() {
    let loopd = read("loopd.sh");
    assert_contains(&loopd, "mkdir -p target-shared", "loopd.sh");
}

#[test]
fn loopd_exports_shared_dir_for_the_cycle_after_its_own_build() {
    let loopd = read("loopd.sh");
    assert_contains(&loopd, "export CARGO_TARGET_DIR=", "loopd.sh");
    // Ordering pin: the supervisor's own binary build (`cargo build` into
    // ./target, feeding `./target/debug/chug`) happens BEFORE the export, so
    // the operator's cache stays separate from the shared children's cache.
    let build = loopd
        .find("  cargo build >> \"$LOG\" 2>&1")
        .expect("loopd.sh builds its own binary first");
    let export = loopd
        .find("export CARGO_TARGET_DIR=")
        .expect("loopd.sh exports CARGO_TARGET_DIR for the cycle");
    assert!(
        build < export,
        "loopd.sh must export CARGO_TARGET_DIR AFTER its own `cargo build` \
         (the supervisor's binary build stays in ./target)"
    );
    assert_contains(&loopd, "$ROOT/target-shared", "loopd.sh export target");
}

#[test]
fn loop_spec_templates_export_the_shared_dir() {
    let spec = read("LOOP-SPEC.md");
    assert_contains(&spec, SHARED, "LOOP-SPEC.md");
    // Worktree build step (1), delegate goal (2), review-gate prefix (3) —
    // each carrier alone matters (delegate has no env parameter; bash calls
    // don't share env), so dropping any ONE of the three must fail this pin.
    assert!(
        spec.matches(SHARED).count() >= 3,
        "LOOP-SPEC.md must carry the export in the step-1 build line, the \
         step-2 goal template, AND the step-3 gate prefix (T47); found {}",
        spec.matches(SHARED).count()
    );
    // Tradeoff note: recovery is operator-owned and cheap.
    assert_contains(&spec, "rm -rf target-shared", "LOOP-SPEC.md");
}

#[test]
fn meta_spec_templates_export_the_shared_dir() {
    let spec = read("META-SPEC.md");
    assert_contains(&spec, SHARED, "META-SPEC.md");
    // Carriers: the Why-bullet, BOTH nohup launch templates (impl step 4 +
    // validator step 6), and §6's goal text (the delegate path has no env
    // parameter, so the goal carries the export to the validator).
    assert!(
        spec.matches(SHARED).count() >= 3,
        "META-SPEC.md must carry the export in the Why bullet, both nohup \
         launch templates, and §6's goal text (T47)"
    );
    assert_contains(&spec, "rm -rf target-shared", "META-SPEC.md");
}

#[test]
fn no_template_points_cargo_target_dir_at_the_repo_own_target() {
    for file in ["LOOP-SPEC.md", "META-SPEC.md"] {
        let spec = read(file);
        // Every `CARGO_TARGET_DIR=<repo>/target…` occurrence must continue
        // with `-shared` — pointing the var at the repo's own target/ is the
        // separation T47 forbids. (Never a bare contains(): the shared path
        // itself has the repo-target path as a prefix.)
        let offenders: Vec<usize> = spec
            .match_indices(REPO_TARGET)
            .filter(|(i, _)| !spec[*i + REPO_TARGET.len()..].starts_with("-shared"))
            .map(|(i, _)| i)
            .collect();
        assert!(
            offenders.is_empty(),
            "{file} must never point CARGO_TARGET_DIR at the repo's own \
             target/ (offsets {offenders:?}) — the shared dir is \
             target-shared (T47 separation)"
        );
    }
}
