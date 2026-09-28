//! T124 — F10 phase 1: the `chug mcp-serve` end-to-end leg.
//!
//! One real spawn of the built binary (`CARGO_BIN_EXE_chug mcp-serve`) with
//! piped stdio — the manual smoke shape from the spec, executable: a
//! Claude Code-compatible stdio client's exact conversation
//! (`initialize` → `notifications/initialized` → `tools/list` →
//! `tools/call chug_status` against a tempdir fixture `.chug/`), then
//! stdin closed → exit success.
//!
//! Pins at the WIRE level, which the bin-internal unit legs cannot reach:
//! - the notification produces NO response — the responses that do arrive
//!   carry ids 1, 2, 3 in order, and nothing else arrives before EOF;
//! - stdout purity — every byte read parses as the expected envelope (a
//!   stray banner byte would corrupt the first read and fail here);
//! - EOF lifecycle — closing stdin ends the process with a zero exit.
//!
//! Bounded reads throughout (the T6 stub-harness rule): every read waits at
//! most [`READ_DEADLINE`] and the exit poll at most [`EXIT_DEADLINE`] — a
//! wedged server fails this test in seconds, it can never hang the suite.

use std::io::Write as _;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;

/// Per-read deadline: a healthy server answers a read-only call in
/// milliseconds; ten seconds is orders of magnitude of slack.
const READ_DEADLINE: Duration = Duration::from_secs(10);
/// The no-reply probe window: long enough that a wrongly-answered
/// notification would have arrived, short enough to keep the suite fast.
const SILENCE_PROBE: Duration = Duration::from_millis(400);
/// Deadline for the process to exit after stdin closes.
const EXIT_DEADLINE: Duration = Duration::from_secs(10);

/// Spawn the server with piped stdio and a background line-reader thread,
/// so every read is deadline-bounded.
fn spawn_server() -> (Child, mpsc::Receiver<String>) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_chug"))
        .arg("mcp-serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn CARGO_BIN_EXE_chug mcp-serve");
    let stdout = child.stdout.take().expect("piped stdout");
    let (tx, rx) = mpsc::channel::<String>();
    thread::spawn(move || {
        use std::io::BufRead;
        let reader = std::io::BufReader::new(stdout);
        for line in reader.lines() {
            match line {
                Ok(line) => {
                    if tx.send(line).is_err() {
                        break; // test side gone; stop reading
                    }
                }
                Err(_) => break, // pipe closed
            }
        }
    });
    (child, rx)
}

/// Write one protocol line to the server's stdin and flush.
fn send(stdin: &mut std::process::ChildStdin, line: &str) {
    stdin
        .write_all(line.as_bytes())
        .expect("write to server stdin");
    stdin.write_all(b"\n").expect("write newline");
    stdin.flush().expect("flush stdin");
}

/// One deadline-bounded read of the next response line.
fn next_response(rx: &mpsc::Receiver<String>, what: &str) -> Value {
    let line = match rx.recv_timeout(READ_DEADLINE) {
        Ok(line) => line,
        Err(RecvTimeoutError::Timeout) => {
            panic!("timed out waiting for {what} — wedged or missing server response")
        }
        Err(RecvTimeoutError::Disconnected) => {
            panic!("server stdout closed before {what} arrived")
        }
    };
    serde_json::from_str(&line).unwrap_or_else(|e| {
        panic!(
            "response line for {what} is not valid JSON (stdout purity broken?): {e}\nline: {line}"
        )
    })
}

/// The `.chug/events.jsonl` fixture: a run_start + iterations + a goal line
/// — the LATEST-segment shape `chug_status` summarizes.
fn write_fixture(cwd: &std::path::Path) {
    std::fs::create_dir_all(cwd.join(".chug")).expect("create fixture .chug");
    let lines = [
        r#"{"type":"run_start","ts":"t0","mode":"run","model":"m","max_iters":30,"max_minutes":35,"max_tokens":null}"#,
        r#"{"type":"iteration","ts":"t1","n":2,"input_tokens":1,"output_tokens":1}"#,
        r#"{"type":"iteration","ts":"t2","n":3,"input_tokens":1,"output_tokens":1}"#,
        r#"{"type":"goal","ts":"t3","outcome":"accepted","summary":"fixture done"}"#,
    ];
    let body: String = lines.iter().map(|l| format!("{l}\n")).collect();
    std::fs::write(cwd.join(".chug/events.jsonl"), body).expect("write fixture events");
}

