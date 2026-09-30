// T104: the body of driver.rs's `#[cfg(test)] pub(crate) mod tests` (4,259
// lines at the split) lives in this file+directory module pair; driver.rs
// keeps the one-line declaration plus its harness comment. THIS FILE HOLDS
// THE SHARED HARNESS ONLY (T84's one rule): every item below is used by two
// or more family files, or is the `pub(crate)` seam src/trim.rs's tests
// reuse (ctx_for/knobs_with/tool_use_response/RecordingSink — see driver.rs
// beside the declaration). Helpers with callers in a single family moved
// with that family. Moved bytes are byte-identical to the pre-split module
// body and keep their in-module 4-space indent (a move, not a rewrite).
use super::*;
use crate::api::{KnownBlock, ScriptedLlm};
use serde_json::json;

// One `mod` line per family file; each family's tests live in exactly one
// file.
mod approve;
mod budget;
mod crash;
mod events;
mod goal;
mod hooks_policy;
mod image;
mod lock;
mod mcp;
mod observability;
mod permissions_policy;
mod plan;
mod preview;
mod resume;
mod spec_check_gate;
mod steering;
mod stuck;
mod truncated;
mod unit;

#[derive(Default)]
pub(crate) struct RecordingSink(Vec<Event>);

impl EventSink for RecordingSink {
    fn emit(&mut self, e: Event) {
        self.0.push(e);
    }
}

fn write_spec(tmp: &tempfile::TempDir) -> PathBuf {
    let spec = tmp.path().join("s.md");
    std::fs::write(&spec, "spec text\ncheck: true\n").unwrap();
    spec
}

/// Read the events log of a run in `tmp`, asserting every line is JSON.
fn events_jsonl(tmp: &tempfile::TempDir) -> Vec<Value> {
    let path = tmp.path().join(".chug").join("events.jsonl");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("events.jsonl readable: {e}"))
        .lines()
        .map(|l| serde_json::from_str(l).expect("every events.jsonl line parses as JSON"))
        .collect()
}

// ---------- MCP integration (fake echo server, no network) ----------

/// LLM double that records the tools array it was offered, like
/// ScriptedLlm but for the tool schemas (which the driver composes).
struct ToolRecordingLlm {
    responses: std::collections::VecDeque<Value>,
    recorded_tools: Vec<Vec<Value>>,
}

impl ToolRecordingLlm {
    fn new(responses: Vec<Value>) -> Self {
        ToolRecordingLlm {
            responses: responses.into(),
            recorded_tools: Vec::new(),
        }
    }
}

impl Llm for ToolRecordingLlm {
    fn complete(
        &mut self,
        _system: &str,
        _messages: &[Message],
        tools: &[Value],
        _obs: &crate::api::ObsCtx<'_>,
    ) -> anyhow::Result<crate::api::Response> {
        self.recorded_tools.push(tools.to_vec());
        self.responses
            .pop_front()
            .map(|body| crate::api::Response { body })
            .ok_or_else(|| anyhow::anyhow!("no scripted response left"))
    }

    fn set_model(&mut self, _model: &str) {}

    fn model(&self) -> &str {
        "tool-recording-model"
    }
}

/// Fake MCP echo server (same script family as mcp.rs's tests) configured
/// via mcp.json in the cwd.
fn write_echo_server(dir: &Path) {
    write_echo_server_prelude(dir, "");
}

/// Variant with a PYTHON PRELUDE prepended to the server script (the T138
/// flag-file harness shape: `import pathlib\npathlib.Path(...).write_text
/// ("ran")\n` runs the instant the command executes, before any
/// handshake) — a flag file is proof of a spawn.
fn write_echo_server_prelude(dir: &Path, prelude: &str) {
    let py = dir.join("fake_srv.py");
    std::fs::write(
            &py,
            format!(
                "{prelude}{}",
                r#"
import sys, json
def send(o):
    sys.stdout.write(json.dumps(o) + "\n"); sys.stdout.flush()
for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    req = json.loads(line)
    if "method" not in req or "id" not in req:
        continue
    m, i = req["method"], req["id"]
    if m == "initialize":
        send({"jsonrpc": "2.0", "id": i, "result": {"protocolVersion": "2025-06-18", "capabilities": {"tools": {}}, "serverInfo": {"name": "fake", "version": "0"}}})
    elif m == "tools/list":
        send({"jsonrpc": "2.0", "id": i, "result": {"tools": [{"name": "echo", "description": "Echo the arguments back", "inputSchema": {"type": "object", "properties": {"text": {"type": "string"}}}}]}})
    elif m == "tools/call":
        send({"jsonrpc": "2.0", "id": i, "result": {"content": [{"type": "text", "text": "echo: " + json.dumps(req["params"]["arguments"])}], "isError": False}})
"#
            ),
        )
        .unwrap();
    let cfg = json!({
        "mcpServers": {
            "fake": {"command": "python3", "args": [py.to_string_lossy()]}
        }
    });
    std::fs::write(dir.join("mcp.json"), cfg.to_string()).unwrap();
}

