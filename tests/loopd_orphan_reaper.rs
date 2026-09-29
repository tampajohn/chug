//! T152 — loopd's pre-cycle orphan-process reaper (fail-closed precision).
//!
//! Cycle-70 (T145-arc) found a 9.8-hour-old spinning test binary at 99% CPU
//! (~590 CPU-minutes burned) left over from the T134-validation era: a T79
//! parallel-mutant leg whose validator died mid-leg, a child killed at the
//! bash 120s cap whose cargo/test process group outlived it, or a removed
//! `/tmp/chug-loop-t*`/`/tmp/chug-mut-*` worktree's leftover process.
//! Nothing reaped it, and the load it caused is the triggering condition of
//! the T151 flake family. The fix: loopd.sh runs `scripts/orphan-reaper.sh`
//! at the ONE safe instant — immediately after the single-driver probe
//! passes and BEFORE the build gate — where no driver, no orchestrator, and
//! no delegate children can legitimately exist, so any leftover
//! loop-artifact process is definitionally orphaned.
//!
//! These tests follow the tests/loopd_stale_binary.rs doctrine: they guard
//! the real script/helper, they do not reimplement it, and the sweep is
//! exercised WITHOUT running the whole supervisor (the helper is loopd's
//! seam) plus two full-supervisor integration runs through the real
//! `loopd.sh` in a sandbox. `ps` is stubbed on PATH (the harness pattern):
//! the sweep's enumeration lists ONLY the rows the test names — the host's
//! real process table is never enumerated, so no foreign process can ever
//! be signalled by a test — while `comm=`/`pgid=` queries delegate to the
//! REAL `/bin/ps`, so judgments resolve the fixture's true identity.
//!
//! Fixtures are real processes spawned by the test itself, in their OWN
//! process group (`process_group(0)` — the reaper's own-process-group
//! exclusion must pass them), and the test owns their cleanup. The leg-(b)
//! fixture is COMPILED (rustc, guaranteed wherever cargo test runs) rather
//! than a copied platform binary: macOS kills signature-constrained copies
//! of platform binaries at exec (proven on this host), and every artifact
//! the reaper targets is itself compiler-built, so a compiled fixture is the
//! faithful shape. Identification legs run under `LOOP_REAPER_DRY_RUN=1` —
//! no fixture is ever signalled by a dry run; the single real-TERM run
//! targets a fixture the harness fully owns and reaps.

#![cfg(unix)]

use std::fs;
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use tempfile::TempDir;

fn repo_root() -> PathBuf {
    std::env::current_dir().expect("cargo sets the test cwd to the package root")
}

/// The spec's example needle: a `/tmp/chug-mut-t999-1` worktree that does
/// not exist (precondition of the removed-worktree leg).
const REMOVED_WT: &str = "/tmp/chug-mut-t999-1";

