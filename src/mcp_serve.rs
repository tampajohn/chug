//! T124 — F10 phase 1 + T128 — phase 2a: `chug mcp-serve`, the stdio MCP
//! SERVER side.
//!
//! Until now chug was an MCP CLIENT only (`src/mcp.rs` consumes servers);
//! nothing exposed chug TO another agent — Claude Code, the bridge fleet,
//! or a second chug could observe a run only by shelling out and mining
//! `.chug/` by hand. This module flips that: `chug mcp-serve` speaks
//! newline-delimited JSON-RPC 2.0 on stdio (the exact framing the client
//! side writes — one object per line) and serves TWO read-only tools,
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
//! Read-only by design (phases 1–2a): no process spawning, no writes
//! anywhere. `chug_status` reads `<cwd>/.chug/events.jsonl` through the
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
//! Deferred (F10 phase 2b/3): `chug_launch` (the write leg, flag-gated),
//! a server log file, `tools/listChanged`, cancellation,
//! resources/prompts.

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

// ---------------------------------------------------------------------------
// The serve loop
// ---------------------------------------------------------------------------

/// Run the server on real stdio until stdin EOF, then return (the caller
/// maps that to exit 0). Responses are written and flushed one at a time —
/// an unflushed response is a wedged client.
pub(crate) fn serve() -> anyhow::Result<()> {
    // Stdout purity by construction: the ONLY stdout writer in the module
    // is the protocol `writeln!` inside serve_from (see the module doc).
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    serve_from(&mut stdin.lock(), &mut out)
}

/// The loop over an injected reader/writer pair, so the EOF and
/// blank-line-skipping behavior is unit-testable without touching real
/// stdio. Serial dispatch (read-only tools are fast; no concurrency in
/// phase 1).
fn serve_from(read: &mut impl BufRead, out: &mut impl Write) -> anyhow::Result<()> {
    for line in read.lines() {
        let line = line?;
        // Blank lines are framing noise, not messages — skip them.
        if line.trim().is_empty() {
            continue;
        }
        if let Some(response) = handle_message(&line) {
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
fn handle_message(line: &str) -> Option<String> {
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
        "tools/list" => Some(ok_response(
            id,
            json!({ "tools": [chug_status_schema(), chug_collect_schema()] }),
        )),
        "tools/call" => Some(call_tool(id, obj)),
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

/// Dispatch `tools/call`. A known tool's OWN failure (bad cwd, unreadable
/// events) is a tool RESULT with `isError: true` — not a JSON-RPC error —
/// so the caller sees the tool ran and failed, the shape the client side
/// already parses (`isError` + text content).
fn call_tool(id: Value, req: &Map<String, Value>) -> String {
    let params = req.get("params").cloned().unwrap_or(Value::Null);
    let Some(name) = params.get("name").and_then(Value::as_str) else {
        return error_response(id, -32602, "Invalid params: tools/call requires a tool name");
    };
    let arguments = params.get("arguments").cloned().unwrap_or(json!({}));
    let (text, is_error) = match name {
        CHUG_STATUS_TOOL => chug_status(&arguments),
        CHUG_COLLECT_TOOL => chug_collect(&arguments),
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
