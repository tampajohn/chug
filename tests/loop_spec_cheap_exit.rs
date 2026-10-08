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
//! T258 FIX-UP (the validator's VERDICT: FAIL, three findings):
//! - F1 — the gate's launch arms resolve the FULL provider-routed model ids
//!   (`$LOOP_ROUTINE_MODEL` for the borderline arm, `$LOOP_ORCH_MODEL` for
//!   the kimi arms). Model ids are passed VERBATIM into the API request body
//!   (src/api.rs `body.insert("model", ...)`); there is no alias layer, so a
//!   bare `glm`/`kimi` shorthand in a launch arm dies at the first call, and
//!   the T81 rollback knob (`LOOP_ROUTINE_MODEL=…kimi-k3`) would be silently
//!   broken. No bare shorthand may reach a launch site in the gate path —
//!   including the run-loop re-route comparison.
//! - F2 — LOOP-SPEC's T247 paragraph scopes BOTH count authorities: the
//!   supervisor's gate/valve read the machine streak (machine == TRUE on a
//!   disciplined chain); the HUMAN-counted rule stays for orchestrator-side
//!   chain adjudication and contested divergences (the cycle-118 class).
//! - F3 — (a) the launched-model pin of F1, (b) loopd_write_disposition
//!   driven END-TO-END (the real `run` loop, one bounded pass: the empty
//!   commit lands, the subject carries the T237 token + TRUE-streak
//!   handoff, the streak increments), (c) the death-line same-second
//!   boundary (>= deliberately counts) pinned, (d) a MISSING loopd.log
//!   reads safe-side (`unknown` → launch), never as a quiet `0`.
//!
//! T48 doctrine: files resolve from the checkout the binary RUNS against
//! (`std::env::current_dir()`), never the compile-time manifest-dir macro.

#![cfg(unix)]

use std::io::{Read, Write};
use std::os::unix::net::UnixListener;
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
/// The FULL provider-routed model ids — loopd.sh's T81 defaults (the gate's
/// launch arms resolve these env vars; there is no alias layer, src/api.rs
/// passes the id verbatim into the request body). The static
/// `the_gate_launch_arms_resolve_full_model_ids_never_shorthands` pin keeps
/// these constants in sync with loopd.sh's default lines.
const GLM_ID: &str = "anthropic-system.ai.glm-5-3-flash";
const KIMI_ID: &str = "anthropic-system.ai.kimi-k3";
const DEATH_HEAD: &str = "cycle ended WITHOUT goal complete";
/// The fixture log line shape: the run loop's verdict echo with a failure
/// count filled in.
const DEATH_LINE: &str = "cycle ended WITHOUT goal complete (consecutive failures: 1)";

/// Run one git command in `root` and return its trimmed stdout (the
/// subject/epoch probes the disposition and boundary pins read back).
fn git_out(root: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
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
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// Write an executable stub script (the run-mode leg's fake `ps` and fake
/// `cargo` — host-facing probes stubbed at the PATH layer, where loopd
/// resolves every command).
fn write_exec(path: &Path, body: &str) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::write(path, body).expect("write fixture stub");
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .expect("chmod fixture stub");
}

