//! T325 — dormant mode: an empty queue is a cheap exit on EVERY cycle and
//! the T247 valve is replaced by a weekly read-only audit.
//!
//! The queue hit 319 done / 0 open and the loop did not go quiet — the T247
//! valve forced a paid eval every 4th consecutive TRUE cheap-exit, and with
//! an empty backlog that eval had nothing outward to audit, so it filed
//! micro-items about its own assertion machinery (the T317–T324 meta class)
//! and the loop "shipped" them. The T258 cheap exit made empty STRETCHES
//! free; the valve's forced launches stayed the leak. T325 suspends that
//! leg while dormant: the cheap exit fires whatever the TRUE streak, and
//! the valve's 4th-TRUE trip is replaced by a weekly audit timer — a real
//! evaluation only when the last evaluation commit is 7 or more days old
//! (the anchor is git: the newest `eval:` commit, recorded in
//! `.chug/loopd/dormant-state.json`; the loop's own dispositions are empty
//! commits and can never refresh it). The audit launches with `DORMANT=1`
//! and is READ-ONLY (LOOP-SPEC's dormant paragraph: gates, pins, site
//! freshness, daemon/Langfuse liveness; rows filed only on observed
//! breakage, never loop-tooling meta-work). Any open row exits dormancy
//! immediately and the T247 valve resumes.
//!
//! Layers, the T258/T259 harness shape:
//! - BEHAVIORAL probe tests running the real `loopd.sh predicate` against
//!   fixture git repos (the spec's battery: open=0 + fresh timer →
//!   suspended-valve skip; open=0 + elapsed timer → the audit launch on
//!   the full orchestrator id; open=1 → the valve live again; an unknown
//!   timer degrades to the audit, never to a skip).
//! - RUN-MODE end-to-end tests (one bounded pass, a stub cycle child):
//!   the dormant skip writes the T325 disposition and launches nothing;
//!   the audit pass restarts the timer, logs the audit line and the child
//!   sees DORMANT=1; the entry leg arms dormancy on the FIRST empty cycle;
//!   an open row logs `dormant exit (T325): open=1`, removes the latch and
//!   resumes normal routing on the transition cycle.
//! - STATIC pins on loopd.sh: the dormancy arm precedes the valve, the
//!   git anchor lives in one function, the DORMANT=1 export is the single
//!   launch-site env write, and the log lines name T325.
//! - DOCTRINE pins on the LOOP-SPEC dormant paragraph (the read-only
//!   audit's filing rule — the eval-side contract the DORMANT=1 child
//!   runs under).
//!
//! T48 doctrine: files resolve from the checkout the binary RUNS against
//! (`std::env::current_dir()`), never the compile-time manifest-dir macro.

#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
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

// --- fixture harness (the T258 shapes, reused) --------------------------------

/// 2026-09-27T12:00:00Z — the pinned "fresh" mtime and TODAY (the routing
/// test's constants, reused so the freshness semantics stay identical).
const FRESH_EPOCH: u64 = 1_790_510_400;
const FRESH_DAY: &str = "2026-09-27";
const STALE_DAY: &str = "2026-09-28";
/// The FULL provider-routed model ids — loopd.sh's T81 defaults (the audit
/// arm resolves $LOOP_ORCH_MODEL like every other kimi arm; there is no
/// alias layer, src/api.rs passes the id verbatim into the request body).
const KIMI_ID: &str = "anthropic-system.ai.kimi-k3";
const DEATH_LINE: &str = "cycle ended WITHOUT goal complete (consecutive failures: 1)";
const FUTURE_DEATH_TS: &str = "2099-01-01T00:00:00Z";
/// A TODO.md with one DONE row — zero `todo`/`open` rows: the dormant
/// shape (the queue is empty; the gate is reached in eval mode).
const TODO_ONE_DONE: &str = "| id | title | spec | pri | status | notes |\n\
     |----|-------|------|-----|--------|-------|\n\
     | T1 | title | specs/T1-slug.md | 2 | done | notes |\n";
/// The same row with the operator's dormant-era active status — one OPEN
/// row is real queue work and must exit dormancy (T325 wake condition).
const TODO_ONE_OPEN: &str = "| id | title | spec | pri | status | notes |\n\
     |----|-------|------|-----|--------|-------|\n\
     | T1 | title | specs/T1-slug.md | 2 | open | notes |\n";

