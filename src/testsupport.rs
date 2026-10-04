// T151: THE one shared serialization domain for the crate's wall-clock /
// spawn-timing test families. T31 (cycle 15) introduced this pattern as
// `RUN_SHELL_TIMING_LOCK`, private to src/tools.rs's test module; the
// cycle-70/71 default-parallelism flake storms (six fires: mcp_http's
// dead-port legs, the driver events/hooks/permissions drive_loop legs, the
// delegate launch poll legs — every one green in isolation, every one red
// under machine load) showed a per-file lock is not enough: two timing tests
// in DIFFERENT files could still run concurrently and manufacture scheduler
// stretch for each other. T151 moves the lock to ONE crate-visible location
// (this module, `#[cfg(test)]`-gated at its mod declaration) so no two
// wall-clock/spawn-timing tests in the process are ever in flight together,
// and converts the probe legs whose load misreads are timing-shaped to
// condition-polling (see src/mcp_http.rs). Nextest gates are unaffected
// (per-process isolation); the victims this protects are `cargo test`
// goal-gate runs at default parallelism.
use std::path::Path;
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// The shared test-timing serialization domain. Every test that asserts on
/// wall-clock windows, spawn timing, deadlines, or probe timing takes
/// [`timing_guard`] as its FIRST acquisition and holds it across the whole
/// timing-sensitive region (spawn → assertion → cleanup), so the only
/// stretch source left is whole-machine starvation — never the suite itself.
/// std-only, poison-tolerant (a panicking timing test must not cascade
/// `PoisonError` failures into its siblings).
///
/// Lock ORDER (deadlock-freedom rule): `timing_guard` is always the FIRST
/// acquisition in any test body. Tests that also mutate the process-global
/// env (`crate::delegate::tests::DELEGATE_ENV_LOCK`) take the timing guard
/// BEFORE the env lock; no test takes the env lock and then the timing
/// guard, so no cycle can form.
///
/// ONE definition, by construction (T151 req 5): the static is private — the
/// only access path in the whole crate is [`timing_guard`], so a second
/// domain cannot be reached through this one. And a second instance
/// introduced NEXT TO it (the realistic failure mode: copying this
/// declaration into another file instead of importing the helper) carries
/// the fixed export symbol below, and rustc rejects the duplicate definition
/// at build time — verified: `error: symbol
/// `chug_t151_test_timing_lock_singleton` is already defined`. (A
/// differently-named independent lock is the "re-implementing a second
/// independent lock is a finding" case: review blocks it.)
#[unsafe(export_name = "chug_t151_test_timing_lock_singleton")]
static TEST_TIMING_LOCK: Mutex<()> = Mutex::new(());

/// Poison-tolerant acquisition of the shared timing domain: recovers the
/// guard from a poisoned lock (a panic inside one timing test must not fail
/// its siblings with `PoisonError`).
pub(crate) fn timing_guard() -> MutexGuard<'static, ()> {
    TEST_TIMING_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

// ---------- T151 req 5: non-vacuousness pin ----------

/// The shared lock is a SINGLE domain: two threads overlapping acquisition
/// are SERIALIZED — the second's non-blocking attempt is refused while the
/// first holds the guard, and its blocking attempt completes only after the
/// first releases.
///
/// Load-flake-proof by construction: every assertion below is a
/// mutex-semantics ORDERING fact carried by channel handshakes — there is no
/// wall-clock bound, no sleep-race, and no elapsed window to stretch.
/// `try_lock` fails strictly while the guard is held and `lock` returns
/// strictly after the holder releases, no matter how the OS schedules the
/// two threads; machine load can only slow the probe down, never reorder it.
/// Even the channel waits are plain blocking `recv()`s (a wedged sibling
/// would hang, not flake, and nothing in this body can wedge one).
#[test]
fn t151_shared_lock_is_one_serialization_domain() {
    use std::sync::mpsc;
    use std::thread;

    // Take the domain for this body first (under default parallelism another
    // timing test may hold it; blocking here IS the serialization working —
    // and it also guarantees the refusal observed below is caused by OUR
    // guard, not a third party's).
    let held = timing_guard();

    let (tried_tx, tried_rx) = mpsc::channel::<bool>();
    let (done_tx, done_rx) = mpsc::channel::<&'static str>();
    let contended = thread::spawn(move || {
        // The overlapping non-blocking attempt: must be REFUSED while the
        // main thread holds the guard (mutual exclusion), recorded BEFORE
        // the blocking attempt so the two observations cannot swap.
        let overlapped_refused = TEST_TIMING_LOCK.try_lock().is_err();
        tried_tx.send(overlapped_refused).expect("receiver still alive");
        // The blocking attempt: returns only after the holder releases — the
        // serialization half of the proof.
        let _acquired = timing_guard();
        done_tx
            .send("acquired-after-release")
            .expect("receiver still alive");
    });

    // The sibling's overlapping attempt already happened (handshake) and was
    // refused: two concurrent acquirers never both hold the domain.
    let overlapped_refused = tried_rx.recv().expect("sibling thread alive");
    assert!(
        overlapped_refused,
        "a second acquisition attempt entered while the guard was held — \
         the timing domain is NOT exclusive"
    );

    // Release; only now may the blocked sibling enter. The `done_rx`
    // handshake completes only after the sibling's blocking acquisition
    // returned, which the Mutex guarantees is after our release above.
    drop(held);
    let done = done_rx.recv().expect("sibling thread alive");
    assert_eq!(done, "acquired-after-release");
    contended.join().expect("sibling thread finished cleanly");
}

// ---------- T214: load-scaled liveness fences ----------
//
// The spawn-heavy loopd families (tests/loopd_orphan_reaper.rs,
// tests/loopd_spoof_guard.rs, tests/loopd_daemon_ensure.rs) fence their
// child-verdict waits with a wall-clock deadline. Solo those waits finish
// in ~3.4s; under full-suite gate load on a busy host they have stretched
// PAST the fence (T152 measured 30.8s at 17-way parallelism — the
// deadline's own margin gone), and the flake signature cost the T82
// family-isolation tax (~2-5 extra minutes per gate, cycles 96/98). The
// fences are RIGHT to fail fast — a hung child must still blow them — what
// they get wrong is only their BASIS: a wall-clock constant on a host whose
// load the loop itself manufactures. T214 re-bases each fence on the host's
// MEASURED 1-minute load: base × clamp(loadavg_1m/cores, 1.0, 4.0). A quiet
// host sees byte-identical behavior (factor 1.0, the base passed through
// untouched); a melting host gets up to 4× before the fence blows; a truly
// hung child still fails, fast. Every seam failure (no sysctl, unreadable
// /proc, unparsable text, no core count) is fail-SAFE: factor 1.0, exactly
// today's behavior. ZERO timeout bumps: the base constants at the adoption
// sites are unchanged — this helper only re-bases the fence on measured
// load.

