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
//!
//! T148 adds the `chug_launch` wire e2e (T129's descoped follow-up): the
//! REAL server over REAL stdio with the `CHUG_DELEGATE_BIN` seam pointing
//! at a stub child that records its argv+env and exits 0 — real wire, real
//! server, real spawn, stubbed child (no model endpoint). Three legs: the
//! `--allow-launch` happy path (flag → advertised ⇔ callable → the exact
//! values the wire carried reach the child argv), the refused-launch
//! `isError` arm, and the default-deny policy boundary. Per the carried
//! T129 nit, NO sub-ms events ordering is asserted anywhere: every launch
//! effect is polled with a bounded deadline for its eventual appearance
//! (or, for the refused launch, its bounded absence).

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
    spawn_server_with(&[], &[])
}

/// The generalization the launch legs ride (T148): extra CLI args (the
/// `--allow-launch` flag) and extra env scoped to THIS ONE SERVER via
/// `Command::env` — never the test process's global env, which parallel
/// test threads share. The server inherits the seam var and passes it down
/// to the stub child through the delegate launch path.
fn spawn_server_with(args: &[&str], env: &[(&str, &str)]) -> (Child, mpsc::Receiver<String>) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_chug"));
    command.arg("mcp-serve").args(args);
    for (key, value) in env {
        command.env(key, value);
    }
    let mut child = command
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

// ---------------------------------------------------------------------------
// T148 — the chug_launch wire e2e: real stdio, real server, stubbed child
// ---------------------------------------------------------------------------

/// The bounded poll for the stub's record: launch returns at spawn and the
/// stub writes its record milliseconds later; five seconds (the spec's ~5 s)
/// is orders of magnitude of slack while keeping the suite fast.
const LAUNCH_RECORD_DEADLINE: Duration = Duration::from_secs(5);
/// The refused-launch absence window (the failure leg's negative proof): a
/// wrongly-spawned stub records itself within milliseconds, so a full second
/// of absence is the bounded "nothing spawned" evidence (the T6 rule — the
/// probe is bounded, never unbounded).
const NO_SPAWN_PROBE: Duration = Duration::from_millis(1000);
/// The launch legs' serialization (the T129 `DELEGATE_ENV_LOCK` pattern).
/// The seam env var here is per-`Command` (scoped to the one spawned
/// server), so no process-global mutation can race — the lock is kept
/// anyway so exactly one launch leg touches the delegate launch seam at a
/// time and their bounded polls never interleave, mirroring the in-process
/// seam legs. The default-deny leg never touches the seam and skips it.
static LAUNCH_LEG_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// The wire-carried goal: spaces and an embedded double quote — the argv
/// mechanism must deliver it byte-exact, with no quoting code anywhere
/// (the same pressure the T129 in-process stub leg applies, now over the
/// real wire). Shorter than the 120-char `goal_tail` echo window, so the
/// launch payload carries it verbatim too.
const GOAL: &str = "t148 wire goal with spaces and \"quotes\"";

/// Deadline-bounded EOF lifecycle shared by the launch legs: close stdin,
/// wait for the exit, assert success. The `what` names the leg in both
/// panic paths.
fn close_stdin_and_expect_success_exit(mut child: Child, what: &str) {
    drop(child.stdin.take());
    let deadline = Instant::now() + EXIT_DEADLINE;
    let status = loop {
        if let Some(status) = child.try_wait().expect("try_wait") {
            break status;
        }
        assert!(
            Instant::now() < deadline,
            "{what}: server did not exit within {EXIT_DEADLINE:?} of stdin EOF"
        );
        thread::sleep(Duration::from_millis(25));
    };
    assert!(status.success(), "{what}: EOF must exit 0, got: {status}");
}

