//! T237 — loopd empty-cycle backoff: the cycle-OK sleep scales with
//! consecutive empty-delta dispositions (60s doubling to a 30-minute cap),
//! the streak counter walks the git record's wrap-notes subjects, and the
//! literal token `empty-delta disposition` is load-bearing in BOTH
//! carriers (the loopd.sh awk needle AND the LOOP-SPEC Phase-3 clause).
//!
//! Two layers, the tests/loopd_model_routing.rs + tests/loopd_stale_binary.rs
//! patterns:
//! - BEHAVIORAL tests that run the real `loopd.sh sleep-ok` mode against
//!   fixture git repos in a temp dir (the script cd's to its own
//!   directory, so a copied script inside a repo with crafted empty
//!   commits is a faithful harness; no repo file is touched). `sleep-ok`
//!   prints "<seconds> <streak>" for the cycle that WOULD sleep now,
//!   sleeping nothing — the routing-probe pattern, so every leg is
//!   instant (no sleeps, no cargo, no children).
//! - STATIC pins (the T47 count_eq pattern) on the wiring: the token
//!   appears EXACTLY ONCE per carrier (so removing it from the awk needle
//!   is RED, not masked by a comment), the base 60 and the default cap
//!   1800 are wired inside ok_sleep_seconds, the explicit-set-wins seam
//!   is present, the `cycle OK:` summary line is byte-identical (site-sync
//!   and log greps read it), and the new subcommand + usage line exist.
//!
//! T48 doctrine: pins resolve files from the checkout the binary RUNS
//! against (`std::env::current_dir()`; cargo runs test binaries with cwd =
//! the package root), never via the compile-time manifest-dir macro.

#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

fn repo_root() -> PathBuf {
    std::env::current_dir().expect("cargo sets the test cwd to the package root")
}

fn read(rel: &str) -> String {
    std::fs::read_to_string(repo_root().join(rel))
        .unwrap_or_else(|e| panic!("reading {rel}: {e}"))
}

/// Per-carrier pin (the T47 count_eq pattern): `needle` must occur EXACTLY
/// `expected` times in `haystack`.
fn count_eq(haystack: &str, needle: &str, expected: usize, what: &str) {
    let found = haystack.matches(needle).count();
    assert_eq!(
        found, expected,
        "{what}: expected {needle:?} exactly {expected}×, found {found}×"
    );
}

// --- fixture harness ----------------------------------------------------------

/// A code-commit subject: any non-`eval:` subject is landed work and stops
/// the streak walk (the reset leg).
const CODE: &str = "feat: T237 empty-cycle backoff lands (code commit, streak resets)";
/// Empty-delta disposition wrap-notes subjects — the real cycle-110..113
/// shape: `eval:` prefix, `wrap notes`, and the load-bearing token.
const W110: &str = "eval: cycle-110 wrap notes (empty-delta disposition, 1st consecutive no-op)";
const W111: &str = "eval: cycle-111 wrap notes (empty-delta disposition, 2nd consecutive no-op)";
const W112: &str = "eval: cycle-112 wrap notes (empty-delta disposition, 3rd consecutive no-op)";
const W113: &str = "eval: cycle-113 wrap notes (empty-delta disposition, 4th consecutive no-op)";
const W114: &str = "eval: cycle-114 wrap notes (empty-delta disposition, 5th consecutive no-op)";
const W115: &str = "eval: cycle-115 wrap notes (empty-delta disposition, 6th consecutive no-op)";
/// The compaction interleave: an `eval:` commit that is NOT a wrap note
/// (Outcomes compaction) — the streak walk must SKIP it without stopping.
const COMPACTION: &str =
    "eval: cycle-111 Outcomes compaction (cycles 104+105 one-lined per the keep-last-6-full rule)";
/// A REAL wrap (an actual eval ran; rows landed): an `eval:` wrap-notes
/// subject WITHOUT the token — the streak resets.
const REAL_EVAL_WRAP: &str =
    "eval: cycle-109 wrap notes (eval-only cycle — ZERO rows filed, queue drained)";

