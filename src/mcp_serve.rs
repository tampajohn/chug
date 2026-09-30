//! T124 — F10 phase 1 + T128 — phase 2a + T129 — phase 2b: `chug mcp-serve`,
//! the stdio MCP SERVER side.
//!
//! Until now chug was an MCP CLIENT only (`src/mcp.rs` consumes servers);
//! nothing exposed chug TO another agent — Claude Code, the bridge fleet,
//! or a second chug could observe a run only by shelling out and mining
//! `.chug/` by hand. This module flips that: `chug mcp-serve` speaks
//! newline-delimited JSON-RPC 2.0 on stdio (the exact framing the client
//! side writes — one object per line) and serves the two read-only tools,
//! [`CHUG_STATUS_TOOL`] and [`CHUG_COLLECT_TOOL`], until stdin EOF.
//!
//! **Stdout purity** — the one rule that shapes everything here: a stdio
//! MCP server's stdout IS the wire. Every byte this process writes to
//! stdout must be a protocol message; a stray banner or log line corrupts
//! the client's framing. So there is no banner, no ledger print, no
//! driver lock, no events.jsonl write, and no console output anywhere in
//! this module — responses go through the single `writeln!` in
//! [`serve_from`], and diagnostics (if any ever appear) go to stderr. The
//! subcommand dispatches STRAIGHT to [`serve`] in `main.rs` — it does not
//! route through any code path that writes a banner or a `run_start` line
//! (pinned structurally by the grep test below).
//!
//! Read-only by default (phases 1–2a): no process spawning, no writes
//! anywhere. T129 adds the FIRST write verb, `chug_launch` (F10 phase 2b),
//! and gates it on the `--allow-launch` server flag — the flag is the
//! policy boundary: without it the server is byte-for-byte the read-only
//! one (`chug_launch` is not advertised in `tools/list` and a call for it
//! gets the unknown-tool error), and with it the tool launches a bounded
//! detached `chug run` through the ONE delegate launch path
//! ([`crate::delegate::delegate_launch`] — no second spawner, no new
//! quoting code). The server adds no bypass: the spawned child is an
//! ordinary `chug run` in the target cwd, subject to that cwd's own
//! permissions/hooks/risk-gate chain and its own `.chug/driver.lock`.
//! T153 adds the SECOND write verb, `chug_cancel` (F10 phase 3a), behind
//! the SAME `--allow-launch` boundary: the fleet's stop button, re-deriving
//! the target's ownership fail-closed from process identity on every call
//! (alive → own process-group leader → command line names `chug run`)
//! before SIGTERM-ing the whole detached process group with one bounded
//! SIGKILL escalation.
//!
//! T157 (F10 phase 3, the control verbs) adds the THIRD and FOURTH write
//! verbs behind a SECOND flag, `--allow-control` — same policy family
//! (default-deny, advertised ⇔ callable), separate boundary: the operator
//! who grants launch need not grant control of running children.
//! `chug_abort` is the RUN-level counterpart of `chug_cancel`: after the
//! same fail-closed ownership re-derivation and the same TERM→grace→KILL
//! group discipline it RECORDS the abort in the child's
//! `.chug/events.jsonl`, so `chug_status`/`chug_collect` report the run as
//! aborted — and it reports the terminal state (`aborted` | `already-done`
//! | `not-found`) instead of a signal payload. `chug_steer` injects an
//! operator steering note into a running child through the driver's
//! EXISTING `[operator]` mechanism: the note is appended to the child's
//! cross-process queue `.chug/steer.jsonl` and the child's
//! [`crate::driver::drive_loop`] drains that queue at its next iteration
//! boundary into the same `append_steering_notes` path the TUI chat dock
//! feeds — a detached child has a dead in-process channel, so the queue
//! file is the transport, not a new mechanism.
//!
//! `chug_status` reads `<cwd>/.chug/events.jsonl` through the
//! delegate seams and renders a COMPACT summary; `chug_collect` answers
//! the structured-result question over the SAME file — the latest
//! segment's verdict, the accepted goal's summary, the check cmd, the
//! child's liveness when a pid is given, and best-effort commit refs —
//! built on delegate's T69 collect seams
//! ([`crate::delegate::summarize_collect`],
//! [`crate::delegate::collect_git_commits`]). Both renders are MCP-side:
//! the delegate `render_status`/`render_collect` texts are byte-pinned by
//! the delegate tests and are deliberately NOT reused.
//!
//! Deferred (F10 phase 3b): a server log file, `tools/listChanged`,
//! notifications, resources/prompts — no consumer pulls MCP-spec
//! completeness surfaces (the cycle-72 EVALUATION §4 reason). Cancellation
//! SHIPPED in phase 3a (T153, [`CHUG_CANCEL_TOOL`]).

use std::fs;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

use crate::delegate::DelegateSummary;
use crate::mcp::PROTOCOL_VERSION;

/// The binary's version, as reported in `initialize`'s `serverInfo`.
const SERVER_VERSION: &str = crate::build_info::VERSION;

/// The one phase-1 tool: a compact, self-describing summary of a chug
/// cwd's LATEST `.chug/events.jsonl` run segment. Read-only: reads one
/// file, spawns nothing, writes nothing.
const CHUG_STATUS_TOOL: &str = "chug_status";

/// The phase-2a tool (T128): a chug cwd's structured RESULT — the latest
/// run segment's verdict, the accepted goal's summary, the check cmd, the
/// child's liveness when a pid is given, and best-effort commit refs.
/// Read-only: reads one file + `git log`, spawns nothing, writes nothing.
const CHUG_COLLECT_TOOL: &str = "chug_collect";

/// The phase-2b write tool (T129): launch a bounded detached `chug run` in
/// a chug working directory. Advertised and callable ONLY when the server
/// was started with `--allow-launch` — the flag is the policy boundary
/// (default OFF: a read-only deployment cannot be surprised into spawning).
const CHUG_LAUNCH_TOOL: &str = "chug_launch";

/// The phase-3a write tool (T153): stop a previously launched detached
/// `chug run` — SIGTERM to its whole process GROUP with one bounded SIGKILL
/// escalation. Advertised and callable ONLY under `--allow-launch`: the
/// flag gates the write SURFACE, not individual tools, so the second write
/// leg rides the same policy boundary as [`CHUG_LAUNCH_TOOL`].
const CHUG_CANCEL_TOOL: &str = "chug_cancel";

/// The phase-3 control verb (T157): abort a previously launched detached
/// `chug run` by id (the pid a `chug_launch` result carried) — the
/// run-level counterpart of [`CHUG_CANCEL_TOOL`] that RECORDS the abort in
/// the child's `.chug/events.jsonl` and reports the terminal state.
/// Advertised and callable ONLY under `--allow-control`.
const CHUG_ABORT_TOOL: &str = "chug_abort";

/// The phase-3 control verb (T157): inject an operator steering note into a
/// running detached `chug run` — the driver's existing `[operator]`
/// mechanism, over the cross-process queue file. Advertised and callable
/// ONLY under `--allow-control`.
const CHUG_STEER_TOOL: &str = "chug_steer";

/// T157: the `reason` recorded in the child's `.chug/events.jsonl` abort
/// line by [`chug_abort`] — distinctive (T130), so a postmortem can tell an
/// MCP abort from the driver's own "operator abort".
const CHUG_ABORT_REASON: &str = "operator abort via chug_abort";

/// T157: the steering-note size ceiling — a note is an operator sentence,
/// not a payload; above it the call is REJECTED naming the received length
/// (never clamped), so a runaway caller cannot flood the child's transcript
/// through the queue.
const CHUG_STEER_NOTE_MAX_CHARS: usize = 4000;

/// T157: the write-surface policy — which write tools this server
/// advertises and serves. Every leg is default-deny (`Gates::default()` is
/// the byte-identical read-only server). Two independent boundaries:
/// `allow_launch` gates [`CHUG_LAUNCH_TOOL`] + [`CHUG_CANCEL_TOOL`]
/// (T129/T153), `allow_control` gates [`CHUG_ABORT_TOOL`] +
/// [`CHUG_STEER_TOOL`] (T157) — the operator who grants spawning need not
/// grant control of running children. Advertised ⇔ callable by
/// construction: the same gates feed `tools/list` and `tools/call`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Gates {
    pub allow_launch: bool,
    pub allow_control: bool,
}

/// T153: the one escalation grace — how long a TERM'd process group has to
/// empty before the group is SIGKILLed. Polled at
/// [`CHUG_CANCEL_POLL_MS`]; `~5 s` per the spec.
const CHUG_CANCEL_GRACE_MS: u64 = 5_000;

/// T153: the escalation poll cadence (≤100 ms per the spec).
const CHUG_CANCEL_POLL_MS: u64 = 50;