pub(crate) fn tool_use_response(name: &str, input: Value) -> Value {
    json!({
        "stop_reason": "tool_use",
        "usage": {"input_tokens": 10, "output_tokens": 5},
        "content": [{"type": "tool_use", "id": "tu_1", "name": name, "input": input}],
    })
}

fn text_only_response(text: &str) -> Value {
    json!({
        "stop_reason": "end_turn",
        "usage": {"input_tokens": 10, "output_tokens": 5},
        "content": [{"type": "text", "text": text}],
    })
}

pub(crate) fn ctx_for<'a>(
    tmp: &'a tempfile::TempDir,
    mode: Mode,
    controls: &'a Controls,
    update_rx: &'a Receiver<SlashUpdate>,
    trace: Option<&'a str>,
    obs: &'a observ::Sink,
) -> LoopCtx<'a> {
    LoopCtx {
        cwd: tmp.path(),
        mode,
        controls,
        updates: update_rx,
        bash_timeout: Duration::from_secs(1),
        trace,
        obs,
        plan_out: None,
    }
}

pub(crate) fn knobs_with(max_iters: u32) -> TurnKnobs {
    TurnKnobs {
        spec_path: None,
        goal: None,
        check_cmd: None,
        max_iters,
        max_minutes: 120,
        max_tokens: 0,
    }
}

fn tool_result_text(messages: &[Message]) -> Option<(String, bool)> {
    messages.iter().rev().find_map(|m| match &m.content[0] {
        ContentBlock::Known(KnownBlock::ToolResult {
            content, is_error, ..
        }) => Some((content.as_str().unwrap_or_default().to_string(), *is_error)),
        _ => None,
    })
}

/// Minimal judge double: always returns the same canned verdict.
struct CannedJudge(&'static str, f64);

impl crate::riskgate::Judge for CannedJudge {
    fn judge(&mut self, _command: &str) -> Result<crate::riskgate::Verdict, String> {
        Ok(crate::riskgate::Verdict {
            choice: self.0.to_string(),
            p_destructive: self.1,
        })
    }
}

/// Like `ctx_for` but with Mode::Plan and plan_out — the `--out` path
/// submit_plan writes.
fn ctx_for_plan<'a>(
    tmp: &'a tempfile::TempDir,
    plan_out: Option<&'a Path>,
    controls: &'a Controls,
    urx: &'a Receiver<SlashUpdate>,
) -> LoopCtx<'a> {
    LoopCtx {
        cwd: tmp.path(),
        mode: Mode::Plan,
        controls,
        updates: urx,
        bash_timeout: Duration::from_secs(1),
        trace: None,
        obs: &observ::Sink::Noop,
        plan_out,
    }
}

/// Write a permissions.json deny config into `cwd/.chug/` (T90; shared
/// since T139 — the spec-check gate family denies bash the same way).
fn write_permissions_json(cwd: &Path, deny: Value) {
    let dir = cwd.join(".chug");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        permissions::permissions_path(cwd),
        json!({"permissions": {"deny": deny}}).to_string(),
    )
    .unwrap();
}

/// Write a hooks.json configuring one PreToolUse entry and one
/// PostToolUse entry into `cwd/.chug/`.
fn write_hooks_json(cwd: &Path, pre: Value, post: Value) {
    let dir = cwd.join(".chug");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        hooks::hooks_path(cwd),
        json!({"hooks": {"PreToolUse": pre, "PostToolUse": post}}).to_string(),
    )
    .unwrap();
}

fn hook_entry(glob: &str, command: &str) -> Value {
    json!({"match": glob, "command": command})
}

