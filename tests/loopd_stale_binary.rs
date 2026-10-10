//! T137 — a failed `cargo build` must never leave the supervisor launching
//! the PREVIOUS release binary (codex adversarial review,
//! reviews/CODEX-REVIEW-20260928.md §1 HIGH "Failed builds do not prevent
//! running an old binary").
//!
//! The bug: loopd.sh ran `cargo build --release >> "$LOG" 2>&1` and never
//! looked at its exit status. A merged change that fails compilation — with
//! a previous `target/release/chug` still on disk — sailed straight into the
//! cycle launch, so the supervisor kept "self-improving" on the stale
//! binary: its verdicts, its pushes, its site-sync stats all came from code
//! the merge never changed. The script also inherited an operator's
//! `CARGO_TARGET_DIR`: a *successful* build could land somewhere else
//! entirely while the launch still exec'd the fixed `./target/release/chug`
//! path — the stale binary again, even with a green build.
//!
//! The fix: `set -euo pipefail` semantics plus an explicit build GATE — the
//! build's rc is latched (`|| build_rc=$?`, the T142 house style), a nonzero
//! rc refuses the launch and counts toward the existing 3-strikes HALT, and
//! the supervisor's own build pins `CARGO_TARGET_DIR="$ROOT/target"` so the
//! launched path is always the one just built.
//!
//! These tests run the REAL loopd.sh in a sandbox against stub `cargo` and
//! stub cycle children (the tests/loopd_spoof_guard.rs pattern: the tests
//! guard the script, they do not reimplement it). The stubs play the
//! review's trigger exactly: a build that fails, and a build that succeeds
//! into an inherited CARGO_TARGET_DIR, both with a previous release binary
//! still installed at ./target/release/chug.

#![cfg(unix)]

use std::fs;
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use tempfile::TempDir;

// T172: the deadline-bearing sandbox tests in this file join THE
// cross-binary load-lock domain for this binary family (the flock harness in
// tests/support/load_lock.rs). A static Mutex is per-binary — nextest runs
// each TEST as its own PROCESS, so the T31/T151 static shape cannot see the
// cross-binary contention; a bounded advisory flock on a lockfile under the
// shared target dir can, under BOTH gate runners, with the kernel releasing
// a dead holder's lock. The pin test at the bottom enforces membership by
// construction.
#[path = "support/load_lock.rs"]
mod t172_load_lock;


fn repo_root() -> PathBuf {
    std::env::current_dir().expect("cargo sets the test cwd to the package root")
}

// ---------- T158: the loopd-fixture invalidation seam (the spoof_guard twin;
// the T59/T66/T151 Invalidation pattern). The fixture's 30s verdict deadline
// is a liveness fence, NOT a load assumption (T159's doctrine): under
// system-wide pressure (a concurrent cargo build, an 8x yes-spinner) the
// whole sandbox startup has stretched past it while production behavior
// stayed correct. So the deadline-blow panic is an INVALIDATION MARKER: the
// seam retries the WHOLE test (fresh sandbox, fresh supervisor) bounded at 3
// attempts; any other panic (a needle reached but a wrong outcome — a
// code-under-test failure) is resumed un-retried, byte-distinct. The 30s
// constant does not change; exhaustion panics naming the class and attempt
// count.
const LOOPD_INVALIDATION_MARKER: &str = "loopd never reached ";
const LOOPD_RETRY_ATTEMPTS: usize = 3;

fn loopd_attempt_with_invalidation_retry(mut attempt: impl FnMut()) {
    let mut last: Option<String> = None;
    for n in 1..=LOOPD_RETRY_ATTEMPTS {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(&mut attempt)) {
            Ok(()) => return,
            Err(payload) => {
                let message = payload
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| payload.downcast_ref::<&'static str>().map(|s| s.to_string()))
                    .unwrap_or_else(|| "<non-string panic payload>".to_string());
                if message.contains(LOOPD_INVALIDATION_MARKER) {
                    eprintln!(
                        "T158 loopd_attempt_with_invalidation_retry: attempt {n}/{LOOPD_RETRY_ATTEMPTS} invalidated (fixture deadline blow under load); retrying the WHOLE test with a fresh sandbox"
                    );
                    last = Some(message);
                } else {
                    std::panic::resume_unwind(payload);
                }
            }
        }
    }
    let message = last.expect("exhaustion implies a classified invalidation");
    panic!(
        "T158 loopd_attempt_with_invalidation_retry: the fixture deadline invalidation \
         persisted across all {LOOPD_RETRY_ATTEMPTS} attempts (each with a fresh sandbox \
         and supervisor). The environment invalidated the test's schedule premise every \
         time. Last red evidence: {message}"
    );
}


/// A sandbox with the real loopd.sh + scripts/, a PATH that stubs `ps`
/// (the T53 single-driver probe must not see a REAL driver — e.g. the
/// outer run executing this very test — or the sandbox loopd skips its
/// cycle) and `cargo` (the scenario installs its own build behavior), and
/// the PREVIOUS release binary still installed at ./target/release/chug.
struct Sandbox {
    // Holds the tempdir open for the test's lifetime; never read directly.
    _keep: TempDir,
    root: PathBuf,
}

