//! T341 — the meta-broker hook battery: the pack's PreToolUse hook honors
//! brokered decisions (allow/deny print Claude's permissionDecision
//! document; `ask` falls through to the stock prompt) and fail-opens to the
//! stock path on every non-answer (socket absent <5ms, timeout >800ms,
//! malformed reply, non-200, no decision). The audit fixture pins the JSONL
//! contract: every brokered decision appends exactly one line; stock-path
//! outcomes append nothing.
//!
//! The fake broker is an in-test unix-socket HTTP server (std only): accept
//! one connection, read the POST to its Content-Length, reply per scenario.
//! The hook itself runs through `hook.sh` (the wired entrypoint) with stdin
//! carrying Claude's hook payload, scoped to a scratch `$CLAUDE_CONFIG_DIR`
//! — never the operator's own.

use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const CONTRACT_VERSION: &str = "1";

/// The payload Claude pipes to a PreToolUse hook (the shape the pack
/// parses; tool_input here is deliberately large-ish to exercise the
/// input_summary bounding).
fn payload() -> String {
    let big = "x".repeat(3000);
    format!(
        r#"{{"session_id":"sess-7","transcript_path":"/tmp/t.jsonl","cwd":"/repo",
"hook_event_name":"PreToolUse","tool_name":"Bash",
"tool_input":{{"command":"rm -rf /tmp/big-{big}","description":"cleanup"}}}}"#
    )
}

/// Run hook.sh (the wired entrypoint) with a scoped claude dir and the
/// payload on stdin. Returns (exit code, stdout).
fn run_hook(claude_dir: &Path) -> (Option<i32>, String) {
    let mut child = Command::new("/bin/sh")
        .arg(pack_dir().join("hook.sh"))
        .env("CLAUDE_CONFIG_DIR", claude_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn hook.sh");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(payload().as_bytes())
        .ok();
    let out = child.wait_with_output().expect("hook.sh runs");
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stdout).to_string(),
    )
}

/// Run hook.sh with a wall-clock bound; returns (exited, exit code, stdout,
/// elapsed). The timeout scenario uses this: the hook must finish well
/// inside the broker's stall.
fn run_hook_bounded(claude_dir: &Path, bound: Duration) -> (bool, Option<i32>, String, Duration) {
    let started = Instant::now();
    let mut child = Command::new("/bin/sh")
        .arg(pack_dir().join("hook.sh"))
        .env("CLAUDE_CONFIG_DIR", claude_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn hook.sh");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(payload().as_bytes())
        .ok();
    loop {
        match child.try_wait().expect("poll hook.sh") {
            Some(status) => {
                let out = child.wait_with_output().expect("collect output");
                return (
                    true,
                    status.code(),
                    String::from_utf8_lossy(&out.stdout).to_string(),
                    started.elapsed(),
                );
            }
            None if started.elapsed() > bound => {
                let _ = child.kill();
                let _ = child.wait();
                return (false, None, String::new(), started.elapsed());
            }
            None => std::thread::sleep(Duration::from_millis(10)),
        }
    }
}

fn pack_dir() -> PathBuf {
    // T48: resolve the repo root at RUNTIME (cargo runs test binaries with
    // cwd = the package root) — the compile-time manifest macro is what
    // false-redded the cycle-21 post-merge gates under the T47 shared cache
    // and is refused by the no_compile lint on every .rs file.
    let root = std::env::current_dir().expect("cargo sets the test cwd to the package root");
    root.join("hooks/meta-broker")
}

/// The scratch claude dir with hooks/ present (the pack directory itself is
/// NOT installed — the hook.sh runs straight from the repo tree, exactly
/// how install.sh's wired command points at the installed copy).
fn scratch() -> (tempfile::TempDir, PathBuf) {
    let home = tempfile::tempdir().expect("tempdir");
    let claude_dir = home.path().join(".claude");
    std::fs::create_dir_all(&claude_dir).unwrap();
    (home, claude_dir)
}

fn audit_path(claude_dir: &Path) -> PathBuf {
    claude_dir.join("meta-broker-audit.jsonl")
}

fn audit_lines(claude_dir: &Path) -> Vec<serde_json::Value> {
    let text = std::fs::read_to_string(audit_path(claude_dir)).expect("audit file exists");
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).expect("every audit line is valid JSON"))
        .collect()
}

