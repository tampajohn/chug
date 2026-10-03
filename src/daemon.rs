//! T204 (F15 phase 1) — the baked-in Laya judge daemon: `chug daemon` hosts
//! the /judge inference server over a UNIX DOMAIN SOCKET so the risk gate
//! (SPEC-3 semantic layer) works with zero external services.
//!
//! Today (pre-T204) two surfaces depend on the external Python `layad`
//! daemon at `LAYA_URL` (default 127.0.0.1:8420): the risk gate's
//! `LayaJudge` and the notify layad sink. On the loop host that daemon is
//! connection-refused dead, so risk-gated runs silently lose the semantic
//! layer. This module is the drop-in replacement for the JUDGE half:
//!
//! * Transport — HTTP/1.1 semantics over `$CHUG_HOME/daemon.sock` (default
//!   `~/.chug/daemon.sock` — host-scoped, NOT the per-repo `.chug`;
//!   `CHUG_DAEMON_SOCK` overrides), mode 0600. No TCP listener in phase 1:
//!   no port doctrine to defend, filesystem perms gate the policy surface to
//!   the user's own processes, and nothing for EDR to flag (F94 S1 history).
//!   `curl --unix-socket` is the debugging tool; the wire shapes are
//!   byte-identical to layad's (`POST /judge` -> the `system_one` response
//!   `{"model","answers","usage"}`; errors are non-2xx + `{"detail": …}`),
//!   so only the framing differs.
//! * Inference — `JudgeModel` (CHILD A, `#[cfg(feature = "daemon")]`) loads
//!   ONCE per host inside the daemon process (`DAEMON-NOT-IN-PROCESS`: the
//!   ~650 MB weights never enter a per-run chug process; the client keeps
//!   its lean hot path — `tests/daemon_feature_off.rs` pins zero candle deps
//!   in the default build).
//! * Lifecycle — single-instance flock on `$CHUG_HOME/daemon.lock` (the
//!   driver-lock pattern, DISTINCT from the per-repo `.chug/driver.lock`);
//!   auto-spawn on the first judge call when `CHUG_JUDGE=daemon` (detached
//!   self-exe spawn, bounded wait for socket + /healthz); stale-socket
//!   recovery (connect ECONNREFUSED -> unlink -> respawn, never a hard
//!   error); `chug daemon --stop|--status|--ensure`.
//! * Client selection — `CHUG_JUDGE=daemon|http|off` (riskgate.rs): `daemon`
//!   is the ~60-line hand-rolled HTTP/1.1-over-UnixStream transport below
//!   (reqwest 0.12 has no UDS support; no new deps), `http` is today's
//!   reqwest TCP path byte-for-byte, `off` disables the judge. Every failure
//!   fails OPEN exactly as the HTTP path does.
//!
//! SPEC-3 constraint carried verbatim: the daemon classifies, it never
//! generates. `/judge` answers questions about a submitted state; there is
//! no completion/stuck/verdict-final surface, and the `CHUG_DAEMON_STUB=1`
//! test seam refuses `/judge` outright — a stub must never fabricate
//! classifications (see [`StubBackend`]).

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, bail};
use serde_json::{Value, json};

use crate::judge_pack::OValue;
use crate::riskgate::{Judge, Verdict};

/// Socket file name under the chug home (host-scoped).
pub const SOCK_FILE: &str = "daemon.sock";
/// Single-instance lock under the chug home (flock — the driver-lock
/// pattern, but the daemon lock is host-scoped and DISTINCT from the
/// per-repo `.chug/driver.lock`).
pub const LOCK_FILE: &str = "daemon.lock";
/// The daemon's own stdout+stderr log (spawn appends; `nohup` parity with
/// the delegate log).
pub const LOG_FILE: &str = "daemon.log";
/// `CHUG_DAEMON_SOCK` — full socket path override.
pub const SOCK_ENV: &str = "CHUG_DAEMON_SOCK";
/// `CHUG_HOME` — the daemon's home dir override (default `~/.chug`).
pub const HOME_ENV: &str = "CHUG_HOME";
/// `CHUG_DAEMON_STUB=1` — the lifecycle test seam: serve real transport +
/// lock + socket with NO model; /judge refuses (see [`StubBackend`]).
pub const STUB_ENV: &str = "CHUG_DAEMON_STUB";
/// `CHUG_DELEGATE_BIN` — the delegate test seam, honored here too so tests
/// can point spawns at a fake binary; production resolves the running
/// chug itself (the one-binary/subcommand pattern).
pub const BINARY_ENV: &str = "CHUG_DELEGATE_BIN";

/// Judge calls must fail fast so the agent loop is never stalled — the same
/// 2s doctrine as the HTTP path's `JUDGE_TIMEOUT_SECS`.
const JUDGE_TIMEOUT: Duration = Duration::from_secs(2);
/// Bounded budget for the auto-spawn wait (socket + /healthz). Covers a warm
/// cache load (~seconds); a cold first-time weights download can exceed it —
/// the client then fails open for THIS call and the daemon finishes loading
/// for the next one.
const SPAWN_WAIT_BUDGET: Duration = Duration::from_secs(60);
/// Healthz probe cadence while waiting for a spawned daemon.
const PROBE_INTERVAL: Duration = Duration::from_millis(250);
/// Bounded per-connection request caps (a hostile/broken client cannot
/// balloon the daemon).
const MAX_HEADER_BYTES: usize = 64 * 1024;
const MAX_BODY_BYTES: usize = 1024 * 1024;
const MAX_RESPONSE_BYTES: u64 = 1024 * 1024;

// ---------------------------------------------------------------------------
// Paths
// ---------------------------------------------------------------------------

/// The daemon's home: `$CHUG_HOME` when set, else `$HOME/.chug` (host-scoped
/// — NOT the per-repo `.chug`).
pub fn chug_home() -> anyhow::Result<PathBuf> {
    if let Some(home) = std::env::var_os(HOME_ENV).filter(|v| !v.is_empty()) {
        return Ok(PathBuf::from(home));
    }
    let home = std::env::var_os("HOME").filter(|v| !v.is_empty()).map(PathBuf::from);
    match home {
        Some(home) => Ok(home.join(".chug")),
        None => bail!("cannot resolve the chug daemon home: neither {HOME_ENV} nor HOME is set"),
    }
}

/// The socket path: `$CHUG_DAEMON_SOCK` wins, else `<chug home>/daemon.sock`.
pub fn sock_path() -> anyhow::Result<PathBuf> {
    if let Some(sock) = std::env::var_os(SOCK_ENV).filter(|v| !v.is_empty()) {
        return Ok(PathBuf::from(sock));
    }
    Ok(chug_home()?.join(SOCK_FILE))
}

