//! T50 — loopd re-execs itself between cycles when its own script changed.
//!
//! Static pins on loopd.sh (the tests/shared_target_dir.rs pattern: loopd
//! changes are static-pin + review + validator territory since T27/T47 —
//! spawning real cycles from the test suite is impractical). Before this
//! fix, loopd.sh activated its own merges only when the operator restarted
//! it: three landed changes (T36 budget 120→160, T46 digest refresh, T47
//! shared-cache prefix) sat dormant for 7 cycles. The fix re-execs BETWEEN
//! cycles, at the top of the while body — the one moment a freshly `exec`'d
//! process can re-read the script with zero incremental-read exposure.
//!
//! These pins are deliberately brittle: the suite must fail if the re-exec
//! block is deleted, moved out of the while-top position, or the same-pid
//! pidfile guard is reverted.

use std::path::PathBuf;

fn repo_root() -> PathBuf {
    std::env::current_dir().expect("cargo sets the test cwd to the package root")
}

fn read(rel: &str) -> String {
    std::fs::read_to_string(repo_root().join(rel))
        .unwrap_or_else(|e| panic!("reading {rel}: {e}"))
}

/// The fingerprint record: POSIX cksum is content-based, so `touch` or a
/// content-preserving checkout must not trigger a spurious re-exec.
const SELF_CKSUM: &str = "SELF_CKSUM=\"$(cksum \"$ROOT/loopd.sh\")\"";
/// The live compare against the recorded fingerprint, at the while-top.
const REEXEC_COMPARE: &str = "[ \"$(cksum \"$ROOT/loopd.sh\")\" != \"$SELF_CKSUM\" ]";
/// The re-exec itself.
const REEXEC_EXEC: &str = "exec \"$ROOT/loopd.sh\" run";
/// The re-exec log line — loopd.log must always explain a budget/behavior
/// change; a silent re-exec would be undiagnosable.
const REEXEC_LOG: &str = "loopd: script changed on disk — re-exec (pid $$)";
/// The single-driver check (T53): ps-based, because pgrep persistently fails
/// to enumerate the launchd-spawned loopd tree on this host (pgrep -f/-l/-P
/// all miss a live in-tree driver; ps sees it every time — cycle-24 eval I1),
/// so a pgrep-based guard fails OPEN. The `[c]hug` bracket excludes the grep
/// pipeline's own argv from the match.
const DRIVER_CHECK: &str = "ps -ax -o command= | grep -q \"[c]hug run --spec LOOP-SPEC.md\"";

#[test]
fn loopd_fingerprints_itself_before_the_cycle_loop() {
    let loopd = read("loopd.sh");
    assert!(
        loopd.contains(SELF_CKSUM),
        "loopd.sh must record a cksum content fingerprint of itself \
         ({SELF_CKSUM}) before the cycle loop (T50)"
    );
    // Position: BEFORE the while loop — it is the baseline the while-top
    // compare is measured against.
    let fp = loopd
        .find(SELF_CKSUM)
        .expect("SELF_CKSUM fingerprint record (T50)");
    let loop_top = loopd
        .find("while [ ! -f \"$STOP\" ]")
        .expect("loopd.sh has a supervisor loop");
    assert!(
        fp < loop_top,
        "SELF_CKSUM must be recorded BEFORE the cycle loop (T50)"
    );
}

#[test]
fn loopd_reexecs_at_the_top_of_the_while_body() {
    let loopd = read("loopd.sh");
    let compare = loopd
        .find(REEXEC_COMPARE)
        .expect("the re-exec must be gated on a live cksum compare against \
                  the recorded SELF_CKSUM fingerprint (T50)");
    let exec = loopd
        .find(REEXEC_EXEC)
        .expect("loopd.sh must re-exec itself when its script changed on \
                  disk (T50)");
    // FIRST statement inside the while body — before the single-driver
    // check and before `cargo build` — so a re-exec only ever happens
    // BETWEEN cycles, never mid-cycle. The while condition itself is the
    // STOP guard: evaluated before the body, a pending stop exits without
    // re-exec'ing.
    let loop_top = loopd
        .find("while [ ! -f \"$STOP\" ]")
        .expect("loopd.sh has a supervisor loop");
    let driver_check = loopd
        .find(DRIVER_CHECK)
        .expect("loopd.sh has the ps-based single-driver check (T53): \
                  `ps -ax -o command= | grep -q \"[c]hug run --spec \
                  LOOP-SPEC.md\"` — pgrep is blind to the launchd-spawned \
                  loopd tree on this host, so a pgrep guard fails OPEN");
    let build = loopd
        .find("  cargo build >> \"$LOG\" 2>&1")
        .expect("loopd.sh builds its own binary");
    assert!(
        loop_top < compare && compare < exec && exec < driver_check && exec < build,
        "the re-exec block must be the FIRST statement inside the while \
         body — before the single-driver check and `cargo build` — so it \
         only ever runs BETWEEN cycles, never mid-cycle (T50)"
    );
}

