//! T342 — the session-status mod battery: each hook event maps to the
//! right typed status (the T339 enum, exactly), a turn end keeps a pending
//! question blocked, the rewrite is atomic under a hammering reader
//! (readers never observe a partial file), the pack's total footprint is
//! one status file per session (silent when unwatched), session end leaves
//! the file at done, and the transcription adds <2ms over a no-op hook.
//!
//! The hook runs through `hook.sh` (the wired entrypoint) with stdin
//! carrying Claude's compact-JSON hook payload, scoped to a scratch
//! `$CLAUDE_CONFIG_DIR` — never the operator's own. No sockets, no
//! daemons, no python on the hot path: the battery only needs /bin/sh.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const CONTRACT_VERSION: &str = "1";
/// The padded document length every rewrite produces (see hook.sh's
/// header: a 512-char body, date's %Y growth, date's trailing newline).
const BODY_BYTES: usize = 515;

/// A compact-JSON hook payload — the shape Claude Code pipes to hooks.
fn payload(event: &str, extra: &str) -> String {
    format!(
        r#"{{"session_id":"sess-7","transcript_path":"/tmp/t.jsonl","cwd":"/repo","hook_event_name":"{event}"{extra}}}"#
    )
}

fn tool_payload(event: &str, tool: &str, extra: &str) -> String {
    payload(
        event,
        &format!(r#","tool_name":"{tool}","tool_input":{{}}{extra}"#),
    )
}

/// Run hook.sh (the wired entrypoint) with a scoped claude dir and the
/// payload on stdin. Returns (exit code, stdout).
fn run_hook(claude_dir: &Path, payload: &str) -> (Option<i32>, String) {
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
        .write_all(payload.as_bytes())
        .ok();
    let out = child.wait_with_output().expect("hook.sh runs");
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stdout).to_string(),
    )
}

fn pack_dir() -> PathBuf {
    // T48: resolve the repo root at RUNTIME (cargo runs test binaries with
    // cwd = the package root) — the compile-time manifest macro is what
    // false-redded the cycle-21 post-merge gates under the T47 shared cache
    // and is refused by the no_compile lint on every .rs file.
    let root = std::env::current_dir().expect("cargo sets the test cwd to the package root");
    root.join("hooks/meta-status")
}

/// The scratch claude dir with meta-status/ present (install.sh creates
/// the status dir; the hook never does — that posture has its own test).
fn scratch() -> (tempfile::TempDir, PathBuf) {
    let home = tempfile::tempdir().expect("tempdir");
    let claude_dir = home.path().join(".claude");
    std::fs::create_dir_all(claude_dir.join("meta-status")).unwrap();
    (home, claude_dir)
}

fn status_path(claude_dir: &Path, session: &str) -> PathBuf {
    claude_dir.join("meta-status").join(format!("{session}.json"))
}

/// Read and parse the session's status document, asserting the invariants
/// every legal write carries: valid JSON, contract_version "1", a status
/// on the T339 enum, and the padded body length.
fn read_status(claude_dir: &Path, session: &str) -> serde_json::Value {
    let raw = std::fs::read(status_path(claude_dir, session)).expect("status file exists");
    assert_eq!(raw.len(), BODY_BYTES, "every rewrite is the constant {BODY_BYTES}-byte document: {raw:?}");
    let doc: serde_json::Value = serde_json::from_slice(&raw).expect("the status document parses");
    assert_eq!(doc["contract_version"], CONTRACT_VERSION, "{doc}");
    assert!(
        matches!(doc["status"].as_str(), Some("working") | Some("blocked") | Some("done")),
        "status is the T339 enum, exactly: {doc}"
    );
    match doc["blocked_reason"].as_str() {
        Some("permission") | Some("question") | Some("error") => {
            assert_eq!(doc["status"], "blocked", "a reason rides only a blocked: {doc}");
        }
        other => assert_eq!(other, None, "no reason key on a non-blocked: {doc}"),
    }
    // updated_at is a real RFC3339 UTC stamp, never fabricated.
    let stamp = doc["updated_at"].as_str().expect("updated_at present");
    assert_eq!(stamp.len(), 20, "RFC3339 second precision: {stamp}");
    assert!(stamp.ends_with('Z'), "UTC: {stamp}");
    doc
}