/// The load-scaled liveness fence (T214 req 1): `base` stretched by
/// `clamp(loadavg_1m / cores, 1.0, 4.0)`. The 1-minute load average is read
/// through a thin OS seam ([`read_loadavg_1m`]); cores come from
/// `std::thread::available_parallelism`. ANY seam failure → factor 1.0 →
/// exactly `base` (fail-safe to the pre-T214 fence; the helper never panics
/// and never returns less than `base` or more than 4× `base`).
pub fn load_scaled_deadline(base: Duration) -> Duration {
    scaled_deadline(
        read_loadavg_1m(),
        std::thread::available_parallelism()
            .ok()
            .and_then(|n| u32::try_from(n.get()).ok()),
        base,
    )
}

/// The pure composition behind [`load_scaled_deadline`]: the (optionally
/// failed) seam reads → the fence duration. A `None` on either side is a
/// failed read and fails SAFE to factor 1.0 — the deadline is exactly
/// `base` (T214 req 4). Split out so the fail-safe legs run without host
/// state, the same reason [`scale_factor`] is pure.
fn scaled_deadline(load: Option<f64>, cores: Option<u32>, base: Duration) -> Duration {
    let factor = match (load, cores) {
        (Some(load), Some(cores)) => scale_factor(load, cores),
        _ => 1.0,
    };
    if factor == 1.0 {
        // The quiet-host path is BYTE-IDENTICAL to the pre-T214 fence: the
        // base passes through with no float round-trip at all.
        return base;
    }
    base.mul_f64(factor)
}

/// The 1-minute load average through the thin OS seam (T214 req 1):
/// `sysctl -n vm.loadavg` on macOS, `/proc/loadavg` on Linux; ANY failure
/// (missing binary, nonzero exit, unreadable file, unparsable text) →
/// `None` → factor 1.0.
fn read_loadavg_1m() -> Option<f64> {
    parse_loadavg_1m(&loadavg_text()?)
}

/// The seam's raw text, per platform.
#[cfg(target_os = "macos")]
fn loadavg_text() -> Option<String> {
    // macOS prints braces: `{ 4.51 4.73 4.91 }` — the parser below takes
    // the first NUMERIC field, so the brace never becomes the load. sysctl
    // lives in /usr/sbin, which stripped gate PATHs can omit; try the PATH
    // lookup first, then the absolute location, so a shaved PATH degrades
    // to the absolute binary instead of silently pinning the factor at 1.0
    // forever. Both failing (and any nonzero exit) is the None leg.
    for sysctl in ["sysctl", "/usr/sbin/sysctl"] {
        let Ok(out) = std::process::Command::new(sysctl)
            .arg("-n")
            .arg("vm.loadavg")
            .output()
        else {
            continue;
        };
        if out.status.success() {
            return Some(String::from_utf8_lossy(&out.stdout).into_owned());
        }
    }
    None
}

