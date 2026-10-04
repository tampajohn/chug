//! T215 — the daemon ensure spawns a DAEMON-CAPABLE binary, not the repo dev
//! build. Operator diagnosis 2026-10-03 (K7): `daemon ensure` had exited
//! nonzero at EVERY cycle start since T204 landed (~16h, fail-open, so no
//! operational impact). Two stacked causes: loopd's ensure spawned the repo
//! release build (feature-lean by T204 — its `daemon` subcommand is the
//! fail-open stub that exits with "built without the judge daemon"), and the
//! release workflow's daemon-capable binaries sat uninstalled in
//! ~/.local/bin (the dogfood upgrade path hadn't pulled v0.15/v0.16). The
//! daemon is HOST-SCOPED (one per box, serves any run), so the right carrier
//! is the INSTALLED release binary.
//!
//! The fix under test: loopd.sh resolves the daemon binary ONCE per run —
//! (a) `$CHUG_DAEMON_BIN` when executable, (b) `$HOME/.local/bin/chug` when
//! the probe (`chug daemon --help` exit 0 — a pre-T204 release refuses the
//! subcommand) reports daemon support, (c) the repo release build when it
//! hosts the judge (the feature-off refusal literal ABSENT from its bytes) —
//! logs one startup line (chosen binary or the skip reason), and has the
//! per-cycle ensure spawn `$DAEMON_BIN`. Nothing daemon-capable → the ensure
//! is skipped for the whole run behind that one log line (fail-open, the
//! judge client degrades per command as before).
//!
//! These tests run the REAL loopd.sh in a sandbox against stub `ps`/`cargo`
//! and stub chug binaries (the tests/loopd_stale_binary.rs pattern: the
//! tests guard the script, they do not reimplement it). The stubs play the
//! resolution legs: a daemon-capable installed release, a pre-T204 installed
//! release (unrecognized subcommand), and feature-on/feature-off repo
//! builds — discriminated exactly as production discriminates them (the
//! refusal literal in the bytes, the probe's exit code). `chug daemon
//! --ensure` never touches a real daemon, socket, or model here: the stubs
//! only append their identity to a log file the assertions read.

#![cfg(unix)]

use std::fs;
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use tempfile::TempDir;

// T214: the load-scaled verdict fence (`load_scaled_deadline`) lives in the
// ONE shared test-support module, joined by the T159 `#[path]`-include
// pattern — the same declaration every adopting family compiles, never a
// copy. This file carries no T151/T172 lock membership and gains none
// (T214 req 5: the families' timing-lock membership is untouched); the
// include exists so the 90s verdict fence routes through the shared helper.
#[path = "../src/testsupport.rs"]
mod testsupport;

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

/// The feature-off refusal literal — the (c)-leg discriminator's needle. It
/// must be the SAME text src/daemon.rs compiles into the feature-off binary
/// (real_backend's cfg(not(feature = "daemon")) bail): the shell probe greps
/// the binary for it, so a reword there must move this pin too (pinned below
/// by `probe_literal_is_coupled_to_the_daemon_source`).
const FEATURE_OFF_LITERAL: &str = "built without the judge daemon";

/// The startup log line when a binary resolved (the `echo` in loopd.sh —
/// pinned as wiring; the sandbox tests assert the RENDERED line).
const CHOSEN_LOG: &str = "judge daemon binary: $DAEMON_BIN";
/// The startup log line when nothing daemon-capable resolved — ONE per run
/// (the probe cache), never one per cycle.
const SKIP_LOG: &str = "judge daemon: ensure skipped for this run";

/// The stub-chug contract: every `daemon --help` (the (b) probe) and every
/// `daemon --ensure` (the per-cycle step) appends `<verb> <$0>` to
/// `$CHUG_DAEMON_TEST_LOG`, so the assertions see WHO was probed/ensured and
/// how often — the probe-cache pin counts probe lines across cycles. The
/// `--help` leg exits `EXIT` (0 = daemon-capable; 2 = a pre-T204 release's
/// "unrecognized subcommand"); `$0` keeps the installed-binary, override,
/// and repo stubs distinguishable in the shared log. No trailing exit here:
/// unmatched verbs FALL THROUGH to each stub's own tail (the cycle-child
/// lines for the repo stub; the unexpected-call guard for the others).
const STUB_DAEMON_LOGIC: &str = r#"log="${CHUG_DAEMON_TEST_LOG:-/dev/null}"
case "$1 $2" in
  "daemon --help")
    printf 'probe %s\n' "$0" >> "$log"
    exit EXIT ;;
  "daemon --ensure")
    printf 'ensure %s\n' "$0" >> "$log"
    exit 0 ;;
