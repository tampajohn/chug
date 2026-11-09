//! T340 — the chugd typed status endpoints, over the REAL binary.
//!
//! `GET /status/loop` and `GET /status/items` render the T339 contract's
//! typed status model from a repo's loop state files, observation-only:
//! what the loop is doing, what it is blocked on, what children (worktrees)
//! it has, what the open queue looks like. The battery here is the spec's
//! test section, wire-level:
//!
//! (a) each status enum value renders from canned loopd state —
//!     working (fresh events tail), idle (live supervisor, stale tail),
//!     dormant (no supervisor / empty queue), and both blocked legs the
//!     loop surface derives (a budget abort, the supervisor's HALTED
//!     latch);
//! (b) the T328 blocked/external case names the missing repo path — it
//!     surfaces with its detail, never vanishes;
//! (c) children enumerate impl + validate worktrees (plus the generic
//!     lane) from a fixture .git worktree admin dir, with states;
//! (d) the response keys match docs/meta-harness.md exactly (the pinned
//!     fixture tests/fixtures/daemon_status/status-loop-v1.json), and every
//!     response carries contract_version "1";
//! (e) /status/items renders the open queue (open + todo rows, pri, verbatim
//!     titles, blocked rows carrying a reason) without the done/proposed
//!     rows;
//! (f) the stub serves /healthz only — /status refuses there exactly as
//!     /sessions refuses, and capabilities against a stub stays all-off;
//! (g) a live status daemon flips the capabilities document
//!     (daemon_status true, lifecycle_events "1), and a socket-absent
//!     probe degrades: daemon_status false, never a hang;
//! (h) the legacy surfaces are untouched: /healthz and /sessions shapes,
//!     a query string on a legacy path still 404s, and non-GET /status
//!     404s.

#![cfg(unix)]

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const HEALTH_DEADLINE: Duration = Duration::from_secs(90);

/// The freshness bound (the spec's "working" leg): mirrors the named const
/// in daemon.rs — an integration pin of the DEFAULT, so a silent const
/// change fails here.
const FRESH_DRIVER_SECS: u64 = 300;

/// Format epoch seconds as RFC3339 UTC — the same shape the daemon speaks
/// (mirrored from the registry harness; no chrono in dev-deps).
fn rfc3339(secs: u64) -> String {
    let days = secs / 86_400;
    let rem = secs % 86_400;
    let (hh, mm, ss) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    // civil_from_days (Howard Hinnant) — days since 1970-01-01 to (y, m, d).
    let z = days as i64 + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
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
        let _ = Command::new("kill")
            .arg("-9")
            .arg(self.child.id().to_string())
            .status();
        let _ = self.child.wait();
    }
}

impl Home {
    /// Spawn `chug daemon` with `CHUG_HOME` in a tempdir plus `extra_env`,
    /// waiting for healthz (the same warm-up exec as the registry harness —
    /// the macOS dyld stall can swallow the first exec).
    fn spawn(extra_env: &[(&str, &str)]) -> Home {
        let dir = tempfile::tempdir().expect("tempdir");
        let exe = env!("CARGO_BIN_EXE_chug");
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
        let child = cmd.arg("daemon").spawn().expect("spawn the daemon under test");
        let home = Home { dir, child };
        let deadline = Instant::now() + HEALTH_DEADLINE;
        while Instant::now() < deadline {
            if let Ok((status, _)) = uds_request(&home.sock(), "GET", "/healthz", None) {
                assert_eq!(status, 200, "healthz must come up");
                return home;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        panic!("daemon never served healthz within {HEALTH_DEADLINE:?}");
    }

    fn sock(&self) -> PathBuf {
        self.dir.path().join("daemon.sock")
    }
}

/// One raw UDS HTTP request (the same wire curl --unix-socket speaks).
fn uds_request(
    sock: &Path,
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
    stream.read_to_end(&mut raw).map_err(|e| e.to_string())?;
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

fn get_json(home: &Home, path: &str) -> (u16, serde_json::Value) {
    let (status, text) = uds_request(&home.sock(), "GET", path, None).expect("GET");
    let value = serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("GET {path} -> HTTP {status} body {text:?}: {e}"));
    (status, value)
}

/// Run `chug <args>` with a scoped environment and return (exit, stdout).
fn run_chug(home_dir: &Path, extra_env: &[(&str, &str)], args: &[&str]) -> (i32, String) {
    let exe = env!("CARGO_BIN_EXE_chug");
    let mut cmd = Command::new(exe);
    cmd.env("CHUG_HOME", home_dir)
        .env_remove("CHUG_DAEMON_SOCK")
        .arg(args[0])
        .args(&args[1..])
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    for (key, value) in extra_env {
        cmd.env(key, value);
    }
    let output = cmd.output().expect("run chug");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
    )
}

fn write_file(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create parent dirs");
    }
    std::fs::write(path, contents).expect("write fixture file");
}