#[test]
fn mcp_serve_full_client_conversation_then_eof_exit() {
    let tmp = tempfile::tempdir().expect("tempdir");
    write_fixture(tmp.path());
    let cwd = tmp.path().display().to_string();
    let (mut child, rx) = spawn_server();

    {
        let mut stdin = child.stdin.take().expect("piped stdin");
        // 1. Handshake — exactly what a Claude Code-compatible client sends.
        send(
            &mut stdin,
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"claude-code","version":"1.0.0"}}}"#,
        );
        // 2. The handshake notification — must produce NO response.
        send(&mut stdin, r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#);
        // 3. Tool discovery.
        send(&mut stdin, r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#);
        // 4. The one read-only tool, against the fixture cwd.
        let call = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {"name": "chug_status", "arguments": {"cwd": cwd}}
        });
        send(&mut stdin, &call.to_string());
    }

    // --- read the responses IN ORDER. Three arrive; the notification's
    // non-response is pinned by the ids below being 1, 2, 3.
    let init = next_response(&rx, "initialize response");
    assert_eq!(init["id"], 1, "first response is the initialize one: {init}");
    assert_eq!(init["result"]["protocolVersion"], "2025-06-18", "{init}");
    assert_eq!(init["result"]["serverInfo"]["name"], "chug", "{init}");
    assert!(
        init["result"]["capabilities"].get("tools").is_some(),
        "capabilities.tools present: {init}"
    );
    assert!(init.get("error").is_none(), "handshake has no error: {init}");

    let list = next_response(&rx, "tools/list response");
    assert_eq!(list["id"], 2, "second response is tools/list (the notification got none): {list}");
    let tools = list["result"]["tools"]
        .as_array()
        .expect("tools array")
        .clone();
    assert_eq!(tools.len(), 2, "phases 1+2a ship exactly two tools: {list}");
    assert_eq!(tools[0]["name"], "chug_status");
    assert_eq!(tools[1]["name"], "chug_collect");
    for tool in &tools {
        let required = tool["inputSchema"]["required"]
            .as_array()
            .expect("required array")
            .clone();
        assert!(
            required.iter().any(|v| v == "cwd"),
            "inputSchema requires cwd: {tools:?}"
        );
    }

    let call = next_response(&rx, "tools/call response");
    assert_eq!(call["id"], 3, "{call}");
    assert!(call.get("error").is_none(), "a bad cwd is a tool error, not here: {call}");
    let text = call["result"]["content"][0]["text"]
        .as_str()
        .expect("text content block")
        .to_string();
    assert_eq!(call["result"]["isError"], false, "{call}");
    // The summary names the fixture's state and iteration counts.
    assert!(text.contains("state: done"), "{text}");
    assert!(text.contains("iteration: 3/30"), "{text}");
    assert!(text.contains("goal_seen: true"), "{text}");

    // And NOTHING else arrives: a fourth response would mean the server
    // answered the notification (framing corruption for a real client).
    match rx.recv_timeout(SILENCE_PROBE) {
        Err(RecvTimeoutError::Timeout) => {} // correct silence
        Err(RecvTimeoutError::Disconnected) => {} // EOF also fine pre-close
        Ok(unexpected) => {
            panic!("unexpected extra response (did the notification get a reply?): {unexpected}")
        }
    }

    // --- EOF lifecycle: close stdin → the server exits 0, in bounded time.
    drop(child.stdin.take());
    let deadline = Instant::now() + EXIT_DEADLINE;
    let status = loop {
        if let Some(status) = child.try_wait().expect("try_wait") {
            break status;
        }
        assert!(
            Instant::now() < deadline,
            "server did not exit within {EXIT_DEADLINE:?} of stdin EOF"
        );
        thread::sleep(Duration::from_millis(25));
    };
    assert!(
        status.success(),
        "EOF must exit 0, got: {status}"
    );
}