/// T129: the `chug_launch` iteration-budget ceiling — loopd's own ceiling.
/// A `max_iters` above it is REJECTED (never clamped) naming the received
/// value and this number.
const CHUG_LAUNCH_MAX_ITERS_CEILING: u64 = 200;

/// T129: the `chug_launch` wall-clock-budget ceiling — loopd's own. A
/// `max_minutes` above it is REJECTED (never clamped) naming the received
/// value and this number.
const CHUG_LAUNCH_MAX_MINUTES_CEILING: u64 = 240;

// ---------------------------------------------------------------------------
// The serve loop
// ---------------------------------------------------------------------------

/// Run the server on real stdio until stdin EOF, then return (the caller
/// maps that to exit 0). Responses are written and flushed one at a time —
/// an unflushed response is a wedged client.
///
/// T129/T153/T157: `gates` decides which write tools exist (see
/// [`Gates`]). The default (`Gates::default()`, and every flagless
/// invocation) is byte-identical to the pre-T129 read-only server.
pub(crate) fn serve(gates: Gates) -> anyhow::Result<()> {
    // Stdout purity by construction: the ONLY stdout writer in the module
    // is the protocol `writeln!` inside serve_from (see the module doc).
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    serve_from_with(&mut stdin.lock(), &mut out, gates)
}

/// The loop over an injected reader/writer pair, so the EOF and
/// blank-line-skipping behavior is unit-testable without touching real
/// stdio. Serial dispatch (read-only tools are fast; the write legs'
/// bounded waits are rare). The flagless entry the framing test legs ride
/// lives in the test module (`serve_from` there → this with
/// `Gates::default()`), so the production half carries only the gated
/// loop.
fn serve_from_with(read: &mut impl BufRead, out: &mut impl Write, gates: Gates) -> anyhow::Result<()> {
    for line in read.lines() {
        let line = line?;
        // Blank lines are framing noise, not messages — skip them.
        if line.trim().is_empty() {
            continue;
        }
        if let Some(response) = handle_message_with(&line, gates) {
            // The single protocol writer. Flushed per response so a client
            // blocked on read never waits on a buffer.
            writeln!(out, "{response}")?;
            out.flush()?;
        }
    }
    // stdin EOF: the server's normal end of life. No summary line, no
    // banner — stdout has carried only protocol messages.
    Ok(())
}

// ---------------------------------------------------------------------------
// Dispatch: one JSON-RPC line → the optional response line
// ---------------------------------------------------------------------------

/// Handle one framed message. `None` = no response (notifications never
/// get one; blank lines never reach here). Every error leg is a JSON-RPC
/// error response — the loop keeps reading no matter what arrives.
///
/// Error taxonomy (JSON-RPC 2.0 standard codes):
/// - line not parseable as JSON → `-32700`, `"id": null`
/// - request missing `method` → `-32600`
/// - unknown method on a request → `-32601`
/// - `tools/call` naming an unlisted tool (or missing a usable name) → `-32602`
///
/// T129/T153/T157: the gates decide which write tools exist — advertised
/// in `tools/list` ⇔ callable in `tools/call`, both fed by the same
/// [`Gates`] value (capability honesty by construction). `allow_launch`
/// gates `chug_launch` and `chug_cancel`; `allow_control` gates the
/// control verbs (`chug_abort`, `chug_steer`). An ungated write tool falls
/// through to the unknown-tool
/// arm — the SAME `-32602` a never-existing tool gets, so a read-only
/// deployment cannot be probed into revealing that a write tool exists
/// behind a flag (and no error kills the loop).
fn handle_message_with(line: &str, gates: Gates) -> Option<String> {
    let value: Value = match serde_json::from_str(line) {
        Ok(value) => value,
        // A line we cannot parse has no id to echo — the spec-mandated null.
        Err(_) => return Some(error_response(Value::Null, -32700, "Parse error")),
    };
    let Some(obj) = value.as_object() else {
        // Parses as JSON but is not a request object — not a Parse error,
        // an Invalid Request (no usable id → null).
        return Some(error_response(Value::Null, -32600, "Invalid Request"));
    };
    let id = obj.get("id").cloned();
    let Some(method) = obj.get("method").and_then(Value::as_str) else {
        // A request (id present) missing its method is invalid. A
        // methodless notification cannot be routed and gets nothing.
        return id.map(|id| error_response(id, -32600, "Invalid Request: missing method"));
    };
    // Notifications NEVER get a response — not for `notifications/initialized`,
    // not for an unknown `notifications/foo`, and not for anything under the
    // `notifications/` prefix even if it illegally carries an id (MCP rule:
    // the prefix means the client does not want a reply).
    if id.is_none() || method.starts_with("notifications/") {
        return None;
    }
    let id = id.expect("id checked Some above");
    match method {
        "initialize" => Some(ok_response(id, initialize_result())),
        "ping" => Some(ok_response(id, json!({}))),
        "tools/list" => Some(ok_response(id, json!({ "tools": tools_list(gates) }))),
        "tools/call" => Some(call_tool(id, obj, gates)),
        other => Some(error_response(
            id,
            -32601,
            &format!("Method not found: {other}"),
        )),
    }
}

/// The `initialize` result. The protocol version is REUSED from the client
/// module (`src/mcp.rs`) — one literal, both sides of the wire.
fn initialize_result() -> Value {
    json!({
        "protocolVersion": PROTOCOL_VERSION,
        "capabilities": { "tools": {} },
        "serverInfo": { "name": "chug", "version": SERVER_VERSION },
    })
}

/// The `tools/list` tool set. Capability honesty (T129, carried by T153 +
/// T157): a write leg is advertised ONLY when the server was started with
/// its flag — the same [`Gates`] value feeds `tools/call`, so advertised ⇔
/// callable by construction. Two independent boundaries: `--allow-launch`
/// gates the write surface (`chug_launch` + `chug_cancel`), `--allow-control`
/// gates the control surface (`chug_abort` + `chug_steer`) — the operator
/// who grants spawning need not grant control of running children.
fn tools_list(gates: Gates) -> Vec<Value> {
    let mut tools = vec![chug_status_schema(), chug_collect_schema()];
    if gates.allow_launch {
        tools.push(chug_launch_schema());
        tools.push(chug_cancel_schema());
    }
    if gates.allow_control {
        tools.push(chug_abort_schema());
        tools.push(chug_steer_schema());
    }
    tools
}

/// The `chug_status` listing entry. `inputSchema.required` names `cwd` —
/// pinned by tests on both sides of the framing.
fn chug_status_schema() -> Value {
    json!({
        "name": CHUG_STATUS_TOOL,
        "description":
            "Summarize the LATEST run segment of a chug cwd's .chug/events.jsonl \
             (state, last_iteration vs max_iters, goal/abort/budget-low flags, \
             abort reason). Read-only: spawns nothing, writes nothing.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "cwd": {
                    "type": "string",
                    "description":
                        "Absolute path to the chug working directory to observe \
                         (must exist and contain .chug/events.jsonl)"
                }
            },
            "required": ["cwd"]
        }
    })
}

/// The `chug_collect` listing entry (T128): same required `cwd`, plus the
/// optional `pid` (liveness line, mirroring `delegate collect`'s pid
/// semantics) and `base` (scopes the commit-refs range as `<base>..HEAD`).
fn chug_collect_schema() -> Value {
    json!({
        "name": CHUG_COLLECT_TOOL,
        "description":
            "Collect a chug cwd's structured result: the LATEST run segment's \
             verdict (goal-accepted/goal-rejected/aborted/running/starting), \
             the accepted goal's summary, the latest check cmd, the child's \
             liveness when a pid is given, and best-effort commit refs. \
             Read-only: spawns nothing, writes nothing.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "cwd": {
                    "type": "string",
                    "description":
                        "Absolute path to the chug working directory to observe \
                         (must exist and contain .chug/)"
                },
                "pid": {
                    "type": "integer",
                    "description":
                        "Optional child pid — adds a liveness line (alive \
                         true/false); absent means no liveness claim"
                },
                "base": {
                    "type": "string",
                    "description":
                        "Optional git ref scoping the commit-refs range as \
                         <base>..HEAD; absent means the bounded default range \
                         over HEAD"
                }
            },
            "required": ["cwd"]
        }
    })
}

