//! T204 phase 1 (F15) — the `chug daemon` lifecycle, proven against the REAL
//! binary (`CARGO_BIN_EXE_chug`) over a REAL unix socket, weightless.
//!
//! Every child runs with `CHUG_HOME` pointed at its own tempdir (host-scoped
//! daemon home per the spec: never the per-repo `.chug`) and
//! `CHUG_DAEMON_STUB=1` — the test seam that serves the REAL transport
//! (0600 socket bind, single-instance flock, HTTP/1.1 loop) with NO model;
//! a stub must never fabricate classifications (SPEC-3), so /judge refuses.
//! Env is scoped per child via `Command::env` — never the test process's
//! global env, which parallel test threads share (the mcp_serve.rs rule).
//!
//! Pins the spec's Tests list at the process level, which the in-process
//! unit legs in src/daemon.rs cannot reach:
//! - a second `chug daemon` exits on the single-instance lock;
//! - auto-spawn (`chug daemon --ensure`) brings up a healthy daemon;
//! - `--stop` leaves no orphan and no socket;
//! - a SIGKILL'd daemon leaves a stale socket file — ensure() recovers:
//!   connect fails, unlink, respawn, healthy again;
//! - the socket file mode is 0600;
//! - a feature-off build refuses to serve with a clear message.
//!
//! Every read is deadline-bounded (the T6 stub-harness rule): a wedged
//! daemon fails its test in seconds, it can never hang the suite. Each test
//! cleans up its daemon through a Drop guard so a failing assertion cannot
//! leak an orphan.
#![cfg(unix)]

use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// Deadline for a fresh stub daemon to answer /healthz (the stub skips the
/// model load, so the bind is immediate; the budget only covers machine
/// load).
const HEALTH_DEADLINE: Duration = Duration::from_secs(90);
/// Deadline for a lifecycle CLI verb to exit. MUST exceed the ensure's own
/// internal SPAWN_WAIT_BUDGET (60s) — a deadline at parity loses the race
/// deterministically whenever the child legitimately needs its full budget —
/// plus headroom for the macOS first-exec stall (a freshly rebuilt test
/// binary's first execs can sit in _dyld_start under Gatekeeper/syspolicyd
/// assessment for tens of seconds; observed live: a 48s sample mid-stall).
const CLI_DEADLINE: Duration = Duration::from_secs(150);
/// Deadline for a daemon expected to DIE (lock refusal / stop) to exit —
/// generous for the same first-exec stall (the child must LOAD before it can
/// refuse anything).
const EXIT_DEADLINE: Duration = Duration::from_secs(60);

/// One test's daemon home: a tempdir used as `CHUG_HOME` (the host-scoped
/// daemon home — lock, socket, and log all live inside it), plus the daemon
/// pid to kill on drop. The TempDir is OWNED here — it removes itself on
/// drop, AFTER the daemon-kill guard below has run (a struct's own drop
/// runs before its fields'), so no `$TMPDIR` litter is left behind.
struct Home {
    dir: tempfile::TempDir,
    pid: Option<u32>,
}

impl Home {
    fn new() -> Self {
        Self {
            dir: tempfile::tempdir().expect("tempdir"),
            pid: None,
        }
    }

    fn sock(&self) -> PathBuf {
        self.dir.path().join("daemon.sock")
    }

    fn lock(&self) -> PathBuf {
        self.dir.path().join("daemon.lock")
    }

    /// Spawn a `chug` child with the home env scoped to THIS child (never
    /// the test process's global env) and its stderr captured for asserts.
    fn spawn_cli(&self, args: &[&str], stub: bool) -> Child {
        let mut command = Command::new(env!("CARGO_BIN_EXE_chug"));
        command
            .args(args)
            .env("CHUG_HOME", self.dir.path())
            .env_remove("CHUG_DAEMON_SOCK")
            .env_remove("CHUG_LAYA_CHECKPOINT")
            .env_remove("CHUG_DELEGATE_BIN")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if stub {
            command.env("CHUG_DAEMON_STUB", "1");
        }
        command.spawn().expect("spawn CARGO_BIN_EXE_chug")
    }