/// Epoch → "%Y-%m-%dT%H:%M:%S" UTC, derived EXACTLY the way cycle_deaths
/// derives the death window's `since` (the BSD `-r` form first, then the GNU
/// `-d @` form) — the boundary pins compare against the same string the
/// production awk compares against.
fn utc_stamp(epoch: &str) -> String {
    let gnu = format!("@{epoch}");
    for argv in [
        vec!["-u", "-r", epoch, "+%Y-%m-%dT%H:%M:%S"],
        vec!["-u", "-d", &gnu, "+%Y-%m-%dT%H:%M:%S"],
    ] {
        let stamp = Command::new("date")
            .args(&argv)
            .output()
            .ok()
            .filter(|out| out.status.success())
            .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string());
        if let Some(stamp) = stamp {
            return stamp;
        }
    }
    panic!("no UTC conversion for epoch {epoch} (BSD -r and GNU -d both failed)");
}

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
    // The supervisor's state dir must never dirty the gate's delta walk: a
    // COMMITTED loopd.log (or cycle log) is a non-bookkeeping file and would
    // flip bookkeeping=no on every later fixture commit. Same ignore set the
    // real repo carries for .chug/ (plus the dirs the run-mode leg creates).
    std::fs::write(tmp.path().join(".gitignore"), "/.chug/\n/target-shared/\n/bin/\n")
        .expect("write fixture .gitignore");
    git(tmp.path(), &["init", "-q"]);
    let f = Fixture(tmp);
    f.commit_tree(
        "eval: cycle-100 fresh eval — the baseline (T258 fixture)",
        Some(TODO_ONE_DONE),
        Some("# eval\n"),
    );
    f.pin_eval_mtime();
    // F3d: a QUIET fixture must PROVE quiet — the log exists and records no
    // death, so `deaths=0` is evidence of absence. A missing log is the
    // unknown record the safe-side rule launches on (the missing-log pin
    // removes this file).
    f.log_line("2026-09-27T12:00:00Z loopd start (pid 4242) — T258 fixture baseline record");
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
///
/// T259 fix-up (F1, the hermeticity class): since the triage moved inside
/// eval_gate, the probe consults the judge daemon's socket — resolved
/// $CHUG_DAEMON_SOCK, then $CHUG_HOME, then $HOME/.chug/daemon.sock. A probe
/// run must NEVER reach the host's LIVE daemon (an operator answer — e.g.
/// needs-eval=yes conf=0.0103 -> route=kimi — would re-route the borderline
/// arms and flip these pins green-by-load under full-parallel contention,
/// failing deterministically under --test-threads=1), so the runner scrubs
/// the daemon vars AFTER the caller's overrides (no outer-env value AND no
/// caller override may leak a socket back in — the leak legs the hermeticity
/// pin below binds live fixture daemons on) and pins HOME to the fixture
/// root — the same hermetic shape the T259 harness achieves with its
/// explicit fixture sockets. The fixture HOME also kills the
/// $HOME/.chug/loopd.env env-file load.
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
    // T259 fix-up (F1), after the overrides: the daemon vars die here — the
    // triage consults its socket resolved $CHUG_DAEMON_SOCK, $CHUG_HOME,
    // $HOME/.chug/daemon.sock, and a probe run must resolve that INSIDE the
    // fixture or nowhere.
    cmd.env_remove("CHUG_DAEMON_SOCK").env_remove("CHUG_HOME");
    // Hermetic HOME, last: the daemon socket resolves $HOME/.chug/daemon.sock
    // — inside the fixture, where nothing listens unless a test binds its own
    // (see the_probe_runner_never_consults_a_daemon_outside_the_fixture).
    cmd.env("HOME", root);
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

/// The gate's routed model (field 2): `-` on skip, the FULL provider-routed
/// orchestrator id on launch — `$LOOP_ROUTINE_MODEL` for the borderline arm,
/// `$LOOP_ORCH_MODEL` for the kimi arms, never a bare shorthand (F1).
fn model_of(gate: &str) -> &str {
    gate.split_whitespace().nth(1).expect("gate line has a model")
}

/// The value of one `<key>=` field in the gate line (empty when absent) —
/// the triage fields (laya/conf/route) ride the line the T259 harness reads.
fn field_of(gate: &str, key: &str) -> String {
    gate.split_whitespace()
        .find(|f| f.starts_with(key))
        .map(|f| f[key.len()..].to_string())
        .unwrap_or_default()
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
        model_of(&gate), GLM_ID,
        "borderline (non-empty only through new rows, bookkeeping-only, fresh) routes the FULL routine-model id — $LOOP_ROUTINE_MODEL, never the bare shorthand (F1): {gate}"
    );
}