/// A canned TODO.md with the queue shapes the battery needs.
const TODO_FIXTURE: &str = concat!(
    "# TODO\n\n",
    "| id | title | spec | pri | status | notes |\n",
    "|----|-------|------|-----|--------|-------|\n",
    "| T341 | permission-broker hook pack | specs/t341-permission-broker-hook.md | 2 | open | operator-filed |\n",
    "| T342 | session-status mod | specs/t342-session-status-mod.md | 3 | todo | operator-filed |\n",
    "| T328 | chug-watch daemon-socket provider | specs/t328-chugwatch-daemon-provider.md | 3 | open | BLOCKED (re-assessed cycle-3155): the chug-watch harness axis |\n",
    "| T324 | anchor skip-rule | specs/t324.md | 4 | done | landed |\n",
    "| P7 | a proposal | specs/p7.md | 3 | proposed | |\n",
);

/// The base tempdir holding the fixture repos; dropped with the test.
struct Fixtures(tempfile::TempDir);

impl Fixtures {
    fn repo(&self, name: &str) -> PathBuf {
        self.0.path().join(name)
    }

    fn path_str(&self, name: &str) -> String {
        self.repo(name).display().to_string()
    }

    /// Canned loopd state: a live supervisor pid (this test process) plus an
    /// events tail. `fresh` controls the tail's age against the freshness
    /// bound; `lines` are raw JSONL lines appended after the run_start.
    fn canned(&self, name: &str, fresh: bool, events_lines: &str) {
        let repo = self.repo(name);
        let age = if fresh { 0 } else { FRESH_DRIVER_SECS + 120 };
        let ts = rfc3339(now_secs().saturating_sub(age));
        write_file(
            &repo.join(".chug/loopd/loopd.pid"),
            &format!("{}", std::process::id()),
        );
        write_file(
            &repo.join(".chug/events.jsonl"),
            &concat!(
                    r#"{"type":"run_start","ts":"{TS}","spec":"specs/t341-permission-broker-hook.md"}"#,
                    "\n",
                    r#"{"type":"iteration","ts":"{TS}","n":3}"#,
                    "\n"
                )
                .replace("{TS}", &ts),
        );
        if !events_lines.is_empty() {
            use std::io::Write as _;
            let mut file = std::fs::OpenOptions::new()
                .append(true)
                .open(repo.join(".chug/events.jsonl"))
                .expect("append events");
            let _ = writeln!(file, "{}", events_lines.replace("{TS}", &ts));
        }
        write_file(&repo.join("TODO.md"), TODO_FIXTURE);
    }
}

