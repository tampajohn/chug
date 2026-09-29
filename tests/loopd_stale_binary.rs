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

fn repo_root() -> PathBuf {
    std::env::current_dir().expect("cargo sets the test cwd to the package root")
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
        let keep = tempfile::tempdir().expect("sandbox tempdir");
        let root = keep.path().to_path_buf();
        // The real script + its scripts/ helpers, byte-for-byte.
        fs::copy(repo_root().join("loopd.sh"), root.join("loopd.sh")).expect("copy loopd.sh");
        let scripts = root.join("scripts");
        fs::create_dir_all(&scripts).expect("scripts dir");
        for entry in fs::read_dir(repo_root().join("scripts")).expect("scripts dir") {
            let entry = entry.expect("scripts entry");
            fs::copy(entry.path(), scripts.join(entry.file_name())).expect("copy script");
        }
        // PATH stubs: `ps` reports no processes (single-driver guard passes),
        // `cargo` plays the scenario's build (failing, or succeeding into
        // whatever CARGO_TARGET_DIR it is handed — a real cargo honors the
        // var, which is exactly the inherited-env leg).
        let bin = root.join("bin");
        fs::create_dir_all(&bin).expect("bin dir");
        stub(&bin.join("ps"), "#!/bin/sh\nexit 1\n");
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
}

/// The gate must count its refusals: 3 consecutive failed builds trip the
/// supervisor's existing 3-strikes HALT (a broken tree must page the
/// operator, not loop refusals forever — the same contract a failed CYCLE
/// already has). Uses the LOOPD_SLEEP_FAIL seam so the three strikes take
/// seconds, not the production 3×300s.
#[test]
fn three_consecutive_failed_builds_halt_the_supervisor() {
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
}

/// Static pins (the tests/loopd_reexec.rs pattern): deliberately brittle, so
/// a revert of the gate fails even if the behavioral stubs above are ever
/// loosened. The class swept: the build's exit status is latched and gated,
/// the build is pinned to ./target (never an export), and the `set -euo
/// pipefail` regime's two pre-existing pipelines — the verdict marker grep
/// and the summary extraction — are pipefail-safe (a SIGPIPE/short-circuit
/// there must never flip a verdict or kill the supervisor).
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
    // The sleep seams exist so behavioral tests can bound the loop.
    assert!(
        loopd.contains("LOOPD_SLEEP_FAIL") && loopd.contains("LOOPD_SLEEP_OK"),
        "the inter-cycle sleeps must honor test seams (the CHUG_ROUTINE_TODAY \
         pattern) so the gate and HALT are behaviorally testable in seconds \
         (T137)"
    );
}
