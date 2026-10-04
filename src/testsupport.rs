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
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

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
/// routes its verdict fence through [`load_scaled_deadline`]. Whole-line
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
        let calls = code.matches("load_scaled_deadline(").count();
        assert!(
            calls >= 1,
            "{family} no longer routes its verdict fence through \
             load_scaled_deadline — the T214 adoption regressed (the family \
             carried exactly one verdict-deadline construction at T214 \
             landing)"
        );
    }
}