/// The previous release binary, still installed where the supervisor
/// launches it. It announces what ran by writing its identity next to the
/// sandbox root — the observable the tests assert on.
const STALE_CHUG: &str = concat!(
    "#!/bin/sh\n",
    "printf stale > \"$PWD/stale-launched.txt\"\n",
    // Mimic the old binary happily running the cycle to completion: an
    // accepted exit (rc 0) with the goal-complete block on stdout.
    "printf 'chug: goal complete\\nsummary: the PREVIOUS release ran this cycle\\n'\n",
    "mkdir -p .chug && touch .chug/STOP-LOOP\n",
    "exit 0\n",
);

impl Sandbox {
    fn new(cargo_body: &str) -> Sandbox {
        Self::with_ps("#!/bin/sh\nexit 0\n", cargo_body)
    }

    /// `ps_body` plays the T53 single-driver probe. The default is a
    /// SUCCESSFUL probe reporting no processes — the realistic "no driver"
    /// answer (a ps rc of 0 with empty output). A failing probe is its own
    /// scenario: `a_failing_driver_probe_must_fail_closed_not_open`.
    fn with_ps(ps_body: &str, cargo_body: &str) -> Sandbox {
        let keep = tempfile::tempdir().expect("sandbox tempdir");
        let root = keep.path().to_path_buf();
        // The real script + its scripts/ helpers, byte-for-byte.
        fs::copy(repo_root().join("loopd.sh"), root.join("loopd.sh")).expect("copy loopd.sh");
        let scripts = root.join("scripts");
        fs::create_dir_all(&scripts).expect("scripts dir");
        for entry in fs::read_dir(repo_root().join("scripts")).expect("scripts dir") {
            let entry = entry.expect("scripts entry");
            // Regular files only (the loopd_orphan_reaper guard shape): a
            // `scripts/__pycache__/` (any python unittest import run in the
            // checkout — the T273 wrap_assert pair's own side effect) is a
            // DIRECTORY and fs::copy fails the whole sandbox on it.
            if entry.file_type().expect("scripts entry type").is_file() {
                fs::copy(entry.path(), scripts.join(entry.file_name())).expect("copy script");
            }
        }
        // PATH stubs: `ps` plays the scenario's single-driver probe (the
        // T53 enumeration — never a REAL driver, e.g. the outer run
        // executing this very test, or the sandbox loopd would skip its
        // cycle), `cargo` plays the scenario's build behavior.
        let bin = root.join("bin");
        fs::create_dir_all(&bin).expect("bin dir");
        stub(&bin.join("ps"), ps_body);
        stub(&bin.join("cargo"), cargo_body);
        // The PREVIOUS release binary, still installed at the launch path.
        let chug_dir = root.join("target/release");
        fs::create_dir_all(&chug_dir).expect("target/release dir");
        stub(&chug_dir.join("chug"), STALE_CHUG);
        // site-sync must NEVER leave the sandbox (a live ~/workspace/chug-site
        // on this host would get synced from fixture garbage): point the env
        // default at a path that does not exist (warn + exit 0 leg).
        fs::write(root.join("NO-SUCH-SITE"), "").expect("site sentinel");
        Sandbox { _keep: keep, root }
    }

    fn run_loopd(&self) -> Child {
        self.run_loopd_with_env(&[])
    }

    /// Spawn the sandbox supervisor with the harness's standard env plus the
    /// caller's overrides (a `Command::env` set AFTER the default wins — the
    /// explicit-env-wins shape the sleep seams pin).
    fn run_loopd_with_env(&self, extra_env: &[(&str, &str)]) -> Child {
        let mut cmd = Command::new("bash");
        cmd.arg("loopd.sh").arg("run").current_dir(&self.root);
        // Sandbox bin first: ps/cargo stubs shadow the host's.
        let path = format!(
            "{}:{}",
            self.root.join("bin").display(),
            std::env::var("PATH").unwrap_or_default()
        );
        cmd.env("PATH", path);
        cmd.env("CHUG_SITE_DIR", self.root.join("NO-SUCH-SITE"));
        cmd.env("CHUG_SITE_SYNC_NO_PUSH", "1");
        // The sleep seams (CHUG_ROUTINE_TODAY pattern): a failed verdict or
        // build would otherwise park the loop 60–300s. The harness kills the
        // loop at its verdict anyway; these only bound a missed kill.
        cmd.env("LOOPD_SLEEP_OK", "1");
        cmd.env("LOOPD_SLEEP_FAIL", "1");
        // T254 — the fixture-leak fail-safe: every spawn of `loopd.sh run`
        // exports LOOPD_MAX_LOOPS so the supervisor self-terminates if THIS
        // harness dies (an outer bounded cap killing a nextest leg mid-test,
        // a crash) and orphans the fixture to launchd — the cycle-168 leak
        // was exactly such a fixture, immortal-but-inert on its probe-fail
        // skip path. 50 sits comfortably above the observed iteration need
        // (every test here completes in ≤ a dozen fast iterations), so the
        // bound only ever fires on a leaked fixture, never on a live one.
        cmd.env("LOOPD_MAX_LOOPS", "50");
        for (k, v) in extra_env {
            cmd.env(k, v);
        }
        cmd.stdout(Stdio::null()).stderr(Stdio::null());
        cmd.spawn().expect("spawn bash loopd.sh run")
    }

    fn log(&self) -> PathBuf {
        self.root.join(".chug/loopd/loopd.log")
    }

    fn read_log(&self) -> String {
        let mut content = String::new();
        if let Ok(mut f) = fs::File::open(self.log()) {
            let _ = f.read_to_string(&mut content);
        }
        content
    }