/// Run one git command in `root` with a throwaway identity (the fixture
/// commits are content, not history worth signing).
fn git(root: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(["-c", "user.name=t325", "-c", "user.email=t325@example.test"])
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

/// Write an executable stub script (the run-mode leg's fake `ps`, `cargo`
/// and cycle child — host-facing probes stubbed at the PATH layer, where
/// loopd resolves every command).
fn write_exec(path: &Path, body: &str) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::write(path, body).expect("write fixture stub");
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .expect("chmod fixture stub");
}

/// A fixture git repo: a copy of the real loopd.sh (plus its T213 loader
/// fragment) inside a fresh repo whose first commit is the BASELINE — an
/// `eval:` commit adding EVALUATION.md (fresh-mtime pinned) and a TODO.md
/// whose only row is done (the empty-queue shape). The script cd's to its
/// own directory, so the copy sees the fixture repo.
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
    // flip bookkeeping=no on every later fixture commit (the T258 fixture's
    // ignore set, plus the dirs the run-mode leg creates).
    std::fs::write(tmp.path().join(".gitignore"), "/.chug/\n/target-shared/\n/bin/\n")
        .expect("write fixture .gitignore");
    git(tmp.path(), &["init", "-q"]);
    let f = Fixture(tmp);
    f.commit_tree(
        "eval: cycle-100 fresh eval — the baseline (T325 fixture)",
        Some(TODO_ONE_DONE),
        Some("# eval\n"),
    );
    f.pin_eval_mtime();
    // A QUIET fixture must PROVE quiet — the log exists and records no
    // death, so `deaths=0` is evidence of absence (the T258 F3d shape).
    f.log_line("2026-09-27T12:00:00Z loopd start (pid 4242) — T325 fixture baseline record");
    f
}

impl Fixture {
    fn path(&self) -> &Path {
        self.0.path()
    }

    /// Commit the current tree (pass files first via `write_todo`), or an
    /// empty commit when nothing changed.
    fn commit_tree(&self, subject: &str, todo: Option<&str>, eval: Option<&str>) {
        if let Some(todo) = todo {
            std::fs::write(self.path().join("TODO.md"), todo).expect("write fixture TODO.md");
        }
        if let Some(eval) = eval {
            std::fs::write(self.path().join("EVALUATION.md"), eval).expect("write fixture EVALUATION.md");
        }
        git(self.path(), &["add", "-A"]);
        let status = Command::new("git")
            .args(["-c", "user.name=t325", "-c", "user.email=t325@example.test"])
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

    /// `n` empty wrap-note commits — the T237 streak walk's input, the
    /// battery's `streak=4` (a valve-trip count, one PAST the trip).
    fn wraps(&self, n: usize) {
        for i in 0..n {
            self.commit_empty(&format!(
                "eval: cycle-{} wrap notes (empty-delta disposition, {}st consecutive no-op)",
                101 + i,
                i + 1
            ));
        }
    }

    /// Arm the dormancy latch with `last_eval_ts = now - age_secs` — the
    /// T325 battery's timer control ("last_eval N ago"): the value the
    /// gate reads, as the run loop would have written it at latch time.
    fn latch(&self, age_secs: u64) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_secs();
        self.latch_raw(&format!("{{\"last_eval_ts\": {}}}\n", now - age_secs));
    }

    /// Write the latch file verbatim (the corrupted-record leg).
    fn latch_raw(&self, body: &str) {
        let dir = self.path().join(".chug/loopd");
        std::fs::create_dir_all(&dir).expect("fixture state dir");
        std::fs::write(dir.join("dormant-state.json"), body).expect("write fixture latch");
    }

    /// The latch's recorded timer value, if present and readable.
    fn latch_ts(&self) -> Option<i64> {
        let text = std::fs::read_to_string(self.path().join(".chug/loopd/dormant-state.json"))
            .ok()?;
        let at = text.find("\"last_eval_ts\"")?;
        let rest = &text[at + "\"last_eval_ts\"".len()..];
        let digits: String = rest.chars().skip_while(|c| !c.is_ascii_digit()).collect();
        digits.split(|c: char| !c.is_ascii_digit()).next()?.parse().ok()
    }

    fn latch_exists(&self) -> bool {
        self.path().join(".chug/loopd/dormant-state.json").exists()
    }

    fn fixture_log(&self) -> String {
        std::fs::read_to_string(self.path().join(".chug/loopd/loopd.log"))
            .expect("read the fixture loopd.log")
    }