/// Run one git command in `root` with a throwaway identity (the fixture
/// commits are content, not history worth signing).
fn git(root: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(["-c", "user.name=t237", "-c", "user.email=t237@example.test"])
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .expect("spawn git");
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Fixture dir: a copy of the real loopd.sh (plus its T213 loader fragment)
/// inside a fresh git repo whose history is `subjects` (oldest first, each
/// an empty commit). The script cd's to its own directory, so the copy sees
/// the fixture repo — a faithful harness for the streak walk (the T81
/// routing-fixture pattern; no repo file is touched).
fn fixture_repo(subjects: &[&str]) -> TempDir {
    let tmp = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        tmp.path().join("loopd.sh"),
        std::fs::read_to_string(repo_root().join("loopd.sh")).expect("loopd.sh readable"),
    )
    .expect("copy loopd.sh into fixture dir");
    // T213: loopd.sh sources scripts/loopd_env_loader.sh relative to its own
    // path — without the fragment beside the copy the sourcing dies under
    // set -euo pipefail before any mode dispatch (the routing fixture's
    // lesson, reused verbatim).
    let scripts = tmp.path().join("scripts");
    std::fs::create_dir_all(&scripts).expect("fixture scripts dir");
    std::fs::copy(
        repo_root().join("scripts/loopd_env_loader.sh"),
        scripts.join("loopd_env_loader.sh"),
    )
    .expect("copy loopd_env_loader.sh into fixture dir");
    git(tmp.path(), &["init", "-q"]);
    for subject in subjects {
        git(tmp.path(), &["commit", "--allow-empty", "-m", subject]);
    }
    tmp
}