#[cfg(target_os = "linux")]
fn loadavg_text() -> Option<String> {
    std::fs::read_to_string("/proc/loadavg").ok()
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn loadavg_text() -> Option<String> {
    None
}

/// The 1-minute load average is the first field of the seam's text that
/// parses to a finite, non-negative number: macOS's `{ 4.51 4.73 4.91 }`
/// and Linux's `3.42 1.90 1.55 2/1200 4242` both resolve by that rule, and
/// every garbage shape (an empty read, an error message, `NaN`, `inf`, a
/// negative) resolves to `None` — a garbage spawn result is a failed read,
/// never a factor (T214 req 1's ANY-failure clause).
fn parse_loadavg_1m(text: &str) -> Option<f64> {
    text.split_whitespace()
        .find_map(|field| {
            field
                .parse::<f64>()
                .ok()
                .filter(|load| load.is_finite() && *load >= 0.0)
        })
}

/// The pure scaling computation (T214 req 1): load-per-core clamped to
/// [1.0, 4.0] — a quiet host is factor 1.0, a melting host at most 4×.
/// Pure so the clamp legs are testable without host state; the degenerate
/// inputs fail SAFE to 1.0 (a NaN must never poison `clamp` into a NaN
/// factor, which would panic the Duration construction and take the
/// fence's test down with it).
fn scale_factor(load: f64, cores: u32) -> f64 {
    if cores == 0 || !load.is_finite() {
        return 1.0;
    }
    (load / f64::from(cores)).clamp(1.0, 4.0)
}

// T214 pins — the pure clamp legs and the fail-safe seam legs run in the
// bin's unit tests AND in every family binary that includes this module
// (the t151 pin's shape): the scaling is bound by tests, not by host state.

/// The clamp legs (T214 spec): quiet → 1.0, exactly at capacity → 1.0,
/// 2× capacity → 2.0, 4× capacity → the upper clamp, and a zero load never
/// shrinks the fence below its base.
#[test]
fn t214_scale_factor_clamp_legs() {
    assert_eq!(scale_factor(0.5, 8), 1.0, "a quiet host must see factor 1.0");
    assert_eq!(
        scale_factor(8.0, 8),
        1.0,
        "exactly at capacity is factor 1.0"
    );
    assert_eq!(scale_factor(16.0, 8), 2.0, "2x capacity must scale the fence 2x");
    assert_eq!(scale_factor(32.0, 8), 4.0, "4x capacity is the upper clamp");
    assert_eq!(
        scale_factor(0.0, 8),
        1.0,
        "a zero load must never shrink the fence below its base"
    );
    // Above the clamp and in the mid-band: the ratio, clamped.
    assert_eq!(scale_factor(400.0, 8), 4.0);
    assert_eq!(scale_factor(64.0, 4), 4.0);
    assert_eq!(scale_factor(36.0, 9), 4.0);
    assert_eq!(scale_factor(12.0, 8), 1.5);
    // Degenerate inputs fail SAFE: no NaN can poison the factor (a NaN
    // deadline would panic the Duration construction), and a zero core
    // count must never divide.
    assert_eq!(scale_factor(f64::NAN, 8), 1.0);
    assert_eq!(scale_factor(f64::INFINITY, 8), 1.0);
    assert_eq!(scale_factor(f64::NEG_INFINITY, 8), 1.0);
    assert_eq!(scale_factor(-3.0, 8), 1.0);
    assert_eq!(scale_factor(8.0, 0), 1.0);
}

/// The seam-failure legs (T214 req 4): a failed load read, a failed core
/// read, or both → the deadline is EXACTLY the base (byte-identical to the
/// pre-T214 fence); the scaled shape is bounded [base, 4×base] on both
/// sides and linear in the measured ratio.
#[test]
fn t214_seam_failures_fail_safe_to_the_exact_base() {
    let base30 = Duration::from_secs(30);
    let base90 = Duration::from_secs(90);
    assert_eq!(
        scaled_deadline(None, Some(8), base30),
        base30,
        "load unreadable → exactly the base"
    );
    assert_eq!(
        scaled_deadline(Some(64.0), None, base30),
        base30,
        "cores unreadable (the available_parallelism failure path) → exactly \
         the base, even under extreme load"
    );
    assert_eq!(
        scaled_deadline(None, None, base30),
        base30,
        "both seams unreadable → exactly the base"
    );
    // The scaled shape: linear in the ratio, clamped at 4×, never below.
    assert_eq!(
        scaled_deadline(Some(16.0), Some(8), base30),
        Duration::from_secs(60)
    );
    assert_eq!(
        scaled_deadline(Some(32.0), Some(8), base30),
        Duration::from_secs(120)
    );
    assert_eq!(
        scaled_deadline(Some(1000.0), Some(8), base90),
        Duration::from_secs(360),
        "the 90s daemon_ensure fence scales too, capped at 4×"
    );
    assert_eq!(
        scaled_deadline(Some(0.1), Some(8), base90),
        base90,
        "never below the base"
    );
}

/// The parser legs: the first numeric field of both platforms' shapes, and
/// garbage spawn results → None (a failed read, T214 req 1's ANY-failure
/// clause).
#[test]
fn t214_loadavg_parser_takes_the_first_numeric_field_and_rejects_garbage() {
    assert_eq!(
        parse_loadavg_1m("3.42 1.90 1.55 2/1200 4242"),
        Some(3.42),
        "Linux /proc/loadavg"
    );
    assert_eq!(
        parse_loadavg_1m("{ 4.51 4.73 4.91 }"),
        Some(4.51),
        "macOS sysctl prints braces"
    );
    assert_eq!(
        parse_loadavg_1m("0.52 0.58 0.59 1/485 12345\n"),
        Some(0.52),
        "trailing newline"
    );
    assert_eq!(parse_loadavg_1m(""), None, "an empty read is a failed read");
    assert_eq!(
        parse_loadavg_1m("vm.loadavg: unknown oid\n"),
        None,
        "an error message is not a load"
    );
    assert_eq!(
        parse_loadavg_1m("NaN inf -1.5"),
        None,
        "non-finite and negative are garbage"
    );
}

/// The live fence on THIS host (whatever its state): always within
/// [base, 4×base] — the invariant the clamps promise, asserted through the
/// real seam chain — and exactly the pure composition of the same seam
/// reads when the seams resolve.
#[test]
fn t214_live_deadline_stays_within_base_and_4x() {
    for base in [
        Duration::from_secs(30),
        Duration::from_secs(90),
        Duration::from_millis(1234),
    ] {
        let scaled = load_scaled_deadline(base);
        assert!(
            scaled >= base,
            "the fence must never shrink below its base: {scaled:?} < {base:?}"
        );
        assert!(
            scaled <= base * 4,
            "the fence must never exceed 4x its base: {scaled:?} > {:?}",
            base * 4
        );
        if let (Some(load), Ok(cores)) = (read_loadavg_1m(), std::thread::available_parallelism()) {
            let expected = scaled_deadline(
                Some(load),
                u32::try_from(cores.get()).ok(),
                base,
            );
            assert_eq!(
                scaled, expected,
                "the live fence must be the pure composition of the same seam reads"
            );
        }
    }
}

/// T214 req 3 — the adoption grep pin: no bare wall-clock verdict-fence
/// construction (`Instant::now() + Duration::from_secs(30|90)`) remains in
/// any of the three spawn-heavy loopd families, and every family still
/// routes its verdict fence through a SHARED fence helper — [`load_scaled_deadline`]
/// or, since T225, the progress-reset [`ProgressDeadline`] (the T225
/// extension: a surface-watching fence routes through the progress shape, a
/// no-surface fence keeps the load-scaled absolute; a BARE construction
/// counts for neither and the needles above ban it). Whole-line
/// comments are skipped (a comment naming the base is documentation, not a
/// fence — the pin binds the CODE); the needles are assembled at runtime so
/// this pin's own source never carries the pattern it greps for. The
/// family sources resolve at RUNTIME (cargo runs every test binary with the
/// package root as cwd) — never a compile-time `env!` of CARGO_MANIFEST_DIR,
/// which the T48 pin bans from every .rs file (the T47 shared target dir
/// hands cached binaries across checkouts; a baked path would read the
/// build-time worktree, possibly a deleted one).
#[test]
fn t214_no_bare_verdict_deadline_construction_in_any_loopd_family() {
    use std::fs;

    let root = std::env::current_dir().expect("cargo sets the test cwd to the package root");
    let families = [
        "tests/loopd_orphan_reaper.rs",
        "tests/loopd_spoof_guard.rs",
        "tests/loopd_daemon_ensure.rs",
    ];
    for family in families {
        let src =
            fs::read_to_string(root.join(family)).unwrap_or_else(|e| panic!("reading {family}: {e}"));
        // Non-comment lines only (a `//`-led line is documentation; the
        // family files use no block comments).
        let code = src
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        for secs in ["30", "90"] {
            let bare = format!("Instant::now() + Duration::from_secs({secs})");
            assert!(
                !code.contains(&bare),
                "{family} still constructs a bare {secs}s verdict fence — the \
                 fence's basis must stay load-scaled via load_scaled_deadline \
                 (T214 req 3; the zero-timeout-bump doctrine keeps the BASE, \
                 not the bare wall-clock construction)"
            );
        }
        let calls = code.matches("load_scaled_deadline(").count()
            + code.matches("ProgressDeadline::new(").count();
        assert!(
            calls >= 1,
            "{family} no longer routes its verdict fence through a shared \
             fence helper (load_scaled_deadline, or T225's progress-reset \
             ProgressDeadline) — the T214/T225 adoption regressed (the family \
             carried exactly one verdict-deadline construction at T214 \
             landing)"
        );
    }
}

// ---------- T225: progress-reset liveness fences ----------
//
// A fence whose child is making progress is a LIVENESS fence; its trip
// condition should be "no progress for N seconds", never "wall clock
// exceeded while progress continued". T214 re-based the loopd fences on
// measured load, but its per-core loadavg is BLIND to suite fan-out on
// many-core hosts (the cycle-101 flake: K7 has 18 cores, ambient load 9.68
// reads 0.54/core → factor 1.0 → NO scaling → the fence blew on main
// mid-cycle while the child was advancing), and cycle-100 burned a full
// 80-iteration child on ten delegate launch bin-test false-reds. T225
// re-bases the fence on the thing the poll ALREADY observes: the watched
// surface advancing (log growth, events-file mtime, a dump file's byte
// length). [`ProgressDeadline`] trips only after `base` of NO observed
// advance; an absolute outer BACKSTOP — `load_scaled_deadline(base)` scaled
// by the named [`ProgressDeadline::BACKSTOP_FACTOR`] (~4x) — still fails a
// genuinely hung child. Fail-safe on ANY read error: an unreadable surface
// reads as PROGRESS, never a false trip (the cost asymmetry is the doctrine:
// a false trip burns an 80-iteration child; a slower hang diagnosis costs
// seconds). The reset/trip computation is PURE (the `scale_factor` pattern)
// and the zero-timeout-bump doctrine holds: every adoption site keeps its
// base constant, only the BASIS moves.
//
// COMPLIANCE NOTES (T225 reqs 2-3) — converted vs stayed, one line each:
// CONVERTED to ProgressDeadline (surface named):
// - tests/loopd_orphan_reaper.rs `wait_for_any` 30s verdict fence — surface:
//   .chug/loopd/loopd.log growth (the poll already reads it every 100ms).
// - tests/loopd_spoof_guard.rs `wait_for_verdict` 30s verdict fence —
//   surface: the same supervisor log (self.log()).
// - tests/loopd_daemon_ensure.rs `run_until` 90s verdict fence — surface:
//   the supervisor log.
// - src/delegate/tests/launch.rs launch-then-status 5s poll — surface: the
//   stub child's .chug/events.jsonl.
// - launch.rs T144 target-dir-scrub leg 5s poll — surface: env.txt.
// - launch.rs T183 explicit-env leg 5s poll — surface: env.txt.
// - launch.rs T183 absent-env leg 5s poll — surface: env.txt.
// RE-BASED, STAYED ABSOLUTE (no observable progress surface):
// - status.rs `delegate_status_reaps_own_exited_child_and_reports_false_twice`
//   10s poll — the watched fact is the child's one-time exit→zombie→reap
//   transition, a scheduler race with no advancing file/counter/byte-length
//   to fingerprint (the seam's answer flips once, at the end) — keeps
//   load_scaled_deadline per the req-2 no-surface rule.
// STAYED BARE ABSOLUTE (not liveness fences — semantic pins or quiescence):
// - collect.rs `delegate_collect_mid_run_reports_running` `elapsed < 30s` —
//   a post-hoc bound on ONE non-blocking call (no poll loop, no real child —
//   fixture events only); the 30s cap pins the non-blocking contract, and no
//   surface exists to observe.
// - wait.rs / wait_terminal.rs `elapsed >= 1s|1.5s|2.9s` lower bounds and
//   `< 15s|30s` upper bounds — the wait_secs round-trip pins (early return,
//   deadline honored, churn wakes): call SEMANTICS, not child liveness;
//   converting them would gut the pin. Also not in the named adoption set
//   (they poll fixture events files, never real children).
// - orphan_reaper `wait_for_any`'s 8s settle cap — a QUIESCENCE bound (it
//   waits for the log to STOP growing); resetting on progress would defeat
//   its purpose, so it stays a bare absolute 8s.

/// Why a progress-reset fence tripped (T225 req 1). The variant carries the
/// measured windows so the panicking fence's message IS the forensic record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressTrip {
    /// The silence base elapsed with NO observed advance on the watched
    /// surface — the liveness trip.
    Stalled {
        /// How long the surface showed the same fingerprint.
        silent_for: Duration,
        /// The fence's silence base.
        base: Duration,
    },
    /// The absolute outer backstop elapsed even though the surface kept
    /// advancing — a genuinely hung child still fails, bounded.
    Backstop {
        /// Total wall clock since the fence was armed.
        elapsed: Duration,
        /// The backstop that expired.
        backstop: Duration,
    },
}