    /// Install the run-mode stubs: fake `ps` (no drivers — the T53 probe
    /// passes), fake `cargo` (the build is not under test), and a cycle
    /// child at ./target/release/chug whose body the scenario injects.
    /// The stub child records its DORMANT env next to the sandbox root —
    /// the observable for the DORMANT=1 pin.
    fn install_run_stubs(&self) -> PathBuf {
        let bin = self.path().join("bin");
        std::fs::create_dir_all(&bin).expect("fixture stub bin dir");
        write_exec(&bin.join("ps"), "#!/bin/sh\necho \"fake ps: no drivers\"\n");
        write_exec(&bin.join("cargo"), "#!/bin/sh\nexit 0\n");
        let chug_dir = self.path().join("target/release");
        std::fs::create_dir_all(&chug_dir).expect("fixture target dir");
        write_exec(
            &chug_dir.join("chug"),
            "#!/bin/sh\n\
             printf 'DORMANT=%s\\n' \"${DORMANT-UNSET}\" > \"$PWD/child-env.txt\"\n\
             printf 'chug: goal complete\\nsummary: t325 stub child ran\\n'\n\
             exit 0\n",
        );
        self.path().join("child-env.txt")
    }

    /// One bounded run-mode pass (the T258 end-to-end shape: stub ps/cargo,
    /// one iteration, 1s sleeps, no daemon ensure, no site sync).
    fn run_one_pass(&self) {
        self.install_run_stubs();
        let status = Command::new("bash")
            .arg(self.path().join("loopd.sh"))
            .arg("run")
            .current_dir(self.path())
            .env(
                "PATH",
                format!(
                    "{}:{}",
                    self.path().join("bin").display(),
                    std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".to_string())
                ),
            )
            .env("HOME", self.path())
            .env("CHUG_ROUTINE_TODAY", FRESH_DAY)
            .env("CHUG_DAEMON_SOCK", self.path().join(".chug/no-daemon.sock"))
            .env("CHUG_SITE_DIR", self.path().join("NO-SUCH-SITE"))
            .env("CHUG_SITE_SYNC_NO_PUSH", "1")
            .env("LOOPD_SLEEP_OK", "1")
            .env("LOOPD_SLEEP_FAIL", "1")
            .env("LOOPD_MAX_LOOPS", "1")
            .env("LOOP_DAEMON_ENSURE", "0")
            .env("GIT_AUTHOR_NAME", "t325-fixture")
            .env("GIT_AUTHOR_EMAIL", "t325@example.test")
            .env("GIT_COMMITTER_NAME", "t325-fixture")
            .env("GIT_COMMITTER_EMAIL", "t325@example.test")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .stdout(Stdio::null())
            .output()
            .expect("spawn loopd.sh run");
        assert!(
            status.status.success(),
            "run pass exited {:?}: {}",
            status.status.code(),
            String::from_utf8_lossy(&status.stderr)
        );
    }
}

/// Run the copied script's `predicate` probe with the env CLEARED of the
/// routing knobs (and the git vars that could redirect the walk), then the
/// caller's overrides applied. Prints the gate's one decision line. The
/// daemon vars die after the overrides and HOME pins to the fixture root —
/// the T259 hermeticity shape (a probe run must never reach the host's
/// live judge daemon, and the fixture HOME kills the loopd.env load).
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
    cmd.env_remove("CHUG_DAEMON_SOCK").env_remove("CHUG_HOME");
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
/// id on launch — never a bare shorthand (the T258 F1 rule).
fn model_of(gate: &str) -> &str {
    gate.split_whitespace().nth(1).expect("gate line has a model")
}

/// The value of one `<key>=` field in the gate line (empty when absent).
fn field_of(gate: &str, key: &str) -> String {
    gate.split_whitespace()
        .find(|f| f.starts_with(key))
        .map(|f| f[key.len()..].to_string())
        .unwrap_or_default()
}

/// The T325 battery's dormant fixture: empty queue, `streak` wrap-notes
/// past the trip (the valve WOULD fire on the awake path — that is the
/// leak), and the latch armed with the timer `age_secs` old.
fn dormant_fixture(streak: usize, age_secs: u64) -> Fixture {
    let f = fixture();
    f.wraps(streak);
    f.latch(age_secs);
    f
}

const DAY: u64 = 86_400;
const WEEK: u64 = 7 * DAY;

// --- the spec's battery, leg 1: dormant = the cheap exit every cycle ----------