// ---------- T158: the spawn-invalidation seam (the T59/T66/T151
// `Invalidation` pattern, applied to the spawn-failure-under-resource-
// pressure test family) ----------
//
// At default `cargo test` parallelism the suite runs ~960 unit tests with
// dozens of concurrent REAL process spawns. Under that system-wide
// pressure a `Command::spawn` can fail outright (EAGAIN-class) or a
// trivial child can stretch past a fixed test deadline. Production
// behavior in every observed leg is CORRECT (tool errors surface as
// is_error results; the loop continues) — the red legs are TEST PREMISES
// that assume spawn/schedule success. T151's timing lock cannot help: the
// pressure is system-wide, not inter-test interference. So the fix is
// T31 doctrine, mechanism not timeouts: an attempt invalidated by the
// ENVIRONMENT is retried (bounded, distinctly named); an attempt
// invalidated by the CODE UNDER TEST still panics immediately,
// un-retried, byte-distinct.
//
// The seam wraps one WHOLE scripted attempt (fresh tempdir, scripted LLM,
// drive_loop, assertions). A panic whose message carries one of the
// EXACT markers below is classified as environment-invalidation and the
// whole attempt is retried; any other panic is resumed as-is (the real
// regression is never retried into a flake-shaped message). The contract
// that makes classification possible: the attempt's red-leg assertions
// must EMBED the observed tool evidence in their panic message (e.g.
// `{lines:?}` / `{tools:?}` / `{content}`) — the marker only reaches the
// classifier through that evidence.

/// run_shell's spawn-context error (`src/tools.rs` run_shell: a bash
/// tool call OR the goal-gate check whose spawn failed outright). Reaches
/// the classifier either bare (a drive_loop Err unwrapped) or wrapped as
/// `tool error: spawning sh -c …` in a tool result.
pub(crate) const MARKER_SPAWNING_SH: &str = "spawning sh -c ";

/// hooks.rs PreToolUse/PostToolUse spawn failure (the fail-open text
/// `spawning \`sh -c …\`` that lands in a hook_error detail when the
/// hook's own spawn failed under pressure).
pub(crate) const MARKER_SPAWNING_HOOK_SH: &str = "spawning `sh -c ";

/// Schedule-starvation timeout: run_shell's bash-timeout kill
/// (`timed out after Ns (process group killed)`) and MCP request
/// timeouts (`mcp … request timed out after Ns`) — both mean a trivial
/// child stretched past a fixed deadline under load, i.e. the same
/// environment-invalidated class. The shared prefix is the marker; the
/// suffixes distinguish the two producers.
pub(crate) const MARKER_TIMED_OUT: &str = "timed out after ";

/// mcp.rs stdio/remote server spawn failure (`spawning mcp server {name}`
/// context error, in the fail-soft start-failure log line).
pub(crate) const MARKER_SPAWNING_MCP: &str = "spawning mcp server ";

/// mcp.rs fail-soft start-failure log line
/// (`chug: mcp server {name} failed to start: …`) — the only place this
/// substring occurs in test evidence is the MCP server's own log.
pub(crate) const MARKER_MCP_FAILED_TO_START: &str = " failed to start: ";

/// Bounded retry: 3 total attempts, exhaustion panics naming the class.
pub(crate) const SPAWN_RETRY_ATTEMPTS: usize = 3;

/// Classify a panic message against the EXACT enumerated markers.
/// Returns (class name, matched marker). Every marker here has a
/// near-miss pin (see the T158 pins at the bottom of this file) that dies
/// if the marker is flipped to a non-matching string or loosened to a
/// broader prefix.
fn spawn_invalidation_class_of(message: &str) -> Option<(&'static str, &'static str)> {
    // Order matters only for naming; every entry is an independent match.
    const MARKERS: &[(&str, &str)] = &[
        ("run_shell spawn", MARKER_SPAWNING_SH),
        ("hook spawn", MARKER_SPAWNING_HOOK_SH),
        ("spawn/timeout stretch", MARKER_TIMED_OUT),
        ("mcp server spawn", MARKER_SPAWNING_MCP),
        ("mcp server fail-soft start", MARKER_MCP_FAILED_TO_START),
    ];
    MARKERS
        .iter()
        .find(|(_, marker)| message.contains(marker))
        .map(|(class, marker)| (*class, *marker))
}

/// Extract the human-readable message from a caught panic payload.
fn panic_payload_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&'static str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "<non-string panic payload>".to_string()
    }
}