impl std::fmt::Display for ProgressTrip {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            ProgressTrip::Stalled { silent_for, base } => write!(
                f,
                "no surface advance for {silent_for:?} (silence base {base:?})"
            ),
            ProgressTrip::Backstop { elapsed, backstop } => write!(
                f,
                "surface still advancing but the absolute backstop {backstop:?} \
                 elapsed (total {elapsed:?})"
            ),
        }
    }
}

/// The progress-reset liveness fence (T225 req 1). [`ProgressDeadline::new`]
/// arms two fences: the SILENCE fence — `base` of no observed advance on the
/// watched surface trips it — and the absolute outer BACKSTOP,
/// `load_scaled_deadline(base) * BACKSTOP_FACTOR`, which trips no matter how
/// much progress was observed. The poll loop calls [`ProgressDeadline::observe`]
/// each iteration with the surface's fingerprint (byte length, mtime mix,
/// iteration count — any u64 the caller chooses) and
/// [`ProgressDeadline::tripped`] before panicking. The reset/trip computation
/// is pure ([`ProgressDeadline::trip_decision`], the `scale_factor` pattern)
/// and every seam failure fails SAFE: an unreadable surface reads as
/// PROGRESS, never a false trip.
pub struct ProgressDeadline {
    /// Wall clock at arming — the backstop's zero.
    started: Instant,
    /// Wall clock at the last observed advance (or unreadable read).
    last_progress: Instant,
    /// The last readable fingerprint observed (`None` = none yet).
    last_seen: Option<u64>,
    /// The silence base (the adoption site's unchanged constant).
    base: Duration,
    /// The absolute outer backstop.
    backstop: Duration,
}