/// Run the copied script's `sleep-ok` mode with the env CLEARED of the
/// backoff knobs (and the git vars that could redirect the walk), then the
/// caller's overrides applied. Prints "<seconds> <streak>".
fn run_sleep_ok(root: &Path, envs: &[(&str, &str)]) -> String {
    let mut cmd = Command::new("bash");
    cmd.arg(root.join("loopd.sh"))
        .arg("sleep-ok")
        .current_dir(root)
        .env_remove("LOOPD_SLEEP_OK")
        .env_remove("LOOPD_EMPTY_SLEEP_CAP")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE");
    for (k, v) in envs {
        cmd.env(k, v);
    }
    let out = cmd.output().expect("spawn loopd.sh sleep-ok");
    assert!(
        out.status.success(),
        "sleep-ok exited {:?}: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

// --- behavioral legs -----------------------------------------------------------

/// n=0: HEAD is a code commit (landed work) — the streak walk stops
/// immediately and the sleep is the untouched 60s default (byte-identical
/// to the pre-T237 behavior at streak 0).
#[test]
fn code_head_yields_the_untouched_60s_default() {
    let root = fixture_repo(&[CODE]);
    assert_eq!(
        run_sleep_ok(root.path(), &[]),
        "60 0",
        "a code commit at HEAD → 60s sleep, streak 0 (req 2's n=0 leg)"
    );
}

/// Each consecutive empty-delta wrap doubles the sleep: 120 → 240 → 480.
#[test]
fn each_consecutive_empty_wrap_doubles_the_sleep() {
    for (want, subjects) in [
        ("120 1", vec![CODE, W110]),
        ("240 2", vec![CODE, W110, W111]),
        ("480 3", vec![CODE, W110, W111, W112]),
    ] {
        let root = fixture_repo(&subjects);
        assert_eq!(
            run_sleep_ok(root.path(), &[]),
            want,
            "each consecutive empty wrap doubles the 60s base (req 2's \
             progression)"
        );
    }
}

/// The compaction interleave (the exact cycle-111 shape): an `eval:`
/// bookkeeping commit BETWEEN two empty wraps must neither count nor stop
/// the walk — skip-does-not-stop, so the streak reads 2, not 1.
#[test]
fn compaction_bookkeeping_skips_without_stopping() {
    let root = fixture_repo(&[CODE, W110, COMPACTION, W111]);
    assert_eq!(
        run_sleep_ok(root.path(), &[]),
        "240 2",
        "empty wrap → compaction → empty wrap counts 2 (the skip rule is \
         load-bearing; req 1's interleave case)"
    );
}

/// A real wrap at HEAD (an `eval:` wrap-notes subject without the token —
/// an actual eval ran) BREAKS the streak: sleep back to 60, streak 0.
#[test]
fn a_real_eval_wrap_resets_the_streak() {
    let root = fixture_repo(&[CODE, W110, W111, REAL_EVAL_WRAP]);
    assert_eq!(
        run_sleep_ok(root.path(), &[]),
        "60 0",
        "a non-empty wrap-notes subject resets the streak (req 1's stop rule)"
    );
}

/// The doubling plateaus at the default 1800s cap: 5 empty wraps reach it,
/// 6+ stay there — the no-op cadence floor is one launch per 30 minutes.
/// This leg is also the DEFAULT-cap guard: a drifted 1800 literal is RED
/// here even if every explicit-cap leg is pinned to its own value.
#[test]
fn the_doubling_plateaus_at_the_default_cap() {
    let five = fixture_repo(&[CODE, W110, W111, W112, W113, W114]);
    assert_eq!(
        run_sleep_ok(five.path(), &[]),
        "1800 5",
        "streak 5 → the 1800s default cap (req 2's n≥5 leg)"
    );
    let six = fixture_repo(&[CODE, W110, W111, W112, W113, W114, W115]);
    assert_eq!(
        run_sleep_ok(six.path(), &[]),
        "1800 6",
        "streak 6 stays at the cap — the doubling never overshoots"
    );
}

/// The T137 test seam wins BYTE-IDENTICALLY (explicit-set-wins): with
/// LOOPD_SLEEP_OK set, the sleep is the seam value verbatim regardless of
/// the streak — the four existing `LOOPD_SLEEP_OK=1` behavioral legs keep
/// their pacing untouched.
#[test]
fn explicit_loopd_sleep_ok_wins_verbatim_regardless_of_streak() {
    let root = fixture_repo(&[CODE, W110, W111, W112]);
    assert_eq!(
        run_sleep_ok(root.path(), &[("LOOPD_SLEEP_OK", "7")]),
        "7 3",
        "the seam echoes 7 verbatim at streak 3 (req 2's explicit-set-wins)"
    );
}

/// LOOPD_EMPTY_SLEEP_CAP retunes the ceiling: at streak 4 (960s uncapped)
/// a 300s cap clamps the doubling at 300.
#[test]
fn loopd_empty_sleep_cap_retunes_the_ceiling() {
    let root = fixture_repo(&[CODE, W110, W111, W112, W113]);
    assert_eq!(
        run_sleep_ok(root.path(), &[("LOOPD_EMPTY_SLEEP_CAP", "300")]),
        "300 4",
        "the cap clamps the streak-4 rung of 960 down to 300 (req 2's cap leg)"
    );
}

// --- static pins ---------------------------------------------------------------

/// The load-bearing token: EXACTLY ONCE in loopd.sh (the awk needle — a
/// comment carrying it would mask a needle-removal mutant) and EXACTLY
/// ONCE in LOOP-SPEC.md (the req-5 Phase-3 clause). Cross-file equality of
/// the literal is what makes the convention doctrine: a future reworded
/// wrap subject that drops the token silently resets the streak, and the
/// spec clause is what forbids that.
#[test]
fn the_token_is_load_bearing_in_both_carriers() {
    let loopd = read("loopd.sh");
    let spec = read("LOOP-SPEC.md");
    count_eq(&loopd, "empty-delta disposition", 1, "the awk needle in loopd.sh (T237 req 1)");
    count_eq(&spec, "empty-delta disposition", 1, "the LOOP-SPEC Phase-3 clause (T237 req 5)");
}

/// The streak walk's three-way rule is wired in the awk program: token →
/// count, bookkeeping (an `eval:` commit that is not a wrap note) → skip,
/// everything else → stop. The skip leg is the easy one to lose silently
/// (a compaction commit between wraps would reset the streak to 1).
#[test]
fn the_streak_walks_rule_shape_is_wired() {
    let loopd = read("loopd.sh");
    let fn_at = loopd
        .find("empty_wrap_streak() {")
        .expect("empty_wrap_streak is defined");
    let body_end = loopd[fn_at..]
        .find("\n}")
        .map(|i| fn_at + i)
        .expect("empty_wrap_streak closes");
    let body = &loopd[fn_at..=body_end];
    assert!(
        body.contains("/^eval:/"),
        "the walk keys on the `eval:` subject prefix (req 1)"
    );
    assert!(
        body.contains("index($0, \"wrap notes\")"),
        "the walk distinguishes wrap notes from eval bookkeeping (req 1)"
    );
    count_eq(&loopd, "git log --format=%s -30", 1, "the walk reads subjects, bounded at 30 (req 1)");
    assert!(
        loopd.contains("subjects=$(git log --format=%s -30 2>/dev/null) || rc=$?"),
        "a non-git cwd degrades to 0, never a set -e/pipefail death (req 1)"
    );
}

/// The cap and base are wired INSIDE ok_sleep_seconds, and the doubling is
/// a POSIX-safe loop (no GNU-isms, no lookup table to drift).
#[test]
fn the_cap_and_base_are_wired_inside_ok_sleep_seconds() {
    let loopd = read("loopd.sh");
    let fn_at = loopd
        .find("ok_sleep_seconds() {")
        .expect("ok_sleep_seconds is defined");
    let body_end = loopd[fn_at..]
        .find("\n}")
        .map(|i| fn_at + i)
        .expect("ok_sleep_seconds closes");
    let body = &loopd[fn_at..=body_end];
    assert!(
        body.contains("s=60"),
        "the 60s base must be wired inside ok_sleep_seconds (req 2: n=0 is \
         byte-identical to the old default)"
    );
    count_eq(
        &loopd,
        "${LOOPD_EMPTY_SLEEP_CAP:-1800}",
        2,
        "the default cap expansion at BOTH carriers: the function's default \
         and the run-loop log line (req 2 + the pacing log line)"
    );
    assert!(
        body.contains("s=$((s * 2))"),
        "the POSIX-safe doubling step (req 2: no GNU-isms)"
    );
}

/// Explicit-set-wins: the seam check reads LOOPD_SLEEP_OK non-empty and
/// echoes it verbatim BEFORE the streak is consulted — the T137 seam
/// semantics the four existing `LOOPD_SLEEP_OK=1` legs depend on.
#[test]
fn the_explicit_set_wins_seam_is_wired() {
    let loopd = read("loopd.sh");
    count_eq(
        &loopd,
        "if [ -n \"${LOOPD_SLEEP_OK:-}\" ]; then",
        1,
        "the seam's non-empty check precedes the doubling (req 2)"
    );
    count_eq(
        &loopd,
        "echo \"$LOOPD_SLEEP_OK\"",
        1,
        "the seam value is echoed verbatim (req 2: byte-identical)"
    );
    // The check precedes the streak walk inside the function.
    let seam = loopd
        .find("if [ -n \"${LOOPD_SLEEP_OK:-}\" ]; then")
        .expect("the seam check is present");
    let streak_call = loopd
        .find("streak=$(empty_wrap_streak)")
        .expect("the streak call is present");
    assert!(
        seam < streak_call,
        "the seam must be consulted BEFORE the streak walk (explicit \
         set wins; the T137 test legs set LOOPD_SLEEP_OK=1 and must never \
         pay a git walk or the doubling)"
    );
}

/// The cycle-OK branch consumes the computed value and logs the pacing
/// once; the `cycle OK: $summary` line stays BYTE-IDENTICAL (site-sync and
/// log greps read it — T142/T137 house doctrine).
#[test]
fn the_cycle_ok_branch_uses_the_computed_sleep() {
    let loopd = read("loopd.sh");
    count_eq(
        &loopd,
        "echo \"$(ts) cycle OK: $summary\" >> \"$LOG\"",
        1,
        "the `cycle OK:` summary line is byte-identical (site-sync and log \
         greps read it)"
    );
    count_eq(
        &loopd,
        "ok_secs=$(ok_sleep_seconds)",
        1,
        "the run loop computes the sleep through ok_sleep_seconds (req 2)"
    );
    count_eq(
        &loopd,
        "sleep \"$ok_secs\"",
        1,
        "the run loop sleeps the computed value (req 2: the literal is gone)"
    );
    assert!(
        !loopd.contains("sleep \"${LOOPD_SLEEP_OK:-60}\""),
        "the old flat default sleep must be gone — the seam now resolves \
         inside ok_sleep_seconds (req 2)"
    );
    count_eq(
        &loopd,
        "cycle-OK sleep ${ok_secs}s (empty streak",
        1,
        "the ONE new pacing log line names the sleep/streak (req 2)"
    );
}

/// The `sleep-ok` subcommand exists, prints "<seconds> <streak>", sleeps
/// nothing, and the usage line names it (req 3).
#[test]
fn the_sleep_ok_subcommand_and_usage_line_exist() {
    let loopd = read("loopd.sh");
    count_eq(&loopd, "sleep-ok)", 1, "the sleep-ok case arm (req 3)");
    count_eq(
        &loopd,
        "usage: loopd.sh [run|stop|status|routing] [sleep-ok]",
        1,
        "the usage line names the new subcommand (req 3)"
    );
    // The arm prints BOTH fields and nothing else.
    let arm_at = loopd.find("sleep-ok)").expect("the sleep-ok arm is present");
    let arm_end = loopd[arm_at..]
        .find(";;")
        .map(|i| arm_at + i)
        .expect("the sleep-ok arm closes");
    let arm = &loopd[arm_at..arm_end];
    assert!(
        arm.contains("echo \"$(ok_sleep_seconds) $(empty_wrap_streak)\""),
        "the arm prints `<seconds> <streak>` from the two functions (req 3)"
    );
    assert!(
        !arm.lines().any(|l| l.trim_start().starts_with("sleep ")),
        "the arm runs no sleep command — it is a probe (req 3: the routing \
         pattern)"
    );
}