/// The `chug_launch` listing entry (T129): the write leg. Four required
/// params (`cwd`, `spec`, `goal`, `model` — the required list is pinned
/// exact) plus the two optional budgets carrying loopd's ceilings
/// (200 iters / 240 minutes) in the schema itself. Advertised only under
/// `--allow-launch`.
fn chug_launch_schema() -> Value {
    json!({
        "name": CHUG_LAUNCH_TOOL,
        "description":
            "Launch a bounded detached `chug run` in a chug working directory \
             (the write leg; requires the server to be started with \
             --allow-launch). The child is an ordinary `chug run` in the \
             target cwd — subject to that cwd's own permissions/hooks/\
             risk-gate chain and its own single-driver lock, exactly as if \
             a human typed the command. Returns the spawned pid, the events \
             path, and the log path.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "cwd": {
                    "type": "string",
                    "description":
                        "Absolute path to the chug working directory to launch \
                         in (must exist and contain .chug/); the child runs \
                         with this as its cwd"
                },
                "spec": {
                    "type": "string",
                    "description":
                        "Absolute path to an existing spec file for the child run"
                },
                "goal": {
                    "type": "string",
                    "description": "Non-empty goal text for the child run"
                },
                "model": {
                    "type": "string",
                    "description":
                        "Non-empty model id, passed through — the spawned \
                         child's own auth/settings chain validates it"
                },
                "max_iters": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 200,
                    "description":
                        "Optional iteration budget, 1..=200 — a value above \
                         the 200 ceiling is rejected, not clamped"
                },
                "max_minutes": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 240,
                    "description":
                        "Optional wall-clock budget in minutes, 1..=240 — a \
                         value above the 240 ceiling is rejected, not clamped"
                }
            },
            "required": ["cwd", "spec", "goal", "model"]
        }
    })
}

/// The `chug_cancel` listing entry (T153): the second write leg, behind the
/// SAME `--allow-launch` policy boundary as `chug_launch`. Two required
/// params: the chug cwd (validated like every tool's) and the pid to stop.
fn chug_cancel_schema() -> Value {
    json!({
        "name": CHUG_CANCEL_TOOL,
        "description":
            "Stop a previously launched detached `chug run` (the write leg; \
             requires the server to be started with --allow-launch). Before \
             any signal the ownership of <pid> is re-derived fail-closed: \
             the pid must be alive, must be its own process-group leader \
             (the delegate detached-spawn fingerprint), and its command \
             line must name a `chug run` invocation — the first failed leg \
             is an isError result and NOTHING is signalled. On pass: \
             SIGTERM to the process group (the whole detached tree dies, \
             not just the driver), up to ~5 s grace, then SIGKILL to the \
             group if it is still alive.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "cwd": {
                    "type": "string",
                    "description":
                        "Absolute path to the chug working directory the \
                         child was launched in (must exist and contain \
                         .chug/)"
                },
                "pid": {
                    "type": "integer",
                    "minimum": 1,
                    "description":
                        "The child pid to cancel — the pid returned by \
                         chug_launch"
                }
            },
            "required": ["cwd", "pid"]
        }
    })
}

/// The `chug_abort` listing entry (T157): the run-level control verb,
/// behind `--allow-control`. Same required input shape as `chug_cancel`
/// (the id a `chug_launch` result carried), a different CONTRACT: the
/// terminal state is reported (`aborted` | `already-done` | `not-found`)
/// and a successful abort is RECORDED in the child's `.chug/events.jsonl`
/// so the read tools report the run as aborted.
fn chug_abort_schema() -> Value {
    json!({
        "name": CHUG_ABORT_TOOL,
        "description":
            "Abort a previously launched detached `chug run` by id — the pid \
             a chug_launch result carried (the control leg; requires the \
             server to be started with --allow-control). The run-level \
             counterpart of chug_cancel: the same fail-closed ownership \
             re-derivation (alive, own process-group leader, `chug run` \
             command line) and the same SIGTERM-group → ~5 s grace → \
             SIGKILL-group discipline, plus the abort RECORDED in the \
             child's .chug/events.jsonl so chug_status/chug_collect report \
             the run as aborted. The result names the terminal state: \
             `aborted` (signalled and recorded), `already-done` (the latest \
             segment already has a verdict — idempotent, nothing signalled), \
             or `not-found` (no such process, no verdict — nothing \
             signalled).",
        "inputSchema": {
            "type": "object",
            "properties": {
                "cwd": {
                    "type": "string",
                    "description":
                        "Absolute path to the chug working directory the \
                         child was launched in (must exist and contain \
                         .chug/); the abort record is written here"
                },
                "pid": {
                    "type": "integer",
                    "minimum": 1,
                    "description":
                        "The child pid to abort — the pid returned by \
                         chug_launch"
                }
            },
            "required": ["cwd", "pid"]
        }
    })
}

/// The `chug_steer` listing entry (T157): the second control verb, behind
/// `--allow-control`. The note rides the driver's EXISTING `[operator]`
/// mechanism — queued in the child's `.chug/steer.jsonl`, drained at the
/// child's next iteration boundary, landed as a user message in the
/// child's transcript. Required: cwd, pid, note.
fn chug_steer_schema() -> Value {
    json!({
        "name": CHUG_STEER_TOOL,
        "description":
            "Inject an operator steering note into a running detached `chug \
             run` (the control leg; requires the server to be started with \
             --allow-control). The note lands through the driver's EXISTING \
             `[operator]` mechanism: appended to the child's cross-process \
             queue (.chug/steer.jsonl), drained at the child's next \
             iteration boundary, and delivered as a user message in the \
             child's transcript context. The result reports `queued` with \
             the queue path, or `undeliverable` when the child's latest run \
             segment already has a verdict or the pid is not alive — in \
             which case NOTHING is written.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "cwd": {
                    "type": "string",
                    "description":
                        "Absolute path to the chug working directory the \
                         child was launched in (must exist and contain \
                         .chug/); the queue lives at .chug/steer.jsonl"
                },
                "pid": {
                    "type": "integer",
                    "minimum": 1,
                    "description":
                        "The child pid to steer — the pid returned by \
                         chug_launch"
                },
                "note": {
                    "type": "string",
                    "description":
                        "The operator steering note (non-empty after trim, \
                         at most 4000 characters — rejected above, never \
                         clamped)"
                }
            },
            "required": ["cwd", "pid", "note"]
        }
    })
}

/// Dispatch `tools/call`. A known tool's OWN failure (bad cwd, unreadable
/// events, refused launch) is a tool RESULT with `isError: true` — not a
/// JSON-RPC error — so the caller sees the tool ran and failed, the shape
/// the client side already parses (`isError` + text content).
///
/// T129: `chug_launch` is routable only under `--allow-launch`; without
/// the flag its name falls through to the unknown-tool arm — the
/// SAME `-32602` a never-existing tool gets, so a read-only deployment
/// cannot be probed into revealing that a launch tool exists behind a
/// flag (and no error kills the loop). T153: `chug_cancel` rides the
/// same arm. T157: the control verbs (`chug_abort`, `chug_steer`) ride
/// the same shape under the SECOND boundary, `--allow-control`.
fn call_tool(id: Value, req: &Map<String, Value>, gates: Gates) -> String {
    let params = req.get("params").cloned().unwrap_or(Value::Null);
    let Some(name) = params.get("name").and_then(Value::as_str) else {
        return error_response(id, -32602, "Invalid params: tools/call requires a tool name");
    };
    let arguments = params.get("arguments").cloned().unwrap_or(json!({}));
    let (text, is_error) = match name {
        CHUG_STATUS_TOOL => chug_status(&arguments),
        CHUG_COLLECT_TOOL => chug_collect(&arguments),
        CHUG_LAUNCH_TOOL if gates.allow_launch => chug_launch(&arguments),
        CHUG_CANCEL_TOOL if gates.allow_launch => chug_cancel(&arguments),
        CHUG_ABORT_TOOL if gates.allow_control => chug_abort(&arguments),
        CHUG_STEER_TOOL if gates.allow_control => chug_steer(&arguments),
        other => {
            return error_response(id, -32602, &format!("unknown tool: {other}"));
        }
    };
    ok_response(
        id,
        json!({
            "content": [ { "type": "text", "text": text } ],
            "isError": is_error,
        }),
    )
}

// ---------------------------------------------------------------------------
// The tools
// ---------------------------------------------------------------------------

/// The shared fail-fast `cwd` validator (T128): the exact delegate-launch
/// legs both tools serve — absolute, then exists as a directory, then has a
/// `.chug/` directory. Each violation names the RECEIVED path verbatim and
/// carries the CALLING tool's name, so `chug_status`'s texts are byte-
/// identical to the pre-factor messages (the phase-1 tests pin them).
fn validate_chug_cwd(tool: &str, raw: &str) -> Result<PathBuf, String> {
    let cwd = PathBuf::from(raw);
    // Same fail-fast legs as delegate's cwd parse: absolute, then exists.
    if !cwd.is_absolute() {
        return Err(format!("{tool}: cwd must be an absolute directory, got {raw:?}"));
    }
    if !cwd.is_dir() {
        return Err(format!(
            "{tool}: cwd does not exist or is not a directory: {}",
            cwd.display()
        ));
    }
    let chug_dir = cwd.join(".chug");
    if !chug_dir.is_dir() {
        return Err(format!("{tool}: no .chug/ directory in {}", cwd.display()));
    }
    Ok(cwd)
}