/// Input 2 — deaths: a cycle that ended WITHOUT goal complete after the
/// baseline. A bookkeeping-only fresh delta with a death is borderline →
/// the routine model — the launch is forced by the death alone. Second leg
/// (F1): the T81 rollback knob rides along — with LOOP_ROUTINE_MODEL set to
/// the kimi id, the death-forced borderline launches kimi (single-model
/// operation preserved), never a hardcoded shorthand.
#[test]
fn a_death_after_the_baseline_forces_the_launch_on_glm() {
    let f = fixture();
    f.log_line(&format!("{FUTURE_DEATH_TS} {DEATH_LINE}"));
    let gate = run_predicate(f.path(), &[("CHUG_ROUTINE_TODAY", FRESH_DAY)]);
    assert_eq!(verb_of(&gate), "launch", "deaths=1 forces the launch: {gate}");
    assert!(gate.contains("deaths=1"), "the input is visible in the decision: {gate}");
    assert_eq!(
        model_of(&gate), GLM_ID,
        "a death-forced borderline eval routes the FULL routine-model id (F1): {gate}"
    );
    let gate = run_predicate(
        f.path(),
        &[("CHUG_ROUTINE_TODAY", FRESH_DAY), ("LOOP_ROUTINE_MODEL", KIMI_ID)],
    );
    assert_eq!(
        model_of(&gate), KIMI_ID,
        "the T81 rollback knob routes the death-forced borderline to kimi too: {gate}"
    );
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

/// (F3c) The death-line boundary is deliberately INCLUSIVE: a death stamped
/// in the baseline commit's OWN second reads `>=` the since and COUNTS — the
/// dangerous direction is the false-negative skip over a real death, never
/// the phantom launch. The `since` is derived exactly the way cycle_deaths
/// derives it (the baseline commit's epoch, UTC-stamped), so this pins the
/// production comparison, not a test-local re-derivation.
#[test]
fn a_death_in_the_baselines_own_second_counts_the_boundary_is_inclusive() {
    let f = fixture();
    let epoch = git_out(f.path(), &["log", "-1", "--format=%ct", "--", "EVALUATION.md"]);
    let since = utc_stamp(&epoch);
    f.log_line(&format!("{since}Z {DEATH_LINE}"));
    let gate = run_predicate(f.path(), &[("CHUG_ROUTINE_TODAY", FRESH_DAY)]);
    assert_eq!(
        verb_of(&gate), "launch",
        "a death in the baseline's own second is >= the since and counts — the boundary excludes nothing: {gate}"
    );
    assert!(gate.contains("deaths=1"), "the boundary death is counted: {gate}");
}

/// (F3c) The complement: one second BEFORE the baseline is strictly outside
/// the window — the scoping stays exact. A boundary that swallowed the
/// second before would pace every quiet cycle like a death cycle; a boundary
/// that swallowed the equal second would skip over a real death.
#[test]
fn a_death_one_second_before_the_baseline_does_not_count() {
    let f = fixture();
    let epoch = git_out(f.path(), &["log", "-1", "--format=%ct", "--", "EVALUATION.md"]);
    let before = utc_stamp(&(epoch.parse::<u64>().expect("epoch is numeric") - 1).to_string());
    f.log_line(&format!("{before}Z {DEATH_LINE}"));
    let gate = run_predicate(f.path(), &[("CHUG_ROUTINE_TODAY", FRESH_DAY)]);
    assert_eq!(
        verb_of(&gate), "skip",
        "one second before the baseline is strictly outside the window: {gate}"
    );
    assert!(gate.contains("deaths=0"), "the pre-window death stays uncounted: {gate}");
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
        model_of(&gate), KIMI_ID,
        "a work delta is a real eval — the fresh-evaluation boundary stays on the FULL orchestrator-model id (F1): {gate}"
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
        model_of(&gate), KIMI_ID,
        "the fresh evaluation itself stays on the orchestrator model (T81 unchanged, full id per F1): {gate}"
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
        model_of(&gate), KIMI_ID,
        "a valve trip is a REAL evaluation — the FULL orchestrator-model id, never the cheap model or a shorthand (F1): {gate}"
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

// --- the launched model ids: full provider-routed ids, never a shorthand -------

/// (F1/3a) The borderline re-route launches the FULL provider-routed id —
/// `$LOOP_ROUTINE_MODEL`, exactly as LOOP-SPEC's sentence says — never the
/// bare shorthand `glm` (which is not a model id anywhere: src/api.rs passes
/// the model verbatim into the request body, there is no alias layer, so the
/// shorthand dies at the first API call and every recorded death then forces
/// further glm launches until the 3-strike HALT). Two legs: the default and
/// the T81 rollback knob — `LOOP_ROUTINE_MODEL=…kimi-k3` must route the
/// borderline eval to kimi too (single-model operation preserved), which the
/// old hardcoded `glm` silently broke.
#[test]
fn the_borderline_reroute_launches_the_full_routine_model_id() {
    let f = fixture();
    f.commit_tree(
        "eval: cycle-101 fresh eval — one row filed, queue re-drained",
        Some(TODO_TWO_DONE),
        None,
    );
    let gate = run_predicate(f.path(), &[("CHUG_ROUTINE_TODAY", FRESH_DAY)]);
    assert_eq!(verb_of(&gate), "launch", "the borderline predicate launches: {gate}");
    assert_eq!(
        model_of(&gate), GLM_ID,
        "the borderline arm resolves $LOOP_ROUTINE_MODEL (the full id), not the shorthand: {gate}"
    );
    let gate = run_predicate(
        f.path(),
        &[("CHUG_ROUTINE_TODAY", FRESH_DAY), ("LOOP_ROUTINE_MODEL", KIMI_ID)],
    );
    assert_eq!(
        model_of(&gate), KIMI_ID,
        "the T81 rollback knob routes the borderline eval to kimi — single-model operation preserved: {gate}"
    );
}

/// (F1 sweep) The kimi arms resolve `$LOOP_ORCH_MODEL` — the full id, never
/// a literal `kimi` — so an operator override of the orchestrator model
/// reaches the launch verbatim on both kimi arms (the valve trip and the
/// work-delta/stale arm).
#[test]
fn the_kimi_arms_launch_the_full_orchestrator_model_id() {
    let override_id = "anthropic-system.ai.kimi-k3-t258-pin";
    let f = fixture();
    for (n, cycle) in ["101", "102", "103"].iter().enumerate() {
        f.commit_empty(&format!(
            "eval: cycle-{cycle} wrap notes (empty-delta disposition, {} consecutive no-op)",
            n + 1
        ));
    }
    let gate = run_predicate(
        f.path(),
        &[("CHUG_ROUTINE_TODAY", FRESH_DAY), ("LOOP_ORCH_MODEL", override_id)],
    );
    assert!(gate.contains("streak=3"), "the valve is tripped: {gate}");
    assert_eq!(
        model_of(&gate), override_id,
        "the valve arm resolves $LOOP_ORCH_MODEL verbatim: {gate}"
    );
    let f = fixture();
    f.commit_empty("feat: T250 lands a real change (code commit, the delta goes dirty)");
    let gate = run_predicate(
        f.path(),
        &[("CHUG_ROUTINE_TODAY", FRESH_DAY), ("LOOP_ORCH_MODEL", override_id)],
    );
    assert!(gate.contains("bookkeeping=no"), "the delta is dirty: {gate}");
    assert_eq!(
        model_of(&gate), override_id,
        "the work-delta arm resolves $LOOP_ORCH_MODEL verbatim: {gate}"
    );
}

// --- the safe side: unknowns never read as quiet -------------------------------

/// (F3d) A MISSING loopd.log is an UNKNOWN record, never a quiet one: the
/// old `echo 0` was the one exception to the gate's own
/// every-unknown-degrades-to-launch rule — a removed or never-written log
/// would read as "no deaths ever" and cheap-exit over a real death the
/// record cannot show. The safe side is the launch, on the orchestrator
/// model: an unknown count may hide a real death, so it is not borderline
/// either (the cheap model never runs on an unknown).
#[test]
fn a_missing_loopd_log_is_never_quiet() {
    let f = fixture();
    std::fs::remove_file(f.path().join(".chug/loopd/loopd.log")).expect("remove the fixture log");
    let gate = run_predicate(f.path(), &[("CHUG_ROUTINE_TODAY", FRESH_DAY)]);
    assert_eq!(
        verb_of(&gate), "launch",
        "an unreadable record cannot prove quiet — the safe side is the launch: {gate}"
    );
    assert!(
        gate.contains("deaths=unknown"),
        "the unknown is carried in the decision line, not hidden behind a 0: {gate}"
    );
    assert_eq!(
        model_of(&gate), KIMI_ID,
        "an unknown deaths count is not borderline — the full model: {gate}"
    );
}

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
    assert_eq!(
        model_of(&gate), KIMI_ID,
        "an unknown delta is a real eval on the FULL orchestrator-model id (F1): {gate}"
    );
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

// --- T259 fix-up F1: the probe runner is hermetic against the live daemon ------

/// Serve ONE canned /judge answer on `sock` until the process exits — the
/// T259 fixture-daemon shape (a unix-socket HTTP server), inlined: the test
/// guards the probe runner's hermeticity, not the daemon protocol. The
/// canned answer is the live host daemon's deterministic borderline verdict
/// (needs-eval=yes, confidence 0.0103 -> route=kimi) — the exact answer that
/// flipped the T258 borderline pins before the scrub.
fn serve_fixture_judge(sock: &Path) {
    let _ = std::fs::remove_file(sock);
    let listener = UnixListener::bind(sock).expect("bind the fixture judge socket");
    let body =
        "{\"answers\":{\"needs_eval\":{\"choice\":\"yes\",\"confidence\":0.0103}}}".to_string();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut stream = stream;
            let mut buf = Vec::new();
            let mut chunk = [0u8; 16384];
            loop {
                match stream.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => buf.extend_from_slice(&chunk[..n]),
                }
                let text = String::from_utf8_lossy(&buf).to_string();
                let Some(head_end) = text.find("\r\n\r\n") else { continue };
                let want = text
                    .lines()
                    .find_map(|l| {
                        l.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .map(|v| v.trim().parse::<usize>().ok())
                    })
                    .flatten()
                    .unwrap_or(0);
                if buf.len() >= head_end + 4 + want {
                    break;
                }
            }
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(resp.as_bytes());
            let _ = stream.flush();
        }
    });
}