/// Compile the leg-(b) fixture: a process that spins (asleep — not a CPU
/// burner) at an executable path under a `target-shared*/deps/` shape.
fn compile_spinner(dest: &Path) {
    let src = dest
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .expect("artifact root")
        .join("fixture-spin.rs");
    fs::write(
        &src,
        "use std::time::Duration;\nfn main() { loop { std::thread::sleep(Duration::from_secs(3600)); } }\n",
    )
    .expect("write fixture source");
    fs::create_dir_all(dest.parent().expect("deps dir")).expect("deps dir");
    let out = Command::new("rustc")
        .arg("-o")
        .arg(dest)
        .arg(&src)
        .output()
        .expect("spawn rustc for the fixture");
    assert!(
        out.status.success(),
        "compiling the leg-(b) fixture failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Spawn a long-lived fixture in its OWN process group: the reaper's
/// own-process-group exclusion (the supervisor + everything it could
/// legitimately have spawned) must be able to pass a fixture, and the test
/// owns cleanup — every fixture is killed by the test, never left behind.
fn spawn_fixture(exe: &Path, arg0: Option<&str>, arg: &str) -> Child {
    let mut cmd = Command::new(exe);
    if let Some(a0) = arg0 {
        cmd.arg0(a0);
    }
    cmd.arg(arg);
    cmd.process_group(0);
    cmd.stdout(Stdio::null()).stderr(Stdio::null());
    cmd.spawn().expect("spawn fixture in its own process group")
}

fn kill_fixture(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// The hermetic stub's fixture answers: the fixture pid gets `comm` (the
/// REAL absolute exec path — the same value the kernel records) and, when
/// set, its own pgid; everything else gets the unknown-row defaults.
fn fixture_env(pid: u32, comm: &str, extra: &[(&str, &str)]) -> Vec<(String, String)> {
    let mut envs = vec![
        ("REAPER_STUB_FIXTURE_PID".to_string(), pid.to_string()),
        ("REAPER_STUB_COMM_TEXT".to_string(), comm.to_string()),
    ];
    for (k, v) in extra {
        envs.push((k.to_string(), v.to_string()));
    }
    envs
}

fn stub(path: &Path, body: &str) {
    fs::write(path, body).expect("write stub");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("chmod stub");
}

/// The ps stub (the tests/loopd_stale_binary.rs pattern) — fully HERMETIC:
/// the real /bin/ps is never consulted (a loaded host can stall it for
/// minutes — observed on this host, the T151 flake class); every answer
/// comes from the test's env. Dispatch by argv shape: the sweep's
/// enumeration (`-ax -o pid= -o command=`) prints ONLY the rows the test
/// named (REAPER_STUB_ROWS); the loopd single-driver probe (`-ax -o
/// command=`) reports no driver (or fails, for the probe-gates-sweep leg);
/// `comm=`/`pgid=` queries answer for the fixture pid from
/// REAPER_STUB_COMM_TEXT / REAPER_STUB_FIXTURE_PGID, and for everything
/// else with the defaults an unknown row would give (empty comm; one shared
/// pgid, so only the fixture sits outside the reaper's own group).
const PS_STUB: &str = r#"#!/bin/sh
last_numeric() {
  for arg in "$@"; do
    case "$arg" in
      *[!0-9]*) ;;
      '') ;;
      *) pid=$arg ;;
    esac
  done
}
case "$*" in
  *"-ax -o pid= -o command="*)
    [ "${REAPER_STUB_ENUM_FAILS:-0}" = "1" ] && exit 9
    printf '%s\n' "$REAPER_STUB_ROWS"
    exit 0
    ;;
  *"-ax -o command="*)
    [ "${REAPER_STUB_PROBE_FAILS:-0}" = "1" ] && exit 7
    exit 0
    ;;
  *" comm="*)
    [ "${REAPER_STUB_COMM_FAILS:-0}" = "1" ] && exit 7
    pid=""
    last_numeric "$@"
    if [ "$pid" = "${REAPER_STUB_FIXTURE_PID:-}" ]; then
      printf '%s\n' "${REAPER_STUB_COMM_TEXT:-}"
      exit 0
    fi
    exit 0
    ;;
  *" pgid="*)
    [ "${REAPER_STUB_PGID_FAILS:-0}" = "1" ] && exit 7
    pid=""
    last_numeric "$@"
    if [ "$pid" = "${REAPER_STUB_FIXTURE_PID:-}" ] && [ -n "${REAPER_STUB_FIXTURE_PGID:-}" ]; then
      printf '%s\n' "$REAPER_STUB_FIXTURE_PGID"
      exit 0
    fi
    printf '%s\n' "${REAPER_STUB_PGID_TEXT:-777}"
    exit 0
    ;;
  *)
    exit 0
    ;;
esac
"#;

/// A green build that installs the launchable stub binary (the
/// tests/loopd_stale_binary.rs pattern): the cycle "completes" with an
/// accepted verdict and stops the loop, so the sandbox supervisor runs
/// probe → sweep → build → cycle → exit in seconds.
const GREEN_BUILD: &str = r#"#!/bin/sh
set -e
[ -n "$CARGO_TARGET_DIR" ] || { echo 'stub cargo: CARGO_TARGET_DIR unset' >&2; exit 1; }
mkdir -p "$CARGO_TARGET_DIR/release"
cat > "$CARGO_TARGET_DIR/release/chug" <<'STUB'
#!/bin/sh
printf 'chug: goal complete\nsummary: the stub binary ran this cycle\n'
mkdir -p .chug && touch .chug/STOP-LOOP
exit 0
STUB
chmod +x "$CARGO_TARGET_DIR/release/chug"
echo "stub-build: green"
exit 0
"#;

/// The direct harness: the real `scripts/orphan-reaper.sh` copied into a
/// sandbox with the ps stub first on PATH, run exactly the way loopd runs
/// it (a child process; stdout captured).
struct Direct {
    _keep: TempDir,
    root: PathBuf,
}

impl Direct {
    fn new() -> Direct {
        let keep = tempfile::tempdir().expect("sandbox tempdir");
        let root = keep.path().to_path_buf();
        fs::create_dir_all(root.join("bin")).expect("bin dir");
        fs::copy(
            repo_root().join("scripts/orphan-reaper.sh"),
            root.join("orphan-reaper.sh"),
        )
        .expect("copy orphan-reaper.sh");
        stub(&root.join("bin/ps"), PS_STUB);
        Direct { _keep: keep, root }
    }