/// The launch-leg fixture, one part per line: the spec file the launch must
/// carry, and the stub child script the `CHUG_DELEGATE_BIN` seam substitutes
/// for the real binary (the T126 idiom). Both live in their own unique
/// tempdirs — parallel-safe, and the tempdir drop cleans every stub file.
#[cfg(unix)]
fn write_launch_spec_and_stub(scratch: &std::path::Path) -> (std::path::PathBuf, std::path::PathBuf) {
    let spec = scratch.join("t148-wire-spec.md");
    std::fs::write(&spec, "# T148 wire e2e spec\n").expect("write spec file");
    // The stub records its cwd, the inherited seam env var, and its full
    // argv into `stub-record.txt` IN ITS OWN CWD — which is the launch
    // target cwd — then exits 0: no model endpoint, no lingering process,
    // nothing to reap.
    let stub = scratch.join("t148-launch-stub.sh");
    std::fs::write(
        &stub,
        "#!/bin/sh\n\
         {\n\
         printf 'cwd: %s\\n' \"$(pwd -P)\"\n\
         printf 'env: CHUG_DELEGATE_BIN=%s\\n' \"${CHUG_DELEGATE_BIN-unset}\"\n\
         printf 'argv:\\n'\n\
         printf '%s\\n' \"$@\"\n\
         } > stub-record.txt\n\
         exit 0\n",
    )
    .expect("write stub script");
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755))
        .expect("make the stub executable");
    (spec, stub)
}

/// Bounded poll (~5 s per the spec) for the stub's record, compared against
/// the COMPLETE expected record — a torn mid-write read just keeps polling.
/// On deadline the panic carries the last partial read for diagnosis.
#[cfg(unix)]
fn wait_for_stub_record(path: &std::path::Path, expected: &[String]) {
    let deadline = Instant::now() + LAUNCH_RECORD_DEADLINE;
    let mut last: Vec<String> = Vec::new();
    loop {
        if let Ok(text) = std::fs::read_to_string(path) {
            let lines: Vec<String> = text.lines().map(str::to_string).collect();
            if lines == expected {
                return;
            }
            last = lines;
        }
        assert!(
            Instant::now() < deadline,
            "stub record never appeared/matched at {} within {LAUNCH_RECORD_DEADLINE:?} (last partial: {last:?})",
            path.display()
        );
        thread::sleep(Duration::from_millis(25));
    }
}

/// The failure leg's bounded negative: the refused launch must spawn
/// NOTHING — the stub's record path stays absent for the whole probe
/// window. (A validation-neutered mutant reaches the seam and spawns the
/// stub within milliseconds, far inside the window, so the absence is
/// evidence, not a race.)
#[cfg(unix)]
fn assert_no_stub_record(path: &std::path::Path) {
    let deadline = Instant::now() + NO_SPAWN_PROBE;
    while Instant::now() < deadline {
        assert!(
            !path.exists(),
            "a refused launch must not spawn anything — found {}",
            path.display()
        );
        thread::sleep(Duration::from_millis(25));
    }
}