/// THE hermeticity pin (T259 fix-up F1, the class): the probe runner must
/// NEVER let a loopd.sh path reach a daemon the test did not place inside
/// the fixture. Three leak legs, each killed by the runner's scrub: an
/// explicit $CHUG_DAEMON_SOCK, $CHUG_HOME, and a hostile $HOME (all bound
/// with LIVE fixture daemons answering the deterministic live-host verdict —
/// needs-eval=yes conf=0.0103 -> route=kimi). With the scrub the probe
/// resolves <fixture>/.chug/daemon.sock, finds nothing, fails open, and the
/// T258 borderline routing stands (glm, laya=down). Remove ANY scrub leg and
/// this pin fails deterministically — under --test-threads=1 AND
/// full-parallel — the green-by-load failure the live host daemon produced
/// before the fix (the three borderline pins' RED).
#[test]
fn the_probe_runner_never_consults_a_daemon_outside_the_fixture() {
    let f = fixture();
    // The BORDERLINE shape (the T259 target case): a new row over a
    // bookkeeping-only delta with a fresh evaluation — the arm that consults
    // the triage layer.
    f.commit_tree(
        "eval: cycle-101 fresh eval — one row filed, queue re-drained",
        Some(TODO_TWO_DONE),
        None,
    );
    // The hostile HOME + CHUG_HOME dirs first — the fixture daemons bind
    // inside them.
    let evil_home = f.path().join("evil-home");
    std::fs::create_dir_all(evil_home.join(".chug")).expect("evil HOME dir");
    let evil_chug_home = f.path().join("evil-chug-home");
    std::fs::create_dir_all(&evil_chug_home).expect("evil CHUG_HOME dir");
    // Leg 1: an explicit daemon socket the caller passes in (the runner must
    // scrub CHUG_DAEMON_SOCK).
    let evil_sock = f.path().join("evil-home/sock.sock");
    serve_fixture_judge(&evil_sock);
    // Leg 2: a CHUG_HOME-scoped socket (the runner must scrub CHUG_HOME).
    serve_fixture_judge(&evil_chug_home.join("daemon.sock"));
    // Leg 3: a hostile HOME (the runner must pin HOME to the fixture root).
    serve_fixture_judge(&evil_home.join(".chug/daemon.sock"));

    let gate = run_predicate(
        f.path(),
        &[
            ("CHUG_ROUTINE_TODAY", FRESH_DAY),
            ("CHUG_DAEMON_SOCK", evil_sock.to_string_lossy().as_ref()),
            ("CHUG_HOME", evil_chug_home.to_string_lossy().as_ref()),
            ("HOME", evil_home.to_string_lossy().as_ref()),
        ],
    );
    assert_eq!(
        verb_of(&gate),
        "launch",
        "no outside daemon was consulted, so the borderline arm keeps its T258 routing: {gate}"
    );
    assert_eq!(
        model_of(&gate),
        GLM_ID,
        "the borderline eval routes glm — never the kimi id a consulted daemon would answer: {gate}"
    );
    assert_eq!(
        field_of(&gate, "laya="),
        "down",
        "the probe saw NO daemon (fail-open at the fixture-local resolution) — a live \
         verdict here would mean an outside socket was consulted: {gate}"
    );
}