/// Build the fixture repo battery: one daemon serves them all via ?repo=.
fn fixtures() -> Fixtures {
    let dir = Fixtures(tempfile::tempdir().expect("tempdir"));
    // (a) working: a live supervisor + a fresh events tail.
    dir.canned("working", true, "");
    // (a) idle: a live supervisor + a stale tail + open queue rows.
    dir.canned("idle", false, "");
    // (a) dormant: the same state with NO live supervisor pid.
    dir.canned("dormant", false, "");
    std::fs::remove_file(dir.repo("dormant").join(".chug/loopd/loopd.pid")).expect("drop pid");
    // (a) blocked/budget: the last run segment died on its iteration budget.
    dir.canned(
        "budget",
        false,
        r#"{"type":"abort","ts":"{TS}","reason":"wrap unfinished","budget_kind":"iterations","budget_max":50}"#,
    );
    // (a) blocked/error: the supervisor's halt latch (3 failed cycles).
    dir.canned("halted", false, "");
    write_file(
        &dir.repo("halted").join(".chug/loopd/HALTED"),
        "3 consecutive cycles without goal_complete — manual review required",
    );
    // (a) dormant via the empty-queue gate: live supervisor, stale tail,
    // no open rows (the T334 dormant state).
    dir.canned("empty-queue", false, "");
    write_file(&dir.repo("empty-queue").join("TODO.md"), "# TODO\n\nno rows yet\n");
    // (c) children: a fixture .git worktree admin dir — impl, validate,
    // generic, locked, prunable, detached lanes.
    let wt = dir.repo("children").join(".git/worktrees");
    write_file(&wt.join("chug-t302-validate/HEAD"), "ref: refs/heads/loop-t302-validate-20261023\n");
    write_file(&wt.join("chug-loop-t309-impl/HEAD"), "ref: refs/heads/loop-t309-impl\n");
    write_file(&wt.join("scratch/HEAD"), "ref: refs/heads/scratch\n");
    write_file(&wt.join("locked-lane/HEAD"), "ref: refs/heads/locked-lane\n");
    write_file(&wt.join("locked-lane/locked"), "operator lock");
    write_file(&wt.join("prunable-one/HEAD"), "ref: refs/heads/prunable-one\n");
    write_file(&wt.join("prunable-one/prunable"), "");
    write_file(&wt.join("detached/HEAD"), "b2707ba6\n");
    write_file(&dir.repo("children").join("TODO.md"), TODO_FIXTURE);
    dir
}

/// (a) Each status enum value renders from canned loopd state.
#[test]
fn status_loop_renders_each_enum_value_from_canned_loopd_state() {
    let fx = fixtures();
    let home = Home::spawn(&[("CHUG_DAEMON_SESSIONS", "1")]);

    let (status, value) = get_json(&home, &format!("/status/loop?repo={}", enc(&fx.path_str("working"))));
    assert_eq!(status, 200);
    assert_eq!(value["status"], "working", "{}", value);
    assert_eq!(value["iteration"], 3);
    assert_eq!(value["item_id"], "T341");
    assert_eq!(value["blocked_reason"], serde_json::Value::Null);
    assert_eq!(value["blocked_detail"], serde_json::Value::Null);
    assert_eq!(value["contract_version"], "1");

    let (_, value) = get_json(&home, &format!("/status/loop?repo={}", enc(&fx.path_str("idle"))));
    assert_eq!(value["status"], "idle", "{}", value);
    assert_eq!(value["blocked_reason"], serde_json::Value::Null);

    let (_, value) = get_json(&home, &format!("/status/loop?repo={}", enc(&fx.path_str("dormant"))));
    assert_eq!(value["status"], "dormant", "{}", value);

    // blocked/budget: the last segment died on its iteration budget, with
    // the named detail.
    let (_, value) = get_json(&home, &format!("/status/loop?repo={}", enc(&fx.path_str("budget"))));
    assert_eq!(value["status"], "blocked", "{}", value);
    assert_eq!(value["blocked_reason"], "budget");
    assert_eq!(
        value["blocked_detail"],
        "iterations budget exhausted (max 50): wrap unfinished"
    );

    // blocked/error: the supervisor's halt latch carries its content.
    let (_, value) = get_json(&home, &format!("/status/loop?repo={}", enc(&fx.path_str("halted"))));
    assert_eq!(value["status"], "blocked", "{}", value);
    assert_eq!(value["blocked_reason"], "error");
    assert!(
        value["blocked_detail"]
            .as_str()
            .unwrap_or_default()
            .contains("manual review required"),
        "{}",
        value
    );

    // dormant via the empty-queue gate: a live supervisor with nothing to
    // do is the T334 dormant state, not idle.
    let (_, value) = get_json(
        &home,
        &format!("/status/loop?repo={}", enc(&fx.path_str("empty-queue"))),
    );
    assert_eq!(value["status"], "dormant", "{}", value);
}