impl ProgressDeadline {
    /// The backstop multiplier (T225 req 1: a NAMED constant, ~4x): the
    /// absolute outer fence is `load_scaled_deadline(base)` × this — ≥ 4x
    /// the silence base on a quiet host, up to 16x under measured load.
    pub const BACKSTOP_FACTOR: u32 = 4;

    /// Arms the fence: silence base `base` (the adoption site's unchanged
    /// constant), backstop `load_scaled_deadline(base) * BACKSTOP_FACTOR`
    /// (the T214 load scaling survives on the OUTER fence only — it is what
    /// bounds the genuinely hung child).
    pub fn new(base: Duration) -> Self {
        Self::armed(
            Instant::now(),
            base,
            load_scaled_deadline(base) * Self::BACKSTOP_FACTOR,
        )
    }

    /// The construction seam the pure legs drive with a synthetic clock
    /// (the `scaled_deadline` pattern: the computation separates from the
    /// host).
    fn armed(started: Instant, base: Duration, backstop: Duration) -> Self {
        Self {
            started,
            last_progress: started,
            last_seen: None,
            base,
            backstop,
        }
    }

    /// Record one observation of the watched surface. A fingerprint CHANGE
    /// (or the first readable observation) resets the silence clock; the
    /// SAME fingerprint repeated is no progress. `None` is an UNREADABLE
    /// surface — fail-SAFE (T225 req 1): the silence clock resets
    /// unconditionally, so a missing or unreadable file can never
    /// manufacture a false trip (the backstop still bounds the total).
    pub fn observe(&mut self, seen: Option<u64>) {
        self.observe_at(seen, Instant::now());
    }

    /// [`ProgressDeadline::observe`] on a synthetic clock — the seam the
    /// pure legs drive.
    fn observe_at(&mut self, seen: Option<u64>, now: Instant) {
        match seen {
            None => {
                self.last_progress = now;
                self.last_seen = None;
            }
            Some(fp) => {
                if self.last_seen != Some(fp) {
                    self.last_progress = now;
                    self.last_seen = Some(fp);
                }
            }
        }
    }

    /// The PURE trip decision (the `scale_factor` pattern): the two elapsed
    /// windows against the two fences. `None` = live. Stall precedence: when
    /// BOTH windows have expired the silence reason is reported (the
    /// immediate cause of death).
    fn trip_decision(
        total: Duration,
        silence: Duration,
        base: Duration,
        backstop: Duration,
    ) -> Option<ProgressTrip> {
        if silence >= base {
            Some(ProgressTrip::Stalled {
                silent_for: silence,
                base,
            })
        } else if total >= backstop {
            Some(ProgressTrip::Backstop { elapsed: total, backstop })
        } else {
            None
        }
    }

    /// The fence's verdict at a synthetic `now` — the seam the pure legs
    /// drive. `saturating` so a synthetic clock can never panic the legs.
    fn tripped_at(&self, now: Instant) -> Option<ProgressTrip> {
        Self::trip_decision(
            now.saturating_duration_since(self.started),
            now.saturating_duration_since(self.last_progress),
            self.base,
            self.backstop,
        )
    }

    /// The fence's verdict NOW — `None` = live (keep polling); `Some(trip)`
    /// = blow the fence with `{trip}` in the panic message.
    pub fn tripped(&self) -> Option<ProgressTrip> {
        self.tripped_at(Instant::now())
    }

    /// The absolute outer backstop, for the panicking fence's message.
    pub fn backstop(&self) -> Duration {
        self.backstop
    }
}

/// The fail-safe surface fingerprint for a file (T225 req 1): the file's
/// (length, mtime) folded to one u64 — either moving is an advance. ANY read
/// failure (missing, permission, pre-epoch mtime, ...) is `None`, which
/// [`ProgressDeadline::observe`] treats as PROGRESS — never a false trip.
pub fn surface_fingerprint(path: &Path) -> Option<u64> {
    let md = std::fs::metadata(path).ok()?;
    let mtime = md
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_nanos() as u64;
    Some(md.len().rotate_left(32) ^ mtime)
}

// T225 pins — the pure reset/trip legs and the fixture legs run in the bin's
// unit tests AND in every family binary that includes this module (the
// t151/t214 pin shape): the fence is bound by tests, not by host state.

/// The pure trip legs (T225 req 1 + Tests): advance → not expired; a base of
/// silence → expired (Stalled); the backstop reached DESPITE continuous
/// progress → expired (Backstop); stall precedence when both windows expire.
#[test]
fn t225_trip_decision_pure_legs() {
    let base = Duration::from_secs(30);
    let backstop = Duration::from_secs(120);
    // Fresh fence, nothing observed: live.
    assert_eq!(
        ProgressDeadline::trip_decision(
            Duration::from_secs(1),
            Duration::from_secs(1),
            base,
            backstop
        ),
        None
    );
    // An advance at 5s: at 20s the fence is live (15s of silence < base).
    assert_eq!(
        ProgressDeadline::trip_decision(
            Duration::from_secs(20),
            Duration::from_secs(15),
            base,
            backstop
        ),
        None
    );
    // One tick short of the base of silence: still live.
    assert_eq!(
        ProgressDeadline::trip_decision(
            Duration::from_secs(34),
            Duration::from_secs(29),
            base,
            backstop
        ),
        None
    );
    // A base of silence trips — Stalled, carrying the measured windows.
    assert_eq!(
        ProgressDeadline::trip_decision(
            Duration::from_secs(35),
            Duration::from_secs(30),
            base,
            backstop
        ),
        Some(ProgressTrip::Stalled {
            silent_for: Duration::from_secs(30),
            base
        })
    );
    // Continuous progress (silence never exceeds 2s) through 3x base: LIVE —
    // exactly the shape the old absolute fence blew (the cycle-101 flake).
    assert_eq!(
        ProgressDeadline::trip_decision(
            Duration::from_secs(90),
            Duration::from_secs(2),
            base,
            backstop
        ),
        None
    );
    // One tick before the backstop, still progressing: live.
    assert_eq!(
        ProgressDeadline::trip_decision(
            Duration::from_secs(119),
            Duration::from_secs(2),
            base,
            backstop
        ),
        None
    );
    // The backstop reached DESPITE the progress: expired — Backstop.
    assert_eq!(
        ProgressDeadline::trip_decision(
            Duration::from_secs(120),
            Duration::from_secs(2),
            base,
            backstop
        ),
        Some(ProgressTrip::Backstop {
            elapsed: Duration::from_secs(120),
            backstop
        })
    );
    // Both windows expired: the SILENCE reason wins (the immediate cause).
    assert_eq!(
        ProgressDeadline::trip_decision(
            Duration::from_secs(200),
            Duration::from_secs(40),
            base,
            backstop
        ),
        Some(ProgressTrip::Stalled {
            silent_for: Duration::from_secs(40),
            base
        })
    );
}