/// The single-instance lock path: `<chug home>/daemon.lock` (independent of
/// `CHUG_DAEMON_SOCK` — one lock per host home).
pub fn lock_path() -> anyhow::Result<PathBuf> {
    Ok(chug_home()?.join(LOCK_FILE))
}

/// The daemon's own log: `<chug home>/daemon.log`.
pub fn log_path() -> anyhow::Result<PathBuf> {
    Ok(chug_home()?.join(LOG_FILE))
}

// ---------------------------------------------------------------------------
// The client: hand-rolled HTTP/1.1 over UnixStream (NO new deps)
// ---------------------------------------------------------------------------

/// One HTTP request over the unix socket: send `method path` with an optional
/// JSON body, read the bounded response (the server answers with
/// `Connection: close`, so read-to-EOF terminates). Returns (status, body).
pub fn uds_request(
    sock: &Path,
    method: &str,
    path: &str,
    body: Option<&str>,
) -> Result<(u16, String), String> {
    #[cfg(unix)]
    {
        use std::os::unix::net::UnixStream;

        let body = body.unwrap_or("");
        let mut stream = UnixStream::connect(sock)
            .map_err(|e| format!("connect {}: {e}", sock.display()))?;
        let _ = stream.set_write_timeout(Some(JUDGE_TIMEOUT));
        let _ = stream.set_read_timeout(Some(JUDGE_TIMEOUT));
        let request = format!(
            "{method} {path} HTTP/1.1\r\nHost: chug-daemon\r\nContent-Type: \
             application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        stream
            .write_all(request.as_bytes())
            .map_err(|e| format!("writing the judge request: {e}"))?;
        let mut raw = Vec::new();
        {
            let mut limited = (&stream).take(MAX_RESPONSE_BYTES);
            limited
                .read_to_end(&mut raw)
                .map_err(|e| format!("reading the judge response: {e}"))?;
        }
        parse_http_response(&raw)
    }
    #[cfg(not(unix))]
    {
        let _ = (sock, method, path, body);
        Err("unix domain sockets are not supported on this platform".into())
    }
}

/// Parse one bounded HTTP/1.1 response: status code + body. The daemon always
/// answers `Content-Length` + `Connection: close`, so the client's
/// read-to-EOF framing is exact.
fn parse_http_response(raw: &[u8]) -> Result<(u16, String), String> {
    let text = String::from_utf8_lossy(raw);
    let (head, body) = text
        .split_once("\r\n\r\n")
        .ok_or_else(|| format!("malformed HTTP response (no header terminator): {} bytes", raw.len()))?;
    let status_line = head.split("\r\n").next().unwrap_or_default();
    let status: u16 = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|part| part.parse().ok())
        .ok_or_else(|| format!("malformed HTTP status line: {status_line:?}"))?;
    Ok((status, body.to_string()))
}

/// The daemon client judge (CHUG_JUDGE=daemon): ensures a healthy daemon,
/// then POSTs the exact same request body the HTTP path builds.
pub struct DaemonJudge {
    sock: PathBuf,
    /// The daemon this process spawned, held for opportunistic reaping (a
    /// handle dropped without `wait` would leave a zombie once the daemon
    /// exits while this process is still alive).
    spawned: Option<Child>,
    /// The permanent-failure latch: once ensure() concludes nothing will ever
    /// serve this socket (the spawned daemon exited with no lock holder — a
    /// feature-off binary, an unloadable checkpoint — or the spawn itself
    /// failed, or the cumulative wait blew the budget), every later judge
    /// call fails open INSTANTLY instead of re-spawning per bash command.
    /// Never set for transient flavors (budget expiry while a daemon is
    /// still loading — the next call may well find it warm).
    dead: Option<String>,
    /// Cumulative wall-clock spent in failed ensure() attempts since the last
    /// success; past [`MAX_LIFETIME_WAIT`] the client latches off (a run
    /// must never stall minutes per bash command on a daemon that will not
    /// come up).
    waited: Duration,
}

/// Cumulative failed-ensure wait before the client latches off for the
/// process lifetime (two full spawn budgets of futile waiting).
const MAX_LIFETIME_WAIT: Duration = Duration::from_secs(2 * SPAWN_WAIT_BUDGET.as_secs());

/// Why a judge client could not reach a healthy daemon. `latch` marks the
/// permanent flavors: retrying within this process cannot help (nothing is
/// coming up); the caller records the message and never re-attempts.
struct EnsureFailure {
    message: String,
    latch: bool,
}

impl DaemonJudge {
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            sock: sock_path()?,
            spawned: None,
            dead: None,
            waited: Duration::ZERO,
        })
    }

    /// Bring up a healthy daemon: probe, recover a stale socket, spawn,
    /// bounded wait. Errors carry a latch flag (see [`EnsureFailure`]).
    fn ensure(&mut self) -> Result<(), EnsureFailure> {
        ensure_with(&self.sock, &mut self.spawned)
    }
}

impl Judge for DaemonJudge {
    fn judge(&mut self, command: &str) -> Result<Verdict, String> {
        if let Some(message) = &self.dead {
            return Err(message.clone());
        }
        let started = Instant::now();
        let ensured = self.ensure();
        self.waited += started.elapsed();
        if let Err(failure) = ensured {
            // Latch on the permanent flavors, or once futile waiting has
            // consumed the lifetime budget (a daemon that never came up).
            if failure.latch || self.waited >= MAX_LIFETIME_WAIT {
                self.dead = Some(failure.message.clone());
            }
            return Err(failure.message);
        }
        self.waited = Duration::ZERO;
        let body = serde_json::to_string(&crate::riskgate::judge_request_body(command))
            .map_err(|e| format!("serializing the judge request: {e}"))?;
        let (status, resp) = uds_request(&self.sock, "POST", "/judge", Some(&body))?;
        if status != 200 {
            return Err(format!("judge daemon returned HTTP {status}"));
        }
        let value: Value = serde_json::from_str(&resp)
            .map_err(|e| format!("judge daemon response is not valid JSON: {e}"))?;
        crate::riskgate::parse_verdict(&value)
    }
}

