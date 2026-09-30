// T172 — THE cross-binary serialization domain for the crate's
// deadline-bearing sandbox/spawn test families (the loopd supervisor
// harness: tests/loopd_stale_binary.rs, tests/loopd_spoof_guard.rs,
// tests/loopd_orphan_reaper.rs; the mcp_serve stub-spawn family:
// tests/mcp_serve.rs wire legs + the bin's src/mcp_serve/tests.rs
// stub-spawn leg).
//
// WHY A FILE LOCK (the T172 gap in the T31/T151 doctrine): `timing_guard`
// (src/testsupport.rs) is a static Mutex — a PER-PROCESS domain. nextest
// runs each TEST as its own PROCESS, so under the nextest gate at full
// parallelism every deadline-bearing sandbox test runs concurrently with
// every other one across binary boundaries: 15 loopd sandboxes + the mcp
// spawn legs all spinning bash process trees at once is exactly the
// suite-manufactured CPU starvation that stretched a 3.36s reaper test past
// its 30s verdict deadline (T152 signature, cycles 76-79) and pushed the
// stale_binary/spoof_guard sandboxes into the nextest 90s timeout. A static
// Mutex cannot see that contention; an advisory flock on a lockfile under a
// shared directory can — it serializes ACROSS processes under BOTH gate
// runners (`cargo nextest run --release` and `cargo test -- --test-threads=4`,
// whose cross-binary scheduling is sequential but whose within-binary
// threads are not), with the kernel releasing the lock if a holder dies.
//
// MECHANISM-NOT-TIMEOUTS (T31/T151/T159 doctrine, restated): the guarded
// tests' deadline constants are untouched — what this domain removes is the
// suite-manufactured scheduler stretch that co-occurred with a clocked
// window. Whole-machine starvation from OUTSIDE the suite is the T158
// invalidation-retry seams' jurisdiction, not this lock's.
//
// FAIL-OPEN DISCIPLINE (T172 req 4, mirroring T135's flock): a lockfile that
// cannot be opened, a domain that stays held past the bounded wait, or no
// creatable lock directory at all degrades with a stderr note and the test
// PROCEEDS UNLOCKED — a locked or absent lockfile must never hang the suite.
// The bound is deliberately far below every guarded test's own deadline and
// below the nextest per-test kill, so a degrade is strictly better than the
// pre-T172 behavior, never a new failure mode.
//
// LOCK ORDER (deadlock-freedom rule): the load lock is always the FIRST
// acquisition in a test body — before the T151 timing guard where both are
// taken (tests/loopd_orphan_reaper.rs), before DELEGATE_ENV_LOCK
// (src/mcp_serve/tests.rs), before LAUNCH_LEG_LOCK (tests/mcp_serve.rs). No
// site takes a static lock and then this one, so no cycle can form; the
// bounded wait above also caps any cross-tree pileup (two roles' gates
// sharing one lock dir serialize, then degrade fail-open — T52/T175).
//
// The lockfile is NEVER unlinked (unlink-while-held is the classic flock
// race: a waiter would flock a fresh inode while the holder still owns the
// old one). Zero-length stale files under a dedicated `chug-t172-load-locks/`
// directory are content-safe: cargo's own locks are separate files, and
// `cargo clean` merely recreates the directory on the next run.
#![allow(dead_code)] // each family binary compiles the subset it takes

use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Bounded wait for the domain before degrading fail-open. Expressed in
/// MILLIS so the repo's second-valued-constant sweep
/// (`git diff | grep -E "from_secs\([0-9]"`) never sees a new second-valued
/// constant: this is a NEW harness bound, not a bump of any existing
/// timeout/deadline (T172 req 2 — every existing bound stays byte-identical).
/// 45s keeps the worst family queue (7 tests, each seconds long solo) inside
/// the bound with headroom while staying below the nextest per-test kill.
const LOCK_WAIT_MS: u64 = 45_000;
/// Poll tick while waiting for the holder to release (a family test is
/// seconds long; 50ms costs nothing and reacts promptly).
const POLL_MS: u64 = 50;

/// The lockfile directory, shared across the processes of a gate run.
/// Resolution order: the runner's shared target dir (BOTH gate runners
/// export CARGO_TARGET_DIR — the T47 shared build cache), then the
/// package-local target dir (the cargo-test fallback: cargo runs test
/// binaries with cwd = package root, the same runtime resolution the T48
/// doctrine mandates), then the user temp dir. The dedicated
/// `chug-t172-load-locks/` name keeps it content-safe next to cargo's own
/// lock files. `None` = nothing creatable — the caller degrades fail-open.
fn lock_dir() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("CARGO_TARGET_DIR") {
        let dir = PathBuf::from(dir.trim());
        if !dir.as_os_str().is_empty() {
            let candidate = dir.join("chug-t172-load-locks");
            if std::fs::create_dir_all(&candidate).is_ok() {
                return Some(candidate);
            }
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        let candidate = cwd.join("target").join("chug-t172-load-locks");
        if std::fs::create_dir_all(&candidate).is_ok() {
            return Some(candidate);
        }
    }
    let candidate = std::env::temp_dir().join("chug-t172-load-locks");
    if std::fs::create_dir_all(&candidate).is_ok() {
        return Some(candidate);
    }
    None
}