/// The observe legs (T225 Tests): a changed fingerprint resets the silence
/// clock, a repeated fingerprint does not, and an UNREADABLE surface
/// (None) reads as PROGRESS — every unreadable read resets, so a surface
/// that stays unreadable never trips the silence fence (the backstop still
/// bounds the total — the fail-safe seam leg).
#[test]
fn t225_observe_resets_on_advance_and_treats_unreadable_as_progress() {
    let t0 = Instant::now();
    let base = Duration::from_secs(30);
    let backstop = Duration::from_secs(120);

    // The first readable observation IS an advance (None → Some(fp)).
    let mut pd = ProgressDeadline::armed(t0, base, backstop);
    pd.observe_at(Some(100), t0 + Duration::from_secs(1));
    assert_eq!(pd.tripped_at(t0 + Duration::from_secs(29)), None);
    // The SAME fingerprint again is NOT an advance: the silence clock keeps
    // running and the base of silence trips.
    pd.observe_at(Some(100), t0 + Duration::from_secs(29));
    assert!(
        matches!(
            pd.tripped_at(t0 + Duration::from_secs(31)),
            Some(ProgressTrip::Stalled { .. })
        ),
        "a repeated fingerprint must not reset the silence clock"
    );

    // A CHANGED fingerprint resets the silence clock...
    let mut pd = ProgressDeadline::armed(t0, base, backstop);
    pd.observe_at(Some(100), t0 + Duration::from_secs(1));
    pd.observe_at(Some(200), t0 + Duration::from_secs(20));
    assert_eq!(
        pd.tripped_at(t0 + Duration::from_secs(49)),
        None,
        "20s of silence after the 20s advance is still short of the base"
    );
    assert!(matches!(
        pd.tripped_at(t0 + Duration::from_secs(51)),
        Some(ProgressTrip::Stalled { .. })
    ));

    // Fail-safe: an UNREADABLE read resets, and EVERY unreadable read
    // resets — a surface that stays unreadable never trips the SILENCE
    // fence, all the way through and past the silence base; the absolute
    // backstop still bounds the total (fail-SAFE, not fail-open forever).
    let mut pd = ProgressDeadline::armed(t0, base, backstop);
    pd.observe_at(Some(100), t0 + Duration::from_secs(1));
    for s in 29..120 {
        pd.observe_at(None, t0 + Duration::from_secs(s));
        assert_eq!(
            pd.tripped_at(t0 + Duration::from_secs(s)),
            None,
            "an unreadable read at {s}s must reset the silence clock — never \
             a false trip"
        );
    }
    assert!(matches!(
        pd.tripped_at(t0 + Duration::from_secs(121)),
        Some(ProgressTrip::Backstop { .. })
    ));

    // A missing-then-appearing surface: the reappearance is an advance.
    let mut pd = ProgressDeadline::armed(t0, base, backstop);
    pd.observe_at(None, t0 + Duration::from_secs(10));
    pd.observe_at(Some(7), t0 + Duration::from_secs(11));
    assert_eq!(pd.tripped_at(t0 + Duration::from_secs(40)), None);
}

/// The fingerprint seam legs: a missing file is `None` (unreadable →
/// progress), and an append moves the fingerprint.
#[test]
fn t225_surface_fingerprint_moves_on_advance_and_reads_none_when_unreadable() {
    let dir = tempfile::tempdir().expect("tempdir");
    let surf = dir.path().join("s.log");
    assert_eq!(
        surface_fingerprint(&surf),
        None,
        "a missing surface is an unreadable surface: None"
    );
    std::fs::write(&surf, b"one").expect("seed the surface");
    let a = surface_fingerprint(&surf).expect("readable once written");
    std::fs::write(&surf, b"one two").expect("advance the surface");
    let b = surface_fingerprint(&surf).expect("still readable");
    assert_ne!(a, b, "an advance must move the fingerprint");
}