/// The env-resolving ensure used by BOTH the per-run client (`DaemonJudge`)
/// and the one-shot CLI (`--ensure`): resolves the daemon binary (the
/// `CHUG_DELEGATE_BIN` seam, else this running chug) and runs
/// [`ensure_at`].
fn ensure_with(sock: &Path, spawned: &mut Option<Child>) -> Result<(), EnsureFailure> {
    match daemon_binary() {
        Ok(binary) => ensure_at(sock, spawned, &Some(binary)),
        Err(e) => Err(EnsureFailure {
            message: format!("resolving the chug binary for the judge daemon: {e:#}"),
            latch: true,
        }),
    }
}

/// The ensure core, parameterized over the caller's spawn slot and the
/// binary so both callers share it — and so tests need no process-global
/// env. A spawned child that is STILL RUNNING owns the load: it is never
/// spawned over, only waited on.
fn ensure_at(
    sock: &Path,
    spawned: &mut Option<Child>,
    binary: &Option<PathBuf>,
) -> Result<(), EnsureFailure> {
    if let Some(child) = spawned.as_mut() {
        // Opportunistic reap: a daemon we spawned that has since died would
        // otherwise sit in the process table as a zombie.
        let _ = child.try_wait();
    }
    if healthz_ok(sock).is_ok() {
        return Ok(());
    }
    // Our spawned daemon is still alive (loading the model): never stack a
    // second daemon on top of it — just keep waiting for its socket.
    if spawned
        .as_mut()
        .map(|child| child.try_wait().map(|status| status.is_none()))
        .transpose()
        .map_err(|e| EnsureFailure {
            message: format!("polling the spawned judge daemon: {e}"),
            latch: false,
        })?
        .unwrap_or(false)
    {
        let child = spawned.as_mut().expect("alive child checked above");
        return wait_for_socket(sock, child, SPAWN_WAIT_BUDGET);
    }
    // Stale socket recovery (spec req 4): connect -> ECONNREFUSED -> unlink
    // -> respawn, never a hard error. Only ECONNREFUSED (file present, no
    // listener) unlinks — ENOENT means there is nothing to clean.
    if connect_refused(sock) {
        let _ = std::fs::remove_file(sock);
    }
    let Some(binary) = binary else {
        return Err(EnsureFailure {
            message: "no chug binary available to spawn the judge daemon".into(),
            latch: true,
        });
    };
    let child =
        spawn_daemon(sock, binary).map_err(|e| EnsureFailure {
            message: format!("spawning the judge daemon: {e:#}"),
            latch: true,
        })?;
    *spawned = Some(child);
    wait_for_socket(sock, spawned.as_mut().expect("just stored"), SPAWN_WAIT_BUDGET)
}

/// `GET /healthz` -> 200 means a warm daemon (the model is loaded BEFORE the
/// socket binds, so healthz never lies about readiness).
fn healthz_ok(sock: &Path) -> Result<String, String> {
    let (status, body) = uds_request(sock, "GET", "/healthz", None)?;
    if status == 200 {
        Ok(body)
    } else {
        Err(format!("healthz returned HTTP {status}"))
    }
}

/// Did connect fail specifically with ECONNREFUSED (a stale socket file with
/// no listener behind it)?
fn connect_refused(sock: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::net::UnixStream;
        matches!(
            UnixStream::connect(sock),
            Err(e) if e.raw_os_error() == Some(libc::ECONNREFUSED)
        )
    }
    #[cfg(not(unix))]
    {
        let _ = sock;
        false
    }
}

/// Resolve the binary children spawn: the `CHUG_DELEGATE_BIN` test seam wins,
/// else the running chug itself — the one-binary/subcommand pattern
/// (delegate.rs is the reference).
fn daemon_binary() -> anyhow::Result<PathBuf> {
    match std::env::var_os(BINARY_ENV) {
        Some(bin) => Ok(PathBuf::from(bin)),
        None => std::env::current_exe().context("resolving the chug binary (current_exe)"),
    }
}

/// Spawn `chug daemon` (serve mode) detached — own process group, SIGHUP
/// ignored (`nohup … &` parity, the delegate spawn shape) — with the daemon's
/// stdout+stderr appending to `<chug home>/daemon.log`. Returns the child so
/// the caller can fast-fail the wait when it exits early; the handle is
/// never waited to completion here (detached by contract).
fn spawn_daemon(sock: &Path, binary: &Path) -> anyhow::Result<Child> {
    let log = log_path()?;
    if let Some(dir) = log.parent() {
        std::fs::create_dir_all(dir)
            .with_context(|| format!("creating {}", dir.display()))?;
    }
    let open_log = || {
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log)
    };
    let mut cmd = Command::new(binary);
    cmd.arg("daemon")
        .stdin(Stdio::null())
        .stdout(Stdio::from(open_log()?))
        .stderr(Stdio::from(open_log()?));
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
        // Ignored dispositions survive exec — the daemon ends up SIG_IGN-ing
        // SIGHUP without this process changing its own disposition.
        unsafe {
            cmd.pre_exec(|| {
                libc::signal(libc::SIGHUP, libc::SIG_IGN);
                Ok(())
            });
        }
    }
    let child = cmd
        .spawn()
        .with_context(|| format!("spawning the judge daemon: {}", binary.display()))?;
    eprintln!(
        "chug: judge daemon not running — spawned pid {} ({} daemon); waiting up to {}s for {} (log: {})",
        child.id(),
        binary.display(),
        SPAWN_WAIT_BUDGET.as_secs(),
        sock.display(),
        log.display()
    );
    Ok(child)
}

/// Bounded wait for socket + /healthz after a spawn. Fast-fails when the
/// spawned daemon exits and nothing else holds the lock (nothing will ever
/// bind — a permanent flavor, latched by the caller); keeps waiting when
/// another daemon holds the lock (it is still loading the model and will
/// bind).
fn wait_for_socket(sock: &Path, child: &mut Child, budget: Duration) -> Result<(), EnsureFailure> {
    let deadline = Instant::now() + budget;
    loop {
        if healthz_ok(sock).is_ok() {
            return Ok(());
        }
        if let Some(status) = child.try_wait().map_err(|e| EnsureFailure {
            message: format!("polling the spawned daemon: {e}"),
            latch: false,
        })? {
            let lock = lock_path().map_err(|e| EnsureFailure {
                message: e.to_string(),
                latch: false,
            })?;
            if daemon_lock_held(&lock) {
                // Another daemon (still loading) owns the lock and will bind.
                // `status` is this spawn's lock refusal — expected.
            } else {
                let _ = status;
                return Err(EnsureFailure {
                    message: format!(
                        "the spawned judge daemon exited before serving; see {}",
                        log_path().map(|p| p.display().to_string()).unwrap_or_default()
                    ),
                    latch: true,
                });
            }
        }
        if Instant::now() >= deadline {
            return Err(EnsureFailure {
                message: format!(
                    "gave up waiting for a healthy judge daemon after {}s",
                    budget.as_secs()
                ),
                latch: false,
            });
        }
        std::thread::sleep(PROBE_INTERVAL);
    }
}

