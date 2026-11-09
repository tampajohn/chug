//! T343 — the usage-events emitter battery: the Stop payload is transcribed
//! into exactly one JSONL line per turn, fields present ONLY when the event
//! carries them (no fabricated zeros; the served slot is filled only by an
//! explicit as-served value, never the requested name), the line is
//! single-line valid JSON, the emitter is silent on every non-Stop or
//! unusable payload, and the transcription adds <2ms over a no-op hook
//! with no network anywhere in the path (Langfuse shipping stays on the
//! OTel path, NOT here).
//!
//! The hook runs through `hook.sh` (the wired entrypoint) with stdin
//! carrying Claude's compact-JSON hook payload, scoped to a scratch
//! `$CLAUDE_CONFIG_DIR` — never the operator's own. No sockets, no
//! daemons: the battery only needs /bin/sh.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// The hook's target: `<claude-dir>/meta-usage.jsonl` (docs/meta-harness.md).
fn usage_log(claude_dir: &Path) -> PathBuf {
    claude_dir.join("meta-usage.jsonl")
}

/// The lines of the usage log, as raw strings.
fn log_lines(claude_dir: &Path) -> Vec<String> {
    std::fs::read_to_string(usage_log(claude_dir))
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect()
}

fn run_hook(claude_dir: &Path, payload: &str) -> (Option<i32>, String) {
    let root = std::env::current_dir().expect("cargo sets the test cwd to the package root");
    let mut child = Command::new("/bin/sh")
        .arg(root.join("hooks/meta-usage").join("hook.sh"))
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
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

fn scratch() -> (tempfile::TempDir, PathBuf) {
    let home = tempfile::tempdir().expect("tempdir");
    let claude_dir = home.path().join(".claude");
    // The pack's target parent is the claude dir itself (which exists when
    // Claude runs); the hook creates the FILE on first append, never the
    // directory.
    std::fs::create_dir_all(&claude_dir).unwrap();
    (home, claude_dir)
}

fn stop_payload(extra: &str) -> String {
    format!(
        r#"{{"session_id":"sess-7","transcript_path":"/tmp/t.jsonl","cwd":"/repo","hook_event_name":"Stop","stop_hook_active":false{extra}}}"#
    )
}

// ---------- the usage fixture (spec Tests 1) ----------

/// A turn event with full fields → the complete line, keys in the
/// documented order, single-line valid JSON.
#[test]
fn full_event_yields_the_complete_line() {
    let (_home, claude) = scratch();
    let payload = stop_payload(
        r#","model_requested":"claude-sonnet-4-5","model_served":"glm-4.6","usage":{"input_tokens":1234,"output_tokens":56,"cache_read_input_tokens":7,"cache_creation_input_tokens":2}"#,
    );
    let (code, err) = run_hook(&claude, &payload);
    assert_eq!(code, Some(0), "exit 0; stderr: {err}");
    assert!(err.is_empty(), "no stderr noise on the happy path: {err}");
    let lines = log_lines(&claude);
    assert_eq!(lines.len(), 1, "exactly one line: {lines:?}");
    let line = &lines[0];
    assert!(!line.contains('\n'), "single line");
    let parsed: serde_json::Value = serde_json::from_str(line).expect("valid JSON");
    assert_eq!(parsed["session_id"], "sess-7");
    assert_eq!(parsed["model_requested"], "claude-sonnet-4-5");
    assert_eq!(parsed["model_served"], "glm-4.6");
    assert_eq!(parsed["input_tokens"], 1234);
    assert_eq!(parsed["output_tokens"], 56);
    assert_eq!(parsed["cache_read_tokens"], 7, "renamed from the API field");
    assert_eq!(
        parsed["cache_creation_tokens"], 2,
        "renamed from the API field"
    );
    assert_eq!(parsed["contract_version"], "1");
    assert!(parsed["ts"].as_str().is_some_and(|ts| ts.ends_with('Z')), "RFC3339 UTC ts");
    // Key order is the documented shape's order (jq-mineable by contract).
    let order = [
        "ts",
        "session_id",
        "model_requested",
        "model_served",
        "input_tokens",
        "output_tokens",
        "cache_read_tokens",
        "cache_creation_tokens",
        "contract_version",
    ];
    let mut cursor = 0;
    for key in order {
        let needle = format!("\"{key}\":");
        let at = line[cursor..]
            .find(&needle)
            .unwrap_or_else(|| panic!("key {key} missing or out of order: {line}"));
        cursor += at + needle.len();
    }
}

/// A partial event → only the present keys. A bare `model` fills the
/// REQUESTED slot and never the served slot (the conflation rule).
#[test]
fn partial_event_omits_absent_keys_and_never_fills_served_from_model() {
    let (_home, claude) = scratch();
    let (code, _) = run_hook(&claude, &stop_payload(r#","model":"claude-sonnet-4-5""#));
    assert_eq!(code, Some(0));
    let lines = log_lines(&claude);
    assert_eq!(lines.len(), 1);
    let parsed: serde_json::Value = serde_json::from_str(&lines[0]).unwrap();
    assert_eq!(parsed["model_requested"], "claude-sonnet-4-5", "the configured name is what was asked for");
    assert!(
        parsed.get("model_served").is_none(),
        "the served slot is NEVER filled from the requested name: {lines:?}"
    );
    for absent in [
        "input_tokens",
        "output_tokens",
        "cache_read_tokens",
        "cache_creation_tokens",
    ] {
        assert!(parsed.get(absent).is_none(), "{absent} omitted, not zeroed");
    }
}

/// A minimal Stop payload (today's Claude Code shape) → the honest marker
/// line: ts, session_id, contract_version. Still one valid JSONL line.
#[test]
fn minimal_stop_payload_yields_the_marker_line() {
    let (_home, claude) = scratch();
    let (code, _) = run_hook(&claude, &stop_payload(""));
    assert_eq!(code, Some(0));
    let parsed: serde_json::Value =
        serde_json::from_str(&log_lines(&claude)[0]).unwrap();
    assert_eq!(parsed["session_id"], "sess-7");
    assert_eq!(parsed["contract_version"], "1");
    for absent in [
        "model_requested",
        "model_served",
        "input_tokens",
        "output_tokens",
    ] {
        assert!(parsed.get(absent).is_none(), "{absent} absent, not fabricated");
    }
}

/// The usage object may ride embedded (the transcript-line shape
/// `message.usage`); the same extraction applies.
#[test]
fn message_usage_embedding_is_extracted() {
    let (_home, claude) = scratch();
    let payload = stop_payload(
        r#","message":{"id":"m1","usage":{"input_tokens":9,"output_tokens":1}}"#,
    );
    let (code, _) = run_hook(&claude, &payload);
    assert_eq!(code, Some(0));
    let parsed: serde_json::Value =
        serde_json::from_str(&log_lines(&claude)[0]).unwrap();
    assert_eq!(parsed["input_tokens"], 9);
    assert_eq!(parsed["output_tokens"], 1);
}

/// A zero the event CARRIES is data, not absence (`"output_tokens":0`
/// rides; GLM's cache accounting reading zero is exactly the fact a
/// dashboard needs to see, not a gap to paper over).
#[test]
fn a_carried_zero_rides_as_zero() {
    let (_home, claude) = scratch();
    let payload = stop_payload(r#","usage":{"input_tokens":10,"output_tokens":0}"#);
    let (code, _) = run_hook(&claude, &payload);
    assert_eq!(code, Some(0));
    let parsed: serde_json::Value =
        serde_json::from_str(&log_lines(&claude)[0]).unwrap();
    assert_eq!(parsed["input_tokens"], 10);
    assert_eq!(parsed["output_tokens"], 0, "present zero is data");
    assert!(parsed.get("cache_read_tokens").is_none(), "truly absent stays absent");
}

// ---------- silence (the absent postures) ----------

/// Non-Stop payloads write nothing (the pack wires Stop and only Stop).
#[test]
fn non_stop_events_write_nothing() {
    let (_home, claude) = scratch();
    for event in ["PreToolUse", "PostToolUse", "UserPromptSubmit", "SessionEnd", ""] {
        let payload = format!(
            r#"{{"session_id":"sess-7","hook_event_name":"{event}","usage":{{"input_tokens":5,"output_tokens":5}}}}"#
        );
        let (code, _) = run_hook(&claude, &payload);
        assert_eq!(code, Some(0), "{event} exits 0");
        assert!(log_lines(&claude).is_empty(), "{event} writes no line");
    }
}

/// A payload without a session id (or with an unsanitary one) emits no
/// line: an unattributable turn is no row, not a blank one.
#[test]
fn unusable_session_ids_emit_no_line() {
    let (_home, claude) = scratch();
    for extra in ["", r#","session_id":"bad id!""#, r#","session_id":""#] {
        let payload = format!(r#"{{"hook_event_name":"Stop"{extra}}}"#);
        let (code, _) = run_hook(&claude, &payload);
        assert_eq!(code, Some(0));
        assert!(log_lines(&claude).is_empty(), "no line for {extra:?}");
    }
}

/// Impure values are omitted, never half-escaped: a percent sign or a
/// backslash would be interpreted by `date` (the format string IS the
/// line), and non-printables would break the one-line JSONL contract.
#[test]
fn impure_values_are_omitted_never_escaped() {
    let (_home, claude) = scratch();
    let payload = stop_payload(r#","model_served":"glm%4.6\nCRITICAL","model":"a\\b""#);
    let (code, _) = run_hook(&claude, &payload);
    assert_eq!(code, Some(0));
    let lines = log_lines(&claude);
    assert_eq!(lines.len(), 1);
    let parsed: serde_json::Value = serde_json::from_str(&lines[0]).unwrap();
    assert!(parsed.get("model_served").is_none(), "percent value omitted");
    assert!(parsed.get("model_requested").is_none(), "backslash value omitted");
    assert_eq!(parsed["session_id"], "sess-7");
}

/// A missing claude dir (unwatched target) exits 0 silently — no per-turn
/// hook-error notice (the T342 posture).
#[test]
fn missing_claude_dir_is_silent() {
    let home = tempfile::tempdir().expect("tempdir");
    let absent = home.path().join(".claude");
    let (code, err) = run_hook(&absent, &stop_payload(""));
    assert_eq!(code, Some(0));
    assert!(err.is_empty(), "silent: {err}");
    assert!(!usage_log(&absent).exists());
}

/// Concurrent writers do not interleave mid-line: every appended line is
/// one valid JSON object (O_APPEND, one write under PIPE_BUF).
#[test]
fn concurrent_writers_never_interleave() {
    let (_home, claude) = scratch();
    let dir = claude.clone();
    let handles: Vec<_> = (0..8)
        .map(|i| {
            let dir = dir.clone();
            std::thread::spawn(move || {
                for n in 0..10 {
                    let input = i * 100 + 7 + n;
                    let payload = stop_payload(&format!(
                        r#","usage":{{"input_tokens":{input},"output_tokens":{n}}}"#
                    ));
                    let _ = Command::new("/bin/sh")
                        .arg(
                            std::env::current_dir()
                                .unwrap()
                                .join("hooks/meta-usage/hook.sh"),
                        )
                        .env("CLAUDE_CONFIG_DIR", &dir)
                        .stdin(Stdio::piped())
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .spawn()
                        .and_then(|mut c| {
                            c.stdin
                                .take()
                                .unwrap()
                                .write_all(payload.as_bytes())
                                .ok();
                            c.wait()
                        })
                        .expect("spawn");
                }
            })
        })
        .collect();
    for h in handles {
        h.join().unwrap();
    }
    let lines = log_lines(&claude);
    assert_eq!(lines.len(), 80, "every turn appended exactly one line");
    for line in &lines {
        serde_json::from_str::<serde_json::Value>(line)
            .unwrap_or_else(|e| panic!("every line valid JSON ({e}): {line}"));
    }
}

// ---------- the budget fixture (spec Tests 3) ----------

/// The transcription adds <2ms over a no-op hook: best-of-30, INTERLEAVED
/// PAIRS against a no-op script paying the identical spawn + stdin-drain
/// cost (the T342 methodology — the statistic is the min of the per-pair
/// deltas, so box load inflates both legs together). The transcription
/// itself is shell builtins plus one `exec date`. And the path opens no
/// network: the pack's handler execs only `date` (Langfuse shipping stays
/// with the OTel path, NOT this hook).
#[test]
fn hook_adds_under_2ms_per_turn_and_touches_no_network() {
    let (_home, claude) = scratch();
    let no_op = claude.join("no-op-hook.sh");
    std::fs::write(
        &no_op,
        "#!/bin/sh\nif [ ! -t 0 ]; then IFS= read -r _ || :; while IFS= read -r _; do :; done; fi\nexit 0\n",
    )
    .unwrap();
    let root = std::env::current_dir().expect("cargo sets the test cwd to the package root");
    let hook = root.join("hooks/meta-usage/hook.sh");
    let payload = stop_payload(r#","usage":{"input_tokens":123,"output_tokens":4}"#);

    let timed = |script: &Path| -> Duration {
        let started = Instant::now();
        let mut child = Command::new("/bin/sh")
            .arg(script)
            .env("CLAUDE_CONFIG_DIR", &claude)
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
    // One warmup round per leg, then interleaved pairs; the statistic is
    // the min of the per-pair deltas (min-vs-min lands in different load
    // windows and inflates the delta — T342's measured finding).
    let _ = timed(&no_op);
    let _ = timed(&hook);
    let mut floor = Duration::from_secs(3600);
    let mut hook_best = Duration::from_secs(3600);
    let mut added = Duration::from_secs(3600);
    for _ in 0..30 {
        let n = timed(&no_op);
        let h = timed(&hook);
        floor = floor.min(n);
        hook_best = hook_best.min(h);
        added = added.min(h.saturating_sub(n));
    }
    assert!(
        added < Duration::from_millis(2),
        "the hook adds {added:?} per turn — the spec's budget is <2ms \
         (no-op best {floor:?}, hook best {hook_best:?})"
    );

    // No network in the hook path: the pack's handler names no fetch/ship
    // helper and execs exactly one process, date.
    let text = std::fs::read_to_string(&hook).unwrap();
    for banned in ["curl", "wget", "nc ", "python3", "otel", "langfuse"] {
        assert!(
            !text.contains(banned),
            "hook.sh must not {banned} — no network in the hook path"
        );
    }
    let execs: Vec<&str> = text
        .lines()
        .filter(|l| l.trim_start().starts_with("exec "))
        .map(str::trim)
        .collect();
    assert_eq!(execs.len(), 1, "one exec: {execs:?}");
    assert!(execs[0].contains("date"), "the exec is date: {execs:?}");
}