/// `open=0, streak=4, last_eval 2d ago` → the cheap exit fires DESPITE a
/// trip-count streak: the valve is suspended (disposition
/// `dormant=suspended-valve`), no model is picked, the timer is not due.
#[test]
fn an_empty_queue_with_a_fresh_timer_takes_the_cheap_exit_every_cycle() {
    let f = dormant_fixture(4, 2 * DAY);
    let gate = run_predicate(f.path(), &[("CHUG_ROUTINE_TODAY", FRESH_DAY)]);
    assert_eq!(verb_of(&gate), "skip", "dormant: the cheap exit, not the valve: {gate}");
    assert_eq!(model_of(&gate), "-", "a suspended cycle picks no orchestrator: {gate}");
    assert_eq!(
        field_of(&gate, "dormant="),
        "suspended-valve",
        "the disposition names the suspension (the spec's battery leg 1): {gate}"
    );
    assert!(gate.contains("streak=4"), "the TRUE streak still rides the line: {gate}");
    assert!(gate.contains("rows=0"), "the empty queue is the dormancy predicate: {gate}");
    assert!(
        gate.contains("laya=none"),
        "a dormant skip never consults the triage layer (nothing to decide): {gate}"
    );
}

// --- the spec's battery, leg 2: the weekly audit timer ------------------------

/// `open=0, streak=4, last_eval 8d ago` → the timer fires: a REAL eval on
/// the FULL orchestrator-model id (the weekly audit), named `dormant=audit`
/// on the decision line.
#[test]
fn the_weekly_audit_timer_launches_the_real_eval_when_the_week_elapsed() {
    let f = dormant_fixture(4, 8 * DAY);
    let gate = run_predicate(f.path(), &[("CHUG_ROUTINE_TODAY", FRESH_DAY)]);
    assert_eq!(verb_of(&gate), "launch", "the weekly audit is due: {gate}");
    assert_eq!(
        model_of(&gate), KIMI_ID,
        "the audit is a real eval: the FULL provider-routed orchestrator id, never a shorthand: {gate}"
    );
    assert_eq!(
        field_of(&gate, "dormant="),
        "audit",
        "the decision names the audit class (the spec's battery leg 2): {gate}"
    );
    assert!(gate.contains("rows=0"), "the queue is still empty — this is the timer's launch: {gate}");
}

/// The audit is mandated mechanically: even a confident-empty triage
/// verdict (needs-eval=no at/above TRIAGE_HIGH — the answer that cancels a
/// T247 valve trip) cannot cancel the one evaluation dormancy schedules.
/// The dormancy arm decides BEFORE the valve leg and never consults the
/// judge daemon; the decision line reads laya=none.
#[test]
fn the_audit_launch_never_consults_the_triage_layer() {
    let f = dormant_fixture(4, 8 * DAY);
    // A fake judge daemon that would confidently cancel any valve trip:
    // needs-eval=no, confidence 0.93 (>= TRIAGE_HIGH=0.85). The gate must
    // never dial it — laya=none on the line, the launch stands.
    let sock = f.path().join(".chug/fixture-judge.sock");
    let listener = std::os::unix::net::UnixListener::bind(&sock).expect("bind the fixture judge socket");
    let t = std::thread::spawn(move || {
        // Accept exactly one connection if the (wrong) consult ever
        // happens; otherwise the thread parks until the test ends.
        let _ = listener.accept();
    });
    let gate = run_predicate(f.path(), &[("CHUG_ROUTINE_TODAY", FRESH_DAY)]);
    assert_eq!(verb_of(&gate), "launch", "the timer mandates the audit launch: {gate}");
    assert_eq!(field_of(&gate, "laya="), "none", "no triage verdict on a dormant line: {gate}");
    assert!(
        !t.is_finished(),
        "the judge daemon was never dialed by the dormant audit decision"
    );
}

/// An unknown timer degrades to the AUDIT, never to a skip — the safe-side
/// rule: every unknown degrades to a launch. A corrupted latch record is
/// exactly that unknown.
#[test]
fn an_unknown_timer_degrades_to_the_audit_not_the_skip() {
    let f = dormant_fixture(4, 8 * DAY);
    f.latch_raw("{\"last_eval_ts\": \"two-weeks-ago\"}\n");
    let gate = run_predicate(f.path(), &[("CHUG_ROUTINE_TODAY", FRESH_DAY)]);
    assert_eq!(verb_of(&gate), "launch", "an unreadable timer is an unknown: {gate}");
    assert_eq!(field_of(&gate, "dormant="), "audit", "unknown → the audit (the safe side): {gate}");
}

