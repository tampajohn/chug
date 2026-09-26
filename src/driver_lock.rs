//! T55 — `.chug/driver.lock`: same-cwd mutual exclusion for concurrent
//! `chug run`s.
//!
//! The driver appends to `.chug/transcript.jsonl`, `.chug/events.jsonl`, and
//! `LEDGER.md` under `--cwd`; two drivers in one cwd splice each other's
//! streams. Until T55 the only guards were doctrine (META-SPEC: children
//! must not share your cwd) and loopd's supervisor-level skip — which cannot
//! stop a hand-launched second `chug run` in the same repo. This module is
//! the in-harness layer: `chug run` (including `--resume`) takes
//! `.chug/driver.lock` at startup — after `.chug/` exists and BEFORE the
//! first transcript rotation/append, so rotations are serialized too — and
//! holds it for the whole run.
//!
//! Decision rule: an existing lock is refused ONLY on the alive+chug
//! double-positive — the recorded pid passes `kill(pid, 0)` (process
//! exists) AND its CURRENT argv still names a chug binary (via
//! `ps -o command= -p <pid>`, NEVER pgrep: cycle-24 eval I1 found pgrep
//! blind to some macOS process trees; ps sees them). The argv leg is the
//! PID-reuse guard: a stale lock whose pid was recycled by an unrelated
//! process must not refuse. Every other leg — dead pid, argv mismatch,
//! unreadable/malformed/empty lock, probe failure (ps missing, spawn error)
//! — degrades to RECLAIM: overwrite the lock and continue (T20 never-fail
//! discipline). Probes return bool; "error" maps to false exactly once, at
//! the probe boundary, and never aborts the run.
//!
//! Release is best-effort on every normal exit path (the [`Guard`] drops
//! when `run_loop` returns — goal accepted, abort, budget death, error
//! unwind). SIGKILL bypasses destructors BY DESIGN: the holder dies without
//! unlinking, leaving a stale lock, and the next acquirer reclaims it via
//! the dead-pid leg above. Removing the lock on every exit path is why this
//! is safe: a lock file on disk is only ever a LIVE holder or provably
//! stale garbage, never a tombstone the operator must clean up. The
//! compare-then-delete in [`Guard::drop`] also means a reclaimed-and-rewritten
//! lock (a concurrent acquirer that saw us dead) is never unlinked by the
//! process it replaced.
//!
//! `chug chat` NEVER acquires, refuses, or removes this lock (LOOP-SPEC hard
//! rule: chat does not block a cycle; shared `.chug/` appends interleave
//! harmlessly). The call site lives only in the run startup path
//! (`driver::run_loop`); a static pin keeps chat lock-free: the
//! `chat_turn_never_creates_or_removes_the_driver_lock` unit test in
//! `src/driver.rs`'s test module.

use std::fs;
use std::path::{Path, PathBuf};

/// The lock file name under `<cwd>/.chug/`.
const LOCK_FILE: &str = "driver.lock";

/// Outcome of examining an existing lock file. [`HolderStatus::Acquire`] and
/// [`HolderStatus::Reclaim`] both proceed (the latter overwrites the file);
/// only [`HolderStatus::Held`] refuses the run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HolderStatus {
    /// No lock file present — acquire freely.
    Acquire,
    /// A lock exists but must be overwritten: its pid is unparseable, or the
    /// holder is dead, or the pid now belongs to a non-chug process, or a
    /// probe errored (mapped to not-alive by design — never abort).
    Reclaim,
    /// A live chug process (this pid) holds the lock — refuse to start.
    Held(u32),
}

/// Path of the lock file under `cwd`.
pub fn lock_path(cwd: &Path) -> PathBuf {
    cwd.join(".chug").join(LOCK_FILE)
}

