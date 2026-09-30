//! T172 req-4 — the load-lock harness's OWN behavioral proof, in one
//! dedicated binary (so the pins run once per suite, not once per
//! including family binary). See tests/support/load_lock.rs for the
//! mechanism and its fail-open doctrine.
#![cfg(unix)]

#[path = "support/load_lock.rs"]
mod t172_load_lock;

use std::time::{Duration, Instant};

use t172_load_lock::{family_guard_with_wait, guard_at};

// ---------------------------------------------------------------------------
// T172 req-4 semantics pins — THE harness's own behavioral proof. flock
// contends across independent opens even within one process, so every
// assertion below is the exact syscall pair a sibling PROCESS would issue:
// mutual exclusion is a lock fact, not a scheduler race — no wall-clock
// window to stretch (the T151 non-vacuousness shape, at the file level).
// ---------------------------------------------------------------------------

/// The domain's family name for the semantics pins below (any stable name —
/// these tests never take the production families' domains).
const SEMANTICS_FAMILY: &str = "t172-load-lock-semantics";

#[test]
fn t172_load_lock_is_exclusive_and_reacquirable_after_release() {
    // Holder A: a plain acquisition at the resolved production path.
    let a = family_guard_with_wait(SEMANTICS_FAMILY, Duration::from_millis(10_000));
    assert!(
        !a.is_degraded(),
        "the semantics domain must be acquirable on a healthy host — a \
         degraded first acquisition means the lock directory itself is \
         broken and the harness would be vacuous"
    );
    // The rival B: the SAME lockfile, an INDEPENDENT open — byte-for-byte
    // the syscall pair a sibling nextest process would issue. While A holds
    // the domain, B's bounded attempt must be REFUSED (mutual exclusion),
    // and it must come back DEGRADED, not blocked: bounded, then proceed.
    let started = Instant::now();
    let b = family_guard_with_wait(SEMANTICS_FAMILY, Duration::from_millis(250));
    let b_elapsed = started.elapsed();
    assert!(
        b.is_degraded(),
        "a second acquirer entered while the first held the domain — the \
         cross-binary load lock is NOT exclusive (flock across independent \
         opens must refuse the rival), so nextest's parallel sandbox tests \
         would still overlap"
    );
    assert!(
        b_elapsed >= Duration::from_millis(250),
        "the rival returned before its bounded wait expired — it did not \
         actually wait for the holder (a no-op mutant)"
    );
    // Release; only now may a third acquisition enter and HOLD.
    drop(a);
    let c = family_guard_with_wait(SEMANTICS_FAMILY, Duration::from_millis(10_000));
    assert!(
        !c.is_degraded(),
        "after the holder released, the domain must be acquirable again — \
         the lock leaked (never released on drop)"
    );
}

#[test]
fn t172_load_lock_degrades_fail_open_on_an_unopenable_lockfile() {
    // A lockfile path that CANNOT be opened: its parent is a regular FILE,
    // so open(2) fails with ENOTDIR immediately. The guard must come back
    // degraded (not panic, not hang) and the caller proceeds — the T172
    // req-4 leg: an absent/uncreatable lockfile must never take the suite
    // down or stall it.
    let scratch = tempfile::tempdir().expect("scratch tempdir");
    let blocker = scratch.path().join("not-a-dir");
    std::fs::write(&blocker, b"").expect("write blocker file");
    let started = Instant::now();
    let guard = guard_at(&blocker.join("family.lock"), SEMANTICS_FAMILY, Duration::from_millis(10_000));
    assert!(
        guard.is_degraded(),
        "an unopenable lockfile must degrade fail-open, not silently appear \
         to hold the domain"
    );
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "the degrade must be immediate (the open failed) — no bounded wait \
         is needed when the lockfile cannot even be opened"
    );
}

#[test]
fn t172_load_lock_bounded_wait_expires_into_degrade_not_hang() {
    // The T172 req-4 core: a STUCK holder (a wedged sibling process) must
    // cost a bounded wait, then a noted degrade — never a hang. The rival's
    // wait here is deliberately tiny; the assertion is that it RETURNS
    // degraded at all. A mutant that turns the bound into an infinite wait
    // (fail-CLOSED) hangs this test and is red-by-timeout in the gates.
    let a = family_guard_with_wait(SEMANTICS_FAMILY, Duration::from_millis(10_000));
    assert!(!a.is_degraded());
    let started = Instant::now();
    let b = family_guard_with_wait(SEMANTICS_FAMILY, Duration::from_millis(200));
    let elapsed = started.elapsed();
    assert!(
        b.is_degraded(),
        "a rival held past its bounded wait must degrade, not acquire (it \
         can never have observed the holder release)"
    );
    assert!(
        elapsed >= Duration::from_millis(200) && elapsed < Duration::from_secs(5),
        "the bounded wait must actually bound: expired after {elapsed:?} — \
         a fail-CLOSED mutant (unbounded wait) hangs here and dies on the \
         runner's per-test timeout instead"
    );
}
