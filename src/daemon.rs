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
//!
//! * Observation (T219, T340) — `GET /sessions` (the host-scoped chug
//!   registry) and `GET /status/loop` + `GET /status/items` (the T339 typed
//!   status contract, rendered from a repo's loop state files under a
//!   `?repo=` selector) are served by every real serve mode; they are
//!   observation only — nothing is written and there is no control surface.
//!   The stub serves /healthz only and refuses the rest, exactly as it
//!   refuses /judge.

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
    // T332: name the decision-model provider the selection table resolves
    // to right now — best-effort, on every status leg (the selection is
    // independent of this daemon's liveness; a proxy-selected box shows it
    // even when no local daemon runs).
    println!("decision provider: {}", crate::decision::status_line());
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
        // T221: day 0 must reject exactly like day 32 — `days_from_civil`
        // computes `… + d - 1` on u64, so day 0 underflows (a debug panic in
        // the conn thread, a silent wrap to the previous day in release).
        // Month is already bounded above by this range check.
        || day < 1
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
// T340 — the typed status endpoints: GET /status/loop, GET /status/items.
//
// Observation only: the daemon READS a repo's loop state files and renders
// the T339 contract's typed status model — what the loop is doing, what it
// is blocked on, what children (worktrees) it has, what the open queue looks
// like. Nothing is written and there is no control surface (control is a
// separate, explicitly-approved future item); the socket policy is unchanged
// (0600, host-scoped) — a /status reader could already read the same files
// the response names.
//
// Sourcing (all read-only, all bounded):
//   <repo>/.chug/loopd/loopd.pid    the supervisor's pid (liveness)
//   <repo>/.chug/loopd/HALTED       the supervisor's halt latch (+ reason)
//   <repo>/.chug/loopd/cycle-*.log  the supervisor's per-cycle logs (cycle_id)
//   <repo>/.chug/events.jsonl       the driver's event log (the run segment)
//   <repo>/TODO.md                  the queue (the /status/items view)
//   <repo>/.git[/worktrees/]        registered worktrees (the children)
//
// Every response carries `contract_version` (the T339 registry's value);
// fields are additive-only within a version, and a component that cannot be
// read degrades to null/empty — a missing repo is the exception: it is the
// T328 blocked/external case and must SURFACE with its detail, never vanish.
// ---------------------------------------------------------------------------

/// The loop's status, drawn from the loop's own working/blocked/idle/dormant
/// shape (the T339 contract's loop row).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopStatus {
    /// A driver is producing events right now (the events tail is fresh).
    Working,
    /// Stopped pending something; [`LoopReport::blocked_reason`] says what.
    Blocked,
    /// Alive between activities: the supervisor is up but no driver is
    /// producing (between cycles, or the last cycle completed).
    Idle,
    /// The loop machinery is not cycling: no live supervisor, or a live one
    /// with an empty queue (the T334 dormant gate), or the last run finished
    /// with no supervisor to relaunch.
    Dormant,
}

impl LoopStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Working => "working",
            Self::Blocked => "blocked",
            Self::Idle => "idle",
            Self::Dormant => "dormant",
        }
    }
}

/// Why a `blocked` loop is blocked — the T339 blocked-reason vocabulary. The
/// loop surface derives `external` (the T328 missing-repo class), `budget`
/// (a budget-kind abort), and `error` (a plain abort or the supervisor's
/// HALTED latch). `permission` and `question` are vocabulary members this
/// surface never derives: a headless loop has no permission prompt and no
/// operator question, and inventing either would fabricate a blocker — they
/// ride the same enum so harnesses render ONE vocabulary (the session-status
/// surfaces, T341/T342, are the ones that carry them).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockedReason {
    /// Not derived by this surface (see the enum doc) — carried so the wire
    /// enum is the full shared vocabulary.
    #[allow(dead_code)]
    Permission,
    /// Not derived by this surface (see the enum doc) — carried so the wire
    /// enum is the full shared vocabulary.
    #[allow(dead_code)]
    Question,
    Error,
    Budget,
    External,
}

impl BlockedReason {
    fn as_str(self) -> &'static str {
        match self {
            Self::Permission => "permission",
            Self::Question => "question",
            Self::Error => "error",
            Self::Budget => "budget",
            Self::External => "external",
        }
    }
}

/// One child: a worktree registered under the repo's git dir, classified by
/// the loop's lane naming (`impl` / `validate` worktrees; anything else is
/// the generic `worktree` kind).
struct StatusChild {
    kind: &'static str,
    id: String,
    state: String,
}

/// The derived loop state behind `GET /status/loop`.
struct LoopReport {
    status: LoopStatus,
    cycle_id: Option<String>,
    iteration: u32,
    item_id: Option<String>,
    blocked_reason: Option<BlockedReason>,
    blocked_detail: Option<String>,
    children: Vec<StatusChild>,
}

/// How long an events tail may stay silent before the driver behind it reads
/// as no-longer-working: a live driver emits iteration/tool events every
/// turn, and an LLM turn exceeding five minutes with zero events is rare
/// enough that silence means between-cycles (or a dead driver). The loop's
/// own pacing bounds it too — loopd relaunches within `LOOPD_SLEEP_FAIL`
/// (300s) of a failed cycle.
const FRESH_DRIVER_SECS: u64 = 300;
/// Bounded reads: the events tail window (the delegate status read's bound),
/// the children and queue-row caps. A repo whose state files exceed a cap
/// renders the newest/capped slice — never an error, never unbounded I/O.
const EVENTS_TAIL_BYTES: u64 = 64 * 1024;
const MAX_CHILDREN: usize = 64;
const MAX_TODO_ROWS: usize = 512;
const MAX_TODO_BYTES: u64 = 1024 * 1024;
/// `CHUG_DAEMON_REPO` — the serve-time default repo for the status endpoints
/// (a supervisor pins it at spawn). The `?repo=` query parameter wins over
/// it; the daemon process's cwd is the last fallback.
pub const REPO_ENV: &str = "CHUG_DAEMON_REPO";

/// The whole `GET /status/loop` body: the derived report plus the contract
/// version and the server's `as_of` stamp.
fn status_loop_body(repo: &Path, now: u64) -> String {
    let report = loop_report(repo, now);
    json!({
        "contract_version": crate::capabilities::CONTRACT_VERSION,
        "status": report.status.as_str(),
        "cycle_id": report.cycle_id,
        "iteration": report.iteration,
        "item_id": report.item_id,
        "blocked_reason": report.blocked_reason.map(|r| r.as_str()),
        "blocked_detail": report.blocked_detail,
        "children": report
            .children
            .iter()
            .map(|child| json!({"kind": child.kind, "id": child.id, "state": child.state}))
            .collect::<Vec<_>>(),
        "as_of": rfc3339(now),
    })
    .to_string()
}

/// The whole `GET /status/items` body: one row per open TODO item — the
/// harness's queue view without parsing TODO.md.
fn status_items_body(repo: &Path, _now: u64) -> String {
    json!({
        "contract_version": crate::capabilities::CONTRACT_VERSION,
        "items": open_todo_rows(repo),
    })
    .to_string()
}