/// Leg 1 — the M7 kill: the `--allow-launch` CLI plumbing, END TO END over a
/// real stdio wire. The flag on the real server's argv → `chug_launch`
/// advertised in `tools/list` → callable in `tools/call` → the ONE delegate
/// launch path spawns the stub child carrying the EXACT spec/goal/model/
/// budget values the wire received. The response's pid/log/events payload
/// is asserted, the stub's argv record is polled with a bounded deadline
/// (never a sub-ms ordering assertion — the carried T129 nit), and stdin
/// EOF still exits 0. A mutant anywhere on that chain — flag dropped,
/// tool unadvertised, unroutable, budgets swapped/dropped, argv reordered —
/// fails one of these wire assertions.
#[cfg(unix)]
#[test]
fn mcp_serve_chug_launch_happy_path_over_the_real_wire() {
    let _guard = LAUNCH_LEG_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let scratch = tempfile::tempdir().expect("scratch tempdir");
    let target = tempfile::tempdir().expect("target tempdir");
    std::fs::create_dir_all(target.path().join(".chug")).expect("target .chug");
    let (spec, stub) = write_launch_spec_and_stub(scratch.path());

    // The flag and the stub seam are THIS server's argv/env only.
    let (mut child, rx) = spawn_server_with(
        &["--allow-launch"],
        &[("CHUG_DELEGATE_BIN", stub.to_str().expect("utf-8 stub path"))],
    );
    {
        let mut stdin = child.stdin.take().expect("piped stdin");
        send(&mut stdin, r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#);
        send(&mut stdin, r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#);
        let call = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {"name": "chug_launch", "arguments": {
                "cwd": target.path().display().to_string(),
                "spec": spec.display().to_string(),
                "goal": GOAL,
                "model": "wire-model",
                "max_iters": 7,
                "max_minutes": 9
            }}
        });
        send(&mut stdin, &call.to_string());
    }

    let init = next_response(&rx, "initialize response");
    assert_eq!(init["id"], 1, "{init}");
    assert!(init.get("error").is_none(), "{init}");

    // The flag advertised the write leg: three tools, and `chug_launch`'s
    // schema requires exactly the four required params.
    let list = next_response(&rx, "tools/list response");
    assert_eq!(list["id"], 2, "{list}");
    let tools = list["result"]["tools"].as_array().expect("tools array").clone();
    let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
    assert_eq!(
        names,
        ["chug_status", "chug_collect", "chug_launch", "chug_cancel"],
        "the flag advertised both write legs: {list}"
    );
    let launch = tools
        .iter()
        .find(|t| t["name"] == "chug_launch")
        .expect("the advertised chug_launch entry");
    assert_eq!(
        launch["inputSchema"]["required"],
        serde_json::json!(["cwd", "spec", "goal", "model"]),
        "{tools:?}"
    );

    // The call is a tool RESULT (not a JSON-RPC error) that is not isError,
    // and it carries the launch payload: a parseable pid plus real log and
    // events paths, and the wire-carried budgets echoed back.
    let call = next_response(&rx, "tools/call chug_launch response");
    assert_eq!(call["id"], 3, "{call}");
    assert!(
        call.get("error").is_none(),
        "a routed launch is a result, not a JSON-RPC error: {call}"
    );
    assert_eq!(call["result"]["isError"], false, "the launch succeeded: {call}");
    let text = call["result"]["content"][0]["text"]
        .as_str()
        .expect("text content block")
        .to_string();
    let pid: u32 = text
        .lines()
        .find_map(|l| l.strip_prefix("launched: pid "))
        .unwrap_or_else(|| panic!("pid line in the launch payload: {text}"))
        .trim()
        .parse()
        .unwrap_or_else(|e| panic!("pid does not parse: {e}\n{text}"));
    assert!(pid > 0, "a real spawned pid: {text}");
    let log_path = target.path().join(".chug/delegate.log");
    let events_path = target.path().join(".chug/events.jsonl");
    assert!(text.contains(&format!("log: {}", log_path.display())), "{text}");
    assert!(text.contains(&format!("events: {}", events_path.display())), "{text}");
    assert!(
        text.contains("model: wire-model max_iters: 7 max_minutes: 9"),
        "the wire-carried budgets echoed verbatim: {text}"
    );
    assert!(
        text.contains(GOAL),
        "the goal echo carries the wire-carried goal bytes: {text}"
    );
    assert!(
        log_path.is_file(),
        "the parent opened the child's log before spawn: {text}"
    );

    // THE CLI-PLUMBING PROOF (M7's killing evidence): within the bounded
    // poll the stub's record carries EXACTLY what the wire carried — the
    // full argv in delegate's order with the distinct budgets (7/9, not the
    // 40/35 defaults, not swapped), the child cwd INSIDE the target chug
    // dir, and the server's seam env inherited by the child.
    let expected = vec![
        format!("cwd: {}", std::fs::canonicalize(target.path()).expect("canonical target").display()),
        format!("env: CHUG_DELEGATE_BIN={}", stub.display()),
        "argv:".to_string(),
        "run".to_string(),
        "--spec".to_string(),
        spec.display().to_string(),
        "--goal".to_string(),
        GOAL.to_string(),
        "--model".to_string(),
        "wire-model".to_string(),
        "--max-iters".to_string(),
        "7".to_string(),
        "--max-minutes".to_string(),
        "9".to_string(),
    ];
    wait_for_stub_record(&target.path().join("stub-record.txt"), &expected);

    close_stdin_and_expect_success_exit(child, "happy path");
}