/// The error taxonomy over the real wire: garbage and unknown methods keep
/// the server alive (errors never kill the loop) and get the spec'd codes.
#[test]
fn mcp_serve_errors_keep_the_loop_alive_over_the_real_wire() {
    let (_child, rx) = {
        let mut child = Command::new(env!("CARGO_BIN_EXE_chug"))
            .arg("mcp-serve")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn mcp-serve");
        let stdout = child.stdout.take().expect("piped stdout");
        let (tx, rx) = mpsc::channel::<String>();
        thread::spawn(move || {
            use std::io::BufRead;
            // map_while, not flatten(): a read error must END the reader
            // thread, never spin on a permanently-Err iterator.
            for line in std::io::BufReader::new(stdout).lines().map_while(Result::ok) {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        let mut stdin = child.stdin.take().expect("piped stdin");
        send(&mut stdin, "{ not json !!!");
        send(&mut stdin, r#"{"jsonrpc":"2.0","id":2,"method":"resources/list"}"#);
        send(&mut stdin, r#"{"jsonrpc":"2.0","id":4,"method":"ping"}"#);
        (child, rx)
    };
    // Parse error → -32700 with id null.
    let resp = next_response(&rx, "parse-error response");
    assert_eq!(resp["id"], Value::Null, "{resp}");
    assert_eq!(resp["error"]["code"], -32700, "{resp}");
    // Unknown method → -32601, and the loop is still alive afterwards.
    let resp = next_response(&rx, "unknown-method response");
    assert_eq!(resp["id"], 2, "{resp}");
    assert_eq!(resp["error"]["code"], -32601, "{resp}");
    // The proof the errors never killed the loop: a later valid ping
    // answers normally.
    let resp = next_response(&rx, "ping after errors");
    assert_eq!(resp["id"], 4, "{resp}");
    assert_eq!(resp["result"], serde_json::json!({}), "{resp}");
    // _child dropped here: stdin closes (EOF) and the process reaps.
}

/// T128 — the phase-2a wire leg: a real server, the full client
/// conversation (`initialize` → `tools/list` → `tools/call chug_collect`)
/// against a tempdir fixture, deadline-bounded throughout. Pins at the WIRE
/// level: the collect result carries the verdict, the accepted goal's
/// summary and the check cmd, and the not-a-repo git leg degrades to a note
/// without failing the call.
#[test]
fn mcp_serve_chug_collect_full_conversation_over_the_wire() {
    let tmp = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(tmp.path().join(".chug")).expect("create fixture .chug");
    let lines = [
        r#"{"type":"run_start","ts":"t0","mode":"run","model":"m","max_iters":30,"max_minutes":35,"max_tokens":null}"#,
        r#"{"type":"verifying","ts":"t2","cmd":"cargo test"}"#,
        r#"{"type":"goal","ts":"t3","outcome":"accepted","summary":"fixture collected"}"#,
    ];
    let body: String = lines.iter().map(|l| format!("{l}\n")).collect();
    std::fs::write(tmp.path().join(".chug/events.jsonl"), body).expect("write fixture events");
    let cwd = tmp.path().display().to_string();
    let (mut child, rx) = spawn_server();

    {
        let mut stdin = child.stdin.take().expect("piped stdin");
        send(
            &mut stdin,
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
        );
        send(&mut stdin, r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#);
        let call = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {"name": "chug_collect", "arguments": {"cwd": cwd}}
        });
        send(&mut stdin, &call.to_string());
    }

    let init = next_response(&rx, "initialize response");
    assert_eq!(init["id"], 1, "{init}");
    assert!(init.get("error").is_none(), "{init}");

    let list = next_response(&rx, "tools/list response");
    assert_eq!(list["id"], 2, "{list}");
    let names: Vec<&str> = list["result"]["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    assert_eq!(names, ["chug_status", "chug_collect"], "{list}");

    let call = next_response(&rx, "tools/call chug_collect response");
    assert_eq!(call["id"], 3, "{call}");
    assert!(call.get("error").is_none(), "the call succeeds: {call}");
    assert_eq!(call["result"]["isError"], false, "{call}");
    let text = call["result"]["content"][0]["text"]
        .as_str()
        .expect("text content block")
        .to_string();
    // The structured result: verdict, accepted summary, check cmd.
    assert!(text.contains("verdict: goal-accepted"), "{text}");
    assert!(text.contains("summary: fixture collected"), "{text}");
    assert!(text.contains("check_cmd: cargo test"), "{text}");
    // Not-a-repo tempdir: the commit-refs section degrades to a note.
    assert!(text.contains("commits: (unavailable:"), "{text}");

    // EOF lifecycle: close stdin → the server exits 0, in bounded time.
    drop(child.stdin.take());
    let deadline = Instant::now() + EXIT_DEADLINE;
    let status = loop {
        if let Some(status) = child.try_wait().expect("try_wait") {
            break status;
        }
        assert!(
            Instant::now() < deadline,
            "server did not exit within {EXIT_DEADLINE:?} of stdin EOF"
        );
        thread::sleep(Duration::from_millis(25));
    };
    assert!(status.success(), "EOF must exit 0, got: {status}");
}