/// One fake broker: a scenario tag + the unix socket path. Accepts ONE
/// connection (the hook's POST), reads it to Content-Length, answers per
/// the scenario. `slow` stalls 3s (the hook's 800ms budget fires first).
enum Broker {
    Allow,
    Deny,
    Ask,
    Malformed,
    BadStatus,
    NoDecision,
    Slow,
}

impl Broker {
    fn reply(&self, conn: &mut UnixStream) {
        match self {
            Broker::Slow => std::thread::sleep(Duration::from_millis(3000)),
            Broker::Malformed => {
                let _ = conn.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 9\r\nConnection: close\r\n\r\nnot json!!");
            }
            Broker::BadStatus => {
                let _ = conn.write_all(b"HTTP/1.1 500 nope\r\nContent-Length: 21\r\nConnection: close\r\n\r\n{\"decision\":\"allow\"}");
            }
            Broker::NoDecision => {
                let _ = conn.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 12\r\nConnection: close\r\n\r\n{\"reason\":\"x\"}");
            }
            Broker::Allow | Broker::Deny | Broker::Ask => {
                let decision = match self {
                    Broker::Allow => "allow",
                    Broker::Deny => "deny",
                    _ => "ask",
                };
                let reason = match self {
                    Broker::Deny => "not while the harness watches",
                    _ => "ok by harness",
                };
                let body = format!(r#"{{"decision":"{decision}","reason":"{reason}","ttl":"60"}}"#);
                let head = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = conn.write_all(&head.into_bytes());
                let _ = conn.write_all(&body.into_bytes());
            }
        }
        let _ = conn.flush();
        let _ = conn.shutdown(std::net::Shutdown::Both);
    }
}

/// Spawn the broker thread and return once the socket is accepting
/// (poll-connect, bounded). The listener serves exactly one POST.
fn serve(claude_dir: &Path, scenario: Broker) {
    let sock = claude_dir.join("meta-broker.sock");
    let _ = std::fs::remove_file(&sock);
    let listener = UnixListener::bind(&sock).expect("bind the broker socket");
    let sock_for_thread = sock.clone();
    std::thread::spawn(move || {
        // The readiness probe (below) opens a connection it never writes
        // to; skip such empty accepts so the hook's real POST is the one
        // that gets answered.
        loop {
            let Ok((mut conn, _)) = listener.accept() else {
                return;
            };
            let mut buf = Vec::new();
            let mut chunk = [0u8; 4096];
            loop {
                match conn.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(n) => {
                        buf.extend_from_slice(&chunk[..n]);
                        let text = String::from_utf8_lossy(&buf).to_string();
                        let cl = text
                            .to_ascii_lowercase()
                            .split("\r\n")
                            .find_map(|l| l.strip_prefix("content-length: "))
                            .and_then(|v| v.trim().parse::<usize>().ok());
                        let complete = cl.zip(text.find("\r\n\r\n")).is_some_and(
                            |(cl, pos)| buf.len() >= pos + 4 + cl,
                        );
                        if complete {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            if buf.is_empty() {
                continue; // a readiness probe, not the hook's POST
            }
            scenario.reply(&mut conn);
            break;
        }
        // Keep the socket file present for the hook's whole run; remove it
        // only after the reply landed.
        std::thread::sleep(Duration::from_millis(250));
        let _ = std::fs::remove_file(&sock_for_thread);
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    while UnixStream::connect(&sock).is_err() {
        assert!(Instant::now() < deadline, "broker socket never accepted");
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// The allowed decision is honored: Claude's permissionDecision document on
/// stdout, exit 0, and exactly one valid JSONL audit line carrying the
/// decision, the reason, a numeric latency, the tool, and the contract
/// version.
#[test]
fn allow_reply_brokers_the_decision_and_audits() {
    let (_home, claude_dir) = scratch();
    serve(&claude_dir, Broker::Allow);
    let (code, stdout) = run_hook(&claude_dir);
    assert_eq!(code, Some(0));
    let doc: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout is the permissionDecision document");
    assert_eq!(doc["hookSpecificOutput"]["hookEventName"], "PreToolUse");
    assert_eq!(doc["hookSpecificOutput"]["permissionDecision"], "allow");
    assert_eq!(doc["hookSpecificOutput"]["permissionDecisionReason"], "ok by harness");
    let lines = audit_lines(&claude_dir);
    assert_eq!(lines.len(), 1, "exactly one audit line for one decision");
    let line = &lines[0];
    assert_eq!(line["decision"], "allow");
    assert_eq!(line["tool_name"], "Bash");
    assert_eq!(line["reason"], "ok by harness");
    assert_eq!(line["ttl"], "60");
    assert_eq!(line["contract_version"], CONTRACT_VERSION);
    assert!(line["broker_latency_ms"].is_number(), "latency is numeric");
    assert!(line["ts"].is_string(), "ts present");
    assert!(
        line["input_summary"].as_str().map(|s| s.len() <= 2004).unwrap_or(false),
        "input_summary bounded"
    );
    assert_eq!(line["session_id"], "sess-7");
    assert_eq!(line["cwd"], "/repo");
}

/// The denied decision is honored with the broker's reason — and the
/// permissionDecision document carries it (Claude shows the harness's
/// reason, not a generic refusal).
#[test]
fn deny_reply_brokers_the_decision_with_the_reason() {
    let (_home, claude_dir) = scratch();
    serve(&claude_dir, Broker::Deny);
    let (code, stdout) = run_hook(&claude_dir);
    assert_eq!(code, Some(0));
    let doc: serde_json::Value = serde_json::from_str(&stdout).expect("deny document parses");
    assert_eq!(doc["hookSpecificOutput"]["permissionDecision"], "deny");
    assert_eq!(
        doc["hookSpecificOutput"]["permissionDecisionReason"],
        "not while the harness watches"
    );
    let lines = audit_lines(&claude_dir);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0]["decision"], "deny");
    assert_eq!(lines[0]["reason"], "not while the harness watches");
}

/// `ask` falls through to the stock prompt path: NO stdout, exit 0 — but
/// the decision was brokered, so the audit line IS its record.
#[test]
fn ask_reply_falls_through_to_the_stock_path_and_still_audits() {
    let (_home, claude_dir) = scratch();
    serve(&claude_dir, Broker::Ask);
    let (code, stdout) = run_hook(&claude_dir);
    assert_eq!(code, Some(0));
    assert!(stdout.is_empty(), "ask prints nothing — the stock prompt runs");
    let lines = audit_lines(&claude_dir);
    assert_eq!(lines.len(), 1, "the brokered ask is audited");
    assert_eq!(lines[0]["decision"], "ask");
}

/// A broker slower than the 800ms budget fails open: the hook finishes long
/// before the 3s stall ends, prints nothing, and appends nothing.
///
/// The budget is pinned TWO-SIDED at its real granularity (the t341
/// validator's finding 3: a lone `<2s` bound cannot tell an 800ms budget
/// from a 1900ms one). The exact "<5ms" number the spec pins is the
/// ABSENT-socket latency ADDED over a no-op hook — pinned by
/// `an_absent_socket_is_the_stock_path_with_zero_added_latency` below,
/// where both legs pay the identical spawn+parse cost. The timeout path
/// cannot be pinned that exactly: it inherently pays one `sh` + `python3`
/// spawn and a connect on top of the 800ms wait, so the mechanically
/// assertable pin is a band on the BEST of 3 runs (best-of-3 so one
/// scheduler hiccup inflating a single spawn is not the hook's doing):
/// - LOWER 750ms: the hook genuinely waited out the stalled broker — an
///   early-fail-open mutant (timeout never armed, connect aborted) would
///   finish in ~100ms;
/// - UPPER 1300ms: the 800ms budget plus ~500ms of spawn/connect slack —
///   a budget mutant >= 1.3s, or the broker's 3s stall leaking through,
///   cannot fit under it.
#[test]
fn a_slow_broker_times_out_to_the_stock_path() {
    let mut best: Option<Duration> = None;
    for i in 0..3 {
        // A fresh scratch + socket per run: the Slow broker thread holds
        // its socket for the whole 3s stall, so a second run against the
        // same path would race the previous thread's teardown.
        let (_home, claude_dir) = scratch();
        serve(&claude_dir, Broker::Slow);
        let (exited, code, stdout, elapsed) =
            run_hook_bounded(&claude_dir, Duration::from_millis(2500));
        assert!(exited, "run {i}: the hook must not wait out the broker's stall");
        assert_eq!(code, Some(0), "run {i}: fail-open exit");
        assert!(stdout.is_empty(), "run {i}: stock path: no output");
        assert!(
            !audit_path(&claude_dir).exists(),
            "run {i}: no audit line for a stock-path outcome"
        );
        best = Some(best.map_or(elapsed, |b: Duration| b.min(elapsed)));
    }
    let best = best.expect("three runs ran");
    assert!(
        best >= Duration::from_millis(750),
        "the hook finished in {best:?} — the 800ms budget did not bound the \
         wait on the stalled broker (an early fail-open?)"
    );
    assert!(
        best < Duration::from_millis(1300),
        "best of 3 was {best:?} — the stock-path fail-open must sit inside \
         the 800ms budget + ~500ms spawn/connect slack; a longer wait means \
         the budget mutated or the broker's 3s stall leaked through"
    );
}

/// A malformed reply body is the stock path (fail-open) — exit 0, no
/// output, no audit. The broker crash / garbage-reply scenario can never
/// wedge a session.
#[test]
fn a_malformed_reply_is_the_stock_path() {
    let (_home, claude_dir) = scratch();
    serve(&claude_dir, Broker::Malformed);
    let (code, stdout) = run_hook(&claude_dir);
    assert_eq!(code, Some(0));
    assert!(stdout.is_empty());
    assert!(!audit_path(&claude_dir).exists());
}

/// A non-200 and a 200-without-decision are both the stock path — only the
/// decision gates brokered-vs-stock.
#[test]
fn non_200_and_decisionless_replies_are_the_stock_path() {
    for scenario in [Broker::BadStatus, Broker::NoDecision] {
        let (_home, claude_dir) = scratch();
        serve(&claude_dir, scenario);
        let (code, stdout) = run_hook(&claude_dir);
        assert_eq!(code, Some(0));
        assert!(stdout.is_empty());
        assert!(!audit_path(&claude_dir).exists());
    }
}

/// The socket-absent fast path adds ZERO latency to the stock path: the
/// hook's best-of-6 sits within 5ms of a no-op hook's best-of-6, measured
/// INTERLEAVED so box load inflates both legs together (a scheduler hiccup
/// is not the hook's doing — the spec's "<5ms" is about latency ADDED, and
/// the no-op script pays the identical spawn+parse cost). No fork, no
/// output, and NO audit file — stock-path decisions append nothing (the
/// audit fixture's quiet half).
#[test]
fn an_absent_socket_is_the_stock_path_with_zero_added_latency() {
    let (_home, claude_dir) = scratch();
    let hook = pack_dir().join("hook.sh");
    let no_op = claude_dir.join("no-op-hook.sh");
    std::fs::write(&no_op, "#!/bin/sh\nexit 0\n").unwrap();
    let best = |script: &Path, claude_dir: &Path| -> Duration {
        let mut best = Duration::from_secs(3600);
        for i in 0..6 {
            let started = Instant::now();
            let mut child = Command::new("/bin/sh")
                .arg(script)
                .env("CLAUDE_CONFIG_DIR", claude_dir)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .expect("spawn hook");
            child
                .stdin
                .take()
                .unwrap()
                .write_all(payload().as_bytes())
                .ok();
            let out = child.wait_with_output().expect("hook runs");
            let elapsed = started.elapsed();
            assert!(out.status.success(), "run {i}");
            assert!(
                out.stdout.is_empty(),
                "run {i}: stock path prints nothing: {}",
                String::from_utf8_lossy(&out.stdout)
            );
            if elapsed < best {
                best = elapsed;
            }
        }
        best
    };
    let floor = best(&no_op, &claude_dir);
    let hook_best = best(&hook, &claude_dir);
    let added = hook_best.saturating_sub(floor);
    assert!(
        added < Duration::from_millis(5),
        "the absent-socket fast path must add <5ms over a no-op hook \
         (no-op best {floor:?}, hook best {hook_best:?}, added {added:?})"
    );
    assert!(
        !audit_path(&claude_dir).exists(),
        "the stock path appends nothing"
    );
}