/// Leg 2 — the M8 kill: the launch-failure `isError` arm over the real wire.
/// A valid launch except `max_iters: 201` (above the 200 ceiling, the leg
/// the T129 unit tests pin least at the wire level) must come back as the
/// `isError: true` tool RESULT naming the RECEIVED value ("got 201") and the
/// ceiling, NOT a JSON-RPC error and NOT a launch — the stub's record stays
/// absent for the bounded probe, and the loop stays alive (the ping after
/// the refusal answers). The stub env is set on this server too, so a
/// validation-neutered mutant can only ever reach the recording stub, never
/// a real `chug run`.
#[cfg(unix)]
#[test]
fn mcp_serve_chug_launch_budget_above_ceiling_is_error_arm_over_the_wire() {
    let _guard = LAUNCH_LEG_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let scratch = tempfile::tempdir().expect("scratch tempdir");
    let target = tempfile::tempdir().expect("target tempdir");
    std::fs::create_dir_all(target.path().join(".chug")).expect("target .chug");
    let (spec, stub) = write_launch_spec_and_stub(scratch.path());

    let (mut child, rx) = spawn_server_with(
        &["--allow-launch"],
        &[("CHUG_DELEGATE_BIN", stub.to_str().expect("utf-8 stub path"))],
    );
    {
        let mut stdin = child.stdin.take().expect("piped stdin");
        send(&mut stdin, r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#);
        let call = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {"name": "chug_launch", "arguments": {
                "cwd": target.path().display().to_string(),
                "spec": spec.display().to_string(),
                "goal": "t148 refused goal",
                "model": "wire-model",
                "max_iters": 201
            }}
        });
        send(&mut stdin, &call.to_string());
        // Sent AFTER the refused call: the proof the tool failure did not
        // kill the server loop.
        send(&mut stdin, r#"{"jsonrpc":"2.0","id":3,"method":"ping"}"#);
    }

    let init = next_response(&rx, "initialize response");
    assert_eq!(init["id"], 1, "{init}");

    // The refusal is the isError arm: a tool RESULT carrying the
    // received-value error text — the received value (201) and the ceiling
    // (200), tool-named, never clamped, never launched.
    let call = next_response(&rx, "refused launch response");
    assert_eq!(call["id"], 2, "{call}");
    assert!(
        call.get("error").is_none(),
        "a validation failure is a tool RESULT, not a JSON-RPC error: {call}"
    );
    assert_eq!(
        call["result"]["isError"], true,
        "the above-ceiling budget must land in the isError arm: {call}"
    );
    let text = call["result"]["content"][0]["text"]
        .as_str()
        .expect("text content block")
        .to_string();
    assert!(text.starts_with("chug_launch:"), "tool-named error: {text}");
    assert!(
        text.contains("`max_iters` must be at most 200"),
        "the ceiling named: {text}"
    );
    assert!(text.contains("got 201"), "the RECEIVED value named: {text}");

    // And NOTHING spawned: the stub's record stays absent for the whole
    // bounded probe (the refused payload never reached the launch seam).
    assert_no_stub_record(&target.path().join("stub-record.txt"));

    // The loop survived the tool failure: the ping after the refusal
    // answers normally.
    let ping = next_response(&rx, "ping after the refused launch");
    assert_eq!(ping["id"], 3, "{ping}");
    assert_eq!(ping["result"], serde_json::json!({}), "{ping}");

    close_stdin_and_expect_success_exit(child, "failure leg");
}

/// Leg 3 — the policy boundary, proven over the real wire: WITHOUT
/// `--allow-launch` the write leg does not exist. `tools/list` names
/// EXACTLY the two read-only tools (`chug_launch` absent — a mutant that
/// advertises it unconditionally dies on the exact-list equality), and a
/// `tools/call` for it gets the SAME unknown-tool `-32602` a never-existing
/// tool gets — a read-only deployment cannot probe the flag into revealing
/// the tool. The call's arguments are deliberately non-absolute, so even a
/// gate-neutered mutant would fail validation without spawning anything.
#[test]
fn mcp_serve_chug_launch_default_deny_over_the_real_wire() {
    let (mut child, rx) = spawn_server_with(&[], &[]);
    {
        let mut stdin = child.stdin.take().expect("piped stdin");
        send(&mut stdin, r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#);
        send(&mut stdin, r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#);
        let call = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {"name": "chug_launch", "arguments": {
                "cwd": "relative/path", "spec": "relative/spec",
                "goal": "g", "model": "m"
            }}
        });
        send(&mut stdin, &call.to_string());
    }

    let init = next_response(&rx, "initialize response");
    assert_eq!(init["id"], 1, "{init}");

    // Not advertised: the read-only tool set, exactly as pre-T129.
    let list = next_response(&rx, "tools/list response");
    assert_eq!(list["id"], 2, "{list}");
    let names: Vec<&str> = list["result"]["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    assert_eq!(
        names,
        ["chug_status", "chug_collect"],
        "chug_launch must be ABSENT from the default-deny tool set: {list}"
    );

    // Not callable: the unknown-tool error, indistinguishable from a tool
    // that never existed.
    let call = next_response(&rx, "default-deny tools/call response");
    assert_eq!(call["id"], 3, "{call}");
    let error = call
        .get("error")
        .unwrap_or_else(|| panic!("the default-deny call must be the unknown-tool error: {call}"));
    assert_eq!(error["code"], -32602, "{call}");
    let message = error["message"].as_str().expect("error message string");
    assert!(message.contains("unknown tool"), "{call}");
    assert!(message.contains("chug_launch"), "{call}");

    close_stdin_and_expect_success_exit(child, "default-deny");
}