/// The 7d boundary is INCLUSIVE-strict: the audit fires only when the age
/// is at or past the week (6d stays suspended) — one knob
/// (DORMANT_AUDIT_SECS=604800), the comparison the gate re-derives.
#[test]
fn the_audit_timer_binds_at_seven_days_not_before() {
    let f = dormant_fixture(4, 6 * DAY);
    let gate = run_predicate(f.path(), &[("CHUG_ROUTINE_TODAY", FRESH_DAY)]);
    assert_eq!(
        field_of(&gate, "dormant="),
        "suspended-valve",
        "6 days is still inside the week: {gate}"
    );
    let f = dormant_fixture(4, WEEK);
    let gate = run_predicate(f.path(), &[("CHUG_ROUTINE_TODAY", FRESH_DAY)]);
    assert_eq!(
        field_of(&gate, "dormant="),
        "audit",
        "exactly 7 days is the audit (>= the DORMANT_AUDIT_SECS bound): {gate}"
    );
}

// --- the spec's battery, leg 3: the wake condition -----------------------------

/// `open=1` → normal routing: the valve is LIVE again (streak 4 trips it)
/// even though the latch is still armed — the gate re-derives dormancy
/// from the LIVE row count every cycle, so an armed latch cannot out-vote
/// real queue work.
#[test]
fn an_open_row_exits_dormancy_and_the_valve_resumes() {
    let f = fixture();
    f.wraps(4);
    f.latch(2 * DAY);
    // One open row in the working tree (real queue work) + a stale
    // evaluation day so the cycle routes eval mode and the gate is reached.
    std::fs::write(f.path().join("TODO.md"), TODO_ONE_OPEN).expect("write fixture TODO.md");
    let gate = run_predicate(f.path(), &[("CHUG_ROUTINE_TODAY", STALE_DAY)]);
    assert_eq!(verb_of(&gate), "launch", "an open row wakes the loop: {gate}");
    assert_eq!(
        model_of(&gate), KIMI_ID,
        "the valve resumes on the wake cycle (streak 4 trips it): {gate}"
    );
    assert_eq!(field_of(&gate, "dormant="), "no", "the loop is awake: {gate}");
    assert!(
        gate.contains("laya=down"),
        "the valve's triage consult fail-opens without a daemon (the T259 shape): {gate}"
    );
}

// --- run mode: the dormant skip's disposition and the audit's launch ----------

/// A dormant cycle SKIPS the launch and loopd writes the T325 disposition
/// itself: the log names the suspension, the empty commit carries the T325
/// reason AND the T237 token with the TRUE-streak handoff (the chain's
/// bookkeeping stays continuous through dormancy), and the stub cycle
/// child never runs.
#[test]
fn a_dormant_cycle_skips_the_launch_and_writes_the_suspended_valve_disposition() {
    let f = dormant_fixture(4, 2 * DAY);
    let child_env = f.install_run_stubs();
    f.run_one_pass();
    let log = f.fixture_log();
    assert!(
        log.contains("dormant cheap-exit (T325): streak=4 suspended-valve"),
        "the dormant skip logs the suspension line: {log}"
    );
    assert!(
        log.contains("cheap-exit disposition (T258):"),
        "the T258 disposition record still lands (the T237 walk needs it): {log}"
    );
    assert!(
        !log.contains("cycle start ->"),
        "a dormant cycle launches nothing: {log}"
    );
    assert!(
        !child_env.exists(),
        "the stub cycle child never ran on a dormant skip"
    );
    let subject = git_out(f.path(), &["log", "-1", "--format=%s"]);
    assert!(
        subject.contains("empty-delta disposition"),
        "the T237 token rides the dormant disposition: {subject}"
    );
    assert!(
        subject.contains("T325 dormant: streak=4 suspended-valve"),
        "the disposition names the suspension class: {subject}"
    );
    assert!(
        subject.contains("TRUE streak 4→5"),
        "the TRUE-streak handoff keeps the chain's bookkeeping continuous: {subject}"
    );
    assert!(
        f.latch_exists(),
        "the latch survives a dormant cycle (the timer keeps elapsing)"
    );
}