/// `chug_status`: validate the cwd the delegate-launch fail-fast way (each
/// violation is an `isError` text naming the RECEIVED path verbatim), then
/// summarize the events file through the delegate seams. Missing/unreadable
/// events → an `isError` result naming the path — never a panic.
fn chug_status(args: &Value) -> (String, bool) {
    let Some(raw) = args.get("cwd").and_then(Value::as_str) else {
        return (
            format!("{CHUG_STATUS_TOOL}: missing required argument: cwd (an absolute path to a chug working directory)"),
            true,
        );
    };
    let cwd = match validate_chug_cwd(CHUG_STATUS_TOOL, raw) {
        Ok(cwd) => cwd,
        Err(message) => return (message, true),
    };
    let events_path = cwd.join(".chug").join("events.jsonl");
    let (summary, note) = crate::delegate::read_events(&events_path);
    if let Some(note) = note {
        return (
            format!(
                "{CHUG_STATUS_TOOL}: events file unreadable: {} ({note})",
                events_path.display()
            ),
            true,
        );
    }
    (render_compact_status(&cwd, &events_path, &summary), false)
}

/// `chug_collect` (T128): the structured-result answer over the same
/// latest-segment events stream, built entirely on delegate's T69 collect
/// seams — `read_events_tail` for the bounded tail, `summarize_collect` for
/// the parse, `reap_and_alive` for the pid line (the exact `delegate
/// collect` semantics: absent pid → no liveness claim), `collect_git_commits`
/// for the commit refs. Read-only: reads one file + one `git log`, spawns
/// nothing, writes nothing. Every failure leg is an `isError` result
/// (missing cwd legs, unreadable events, non-string base) or a degraded
/// note (git absent/failing) — never a panic, never a killed server loop.
fn chug_collect(args: &Value) -> (String, bool) {
    let Some(raw) = args.get("cwd").and_then(Value::as_str) else {
        return (
            format!("{CHUG_COLLECT_TOOL}: missing required argument: cwd (an absolute path to a chug working directory)"),
            true,
        );
    };
    let cwd = match validate_chug_cwd(CHUG_COLLECT_TOOL, raw) {
        Ok(cwd) => cwd,
        Err(message) => return (message, true),
    };
    let events_path = cwd.join(".chug").join("events.jsonl");
    let lines = match crate::delegate::read_events_tail(&events_path) {
        Ok(lines) => lines,
        Err(e) => {
            return (
                format!(
                    "{CHUG_COLLECT_TOOL}: events file unreadable: {} (events: nothing read ({e:#}))",
                    events_path.display()
                ),
                true,
            );
        }
    };
    let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
    let summary = crate::delegate::summarize_collect(&refs);
    // Same liveness leg `delegate collect` has — absent pid → no liveness
    // claim at all; a non-integer pid is the same silence (the schema says
    // integer, and the mirrored seam's semantics win over a second parser).
    let pid = args.get("pid").and_then(Value::as_u64);
    let alive = pid.and_then(crate::delegate::reap_and_alive);
    // `base` scopes the commit-refs range as `<base>..HEAD`. A non-string
    // value is an error, never a silent default-range fallback — the T69
    // `delegate_base` rule: a caller that asked for a range must not
    // silently get the default range.
    let base = match args.get("base") {
        None | Some(Value::Null) => None,
        Some(value) => match value.as_str() {
            Some(s) => Some(s.to_string()),
            None => {
                return (
                    format!(
                        "{CHUG_COLLECT_TOOL}: `base` must be a string git ref \
                         (e.g. \"origin/main\") scoping the commit range <base>..HEAD"
                    ),
                    true,
                );
            }
        },
    };
    let commits = crate::delegate::collect_git_commits(&cwd, base.as_deref());
    (
        render_compact_collect(&cwd, &events_path, &summary, alive, base.as_deref(), &commits),
        false,
    )
}

// ---------------------------------------------------------------------------
// T129: the write leg — chug_launch (flag-gated)
// ---------------------------------------------------------------------------

/// `chug_launch` (T129): validate the request, then hand it to the ONE
/// delegate launch path ([`crate::delegate::delegate_launch`]) — no second
/// spawner: the detached spawn (`process_group(0)` + SIGHUP-ignore nohup
/// parity), the `<cwd>/.chug/delegate.log` log, the `CHUG_DELEGATE_BIN`
/// binary seam, the goal's byte-exact argv delivery (the delegate path
/// spawns the binary directly with `Command` args, so quoting is the argv
/// mechanism's job — no new quoting code here), and the default budgets
/// (40/35 when absent) all belong to that mechanism.
///
/// The server adds no bypass: the child is an ordinary `chug run` in the
/// target cwd, subject to that cwd's own `.chug/permissions.json` deny
/// rules, `.chug/hooks.json` vetoes, risk gate, and its own
/// `.chug/driver.lock` (a conflicting launch fails fast child-side and
/// surfaces via `chug_status`/`chug_collect`).
///
/// Every validation failure is an `isError` result naming the RECEIVED
/// value (the phase-1 text shape); a launch failure (spawn error, or a
/// lock conflict surfacing at spawn time) is likewise an `isError` result —
/// never a panic, never a killed server loop, never a stray stdout byte.
fn chug_launch(args: &Value) -> (String, bool) {
    let launch_input = match validate_launch_request(args) {
        Ok(input) => input,
        Err(message) => return (message, true),
    };
    match crate::delegate::delegate_launch(&launch_input) {
        // The delegate launch result text — pid, log, events, the budgets —
        // returned as-is (mirroring delegate launch's return shape).
        Ok(result) => (result.content, false),
        Err(e) => (format!("{CHUG_LAUNCH_TOOL}: {e:#}"), true),
    }
}

/// The full `chug_launch` validation chain, producing the delegate launch
/// input. Four required params plus the two optional budgets; every
/// violation names the RECEIVED value verbatim (the phase-1 error shape).
fn validate_launch_request(args: &Value) -> Result<Value, String> {
    // cwd: the SAME fail-fast validator `chug_status`/`chug_collect` serve —
    // absolute, then exists as a directory, then has a `.chug/` directory —
    // with error-text parity (the tool name prefixes each message).
    let Some(raw_cwd) = args.get("cwd").and_then(Value::as_str) else {
        return Err(format!(
            "{CHUG_LAUNCH_TOOL}: missing required argument: cwd (an absolute path to a chug working directory)"
        ));
    };
    let cwd = validate_chug_cwd(CHUG_LAUNCH_TOOL, raw_cwd)?;
    // spec: an absolute path to an existing FILE — the same probe delegate
    // launch serves (`File::open` is the readability ground truth; open(2)
    // alone would admit a directory), so every leg carries the
    // `chug_launch` voice and the seam-side re-validation is a no-op.
    let Some(raw_spec) = args.get("spec").and_then(Value::as_str) else {
        return Err(format!(
            "{CHUG_LAUNCH_TOOL}: missing required argument: spec (an absolute path to a spec file)"
        ));
    };
    let spec = PathBuf::from(raw_spec);
    if !spec.is_absolute() {
        return Err(format!(
            "{CHUG_LAUNCH_TOOL}: spec must be an absolute path, got {raw_spec:?}"
        ));
    }
    if !(spec.is_file() && fs::File::open(&spec).is_ok()) {
        return Err(format!(
            "{CHUG_LAUNCH_TOOL}: spec does not exist or is not a readable file: {}",
            spec.display()
        ));
    }
    // goal: non-empty after trim — an empty goal would spawn a child with
    // nothing to do.
    let Some(goal) = args.get("goal").and_then(Value::as_str) else {
        return Err(format!(
            "{CHUG_LAUNCH_TOOL}: missing required argument: goal (the child run's goal text)"
        ));
    };
    if goal.trim().is_empty() {
        return Err(format!(
            "{CHUG_LAUNCH_TOOL}: goal must be a non-empty string after trim, got {goal:?}"
        ));
    }
    // model: non-empty, passed through — the spawned child's own
    // auth/settings chain validates it; the server adds no model policy.
    let Some(model) = args.get("model").and_then(Value::as_str) else {
        return Err(format!(
            "{CHUG_LAUNCH_TOOL}: missing required argument: model (the child run's model id)"
        ));
    };
    if model.is_empty() {
        return Err(format!(
            "{CHUG_LAUNCH_TOOL}: model must be a non-empty string, got {model:?}"
        ));
    }
    let (max_iters, max_minutes) = parse_launch_budgets(args)?;
    // The delegate launch input, assembled: exactly the fields the seam
    // reads. Absent budgets are OMITTED (the delegate defaults 40/35 then
    // apply seam-side — same as a human's flagless launch);
    // `max_tokens`/`resume` pass-through is out of T129's scope.
    let mut input = serde_json::Map::new();
    input.insert("cwd".to_string(), json!(cwd.display().to_string()));
    input.insert("spec".to_string(), json!(spec.display().to_string()));
    input.insert("goal".to_string(), json!(goal));
    input.insert("model".to_string(), json!(model));
    if let Some(iters) = max_iters {
        input.insert("max_iters".to_string(), json!(iters));
    }
    if let Some(mins) = max_minutes {
        input.insert("max_minutes".to_string(), json!(mins));
    }
    Ok(Value::Object(input))
}