/// The pure decision matrix (T28/T20 seam style): given the lock file's
/// contents (`None` when absent) and two injectable probes, decide acquire /
/// reclaim / refuse. The probes return bool; an impure probe that errors has
/// ALREADY been mapped to `false` by its wrapper (see [`pid_alive`] /
/// [`argv_names_chug`]) — so "probe error" lands on the not-alive legs here
/// and degrades to reclaim, never abort. The ONLY refusal is the explicit
/// alive + argv-names-chug double-positive.
pub fn holder_status<F, G>(lock_contents: Option<&str>, pid_alive: F, argv_names_chug: G) -> HolderStatus
where
    F: Fn(u32) -> bool,
    G: Fn(u32) -> bool,
{
    let Some(text) = lock_contents else {
        return HolderStatus::Acquire;
    };
    let Some(pid) = parse_holder_pid(text) else {
        return HolderStatus::Reclaim; // empty, malformed, or unparseable
    };
    if !pid_alive(pid) {
        return HolderStatus::Reclaim; // dead holder (incl. errored kill probe)
    }
    if !argv_names_chug(pid) {
        return HolderStatus::Reclaim; // pid reused by a non-chug process
    }
    HolderStatus::Held(pid)
}

/// Does `ps` say the pid's CURRENT argv names a chug binary? Read via
/// `ps -o command= -p <pid>` — NEVER pgrep (cycle-24 eval I1: pgrep
/// persistently fails to enumerate some macOS process trees; ps sees them).
/// Every failure leg — ps missing, spawn failure, non-zero exit (dead pid) —
/// reports `false`, which the caller treats as not-a-live-chug (reclaim).
/// A live, unrelated process whose command merely contains "chug" (e.g. an
/// editor open on `.chug/driver.lock`) is refused against too: the remedy is
/// deleting the lock, so the cost of this narrow false-positive is one
/// manual `rm`, while the cost of a false-negative is corrupted state.
pub fn argv_names_chug(pid: u32) -> bool {
    let out = std::process::Command::new("ps")
        .args(["-o", "command=", "-p", &pid.to_string()])
        .output();
    match out {
        Ok(out) if out.status.success() => {
            String::from_utf8_lossy(&out.stdout).contains("chug")
        }
        _ => false,
    }
}

/// Liveness probe: `kill(pid, 0)` delivers no signal and reports existence.
/// `EPERM` (exists, owned by another user) counts as alive — same semantics
/// as `tools.rs`'s `process_alive`. Pids that cannot name a process (0 =
/// "my process group", or anything past `i32::MAX`) report not-alive so
/// they degrade to reclaim. Non-unix has no probe: not-alive (reclaim).
pub fn pid_alive(pid: u32) -> bool {
    if pid == 0 || pid > i32::MAX as u32 {
        return false;
    }
    #[cfg(unix)]
    {
        // SAFETY: kill(2) with signal 0 on a pid that fits i32 — an
        // existence check that delivers no signal.
        let rc = unsafe { libc::kill(pid as i32, 0) };
        rc == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        false
    }
}

/// The lock file contract: the holder's pid ALONE on line 1; any later lines
/// are informational. Empty/malformed input → `None` (reclaim). Pid 0 and
/// out-of-i32 values are unparseable-as-a-real-pid → `None`.
fn parse_holder_pid(contents: &str) -> Option<u32> {
    let first = contents.lines().next()?.trim();
    let pid: u32 = first.parse().ok()?;
    (1..=i32::MAX as u32).contains(&pid).then_some(pid)
}

/// RAII holder of `.chug/driver.lock`. Dropping releases the lock
/// (compare-then-delete, best-effort) — this is what makes release cover
/// every normal exit path of the run, including error returns and panic
/// unwinds. SIGKILL skips destructors and leaves a stale lock by design;
/// the next acquirer reclaims it (see the module comment).
#[derive(Debug)]
pub struct Guard {
    /// `Some` only when this process actually wrote the lock file. `None`
    /// means the run proceeds WITHOUT exclusivity (the lock infrastructure
    /// was unwritable — warned on stderr; T20 never-fail).
    path: Option<PathBuf>,
    /// This acquirer's pid, for the compare-then-delete release.
    pid: u32,
}