/// Run one WHOLE scripted drive attempt with bounded spawn-invalidation
/// retries (T158). `attempt` must build its own tempdir, run the scripted
/// drive, and assert — success returns normally; a panic carrying an
/// enumerated spawn-invalidation marker (see the T158 comment block)
/// invalidates the whole attempt and retries it from scratch (fresh
/// tempdir), up to [`SPAWN_RETRY_ATTEMPTS`] total attempts. A panic
/// WITHOUT a marker — a real regression — is resumed un-retried with its
/// original payload, byte-distinct: the message the harness reports is
/// the assertion's own wording, never a flake-shaped retry note.
pub(crate) fn drive_attempt_with_spawn_retry<T>(mut attempt: impl FnMut() -> T) -> T {
    let mut last: Option<(&'static str, String)> = None;
    for n in 1..=SPAWN_RETRY_ATTEMPTS {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(&mut attempt)) {
            Ok(value) => return value,
            Err(payload) => {
                let message = panic_payload_message(payload.as_ref());
                match spawn_invalidation_class_of(&message) {
                    Some((class, marker)) => {
                        eprintln!(
                            "T158 drive_attempt_with_spawn_retry: attempt \
                                 {n}/{SPAWN_RETRY_ATTEMPTS} invalidated by the environment \
                                 ({class}, marker {marker:?}); retrying the WHOLE attempt \
                                 with a fresh tempdir"
                        );
                        last = Some((class, message));
                    }
                    // No marker: a code-under-test failure. Byte-distinct,
                    // un-retried (T59's guarantee).
                    None => std::panic::resume_unwind(payload),
                }
            }
        }
    }
    let (class, message) = last.expect("exhaustion implies at least one classified invalidation");
    panic!(
        "T158 drive_attempt_with_spawn_retry: the {class} spawn invalidation persisted \
             across all {SPAWN_RETRY_ATTEMPTS} attempts (each on a fresh tempdir). The \
             environment invalidated the attempt's spawn premise every time; production \
             behavior (the error surfaces, the loop continues) is correct. \
             Last red evidence: {message}"
    );
}

/// The `.chug/mcp-<name>.log` fail-soft start-failure log, read for the
/// T158 evidence contract (converted MCP legs embed it in their red-leg
/// assertions so the classifier can see the spawn-failure marker).
fn mcp_server_log(cwd: &Path, name: &str) -> String {
    fs::read_to_string(cwd.join(".chug").join(format!("mcp-{name}.log"))).unwrap_or_default()
}

// ---------- T158 pins (non-vacuousness, req 5) ----------

/// Counts drive attempts through the seam (the pins' observability).
#[derive(Default)]
struct AttemptCounter(std::sync::Arc<std::sync::atomic::AtomicUsize>);