/// One optional budget (`max_iters` / `max_minutes`): absent (or JSON null)
/// → `None` (the delegate launch defaults apply seam-side); present → an
/// integer in `1..=ceiling`. Reject, never clamp (the delegate `wait_secs`
/// rule): a value above the ceiling is REFUSED naming the received value
/// and the ceiling — `chug_launch`'s ceilings are loopd's own (200
/// iterations / 240 minutes), so a silently-clamped launch would run a
/// different loop than the caller asked for.
fn parse_launch_budget(args: &Value, key: &str, ceiling: u64) -> Result<Option<u64>, String> {
    let Some(value) = args.get(key) else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    let Some(n) = value.as_i64() else {
        return Err(format!(
            "{CHUG_LAUNCH_TOOL}: `{key}` must be an integer (1..={ceiling}), got {value}"
        ));
    };
    if n < 1 {
        return Err(format!("{CHUG_LAUNCH_TOOL}: `{key}` must be at least 1, got {n}"));
    }
    // n >= 1 here, so the cast is lossless.
    let n = n as u64;
    if n > ceiling {
        return Err(format!(
            "{CHUG_LAUNCH_TOOL}: `{key}` must be at most {ceiling}, got {n}"
        ));
    }
    Ok(Some(n))
}

/// Both optional budgets in one pass, in schema order.
fn parse_launch_budgets(args: &Value) -> Result<(Option<u64>, Option<u64>), String> {
    let max_iters = parse_launch_budget(args, "max_iters", CHUG_LAUNCH_MAX_ITERS_CEILING)?;
    let max_minutes = parse_launch_budget(args, "max_minutes", CHUG_LAUNCH_MAX_MINUTES_CEILING)?;
    Ok((max_iters, max_minutes))
}

// ---------------------------------------------------------------------------
// T153: the second write leg — chug_cancel (flag-gated)
// ---------------------------------------------------------------------------

/// `chug_cancel` (T153): stop a previously launched detached `chug run`.
///
/// **Ownership is re-derived per call, never remembered.** The server is
/// stateless across requests — it keeps no launch registry and no
/// server-side state of any kind (out of scope by design) — so "is this
/// pid mine to signal?" is answered fresh from PROCESS IDENTITY on every
/// call, never from a server-side launch log. All three legs must hold or
/// the result is an `isError` naming the FIRST failed leg and NO signal is
/// sent; an unresolvable leg fails closed (skip, name the leg). A wrong
/// cancel is un-undoable; a refused one is retryable.
///
/// On pass: SIGTERM to the process GROUP (negative-pid kill — the whole
/// detached tree dies, not just the driver), then ONE bounded escalation:
/// up to [`CHUG_CANCEL_GRACE_MS`] (polled at [`CHUG_CANCEL_POLL_MS`]) for
/// the group to empty, then SIGKILL to the group. Payload: `pid`,
/// `signaled: "term"|"kill"`, `waited_ms` — `"term"` when the group
/// emptied within the grace, `"kill"` when the escalation fired.
///
/// Serial-loop note: the escalation wait can hold the single-threaded
/// dispatch for up to the grace. A cancel is rare and bounded, so phase 3a
/// adds no concurrency.
fn chug_cancel(args: &Value) -> (String, bool) {
    // cwd: the SAME fail-fast validator every tool serves — validated (the
    // caller names the chug cwd the child was launched in) but otherwise
    // unused: the pid is the kill target, the cwd is the address space the
    // driver is talking about.
    let Some(raw_cwd) = args.get("cwd").and_then(Value::as_str) else {
        return (
            format!("{CHUG_CANCEL_TOOL}: missing required argument: cwd (an absolute path to a chug working directory)"),
            true,
        );
    };
    if let Err(message) = validate_chug_cwd(CHUG_CANCEL_TOOL, raw_cwd) {
        return (message, true);
    }
    // pid: a positive integer — the shared control-verb contract (T153 +
    // T157). Non-integer / non-positive / missing is the validation-chain
    // error naming what was RECEIVED (the T129 received-value honesty
    // pattern).
    let pid = match parse_pid_argument(CHUG_CANCEL_TOOL, args) {
        Ok(pid) => pid,
        Err(message) => return (message, true),
    };
    #[cfg(unix)]
    {
        cancel_unix(pid)
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        (
            format!(
                "{CHUG_CANCEL_TOOL}: not supported on this platform (process \
                 groups and signals are unix-only) — nothing signalled"
            ),
            true,
        )
    }
}

/// The shared `pid` input contract of the signal verbs (T153 `chug_cancel`
/// and T157 `chug_abort`): required, a positive integer, at most i32::MAX
/// (a pid must fit the kill(2) pid_t). Every violation names the RECEIVED
/// value and states that nothing was signalled (the T129 honesty pattern;
/// the T130 distinctive-phrase rule). The byte-identical chug_cancel texts
/// are pinned by the T153 tests — the tool name is interpolated, the rest
/// is shared verbatim.
fn parse_pid_argument(tool: &str, args: &Value) -> Result<u64, String> {
    let Some(pid_value) = args.get("pid") else {
        return Err(format!(
            "{tool}: missing required argument: pid (a positive integer — the \
             child pid returned by chug_launch) — nothing signalled"
        ));
    };
    let Some(pid) = pid_value.as_u64() else {
        return Err(format!(
            "{tool}: `pid` must be a positive integer, got {pid_value} — nothing \
             signalled"
        ));
    };
    if pid == 0 {
        return Err(format!(
            "{tool}: `pid` must be a positive integer, got 0 — nothing signalled"
        ));
    }
    if pid > i32::MAX as u64 {
        return Err(format!(
            "{tool}: `pid` must be a valid pid (at most {}), got {pid} — nothing \
             signalled",
            i32::MAX
        ));
    }
    Ok(pid)
}

/// The three fail-closed ownership legs (T153, shared with T157's
/// `chug_abort`): (a) alive, (b) own process-group leader (the delegate
/// detached-spawn fingerprint), (c) the command line names a `chug run`
/// invocation. `Ok(pgid)` = all legs hold, signalling may proceed;
/// `Err(text)` = the FIRST failed leg, an `isError` text naming it — the
/// caller's contract is that nothing was signalled.
///
/// The server is stateless across requests and keeps no launch registry,
/// so "is this pid mine to signal?" is answered fresh from PROCESS IDENTITY
/// on every call, never from server-side state. A wrong cancel/abort is
/// un-undoable; a refused one is retryable.
#[cfg(unix)]
fn ownership_pgid(tool: &str, pid: u64) -> Result<i32, String> {
    let pid_i = pid as i32;
    // Leg (a) — alive: `kill(pid, 0)` through the T28 zombie-reap seam (an
    // exited child of THIS server is reaped first, so a zombie never reads
    // as a live target). ESRCH → the honest "no such process" refusal, not
    // a crash; any other leg shape fails closed too.
    match crate::delegate::reap_and_alive(pid) {
        Some(true) => {}
        Some(false) => {
            return Err(format!(
                "{tool}: pid {pid} is not alive (no such process) — nothing \
                 signalled"
            ));
        }
        None => {
            return Err(format!(
                "{tool}: pid {pid} liveness could not be probed — fail-closed, \
                 nothing signalled"
            ));
        }
    }
    // Leg (b) — the pid is its OWN process-group leader: the delegate
    // detached-spawn fingerprint (`process_group(0)` makes the child the
    // leader, so pgid == pid). A pid that is not a group leader was not
    // launched through the delegate path. Unresolvable → fail closed.
    // SAFETY: getpgid(2) on one bounded pid — a pure query, no side effects.
    let pgid = unsafe { libc::getpgid(pid_i) };
    if pgid < 0 {
        return Err(format!(
            "{tool}: pid {pid} process group could not be resolved — \
             fail-closed, nothing signalled"
        ));
    }
    if pgid != pid_i {
        return Err(format!(
            "{tool}: pid {pid} is not its own process-group leader (pgid {pgid} \
             != pid) — not a delegate-detached child, nothing signalled"
        ));
    }
    // Leg (c) — the command line names a `chug run` invocation, resolved
    // via `ps -o command=` (the driver-lock precedent: ps, NEVER pgrep).
    // Unresolvable → fail closed (skip, name the leg).
    let Some(command) = ps_command_line(pid) else {
        return Err(format!(
            "{tool}: pid {pid} command line could not be resolved — \
             fail-closed, nothing signalled"
        ));
    };
    if !command_names_chug_run(&command) {
        return Err(format!(
            "{tool}: pid {pid} command line is not a `chug run` invocation \
             ({command:?}) — nothing signalled"
        ));
    }
    Ok(pgid)
}