// ---------------------------------------------------------------------------
// T153 — the chug_cancel wire e2e: real stdio, real server, stubbed child
// ---------------------------------------------------------------------------

/// The cancel legs' fixture: the spec file the launch must carry, and the
/// stub child the `CHUG_DELEGATE_BIN` seam substitutes for the real binary.
/// Unlike the T148 launch stub (records + exits 0), THIS stub records its
/// argv and then `sleep 60`s — a LIVE delegate-shaped child for the cancel
/// to stop (a dead pid never reaches the signal legs). Both live in their
/// own unique tempdirs; the tempdir drop cleans every stub file.
#[cfg(unix)]
fn write_cancel_spec_and_stub(scratch: &std::path::Path) -> (std::path::PathBuf, std::path::PathBuf) {
    let spec = scratch.join("t153-wire-spec.md");
    std::fs::write(&spec, "# T153 cancel wire spec\n").expect("write spec file");
    let stub = scratch.join("t153-cancel-stub.sh");
    std::fs::write(
        &stub,
        "#!/bin/sh\n\
         { printf 'argv:\\n'; printf '%s\\n' \"$@\"; } > stub-record.txt\n\
         sleep 60\n",
    )
    .expect("write stub script");
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755))
        .expect("make the stub executable");
    (spec, stub)
}

/// Bounded poll (~5 s, the T148 cadence) until `probe(pid)` turns true.
#[cfg(unix)]
fn poll_pid_state(pid: u32, want_alive: bool, what: &str) {
    let deadline = Instant::now() + LAUNCH_RECORD_DEADLINE;
    // SAFETY: kill(pid, 0) — a pure liveness probe, no signal delivered.
    let alive = |pid: u32| unsafe { libc::kill(pid as i32, 0) == 0 };
    while alive(pid) != want_alive {
        assert!(
            Instant::now() < deadline,
            "{what}: pid {pid} never became {} within {LAUNCH_RECORD_DEADLINE:?}",
            if want_alive { "alive" } else { "dead" }
        );
        thread::sleep(Duration::from_millis(25));
    }
}

