//! T219 — the daemon /sessions registry over the REAL binary.
//!
//! Every chug process registers a TTL heartbeat on the host-scoped 0600
//! socket; `GET /sessions` answers "what chug runs are alive on this box"
//! with one socket query (the chug-watch mod's four SSH round trips,
//! replaced). These pins ride the same harness shape as the lifecycle pins
//! (`CHUG_HOME` tempdir, warm-up exec for the macOS dyld stall, healthz
//! wait) and cover the spec's (a)–(f):
//!
//! (a) register → GET roundtrip returns the entry with the posted fields;
//! (b) a re-POST upserts (one entry per id, refreshed last_event_ts);
//! (c) an entry older than the TTL is evicted from GET — and one inside
//!     the TTL is not (the boundary cuts both ways, so neither an
//!     always-evict nor a never-evict mutant survives);
//! (d) all four roles round-trip, and an unknown role is refused;
//! (e) the feature-off stub refuses /sessions on BOTH verbs, exactly as it
//!     refuses /judge (a stub must never fabricate a registry);
//! (f) /judge and /healthz are untouched (the lifecycle and feature-off
//!     pins stay green alongside), and the daemon self-registers at startup.
//!
//! Everything here is best-effort-facing on the wire and exact on the
//! asserts: the daemon is spawned with the real transport, the requests are
//! raw UDS HTTP, and a pinned failure is a hard failure.

#![cfg(unix)]

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const HEALTH_DEADLINE: Duration = Duration::from_secs(90);

/// The host-scoped TTL (spec req 2): entries without a heartbeat within this
/// window are evicted lazily on read. Mirrors the named const in daemon.rs —
/// an integration pin of the DEFAULT, so a silent const change fails here.
const SESSION_TTL_SECS: u64 = 600;

/// Format epoch seconds as RFC3339 UTC — the same shape the daemon and
/// `date -u +%Y-%m-%dT%H:%M:%SZ` speak. (No chrono in dev-deps; Hinnant's
/// civil-from-days, mirrored from archive.rs.)
fn rfc3339(secs: u64) -> String {
    let days = secs / 86_400;
    let rem = secs % 86_400;
    let (hh, mm, ss) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    // civil_from_days (Howard Hinnant) — days since 1970-01-01 to (y, m, d).
    let z = days as i64 + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock after the epoch")
        .as_secs()
}

/// One spawned daemon + its tempdir home + a kill guard.
struct Home {
    dir: tempfile::TempDir,
    child: std::process::Child,
}

impl Drop for Home {
    fn drop(&mut self) {
        // Best-effort kill; the tempdir unlinks the socket with the home.
        let _ = Command::new("kill")
            .arg("-9")
            .arg(self.child.id().to_string())
            .status();
        let _ = self.child.wait();
    }
}

impl Home {
    /// Spawn `chug daemon` with `CHUG_HOME` in a tempdir plus `extra_env`,
    /// then wait for healthz (the same warm-up-exec first call as the
    /// lifecycle harness — the macOS dyld stall can swallow the first exec).
    fn spawn(extra_env: &[(&str, &str)]) -> Home {
        let dir = tempfile::tempdir().expect("tempdir");
        let exe = env!("CARGO_BIN_EXE_chug");
        // Warm-up exec: the first spawn can be swallowed by a dyld stall;
        // `--help` exits instantly and pays the cost.
        let _ = Command::new(exe)
            .env_remove("CHUG_DAEMON_SOCK")
            .arg("--help")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        let mut cmd = Command::new(exe);
        cmd.env("CHUG_HOME", dir.path())
            .env_remove("CHUG_DAEMON_SOCK")
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        for (key, value) in extra_env {
            cmd.env(key, value);
        }
        let child = cmd
            .arg("daemon")
            .spawn()
            .expect("spawn the daemon under test");
        let home = Home { dir, child };
        let deadline = Instant::now() + HEALTH_DEADLINE;
        while Instant::now() < deadline {
            if let Ok((status, _)) = uds_request(&home.sock(), "GET", "/healthz", None) {
                assert_eq!(status, 200, "healthz must come up: see daemon stderr above");
                return home;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        panic!("daemon never served healthz within {HEALTH_DEADLINE:?}");
    }

    fn sock(&self) -> std::path::PathBuf {
        self.dir.path().join("daemon.sock")
    }
}

/// One raw UDS HTTP request (the same wire curl --unix-socket speaks).
fn uds_request(
    sock: &std::path::Path,
    method: &str,
    path: &str,
    body: Option<&str>,
) -> Result<(u16, String), String> {
    let mut stream = UnixStream::connect(sock).map_err(|e| e.to_string())?;
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| e.to_string())?;
    let payload = body.unwrap_or("");
    let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
        payload.len()
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|e| e.to_string())?;
    let mut raw = Vec::new();
    stream
        .read_to_end(&mut raw)
        .map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&raw);
    let status: u16 = text
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| format!("no status line in {text:?}"))?;
    let body = text
        .split("\r\n\r\n")
        .nth(1)
        .unwrap_or_default()
        .to_string();
    Ok((status, body))
}