    /// Spawn the daemon in serve mode (stub) as a tracked child: its pid is
    /// recorded for the Drop guard before ownership moves to the caller.
    fn spawn_daemon(&mut self) -> Child {
        let child = self.spawn_cli(&["daemon"], true);
        self.pid = Some(child.id());
        child
    }

    /// Spawn a RIVAL daemon (the lock-refusal leg) WITHOUT tracking it: the
    /// rival must exit on the held lock, so the Drop guard must keep killing
    /// daemon ONE. Tracking the rival would overwrite `pid` — an early
    /// assertion failure would then kill the (already-exited) rival's pid
    /// and orphan the live daemon. And the lock file still names daemon one
    /// at that point (the rival refused before writing its line), so no pid
    /// read back from the home can ever be attributed to the rival.
    fn spawn_rival(&self) -> Child {
        self.spawn_cli(&["daemon"], true)
    }

    /// Absorb the macOS first-exec stall (Gatekeeper/syspolicyd assessment of
    /// a freshly rebuilt binary — `sample` shows the child parked in
    /// `_dyld_start` for tens of seconds): exec the binary once, bounded, and
    /// wait for it BEFORE any timed lifecycle section. `--version` is instant
    /// and side-effect-free. Called at the top of every test.
    fn warm_exec(&self) {
        let child = Command::new(env!("CARGO_BIN_EXE_chug"))
            .arg("--version")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn the warm-up exec");
        let output = wait_with_deadline(child, Duration::from_secs(120))
            .expect("the warm-up exec never exited");
        assert_eq!(
            output.status.code(),
            Some(0),
            "the warm-up exec (--version) failed"
        );
    }

    /// Run a bounded CLI verb to completion, returning (status, stdout, stderr).
    fn run_cli(&self, args: &[&str], stub: bool) -> (Option<i32>, String, String) {
        let child = self.spawn_cli(args, stub);
        let output =
            wait_with_deadline(child, CLI_DEADLINE).expect("the CLI verb never exited in time");
        (output.status.code(), output.stdout, output.stderr)
    }
}

impl Drop for Home {
    fn drop(&mut self) {
        // Never leak a daemon on a failing assertion: kill the tracked daemon
        // hard (stale-socket recovery is itself a tested behavior), else stop
        // through the tested --stop interface (covers --ensure-spawned
        // granddaughters, which are detached and not our direct children).
        if let Some(pid) = self.pid.take() {
            let _ = Command::new("kill")
                .args(["-9", &pid.to_string()])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        } else if self.sock().exists() {
            let _ = self.spawn_cli(&["daemon", "--stop"], false).wait();
        }
    }
}