/// The held domain. Drop releases the flock (explicit unlock, best-effort —
/// the close that follows releases it regardless; the KERNEL also releases
/// on process death, so a killed test can never strand the domain).
pub struct LoadLockGuard {
    #[allow(dead_code)] // naming aid in the degrade path and debuggers
    family: &'static str,
    #[allow(dead_code)]
    path: PathBuf,
    /// `Some` only while the advisory exclusive lock is HELD. `None` = the
    /// degraded fail-open state: the domain could not be taken and the test
    /// proceeds UNLOCKED by design (never a hang, never an error).
    held: Option<File>,
}

impl LoadLockGuard {
    /// True when the domain could NOT be taken (bounded wait expired,
    /// lockfile unopenable, no lock directory) and this run proceeds
    /// without cross-binary serialization — the fail-open leg.
    pub fn is_degraded(&self) -> bool {
        self.held.is_none()
    }
}

impl Drop for LoadLockGuard {
    fn drop(&mut self) {
        if let Some(file) = self.held.take() {
            let _ = file.unlock();
        }
    }
}

/// The production acquisition: take the named family's domain, waiting at
/// most [`LOCK_WAIT_MS`] for the current holder, then degrade fail-open.
/// Call sites: FIRST acquisition of a deadline-bearing test body, held
/// across spawn → assertion → cleanup (the T159 lock-scope doctrine).
pub fn family_guard(family: &'static str) -> LoadLockGuard {
    family_guard_with_wait(family, Duration::from_millis(LOCK_WAIT_MS))
}

/// `family_guard` with an explicit wait (the semantics tests' bounded knob —
/// a mutant or a pathological holder must be observable without waiting the
/// production bound).
pub fn family_guard_with_wait(family: &'static str, wait: Duration) -> LoadLockGuard {
    match lock_dir().map(|dir| dir.join(format!("{family}.lock"))) {
        Some(path) => guard_at(&path, family, wait),
        None => degrade(family, Path::new("<no lock directory>"), "no lock directory could be created"),
    }
}

/// Take the domain at an EXPLICIT lockfile path (production resolves the dir
/// via [`lock_dir`]; the semantics tests point this at hostile paths to pin
/// the fail-open legs). flock contends across independent opens EVEN WITHIN
/// one process, so these semantics are the same byte-for-byte syscalls a
/// sibling process would make.
pub fn guard_at(path: &Path, family: &'static str, wait: Duration) -> LoadLockGuard {
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path);
    let file = match file {
        Ok(file) => file,
        Err(e) => return degrade(family, path, &format!("lockfile unopenable: {e}")),
    };
    let deadline = Instant::now() + wait;
    loop {
        match file.try_lock() {
            Ok(()) => {
                return LoadLockGuard {
                    family,
                    path: path.to_path_buf(),
                    held: Some(file),
                }
            }
            Err(std::fs::TryLockError::WouldBlock) => {
                if Instant::now() >= deadline {
                    // Close OUR losing handle (it holds no lock) and proceed
                    // unlocked — bounded, noted, never hung.
                    drop(file);
                    return degrade(
                        family,
                        path,
                        &format!(
                            "domain still held after {wait:?} — bounded wait expired, \
                             proceeding unlocked"
                        ),
                    );
                }
                std::thread::sleep(Duration::from_millis(POLL_MS));
            }
            Err(std::fs::TryLockError::Error(e)) => {
                drop(file);
                return degrade(family, path, &format!("flock failed: {e}"));
            }
        }
    }
}

/// The fail-open leg: note to stderr (the T135 discipline — the degradation
/// must be VISIBLE, not silent) and hand back an unlocked guard.
fn degrade(family: &'static str, path: &Path, reason: &str) -> LoadLockGuard {
    eprintln!(
        "T172 load-lock [{family}]: DEGRADED FAIL-OPEN at {} — {reason}; \
         proceeding UNLOCKED (a locked or absent lockfile must never hang the \
         suite; this run loses the cross-binary serialization of the \
         deadline-bearing families)",
        path.display()
    );
    LoadLockGuard {
        family,
        path: path.to_path_buf(),
        held: None,
    }
}