/// Req 4(a) — the stalled fixture: a child double whose surface EXISTS and
/// never advances trips the fence in ~base, via the SILENCE reason, well
/// before the backstop. (A surface that never APPEARS at all is the other
/// fail-safe leg above: it reads as progress and the backstop catches it.)
#[test]
fn t225_stalled_surface_double_trips_in_about_base() {
    let _timing = timing_guard();
    let dir = tempfile::tempdir().expect("tempdir");
    let surf = dir.path().join("surface.log");
    std::fs::write(&surf, b"the double wrote once and hung\n").expect("seed the stalled surface");

    let base = Duration::from_millis(500);
    let mut pd = ProgressDeadline::new(base);
    let started = Instant::now();
    loop {
        pd.observe(surface_fingerprint(&surf));
        if let Some(trip) = pd.tripped() {
            let elapsed = started.elapsed();
            assert!(
                matches!(trip, ProgressTrip::Stalled { .. }),
                "a never-advancing surface must trip the SILENCE fence, not \
                 the backstop: {trip}"
            );
            assert!(elapsed >= base, "tripped before its base: {elapsed:?}");
            assert!(
                elapsed < base * 3,
                "the stall trip must land at ~base, nowhere near the \
                 backstop: {elapsed:?}"
            );
            return;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

/// Req 4(b) — the slow-progress fixture (THE flake reproduction): a double
/// that advances every base/6 for 3x base PASSES the progress-reset fence.
/// Under the pre-T225 absolute fence shape (deadline = start + base) this
/// exact loop FAILS at `base` while the double is still advancing — the
/// demonstrated RED (old-shape scratch run recorded in the t225 notes); the
/// non-vacuousness asserts pin that this leg really outlives the old fence,
/// so the pass is discrimination, not slack. A Backstop trip under extreme
/// host load is the OUTER fence working, not a silence trip: the leg ends
/// early, still having outlived the old fence by 2x (the backstop is ≥ 4x
/// base by construction, so an early break is impossible).
#[test]
fn t225_slow_progress_surface_double_outlives_the_absolute_fence() {
    let _timing = timing_guard();
    let dir = tempfile::tempdir().expect("tempdir");
    let surf = dir.path().join("surface.log");
    std::fs::write(&surf, b"").expect("seed the surface");

    let base = Duration::from_millis(600);
    let advance_every = Duration::from_millis(100); // base/6 — well inside the silence base
    let mut pd = ProgressDeadline::new(base);
    let started = Instant::now();
    let mut advances = 0u32;
    while started.elapsed() < base * 3 {
        // The double advances: one appended line (len + mtime both move).
        use std::io::Write as _;
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(&surf)
            .expect("append to the double's surface");
        writeln!(f, "advance {advances}").expect("write the advance");
        advances += 1;

        pd.observe(surface_fingerprint(&surf));
        match pd.tripped() {
            None => {}
            // The outer fence, not the silence fence — the leg's claim
            // already held (it broke at ≥ 4x base, 2x past the old fence).
            Some(ProgressTrip::Backstop { .. }) => break,
            Some(trip @ ProgressTrip::Stalled { .. }) => panic!(
                "the progress-reset fence must stay live while the double \
                 advances (advance #{advances}) — this is the cycle-101 \
                 flake shape the old absolute fence blew: {trip}"
            ),
        }
        std::thread::sleep(advance_every);
    }
    let total = started.elapsed();
    // Non-vacuousness: the loop REALLY outlived the pre-T225 absolute fence
    // (`base`) by 2x — under the old shape this leg dies at `base` mid-run.
    assert!(
        total >= base * 2,
        "the double must outlive the old absolute fence for this leg to \
         discriminate: total {total:?} vs base {base:?}"
    );
    assert!(
        advances >= 3,
        "the surface must have actually advanced: {advances} advances"
    );
}

/// T225 req 5 — the adoption grep pin EXTENDED to every converted surface:
/// zero bare wall-clock verdict-fence constructions (`Instant::now() +
/// Duration::from_secs(N)`) remain on any of them, and every surface that
/// CARRIES a fence routes it through a shared helper — ProgressDeadline for
/// the surface-watching fences, load_scaled_deadline for the one no-surface
/// fence (status.rs's reap poll). The loopd families overlap the T214 pin
/// above deliberately (belt-and-suspenders on the wider needle set). Same
/// doctrine as the T214 pin: non-comment code lines only, needles assembled
/// at runtime, sources resolved at RUNTIME from the package root (never a
/// baked CARGO_MANIFEST_DIR — the T48 rule).
#[test]
fn t225_no_bare_verdict_fence_construction_on_any_converted_surface() {
    use std::fs;

    const PROGRESS: &str = "ProgressDeadline::new(";
    const SCALED: &str = "load_scaled_deadline(";
    // (surface, needle secs, expected fence marker — None = the surface
    // carries no fence at all, only the bare-needle absence is pinned).
    let surfaces: &[(&str, &[&str], Option<&str>)] = &[
        ("tests/loopd_orphan_reaper.rs", &["30", "90"], Some(PROGRESS)),
        ("tests/loopd_spoof_guard.rs", &["30", "90"], Some(PROGRESS)),
        ("tests/loopd_daemon_ensure.rs", &["30", "90"], Some(PROGRESS)),
        (
            "src/delegate/tests/launch.rs",
            &["5", "10", "30", "90"],
            Some(PROGRESS),
        ),
        (
            "src/delegate/tests/status.rs",
            &["5", "10", "30", "90"],
            Some(SCALED),
        ),
        (
            "src/delegate/tests/collect.rs",
            &["5", "10", "30", "90"],
            None,
        ),
    ];
    let root = std::env::current_dir().expect("cargo sets the test cwd to the package root");
    for (surface, secs_list, marker) in surfaces {
        let src = fs::read_to_string(root.join(surface))
            .unwrap_or_else(|e| panic!("reading {surface}: {e}"));
        let code = src
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        for secs in *secs_list {
            let bare = format!("Instant::now() + Duration::from_secs({secs})");
            assert!(
                !code.contains(&bare),
                "{surface} still constructs a bare {secs}s verdict fence — \
                 the fence's basis must move to the surface's observed \
                 advance (T225: ProgressDeadline) or, with no observable \
                 surface, stay load-scaled (T214 req 3)"
            );
        }
        if let Some(marker) = marker {
            let calls = code.matches(marker).count();
            assert!(
                calls >= 1,
                "{surface} no longer routes its verdict fence through \
                 {marker} — the T225 adoption regressed"
            );
        }
    }
}

// ---------- T229: closing the T225 validator's pin-strength findings ----------
//
// The T225 kimi verdict (PASS, .chug/verdict-t225-validate-20261004.md)
// left two predicted survivors behind (the T224 pattern — file them
// forward):
//
// - M7-fingerprint-len-only (mtime arm dropped): every T225 leg advances
//   the watched surface by APPENDING, so a fingerprint keyed on LENGTH
//   ONLY passed all six legs. Benign today (every adopted surface is
//   length-monotonic: append-only supervisor logs, appended events.jsonl,
//   one-shot env dumps) but one refactor from silently never resetting a
//   fence watching a fixed-width or rewrite-style surface.
// - BACKSTOP_FACTOR upper bound: the constant was pinned against REDUCTION
//   (M8-backstop-factor-1 died RED on the slow-progress leg's
//   non-vacuousness pin) but an INCREASE (4→N) survives — a bigger
//   backstop only trips LATER, and every wall-clock leg ends before the
//   backstop. The pure legs pinned the backstop STRUCTURE, not the
//   constant's value.
//
// One pin leg per finding, each RED-proven against its named mutant in the
// worktree and reverted byte-clean (mutant → red test recorded in the T229
// commit message, the T69 doctrine).

/// T229 req 1 — the MTIME-arm pin (the M7 survivor): REWRITING the surface
/// with DIFFERENT bytes of the SAME length while mtime advances must still
/// MOVE the fingerprint. Both halves of the fold are pinned:
/// (a) same length, mtime ADVANCED → the fingerprint MOVES — the len-only
///     mutant's killer (no append leg exercises it: length moves with
///     mtime there, so a len-only fingerprint passes every append);
/// (b) same length, mtime RESTORED → the fingerprint is UNCHANGED — the
///     fold keys (len, mtime), never the content bytes (the doc contract:
///     "either moving is an advance"); a content arm would break this half
///     and must revisit the doc + the fail-safe semantics with it.
/// Both mtimes are pinned explicitly (std `File::set_modified` — a
/// synthetic clock for the mtime arm, the same reason the trip legs drive
/// `observe_at`): two real writes microseconds apart can land in ONE
/// timestamp tick on a coarse filesystem, which would flake (a).
#[test]
fn t229_surface_fingerprint_moves_on_same_length_rewrite() {
    let set_mtime = |path: &Path, t: std::time::SystemTime| {
        std::fs::OpenOptions::new()
            .write(true)
            .open(path)
            .and_then(|f| f.set_modified(t))
            .unwrap_or_else(|e| panic!("setting the surface's mtime: {e}"));
    };
    let dir = tempfile::tempdir().expect("tempdir");
    let surf = dir.path().join("s.log");
    let t1 = std::time::SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
    let t2 = t1 + Duration::from_secs(1);

    std::fs::write(&surf, b"aaaa").expect("seed the surface");
    set_mtime(&surf, t1);
    let a = surface_fingerprint(&surf).expect("readable at t1");

    // (a) Same length (4 bytes), different bytes, mtime advanced: the
    // fingerprint MUST move.
    std::fs::write(&surf, b"bbbb").expect("rewrite, same length");
    set_mtime(&surf, t2);
    let b = surface_fingerprint(&surf).expect("readable at t2");
    assert_ne!(
        a, b,
        "a same-length rewrite with an advanced mtime must move the \
         fingerprint — a length-only fingerprint (the M7 mutant) would \
         silently never reset a fence watching a fixed-width surface"
    );

    // (b) Same length, mtime restored to t2: NOT an advance.
    std::fs::write(&surf, b"cccc").expect("rewrite again, same length");
    set_mtime(&surf, t2);
    let c = surface_fingerprint(&surf).expect("readable at t2 again");
    assert_eq!(
        b, c,
        "same length and same mtime must NOT move the fingerprint — the \
         fold keys (len, mtime), not content"
    );
}

/// T229 req 2 — the BACKSTOP_FACTOR VALUE pin: the constant is 4, pinned
/// through the PURE seam with arithmetic that fails if the factor moves in
/// EITHER direction. The reduction died RED on the slow-progress leg's
/// non-vacuousness pin; the INCREASE survived every wall-clock leg because
/// a bigger backstop only trips later. Every load here is a FIXED SYNTHETIC
/// value driving the T214 pure seam ([`scaled_deadline`]) — never the
/// host's:
/// (a) the value: quiet host (factor exactly 1.0 → scaled == base) times
///     the named constant == 4× base — the named `BACKSTOP_FACTOR == 4`
///     assertion routed through the fence arithmetic, not a source grep;
/// (b) the window it feeds: continuous progress keeps the fence live for
///     the whole backstop and trips Backstop — never Stalled — exactly AT
///     it (armed → observe_at → tripped_at, the full pure chain). A
///     REDUCED factor trips early and dies mid-loop; an INCREASED factor
///     never trips and dies on the final assert;
/// (c) the loaded shape: clamp-max load (factor exactly 4.0 → scaled ==
///     4× base) times the constant == 16× base — the documented "up to 16x
///     under measured load" upper shape;
/// plus the LIVE constructor (real host load): the constant still feeds it
/// and the load scaling only ever stretches — [4× base, 16× base] on any
/// host (T214's live leg pins load_scaled_deadline to [base, 4× base] on
/// every run, so this window cannot flake in either direction).
#[test]
fn t229_backstop_factor_value_is_pinned_at_four() {
    let base = Duration::from_secs(30);

    // (a) The named constant's value, through the pure seam.
    let quiet = scaled_deadline(Some(0.0), Some(1), base);
    assert_eq!(
        quiet, base,
        "the synthetic quiet load must be factor 1.0: scaled == base"
    );
    assert_eq!(
        ProgressDeadline::BACKSTOP_FACTOR,
        4u32,
        "BACKSTOP_FACTOR is the named 4x constant"
    );
    assert_eq!(
        quiet * ProgressDeadline::BACKSTOP_FACTOR,
        base * 4,
        "the backstop must be exactly 4x base on a quiet host: a SMALLER \
         factor weakens the genuinely-hung bound (M8-backstop-factor-1 died \
         RED on the slow-progress leg), a LARGER factor slows every hang \
         diagnosis by the same multiple (M8's 4→8 increase survived every \
         wall-clock leg) — BOTH directions are this pin"
    );

    // (b) The window the factor feeds.
    let backstop = quiet * ProgressDeadline::BACKSTOP_FACTOR;
    let t0 = Instant::now();
    let mut pd = ProgressDeadline::armed(t0, base, backstop);
    for s in 1u64..120 {
        pd.observe_at(Some(s), t0 + Duration::from_secs(s));
        assert_eq!(
            pd.tripped_at(t0 + Duration::from_secs(s)),
            None,
            "observed progress at {s}s must keep the fence live (backstop \
             {backstop:?}) — a REDUCED factor trips early and dies here"
        );
    }
    pd.observe_at(Some(120), t0 + Duration::from_secs(120));
    assert!(
        matches!(
            pd.tripped_at(t0 + Duration::from_secs(120)),
            Some(ProgressTrip::Backstop { .. })
        ),
        "at the backstop the fence must trip DESPITE continuous progress — \
         an INCREASED factor never trips by now (the 4→8 mutant dies here), \
         and the reason must be Backstop, never Stalled"
    );

    // (c) The loaded shape: the clamp-max synthetic load scales the base
    // 4x, and the constant multiplies THAT.
    let loaded = scaled_deadline(Some(32.0), Some(8), base);
    assert_eq!(
        loaded, base * 4,
        "the synthetic 4x-capacity load must scale to the upper clamp"
    );
    assert_eq!(
        loaded * ProgressDeadline::BACKSTOP_FACTOR,
        base * 16,
        "under the clamp-max load the backstop is 16x base — the documented \
         'up to 16x under measured load' shape"
    );

    // The LIVE constructor (real host load, not synthetic): the constant
    // still feeds it, and the load scaling only ever stretches.
    let live = ProgressDeadline::new(base);
    assert!(
        live.backstop() >= base * ProgressDeadline::BACKSTOP_FACTOR,
        "the live backstop {:?} must be at least the constant × base (load \
         scaling only ever stretches): {:?}",
        live.backstop(),
        base * ProgressDeadline::BACKSTOP_FACTOR
    );
    assert!(
        live.backstop() <= base * 4 * ProgressDeadline::BACKSTOP_FACTOR,
        "the live backstop {:?} must be at most 4× the constant × base (the \
         load clamp's upper bound): {:?}",
        live.backstop(),
        base * 4 * ProgressDeadline::BACKSTOP_FACTOR
    );
}