/// Leg 1 — the cancel happy path over a REAL stdio wire: the server with
/// `--allow-launch` launches a LIVE stub child through the ONE delegate
/// launch path (so the child IS its own process-group leader and its argv
/// is delegate-shaped `run --spec …` — both ownership legs hold for real),
/// `tools/list` advertises `chug_cancel` beside `chug_launch`, and the
/// cancel call returns the payload with the stub PROVABLY dead afterwards.
/// Deadline-bounded throughout — no sub-ms ordering assertions (the carried
/// T129 nit).
#[cfg(unix)]
#[test]
fn mcp_serve_chug_cancel_happy_path_over_the_real_wire() {
    let _guard = LAUNCH_LEG_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let scratch = tempfile::tempdir().expect("scratch tempdir");
    let target = tempfile::tempdir().expect("target tempdir");
    std::fs::create_dir_all(target.path().join(".chug")).expect("target .chug");
    let (spec, stub) = write_cancel_spec_and_stub(scratch.path());

    // The flag and the stub seam are THIS server's argv/env only; the stub
    // stays alive in `sleep 60` until the cancel stops it. ONE stdin handle
    // is held across all sends (a second `take()` would find None).
    let (mut child, rx) = spawn_server_with(
        &["--allow-launch"],
        &[("CHUG_DELEGATE_BIN", stub.to_str().expect("utf-8 stub path"))],
    );
    let mut stdin = child.stdin.take().expect("piped stdin");
    send(&mut stdin, r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#);
    send(&mut stdin, r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#);
    let launch = serde_json::json!({
        "jsonrpc": "2.0", "id": 3, "method": "tools/call",
        "params": {"name": "chug_launch", "arguments": {
            "cwd": target.path().display().to_string(),
            "spec": spec.display().to_string(),
            "goal": "t153 wire cancel goal",
            "model": "wire-model"
        }}
    });
    send(&mut stdin, &launch.to_string());

    let init = next_response(&rx, "initialize response");
    assert_eq!(init["id"], 1, "{init}");

    // Both write legs advertised under the one flag.
    let list = next_response(&rx, "tools/list response");
    assert_eq!(list["id"], 2, "{list}");
    let names: Vec<&str> = list["result"]["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    assert_eq!(
        names,
        ["chug_status", "chug_collect", "chug_launch", "chug_cancel"],
        "the flag advertised both write legs: {list}"
    );

    // Launch: the payload carries the stub child's pid.
    let launch = next_response(&rx, "tools/call chug_launch response");
    assert_eq!(launch["id"], 3, "{launch}");
    assert_eq!(launch["result"]["isError"], false, "{launch}");
    let launch_text = launch["result"]["content"][0]["text"]
        .as_str()
        .expect("text content block")
        .to_string();
    let pid: u32 = launch_text
        .lines()
        .find_map(|l| l.strip_prefix("launched: pid "))
        .unwrap_or_else(|| panic!("pid line in the launch payload: {launch_text}"))
        .trim()
        .parse()
        .unwrap_or_else(|e| panic!("pid does not parse: {e}\n{launch_text}"));
    // The stub is LIVE: its record landed (it started) and it is asleep in
    // `sleep 60`, a real delegate-shaped child to cancel.
    wait_for_stub_record(
        &target.path().join("stub-record.txt"),
        &[
            "argv:".to_string(),
            "run".to_string(),
            "--spec".to_string(),
            spec.display().to_string(),
            "--goal".to_string(),
            "t153 wire cancel goal".to_string(),
            "--model".to_string(),
            "wire-model".to_string(),
            // The delegate launch path appends the DEFAULT budgets when the
            // caller omits them — the launch record carries them too.
            "--max-iters".to_string(),
            "40".to_string(),
            "--max-minutes".to_string(),
            "35".to_string(),
        ],
    );
    poll_pid_state(pid, true, "the stub child should be alive after launch");

    // Cancel it over the wire (the same held stdin handle).
    let cancel = serde_json::json!({
        "jsonrpc": "2.0", "id": 4, "method": "tools/call",
        "params": {"name": "chug_cancel", "arguments": {
            "cwd": target.path().display().to_string(),
            "pid": pid
        }}
    });
    send(&mut stdin, &cancel.to_string());
    let cancel = next_response(&rx, "tools/call chug_cancel response");
    assert_eq!(cancel["id"], 4, "{cancel}");
    assert!(
        cancel.get("error").is_none(),
        "a routed cancel is a tool result, not a JSON-RPC error: {cancel}"
    );
    assert_eq!(cancel["result"]["isError"], false, "the cancel succeeded: {cancel}");
    let text = cancel["result"]["content"][0]["text"]
        .as_str()
        .expect("text content block")
        .to_string();
    assert!(
        text.contains(&format!("pid {pid}")),
        "the payload names the cancelled pid: {text}"
    );
    assert!(text.contains("signaled: term"), "the TERM sufficed: {text}");
    assert!(text.contains("waited_ms: "), "{text}");

    // The stub is PROVABLY dead: within the bounded poll the pid is gone
    // (the server's own poll reaped it — the production zombie shape).
    poll_pid_state(pid, false, "the cancelled stub child should be dead");

    // The loop is alive, and EOF still exits 0.
    send(&mut stdin, r#"{"jsonrpc":"2.0","id":5,"method":"ping"}"#);
    let ping = next_response(&rx, "ping after the cancel");
    assert_eq!(ping["id"], 5, "{ping}");
    assert_eq!(ping["result"], serde_json::json!({}), "{ping}");

    drop(stdin);
    close_stdin_and_expect_success_exit(child, "cancel happy path");
}

/// Leg 2 — the policy boundary over the real wire: WITHOUT `--allow-launch`
/// the second write leg does not exist. `tools/list` names EXACTLY the two
/// read-only tools (a mutant that advertises `chug_cancel` unconditionally
/// dies on the exact-list equality), and a `tools/call` for it gets the
/// SAME unknown-tool `-32602` a never-existing tool gets — nothing probed,
/// nothing signalled.
#[test]
fn mcp_serve_chug_cancel_default_deny_over_the_real_wire() {
    let (mut child, rx) = spawn_server_with(&[], &[]);
    {
        let mut stdin = child.stdin.take().expect("piped stdin");
        send(&mut stdin, r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#);
        send(&mut stdin, r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#);
        let call = serde_json::json!({
            "jsonrpc": "2.0", "id": 3, "method": "tools/call",
            "params": {"name": "chug_cancel", "arguments": {
                "cwd": "relative/path", "pid": 1
            }}
        });
        send(&mut stdin, &call.to_string());
    }

    let init = next_response(&rx, "initialize response");
    assert_eq!(init["id"], 1, "{init}");

    let list = next_response(&rx, "tools/list response");
    assert_eq!(list["id"], 2, "{list}");
    let names: Vec<&str> = list["result"]["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    assert_eq!(
        names,
        ["chug_status", "chug_collect"],
        "chug_cancel must be ABSENT from the default-deny tool set: {list}"
    );

    let call = next_response(&rx, "default-deny chug_cancel response");
    assert_eq!(call["id"], 3, "{call}");
    let error = call.get("error").unwrap_or_else(|| {
        panic!("the default-deny call must be the unknown-tool error: {call}")
    });
    assert_eq!(error["code"], -32602, "{call}");
    let message = error["message"].as_str().expect("error message string");
    assert!(message.contains("unknown tool"), "{call}");
    assert!(message.contains("chug_cancel"), "{call}");

    close_stdin_and_expect_success_exit(child, "cancel default-deny");
}

/// Leg 3 — the ESRCH leg over the real wire: a call naming a pid that CANNOT
/// exist (2_000_000_000 is above every kernel pid ceiling — macOS 99999,
/// Linux 2^22 — yet within i32) is an `isError` tool RESULT naming "no such
/// process" and stating nothing was signalled, NOT a JSON-RPC error and NOT
/// a crash — and the loop is alive afterwards (the ping round-trips).
#[cfg(unix)]
#[test]
fn mcp_serve_chug_cancel_dead_pid_is_error_and_loop_alive_over_the_wire() {
    let target = tempfile::tempdir().expect("target tempdir");
    std::fs::create_dir_all(target.path().join(".chug")).expect("target .chug");
    let (mut child, rx) = spawn_server_with(&["--allow-launch"], &[]);
    {
        let mut stdin = child.stdin.take().expect("piped stdin");
        send(&mut stdin, r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#);
        let call = serde_json::json!({
            "jsonrpc": "2.0", "id": 2, "method": "tools/call",
            "params": {"name": "chug_cancel", "arguments": {
                "cwd": target.path().display().to_string(),
                "pid": 2_000_000_000u64
            }}
        });
        send(&mut stdin, &call.to_string());
        // Sent AFTER the dead-pid call: the proof the tool failure did not
        // kill the server loop.
        send(&mut stdin, r#"{"jsonrpc":"2.0","id":3,"method":"ping"}"#);
    }

    let init = next_response(&rx, "initialize response");
    assert_eq!(init["id"], 1, "{init}");

    let call = next_response(&rx, "dead-pid cancel response");
    assert_eq!(call["id"], 2, "{call}");
    assert!(
        call.get("error").is_none(),
        "a verification failure is a tool RESULT, not a JSON-RPC error: {call}"
    );
    assert_eq!(
        call["result"]["isError"], true,
        "the dead pid must land in the isError arm: {call}"
    );
    let text = call["result"]["content"][0]["text"]
        .as_str()
        .expect("text content block")
        .to_string();
    assert!(text.starts_with("chug_cancel:"), "tool-named error: {text}");
    assert!(text.contains("no such process"), "{text}");
    assert!(text.contains("nothing signalled"), "{text}");

    // The loop survived: the ping after the refusal answers normally.
    let ping = next_response(&rx, "ping after the dead-pid refusal");
    assert_eq!(ping["id"], 3, "{ping}");
    assert_eq!(ping["result"], serde_json::json!({}), "{ping}");

    close_stdin_and_expect_success_exit(child, "cancel dead-pid leg");
}