/// True when some live daemon holds the flock (probe: try-acquire + release).
#[cfg(unix)]
fn daemon_lock_held(path: &Path) -> bool {
    use std::os::unix::io::AsRawFd;
    let Ok(file) = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
    else {
        return false;
    };
    // SAFETY: flock(2) on our own open fd — a non-blocking advisory lock probe.
    let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    rc != 0
}

#[cfg(not(unix))]
fn daemon_lock_held(_path: &Path) -> bool {
    false
}

// ---------------------------------------------------------------------------
// Lifecycle: the single-instance lock (driver-lock pattern, host-scoped)
// ---------------------------------------------------------------------------

/// Acquire the daemon lock (flock LOCK_EX|LOCK_NB on `<chug home>/daemon.lock`).
/// `Ok(file)` — this process holds it until the file is dropped (kernel
/// releases on death, so a SIGKILL'd daemon needs no reclaim dance; the pid
/// line is informational for --stop/--status). `Err` — a live daemon holds
/// it: the caller refuses to start.
pub fn acquire_daemon_lock() -> anyhow::Result<std::fs::File> {
    let path = lock_path()?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    acquire_lock_at(&path).map_err(|e| anyhow::anyhow!("refusing to start the judge daemon: {e}"))
}

/// The lock core, parameterized over the path (unit tests use a temp home).
#[cfg(unix)]
pub fn acquire_lock_at(path: &Path) -> Result<std::fs::File, String> {
    use std::os::unix::io::AsRawFd;

    let file = std::fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(path)
        .map_err(|e| format!("opening the daemon lock: {e}"))?;
    // SAFETY: flock(2) on our own open fd — the driver-lock pattern's
    // single-instance gate, LOCK_NB so a second daemon refuses instead of
    // parking.
    let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if rc != 0 {
        let err = std::io::Error::last_os_error();
        let holder = std::fs::read_to_string(path)
            .ok()
            .and_then(|text| text.lines().next().map(str::to_string))
            .unwrap_or_else(|| "another daemon".into());
        return Err(format!("the daemon lock is held by {holder} ({err})"));
    }
    // Record OUR pid (the informational line --stop/--status read).
    let me = std::process::id().to_string();
    file.set_len(0)
        .and_then(|_| {
            let mut f = &file;
            f.write_all(me.as_bytes())
        })
        .and_then(|_| (&file).flush())
        .map_err(|e| format!("writing the daemon lock pid: {e}"))?;
    Ok(file)
}

#[cfg(not(unix))]
pub fn acquire_lock_at(_path: &Path) -> Result<std::fs::File, String> {
    Err("unix domain sockets and flock are not supported on this platform".into())
}

/// The pid named by the lock file's first line, if parseable.
fn lock_holder_pid(path: &Path) -> Option<u32> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| text.lines().next()?.trim().parse().ok())
}

/// `chug daemon --stop`: TERM the lock holder (identity-checked: alive AND
/// argv names chug — never signal an unresolved identity), bounded wait with
/// one SIGKILL escalation, then remove the socket file. Idempotent.
pub fn stop() -> anyhow::Result<i32> {
    let sock = sock_path()?;
    let pid = lock_holder_pid(&lock_path()?);
    let holder = pid.filter(|p| crate::driver_lock::pid_alive(*p) && crate::driver_lock::argv_names_chug(*p));
    match holder {
        Some(pid) => {
            terminate_daemon(pid);
            let removed = cleanup_socket(&sock);
            println!(
                "chug daemon: stopped pid {pid}{}",
                if removed { " (removed the socket)" } else { "" }
            );
            Ok(0)
        }
        None => {
            let removed = cleanup_socket(&sock);
            println!(
                "chug daemon: not running{}",
                if removed { " (removed a stale socket)" } else { "" }
            );
            Ok(0)
        }
    }
}