// --- F3b: the skip path end-to-end — the supervisor writes the disposition ----

/// (F3b) loopd_write_disposition driven END-TO-END through the real `run`
/// loop: one bounded main-loop pass (the T254 knob) in a fixture with the
/// two host-facing probes stubbed at the PATH layer — a fake `ps` reporting
/// no driver (the single-driver probe reads PATH like any command, and the
/// host's real ps could show a live production driver) and a fake `cargo`
/// passing the build gate instantly — plus a hermetic HOME (kills the
/// env-file load, the installed-binary daemon probe, and the default daemon
/// socket) and a redirected daemon socket (no real-registry POSTs). Asserts
/// the disposition commit LANDS (an EMPTY commit — loopd touches no
/// eval-domain file), its subject carries the T237 token + the TRUE-streak
/// handoff + the predicate inputs, the streak increments (0→1, then a second
/// pass 1→2), and no chug launch happened (no `cycle start` line, no cycle
/// log). A runtime breakage here used to degrade silently: the commit fails,
/// the fallthrough launches a real kimi eval, and nothing in the suite
/// noticed.
#[test]
fn the_skip_path_writes_the_disposition_itself_end_to_end() {
    let f = fixture();
    let bin = f.path().join("bin");
    std::fs::create_dir_all(&bin).expect("fixture stub bin dir");
    write_exec(
        &bin.join("ps"),
        "#!/bin/sh\necho \"fake ps (T258 fixture): no chug drivers here\"\n",
    );
    write_exec(&bin.join("cargo"), "#!/bin/sh\nexit 0\n");
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".to_string())
    );
    let sock = f.path().join(".chug/no-daemon.sock");
    let run_one_pass = || {
        Command::new("bash")
            .arg(f.path().join("loopd.sh"))
            .arg("run")
            .current_dir(f.path())
            .env("PATH", &path)
            .env("HOME", f.path())
            .env("CHUG_ROUTINE_TODAY", FRESH_DAY)
            .env("CHUG_DAEMON_SOCK", &sock)
            .env("LOOPD_SLEEP_OK", "1")
            .env("LOOPD_SLEEP_FAIL", "1")
            .env("LOOPD_MAX_LOOPS", "1")
            .env("LOOP_DAEMON_ENSURE", "0")
            .env("LOOP_ORCH_MODEL", KIMI_ID)
            .env("LOOP_ROUTINE_MODEL", GLM_ID)
            .env("GIT_AUTHOR_NAME", "t258-fixture")
            .env("GIT_AUTHOR_EMAIL", "t258@example.test")
            .env("GIT_COMMITTER_NAME", "t258-fixture")
            .env("GIT_COMMITTER_EMAIL", "t258@example.test")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .output()
            .expect("spawn loopd.sh run")
    };
    let out = run_one_pass();
    assert!(
        out.status.success(),
        "one bounded pass exits clean: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let log = std::fs::read_to_string(f.path().join(".chug/loopd/loopd.log"))
        .expect("read the fixture loopd.log");
    assert!(
        log.contains("cheap-exit disposition (T258):"),
        "the skip path logged the disposition decision: {log}"
    );
    assert!(
        log.contains("cheap-exit pacing: sleeping 1s (empty streak 1)"),
        "the skip path paced off the T237 streak (post-write read): {log}"
    );
    assert!(
        !log.contains("cycle start ->"),
        "no chug launch on the skip path: {log}"
    );
    assert!(
        !log.contains("commit FAILED"),
        "the disposition commit landed — no fallthrough to a kimi eval: {log}"
    );
    let subject = git_out(f.path(), &["log", "-1", "--format=%s"]);
    assert!(
        subject.contains("empty-delta disposition"),
        "the T237 token rides the subject (the streak walk and the valve count it): {subject}"
    );
    assert!(
        subject.contains("TRUE streak 0→1"),
        "the TRUE-streak handoff names the increment: {subject}"
    );
    for field in ["rows=0", "deaths=0", "bookkeeping=yes", "fresh=yes", "streak=0"] {
        assert!(
            subject.contains(field),
            "the predicate inputs ride the subject ({field}): {subject}"
        );
    }
    let files = git_out(f.path(), &["show", "--format=", "--name-only", "HEAD"]);
    assert!(
        files.is_empty(),
        "the disposition is an EMPTY commit — loopd touches no eval-domain file: {files:?}"
    );
    let probe = Command::new("bash")
        .arg(f.path().join("loopd.sh"))
        .arg("sleep-ok")
        .current_dir(f.path())
        .env("CHUG_ROUTINE_TODAY", FRESH_DAY)
        .env_remove("LOOPD_SLEEP_OK")
        .output()
        .expect("spawn sleep-ok probe");
    let line = String::from_utf8_lossy(&probe.stdout);
    assert_eq!(
        line.split_whitespace().nth(1),
        Some("1"),
        "the T237 walk counts the supervisor's own disposition — the streak incremented: {line}"
    );
    // A second bounded pass: the chain continues (streak 1 < 3, the delta
    // still bookkeeping-only, the evaluation still fresh) and hands the
    // streak forward 1→2 — the same discipline the orchestrator's own
    // dispositions follow, now proven for the supervisor's.
    let out = run_one_pass();
    assert!(
        out.status.success(),
        "the second bounded pass exits clean: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let subject = git_out(f.path(), &["log", "-1", "--format=%s"]);
    assert!(
        subject.contains("TRUE streak 1→2"),
        "the chain hands the streak forward: {subject}"
    );
    let log = std::fs::read_to_string(f.path().join(".chug/loopd/loopd.log"))
        .expect("read the fixture loopd.log");
    assert!(
        log.contains("cheap-exit pacing: sleeping 1s (empty streak 2)"),
        "the second pass paced off streak 2: {log}"
    );
    let launched = std::fs::read_dir(f.path().join(".chug/loopd"))
        .expect("read the state dir")
        .filter_map(|e| e.ok())
        .any(|e| e.file_name().to_string_lossy().starts_with("cycle-"));
    assert!(!launched, "no chug launch across both passes — no cycle log");
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

// --- static pins: the launched model ids (F1) -----------------------------------

/// (F1) The gate's launch arms resolve the FULL provider-routed model ids
/// from the T81 env — never a bare shorthand. Model ids are passed verbatim
/// into the API request body (src/api.rs `body.insert("model", ...)`); there
/// is no alias layer, so a literal `glm`/`kimi` in a launch arm dies at the
/// first call, the recorded deaths then force further launches (deaths>0 is
/// itself a borderline input), and the T81 rollback knob is silently broken.
/// The defaults legs keep this file's GLM_ID/KIMI_ID constants in sync with
/// loopd.sh; the re-route-comparison leg kills the same class one frame up
/// (a literal `kimi` there would log a phantom re-route on every kimi arm
/// and mis-key the rollback comparison).
#[test]
fn the_gate_launch_arms_resolve_full_model_ids_never_shorthands() {
    let loopd = read("loopd.sh");
    count_eq(
        &loopd,
        "LOOP_ORCH_MODEL=\"${LOOP_ORCH_MODEL:-anthropic-system.ai.kimi-k3}\"",
        1,
        "the kimi default (the behavioral legs' KIMI_ID)"
    );
    count_eq(
        &loopd,
        "LOOP_ROUTINE_MODEL=\"${LOOP_ROUTINE_MODEL:-anthropic-system.ai.glm-5-3-flash}\"",
        1,
        "the glm default (the behavioral legs' GLM_ID)"
    );
    let gate_at = loopd.find("eval_gate() {").expect("eval_gate is defined");
    let gate_end = loopd[gate_at..]
        .find("\n}")
        .map(|i| gate_at + i)
        .expect("eval_gate closes");
    let body = &loopd[gate_at..=gate_end];
    assert!(
        !body.contains("model=glm"),
        "no bare shorthand reaches a launch arm (F1): {body}"
    );
    assert!(
        !body.contains("model=kimi"),
        "no bare shorthand reaches a launch arm (F1): {body}"
    );
    count_eq(
        &loopd,
        "verb=launch; model=$LOOP_ROUTINE_MODEL",
        1,
        "the borderline arm resolves the routine model (the T81 rollback knob rides along)"
    );
    count_eq(
        &loopd,
        "verb=launch; model=$LOOP_ORCH_MODEL",
        2,
        "both kimi arms (valve trip + work delta/stale/unknown) resolve the orchestrator model"
    );
    count_eq(
        &loopd,
        "elif [ \"$gate_model\" != \"$LOOP_ORCH_MODEL\" ]; then",
        1,
        "the run-loop re-route compares against $LOOP_ORCH_MODEL, never a bare literal"
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
/// two-authority count rule) and before the commit-artifacts line —
/// extending the T247 placement pin's ordering.
#[test]
fn the_cheap_exit_clause_sits_between_the_chain_rule_and_the_commit_artifacts() {
    let spec = flat(&read("LOOP-SPEC.md"));
    let chain_tail = spec
        .find("the supervisor's valve still trips on its own mechanical record.")
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

/// (F2) The T247 paragraph scopes BOTH count authorities — the fix-up's
/// doctrine reconciliation. The supervisor's gate and its valve read the
/// MACHINE streak (an authorized reading of the TRUE count on a disciplined
/// chain, because supervisor-written dispositions carry the token by
/// construction and pin and the quote discipline keeps orchestrator-written
/// mentions clean); the HUMAN-counted rule stays binding for
/// orchestrator-side chain adjudication and contested divergences (the
/// cycle-118 class). The old text bound the trip to the TRUE count ONLY and
/// NEVER the machine streak — a direct contradiction with the gate, which
/// trips on `empty_wrap_streak`; both contradictory phrases must be gone.
#[test]
fn the_t247_valve_scopes_both_count_authorities() {
    let spec = flat(&read("LOOP-SPEC.md"));
    for needle in [
        "the count authority is scoped by who acts",
        "The supervisor's gate and its valve read the machine streak",
        "machine == TRUE on any disciplined chain",
        "the mechanical trip is an authorized reading of the TRUE count",
        "stays binding for orchestrator-side chain adjudication and for contested divergences",
        "an orchestrator who finds such a divergence adjudicates the chain by hand",
        "the supervisor's valve still trips on its own mechanical record.",
    ] {
        count_eq(&spec, needle, 1, "the T247 two-authority scoping (F2)");
    }
    assert!(
        !spec.contains("NEVER on loopd's machine streak"),
        "the old contradiction is gone — the trip no longer binds NEVER on \
         the machine streak the gate itself reads"
    );
    assert!(
        !spec.contains("the trip threshold reads the TRUE count only"),
        "the old contradiction is gone — the threshold is no longer \
         TRUE-count-only while the gate trips on the machine streak"
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