esac
"#;

/// A stub binary with the daemon-verb legs; `capable` decides the probe's
/// exit code (baked into the bytes — no shared env to leak between stubs).
/// Any verb beyond the daemon legs hits the unexpected-call guard (rc 9).
fn daemon_stub(capable: bool) -> String {
    format!(
        "#!/bin/sh\n{}\nexit 9  # unexpected call — the stub only answers daemon verbs\n",
        STUB_DAEMON_LOGIC.replace("EXIT", if capable { "0" } else { "2" })
    )
}

/// The repo release build at `./target/release/chug`: the cycle child AND
/// the (c) resolution leg. As the cycle child it prints the goal-complete
/// block and stops the supervisor after `stop_after` cycles (counter file —
/// the multi-cycle harness for the probe-cache pin). As a (c) candidate its
/// BYTES decide: `feature_on = false` embeds the feature-off refusal literal
/// (a feature-lean T204 build — loopd's own), `true` omits it (an operator's
/// feature-on build; never produced by loopd's build gate).
fn repo_chug_body(stop_after: usize, feature_on: bool) -> String {
    let mut body = String::from("#!/bin/sh\n");
    body.push_str(&STUB_DAEMON_LOGIC.replace("EXIT", "0"));
    body.push_str(&format!(
        r#"
# cycle child: accept the goal, count the cycle, stop after {stop_after}
printf 'chug: goal complete\nsummary: T215 sandbox cycle\n'
mkdir -p .chug
n=$(cat .chug/cycles 2>/dev/null || echo 0); n=$((n+1)); echo "$n" > .chug/cycles
if [ "$n" -ge {stop_after} ]; then touch .chug/STOP-LOOP; fi
exit 0
"#
    ));
    if !feature_on {
        // The (c) discriminator's negative: these bytes carry the literal the
        // feature-off build compiles in (real_backend's refusal), so the
        // grep reads this stub as CLIENTS-ONLY.
        body.push_str(&format!(
            "# {FEATURE_OFF_LITERAL} (the default): rebuild to host the judge\n"
        ));
    }
    body
}

struct Sandbox {
    // Holds the tempdir open for the test's lifetime; never read directly.
    _keep: TempDir,
    root: PathBuf,
    /// The installed stub's probe behavior (`with_installed`), replayed as an
    /// env by `run_loopd` — belt-and-braces against the outer test
    /// environment carrying a CHUG_DAEMON_BIN (scenarios without an override
    /// must not inherit one).
    installed_capable: bool,
}

impl Sandbox {
    /// `feature_on_repo` plays the (c) leg's byte state; `stop_after` sets
    /// how many cycles the cycle child accepts before STOP-LOOP.
    fn new(feature_on_repo: bool, stop_after: usize) -> Sandbox {
        let keep = tempfile::tempdir().expect("sandbox tempdir");
        // Canonicalize: loopd resolves its ROOT via `cd && pwd`, which on
        // macOS turns /var/folders/... into /private/var/folders/... — every
        // path loopd LOGS (the chosen-binary line, the ensure's $0) carries
        // the resolved form, so the sandbox root must match it.
        let root = fs::canonicalize(keep.path()).expect("canonicalize sandbox root");
        // The real script + its scripts/ helpers, byte-for-byte.
        fs::copy(repo_root().join("loopd.sh"), root.join("loopd.sh")).expect("copy loopd.sh");
        let scripts = root.join("scripts");
        fs::create_dir_all(&scripts).expect("scripts dir");
        for entry in fs::read_dir(repo_root().join("scripts")).expect("scripts dir") {
            let entry = entry.expect("scripts entry");
            fs::copy(entry.path(), scripts.join(entry.file_name())).expect("copy script");
        }
        // PATH stubs: `ps` reports no processes (the T53 single-driver probe
        // must not see a REAL driver — e.g. the outer run executing this
        // very test — or the sandbox loopd skips its cycle); `cargo` plays
        // the build gate (always green here — the build is not T215's
        // subject).
        let bin = root.join("bin");
        fs::create_dir_all(&bin).expect("bin dir");
        stub(&bin.join("ps"), "#!/bin/sh\nexit 0\n");
        stub(&bin.join("cargo"), "#!/bin/sh\nexit 0\n");
        // The repo release build: the cycle child + (c) candidate.
        let chug_dir = root.join("target/release");
        fs::create_dir_all(&chug_dir).expect("target/release dir");
        stub(
            &chug_dir.join("chug"),
            &repo_chug_body(stop_after, feature_on_repo),
        );
        // site-sync must NEVER leave the sandbox (a live ~/workspace/chug-site
        // on this host would get synced from fixture garbage): point the env
        // default at a path that does not exist (warn + exit 0 leg).
        fs::write(root.join("NO-SUCH-SITE"), "").expect("site sentinel");
        Sandbox {
            _keep: keep,
            root,
            installed_capable: false,
        }
    }