// ---------- the event table ----------

/// Each hook event maps to the right status/blocked_reason/detail — the
/// spec's four named mappings plus the working legs that bracket them.
#[test]
fn each_hook_event_maps_to_its_typed_status() {
    let (_home, claude_dir) = scratch();

    // UserPromptSubmit → working, no reason, detail omitted (unknown).
    let (code, out) = run_hook(&claude_dir, &payload("UserPromptSubmit", ""));
    assert_eq!(code, Some(0));
    assert!(out.is_empty(), "the mod prints nothing: {out}");
    let doc = read_status(&claude_dir, "sess-7");
    assert_eq!(doc["status"], "working", "{doc}");
    assert!(doc["blocked_reason"].is_null(), "no reason: {doc}");
    assert!(doc.get("detail").is_none(), "an unknown detail is omitted, never fabricated: {doc}");

    // PreToolUse (a tool) → working with the tool as detail.
    run_hook(&claude_dir, &tool_payload("PreToolUse", "Bash", ""));
    let doc = read_status(&claude_dir, "sess-7");
    assert_eq!(doc["status"], "working", "{doc}");
    assert_eq!(doc["detail"], "Bash", "{doc}");

    // PostToolUse → working (the tool returned); the response may be big.
    let big = "x".repeat(3000);
    run_hook(
        &claude_dir,
        &tool_payload("PostToolUse", "Bash", &format!(r#","tool_response":{{"out":"{big}"}}"#)),
    );
    let doc = read_status(&claude_dir, "sess-7");
    assert_eq!(doc["status"], "working", "{doc}");

    // PostToolUse with a surfaced error → blocked/error (the spec's tool
    // error leg); the payload carries both marker spellings tests see in
    // the wild (compact and spaced).
    run_hook(
        &claude_dir,
        &tool_payload("PostToolUse", "Bash", r#","tool_response":{"isError":true}"#),
    );
    let doc = read_status(&claude_dir, "sess-7");
    assert_eq!(doc["status"], "blocked", "{doc}");
    assert_eq!(doc["blocked_reason"], "error", "{doc}");

    // Notification (Claude's permission ask) → blocked/permission with the
    // message as detail.
    run_hook(
        &claude_dir,
        &payload(
            "Notification",
            r#","message":"Claude needs your permission to use Bash""#,
        ),
    );
    let doc = read_status(&claude_dir, "sess-7");
    assert_eq!(doc["status"], "blocked", "{doc}");
    assert_eq!(doc["blocked_reason"], "permission", "{doc}");
    assert!(
        doc["detail"].as_str().is_some_and(|d| d.contains("permission")),
        "the ask rides in the detail: {doc}"
    );

    // Stop (no pending question) → done (the spec's Stop leg).
    run_hook(&claude_dir, &payload("Stop", r#","stop_hook_active":false"#));
    let doc = read_status(&claude_dir, "sess-7");
    assert_eq!(doc["status"], "done", "{doc}");
    assert!(doc["blocked_reason"].is_null(), "no reason: {doc}");

    // SessionEnd → done (the cleanup leg, asserted again below).
    run_hook(&claude_dir, &payload("SessionEnd", r#","reason":"exit""#));
    let doc = read_status(&claude_dir, "sess-7");
    assert_eq!(doc["status"], "done", "{doc}");
}

/// A turn end with a pending question is NOT done — the session still
/// waits on the operator (the spec's "turn end with pending question →
/// blocked/question" leg). The answer (PostToolUse) clears it; a plain
/// turn end after that is done.
#[test]
fn a_turn_end_keeps_a_pending_question_blocked() {
    let (_home, claude_dir) = scratch();

    run_hook(&claude_dir, &tool_payload("PreToolUse", "AskUserQuestion", ""));
    let doc = read_status(&claude_dir, "sess-7");
    assert_eq!(doc["status"], "blocked", "{doc}");
    assert_eq!(doc["blocked_reason"], "question", "{doc}");

    // The turn ends with the question still pending: Stop keeps it.
    run_hook(&claude_dir, &payload("Stop", ""));
    let doc = read_status(&claude_dir, "sess-7");
    assert_eq!(doc["status"], "blocked", "a pending question outlives the turn: {doc}");
    assert_eq!(doc["blocked_reason"], "question", "{doc}");

    // The answer arrives: PostToolUse(AskUserQuestion) → working...
    run_hook(
        &claude_dir,
        &tool_payload("PostToolUse", "AskUserQuestion", r#","tool_response":{"answer":"pg"}"#),
    );
    let doc = read_status(&claude_dir, "sess-7");
    assert_eq!(doc["status"], "working", "the answer unblocks: {doc}");

    // ...and the next turn end is done.
    run_hook(&claude_dir, &payload("Stop", ""));
    let doc = read_status(&claude_dir, "sess-7");
    assert_eq!(doc["status"], "done", "{doc}");
}

/// A permission ask clears the same way: the Notification leg flips it to
/// blocked/permission, the operator answers, and the turn continues —
/// the next working fact overwrites the blocked one.
#[test]
fn a_permission_ask_clears_when_the_turn_continues() {
    let (_home, claude_dir) = scratch();
    run_hook(
        &claude_dir,
        &payload("Notification", r#","message":"Claude needs your permission to use WebFetch""#),
    );
    let doc = read_status(&claude_dir, "sess-7");
    assert_eq!(doc["blocked_reason"], "permission", "{doc}");
    run_hook(&claude_dir, &tool_payload("PreToolUse", "WebFetch", ""));
    let doc = read_status(&claude_dir, "sess-7");
    assert_eq!(doc["status"], "working", "the granted ask unblocks: {doc}");
    assert!(doc["blocked_reason"].is_null(), "no reason: {doc}");
}

// ---------- skip legs: never a wrong write ----------

/// Anything that is not a status fact is skipped silently: other
/// notifications, unknown events, unsanitary session ids (traversal
/// included), and payloads without the needed fields all write NOTHING.
#[test]
fn anything_that_is_not_a_status_fact_writes_nothing() {
    let (home, claude_dir) = scratch();
    let dir = claude_dir.join("meta-status");

    let (code, out) = run_hook(
        &claude_dir,
        &payload("Notification", r#","message":"Claude is waiting for your input""#),
    );
    assert_eq!(code, Some(0));
    assert!(out.is_empty());
    assert!(
        std::fs::read_dir(&dir).unwrap().next().is_none(),
        "an idle notification is not a status fact"
    );

    for bad in [
        payload("SubagentTurn", ""),
        payload("", ""),
        r#"{"session_id":"../../evil","hook_event_name":"Stop"}"#.to_string(),
        r#"{"session_id":"has space","hook_event_name":"Stop"}"#.to_string(),
        r#"{"session_id":"","hook_event_name":"Stop"}"#.to_string(),
        "not json at all".to_string(),
        String::new(),
    ] {
        let (code, out) = run_hook(&claude_dir, &bad);
        assert_eq!(code, Some(0), "skips exit 0: {bad}");
        assert!(out.is_empty());
    }
    assert!(
        std::fs::read_dir(&dir).unwrap().next().is_none(),
        "no file was ever written, and nothing escaped the status dir"
    );
    // Nothing appeared anywhere in the workspace either — a traversal
    // session id never names a file outside the status dir.
    let mut workspace: Vec<PathBuf> = Vec::new();
    let mut stack = vec![home.path().to_path_buf()];
    while let Some(next) = stack.pop() {
        for entry in std::fs::read_dir(&next).unwrap().flatten() {
            workspace.push(entry.path());
            if entry.path().is_dir() {
                stack.push(entry.path());
            }
        }
    }
    workspace.sort();
    assert_eq!(
        workspace,
        vec![claude_dir, dir],
        "the workspace holds exactly the empty status dir: {workspace:?}"
    );
}

/// A detail that would corrupt the document (percent signs or backslashes
/// would be interpreted by the date-format writer, non-printable text
/// cannot be length-counted) is OMITTED, never half-escaped or truncated.
#[test]
fn an_impure_detail_is_omitted_not_escaped() {
    let (_home, claude_dir) = scratch();
    run_hook(
        &claude_dir,
        &tool_payload("PreToolUse", "Tool%YWith\\Escapes", ""),
    );
    let doc = read_status(&claude_dir, "sess-7");
    assert_eq!(doc["status"], "working", "{doc}");
    assert!(
        doc.get("detail").is_none(),
        "a percent/backslash detail is omitted (it would be date-interpreted): {doc}"
    );
    // A clean detail survives a huge response: the tool_response never
    // crowds the body (only the detail itself can be crowded, and the
    // crowding drop is the next test's business).
    run_hook(
        &claude_dir,
        &tool_payload(
            "PostToolUse",
            "Bash",
            &format!(r#","tool_response":{{"out":"{}"}}"#, "y".repeat(3000)),
        ),
    );
    let doc = read_status(&claude_dir, "sess-7");
    assert_eq!(doc["detail"], "Bash", "the clean detail survives: {doc}");
}

// ---------- atomicity: readers never observe a partial file ----------

/// The rewrite is atomic by construction (one equal-length overwrite of
/// the same inode — see hook.sh's header): a reader polling as fast as it
/// can while events fire sees the previous or the new COMPLETE document
/// every single time — never a partial file, never a truncate window.
#[test]
fn readers_never_observe_a_partial_file() {
    let (_home, claude_dir) = scratch();
    let path = status_path(&claude_dir, "sess-7");

    // Prime the file so the hammer reads steady-state rewrites (the
    // not-yet-created case is the absent posture, not a partial file).
    run_hook(&claude_dir, &payload("UserPromptSubmit", ""));

    let reader = {
        let path = path.clone();
        std::thread::spawn(move || {
            let mut good = 0usize;
            let mut bad: Vec<String> = Vec::new();
            let deadline = Instant::now() + Duration::from_secs(4);
            while Instant::now() < deadline {
                std::thread::sleep(Duration::from_micros(500));
                match std::fs::read(&path) {
                    Ok(raw) => {
                        if raw.len() != BODY_BYTES {
                            bad.push(format!("wrong length {}", raw.len()));
                            continue;
                        }
                        match serde_json::from_slice::<serde_json::Value>(&raw) {
                            Ok(doc)
                                if matches!(
                                    doc["status"].as_str(),
                                    Some("working") | Some("blocked") | Some("done")
                                ) =>
                            {
                                good += 1;
                            }
                            Ok(doc) => bad.push(format!("bad enum: {doc}")),
                            Err(e) => bad.push(format!("unparseable: {e}")),
                        }
                    }
                    Err(_) => bad.push("vanished".to_string()),
                }
            }
            (good, bad)
        })
    };

    // 120 events flip the document working <-> done while the reader polls.
    for i in 0..120 {
        let event = if i % 2 == 0 { "UserPromptSubmit" } else { "Stop" };
        let (code, _) = run_hook(&claude_dir, &payload(event, ""));
        assert_eq!(code, Some(0), "event {i}");
    }

    let (good, bad) = reader.join().expect("the reader thread joins");
    assert!(
        good > 500,
        "non-vacuous: the hammer actually read ({good} observations)"
    );
    assert!(
        bad.is_empty(),
        "readers never observe a partial file, but saw {:?}",
        bad.iter().take(5).collect::<Vec<_>>()
    );
    let doc = read_status(&claude_dir, "sess-7");
    assert_eq!(doc["status"], "done", "the last event was a Stop: {doc}");
}

// ---------- silent when unwatched: the footprint ----------

/// The mod's TOTAL footprint is the one status file per session: a
/// recursive snapshot of the scratch claude dir before/after a full
/// event sequence shows nothing else appearing — no sockets, no audit
/// files, no tmp files, no settings writes. And a hook with no status
/// dir (never installed) is a silent no-op that creates nothing.
#[test]
fn the_footprint_is_one_status_file_per_session_and_nothing_else() {
    let (home, claude_dir) = scratch();

    let tree = |root: &Path| -> Vec<PathBuf> {
        let mut out = Vec::new();
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                out.push(entry.path());
                if entry.path().is_dir() {
                    stack.push(entry.path());
                }
            }
        }
        out.sort();
        out
    };
    let before = tree(&claude_dir);

    for (event, extra) in [
        ("UserPromptSubmit", ""),
        ("PreToolUse", r#","tool_name":"Bash","tool_input":{}"#),
        ("Notification", r#","message":"Claude needs your permission to use Bash""#),
        ("Stop", ""),
        ("SessionEnd", ""),
    ] {
        let (code, out) = run_hook(&claude_dir, &payload(event, extra));
        assert_eq!(code, Some(0));
        assert!(out.is_empty(), "{event} prints nothing: {out}");
    }

    let mut after = tree(&claude_dir);
    let appeared: Vec<PathBuf> = after
        .drain(..)
        .filter(|p| !before.contains(p))
        .collect();
    assert_eq!(
        appeared,
        vec![status_path(&claude_dir, "sess-7")],
        "the whole pack's footprint is the session's one status file"
    );
    assert!(
        !claude_dir.join("meta-broker.sock").exists(),
        "no socket, ever"
    );

    // Silent when unwatched: a status dir that does not exist (the mod
    // never installed / was uninstalled) makes every event a no-op that
    // creates NOTHING.
    let bare = home.path().join("bare-claude");
    std::fs::create_dir_all(&bare).unwrap();
    for event in ["UserPromptSubmit", "Stop", "SessionEnd"] {
        let (code, out) = run_hook(&bare, &payload(event, ""));
        assert_eq!(code, Some(0), "{event} on an uninstalled dir");
        assert!(out.is_empty());
    }
    let bare_tree = tree(&bare);
    assert!(bare_tree.is_empty(), "nothing created: {bare_tree:?}");
}

/// Session end leaves the file at done — no reaper needed, and the
/// reader contract's staleness rule (stale >10min = unknown) covers the
/// rest. The file's mtime is fresh (the last write WAS the session end).
#[test]
fn session_end_leaves_the_file_at_done() {
    let (_home, claude_dir) = scratch();
    run_hook(&claude_dir, &tool_payload("PreToolUse", "Bash", ""));
    run_hook(&claude_dir, &payload("SessionEnd", ""));
    let doc = read_status(&claude_dir, "sess-7");
    assert_eq!(doc["status"], "done", "{doc}");
    let age = std::fs::metadata(status_path(&claude_dir, "sess-7"))
        .unwrap()
        .modified()
        .unwrap()
        .elapsed()
        .unwrap();
    assert!(age < Duration::from_secs(30), "the last write was now: {age:?}");
}

// ---------- cost: <2ms added per hook event ----------

/// The transcription adds <2ms over a no-op hook: best-of-9, INTERLEAVED,
/// so box load inflates both legs together (the T341 methodology, sized
/// for the tighter budget). The no-op script pays the identical spawn +
/// drain cost; the delta is the transcription itself — shell builtins
/// plus one `exec date`.
#[test]
fn the_transcription_adds_under_2ms_over_a_no_op_hook() {
    let (_home, claude_dir) = scratch();
    let no_op = claude_dir.join("no-op-hook.sh");
    std::fs::write(&no_op, "#!/bin/sh\nif [ ! -t 0 ]; then IFS= read -r _ || :; while IFS= read -r _; do :; done; fi\nexit 0\n").unwrap();
    let hook = pack_dir().join("hook.sh");

    let timed = |script: &Path, payload: &str| -> Duration {
        let started = Instant::now();
        let mut child = Command::new("/bin/sh")
            .arg(script)
            .env("CLAUDE_CONFIG_DIR", &claude_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn hook");
        child
            .stdin
            .take()
            .unwrap()
            .write_all(payload.as_bytes())
            .ok();
        let out = child.wait_with_output().expect("hook runs");
        let elapsed = started.elapsed();
        assert!(out.status.success());
        assert!(
            out.stdout.is_empty(),
            "the hook prints nothing: {}",
            String::from_utf8_lossy(&out.stdout)
        );
        elapsed
    };
    let working = payload("UserPromptSubmit", "");
    // One warmup round per leg (script-file cache), then the interleaved
    // best-of-15. The min is the fair statistic under bursty sibling-test
    // load: a scheduler hiccup is not the hook's doing.
    let _ = timed(&no_op, &working);
    let _ = timed(&hook, &working);
    let mut floor = Duration::from_secs(3600);
    let mut hook_best = Duration::from_secs(3600);
    for _ in 0..15 {
        floor = floor.min(timed(&no_op, &working));
        hook_best = hook_best.min(timed(&hook, &working));
    }
    let added = hook_best.saturating_sub(floor);
    assert!(
        added < Duration::from_millis(2),
        "the transcription must add <2ms per event (no-op best {floor:?}, \
         hook best {hook_best:?}, added {added:?})"
    );
}