/// Bounded wait for a child to exit, capturing output (the piped stderr
/// volume here is a few lines, well under the pipe buffer, so collecting
/// after exit cannot deadlock).
fn wait_with_deadline(mut child: Child, deadline: Duration) -> std::io::Result<ChildOutput> {
    let started = Instant::now();
    loop {
        if child.try_wait()?.is_some() {
            let output = child.wait_with_output()?;
            return Ok(ChildOutput {
                status: output.status,
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }
        if started.elapsed() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("child did not exit within {deadline:?}");
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

/// The collected output of an exited child (status + both pipes as strings).
struct ChildOutput {
    status: std::process::ExitStatus,
    #[allow(dead_code)]
    stdout: String,
    stderr: String,
}

/// One hand-rolled HTTP GET over the unix socket (no lib access from an
/// integration test; the wire is the daemon's public contract). Returns
/// (status, body).
fn uds_get(sock: &Path, path: &str) -> Result<(u16, String), String> {
    let mut stream = UnixStream::connect(sock).map_err(|e| e.to_string())?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let request = format!("GET {path} HTTP/1.1\r\nHost: t\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).map_err(|e| e.to_string())?;
    let mut raw = Vec::new();
    (&stream)
        .take(1 << 20)
        .read_to_end(&mut raw)
        .map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&raw);
    let (head, body) = text
        .split_once("\r\n\r\n")
        .ok_or_else(|| "no header terminator".to_string())?;
    let status = head
        .split_whitespace()
        .nth(1)
        .and_then(|part| part.parse().ok())
        .ok_or_else(|| "malformed status line".to_string())?;
    Ok((status, body.to_string()))
}

/// Bounded poll until /healthz answers 200; returns the body.
fn wait_healthz(sock: &Path) -> String {
    let deadline = Instant::now() + HEALTH_DEADLINE;
    loop {
        if let Ok((200, body)) = uds_get(sock, "/healthz") {
            return body;
        }
        assert!(
            Instant::now() < deadline,
            "the daemon never answered /healthz at {}",
            sock.display()
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// The pid line the lock file records (informational; --stop identity-checks).
fn lock_pid(lock: &Path) -> Option<u32> {
    std::fs::read_to_string(lock)
        .ok()?
        .lines()
        .next()?
        .trim()
        .parse()
        .ok()
}

#[test]
fn second_daemon_exits_on_the_lock() {
    let mut home = Home::new();
    home.warm_exec();
    let mut first = home.spawn_daemon();
    let body = wait_healthz(&home.sock());
    assert!(body.contains("\"status\":\"ok\""), "healthz body: {body}");

    // The rival: same home, same stub — must refuse on the flock and exit
    // non-zero with the holder named, never park. NOT tracked in `pid` (see
    // spawn_rival): the Drop guard must keep pointing at daemon one.
    let second = home.spawn_rival();
    let output = wait_with_deadline(second, EXIT_DEADLINE)
        .expect("the second daemon never exited on the lock");
    assert_ne!(
        output.status.code(),
        Some(0),
        "the second daemon must exit non-zero on the held lock"
    );
    let stderr = &output.stderr;
    assert!(
        stderr.contains("held by"),
        "the lock refusal must name the holder: {stderr}"
    );

    // The first daemon is unaffected and still serves.
    let (status, body) = uds_get(&home.sock(), "/healthz").expect("first daemon alive");
    assert_eq!(status, 200, "first daemon still healthy: {body}");
    first.kill().expect("kill first");
    let _ = first.wait();
    home.pid = None;
}

#[test]
fn ensure_brings_up_a_healthy_daemon_and_socket_is_0600() {
    let home = Home::new();
    home.warm_exec();
    let (code, stdout, stderr) = home.run_cli(&["daemon", "--ensure"], true);
    assert_eq!(code, Some(0), "ensure failed: {stdout} / {stderr}");
    assert!(stdout.contains("healthy"), "ensure stdout: {stdout}");

    // The socket file exists, is a real socket, and is mode 0600 (the
    // filesystem-perms policy gate — no port, no network listener).
    let sock = home.sock();
    assert!(sock.exists(), "the socket file exists after ensure");
    let mode = std::fs::metadata(&sock)
        .expect("stat the socket")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600, "the daemon socket must be mode 0600");

    let (status, body) = uds_get(&sock, "/healthz").expect("healthz after ensure");
    assert_eq!(status, 200);
    assert!(body.contains("\"status\":\"ok\""), "healthz body: {body}");

    // Idempotent: a second ensure finds the healthy daemon and exits 0.
    let (code, stdout, stderr) = home.run_cli(&["daemon", "--ensure"], true);
    assert_eq!(code, Some(0), "second ensure failed: {stdout} / {stderr}");

    // --status reports running (exit 0) while it answers.
    let (code, stdout, _) = home.run_cli(&["daemon", "--status"], false);
    assert_eq!(code, Some(0), "status on a running daemon: {stdout}");
    assert!(stdout.contains("running"), "status stdout: {stdout}");

    // /judge is refused by the stub (never fabricates classifications) with
    // the layad error shape — the fail-open contract downstream.
    let mut stream = UnixStream::connect(&sock).expect("connect for judge");
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let request = "POST /judge HTTP/1.1\r\nHost: t\r\nContent-Type: application/json\r\n\
                   Content-Length: 27\r\nConnection: close\r\n\r\n{\"state\":{},\"questions\":{}}";
    stream.write_all(request.as_bytes()).expect("write judge");
    let mut raw = Vec::new();
    let _ = (&stream).take(1 << 20).read_to_end(&mut raw);
    let text = String::from_utf8_lossy(&raw);
    assert!(text.starts_with("HTTP/1.1 500"), "stub /judge status: {text}");
    assert!(text.contains("\"detail\""), "stub /judge error shape: {text}");

    let _ = home.spawn_cli(&["daemon", "--stop"], false).wait();
}

#[test]
fn stop_leaves_no_orphan() {
    let mut home = Home::new();
    home.warm_exec();
    // HOLD the child handle: dropping it would close the daemon's piped
    // stdout/stderr and SIGPIPE its banner write (the daemon dies, the test
    // degrades into the stale-socket shape).
    let mut daemon = home.spawn_daemon();
    wait_healthz(&home.sock());
    let pid = lock_pid(&home.lock()).expect("the lock records the daemon pid");

    let (code, stdout, stderr) = home.run_cli(&["daemon", "--stop"], false);
    assert_eq!(code, Some(0), "stop failed: {stdout} / {stderr}");
    // The stop line names the SAME pid the lock recorded (identity, not luck).
    assert!(
        stdout.contains(&format!("stopped pid {pid}")),
        "stop stdout: {stdout}"
    );

    // No orphan: the process is gone within the bounded window. Reap through
    // the held child handle — kill(0) alone cannot see death here: the daemon
    // is OUR child, and an unreaped zombie still answers a liveness probe.
    let deadline = Instant::now() + EXIT_DEADLINE;
    while daemon.try_wait().expect("poll the daemon").is_none() {
        assert!(Instant::now() < deadline, "the daemon outlived --stop");
        std::thread::sleep(Duration::from_millis(50));
    }
    // And the socket file is gone with it.
    assert!(!home.sock().exists(), "--stop removed the socket file");

    // Status now reports absent (exit 1).
    let (code, stdout, _) = home.run_cli(&["daemon", "--status"], false);
    assert_eq!(code, Some(1), "status on a stopped daemon: {stdout}");
    assert!(stdout.contains("not running"), "status stdout: {stdout}");
    // Reap the TERM'd daemon (the handle was held open above) so the guard
    // does not kill a recycled pid.
    let _ = daemon.wait();
    home.pid = None;
}

#[test]
fn stale_socket_after_sigkill_recovers() {
    let mut home = Home::new();
    home.warm_exec();
    let mut first = home.spawn_daemon();
    wait_healthz(&home.sock());
    let old_pid = lock_pid(&home.lock()).expect("lock pid before the kill");
    assert_ne!(old_pid, std::process::id());

    // SIGKILL — no cleanup, no unlink: exactly the orphaned-daemon shape.
    // The reap through the held handle IS the death proof (a recycled pid
    // would make any later kill(0) probe lie).
    first.kill().expect("SIGKILL the daemon");
    let _ = first.wait();
    home.pid = None;
    assert!(home.sock().exists(), "the stale socket file remains after SIGKILL");

    // Recovery: connect fails (no listener) -> unlink -> respawn -> healthy.
    let (code, stdout, stderr) = home.run_cli(&["daemon", "--ensure"], true);
    assert_eq!(code, Some(0), "ensure after a stale socket: {stdout} / {stderr}");
    let (status, body) = uds_get(&home.sock(), "/healthz").expect("healthz after recovery");
    assert_eq!(status, 200, "recovered daemon body: {body}");
    let new_pid = lock_pid(&home.lock()).expect("lock pid after recovery");
    assert_ne!(new_pid, old_pid, "a NEW daemon process owns the lock");

    let _ = home.spawn_cli(&["daemon", "--stop"], false).wait();
}

/// A default (feature-off) build refuses to serve with a clear message —
/// the spec's "fail with a clear message on a default build" requirement.
/// Compiled out of feature builds (where serve would attempt a real model
/// load, never done in tests).
#[cfg(not(feature = "daemon"))]
#[test]
fn feature_off_build_refuses_to_serve() {
    let home = Home::new();
    home.warm_exec();
    // No CHUG_DAEMON_STUB: serve takes the lock, then refuses on the missing
    // inference stack.
    let child = home.spawn_cli(&["daemon"], false);
    let output = wait_with_deadline(child, EXIT_DEADLINE)
        .expect("the feature-off daemon never exited");
    assert_ne!(output.status.code(), Some(0), "serve must fail on a feature-off build");
    let stderr = &output.stderr;
    assert!(
        stderr.contains("built without the judge daemon"),
        "the refusal must be clear: {stderr}"
    );
    // And no socket was bound (the refusal precedes serving).
    assert!(!home.sock().exists(), "a feature-off daemon binds no socket");
}