/// The weekly audit pass: the timer restarts BEFORE the launch (a crashed
/// audit re-audits next cycle instead of re-auditing forever), the log
/// names the audit line with DORMANT=1, and the stub cycle child observes
/// DORMANT=1 in its env — the read-only audit's contract flag.
#[test]
fn the_dormant_weekly_audit_launches_the_child_with_dormant_1() {
    let f = dormant_fixture(4, 8 * DAY);
    let before = f.latch_ts().expect("the fixture latch is armed");
    let child_env = f.install_run_stubs();
    f.run_one_pass();
    let log = f.fixture_log();
    assert!(
        log.contains("dormant weekly audit (T325):"),
        "the audit launch is named in the log: {log}"
    );
    assert!(
        log.contains("DORMANT=1"),
        "the audit log line names the env flag: {log}"
    );
    assert!(
        log.contains("cycle start ->"),
        "the audit is a real launch (the child runs): {log}"
    );
    let captured = std::fs::read_to_string(&child_env).expect("the stub child recorded its env");
    assert_eq!(
        captured.trim(),
        "DORMANT=1",
        "the child env carries DORMANT=1 (the LOOP-SPEC dormant paragraph's flag): {captured}"
    );
    let after = f.latch_ts().expect("the audit rewrote the latch");
    assert!(
        after > before,
        "the timer restarted at the audit launch (before {before}, after {after})"
    );
}

/// The entry leg: the FIRST empty cycle arms dormancy itself (the latch is
/// written at cycle routing time, before the gate) — a loop that just
/// drained goes quiet on that very cycle, with the timer anchored to git's
/// newest eval: commit (recent here, so the audit is a week out).
#[test]
fn the_first_empty_cycle_arms_dormancy_itself() {
    let f = fixture();
    f.wraps(4); // a trip-count streak: the AWAKE path would have launched
    assert!(!f.latch_exists(), "the fixture starts awake (no latch)");
    let child_env = f.install_run_stubs();
    f.run_one_pass();
    let log = f.fixture_log();
    assert!(
        log.contains("dormant entry (T325): open=0"),
        "the entry leg logs the arming: {log}"
    );
    assert!(
        f.latch_exists(),
        "the latch is written at routing time, before the gate reads it"
    );
    let ts = f.latch_ts().expect("the anchor was written from git");
    assert!(
        ts > 0,
        "the anchor is the git-derived newest eval: commit epoch (nonzero): {ts}"
    );
    assert!(
        !child_env.exists(),
        "the just-drained loop goes quiet on the arming cycle itself (the timer is fresh)"
    );
    assert!(
        log.contains("dormant cheap-exit (T325):"),
        "the arming cycle takes the suspended-valve cheap exit: {log}"
    );
}

/// The wake transition: an open row exits dormancy ON the transition cycle
/// — the latch is removed, `dormant exit (T325): open=1` is logged, and
/// normal routing resumes (the stub child runs the real cycle).
#[test]
fn an_open_row_logs_the_dormant_exit_and_resumes_normal_routing() {
    let f = dormant_fixture(4, 2 * DAY);
    std::fs::write(f.path().join("TODO.md"), TODO_ONE_OPEN).expect("write fixture TODO.md");
    let child_env = f.install_run_stubs();
    f.run_one_pass();
    let log = f.fixture_log();
    assert!(
        log.contains("dormant exit (T325): open=1"),
        "the transition cycle logs the wake (the spec's battery leg 3): {log}"
    );
    assert!(
        !f.latch_exists(),
        "the latch is removed on the transition — the loop is awake"
    );
    let captured_launch = std::fs::read_to_string(&child_env).is_ok();
    assert!(
        captured_launch,
        "normal routing resumed: the cycle launched (the stub child ran); log: {}",
        log
    );
}

// --- static pins: the wiring ---------------------------------------------------