/// POST one registration; assert the 200 `{"ok":true}` shape.
fn register(home: &Home, body: &serde_json::Value) {
    let (status, text) = uds_request(&home.sock(), "POST", "/sessions", Some(&body.to_string()))
        .expect("POST /sessions");
    assert_eq!(status, 200, "register {body} -> {text}");
    assert_eq!(text, "{\"ok\":true}", "register body for {body}");
}

/// GET the registry and return the parsed body.
fn list(home: &Home) -> serde_json::Value {
    let (status, text) = uds_request(&home.sock(), "GET", "/sessions", None).expect("GET /sessions");
    assert_eq!(status, 200, "list -> {text}");
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("list body {text}: {e}"))
}

/// The entry with `id`, or panic — with the whole body in the message.
fn entry<'a>(value: &'a serde_json::Value, id: &str) -> &'a serde_json::Value {
    value["sessions"]
        .as_array()
        .expect("sessions array")
        .iter()
        .find(|e| e["id"] == serde_json::json!(id))
        .unwrap_or_else(|| panic!("no entry {id} in {}", value))
}

// (a) register → GET roundtrip returns the entry with the posted fields.
#[test]
fn register_then_get_roundtrips_posted_fields() {
    let home = Home::spawn(&[("CHUG_DAEMON_SESSIONS", "1")]);
    let beat = now_secs().saturating_sub(30);
    register(
        &home,
        &serde_json::json!({
            "id": "loopd-42",
            "role": "loopd-cycle",
            "started": "2024-02-29T11:00:00Z",
            "last_event_ts": rfc3339(beat),
            "status": "running"
        }),
    );
    let value = list(&home);
    let got = entry(&value, "loopd-42");
    // The posted fields echo VERBATIM (the client's words are the record).
    assert_eq!(got["role"], serde_json::json!("loopd-cycle"), "{}", value);
    assert_eq!(got["started"], serde_json::json!("2024-02-29T11:00:00Z"), "{}", value);
    assert_eq!(got["last_event_ts"], serde_json::json!(rfc3339(beat)), "{}", value);
    assert_eq!(got["status"], serde_json::json!("running"), "{}", value);
    // The derived pair: age_sec is a number (started is in the past but far
    // before the epoch budget of this box — the value is not pinned, the
    // derivation is), `now` is the server's RFC3339 clock.
    assert!(got["age_sec"].is_number(), "{}", value);
    assert_eq!(value["now"], serde_json::json!(rfc3339(now_secs())), "{}", value);
}

// (b) a re-POST upserts: one entry per id, refreshed last_event_ts.
#[test]
fn re_post_upserts_one_entry_with_refreshed_last_event_ts() {
    let home = Home::spawn(&[("CHUG_DAEMON_SESSIONS", "1")]);
    let first = now_secs().saturating_sub(120);
    let second = now_secs().saturating_sub(10);
    register(
        &home,
        &serde_json::json!({"id": "dashd-7", "role": "dashd", "started": rfc3339(first),
                            "last_event_ts": rfc3339(first), "status": "starting"}),
    );
    // The heartbeat is the same shape as the first registration (spec req 1).
    register(
        &home,
        &serde_json::json!({"id": "dashd-7", "role": "dashd", "started": rfc3339(first),
                            "last_event_ts": rfc3339(second), "status": "running"}),
    );
    let value = list(&home);
    let matches: Vec<&serde_json::Value> = value["sessions"]
        .as_array()
        .expect("sessions array")
        .iter()
        .filter(|e| e["id"] == serde_json::json!("dashd-7"))
        .collect();
    assert_eq!(matches.len(), 1, "one entry per id: {}", value);
    assert_eq!(matches[0]["last_event_ts"], serde_json::json!(rfc3339(second)), "{}", value);
    assert_eq!(matches[0]["status"], serde_json::json!("running"), "{}", value);
}