/// Derive the loop state from the repo's state files, in documented order:
/// (1) a missing repo is blocked/external; (2) the supervisor's HALTED latch
/// is blocked/error; (3) the last run segment's terminal state — a budget
/// abort is blocked/budget, a plain abort blocked/error, a goal-accepted
/// completion is idle/dormant; (4) an open segment with a fresh tail is
/// working; (5) otherwise the supervisor decides: alive with open queue rows
/// is idle, everything else is dormant.
fn loop_report(repo: &Path, now: u64) -> LoopReport {
    let blocked = |reason, detail: String| LoopReport {
        status: LoopStatus::Blocked,
        cycle_id: None,
        iteration: 0,
        item_id: None,
        blocked_reason: Some(reason),
        blocked_detail: Some(detail),
        children: Vec::new(),
    };
    // (1) The T328 class: a repo that is not there is blocked/external and
    // names the path — it never vanishes into a 404.
    if !repo.is_dir() {
        return blocked(
            BlockedReason::External,
            format!("repo path missing or not a directory: {}", repo.display()),
        );
    }
    let children = status_children(repo);
    let mut report = LoopReport {
        status: LoopStatus::Dormant,
        cycle_id: cycle_id(repo),
        iteration: 0,
        item_id: item_id(repo),
        blocked_reason: None,
        blocked_detail: None,
        children,
    };
    // (2) The supervisor's halt latch (3 consecutive cycles without
    // goal_complete): the loop needs an operator — blocked/error.
    if let Some(halted) = read_small_text(&repo.join(".chug/loopd/HALTED"), 4096) {
        report.status = LoopStatus::Blocked;
        report.blocked_reason = Some(BlockedReason::Error);
        report.blocked_detail = Some(halted);
        return report;
    }
    // (3) + (4): the driver's event tail decides the run-segment state.
    let segment = segment_state(&events_tail(&repo.join(".chug/events.jsonl")));
    report.iteration = segment.iteration;
    if report.cycle_id.is_none() {
        report.cycle_id = segment.run_start_ts.clone();
    }
    if let Some((reason, detail)) = segment.terminal_block {
        report.status = LoopStatus::Blocked;
        report.blocked_reason = Some(reason);
        report.blocked_detail = Some(detail);
        return report;
    }
    if segment.completed {
        // The last run finished cleanly: between cycles (idle) unless no
        // supervisor will relaunch (dormant).
        report.status = if supervisor_alive(repo) {
            LoopStatus::Idle
        } else {
            LoopStatus::Dormant
        };
        return report;
    }
    if segment.fresh(now) {
        report.status = LoopStatus::Working;
        return report;
    }
    // (5) No driver evidence: the supervisor (or its absence) decides.
    report.status = if supervisor_alive(repo) && !open_todo_rows(repo).is_empty() {
        LoopStatus::Idle
    } else {
        LoopStatus::Dormant
    };
    report
}

/// The run-segment state read out of one bounded events tail (newest last).
struct SegmentState {
    /// The last `iteration` line's `n` in the window (0 when none — a
    /// long-running driver's earlier iteration lines can scroll out of the
    /// tail window; the count then understates, never overstates).
    iteration: u32,
    /// The tail's newest line timestamp — the freshness clock.
    last_ts: Option<u64>,
    /// The newest `run_start`'s ts string — the cycle_id fallback for repos
    /// without supervisor cycle logs.
    run_start_ts: Option<String>,
    /// The segment's terminal block, when the tail shows the run ended in an
    /// abort: (reason, detail).
    terminal_block: Option<(BlockedReason, String)>,
    /// The segment's terminal completion (goal accepted): not a block.
    completed: bool,
}

impl SegmentState {
    fn fresh(&self, now: u64) -> bool {
        self.last_ts
            .is_some_and(|ts| now.saturating_sub(ts) <= FRESH_DRIVER_SECS)
    }
}

/// Walk one events tail BACKWARD for the last run segment's state. The walk
/// stops at the first terminal line (abort / goal accepted) or the newest
/// `run_start` (an open segment); a goal REJECTION does not terminate — the
/// driver keeps iterating past a rejected goal gate. Unparseable lines are
/// skipped, so a truncated first line (the window cut) is harmless.
fn segment_state(lines: &[String]) -> SegmentState {
    let mut state = SegmentState {
        iteration: 0,
        last_ts: None,
        run_start_ts: None,
        terminal_block: None,
        completed: false,
    };
    for line in lines.iter().rev() {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if state.last_ts.is_none() {
            state.last_ts = value.get("ts").and_then(Value::as_str).and_then(parse_rfc3339);
        }
        match value.get("type").and_then(Value::as_str) {
            Some("iteration") => {
                if state.iteration == 0 {
                    state.iteration = value
                        .get("n")
                        .and_then(Value::as_u64)
                        .unwrap_or(0)
                        .min(u32::MAX as u64) as u32;
                }
            }
            Some("abort") => {
                let reason = value
                    .get("reason")
                    .and_then(Value::as_str)
                    .unwrap_or("aborted");
                state.terminal_block = Some(match value.get("budget_kind").and_then(Value::as_str) {
                    Some(kind) => (
                        BlockedReason::Budget,
                        format!(
                            "{kind} budget exhausted (max {}): {reason}",
                            value.get("budget_max").and_then(Value::as_u64).unwrap_or(0)
                        ),
                    ),
                    None => (BlockedReason::Error, reason.to_string()),
                });
                break;
            }
            Some("goal") if value.get("outcome").and_then(Value::as_str) == Some("accepted") => {
                state.completed = true;
                break;
            }
            Some("run_start") => {
                state.run_start_ts = value.get("ts").and_then(Value::as_str).map(str::to_string);
                break;
            }
            _ => {}
        }
    }
    state
}

/// The supervisor's liveness: the pid in `.chug/loopd/loopd.pid` names a live
/// process. Missing, unparseable, or dead → false (the loop is not being
/// supervised).
fn supervisor_alive(repo: &Path) -> bool {
    read_small_text(&repo.join(".chug/loopd/loopd.pid"), 64)
        .and_then(|text| text.lines().next()?.trim().parse::<u32>().ok())
        .is_some_and(crate::driver_lock::pid_alive)
}

/// The cycle id: the newest supervisor cycle log's stem (`cycle-<ts>`), else
/// the segment's `run_start` ts (a repo without supervisor logs still has a
/// current run), else None.
fn cycle_id(repo: &Path) -> Option<String> {
    let mut newest: Option<String> = None;
    if let Ok(entries) = std::fs::read_dir(repo.join(".chug/loopd")) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !(name.starts_with("cycle-") && name.ends_with(".log")) {
                continue;
            }
            let stem = name.trim_end_matches(".log").to_string();
            if newest.as_ref().is_none_or(|best| stem > *best) {
                newest = Some(stem);
            }
        }
    }
    newest
}

/// The item the loop is working: the newest run's `spec` when it names a
/// `tNNN` spec file, else the newest harvested `LEDGER-tNNN-*` /
/// `verdict-tNNN-*` file's item, else None (a pilot repo's queue may not use
/// the convention at all — absent, never fabricated).
fn item_id(repo: &Path) -> Option<String> {
    let spec_item = events_tail(&repo.join(".chug/events.jsonl"))
        .iter()
        .rev()
        .find_map(|line| {
            let value = serde_json::from_str::<Value>(line).ok()?;
            let spec = value.get("spec")?.as_str()?;
            item_id_from_spec(spec)
        });
    spec_item.or_else(|| item_id_from_ledgers(repo))
}