    /// A daemon-capable or pre-T204 `~/.local/bin/chug` inside the sandbox's
    /// HOME (the (b) leg's candidate).
    fn with_installed(&mut self, capable: bool) {
        let dir = self.root.join("home/.local/bin");
        fs::create_dir_all(&dir).expect("home/.local/bin dir");
        stub(&dir.join("chug"), &daemon_stub(capable));
        self.installed_capable = capable;
    }

    /// An explicit `$CHUG_DAEMON_BIN` override stub (the (a) leg) —
    /// daemon-capable by construction (it answers `--ensure`).
    fn with_override(&self) -> PathBuf {
        let path = self.root.join("bin/override-chug");
        stub(&path, &daemon_stub(true));
        path
    }

    fn run_loopd(&self, override_bin: Option<&Path>) -> Child {
        let mut cmd = Command::new("bash");
        cmd.arg("loopd.sh").arg("run").current_dir(&self.root);
        // Sandbox bin first: ps/cargo stubs shadow the host's.
        let path = format!(
            "{}:{}",
            self.root.join("bin").display(),
            std::env::var("PATH").unwrap_or_default()
        );
        cmd.env("PATH", path);
        // The (b) probe resolves through HOME: point it INSIDE the sandbox so
        // the test never probes the host's real ~/.local/bin/chug (this dev
        // machine may legitimately carry one).
        cmd.env("HOME", self.root.join("home"));
        cmd.env("CHUG_SITE_DIR", self.root.join("NO-SUCH-SITE"));
        cmd.env("CHUG_SITE_SYNC_NO_PUSH", "1");
        // The sleep seams (CHUG_ROUTINE_TODAY pattern): a failed verdict or
        // build would otherwise park the loop 60–300s. The harness kills the
        // loop at its predicate anyway; these only bound a missed kill.
        cmd.env("LOOPD_SLEEP_OK", "1");
        cmd.env("LOOPD_SLEEP_FAIL", "1");
        // The stub-chug contract's log: who probed/ensured, how often.
        cmd.env("CHUG_DAEMON_TEST_LOG", self.root.join("daemon-test.log"));
        // The (a) leg or its explicit absence — the outer env must never leak
        // an override into a scenario that did not ask for one.
        match override_bin {
            Some(path) => {
                cmd.env("CHUG_DAEMON_BIN", path);
            }
            None => {
                cmd.env_remove("CHUG_DAEMON_BIN");
            }
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

    fn read_stub_log(&self) -> String {
        let mut content = String::new();
        if let Ok(mut f) = fs::File::open(self.root.join("daemon-test.log")) {
            let _ = f.read_to_string(&mut content);
        }
        content
    }

    /// Poll the supervisor log until `needle` occurs `min` times (the
    /// verdict/stub lines are written BEFORE the inter-cycle sleep, so
    /// seeing the count means the decisions are final), then kill the loop
    /// and return both logs. The panic text carries both logs — the
    /// T137-style forensic record.
    fn run_until(&self, child: &mut Child, needle: &str, min: usize) -> (String, String) {
        // T214: the 90s BASE is unchanged (the zero-timeout-bump doctrine) —
        // the fence's BASIS is now the host's measured load: base ×
        // clamp(loadavg_1m/cores, 1.0, 4.0), fail-safe to exactly the base
        // when the load seam fails. A quiet host sees byte-identical
        // behavior; a gate-load-melted host gets up to 4× before the fence
        // blows; a truly hung child still fails, fast.
        let deadline = Instant::now() + testsupport::load_scaled_deadline(Duration::from_secs(90));
        loop {
            let log = self.read_log();
            if log.matches(needle).count() >= min {
                let _ = child.kill();
                let _ = child.wait();
                return (log, self.read_stub_log());
            }
            if Instant::now() > deadline {
                let _ = child.kill();
                let _ = child.wait();
                let stub_log = self.read_stub_log();
                panic!(
                    "loopd never reached {needle}×{min} within the \
                     load-scaled verdict fence (T214: base 90s × measured \
                     load factor).\n\
                     --- loopd.log ---\n{log}\n--- daemon-test.log ---\n{stub_log}"
                );
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

fn stub(path: &Path, body: &str) {
    fs::write(path, body).expect("write stub");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("chmod stub");
}

/// The rendered startup line names the installed release binary — and the
/// per-cycle ensure SPAWNED it (the `ensure <$0>` stub record), never the
/// repo dev build. This is K7's acceptance shape: a daemon-capable installed
/// binary now carries the ensure.
#[test]
fn installed_daemon_capable_binary_carries_the_ensure_not_the_repo_dev_build() {
    let mut sb = Sandbox::new(true, 1); // (c) would also fire — proves (b) beats (c)
    sb.with_installed(true);
    let (log, stub_log) = {
        let mut child = sb.run_loopd(None);
        sb.run_until(&mut child, "cycle OK", 1)
    };
    let installed = sb.root.join("home/.local/bin/chug");
    count_eq(&log, &format!("judge daemon binary: {}", installed.display()), 1, "startup line names the installed binary");
    count_eq(&stub_log, &format!("ensure {}", installed.display()), 1, "the ensure spawned the installed binary");
    let repo = sb.root.join("target/release/chug");
    assert!(
        !stub_log.contains(&format!("ensure {}", repo.display())),
        "the ensure must never spawn the repo dev build when the installed \
         release is daemon-capable\n{stub_log}"
    );
    count_eq(&stub_log, &format!("probe {}", installed.display()), 1, "exactly one probe (the (b) leg's, cached for the run)");
}

/// Req 1(a): the explicit override wins over a daemon-capable installed
/// binary — and wins WITHOUT being probed (an explicit choice is not
/// second-guessed): zero probe lines in the record.
#[test]
fn override_wins_even_over_a_daemon_capable_installed_binary() {
    let mut sb = Sandbox::new(true, 1);
    sb.with_installed(true);
    let override_bin = sb.with_override();
    let (log, stub_log) = {
        let mut child = sb.run_loopd(Some(&override_bin));
        sb.run_until(&mut child, "cycle OK", 1)
    };
    count_eq(&log, &format!("judge daemon binary: {}", override_bin.display()), 1, "startup line names the override");
    count_eq(&stub_log, &format!("ensure {}", override_bin.display()), 1, "the ensure spawned the override");
    let installed = sb.root.join("home/.local/bin/chug");
    assert!(
        !stub_log.contains(&format!("ensure {}", installed.display())),
        "the installed binary must not be ensured when the override is set\n{stub_log}"
    );
    assert!(
        !stub_log.contains("probe"),
        "the (a) leg is probe-free: an explicit override is not second-guessed\n{stub_log}"
    );
}

/// The probe cache: ONE `chug daemon --help` per loopd RUN, not per cycle —
/// two full cycles still show a single probe line while every cycle ran its
/// own ensure.
#[test]
fn probe_is_cached_one_probe_per_loopd_run_not_per_cycle() {
    let mut sb = Sandbox::new(true, 2); // the cycle child stops the loop after cycle 2
    sb.with_installed(true);
    let (log, stub_log) = {
        let mut child = sb.run_loopd(None);
        sb.run_until(&mut child, "cycle OK", 2)
    };
    count_eq(&log, "cycle OK", 2, "two cycles actually ran");
    let installed = sb.root.join("home/.local/bin/chug");
    count_eq(&stub_log, &format!("probe {}", installed.display()), 1, "ONE probe per loopd run (T215 probe cache)");
    count_eq(&stub_log, &format!("ensure {}", installed.display()), 2, "one ensure per cycle");
    count_eq(&log, "judge daemon binary:", 1, "the resolution logged once, not per cycle");
}

/// The K7 shape: a pre-T204 installed release (the probe's exit 2) falls
/// through to (c) — and a feature-ON repo build (an operator's, never
/// loopd's own) then carries the ensure.
#[test]
fn stale_installed_binary_falls_through_to_a_feature_on_repo_build() {
    let mut sb = Sandbox::new(true, 1);
    sb.with_installed(false); // pre-T204: unrecognized subcommand
    let (log, stub_log) = {
        let mut child = sb.run_loopd(None);
        sb.run_until(&mut child, "cycle OK", 1)
    };
    let installed = sb.root.join("home/.local/bin/chug");
    let repo = sb.root.join("target/release/chug");
    count_eq(&log, &format!("judge daemon binary: {}", repo.display()), 1, "startup line names the repo build");
    count_eq(&stub_log, &format!("probe {}", installed.display()), 1, "the stale binary WAS probed exactly once");
    count_eq(&stub_log, &format!("ensure {}", repo.display()), 1, "the ensure fell through to the feature-on repo build");
    assert!(
        !stub_log.contains(&format!("ensure {}", installed.display())),
        "a pre-T204 installed binary must never be spawned for the ensure\n{stub_log}"
    );
}

/// The observed K7 failure, end to end: stale installed binary AND a
/// feature-less repo build (loopd's own feature-lean build) → the ensure is
/// skipped for the whole run behind ONE startup log line — no per-cycle
/// spawn attempts, no per-cycle spam, the cycle itself still runs (fail-open).
#[test]
fn featureless_repo_build_falls_through_to_one_skip_log_line() {
    let mut sb = Sandbox::new(false, 1);
    sb.with_installed(false); // pre-T204
    let (log, stub_log) = {
        let mut child = sb.run_loopd(None);
        sb.run_until(&mut child, "cycle OK", 1)
    };
    count_eq(&log, SKIP_LOG, 1, "exactly ONE skip line for the whole run");
    count_eq(&log, "the repo release build is feature-lean (T204: clients only)", 1, "the skip reason names the feature-lean repo build");
    assert!(
        !log.contains("judge daemon binary:"),
        "no binary resolved — no chosen-binary line\n{log}"
    );
    assert!(
        !stub_log.contains("ensure"),
        "no stub was ever spawned for the ensure\n{stub_log}"
    );
    count_eq(&stub_log, "probe", 1, "the stale installed binary was probed once, then cached");
    count_eq(&log, "daemon ensure: nonzero exit", 0, "the skipped run never attempts a per-cycle ensure");
}

/// Nothing daemon-capable anywhere (no installed binary at all, feature-lean
/// repo build): one skip line naming the full miss, cycles still run.
#[test]
fn no_daemon_capable_binary_anywhere_skips_the_ensure() {
    let sb = Sandbox::new(false, 1); // no with_installed: empty sandbox HOME
    // The full-miss reason requires the repo build ABSENT too (a present
    // feature-lean repo build is the previous test's reason).
    fs::remove_file(sb.root.join("target/release/chug")).expect("remove the repo stub");
    let (log, stub_log) = {
        let mut child = sb.run_loopd(None);
        // No cycle child exists (it was the removed repo stub), so the
        // verdict never lands — the predicate is the startup skip line
        // itself (written once, before any cycle).
        sb.run_until(&mut child, SKIP_LOG, 1)
    };
    count_eq(&log, SKIP_LOG, 1, "exactly ONE skip line for the whole run");
    assert!(
        log.contains("no daemon-capable binary found"),
        "the skip reason must name the full miss (override, installed, repo build)\n{log}"
    );
    assert!(
        !stub_log.contains("ensure") && !stub_log.contains("probe"),
        "nothing was probed or ensured\n{stub_log}"
    );
}

// --- static wiring pins (the loopd_model_routing.rs layer) -------------------

/// The resolution order is wired exactly once per leg, the per-cycle ensure
/// spawns the resolved binary, and the old repo-dev-build spawn is GONE.
#[test]
fn loopd_resolution_order_and_ensure_wiring() {
    let loopd = read("loopd.sh");
    count_eq(&loopd, "if [ -n \"${CHUG_DAEMON_BIN:-}\" ]; then", 1, "leg (a): the CHUG_DAEMON_BIN override");
    count_eq(&loopd, "local installed=\"${HOME:-}/.local/bin/chug\"", 1, "leg (b): the installed release path");
    count_eq(&loopd, "judge_probe_ok \"$installed\"", 1, "leg (b): the daemon --help probe");
    count_eq(&loopd, "grep -aq \"built without the judge daemon\" \"$repo_bin\"", 1, "leg (c): the feature-off literal grep");
    count_eq(&loopd, CHOSEN_LOG, 1, "the chosen-binary startup line");
    count_eq(&loopd, SKIP_LOG, 1, "the skip startup line");
    // The per-cycle ensure spawns the RESOLVED binary; the repo dev build is
    // gone from the ensure (K7's bug).
    count_eq(
        &loopd,
        "\"$DAEMON_BIN\" daemon --ensure >/dev/null 2>&1 \\",
        1,
        "the per-cycle ensure spawns $DAEMON_BIN",
    );
    count_eq(
        &loopd,
        "\"$ROOT/target/release/chug\" daemon --ensure",
        0,
        "the old repo-dev-build ensure line is gone (T215's root cause)",
    );
    // Resolution happens ONCE, before the cycle loop (the probe cache) — the
    // startup log lines precede the while body that consumes $DAEMON_BIN.
    let chosen = loopd.find(CHOSEN_LOG).expect("chosen line present");
    let skip = loopd.find(SKIP_LOG).expect("skip line present");
    let loop_top = loopd
        .find("while [ ! -f \"$STOP\" ]")
        .expect("loopd.sh has a supervisor loop");
    let ensure = loopd
        .find("\"$DAEMON_BIN\" daemon --ensure")
        .expect("per-cycle ensure present");
    assert!(
        chosen < loop_top && skip < loop_top && ensure > loop_top,
        "resolution + its log live before the cycle loop; the ensure inside it"
    );
    // The LOOP_DAEMON_ENSURE opt-out survives (the LOOP_REAPER pattern), now
    // conjunct with the resolved-binary guard.
    count_eq(
        &loopd,
        "if [ \"${LOOP_DAEMON_ENSURE:-1}\" = \"1\" ] && [ -n \"$DAEMON_BIN\" ]; then",
        1,
        "the opt-out + resolved-binary guard",
    );
}

/// The (c) leg's discriminator is coupled to the source it reads: the
/// literal the shell greps the binary for must be the literal
/// src/daemon.rs's feature-off bail compiles in. A reword on either side
/// without the other silently flips leg (c) — this pin forces the pair to
/// move together.
#[test]
fn probe_literal_is_coupled_to_the_daemon_source() {
    let loopd = read("loopd.sh");
    let daemon = read("src/daemon.rs");
    count_eq(&loopd, FEATURE_OFF_LITERAL, 1, "the probe needle in loopd.sh");
    count_eq(&daemon, FEATURE_OFF_LITERAL, 1, "the compiled refusal in src/daemon.rs");
}

/// Req 2 — loopd's repo builds stay feature-lean (the T204 design stands):
/// the build gate line must carry no --features. The daemon comes from the
/// installed release binary; the repo build is a client.
#[test]
fn loopds_own_builds_stay_feature_lean() {
    let loopd = read("loopd.sh");
    let build_line = loopd
        .lines()
        .find(|l| l.contains("cargo build --release"))
        .expect("the build gate line exists")
        .trim();
    assert!(
        !build_line.contains("--features"),
        "loopd's own build must stay feature-lean (T204 req 2; T215 req 2): \
         {build_line:?}"
    );
    assert!(
        build_line.contains("CARGO_TARGET_DIR=\"$ROOT/target\""),
        "the build gate pins its own target dir (T137): {build_line:?}"
    );
}

/// Req 4 — the carrier doctrine is written down where an operator looks:
/// DEPENDENCIES.md's judge-daemon env row and runbooks/loop-ops.md's daemon
/// section both say the daemon comes from the INSTALLED release binary on
/// loop hosts and that repo dev builds are clients only.
#[test]
fn carrier_doctrine_is_pinned_in_the_docs() {
    let deps = read("DEPENDENCIES.md");
    count_eq(&deps, "CHUG_DAEMON_BIN", 1, "the override knob is documented");
    assert!(
        deps.contains("the daemon comes from the INSTALLED release binary on loop hosts"),
        "DEPENDENCIES.md must carry the carrier doctrine (T215 req 4)"
    );
    let ops = read("runbooks/loop-ops.md");
    count_eq(&ops, "installed release binary", 1, "the runbook names the carrier");
    count_eq(&ops, "CLIENTS ONLY", 1, "the runbook names repo dev builds clients-only");
}

/// Req 3 — the pinned cold-load remedy: the runbook's daemon entry documents
/// the pre-warm one-liner (`chug daemon` foreground once) instead of growing
/// the ensure's wait budget, and names the warm marker to watch for.
#[test]
fn prewarm_one_liner_is_pinned_in_the_runbook() {
    let ops = read("runbooks/loop-ops.md");
    count_eq(&ops, "~/.local/bin/chug daemon", 1, "the pre-warm one-liner");
    count_eq(&ops, "judge model warm", 1, "the readiness marker to Ctrl-C on");
    count_eq(&ops, "LOOP_DAEMON_ENSURE=0", 1, "the opt-out is documented");
}