/// The gate's dormancy arm precedes the T247 valve leg, the timer reads one
/// constant, the git anchor lives in ONE function, and the decision line
/// carries the dormant field. The run loop arms/disarms the latch at cycle
/// routing time — after the routing echo, before the mode branch — and the
/// DORMANT=1 export is the single launch-site env write.
#[test]
fn the_dormancy_wiring_precedes_the_valve_and_anchors_the_timer_in_git() {
    let loopd = read("loopd.sh");
    // The arm: dormancy = empty queue AND the latch, inside eval_gate,
    // BEFORE the valve condition.
    let gate_at = loopd.find("eval_gate() {").expect("eval_gate is defined");
    let gate_end = loopd[gate_at..]
        .find("\n}")
        .map(|i| gate_at + i)
        .expect("eval_gate closes");
    let body = &loopd[gate_at..=gate_end];
    let dormant_arm = body
        .find("if [ \"$(todo_rows TODO.md)\" -eq 0 ] && [ -f \"$STATE/dormant-state.json\" ]; then")
        .expect("the dormancy arm is wired");
    let valve = body
        .find("[ \"$streak\" -ge 3 ]")
        .expect("the valve arm is wired");
    assert!(
        dormant_arm < valve,
        "dormancy decides BEFORE the T247 valve leg (the spec's routing order): \
         the valve may not bind a dormant cycle"
    );
    assert!(
        body.contains("dormant=suspended-valve") && body.contains("dormant=audit"),
        "the two dormant verdicts are named on the decision line"
    );
    assert!(
        body.contains("verb=skip; model=-; dormant=suspended-valve"),
        "the suspended skip picks no orchestrator model"
    );
    assert!(
        body.contains("verb=launch; model=$LOOP_ORCH_MODEL; dormant=audit"),
        "the audit arm resolves the FULL orchestrator id (the F1 rule)"
    );
    // The timer: one constant, one anchor, one read.
    count_eq(
        &loopd,
        "DORMANT_AUDIT_SECS=604800",
        1,
        "the weekly bound is one knob, one place (7d in seconds)"
    );
    count_eq(
        &loopd,
        "git log -1 --format=%ct -- EVALUATION.md",
        1,
        "the git anchor lives in one function (dormant_newest_eval_ts)"
    );
    count_eq(
        &loopd,
        "printf '{\"last_eval_ts\": %s}\\n' \"$ts\" > \"$STATE/dormant-state.json\"",
        1,
        "the latch write is one site (the file is the record, never the authority)"
    );
    // The run loop: routing-time maintenance before the mode branch; the
    // child env write at the launch site.
    let route_at = loopd
        .find("orch_model=${routing#* }")
        .expect("the run loop parses the routing");
    let mode_at = loopd[route_at..]
        .find("if [ \"$mode\" = eval ]; then")
        .map(|i| route_at + i)
        .expect("the mode branch follows the routing");
    let entry_at = loopd[route_at..]
        .find("dormant_state_write")
        .map(|i| route_at + i)
        .expect("the entry leg arms the latch in the run loop");
    assert!(
        route_at < entry_at && entry_at < mode_at,
        "the latch maintenance sits at cycle routing time, BEFORE the gate is \
         consulted (the spec's insertion point)"
    );
    count_eq(
        &loopd,
        "export DORMANT=1",
        1,
        "the child env write is one site — the audit launch"
    );
    count_eq(
        &loopd,
        "unset DORMANT",
        1,
        "every other cycle launches with DORMANT explicitly unset (the T47 \
         in-loop-export lesson, applied defensively)"
    );
}

/// The four T325 log lines exist exactly once each: the entry arming, the
/// wake transition, the suspended skip, and the audit launch.
#[test]
fn the_dormant_log_lines_are_named_once_each() {
    let loopd = read("loopd.sh");
    count_eq(
        &loopd,
        "dormant entry (T325): open=0",
        1,
        "the entry leg logs the arming"
    );
    count_eq(
        &loopd,
        "dormant exit (T325): open=$open_rows",
        1,
        "the wake transition logs the live open count"
    );
    count_eq(
        &loopd,
        "dormant cheap-exit (T325): streak=$gate_streak suspended-valve",
        1,
        "the suspended skip logs the suspension with the TRUE streak"
    );
    count_eq(
        &loopd,
        "dormant weekly audit (T325): timer restarted",
        1,
        "the audit launch logs the timer restart"
    );
    count_eq(
        &loopd,
        "dormant_state_write",
        3,
        "the latch write helper is called at exactly three sites: the entry \
         arm, the audit restart, and its own definition"
    );
}

/// The disposition writer names the T325 suspension in the subject — the
/// dormant skip's commit record (the battery asserts the committed shape
/// end-to-end; this pin keeps the wiring at the writer).
#[test]
fn the_disposition_writer_names_the_suspended_valve() {
    let loopd = read("loopd.sh");
    count_eq(
        &loopd,
        "(T325 dormant: streak=$streak suspended-valve — the weekly audit timer replaces the valve)",
        1,
        "the dormant reason is one string, assembled once"
    );
    count_eq(
        &loopd,
        "dormant=$(laya_gate_field dormant= \"$fields\")",
        1,
        "the writer reads the dormant field off the gate line (one parse)"
    );
}

