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
//! anywhere. T129 adds the ONE write verb, `chug_launch` (F10 phase 2b),
//! and gates it on the `--allow-launch` server flag — the flag is the
//! policy boundary: without it the server is byte-for-byte the read-only
//! one (`chug_launch` is not advertised in `tools/list` and a call for it
//! gets the unknown-tool error), and with it the tool launches a bounded
//! detached `chug run` through the ONE delegate launch path
//! ([`crate::delegate::delegate_launch`] — no second spawner, no new
//! quoting code). The server adds no bypass: the spawned child is an
//! ordinary `chug run` in the target cwd, subject to that cwd's own
//! permissions/hooks/risk-gate chain and its own `.chug/driver.lock`.
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
//! Deferred (F10 phase 3): a server log file, `tools/listChanged`,
//! cancellation, resources/prompts.

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
/// T129: `allow_launch` gates the `chug_launch` write tool. `false` (the
/// default, and every flagless invocation) is byte-identical to the
/// pre-T129 read-only server.
pub(crate) fn serve(allow_launch: bool) -> anyhow::Result<()> {
    // Stdout purity by construction: the ONLY stdout writer in the module
    // is the protocol `writeln!` inside serve_from (see the module doc).
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    serve_from_with(&mut stdin.lock(), &mut out, allow_launch)
}

