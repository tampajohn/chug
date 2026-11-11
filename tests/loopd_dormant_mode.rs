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

#[path = "common/mod.rs"]
mod common;

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
    // T213/T330: loopd.sh sources scripts/*.sh fragments relative to its own
    // path — the fixture copies loopd.sh plus the DERIVED fragment set
    // (common::copy_loopd_fixture), so a new fragment never re-breaks this
    // fixture silently (the routing fixture's lesson, generalized).
    common::copy_loopd_fixture(tmp.path());
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

    /// Backdate the GIT ANCHOR — the newest EVALUATION.md-touching commit's
    /// committer epoch — to `now - age_secs`. Must run BEFORE `wraps()`
    /// (an amend rewrites the commit; children would orphan). The T325-F1
    /// battery's anchor control: the timer's due decision reads the NEWER
    /// of the record and the anchor, so a timer leg that means "8d old"
    /// must stale BOTH halves — a landed audit moves the anchor (the
    /// duplicate stops), a crashed one does not (the audit re-fires).
    fn backdate_anchor(&self, age_secs: u64) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_secs();
        let when = format!("@{} +0000", now - age_secs);
        let out = Command::new("git")
            .args(["-c", "user.name=t325", "-c", "user.email=t325@example.test"])
            .arg("-C")
            .arg(self.path())
            .env("GIT_COMMITTER_DATE", &when)
            .args(["commit", "--amend", "--no-edit", "-q"])
            .output()
            .expect("spawn git amend");
        assert!(
            out.status.success(),
            "anchor amend failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    /// The git anchor's committer epoch — the value the gate's timer half
    /// reads straight from git (`git log -1 --format=%ct -- EVALUATION.md`).
    fn anchor_ts(&self) -> i64 {
        git_out(self.path(), &["log", "-1", "--format=%ct", "--", "EVALUATION.md"])
            .parse()
            .expect("the anchor epoch parses")
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
        let mut cmd = Command::new("bash");
        cmd.arg(self.path().join("loopd.sh"))
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
            .stdout(Stdio::null());
        // T329: pin the T213/T330 loader files to empty fixture paths.
        common::pin_loopd_env_files(&mut cmd, self.path());
        let status = cmd
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
    // T329: pin the T213/T330 loader files to empty fixture paths — a host
    // ~/.chug/profile.env must never re-export routing knobs into the probe
    // (the T332 hermeticity contract).
    common::pin_loopd_env_files(&mut cmd, root);
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
/// leak), and the timer `age_secs` old on BOTH halves — the latch record
/// AND the git anchor (the T325-F1 gate reads the NEWER of the two, so a
/// timer leg must stale both: the anchor half is what a landed audit
/// moves and a crashed one leaves).
fn dormant_fixture(streak: usize, age_secs: u64) -> Fixture {
    let f = fixture();
    f.backdate_anchor(age_secs);
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
/// rule: an unknown clock must never suppress the audit. A corrupted latch
/// record is an unknown HALF (it degrades to 0, the due side); with the
/// git anchor stale too, the timer is unknown-overall and the audit fires.
/// (A corrupt record over a FRESH anchor is not an unknown timer — git is
/// the source and proves the week restarted; that stop is the F1 fix's
/// own regression leg below.)
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

/// The weekly audit pass: the launch names itself and carries DORMANT=1 to
/// the child (the LOOP-SPEC read-only contract flag), and the RECORD
/// re-arms AFTER the child returns — from the git anchor, never a
/// launch-time stamp (the T325-F1 fix: the launch rewrite read the
/// PRE-audit anchor, kept the stale week, and re-fired a duplicate; the
/// restart is the audit's OWN eval: commit, so a crashed audit — the
/// anchor unmoved — re-arms stale and simply re-audits next cycle).
#[test]
fn the_dormant_weekly_audit_launches_the_child_with_dormant_1() {
    let f = dormant_fixture(4, 8 * DAY);
    let anchor = f.anchor_ts();
    let child_env = f.install_run_stubs();
    f.run_one_pass();
    let log = f.fixture_log();
    assert!(
        log.contains("dormant weekly audit (T325): due — read-only audit launching, DORMANT=1"),
        "the audit launch is named in the log: {log}"
    );
    assert!(
        log.contains("DORMANT=1"),
        "the audit log line names the env flag: {log}"
    );
    assert!(
        log.contains("dormant weekly audit (T325): timer re-armed from the git anchor"),
        "the record re-arm is named in the log (after the child): {log}"
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
    assert_eq!(
        f.latch_ts(),
        Some(anchor),
        "the record re-arms from the git anchor the audit has NOT moved yet — \
         never a launch-time stamp (T325-F1: the restart is the audit's own \
         eval: commit, so a crashed audit's re-fire stays live)"
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
    // The timer: one constant, one anchor, one read — and the T325-F1
    // due decision: the gate reads the max helper, the record re-arms
    // AFTER the child returns (a launch-time restart re-reads the
    // pre-audit anchor and re-fires a duplicate).
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
        "dormant_timer_ts() {",
        1,
        "the effective-anchor max helper is defined once"
    );
    count_eq(
        &loopd,
        "dts=$(dormant_timer_ts)",
        1,
        "the gate's due decision reads the NEWER of record and git anchor \
         (T325-F1) — never the record alone"
    );
    count_eq(
        &loopd,
        "printf '{\"last_eval_ts\": %s}\\n' \"$ts\" > \"$STATE/dormant-state.json\"",
        1,
        "the latch write is one site (the file is the record, never the authority)"
    );
    let export_at = loopd.find("export DORMANT=1").expect("the audit env write exists");
    let chug_at = loopd
        .find("chug_out=\"$(CARGO_TARGET_DIR=")
        .expect("the cycle child launch exists");
    let rearm_at = loopd[chug_at..]
        .find("dormant_state_write")
        .map(|i| chug_at + i)
        .expect("the audit's record re-arm exists");
    assert!(
        export_at < chug_at && chug_at < rearm_at,
        "the audit's record re-arm sits AFTER the child returns (T325-F1): a \
         launch-time restart re-reads the PRE-audit anchor, keeps the stale \
         week, and re-fires the audit — the duplicate this fix removes"
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
        "dormant weekly audit (T325): due — read-only audit launching, DORMANT=1",
        1,
        "the audit launch names the env flag and defers the restart to the \
         audit's own eval: commit"
    );
    count_eq(
        &loopd,
        "dormant weekly audit (T325): timer re-armed from the git anchor",
        1,
        "the post-child re-arm names the anchor — the record never claims a \
         restart the audit did not land"
    );
    count_eq(
        &loopd,
        "dormant_state_write",
        3,
        "the latch write helper is called at exactly three sites: the entry \
         arm, the post-child audit re-arm, and its own definition"
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
        "The gate's due decision reads the NEWER of the record and the git anchor",
        "a landed audit's own `eval:` commit restarts the week for the very next cycle",
        "a crashed audit (nothing landed) simply re-fires",
        "the launch never stamps the timer itself",
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

/// THE T325-F1 REGRESSION — the duplicate-audit storm, end to end, at the
/// gate. The anchor is 8d old → cycle D launches the audit. The audit
/// LANDS (an `eval:` commit touches EVALUATION.md seconds later) while the
/// latch still holds the stale PRE-audit ts — the exact state the old
/// launch-time rewrite left behind — and cycle D+1 must SKIP: the gate's
/// timer reads max(record, anchor), the landed commit moves the anchor
/// half, the week has restarted, and no second audit fires. Before the
/// fix the stale record alone read 8d-old and cycle D+1 re-launched — one
/// duplicate READ-ONLY audit per weekly event, forever.
#[test]
fn a_landed_audit_stops_the_duplicate_the_next_cycle_despite_the_stale_latch() {
    let f = fixture();
    f.backdate_anchor(8 * DAY);
    f.wraps(4);
    f.latch(8 * DAY); // the stale pre-audit record (the old launch-time rewrite's state)
    let gate = run_predicate(f.path(), &[("CHUG_ROUTINE_TODAY", FRESH_DAY)]);
    assert_eq!(
        verb_of(&gate), "launch",
        "cycle D: the audit is due on the 8d-old timer: {gate}"
    );
    assert_eq!(field_of(&gate, "dormant="), "audit", ": {gate}");
    // The crash mirror, BEFORE the landing: the audit ran and died —
    // nothing landed, nothing moved — so the timer is still due and the
    // audit simply re-fires next cycle (the restart-on-crash property the
    // anchor-max fix must preserve: no fabricated `now`, no suppression).
    let gate = run_predicate(f.path(), &[("CHUG_ROUTINE_TODAY", FRESH_DAY)]);
    assert_eq!(
        verb_of(&gate), "launch",
        "a crashed audit (nothing landed) re-fires next cycle: {gate}"
    );
    assert_eq!(field_of(&gate, "dormant="), "audit", ": {gate}");
    // The audit LANDS: an `eval:` commit touches EVALUATION.md NOW. The
    // latch is deliberately LEFT at the stale ts (the F1 state).
    f.commit_tree(
        "eval: cycle-101 dormant weekly audit (T325): gates=green pins=23/23 site=fresh daemon=ok filings=0",
        None,
        Some("# eval — the audit's artifacts\n"),
    );
    assert!(
        f.anchor_ts() > f.latch_ts().expect("the stale latch is armed"),
        "the landing moved the anchor past the stale record (the F1 state)"
    );
    let gate = run_predicate(f.path(), &[("CHUG_ROUTINE_TODAY", FRESH_DAY)]);
    assert_eq!(
        verb_of(&gate), "skip",
        "cycle D+1: the landed audit stopped the duplicate — the storm ends \
         at zero extras: {gate}"
    );
    assert_eq!(
        field_of(&gate, "dormant="),
        "suspended-valve",
        "the quiet cycle names the suspension, not a fresh timer: {gate}"
    );
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

// ---------------------------------------------------------------- T361 ----
// The dormant release flush: the release channel must not freeze behind
// the T325 cheap exit. Dormancy suppresses EVAL cycles and the T100
// release recount happens inside EVAL cycles, so a due release waits
// forever once the queue drains. The flush is the RELEASE-ONLY mechanical
// cycle: when a dormant cycle's recount (the T269 method against the
// newest ANCESTOR tag) crossed 3, the gate flips the suspended-valve skip
// to `dormant=release-flush` (the verb/model stay the skip's — no model
// is picked, the flush is free) and the run mode cuts + pushes the tag —
// zero LLM calls. < 3 flips changes nothing; the audit cycle gains the
// recount as a rider (the flush runs BEFORE the audit child); an
// anchor-unsafe candidate (a dirty tracked tree) is a NAMED skip.

impl Fixture {
    /// Write a fixture file (any relative path, dirs created).
    fn write_file(&self, rel: &str, body: &str) {
        let p = self.path().join(rel);
        if let Some(dir) = p.parent() {
            std::fs::create_dir_all(dir).expect("fixture file dir");
        }
        std::fs::write(p, body).expect("write fixture file");
    }

    /// A bare origin to receive the flush's push. Created AFTER every
    /// commit on purpose — a later fixture `git add -A` would sweep the
    /// bare repo's files into a commit, while an untracked dir never reads
    /// dirty to the flush's `--untracked-files=no` anchor-safety check.
    fn add_origin(&self) {
        git(self.path(), &["init", "-q", "--bare", "origin.git"]);
        git(self.path(), &["remote", "add", "origin", "origin.git"]);
    }
}

/// The T361 flush fixture: the dormant battery's shape (the timer stale on
/// BOTH halves, `streak` wrap-notes NEWEST — the flush commits land BEFORE
/// the wraps, or the streak walk would reset at the first non-eval commit
/// and read 0) PLUS a release channel on the books — Cargo.toml/Cargo.lock
/// at v0.25.0, the pairing/notes scripts, an annotated v0.25.0 tag, one
/// `t<N>:` feature impl and `n_flips` distinct `todo:` flip commits (whose
/// trailing prose carries a stray T-token — the T269 leading-anchor keeps
/// it out of the count), and (by the caller) a bare origin. All commits
/// land BEFORE `add_origin()`.
fn flush_fixture(streak: usize, age_secs: u64, n_flips: usize) -> Fixture {
    let f = fixture();
    f.backdate_anchor(age_secs);
    f.write_file(
        "Cargo.toml",
        "[package]\nname = \"chug\"\nversion = \"0.25.0\"\nedition = \"2024\"\n",
    );
    f.write_file(
        "Cargo.lock",
        "version = 4\n\n[[package]]\nname = \"chug\"\nversion = \"0.25.0\"\ndependencies = [\n]\n",
    );
    for s in ["check-tag-version.sh", "release-notes.sh"] {
        std::fs::copy(repo_root().join("scripts").join(s), f.path().join("scripts").join(s))
            .unwrap_or_else(|e| panic!("copy {s} into the fixture: {e}"));
    }
    f.commit_empty("chore: baseline the release channel (T361 fixture)");
    git(f.path(), &["tag", "-a", "v0.25.0", "-m", "v0.25.0"]);
    f.commit_empty("t359: implement the dormant wake ledger (T361 fixture)");
    // T359, T328, T297 — three DISTINCT row ids (the count is distinct
    // ids, not commits), each flip's tail riding a stray `T361` token the
    // T269 anchor must keep out of the extraction.
    for i in 0..n_flips {
        let id = 359 - i * 31;
        f.commit_empty(&format!(
            "todo: T{id} done (merge deadbe{i}) — the T361 flush fixture flip {i}"
        ));
    }
    f.wraps(streak);
    f.latch(age_secs);
    f
}

/// A dormant cycle whose recount crossed 3 is the RELEASE-ONLY cycle: the
/// gate keeps the skip's verb and no-model placeholder, names
/// `dormant=release-flush`, and carries the recount on the line.
#[test]
fn a_dormant_cycle_with_three_distinct_flips_names_the_flush() {
    let f = flush_fixture(4, 2 * DAY, 3);
    let gate = run_predicate(f.path(), &[("CHUG_ROUTINE_TODAY", FRESH_DAY)]);
    assert_eq!(verb_of(&gate), "skip", "the flush rides the skip class: {gate}");
    assert_eq!(model_of(&gate), "-", "the flush is mechanical — no model is picked: {gate}");
    assert_eq!(
        field_of(&gate, "dormant="),
        "release-flush",
        "the decision names the flush cycle (T361): {gate}"
    );
    assert_eq!(field_of(&gate, "flips="), "3", "the recount rides the line: {gate}");
    assert!(gate.contains("laya=none"), "the flush never consults the triage: {gate}");
}

/// The wake outranks the flush: an OPEN row is real queue work — the loop
/// exits dormancy FIRST and the recount is never consulted (a flush must
/// never fire on top of an awake cycle).
#[test]
fn an_open_row_exits_dormancy_before_any_flush() {
    let f = flush_fixture(4, 2 * DAY, 3);
    f.write_file("TODO.md", TODO_ONE_OPEN);
    f.commit_empty("eval: cycle-110 wake — the operator flipped a row open (T361 fixture)");
    let gate = run_predicate(f.path(), &[("CHUG_ROUTINE_TODAY", FRESH_DAY)]);
    assert_eq!(verb_of(&gate), "launch", "the wake fires before any flush: {gate}");
    assert_eq!(field_of(&gate, "dormant="), "no", "the loop is awake: {gate}");
    assert_eq!(field_of(&gate, "rows="), "1", "the open row is the queue: {gate}");
    assert_eq!(field_of(&gate, "flips="), "0", "the recount is a dormant-arm input only: {gate}");
}

/// Below the trigger the rule is byte-identical to T325: two distinct
/// flips stay a plain suspended-valve cheap exit — no tag, no flush line,
/// the normal disposition, zero model calls.
#[test]
fn two_flips_stay_a_plain_suspended_valve_cheap_exit() {
    let f = flush_fixture(4, 2 * DAY, 2);
    let gate = run_predicate(f.path(), &[("CHUG_ROUTINE_TODAY", FRESH_DAY)]);
    assert_eq!(field_of(&gate, "dormant="), "suspended-valve", "< 3 flips: the rule is unchanged: {gate}");
    assert_eq!(field_of(&gate, "flips="), "2", "the recount still rides the line: {gate}");
    let child_env = f.install_run_stubs();
    f.run_one_pass();
    let log = f.fixture_log();
    assert!(
        !git_out(f.path(), &["tag", "-l"]).contains("v0.26.0"),
        "no tag below the trigger"
    );
    assert!(!log.contains("dormant release flush (T361)"), "no flush below the trigger: {log}");
    assert!(
        log.contains("dormant cheap-exit (T325): streak=4 suspended-valve"),
        "the normal dormant cheap exit stands: {log}"
    );
    assert!(!child_env.exists(), "zero model calls on the quiet cycle");
}

/// The flush end to end: dormant + 3 distinct flips → the tag is cut AND
/// pushed (branch + tag together), the release commit bumps manifest AND
/// lock (minor: the range carries a `t<N>:` feature), the log line names
/// the tag and the three flips in the spec's shape — and ZERO model calls
/// (the stub child never ran, no cycle ever started).
#[test]
fn the_flush_cuts_and_pushes_the_release_tag_with_zero_model_calls() {
    let f = flush_fixture(4, 2 * DAY, 3);
    f.add_origin();
    let child_env = f.install_run_stubs();
    f.run_one_pass();
    let log = f.fixture_log();
    assert!(
        git_out(f.path(), &["tag", "-l", "v0.26.0"]).contains("v0.26.0"),
        "the tag was cut at HEAD: {log}"
    );
    let remote = git_out(f.path(), &["ls-remote", "origin"]);
    assert!(
        remote.contains("refs/tags/v0.26.0"),
        "the tag was PUSHED to the origin: {remote}"
    );
    assert_eq!(
        git_out(f.path(), &["log", "-1", "--format=%s"]).trim(),
        "chore: release v0.26.0",
        "the release commit is HEAD (the T100 subject shape): {log}"
    );
    let toml = std::fs::read_to_string(f.path().join("Cargo.toml")).expect("read manifest");    assert!(
        toml.contains("version = \"0.26.0\""),
        "the manifest was bumped (minor: the range has a feature impl): {toml}"
    );
    let lock = std::fs::read_to_string(f.path().join("Cargo.lock")).expect("read lock");
    let chug = lock.split("name = \"chug\"").nth(1).expect("the chug block");
    assert!(
        chug.starts_with("\nversion = \"0.26.0\""),
        "the lock's chug block was bumped in lockstep (--locked builds stay green): {lock}"
    );
    assert!(
        log.contains("dormant release flush (T361): v0.26.0 (3 flips: T297, T328, T359)"),
        "the spec's flush line: tag + count + the distinct ids (sorted, the stray tail T-token kept out): {log}"
    );
    assert!(
        !log.contains("dormant release flush (T361): declined"),
        "nothing declined the flush: {log}"
    );
    assert!(!child_env.exists(), "ZERO model calls — the stub child never ran");
    assert!(!log.contains("cycle start ->"), "no eval cycle ever started: {log}");
    // The NEXT cycle must not re-fire: the recount anchors on the new tag.
    let gate = run_predicate(f.path(), &[("CHUG_ROUTINE_TODAY", FRESH_DAY)]);
    assert_eq!(
        field_of(&gate, "dormant="),
        "suspended-valve",
        "the channel is flushed — the recount reads 0 against v0.26.0: {gate}"
    );
    assert_eq!(field_of(&gate, "flips="), "0", "no flips since the new tag: {gate}");
}

/// ANCHOR SAFETY: a dirty tracked tree means the tag candidate (the
/// release commit the flush is about to cut) would differ from HEAD's
/// tree by more than the version bump — the flush declines with a NAMED
/// skip, cuts nothing, and the normal dormant disposition records the
/// refusal in the git record.
#[test]
fn a_dirty_tree_declines_the_flush_with_a_named_skip() {
    let f = flush_fixture(4, 2 * DAY, 3);
    f.add_origin();
    // Dirty a TRACKED file (uncommitted) — the anchor-unsafe candidate.
    f.write_file("TODO.md", &format!("{TODO_ONE_DONE}<!-- T361 dirty candidate -->\n"));
    let child_env = f.install_run_stubs();
    f.run_one_pass();
    let log = f.fixture_log();
    assert!(
        log.contains(
            "dormant release flush (T361): declined — dirty tree (HEAD's tree differs from the tag candidate's beyond the version bump)"
        ),
        "the named skip: {log}"
    );
    assert!(
        !git_out(f.path(), &["tag", "-l", "v0.26.0"]).contains("v0.26.0"),
        "no tag on an unsafe candidate"
    );
    assert!(
        !git_out(f.path(), &["ls-remote", "origin"]).contains("v0.26.0"),
        "nothing was pushed"
    );
    assert!(
        git_out(f.path(), &["log", "-1", "--format=%s"]).contains("T361 flush declined: dirty-tree"),
        "the disposition names the refusal in the git record: {log}"
    );
    assert!(!child_env.exists(), "the declined cycle is still zero-model");
}

/// THE T361 UNDO REGRESSION — a FAILED PUSH must UNDO the local cut, tag
/// delete included. An unpushed tag is not a published release, and a
/// swallowed local cut would read 0 on the next recount and refreeze the
/// release channel the flush exists to unstick — so the undo's DELETE leg
/// is load-bearing exactly like the reset. The fixture's origin is
/// unreachable (the remote points at a path that does not exist), so the
/// cut + pairing check + notes + tag all succeed and the PUSH fails: the
/// tag must be DELETED (a surviving tag orphans the cut — describe only
/// walks ancestors, so the phantom tag hides from the recount while the
/// channel stays frozen), HEAD reset off the release commit, the manifest
/// back at the pre-cut version, the disposition naming push-failed in the
/// git record, and the recount STILL 3 on the next cycle (the flush
/// re-fires) — all at zero model calls.
#[test]
fn a_failed_push_undoes_the_local_cut_so_the_recount_stays_live() {
    let f = flush_fixture(4, 2 * DAY, 3);
    // The unreachable origin: a remote named origin at a path that does
    // not exist — everything before the push succeeds, the push cannot.
    git(f.path(), &["remote", "add", "origin", "no-such-origin.git"]);
    let pre = git_out(f.path(), &["rev-parse", "HEAD"]);
    let child_env = f.install_run_stubs();
    f.run_one_pass();
    let log = f.fixture_log();
    assert!(
        log.contains("dormant release flush (T361): declined — push failed"),
        "the push failure is a NAMED refusal (not a swallowed one): {log}"
    );
    assert!(
        !git_out(f.path(), &["tag", "-l"]).contains("v0.26.0"),
        "the unpushed local tag is DELETED — a surviving tag is an unpublished \
         release squatting on the name (never re-tag) while the recount reads \
         past it: {log}"
    );
    let landed = git_out(f.path(), &["log", "--format=%s", &format!("{pre}..HEAD")]);
    assert!(
        !landed.contains("chore: release v0.26.0"),
        "HEAD was reset off the release commit — the cut is undone: {landed}"
    );
    let toml = std::fs::read_to_string(f.path().join("Cargo.toml")).expect("read manifest");
    assert!(
        toml.contains("version = \"0.25.0\""),
        "the manifest bump rolled back with the reset (the tree is pre-cut): {toml}"
    );
    let subject = git_out(f.path(), &["log", "-1", "--format=%s"]);
    assert!(
        subject.contains("T361 flush declined: push-failed"),
        "the disposition names the refusal in the git record: {subject}"
    );
    assert!(
        !child_env.exists() && !log.contains("cycle start ->"),
        "the declined flush stays zero-model: {log}"
    );
    // The load-bearing half: the recount is still LIVE — the next cycle
    // re-fires the flush instead of reading 0 against a phantom tag and
    // refreezing the channel.
    let gate = run_predicate(f.path(), &[("CHUG_ROUTINE_TODAY", FRESH_DAY)]);
    assert_eq!(field_of(&gate, "dormant="), "release-flush", "the channel re-fires: {gate}");
    assert_eq!(field_of(&gate, "flips="), "3", "the recount stays at 3: {gate}");
}

/// The weekly audit gains the recount (belt and suspenders): a due audit
/// on a flush-eligible queue stays a launch (dormant=audit, the full
/// orchestrator id) but carries flips=3 — the run mode flushes BEFORE the
/// audit child launches.
#[test]
fn the_audit_cycle_gains_the_flush_recount() {
    let f = flush_fixture(4, 8 * DAY, 3);
    let gate = run_predicate(f.path(), &[("CHUG_ROUTINE_TODAY", FRESH_DAY)]);
    assert_eq!(verb_of(&gate), "launch", "the audit keeps its launch: {gate}");
    assert_eq!(model_of(&gate), KIMI_ID, "the audit is a real eval: {gate}");
    assert_eq!(field_of(&gate, "dormant="), "audit", "the audit class stands: {gate}");
    assert_eq!(field_of(&gate, "flips="), "3", "the recount rides the audit line too: {gate}");
}

/// THE T361 AUDIT-RIDER REGRESSION — the rider's threshold is BEHAVIORAL,
/// not just a field on the gate line. A due audit on a flush-eligible
/// queue (flips=3, the T245 trigger) cuts + pushes the tag BEFORE the
/// read-only child launches: the flush line precedes the audit line in
/// the log, the tag is on the origin, and the audit child still runs with
/// DORMANT=1 (a declined flush never blocks the audit — independent
/// legs). A threshold drift past 3 would leave the channel frozen behind
/// the audit — the belt-and-suspenders leg silently gone.
#[test]
fn the_audit_rider_flushes_before_the_audit_child_at_three_flips() {
    let f = flush_fixture(4, 8 * DAY, 3);
    f.add_origin();
    let child_env = f.install_run_stubs();
    f.run_one_pass();
    let log = f.fixture_log();
    assert!(
        git_out(f.path(), &["tag", "-l", "v0.26.0"]).contains("v0.26.0"),
        "the rider flushed the tag on the audit cycle: {log}"
    );
    let remote = git_out(f.path(), &["ls-remote", "origin"]);
    assert!(
        remote.contains("refs/tags/v0.26.0"),
        "the rider's tag was pushed: {remote}"
    );
    let flush_at = log
        .find("dormant release flush (T361): v0.26.0")
        .expect("the flush line is in the log");
    let audit_at = log
        .find("dormant weekly audit (T325): due")
        .expect("the audit line is in the log");
    assert!(
        flush_at < audit_at,
        "the flush runs BEFORE the audit child (belt and suspenders): {log}"
    );
    let captured = std::fs::read_to_string(&child_env).expect("the stub child ran");
    assert_eq!(
        captured.trim(),
        "DORMANT=1",
        "a flushed audit cycle is still an audit — the read-only child launches: {captured}"
    );
    // The flush consumed the recount: the next decision reads 0 against
    // the new tag (the timer may still be due — the stub audit lands no
    // eval: commit — but the channel is flushed either way).
    let gate = run_predicate(f.path(), &[("CHUG_ROUTINE_TODAY", FRESH_DAY)]);
    assert_eq!(field_of(&gate, "flips="), "0", "the recount anchored on the new tag: {gate}");
}

/// The rider's threshold holds from below too: two flips on a due audit —
/// the audit launches, the flush does not (the T245 rule is untouched on
/// the audit arm exactly as on the suspended-valve arm).
#[test]
fn the_audit_rider_stays_below_the_trigger() {
    let f = flush_fixture(4, 8 * DAY, 2);
    f.add_origin();
    f.install_run_stubs();
    f.run_one_pass();
    let log = f.fixture_log();
    assert!(
        !git_out(f.path(), &["tag", "-l"]).contains("v0.26.0"),
        "no flush below the trigger on an audit cycle: {log}"
    );
    assert!(
        !log.contains("dormant release flush (T361)"),
        "no flush line below the trigger: {log}"
    );
    assert!(
        log.contains("dormant weekly audit (T325): due"),
        "the audit still launched: {log}"
    );
}

/// The static wiring: the recount is defined once and consulted once in
/// the gate (inside the dormant arm, after the timer, before the echo);
/// the flush conversion rides the suspended-valve assignment (the < 3
/// path byte-identical); the run-mode leg precedes the T325 echo and the
/// audit rider precedes the audit launch; the flush is cargo-free and
/// never touches the latch; the undo + never-re-tag guards are on the
/// books; the declined disposition names the refusal.
#[test]
fn the_flush_wiring_is_pinned_in_loopd() {
    let loopd = read("loopd.sh");
    count_eq(&loopd, "release_flip_count() {", 1, "the recount, defined once");
    count_eq(&loopd, "flips=$(release_flip_count)", 1, "the gate's one consult");
    count_eq(
        &loopd,
        r"s/^todo: (file |fix )?(T[0-9]+( *[+,] *T[0-9]+)*).*/\2/",
        1,
        "the T269 leading-id-list anchor (the stray trailing-prose T-token stays out)",
    );
    count_eq(&loopd, "dormant=release-flush", 4, "the verdict: header + gate docs + probe docs + the gate's conversion");
    count_eq(
        &loopd,
        "verb=skip; model=-; dormant=suspended-valve",
        1,
        "the < 3 path byte-identical (the T325 assignment the conversion rides)",
    );
    // Ordering: the conversion sits AFTER the suspended-valve assignment
    // and BEFORE the gate echo; the recount after the timer read.
    let arm = loopd.find("dts=$(dormant_timer_ts)").expect("the timer read");
    let convert = loopd.find("dormant=release-flush\n").expect("the conversion");
    let echo = loopd.find("closed=$closed $laya dormant=$dormant flips=$flips").expect("the gate echo");
    assert!(arm < convert && convert < echo, "the recount converts inside the dormant arm, before the echo");
    // The run-mode leg: precedes the T325 echo, carries no chug run.
    let leg = loopd.find("= \"release-flush\" ] && dormant_release_flush; then").expect("the run-mode leg");
    let t325 = loopd.find("dormant cheap-exit (T325): streak=$gate_streak suspended-valve").expect("the T325 echo");
    assert!(leg < t325, "the flush leg precedes the dormant cheap-exit echo");
    let leg_body = &loopd[leg..t325];
    assert!(
        !leg_body.contains("chug run") && !leg_body.contains("./target/release/chug"),
        "the flush cycle launches NOTHING — the mechanical cycle is zero-model"
    );
    // The audit rider precedes the audit launch line.
    let rider = loopd.find("dormant_release_flush || true").expect("the audit rider");
    let audit_line = loopd.find("dormant weekly audit (T325): due").expect("the audit launch line");
    assert!(rider < audit_line, "the flush runs BEFORE the audit child (belt and suspenders)");
    // The flush body: no cargo invocation, no latch write, the guards on.
    let flush_at = loopd.find("dormant_release_flush() {").expect("the flush fn");
    let flush_body = &loopd[flush_at..loopd.find("\n# T326 — ideation mode").expect("the fn end")];
    assert!(
        !flush_body.contains("cargo build") && !flush_body.contains("cargo test") && !flush_body.contains("cargo check"),
        "the flush never runs cargo — the bump is a mechanical rewrite of both files"
    );
    assert!(
        !flush_body.contains("dormant_state_write"),
        "the flush never touches the dormancy latch (a release is not an eval)"
    );
    count_eq(&loopd, "already exists (never re-tag or move a tag)", 1, "an existing tag is never re-tagged");
    count_eq(&loopd, "t361_undo", 5, "the unpushed-cut undo: defined once, four refusal call sites");
    count_eq(&loopd, "git reset --hard \"$prev\"", 1, "the guarded reset (only when the release commit is HEAD)");
    count_eq(
        &loopd,
        "git tag -d \"$tag\"",
        1,
        "the undo's tag-DELETE leg — as load-bearing as the reset: a swallowed \
         delete leaves an unpushed tag squatting on the name while the next \
         recount reads past it (describe only walks ancestors) and the \
         channel refreezes",
    );
    count_eq(
        &loopd,
        "if [ \"$(laya_gate_field flips= \"$gate\")\" -ge 3 ]; then",
        1,
        "the audit rider's threshold IS the T245 trigger — one byte-pinned \
         condition, never drifted",
    );
    count_eq(
        &loopd,
        "dormant release flush (T361): $tagname ($count flips:",
        1,
        "the success line, named once",
    );
    count_eq(
        &loopd,
        "dirty tree (HEAD's tree differs from the tag candidate's beyond the version bump)",
        1,
        "the anchor-safety named skip (the refusal composer, named once)",
    );
    count_eq(
        &loopd,
        "(T361 flush declined: ${T361_FLUSH_DECLINE:-unknown} — the dormant cheap exit stands)",
        1,
        "the declined disposition names the refusal word",
    );
    count_eq(&loopd, "dormant release flush (T361): declined —", 1, "every refusal is a NAMED skip — one composer, per-reason words");
}