/// The outcome of the shared TERM→grace→KILL group discipline (T153,
/// shared with T157's `chug_abort`).
#[cfg(unix)]
enum GroupSignal {
    /// The group emptied within the [`CHUG_CANCEL_GRACE_MS`] grace.
    Term { waited_ms: u64 },
    /// The grace expired; the SIGKILL escalation fired.
    Kill { waited_ms: u64 },
    /// The grace expired AND the escalation kill(2) itself failed — the
    /// group WAS TERM-signalled; the caller reports the failure honestly.
    KillFailed { err: std::io::Error, waited_ms: u64 },
}

/// The signal discipline shared by `chug_cancel` (T153) and `chug_abort`
/// (T157): SIGTERM to the whole process GROUP (negative-pid kill — the
/// detached tree dies, not just the driver), then ONE bounded escalation —
/// up to [`CHUG_CANCEL_GRACE_MS`] (polled at [`CHUG_CANCEL_POLL_MS`]) for
/// the group to empty, then SIGKILL to the group. `pid` is the group
/// leader (the ownership legs proved pgid == pid) — the T28 reap inside
/// [`group_gone`] needs it.
#[cfg(unix)]
fn term_group_with_escalation(pgid: i32) -> Result<GroupSignal, String> {
    // SAFETY: kill(2) with SIGTERM on a negated, resolved pgid.
    if unsafe { libc::kill(-pgid, libc::SIGTERM) } != 0 {
        let err = std::io::Error::last_os_error();
        return Err(format!(
            "signalling process group {pgid} failed ({err}) — nothing signalled"
        ));
    }
    let started = std::time::Instant::now();
    let deadline = started + std::time::Duration::from_millis(CHUG_CANCEL_GRACE_MS);
    loop {
        if group_gone(pgid, pgid) {
            return Ok(GroupSignal::Term {
                waited_ms: started.elapsed().as_millis() as u64,
            });
        }
        if std::time::Instant::now() >= deadline {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(CHUG_CANCEL_POLL_MS));
    }
    // Still alive after the grace: SIGKILL the group.
    // SAFETY: kill(2) with SIGKILL on a negated, resolved pgid.
    let kill_rc = unsafe { libc::kill(-pgid, libc::SIGKILL) };
    let waited_ms = started.elapsed().as_millis() as u64;
    if kill_rc != 0 {
        return Ok(GroupSignal::KillFailed {
            err: std::io::Error::last_os_error(),
            waited_ms,
        });
    }
    Ok(GroupSignal::Kill { waited_ms })
}

/// The unix ownership + signal legs (T153). Every failure leg is an
/// `isError` text naming the FIRST failed leg and stating that nothing was
/// signalled; the server loop survives every leg (the T129 refusal
/// posture — a tool failure is never a JSON-RPC error, never fatal).
#[cfg(unix)]
fn cancel_unix(pid: u64) -> (String, bool) {
    let pgid = match ownership_pgid(CHUG_CANCEL_TOOL, pid) {
        Ok(pgid) => pgid,
        Err(message) => return (message, true),
    };
    // All legs hold: the shared signal discipline, mapped back into the
    // chug_cancel payload voice.
    match term_group_with_escalation(pgid) {
        Err(failure) => (
            format!("{CHUG_CANCEL_TOOL}: {failure}"),
            true,
        ),
        Ok(GroupSignal::Term { waited_ms }) => (
            format!("{CHUG_CANCEL_TOOL}: pid {pid}\nsignaled: term\nwaited_ms: {waited_ms}"),
            false,
        ),
        Ok(GroupSignal::Kill { waited_ms }) => (
            format!("{CHUG_CANCEL_TOOL}: pid {pid}\nsignaled: kill\nwaited_ms: {waited_ms}"),
            false,
        ),
        Ok(GroupSignal::KillFailed { err, waited_ms }) => (
            format!(
                "{CHUG_CANCEL_TOOL}: pid {pid} survived the \
                 {CHUG_CANCEL_GRACE_MS} ms grace and the escalation kill of \
                 group {pgid} failed ({err}) — the group was TERM-signalled \
                 at {waited_ms} ms"
            ),
            true,
        ),
    }
}

// ---------------------------------------------------------------------------
// T157: the control verbs — chug_abort + chug_steer (--allow-control)
// ---------------------------------------------------------------------------

/// `chug_abort` (T157): the RUN-level counterpart of [`chug_cancel`].
///
/// Same fail-closed ownership re-derivation and the same TERM→grace→KILL
/// group discipline — the differences are the CONTRACT around them:
///
/// 1. **Terminal states, not a signal payload.** The result names what the
///    run's terminal state now is: `aborted` (this call signalled the
///    group and recorded the abort), `already-done` (the child's latest
///    run segment already carries a verdict — goal accepted/rejected or an
///    abort — so the run is over; IDEMPOTENT, `isError: false`, nothing
///    signalled, a retried abort lands here), or `not-found` (the pid is
///    dead with no verdict on record — `isError: true`, nothing
///    signalled, same distinctive phrase `chug_cancel` uses).
/// 2. **The abort is RECORDED.** After the group is verified dying, an
///    `abort` line is appended to the child's `.chug/events.jsonl` (the
///    reason names this verb, [`CHUG_ABORT_REASON`], so a postmortem can
///    tell an MCP abort from the driver's own "operator abort"). This is
///    what makes abort run-level: `chug_status`/`chug_collect` then report
///    the run as aborted instead of a run that reads `running` forever
///    after its process was killed (the exact gap a bare `chug_cancel`
///    leaves). The write is best-effort telemetry: the process IS dead
///    either way, so a record-write failure degrades to `recorded: false`
///    naming the path — never `isError`, never a killed loop.
/// 3. **A verdict wins over liveness.** A latest-segment verdict is
///    checked BEFORE the ownership legs: a run that already ended is
///    reported `already-done` without signalling anything, even if its
///    (usually already-reaped) pid still answers a probe. An unreadable or
///    missing events file degrades to "no verdict" — telemetry failure
///    must never block the operator's stop button.
fn chug_abort(args: &Value) -> (String, bool) {
    // cwd: the SAME fail-fast validator every tool serves — it names the
    // chug cwd the child was launched in AND where the abort record goes.
    let Some(raw_cwd) = args.get("cwd").and_then(Value::as_str) else {
        return (
            format!("{CHUG_ABORT_TOOL}: missing required argument: cwd (an absolute path to a chug working directory)"),
            true,
        );
    };
    let cwd = match validate_chug_cwd(CHUG_ABORT_TOOL, raw_cwd) {
        Ok(cwd) => cwd,
        Err(message) => return (message, true),
    };
    // pid: the shared control-verb input contract.
    let pid = match parse_pid_argument(CHUG_ABORT_TOOL, args) {
        Ok(pid) => pid,
        Err(message) => return (message, true),
    };
    // The verdict leg FIRST (see doc point 3): one bounded tail read, then
    // the delegate summaries over it — the latest segment's terminal
    // verdict decides between already-done and a live abort.
    let events_path = cwd.join(".chug").join("events.jsonl");
    // Unreadable events degrade to "no verdict" — the abort proceeds on
    // process identity alone.
    let lines: Vec<String> = crate::delegate::read_events_tail(&events_path).unwrap_or_default();
    let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
    let segment = crate::delegate::summarize_events(&refs);
    if segment.state() == "done" || segment.state() == "aborted" {
        let verdict = crate::delegate::summarize_collect(&refs).verdict().to_string();
        return (
            format!(
                "{CHUG_ABORT_TOOL}: pid {pid}\nstate: already-done\nverdict: \
                 {verdict}\nnothing signalled"
            ),
            false,
        );
    }
    #[cfg(unix)]
    {
        abort_unix(&cwd, pid, &events_path, refs.is_empty())
    }
    #[cfg(not(unix))]
    {
        let _ = (pid, refs.is_empty());
        (
            format!(
                "{CHUG_ABORT_TOOL}: not supported on this platform (process \
                 groups and signals are unix-only) — nothing signalled"
            ),
            true,
        )
    }
}