    fn helper(&self) -> PathBuf {
        self.root.join("orphan-reaper.sh")
    }

    fn run(&self, rows: &str, envs: &[(String, String)]) -> String {
        let mut cmd = Command::new("bash");
        cmd.arg(self.helper());
        cmd.env(
            "PATH",
            format!(
                "{}:{}",
                self.root.join("bin").display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        );
        cmd.env("REAPER_STUB_ROWS", rows);
        for (k, v) in envs {
            cmd.env(k, v);
        }
        let out = cmd.output().expect("run the reaper helper");
        assert!(
            out.status.success(),
            "the helper must exit 0 on every path (loopd's launch is best-effort \
             guarded, but a nonzero exit would be a doctrine violation): {} — \
             stderr: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).into_owned()
    }
}

/// The full-supervisor harness (the tests/loopd_stale_binary.rs pattern):
/// the REAL loopd.sh + scripts/, the ps stub on PATH, a green stub build,
/// and the reaper's rows injected via env.
struct Sandbox {
    _keep: TempDir,
    root: PathBuf,
}

impl Sandbox {
    fn new() -> Sandbox {
        let keep = tempfile::tempdir().expect("sandbox tempdir");
        let root = keep.path().to_path_buf();
        fs::copy(repo_root().join("loopd.sh"), root.join("loopd.sh")).expect("copy loopd.sh");
        let scripts = root.join("scripts");
        fs::create_dir_all(&scripts).expect("scripts dir");
        for entry in fs::read_dir(repo_root().join("scripts")).expect("scripts dir") {
            let entry = entry.expect("scripts entry");
            fs::copy(entry.path(), scripts.join(entry.file_name())).expect("copy script");
        }
        fs::create_dir_all(root.join("bin")).expect("bin dir");
        stub(&root.join("bin/ps"), PS_STUB);
        stub(&root.join("bin/cargo"), GREEN_BUILD);
        // site-sync must NEVER leave the sandbox (a live site dir on this
        // host would get synced from fixture garbage): point the env default
        // at a path that does not exist (warn + exit 0 leg).
        fs::write(root.join("NO-SUCH-SITE"), "").expect("site sentinel");
        Sandbox { _keep: keep, root }
    }

    fn run_loopd(&self, envs: &[(&str, String)]) -> Child {
        let mut cmd = Command::new("bash");
        cmd.arg("loopd.sh").arg("run").current_dir(&self.root);
        cmd.env(
            "PATH",
            format!(
                "{}:{}",
                self.root.join("bin").display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        );
        cmd.env("CHUG_SITE_DIR", self.root.join("NO-SUCH-SITE"));
        cmd.env("CHUG_SITE_SYNC_NO_PUSH", "1");
        // The sleep seams (the tests/loopd_stale_binary.rs pattern): bound a
        // missed kill so a stuck loop cannot park the test.
        cmd.env("LOOPD_SLEEP_OK", "1");
        cmd.env("LOOPD_SLEEP_FAIL", "1");
        for (k, v) in envs {
            cmd.env(k, v);
        }
        cmd.stdout(Stdio::null()).stderr(Stdio::null());
        cmd.spawn().expect("spawn bash loopd.sh run")
    }

    fn read_log(&self) -> String {
        let mut content = String::new();
        if let Ok(mut f) = fs::File::open(self.root.join(".chug/loopd/loopd.log")) {
            let _ = f.read_to_string(&mut content);
        }
        content
    }

    /// Poll the supervisor log until ANY of `needles` appears, then kill the
    /// loop and return the log (the tests/loopd_stale_binary.rs pattern).
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
                panic!(
                    "loopd never reached any of {needles:?} in 30s.\n--- loopd.log ---\n{content}"
                );
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

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

/// The judgment line for one pid, by kind ("term", "would-term", "skip").
/// The full "orphan-reaper: <kind> pid=" prefix keeps "term" from matching
/// inside "would-term".
fn line_for<'a>(log: &'a str, kind: &str, pid: u32) -> Option<&'a str> {
    log.lines()
        .find(|l| l.contains(&format!("orphan-reaper: {kind} pid={pid} ")))
}

fn assert_no_judgment_for(log: &str, pid: u32) {
    for kind in ["term", "would-term", "skip"] {
        assert!(
            line_for(log, kind, pid).is_none(),
            "pid {pid} must not be judged ({kind}) — it matches no needle:\n{log}"
        );
    }
}

// ---------------------------------------------------------------------------
// Spec test leg (a): the argv needle.
// ---------------------------------------------------------------------------

/// A fixture whose argv carries a `/tmp/chug-mut-t999-1` needle with that
/// directory absent must be IDENTIFIED as a candidate via the
/// removed-worktree leg — under DRY_RUN (identify, never signal; cleanup is
/// the test's duty).
#[test]
fn argv_needle_with_removed_worktree_is_identified() {
    assert!(
        !Path::new(REMOVED_WT).exists(),
        "precondition: {REMOVED_WT} must not exist for this leg"
    );
    // arg0 carries the needle; the REAL executable is /bin/sleep — so the
    // artifact leg cannot fire and the argv leg is the only signal.
    let mut fixture = spawn_fixture(Path::new("/bin/sleep"), Some(&format!("{REMOVED_WT}/spinner")), "120");
    let pid = fixture.id();
    let direct = Direct::new();
    let out = direct.run(
        &format!("{pid} {REMOVED_WT}/spinner 120"),
        &fixture_env(pid, "/bin/sleep", &[("LOOP_REAPER_DRY_RUN", "1")]),
    );
    let line = line_for(&out, "would-term", pid)
        .unwrap_or_else(|| panic!("the argv-needle fixture must be identified:\n{out}"));
    assert!(
        line.contains("removed-worktree-argv") && line.contains(REMOVED_WT),
        "the judgment must name the removed-worktree leg and the extracted \
         directory:\n{out}"
    );
    assert!(
        line_for(&out, "term", pid).is_none(),
        "DRY_RUN must never signal — only would-term judgments may appear:\n{out}"
    );
    assert!(
        fixture.try_wait().expect("poll fixture").is_none(),
        "the dry-run sweep must have left the fixture alive:\n{out}"
    );
    kill_fixture(&mut fixture);
}

// ---------------------------------------------------------------------------
// Spec test leg (b): the artifact (executable-path) leg.
// ---------------------------------------------------------------------------

/// A fixture whose REAL executable path (comm — resolved via the real
/// `ps -o comm=`, delegated by the stub) sits under a
/// `target-shared*/deps/` shape must be identified via the artifact leg.
/// The enumeration row's argv text deliberately carries NO shape and NO
/// needle, so the ONLY path to candidacy is the real comm — the exact
/// discrimination the spec draws ("executable path, not argv[0] text").
#[test]
fn target_shared_deps_comm_identifies_the_artifact_shape() {
    let direct = Direct::new();
    let exe = direct.root.join("target-shared-mut-1/deps/spinner");
    compile_spinner(&exe);
    let mut fixture = spawn_fixture(&exe, None, "120");
    let pid = fixture.id();
    let out = direct.run(
        &format!("{pid} spinner 120"),
        &fixture_env(pid, &exe.to_string_lossy(), &[("LOOP_REAPER_DRY_RUN", "1")]),
    );
    let line = line_for(&out, "would-term", pid)
        .unwrap_or_else(|| panic!("the artifact-shape fixture must be identified:\n{out}"));
    assert!(
        line.contains("target-shared-deps") && line.contains("/target-shared-mut-1/deps/"),
        "the judgment must name the artifact leg with the REAL comm as \
         evidence:\n{out}"
    );
    assert!(
        line_for(&out, "term", pid).is_none(),
        "DRY_RUN must never signal:\n{out}"
    );
    kill_fixture(&mut fixture);
}

/// The negative half of the artifact leg: argv[0] TEXT carrying the
/// target-shared shape must never qualify — the leg resolves `ps -o comm=`,
/// and this fixture's real comm (/bin/sleep) matches nothing. A spoofed
/// argv[0] (or any argv text) claiming the shape is not identity.
#[test]
fn argv0_text_never_qualifies_the_artifact_leg() {
    let spoof = "/nonexistent/target-shared-spoof/deps/spoof";
    let mut fixture = spawn_fixture(Path::new("/bin/sleep"), Some(spoof), "120");
    let pid = fixture.id();
    let direct = Direct::new();
    let out = direct.run(
        &format!("{pid} {spoof} 120"),
        &fixture_env(pid, "/bin/sleep", &[("LOOP_REAPER_DRY_RUN", "1")]),
    );
    assert_no_judgment_for(&out, pid);
    kill_fixture(&mut fixture);
}

// ---------------------------------------------------------------------------
// Spec test leg (c): neither needle.
// ---------------------------------------------------------------------------

/// A fixture with neither needle (plain `sleep`) is never a candidate: no
/// judgment line of any kind, and the summary says so.
#[test]
fn neither_needle_is_never_a_candidate() {
    let mut fixture = spawn_fixture(Path::new("/bin/sleep"), None, "120");
    let pid = fixture.id();
    let direct = Direct::new();
    let out = direct.run(
        &format!("{pid} sleep 120"),
        &fixture_env(pid, "/bin/sleep", &[("LOOP_REAPER_DRY_RUN", "1")]),
    );
    assert_no_judgment_for(&out, pid);
    assert!(
        out.contains("done (examined=1 would-term=0 skipped=0)"),
        "the summary must show the row was examined and judged a \
         non-candidate:\n{out}"
    );
    kill_fixture(&mut fixture);
}

// ---------------------------------------------------------------------------
// Spec test leg (d): unresolved identity → skip, never kill.
// ---------------------------------------------------------------------------

/// Every fail-closed gate: (i) the comm query fails; (ii) the comm resolves
/// but is NOT absolute (the spec's non-absolute-comm example); (iii) the
/// candidate's process group cannot be resolved. In every variant the
/// candidate is SKIPPED with the spec-pinned `(unresolved)` line and never
/// signalled (dry-run on top: even a would-term would fail this test).
#[test]
fn unresolved_identity_is_skipped_never_signalled() {
    // (i) ps failure on the identity query.
    let direct = Direct::new();
    let out = direct.run(
        "424242 /tmp/chug-mut-t777-1/spin 120",
        &fixture_env(
            424242,
            "",
            &[("LOOP_REAPER_DRY_RUN", "1"), ("REAPER_STUB_COMM_FAILS", "1")],
        ),
    );
    let line = line_for(&out, "skip", 424242)
        .unwrap_or_else(|| panic!("a ps failure must be logged as an unresolved skip:\n{out}"));
    assert!(
        line.contains("(unresolved)"),
        "the skip line must carry the spec-pinned (unresolved) marker:\n{out}"
    );
    assert!(
        line_for(&out, "would-term", 424242).is_none(),
        "an unresolved candidate must never be signalled (not even \
         would-term):\n{out}"
    );

    // (ii) comm resolves but is relative — ambiguity is never killed.
    assert!(
        !Path::new("/tmp/chug-mut-t555-1").exists(),
        "precondition: the (ii) needle worktree must not exist"
    );
    let mut fixture = spawn_fixture(Path::new("/bin/sleep"), None, "120");
    let pid = fixture.id();
    let out = direct.run(
        &format!("{pid} /tmp/chug-mut-t555-1/spin 120"),
        &fixture_env(pid, "sleep", &[("LOOP_REAPER_DRY_RUN", "1")]),
    );
    assert!(
        line_for(&out, "skip", pid).is_some(),
        "a candidate with a non-absolute comm must be skipped as unresolved:\n{out}"
    );
    assert!(
        line_for(&out, "would-term", pid).is_none(),
        "a non-absolute comm must never reach a signal:\n{out}"
    );
    kill_fixture(&mut fixture);

    // (iii) the process group cannot be resolved.
    assert!(
        !Path::new("/tmp/chug-mut-t556-1").exists(),
        "precondition: the (iii) needle worktree must not exist"
    );
    let mut fixture = spawn_fixture(Path::new("/bin/sleep"), None, "120");
    let pid = fixture.id();
    let out = direct.run(
        &format!("{pid} /tmp/chug-mut-t556-1/spin 120"),
        &fixture_env(
            pid,
            "/bin/sleep",
            &[("LOOP_REAPER_DRY_RUN", "1"), ("REAPER_STUB_PGID_FAILS", "1")],
        ),
    );
    assert!(
        line_for(&out, "skip", pid).is_some(),
        "a candidate whose process group is unresolvable must be skipped:\n{out}"
    );
    assert!(
        line_for(&out, "would-term", pid).is_none(),
        "an unresolvable process group must never reach a signal:\n{out}"
    );
    kill_fixture(&mut fixture);
}

/// The ONE absolute exclusion, self-referentially: the reaper's own process
/// group — here the parent test binary itself, a real candidate (its
/// artifact path under the shared cache matches the shape, and the
/// enumeration row carries a removed-worktree needle) — is skipped with the
/// `(own process group)` line and never signalled.
#[test]
fn own_process_group_is_never_signalled() {
    assert!(
        !Path::new("/tmp/chug-mut-t666-1").exists(),
        "precondition: the needle worktree must not exist"
    );
    let pid = std::process::id();
    let direct = Direct::new();
    let out = direct.run(
        &format!("{pid} /tmp/chug-mut-t666-1/spin 120"),
        &fixture_env(pid, "/bin/sleep", &[("LOOP_REAPER_DRY_RUN", "1")]),
    );
    let line = line_for(&out, "skip", pid)
        .unwrap_or_else(|| panic!("the reaper's own process group must be skipped:\n{out}"));
    assert!(
        line.contains("(own process group)"),
        "the skip must name the own-process-group exclusion:\n{out}"
    );
    assert!(
        line_for(&out, "would-term", pid).is_none(),
        "the own process group must never reach even a would-term:\n{out}"
    );
}

// ---------------------------------------------------------------------------
// Spec test leg (e): the opt-out.
// ---------------------------------------------------------------------------

/// `LOOP_REAPER=0` disables the sweep: one logged line, zero judgments —
/// even with a live candidate in the table.
#[test]
fn loop_reaper_0_disables_the_sweep() {
    let direct = Direct::new();
    let exe = direct.root.join("target-shared-mut-2/deps/spinner");
    compile_spinner(&exe);
    let mut fixture = spawn_fixture(&exe, None, "120");
    let pid = fixture.id();
    let out = direct.run(
        &format!("{pid} spinner 120"),
        &fixture_env(pid, &exe.to_string_lossy(), &[("LOOP_REAPER", "0")]),
    );
    assert!(
        out.contains("orphan-reaper: disabled (LOOP_REAPER=0)"),
        "the opt-out must be logged:\n{out}"
    );
    assert_no_judgment_for(&out, pid);
    assert!(
        !out.contains("done (examined="),
        "a disabled sweep must not run at all (no summary line):\n{out}"
    );
    assert!(
        fixture.try_wait().expect("poll fixture").is_none(),
        "the disabled sweep must have left the fixture alive:\n{out}"
    );
    kill_fixture(&mut fixture);
}

// ---------------------------------------------------------------------------
// Bounded sweep: deterministic pagination.
// ---------------------------------------------------------------------------

/// At most LOOP_REAPER_MAX rows are examined per sweep, selected
/// deterministically (pid-ascending page, persisted offset): 5 rows with a
/// bound of 3 → rows 1-3, then 4-5, then 1-3 again. A pathological table
/// costs a bounded couple of ps calls per cycle, and every row eventually
/// gets its page.
#[test]
fn the_sweep_pages_deterministically() {
    let direct = Direct::new();
    // Fake pids far above the host's pid space: the comm query fails, the
    // argv is needle-free, so every row is a silent non-candidate — the
    // test asserts on the pagination bookkeeping, not judgments.
    let rows = "900001 sleep 120\n900002 sleep 120\n900003 sleep 120\n900004 sleep 120\n900005 sleep 120";
    let page = direct.root.join("reaper-page");
    let envs = [
        ("LOOP_REAPER_MAX".to_string(), "3".to_string()),
        (
            "LOOP_REAPER_PAGE_FILE".to_string(),
            page.to_string_lossy().into_owned(),
        ),
    ];
    let out1 = direct.run(rows, &envs);
    assert!(
        out1.contains("bound: examining rows 1-3 of 5"),
        "page 1 must be rows 1-3 with the bound logged:\n{out1}"
    );
    assert!(
        out1.contains("done (examined=3 killed=0 skipped=0)"),
        "page 1 must examine exactly the bound (3 of 5):\n{out1}"
    );
    assert_eq!(
        fs::read_to_string(&page).expect("page file").trim(),
        "3",
        "the persisted offset must advance by the bound"
    );
    let out2 = direct.run(rows, &envs);
    assert!(
        out2.contains("bound: examining rows 4-5 of 5")
            && out2.contains("done (examined=2 killed=0 skipped=0)"),
        "page 2 must be the remainder (rows 4-5):\n{out2}"
    );
    assert_eq!(
        fs::read_to_string(&page).expect("page file").trim(),
        "0",
        "a consumed table must reset the page to the first"
    );
    let out3 = direct.run(rows, &envs);
    assert!(
        out3.contains("bound: examining rows 1-3 of 5"),
        "the rotation must be deterministic (page 1 again):\n{out3}"
    );
}

/// A failing enumeration is UNKNOWN, and unknown fails CLOSED: the sweep
/// skips with a logged reason and judges nothing.
#[test]
fn a_failing_enumeration_fails_closed() {
    let direct = Direct::new();
    let out = direct.run(
        "424242 /tmp/chug-mut-t778-1/spin 120",
        &fixture_env(424242, "", &[("REAPER_STUB_ENUM_FAILS", "1")]),
    );
    assert!(
        out.contains("sweep skipped (ps enumeration failed rc=9 — fail-closed)"),
        "a failing enumeration must be latched and logged fail-closed:\n{out}"
    );
    assert!(
        !out.contains("pid="),
        "a failed enumeration must judge nothing:\n{out}"
    );
}

// ---------------------------------------------------------------------------
// Full-supervisor integration: the sweep point and its gate.
// ---------------------------------------------------------------------------

/// The real loopd.sh, end to end: probe passes → sweep → the orphan is
/// TERM'd (a fixture the harness fully owns) → THEN the build gate runs and
/// the cycle launches. This pins the sweep point (after the single-driver
/// probe, before the build) by log order, and the kill leg (SIGTERM, real
/// signal, real fixture) by the fixture's exit status.
#[test]
fn the_reaper_terms_an_orphan_through_loopd_before_the_build() {
    let sandbox = Sandbox::new();
    let exe = sandbox.root.join("target-shared-mut-1/deps/spinner");
    compile_spinner(&exe);
    let mut fixture = spawn_fixture(&exe, None, "300");
    let pid = fixture.id();
    let rows = format!("{pid} spinner 300");
    let mut child = sandbox.run_loopd(&[
        ("REAPER_STUB_ROWS", rows.clone()),
        ("REAPER_STUB_FIXTURE_PID", pid.to_string()),
        ("REAPER_STUB_COMM_TEXT", exe.to_string_lossy().into_owned()),
        // The fixture sits in its OWN process group (pgid == pid); the
        // helper itself shares the test's group — answered by the stub's
        // default — so the exclusion passes the fixture.
        ("REAPER_STUB_FIXTURE_PGID", pid.to_string()),
    ]);
    // Either the reaper terms the orphan (good) or the cycle launches
    // anyway (the bug) — first needle wins, so the RED leg fails fast with
    // the supervisor's own words.
    let log = sandbox.wait_for_any(&mut child, &["orphan-reaper: term pid=", "cycle OK:"]);
    let term_line = line_for(&log, "term", pid)
        .unwrap_or_else(|| panic!("the orphan must be reaped through loopd:\n{log}"));
    assert!(
        term_line.contains("target-shared-deps:") && term_line.contains("/target-shared-mut-1/deps/"),
        "the kill judgment must name the artifact leg with the real comm:\n{log}"
    );
    assert!(
        log.contains("done (examined=1 killed=1 skipped=0)"),
        "the sweep summary must account for the single candidate:\n{log}"
    );
    // The sweep point: the reaper's judgment lands BEFORE the build gate's
    // output and before the cycle starts.
    let at_reaper = log.find(term_line).expect("term line position");
    let at_build = log.find("stub-build: green").expect("build ran");
    let at_cycle = log.find("cycle start").expect("cycle started");
    assert!(
        at_reaper < at_build && at_reaper < at_cycle,
        "the sweep must run AFTER the driver probe and BEFORE the build gate \
         (reaper@{at_reaper} vs build@{at_build}, cycle@{at_cycle}):\n{log}"
    );
    // The kill leg is a real SIGTERM to the fixture — the child the harness
    // spawned must have died by signal 15.
    let status = fixture.wait().expect("wait fixture");
    assert_eq!(
        status.signal(),
        Some(15),
        "the orphan must be reaped by SIGTERM (never SIGKILL), got: {status:?}"
    );
    // And the sweep must not have blocked the launch: the cycle ran.
    assert!(
        !sandbox.cycle_logs().is_empty(),
        "the sweep is pre-build, not pre-launch — the cycle must still run:\n{log}"
    );
}

/// The sweep is gated by the single-driver probe: a FAILING probe (unknown
/// enumeration) skips the cycle — and must never reach the sweep, because
/// the sweep-point invariant (no legitimate chug processes exist) is exactly
/// what the probe establishes.
#[test]
fn a_failing_driver_probe_means_no_sweep() {
    let sandbox = Sandbox::new();
    let exe = sandbox.root.join("target-shared-mut-1/deps/spinner");
    compile_spinner(&exe);
    let mut fixture = spawn_fixture(&exe, None, "300");
    let pid = fixture.id();
    let rows = format!("{pid} spinner 300");
    let mut child = sandbox.run_loopd(&[
        ("REAPER_STUB_ROWS", rows),
        ("REAPER_STUB_FIXTURE_PID", pid.to_string()),
        ("REAPER_STUB_COMM_TEXT", exe.to_string_lossy().into_owned()),
        ("REAPER_STUB_PROBE_FAILS", "1".to_string()),
    ]);
    let log = sandbox.wait_for_any(&mut child, &["driver probe FAILED"]);
    assert!(
        !log.contains("orphan-reaper:"),
        "a failing probe must gate the sweep off entirely — the sweep-point \
         invariant does not hold when the enumeration is unknown:\n{log}"
    );
    assert!(
        sandbox.cycle_logs().is_empty(),
        "no cycle may be logged when the driver enumeration failed:\n{log}"
    );
    assert!(
        fixture.try_wait().expect("poll fixture").is_none(),
        "the fixture must survive a skipped cycle untouched:\n{log}"
    );
    kill_fixture(&mut fixture);
}

// ---------------------------------------------------------------------------
// Static pins (the tests/loopd_stale_binary.rs pattern): deliberately
// brittle, so a revert fails even if the behavioral tests are ever loosened.
// ---------------------------------------------------------------------------

#[test]
fn pin_the_reaper_wiring_and_doctrine() {
    let loopd = fs::read_to_string(repo_root().join("loopd.sh")).expect("read loopd.sh");
    let helper = fs::read_to_string(repo_root().join("scripts/orphan-reaper.sh"))
        .expect("read orphan-reaper.sh");
    let readme = fs::read_to_string(repo_root().join("README.md")).expect("read README.md");

    // The sweep call sits at the ONE safe instant: after the single-driver
    // probe's grep passes and before the build gate.
    let at_probe = loopd
        .find("grep -q \"[c]hug run --spec LOOP-SPEC.md\" <<<\"$ps_out\"")
        .expect("the single-driver probe grep");
    let at_reaper = loopd
        .find("LOOP_REAPER_PAGE_FILE=\"$STATE/reaper-page\" scripts/orphan-reaper.sh >> \"$LOG\" 2>&1 \\")
        .expect("the reaper sweep call");
    let at_build = loopd
        .find("CARGO_TARGET_DIR=\"$ROOT/target\" cargo build --release")
        .expect("the build gate");
    assert!(
        at_probe < at_reaper && at_reaper < at_build,
        "the sweep must sit between the single-driver probe and the build \
         gate (probe@{at_probe} < reaper@{at_reaper} < build@{at_build})"
    );
    assert!(
        loopd.contains(
            "orphan-reaper: nonzero exit (best-effort, ignored — the cycle proceeds)"
        ),
        "the sweep call must be best-effort guarded (the eval-digest shape) — \
         a reaper failure must never block the launch under set -e"
    );

    // The needle legs and the fail-closed doctrine, pinned in the helper.
    assert!(
        helper.contains("kill -TERM"),
        "the kill leg is SIGTERM only — no SIGKILL (a survivor is re-judged \
         by the next cycle's sweep)"
    );
    assert!(
        !helper.contains("kill -9") && !helper.contains("kill -KILL") && !helper.contains("kill -SIGKILL"),
        "no SIGKILL anywhere in the reaper — precision means the kill is \
         always revocable"
    );
    assert!(
        helper.contains("skip pid=$pid (unresolved)"),
        "the spec-pinned unresolved-skip line must exist verbatim"
    );
    assert!(
        helper.contains("LOOP_REAPER:-1") && helper.contains("disabled (LOOP_REAPER=0)"),
        "the opt-out must default ON and log its disabling"
    );
    assert!(
        helper.contains("LOOP_REAPER_DRY_RUN"),
        "the dry-run seam must exist (identification without signalling)"
    );
    assert!(
        helper.contains("LOOP_REAPER_MAX:-64"),
        "the examination bound must default to 64 (the bounded sweep)"
    );
    assert!(
        helper.contains("definitionally orphaned") && helper.contains("sweep-point invariant"),
        "the header must state the sweep-point invariant: at the sweep point \
         no current-cycle process can exist by construction"
    );
    assert!(
        helper.contains("own process group"),
        "the own-process-group exclusion must be pinned"
    );
    assert!(
        helper.contains("-ax -o pid= -o command=")
            && !helper.contains("ps -ax -o pid= -o command= |"),
        "the enumeration must be captured, never a ps-to-X pipeline (the T137 \
         house rule)"
    );

    // The README sentence: the reaper exists, where it runs, the opt-out.
    assert!(
        readme.contains("orphan-reaper") && readme.contains("LOOP_REAPER=0"),
        "the README loopd section must mention the reaper, its sweep point, \
         and LOOP_REAPER=0"
    );
}