/// The loop over an injected reader/writer pair, so the EOF and
/// blank-line-skipping behavior is unit-testable without touching real
/// stdio. Serial dispatch (read-only tools are fast; no concurrency in
/// phase 1). The flagless entry the framing test legs ride lives in the
/// test module (`serve_from` there → this with `false`), so the production
/// half carries only the flag-aware loop.
fn serve_from_with(
    read: &mut impl BufRead,
    out: &mut impl Write,
    allow_launch: bool,
) -> anyhow::Result<()> {
    for line in read.lines() {
        let line = line?;
        // Blank lines are framing noise, not messages — skip them.
        if line.trim().is_empty() {
            continue;
        }
        if let Some(response) = handle_message_with(&line, allow_launch) {
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
/// T129: `allow_launch` decides whether `chug_launch` exists — advertised
/// in `tools/list` ⇔ callable in `tools/call`, both fed by this one boolean
/// (capability honesty by construction). The flagless entry the phase-1/2a
/// test legs ride lives in the test module (`handle_message` there → this
/// with `false`), so the production half carries only the flag-aware
/// dispatch.
fn handle_message_with(line: &str, allow_launch: bool) -> Option<String> {
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
        "tools/list" => Some(ok_response(id, json!({ "tools": tools_list(allow_launch) }))),
        "tools/call" => Some(call_tool(id, obj, allow_launch)),
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

/// The `tools/list` tool set. Capability honesty (T129): `chug_launch` is
/// advertised ONLY when the server was started with `--allow-launch` — the
/// same boolean feeds `tools/call`, so advertised ⇔ callable by
/// construction.
fn tools_list(allow_launch: bool) -> Vec<Value> {
    let mut tools = vec![chug_status_schema(), chug_collect_schema()];
    if allow_launch {
        tools.push(chug_launch_schema());
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

/// Dispatch `tools/call`. A known tool's OWN failure (bad cwd, unreadable
/// events, refused launch) is a tool RESULT with `isError: true` — not a
/// JSON-RPC error — so the caller sees the tool ran and failed, the shape
/// the client side already parses (`isError` + text content).
///
/// T129: `chug_launch` is routable only under the flag; without
/// `allow_launch` its name falls through to the unknown-tool arm — the
/// SAME `-32602` a never-existing tool gets, so a read-only deployment
/// cannot be probed into revealing that a launch tool exists behind a
/// flag (and no error kills the loop).
fn call_tool(id: Value, req: &Map<String, Value>, allow_launch: bool) -> String {
    let params = req.get("params").cloned().unwrap_or(Value::Null);
    let Some(name) = params.get("name").and_then(Value::as_str) else {
        return error_response(id, -32602, "Invalid params: tools/call requires a tool name");
    };
    let arguments = params.get("arguments").cloned().unwrap_or(json!({}));
    let (text, is_error) = match name {
        CHUG_STATUS_TOOL => chug_status(&arguments),
        CHUG_COLLECT_TOOL => chug_collect(&arguments),
        CHUG_LAUNCH_TOOL if allow_launch => chug_launch(&arguments),
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

// ---------------------------------------------------------------------------
// Tests — bin-internal, pure handle_message legs (no real stdio)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// The flagless dispatch — the pre-T129 entry, byte-identical read-only
    /// behavior, and the call site every phase-1/2a leg already rides
    /// (unchanged by T129). The flag-ON legs call
    /// [`handle_message_with`](super::handle_message_with) directly.
    fn handle_message(line: &str) -> Option<String> {
        super::handle_message_with(line, false)
    }

    /// The flagless serve loop — the pre-T129 entry for the framing legs.
    fn serve_from(read: &mut impl BufRead, out: &mut impl Write) -> anyhow::Result<()> {
        super::serve_from_with(read, out, false)
    }

    /// Parse a response line into its envelope parts.
    fn parts(line: &str) -> (Value, Option<Value>, Option<Value>, Option<Value>) {
        let v: Value = serde_json::from_str(line).expect("response is valid JSON");
        assert_eq!(v["jsonrpc"], "2.0", "envelope jsonrpc: {line}");
        (
            v.clone(),
            v.get("id").cloned(),
            v.get("result").cloned(),
            v.get("error").cloned(),
        )
    }

    /// The `result` of a tools/call response, as (text, isError).
    fn tool_result(line: &str) -> (String, Option<bool>) {
        let (_, _, result, _) = parts(line);
        let result = result.expect("tools/call returns a result, not an error");
        let text = result["content"][0]["text"].as_str().expect("text block").to_string();
        assert_eq!(result["content"][0]["type"], "text", "content type");
        (text, result.get("isError").and_then(Value::as_bool))
    }

    // ---------- handshake ----------

    #[test]
    fn initialize_shape_protocol_version_serverinfo_and_capabilities() {
        let line = handle_message(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#)
            .expect("initialize responds");
        let (_, id, result, error) = parts(&line);
        assert_eq!(id, Some(json!(1)), "response id echoes the request id");
        assert!(error.is_none(), "no error: {line}");
        let result = result.expect("initialize result");
        // The version literal is REUSED from the client module, not duplicated.
        assert_eq!(result["protocolVersion"], crate::mcp::PROTOCOL_VERSION);
        assert_eq!(result["protocolVersion"], "2025-06-18");
        assert!(
            result["capabilities"].get("tools").is_some(),
            "capabilities.tools present: {result}"
        );
        assert_eq!(result["serverInfo"]["name"], "chug");
        assert_eq!(result["serverInfo"]["version"], crate::build_info::VERSION);
    }

    #[test]
    fn initialize_echoes_string_and_null_ids_verbatim() {
        for id in [json!("abc"), json!(null), json!(42)] {
            let req = json!({"jsonrpc":"2.0","id":id,"method":"initialize"}).to_string();
            let line = handle_message(&req).expect("responds");
            let (_, echoed, _, _) = parts(&line);
            assert_eq!(echoed, Some(id), "id echoed verbatim");
        }
    }

    #[test]
    fn ping_replies_with_empty_result() {
        let line = handle_message(r#"{"jsonrpc":"2.0","id":7,"method":"ping"}"#)
            .expect("ping responds");
        let (_, id, result, error) = parts(&line);
        assert_eq!(id, Some(json!(7)));
        assert!(error.is_none(), "{line}");
        assert_eq!(result, Some(json!({})));
    }

    #[test]
    fn notifications_get_no_response() {
        // The handshake notification…
        assert_eq!(
            handle_message(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#),
            None
        );
        // …an arbitrary unknown notification…
        assert_eq!(
            handle_message(r#"{"jsonrpc":"2.0","method":"notifications/foo","params":{}}"#),
            None
        );
        // …and a notifications/* method that illegally carries an id — the
        // prefix means the client does not want a reply, so none comes.
        assert_eq!(
            handle_message(r#"{"jsonrpc":"2.0","id":3,"method":"notifications/initialized"}"#),
            None
        );
    }

    // ---------- tools/list ----------

    #[test]
    fn tools_list_exposes_both_read_only_tools_requiring_cwd() {
        let line = handle_message(r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#)
            .expect("tools/list responds");
        let (_, id, result, _) = parts(&line);
        assert_eq!(id, Some(json!(2)));
        let tools = result.expect("result")["tools"]
            .as_array()
            .expect("tools array")
            .clone();
        assert_eq!(tools.len(), 2, "phases 1+2a ship exactly two tools");
        assert_eq!(tools[0]["name"], "chug_status");
        assert_eq!(tools[1]["name"], "chug_collect");
        for tool in &tools {
            assert_eq!(tool["inputSchema"]["type"], "object");
            let required: Vec<&str> = tool["inputSchema"]["required"]
                .as_array()
                .expect("required array")
                .iter()
                .filter_map(Value::as_str)
                .collect();
            assert!(
                required.contains(&"cwd"),
                "inputSchema requires cwd: {tools:?}"
            );
        }
        // T128: `chug_collect` advertises its three params — cwd required,
        // pid + base optional (properties, NOT required).
        let collect = &tools[1];
        let props = collect["inputSchema"]["properties"]
            .as_object()
            .expect("properties object");
        assert_eq!(props.len(), 3, "cwd + pid + base: {props:?}");
        assert_eq!(props["pid"]["type"], "integer", "{props:?}");
        assert_eq!(props["base"]["type"], "string", "{props:?}");
        assert_eq!(collect["inputSchema"]["required"], json!(["cwd"]));
    }

    // ---------- error taxonomy ----------

    #[test]
    fn unparseable_line_is_parse_error_with_null_id() {
        for garbage in ["{ not json !!!", "", "   ", "[1,2,"] {
            // Blank lines are skipped at the serve layer; here a blank line
            // still must not panic (it parses as no JSON → -32700).
            let line = handle_message(garbage).expect("parse error responds");
            let (_, id, _, error) = parts(&line);
            assert_eq!(id, Some(Value::Null), "id null on parse error: {line}");
            assert_eq!(error.expect("error object")["code"], -32700, "{line}");
        }
    }

    #[test]
    fn unknown_method_on_a_request_is_method_not_found() {
        let line =
            handle_message(r#"{"jsonrpc":"2.0","id":9,"method":"resources/list"}"#)
                .expect("responds");
        let (_, id, _, error) = parts(&line);
        assert_eq!(id, Some(json!(9)));
        let error = error.expect("error object");
        assert_eq!(error["code"], -32601, "{line}");
        assert!(error["message"].as_str().unwrap().contains("resources/list"));
    }

    #[test]
    fn request_missing_method_is_invalid_request() {
        let line = handle_message(r#"{"jsonrpc":"2.0","id":4,"params":{}}"#).expect("responds");
        let (_, id, _, error) = parts(&line);
        assert_eq!(id, Some(json!(4)));
        assert_eq!(error.expect("error object")["code"], -32600, "{line}");
    }

    #[test]
    fn methodless_idless_object_gets_no_response() {
        // Without a method AND without an id there is nothing to route and
        // nobody to reply to — silence, not a broadcast error.
        assert_eq!(handle_message(r#"{"jsonrpc":"2.0"}"#), None);
    }

    #[test]
    fn non_object_json_is_invalid_request_with_null_id() {
        for not_a_request in ["5", "\"hi\"", "[1,2,3]", "true"] {
            let line = handle_message(not_a_request).expect("responds");
            let (_, id, _, error) = parts(&line);
            assert_eq!(id, Some(Value::Null));
            assert_eq!(error.expect("error object")["code"], -32600, "{line}");
        }
    }

    #[test]
    fn tools_call_unknown_tool_is_invalid_params() {
        let line = handle_message(
            r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"bash","arguments":{}}}"#,
        )
        .expect("responds");
        let (_, id, _, error) = parts(&line);
        assert_eq!(id, Some(json!(5)));
        let error = error.expect("error object");
        assert_eq!(error["code"], -32602, "{line}");
        assert!(error["message"].as_str().unwrap().contains("bash"));
    }

    #[test]
    fn tools_call_missing_name_is_invalid_params() {
        let line = handle_message(
            r#"{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{}}"#,
        )
        .expect("responds");
        let (_, _, _, error) = parts(&line);
        assert_eq!(error.expect("error object")["code"], -32602, "{line}");
    }

    // ---------- chug_status ----------

    /// A synthetic latest-segment events fixture: run_start + a few
    /// iterations + a goal line.
    fn write_fixture(cwd: &Path, lines: &[&str]) {
        std::fs::create_dir_all(cwd.join(".chug")).unwrap();
        let body: String = lines.iter().map(|l| format!("{l}\n")).collect();
        std::fs::write(cwd.join(".chug/events.jsonl"), body).unwrap();
    }

    /// Borrow a `Vec<String>` fixture as the `&[&str]` `write_fixture`
    /// takes (the same ref-map the `read_events` seam does).
    fn as_str_refs(lines: &[String]) -> Vec<&str> {
        lines.iter().map(String::as_str).collect()
    }

    fn run_then_iterations_then_goal() -> Vec<String> {
        [
            r#"{"type":"run_start","ts":"t0","mode":"run","model":"m","max_iters":50,"max_minutes":35,"max_tokens":null}"#,
            r#"{"type":"iteration","ts":"t1","n":3,"input_tokens":1,"output_tokens":1}"#,
            r#"{"type":"iteration","ts":"t2","n":4,"input_tokens":1,"output_tokens":1}"#,
            r#"{"type":"tool_result","ts":"t2b","tool":"bash","ok":true}"#,
            r#"{"type":"goal","ts":"t3","outcome":"accepted","summary":"did the thing"}"#,
        ]
        .into_iter()
        .map(str::to_string)
        .collect()
    }

    #[test]
    fn chug_status_happy_path_names_state_and_iteration_counts() {
        let tmp = tempfile::tempdir().unwrap();
        write_fixture(tmp.path(), &as_str_refs(&run_then_iterations_then_goal()));
        let (text, is_error) = chug_status(&json!({ "cwd": tmp.path().display().to_string() }));
        assert!(!is_error, "{text}");
        // State + iteration counts present and correct.
        assert!(text.contains("state: done"), "{text}");
        assert!(text.contains("iteration: 4/50"), "{text}");
        assert!(text.contains("goal_seen: true"), "{text}");
        assert!(text.contains("abort_seen: false"), "{text}");
        assert!(text.contains("budget_low_seen: false"), "{text}");
        // Self-describing: names the cwd and the events file it read.
        assert!(text.contains(&tmp.path().display().to_string()), "{text}");
        assert!(text.contains(".chug/events.jsonl"), "{text}");
    }

    #[test]
    fn chug_status_renders_abort_reason_and_budget_low_flags() {
        let tmp = tempfile::tempdir().unwrap();
        write_fixture(
            tmp.path(),
            &[
                r#"{"type":"run_start","ts":"t0","max_iters":10}"#,
                r#"{"type":"iteration","ts":"t1","n":9}"#,
                r#"{"type":"budget_low","ts":"t2","budget_kind":"iterations","budget_max":10}"#,
                r#"{"type":"abort","ts":"t3","reason":"iteration budget exhausted","model":"m"}"#,
            ],
        );
        let (text, is_error) = chug_status(&json!({ "cwd": tmp.path().display().to_string() }));
        assert!(!is_error, "{text}");
        assert!(text.contains("state: aborted"), "{text}");
        assert!(text.contains("iteration: 9/10"), "{text}");
        assert!(text.contains("budget_low_seen: true"), "{text}");
        assert!(text.contains("abort_seen: true"), "{text}");
        assert!(text.contains("abort_reason: iteration budget exhausted"), "{text}");
    }

    #[test]
    fn chug_status_segment_reset_reports_the_latest_segment() {
        // T58 semantics ride the shared seam: a resumed child's fresh
        // run_start resets the verdict latches — the summary describes the
        // LATEST segment, not the pre-resume abort.
        let tmp = tempfile::tempdir().unwrap();
        let lines: Vec<String> = [
            r#"{"type":"run_start","ts":"t0","max_iters":40}"#,
            r#"{"type":"abort","ts":"t1","reason":"old death"}"#,
            r#"{"type":"run_start","ts":"t2","max_iters":20}"#,
            r#"{"type":"iteration","ts":"t3","n":1}"#,
        ]
        .into_iter()
        .map(str::to_string)
        .collect();
        write_fixture(tmp.path(), &as_str_refs(&lines));
        let (text, is_error) = chug_status(&json!({ "cwd": tmp.path().display().to_string() }));
        assert!(!is_error, "{text}");
        assert!(text.contains("state: running"), "{text}");
        assert!(text.contains("iteration: 1/20"), "{text}");
        assert!(text.contains("abort_seen: false"), "{text}");
        assert!(!text.contains("old death"), "{text}");
    }

    #[test]
    fn chug_status_with_no_events_seen_reports_starting_state() {
        // A `.chug/events.jsonl` that exists but is empty: a successful read
        // of nothing — not an error.
        let tmp = tempfile::tempdir().unwrap();
        write_fixture(tmp.path(), &[]);
        let (text, is_error) = chug_status(&json!({ "cwd": tmp.path().display().to_string() }));
        assert!(!is_error, "{text}");
        assert!(text.contains("state: starting"), "{text}");
        assert!(text.contains("iteration: none/-"), "{text}");
    }

    #[test]
    fn chug_status_missing_chug_dir_names_received_path() {
        let tmp = tempfile::tempdir().unwrap();
        let (text, is_error) = chug_status(&json!({ "cwd": tmp.path().display().to_string() }));
        assert!(is_error, "{text}");
        assert!(
            text.contains(&tmp.path().display().to_string()),
            "names the received path verbatim: {text}"
        );
        assert!(text.contains(".chug"), "{text}");
        // T130 (the T124 M3 survivor): the three assertions above are ALL
        // substring-satisfied by the downstream events-unreadable message
        // (it embeds `<tmp>/.chug/events.jsonl`, is an error, and contains
        // the literal `.chug`), so deleting the `.chug/`-existence check
        // used to survive. The DISTINCTIVE phrase of this leg's own
        // message — plus the negative guard against the shadow message —
        // makes the mutant die.
        assert!(text.contains("no .chug/ directory in"), "{text}");
        assert!(
            !text.contains("events file unreadable"),
            "must be the missing-dir leg, not the shadowing events-unreadable one: {text}"
        );
    }

    #[test]
    fn chug_status_missing_events_file_names_the_path() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".chug")).unwrap();
        let (text, is_error) = chug_status(&json!({ "cwd": tmp.path().display().to_string() }));
        assert!(is_error, "{text}");
        let events = tmp.path().join(".chug/events.jsonl");
        assert!(
            text.contains(events.display().to_string().as_str()),
            "names the events path: {text}"
        );
        // T130 sweep: the DISTINCTIVE phrase of this leg's own message (a
        // reworded message used to survive — the path assertion alone is
        // message-text-blind), plus the sibling-shadow guard: `.chug/`
        // EXISTS in this fixture, so the error must be the events one.
        assert!(text.contains("events file unreadable"), "{text}");
        assert!(
            !text.contains("no .chug/ directory in"),
            "must be the events-unreadable leg, not the shadowing missing-dir one: {text}"
        );
    }

    #[test]
    fn chug_status_relative_cwd_is_refused_naming_the_received_string() {
        let (text, is_error) = chug_status(&json!({ "cwd": "some/relative/path" }));
        assert!(is_error, "{text}");
        // The delegate-launch fail-fast shape: the RAW received string,
        // quoted, not a silently-resolved path.
        assert!(text.contains("\"some/relative/path\""), "{text}");
        // T130 sweep: the DISTINCTIVE phrase of this leg's own message
        // (a reworded message used to survive — the quoted-raw assertion
        // is message-text-blind), plus the fall-through shadow guard:
        // with the absolute-check deleted the nonexistent-dir leg fires,
        // and its display() output is unquoted.
        assert!(text.contains("must be an absolute directory"), "{text}");
        assert!(
            !text.contains("does not exist or is not a directory"),
            "must be the relative-cwd leg, not the fall-through nonexistent-dir shadow: {text}"
        );
    }

    #[test]
    fn chug_status_nonexistent_cwd_is_refused_naming_the_path() {
        let bogus = "/definitely/not/a/chug/cwd-t124";
        let (text, is_error) = chug_status(&json!({ "cwd": bogus }));
        assert!(is_error, "{text}");
        assert!(text.contains(bogus), "{text}");
        // T130 sweep (the M3 shadow class again): the no-`.chug/` message
        // for the SAME bogus path also contains it verbatim, so deleting
        // the is_dir() check used to survive on the path assertion alone.
        assert!(
            text.contains("does not exist or is not a directory"),
            "{text}"
        );
        assert!(
            !text.contains("no .chug/ directory in"),
            "must be the nonexistent-cwd leg, not the shadowing missing-dir one: {text}"
        );
    }

    #[test]
    fn chug_status_missing_cwd_argument_is_an_error_result() {
        let (text, is_error) = chug_status(&json!({}));
        assert!(is_error, "{text}");
        assert!(text.contains("cwd"), "{text}");
        // T130 sweep: "cwd" alone is substring-satisfied by the
        // relative-cwd message a neutered guard falls through to — pin
        // the DISTINCTIVE phrase and guard against that shadow.
        assert!(text.contains("missing required argument: cwd"), "{text}");
        assert!(
            !text.contains("must be an absolute directory"),
            "must be the missing-argument leg, not the fall-through relative-cwd shadow: {text}"
        );
        // And via the full protocol path it is a tool RESULT, not a
        // JSON-RPC error (the tool ran; its input was bad).
        let line = handle_message(
            r#"{"jsonrpc":"2.0","id":8,"method":"tools/call","params":{"name":"chug_status","arguments":{}}}"#,
        )
        .expect("responds");
        let (_, _, _, error) = parts(&line);
        assert!(error.is_none(), "tool failure is a result, not a JSON-RPC error: {line}");
        let (text2, is_error2) = tool_result(&line);
        assert_eq!(is_error2, Some(true), "{text2}");
    }

    #[test]
    fn chug_status_non_string_cwd_is_an_error_result() {
        let (text, is_error) = chug_status(&json!({ "cwd": 17 }));
        assert!(is_error, "{text}");
        assert!(text.contains("cwd"), "{text}");
        // T130 sweep: same guard as the missing-argument leg (as_str()
        // yields None either way) — pin its DISTINCTIVE phrase and guard
        // against the same relative-cwd fall-through shadow.
        assert!(text.contains("missing required argument: cwd"), "{text}");
        assert!(
            !text.contains("must be an absolute directory"),
            "must be the missing-argument leg, not the fall-through relative-cwd shadow: {text}"
        );
    }

    #[test]
    fn chug_status_malformed_final_line_degrades_best_effort_without_panicking() {
        // The torn-final-write case: the good lines still summarize; the
        // garbage line is skipped (the shared summarize_events contract).
        let tmp = tempfile::tempdir().unwrap();
        let mut body = String::new();
        for line in run_then_iterations_then_goal() {
            body.push_str(&line);
            body.push('\n');
        }
        body.push_str("{\"type\":\"iteration\",\"ts\":\"t4\",\"n\":"); // torn
        std::fs::create_dir_all(tmp.path().join(".chug")).unwrap();
        std::fs::write(tmp.path().join(".chug/events.jsonl"), body).unwrap();
        let (text, is_error) = chug_status(&json!({ "cwd": tmp.path().display().to_string() }));
        assert!(!is_error, "best-effort summary, not an error: {text}");
        assert!(text.contains("state: done"), "{text}");
        assert!(text.contains("iteration: 4/50"), "{text}");
    }

    // ---------- chug_collect (T128) ----------

    /// A synthetic latest-segment events fixture in the collect shape:
    /// run_start + a verifying gate + an accepted goal.
    fn collect_fixture(cwd: &Path) {
        write_fixture(
            cwd,
            &[
                r#"{"type":"run_start","ts":"t0","mode":"run","model":"m","max_iters":50}"#,
                r#"{"type":"verifying","ts":"t2","cmd":"cargo test"}"#,
                r#"{"type":"goal","ts":"t3","outcome":"accepted","summary":"did the thing, verified"}"#,
            ],
        );
    }

    #[test]
    fn chug_collect_happy_path_names_verdict_summary_check_cmd_and_degraded_commits() {
        let tmp = tempfile::tempdir().unwrap();
        collect_fixture(tmp.path());
        let (text, is_error) = chug_collect(&json!({ "cwd": tmp.path().display().to_string() }));
        assert!(!is_error, "{text}");
        assert!(text.contains("verdict: goal-accepted"), "{text}");
        assert!(text.contains("summary: did the thing, verified"), "{text}");
        assert!(text.contains("check_cmd: cargo test"), "{text}");
        // Self-describing: names the cwd and the events file it read.
        assert!(text.contains(&tmp.path().display().to_string()), "{text}");
        assert!(text.contains(".chug/events.jsonl"), "{text}");
        // Not-a-repo tempdir: the git leg DEGRADES to a note, never fails
        // the call (the delegate collect rule, mirrored by this renderer).
        assert!(text.contains("commits: (unavailable:"), "{text}");
    }

    #[test]
    fn chug_collect_aborted_segment_renders_verdict_with_reason() {
        let tmp = tempfile::tempdir().unwrap();
        write_fixture(
            tmp.path(),
            &[
                r#"{"type":"run_start","ts":"t0","max_iters":10}"#,
                r#"{"type":"iteration","ts":"t1","n":9}"#,
                r#"{"type":"abort","ts":"t2","reason":"iteration budget exhausted","model":"m"}"#,
            ],
        );
        let (text, is_error) = chug_collect(&json!({ "cwd": tmp.path().display().to_string() }));
        assert!(!is_error, "{text}");
        assert!(text.contains("verdict: aborted"), "{text}");
        assert!(text.contains("abort_reason: iteration budget exhausted"), "{text}");
        // No accepted verdict → no summary and no check_cmd lines.
        assert!(!text.contains("summary:"), "{text}");
        assert!(!text.contains("check_cmd:"), "{text}");
    }

    #[test]
    fn chug_collect_missing_events_file_names_the_path() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".chug")).unwrap();
        let (text, is_error) = chug_collect(&json!({ "cwd": tmp.path().display().to_string() }));
        assert!(is_error, "{text}");
        let events = tmp.path().join(".chug/events.jsonl");
        assert!(
            text.contains(events.display().to_string().as_str()),
            "names the events path: {text}"
        );
        assert!(text.contains("events file unreadable"), "{text}");
    }

    #[test]
    fn chug_collect_relative_cwd_is_refused_naming_the_received_string() {
        // One leg per violation class on the SHARED validator (the
        // nonexistent-cwd class is pinned chug_status-side; both tools run
        // the same validator, so together the matrices cover it).
        let (text, is_error) = chug_collect(&json!({ "cwd": "some/relative/path" }));
        assert!(is_error, "{text}");
        assert!(text.contains("\"some/relative/path\""), "{text}");
        assert!(text.contains("must be an absolute directory"), "{text}");
    }

    #[test]
    fn chug_collect_missing_chug_dir_is_refused_naming_the_path() {
        let tmp = tempfile::tempdir().unwrap();
        let (text, is_error) = chug_collect(&json!({ "cwd": tmp.path().display().to_string() }));
        assert!(is_error, "{text}");
        assert!(text.contains(&tmp.path().display().to_string()), "{text}");
        assert!(text.contains("no .chug/ directory in"), "{text}");
    }

    #[test]
    fn chug_collect_missing_cwd_argument_is_an_error_result() {
        let (text, is_error) = chug_collect(&json!({}));
        assert!(is_error, "{text}");
        assert!(text.contains("missing required argument: cwd"), "{text}");
        assert!(text.contains(CHUG_COLLECT_TOOL), "{text}");
    }

    #[test]
    fn chug_collect_non_string_base_is_an_error_result() {
        // The T69 rule on the new tool: a caller that asked for a range must
        // not silently get the default range.
        let tmp = tempfile::tempdir().unwrap();
        collect_fixture(tmp.path());
        let (text, is_error) = chug_collect(&json!({
            "cwd": tmp.path().display().to_string(), "base": 17
        }));
        assert!(is_error, "{text}");
        assert!(text.contains("`base` must be a string git ref"), "{text}");
        assert!(!text.contains("commits"), "no silent default-range fallback: {text}");
    }

    #[test]
    fn chug_collect_pid_renders_liveness_line_absent_pid_renders_none() {
        let tmp = tempfile::tempdir().unwrap();
        collect_fixture(tmp.path());
        // Absent pid → NO liveness claim at all (the delegate collect rule).
        let (text, is_error) = chug_collect(&json!({ "cwd": tmp.path().display().to_string() }));
        assert!(!is_error, "{text}");
        assert!(!text.contains("alive:"), "{text}");
        // A pid that cannot exist (macOS caps pids far below this; Linux's
        // pid_max caps at 2^22) probes dead — the liveness line renders.
        let (with_pid, is_error) = chug_collect(&json!({
            "cwd": tmp.path().display().to_string(), "pid": 2_000_000_000u64
        }));
        assert!(!is_error, "{with_pid}");
        assert!(with_pid.contains("alive: false"), "{with_pid}");
    }

    #[test]
    fn chug_collect_base_is_forwarded_into_the_commit_range_seam() {
        // A real git fixture: the distinguishing observable is WHICH commits
        // the spawn returned — `<first>..HEAD` lists only the second, so a
        // mutant that dropped the forward (and silently queried the default
        // range) lists both and dies. The git plumbing itself stays pinned
        // delegate-side (delegate_collect_git_legs_refs_base_and_degrades).
        let tmp = tempfile::tempdir().unwrap();
        let git = |args: &[&str]| {
            let out = std::process::Command::new("git")
                .args(args)
                .current_dir(tmp.path())
                .output()
                .expect("git available for the integration pin");
            assert!(
                out.status.success(),
                "git {args:?} failed: {}",
                String::from_utf8_lossy(&out.stderr)
            );
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        };
        git(&["init", "-q"]);
        git(&["-c", "user.email=t@t", "-c", "user.name=t", "commit", "--allow-empty", "-qm", "first"]);
        let first = git(&["rev-parse", "--short", "HEAD"]);
        git(&["-c", "user.email=t@t", "-c", "user.name=t", "commit", "--allow-empty", "-qm", "second"]);
        collect_fixture(tmp.path());

        // Absent base: the bounded default range over HEAD lists both.
        let (text, is_error) = chug_collect(&json!({ "cwd": tmp.path().display().to_string() }));
        assert!(!is_error, "{text}");
        assert!(text.contains("commits (range HEAD, up to 20):"), "{text}");
        assert!(text.contains(" second"), "{text}");
        assert!(text.contains(" first"), "{text}");

        // base = the first commit: `<first>..HEAD` lists ONLY the second —
        // the forwarded range reached the real git spawn.
        let (scoped, is_error) = chug_collect(&json!({
            "cwd": tmp.path().display().to_string(), "base": first
        }));
        assert!(!is_error, "{scoped}");
        assert!(
            scoped.contains(&format!("commits (range {first}..HEAD, up to 20):")),
            "{scoped}"
        );
        assert!(scoped.contains(" second"), "{scoped}");
        assert!(
            !scoped.contains(&format!("{first} first")),
            "the first commit is outside the forwarded range: {scoped}"
        );
    }

    // ---------- chug_launch (T129) ----------

    /// Route a `chug_launch` tools/call through the FULL dispatch with the
    /// given flag, returning (text, isError) — the same shape the wire
    /// serves. A routed call must always be a tool RESULT (a JSON-RPC error
    /// would mean the tool was not routable at all).
    fn launch_call(allow_launch: bool, arguments: &Value) -> (String, Option<bool>) {
        let req = json!({
            "jsonrpc": "2.0", "id": 129, "method": "tools/call",
            "params": {"name": "chug_launch", "arguments": arguments}
        });
        let line = handle_message_with(&req.to_string(), allow_launch).expect("responds");
        let (_, id, _, error) = parts(&line);
        assert_eq!(id, Some(json!(129)), "id echoed: {line}");
        assert!(
            error.is_none(),
            "a routed launch call is a tool result, not a JSON-RPC error: {line}"
        );
        tool_result(&line)
    }

    /// A real spec file in `dir` (the launch spec probe needs one).
    fn write_launch_spec(dir: &Path) -> PathBuf {
        let spec = dir.join("t129-spec.md");
        fs::write(&spec, "# t129 launch-matrix spec\n").unwrap();
        spec
    }

    /// The launch-matrix payload with every required param valid.
    fn full_launch_args(cwd: &Path, spec: &Path) -> Value {
        json!({
            "cwd": cwd.display().to_string(),
            "spec": spec.display().to_string(),
            "goal": "t129 matrix goal",
            "model": "matrix-model",
        })
    }

    #[test]
    fn chug_launch_flagless_call_is_the_unknown_tool_error() {
        // Without the flag the write leg DOES NOT EXIST: the unknown-tool
        // error, the same -32602 a never-existing tool gets — a read-only
        // deployment cannot be probed into revealing a hidden tool.
        let line = handle_message(
            r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"chug_launch","arguments":{}}}"#,
        )
        .expect("responds");
        let (_, id, _, error) = parts(&line);
        assert_eq!(id, Some(json!(5)), "{line}");
        let error = error.expect("error object");
        assert_eq!(error["code"], -32602, "{line}");
        let message = error["message"].as_str().unwrap();
        assert!(message.contains("chug_launch"), "{line}");
        assert!(message.contains("unknown tool"), "{line}");
        // And the flagless tools/list does not advertise it — the read-only
        // tool set is byte-identical to pre-T129.
        let list = handle_message(r#"{"jsonrpc":"2.0","id":6,"method":"tools/list"}"#)
            .expect("responds");
        let (_, _, result, _) = parts(&list);
        let tools = result.expect("result")["tools"]
            .as_array()
            .expect("tools array")
            .clone();
        let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
        assert_eq!(names, ["chug_status", "chug_collect"], "{list}");
    }

    #[test]
    fn chug_launch_flag_on_advertised_with_the_exact_schema() {
        let line = handle_message_with(r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#, true)
            .expect("responds");
        let (_, _, result, _) = parts(&line);
        let tools = result.expect("result")["tools"]
            .as_array()
            .expect("tools array")
            .clone();
        assert_eq!(tools.len(), 3, "two read-only tools + the write leg: {tools:?}");
        assert_eq!(tools[0]["name"], "chug_status");
        assert_eq!(tools[1]["name"], "chug_collect");
        assert_eq!(tools[2]["name"], "chug_launch");
        let schema = &tools[2]["inputSchema"];
        // The required list is EXACT: cwd, spec, goal, model.
        assert_eq!(schema["required"], json!(["cwd", "spec", "goal", "model"]));
        let props = schema["properties"].as_object().expect("properties object");
        assert_eq!(props.len(), 6, "cwd + spec + goal + model + two budgets: {props:?}");
        for key in ["cwd", "spec", "goal", "model"] {
            assert_eq!(props[key]["type"], "string", "{props:?}");
        }
        // The loopd ceilings are IN the schema: 1..=200 and 1..=240.
        assert_eq!(props["max_iters"]["type"], "integer", "{props:?}");
        assert_eq!(props["max_iters"]["minimum"], 1, "{props:?}");
        assert_eq!(props["max_iters"]["maximum"], 200, "{props:?}");
        assert_eq!(props["max_minutes"]["type"], "integer", "{props:?}");
        assert_eq!(props["max_minutes"]["minimum"], 1, "{props:?}");
        assert_eq!(props["max_minutes"]["maximum"], 240, "{props:?}");
    }

    #[test]
    fn chug_launch_flag_on_dispatch_is_a_tool_result_not_unknown_tool() {
        // Under the flag the tool is ROUTABLE: a validation failure is an
        // isError RESULT (the tool ran; the input was bad) — the same call
        // without the flag is the -32602 pinned above. Advertised ⇔
        // callable, both directions.
        let (text, is_error) = launch_call(true, &json!({ "cwd": "relative/cwd" }));
        assert_eq!(is_error, Some(true), "{text}");
    }

    #[test]
    fn chug_launch_missing_required_arguments_are_is_error_results() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".chug")).unwrap();
        let spec = write_launch_spec(tmp.path());
        // Each required param absent → an isError naming the argument.
        for (args, key) in [
            (json!({}), "cwd"),
            (json!({ "cwd": tmp.path() }), "spec"),
            (
                json!({ "cwd": tmp.path(), "spec": spec.display().to_string() }),
                "goal",
            ),
            (
                json!({
                    "cwd": tmp.path(),
                    "spec": spec.display().to_string(),
                    "goal": "g"
                }),
                "model",
            ),
        ] {
            let (text, is_error) = launch_call(true, &args);
            assert_eq!(is_error, Some(true), "{text}");
            assert!(
                text.contains(&format!("missing required argument: {key}")),
                "{key}: {text}"
            );
        }
    }

    #[test]
    fn chug_launch_relative_cwd_is_refused_naming_the_received_string() {
        let tmp = tempfile::tempdir().unwrap();
        let spec = write_launch_spec(tmp.path());
        let (text, is_error) = launch_call(
            true,
            &json!({
                "cwd": "relative/cwd",
                "spec": spec.display().to_string(),
                "goal": "g",
                "model": "m"
            }),
        );
        assert_eq!(is_error, Some(true), "{text}");
        // The SAME validator and error-text parity as chug_status/chug_collect:
        // the RAW received string, quoted.
        assert!(text.contains("\"relative/cwd\""), "{text}");
        assert!(text.contains("must be an absolute directory"), "{text}");
        assert!(text.starts_with("chug_launch:"), "tool-named error: {text}");
    }

    #[test]
    fn chug_launch_cwd_without_chug_dir_is_refused_naming_the_path() {
        let empty = tempfile::tempdir().unwrap(); // exists, but no .chug/
        let scratch = tempfile::tempdir().unwrap();
        let spec = write_launch_spec(scratch.path());
        let (text, is_error) = launch_call(true, &full_launch_args(empty.path(), &spec));
        assert_eq!(is_error, Some(true), "{text}");
        assert!(text.contains(&empty.path().display().to_string()), "{text}");
        assert!(text.contains("no .chug/ directory in"), "{text}");
    }

    #[test]
    fn chug_launch_nonexistent_spec_is_refused_naming_the_path() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".chug")).unwrap();
        let bogus = "/definitely/not/t129-spec.md";
        let (text, is_error) = launch_call(
            true,
            &json!({
                "cwd": tmp.path().display().to_string(),
                "spec": bogus,
                "goal": "g",
                "model": "m"
            }),
        );
        assert_eq!(is_error, Some(true), "{text}");
        assert!(text.contains(bogus), "names the received path: {text}");
        assert!(
            text.contains("spec does not exist or is not a readable file"),
            "{text}"
        );
    }

    #[test]
    fn chug_launch_relative_spec_is_refused_naming_the_received_string() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".chug")).unwrap();
        let (text, is_error) = launch_call(
            true,
            &json!({
                "cwd": tmp.path().display().to_string(),
                "spec": "t129-relative-spec.md",
                "goal": "g",
                "model": "m"
            }),
        );
        assert_eq!(is_error, Some(true), "{text}");
        assert!(text.contains("\"t129-relative-spec.md\""), "{text}");
        assert!(text.contains("spec must be an absolute path"), "{text}");
    }

    #[test]
    fn chug_launch_empty_and_whitespace_goals_are_refused() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".chug")).unwrap();
        let spec = write_launch_spec(tmp.path());
        for goal in ["", "   \t "] {
            let (text, is_error) = launch_call(
                true,
                &json!({
                    "cwd": tmp.path().display().to_string(),
                    "spec": spec.display().to_string(),
                    "goal": goal,
                    "model": "m"
                }),
            );
            assert_eq!(is_error, Some(true), "{goal:?}: {text}");
            assert!(
                text.contains("goal must be a non-empty string after trim"),
                "{goal:?}: {text}"
            );
            // Validation precedes the spawn seam: a refused payload must not
            // have launched anything (no delegate.log was opened).
            assert!(
                !tmp.path().join(".chug/delegate.log").exists(),
                "a refused goal must not spawn: {goal:?}"
            );
        }
    }

    #[test]
    fn chug_launch_empty_model_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".chug")).unwrap();
        let spec = write_launch_spec(tmp.path());
        let (text, is_error) = launch_call(
            true,
            &json!({
                "cwd": tmp.path().display().to_string(),
                "spec": spec.display().to_string(),
                "goal": "g",
                "model": ""
            }),
        );
        assert_eq!(is_error, Some(true), "{text}");
        assert!(text.contains("model must be a non-empty string"), "{text}");
        assert!(!tmp.path().join(".chug/delegate.log").exists(), "no spawn: {text}");
    }

    #[test]
    fn chug_launch_budgets_above_the_ceilings_are_refused_naming_the_ceiling() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".chug")).unwrap();
        let spec = write_launch_spec(tmp.path());
        // 201 > 200: REJECTED (not clamped), naming the received value and
        // the 200 ceiling (loopd's own).
        let (text, is_error) = launch_call(
            true,
            &json!({
                "cwd": tmp.path().display().to_string(),
                "spec": spec.display().to_string(),
                "goal": "g",
                "model": "m",
                "max_iters": 201
            }),
        );
        assert_eq!(is_error, Some(true), "{text}");
        assert!(text.contains("`max_iters` must be at most 200"), "{text}");
        assert!(text.contains("got 201"), "{text}");
        // 241 > 240: the same reject-above semantics.
        let (text, is_error) = launch_call(
            true,
            &json!({
                "cwd": tmp.path().display().to_string(),
                "spec": spec.display().to_string(),
                "goal": "g",
                "model": "m",
                "max_minutes": 241
            }),
        );
        assert_eq!(is_error, Some(true), "{text}");
        assert!(text.contains("`max_minutes` must be at most 240"), "{text}");
        assert!(text.contains("got 241"), "{text}");
        // Neither refusal spawned anything.
        assert!(!tmp.path().join(".chug/delegate.log").exists(), "no spawn: {text}");
    }

    #[test]
    fn chug_launch_budgets_parse_reject_above_accept_boundary_and_default_to_none() {
        // Boundary legs 200/240 ACCEPTED (at the validation layer — the
        // stub-spawn leg covers the full path with these values).
        let parsed = parse_launch_budgets(&json!({ "max_iters": 200, "max_minutes": 240 }))
            .expect("boundary values accepted");
        assert_eq!(parsed, (Some(200), Some(240)));
        // Reject-above names the received value and the ceiling.
        let err = parse_launch_budgets(&json!({ "max_iters": 201 })).unwrap_err();
        assert!(err.contains("max_iters") && err.contains("at most 200") && err.contains("201"), "{err}");
        let err = parse_launch_budgets(&json!({ "max_minutes": 241 })).unwrap_err();
        assert!(err.contains("max_minutes") && err.contains("at most 240") && err.contains("241"), "{err}");
        // Below 1 and non-integers are refused too — never clamped, never
        // silently defaulted.
        for bad in [json!(0), json!(-5), json!("200"), json!(7.5), json!(true)] {
            let err = parse_launch_budgets(&json!({ "max_iters": bad })).unwrap_err();
            assert!(err.contains("max_iters"), "{bad}: {err}");
        }
        // Absent (or null) → None: the delegate defaults (40/35) apply
        // seam-side, exactly like a flagless human launch.
        assert_eq!(parse_launch_budgets(&json!({})).unwrap(), (None, None));
        assert_eq!(
            parse_launch_budgets(&json!({ "max_iters": null, "max_minutes": null })).unwrap(),
            (None, None)
        );
    }

    /// Poll (deadline-bounded) for the stub's atomically-published dump —
    /// launch returns at spawn, so the dump lands milliseconds later.
    #[cfg(unix)]
    fn wait_for_stub_dump(path: &Path) -> Vec<String> {
        use std::time::{Duration, Instant};
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Ok(text) = fs::read_to_string(path) {
                let dump: Vec<String> = text.lines().map(str::to_string).collect();
                if !dump.is_empty() {
                    return dump;
                }
            }
            assert!(
                Instant::now() < deadline,
                "stub never wrote {}",
                path.display()
            );
            std::thread::sleep(Duration::from_millis(25));
        }
    }

    /// The stub-spawn leg: `CHUG_DELEGATE_BIN` pointed at a tiny shell stub
    /// (the T126 idiom — written in the test's tempdir, records its argv and
    /// cwd, sleeps as a fake child) that the launch seam substitutes for the
    /// real binary. Pins the EXACT child argv order/values, the child's cwd,
    /// the boundary budgets (200/240) reaching the argv verbatim, and that
    /// the returned pid/log/events name real paths. The stub is killed at
    /// leg end. The goal carries spaces and double quotes — the delegate
    /// argv mechanism delivers it byte-exact with no quoting code.
    #[cfg(unix)]
    #[test]
    fn chug_launch_stub_spawn_pins_exact_argv_cwd_and_return_paths() {
        use std::os::unix::fs::PermissionsExt;
        use std::time::Duration;
        // The env var is process-global and the delegate tests mutate it too
        // (same test binary) — one env, one lock.
        let _guard = crate::delegate::tests::DELEGATE_ENV_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let target = tempfile::tempdir().unwrap();
        let scratch = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(target.path().join(".chug")).unwrap();
        let spec = write_launch_spec(scratch.path());
        let stub = scratch.path().join("chug-launch-stub.sh");
        std::fs::write(
            &stub,
            concat!(
                "#!/bin/sh\n",
                "pwd -P > cwd.txt\n",
                "printf '%s\\n' \"$@\" > argv.tmp && mv argv.tmp argv.txt\n",
                ": > .chug/events.jsonl\n",
                "sleep 60\n",
            ),
        )
        .unwrap();
        std::fs::set_permissions(&stub, fs::Permissions::from_mode(0o755)).unwrap();
        // SAFETY: serialized by the delegate env lock; removed before return.
        unsafe { std::env::set_var("CHUG_DELEGATE_BIN", &stub) };
        let goal = "t129 goal with spaces and \"quotes\"";
        let (text, is_error) = launch_call(
            true,
            &json!({
                "cwd": target.path().display().to_string(),
                "spec": spec.display().to_string(),
                "goal": goal,
                "model": "stub-model",
                "max_iters": 200,
                "max_minutes": 240,
            }),
        );
        assert_eq!(is_error, Some(false), "{text}");
        // The return shape mirrors delegate launch: pid, log, events.
        let pid: u32 = text
            .lines()
            .find_map(|l| l.strip_prefix("launched: pid "))
            .expect("pid line in launch result")
            .trim()
            .parse()
            .expect("pid parses");
        let log_path = target.path().join(".chug/delegate.log");
        let events_path = target.path().join(".chug/events.jsonl");
        assert!(text.contains(&format!("log: {}", log_path.display())), "{text}");
        assert!(text.contains(&format!("events: {}", events_path.display())), "{text}");
        // Real paths: the log exists (the parent opened it for the child's
        // stdout+stderr before spawn).
        assert!(log_path.is_file(), "log must exist: {text}");
        // The EXACT child argv, in order, with the boundary budgets verbatim.
        let argv = wait_for_stub_dump(&target.path().join("argv.txt"));
        let expected = [
            "run",
            "--spec",
            spec.to_str().unwrap(),
            "--goal",
            goal,
            "--model",
            "stub-model",
            "--max-iters",
            "200",
            "--max-minutes",
            "240",
        ];
        assert_eq!(argv, expected, "exact child argv");
        // The child ran IN the target cwd: `pwd -P` names it, and its
        // relative events touch landed in that cwd's .chug/.
        let cwd_txt = fs::read_to_string(target.path().join("cwd.txt")).expect("cwd.txt");
        let expected_cwd = fs::canonicalize(target.path()).unwrap();
        assert_eq!(cwd_txt.trim(), expected_cwd.to_str().unwrap(), "child cwd");
        assert!(events_path.is_file(), "events file created by the stub: {text}");
        // Reap/kill the stub at leg end (its own process group), then
        // restore the seam.
        crate::tools::kill_pid_group(pid);
        std::thread::sleep(Duration::from_millis(50));
        // SAFETY: serialized by the delegate env lock.
        unsafe { std::env::remove_var("CHUG_DELEGATE_BIN") };
    }

    // ---------- structural stdout purity ----------

    /// The subcommand's code path must never call the banner / run_start
    /// writers, and must have no stdout writer except the protocol
    /// `writeln!`. Pinned by construction here (the module greps clean) AND
    /// by the dispatch shape in main.rs (`CliCommand::McpServe =>` goes
    /// straight to `mcp_serve::serve`, not through cmd_run/cmd_chat).
    ///
    /// The needles are built with `concat!` so this test's own source does
    /// not contain them — a literal needle would self-match and the grep
    /// could never pass.
    #[test]
    fn stdout_purity_module_has_no_stdout_writers_and_no_banner_calls() {
        let src = include_str!("mcp_serve.rs");
        // No stdout writers anywhere — including inside tests (a test
        // println! is noise, but the grep is cheap and absolute).
        let println_needle = concat!("print", "ln!(");
        let print_needle = concat!("print", "!(");
        assert!(!src.contains(println_needle), "no stdout print allowed: the wire is stdout");
        assert!(!src.contains(print_needle), "no stdout print allowed: the wire is stdout");
        // The one protocol writer exists (the assertion keeps the grep from
        // passing vacuously after a rewrite of the I/O layer).
        let writeln_needle = concat!("write", "ln!(out, \"{response}\")");
        assert!(src.contains(writeln_needle), "protocol writer present");
        // The production half only: the test module below the `#[cfg(test)]`
        // boundary legitimately builds fixtures (tempdir mkdirs, doc-mention
        // needles), which are not the serve path. Everything up to the test
        // boundary must be free of the banner / events-writer / driver-lock
        // / filesystem-write surface `chug run` starts with.
        let prod = src
            .split("#[cfg(test)]")
            .next()
            .expect("the module always has a non-test half");
        let banner_needle = concat!("print_startup", "_banner");
        assert!(!prod.contains(banner_needle), "no banner call on the mcp-serve path");
        let run_start_needle = concat!("eventlog::", "run_start");
        assert!(!prod.contains(run_start_needle), "no run_start call on the mcp-serve path");
        let lock_needle = concat!("driver_", "lock");
        assert!(!prod.contains(lock_needle), "no driver lock on the mcp-serve path");
        let mkdir_needle = concat!("create_dir", "_all");
        let write_needle = concat!("fs::", "write(");
        let open_needle = concat!("Open", "Options");
        for needle in [mkdir_needle, write_needle, open_needle] {
            assert!(!prod.contains(needle), "phase 1 writes nothing ({needle})");
        }
    }

    // ---------- EOF / framing ----------

    #[test]
    fn serve_loop_returns_ok_on_immediate_eof() {
        // No lines at all → Ok (the "closed stdin exits cleanly" leg).
        let mut out: Vec<u8> = Vec::new();
        serve_from(&mut "".as_bytes(), &mut out).expect("EOF is a clean exit");
        assert!(out.is_empty(), "no output without input: {out:?}");
    }

    #[test]
    fn serve_loop_writes_one_line_per_request_and_skips_blank_lines_and_notifications() {
        let input = "\
{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{}}

{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}
{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"ping\"}
";
        let mut out: Vec<u8> = Vec::new();
        serve_from(&mut input.as_bytes(), &mut out).expect("EOF is a clean exit");
        let text = String::from_utf8(out).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2, "exactly the two request responses, one per line: {text}");
        let (_, id1, _, _) = parts(lines[0]);
        let (_, id2, _, _) = parts(lines[1]);
        assert_eq!(id1, Some(json!(1)), "initialize first");
        assert_eq!(id2, Some(json!(2)), "the notification produced no response");
    }

    #[test]
    fn serve_loop_keeps_reading_after_error_responses() {
        // Errors never kill the loop: garbage, then a valid ping, then EOF.
        let input = "\
{ not json !!!
{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"ping\"}
";
        let mut out: Vec<u8> = Vec::new();
        serve_from(&mut input.as_bytes(), &mut out).expect("keeps reading");
        let text = String::from_utf8(out).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2, "{text}");
        let (_, _, _, error) = parts(lines[0]);
        assert_eq!(error.expect("parse error")["code"], -32700);
        let (_, id, result, _) = parts(lines[1]);
        assert_eq!(id, Some(json!(3)));
        assert_eq!(result, Some(json!({})));
    }
}