/// SIGTERM the pid, bounded wait, one SIGKILL escalation — the chug_cancel
/// discipline. Never blocks past ~12s.
#[cfg(unix)]
fn terminate_daemon(pid: u32) {
    // SAFETY: kill(2) with SIGTERM/SIGKILL on an identity-checked pid.
    unsafe {
        let _ = libc::kill(pid as i32, libc::SIGTERM);
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    while crate::driver_lock::pid_alive(pid) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(100));
    }
    if crate::driver_lock::pid_alive(pid) {
        unsafe {
            let _ = libc::kill(pid as i32, libc::SIGKILL);
        }
        let hard = Instant::now() + Duration::from_secs(2);
        while crate::driver_lock::pid_alive(pid) && Instant::now() < hard {
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

#[cfg(not(unix))]
fn terminate_daemon(_pid: u32) {}

/// Remove the socket file (also valid on a bound-but-orphaned socket inode).
/// Returns whether something was removed.
fn cleanup_socket(sock: &Path) -> bool {
    std::fs::remove_file(sock).is_ok()
}

/// `chug daemon --status`: running (exit 0) / starting or absent (exit 1).
pub fn status() -> anyhow::Result<i32> {
    let sock = sock_path()?;
    let pid = lock_holder_pid(&lock_path()?);
    let alive = pid.filter(|p| crate::driver_lock::pid_alive(*p));
    match healthz_ok(&sock) {
        Ok(body) => {
            println!(
                "chug daemon: running (sock {}, pid {}, {body})",
                sock.display(),
                alive.map(|p| p.to_string()).unwrap_or_else(|| "?".into())
            );
            Ok(0)
        }
        Err(_) => match alive {
            Some(pid) => {
                println!(
                    "chug daemon: starting (pid {pid} holds {} but the socket is not answering yet)",
                    lock_path()?.display()
                );
                Ok(1)
            }
            None => {
                println!("chug daemon: not running (sock {})", sock.display());
                Ok(1)
            }
        },
    }
}

/// `chug daemon --ensure`: spawn + bounded wait, exit 0 only on a healthy
/// daemon. The loopd cycle-start step and the tests use this; it is
/// best-effort by contract (a nonzero exit never blocks the caller).
pub fn ensure_cmd() -> anyhow::Result<i32> {
    let sock = sock_path()?;
    let mut spawned = None;
    match ensure_with(&sock, &mut spawned) {
        Ok(()) => {
            println!("chug daemon: healthy at {}", sock.display());
            Ok(0)
        }
        Err(e) => {
            eprintln!("chug: judge daemon ensure failed: {}", e.message);
            Ok(1)
        }
    }
}

// ---------------------------------------------------------------------------
// The server: the inference seam + the UDS HTTP loop
// ---------------------------------------------------------------------------

/// The inference seam behind the socket: `JudgeModel` under the `daemon`
/// feature; the lifecycle stub and the protocol-contract fixtures in tests.
pub trait JudgeBackend: Send + Sync {
    /// The full /judge body parse + forward — the layad-compatible response
    /// payload, or an error (routed to 400 when the request itself is
    /// malformed, 500 otherwise).
    fn judge_request(&self, raw: &str) -> anyhow::Result<Value>;
    /// One line for /healthz's `model` field.
    fn describe(&self) -> String;
}

/// The lifecycle test seam's backend: real transport, lock, socket — NO
/// inference. /judge refuses outright: a stub must never fabricate
/// classifications (SPEC-3).
struct StubBackend;

impl JudgeBackend for StubBackend {
    fn judge_request(&self, _raw: &str) -> anyhow::Result<Value> {
        bail!("CHUG_DAEMON_STUB test backend: /judge is never served by the stub")
    }
    fn describe(&self) -> String {
        "stub (CHUG_DAEMON_STUB=1 — /healthz only)".into()
    }
}

/// `chug daemon` (serve mode): take the single-instance lock, load the model
/// BEFORE the socket binds (healthz == warm), serve until killed.
pub fn serve() -> anyhow::Result<i32> {
    let _lock = acquire_daemon_lock()?;
    let backend: std::sync::Arc<dyn JudgeBackend> = if stub_requested() {
        std::sync::Arc::new(StubBackend)
    } else {
        real_backend()?
    };
    serve_unix(&sock_path()?, backend)
}

/// The `CHUG_DAEMON_STUB=1` test seam switch.
fn stub_requested() -> bool {
    std::env::var(STUB_ENV).map(|v| v.trim() == "1").unwrap_or(false)
}

/// The real inference backend: CHILD A's `JudgeModel` (feature-gated).
#[cfg(feature = "daemon")]
fn real_backend() -> anyhow::Result<std::sync::Arc<dyn JudgeBackend>> {
    let spec = crate::judge_model::CheckpointSpec::resolve();
    eprintln!(
        "chug daemon: loading judge checkpoint {spec:?} — the first load may download ~650 MB into the HF cache"
    );
    let model = crate::judge_model::JudgeModel::load(&spec)?;
    eprintln!("chug daemon: judge model warm (cpu)");
    Ok(std::sync::Arc::new(model))
}

/// Feature-off builds refuse with a clear message (spec req 1); clients fail
/// open exactly as they do for an unreachable layad.
#[cfg(not(feature = "daemon"))]
fn real_backend() -> anyhow::Result<std::sync::Arc<dyn JudgeBackend>> {
    bail!(
        "this chug binary was built without the judge daemon (the default): rebuild with \
         `cargo build --features daemon` to host the Laya judge; risk-gate clients fail open \
         meanwhile"
    )
}

#[cfg(feature = "daemon")]
impl JudgeBackend for crate::judge_model::JudgeModel {
    fn judge_request(&self, raw: &str) -> anyhow::Result<Value> {
        use crate::judge_model::JudgeModel;
        JudgeModel::judge_request(self, raw)
    }
    fn describe(&self) -> String {
        match self.spec() {
            crate::judge_model::CheckpointSpec::Dir(path) => {
                format!("rl-agent (local {})", path.display())
            }
            crate::judge_model::CheckpointSpec::Hub(repo) => format!("rl-agent (hub {repo})"),
        }
    }
}

/// Bind the socket at 0600 and serve until killed. One thread per connection
/// (a slow client must never starve the healthz probes).
pub fn serve_unix(sock: &Path, backend: std::sync::Arc<dyn JudgeBackend>) -> anyhow::Result<i32> {
    #[cfg(unix)]
    {
        let listener = bind_socket(sock).with_context(|| format!("binding {}", sock.display()))?;
        eprintln!(
            "chug daemon: serving /healthz + /judge on {} (mode 0600) — {}",
            sock.display(),
            backend.describe()
        );
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let backend = backend.clone();
                    if let Err(e) = std::thread::Builder::new()
                        .name("chug-daemon-conn".into())
                        .spawn(move || handle_conn(backend, stream))
                    {
                        eprintln!("chug daemon: could not spawn a connection thread: {e}");
                    }
                }
                Err(e) => eprintln!("chug daemon: accept failed: {e}"),
            }
        }
        Ok(0)
    }
    #[cfg(not(unix))]
    {
        let _ = (sock, backend);
        bail!("unix domain sockets are not supported on this platform")
    }
}

