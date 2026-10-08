//! T258 — the cheap exit: the mechanical disposition predicate, gated by
//! `loopd.sh` BEFORE any launch, so an empty delta costs zero LLM calls.
//!
//! Operator 2026-10-07: "why do we need 20-30 kimi iterations to say
//! nothing to do?" Every eval-mode cycle — including the non-trip empty
//! dispositions the T247 chain authorizes — launched a full kimi stream.
//! The disposition predicate is mechanical (no new TODO rows, no child
//! deaths, a bookkeeping-only delta since the last evaluation, EVALUATION.md
//! fresh), so the supervisor computes it itself: when ALL four inputs are
//! quiet and the T247 valve has not tripped, loopd writes the one-line
//! disposition ITSELF (an empty commit whose subject carries the T237
//! token) and skips the launch; any single non-empty input forces the
//! launch, and a borderline non-trip eval (non-empty only through deaths
//! or new rows over a bookkeeping-only delta with a fresh evaluation)
//! routes glm instead of kimi.
//!
//! Three layers, the tests/loopd_empty_backoff.rs + tests/loopd_model_routing.rs
//! patterns:
//! - BEHAVIORAL tests that run the real `loopd.sh predicate` probe mode
//!   against fixture git repos in a temp dir (the script cd's to its own
//!   directory, so a copied script inside a crafted repo is a faithful
//!   harness; the probe launches nothing and writes nothing). The probe
//!   prints "<verb> <model> rows=… new=… deaths=… bookkeeping=… fresh=…
//!   base=… streak=…" — the gate's full decision surface.
//! - STATIC pins on loopd.sh's wiring: the split-assembly guard that keeps
//!   the T237 token at exactly one contiguous occurrence (the awk needle),
//!   the safe-side rule (an unknown baseline never reads as quiet), the
//!   valve arm ordering inside eval_gate, the run-loop integration (skip
//!   path, commit-failure fallthrough, glm re-route), and the death-needle
//!   coupling (the awk input reads the same verdict line the run loop
//!   writes).
//! - DOCTRINE pins: the LOOP-SPEC Phase-1 cheap-exit clause (predicate
//!   listed, who may write the disposition, the glm borderline exception)
//!   and the META-META-SPEC scope note (the full checklist applies only
//!   when the predicate is non-empty or the valve trips).
//!
//! T48 doctrine: files resolve from the checkout the binary RUNS against
//! (`std::env::current_dir()`), never the compile-time manifest-dir macro.

#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;
use tempfile::TempDir;

fn repo_root() -> PathBuf {
    std::env::current_dir().expect("cargo sets the test cwd to the package root")
}

fn read(rel: &str) -> String {
    std::fs::read_to_string(repo_root().join(rel))
        .unwrap_or_else(|e| panic!("reading {rel}: {e}"))
}