/// `specs/t340-chugd-typed-status.md` (or a bare `t340-name.md`) → `T340`.
/// The digits must run to the stem's end or a `-`/`_` separator (the
/// convention) — `t9x` is not an item. Anything else (LOOP-SPEC.md, a
/// pilot's custom name) → None.
fn item_id_from_spec(spec: &str) -> Option<String> {
    let name = spec.rsplit('/').next()?;
    let stem = name.strip_suffix(".md").unwrap_or(name);
    let rest = stem.strip_prefix('t').or_else(|| stem.strip_prefix('T'))?;
    let digits_end = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(rest.len());
    if digits_end == 0 {
        return None;
    }
    let digits = &rest[..digits_end];
    let tail_ok = rest[digits_end..]
        .chars()
        .next()
        .is_none_or(|c| c == '-' || c == '_');
    tail_ok.then(|| format!("T{digits}"))
}

/// The newest `.chug/LEDGER-tNNN-*` / `.chug/verdict-tNNN-*` file's item, by
/// modification time (the harvest artifacts land in the loop repo's .chug as
/// a cycle closes, so the newest names the current item).
fn item_id_from_ledgers(repo: &Path) -> Option<String> {
    let mut newest: Option<(std::time::SystemTime, String)> = None;
    let entries = std::fs::read_dir(repo.join(".chug")).ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let stem = if name.starts_with("LEDGER-t") {
            name.strip_prefix("LEDGER-")
        } else if name.starts_with("verdict-t") {
            name.strip_prefix("verdict-")
        } else {
            None
        };
        let Some(rest) = stem else { continue };
        let digits: String = rest
            .strip_prefix('t')
            .unwrap_or(rest)
            .chars()
            .take_while(char::is_ascii_digit)
            .collect();
        if digits.is_empty() {
            continue;
        }
        let Ok(modified) = entry.metadata().and_then(|meta| meta.modified()) else {
            continue;
        };
        if newest.as_ref().is_none_or(|(best, _)| modified > *best) {
            newest = Some((modified, format!("T{digits}")));
        }
    }
    newest.map(|(_, item)| item)
}

/// The repo's registered worktrees (the children), read straight from the
/// git dir's worktree admin area — pure bounded file reads, no subprocess.
/// The main worktree IS the repo, so it is never listed; a linked-worktree
/// repo (its `.git` is a file naming the main admin dir) lists its siblings
/// and itself.
fn status_children(repo: &Path) -> Vec<StatusChild> {
    let Some(worktrees_dir) = git_worktrees_dir(repo) else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(&worktrees_dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names.truncate(MAX_CHILDREN);
    names
        .into_iter()
        .map(|name| {
            let dir = worktrees_dir.join(&name);
            // The branch rides HEAD ("ref: refs/heads/loop-t302-validate");
            // a raw sha is a detached worktree.
            let head = read_small_text(&dir.join("HEAD"), 256).unwrap_or_default();
            let branch = head.trim().strip_prefix("ref: ").unwrap_or("");
            let state = if dir.join("locked").exists() {
                "locked"
            } else if dir.join("prunable").exists() {
                "prunable"
            } else if branch.is_empty() {
                "detached"
            } else {
                "ready"
            };
            // The loop's lane naming: validate first (the more specific
            // marker), then impl; anything else is a plain worktree.
            let label = format!("{name} {branch}");
            let kind = if label.contains("validate") {
                "validate"
            } else if label.contains("impl") {
                "impl"
            } else {
                "worktree"
            };
            StatusChild {
                kind,
                id: name,
                state: state.to_string(),
            }
        })
        .collect()
}

/// The git dir's worktree admin dir for `repo`: `<repo>/.git/worktrees/`
/// when `.git` is a directory; when `.git` is a FILE (a linked worktree) its
/// `gitdir:` line names `<main>/.git/worktrees/<name>` and the parent of
/// that is the admin dir. Non-git repos have none.
fn git_worktrees_dir(repo: &Path) -> Option<PathBuf> {
    let dot_git = repo.join(".git");
    if dot_git.is_dir() {
        return Some(dot_git.join("worktrees"));
    }
    if dot_git.is_file() {
        let text = read_small_text(&dot_git, 4096)?;
        let target = text.trim().strip_prefix("gitdir:")?.trim();
        return Path::new(target).parent().map(Path::to_path_buf);
    }
    None
}

/// One row per open TODO item (the loop's own open-row rule: the status cell
/// is `todo` or `open`, plus a `blocked` cell so a queue-level blocked row
/// surfaces). The title echoes verbatim; `pri` parses when it can; a row
/// blocked at the queue level (a `blocked` status cell, or notes opening
/// with `BLOCKED` — the T328 shape) carries `blocked_reason`.
fn open_todo_rows(repo: &Path) -> Vec<Value> {
    let Some(text) = read_small_text(&repo.join("TODO.md"), MAX_TODO_BYTES) else {
        return Vec::new();
    };
    let mut items = Vec::new();
    for line in text.lines() {
        if !line.starts_with('|') {
            continue;
        }
        // | id | title | spec | pri | status | notes | → cells[1..=6].
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        if cells.len() < 7 {
            continue;
        }
        let (id, title, pri, status, notes) =
            (cells[1], cells[2], cells[4], cells[5], cells[6]);
        if !is_todo_id(id) {
            continue; // header/separator rows and anything unkeyed
        }
        if !matches!(status, "todo" | "open" | "blocked") {
            continue;
        }
        let mut row = json!({"id": id, "title": title, "status": status});
        if let Ok(parsed) = pri.parse::<u64>() {
            row["pri"] = json!(parsed);
        }
        if let Some(reason) = row_blocked_reason(notes, status) {
            row["blocked_reason"] = json!(reason);
        }
        items.push(row);
        if items.len() >= MAX_TODO_ROWS {
            break;
        }
    }
    items
}

/// A TODO id: `T`/`P` + digits (the queue's real rows; the header, the
/// separator, and prose tables never match).
fn is_todo_id(id: &str) -> bool {
    let mut chars = id.chars();
    matches!(chars.next(), Some('T') | Some('P')) && !id[1..].is_empty() && id[1..].bytes().all(|b| b.is_ascii_digit())
}

/// The queue-level blocked reason: a `blocked` status cell, or notes opening
/// with `BLOCKED`. `BLOCKED (<keyword>` names the vocabulary member when the
/// row says so; a queue row's blocker defaults to `external` — it lives
/// outside the row itself (the T328 class: the harness axis, not the code).
fn row_blocked_reason(notes: &str, status: &str) -> Option<&'static str> {
    let after = if status == "blocked" {
        notes.trim()
    } else {
        notes.trim().strip_prefix("BLOCKED")?.trim_start()
    };
    let paren = after.strip_prefix('(').unwrap_or(after);
    for (keyword, reason) in [
        ("permission", "permission"),
        ("question", "question"),
        ("budget", "budget"),
        ("error", "error"),
    ] {
        if paren.starts_with(keyword) {
            return Some(reason);
        }
    }
    Some("external")
}

/// A small regular file's text, bounded: anything that is not a regular file
/// (a directory, a FIFO, a device) or past the cap reads as absent — the
/// derivation degrades, it never blocks and never errors.
fn read_small_text(path: &Path, cap: u64) -> Option<String> {
    use std::io::Read;
    let meta = std::fs::metadata(path).ok()?;
    if !meta.is_file() {
        return None;
    }
    let mut file = std::fs::File::open(path).ok()?;
    let mut raw = Vec::new();
    (&mut file)
        .take(cap)
        .read_to_end(&mut raw)
        .ok()?;
    let text = String::from_utf8_lossy(&raw);
    let text = text.trim_end();
    (!text.is_empty()).then(|| text.to_string())
}