/// Acquire `.chug/driver.lock` for the current process (run mode only).
///
/// - `Ok(guard)` — this process holds the lock until `guard` drops.
/// - `Err(message)` — a live chug run (pid named in the message) holds the
///   lock: the caller must refuse to start, printing the message to stderr
///   and exiting non-zero (the startup-error convention in `main.rs`).
///
/// Creates `.chug/` if needed, so this can sit BEFORE the first transcript
/// rotation/append and serialize the rotations too. A concurrent starter may
/// race the write; the write is verified and re-evaluated a bounded number
/// of times before degrading to lockless continuation. Unwritable lock
/// infrastructure degrades the same way — warn on stderr, run anyway
/// (T20: no new abort legs on the startup path).
pub fn acquire(cwd: &Path) -> Result<Guard, String> {
    let path = lock_path(cwd);
    let dir = path.parent().unwrap_or(cwd).to_path_buf();
    if let Err(e) = fs::create_dir_all(&dir) {
        eprintln!(
            "chug: warning: could not create {} ({e}); continuing without the driver lock",
            dir.display()
        );
        return Ok(Guard { path: None, pid: std::process::id() });
    }
    let me = std::process::id();
    for _ in 0..3 {
        let contents = fs::read_to_string(&path).ok();
        if let HolderStatus::Held(pid) =
            holder_status(contents.as_deref(), pid_alive, argv_names_chug)
        {
            return Err(refusal_message(pid));
        }
        if let Err(e) = write_lock(&path, me) {
            eprintln!(
                "chug: warning: could not write {} ({e}); continuing without the driver lock",
                path.display()
            );
            return Ok(Guard { path: None, pid: me });
        }
        // Verify we own what we wrote: a concurrent starter may have written
        // its own pid after our read. Not ours → loop and re-decide (refuse
        // if the winner is a live chug, otherwise race again).
        let mine = fs::read_to_string(&path)
            .ok()
            .and_then(|t| parse_holder_pid(&t))
            == Some(me);
        if mine {
            return Ok(Guard { path: Some(path), pid: me });
        }
    }
    eprintln!(
        "chug: warning: could not secure {} against concurrent writers; continuing without the driver lock",
        path.display()
    );
    Ok(Guard { path: None, pid: me })
}

/// The stderr message a refused run gets: names the holding pid and the
/// manual remedy (there is no override flag — the remedy is deleting the
/// file, which only makes sense if that run is truly gone).
fn refusal_message(pid: u32) -> String {
    format!(
        "refusing to start: .chug/driver.lock is held by a live chug run (pid {pid}) in this \
         cwd; if you know that run is gone, remove .chug/driver.lock"
    )
}

/// Write the lock: the holder's pid ALONE on line 1 (the parse contract),
/// the start epoch on line 2 (informational — how old is this lock?).
fn write_lock(path: &Path, pid: u32) -> std::io::Result<()> {
    let epoch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    fs::write(path, format!("{pid}\nstart_epoch {epoch}\n"))
}