/// Bind the socket: remove a stale file first (we hold the lock, so any
/// existing socket file is garbage from a SIGKILL'd daemon), create under a
/// private umask (no group/world-visible window), pin the exact 0600 mode.
#[cfg(unix)]
fn bind_socket(sock: &Path) -> anyhow::Result<std::os::unix::net::UnixListener> {
    use std::os::unix::net::UnixListener;

    if let Some(dir) = sock.parent().filter(|dir| !dir.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    let _ = std::fs::remove_file(sock);
    // SAFETY: umask(2) — set a private mask around bind so the socket file is
    // never group/world readable even between bind and the chmod pin.
    let old = unsafe { libc::umask(0o077) };
    let bind_result = UnixListener::bind(sock);
    unsafe {
        libc::umask(old);
    }
    let listener = bind_result.with_context(|| format!("binding {}", sock.display()))?;
    set_file_mode(sock, 0o600)?;
    Ok(listener)
}

/// chmod a path to exactly `mode` (0600 pins the spec's socket policy).
#[cfg(unix)]
fn set_file_mode(path: &Path, mode: u32) -> anyhow::Result<()> {
    use std::os::unix::ffi::OsStrExt;

    let c = std::ffi::CString::new(path.as_os_str().as_bytes())
        .map_err(|_| anyhow::anyhow!("socket path has an interior NUL: {}", path.display()))?;
    // SAFETY: chmod(2) on a NUL-terminated path we just built.
    let rc = unsafe { libc::chmod(c.as_ptr(), mode as libc::mode_t) };
    if rc != 0 {
        bail!(
            "chmod {mode:o} failed on {}: {}",
            path.display(),
            std::io::Error::last_os_error()
        );
    }
    Ok(())
}

#[cfg(unix)]
fn handle_conn(backend: std::sync::Arc<dyn JudgeBackend>, mut stream: std::os::unix::net::UnixStream) {
    let _ = stream.set_read_timeout(Some(JUDGE_TIMEOUT));
    let _ = stream.set_write_timeout(Some(JUDGE_TIMEOUT));
    let (status, body) = match read_request(&mut stream) {
        Ok(request) => route(&*backend, &request),
        Err(message) => (400, detail(message)),
    };
    if let Err(e) = write_response(&mut stream, status, &body) {
        // A client that vanished mid-response is routine — never fatal.
        eprintln!("chug daemon: response write failed: {e}");
    }
}

struct HttpRequest {
    method: String,
    path: String,
    body: String,
}

/// Read one HTTP/1.1 request: headers bounded by a double-CRLF scan, then
/// exactly Content-Length body bytes (the client never half-closes its write
/// side, so EOF-based framing is not available).
fn read_request<R: Read>(stream: &mut R) -> Result<HttpRequest, String> {
    let mut buf: Vec<u8> = Vec::with_capacity(1024);
    let mut chunk = [0u8; 4096];
    let header_end = loop {
        if let Some(pos) = buf.windows(4).position(|window| window == b"\r\n\r\n") {
            break pos;
        }
        if buf.len() > MAX_HEADER_BYTES {
            return Err("request headers exceed the 64 KiB cap".into());
        }
        let n = stream.read(&mut chunk).map_err(|e| format!("reading request: {e}"))?;
        if n == 0 {
            return Err("client closed before sending a full request".into());
        }
        buf.extend_from_slice(&chunk[..n]);
    };
    let head = String::from_utf8_lossy(&buf[..header_end]).into_owned();
    let mut parts = head.split("\r\n").next().unwrap_or_default().split_whitespace();
    let method = parts.next().ok_or("malformed request line")?.to_string();
    let path = parts.next().ok_or("malformed request line")?.to_string();
    let content_length: usize = head
        .to_ascii_lowercase()
        .split("\r\n")
        .find_map(|line| line.strip_prefix("content-length:"))
        .map(|value| value.trim().parse::<usize>())
        .transpose()
        .map_err(|_| "malformed Content-Length".to_string())?
        .unwrap_or(0);
    if content_length > MAX_BODY_BYTES {
        return Err("request body exceeds the 1 MiB cap".into());
    }
    let mut body_bytes = buf[header_end + 4..].to_vec();
    while body_bytes.len() < content_length {
        let n = stream
            .read(&mut chunk)
            .map_err(|e| format!("reading request body: {e}"))?;
        if n == 0 {
            return Err("client closed before sending the full body".into());
        }
        body_bytes.extend_from_slice(&chunk[..n]);
    }
    body_bytes.truncate(content_length);
    let body = String::from_utf8(body_bytes).map_err(|_| "request body is not valid UTF-8".to_string())?;
    Ok(HttpRequest { method, path, body })
}

/// The dispatch: GET /healthz (liveness, warm by construction), POST /judge
/// (the layad drop-in), everything else 404.
fn route(backend: &dyn JudgeBackend, request: &HttpRequest) -> (u16, String) {
    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/healthz") => (
            200,
            json!({"status": "ok", "model": backend.describe()}).to_string(),
        ),
        ("POST", "/judge") => {
            let shape_error = request_shape_error(&request.body);
            match backend.judge_request(&request.body) {
                Ok(payload) => (200, payload.to_string()),
                Err(e) => {
                    // Request-shape problems are client errors; anything past
                    // parsing/shape is a daemon fault. Both carry the layad
                    // error shape `{"detail": …}`; the client fails open on
                    // any non-success either way.
                    let status = if shape_error.is_some() { 400 } else { 500 };
                    (status, detail(format!("{e:#}")))
                }
            }
        }
        _ => (404, detail("not found")),
    }
}

/// The layad error shape: a JSON object with a string `detail`.
fn detail(message: impl std::fmt::Display) -> String {
    json!({"detail": message.to_string()}).to_string()
}

/// 400-vs-500 classification: the same pure checks the model's request parse
/// performs (ordered JSON parse + the two required fields). Runs BEFORE the
/// inference call so the status reflects the request, not the backend.
fn request_shape_error(body: &str) -> Option<String> {
    let parsed = match crate::judge_pack::parse_ordered(body) {
        Ok(value) => value,
        Err(e) => return Some(format!("invalid request JSON: {e}")),
    };
    for key in ["state", "questions"] {
        if field(&parsed, key).is_none() {
            return Some(format!("request missing \"{key}\""));
        }
    }
    None
}

/// Ordered-object field lookup (judge_pack's OValue keeps insertion order).
fn field<'a>(value: &'a OValue, key: &str) -> Option<&'a OValue> {
    match value {
        OValue::Obj(entries) => entries.iter().find_map(|(k, v)| (k == key).then_some(v)),
        _ => None,
    }
}