/// The unix abort legs: ownership re-derivation → shared signal discipline
/// → the abort record (T157).
#[cfg(unix)]
fn abort_unix(cwd: &Path, pid: u64, events_path: &Path, events_absent: bool) -> (String, bool) {
    let pgid = match ownership_pgid(CHUG_ABORT_TOOL, pid) {
        Ok(pgid) => pgid,
        Err(message) => return (message, true),
    };
    match term_group_with_escalation(pgid) {
        Err(failure) => (format!("{CHUG_ABORT_TOOL}: {failure}"), true),
        Ok(signal) => {
            let (signaled, waited_ms) = match signal {
                GroupSignal::Term { waited_ms } => ("term", waited_ms),
                GroupSignal::Kill { waited_ms } => ("kill", waited_ms),
                GroupSignal::KillFailed { err, waited_ms } => {
                    return (
                        format!(
                            "{CHUG_ABORT_TOOL}: pid {pid} survived the \
                             {CHUG_CANCEL_GRACE_MS} ms grace and the \
                             escalation kill of group {pgid} failed ({err}) — \
                             the group was TERM-signalled at {waited_ms} ms"
                        ),
                        true,
                    );
                }
            };
            // The group was signalled — record the abort so the run's
            // events stream carries the terminal state (the run-level half
            // of the verb). Best-effort: the process is dead either way.
            let (recorded, record_note) = match append_abort_record(cwd) {
                Ok(()) => (true, None),
                Err(why) => (false, Some(why)),
            };
            let mut out = format!(
                "{CHUG_ABORT_TOOL}: pid {pid}\nstate: aborted\nsignaled: \
                 {signaled}\nwaited_ms: {waited_ms}\nrecorded: {recorded}"
            );
            if events_absent {
                // The child never wrote an events stream (a stub/crashed
                // child) — the record CREATED it; say so, so a reader does
                // not expect a run_start line before the abort.
                out.push_str("\nevents_created: true");
            }
            if let Some(why) = record_note {
                out.push_str(&format!("\nrecord_failed: {why}"));
            }
            out.push_str(&format!("\nevents: {}", events_path.display()));
            (out, false)
        }
    }
}

/// The abort record (T157): one `abort` line appended to the child's
/// `.chug/events.jsonl`, the shape the driver's own `abort_exit` writes
/// (type/ts/reason/model) with the distinctive [`CHUG_ABORT_REASON`] and
/// `model: null` (the aborting server does not know the child's model).
/// A single small append-write is line-atomic against the child's own
/// O_APPEND writes.
fn append_abort_record(cwd: &Path) -> Result<(), String> {
    let path = cwd.join(".chug").join("events.jsonl");
    let line = json!({
        "type": "abort",
        "ts": crate::observ::now_rfc3339(),
        "reason": CHUG_ABORT_REASON,
        "model": Value::Null,
    });
    fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .and_then(|mut file| writeln!(file, "{line}"))
        .map_err(|e| format!("{} ({e:#})", path.display()))
}

/// `chug_steer` (T157): inject an operator steering note into a running
/// detached `chug run` through the driver's EXISTING `[operator]`
/// mechanism — no new mechanism. The in-process transport (the TUI chat
/// dock's mpsc channel) is dead for a detached child (its sender was
/// dropped at spawn), so the note goes over the CROSS-PROCESS queue file
/// `.chug/steer.jsonl`: one `{"note": …}` JSON object per line, appended
/// here, drained by the child's [`crate::driver::drive_loop`] at its next
/// iteration boundary and fed through the same `append_steering_notes`
/// path that lands channel notes as `[operator] …` user messages in the
/// transcript (steering stays OUT of events.jsonl by the T10 pin — the
/// note lands in the transcript, which is the "next-iteration context").
///
/// Delivery report: `queued` (the note is durably on the queue the child
/// drains) vs `undeliverable` (`isError: true`, distinctive phrase, and
/// NOTHING written) — the child's latest run segment already has a verdict
/// (`child done`) or the pid is not alive (`child gone`). A note queued
/// for a child that died without a verdict lingers undrained; the queue is
/// the cwd's, so the next driver in that cwd consumes it at its first
/// boundary — the documented boundary of file-based steering.
fn chug_steer(args: &Value) -> (String, bool) {
    let Some(raw_cwd) = args.get("cwd").and_then(Value::as_str) else {
        return (
            format!("{CHUG_STEER_TOOL}: missing required argument: cwd (an absolute path to a chug working directory)"),
            true,
        );
    };
    let cwd = match validate_chug_cwd(CHUG_STEER_TOOL, raw_cwd) {
        Ok(cwd) => cwd,
        Err(message) => return (message, true),
    };
    let pid = match parse_pid_argument(CHUG_STEER_TOOL, args) {
        Ok(pid) => pid,
        Err(message) => return (message, true),
    };
    // The note: required, non-empty after trim, bounded — rejected above
    // the ceiling naming the received length, never clamped (the launch
    // budget rule).
    let Some(note) = args.get("note").and_then(Value::as_str) else {
        return (
            format!(
                "{CHUG_STEER_TOOL}: missing required argument: note (the \
                 operator steering note)"
            ),
            true,
        );
    };
    if note.trim().is_empty() {
        return (
            format!(
                "{CHUG_STEER_TOOL}: note must be a non-empty string after \
                 trim, got {note:?}"
            ),
            true,
        );
    }
    let note_chars = note.chars().count();
    if note_chars > CHUG_STEER_NOTE_MAX_CHARS {
        return (
            format!(
                "{CHUG_STEER_TOOL}: note must be at most \
                 {CHUG_STEER_NOTE_MAX_CHARS} characters, got {note_chars}"
            ),
            true,
        );
    }
    // The done leg: a latest-segment verdict means the run is over —
    // steering a finished run is undeliverable (distinctive phrase), and
    // nothing is written (a stale note must not sit in the queue for the
    // cwd's NEXT driver).
    let events_path = cwd.join(".chug").join("events.jsonl");
    // Unreadable events degrade to "no verdict" — steering proceeds on
    // process identity alone.
    let lines: Vec<String> = crate::delegate::read_events_tail(&events_path).unwrap_or_default();
    let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
    let segment = crate::delegate::summarize_events(&refs);
    if segment.state() == "done" || segment.state() == "aborted" {
        let verdict = crate::delegate::summarize_collect(&refs).verdict().to_string();
        return (
            format!(
                "{CHUG_STEER_TOOL}: pid {pid} already completed its latest \
                 run segment (verdict: {verdict}) — note NOT queued"
            ),
            true,
        );
    }
    // The gone leg: the pid's liveness through the same T28 reap seam the
    // signal verbs serve. Fail-closed on an unresolvable probe.
    #[cfg(unix)]
    {
        match crate::delegate::reap_and_alive(pid) {
            Some(true) => {}
            Some(false) => {
                return (
                    format!(
                        "{CHUG_STEER_TOOL}: pid {pid} is not alive (no such \
                         process) — note NOT queued"
                    ),
                    true,
                );
            }
            None => {
                return (
                    format!(
                        "{CHUG_STEER_TOOL}: pid {pid} liveness could not be \
                         probed — fail-closed, note NOT queued"
                    ),
                    true,
                );
            }
        }
    }
    // Queue it: one JSON line appended (create if absent). A write failure
    // is an isError naming the path — never a silent "queued" lie.
    let queue_path = crate::driver::steering_queue_path(&cwd);
    let line = json!({ "note": note, "ts": crate::observ::now_rfc3339() });
    if let Err(e) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&queue_path)
        .and_then(|mut file| writeln!(file, "{line}"))
    {
        return (
            format!(
                "{CHUG_STEER_TOOL}: queue write failed: {} ({e:#}) — note NOT \
                 queued",
                queue_path.display()
            ),
            true,
        );
    }
    let mut out = format!(
        "{CHUG_STEER_TOOL}: pid {pid}\nqueued: true\nqueue: {}\nnote_chars: \
         {note_chars}",
        queue_path.display()
    );
    // A tail preview (the T115 launch-goal precedent): the note's LAST
    // chars are where truncation and quoting damage cluster.
    let tail = crate::delegate::tail_preview(note, 120);
    out.push_str(&format!("\nnote_tail: {tail}"));
    (out, false)
}