fn flat(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
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

/// 2026-09-27T12:00:00Z — the pinned "fresh" mtime and TODAY (the routing
/// test's constants, reused so the freshness semantics stay identical).
const FRESH_EPOCH: u64 = 1_790_510_400;
const FRESH_DAY: &str = "2026-09-27";
const STALE_DAY: &str = "2026-09-28";
/// Death-line timestamps, chosen far outside any plausible commit time so
/// the since-scope is deterministic: one years BEFORE any fixture baseline
/// (must never count) and one years AFTER (always counts).
const OLD_DEATH_TS: &str = "2000-01-01T00:00:00Z";
const FUTURE_DEATH_TS: &str = "2099-01-01T00:00:00Z";
const DEATH_HEAD: &str = "cycle ended WITHOUT goal complete";
/// The fixture log line shape: the run loop's verdict echo with a failure
/// count filled in.
const DEATH_LINE: &str = "cycle ended WITHOUT goal complete (consecutive failures: 1)";

/// A TODO.md with one DONE row — zero `todo` rows, so the T81 route is
/// eval mode and the gate is reached (the routing test's table shape).
const TODO_ONE_DONE: &str = "| id | title | spec | pri | status | notes |\n\
     |----|-------|------|-----|--------|-------|\n\
     | T1 | title | specs/T1-slug.md | 2 | done | notes |\n";
/// The same table plus a second DONE row — still eval mode, but the row id
/// is NEW against a baseline that only knew T1.
const TODO_TWO_DONE: &str = "| id | title | spec | pri | status | notes |\n\
     |----|-------|------|-----|--------|-------|\n\
     | T1 | title | specs/T1-slug.md | 2 | done | notes |\n\
     | T2 | title | specs/T2-slug.md | 2 | done | notes |\n";

/// Run one git command in `root` with a throwaway identity (the fixture
/// commits are content, not history worth signing).
fn git(root: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(["-c", "user.name=t258", "-c", "user.email=t258@example.test"])
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

/// A fixture git repo: a copy of the real loopd.sh (plus its T213 loader
/// fragment) inside a fresh repo whose first commit is the BASELINE — an
/// `eval:` commit adding EVALUATION.md (fresh-mtime pinned) and TODO.md.
/// The script cd's to its own directory, so the copy sees the fixture repo.
struct Fixture(TempDir);

fn fixture() -> Fixture {
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
    let f = Fixture(tmp);
    f.commit_tree(
        "eval: cycle-100 fresh eval — the baseline (T258 fixture)",
        Some(TODO_ONE_DONE),
        Some("# eval\n"),
    );
    f.pin_eval_mtime();
    f
}

impl Fixture {
    fn path(&self) -> &Path {
        self.0.path()
    }

    /// Commit the current tree (pass files first via `write_todo`/
    /// `write_eval`), or an empty commit when nothing changed.
    fn commit_tree(&self, subject: &str, todo: Option<&str>, eval: Option<&str>) {
        if let Some(todo) = todo {
            std::fs::write(self.path().join("TODO.md"), todo).expect("write fixture TODO.md");
        }
        if let Some(eval) = eval {
            std::fs::write(self.path().join("EVALUATION.md"), eval).expect("write fixture EVALUATION.md");
        }
        git(self.path(), &["add", "-A"]);
        let status = Command::new("git")
            .args(["-c", "user.name=t258", "-c", "user.email=t258@example.test"])
            .arg("-C")
            .arg(self.path())
            .args(["commit", "-q", "--allow-empty", "-m", subject])
            .output()
            .expect("spawn git commit");
        assert!(
            status.status.success(),
            "fixture commit failed: {}",
            String::from_utf8_lossy(&status.stderr)
        );
    }

    /// An empty commit (a wrap-notes disposition or landed-work marker).
    fn commit_empty(&self, subject: &str) {
        self.commit_tree(subject, None, None);
    }

    /// Append a line to the fixture's loopd.log (the death record the
    /// gate's deaths input reads).
    fn log_line(&self, line: &str) {
        let log = self.path().join(".chug/loopd/loopd.log");
        std::fs::create_dir_all(log.parent().unwrap()).expect("fixture log dir");
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log)
            .expect("open fixture loopd.log");
        writeln!(f, "{line}").expect("append fixture log line");
    }

    /// Pin EVALUATION.md's mtime to FRESH_EPOCH — deterministic regardless
    /// of when the suite runs (the routing test's helper, inlined).
    fn pin_eval_mtime(&self) {
        use std::fs::FileTimes;
        let path = self.path().join("EVALUATION.md");
        if !path.exists() {
            std::fs::write(&path, "# eval\n").expect("write fixture EVALUATION.md");
        }
        let f = std::fs::File::options()
            .write(true)
            .open(&path)
            .expect("open fixture EVALUATION.md for mtime pinning");
        f.set_times(FileTimes::new().set_modified(
            std::time::SystemTime::UNIX_EPOCH + Duration::from_secs(FRESH_EPOCH),
        ))
        .expect("pin fixture mtime");
    }
}

/// Run the copied script's `predicate` probe with the env CLEARED of the
/// routing knobs (and the git vars that could redirect the walk), then the
/// caller's overrides applied. Prints the gate's one decision line.
fn run_predicate(root: &Path, envs: &[(&str, &str)]) -> String {
    let mut cmd = Command::new("bash");
    cmd.arg(root.join("loopd.sh"))
        .arg("predicate")
        .current_dir(root)
        .env_remove("CHUG_ROUTINE_TODAY")
        .env_remove("LOOP_ORCH_MODEL")
        .env_remove("LOOP_ROUTINE_MODEL")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE");
    for (k, v) in envs {
        cmd.env(k, v);
    }
    let out = cmd.output().expect("spawn loopd.sh predicate");
    assert!(
        out.status.success(),
        "predicate probe exited {:?}: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// The gate's verb (field 1): `skip` or `launch`.
fn verb_of(gate: &str) -> &str {
    gate.split_whitespace().next().expect("gate line has a verb")
}

/// The gate's routed model (field 2): `-` on skip, glm or kimi on launch.
fn model_of(gate: &str) -> &str {
    gate.split_whitespace().nth(1).expect("gate line has a model")
}

// --- pin 1: the cheap exit skips the launch when inputs are empty -------------

/// The req-1 happy path: all four inputs quiet, streak 0 — the supervisor
/// skips the launch and writes the disposition itself. The routed model
/// field is `-`: no orchestrator is picked because none is needed.
#[test]
fn empty_predicate_skips_the_launch() {
    let f = fixture();
    let gate = run_predicate(f.path(), &[("CHUG_ROUTINE_TODAY", FRESH_DAY)]);
    assert_eq!(verb_of(&gate), "skip", "all four inputs quiet → the cheap exit: {gate}");
    assert_eq!(model_of(&gate), "-", "a skip picks no orchestrator model: {gate}");
    for field in ["rows=0", "new=0", "deaths=0", "bookkeeping=yes", "fresh=yes", "streak=0"] {
        assert!(
            gate.split_whitespace().any(|f| f == field),
            "the skip decision carries the input {field}: {gate}"
        );
    }
}

/// A bookkeeping-only delta after the baseline (an `eval:` disposition wrap
/// whose subject carries the T237 token) is still an EMPTY predicate — the
/// chain's own commits never dirty the gate. Streak 1 < 3, so no valve.
#[test]
fn a_bookkeeping_delta_after_the_baseline_still_skips() {
    let f = fixture();
    f.commit_empty("eval: cycle-101 wrap notes (empty-delta disposition, 1st consecutive no-op)");
    let gate = run_predicate(f.path(), &[("CHUG_ROUTINE_TODAY", FRESH_DAY)]);
    assert_eq!(
        verb_of(&gate), "skip",
        "an eval-bookkeeping delta (the T247 chain's own shape) stays empty: {gate}"
    );
    assert!(gate.contains("streak=1"), "the token-subject wrap feeds the T237 streak walk: {gate}");
}

// --- pin 2: any single non-empty input forces the launch ----------------------

/// Input 1 — new rows: an `eval:` commit filed T2 since the baseline (the
/// production shape: an eval that files a row and re-drains the queue).
/// Bookkeeping-only and fresh, so the non-empty input alone forces the
/// launch — and the borderline routing picks glm (req 2).
#[test]
fn new_rows_force_the_launch_on_glm() {
    let f = fixture();
    f.commit_tree(
        "eval: cycle-101 fresh eval — one row filed, queue re-drained",
        Some(TODO_TWO_DONE),
        None,
    );
    let gate = run_predicate(f.path(), &[("CHUG_ROUTINE_TODAY", FRESH_DAY)]);
    assert_eq!(verb_of(&gate), "launch", "new=1 forces the launch: {gate}");
    assert!(gate.contains("new=1"), "the input is visible in the decision: {gate}");
    assert_eq!(
        model_of(&gate), "glm",
        "borderline (non-empty only through new rows, bookkeeping-only, fresh) routes glm: {gate}"
    );
}

/// Input 2 — deaths: a cycle that ended WITHOUT goal complete after the
/// baseline. A bookkeeping-only fresh delta with a death is borderline →
/// glm — the launch is forced by the death alone.
#[test]
fn a_death_after_the_baseline_forces_the_launch_on_glm() {
    let f = fixture();
    f.log_line(&format!("{FUTURE_DEATH_TS} {DEATH_LINE}"));
    let gate = run_predicate(f.path(), &[("CHUG_ROUTINE_TODAY", FRESH_DAY)]);
    assert_eq!(verb_of(&gate), "launch", "deaths=1 forces the launch: {gate}");
    assert!(gate.contains("deaths=1"), "the input is visible in the decision: {gate}");
    assert_eq!(model_of(&gate), "glm", "a death-forced borderline eval routes glm: {gate}");
}

/// The deaths input is SCOPED to the baseline: a death recorded years
/// BEFORE the last evaluation is old news the baseline's cycle already
/// absorbed — the predicate stays empty and the cheap exit still fires.
#[test]
fn a_death_before_the_baseline_does_not_force_the_launch() {
    let f = fixture();
    f.log_line(&format!("{OLD_DEATH_TS} {DEATH_LINE}"));
    let gate = run_predicate(f.path(), &[("CHUG_ROUTINE_TODAY", FRESH_DAY)]);
    assert_eq!(
        verb_of(&gate), "skip",
        "a pre-baseline death is absorbed history, not a predicate input: {gate}"
    );
    assert!(gate.contains("deaths=0"), "the scoped count stays 0: {gate}");
}

/// Input 3 — changes: a non-`eval:` commit since the baseline (landed work)
/// dirties the delta. This is the REAL-eval shape (the corpus changed) —
/// kimi keeps it, the glm-never-evaluates boundary holds.
#[test]
fn a_work_delta_forces_the_launch_on_kimi() {
    let f = fixture();
    f.commit_empty("feat: T250 lands a real change (code commit, the delta goes dirty)");
    let gate = run_predicate(f.path(), &[("CHUG_ROUTINE_TODAY", FRESH_DAY)]);
    assert_eq!(verb_of(&gate), "launch", "a non-bookkeeping delta forces the launch: {gate}");
    assert!(
        gate.contains("bookkeeping=no"),
        "the delta input flips on a non-eval commit: {gate}"
    );
    assert_eq!(
        model_of(&gate), "kimi",
        "a work delta is a real eval — the fresh-evaluation boundary stays kimi: {gate}"
    );
}

/// Input 4 — freshness: a stale EVALUATION.md (mtime yesterday against a
/// pinned tomorrow) forces the launch even over an otherwise-empty delta —
/// the daily full evaluation, kimi by the T81 construction.
#[test]
fn a_stale_evaluation_forces_the_launch_on_kimi() {
    let f = fixture();
    let gate = run_predicate(f.path(), &[("CHUG_ROUTINE_TODAY", STALE_DAY)]);
    assert_eq!(verb_of(&gate), "launch", "fresh=no forces the launch: {gate}");
    assert!(gate.contains("fresh=no"), "the input is visible in the decision: {gate}");
    assert_eq!(
        model_of(&gate), "kimi",
        "the fresh evaluation itself stays kimi (T81 unchanged): {gate}"
    );
}

// --- the valve still trips -----------------------------------------------------

/// Req 1's valve clause: after 3 consecutive token-carrying dispositions the
/// gate LAUNCHES the real evaluation on kimi even though every predicate
/// input is still quiet — the chain converts itself into its own evaluation
/// at the trip point, exactly the T247 rule, now enforced before the skip.
#[test]
fn the_valve_trips_the_fourth_empty_cycle() {
    let f = fixture();
    for (n, cycle) in ["101", "102", "103"].iter().enumerate() {
        f.commit_empty(&format!(
            "eval: cycle-{cycle} wrap notes (empty-delta disposition, {} consecutive no-op)",
            n + 1
        ));
    }
    let gate = run_predicate(f.path(), &[("CHUG_ROUTINE_TODAY", FRESH_DAY)]);
    assert!(gate.contains("streak=3"), "three token wraps build the TRUE streak: {gate}");
    assert_eq!(
        verb_of(&gate), "launch",
        "the valve overrides the cheap exit at streak 3: {gate}"
    );
    assert_eq!(
        model_of(&gate), "kimi",
        "a valve trip is a REAL evaluation — kimi, never the cheap model: {gate}"
    );
}

/// The boundary below the trip: streak 2 (the 3rd consecutive empty cycle)
/// is still inside the chain — the cheap exit fires and the valve waits.
#[test]
fn streak_two_still_takes_the_cheap_exit() {
    let f = fixture();
    for (n, cycle) in ["101", "102"].iter().enumerate() {
        f.commit_empty(&format!(
            "eval: cycle-{cycle} wrap notes (empty-delta disposition, {} consecutive no-op)",
            n + 1
        ));
    }
    let gate = run_predicate(f.path(), &[("CHUG_ROUTINE_TODAY", FRESH_DAY)]);
    assert!(gate.contains("streak=2"), "two token wraps build streak 2: {gate}");
    assert_eq!(verb_of(&gate), "skip", "the 3rd empty cycle is still chain material: {gate}");
}

// --- the safe side: unknowns never read as quiet -------------------------------

/// The skip requires PROOF on all four inputs: in a non-git cwd (no
/// baseline, no record) the gate degrades to a launch on kimi — the gate
/// can only ever ADD launches over the T81 routing, never skip on unknowns.
#[test]
fn an_unknown_baseline_degrades_to_a_launch() {
    let tmp = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        tmp.path().join("loopd.sh"),
        read("loopd.sh").as_bytes(),
    )
    .expect("copy loopd.sh into a non-git fixture dir");
    let scripts = tmp.path().join("scripts");
    std::fs::create_dir_all(&scripts).expect("fixture scripts dir");
    std::fs::copy(
        repo_root().join("scripts/loopd_env_loader.sh"),
        scripts.join("loopd_env_loader.sh"),
    )
    .expect("copy loopd_env_loader.sh into fixture dir");
    std::fs::write(tmp.path().join("TODO.md"), TODO_ONE_DONE).expect("write fixture TODO.md");
    let gate = run_predicate(tmp.path(), &[("CHUG_ROUTINE_TODAY", FRESH_DAY)]);
    assert_eq!(
        verb_of(&gate), "launch",
        "no git record → no proof → the safe side is the launch: {gate}"
    );
    assert_eq!(model_of(&gate), "kimi", "an unknown delta is a real eval: {gate}");
    assert!(
        gate.contains("base=none"),
        "the unknown baseline is carried in the decision line, not hidden: {gate}"
    );
}

/// A delta read is fail-closed on the SUBJECT leg too: a commit whose
/// subject CONTAINS `eval:` but does not START with it (a mention, not a
/// bookkeeping commit) is non-bookkeeping — the `^eval:` anchor of the
/// prefix test is load-bearing.
#[test]
fn an_improperly_prefixed_subject_is_never_bookkeeping() {
    let f = fixture();
    f.commit_empty("re: eval: mentioning an eval commit is not an eval commit");
    let gate = run_predicate(f.path(), &[("CHUG_ROUTINE_TODAY", FRESH_DAY)]);
    assert_eq!(verb_of(&gate), "launch", "a mention-subject commit is landed work: {gate}");
    assert!(gate.contains("bookkeeping=no"), "the subject anchor flipped the delta: {gate}");
}

// --- pin 3: the disposition line carries the inputs ----------------------------

/// Req 3: the skip decision records the predicate inputs — row count,
/// last-change hash, freshness — in the line the supervisor logs. The probe
/// prints the same fields the run-loop disposition line embeds, so the
/// behavioral surface here pins both carriers' content.
#[test]
fn the_disposition_decision_carries_rows_hash_and_freshness() {
    let f = fixture();
    let gate = run_predicate(f.path(), &[("CHUG_ROUTINE_TODAY", FRESH_DAY)]);
    assert!(gate.contains("rows=0"), "the row count rides the line: {gate}");
    assert!(gate.contains("fresh=yes"), "the freshness rides the line: {gate}");
    let base = gate
        .split_whitespace()
        .find(|f| f.starts_with("base="))
        .expect("the line names the baseline")
        .trim_start_matches("base=")
        .to_string();
    assert_eq!(
        base.len(),
        40,
        "the last-change hash is the full baseline sha: {gate}"
    );
    assert!(
        base.chars().all(|c| c.is_ascii_hexdigit()),
        "the last-change hash is a git object id: {gate}"
    );
}

// --- static pins: the loopd.sh wiring ------------------------------------------

/// The T237 token stays at EXACTLY ONE contiguous occurrence in loopd.sh
/// (the awk needle — re-pinned here because the cheap-exit disposition
/// subject ALSO needs the token at runtime): the writer assembles it from
/// split halves, so a second literal can never mask a needle-removal
/// mutant, and the split-assembly seam itself is pinned.
#[test]
fn the_disposition_subject_token_is_assembled_not_duplicated() {
    let loopd = read("loopd.sh");
    count_eq(
        &loopd,
        "empty-delta disposition",
        1,
        "the awk needle stays the ONE contiguous token in loopd.sh (the T237 \
         count pin's invariant, guarded by the split assembly)",
    );
    count_eq(
        &loopd,
        "local t1=\"empty-delta\" t2=\"disposition\"",
        1,
        "the split-assembly seam: the writer builds the token from halves"
    );
    let writer_at = loopd
        .find("loopd_write_disposition() {")
        .expect("the disposition writer is defined");
    let body_end = loopd[writer_at..]
        .find("\n}")
        .map(|i| writer_at + i)
        .expect("the writer closes");
    let body = &loopd[writer_at..=body_end];
    assert!(
        body.contains("${t1} ${t2}"),
        "the runtime subject embeds the assembled token (the streak walk and \
         the valve must count loopd's own disposition): {body}"
    );
    assert!(
        body.contains("TRUE streak ${streak}→${snext}"),
        "the runtime subject carries the TRUE streak handoff (the trip \
         human-counts the wrap-notes chain loopd now extends)"
    );
    assert!(
        body.contains("git commit --allow-empty"),
        "the disposition is an empty commit — loopd touches no eval-domain file"
    );
}

/// The deaths input reads the SAME verdict line the run loop writes — the
/// failure echo is the one writer and the death input's two awk needles
/// (the scoped reader and the unknown-window reader) are the readers: three
/// occurrences of the one literal, so a reworded verdict silently ungrounds
/// the death input.
#[test]
fn the_death_input_reads_the_run_loops_own_verdict_line() {
    count_eq(
        &read("loopd.sh"),
        DEATH_HEAD,
        3,
        "the failure verdict line (the run loop's echo, 1) + the death \
         input's two awk needles (the since-scoped reader + the \
         unknown-window reader) — one literal, three carriers"
    );
}

/// The gate's skip requires positive proof: the unknown-baseline refusal is
/// wired (an empty base never reads as quiet), and the valve arm precedes
/// the skip arm inside eval_gate (the trip converts the 4th empty cycle
/// whatever the delta says).
#[test]
fn the_gate_orders_the_valve_before_the_skip_and_refuses_unknown_baselines() {
    let loopd = read("loopd.sh");
    count_eq(
        &loopd,
        "if [ -z \"$base\" ]; then return 1; fi",
        1,
        "an unknown baseline is never bookkeeping — the safe-side refusal"
    );
    let gate_at = loopd.find("eval_gate() {").expect("eval_gate is defined");
    let gate_end = loopd[gate_at..]
        .find("\n}")
        .map(|i| gate_at + i)
        .expect("eval_gate closes");
    let body = &loopd[gate_at..=gate_end];
    let valve = body
        .find("[ \"$streak\" -ge 3 ]")
        .expect("the valve arm is wired");
    let skip = body
        .find("verb=skip")
        .expect("the skip arm is wired");
    assert!(
        valve < skip,
        "the valve arm must precede the skip arm — the 4th consecutive empty \
         cycle runs the real evaluation even on a quiet delta (req 1)"
    );
    count_eq(
        &loopd,
        "rows=$(todo_rows TODO.md) new=$new deaths=$deaths bookkeeping=$book fresh=$fresh base=${base:-none} streak=$streak",
        1,
        "the gate's field echo — the single format the run-loop line and the \
         probe share (req 3)"
    );
}

/// The run-loop integration: the gate runs in eval mode only, the skip path
/// logs the disposition line, paces off the SAME T237 function, and never
/// launches; the commit-failure fallthrough re-launches kimi (a skip whose
/// git record never landed would be invisible to the streak walk and the
/// valve); the glm re-route is logged and feeds the ONE launch site.
#[test]
fn the_run_loop_integrates_the_gate_before_any_launch() {
    let loopd = read("loopd.sh");
    count_eq(
        &loopd,
        "gate=\"$(eval_gate)\"",
        1,
        "the run loop consults the gate exactly once (eval mode only)"
    );
    count_eq(
        &loopd,
        "cheap-exit disposition (T258):",
        1,
        "the skip path's disposition log line"
    );
    count_eq(
        &loopd,
        "cheap-exit pacing: sleeping ${skip_secs}s (empty streak $(empty_wrap_streak))",
        1,
        "the skip path paces off the T237 function (no flat sleep, no second \
         ok_secs call site)"
    );
    count_eq(
        &loopd,
        "cheap-exit disposition commit FAILED",
        1,
        "the commit-failure fallthrough is logged (the safe side: the real \
         eval launches)"
    );
    count_eq(
        &loopd,
        "eval gate (T258):",
        1,
        "the glm re-route is explained in the log (the T50 model-change rule)"
    );
    count_eq(
        &loopd,
        "orch_model=$gate_model",
        1,
        "the re-routed model feeds the existing launch site (no second \
         --model flag)"
    );
    // The skip path must end in `continue` — no cycle log, no launch.
    let skip_at = loopd
        .find("cheap-exit disposition (T258):")
        .expect("the skip block is present");
    let window = &loopd[skip_at..skip_at + 900];
    let cont = window
        .find("\n        continue")
        .expect("the skip path continues the run loop without launching");
    let sleep = window
        .find("sleep \"$skip_secs\"")
        .expect("the skip path sleeps the paced value");
    assert!(
        sleep < cont,
        "the skip path sleeps THEN continues — no cycle log, no chug launch"
    );
    count_eq(
        &loopd,
        "predicate)",
        1,
        "the probe subcommand arm exists"
    );
    count_eq(
        &loopd,
        "usage: loopd.sh [run|stop|status|routing] [sleep-ok] [predicate]",
        1,
        "the usage line names the probe (keeping the T237 needle a prefix)"
    );
}

// --- static pins: the doctrine carriers -----------------------------------------

/// The LOOP-SPEC Phase-1 cheap-exit clause: the predicate listed verbatim,
/// who may write the disposition, any-single-input forces the launch, the
/// valve clause, the glm borderline exception, and the inputs-record rule.
#[test]
fn loop_spec_phase_1_carries_the_cheap_exit_clause() {
    let spec = flat(&read("LOOP-SPEC.md"));
    for needle in [
        "The cheap exit (T258).",
        "no new TODO rows since the last evaluation",
        "no child deaths since the last evaluation",
        "bookkeeping-only delta since the last evaluation",
        "EVALUATION.md fresh (same UTC day)",
        "writes the one-line disposition itself",
        "Who may write the disposition: the supervisor, mechanically, on an empty predicate (this rule), or the orchestrator under the chain rule above — never a third path.",
        "Any single non-empty input forces the launch instead",
        "the valve still converts the 4th consecutive empty cycle into the real evaluation",
        "is borderline and launches `LOOP_ROUTINE_MODEL` (glm) for a bounded evaluation",
        "any non-bookkeeping delta (source, spec, or doctrine work landed), a stale EVALUATION.md (the fresh evaluation itself), every valve trip, and all validation (family independence unchanged)",
        "records the predicate inputs (row count, last-change hash, freshness) in loopd.log",
    ] {
        count_eq(&spec, needle, 1, "the LOOP-SPEC Phase-1 cheap-exit clause");
    }
    // Quote discipline (T248): the contiguous token stays a Phase-3-only
    // carrier — the cheap-exit clause writes around it. RAW text, like the
    // T237 pin: the invariant is a raw-byte property (line-wrapped mentions
    // are legal write-arounds and collapse into matches only under flat()).
    count_eq(
        &read("LOOP-SPEC.md"),
        "empty-delta disposition",
        1,
        "the contiguous token stays exactly once in LOOP-SPEC.md RAW (the T237 \
         count pin; the cheap-exit clause says \"the T237 token\", never the \
         literal)"
    );
}

/// The clause sits INSIDE Phase 1 — after the T247 chain paragraph (the
/// human-counted TRUE-streak rule) and before the commit-artifacts line —
/// extending the T247 placement pin's ordering.
#[test]
fn the_cheap_exit_clause_sits_between_the_chain_rule_and_the_commit_artifacts() {
    let spec = flat(&read("LOOP-SPEC.md"));
    let chain_tail = spec
        .find("but the trip threshold reads the TRUE count only.")
        .expect("the T247 chain paragraph's close (deleted or reworded?)");
    let cheap = spec
        .find("The cheap exit (T258).")
        .expect("the cheap-exit clause (deleted?)");
    let commit = spec
        .find("Commit the evaluation artifacts (`eval: ...`) before dispatching.")
        .expect("the commit-artifacts line (deleted?)");
    assert!(
        chain_tail < cheap && cheap < commit,
        "the cheap exit extends the chain rule and stays inside Phase 1 \
         (before the commit-artifacts line)"
    );
}

/// The META-META-SPEC scope note (req 4): the full checklist applies only
/// when the predicate is non-empty or the valve trips — a quiet cycle never
/// reaches the eval spec at all.
#[test]
fn meta_meta_spec_scopes_the_full_checklist() {
    let spec = flat(&read("META-META-SPEC.md"));
    for needle in [
        "When the full checklist applies (T258).",
        "skips the launch entirely when the mechanical disposition predicate is empty",
        "The full checklist below applies only when that predicate is non-empty or the T247 valve trips the real evaluation.",
        "re-runs this checklist on glm against an unchanged corpus",
        "source/spec/doctrine deltas and stale evaluations stay kimi (T81).",
    ] {
        count_eq(&spec, needle, 1, "the META-META-SPEC scope note");
    }
}