#[test]
fn loopd_logs_the_reexec_before_execing() {
    let loopd = read("loopd.sh");
    let log = loopd
        .find(REEXEC_LOG)
        .expect("a re-exec must be logged to $LOG so .chug/loopd/loopd.log \
                  always explains a budget/behavior change (T50)");
    let exec = loopd
        .find(REEXEC_EXEC)
        .expect("loopd.sh must re-exec itself when its script changed on \
                  disk (T50)");
    assert!(
        log < exec,
        "the re-exec log line must precede the exec — a re-exec must never \
         be silent (T50)"
    );
}

#[test]
fn loopd_pidfile_guard_passes_for_the_same_pid() {
    let loopd = read("loopd.sh");
    // The run-mode guard is the unindented `if [ -f "$PIDFILE" ]` line; the
    // `status` branch's check is indented inside the case, so an exact
    // column-0 match selects the guard this item amends. The statement is
    // `\`-continued across physical lines — take it whole, down to `then`.
    let lines: Vec<&str> = loopd.lines().collect();
    let start = lines
        .iter()
        .position(|l| l.starts_with("if [ -f \"$PIDFILE\" ]"))
        .expect("loopd.sh has a run-mode pidfile guard");
    let end = start
        + lines[start..]
            .iter()
            .position(|l| l.contains("then"))
            .expect("the pidfile guard terminates with `then`");
    let guard = lines[start..=end].join("\n");
    assert!(
        guard.contains("cat \"$PIDFILE\"") && guard.contains("!=") && guard.contains("\"$$\""),
        "the pidfile guard must compare the recorded pid against $$ (T50): \
         exec preserves the pid, so a re-exec'd self finds its own LIVE pid \
         in the pidfile and must fall through, not be refused. Guard:\n\
         {guard}"
    );
    // Refusal still requires liveness: a stale (dead) pid keeps today's
    // fall-through; only a live FOREIGN supervisor is refused.
    assert!(
        guard.contains("kill -0"),
        "the pidfile guard must keep requiring liveness (kill -0) — a stale \
         pid falls through, a foreign live supervisor is refused (T50). \
         Guard:\n{guard}"
    );
}

#[test]
fn loopd_single_driver_check_does_not_use_pgrep() {
    let loopd = read("loopd.sh");
    assert!(
        !loopd.contains("pgrep -f \"chug run --spec LOOP-SPEC.md\""),
        "loopd.sh must NOT guard the single-driver check with \
         `pgrep -f \"chug run --spec LOOP-SPEC.md\"`: on this host pgrep \
         persistently fails to enumerate the launchd-spawned loopd tree \
         (pgrep -f/-l/-P all miss a live in-tree driver while ps sees it \
         every time), so a pgrep-based guard fails OPEN and duplicate \
         drivers become possible (T53, cycle-24 eval I1)"
    );
    assert!(
        loopd.contains(DRIVER_CHECK),
        "loopd.sh must detect an active driver with the ps pipeline \
         ({DRIVER_CHECK}) — the only enumeration that sees the \
         launchd-spawned loopd tree on this host (T53, cycle-24 eval I1)"
    );
}

#[test]
fn loopd_single_driver_check_grep_excludes_itself_via_bracket_idiom() {
    let loopd = read("loopd.sh");
    // The `[c]hug` bracket idiom is load-bearing: a plain `grep -q "chug
    // ..."` pattern matches the grep process's OWN argv (its command line
    // contains the needle), so the check would fire on itself forever.
    assert!(
        loopd.contains("grep -q \"[c]hug run --spec LOOP-SPEC.md\""),
        "the single-driver grep pattern must use the `[c]hug` bracket idiom — \
         a plain `chug` needle matches the grep process's own argv, so the \
         guard would report another driver active every cycle (T53)"
    );
    // Exactly one check: a duplicated guard line would log + sleep 120 twice
    // per skipped cycle for no additional safety.
    let hits = loopd.matches("[c]hug run --spec LOOP-SPEC.md").count();
    assert_eq!(
        hits, 1,
        "the `[c]hug run --spec LOOP-SPEC.md` needle must occur exactly once \
         in loopd.sh — inside the single-driver check (T53)"
    );
}
