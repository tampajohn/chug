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
/// `CHUG_DAEMON_SESSIONS=1` — T219's weightless registry host: serve mode
/// with the REAL transport, lock, socket, and /sessions registry but NO
/// judge model; /judge refuses exactly as the stub's does (a weightless
/// host must never fabricate classifications — SPEC-3). This is the seam
/// the /sessions wire pins run against (the default build has no model and
/// the lifecycle stub must never fabricate a registry), and an ops shape in
/// its own right: a daemon-capable binary can host the host-scoped session
/// registry without pulling ~650 MB of weights onto the box.
pub const SESSIONS_ENV: &str = "CHUG_DAEMON_SESSIONS";

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
    /// The daemon binary to spawn, resolved ONCE at construction (the one
    /// env read — judge() never re-reads process-global env). `None` is the
    /// no-binary seam: ensure fails PERMANENTLY on the spot (production
    /// never stores None with a clean latch — see [`DaemonJudge::from_env`]).
    binary: Option<PathBuf>,
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
        // The daemon binary is resolved ONCE, here — judge() never re-reads
        // process-global env (the mcp_serve.rs env rule). A resolution
        // failure is NOT a construction error (the gate must keep failing
        // open per command): it becomes the client's permanent-failure
        // latch — byte-for-byte the message and latch semantics a per-call
        // resolution failure produced before.
        let (binary, pre_latch) = match daemon_binary() {
            Ok(path) => (Some(path), None),
            Err(e) => (
                None,
                Some(format!("resolving the chug binary for the judge daemon: {e:#}")),
            ),
        };
        Ok(Self {
            sock: sock_path()?,
            binary,
            spawned: None,
            dead: pre_latch,
            waited: Duration::ZERO,
        })
    }

    /// Bring up a healthy daemon: probe, recover a stale socket, spawn,
    /// bounded wait. Errors carry a latch flag (see [`EnsureFailure`]).
    fn ensure(&mut self) -> Result<(), EnsureFailure> {
        ensure_at(&self.sock, &mut self.spawned, &self.binary)
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
            // The latch decision is the pure [`latch_reason`] (table-tested):
            // permanent flavors, or futile waiting past the lifetime budget,
            // latch the client off; transient flavors never do.
            if let Some(message) = latch_reason(&failure, self.waited) {
                self.dead = Some(message);
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

/// The PURE latch decision behind [`DaemonJudge::judge`] — a
/// permanent ensure failure, or cumulative futile waiting that has consumed
/// [`MAX_LIFETIME_WAIT`], latches the client off for the process lifetime
/// (`Some(message)`: every later judge call replays it instantly instead of
/// re-spawning per bash command). Any transient flavor below the lifetime
/// budget never latches: the next call may well succeed.
fn latch_reason(failure: &EnsureFailure, waited: Duration) -> Option<String> {
    if failure.latch || waited >= MAX_LIFETIME_WAIT {
        Some(failure.message.clone())
    } else {
        None
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
// T219 — the /sessions registry: chug runs register TTL heartbeats on the
// same host-scoped 0600 socket (`POST /sessions` upserts one run,
// `GET /sessions` lists the live ones). The store is in-memory and host-
// scoped like the daemon itself: it lives and dies with the daemon process
// (historical archives are out of scope), every store error fails open
// (a registry problem can never fail /judge — the routes share nothing),
// and emitters treat a failed POST as a no-op.
// ---------------------------------------------------------------------------

/// TTL for a session entry: no heartbeat (POST) within this many seconds
/// evicts it, lazily on read. ONE named const (spec req 2) — the default is
/// 10 minutes.
pub const SESSION_TTL_SECS: u64 = 600;

/// The four emitter roles a registration may carry (spec req 1). `dashd`
/// lives here even though that supervisor is not this repo's code: the
/// registry is the host-wide answer, and the dogfood loop registers through
/// the same one-line wire (the shell POST).
pub const SESSION_ROLES: [&str; 4] = ["loopd-cycle", "delegate-child", "dashd", "daemon"];

/// One registered run. The posted strings are echoed VERBATIM (the client's
/// words are the record); the parsed epoch copies drive the derived math so
/// a client that sends a format we cannot parse still registers (fail-open
/// to server-now) without corrupting the echoed record.
#[derive(Debug, Clone)]
struct SessionEntry {
    id: String,
    role: String,
    /// Echoed verbatim. On upsert this STICKS to the first registration: a
    /// heartbeat is the same shape as a registration, and the session's age
    /// must track the process, not the last heartbeat (the emitters are
    /// stateless on purpose — they do not re-send their start time).
    started: String,
    /// Parsed `started` (None: the posted string was not a timestamp we
    /// could read — age renders as 0 rather than inventing one).
    started_epoch: Option<u64>,
    /// Echoed verbatim (server-stamped when the POST omits it).
    last_event_ts: String,
    /// The TTL clock: the parsed `last_event_ts`, or the server's now when
    /// absent or unparseable — fail-open FRESH (an emitter with a broken
    /// clock still registers; it never registers as instantly stale).
    last_event_epoch: u64,
    status: String,
}

/// The store: upsert-by-id, lazy TTL eviction on read. Not `pub`-shaped API —
/// reached only through the routes and the unit tests.
#[derive(Default)]
pub struct SessionRegistry {
    entries: std::collections::HashMap<String, SessionEntry>,
}

impl SessionRegistry {
    /// Insert or refresh one entry by id. A heartbeat is the same shape as a
    /// first registration (spec req 1): every field takes the POST's value
    /// EXCEPT `started`, which sticks to the first registration (see the
    /// field doc).
    fn upsert(&mut self, entry: SessionEntry) {
        match self.entries.get_mut(&entry.id) {
            Some(existing) => {
                let started = existing.started.clone();
                let started_epoch = existing.started_epoch;
                *existing = entry;
                existing.started = started;
                existing.started_epoch = started_epoch;
            }
            None => {
                self.entries.insert(entry.id.clone(), entry);
            }
        }
    }

    /// Lazy TTL eviction on read (spec req 2): drop every entry whose last
    /// heartbeat is older than [`SESSION_TTL_SECS`], return the live ones
    /// sorted by id (deterministic — identical state renders byte-identical,
    /// the house render rule). A future-dated heartbeat (client clock ahead)
    /// stays live: saturating subtraction pins the age at 0, never negative.
    fn evict_live(&mut self, now: u64) -> Vec<SessionEntry> {
        self.entries
            .retain(|_, entry| now.saturating_sub(entry.last_event_epoch) <= SESSION_TTL_SECS);
        let mut live: Vec<SessionEntry> = self.entries.values().cloned().collect();
        live.sort_by(|a, b| a.id.cmp(&b.id));
        live
    }
}

/// The serve-side sessions host: the store plus the daemon's own identity.
/// Cloned into every connection thread (the Arc is the shared store).
#[derive(Clone, Default)]
struct SessionsHost {
    registry: std::sync::Arc<std::sync::Mutex<SessionRegistry>>,
    /// The daemon's own registration (role `daemon`): upserted at startup
    /// and refreshed on each registry serve (spec req 1). `None` = no
    /// registry service at all — the stub, whose /sessions refuses (a stub
    /// must never fabricate a registry, and an empty-but-200 answer would
    /// be exactly that fabrication).
    self_entry: Option<SessionEntry>,
}

impl SessionsHost {
    /// A host with the daemon's own registration already stored (startup —
    /// spec req 1). The id is the bare `daemon`: the single-instance flock
    /// admits at most one daemon per host home, so the id is unique by
    /// construction and a restart upserts the same row instead of churning
    /// pid-keyed ones.
    fn with_self() -> Self {
        let self_entry = daemon_self_entry();
        let registry = std::sync::Arc::new(std::sync::Mutex::new(SessionRegistry::default()));
        registry
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .upsert(self_entry.clone());
        Self {
            registry,
            self_entry: Some(self_entry),
        }
    }
}

/// The daemon's own session identity.
fn daemon_self_entry() -> SessionEntry {
    let now = epoch_now();
    SessionEntry {
        id: "daemon".into(),
        role: "daemon".into(),
        started: rfc3339(now),
        started_epoch: Some(now),
        last_event_ts: rfc3339(now),
        last_event_epoch: now,
        status: "serving".into(),
    }
}

/// Seconds since the Unix epoch (the registry's one clock). A clock that
/// went backwards yields 0 — fail-open, never a panic.
fn epoch_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Format epoch seconds as RFC3339 UTC (`YYYY-MM-DDTHH:MM:SSZ`) — the wire
/// timestamp shape every emitter speaks (the shell side emits exactly this
/// with `date -u +%Y-%m-%dT%H:%M:%SZ`). Reuses archive.rs's civil-from-days
/// math (no chrono dependency) and re-clothes it in RFC3339 punctuation.
fn rfc3339(secs: u64) -> String {
    let compact = crate::archive::format_timestamp(secs); // YYYYMMDD-HHMMSS
    format!(
        "{}-{}-{}T{}:{}:{}Z",
        &compact[0..4],
        &compact[4..6],
        &compact[6..8],
        &compact[9..11],
        &compact[11..13],
        &compact[13..15]
    )
}

/// Parse an RFC3339 timestamp to epoch seconds. Accepts the emitter shapes:
/// `YYYY-MM-DDTHH:MM:SS` (space tolerated for `T`), optional fractional
/// seconds, optional zone (`Z`, or `±HH:MM`/`±HHMM`; a missing zone reads
/// as UTC). Anything else is `None` — the caller fails open to server-now.
/// Deliberately narrow: the emitters are this repo's helper and `date -u`;
/// a full RFC3339 grammar is not the registry's job.
fn parse_rfc3339(text: &str) -> Option<u64> {
    let text = text.trim();
    let bytes = text.as_bytes();
    if bytes.len() < 19 {
        return None;
    }
    // Fixed-width shape check before any parse: YYYY-MM-DD(T| )HH:MM:SS.
    if bytes[4] != b'-'
        || bytes[7] != b'-'
        || !(bytes[10] == b'T' || bytes[10] == b' ')
        || bytes[13] != b':'
        || bytes[16] != b':'
    {
        return None;
    }
    let quad = |range: std::ops::Range<usize>| text.get(range)?.parse::<u64>().ok();
    let (year, month, day) = (quad(0..4)?, quad(5..7)?, quad(8..10)?);
    let (hour, minute, second) = (quad(11..13)?, quad(14..16)?, quad(17..19)?);
    if !(1..=12).contains(&month)
        || day > u64::from(days_in_month(year as i64, month as u32))
        || hour > 23
        || minute > 59
        || second > 60
    {
        return None;
    }
    // Fractional seconds: parsed and discarded (the registry's clock is
    // second-resolution).
    let mut rest = &text[19..];
    if let Some(after_dot) = rest.strip_prefix('.') {
        let digits = after_dot.find(|c: char| !c.is_ascii_digit()).unwrap_or(after_dot.len());
        if digits == 0 {
            return None;
        }
        rest = &after_dot[digits..];
    }
    let offset_secs: i64 = match rest {
        "" | "Z" | "z" => 0,
        _ => {
            let sign = match rest.as_bytes()[0] {
                b'+' => 1,
                b'-' => -1,
                _ => return None,
            };
            let digits: String = rest[1..].chars().filter(|c| c.is_ascii_digit()).collect();
            if digits.len() != 4 {
                return None;
            }
            let (hh, mm) = (digits[..2].parse::<i64>().ok()?, digits[2..].parse::<i64>().ok()?);
            if hh > 23 || mm > 59 {
                return None;
            }
            sign * (hh * 3600 + mm * 60)
        }
    };
    let days = days_from_civil(year as i64, month as u32, day as u32);
    let secs = days * 86_400 + (hour * 3600 + minute * 60 + second) as i64 - offset_secs;
    u64::try_from(secs).ok()
}

/// Days in a month of a (possibly leap) year — the proleptic Gregorian rule.
fn days_in_month(year: i64, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

/// Howard Hinnant's `days_from_civil` — the inverse of archive.rs's
/// civil-from-days (the same no-chrono doctrine, mirrored).
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u64; // [0, 399]
    let mp = if m > 2 { m - 3 } else { m + 9 } as u64; // [0, 11]
    let doy = (153 * mp + 2) / 5 + u64::from(d) - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    era * 146_097 + doe as i64 - 719_468
}

/// Parse one `POST /sessions` body into a store entry. `id` (non-empty
/// string) and `role` (one of [`SESSION_ROLES`]) are required — their
/// absence or a bad role is a 400. `started`, `last_event_ts`, and `status`
/// are optional: absent or unparseable timestamps fail open to server-now
/// (an emitter with a broken clock still registers, fresh), an absent
/// status defaults to `active`.
fn parse_session_registration(body: &str, now: u64) -> Result<SessionEntry, String> {
    let parsed: Value =
        serde_json::from_str(body).map_err(|e| format!("invalid session JSON: {e}"))?;
    let object = parsed
        .as_object()
        .ok_or_else(|| "session body must be a JSON object".to_string())?;
    let bad_role = || {
        format!(
            "session \"role\" must be one of {}",
            SESSION_ROLES.join(", ")
        )
    };
    let id = object
        .get("id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .filter(|id| !id.is_empty())
        .ok_or_else(|| "session \"id\" must be a non-empty string".to_string())?;
    let role = object
        .get("role")
        .and_then(Value::as_str)
        .ok_or_else(bad_role)?
        .to_string();
    if !SESSION_ROLES.contains(&role.as_str()) {
        return Err(bad_role());
    }
    let started = object
        .get("started")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| rfc3339(now));
    // The EFFECTIVE started is what gets parsed — the server-stamped default
    // is a real timestamp, so its epoch must be derived from it, not lost.
    let started_epoch = parse_rfc3339(&started);
    let last_event = object
        .get("last_event_ts")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| rfc3339(now));
    let last_event_epoch = parse_rfc3339(&last_event).unwrap_or(now);
    let status = object
        .get("status")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| "active".into());
    Ok(SessionEntry {
        id,
        role,
        started,
        started_epoch,
        last_event_ts: last_event,
        last_event_epoch,
        status,
    })
}

/// The `GET /sessions` body: the daemon's own heartbeat lands first (spec
/// req 1: heartbeats on each registry serve), expired entries are evicted,
/// and the live ones render with the derived `age_sec` plus the server's
/// `now`. A poisoned store lock is recovered (fail-open), never a 500.
fn sessions_body(host: &SessionsHost, now: u64) -> String {
    let mut registry = host
        .registry
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if let Some(self_entry) = &host.self_entry {
        let mut heartbeat = self_entry.clone();
        heartbeat.last_event_ts = rfc3339(now);
        heartbeat.last_event_epoch = now;
        registry.upsert(heartbeat);
    }
    let sessions: Vec<Value> = registry
        .evict_live(now)
        .into_iter()
        .map(|entry| {
            // Age tracks the session's own `started` (the client's claim,
            // echoed); an unparseable one renders 0 rather than a guess.
            let age_sec = entry
                .started_epoch
                .map(|started| now.saturating_sub(started))
                .unwrap_or(0);
            json!({
                "id": entry.id,
                "role": entry.role,
                "started": entry.started,
                "last_event_ts": entry.last_event_ts,
                "status": entry.status,
                "age_sec": age_sec,
            })
        })
        .collect();
    json!({"sessions": sessions, "now": rfc3339(now)}).to_string()
}

/// The `POST /sessions` route: parse (400 on a bad request), upsert (the
/// store lock is poisoned-recovered, fail-open), 200 `{"ok":true}`.
fn session_register(host: &SessionsHost, body: &str) -> (u16, String) {
    match parse_session_registration(body, epoch_now()) {
        Ok(entry) => {
            host.registry
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .upsert(entry);
            (200, json!({"ok": true}).to_string())
        }
        Err(message) => (400, detail(message)),
    }
}

/// The stub's refusal shape for /sessions — exactly the stub /judge refusal
/// (non-2xx + `{"detail": …}`): a stub must never fabricate a registry any
/// more than it fabricates a verdict (spec req 6).
fn stub_sessions_refusal() -> (u16, String) {
    (
        500,
        detail("CHUG_DAEMON_STUB test backend: /sessions is never served by the stub"),
    )
}

/// T219 — the shared best-effort registration/heartbeat helper for
/// in-process emitters (the delegate launch site registers its children and
/// heartbeats them on status polls; loopd.sh and dashd ride the same one
/// wire from the shell via curl). NEVER errors, NEVER blocks a run beyond
/// the judge client's own 2s socket bound, NEVER prints: a connect failure
/// (no daemon running — the common case) is an instant no-op, and every
/// other failure is swallowed (the registry is visibility, not control).
/// The POST carries no `started`: the daemon-side upsert stamps it at first
/// registration and heartbeats keep the original, so one stateless call
/// serves as both registration and heartbeat.
pub fn register_session(role: &str, id: &str, status: &str) {
    let Ok(sock) = sock_path() else {
        return;
    };
    let body = json!({
        "id": id,
        "role": role,
        "last_event_ts": rfc3339(epoch_now()),
        "status": status,
    })
    .to_string();
    let _ = uds_request(&sock, "POST", "/sessions", Some(&body));
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

/// The weightless registry host's backend (`CHUG_DAEMON_SESSIONS=1`): real
/// transport, lock, socket, and /sessions registry — NO model. /judge
/// refuses with the stub's shape: a weightless host must never fabricate
/// classifications (SPEC-3); it only hosts registrations.
struct SessionsBackend;

impl JudgeBackend for SessionsBackend {
    fn judge_request(&self, _raw: &str) -> anyhow::Result<Value> {
        bail!(
            "CHUG_DAEMON_SESSIONS registry host: /judge is never served without the judge model"
        )
    }
    fn describe(&self) -> String {
        "registry-only (CHUG_DAEMON_SESSIONS=1 — /judge refused)".into()
    }
}

/// `chug daemon` (serve mode): take the single-instance lock, load the model
/// BEFORE the socket binds (healthz == warm), serve until killed.
pub fn serve() -> anyhow::Result<i32> {
    let _lock = acquire_daemon_lock()?;
    let backend: std::sync::Arc<dyn JudgeBackend> = if sessions_requested() {
        std::sync::Arc::new(SessionsBackend)
    } else if stub_requested() {
        std::sync::Arc::new(StubBackend)
    } else {
        real_backend()?
    };
    // T219: the sessions registry is served by every real (non-stub) serve —
    // the full model daemon and the weightless registry host alike. The
    // stub serves NO registry: /sessions refuses there (a stub must never
    // fabricate a registry — an empty-but-200 answer would be exactly that
    // lie).
    let sessions = if stub_requested() {
        None
    } else {
        Some(SessionsHost::with_self())
    };
    serve_unix_with(&sock_path()?, backend, sessions)
}

/// The `CHUG_DAEMON_STUB=1` test seam switch.
fn stub_requested() -> bool {
    std::env::var(STUB_ENV).map(|v| v.trim() == "1").unwrap_or(false)
}

/// The `CHUG_DAEMON_SESSIONS=1` weightless-registry-host switch.
fn sessions_requested() -> bool {
    std::env::var(SESSIONS_ENV).map(|v| v.trim() == "1").unwrap_or(false)
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
            crate::judge_model::CheckpointSpec::Hub { repo, revision } => {
                format!("rl-agent (hub {repo}@{revision})")
            }
        }
    }
}

/// Bind the socket at 0600 and serve until killed, with a fresh (self-less)
/// sessions registry. Test-fixture entry only (the real serve path is
/// [`serve_unix_with`] via [`serve`], which brings the daemon's own
/// identity); one thread per connection (a slow client must never starve
/// the healthz probes).
#[cfg(test)]
pub fn serve_unix(sock: &Path, backend: std::sync::Arc<dyn JudgeBackend>) -> anyhow::Result<i32> {
    serve_unix_with(sock, backend, Some(SessionsHost::default()))
}

/// The parameterized serve: `sessions` carries the registry (and the
/// daemon's own identity); `None` refuses /sessions (the stub serve — a
/// stub must never fabricate a registry).
fn serve_unix_with(
    sock: &Path,
    backend: std::sync::Arc<dyn JudgeBackend>,
    sessions: Option<SessionsHost>,
) -> anyhow::Result<i32> {
    #[cfg(unix)]
    {
        let listener = bind_socket(sock).with_context(|| format!("binding {}", sock.display()))?;
        eprintln!(
            "chug daemon: serving /healthz + /judge{} on {} (mode 0600) — {}",
            if sessions.is_some() { " + /sessions" } else { "" },
            sock.display(),
            backend.describe()
        );
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let backend = backend.clone();
                    let sessions = sessions.clone();
                    if let Err(e) = std::thread::Builder::new()
                        .name("chug-daemon-conn".into())
                        .spawn(move || handle_conn(backend, sessions, stream))
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
        let _ = (sock, backend, sessions);
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
fn handle_conn(
    backend: std::sync::Arc<dyn JudgeBackend>,
    sessions: Option<SessionsHost>,
    mut stream: std::os::unix::net::UnixStream,
) {
    let _ = stream.set_read_timeout(Some(JUDGE_TIMEOUT));
    let _ = stream.set_write_timeout(Some(JUDGE_TIMEOUT));
    let (status, body) = match read_request(&mut stream) {
        Ok(request) => route(&*backend, sessions.as_ref(), &request),
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
/// (the layad drop-in), POST/GET /sessions (the T219 registry — upsert one
/// run / list the live ones), everything else 404. The registry rides the
/// same socket and the same failure shapes (`{"detail": …}`) but shares NO
/// state with the judge path: a registry store error can never fail /judge
/// (spec req 4), and a stub serve (no registry) refuses /sessions exactly as
/// it refuses /judge (spec req 6).
fn route(
    backend: &dyn JudgeBackend,
    sessions: Option<&SessionsHost>,
    request: &HttpRequest,
) -> (u16, String) {
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
        ("POST", "/sessions") => match sessions {
            Some(host) => session_register(host, &request.body),
            None => stub_sessions_refusal(),
        },
        ("GET", "/sessions") => match sessions {
            Some(host) => (200, sessions_body(host, epoch_now())),
            None => stub_sessions_refusal(),
        },
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

    /// Bounded retry for `daemon_lock_is_exclusive`'s post-release
    /// reacquisition: generous enough to outlive any transient fork child's
    /// exec lag, short enough that a stuck holder fails the test in seconds
    /// (the never-releases mutant must time out RED here, never pass).
    const LOCK_REACQUIRE_DEADLINE: Duration = Duration::from_secs(10);
    /// Poll cadence for the bounded reacquire (~400 attempts inside the
    /// deadline; a transient fork child's inherited fd clears the instant
    /// it execs).
    const LOCK_REACQUIRE_BACKOFF: Duration = Duration::from_millis(25);
    /// Bounded retry for `stale_socket_connects_refused`'s classification:
    /// outlives the suite's fd-pressure/teardown transients, still fails in
    /// seconds against a genuinely broken classification.
    const STALE_CLASSIFY_DEADLINE: Duration = Duration::from_secs(10);
    /// Poll cadence for the bounded classification.
    const STALE_CLASSIFY_BACKOFF: Duration = Duration::from_millis(25);

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

    /// Spin serve_unix on a fresh temp socket and wait for healthz —
    /// parameterized over the backend so the non-200 latch pin can serve a
    /// failing /judge.
    fn spawn_fixture_backend(backend: Arc<dyn JudgeBackend>) -> (tempfile::TempDir, PathBuf) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let sock = tmp.path().join("contract.sock");
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

    /// Spin serve_unix on a fresh temp socket and wait for healthz.
    fn spawn_fixture_server(response: Value) -> (tempfile::TempDir, PathBuf) {
        spawn_fixture_backend(Arc::new(FixtureBackend { response }))
    }

    /// Ordered-object field lookup (judge_model's `field_pub` is its own
    /// test-module helper; this is the same one-liner, local to daemon's).
    #[cfg(feature = "daemon")]
    fn ov_field<'a>(v: &'a OValue, key: &str) -> Option<&'a OValue> {
        match v {
            OValue::Obj(entries) => {
                entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
            }
            _ => None,
        }
    }

    /// OValue -> serde_json Value for the golden comparison (object key order
    /// is not a shape concern for the response contract).
    #[cfg(feature = "daemon")]
    fn ov_to_json(v: &OValue) -> Value {
        match v {
            OValue::Null => Value::Null,
            OValue::Bool(b) => Value::Bool(*b),
            OValue::Int(i) => json!(i),
            OValue::Float(f) => json!(f),
            OValue::Str(s) => json!(s),
            OValue::Arr(items) => Value::Array(items.iter().map(ov_to_json).collect()),
            OValue::Obj(entries) => {
                let mut m = serde_json::Map::new();
                for (k, v) in entries {
                    m.insert(k.clone(), ov_to_json(v));
                }
                Value::Object(m)
            }
        }
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
        // After release, acquisition works — EVENTUALLY. flock locks ride the
        // open file description: a `Command::spawn` (fork+exec; sibling
        // judge_path tests spawn with `pre_exec`, forcing the fork path)
        // forked while `first` was held inherits this fd, and the child keeps
        // the description open until its exec lands (CLOEXEC). Under a loaded
        // threaded harness the fork→exec lag beats this thread from `drop`
        // to reacquire, so ONE attempt can hit EAGAIN "held by <own pid>"
        // against an fd that is about to close (cycle-95 eval: 3/3 red at
        // --test-threads=4 with the spawn family in the run set). What the
        // test MEANS: once every holder — including those transient fork
        // children — is gone, the lock is free. Bounded retry on a ~10s
        // deadline; the live-holder refusal leg above stays single-shot.
        let deadline = Instant::now() + LOCK_REACQUIRE_DEADLINE;
        let third = loop {
            match acquire_lock_at(&lock) {
                Ok(acquired) => break acquired,
                Err(err) => {
                    assert!(
                        Instant::now() < deadline,
                        "the lock never became free within {LOCK_REACQUIRE_DEADLINE:?} of release: {err}"
                    );
                    std::thread::sleep(LOCK_REACQUIRE_BACKOFF);
                }
            }
        };
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
    ///
    /// The classification is a BOUNDED RETRY, same shape as
    /// `daemon_lock_is_exclusive`: under the full-suite threaded run the
    /// one-shot assert has raced kernel transients that have nothing to do
    /// with the stale socket — the observed flavors are EMFILE ("Too many
    /// open files", os error 24: the suite's fd churn against the 256
    /// default soft limit makes the socket(2) inside connect fail even
    /// though the bind a moment earlier succeeded) and ENOENT (os error 2,
    /// the close-vs-connect teardown burst reproduced in the scratch
    /// bind/drop/connect racer). ECONNREFUSED must EVENTUALLY be the
    /// classification; on timeout the last observed error names itself.
    #[cfg(unix)]
    #[test]
    fn stale_socket_connects_refused() {
        use std::os::unix::net::{UnixListener, UnixStream};

        let tmp = tempfile::tempdir().expect("tempdir");
        let sock = tmp.path().join("stale.sock");
        // Create the socket file, then drop the listener (SIGKILL'd-daemon
        // shape: file present, no listener).
        drop(UnixListener::bind(&sock).expect("bind"));
        assert!(sock.exists(), "the stale socket file exists");
        // Poll for the exact ECONNREFUSED classification (raw errno, not the
        // shipping bool) so an over-eager classifier cannot make this pass.
        let deadline = Instant::now() + STALE_CLASSIFY_DEADLINE;
        loop {
            match UnixStream::connect(&sock) {
                Err(e) if e.raw_os_error() == Some(libc::ECONNREFUSED) => break,
                Err(e) => {
                    assert!(
                        Instant::now() < deadline,
                        "connect to the dead socket never classified ECONNREFUSED within \
                         {STALE_CLASSIFY_DEADLINE:?} (last transient: {e})"
                    );
                    std::thread::sleep(STALE_CLASSIFY_BACKOFF);
                }
                Ok(_) => panic!("connect to the dead socket SUCCEEDED — something is listening"),
            }
        }
        // The shipping classifier agrees on BOTH sides of the line: a dead
        // socket file IS refused; a missing file's ENOENT is NOT (this pair
        // kills an always-refused mutant of `connect_refused`, which gates
        // ensure's stale-socket unlink).
        assert!(connect_refused(&sock), "the dead socket classifies ECONNREFUSED");
        assert!(
            !connect_refused(&tmp.path().join("missing.sock")),
            "ENOENT (no socket file) must not classify as refused"
        );
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
            binary: None,
            spawned: None,
            dead: Some("latched for the test".into()),
            waited: Duration::ZERO,
        };
        let err = judge.judge("rm -rf /").expect_err("latched");
        assert_eq!(err, "latched for the test");
    }

    /// The PURE latch decision table: a permanent ensure failure latches at
    /// any `waited`; a transient flavor latches ONLY once cumulative futile
    /// waiting has consumed [`MAX_LIFETIME_WAIT`] (the unreachable-socket
    /// leg — retrying forever must never stall a run minutes per bash
    /// command, but a daemon still loading must never be given up on early).
    #[test]
    fn latch_decision_table_is_pinned() {
        let permanent = EnsureFailure {
            message: "spawn failed".into(),
            latch: true,
        };
        let transient = EnsureFailure {
            message: "still loading".into(),
            latch: false,
        };
        assert_eq!(
            latch_reason(&permanent, Duration::ZERO),
            Some("spawn failed".into()),
            "a permanent flavor latches immediately"
        );
        assert_eq!(
            latch_reason(&transient, MAX_LIFETIME_WAIT - Duration::from_secs(1)),
            None,
            "a transient flavor below the lifetime budget never latches"
        );
        assert_eq!(
            latch_reason(&transient, MAX_LIFETIME_WAIT),
            Some("still loading".into()),
            "cumulative futile waiting past the budget latches even a transient flavor"
        );
        assert_eq!(
            latch_reason(&transient, MAX_LIFETIME_WAIT * 2),
            Some("still loading".into())
        );
    }

    /// THE M4 PIN: the latch is SET by the judge() path the RiskGate
    /// actually calls through (`Box<dyn Judge>` -> [`DaemonJudge::judge`]).
    /// The sibling test above pins ensure_at's latch FLAG and the dead
    /// short-circuit; THIS pins the set itself: one judge() call on a
    /// permanent ensure failure must STORE the failure, so every later call
    /// replays it instantly. Deleting the set (the validator's M4 mutant —
    /// the full suite survived it) leaves `dead` None and re-ensures per
    /// bash command forever; this test fails on exactly that shape.
    #[test]
    fn judge_path_sets_the_latch_on_a_permanent_ensure_failure() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let mut judge = DaemonJudge {
            sock: tmp.path().join("missing.sock"),
            binary: None, // the no-binary seam: ensure fails PERMANENTLY, instantly
            spawned: None,
            dead: None,
            waited: Duration::ZERO,
        };
        let first = match judge.judge("rm -rf /") {
            Ok(_) => panic!("a permanent ensure failure must not judge"),
            Err(e) => e,
        };
        assert!(
            judge.dead.is_some(),
            "the first judge() call must latch the permanent failure"
        );
        assert!(judge.spawned.is_none(), "no daemon was ever spawned");
        // The latched client replays the SAME recorded failure.
        let second = match judge.judge("ls") {
            Ok(_) => panic!("a latched client must not judge"),
            Err(e) => e,
        };
        assert_eq!(second, first, "the latched client replays the recorded failure");
    }

    /// The spawn-exited permanent flavor through the same judge() path: a
    /// binary that dies instantly with no lock holder (the feature-off
    /// binary / unloadable-checkpoint shape) means nothing will ever serve —
    /// the FIRST judge() call must latch. CHUG_HOME is scoped to a tempdir
    /// (under the crate's env lock) so the daemon log/lock touch nothing
    /// real.
    #[test]
    fn judge_path_latches_when_the_spawned_daemon_dies_without_serving() {
        let _timing = crate::testsupport::timing_guard();
        let _guard = crate::delegate::tests::DELEGATE_ENV_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let saved_home = std::env::var_os(HOME_ENV);
        // SAFETY: env mutation serialized by DELEGATE_ENV_LOCK (held above);
        // restored by the Restore guard before any other test reads HOME_ENV.
        unsafe { std::env::remove_var(HOME_ENV) };
        let tmp = tempfile::tempdir().expect("tempdir");
        // SAFETY: serialized by DELEGATE_ENV_LOCK; restored before return.
        unsafe { std::env::set_var(HOME_ENV, tmp.path()) };
        struct Restore(Option<std::ffi::OsString>);
        impl Drop for Restore {
            fn drop(&mut self) {
                match self.0.as_deref() {
                    Some(v) => unsafe { std::env::set_var(HOME_ENV, v) },
                    None => unsafe { std::env::remove_var(HOME_ENV) },
                }
            }
        }
        let _restore = Restore(saved_home);

        let mut judge = DaemonJudge {
            sock: tmp.path().join("missing.sock"),
            // Dies instantly, holds no lock ("false" resolves via PATH on
            // both macOS — /usr/bin/false — and Linux — /bin/false).
            binary: Some(PathBuf::from("false")),
            spawned: None,
            dead: None,
            waited: Duration::ZERO,
        };
        let err = match judge.judge("rm -rf /") {
            Ok(_) => panic!("a daemon that died without serving must not judge"),
            Err(e) => e,
        };
        assert!(
            err.contains("exited before serving"),
            "the spawn-exited failure is named: {err}"
        );
        assert!(
            judge.dead.is_some(),
            "the judge() path latched the spawn-exited permanent failure"
        );
    }

    /// The non-200 leg: a LIVE daemon that refuses the judgment is a
    /// TRANSIENT judge failure — the client returns the error but never
    /// latches (the next bash command retries the POST against the same
    /// live daemon, which costs no spawn; latch here would turn one bad
    /// backend response into a judge disabled for the whole process).
    #[test]
    fn judge_path_does_not_latch_a_non_200_from_a_live_daemon() {
        struct FailingBackend;
        impl JudgeBackend for FailingBackend {
            fn judge_request(&self, _raw: &str) -> anyhow::Result<Value> {
                bail!("the judge backend is broken (fixture)")
            }
            fn describe(&self) -> String {
                "failing (non-200 fixture)".into()
            }
        }
        let (tmp, sock) = spawn_fixture_backend(Arc::new(FailingBackend));
        let mut judge = DaemonJudge {
            sock,
            binary: None, // never consulted: healthz answers, ensure returns first
            spawned: None,
            dead: None,
            waited: Duration::ZERO,
        };
        let err = match judge.judge("rm -rf /") {
            Ok(_) => panic!("a failing backend must not judge"),
            Err(e) => e,
        };
        assert!(
            err.contains("judge daemon returned HTTP 500"),
            "the non-200 error flavor: {err}"
        );
        assert!(
            judge.dead.is_none(),
            "a live daemon's non-200 is transient — never latched"
        );
        drop(tmp);
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
        let judge = DaemonJudge {
            sock: tmp.path().join("absent.sock"),
            binary: None,
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
    // T219 — the /sessions registry: pure store/parse/TTL tables plus the
    // wire shapes over the REAL transport (in-process, no model needed).
    // ------------------------------------------------------------------

    /// One store entry for the table tests (a loopd-shaped run at `beat`).
    fn session_entry(id: &str, beat: u64) -> SessionEntry {
        SessionEntry {
            id: id.into(),
            role: "loopd-cycle".into(),
            started: rfc3339(beat),
            started_epoch: Some(beat),
            last_event_ts: rfc3339(beat),
            last_event_epoch: beat,
            status: "running".into(),
        }
    }

    /// The registration parse table: required fields enforced (400-class
    /// errors), optional fields defaulted server-side, unparseable
    /// timestamps fail open to server-now while the client's words still
    /// echo verbatim.
    #[test]
    fn session_registration_parse_table() {
        let now = 1_709_208_000u64; // 2024-02-29T12:00:00Z (the leap-day anchor)
        // Full body: every posted field is kept, epochs are parsed.
        let full = parse_session_registration(
            "{\"id\":\"loopd-1\",\"role\":\"loopd-cycle\",\"started\":\"2024-02-29T11:00:00Z\",\
             \"last_event_ts\":\"2024-02-29T11:59:00Z\",\"status\":\"running\"}",
            now,
        )
        .expect("full body parses");
        assert_eq!(full.id, "loopd-1");
        assert_eq!(full.role, "loopd-cycle");
        assert_eq!(full.started, "2024-02-29T11:00:00Z");
        assert_eq!(full.started_epoch, Some(1_709_204_400));
        assert_eq!(full.last_event_ts, "2024-02-29T11:59:00Z");
        assert_eq!(full.last_event_epoch, 1_709_207_940);
        assert_eq!(full.status, "running");

        // Minimal body: server stamps both timestamps (fail-open fresh) and
        // defaults the status.
        let minimal =
            parse_session_registration("{\"id\":\"child-2\",\"role\":\"delegate-child\"}", now)
                .expect("minimal body parses");
        assert_eq!(minimal.started, rfc3339(now));
        assert_eq!(minimal.started_epoch, Some(now));
        assert_eq!(minimal.last_event_epoch, now);
        assert_eq!(minimal.status, "active");

        // Unparseable timestamps fail open (fresh) but echo verbatim.
        let garbled = parse_session_registration(
            "{\"id\":\"dashd-1\",\"role\":\"dashd\",\"last_event_ts\":\"yesterday\"}",
            now,
        )
        .expect("a garbled timestamp still registers");
        assert_eq!(garbled.last_event_ts, "yesterday");
        assert_eq!(garbled.last_event_epoch, now);

        // 400 class: not JSON, not an object, missing/empty id, missing or
        // unknown role.
        assert!(parse_session_registration("{not json", now).is_err());
        assert!(parse_session_registration("[1]", now).is_err());
        assert!(parse_session_registration("{\"role\":\"dashd\"}", now).is_err());
        assert!(parse_session_registration("{\"id\":\"\",\"role\":\"dashd\"}", now).is_err());
        assert!(parse_session_registration("{\"id\":\"x\"}", now).is_err());
        let bad_role = parse_session_registration("{\"id\":\"x\",\"role\":\"cron\"}", now)
            .expect_err("unknown role refused");
        assert!(bad_role.contains("must be one of"), "{bad_role}");
        assert!(bad_role.contains("loopd-cycle") && bad_role.contains("daemon"), "{bad_role}");
    }

    /// The store table: upsert-by-id keeps ONE entry and refreshes liveness
    /// fields while `started` sticks to the FIRST registration; TTL eviction
    /// is exact at the boundary (<= TTL live, > TTL evicted) and sorted by
    /// id (deterministic renders).
    #[test]
    fn session_registry_upsert_and_ttl_table() {
        let now = 1_000_000_000u64;
        let mut registry = SessionRegistry::default();
        registry.upsert(session_entry("b-second", now - 100));
        registry.upsert(session_entry("a-first", now - 200));
        // Upsert the same id: one entry, started sticks, last_event refreshes.
        let mut heartbeat = session_entry("a-first", now);
        heartbeat.started = rfc3339(now - 900);
        heartbeat.started_epoch = Some(now - 900);
        heartbeat.status = "wrapping".into();
        registry.upsert(heartbeat);
        let live = registry.evict_live(now);
        assert_eq!(live.len(), 2, "one entry per id");
        assert_eq!(live[0].id, "a-first", "sorted by id");
        assert_eq!(
            live[0].started,
            rfc3339(now - 200),
            "started sticks to the FIRST registration (the heartbeat's newer value is discarded)"
        );
        assert_eq!(live[0].last_event_epoch, now, "the heartbeat refreshed liveness");
        assert_eq!(live[0].status, "wrapping", "status takes the latest POST");

        // TTL boundary: exactly SESSION_TTL_SECS old is still live; one
        // second past it is evicted; a future-dated heartbeat stays live.
        let mut registry = SessionRegistry::default();
        registry.upsert(session_entry("edge", now - SESSION_TTL_SECS));
        assert_eq!(registry.evict_live(now).len(), 1, "TTL boundary is inclusive");
        registry.upsert(session_entry("edge", now - SESSION_TTL_SECS - 1));
        assert!(registry.evict_live(now).is_empty(), "past the TTL is evicted");
        let mut future = session_entry("future", now + 60);
        future.last_event_ts = "2099-01-01T00:00:00Z".into();
        registry.upsert(future);
        assert_eq!(registry.evict_live(now).len(), 1, "future-dated heartbeats stay live");
    }

    /// The wire timestamp helpers: RFC3339 formatting anchors (no chrono —
    /// archive.rs's civil math re-clothed) and the parser's accept/reject
    /// table (the emitter shapes only).
    #[test]
    fn rfc3339_format_and_parse_table() {
        assert_eq!(rfc3339(0), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339(1_000_000_000), "2001-09-09T01:46:40Z");
        assert_eq!(rfc3339(1_709_208_000), "2024-02-29T12:00:00Z");
        for (text, want) in [
            ("1970-01-01T00:00:00Z", 0u64),
            ("2001-09-09T01:46:40Z", 1_000_000_000),
            ("2024-02-29T12:00:00Z", 1_709_208_000),
            // Space for T (a `date` dialect) and a missing zone read as UTC.
            ("2024-02-29 12:00:00", 1_709_208_000),
            ("2024-02-29T12:00:00", 1_709_208_000),
            // Fractional seconds parse and are discarded.
            ("2024-02-29T12:00:00.123456Z", 1_709_208_000),
            // A +01:00 offset means the wall clock is BEHIND UTC.
            ("2024-02-29T13:00:00+01:00", 1_709_208_000),
            ("2024-02-29T13:00:00+0100", 1_709_208_000),
            ("2024-02-29T11:00:00-01:00", 1_709_208_000),
        ] {
            assert_eq!(parse_rfc3339(text), Some(want), "parsing {text}");
        }
        // Roundtrip: format then parse returns the same epoch second.
        let now = epoch_now();
        assert_eq!(parse_rfc3339(&rfc3339(now)), Some(now));
        // Rejects: junk, bad shapes, out-of-range fields.
        for text in [
            "", "not a time", "2024-02-29", "2024-02-29T12:00", "24-02-29T12:00:00Z",
            "2024-13-01T00:00:00Z", "2024-02-30T00:00:00Z", "2024-02-29T24:00:00Z",
            "2024-02-29T12:60:00Z", "2024-02-29T12:00:00Z.", "2024-02-29T12:00:00X",
            "2024-02-29T12:00:00+99:00",
        ] {
            assert_eq!(parse_rfc3339(text), None, "rejecting {text:?}");
        }
    }

    /// The /sessions wire over the REAL transport, in-process: register ->
    /// GET roundtrip with the posted fields, the 400 class, the role gate,
    /// the daemon's self-entry, the self-heartbeat on each serve, and — on
    /// the sessions host — /judge refuses exactly as the stub's does while
    /// /healthz and unknown-path refusals keep today's shapes. The stub
    /// configuration (no registry) refuses /sessions on BOTH verbs.
    #[cfg(unix)]
    #[test]
    fn sessions_wire_over_uds() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let sock = tmp.path().join("sessions.sock");
        let server_sock = sock.clone();
        std::thread::spawn(move || {
            let _ = serve_unix_with(
                &server_sock,
                Arc::new(SessionsBackend),
                Some(SessionsHost::with_self()),
            );
        });
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline && healthz_ok(&sock).is_err() {
            std::thread::sleep(Duration::from_millis(20));
        }
        healthz_ok(&sock).expect("the sessions server never came up");

        // Register one run (all five posted fields) and read it back. The
        // last_event timestamp is FRESH (near the real clock — a stale one
        // is honestly evicted by the TTL before the GET); `started` is a
        // fixed historical string to pin the verbatim echo (it does not
        // drive the TTL, and an age before the epoch clamps to 0).
        let beat = epoch_now().saturating_sub(30);
        let posted = json!({
            "id": "loopd-42",
            "role": "loopd-cycle",
            "started": "2024-02-29T11:00:00Z",
            "last_event_ts": rfc3339(beat),
            "status": "running"
        })
        .to_string();
        let (status, body) = uds_request(&sock, "POST", "/sessions", Some(&posted)).expect("register");
        assert_eq!(status, 200, "register status: {body}");
        assert_eq!(body, "{\"ok\":true}", "register body");
        let (status, body) = uds_request(&sock, "GET", "/sessions", None).expect("list");
        assert_eq!(status, 200, "list status: {body}");
        let value: Value = serde_json::from_str(&body).expect("list JSON");
        assert!(value["now"].is_string(), "the body carries the server's now: {body}");
        let entries = value["sessions"].as_array().expect("sessions array");
        let entry = entries
            .iter()
            .find(|e| e["id"] == json!("loopd-42"))
            .unwrap_or_else(|| panic!("the posted entry is listed: {body}"));
        assert_eq!(entry["role"], json!("loopd-cycle"));
        assert_eq!(entry["started"], json!("2024-02-29T11:00:00Z"));
        assert_eq!(entry["last_event_ts"], json!(rfc3339(beat)));
        assert_eq!(entry["status"], json!("running"));
        assert!(entry["age_sec"].is_number(), "age_sec is derived: {entry}");

        // The daemon's own registration (startup, spec req 1): one entry,
        // role daemon, refreshed by this very serve (its last_event_ts is
        // the serve-time now — the roundtrip above happened seconds ago, so
        // a stale fixed timestamp would have been overwritten).
        let daemon_entry = entries
            .iter()
            .find(|e| e["id"] == json!("daemon"))
            .unwrap_or_else(|| panic!("the daemon self-registers: {body}"));
        assert_eq!(daemon_entry["role"], json!("daemon"));
        assert_eq!(daemon_entry["status"], json!("serving"));

        // The 400 class: bad JSON, missing id, unknown role — each with the
        // layad error shape.
        for (bad_body, fragment) in [
            ("{not json", "invalid session JSON"),
            ("{\"role\":\"dashd\"}", "\"id\""),
            ("{\"id\":\"x\",\"role\":\"cron\"}", "must be one of"),
        ] {
            let (status, body) =
                uds_request(&sock, "POST", "/sessions", Some(bad_body)).expect("bad register");
            assert_eq!(status, 400, "{bad_body} -> {body}");
            let err: Value = serde_json::from_str(&body).expect("error JSON");
            assert!(
                err["detail"].as_str().expect("detail string").contains(fragment),
                "{bad_body} -> {body}"
            );
        }

        // On the registry host /judge refuses with the stub's shape (never
        // fabricate a classification without the model), while /healthz and
        // the unknown-path refusal keep today's shapes byte-for-byte.
        let (status, body) = uds_request(&sock, "POST", "/judge", Some("{\"state\":{},\"questions\":{}}"))
            .expect("judge on the registry host");
        assert_eq!(status, 500, "the registry host never serves /judge: {body}");
        assert!(body.contains("\"detail\""), "refusal shape: {body}");
        let (status, body) = uds_request(&sock, "GET", "/nope", None).expect("404");
        assert_eq!(status, 404, "unknown path: {body}");
        assert!(body.contains("\"detail\""));

        // The stub configuration (sessions = None): BOTH verbs refuse with
        // the stub judge shape — a stub must never fabricate a registry.
        let stub_sock = tmp.path().join("stub.sock");
        let stub_server_sock = stub_sock.clone();
        std::thread::spawn(move || {
            let _ = serve_unix_with(&stub_server_sock, Arc::new(StubBackend), None);
        });
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline && healthz_ok(&stub_sock).is_err() {
            std::thread::sleep(Duration::from_millis(20));
        }
        for (method, body) in [("POST", "{\"id\":\"x\",\"role\":\"daemon\"}"), ("GET", "")] {
            let request_body = if body.is_empty() { None } else { Some(body) };
            let (status, text) =
                uds_request(&stub_sock, method, "/sessions", request_body).expect("stub refusal");
            assert_eq!(status, 500, "stub {method} /sessions -> {text}");
            let err: Value = serde_json::from_str(&text).expect("error JSON");
            assert!(
                err["detail"].as_str().expect("detail string").contains("never served by the stub"),
                "stub refusal names itself: {text}"
            );
        }
    }

    /// The self-heartbeat on each registry serve (spec req 1): a daemon
    /// entry whose stored heartbeat has gone stale is refreshed by the next
    /// GET instead of being evicted, while an equally stale foreign entry
    /// IS evicted — the daemon keeps itself alive as long as anyone asks.
    #[test]
    fn daemon_self_heartbeat_on_each_registry_serve() {
        let now = 1_000_000_000u64;
        let host = SessionsHost::with_self();
        // Simulate time passing: every stored heartbeat is far past the TTL.
        {
            let mut registry = host.registry.lock().unwrap_or_else(|e| e.into_inner());
            let mut stale_foreign = session_entry("loopd-dead", now - 60);
            stale_foreign.last_event_epoch = now - 60 - 4 * SESSION_TTL_SECS;
            stale_foreign.last_event_ts = rfc3339(stale_foreign.last_event_epoch);
            registry.upsert(stale_foreign);
            if let Some(self_entry) = registry.entries.get_mut("daemon") {
                self_entry.last_event_epoch = now - 4 * SESSION_TTL_SECS;
                self_entry.last_event_ts = rfc3339(self_entry.last_event_epoch);
            }
        }
        let value: Value =
            serde_json::from_str(&sessions_body(&host, now)).expect("list JSON");
        let ids: Vec<&str> = value["sessions"]
            .as_array()
            .expect("sessions array")
            .iter()
            .filter_map(|e| e["id"].as_str())
            .collect();
        assert!(
            ids.contains(&"daemon"),
            "the daemon's own stale entry is refreshed by the serve, never evicted: {value}"
        );
        assert!(
            !ids.contains(&"loopd-dead"),
            "an equally stale foreign entry is evicted: {value}"
        );
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
        // The wire body must carry the SDK's ORIGINAL question order: the
        // packing is order-sensitive (parse_ordered exists for exactly this),
        // so a serde_json roundtrip (BTreeMap-sorted) silently reorders the
        // questions and drifts the probabilities. Parse the golden file with
        // the order-preserving parser and serialize with OValue::dumps —
        // byte-for-byte what the Python SDK client would put on the wire.
        let golden_raw =
            std::fs::read_to_string("tests/fixtures/laya/golden-vectors.json")
                .expect("reading golden-vectors.json");
        let golden = crate::judge_pack::parse_ordered(&golden_raw)
            .expect("parsing golden-vectors.json");
        let fixtures = match ov_field(&golden, "fixtures") {
            Some(OValue::Arr(items)) => items,
            other => panic!("fixtures array, got {other:?}"),
        };
        let fx = fixtures
            .iter()
            .find(|fx| matches!(ov_field(fx, "name"), Some(OValue::Str(s)) if s == "riskgate_base"))
            .expect("riskgate_base fixture missing");
        let state = ov_field(fx, "state").expect("fixture state").clone();
        let questions = ov_field(fx, "questions")
            .expect("fixture questions")
            .clone();
        let request = OValue::Obj(vec![
            ("state".to_string(), state),
            ("questions".to_string(), questions),
        ])
        .dumps(false);
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

        let started = Instant::now();
        let (status, body) =
            uds_request(&sock, "POST", "/judge", Some(&request)).expect("live judge");
        let elapsed = started.elapsed();
        assert_eq!(status, 200, "live judge status");
        let value: Value = serde_json::from_str(&body).expect("live judge JSON");
        let expected = ov_field(fx, "expected").expect("fixture expected");
        assert_value_close(&value, &ov_to_json(expected), 1e-3, "riskgate_base over the socket");
        // The judge latency budget (spec req 5): warm ~50ms-class on MPS,
        // bounded CPU forward here — never the seconds-scale stall.
        assert!(
            elapsed < Duration::from_secs(2),
            "the warm judge took {elapsed:?} — over the fail-fast budget"
        );
    }
}
