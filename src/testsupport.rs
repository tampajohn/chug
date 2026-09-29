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