impl Drop for Guard {
    fn drop(&mut self) {
        let Some(path) = self.path.take() else {
            return; // never held the file — nothing to release
        };
        // Compare-then-delete: unlink only when the file still names OUR
        // pid. If a concurrent acquirer reclaimed and rewrote the lock
        // (it believed us dead), deleting it would orphan THEIR exclusion.
        // Best-effort throughout: a read error keeps the file, which is the
        // documented stale-lock outcome — the next acquirer reclaims it.
        let still_ours = fs::read_to_string(&path)
            .ok()
            .and_then(|t| parse_holder_pid(&t))
            == Some(self.pid);
        if still_ours {
            let _ = fs::remove_file(&path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---------- T55 pure decision matrix (injectable probes) ----------

    /// Convenience wrapper: status with constant fakes for both probes.
    fn status_with(contents: Option<&str>, alive: bool, chug: bool) -> HolderStatus {
        holder_status(
            contents,
            move |_| alive,
            move |_| chug,
        )
    }

    #[test]
    fn no_lock_acquires() {
        // No lock file → acquire, regardless of what the probes would say.
        assert_eq!(status_with(None, false, false), HolderStatus::Acquire);
        assert_eq!(status_with(None, true, true), HolderStatus::Acquire);
    }

    #[test]
    fn dead_pid_reclaims() {
        assert_eq!(
            status_with(Some("4242\n"), false, true),
            HolderStatus::Reclaim,
            "dead holder: never refuse, overwrite and continue"
        );
    }

    #[test]
    fn alive_non_chug_argv_reclaims_pid_reuse_leg() {
        // Alive probe passes but the pid's argv no longer names chug: the
        // lock's pid was recycled by an unrelated process — must NOT refuse.
        assert_eq!(
            status_with(Some("4242\n"), true, false),
            HolderStatus::Reclaim
        );
    }

    #[test]
    fn alive_chug_argv_refuses() {
        // The ONLY refusal: the alive + argv-names-chug double-positive.
        assert_eq!(
            status_with(Some("4242\n"), true, true),
            HolderStatus::Held(4242)
        );
    }

    #[test]
    fn malformed_pid_reclaims() {
        for garbage in ["not-a-pid", "12.5", "-7", "0", "4294967295", "99999999999999"] {
            assert_eq!(
                status_with(Some(garbage), true, true),
                HolderStatus::Reclaim,
                "unparseable pid {garbage:?} must reclaim even with both probes positive"
            );
        }
    }

    #[test]
    fn empty_file_reclaims() {
        assert_eq!(status_with(Some(""), true, true), HolderStatus::Reclaim);
        assert_eq!(status_with(Some("   \n "), true, true), HolderStatus::Reclaim);
    }

    #[test]
    fn probe_errors_degrade_to_reclaim_not_abort() {
        // The impure wrappers map probe errors to `false` exactly once at the
        // probe boundary; at this seam an errored probe is indistinguishable
        // from a negative one. Both error-shaped legs must land on Reclaim:
        // kill probe errors (→ not alive)…
        assert_eq!(status_with(Some("4242\n"), false, true), HolderStatus::Reclaim);
        // …and ps probe errors on a live pid (→ argv doesn't name chug).
        assert_eq!(status_with(Some("4242\n"), true, false), HolderStatus::Reclaim);
    }

    #[test]
    fn pid_alone_on_line_one_is_the_contract_tail_is_informational() {
        let text = "4242\nstart_epoch 1696000000\n";
        assert_eq!(
            holder_status(Some(text), |_| true, |_| true),
            HolderStatus::Held(4242),
            "informational lines after the pid never break the parse"
        );
        assert_eq!(
            holder_status(Some("4242\n"), |_| true, |_| true),
            HolderStatus::Held(4242)
        );
        assert_eq!(
            holder_status(Some(" 4242 \n"), |_| true, |_| true),
            HolderStatus::Held(4242),
            "surrounding whitespace tolerated"
        );
    }

    #[test]
    fn parsed_pid_is_the_holding_pid() {
        // The refusal must name the pid that actually holds the lock.
        match status_with(Some("777\n"), true, true) {
            HolderStatus::Held(pid) => assert_eq!(pid, 777),
            other => panic!("expected Held, got {other:?}"),
        }
    }

    // ---------- impure probe wiring (real processes, no injection) ----------

    #[cfg(unix)]
    #[test]
    fn real_sleep_is_alive_but_not_chug_then_dead_after_reap() {
        let mut child = std::process::Command::new("sleep")
            .arg("37")
            .spawn()
            .expect("spawning sleep");
        let pid = child.id();

        assert!(pid_alive(pid), "a spawned sleep answers kill(pid, 0)");
        assert!(
            !argv_names_chug(pid),
            "sleep's argv does not name chug → not a live chug holder"
        );
        // Full wiring, no injection: a lock naming this LIVE non-chug pid
        // reclaims (the PID-reuse leg against a real process).
        let text = format!("{pid}\nstart_epoch 0\n");
        assert_eq!(
            holder_status(Some(&text), pid_alive, argv_names_chug),
            HolderStatus::Reclaim
        );

        // Kill + reap: a zombie child still answers kill(pid, 0) (T28), so
        // the wait must precede the dead-pid assertion.
        child.kill().expect("killing sleep");
        child.wait().expect("reaping sleep");
        assert!(!pid_alive(pid), "reaped pid is gone");
        assert_eq!(
            holder_status(Some(&text), pid_alive, argv_names_chug),
            HolderStatus::Reclaim,
            "dead-pid leg against a real process"
        );
    }

    #[test]
    fn pid_alive_rejects_pids_that_cannot_name_a_process() {
        assert!(!pid_alive(0), "pid 0 means 'my process group', not a holder");
        assert!(!pid_alive(u32::MAX), "beyond i32::MAX — cast would go negative");
    }

    #[cfg(unix)]
    #[test]
    fn argv_probe_errors_and_dead_pids_map_to_false() {
        // A pid far beyond every platform's pid_max: ps exits non-zero and
        // kill reports ESRCH — both legs must report false (reclaim), never
        // panic or abort.
        let impossible = u32::try_from(i32::MAX).unwrap();
        assert!(!argv_names_chug(impossible));
        assert!(!pid_alive(impossible));
        assert_eq!(
            holder_status(Some(&format!("{impossible}\n")), pid_alive, argv_names_chug),
            HolderStatus::Reclaim
        );
    }

    // ---------- acquire + release against the real filesystem ----------

    #[test]
    fn acquire_writes_pid_on_line_one_and_release_removes() {
        let tmp = tempfile::tempdir().unwrap();
        let guard = acquire(tmp.path()).expect("fresh cwd acquires");
        let path = lock_path(tmp.path());
        assert!(path.exists(), "lock written");
        let text = fs::read_to_string(&path).unwrap();
        let first = text.lines().next().expect("lock has a line").trim();
        assert_eq!(
            first,
            std::process::id().to_string(),
            "the acquirer's pid alone on line 1"
        );
        assert!(text.lines().count() >= 1);
        drop(guard);
        assert!(!path.exists(), "release removes the lock");
    }

    #[test]
    fn acquire_reclaims_stale_dead_holder_lock() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join(".chug")).unwrap();
        // Far beyond every platform's pid_max: no live process can hold it.
        let stale = format!("{}\nstart_epoch 0\n", i32::MAX);
        fs::write(lock_path(tmp.path()), stale).unwrap();
        let guard = acquire(tmp.path()).expect("stale lock is reclaimed, never refused");
        let text = fs::read_to_string(lock_path(tmp.path())).unwrap();
        assert_eq!(
            text.lines().next().unwrap().trim(),
            std::process::id().to_string(),
            "reclaim overwrites the lock with the acquirer's pid"
        );
        drop(guard);
        assert!(!lock_path(tmp.path()).exists());
    }

    #[test]
    fn release_leaves_a_rewritten_lock_alone() {
        // Compare-then-delete: if a concurrent acquirer reclaimed the lock
        // (it believed us dead) and rewrote it with ITS pid, our release
        // must not unlink their exclusion.
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join(".chug")).unwrap();
        let path = lock_path(tmp.path());
        let guard = acquire(tmp.path()).unwrap();
        // Another acquirer overwrites between our acquire and our release.
        let other = format!("{}\nstart_epoch 0\n", i32::MAX);
        fs::write(&path, other).unwrap();
        drop(guard);
        let text = fs::read_to_string(&path).unwrap();
        assert_eq!(
            text.lines().next().unwrap().trim(),
            i32::MAX.to_string(),
            "the new holder's lock survives our release"
        );
    }

    #[test]
    fn lock_never_refuses_on_an_unwritable_lock_path() {
        // T20 never-fail: lock infrastructure failures degrade to running
        // without exclusivity (warned on stderr), never aborting the run.
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join(".chug")).unwrap();
        // A DIRECTORY where the lock FILE must live: fs::write fails.
        let as_dir = lock_path(tmp.path());
        fs::create_dir_all(&as_dir).unwrap();
        let guard = acquire(tmp.path())
            .expect("an unwritable lock file must never refuse the run");
        drop(guard);
        assert!(as_dir.is_dir(), "the obstacle is left as it was");
    }
}