/// Percent-encode a path for the ?repo= selector (the bytes a harness gets
/// from `python3 -c 'urllib.parse.quote'` — / is literal, everything else
/// hex).
fn enc(path: &str) -> String {
    let mut out = String::new();
    for byte in path.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'-' | b'.' | b'_' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// (b) The T328 case: a repo that is not there reports blocked/external
/// with the missing path in the detail — it surfaces, never vanishes.
#[test]
fn status_loop_blocked_external_names_the_missing_repo() {
    let home = Home::spawn(&[("CHUG_DAEMON_SESSIONS", "1")]);
    let missing = "/tmp/chug-t340-definitely-not-here";
    let (status, value) = get_json(&home, &format!("/status/loop?repo={}", enc(missing)));
    assert_eq!(status, 200, "the case must surface as a status, not a 404");
    assert_eq!(value["status"], "blocked", "{}", value);
    assert_eq!(value["blocked_reason"], "external");
    let detail = value["blocked_detail"].as_str().expect("detail");
    assert!(detail.contains(missing), "{detail} must name the missing path");
    assert_eq!(value["children"], serde_json::json!([]));
}

/// (c) Children enumerate the impl + validate worktrees (and the generic,
/// locked, prunable, detached states) from the fixture admin dir.
#[test]
fn children_enumerate_impl_and_validate_worktrees() {
    let fx = fixtures();
    let home = Home::spawn(&[("CHUG_DAEMON_SESSIONS", "1")]);
    let (_, value) = get_json(&home, &format!("/status/loop?repo={}", enc(&fx.path_str("children"))));
    let children = value["children"].as_array().expect("children array");
    let by_id = |id: &str| -> serde_json::Value {
        children
            .iter()
            .find(|c| c["id"] == serde_json::json!(id))
            .unwrap_or_else(|| panic!("no child {id} in {children:?}"))
            .clone()
    };
    assert_eq!(children.len(), 6, "{children:?}");
    assert_eq!(by_id("chug-loop-t309-impl")["kind"], "impl");
    assert_eq!(by_id("chug-t302-validate")["kind"], "validate");
    assert_eq!(by_id("scratch")["kind"], "worktree");
    for id in ["chug-loop-t309-impl", "chug-t302-validate", "scratch"] {
        assert_eq!(by_id(id)["state"], "ready", "{id}");
    }
    assert_eq!(by_id("locked-lane")["state"], "locked");
    assert_eq!(by_id("prunable-one")["state"], "prunable");
    assert_eq!(by_id("detached")["state"], "detached");
    // Every child row is exactly the three-key shape.
    for child in children {
        let keys: Vec<&String> = child.as_object().expect("object").keys().collect();
        assert_eq!(keys.len(), 3, "{child}");
        for key in ["kind", "id", "state"] {
            assert!(child.get(key).is_some(), "{child} missing {key}");
        }
    }
}

/// (d) The response keys match docs/meta-harness.md exactly — the pinned
/// fixture is the doc shape plus `contract_version`, and the live response
/// carries exactly those keys.
fn repo_root() -> std::path::PathBuf {
    // Runtime cwd, never a compile-time env!() of CARGO_MANIFEST_DIR — the T48
    // rule (cargo sets the test cwd to the package root).
    std::env::current_dir().expect("cargo sets the test cwd to the package root")
}

#[test]
fn status_loop_keys_match_the_doc_and_the_pinned_fixture() {
    // The docs' T340 example JSON block (the one carrying "children").
    let docs =
        std::fs::read_to_string(repo_root().join("docs/meta-harness.md")).expect("read docs/meta-harness.md");
    let block_start = docs
        .rfind("```json\n{\n  \"status\": \"working\"")
        .expect("the T340 example block in docs/meta-harness.md");
    let rest = &docs[block_start + "```json\n".len()..];
    let end = rest.find("```").expect("block close");
    let doc_example: serde_json::Value =
        serde_json::from_str(&rest[..end]).expect("the docs example parses");
    let doc_keys: Vec<&String> = doc_example.as_object().expect("object").keys().collect();

    let fixture_text = std::fs::read_to_string(
        repo_root().join("tests/fixtures/daemon_status/status-loop-v1.json"),
    )
    .expect("read the pinned fixture");
    let fixture: serde_json::Value = serde_json::from_str(&fixture_text).expect("fixture parses");
    let fixture_keys: Vec<&String> = fixture.as_object().expect("object").keys().collect();
    assert_eq!(fixture["contract_version"], "1");

    // The fixture = the doc shape + contract_version (the spec's "every
    // response carries contract_version"), exactly — no extra keys.
    assert_eq!(doc_keys.len() + 1, fixture_keys.len(), "{fixture_keys:?}");
    for key in &doc_keys {
        assert!(
            fixture_keys.contains(key),
            "docs key {key} missing from the fixture"
        );
    }

    let fx = fixtures();
    let home = Home::spawn(&[("CHUG_DAEMON_SESSIONS", "1")]);
    let (_, value) = get_json(&home, &format!("/status/loop?repo={}", enc(&fx.path_str("working"))));
    let live_keys: Vec<&String> = value.as_object().expect("object").keys().collect();
    assert_eq!(live_keys, fixture_keys, "the live wire is the pinned shape");
    // Child rows are the fixture's child shape too.
    let fixture_child = fixture["children"].as_array().expect("arr")[0].clone();
    let fixture_child_keys: Vec<&String> = fixture_child
        .as_object()
        .expect("object")
        .keys()
        .collect();
    let (_, value) = get_json(&home, &format!("/status/loop?repo={}", enc(&fx.path_str("children"))));
    let live_child = value["children"].as_array().expect("arr")[0].clone();
    let live_child_keys: Vec<&String> = live_child.as_object().expect("object").keys().collect();
    assert_eq!(live_child_keys, fixture_child_keys);
}

/// (e) /status/items renders the open queue: open + todo rows with pri and
/// verbatim titles, blocked rows carrying a reason, done/proposed skipped.
#[test]
fn status_items_renders_the_open_queue() {
    let fx = fixtures();
    let home = Home::spawn(&[("CHUG_DAEMON_SESSIONS", "1")]);
    let (status, value) = get_json(&home, &format!("/status/items?repo={}", enc(&fx.path_str("working"))));
    assert_eq!(status, 200);
    assert_eq!(value["contract_version"], "1");
    let items = value["items"].as_array().expect("items array");
    let ids: Vec<&str> = items.iter().filter_map(|i| i["id"].as_str()).collect();
    assert_eq!(ids, vec!["T341", "T342", "T328"], "{items:?}");
    let by_id = |id: &str| -> serde_json::Value {
        items
            .iter()
            .find(|i| i["id"] == serde_json::json!(id))
            .unwrap_or_else(|| panic!("no {id} in {items:?}"))
            .clone()
    };
    let row = by_id("T341");
    assert_eq!(row["title"], "permission-broker hook pack", "verbatim");
    assert_eq!(row["pri"], 2);
    assert_eq!(row["status"], "open");
    assert!(row.get("blocked_reason").is_none(), "unblocked rows carry no reason");
    assert_eq!(by_id("T342")["status"], "todo");
    assert_eq!(by_id("T328")["blocked_reason"], "external");
    // Row keys are exactly the doc's shape (+ pri always present here).
    for row in items {
        let keys: Vec<&String> = row.as_object().expect("object").keys().collect();
        assert!(keys.iter().any(|k| k.as_str() == "id"));
        assert!(row.get("title").is_some() && row.get("status").is_some() && row.get("pri").is_some());
        assert!(
            keys.iter().all(|k| ["id", "title", "pri", "status", "blocked_reason"].contains(&k.as_str())),
            "{row} has out-of-shape keys"
        );
    }
    // A repo with no TODO.md is an empty queue, never an error.
    let (_, value) = get_json(&home, &format!("/status/items?repo={}", enc(&missing_repo())));
    assert_eq!(value["items"], serde_json::json!([]));
}

fn missing_repo() -> String {
    "/tmp/chug-t340-definitely-not-here".to_string()
}

/// (f) The stub serves /healthz only: /status refuses there exactly as
/// /sessions refuses, and capabilities against a stub stays all-off.
#[test]
fn stub_serves_healthz_only_and_refuses_status() {
    let home = Home::spawn(&[("CHUG_DAEMON_STUB", "1")]);
    let (status, text) = uds_request(&home.sock(), "GET", "/status/loop", None).expect("GET");
    assert_eq!(status, 500, "{text}");
    assert!(text.contains("detail"), "{text}");
    let (status, _) = uds_request(&home.sock(), "GET", "/status/items", None).expect("GET");
    assert_eq!(status, 500);
    // And the capabilities probe against the stub reports all-off.
    let (code, out) = run_chug(home.dir.path(), &[], &["capabilities"]);
    assert_eq!(code, 0);
    let doc: serde_json::Value = serde_json::from_str(&out).expect("capabilities parses");
    assert_eq!(doc["features"]["daemon_status"], false);
    assert_eq!(doc["features"]["lifecycle_events"], false);
}

/// (g) A live status daemon flips the capabilities document
/// (daemon_status true, lifecycle_events "1).
#[test]
fn capabilities_flip_when_a_status_daemon_answers() {
    let fx = fixtures();
    let home = Home::spawn(&[
        ("CHUG_DAEMON_SESSIONS", "1"),
        ("CHUG_DAEMON_REPO", fx.path_str("working").as_str()),
    ]);
    let (code, out) = run_chug(home.dir.path(), &[], &["capabilities"]);
    assert_eq!(code, 0, "{out}");
    let doc: serde_json::Value = serde_json::from_str(&out).expect("capabilities parses");
    assert_eq!(doc["features"]["daemon_status"], true, "{doc}");
    assert_eq!(doc["features"]["lifecycle_events"], "1", "{doc}");
    // The other rows stay off.
    assert_eq!(doc["features"]["permission_broker"], false);
    assert_eq!(doc["features"]["usage_events"], false);
    assert_eq!(doc["features"]["status_mod"], false);
}

/// (g) Socket-absent degrade: with no daemon, capabilities reports
/// daemon_status false and daemon null, bounded (never a hang).
#[test]
fn socket_absent_capabilities_degrade_bounded_and_false() {
    let dir = tempfile::tempdir().expect("tempdir");
    let started = Instant::now();
    let (code, out) = run_chug(dir.path(), &[], &["capabilities"]);
    let elapsed = started.elapsed();
    assert_eq!(code, 0, "{out}");
    let doc: serde_json::Value = serde_json::from_str(&out).expect("capabilities parses");
    assert_eq!(doc["features"]["daemon_status"], false);
    assert_eq!(doc["features"]["lifecycle_events"], false);
    assert_eq!(doc["versions"]["daemon"], serde_json::Value::Null);
    assert!(
        elapsed < Duration::from_secs(10),
        "the absent probe took {elapsed:?} — the degrade is bounded"
    );
}

/// (h) The legacy surfaces are untouched: healthz and /sessions shapes, a
/// query string on a legacy path still 404s, and non-GET /status 404s.
#[test]
fn legacy_surfaces_are_untouched() {
    let home = Home::spawn(&[("CHUG_DAEMON_SESSIONS", "1")]);
    // healthz shape unchanged.
    let (status, body) = uds_request(&home.sock(), "GET", "/healthz", None).expect("GET");
    assert_eq!(status, 200);
    let value: serde_json::Value = serde_json::from_str(&body).expect("healthz parses");
    assert_eq!(value["status"], "ok");
    assert!(value.get("version").is_none(), "health stays versionless in T340");
    // /sessions registers and lists (the T219 shape, with its envelope).
    let (status, body) = uds_request(
        &home.sock(),
        "POST",
        "/sessions",
        Some(r#"{"id":"t340-probe","role":"delegate-child","status":"working"}"#),
    )
    .expect("POST");
    assert_eq!(status, 200, "{body}");
    assert_eq!(body, "{\"ok\":true}");
    let (status, value) = get_json(&home, "/sessions");
    assert_eq!(status, 200);
    assert!(value["sessions"].as_array().expect("arr").iter().any(|s| s["id"] == "t340-probe"));
    // A query string on a LEGACY path is still a 404 (pre-T340 behavior).
    let (status, _) = uds_request(&home.sock(), "GET", "/healthz?x=1", None).expect("GET");
    assert_eq!(status, 404);
    let (status, _) = uds_request(&home.sock(), "GET", "/sessions?x=1", None).expect("GET");
    assert_eq!(status, 404);
    // Only GET serves the status endpoints; near-miss paths 404.
    let (status, _) = uds_request(&home.sock(), "POST", "/status/loop", None).expect("POST");
    assert_eq!(status, 404);
    let (status, _) = get_json(&home, "/status/loopy");
    assert_eq!(status, 404);
    let (status, _) = get_json(&home, "/status");
    assert_eq!(status, 404);
}