// --- doctrine pins: the LOOP-SPEC dormant paragraph ----------------------------

/// The eval-side contract the DORMANT=1 child runs under: the dormant
/// paragraph in LOOP-SPEC Phase 1 (between the triage layer and the
/// commit-artifacts sentence) carries the read-only audit's scope, the
/// filing rule (rows ONLY on observed breakage — a green audit files
/// zero), the anti-meta-work clause, the disposition-line shape, and the
/// wake condition.
#[test]
fn loop_spec_carries_the_dormant_paragraph() {
    let spec = flat(&read("LOOP-SPEC.md"));
    for needle in [
        "Dormant mode (T325).",
        "zero `todo`/`open` rows in TODO.md",
        "the supervisor's gate suspends the valve",
        "the cheap exit fires on every cycle whatever the TRUE streak",
        "suspended-valve",
        "replaced by a weekly audit timer",
        "a real evaluation launches only when the last evaluation commit is 7 or more days old",
        "`.chug/loopd/dormant-state.json`",
        "written from the newest `eval:` commit's timestamp",
        "no new authority, git is the source",
        "launches with `DORMANT=1` in the env and is READ-ONLY",
        "gates green (build, clippy, test), pins verified, site-sync freshness, daemon and Langfuse liveness",
        "files TODO rows ONLY on observed breakage",
        "exactly one row per observed breakage",
        "a green audit files zero rows",
        "MUST NOT file loop-tooling items",
        "the wrap_assert/fixture/citation-hygiene meta class is precisely the manufactured work dormancy exists to stop",
        "dormant audit (T325): gates=green pins=23/23 site=fresh daemon=ok filings=0",
        "exits dormancy immediately and the normal T247 valve semantics resume",
        "dormant exit (T325): open=1",
        "An empty backlog must make the loop nearly free; it must never make the loop invent meta-work.",
    ] {
        assert!(
            spec.contains(needle),
            "the dormant paragraph carries {needle:?}: {spec:?}"
        );
    }
}

/// The paragraph's POSITION: the dormant doctrine sits after the T259
/// triage paragraph and before the commit-artifacts sentence — the chain
/// rule (T247), the cheap exit (T258), the triage (T259), dormancy (T325),
/// then the commit instruction.
#[test]
fn the_dormant_paragraph_sits_between_the_triage_layer_and_the_commit_rule() {
    let spec = flat(&read("LOOP-SPEC.md"));
    let triage = spec
        .find("The triage layer (T259).")
        .expect("the triage paragraph is present");
    let dormant = spec
        .find("Dormant mode (T325).")
        .expect("the dormant paragraph is present");
    let commit = spec
        .find("Commit the evaluation artifacts")
        .expect("the commit-artifacts sentence is present");
    assert!(
        triage < dormant && dormant < commit,
        "the doctrine reads in the written order — triage ({triage}), \
         dormancy ({dormant}), commit artifacts ({commit})"
    );
}

/// git_out: run one git command in `root` and return its trimmed stdout.
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

/// The suspension outranks the mechanical inputs: a death after the
/// baseline does NOT force a dormant launch (the T258 safe-side arms are
/// suspended with the valve). The safety net is the RUN LOOP's
/// consecutive-failure HALT, which counts the same death line
/// independently of the gate — dormancy cannot hide a dying loop.
#[test]
fn a_death_does_not_force_a_dormant_launch() {
    let f = dormant_fixture(4, 2 * DAY);
    f.log_line(&format!("{FUTURE_DEATH_TS} {DEATH_LINE}"));
    let gate = run_predicate(f.path(), &[("CHUG_ROUTINE_TODAY", FRESH_DAY)]);
    assert_eq!(verb_of(&gate), "skip", "dormant: the timer, not the death, decides: {gate}");
    assert_eq!(field_of(&gate, "dormant="), "suspended-valve", ": {gate}");
    assert!(gate.contains("deaths=1"), "the death is still visible on the line: {gate}");
    // The same fixture awake (no latch) launches — the death forces it.
    std::fs::remove_file(f.path().join(".chug/loopd/dormant-state.json")).expect("remove latch");
    let gate = run_predicate(f.path(), &[("CHUG_ROUTINE_TODAY", FRESH_DAY)]);
    assert_eq!(verb_of(&gate), "launch", "awake: the death forces the launch: {gate}");
}