/// The bounded events tail: the last [`EVENTS_TAIL_BYTES`] of the driver's
/// JSONL event log, newest last. The window's first line may be a partial —
/// it is dropped (the walk skips unparseable lines anyway, but a partial
/// JSON line that HAPPENS to parse would be a lie). Missing/empty log → an
/// empty tail (no driver evidence at all).
fn events_tail(path: &Path) -> Vec<String> {
    use std::io::{Read, Seek, SeekFrom};
    let Ok(meta) = std::fs::metadata(path) else {
        return Vec::new();
    };
    if !meta.is_file() {
        return Vec::new();
    }
    let len = meta.len();
    let start = len.saturating_sub(EVENTS_TAIL_BYTES);
    let Ok(mut file) = std::fs::File::open(path) else {
        return Vec::new();
    };
    if file.seek(SeekFrom::Start(start)).is_err() {
        return Vec::new();
    }
    let mut raw = Vec::new();
    if (&mut file).take(EVENTS_TAIL_BYTES).read_to_end(&mut raw).is_err() {
        return Vec::new();
    }
    let text = String::from_utf8_lossy(&raw);
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    if start > 0 {
        lines.remove(0); // the window cut: the first line may be partial
    }
    lines.retain(|line| !line.trim().is_empty());
    lines
}

/// The status endpoints' repo selector, in precedence order: the
/// `?repo=<path>` query parameter (percent-decoded), then `CHUG_DAEMON_REPO`
/// (the serve-time pin), then the daemon process's cwd (the loop repo a
/// spawned daemon inherits).
fn status_repo(raw_path: &str) -> PathBuf {
    raw_path
        .split_once('?')
        .and_then(|(_, query)| query_param(query, "repo"))
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os(REPO_ENV)
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
        })
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."))
}

/// The typed status path a raw request line carries (`None` = not a status
/// route). The legacy routes above keep matching the RAW path, so a query
/// string on /healthz, /judge, or /sessions stays a 404 exactly as pre-T340.
fn status_route(raw: &str) -> Option<&'static str> {
    match raw.split('?').next().unwrap_or(raw) {
        "/status/loop" => Some("/status/loop"),
        "/status/items" => Some("/status/items"),
        _ => None,
    }
}