    /// Poll the supervisor log until ANY of `needles` appears, then kill the
    /// loop and return the log. Verdict/refusal lines are written BEFORE the
    /// inter-cycle sleep, so seeing one means the decision for this build or
    /// cycle is final.
    fn wait_for_any(&self, child: &mut Child, needles: &[&str]) -> String {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let content = self.read_log();
            if needles.iter().any(|n| content.contains(n)) {
                let _ = child.kill();
                let _ = child.wait();
                return content;
            }
            if Instant::now() > deadline {
                let _ = child.kill();
                let _ = child.wait();
                let state = fs::read_dir(self.root.join(".chug/loopd"))
                    .map(|d| {
                        d.filter_map(|e| e.ok())
                            .map(|e| e.file_name().to_string_lossy().into_owned())
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_default();
                let stale = self.root.join("stale-launched.txt").exists();
                panic!(
                    "loopd never reached any of {needles:?} in 30s (stale binary launched: {stale}).\n\
                     --- loopd.log ---\n{content}\n--- .chug/loopd: {state}"
                );
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    /// The cycle logs the supervisor created, if any (the gate must refuse a
    /// cycle entirely — no cycle log may exist after a failed build).
    fn cycle_logs(&self) -> Vec<PathBuf> {
        fs::read_dir(self.root.join(".chug/loopd"))
            .map(|d| {
                d.filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| {
                        p.file_name()
                            .map(|n| n.to_string_lossy().starts_with("cycle-"))
                            .unwrap_or(false)
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

fn stub(path: &Path, body: &str) {
    fs::write(path, body).expect("write stub");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("chmod stub");
}

/// The review's trigger verbatim: "A merged change fails compilation while a
/// previous `target/release/chug` exists." The build fails; the supervisor
/// must REFUSE to launch the cycle on the stale binary that is still
/// installed — log the refusal, count the failure, retry later. (Pre-fix:
/// the failed build was ignored and the PREVIOUS release ran the cycle to a
/// recorded `cycle OK`.)
#[test]
fn failed_build_must_not_launch_the_stale_binary() {
    // T172: FIRST acquisition — hold the cross-binary load lock across
    // the whole invalidation-retry span (all attempts) and the sandbox
    // spawn → assertion → cleanup, so sibling sandbox tests (own
    // processes under nextest) can no longer manufacture scheduler
    // stretch inside this test's clocked verdict window.
    let _t172_load = t172_load_lock::family_guard("loopd-stale-binary");
    // T158: the fixture's 30s verdict deadline is a liveness fence, not a load
    // assumption — a deadline blow invalidates the attempt and the WHOLE test
    // retries with a fresh sandbox (bounded); a wrong verdict still panics
    // un-retried.
    loopd_attempt_with_invalidation_retry(|| {
    let sandbox = Sandbox::new(concat!(
        "#!/bin/sh\n",
        "echo 'error[E0432]: unresolved import — the merged change does not compile' >&2\n",
        "exit 101\n",
    ));
    let mut child = sandbox.run_loopd();
    // Either the gate refuses (good) or the stale binary "completes" the
    // cycle (the bug) — first needle wins, so the RED leg fails fast with
    // the supervisor's own words instead of a 30s deadline panic.
    let log = sandbox.wait_for_any(&mut child, &["refusing to launch", "cycle OK:"]);
    assert!(
        log.contains("build FAILED") && log.contains("refusing to launch"),
        "a failed `cargo build` must be logged as a refusal before any cycle \
         starts — the supervisor must not silently ignore the build's exit \
         status:\n{log}"
    );
    assert!(
        !sandbox.root.join("stale-launched.txt").exists(),
        "a failed build must NOT launch the cycle — ./target/release/chug is \
         the PREVIOUS release binary and running it would record verdicts, \
         push, and sync site stats from code the merge never changed:\n{log}"
    );
    assert!(
        sandbox.cycle_logs().is_empty(),
        "a failed build must refuse the cycle ENTIRELY — no cycle log may \
         exist for a cycle that never launched: {:?}",
        sandbox.cycle_logs()
    );
    });
}

/// The review's second leg: "An inherited `CARGO_TARGET_DIR` can also send a
/// successful build elsewhere while the supervisor still launches the fixed
/// `./target/release/chug` path." The operator's env points the build at a
/// different dir; cargo (the real one) honors it, so the supervisor's
/// "successful" build lands there and ./target/release/chug is STILL the
/// previous release. The supervisor must pin its own build to
/// CARGO_TARGET_DIR="$ROOT/target" so the path it launches is the one it
/// just built. (Pre-fix: the fresh binary sits unused in the inherited dir
/// while the stale one runs the cycle.)
#[test]
fn inherited_cargo_target_dir_cannot_leave_the_stale_binary_running() {
    // T172: FIRST acquisition — hold the cross-binary load lock across
    // the whole invalidation-retry span (all attempts) and the sandbox
    // spawn → assertion → cleanup, so sibling sandbox tests (own
    // processes under nextest) can no longer manufacture scheduler
    // stretch inside this test's clocked verdict window.
    let _t172_load = t172_load_lock::family_guard("loopd-stale-binary");
    // T158: the fixture's 30s verdict deadline is a liveness fence, not a load
    // assumption — a deadline blow invalidates the attempt and the WHOLE test
    // retries with a fresh sandbox (bounded); a wrong verdict still panics
    // un-retried.
    loopd_attempt_with_invalidation_retry(|| {
    // A green build that installs the NEW binary into whatever
    // CARGO_TARGET_DIR it is handed — never into ./target by accident: the
    // stub does not know the supervisor's layout, it only honors the var.
    let sandbox = Sandbox::new(
        r#"#!/bin/sh
set -e
[ -n "$CARGO_TARGET_DIR" ] || { echo 'stub cargo: CARGO_TARGET_DIR unset' >&2; exit 1; }
mkdir -p "$CARGO_TARGET_DIR/release"
cat > "$CARGO_TARGET_DIR/release/chug" <<'STUB'
#!/bin/sh
printf fresh > "$PWD/stale-launched.txt"
printf 'chug: goal complete\nsummary: the freshly built binary ran this cycle\n'
mkdir -p .chug && touch .chug/STOP-LOOP
exit 0
STUB
chmod +x "$CARGO_TARGET_DIR/release/chug"
exit 0
"#,
    );
    // The operator's inherited env: builds go ELSEWHERE.
    let inherited = sandbox.root.join("inherited-target");
    let mut child = {
        let mut cmd = Command::new("bash");
        cmd.arg("loopd.sh").arg("run").current_dir(&sandbox.root);
        let path = format!(
            "{}:{}",
            sandbox.root.join("bin").display(),
            std::env::var("PATH").unwrap_or_default()
        );
        cmd.env("PATH", path);
        cmd.env("CHUG_SITE_DIR", sandbox.root.join("NO-SUCH-SITE"));
        cmd.env("CHUG_SITE_SYNC_NO_PUSH", "1");
        cmd.env("LOOPD_SLEEP_OK", "1");
        cmd.env("LOOPD_SLEEP_FAIL", "1");
        // T254 — the fixture-leak fail-safe (the run_loopd_with_env seam):
        // this test builds its own Command, so the knob export rides here
        // too — every `loopd.sh run` spawn site carries the bound.
        cmd.env("LOOPD_MAX_LOOPS", "50");
        cmd.env("CARGO_TARGET_DIR", &inherited);
        cmd.stdout(Stdio::null()).stderr(Stdio::null());
        cmd.spawn().expect("spawn bash loopd.sh run")
    };
    // Both scenarios end in a recorded cycle; what differs is WHO ran it.
    sandbox.wait_for_any(&mut child, &["cycle OK:", "cycle ended WITHOUT goal complete"]);
    let who = fs::read_to_string(sandbox.root.join("stale-launched.txt"))
        .unwrap_or_else(|_| "NOTHING — no cycle ran".to_string());
    assert_eq!(
        who.trim(),
        "fresh",
        "with an inherited CARGO_TARGET_DIR the supervisor's build must be \
         pinned to $ROOT/target so ./target/release/chug is the binary just \
         built — a green build that lands in the inherited dir while the \
         launch execs the fixed path runs the PREVIOUS release (who ran it: \
         {who:?})"
    );
    // And the pin must be a per-invocation prefix, not an export — pinned
    // statically below; the inherited var itself stays a decoy here.
    });
}

/// The gate must count its refusals: 3 consecutive failed builds trip the
/// supervisor's existing 3-strikes HALT (a broken tree must page the
/// operator, not loop refusals forever — the same contract a failed CYCLE
/// already has). Uses the LOOPD_SLEEP_FAIL seam so the three strikes take
/// seconds, not the production 3×300s.
#[test]
fn three_consecutive_failed_builds_halt_the_supervisor() {
    // T172: FIRST acquisition — hold the cross-binary load lock across
    // the whole invalidation-retry span (all attempts) and the sandbox
    // spawn → assertion → cleanup, so sibling sandbox tests (own
    // processes under nextest) can no longer manufacture scheduler
    // stretch inside this test's clocked verdict window.
    let _t172_load = t172_load_lock::family_guard("loopd-stale-binary");
    // T158: the fixture's 30s verdict deadline is a liveness fence, not a load
    // assumption — a deadline blow invalidates the attempt and the WHOLE test
    // retries with a fresh sandbox (bounded); a wrong verdict still panics
    // un-retried.
    loopd_attempt_with_invalidation_retry(|| {
    let sandbox = Sandbox::new(concat!(
        "#!/bin/sh\n",
        "echo 'error: could not compile chug' >&2\n",
        "exit 101\n",
    ));
    let mut child = sandbox.run_loopd();
    let log = sandbox.wait_for_any(&mut child, &["HALTED"]);
    assert!(
        !sandbox.root.join("stale-launched.txt").exists(),
        "even on the road to HALT no cycle may run on the stale binary:\n{log}"
    );
    let halted = fs::read_to_string(sandbox.root.join(".chug/loopd/HALTED"))
        .unwrap_or_else(|_| String::new());
    assert!(
        halted.contains("build"),
        "the HALT marker must name build failures as the reason — an operator \
         reading .chug/loopd/HALTED must learn the tree does not compile, not \
         that cycles failed:\n{halted:?}"
    );
    let refusals = log.lines().filter(|l| l.contains("refusing to launch")).count();
    assert_eq!(
        refusals, 3,
        "the HALT must fire on the THIRD consecutive refused build — two \
         refusals mean the guard tripped early, four mean it never fired:\n{log}"
    );
    });
}

/// Validator F1 (T137 fix-up) — the single-driver probe must survive
/// EARLY-MATCH-FLIPPED-FALSE. Pre-fix the probe was a ps-to-grep PIPELINE:
/// under `set -o pipefail`, `grep -q` exits at the FIRST match while the
/// still-writing ps leg keeps filling the pipe, gets SIGPIPE, and pipefail
/// makes that 141 the pipeline's rc — so a LIVE driver read as "no driver"
/// and a second driver launched (T135's flock only degrades-with-warning).
/// The validator proved the flip with real 83KB ps output on this host. The
/// fix greps the CAPTURED listing: no pipe, no SIGPIPE leg, the match alone
/// decides.
///
/// The stub ps prints the driver argv FIRST (grep -q exits HERE) and then
/// ~2.4MB of filler — far beyond any pipe buffer — so the writer must
/// SIGPIPE after the early exit. The writer must BE the probe process
/// (`exec awk`, exactly one process like the real ps): a wrapper script
/// that continues after its writer child dies would swallow the SIGPIPE
/// into its own `exit 0` and never flip. Pre-fix this flips the guard OPEN
/// and the stale binary "completes" the cycle; post-fix the guard must log
/// the active driver and skip.
#[test]
fn an_early_driver_match_must_survive_a_sigpoled_ps_leg() {
    // T172: FIRST acquisition — hold the cross-binary load lock across
    // the whole invalidation-retry span (all attempts) and the sandbox
    // spawn → assertion → cleanup, so sibling sandbox tests (own
    // processes under nextest) can no longer manufacture scheduler
    // stretch inside this test's clocked verdict window.
    let _t172_load = t172_load_lock::family_guard("loopd-stale-binary");
    // T158: the fixture's 30s verdict deadline is a liveness fence, not a load
    // assumption — a deadline blow invalidates the attempt and the WHOLE test
    // retries with a fresh sandbox (bounded); a wrong verdict still panics
    // un-retried.
    loopd_attempt_with_invalidation_retry(|| {
    let ps_body = concat!(
        "#!/bin/sh\n",
        // The live driver: the needle sits on line 1, so `grep -q` exits at
        // the very first read — the pre-condition of the flip. `exec` makes
        // awk the probe process itself, so when grep -q exits and awk's
        // blocked write fails, the SIGPIPE death (rc 141) is the probe
        // leg's rc — mimicking the single-process real ps.
        "exec awk 'BEGIN { printf \"%s\\n\", \
         \"/Users/op/chug/target/release/chug run --spec LOOP-SPEC.md --goal live-driver\";",
        " for (i = 0; i < 40000; i++)",
        " printf \"filler %05d xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\\n\", i }'\n",
    );
    // A GREEN build: the RED signal pre-fix is the LAUNCHED cycle, not a
    // build refusal — the flip defeats the driver guard, the build gate is
    // irrelevant here.
    let sandbox = Sandbox::with_ps(ps_body, "#!/bin/sh\nexit 0\n");
    let mut child = sandbox.run_loopd();
    // Either the guard skips (good) or the cycle launches (the bug) — first
    // needle wins, so the RED leg fails fast with the supervisor's own words.
    let log = sandbox.wait_for_any(&mut child, &["another LOOP-SPEC driver active", "cycle OK:"]);
    assert!(
        log.contains("another LOOP-SPEC driver active; skipping"),
        "a live driver must be reported as ACTIVE even when the process \
         listing dwarfs a pipe buffer — the pre-fix probe pipeline flipped \
         FAIL-OPEN under pipefail (grep -q's early exit SIGPIPEs the ps leg; \
         the 141 became the pipeline rc and a real match read as 'no \
         driver'):\n{log}"
    );
    assert!(
        !sandbox.root.join("stale-launched.txt").exists(),
        "the guard must SKIP the cycle while a driver is active — no second \
         driver may launch (T135's flock only degrades-with-warning):\n{log}"
    );
    assert!(
        sandbox.cycle_logs().is_empty(),
        "no cycle may be logged while a driver is active: {:?}",
        sandbox.cycle_logs()
    );
    });
}

/// The sweep's other leg (T137 fix-up): the probe FAILING must fail CLOSED.
/// Pre-fix the pipeline's rc came from grep alone, so an erroring ps leg was
/// silently ignored and the guard proceeded on an UNKNOWN enumeration —
/// fail-open. A post-fix bare capture would be equally wrong from the other
/// side: under `set -e` it kills the whole supervisor the first time ps
/// hiccups. The latched shape treats an unknown enumeration as "assume a
/// driver": skip, say why, retry.
#[test]
fn a_failing_driver_probe_must_fail_closed_not_open() {
    // T172: FIRST acquisition — hold the cross-binary load lock across
    // the whole invalidation-retry span (all attempts) and the sandbox
    // spawn → assertion → cleanup, so sibling sandbox tests (own
    // processes under nextest) can no longer manufacture scheduler
    // stretch inside this test's clocked verdict window.
    let _t172_load = t172_load_lock::family_guard("loopd-stale-binary");
    // T158: the fixture's 30s verdict deadline is a liveness fence, not a load
    // assumption — a deadline blow invalidates the attempt and the WHOLE test
    // retries with a fresh sandbox (bounded); a wrong verdict still panics
    // un-retried.
    loopd_attempt_with_invalidation_retry(|| {
    let sandbox = Sandbox::with_ps(
        "#!/bin/sh\necho 'ps: stub boom' >&2\nexit 7\n",
        "#!/bin/sh\nexit 0\n",
    );
    let mut child = sandbox.run_loopd();
    // Either the probe failure is latched (good) or the cycle launches (the
    // fail-open bug) — first needle wins.
    let log = sandbox.wait_for_any(&mut child, &["driver probe FAILED", "cycle OK:"]);
    assert!(
        log.contains("driver probe FAILED"),
        "a failing ps probe must be latched and logged — an UNKNOWN \
         enumeration must fail CLOSED (skip the cycle), not sail into a \
         launch and not kill the supervisor under set -e:\n{log}"
    );
    assert!(
        !sandbox.root.join("stale-launched.txt").exists(),
        "a failing probe must NOT launch a cycle — the enumeration is \
         unknown and a duplicate driver is the one unrecoverable \
         outcome:\n{log}"
    );
    assert!(
        sandbox.cycle_logs().is_empty(),
        "no cycle may be logged when the driver enumeration failed: {:?}",
        sandbox.cycle_logs()
    );
    });
}

/// The cycle child for the T254 bound test: completes a cycle (rc 0, the
/// goal-complete block on stdout — the shape the verdict gate requires for
/// a recorded `cycle OK`) but NEVER touches STOP-LOOP — the leak shape,
/// where nothing but the iteration bound ends the loop.
const NON_STOPPING_CHUG: &str = concat!(
    "#!/bin/sh\n",
    "printf 'chug: goal complete\\nsummary: a cycle that never stops the loop\\n'\n",
    "exit 0\n",
);

/// T254 — the fixture-leak fail-safe, behaviorally. Launched with
/// `LOOPD_MAX_LOOPS=3` the supervisor runs exactly 3 fast cycles and then
/// exits ON ITS OWN — never killed by the harness — logging one line naming
/// the knob, with exit status 0 and the pidfile removed (the EXIT trap's
/// record of a clean self-termination). The cycle child here never touches
/// STOP-LOOP, so nothing but the bound ends the loop: the leak shape of the
/// cycle-168 indictment (a `bash loopd.sh run` orphaned to launchd for six
/// days, immortal-but-inert on its probe-fail skip path — the skip paths
/// `continue` past any bottom-of-body counter, so the bound must count at
/// the TOP of the body).
///
/// Mutation leg (the RED proof): with the bound check removed from loopd.sh
/// the loop never ends, so the poll's OVER-RUN detector — a FOURTH recorded
/// `cycle OK:` inside the window — fails fast in seconds, un-retried (a
/// wrong outcome is a code-under-test failure, not a fixture stretch). The
/// 30s deadline is only the hang backstop and carries the T158 invalidation
/// marker like every deadline in this file.
#[test]
fn a_leaked_supervisor_self_terminates_at_loopd_max_loops() {
    // T172: FIRST acquisition — hold the cross-binary load lock across
    // the whole invalidation-retry span (all attempts) and the sandbox
    // spawn → assertion → cleanup, so sibling sandbox tests (own
    // processes under nextest) can no longer manufacture scheduler
    // stretch inside this test's clocked verdict window.
    let _t172_load = t172_load_lock::family_guard("loopd-stale-binary");
    // T158: the fixture's 30s verdict deadline is a liveness fence, not a load
    // assumption — a deadline blow invalidates the attempt and the WHOLE test
    // retries with a fresh sandbox (bounded); a wrong verdict still panics
    // un-retried.
    loopd_attempt_with_invalidation_retry(|| {
    const BOUND: usize = 3;
    let sandbox = Sandbox::new("#!/bin/sh\nexit 0\n");
    // The cycle child completes but never stops the loop.
    stub(&sandbox.root.join("target/release/chug"), NON_STOPPING_CHUG);
    // The harness's own bound stays at 50 (run_loopd_with_env); the scenario
    // overrides it down to 3 so the self-termination lands inside the window.
    let mut child = sandbox.run_loopd_with_env(&[("LOOPD_MAX_LOOPS", "3")]);
    let exit_needle = "LOOPD_MAX_LOOPS=3 reached — self-terminating";
    let deadline = Instant::now() + Duration::from_secs(30);
    let log = loop {
        let content = sandbox.read_log();
        if content.contains(exit_needle) {
            break content;
        }
        // The mutation detector: MORE than BOUND recorded cycles means the
        // bound did not fire — the code under test is wrong, so fail FAST
        // and un-retried (the RED leg of the mutation, ~seconds, no hang).
        let oks = content.lines().filter(|l| l.contains("cycle OK:")).count();
        assert!(
            oks <= BOUND,
            "LOOPD_MAX_LOOPS={BOUND} did not bound the supervisor: {oks} cycles \
             already completed and no self-termination — the iteration bound is \
             broken (T254; mutation leg: with the bound check removed from \
             loopd.sh this assertion is the RED that fires in seconds)"
        );
        if let Some(status) = child.try_wait().expect("poll the supervisor") {
            panic!(
                "the supervisor exited on its own ({status}) BEFORE logging the \
                 self-termination line — the bound must announce itself in \
                 .chug/loopd/loopd.log before exiting (T254):\n{content}"
            );
        }
        if Instant::now() > deadline {
            panic!(
                "loopd never reached {exit_needle:?} in 30s with \
                 LOOPD_MAX_LOOPS={BOUND} — the supervisor did not self-terminate \
                 (the T158 invalidation class: fixture deadline blow under load, \
                 retrying with a fresh sandbox):\n{content}"
            );
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    // The exit line is in the log: the process must have terminated ON ITS
    // OWN (never killed by this harness) and cleanly.
    let status = child.wait().expect("wait for the self-terminated supervisor");
    assert_eq!(
        status.code(),
        Some(0),
        "the self-termination must be exit 0 — a fixture supervisor reaching \
         its bound has done nothing wrong and must leave no failure residue \
         (a HALT would page an operator nobody is watching):\n{log}"
    );
    assert_eq!(
        log.lines().filter(|l| l.contains("cycle OK:")).count(),
        BOUND,
        "exactly N cycles run before the bound fires: the Nth full iteration \
         still runs its cycle, and the pass after it self-terminates — an \
         early exit would strand work, a late one is not a bound (T254):\n{log}"
    );
    assert!(
        !sandbox.root.join(".chug/loopd/loopd.pid").exists(),
        "a self-termination must be a CLEAN exit — the EXIT trap removes the \
         pidfile, so a stale loopd.pid would read as a live supervisor to \
         `loopd.sh status` (T254):\n{log}"
    );
    });
}

/// Static pins (the tests/loopd_reexec.rs pattern): deliberately brittle, so
/// a revert of the gate fails even if the behavioral stubs above are ever
/// loosened. The class swept: the build's exit status is latched and gated,
/// the build is pinned to ./target (never an export), and the `set -euo
/// pipefail` regime's pipelines — the single-driver probe, the verdict
/// marker grep, and the summary/status extraction — are pipefail-safe (a
/// SIGPIPE/short-circuit there must never flip a verdict, defeat the driver
/// guard, or kill the supervisor).
#[test]
fn pin_the_build_gate_and_the_pipefail_regime() {
    let loopd = fs::read_to_string(repo_root().join("loopd.sh")).expect("read loopd.sh");
    // The regime: -e (a nonzero command must not sail past), -u (was already
    // there), pipefail (a failing pipeline member is a failure).
    assert!(
        loopd.contains("set -euo pipefail"),
        "loopd.sh must run under `set -euo pipefail` — the review's minimum \
         is that a failed build cannot silently sail past the launch (T137)"
    );
    // The gate: the build's rc is latched (the T142 `|| chug_rc=$?` house
    // style — command substitution and `if !` both mask the real rc) and the
    // build is PINNED to ./target so the launched path is the built path.
    assert!(
        loopd.contains(
            "CARGO_TARGET_DIR=\"$ROOT/target\" cargo build --release >> \"$LOG\" 2>&1 || build_rc=$?"
        ),
        "the supervisor's own build must latch its exit status \
         (`|| build_rc=$?`) and pin CARGO_TARGET_DIR=\"$ROOT/target\" — an \
         inherited CARGO_TARGET_DIR must not send the build elsewhere while \
         the launch execs the fixed ./target/release/chug path (T137)"
    );
    // The old shape must stay dead: an ungated build line (status ignored,
    // straight into the launch).
    assert!(
        !loopd.contains("\n  cargo build --release >> "),
        "the ungated `cargo build --release >> $LOG` line must not return — \
         it is the bug: the build's exit status goes straight to the bit \
         bucket and the previous release binary launches (T137)"
    );
    // The refusal is gated on the latched rc and counts toward the HALT.
    assert!(
        loopd.contains("if [ \"$build_rc\" -ne 0 ]; then"),
        "the launch must be gated on the latched build rc (T137)"
    );
    assert!(
        loopd.contains("refusing to launch") && loopd.contains("HALTED after 3 consecutive build failures"),
        "a refused launch must be logged with a visible reason and count \
         toward the 3-strikes HALT — a permanently broken tree must halt the \
         supervisor loudly, not loop refusals forever (T137)"
    );
    // T47 restated by the fix: per-invocation prefix, never an export.
    assert!(
        !loopd.contains("export CARGO_TARGET_DIR"),
        "the ./target pin must stay a per-invocation prefix — a bare export \
         would persist across iterations and redirect the supervisor's build \
         into the shared cache (T47 finding 1, restated by T137)"
    );
    // pipefail sweep (1): the verdict marker must be grepped WITHOUT a
    // pipeline — `printf | grep -q` can EPIPE the writer on a large stdout
    // and flip a real goal-complete verdict into a failure under pipefail.
    assert!(
        loopd.contains("grep -q \"chug: goal complete\" <<<\"$chug_out\""),
        "the verdict marker grep must not be a `printf | grep -q` pipeline — \
         under pipefail a writer SIGPIPE on a large stdout flips the \
         verdict; grep the captured stdout directly (T137 sweep)"
    );
    // pipefail sweep (2): the summary extraction pipelines into `head -1`,
    // which closes the pipe early — under pipefail a second `summary:` line
    // SIGPIPEs grep and `set -e` kills the supervisor mid-success. The
    // pipeline is best-effort by design and must be guarded.
    assert!(
        loopd.contains("| cut -c1-200 || true)"),
        "the summary extraction must be guarded `|| true` — `head -1` closes \
         the pipe early and under pipefail+set -e that kills the supervisor \
         on the happy path (T137 sweep)"
    );
    // pipefail sweep (3): the same regime needs the best-effort producers
    // guarded so a nonzero exit cannot kill the supervisor pre-cycle.
    assert!(
        loopd.contains("eval-digest: nonzero exit (best-effort, ignored"),
        "eval-digest.sh is best-effort — under set -e an unguarded nonzero \
         exit would kill the supervisor before the cycle; the guard's \
         degradation line must carry it (T137 sweep)"
    );
    // pipefail sweep (4, T137 fix-up validator F1): the single-driver probe
    // must grep a CAPTURED ps listing, never a ps-to-grep pipeline — grep -q
    // exits at the first match, the still-writing ps leg SIGPIPEs, and under
    // pipefail that 141 flips a LIVE driver into "no driver" (fail-open
    // duplicate drivers; T135's flock only degrades-with-warning). The
    // probe's rc is latched so a failing enumeration fails CLOSED.
    assert!(
        loopd.contains("ps_out=\"$(ps -ax -o command=)\" || ps_rc=$?")
            && loopd.contains("grep -q \"[c]hug run --spec LOOP-SPEC.md\" <<<\"$ps_out\""),
        "the single-driver probe must capture the ps listing (rc latched) and \
         grep the capture — under pipefail a ps-to-grep pipeline flips \
         FAIL-OPEN when grep -q's early exit SIGPIPEs the ps leg \
         (EARLY-MATCH-FLIPPED-FALSE, T137 fix-up validator F1)"
    );
    assert!(
        loopd.contains("driver probe FAILED"),
        "a failing ps probe must fail CLOSED (log + skip) — never open and \
         never a silent set -e supervisor death (T137 fix-up sweep)"
    );
    assert!(
        !loopd.contains("ps -ax -o command= | grep"),
        "the ps-to-grep pipeline must stay dead — under pipefail it flips a \
         live driver into 'no driver' when grep -q's early exit SIGPIPEs the \
         ps leg (EARLY-MATCH-FLIPPED-FALSE, T137 fix-up validator F1)"
    );
    // The sleep seams exist so behavioral tests can bound the loop.
    assert!(
        loopd.contains("LOOPD_SLEEP_FAIL") && loopd.contains("LOOPD_SLEEP_OK"),
        "the inter-cycle sleeps must honor test seams (the CHUG_ROUTINE_TODAY \
         pattern) so the gate and HALT are behaviorally testable in seconds \
         (T137)"
    );
}

/// T172 pin (the T159 lock-scope pin shape): every deadline-bearing test in
/// this file — every body that constructs a sandbox (a real loopd.sh + a
/// clocked `wait_for_any` verdict window) — takes the cross-binary load lock
/// as its FIRST acquisition, before the T158 invalidation-retry wrapper and
/// the sandbox spawn. A static Mutex cannot do this job (nextest runs each
/// TEST as its own PROCESS — the contention this guards is cross-binary), so
/// the domain is the flock harness in tests/support/load_lock.rs; a future
/// unguarded sandbox test here is RED by construction even while every
/// behavioral test stays green (the cycle-33 sweep-the-family lesson).
#[test]
fn pin_deadline_tests_hold_the_t172_cross_binary_load_lock() {
    let src = fs::read_to_string(repo_root().join("tests/loopd_stale_binary.rs"))
        .expect("read own source (cargo runs test binaries with cwd = package root)");
    // The join must be THE #[path] include of the harness file — a copy
    // would be a second, independent domain (the T151 finding, at file
    // granularity).
    assert!(
        src.contains("#[path = \"support/load_lock.rs\"]\nmod t172_load_lock;"),
        "the cross-binary join must be the #[path] include of \
         tests/support/load_lock.rs — any other lock source is a second, \
         independent domain"
    );
    let guard_line = "let _t172_load = t172_load_lock::family_guard(\"loopd-stale-binary\");";
    let mut guarded: Vec<&str> = Vec::new();
    for chunk in src.split("\n#[test]").skip(1) {
        let body = chunk.trim_start_matches('\n');
        let name = body
            .strip_prefix("fn ")
            .and_then(|rest| rest.split(['(', '<']).next())
            .unwrap_or("")
            .trim();
        assert!(!name.is_empty(), "a test chunk failed to yield its fn name");
        // This pin's own chunk mentions the scanned markers as TEXT; it is
        // not a sandbox test and takes no guard.
        if name == "pin_deadline_tests_hold_the_t172_cross_binary_load_lock" {
            continue;
        }
        if !chunk.contains("Sandbox::") {
            // The static source pin carries no clock and no sandbox — it
            // takes no guard.
            continue;
        }
        let at_guard = chunk.find(guard_line).unwrap_or_else(|| {
            panic!(
                "{name} spawns a clocked sandbox but never takes the T172 \
                 cross-binary load lock — nextest runs each test as its own \
                 PROCESS, so without the flock domain the suite's parallel \
                 sandboxes manufacture the scheduler stretch that busts this \
                 test's 30s verdict window (the T152 signature, cycles 76-79)"
            )
        });
        let at_retry = chunk
            .find("loopd_attempt_with_invalidation_retry(")
            .expect("every sandbox test here rides the T158 retry seam");
        assert!(
            at_guard < at_retry,
            "{name} must take the load lock as its FIRST acquisition — before \
             the T158 retry wrapper — so ALL invalidation attempts of one test \
             hold the domain (a mid-body guard would let a sibling sandbox \
             interleave between attempts)"
        );
        guarded.push(name);
    }
    assert!(
        guarded.len() >= 5,
        "the sandbox scan went empty — the deadline-bearing family must still \
         be named Sandbox (5 guarded tests at T172 landing)"
    );
}