/// One HTTP/1.1 response with `Connection: close` framing (the client reads
/// to EOF, so close IS the terminator).
fn write_response<W: Write>(stream: &mut W, status: u16, body: &str) -> std::io::Result<()> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "Error",
    };
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(body.as_bytes())?;
    stream.flush()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::Arc;

    /// The committed `riskgate_base` fixture — the risk-gate request/response
    /// pair the Python SDK recorded (CHILD A's goldens). Both the contract
    /// test and the live socket test read it, so the daemon pins EXACTLY the
    /// shapes riskgate.rs consumes.
    fn fixture(name: &str) -> Value {
        let path = std::path::Path::new("tests/fixtures/laya/golden-vectors.json");
        let text = std::fs::read_to_string(path).expect("reading golden-vectors.json");
        let all: Value = serde_json::from_str(&text).expect("parsing golden-vectors.json");
        all["fixtures"]
            .as_array()
            .expect("fixtures array")
            .iter()
            .find(|fx| fx["name"] == json!(name))
            .unwrap_or_else(|| panic!("fixture {name} missing from golden-vectors.json"))
            .clone()
    }

    /// A backend that answers /judge with a canned payload — the protocol
    /// contract test's server half (no weights, no network).
    struct FixtureBackend {
        response: Value,
    }

    impl JudgeBackend for FixtureBackend {
        fn judge_request(&self, raw: &str) -> anyhow::Result<Value> {
            // The real backend's front door: parse + shape-check the request
            // body BEFORE serving — a malformed or incomplete body is a
            // backend error, which is what route() classifies as 400 (the
            // contract under test). Same pure checks the model parse runs.
            if let Some(error) = request_shape_error(raw) {
                return Err(anyhow::anyhow!(error));
            }
            Ok(self.response.clone())
        }
        fn describe(&self) -> String {
            "fixture (protocol contract test)".into()
        }
    }

    /// Spin serve_unix on a fresh temp socket and wait for healthz.
    fn spawn_fixture_server(response: Value) -> (tempfile::TempDir, PathBuf) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let sock = tmp.path().join("contract.sock");
        let backend: Arc<dyn JudgeBackend> = Arc::new(FixtureBackend { response });
        let server_sock = sock.clone();
        std::thread::spawn(move || {
            let _ = serve_unix(&server_sock, backend);
        });
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            if healthz_ok(&sock).is_ok() {
                return (tmp, sock);
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        panic!("the fixture server never answered /healthz");
    }

    /// Structural equality with a float tolerance — the same contract the
    /// judge_model live test asserts, applied to socket responses.
    fn assert_value_close(got: &Value, want: &Value, tol: f64, name: &str) {
        match (got, want) {
            (Value::Object(g), Value::Object(w)) => {
                assert_eq!(g.len(), w.len(), "{name}: object field count");
                for (k, wv) in w {
                    let gv = g.get(k).unwrap_or_else(|| panic!("{name}: missing field {k}"));
                    assert_value_close(gv, wv, tol, name);
                }
            }
            (Value::Array(g), Value::Array(w)) => {
                assert_eq!(g.len(), w.len(), "{name}: array length");
                for (gv, wv) in g.iter().zip(w.iter()) {
                    assert_value_close(gv, wv, tol, name);
                }
            }
            (Value::Number(g), Value::Number(w)) => {
                let (g, w) = (g.as_f64().expect("f64"), w.as_f64().expect("f64"));
                assert!((g - w).abs() <= tol, "{name}: numeric drift {g} vs {w} (tol {tol})");
            }
            _ => assert_eq!(got, want, "{name}: exact field mismatch"),
        }
    }

    /// The protocol drop-in contract (spec Tests): fixture /judge request ->
    /// response shape matches layad's (fields, types, error shape), pinned
    /// over the REAL transport (UDS + hand-rolled HTTP/1.1) in both build
    /// configurations. The model-backed payload itself is CHILD A's live
    /// parity; this pins the WIRE.
    #[test]
    fn judge_contract_over_uds_matches_layad_shape() {
        let fx = fixture("riskgate_base");
        let (_tmp, sock) = spawn_fixture_server(fx["expected"].clone());

        // /healthz: 200 + parseable JSON with the two fields.
        let (status, body) = uds_request(&sock, "GET", "/healthz", None).expect("healthz");
        assert_eq!(status, 200, "healthz status");
        let health: Value = serde_json::from_str(&body).expect("healthz JSON");
        assert_eq!(health["status"], json!("ok"));
        assert!(health["model"].is_string(), "healthz model field is a string");

        // /judge: the fixture request (state + questions) -> the layad
        // response shape: {"model": str, "answers": {…}, "usage": {…}}.
        let request = json!({"state": fx["state"], "questions": fx["questions"]}).to_string();
        let (status, body) =
            uds_request(&sock, "POST", "/judge", Some(&request)).expect("judge");
        assert_eq!(status, 200, "judge status");
        let value: Value = serde_json::from_str(&body).expect("judge JSON");
        assert!(value["model"].is_string(), "model field is a string");
        assert!(value["answers"].is_object(), "answers field is an object");
        assert!(value["usage"]["input_tokens"].is_number(), "usage.input_tokens is a number");
        assert!(value["usage"]["output_tokens"].is_number(), "usage.output_tokens is a number");
        // The risk verdict the gate parses is typed exactly as parse_verdict
        // needs: answers.risk.choice string + probabilities.destructive number.
        assert!(value["answers"]["risk"]["choice"].is_string());
        assert!(value["answers"]["risk"]["probabilities"]["destructive"].is_number());
        // And byte-for-byte the recorded SDK response (within 1e-3).
        assert_value_close(&value, &fx["expected"], 1e-3, "riskgate_base");

        // Error shape: malformed JSON -> 400 + {"detail": str}.
        let (status, body) = uds_request(&sock, "POST", "/judge", Some("{not json")).expect("bad json");
        assert_eq!(status, 400, "malformed request -> 400");
        let err: Value = serde_json::from_str(&body).expect("error JSON");
        assert!(err["detail"].is_string(), "error shape is {{\"detail\": string}}");

        // Error shape: missing questions -> 400 + {"detail": str}.
        let (status, body) =
            uds_request(&sock, "POST", "/judge", Some("{\"state\": {}}")).expect("missing field");
        assert_eq!(status, 400, "missing questions -> 400");
        let err: Value = serde_json::from_str(&body).expect("error JSON");
        assert!(err["detail"].as_str().expect("detail string").contains("questions"));

        // Unknown path -> 404 + {"detail": str}; wrong method -> 404.
        let (status, _) = uds_request(&sock, "GET", "/nope", None).expect("404 path");
        assert_eq!(status, 404);
        let (status, _) = uds_request(&sock, "GET", "/judge", None).expect("404 method");
        assert_eq!(status, 404);
    }

    /// The socket file is mode 0600 (spec Tests) — filesystem perms are the
    /// phase-1 policy gate (no port, no network listener).
    #[cfg(unix)]
    #[test]
    fn socket_file_mode_is_0600() {
        use std::os::unix::fs::PermissionsExt;

        let (tmp, sock) = spawn_fixture_server(json!({"model": "rl-agent"}));
        let mode = std::fs::metadata(&sock)
            .expect("stat the socket")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600, "the daemon socket must be mode 0600");
        drop(tmp);
    }

    /// The daemon lock is exclusive while held and re-acquirable after
    /// release (flock semantics — two `chug daemon` processes cannot both
    /// serve).
    #[cfg(unix)]
    #[test]
    fn daemon_lock_is_exclusive() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let lock = tmp.path().join(LOCK_FILE);
        let first = acquire_lock_at(&lock).expect("first acquisition");
        let second = acquire_lock_at(&lock);
        let err = second.expect_err("the second acquisition must refuse");
        assert!(err.contains("held by"), "the refusal names the holder: {err}");
        assert!(
            std::fs::read_to_string(&lock).expect("lock contents").contains(&std::process::id().to_string()),
            "the lock records the holder pid"
        );
        drop(first);
        // After release (the lock-holding daemon exiting), acquisition works.
        let third = acquire_lock_at(&lock).expect("acquisition after release");
        drop(third);
    }

    /// The client fails cleanly (message, no panic) when no daemon/socket
    /// exists — the fail-open path the risk gate rides.
    #[test]
    fn uds_request_fails_cleanly_without_a_server() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let sock = tmp.path().join("missing.sock");
        let err = uds_request(&sock, "GET", "/healthz", None).expect_err("no server");
        assert!(!err.is_empty());
    }

    /// Stale-socket recovery at the transport level: a socket file with no
    /// listener connects ECONNREFUSED, which is the unlink trigger.
    #[cfg(unix)]
    #[test]
    fn stale_socket_connects_refused() {
        use std::os::unix::net::UnixListener;

        let tmp = tempfile::tempdir().expect("tempdir");
        let sock = tmp.path().join("stale.sock");
        // Create the socket file, then drop the listener (SIGKILL'd-daemon
        // shape: file present, no listener).
        drop(UnixListener::bind(&sock).expect("bind"));
        assert!(sock.exists(), "the stale socket file exists");
        assert!(connect_refused(&sock), "connect to the dead socket is ECONNREFUSED");
        assert!(uds_request(&sock, "GET", "/healthz", None).is_err());
    }

    /// The permanent-failure latch: an ensure that cannot even spawn (no
    /// binary) reports latch=true, and a latched client answers every judge
    /// call with the SAME recorded failure instantly — no re-spawn per bash
    /// command. (The no-spawn shape is exactly the feature-off refusal's
    /// client view; exercised here with binary=None, no global env.)
    #[test]
    fn daemon_client_latches_after_a_permanent_ensure_failure() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let sock = tmp.path().join("missing.sock");
        let mut spawned = None;
        let failure = ensure_at(&sock, &mut spawned, &None).expect_err("no binary, no server");
        assert!(failure.latch, "a failed spawn is permanent for this process");
        assert!(spawned.is_none(), "no child was ever created");

        // The latched client: judge() returns the recorded failure without
        // touching the socket again (dead short-circuits before ensure).
        let mut judge = DaemonJudge {
            sock,
            spawned: None,
            dead: Some("latched for the test".into()),
            waited: Duration::ZERO,
        };
        let err = judge.judge("rm -rf /").expect_err("latched");
        assert_eq!(err, "latched for the test");
    }

    /// The gate-level fail-open contract with the REAL daemon client: a
    /// client that cannot reach a daemon is an Err judge, so the gate allows
    /// the command and logs the failure — never blocks, never panics (the
    /// exact degrade shape the HTTP path has today).
    #[test]
    fn risk_gate_fails_open_with_a_dead_daemon_client() {
        use crate::events::{Event, EventSink};
        use crate::riskgate::RiskGate;

        let tmp = tempfile::tempdir().expect("tempdir");
        struct Recording(Vec<Event>);
        impl EventSink for Recording {
            fn emit(&mut self, e: Event) {
                self.0.push(e);
            }
        }
        let mut judge = DaemonJudge {
            sock: tmp.path().join("absent.sock"),
            spawned: None,
            dead: Some("the judge daemon is unreachable (latched)".into()),
            waited: Duration::ZERO,
        };
        let mut gate = RiskGate::new(Box::new(judge), tmp.path());
        let mut sink = Recording(Vec::new());
        assert!(
            matches!(gate.check("rm -rf build/", &mut sink), crate::riskgate::GateDecision::Allowed),
            "a dead judge must fail OPEN"
        );
        assert!(sink.0.is_empty(), "no RiskVerdict event for a failed judgment");
        let log = std::fs::read_to_string(tmp.path().join(".chug/risk_verdicts.jsonl"))
            .expect("the verdict log exists");
        let entry: Value = serde_json::from_str(log.lines().last().expect("one entry"))
            .expect("log line parses");
        assert_eq!(entry["choice"], "gate_failure");
        assert_eq!(entry["blocked"], false);
        assert_eq!(entry["gate_failure"], "the judge daemon is unreachable (latched)");
    }

    /// `request_shape_error` classifies exactly the legs the route() status
    /// codes pin: malformed JSON and missing fields are client errors; a
    /// well-shaped request passes the check (its failure becomes a 500).
    #[test]
    fn request_shape_classification() {
        assert!(request_shape_error("{not json").is_some());
        assert!(request_shape_error("{}").is_some());
        assert!(request_shape_error("{\"state\": {}}").is_some());
        assert!(request_shape_error("{\"questions\": {}}").is_some());
        assert!(request_shape_error("{\"state\": {}, \"questions\": {}}").is_none());
        // Duplicate keys keep Python dict semantics (first position, last
        // value) and still parse.
        assert!(request_shape_error("{\"state\": 1, \"state\": {}, \"questions\": {}}").is_none());
    }

    // ------------------------------------------------------------------
    // Live: CHUG_LAYA_LIVE_PARITY=1 — the committed goldens through the
    // FULL daemon path (model -> serve_unix -> UDS HTTP -> client).
    // Default runs never load weights.
    // ------------------------------------------------------------------
    #[cfg(feature = "daemon")]
    #[test]
    fn live_judge_over_socket_matches_golden() {
        if std::env::var(crate::judge_model::LIVE_PARITY_ENV).ok().as_deref() != Some("1") {
            return;
        }
        let model = crate::judge_model::JudgeModel::load_default()
            .expect("loading the default checkpoint");
        let fx = fixture("riskgate_base");
        let backend: Arc<dyn JudgeBackend> = Arc::new(model);
        let tmp = tempfile::tempdir().expect("tempdir");
        let sock = tmp.path().join("live.sock");
        let server_sock = sock.clone();
        std::thread::spawn(move || {
            let _ = serve_unix(&server_sock, backend);
        });
        let deadline = Instant::now() + Duration::from_secs(30);
        while Instant::now() < deadline && healthz_ok(&sock).is_err() {
            std::thread::sleep(Duration::from_millis(50));
        }
        healthz_ok(&sock).expect("the live daemon never became healthy");

        let request = json!({"state": fx["state"], "questions": fx["questions"]}).to_string();
        let started = Instant::now();
        let (status, body) =
            uds_request(&sock, "POST", "/judge", Some(&request)).expect("live judge");
        let elapsed = started.elapsed();
        assert_eq!(status, 200, "live judge status");
        let value: Value = serde_json::from_str(&body).expect("live judge JSON");
        assert_value_close(&value, &fx["expected"], 1e-3, "riskgate_base over the socket");
        // The judge latency budget (spec req 5): warm ~50ms-class on MPS,
        // bounded CPU forward here — never the seconds-scale stall.
        assert!(
            elapsed < Duration::from_secs(2),
            "the warm judge took {elapsed:?} — over the fail-fast budget"
        );
    }
}