/// One query parameter, percent-decoded (`%XX` triplets; `+` reads as a
/// space — the form encoding curl's `--data-urlencode` produces). A malformed
/// `%` is literal; undecodable bytes render lossily.
fn query_param(query: &str, key: &str) -> Option<String> {
    query
        .split('&')
        .find_map(|pair| {
            let (name, value) = pair.split_once('=')?;
            (percent_decode(name) == key).then(|| percent_decode(value))
        })
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'+' {
            out.push(b' ');
            i += 1;
        } else if bytes[i] == b'%' && i + 2 < bytes.len() {
            let decoded = std::str::from_utf8(&bytes[i + 1..i + 3])
                .ok()
                .and_then(|hex| u8::from_str_radix(hex, 16).ok());
            match decoded {
                Some(byte) => {
                    out.push(byte);
                    i += 3;
                }
                None => {
                    out.push(b'%');
                    i += 1;
                }
            }
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The stub's refusal shape for /status — exactly the stub /judge and
/// /sessions refusal (non-2xx + `{"detail": …}`): the stub seam exists for
/// the transport/lifecycle pins and serves /healthz only. It has no repo
/// context to report, and a fabricated empty-but-200 status would be
/// exactly the lie the stub must never tell.
fn stub_status_refusal() -> (u16, String) {
    (
        500,
        detail("CHUG_DAEMON_STUB test backend: /status is never served by the stub"),
    )
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
    // T340: the typed status endpoints ride every real serve mode the same
    // way (they need no model — pure observation of the loop's state files);
    // the stub refuses them exactly as it refuses /sessions.
    let serve_status = !stub_requested();
    serve_unix_with(&sock_path()?, backend, sessions, serve_status)
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
    // T222: a kev-layout checkpoint (adapter + head — the kev family)
    // routes to the kev loader, which resolves, pin-verifies and
    // classifies the base, then refuses with the precise reason (the
    // confirmed 0.8B sits on a Gated DeltaNet hybrid candle 0.11 cannot
    // load). Routing here — instead of letting the RLAgent loader 404 on
    // model.safetensors — is what makes the refusal name the right gap
    // instead of a misleading layout error; gating behavior is unchanged
    // and no traffic flows (SPEC-3).
    if crate::kev_model::is_kev_layout(&spec)? {
        let insp = crate::kev_model::inspect(&spec)?;
        bail!(
            "chug daemon: the kev-layout checkpoint {spec:?} cannot serve yet: {}",
            crate::kev_model::refusal(&insp)
        );
    }
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
    serve_unix_with(sock, backend, Some(SessionsHost::default()), true)
}

/// The parameterized serve: `sessions` carries the registry (and the
/// daemon's own identity); `None` refuses /sessions (the stub serve — a
/// stub must never fabricate a registry). `serve_status` gates the T340
/// typed status endpoints the same way.
fn serve_unix_with(
    sock: &Path,
    backend: std::sync::Arc<dyn JudgeBackend>,
    sessions: Option<SessionsHost>,
    serve_status: bool,
) -> anyhow::Result<i32> {
    #[cfg(unix)]
    {
        let listener = bind_socket(sock).with_context(|| format!("binding {}", sock.display()))?;
        eprintln!(
            "chug daemon: serving /healthz + /judge{}{} on {} (mode 0600) — {}",
            if sessions.is_some() { " + /sessions" } else { "" },
            if serve_status { " + /status" } else { "" },
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
                        .spawn(move || handle_conn(backend, sessions, serve_status, stream))
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
        let _ = (sock, backend, sessions, serve_status);
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
    serve_status: bool,
    mut stream: std::os::unix::net::UnixStream,
) {
    let _ = stream.set_read_timeout(Some(JUDGE_TIMEOUT));
    let _ = stream.set_write_timeout(Some(JUDGE_TIMEOUT));
    let (status, body) = match read_request(&mut stream) {
        Ok(request) => route(&*backend, sessions.as_ref(), serve_status, &request),
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
    serve_status: bool,
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
        // T340: the typed status endpoints — observation only, sourced from
        // the loop's state files. The legacy routes above match the RAW
        // path, so they are untouched (a query string on them stays a 404);
        // only the status routes carry a `?repo=` selector.
        ("GET", raw) if status_route(raw).is_some() => {
            if !serve_status {
                stub_status_refusal()
            } else {
                let repo = status_repo(raw);
                match status_route(raw) {
                    Some("/status/loop") => (200, status_loop_body(&repo, epoch_now())),
                    _ => (200, status_items_body(&repo, epoch_now())),
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
    /// Confirmation window for a transient connect SUCCESS (T233): how long
    /// one success must SUSTAIN before it names a genuine listener. Rides
    /// the existing backoff scale — exactly 40 classification-backoff steps
    /// (1s at today's 25ms cadence), so the two constants move together and
    /// no unscaled wait is introduced; no existing constant's value changes.
    /// A macOS teardown artifact (one connect completing from a just-closed
    /// listener's pending-accept backlog, then refusing once it drains)
    /// re-verifies REFUSED inside this window; a real rogue listener keeps
    /// answering across it and is panicked on fast (~1s, not the deadline).
    const STALE_CLASSIFY_CONFIRM_WINDOW: Duration =
        Duration::from_millis(STALE_CLASSIFY_BACKOFF.as_millis() as u64 * 40);

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
    ///
    /// T233: a transient connect SUCCESS is a NON-VERDICT too, not an
    /// instant red. The organic sighting (the t232-VALIDATE goal gate,
    /// failed 1231/1 — "connect to the dead socket SUCCEEDED" — then green
    /// 5/5 isolated and 1232/1232 on the identical-bytes re-run, same
    /// binary hash, diff-untouched module) is the teardown shape the racer
    /// above names: on macOS a connect to a just-closed listener can
    /// complete from the kernel's pending-accept backlog, then refuses once
    /// the backlog drains (T151 proved the kernel shape for TCP in
    /// mcp_http's `Invalidation::TeardownArtifact`). So the Ok arm no
    /// longer panics on the spot: it re-verifies within
    /// [`STALE_CLASSIFY_CONFIRM_WINDOW`] via
    /// [`confirm_transient_success`] — success-then-REFUSED = teardown
    /// artifact (recorded, keep polling to the deadline); success SUSTAINED
    /// across the window = a genuine listener answers — panic naming BOTH
    /// classes, so a real rogue listener still dies, fast.
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
                // T233: ONE success is a non-verdict (macOS pending-accept
                // backlog completion during teardown) — re-verify inside a
                // short confirmation window instead of panicking. Same
                // shape as the error arms: bounded, backoff-paced, and the
                // outer deadline still governs the total.
                Ok(_) => {
                    if confirm_transient_success(
                        || UnixStream::connect(&sock),
                        STALE_CLASSIFY_CONFIRM_WINDOW,
                    ) {
                        eprintln!(
                            "stale_socket: transient connect success on {} re-verified REFUSED \
                             within {STALE_CLASSIFY_CONFIRM_WINDOW:?} — listener-teardown \
                             backlog artifact (macOS pending-accept completion), not a \
                             listener; continuing the poll",
                            sock.display()
                        );
                    } else {
                        eprintln!(
                            "stale_socket: connect-success confirmation unresolved within \
                             {STALE_CLASSIFY_CONFIRM_WINDOW:?} (ambiguous transients only) — \
                             falling back to the outer poll"
                        );
                    }
                    // Bounded by the OUTER deadline no matter how the
                    // confirmation resolved: a path that keeps handing out
                    // successes re-arms the window, so check the deadline
                    // here too, naming the shape.
                    assert!(
                        Instant::now() < deadline,
                        "the dead socket never settled to ECONNREFUSED within \
                         {STALE_CLASSIFY_DEADLINE:?} (transient connect successes kept \
                         re-arming the confirmation window)"
                    );
                }
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

    /// T233: the bounded re-verify behind
    /// [`stale_socket_connects_refused`]'s Ok arm — the transient-SUCCESS
    /// classification, same shape as the error arms (bounded, backoff-paced
    /// on [`STALE_CLASSIFY_BACKOFF`], window-parameterized so the pins run
    /// fast while the real leg rides [`STALE_CLASSIFY_CONFIRM_WINDOW`]).
    ///
    /// Decision table (T151's "first-succeeds-then-refuses = artifact;
    /// sustained-live = genuine listener" rule, ported to the UDS path):
    /// - a re-verify connect REFUSES inside the window → the first success
    ///   completed from a just-closed listener's pending-accept backlog (a
    ///   teardown artifact, NOT a listener) → `Ok(true)` — recorded by the
    ///   caller and the outer poll continues to the deadline;
    /// - a connect SUCCEEDS at/after the window → the success is SUSTAINED
    ///   → a GENUINE listener answers → panic naming BOTH classes (a real
    ///   rogue listener still dies, fast — ~1 window, not the deadline);
    /// - only ambiguous errors (EMFILE/ENOENT-class) through the window →
    ///   `Ok(false)` — unresolved, a NON-VERDICT: the caller falls back to
    ///   the outer poll (no new red leg — ambiguity is exactly what the
    ///   error arms retry).
    #[cfg(unix)]
    fn confirm_transient_success(
        mut connect: impl FnMut() -> std::io::Result<std::os::unix::net::UnixStream>,
        window: Duration,
    ) -> bool {
        let confirm_deadline = Instant::now() + window;
        loop {
            match connect() {
                // Success-then-REFUSED: the backlog drained — teardown
                // artifact, not a listener. A non-verdict, recorded.
                Err(e) if e.raw_os_error() == Some(libc::ECONNREFUSED) => return true,
                // Sustained success: at/after the window a listener is
                // STILL answering. Panic naming BOTH classes so a future
                // red self-diagnoses (what a sustained listener means vs
                // what a transient backlog artifact was).
                Ok(_) => {
                    assert!(
                        Instant::now() < confirm_deadline,
                        "connect to the dead socket KEPT SUCCEEDING for {window:?} — a GENUINE \
                         listener is answering (a real rogue listener/daemon holding the stale \
                         path: this must stay RED), not the transient teardown artifact the \
                         first success suggested — that artifact re-verifies REFUSED inside \
                         the window (on macOS one connect can complete from a just-closed \
                         listener's pending-accept backlog, then refuses once it drains)"
                    );
                    std::thread::sleep(STALE_CLASSIFY_BACKOFF);
                }
                // Ambiguous (EMFILE/ENOENT-class): a non-verdict — poll
                // inside the window only; expiry on ambiguity falls back to
                // the outer poll rather than reding here.
                Err(_) => {
                    if Instant::now() >= confirm_deadline {
                        return false;
                    }
                    std::thread::sleep(STALE_CLASSIFY_BACKOFF);
                }
            }
        }
    }

    /// T233: the transient-success re-verify's decision table, scripted
    /// deterministically (the REAL macOS race is microsecond-scale and
    /// cannot be staged deterministically — same filing bar as T151's
    /// classification pin, which scripts the classifier's inputs for the
    /// same reason). The connect seam is scripted; the confirm mechanics
    /// (window, backoff pacing, the success-panics-sustained rule) run for
    /// real. Non-vacuousness: the pre-T233 shape — instant panic on the
    /// first Ok — dies on leg 1; a masked-listener mutant (artifact-tolerant
    /// forever) dies on leg 2.
    #[cfg(unix)]
    #[test]
    fn stale_socket_transient_success_confirmation_pins() {
        use std::os::unix::net::UnixStream;

        // 50ms window: two backoff steps at the 25ms cadence — fast pins
        // with the real confirm mechanics (the REAL leg rides
        // STALE_CLASSIFY_CONFIRM_WINDOW = 40 backoff steps).
        let window = Duration::from_millis(50);
        let refused = || std::io::Error::from_raw_os_error(libc::ECONNREFUSED);
        let transient = || std::io::Error::from_raw_os_error(libc::EMFILE);

        // Leg 1 (THE FIX): one scripted success, then ECONNREFUSED — the
        // teardown artifact — resolves as artifact (keep polling) without
        // panicking or exhausting anything. The pre-T233 shape panics on
        // the first Ok and cannot pass this leg.
        let pair = UnixStream::pair().expect("socketpair for the scripted success");
        let steps: Vec<std::io::Result<UnixStream>> =
            vec![Ok(pair.0), Err(refused())];
        let mut steps = steps.into_iter();
        let artifact = confirm_transient_success(
            || {
                steps
                    .next()
                    .unwrap_or_else(|| Err(refused()))
            },
            window,
        );
        assert!(
            artifact,
            "success-then-refused must classify the teardown artifact (record + keep polling), \
             not red and not exhaust"
        );

        // Leg 2 (NON-VACUOUSNESS): success SUSTAINED across the window — a
        // genuine listener answers — must panic, naming BOTH classes (what
        // a sustained listener means vs what a transient backlog artifact
        // was), so the artifact tolerance can never mask a real listener
        // into green.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            confirm_transient_success(
                || UnixStream::pair().map(|(s, _)| s),
                window,
            )
        }));
        let err = result.expect_err("a sustained listener must fail the leg, not pass green");
        let msg = err
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| err.downcast_ref::<&'static str>().copied())
            .expect("panic payload is a string");
        assert!(
            msg.contains("GENUINE listener") && msg.contains("pending-accept backlog"),
            "the sustained panic must name BOTH classes (genuine listener vs teardown \
             artifact) so a future red is self-diagnosing: {msg}"
        );

        // Leg 3 (no new red leg): ambiguous transients only, through the
        // window — unresolved, a NON-VERDICT: the caller falls back to the
        // outer poll (where the error arms keep retrying). Ambiguity must
        // not red through the confirmation.
        let unresolved = confirm_transient_success(|| Err(transient()), window);
        assert!(
            !unresolved,
            "ambiguity-only confirmation must fall back to the outer poll, not classify"
        );
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

    /// T221 — the TTL boundary pins the DEFAULT at 600 exactly, in literal
    /// seconds: a 599s-old entry LIVES and a 601s-old one is EVICTED. The
    /// symbolic `SESSION_TTL_SECS` legs above cannot see a const flip (they
    /// move with it); these literal legs fail if the default is tuned to
    /// anything but 600.
    #[test]
    fn session_ttl_boundary_pins_default_at_600() {
        let now = 1_000_000_000u64;
        let mut registry = SessionRegistry::default();
        registry.upsert(session_entry("under", now - 599));
        registry.upsert(session_entry("over", now - 601));
        let live = registry.evict_live(now);
        let ids: Vec<&str> = live.iter().map(|entry| entry.id.as_str()).collect();
        assert!(
            ids.contains(&"under"),
            "a 599s-old entry lives under the 600s TTL: {ids:?}"
        );
        assert!(
            !ids.contains(&"over"),
            "a 601s-old entry is evicted past the 600s TTL: {ids:?}"
        );
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
        // Rejects: junk, bad shapes, out-of-range fields. Day 0 rejects
        // exactly like day 32 (the T221 guard — no days_from_civil
        // underflow on either side of the month's day range).
        for text in [
            "", "not a time", "2024-02-29", "2024-02-29T12:00", "24-02-29T12:00:00Z",
            "2024-13-01T00:00:00Z", "2024-02-30T00:00:00Z", "2024-02-29T24:00:00Z",
            "2024-02-29T12:60:00Z", "2024-02-29T12:00:00Z.", "2024-02-29T12:00:00X",
            "2024-02-29T12:00:00+99:00",
            "2024-03-00T12:00:00Z", "2024-02-00T00:00:00Z", "2024-03-32T00:00:00Z",
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
                true,
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
            let _ = serve_unix_with(&stub_server_sock, Arc::new(StubBackend), None, false);
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

    // -------------------------------------------------------------------
    // T340 — the typed status endpoints: derivation + wire-shape units.
    // The full wire battery (fixture repos, a real spawned daemon, the
    // capabilities flips) lives in tests/daemon_status_contract.rs; these
    // units pin the pure edges.
    // -------------------------------------------------------------------

    fn write_small(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create parent dirs");
        }
        std::fs::write(path, contents).expect("write fixture file");
    }

    #[test]
    fn status_route_matches_only_the_typed_paths() {
        assert_eq!(status_route("/status/loop"), Some("/status/loop"));
        assert_eq!(status_route("/status/loop?repo=%2Ftmp"), Some("/status/loop"));
        assert_eq!(status_route("/status/items"), Some("/status/items"));
        assert_eq!(status_route("/status/items?x=1"), Some("/status/items"));
        assert_eq!(status_route("/status/loopy"), None);
        assert_eq!(status_route("/status"), None);
        assert_eq!(status_route("/healthz"), None);
        // POST /status/loop falls through to the 404 arm: the guard only
        // arms on ("GET", raw).
        assert_eq!(status_route("/status/loop/extra"), None);
    }

    #[test]
    fn query_param_and_percent_decode_handle_paths_plus_and_malformed() {
        assert_eq!(
            query_param("repo=%2Ftmp%2Fchug-loop-t1", "repo"),
            Some("/tmp/chug-loop-t1".to_string())
        );
        assert_eq!(
            query_param("repo=%2Ftmp%2Fmy+repo", "repo"),
            Some("/tmp/my repo".to_string())
        );
        assert_eq!(query_param("repo=%2Ftmp&x=1", "repo"), Some("/tmp".to_string()));
        assert_eq!(query_param("x=1&repo=%2Ftmp", "repo"), Some("/tmp".to_string()));
        assert_eq!(query_param("x=1", "repo"), None);
        assert_eq!(query_param("repo", "repo"), None);
        // A malformed % is literal; the selector still resolves.
        assert_eq!(percent_decode("100%"), "100%");
        assert_eq!(percent_decode("%ZZ"), "%ZZ");
        assert_eq!(percent_decode("a+b%2Fc"), "a b/c");
        // The ?repo= selector decodes a real absolute path end to end.
        assert_eq!(
            status_repo("/status/loop?repo=%2Ftmp%2Fchug-loop-t340"),
            PathBuf::from("/tmp/chug-loop-t340")
        );
    }

    #[test]
    fn loop_report_missing_repo_is_blocked_external_and_names_the_path() {
        let repo = Path::new("/tmp/chug-t340-does-not-exist-anywhere");
        let _ = std::fs::remove_dir_all(repo);
        let report = loop_report(repo, epoch_now());
        assert_eq!(report.status, LoopStatus::Blocked);
        assert_eq!(report.blocked_reason, Some(BlockedReason::External));
        let detail = report.blocked_detail.expect("the external case carries its detail");
        assert!(detail.contains("/tmp/chug-t340-does-not-exist-anywhere"), "{detail}");
        assert!(report.children.is_empty());
        assert_eq!(report.item_id, None);
    }

    #[test]
    fn loop_report_halted_supervisor_is_blocked_error_with_the_latch_detail() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let reason = "3 consecutive cycles without goal_complete — manual review required";
        write_small(&tmp.path().join(".chug/loopd/HALTED"), reason);
        let report = loop_report(tmp.path(), epoch_now());
        assert_eq!(report.status, LoopStatus::Blocked);
        assert_eq!(report.blocked_reason, Some(BlockedReason::Error));
        assert_eq!(report.blocked_detail.as_deref(), Some(reason));
    }

    #[test]
    fn loop_report_repo_with_no_evidence_is_dormant_with_empty_children() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let report = loop_report(tmp.path(), epoch_now());
        assert_eq!(report.status, LoopStatus::Dormant);
        assert_eq!(report.blocked_reason, None);
        assert_eq!(report.blocked_detail, None);
        assert!(report.children.is_empty());
        assert_eq!(report.iteration, 0);
    }

    #[test]
    fn segment_state_reads_budget_abort_plain_abort_and_completion() {
        let ts = rfc3339(epoch_now());
        let budget = format!(
            r#"{{"type":"abort","ts":"{ts}","reason":"hit the iteration ceiling","budget_kind":"iterations","budget_max":80}}"#
        );
        let state = segment_state(&[budget]);
        assert_eq!(state.iteration, 0);
        let (reason, detail) = state.terminal_block.expect("budget abort terminates");
        assert_eq!(reason, BlockedReason::Budget);
        assert_eq!(
            detail,
            "iterations budget exhausted (max 80): hit the iteration ceiling"
        );
        assert!(!state.completed);

        let plain = format!(r#"{{"type":"abort","ts":"{ts}","reason":"checkpoint unload failed"}}"#);
        let state = segment_state(&[plain]);
        let (reason, detail) = state.terminal_block.expect("plain abort terminates");
        assert_eq!(reason, BlockedReason::Error);
        assert_eq!(detail, "checkpoint unload failed");

        let accepted = format!(r#"{{"type":"goal","ts":"{ts}","outcome":"accepted"}}"#);
        let state = segment_state(&[accepted]);
        assert!(state.completed, "a goal acceptance completes the segment");
        assert!(state.terminal_block.is_none());

        // A goal REJECTION does not terminate — the driver iterates past a
        // rejected gate; the segment stays open.
        let rejected = format!(r#"{{"type":"goal","ts":"{ts}","outcome":"rejected"}}"#);
        let state = segment_state(&[rejected]);
        assert!(!state.completed);
        assert!(state.terminal_block.is_none());
    }

    #[test]
    fn segment_state_walk_stops_at_the_newest_run_start_and_reads_iteration() {
        let old = rfc3339(epoch_now().saturating_sub(3600));
        let fresh = rfc3339(epoch_now());
        // An abort in the PREVIOUS segment (before the newest run_start) must
        // not block the current one: the walk hits run_start first.
        let lines = vec![
            format!(r#"{{"type":"run_start","ts":"{old}","model":"glm"}}"#),
            format!(r#"{{"type":"iteration","ts":"{old}","n":80}}"#),
            format!(r#"{{"type":"abort","ts":"{old}","reason":"budget","budget_kind":"iterations","budget_max":80}}"#),
            format!(r#"{{"type":"run_start","ts":"{fresh}","model":"glm"}}"#),
            format!(r#"{{"type":"iteration","ts":"{fresh}","n":3}}"#),
            format!(r#"{{"type":"iteration","ts":"{fresh}","n":4}}"#),
        ];
        let state = segment_state(&lines);
        assert!(state.terminal_block.is_none(), "the old abort is behind the boundary");
        assert!(!state.completed);
        assert_eq!(state.iteration, 4, "the newest iteration line's n");
        assert_eq!(state.run_start_ts.as_deref(), Some(fresh.as_str()));
        assert!(state.fresh(epoch_now()), "a fresh tail reads as working");
        // Garbage and partial lines are skipped, not fatal.
        let state = segment_state(&["{oops".to_string(), lines[4].clone()]);
        assert_eq!(state.iteration, 3);
    }

    #[test]
    fn segment_state_stale_tail_is_not_fresh() {
        let stale = rfc3339(epoch_now().saturating_sub(FRESH_DRIVER_SECS + 30));
        let lines = vec![
            format!(r#"{{"type":"run_start","ts":"{stale}"}}"#),
            format!(r#"{{"type":"iteration","ts":"{stale}","n":7}}"#),
        ];
        let state = segment_state(&lines);
        assert!(!state.fresh(epoch_now()));
        assert_eq!(state.iteration, 7);
    }

    #[test]
    fn loop_report_fresh_segment_is_working_and_carries_item_and_iteration() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let now = epoch_now();
        let ts = rfc3339(now);
        write_small(
            &tmp.path().join(".chug/events.jsonl"),
            &concat!(
                    r#"{"type":"run_start","ts":"{TS}","spec":"specs/t341-permission-broker-hook.md"}"#,
                    "\n",
                    r#"{"type":"iteration","ts":"{TS}","n":2}"#,
                    "\n"
                )
                .replace("{TS}", &ts),
        );
        write_small(
            &tmp.path().join("TODO.md"),
            "| id | title | spec | pri | status | notes |\n|----|-------|------|-----|--------|-------|\n| T341 | x | specs/t341.md | 2 | todo | |\n",
        );
        let report = loop_report(tmp.path(), now);
        assert_eq!(report.status, LoopStatus::Working);
        assert_eq!(report.iteration, 2);
        assert_eq!(report.item_id.as_deref(), Some("T341"));
        assert_eq!(report.cycle_id.as_deref(), Some(ts.as_str()), "run_start ts is the cycle fallback");
    }

    #[test]
    fn loop_report_completed_run_is_idle_with_a_live_supervisor() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let now = epoch_now();
        let ts = rfc3339(now);
        write_small(
            &tmp.path().join(".chug/events.jsonl"),
            &format!(r#"{{"type":"goal","ts":"{ts}","outcome":"accepted"}}"#),
        );
        // This test process is alive: a valid stand-in supervisor pid.
        write_small(
            &tmp.path().join(".chug/loopd/loopd.pid"),
            &format!("{}", std::process::id()),
        );
        write_small(
            &tmp.path().join("TODO.md"),
            "| id | title | spec | pri | status | notes |\n|----|-------|------|-----|--------|-------|\n| T340 | x | specs/t340.md | 2 | open | |\n",
        );
        let report = loop_report(tmp.path(), now);
        assert_eq!(report.status, LoopStatus::Idle);
        // Without a live supervisor the same completion reads dormant.
        write_small(&tmp.path().join(".chug/loopd/loopd.pid"), "999999999");
        let report = loop_report(tmp.path(), now);
        assert_eq!(report.status, LoopStatus::Dormant);
    }

    #[test]
    fn loop_report_budget_abort_surfaces_blocked_budget() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let stale = rfc3339(epoch_now().saturating_sub(FRESH_DRIVER_SECS + 10));
        write_small(
            &tmp.path().join(".chug/events.jsonl"),
            &concat!(
                    r#"{"type":"run_start","ts":"{TS}","spec":"specs/t340-chugd-typed-status.md"}"#,
                    "\n",
                    r#"{"type":"iteration","ts":"{TS}","n":50}"#,
                    "\n",
                    r#"{"type":"abort","ts":"{TS}","reason":"wrap unfinished","budget_kind":"iterations","budget_max":50}"#,
                    "\n"
                )
                .replace("{TS}", &stale),
        );
        let report = loop_report(tmp.path(), epoch_now());
        assert_eq!(report.status, LoopStatus::Blocked);
        assert_eq!(report.blocked_reason, Some(BlockedReason::Budget));
        assert_eq!(
            report.blocked_detail.as_deref(),
            Some("iterations budget exhausted (max 50): wrap unfinished")
        );
        assert_eq!(report.item_id.as_deref(), Some("T340"));
    }

    #[test]
    fn item_id_from_spec_handles_the_convention_and_declines_others() {
        assert_eq!(
            item_id_from_spec("specs/t340-chugd-typed-status.md"),
            Some("T340".to_string())
        );
        assert_eq!(item_id_from_spec("t324-one-liner-anchor-skip-rule.md"), Some("T324".to_string()));
        assert_eq!(item_id_from_spec("specs/T9-api-retry.md"), Some("T9".to_string()));
        assert_eq!(item_id_from_spec("LOOP-SPEC.md"), None);
        assert_eq!(item_id_from_spec("specs/t9x-not-a-number.md"), None);
        assert_eq!(item_id_from_spec(""), None);
    }

    #[test]
    fn item_id_falls_back_to_the_newest_harvest_artifact() {
        let tmp = tempfile::tempdir().expect("tempdir");
        assert_eq!(item_id(tmp.path()), None, "no events, no artifacts");
        write_small(
            &tmp.path().join(".chug/events.jsonl"),
            r#"{"type":"run_start","ts":"2026-11-09T00:00:00Z","spec":"LOOP-SPEC.md"}"#,
        );
        assert_eq!(item_id(tmp.path()), None, "LOOP-SPEC.md names no item");
        // mtime ordering needs real distance (coarse filesystem timestamps):
        // the verdict lands first, the LEDGER a moment later and wins.
        write_small(&tmp.path().join(".chug/verdict-t310-validate.md"), "x");
        std::thread::sleep(std::time::Duration::from_millis(1100));
        write_small(&tmp.path().join(".chug/LEDGER-t309-impl-20261028-133351.md"), "x");
        assert_eq!(item_id(tmp.path()), Some("T309".to_string()), "newest by mtime wins");
    }

    #[test]
    fn git_worktrees_dir_resolves_the_real_and_the_linked_layout() {
        let tmp = tempfile::tempdir().expect("tempdir");
        assert_eq!(git_worktrees_dir(tmp.path()), None, "not a git repo");
        // A plain repo: .git is a directory.
        write_small(&tmp.path().join(".git/HEAD"), "ref: refs/heads/main");
        assert_eq!(
            git_worktrees_dir(tmp.path()),
            Some(tmp.path().join(".git/worktrees"))
        );
        // A linked worktree: .git is a FILE naming the main admin dir, and
        // the worktrees dir is its parent.
        let tmp2 = tempfile::tempdir().expect("tempdir");
        write_small(
            &tmp2.path().join(".git"),
            "gitdir: /Users/x/chug/.git/worktrees/chug-loop-t339\n",
        );
        assert_eq!(
            git_worktrees_dir(tmp2.path()),
            Some(PathBuf::from("/Users/x/chug/.git/worktrees"))
        );
    }

    #[test]
    fn status_children_enumerate_and_classify_the_worktree_lanes() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let wt = tmp.path().join(".git/worktrees");
        write_small(&wt.join("chug-t302-validate/HEAD"), "ref: refs/heads/loop-t302-validate-20261023\n");
        write_small(&wt.join("chug-loop-t309-impl/HEAD"), "ref: refs/heads/loop-t309-impl\n");
        write_small(&wt.join("scratch/HEAD"), "ref: refs/heads/scratch\n");
        write_small(&wt.join("locked-lane/HEAD"), "ref: refs/heads/locked-lane\n");
        write_small(&wt.join("locked-lane/locked"), "reason");
        write_small(&wt.join("prunable-one/HEAD"), "ref: refs/heads/prunable-one\n");
        write_small(&wt.join("prunable-one/prunable"), "");
        write_small(&wt.join("detached/HEAD"), "b2707ba6\n");
        // Files (not dirs) under the admin dir are not worktrees.
        write_small(&wt.join("stray-file"), "x");
        let children = status_children(tmp.path());
        let names: Vec<&str> = children.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(
            names,
            vec!["chug-loop-t309-impl", "chug-t302-validate", "detached", "locked-lane", "prunable-one", "scratch"],
            "sorted, deterministic, files skipped"
        );
        let by_id = |id: &str| children.iter().find(|c| c.id == id).expect("child");
        assert_eq!(by_id("chug-loop-t309-impl").kind, "impl");
        assert_eq!(by_id("chug-t302-validate").kind, "validate");
        assert_eq!(by_id("scratch").kind, "worktree");
        assert_eq!(by_id("locked-lane").state, "locked");
        assert_eq!(by_id("prunable-one").state, "prunable");
        assert_eq!(by_id("detached").state, "detached");
        assert_eq!(by_id("scratch").state, "ready");
    }

    #[test]
    fn open_todo_rows_renders_the_open_queue_and_skips_the_rest() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let todo = concat!(
            "# TODO\n\n",
            "| id | title | spec | pri | status | notes |\n",
            "|----|-------|------|-----|--------|-------|\n",
            "| T340 | typed status endpoints | specs/t340.md | 2 | open | |\n",
            "| T341 | permission broker | specs/t341.md | 2 | todo | operator-filed |\n",
            "| T328 | daemon provider | specs/t328.md | 3 | open | BLOCKED (re-assessed cycle-3155): the harness axis |\n",
            "| T345 | budget thing | specs/t345.md | 1 | open | BLOCKED (budget exhausted on two children) |\n",
            "| T344 | harness | specs/t344.md | 3 | todo | |\n",
            "| T324 | anchor skip-rule | specs/t324.md | 4 | done | landed |\n",
            "| P7 | proposed idea | specs/p7.md | 3 | proposed | |\n",
            "\n",
            "## Proposed\n",
            "| id | title | spec | pri | status | notes |\n",
            "|----|-------|------|-----|--------|-------|\n",
            "| P9 | another | specs/p9.md | 2 | proposed | |\n",
        );
        write_small(&tmp.path().join("TODO.md"), todo);
        let items = open_todo_rows(tmp.path());
        let ids: Vec<&str> = items.iter().filter_map(|i| i["id"].as_str()).collect();
        assert_eq!(ids, vec!["T340", "T341", "T328", "T345", "T344"], "open+todo only, in file order");
        let by_id = |id: &str| items.iter().find(|i| i["id"] == serde_json::json!(id)).expect("row");
        let row = by_id("T340");
        assert_eq!(row["pri"], serde_json::json!(2));
        assert_eq!(row["status"], serde_json::json!("open"));
        assert!(row.get("blocked_reason").is_none(), "unblocked rows carry no reason");
        assert_eq!(
            by_id("T341")["title"],
            serde_json::json!("permission broker"),
            "verbatim title"
        );
        assert_eq!(by_id("T328")["blocked_reason"], serde_json::json!("external"));
        assert_eq!(by_id("T345")["blocked_reason"], serde_json::json!("budget"));
        assert_eq!(by_id("T344")["status"], serde_json::json!("todo"));
        // A missing TODO.md is an empty queue, never an error.
        assert!(open_todo_rows(Path::new("/tmp/chug-t340-does-not-exist-anywhere")).is_empty());
    }

    #[test]
    fn is_todo_id_and_row_blocked_reason_pin_the_queue_rules() {
        assert!(is_todo_id("T340"));
        assert!(is_todo_id("P7"));
        assert!(is_todo_id("T9"));
        assert!(!is_todo_id("id"));
        assert!(!is_todo_id(""));
        assert!(!is_todo_id("T"));
        assert!(!is_todo_id("----"));
        assert!(!is_todo_id("T3x4"));

        assert_eq!(row_blocked_reason("clean notes", "open"), None);
        assert_eq!(row_blocked_reason("", "todo"), None);
        assert_eq!(row_blocked_reason("anything", "blocked"), Some("external"));
        assert_eq!(row_blocked_reason("BLOCKED (re-assessed cycle-3155): the axis", "open"), Some("external"));
        assert_eq!(row_blocked_reason("BLOCKED (budget exhausted)", "open"), Some("budget"));
        assert_eq!(row_blocked_reason("BLOCKED (permission broker not installed)", "open"), Some("permission"));
        assert_eq!(row_blocked_reason("BLOCKED (error in the harness)", "open"), Some("error"));
        assert_eq!(row_blocked_reason("BLOCKED (question pending)", "open"), Some("question"));
        assert_eq!(row_blocked_reason("BLOCKED on the harness axis", "open"), Some("external"));
    }
}