/// Has the process group emptied? The T153 fix-up predicate: ONLY a probe
/// that FAILED with ESRCH means the group emptied — `rc == 0` (a live,
/// signallable member) and every other rc/errno (EPERM, EINVAL, …) mean
/// NOT gone: fail-safe, keep waiting inside the bounded grace (the SIGKILL
/// escalation remains the backstop for a TERM-ignoring fixture).
///
/// The zombie caveat, with the empirical host behavior the T153 forensics
/// pinned: the pre-fix comment claimed "a zombie group leader keeps a bare
/// `kill(-pgid, 0)` green forever" — FALSE on macOS. A group whose SOLE
/// member is THIS server's own unreaped zombie child answers
/// `kill(-pgid, 0)` with **-1/EPERM**, while per-pid `kill(zombie_pid, 0)`
/// on that same zombie answers 0 — the group and pid probes DISAGREE, and
/// the pre-fix shape (`rc != 0` → gone) misread the EPERM as "the group
/// emptied", returning `signaled: term` with the leader an unreaped
/// zombie (the wire happy path's dead-poll timeout, solo-flaky 12/30). So
/// each tick still offers the leader a non-blocking wait FIRST (the T28
/// reap, best-effort: a foreign pid just yields ECHILD) — a zombie child
/// is immediately reapable, so the NEXT tick's reap clears it and the
/// probe turns ESRCH: the loop converges deterministically instead of
/// returning early on a zombie.
#[cfg(unix)]
fn group_gone(pid: i32, pgid: i32) -> bool {
    let mut status: libc::c_int = 0;
    // SAFETY: WNOHANG waitpid on one pid with a valid status pointer — it
    // can only reap a child of THIS process, never blocks, and any other
    // errno is irrelevant to the probe below.
    unsafe { libc::waitpid(pid, &mut status, libc::WNOHANG) };
    // SAFETY: kill(2) with signal 0 on a negated pgid — an existence probe
    // that delivers no signal.
    let krc = unsafe { libc::kill(-pgid, 0) };
    // The errno is only meaningful when the probe FAILED (rc == -1) —
    // reading it beside a successful rc is the stale-errno trap the T153
    // forensics flagged (a stale errno text printed beside a `killleader=0`
    // rc sent the first read of the evidence down the wrong path).
    let errno = if krc == -1 {
        std::io::Error::last_os_error().raw_os_error()
    } else {
        None
    };
    group_gone_rc(krc, errno)
}

/// The probe-predicate seam (T153 fix-up, pure so the predicate-table test
/// can pin it): ONLY a failed probe whose errno is ESRCH means the group
/// emptied. `rc == 0` — a live, signallable member — and every other
/// rc/errno (EPERM: macOS's answer for our own unreaped zombie sole
/// member; EINVAL; an errno-less failure) mean NOT gone.
#[cfg(unix)]
fn group_gone_rc(rc: i32, errno: Option<i32>) -> bool {
    rc == -1 && errno == Some(libc::ESRCH)
}

/// The pid's current command line, via `ps -o command= -p <pid>` — the
/// driver-lock precedent (ps, NEVER pgrep: cycle-24 eval I1 found pgrep
/// persistently missing macOS process trees that ps sees). Any failure leg
/// — ps missing, spawn failure, non-zero exit, empty output — is `None`:
/// the caller fails closed (an unresolvable command line is never judged).
#[cfg(unix)]
fn ps_command_line(pid: u64) -> Option<String> {
    let out = std::process::Command::new("ps")
        .args(["-o", "command=", "-p", &pid.to_string()])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if text.is_empty() {
        return None;
    }
    Some(text)
}

/// Does a resolved command line name a `chug run` invocation? The needle is
/// the ADJACENT token pair `run --spec` — `--spec` is a REQUIRED argument
/// of the `run` subcommand, so EVERY `chug run` invocation (a human's or
/// the delegate launch path's `<binary> run --spec … --goal … --model …`)
/// carries it, in either the `--spec p` and `--spec=p` spellings, at any
/// position (a `sh -c '…' chug run --spec …` wrapper shape matches too).
/// Token adjacency, not substring: `--special` or a path containing "run"
/// must not match.
#[cfg(unix)]
fn command_names_chug_run(command: &str) -> bool {
    let words: Vec<&str> = command.split_whitespace().collect();
    words.windows(2).any(|pair| {
        pair[0] == "run" && (pair[1] == "--spec" || pair[1].starts_with("--spec="))
    })
}

/// The compact, self-describing summary — the MCP-side renderer. Deliberately
/// NOT [`crate::delegate::render_status`]: that text is byte-pinned by the
/// delegate tests (it serves the in-loop `delegate status` tool), while this
/// one is optimized for an observing agent: state first, the iteration
/// ratio on one line, the three flags, the abort reason when present, and
/// the last event for freshness.
fn render_compact_status(cwd: &Path, events_path: &Path, s: &DelegateSummary) -> String {
    let max = match s.max_iters {
        Some(max) => max.to_string(),
        None => "-".to_string(),
    };
    let iter = match s.last_iteration {
        Some(n) => n.to_string(),
        None => "none".to_string(),
    };
    let mut out = format!("{CHUG_STATUS_TOOL}: {}", cwd.display());
    out.push_str(&format!("\nevents: {}", events_path.display()));
    out.push_str(&format!("\nstate: {}", s.state()));
    out.push_str(&format!("\niteration: {iter}/{max}"));
    out.push_str(&format!(
        "\nbudget_low_seen: {}\ngoal_seen: {}\nabort_seen: {}",
        s.budget_low_seen, s.goal_seen, s.abort_seen
    ));
    if let Some(reason) = &s.abort_reason {
        out.push_str(&format!("\nabort_reason: {reason}"));
    }
    match (&s.last_event_type, &s.last_event_ts) {
        (Some(t), Some(ts)) => out.push_str(&format!("\nlast_event: {t} {ts}")),
        (Some(t), None) => out.push_str(&format!("\nlast_event: {t}")),
        (None, _) => {}
    }
    out
}

/// The `chug_collect` compact renderer — the MCP-side shape of delegate's
/// `render_collect` (that text is byte-pinned by the delegate tests and is
/// deliberately NOT reused, the same relationship as
/// [`render_compact_status`] vs `render_status`). Field order mirrors the
/// delegate renderer so the two are shape-comparable: verdict, liveness
/// (ONLY when a pid was given — no pid, no claim), the accepted goal's
/// summary, the check cmd, the commits block (header names the queried
/// range), and the abort reason when the verdict is `aborted`. The two
/// self-describing header lines name the observed cwd and the events file
/// that was read.
fn render_compact_collect(
    cwd: &Path,
    events_path: &Path,
    s: &crate::delegate::CollectSummary,
    alive: Option<bool>,
    base: Option<&str>,
    commits: &Result<Vec<String>, String>,
) -> String {
    let mut out = format!("{CHUG_COLLECT_TOOL}: {}", cwd.display());
    out.push_str(&format!("\nevents: {}", events_path.display()));
    out.push_str(&format!("\nverdict: {}", s.verdict()));
    match alive {
        Some(true) => out.push_str("\nalive: true"),
        Some(false) => out.push_str("\nalive: false"),
        None => {}
    }
    if let Some(text) = &s.goal_summary {
        // The FULL accepted summary, verbatim (it may wrap lines).
        out.push_str("\nsummary: ");
        out.push_str(text);
    }
    if let Some(cmd) = &s.check_cmd {
        out.push_str(&format!("\ncheck_cmd: {cmd}"));
    }
    match commits {
        Ok(lines) if lines.is_empty() => {
            out.push_str(&format!(
                "\ncommits: (none in range {})",
                crate::delegate::commit_range(base)
            ));
        }
        Ok(lines) => {
            out.push_str(&format!(
                "\ncommits (range {}, up to {}):",
                crate::delegate::commit_range(base),
                crate::delegate::DELEGATE_COLLECT_COMMIT_CAP
            ));
            for line in lines {
                out.push_str(&format!("\n  {line}"));
            }
        }
        Err(why) => out.push_str(&format!("\ncommits: (unavailable: {why})")),
    }
    if s.verdict_latch == Some("aborted")
        && let Some(reason) = &s.abort_reason
    {
        out.push_str(&format!("\nabort_reason: {reason}"));
    }
    out
}

// ---------------------------------------------------------------------------
// Response rendering
// ---------------------------------------------------------------------------

fn ok_response(id: Value, result: Value) -> String {
    to_line(&json!({ "jsonrpc": "2.0", "id": id, "result": result }))
}

fn error_response(id: Value, code: i32, message: &str) -> String {
    to_line(&json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message },
    }))
}

/// One response object → one stdout line. The only place a protocol
/// message is serialized; compact separators keep every response on
/// exactly one line.
fn to_line(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| {
        // A Value built here from json! cannot fail to serialize; the
        // fallback keeps the loop alive with a legal error response even
        // if that ever changes.
        "{\"jsonrpc\":\"2.0\",\"id\":null,\"error\":{\"code\":-32700,\"message\":\"Internal serialization failure\"}}"
            .to_string()
    })
}

// T154: the body of this file's `#[cfg(test)] mod tests` (1,168 lines at
// the split, 49 #[test] fns) lives in src/mcp_serve/tests.rs — byte-identical
// move, the T104/T109 shape (driver.rs/delegate.rs keep the same one-line
// declaration; this module needs no pub(crate) seam — no external consumer).
#[cfg(test)]
mod tests;