impl AttemptCounter {
    fn bump(&self) {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
    fn get(&self) -> usize {
        self.0.load(std::sync::atomic::Ordering::SeqCst)
    }
}

/// (a) The seam retries ONLY on the enumerated markers, EXACTLY: each
/// real marker in a red leg is classified and retried (the counter
/// proves a second attempt ran); each near-miss (marker flipped at one
/// load-bearing token) is NOT classified — the attempt count stays 1 and
/// the original payload is resumed byte-distinct. Flip or loosen a
/// marker string in `spawn_invalidation_class_of` and the matching leg
/// of this pin dies (RED-proof per the spec).
#[test]
fn spawn_retry_seam_classifies_only_the_enumerated_markers() {
    struct Case {
        name: &'static str,
        marker: &'static str,
        near_miss: &'static str,
    }
    let cases = [
        Case {
            name: "run_shell spawn",
            marker: MARKER_SPAWNING_SH,
            near_miss: "spawning sh-c ",
        },
        Case {
            name: "hook spawn",
            marker: MARKER_SPAWNING_HOOK_SH,
            near_miss: "spawning 'sh -c ",
        },
        Case {
            name: "spawn/timeout stretch",
            marker: MARKER_TIMED_OUT,
            near_miss: "timed-out after ",
        },
        Case {
            name: "mcp server spawn",
            marker: MARKER_SPAWNING_MCP,
            near_miss: "spawning mcp-server ",
        },
        Case {
            name: "mcp server fail-soft start",
            marker: MARKER_MCP_FAILED_TO_START,
            near_miss: " failed to start ",
        },
    ];
    for case in &cases {
        let real = format!(
            "called `Result::unwrap()` on an `Err` value: tool error: \
                 {marker}printf 'x…x500': Broken pipe",
            marker = case.marker
        );
        assert_eq!(
            spawn_invalidation_class_of(&real).map(|(class, _)| class),
            Some(case.name),
            "the enumerated marker must classify as its own class: {real:?}"
        );
        let miss = real.replace(case.marker, case.near_miss);
        assert_eq!(
            spawn_invalidation_class_of(&miss),
            None,
            "a near-miss must NOT classify (the marker is exact): {miss:?}"
        );
    }

    // Behavioral half through the seam itself: a marker-carrying red leg
    // is retried (2 attempts observed), a marker-free red leg panics
    // un-retried on attempt 1 with the byte-identical payload.
    let marker_hits = AttemptCounter::default();
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        drive_attempt_with_spawn_retry(|| {
            marker_hits.bump();
            if marker_hits.get() == 1 {
                panic!(
                    "premise broke: tool error: {MARKER_SPAWNING_SH}printf 'x…x500': \
                         os error 35"
                );
            }
        })
    }));
    assert!(outcome.is_ok(), "attempt 2 of a marker-carrying red lands");
    assert_eq!(marker_hits.get(), 2, "exactly one invalidation retry ran");

    let clean_hits = AttemptCounter::default();
    let regressed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            drive_attempt_with_spawn_retry(|| {
                clean_hits.bump();
                panic!("assertion failed: the deny must block: deny-marker.txt exists");
            })
        }));
        // The seam must NOT have retried: re-panic the resumed payload so
        // this test's failure carries the ORIGINAL regression message.
        let Err(payload) = result else {
            panic!("a marker-free red leg must not pass");
        };
        let message = panic_payload_message(payload.as_ref());
        assert_eq!(
            message, "assertion failed: the deny must block: deny-marker.txt exists",
            "the resumed payload must be byte-distinct (un-retried)"
        );
        assert_eq!(
            clean_hits.get(),
            1,
            "a real regression panics un-retried on attempt 1"
        );
    }));
    assert!(regressed.is_ok(), "the un-retried leg must not retry");
}

/// (b) Exhaustion wording names the marker class and attempt count
/// exactly (req 5b): a persistently-invalidated attempt exhausts 3
/// attempts and the panic says both.
#[test]
fn spawn_retry_seam_exhaustion_names_class_and_attempt_count() {
    let hits = AttemptCounter::default();
    let exhausted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        drive_attempt_with_spawn_retry(|| {
            hits.bump();
            panic!("premise broke again: tool error: {MARKER_TIMED_OUT}30s (process group killed)");
        })
    }));
    let Err(payload) = exhausted else {
        panic!("a persistently-invalidated attempt must exhaust");
    };
    let message = panic_payload_message(payload.as_ref());
    assert!(
        message.contains("across all 3 attempts"),
        "exhaustion names the attempt count: {message}"
    );
    assert!(
        message.contains("spawn/timeout stretch"),
        "exhaustion names the marker class: {message}"
    );
    assert_eq!(hits.get(), SPAWN_RETRY_ATTEMPTS, "bounded: 3 attempts");
}

/// (c) Static tie to the production error texts: the seam's markers are
/// the EXACT substrings production emits. A production rewording that
/// silently orphans a marker (or a marker edit that drifts from
/// production) turns this pin red — the flip-proof's static half.
#[test]
fn spawn_retry_markers_match_the_production_error_texts() {
    let tools = fs::read_to_string("src/tools.rs").expect("production tools source");
    let hooks = fs::read_to_string("src/hooks.rs").expect("production hooks source");
    let mcp = fs::read_to_string("src/mcp.rs").expect("production mcp source");
    assert!(
        tools.contains(&format!(
            r#"with_context(|| format!("{MARKER_SPAWNING_SH}{{command}}"))"#
        )),
        "run_shell's spawn context error carries the marker"
    );
    assert!(
        tools.contains(r#""timed out after {}s (process group killed)\n{body}","#),
        "run_shell's timeout marker text is pinned"
    );
    assert!(
        hooks.contains(r#"format!("spawning `sh -c {command}`: {e}")"#),
        "the hook spawn-failure text is pinned"
    );
    assert!(
        mcp.contains(r#"with_context(|| format!("spawning mcp server {name}"))"#),
        "the mcp spawn context error is pinned"
    );
    assert!(
        mcp.contains(r#"mcp server {name} failed to start"#),
        "the mcp fail-soft start-failure log text is pinned"
    );
}