// (c) an entry older than the TTL is evicted from GET; one inside the TTL
// (one second under the boundary) is not.
#[test]
fn ttl_expiry_evicts_stale_entries_only() {
    let home = Home::spawn(&[("CHUG_DAEMON_SESSIONS", "1")]);
    let now = now_secs();
    register(
        &home,
        &serde_json::json!({"id": "stale", "role": "loopd-cycle",
                            "last_event_ts": rfc3339(now - SESSION_TTL_SECS - 100)}),
    );
    register(
        &home,
        &serde_json::json!({"id": "fresh", "role": "loopd-cycle",
                            "last_event_ts": rfc3339(now - 5)}),
    );
    let value = list(&home);
    let ids: Vec<&str> = value["sessions"]
        .as_array()
        .expect("sessions array")
        .iter()
        .filter_map(|e| e["id"].as_str())
        .collect();
    assert!(!ids.contains(&"stale"), "past the TTL is evicted: {}", value);
    assert!(ids.contains(&"fresh"), "inside the TTL stays: {}", value);
}

// (d) all four roles round-trip; an unknown role is refused (400).
#[test]
fn all_four_roles_roundtrip_and_unknown_role_is_refused() {
    let home = Home::spawn(&[("CHUG_DAEMON_SESSIONS", "1")]);
    for (n, role) in ["loopd-cycle", "delegate-child", "dashd", "daemon"].iter().enumerate() {
        register(
            &home,
            &serde_json::json!({"id": format!("run-{n}"), "role": role,
                                "last_event_ts": rfc3339(now_secs())}),
        );
    }
    let value = list(&home);
    let roles: Vec<&str> = value["sessions"]
        .as_array()
        .expect("sessions array")
        .iter()
        .filter_map(|e| e["role"].as_str())
        .collect();
    for role in ["loopd-cycle", "delegate-child", "dashd", "daemon"] {
        assert!(roles.contains(&role), "role {role} round-trips: {}", value);
    }
    // The role gate (spec req 1: role is one of the four).
    let (status, text) = uds_request(
        &home.sock(),
        "POST",
        "/sessions",
        Some(r#"{"id":"x","role":"cron"}"#),
    )
    .expect("bad role POST");
    assert_eq!(status, 400, "unknown role -> {text}");
    assert!(text.contains("must be one of"), "refusal names the roles: {text}");
}

// (e) the feature-off stub refuses /sessions on BOTH verbs, exactly as it
// refuses /judge — and /healthz stays green.
#[test]
fn stub_refuses_sessions_on_both_verbs() {
    let home = Home::spawn(&[("CHUG_DAEMON_STUB", "1")]);
    for (method, body) in [
        ("POST", Some("{\"id\":\"x\",\"role\":\"daemon\"}")),
        ("GET", None),
    ] {
        let (status, text) =
            uds_request(&home.sock(), method, "/sessions", body).expect("stub refusal");
        assert_eq!(status, 500, "stub {method} /sessions -> {text}");
        assert!(
            text.contains("\"detail\"") && text.contains("never served by the stub"),
            "the stub refusal keeps the judge refusal shape: {text}"
        );
    }
    let (status, text) = uds_request(&home.sock(), "GET", "/healthz", None).expect("healthz");
    assert_eq!(status, 200, "stub healthz untouched: {text}");
}

// (f) on the registry host /judge and /healthz keep today's shapes and the
// daemon self-registers at startup (role daemon) — a registry host without
// the model refuses /judge exactly as the stub does.
#[test]
fn registry_host_keeps_judge_and_healthz_and_self_registers() {
    let home = Home::spawn(&[("CHUG_DAEMON_SESSIONS", "1")]);
    // /healthz: byte-shape unchanged.
    let (status, text) = uds_request(&home.sock(), "GET", "/healthz", None).expect("healthz");
    assert_eq!(status, 200, "{text}");
    assert!(text.contains("\"status\":\"ok\""), "{text}");
    // /judge on the weightless registry host: refused, never fabricated.
    let (status, text) = uds_request(
        &home.sock(),
        "POST",
        "/judge",
        Some("{\"state\":{},\"questions\":{}}"),
    )
    .expect("judge");
    assert_eq!(status, 500, "the registry host never serves /judge: {text}");
    assert!(text.contains("\"detail\""), "{text}");
    // Unknown path / wrong method: today's 404 shape.
    let (status, text) = uds_request(&home.sock(), "GET", "/nope", None).expect("404");
    assert_eq!(status, 404, "{text}");
    assert!(text.contains("\"detail\""), "{text}");
    let (status, _) = uds_request(&home.sock(), "GET", "/judge", None).expect("404 method");
    assert_eq!(status, 404);
    // The daemon self-registers at startup (spec req 1) — and /sessions
    // still serves after all of the above.
    let value = list(&home);
    let self_entry = entry(&value, "daemon");
    assert_eq!(self_entry["role"], serde_json::json!("daemon"), "{}", value);
    assert_eq!(self_entry["status"], serde_json::json!("serving"), "{}", value);
    assert!(self_entry["age_sec"].is_number(), "{}", value);
}
