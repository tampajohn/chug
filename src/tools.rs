use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::Read;
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, anyhow, bail};
use serde_json::{Value, json};

pub const BASH_TIMEOUT_SECS: u64 = 120;
/// Verification (`check:`) commands get a more generous ceiling than the bash
/// tool: 1200s covers the observed 800–1000s warm full-suite wall with
/// headroom (T163 — at the old 600s cap the cycle-76 t153-fixup child was
/// rejected twice on green work), while a truly hung check still dies well
/// inside an 80-iter/35-min child budget.
pub const CHECK_TIMEOUT_SECS: u64 = 1200;
const READ_MAX_LINES: usize = 2000;
const OUTPUT_KEEP_HEAD: usize = 20_000;
const OUTPUT_KEEP_TAIL: usize = 10_000;
const GLOB_MAX: usize = 200;
const LIST_DIR_MAX: usize = 500;

#[derive(Debug, Clone)]
pub struct ToolCtx {
    pub cwd: PathBuf,
    /// Per-command wall-clock budget for the `bash` tool.
    pub bash_timeout: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolResult {
    pub content: String,
    pub is_error: bool,
    /// T91: image payloads riding a tool result (`read_file` on an image
    /// file). Default-empty — every text-only result is unchanged. Previews
    /// and events ride `content` only, never these.
    pub images: Vec<crate::api::ImageBlock>,
}

/// JSON schemas for the tools, in registration order.
pub fn tool_schemas() -> Vec<Value> {
    // T111: the todo tool schemas (todos.rs) are spliced in here — directly
    // after `update_ledger`, the bookkeeping group — so the vec below keeps
    // its single-expression shape and the todo entries ride a extend.
    let mut schemas = vec![
        json!({
            "name": "read_file",
            "description": "Read a text file. Paths are relative to the working directory. Output is capped at 2000 lines and truncation is noted. Use `offset`/`limit` to page beyond the cap. Paths outside the cwd are refused (`path escapes cwd`); cross-tree reads/writes (such as a child worktree in /tmp) go through `bash`.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "File path relative to cwd (must stay inside cwd; outside paths are refused: `path escapes cwd` — cross-tree reads go through `bash`)"},
                    "offset": {"type": "integer", "description": "1-based first line to show (default 1)"},
                    "limit": {"type": "integer", "description": "Max lines to show (default 2000; may exceed the cap)"}
                },
                "required": ["path"]
            }
        }),
        json!({
            "name": "write_file",
            "description": "Create or overwrite a file. Parent directories are created automatically. Paths outside the cwd are refused (`path escapes cwd`); cross-tree reads/writes (such as a child worktree in /tmp) go through `bash`.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "File path relative to cwd (must stay inside cwd; outside paths are refused: `path escapes cwd` — cross-tree writes go through `bash`)"},
                    "content": {"type": "string", "description": "Full file contents"}
                },
                "required": ["path", "content"]
            }
        }),
        json!({
            "name": "edit_file",
            "description": "Exact string replacement in a file. `old` must occur exactly once unless `replace_all` is true, which replaces every occurrence. Paths outside the cwd are refused (`path escapes cwd`); cross-tree reads/writes (such as a child worktree in /tmp) go through `bash`.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "File path relative to cwd (must stay inside cwd; outside paths are refused: `path escapes cwd` — cross-tree edits go through `bash`)"},
                    "old": {"type": "string", "description": "Exact text to replace"},
                    "new": {"type": "string", "description": "Replacement text"},
                    "replace_all": {"type": "boolean", "description": "Replace every occurrence (default false)"}
                },
                "required": ["path", "old", "new"]
            }
        }),
        json!({
            "name": "bash",
            "description": "Run a shell command via `sh -c` in the working directory. Captures stdout+stderr and the exit code. 120s timeout; long output is truncated (head+tail kept). macOS sed is BSD sed: GNU range forms like `,+N` do not exist — use `awk` or `sed -n 'N,Mp'` with absolute line numbers. On macOS there is no `timeout` command; bound long commands with `perl -e 'alarm N; exec @ARGV' <cmd>` instead.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "command": {"type": "string", "description": "Shell command to run"}
                },
                "required": ["command"]
            }
        }),
        json!({
            "name": "grep",
            "description": "Search file contents with ripgrep (falls back to grep -rn). Line-numbered matches, max 100 per file.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "pattern": {"type": "string", "description": "Regex or literal pattern to search for"},
                    "path": {"type": "string", "description": "Optional file or directory to search (default: cwd)"}
                },
                "required": ["pattern"]
            }
        }),
        json!({
            "name": "glob",
            "description": "Match file paths under the working directory with a glob pattern (e.g. src/**/*.rs). Returns sorted relative paths, capped at 200 with a truncation note. Paths outside the cwd are refused (`path escapes cwd`), and matches that resolve outside via a symlink are dropped, never reported (T134 F1); cross-tree reads/writes (such as a child worktree in /tmp) go through `bash`.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "pattern": {"type": "string", "description": "Glob pattern, relative to `path` (or cwd)"},
                    "path": {"type": "string", "description": "Optional base directory relative to cwd (must stay inside cwd; outside paths are refused: `path escapes cwd` — cross-tree listings go through `bash`)"}
                },
                "required": ["pattern"]
            }
        }),
        json!({
            "name": "list_dir",
            "description": "List the immediate entries of a directory (default cwd), one per line, directories suffixed with `/`, directories first, sorted. Capped at 500 with a truncation note. Paths outside the cwd are refused (`path escapes cwd`); cross-tree reads/writes (such as a child worktree in /tmp) go through `bash`.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Directory relative to cwd (default: cwd; outside paths are refused: `path escapes cwd` — cross-tree listings go through `bash`)"}
                }
            }
        }),
        json!({
            "name": "update_ledger",
            "description": "Overwrite LEDGER.md, your external memory. Keep sections: ## Done, ## Next, ## Blockers. Call this after every meaningful step.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "content": {"type": "string", "description": "Full new LEDGER.md contents"}
                },
                "required": ["content"]
            }
        }),
        // T111: schemas live in todos.rs (single source of truth for the
        // descriptions the model sees), registered here beside update_ledger
        // — the bookkeeping group. Run+chat only; plan.rs filters by name,
        // so the todo tools are structurally absent from plan mode.
    ];
    schemas.extend(crate::todos::schemas());
    schemas.extend(vec![
        // T70: schema lives in decisions.rs (single source of truth for the
        // description the model sees), registered here alongside the builtins.
        crate::decisions::schema(),
        json!({
            "name": "delegate",
            "description": "Launch, observe, or collect a bounded child `chug run` (e.g. in a worktree you created). action=launch: spawns a detached child with its working directory at `cwd` (absolute), spec/goal/model required, max_iters/max_minutes optional (defaults 40/35), max_tokens optional (child token ceiling; omitted = unlimited), resume optional (true = append --resume, continue the child's prior run instead of starting fresh), env optional (allowlisted child-env map, see its property description — applied after the inherited-env target-dir scrub, so an explicit CARGO_TARGET_DIR in env WINS over the scrub); returns immediately with the child pid, the log/events paths, and the applied env keys (keys only, never values) — it never waits on the child. action=status: reports the child's liveness (when you pass the `pid` from launch), a summary of its .chug/events.jsonl (state, last_iteration, budget-low/goal/abort flags — covering the child's latest run segment), and the tail of its console log. action=collect: returns the child's structured result in ONE bounded non-blocking read — the latest run segment's verdict (goal-accepted / goal-rejected / aborted with reason / running / starting), the accepted goal's summary, the segment's latest check cmd, and best-effort commit refs of the child's cwd (optional `base` scopes the range <base>..HEAD; every git failure degrades to a note, never an error). Never blocks: launch returns at spawn, status reads tails only, collect reads tails only. Optionally pass `wait_secs` on status (0/absent = instant, max 600) to block up to that many seconds, returning early when the child's iteration advances, a verdict or budget-low flag appears, or its liveness flips to dead — per-tool-call last_event churn renders at the deadline but never wakes it (status only — launch and collect reject it). Pass `terminal: true` (status only, default false, requires `wait_secs > 0`) to narrow the wake set to the terminal facts — the goal or abort verdict, a liveness flip to dead, events-file creation when missing at entry — so an actively-working child never wakes the wait on iteration advances; iteration/budget-low telemetry still renders at the deadline (launch and collect reject `terminal` too).",
            "input_schema": {
                "type": "object",
                "properties": {
                    "action": {"type": "string", "enum": ["launch", "status", "collect"], "description": "launch spawns a detached child chug run; status observes a previously launched one; collect returns a finished (or in-progress) child's structured result (verdict + goal summary + check cmd + commit refs) in one bounded non-blocking read"},
                    "cwd": {"type": "string", "description": "Absolute directory the child runs in (all three actions; the worktree you created — NOT confined to your cwd)"},
                    "spec": {"type": "string", "description": "Absolute path to the spec file (launch only, required)"},
                    "goal": {"type": "string", "description": "Goal text for the child (launch only, required)"},
                    "model": {"type": "string", "description": "Model id the child runs with (launch only, required — routing stays your explicit choice)"},
                    "max_iters": {"type": "integer", "description": "Child iteration budget (launch only; default 40)"},
                    "max_minutes": {"type": "integer", "description": "Child wall-clock budget in minutes (launch only; default 35)"},
                    "max_tokens": {"type": "integer", "minimum": 1, "description": "Child token budget: cumulative input+output tokens across the child run (launch only; omitted = no token ceiling)"},
                    "resume": {"type": "boolean", "description": "launch only (default false): append `--resume` to the child argv, continuing the child's prior run from its .chug/transcript.jsonl instead of starting fresh"},
                    "env": {"type": "object", "additionalProperties": {"type": "string"}, "description": "launch only (optional): a string→string map of environment variables passed to the CHILD process at spawn. Keys must match ^(CARGO_|CHUG_|RUST)[A-Z0-9_]*$ — at most 16 entries, values at most 4 KiB with no NUL bytes; anything else (PATH, lowercase, empty) is a tool error naming the key, because the allowlist keeps a model-influenced goal from rewriting PATH/HOME/DYLD_* on the child. Application order at spawn: the inherited env → the T144 target-dir scrub → these entries, so an explicit CARGO_TARGET_DIR here WINS over the scrub (the scrub guards the ABSENT case; env is the EXPLICIT case). When env is absent the goal-carried `export` remains the fallback and the spawn is byte-identical; the launch payload names the applied keys, never the values."},
                    "base": {"type": "string", "description": "collect only (optional): a git ref scoping the commit-refs range as <base>..HEAD (e.g. \"origin/main\"); absent = the bounded default range over HEAD (last 20 commits). Non-string → tool error"},
                    "pid": {"type": "integer", "description": "The pid launch returned (status and collect, optional): status reports liveness, collect adds the same alive line; omit → status reports liveness unknown and collect renders no liveness line"},
                    "wait_secs": {"type": "integer", "minimum": 0, "maximum": 600, "description": "Seconds to block on status waiting for a significant child change (iteration advance, verdict or budget-low flag, liveness flip to dead; last_event churn renders at the deadline but never wakes) or this deadline (0/absent = instant; status only — launch and collect reject it: collect never blocks)"},
                    "terminal": {"type": "boolean", "description": "status only (default false): with `wait_secs > 0`, the wait wakes only on the terminal facts — `goal_seen` or `abort_seen` flipping true, liveness alive→dead, or events-file creation when missing at entry — never on iteration advances, `budget_low_seen` flips, or `max_iters` appearance (that telemetry still renders at the deadline); `terminal: true` with `wait_secs` absent or 0 → tool error (launch and collect reject it)"}
                },
                "required": ["action", "cwd"]
            }
        }),
        // T37: schema lives in webfetch.rs (single source of truth for the
        // description the model sees), registered here alongside the builtins.
        crate::webfetch::schema(),
        // T180: same single-source pattern — the web_search schema lives in
        // websearch.rs (provider seam + DuckDuckGo HTML provider).
        crate::websearch::schema(),
        // T76: schema lives in tgrep.rs (same single-source pattern).
        crate::tgrep::schema(),
        // T169: the model-facing MCP resources tool (list/read over the
        // T162 registry legs). Builtin — NOT an mcp__ server tool — so T90
        // deny rules can match the plain name; read-only like the mcp__
        // calls (the laya risk gate judges bash only, same bypass posture).
        json!({
            "name": "mcp_resource",
            "description": "List or read resources from configured MCP servers. action=list: the resource catalog, one resource per line (`server uri — description (mimeType)`), across all servers or one named `server` (optional). action=read: one resource's contents from `server` at `uri` (both required) — text contents inline, binary contents as base64 with the mimeType named, output char-capped with a truncation note. Capability-gated: only servers whose initialize handshake advertised `resources` are queried — a non-capable server, an unknown server name, or a dead server is a named tool error, never a hang; with no servers connected, list says so plainly.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "action": {"type": "string", "enum": ["list", "read"], "description": "list: the resource catalog (all servers, or one when `server` is given); read: one resource's contents"},
                    "server": {"type": "string", "description": "MCP server name (list: optional, filters to one server; read: required)"},
                    "uri": {"type": "string", "description": "Resource uri (read only, required)"}
                },
                "required": ["action"]
            }
        }),
        json!({
            "name": "goal_complete",
            "description": "Assert that the goal is fully met and verified. Verification runs the spec's `check:` command if present; a failing check rejects the claim and the loop continues.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "summary": {"type": "string", "description": "Short summary of what was accomplished and how it was verified"}
                },
                "required": ["summary"]
            }
        }),
    ]);
    schemas
}

/// Dispatch a tool call. Internal failures are converted into `is_error` results
/// so the model can see and recover from them.
pub fn dispatch(ctx: &ToolCtx, name: &str, input: &Value) -> ToolResult {
    match inner(ctx, name, input) {
        Ok(result) => result,
        Err(e) => ToolResult {
            content: format!("tool error: {e:#}"),
            is_error: true,
        images: Vec::new(),
        },
    }
}

fn inner(ctx: &ToolCtx, name: &str, input: &Value) -> anyhow::Result<ToolResult> {
    match name {
        "read_file" => read_file(ctx, input),
        "write_file" => write_file(ctx, input),
        "edit_file" => edit_file(ctx, input),
        "bash" => bash(ctx, input),
        "grep" => grep(ctx, input),
        "glob" => glob_tool(ctx, input),
        "list_dir" => list_dir(ctx, input),
        "delegate" => crate::delegate::delegate(ctx, input),
        "web_fetch" => crate::webfetch::web_fetch(input),
        "web_search" => crate::websearch::web_search(input),
        "tgrep" => crate::tgrep::tgrep(ctx, input),
        "decision_log" => crate::decisions::decision_log(&ctx.cwd, input),
        "update_ledger" => update_ledger(ctx, input),
        // T111: the todo tools (schemas + logic in todos.rs).
        "todo_add" => crate::todos::todo_add(&ctx.cwd, input),
        "todo_update" => crate::todos::todo_update(&ctx.cwd, input),
        "todo_list" => crate::todos::todo_list(&ctx.cwd),
        "goal_complete" => Ok(ToolResult {
            content: "goal_complete acknowledged. Verification will run; do not assume acceptance until the loop confirms it.".to_string(),
            is_error: false,
        images: Vec::new(),
        }),
        other => Ok(ToolResult {
            content: format!("unknown tool: {other}"),
            is_error: true,
        images: Vec::new(),
        }),
    }
}

/// T94: how many received keys a param-extraction miss error names before
/// the `, … (+N more)` suffix. A malicious or accidental 500-key object must
/// not balloon the error.
const RECEIVED_KEYS_CAP: usize = 12;

/// T94: the JSON type name for the non-object leg of [`received_hint`]
/// (the T88 req-3 shape). Local copy: decisions.rs has its own for its
/// one-module validation, and this row's diff is confined to tools.rs.
fn json_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

/// T94: the diagnostic appended to a param-extraction miss, naming what WAS
/// received so an alias fumble (cycle-53 eval §2 I3: `old_string` sent where
/// `old` is expected, three times, each error naming only the wanted field —
/// self-reported as a tool bug) self-corrects in one iteration. Object input
/// → the received keys, sorted, capped at [`RECEIVED_KEYS_CAP`] with a
/// `, … (+N more)` suffix; an empty object → `(received keys: none)`. Any
/// non-object input → the JSON type: `(received: array)`. T169: pub(crate)
/// so the `mcp_resource` tool's T88 errors carry the same received-shape
/// hint (one shape everywhere, no per-module copy).
pub(crate) fn received_hint(input: &Value) -> String {
    match input {
        Value::Object(map) => {
            let mut keys: Vec<&str> = map.keys().map(String::as_str).collect();
            keys.sort_unstable();
            if keys.is_empty() {
                return "(received keys: none)".to_string();
            }
            let total = keys.len();
            keys.truncate(RECEIVED_KEYS_CAP);
            let mut list = keys.join(", ");
            if total > RECEIVED_KEYS_CAP {
                list.push_str(&format!(", … (+{} more)", total - RECEIVED_KEYS_CAP));
            }
            format!("(received keys: {list})")
        }
        other => format!("(received: {})", json_type_name(other)),
    }
}

pub(crate) fn get_str<'a>(input: &'a Value, key: &str) -> anyhow::Result<&'a str> {
    input
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing or non-string field: {key} {}", received_hint(input)))
}

fn get_path(ctx: &ToolCtx, input: &Value) -> anyhow::Result<PathBuf> {
    let raw = get_str(input, "path")?;
    resolve_safe(&ctx.cwd, raw).map_err(|e| anyhow!("{e}"))
}

/// T26: `read_file` — optional `offset`/`limit` pagination.
///
/// With neither param the behavior is pre-T26 byte-for-byte: the whole file
/// when it fits under [`READ_MAX_LINES`], else head-2000 plus the legacy
/// truncation note. With either param, window semantics apply: show lines
/// `offset ..= min(offset + limit - 1, line_count)` (`limit` may exceed the
/// cap; `offset` is 1-based and an `offset < 1` is a tool error). A window
/// that is not the whole file gets a note naming the actual window; an
/// `offset` past EOF is not an error — a short note naming the file length is
/// returned so paging loops can stop cleanly.
fn read_file(ctx: &ToolCtx, input: &Value) -> anyhow::Result<ToolResult> {
    let path = get_path(ctx, input)?;
    if let Some(media_type) = image_media_type(&path) {
        return read_image(&path, media_type);
    }
    let data =
        fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let line_count = data.lines().count();

    // Default path (neither param): exactly today's output, including the
    // legacy note wording — pinned byte-identical by test.
    if input.get("offset").is_none() && input.get("limit").is_none() {
        let content = if line_count > READ_MAX_LINES {
            let head: Vec<&str> = data.lines().take(READ_MAX_LINES).collect();
            format!(
                "{}\n\n[truncated: showing lines 1-{READ_MAX_LINES} of {line_count}]",
                head.join("\n")
            )
        } else {
            data
        };
        return Ok(ToolResult {
            content,
            is_error: false,
        images: Vec::new(),
        });
    }

    // Window path (either param present). `offset` is 1-based.
    let offset = match input.get("offset") {
        Some(v) => {
            let n = v.as_u64().ok_or_else(|| {
                anyhow!("read_file: `offset` must be an integer (1-based first line to show)")
            })?;
            if n < 1 {
                bail!("read_file: `offset` is 1-based — the first line is 1, got {n}");
            }
            usize::try_from(n).unwrap_or(usize::MAX)
        }
        None => 1,
    };
    let limit = match input.get("limit") {
        Some(v) => {
            let n = v.as_u64().ok_or_else(|| {
                anyhow!("read_file: `limit` must be an integer (max lines to show)")
            })?;
            if n < 1 {
                bail!("read_file: `limit` must be at least 1, got {n}");
            }
            usize::try_from(n).unwrap_or(usize::MAX)
        }
        // Default `limit` stays the cap; explicit paging may exceed it.
        None => READ_MAX_LINES,
    };

    // Past-EOF offset: not an error — name the file length so a paging loop
    // can stop.
    if offset > line_count {
        return Ok(ToolResult {
            content: format!(
                "[offset {offset} is past the end of this file: it has {line_count} lines]"
            ),
            is_error: false,
        images: Vec::new(),
        });
    }
    let end = offset.saturating_add(limit - 1).min(line_count);
    let shown: Vec<&str> = data
        .lines()
        .skip(offset - 1)
        .take(end - offset + 1)
        .collect();
    let mut content = shown.join("\n");
    if offset != 1 || end != line_count {
        content.push_str(&format!(
            "\n\n[showing lines {offset}-{end} of {line_count}]"
        ));
    }
    Ok(ToolResult {
        content,
        is_error: false,
    images: Vec::new(),
    })
}

/// T91: the cap for an image read (`read_file` image leg) — 5 MiB of raw
/// bytes. Bigger files are a tool error naming the cap and the actual size,
/// with no partial read and no base64 anywhere in the error text.
const IMAGE_MAX_BYTES: u64 = 5 * 1024 * 1024;

/// T91: the image extensions `read_file` returns as images, mapped to their
/// media types. Extension match is on the lowercased extension; every other
/// extension — including unknown binary ones — keeps the text path exactly.
fn image_media_type(path: &Path) -> Option<&'static str> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    match ext.as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        _ => None,
    }
}

/// T91: the image leg of `read_file` — bytes, not text. The path has already
/// been through `get_path` (resolve_safe) BEFORE any read happens here; this
/// function only changes the response shape. The text content is a short
/// note naming path, size and media type — previews and events ride it, the
/// base64 rides the result's `images` only.
fn read_image(path: &Path, media_type: &'static str) -> anyhow::Result<ToolResult> {
    let bytes = fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    let size = bytes.len() as u64;
    if size > IMAGE_MAX_BYTES {
        bail!(
            "read_file: image too large: {} is {size} bytes; the cap is {IMAGE_MAX_BYTES} bytes (5 MiB)",
            path.display()
        );
    }
    let data = base64_encode(&bytes);
    let content = format!("[image: {} ({size} bytes, {media_type})]", path.display());
    Ok(ToolResult {
        content,
        is_error: false,
        images: vec![crate::api::ImageBlock {
            media_type: media_type.to_string(),
            data,
        }],
    })
}

/// Standard-alphabet base64 (RFC 4648, with padding), hand-rolled: one encode
/// direction does not justify a crate dependency. Pinned against RFC test
/// vectors by test.
fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = u32::from(chunk[0]);
        let b1 = u32::from(chunk.get(1).copied().unwrap_or(0));
        let b2 = u32::from(chunk.get(2).copied().unwrap_or(0));
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[(n >> 18 & 63) as usize] as char);
        out.push(TABLE[(n >> 12 & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            TABLE[(n >> 6 & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

fn write_file(ctx: &ToolCtx, input: &Value) -> anyhow::Result<ToolResult> {
    let path = get_path(ctx, input)?;
    let content = get_str(input, "content")?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("creating parent dirs for {}", path.display()))?;
    }
    fs::write(&path, content).with_context(|| format!("writing {}", path.display()))?;
    Ok(ToolResult {
        content: format!("wrote {} bytes to {}", content.len(), path.display()),
        is_error: false,
    images: Vec::new(),
    })
}

fn edit_file(ctx: &ToolCtx, input: &Value) -> anyhow::Result<ToolResult> {
    let path = get_path(ctx, input)?;
    let old = get_str(input, "old")?;
    let new = get_str(input, "new")?;
    let replace_all = input
        .get("replace_all")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let data =
        fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    if replace_all {
        let (updated, count) = apply_edit_all(&data, old, new)
            .map_err(|e| anyhow!("edit_file {}: {e}", path.display()))?;
        fs::write(&path, &updated).with_context(|| format!("writing {}", path.display()))?;
        Ok(ToolResult {
            content: format!(
                "edited {} (replaced {count} occurrence{})",
                path.display(),
                if count == 1 { "" } else { "s" }
            ),
            is_error: false,
        images: Vec::new(),
        })
    } else {
        let updated = apply_edit(&data, old, new)
            .map_err(|e| anyhow!("edit_file {}: {e}", path.display()))?;
        fs::write(&path, &updated).with_context(|| format!("writing {}", path.display()))?;
        Ok(ToolResult {
            content: format!("edited {}", path.display()),
            is_error: false,
        images: Vec::new(),
        })
    }
}

/// Pure string-replacement logic: `old` must match exactly once.
fn apply_edit(content: &str, old: &str, new: &str) -> Result<String, String> {
    if old.is_empty() {
        return Err("`old` must not be empty".to_string());
    }
    let count = content.matches(old).count();
    match count {
        0 => Err("`old` not found in file".to_string()),
        1 => Ok(content.replacen(old, new, 1)),
        n => Err(format!(
            "`old` found {n} times; must match exactly once.\n{}",
            describe_matches(content, old)
        )),
    }
}

/// Where `old` occurs in `content`, for the non-unique-match error (T5): the
/// 1-based line number of every match (capped) plus a few lines of context
/// around the first matches, so the model can disambiguate with a longer
/// `old` instead of reverting the file.
fn describe_matches(content: &str, old: &str) -> String {
    const LINE_CAP: usize = 20;
    const CONTEXT_MATCHES: usize = 5;
    const CONTEXT_RADIUS: usize = 2;

    let lines: Vec<&str> = content.lines().collect();
    let match_lines: Vec<usize> = content
        .match_indices(old)
        .map(|(offset, _)| content[..offset].bytes().filter(|&b| b == b'\n').count() + 1)
        .collect();

    let shown = &match_lines[..match_lines.len().min(LINE_CAP)];
    let list = shown
        .iter()
        .map(|ln| ln.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    let mut out = format!("matches at lines: {list}");
    if match_lines.len() > LINE_CAP {
        out.push_str(&format!(" (and {} more)", match_lines.len() - LINE_CAP));
    }
    out.push_str("\ncontext around the first matches:");
    for &ln in match_lines.iter().take(CONTEXT_MATCHES) {
        out.push_str(&format!("\n-- around line {ln} --"));
        let start = ln.saturating_sub(CONTEXT_RADIUS).max(1);
        let end = (ln + CONTEXT_RADIUS).min(lines.len());
        for l in start..=end {
            let marker = if l == ln { '>' } else { ' ' };
            out.push_str(&format!("\n{marker} {l} | {}", lines[l - 1]));
        }
    }
    out
}

/// Replace every occurrence, returning the updated text and the count.
fn apply_edit_all(content: &str, old: &str, new: &str) -> Result<(String, usize), String> {
    if old.is_empty() {
        return Err("`old` must not be empty".to_string());
    }
    let count = content.matches(old).count();
    if count == 0 {
        return Err("`old` not found in file".to_string());
    }
    Ok((content.replace(old, new), count))
}

fn bash(ctx: &ToolCtx, input: &Value) -> anyhow::Result<ToolResult> {
    let command = get_str(input, "command")?;
    let outcome = run_shell(&ctx.cwd, command, ctx.bash_timeout)?;
    let body = truncate_middle(&outcome.output, OUTPUT_KEEP_HEAD, OUTPUT_KEEP_TAIL);
    let exit_label = match outcome.exit_code {
        Some(code) => code.to_string(),
        None => "none (killed after timeout)".to_string(),
    };
    Ok(ToolResult {
        content: format!("{body}\n[exit code: {exit_label}]"),
        is_error: outcome.timed_out || outcome.exit_code.is_some_and(|c| c != 0),
    images: Vec::new(),
    })
}

fn grep(ctx: &ToolCtx, input: &Value) -> anyhow::Result<ToolResult> {
    let pattern = get_str(input, "pattern")?;
    let target: PathBuf = match input.get("path").and_then(Value::as_str) {
        Some(p) => resolve_safe(&ctx.cwd, p).map_err(|e| anyhow!("{e}"))?,
        None => ctx.cwd.clone(),
    };
    match Command::new("rg")
        .arg("-n")
        .arg("--max-count")
        .arg("100")
        .arg("--")
        .arg(pattern)
        .arg(&target)
        .output()
    {
        Ok(out) => Ok(grep_result(out.stdout, out.stderr, out.status.code())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let out = Command::new("grep")
                .arg("-rn")
                .arg("-e")
                .arg(pattern)
                .arg(&target)
                .output()
                .context("running grep fallback")?;
            Ok(grep_result(out.stdout, out.stderr, out.status.code()))
        }
        Err(e) => bail!("spawning rg: {e}"),
    }
}

fn glob_tool(ctx: &ToolCtx, input: &Value) -> anyhow::Result<ToolResult> {
    let pattern = get_str(input, "pattern")?;
    let base: PathBuf = match input.get("path").and_then(Value::as_str) {
        Some(p) => resolve_safe(&ctx.cwd, p).map_err(|e| anyhow!("{e}"))?,
        None => ctx.cwd.clone(),
    };
    let joined = base.join(pattern);
    // Same path-safety rules as the other file tools — rejects `..`
    // traversal, absolute paths outside cwd, and symlinks resolving outside
    // cwd (T134) — plus the pattern-aware rules (T134 F1): the literal
    // prefix is stage-2 confined, and every concrete match below is
    // re-confined through the full two-stage check because the glob crate
    // follows symlinked directories during expansion.
    let pattern_path = resolve_glob_pattern(&ctx.cwd, &joined.to_string_lossy())
        .map_err(|e| anyhow!("{e}"))?;
    let matched = glob::glob(&pattern_path.to_string_lossy())
        .map_err(|e| anyhow!("invalid glob pattern: {e}"))?
        .collect::<Result<Vec<PathBuf>, _>>()
        .map_err(|e| anyhow!("glob error: {e}"))?;
    // Confinement, not a lexical courtesy: a match that resolves outside
    // cwd through a symlinked directory is dropped, never reported
    // (T134 F1 — the old lexical `starts_with` net passed `etcdir/...`
    // straight through). Survivors are reported relative.
    let relative: Vec<String> = matched
        .into_iter()
        .filter(|p| confine_glob_match(&ctx.cwd, p))
        .filter_map(|p| {
            p.strip_prefix(&ctx.cwd)
                .ok()
                .map(|rel| rel.to_string_lossy().into_owned())
        })
        .collect();
    Ok(ToolResult {
        content: format_sorted_capped(relative, GLOB_MAX, "matches"),
        is_error: false,
    images: Vec::new(),
    })
}

fn list_dir(ctx: &ToolCtx, input: &Value) -> anyhow::Result<ToolResult> {
    let path: PathBuf = match input.get("path").and_then(Value::as_str) {
        Some(p) => resolve_safe(&ctx.cwd, p).map_err(|e| anyhow!("{e}"))?,
        None => ctx.cwd.clone(),
    };
    let mut entries: Vec<(bool, String)> = Vec::new();
    for entry in fs::read_dir(&path).with_context(|| format!("reading {}", path.display()))? {
        let entry = entry.with_context(|| format!("reading {}", path.display()))?;
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        entries.push((
            is_dir,
            entry.file_name().to_string_lossy().into_owned(),
        ));
    }
    // Directories first, then names, within each group.
    entries.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    let names: Vec<String> = entries
        .into_iter()
        .map(|(is_dir, name)| if is_dir { format!("{name}/") } else { name })
        .collect();
    Ok(ToolResult {
        content: cap_lines(names, LIST_DIR_MAX),
        is_error: false,
    images: Vec::new(),
    })
}

/// Sort, cap, and format path/entry listings with a truncation note.
pub fn format_sorted_capped(mut items: Vec<String>, cap: usize, label: &str) -> String {
    items.sort();
    if items.len() <= cap {
        return if items.is_empty() {
            format!("no {label}")
        } else {
            items.join("\n")
        };
    }
    let total = items.len();
    items.truncate(cap);
    format!(
        "{}\n[truncated: showing first {cap} of {total}]",
        items.join("\n")
    )
}

/// Cap an already-ordered line list, keeping order, with a truncation note.
pub fn cap_lines(mut lines: Vec<String>, cap: usize) -> String {
    if lines.len() <= cap {
        return if lines.is_empty() {
            "(empty directory)".to_string()
        } else {
            lines.join("\n")
        };
    }
    let total = lines.len();
    lines.truncate(cap);
    format!(
        "{}\n[truncated: showing first {cap} of {total} entries]",
        lines.join("\n")
    )
}

fn grep_result(stdout: Vec<u8>, stderr: Vec<u8>, exit_code: Option<i32>) -> ToolResult {
    let mut text = lossy_trimmed(&stdout);
    let err = lossy_trimmed(&stderr);
    if !err.is_empty() {
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(&err);
    }
    if text.is_empty() {
        text = "no matches".to_string();
    }
    ToolResult {
        content: truncate_middle(&text, OUTPUT_KEEP_HEAD, OUTPUT_KEEP_TAIL),
        // exit 1 means "no matches" for both rg and grep; >= 2 is a real failure
        is_error: exit_code.is_some_and(|c| c > 1),
    images: Vec::new(),
    }
}

/// `update_ledger` `{content}`: wholesale-replace `<cwd>/LEDGER.md`, the
/// orchestrator's and every child's external memory. T145: the write goes
/// through `fsatomic::write_atomic` (same-dir temp + fsync + rename) like
/// the transcript and todo store (T136) — a crash mid-write must never
/// leave a torn or empty ledger, and a failed write surfaces as a tool
/// error (never aborts the run) with the previous ledger byte-intact.
fn update_ledger(ctx: &ToolCtx, input: &Value) -> anyhow::Result<ToolResult> {
    let content = get_str(input, "content")?;
    let path = ctx.cwd.join("LEDGER.md");
    crate::fsatomic::write_atomic(&path, content.as_bytes())
        .with_context(|| format!("writing {}", path.display()))?;
    Ok(ToolResult {
        content: format!("ledger updated ({} bytes)", content.len()),
        is_error: false,
    images: Vec::new(),
    })
}

/// Symlink expansions allowed per `resolve_safe` call (T134): deep chains
/// must hit a bounded refusal, not spin — fail closed.
const MAX_SYMLINK_HOPS: usize = 40;

/// Both refusal messages of `resolve_safe` carry a suffix naming the `bash`
/// escape hatch (T61): the tool descriptions are read once at turn 0, but
/// the error string is what the model sees at the moment of need — it
/// must name the fallback.
const PATH_ESCAPES_CWD_SUFFIX: &str = " — cross-tree paths go through bash";

/// The glob metacharacters the `glob` crate patterns chug builds recognize
/// (T134 F1): a path component containing any of these is a PATTERN, not a
/// literal file name. Public because tgrep's corpus arm branches
/// pattern-vs-literal on the same set it resolves patterns with.
pub const GLOB_METACHARS: [char; 3] = ['*', '?', '['];

/// Does any component of `p` contain a glob metacharacter? Component-scoped
/// and applied only to the model-supplied relative part, so a sandbox root
/// whose own absolute path contains `[` (a bracketed checkout dir) keeps
/// working — only the model's choice of name is policed.
fn has_glob_metachar(p: &Path) -> bool {
    p.components()
        .any(|c| c.as_os_str().to_string_lossy().contains(&GLOB_METACHARS[..]))
}

fn metachar_refusal(path: &str) -> String {
    format!(
        "glob metacharacters in path: {path} — this tool reads literal paths only; \
         put patterns in the glob/tgrep `pattern` or `path` glob arg, or use bash \
         for a filename that really contains `*?[`"
    )
}

/// Resolve `path` against `cwd`, rejecting anything that escapes it. Stage 1
/// (`lexical_normalize`) is the original lexical pass: `..` traversal and
/// absolute paths outside cwd. Stage 2 (`confine_symlinks`, T134 — codex
/// review 20260928 §2 HIGH) walks the real filesystem: a purely lexical
/// check was bypassable with an in-tree `outside -> /etc` symlink, which
/// read_file/write_file/edit_file then followed on disk. Stage 2 resolves
/// every component (lstat, so dangling links are seen too), expands
/// symlinks with a hop budget, and refuses when the resolved path escapes
/// the sandbox root's real location (an alias like macOS `/tmp` →
/// `/private/tmp` for the cwd itself is handled by comparing against the
/// canonical root). Nonexistent tails stay legal — write_file creates
/// those — and the returned path stays lexical: this is a verification
/// pass, not a rewrite; callers keep the path they asked for. Both refusal
/// messages carry the T61 `bash` suffix above.
///
/// Race note: this closes the no-race escape the review describes; an
/// attacker swapping a link between the walk and the actual read/write
/// would need openat(2)-style handling, which std does not offer.
///
/// T134 F1 (kimi validator follow-up): `resolve_safe` is a LITERAL-path
/// contract. Glob-metachar components are refused (fail closed, regardless
/// of existence) — they used to fall into stage 2's "missing from here
/// down" arm and pass as inert, which is exactly what let the glob crate's
/// symlink-following expansion turn `**/passwd` into an external read.
/// Pattern-carrying callers (the glob tool and tgrep's corpus glob arm)
/// must go through [`resolve_glob_pattern`] and re-confine every concrete
/// match with [`confine_glob_match`] instead.
pub fn resolve_safe(cwd: &Path, path: &str) -> Result<PathBuf, String> {
    let normalized = lexical_normalize(cwd, path)
        .map_err(|_| format!("path escapes cwd: {path}{PATH_ESCAPES_CWD_SUFFIX}"))?;
    if !normalized.starts_with(cwd) {
        return Err(format!("path escapes cwd: {path}{PATH_ESCAPES_CWD_SUFFIX}"));
    }
    // Metachars are patterns, not literal names (T134 F1) — scoped to the
    // model-supplied relative part, so a sandbox root with `[` in its own
    // path is unaffected.
    let rel = normalized.strip_prefix(cwd).unwrap_or(&normalized);
    if has_glob_metachar(rel) {
        return Err(metachar_refusal(path));
    }
    confine_symlinks(cwd, &normalized).map_err(|why| {
        format!("path escapes cwd: {path}{PATH_ESCAPES_CWD_SUFFIX} ({why})")
    })?;
    Ok(normalized)
}

/// Pattern-aware resolve for the two glob-expanding surfaces — the glob
/// tool's joined `path` + `pattern` and tgrep's corpus glob arm (T134 F1).
/// The same stage-1 lexical rules as [`resolve_safe`] (`..` traversal,
/// absolute paths outside cwd), then stage-2 confinement of the LITERAL
/// prefix: every component before the first metacharacter is resolved
/// against the real filesystem, so a pattern rooted at an in-tree symlink
/// to an external directory is refused up front. The metachar tail cannot
/// be lstat'd, so it is NOT confined here — that is what
/// [`confine_glob_match`] is for: every concrete match must pass the full
/// two-stage check before its name is reported or its bytes are read.
pub fn resolve_glob_pattern(cwd: &Path, pattern: &str) -> Result<PathBuf, String> {
    let normalized = lexical_normalize(cwd, pattern)
        .map_err(|_| format!("path escapes cwd: {pattern}{PATH_ESCAPES_CWD_SUFFIX}"))?;
    if !normalized.starts_with(cwd) {
        return Err(format!(
            "path escapes cwd: {pattern}{PATH_ESCAPES_CWD_SUFFIX}"
        ));
    }
    // Confine the literal prefix through stage 2. `lexical_normalize` keeps
    // metachar components in place, so the walk below stops at the first
    // one; a pattern that normalizes to a literal path (e.g. `*/../f`
    // collapses the `*`) falls through with no metachars left.
    let rel = normalized.strip_prefix(cwd).unwrap_or(&normalized);
    let mut prefix = cwd.to_path_buf();
    for comp in rel.components() {
        if comp
            .as_os_str()
            .to_string_lossy()
            .contains(&GLOB_METACHARS[..])
        {
            break;
        }
        prefix.push(comp);
    }
    confine_symlinks(cwd, &prefix).map_err(|why| {
        format!("path escapes cwd: {pattern}{PATH_ESCAPES_CWD_SUFFIX} ({why})")
    })?;
    Ok(normalized)
}

/// Post-glob confinement (T134 F1): the `glob` crate FOLLOWS symlinked
/// directories during pattern expansion, so a pattern like `**/passwd`
/// yields matches that lexically sit inside cwd (`etcdir/passwd`) but
/// physically resolve outside it. Every concrete match must re-pass the
/// full two-stage check before its name is reported or its bytes are read;
/// matches that fail are dropped (never surfaced), as are matches whose
/// name itself carries a metacharacter — a literal `*`-named file is not
/// worth reporting through a glob surface, and refusing keeps the
/// drop-or-keep decision independent of filesystem state.
pub fn confine_glob_match(cwd: &Path, matched: &Path) -> bool {
    if !matched.starts_with(cwd) {
        return false;
    }
    let rel = matched.strip_prefix(cwd).unwrap_or(matched);
    if has_glob_metachar(rel) {
        return false;
    }
    confine_symlinks(cwd, matched).is_ok()
}

/// Stage 1 (pre-T134 behavior, kept verbatim): resolve `path` lexically
/// against `cwd`, rejecting `..` traversal and absolute paths outside cwd.
/// No filesystem access here.
fn lexical_normalize(cwd: &Path, path: &str) -> Result<PathBuf, ()> {
    let given = Path::new(path);
    let combined = if given.is_absolute() {
        given.to_path_buf()
    } else {
        cwd.join(given)
    };
    let mut normalized = PathBuf::new();
    for component in combined.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err(());
                }
            }
            Component::RootDir => {
                if normalized.as_os_str().is_empty() {
                    normalized.push(std::path::MAIN_SEPARATOR_STR);
                }
            }
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::Normal(part) => normalized.push(part),
        }
    }
    if !normalized.starts_with(cwd) {
        return Err(());
    }
    Ok(normalized)
}

/// Stage 2 (T134): resolve every component of `resolved` — already proven
/// lexically inside `cwd` — against the real filesystem and refuse if the
/// resolution escapes `cwd`'s real location. The walk keeps `real` as the
/// physical path built so far and a queue of remaining components; a
/// symlink splices its target's components into the queue (absolute
/// targets restart the walk from the filesystem root), the first missing
/// component makes the rest inert (nothing below a missing directory can
/// redirect the resolution, and a dangling link would still lstat as a
/// symlink — this arm cannot hide one), and the containment check runs on
/// the fully resolved path.
fn confine_symlinks(cwd: &Path, resolved: &Path) -> Result<(), String> {
    use std::collections::VecDeque;

    let root = fs::canonicalize(cwd)
        .map_err(|e| format!("sandbox root {} does not resolve: {e}", cwd.display()))?;
    let rel = resolved.strip_prefix(cwd).unwrap_or(resolved);
    let mut real = root.clone();
    let mut queue: VecDeque<std::ffi::OsString> =
        rel.components().map(|c| c.as_os_str().to_os_string()).collect();
    let mut hops = 0usize;
    while let Some(part) = queue.pop_front() {
        if part == "." {
            continue;
        }
        if part == ".." {
            real.pop();
            continue;
        }
        real.push(&part);
        match fs::symlink_metadata(&real) {
            Ok(md) if md.file_type().is_symlink() => {
                hops += 1;
                if hops > MAX_SYMLINK_HOPS {
                    return Err("symlink expansion exceeded the hop limit".to_string());
                }
                let target =
                    fs::read_link(&real).map_err(|e| format!("unreadable symlink: {e}"))?;
                real.pop(); // the link itself is replaced by its target
                if target.is_absolute() {
                    real = PathBuf::new();
                }
                for comp in target.components().rev() {
                    queue.push_front(comp.as_os_str().to_os_string());
                }
            }
            Ok(_) => {}
            Err(_) => {
                // Missing from here down: finish the walk lexically.
                while let Some(next) = queue.pop_front() {
                    if next == "." {
                        continue;
                    }
                    if next == ".." {
                        real.pop();
                    } else {
                        real.push(next);
                    }
                }
            }
        }
    }
    if real.starts_with(&root) {
        Ok(())
    } else {
        Err(format!(
            "resolves to {} via a symlink",
            if real.as_os_str().is_empty() {
                "<above the filesystem root>".to_string()
            } else {
                real.display().to_string()
            }
        ))
    }
}

pub struct ShellOutcome {
    pub exit_code: Option<i32>,
    pub output: String,
    pub timed_out: bool,
}

/// Grace period for the pipe-reader threads after the process exits or is
/// killed. A grandchild that escaped the process group (e.g. via setsid) can
/// hold the pipes open forever; leaking the reader thread is acceptable,
/// blocking the driver is not.
const READER_GRACE: Duration = Duration::from_secs(5);

/// The target-dir variables scrubbed from every driver-spawned child
/// environment (T144): `CARGO_TARGET_DIR` and its `CARGO_BUILD_TARGET_DIR`
/// alias. loopd.sh prefixes every orchestrator with a SHARED cache dir, and
/// the driver's full inherited env reaches `run_shell`'s `sh -c` (the bash
/// tool AND the goal-gate check), hook commands, and the delegate launch's
/// child chug binary. Cargo's artifact filename excludes the checkout path
/// (the T52 lesson), so a shared dir is last-builder-wins — a worktree's
/// goal gate could execute a FOREIGN worktree's test binary. After the
/// scrub a bare `cargo …` in a spawned shell builds `<cwd>/target`,
/// content-correct by construction; an explicit `CARGO_TARGET_DIR=… `
/// prefix inside the command string (the goal-carried `export …`) is
/// unaffected — the child's own shell sets it after spawn, it is never
/// inherited.
pub(crate) const TARGET_DIR_VARS: [&str; 2] = ["CARGO_TARGET_DIR", "CARGO_BUILD_TARGET_DIR"];

/// Remove [`TARGET_DIR_VARS`] from the environment `cmd`'s child inherits.
/// The one scrub mechanism for every driver-side spawn that passes the
/// inherited env through: the `sh -c` wrapper ([`run_shell`] — shared by
/// the bash tool and the goal-gate check), hook commands
/// ([`crate::hooks::run_hook`]), and the delegate launch's child chug
/// binary ([`crate::delegate::delegate_launch`]). `Command::env_remove`
/// wins over the inherited value; nothing else about the env changes.
pub(crate) fn scrub_target_dir_vars(cmd: &mut Command) {
    for var in TARGET_DIR_VARS {
        cmd.env_remove(var);
    }
}

/// Run `sh -c <command>` in `cwd`, capturing stdout+stderr and the exit code.
/// Drains both pipes on background threads to avoid pipe-buffer deadlock.
///
/// The child's PATH gets `~/.cargo/bin` prepended when that directory exists
/// (T4: cargo is chug's own toolchain; children should never have to discover
/// `cargo: command not found` themselves). An existing PATH is inherited
/// verbatim — the prepend never removes or reorders entries, and a PATH that
/// already contains `~/.cargo/bin` is left untouched.
///
/// T144: the inherited environment arrives with `CARGO_TARGET_DIR` and
/// `CARGO_BUILD_TARGET_DIR` removed ([`TARGET_DIR_VARS`]) — a bare
/// `cargo …` here builds `<cwd>/target`, never a shared dir other checkouts
/// also write. An explicit `CARGO_TARGET_DIR=… ` prefix inside the command
/// string still selects one (the goal-carried `export …` warm path).
///
/// The shell runs in its own process group; when `timeout` elapses the whole
/// group is SIGKILLed (a plain `child.kill()` would orphan grandchildren that
/// keep the pipes open and wedge the caller on join). Reader threads are never
/// joined without a deadline: each forwards what it has read so far over its
/// channel, and the receive side keeps every byte that arrived within the
/// grace period. When a reader has not seen EOF after the grace period (an
/// escaped process still holding the pipe), that side's captured output is
/// returned as-is with a truncation note (`(output truncated: reader did not
/// drain)`, or the `... after kill` spelling when the group kill fired) —
/// only the bytes still in flight beyond the grace are lost.
pub fn run_shell(cwd: &Path, command: &str, timeout: Duration) -> anyhow::Result<ShellOutcome> {
    let mut shell_cmd = Command::new("sh");
    shell_cmd
        .arg("-c")
        .arg(command)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    shell_cmd.process_group(0);
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from)
        && let Some(cargo_bin) = cargo_bin_dir(&home)
    {
        let inherited = std::env::var_os("PATH").unwrap_or_default();
        shell_cmd.env("PATH", child_path(&inherited, &cargo_bin));
    }
    // T144: the driver's inherited env can carry a SHARED target dir (the
    // orchestrator's per-invocation `CARGO_TARGET_DIR` prefix). A shell that
    // inherits it builds/tests into a dir other checkouts also write — and
    // cargo artifact filenames exclude the checkout path, so the goal gate
    // could execute a foreign worktree's binary. Scrub both spellings (see
    // [`TARGET_DIR_VARS`]); an explicit prefix inside `command` still wins.
    scrub_target_dir_vars(&mut shell_cmd);
    let mut child = shell_cmd
        .spawn()
        .with_context(|| format!("spawning sh -c {command}"))?;

    // Readers forward chunks over a channel instead of being joined, so a
    // stuck reader (orphan holding the pipe) can never block the caller —
    // and every chunk read before the grace cutoff is already on its way to
    // the receive side (T185), not held hostage in the reader's local buffer.
    let (out_tx, out_rx) = mpsc::channel::<Vec<u8>>();
    let (err_tx, err_rx) = mpsc::channel::<Vec<u8>>();
    let out_pipe = child.stdout.take().context("stdout not captured")?;
    let err_pipe = child.stderr.take().context("stderr not captured")?;
    thread::spawn(move || drain_pipe(out_pipe, out_tx));
    thread::spawn(move || drain_pipe(err_pipe, err_tx));

    let deadline = Instant::now() + timeout;
    let mut timed_out = false;
    let exit_code;
    loop {
        match child.try_wait()? {
            Some(status) => {
                exit_code = status.code();
                break;
            }
            None => {
                if Instant::now() >= deadline {
                    kill_process_group(&mut child);
                    let _ = child.wait();
                    exit_code = None;
                    timed_out = true;
                    break;
                }
                thread::sleep(Duration::from_millis(50));
            }
        }
    }

    // Both capped waits run concurrently so total wait <= READER_GRACE.
    let out_waiter = thread::spawn(move || recv_capped(out_rx));
    let err_waiter = thread::spawn(move || recv_capped(err_rx));
    let (stdout, out_drained) = out_waiter.join().unwrap_or((Vec::new(), true));
    let (stderr, err_drained) = err_waiter.join().unwrap_or((Vec::new(), true));
    let mut body = combine_out_err(&stdout, &stderr);
    if !out_drained || !err_drained {
        let note = if timed_out {
            "(output truncated: reader did not drain after kill)"
        } else {
            "(output truncated: reader did not drain)"
        };
        if !body.is_empty() {
            body.push('\n');
        }
        body.push_str(note);
    }
    let output = if timed_out {
        format!(
            "timed out after {}s (process group killed)\n{body}",
            timeout.as_secs()
        )
    } else {
        body
    };
    Ok(ShellOutcome {
        exit_code,
        output,
        timed_out,
    })
}

/// The cargo toolchain directory (relative to `$HOME`) prepended to
/// bash-tool children (`T4`), when it exists. The goal-check rejection
/// message (T9) quotes this path back to the model, so the two never drift.
pub(crate) const CARGO_BIN_REL: &str = ".cargo/bin";

/// The cargo toolchain directory to prepend to bash-tool children (`T4`),
/// when it exists under `home`.
fn cargo_bin_dir(home: &Path) -> Option<PathBuf> {
    let dir = home.join(CARGO_BIN_REL);
    dir.is_dir().then_some(dir)
}

/// The child's PATH value: `cargo_bin` prepended to `inherited`, unless
/// `inherited` already contains it. Never removes or reorders existing
/// entries, so an explicitly-set PATH is preserved.
fn child_path(inherited: &OsStr, cargo_bin: &Path) -> OsString {
    let cargo_str = cargo_bin.to_string_lossy();
    if inherited.to_string_lossy().split(':').any(|p| p == cargo_str) {
        return inherited.to_os_string();
    }
    let mut path = OsString::from(cargo_str.as_ref());
    if !inherited.is_empty() {
        path.push(":");
        path.push(inherited);
    }
    path
}

/// Kill the process group led by `pid` (the child is the group leader via
/// `process_group(0)`). Shared with `mcp.rs` through [`kill_process_group`]
/// and with the `delegate` end-to-end test, which only has the pid — the
/// tool detaches and drops the handle on purpose.
#[cfg(unix)]
pub(crate) fn kill_pid_group(pid: u32) {
    // Negative pid targets the entire process group.
    unsafe {
        libc::kill(-(pid as i32), libc::SIGKILL);
    }
}

/// Kill the child's whole process group (the child is the group leader via
/// `process_group(0)`), falling back to killing just the direct child on
/// platforms without process groups. Shared with `mcp.rs`, which must honor
/// the same no-orphan discipline.
pub(crate) fn kill_process_group(child: &mut std::process::Child) {
    #[cfg(unix)]
    kill_pid_group(child.id());
    // Belt and braces: also kill the direct child (no-op if the group kill got it).
    let _ = child.kill();
}

/// Read `pipe` to EOF, forwarding every chunk to `tx` AS IT ARRIVES (T185):
/// a reader blocked on a pipe held open by an escaped grandchild must not
/// hold already-read bytes hostage in its local buffer, so each completed
/// read is handed over immediately. Dropping `tx` at EOF is the clean-drain
/// signal (the channel disconnects). A read error is treated as EOF — the
/// same ignore-the-error shape `read_to_end` had, with `Interrupted` retried.
fn drain_pipe(mut pipe: impl Read, tx: mpsc::Sender<Vec<u8>>) {
    let mut chunk = vec![0u8; 64 * 1024];
    loop {
        match pipe.read(&mut chunk) {
            Ok(0) => break, // EOF
            Ok(n) => {
                let _ = tx.send(chunk[..n].to_vec());
            }
            // read_to_end retried interrupted reads; so does this loop.
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
    }
}

/// Accumulate one reader's forwarded chunks for up to [`READER_GRACE`]; never
/// blocks longer. Returns `(buffer, drained)` where `drained` is false when
/// the grace period expired with the pipe still held open by an escaped
/// process — the chunks received SO FAR are kept (T185), only the tail still
/// in flight beyond the grace is lost. Channel disconnect (the reader dropped
/// its sender at EOF) means a clean drain regardless of how many chunks
/// arrived; a reader that died without ever forwarding yields the same
/// `(empty, drained)` shape the one-send-at-EOF version produced.
fn recv_capped(rx: mpsc::Receiver<Vec<u8>>) -> (Vec<u8>, bool) {
    let deadline = Instant::now() + READER_GRACE;
    let mut acc = Vec::new();
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return (acc, false);
        }
        match rx.recv_timeout(remaining) {
            Ok(chunk) => acc.extend_from_slice(&chunk),
            Err(mpsc::RecvTimeoutError::Timeout) => return (acc, false),
            Err(mpsc::RecvTimeoutError::Disconnected) => return (acc, true),
        }
    }
}

fn combine_out_err(stdout: &[u8], stderr: &[u8]) -> String {
    let mut out = String::from_utf8_lossy(stdout).into_owned();
    let err = String::from_utf8_lossy(stderr);
    if !err.trim().is_empty() {
        if !out.is_empty() {
            out.push_str("\n--- stderr ---\n");
        }
        out.push_str(&err);
    }
    out
}

fn lossy_trimmed(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).trim_end().to_string()
}

/// Keep head + tail when output exceeds the cap, inserting a truncation marker.
pub fn truncate_middle(s: &str, head: usize, tail: usize) -> String {
    if s.chars().count() <= head + tail {
        return s.to_string();
    }
    let head_text: String = s.chars().take(head).collect();
    let tail_text: String = s
        .chars()
        .rev()
        .take(tail)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("{head_text}\n...[output truncated]...\n{tail_text}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_safety_rejects_parent_traversal() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(resolve_safe(tmp.path(), "../x").is_err());
    }

    #[test]
    fn path_safety_rejects_deep_traversal() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(resolve_safe(tmp.path(), "a/../../x").is_err());
    }

    #[test]
    fn path_safety_rejects_absolute_outside() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(resolve_safe(tmp.path(), "/etc/passwd").is_err());
    }

    #[test]
    fn path_safety_accepts_relative_inside() {
        let tmp = tempfile::tempdir().unwrap();
        let resolved = resolve_safe(tmp.path(), "src/lib.rs").unwrap();
        assert_eq!(resolved, tmp.path().join("src/lib.rs"));
    }

    #[test]
    fn path_safety_accepts_absolute_inside() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("f.txt");
        let resolved = resolve_safe(tmp.path(), target.to_str().unwrap()).unwrap();
        assert_eq!(resolved, target);
    }

    #[test]
    fn path_safety_rejects_dotdot_only() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(resolve_safe(tmp.path(), "..").is_err());
    }

    // ---- T134: symlink confinement (codex review 20260928 §2 HIGH) ----
    //
    // `resolve_safe` was lexical-only: an in-tree symlink pointing outside
    // cwd passed the `..`/prefix checks and read_file/write_file/edit_file
    // followed it on disk. The review trigger needs no race — a checkout
    // containing `outside -> /some/external/dir` plus a call on
    // `outside/file` escapes. These tests pin the filesystem-level refusal
    // at the resolve_safe core and at the file-tool surfaces, plus the
    // no-false-positive inverse (a link resolving INSIDE cwd keeps working).

    /// In-tree symlink to an external directory: the lexical checks pass,
    /// the real resolution escapes. RED pre-T134.
    #[cfg(unix)]
    #[test]
    fn t134_resolve_safe_rejects_symlink_to_external_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("secret.txt"), "top secret").unwrap();
        std::os::unix::fs::symlink(outside.path(), tmp.path().join("outside")).unwrap();
        // Relative form — the review's trigger.
        assert!(
            resolve_safe(tmp.path(), "outside/secret.txt").is_err(),
            "in-tree symlink to an external dir must be refused"
        );
        // The same escape spelled as an absolute path inside cwd is still a
        // symlink escape, not a lexical one.
        let abs = tmp.path().join("outside/secret.txt");
        assert!(resolve_safe(tmp.path(), abs.to_str().unwrap()).is_err());
        // The refusal happened before any filesystem access.
        assert_eq!(
            fs::read_to_string(outside.path().join("secret.txt")).unwrap(),
            "top secret"
        );
    }

    /// Dangling in-tree symlink pointing outside: a write_file through it
    /// would CREATE the external file (std follows the link on write), so
    /// the refusal cannot wait for the target to exist. RED pre-T134.
    #[cfg(unix)]
    #[test]
    fn t134_resolve_safe_rejects_dangling_symlink_escape() {
        let tmp = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink("../t134-elsewhere/deep", tmp.path().join("link")).unwrap();
        assert!(resolve_safe(tmp.path(), "link").is_err());
        assert!(resolve_safe(tmp.path(), "link/created.txt").is_err());
    }

    /// A chain of in-tree symlinks (hop1 -> hop2 -> external) must resolve
    /// every hop, not just the first. RED pre-T134.
    #[cfg(unix)]
    #[test]
    fn t134_resolve_safe_rejects_symlink_chain_escape() {
        let tmp = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), tmp.path().join("hop2")).unwrap();
        std::os::unix::fs::symlink("hop2", tmp.path().join("hop1")).unwrap();
        assert!(resolve_safe(tmp.path(), "hop1/f.txt").is_err());
    }

    /// A symlink loop must fail closed, not hang or panic.
    #[cfg(unix)]
    #[test]
    fn t134_resolve_safe_rejects_symlink_loop_fail_closed() {
        let tmp = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink("loop", tmp.path().join("loop")).unwrap();
        assert!(resolve_safe(tmp.path(), "loop/f.txt").is_err());
    }

    /// The inverse pin: links that resolve INSIDE the sandbox keep working —
    /// relative and absolute link targets, and a dangling link whose target
    /// write_file is allowed to create. Guards against over-blocking.
    #[cfg(unix)]
    #[test]
    fn t134_symlink_inside_cwd_still_reads_and_writes() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("real/nested")).unwrap();
        fs::write(tmp.path().join("real/nested/f.txt"), "in-tree").unwrap();
        std::os::unix::fs::symlink("real", tmp.path().join("alias")).unwrap();
        std::os::unix::fs::symlink(tmp.path().join("real"), tmp.path().join("abslink")).unwrap();
        std::os::unix::fs::symlink("real/new.txt", tmp.path().join("dangling-in")).unwrap();

        for p in ["alias/nested/f.txt", "abslink/nested/f.txt"] {
            let resolved = resolve_safe(tmp.path(), p).unwrap();
            assert_eq!(resolved, tmp.path().join(p), "resolved path stays lexical");
        }
        let ctx = ToolCtx {
            cwd: tmp.path().to_path_buf(),
            bash_timeout: Duration::from_secs(BASH_TIMEOUT_SECS),
        };
        let r = dispatch(&ctx, "read_file", &json!({"path": "alias/nested/f.txt"}));
        assert!(!r.is_error, "{}", r.content);
        assert!(r.content.contains("in-tree"), "{}", r.content);
        let r = dispatch(&ctx, "write_file", &json!({"path": "dangling-in", "content": "made"}));
        assert!(!r.is_error, "{}", r.content);
        assert_eq!(fs::read_to_string(tmp.path().join("real/new.txt")).unwrap(), "made");
    }

    /// The tool surfaces must refuse too, not just the core: write_file must
    /// not create/clobber the external file, edit_file and read_file must
    /// not touch it. This is the review's exact read/write/edit trigger.
    #[cfg(unix)]
    #[test]
    fn t134_dispatch_file_tools_reject_symlink_escape() {
        let tmp = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let secret = outside.path().join("secret.txt");
        fs::write(&secret, "original").unwrap();
        std::os::unix::fs::symlink(outside.path(), tmp.path().join("outside")).unwrap();
        let ctx = ToolCtx {
            cwd: tmp.path().to_path_buf(),
            bash_timeout: Duration::from_secs(BASH_TIMEOUT_SECS),
        };

        let r = dispatch(
            &ctx,
            "write_file",
            &json!({"path": "outside/secret.txt", "content": "pwned"}),
        );
        assert!(r.is_error, "write_file must refuse: {}", r.content);
        assert_eq!(fs::read_to_string(&secret).unwrap(), "original", "external file untouched");

        let r = dispatch(
            &ctx,
            "edit_file",
            &json!({"path": "outside/secret.txt", "old": "original", "new": "pwned"}),
        );
        assert!(r.is_error, "edit_file must refuse: {}", r.content);
        assert_eq!(fs::read_to_string(&secret).unwrap(), "original");

        let r = dispatch(&ctx, "read_file", &json!({"path": "outside/secret.txt"}));
        assert!(r.is_error, "read_file must refuse: {}", r.content);
        assert!(!r.content.contains("top secret"), "{}", r.content);
    }

    // ---- T134 F1 (kimi validator follow-up): glob-metachar paths bypassed
    // stage 2. `resolve_safe` treated literal `*`/`**` components as
    // inert-missing (they lexically don't exist, so confine_symlinks' "missing
    // from here down" arm made the tail unreachable), while the `glob` crate
    // FOLLOWS symlinked directories during pattern expansion — so an in-tree
    // `etcdir -> /etc` symlink plus `**/passwd` escaped. The glob tool leaked
    // external NAMES (its safety net was lexical `starts_with`), and tgrep's
    // corpus glob arm leaked external CONTENTS. Fix: strict `resolve_safe`
    // refuses metachar paths, glob patterns resolve through
    // `resolve_glob_pattern` (lexical + literal-prefix stage 2), and every
    // concrete match is re-confined through the full two-stage check before
    // its name is reported or its bytes are read. ----

    /// The validator's exact glob-tool vector: an in-tree symlink to an
    /// external directory plus a `**` pattern must surface no external
    /// names. RED pre-fix (the lexical `starts_with` safety net passed
    /// `etcdir/...` matches straight through).
    #[cfg(unix)]
    #[test]
    fn t134f1_glob_tool_drops_matches_resolving_outside_sandbox() {
        let tmp = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("passwd"), "root:x:0\n").unwrap();
        fs::write(outside.path().join("shadow-secret"), "hash").unwrap();
        std::os::unix::fs::symlink(outside.path(), tmp.path().join("etcdir")).unwrap();
        fs::write(tmp.path().join("keep.txt"), "").unwrap();
        let ctx = ToolCtx {
            cwd: tmp.path().to_path_buf(),
            bash_timeout: Duration::from_secs(BASH_TIMEOUT_SECS),
        };

        // A pattern that can only match through the symlinked dir: no
        // external path may be reported.
        let r = dispatch(&ctx, "glob", &json!({"pattern": "**/passwd"}));
        assert!(!r.is_error, "{}", r.content);
        assert_eq!(r.content, "no matches", "external name leaked: {}", r.content);

        // The validator's name-leak probe (`**/*` listed every external
        // entry — 60 hits probed against /etc): in-tree matches stay, all
        // matches resolving outside are dropped.
        let r = dispatch(&ctx, "glob", &json!({"pattern": "**/*"}));
        assert!(!r.is_error, "{}", r.content);
        assert!(r.content.contains("keep.txt"), "in-tree matches must survive: {}", r.content);
        assert!(!r.content.contains("shadow-secret"), "external name leaked: {}", r.content);
        assert!(!r.content.contains("passwd"), "external name leaked: {}", r.content);
        assert!(!r.content.contains("etcdir/"), "external path leaked: {}", r.content);
    }

    /// A metacharacter in the `path` (base) argument is a PATTERN, not a
    /// literal directory — the glob tool's base must be refused, not
    /// expanded through symlinked dirs. RED pre-fix (`*` expanded to
    /// `etcdir` and reported `etcdir/passwd`).
    #[cfg(unix)]
    #[test]
    fn t134f1_glob_tool_metachar_base_is_refused_not_expanded() {
        let tmp = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("passwd"), "root:x:0\n").unwrap();
        std::os::unix::fs::symlink(outside.path(), tmp.path().join("etcdir")).unwrap();
        let ctx = ToolCtx {
            cwd: tmp.path().to_path_buf(),
            bash_timeout: Duration::from_secs(BASH_TIMEOUT_SECS),
        };
        let r = dispatch(&ctx, "glob", &json!({"pattern": "passwd", "path": "*"}));
        assert!(r.is_error, "metachar base must be refused: {}", r.content);
        assert!(r.content.contains("metachar"), "{}", r.content);
        assert!(!r.content.contains("etcdir/passwd"), "{}", r.content);
    }

    /// Strict `resolve_safe` refuses glob-metachar paths: this tool surface
    /// reads literal paths, and a metachar component is either a mistake or
    /// an expansion attempt. Fail closed regardless of existence — the old
    /// inert-missing treatment is exactly what the glob arm exploited.
    /// RED pre-fix (resolve_safe returned Ok for every case below).
    #[test]
    fn t134f1_resolve_safe_refuses_glob_metachar_paths() {
        let tmp = tempfile::tempdir().unwrap();
        for p in ["a/*/b", "*/x", "**/y", "notes[1].txt", "q?.txt", "*"] {
            let err = resolve_safe(tmp.path(), p).unwrap_err();
            assert!(err.contains("metachar"), "{p}: {err}");
        }
        // Scoping: metachars in the CWD PREFIX (the sandbox root's own
        // name) must not break literal reads — only the model-supplied
        // relative part is checked.
        let weird = tmp.path().join("we[ird]");
        fs::create_dir_all(&weird).unwrap();
        fs::write(weird.join("f.txt"), "ok").unwrap();
        let resolved = resolve_safe(&weird, "f.txt").unwrap();
        assert_eq!(resolved, weird.join("f.txt"));
    }

    /// Class sweep — the single-path surfaces must refuse both vectors:
    /// stage-2 symlink resolution (green pre-fix, pinned per surface) and
    /// the new metachar refusal (RED pre-fix: read_file's message was a
    /// plain ENOENT, list_dir's too — actionable wording only post-fix).
    #[cfg(unix)]
    #[test]
    fn t134f1_list_dir_grep_read_file_refuse_symlink_and_metachar_paths() {
        let tmp = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("f.txt"), "outside-bytes").unwrap();
        std::os::unix::fs::symlink(outside.path(), tmp.path().join("etcdir")).unwrap();
        let ctx = ToolCtx {
            cwd: tmp.path().to_path_buf(),
            bash_timeout: Duration::from_secs(BASH_TIMEOUT_SECS),
        };

        // Symlinked path args: stage 2 refuses before any fs touch.
        for (tool, input) in [
            ("list_dir", json!({"path": "etcdir"})),
            ("grep", json!({"pattern": "outside-bytes", "path": "etcdir"})),
            ("read_file", json!({"path": "etcdir/f.txt"})),
        ] {
            let r = dispatch(&ctx, tool, &input);
            assert!(r.is_error, "{tool} must refuse: {}", r.content);
            assert!(r.content.contains("escapes cwd"), "{tool}: {}", r.content);
            assert!(!r.content.contains("outside-bytes"), "{tool}: {}", r.content);
        }
        // A metachar path arg: refused with the metachar wording, never
        // silently expanded or treated as a missing literal.
        for (tool, input) in [
            ("list_dir", json!({"path": "*"})),
            ("read_file", json!({"path": "*/secret"})),
            ("grep", json!({"pattern": "x", "path": "*.txt"})),
        ] {
            let r = dispatch(&ctx, tool, &input);
            assert!(r.is_error, "{tool} must refuse: {}", r.content);
            assert!(r.content.contains("metachar"), "{tool}: {}", r.content);
        }
    }

    /// T134 F2 (kimi validator): SPEC.md's corrected sandbox claim had no
    /// pin — only README's did — so the doctrine could drift silently
    /// again. Pin the same contract the README pin enforces: file-tool
    /// paths confined including symlink resolution, glob matches confined,
    /// and bash documented as NOT filesystem-confined. RED until SPEC.md
    /// carries the wording.
    #[test]
    fn spec_sandbox_claim_names_symlink_confinement_and_bash_limits() {
        let spec = fs::read_to_string(
            std::env::current_dir()
                .expect("cargo sets the test cwd to the package root")
                .join("SPEC.md"),
        )
        .expect("SPEC.md readable from the crate root");
        let flat: String = spec.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(
            flat.contains(
                "symlinks resolving outside it are refused (T134 — `resolve_safe` resolves the real filesystem"
            ),
            "SPEC.md does not state the symlink-confinement claim: {flat}"
        );
        assert!(
            flat.contains(
                "Glob patterns expand only to matches confined inside it — a match resolving outside via a symlink is dropped, never reported or read"
            ),
            "SPEC.md does not state the T134 F1 glob-confinement claim: {flat}"
        );
        assert!(
            flat.contains("`bash` starts in this directory but is NOT filesystem-confined"),
            "SPEC.md must keep bash's honest limits (drift §3): {flat}"
        );
    }

    #[test]
    fn edit_file_rejects_double_match() {
        assert!(apply_edit("a-b-c-b", "b", "X").is_err());
    }

    #[test]
    fn edit_file_single_match_replaces() {
        assert_eq!(apply_edit("a-b-c", "b", "X").unwrap(), "a-X-c");
    }

    #[test]
    fn edit_file_missing_errors() {
        assert!(apply_edit("abc", "zzz", "X").is_err());
    }

    #[test]
    fn edit_file_empty_old_errors() {
        assert!(apply_edit("abc", "", "X").is_err());
    }

    #[test]
    fn edit_file_replace_all_replaces_every_occurrence() {
        let (out, count) = apply_edit_all("a-b-c-b", "b", "X").unwrap();
        assert_eq!(out, "a-X-c-X");
        assert_eq!(count, 2);
    }

    #[test]
    fn edit_file_replace_all_missing_errors() {
        assert!(apply_edit_all("abc", "zzz", "X").is_err());
    }

    #[test]
    fn edit_file_replace_all_empty_old_errors() {
        assert!(apply_edit_all("abc", "", "X").is_err());
    }

    #[test]
    fn edit_file_dispatch_replace_all_returns_count() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("f.txt"), "x y x y x").unwrap();
        let ctx = ToolCtx { cwd: tmp.path().to_path_buf(), bash_timeout: Duration::from_secs(BASH_TIMEOUT_SECS) };
        let result = dispatch(
            &ctx,
            "edit_file",
            &json!({"path": "f.txt", "old": "y", "new": "z", "replace_all": true}),
        );
        assert!(!result.is_error, "{}", result.content);
        assert!(result.content.contains("replaced 2 occurrences"));
        assert_eq!(fs::read_to_string(tmp.path().join("f.txt")).unwrap(), "x z x z x");
    }

    #[test]
    fn bash_dispatch_honors_ctx_bash_timeout() {
        // The override path must reach run_shell: a 1s ToolCtx timeout kills a
        // 5s sleep at ~1s (not the 120s default), reporting a timeout error.
        // T151 family sweep (tools.rs sibling, req 2): the `elapsed < 4s`
        // bound below is an absolute wall-clock assert of exactly the shape
        // parallel scheduler stretch violates — hold the shared timing lock
        // so no other file's clocked window can overlap ours (same mechanism
        // as the three run_shell legs above). Bound itself unchanged.
        let _timing = crate::testsupport::timing_guard();
        let tmp = tempfile::tempdir().unwrap();
        let ctx = ToolCtx {
            cwd: tmp.path().to_path_buf(),
            bash_timeout: Duration::from_secs(1),
        };
        let start = Instant::now();
        let result = dispatch(&ctx, "bash", &json!({"command": "sleep 5"}));
        let elapsed = start.elapsed();
        assert!(result.is_error);
        assert!(result.content.contains("timed out after 1s"));
        assert!(
            elapsed < Duration::from_secs(4),
            "override not honored: took {elapsed:?}"
        );
    }

    #[test]
    fn edit_file_dispatch_default_still_errors_on_multi_match() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("f.txt"), "x y x y x").unwrap();
        let ctx = ToolCtx { cwd: tmp.path().to_path_buf(), bash_timeout: Duration::from_secs(BASH_TIMEOUT_SECS) };
        let result = dispatch(
            &ctx,
            "edit_file",
            &json!({"path": "f.txt", "old": "y", "new": "z"}),
        );
        assert!(result.is_error);
        assert!(result.content.contains("found 2 times"));
        // File untouched.
        assert_eq!(
            fs::read_to_string(tmp.path().join("f.txt")).unwrap(),
            "x y x y x"
        );
    }

    /// T5: a 2-way non-unique match must report both match line numbers (and
    /// context), so the model can re-anchor instead of reverting the file.
    #[test]
    fn edit_file_multi_match_error_lists_match_lines_with_context() {
        let content = "alpha\nbar\nmid\nbar\nend\n";
        let err = apply_edit(content, "bar", "X").unwrap_err();
        assert!(err.contains("found 2 times"), "{err}");
        assert!(err.contains("matches at lines: 2, 4"), "{err}");
        assert!(err.contains("-- around line 2 --"), "{err}");
        assert!(err.contains("-- around line 4 --"), "{err}");
        // Context shows numbered lines with the match marked.
        assert!(err.contains("> 2 | bar"), "{err}");
        assert!(err.contains("  1 | alpha"), "{err}");
    }

    /// T5: with more matches than the cap, the line list stops at 20 and says
    /// how many were omitted; context still covers only the first 5.
    #[test]
    fn edit_file_multi_match_error_caps_line_list_at_20() {
        let content: String = (0..25)
            .map(|i| format!("line with x here {i}\n"))
            .collect();
        let err = apply_edit(&content, "x", "X").unwrap_err();
        assert!(err.contains("found 25 times"), "{err}");
        assert!(
            err.contains("matches at lines: 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20 (and 5 more)"),
            "{err}"
        );
        assert!(!err.contains(" 25,"), "{err}");
        assert!(err.contains("-- around line 5 --"), "{err}");
        assert!(!err.contains("-- around line 6 --"), "{err}");
    }

    /// T4 helper: cargo bin dir must exist on disk to be prepended.
    #[test]
    fn cargo_bin_dir_only_when_exists() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(cargo_bin_dir(tmp.path()).is_none());
        fs::create_dir_all(tmp.path().join(".cargo/bin")).unwrap();
        assert_eq!(
            cargo_bin_dir(tmp.path()).unwrap(),
            tmp.path().join(".cargo/bin")
        );
    }

    /// T4: the prepend keeps every existing PATH entry (nothing is clobbered).
    #[test]
    fn child_path_prepends_and_preserves_inherited() {
        let tmp = tempfile::tempdir().unwrap();
        let cargo_bin = tmp.path().join(".cargo/bin");
        fs::create_dir_all(&cargo_bin).unwrap();
        let out = child_path(OsStr::new("/usr/bin:/bin"), &cargo_bin);
        assert_eq!(out, OsString::from(format!("{}:/usr/bin:/bin", cargo_bin.display())));
        // Empty inherited PATH: just the cargo dir.
        assert_eq!(child_path(OsStr::new(""), &cargo_bin), OsString::from(cargo_bin.to_string_lossy().as_ref()));
    }

    /// T4: a PATH that already contains ~/.cargo/bin is left untouched.
    #[test]
    fn child_path_noop_when_already_present() {
        let tmp = tempfile::tempdir().unwrap();
        let cargo_bin = tmp.path().join(".cargo/bin");
        let inherited = format!("{}:/usr/bin", cargo_bin.display());
        let out = child_path(OsStr::new(&inherited), &cargo_bin);
        assert_eq!(out, OsString::from(inherited));
    }

    /// T4 regression: a bash tool call for `cargo --version` must succeed with
    /// no PATH prefix in the command — run_shell prepends ~/.cargo/bin itself.
    /// (Skipped on hosts without a cargo checkout, e.g. hermetic CI sandboxes.)
    #[test]
    fn bash_tool_finds_cargo_without_path_prefix() {
        let home = match std::env::var_os("HOME").map(PathBuf::from) {
            Some(h) => h,
            None => return,
        };
        if cargo_bin_dir(&home).is_none() {
            eprintln!("skipping: no ~/.cargo/bin on this host");
            return;
        }
        let tmp = tempfile::tempdir().unwrap();
        let ctx = ToolCtx {
            cwd: tmp.path().to_path_buf(),
            bash_timeout: Duration::from_secs(BASH_TIMEOUT_SECS),
        };
        let result = dispatch(&ctx, "bash", &json!({"command": "cargo --version"}));
        assert!(!result.is_error, "{}", result.content);
        assert!(
            result.content.contains("cargo "),
            "unexpected output: {}",
            result.content
        );
        // The child actually sees the prepended PATH.
        let result = dispatch(&ctx, "bash", &json!({"command": "echo $PATH"}));
        assert!(!result.is_error, "{}", result.content);
        assert!(
            result.content.contains(".cargo/bin"),
            "PATH not prepended: {}",
            result.content
        );
    }

    #[test]
    fn glob_matches_nested_patterns_sorted() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("src/deep")).unwrap();
        fs::write(tmp.path().join("b.rs"), "").unwrap();
        fs::write(tmp.path().join("src/a.rs"), "").unwrap();
        fs::write(tmp.path().join("src/deep/c.rs"), "").unwrap();
        fs::write(tmp.path().join("src/other.txt"), "").unwrap();
        let ctx = ToolCtx { cwd: tmp.path().to_path_buf(), bash_timeout: Duration::from_secs(BASH_TIMEOUT_SECS) };
        let result = dispatch(&ctx, "glob", &json!({"pattern": "**/*.rs"}));
        assert!(!result.is_error, "{}", result.content);
        let lines: Vec<&str> = result.content.lines().collect();
        assert_eq!(lines, vec!["b.rs", "src/a.rs", "src/deep/c.rs"]);
    }

    #[test]
    fn glob_rejects_path_escape() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = ToolCtx { cwd: tmp.path().to_path_buf(), bash_timeout: Duration::from_secs(BASH_TIMEOUT_SECS) };
        let result = dispatch(&ctx, "glob", &json!({"pattern": "../../etc/*"}));
        assert!(result.is_error);
        assert!(result.content.contains("escapes cwd"), "{}", result.content);
        // Absolute pattern outside cwd is rejected too.
        let result = dispatch(&ctx, "glob", &json!({"pattern": "/etc/*"}));
        assert!(result.is_error);
        assert!(result.content.contains("escapes cwd"), "{}", result.content);
    }

    #[test]
    fn glob_cap_truncation_note() {
        let items: Vec<String> = (0..250).map(|i| format!("f{i:03}.txt")).collect();
        let out = format_sorted_capped(items, GLOB_MAX, "matches");
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), GLOB_MAX + 1);
        assert_eq!(lines[GLOB_MAX], "[truncated: showing first 200 of 250]");
        assert_eq!(lines[0], "f000.txt");
        // Under the cap: no note.
        let out = format_sorted_capped(vec!["a".into()], GLOB_MAX, "matches");
        assert_eq!(out, "a");
        let out = format_sorted_capped(Vec::new(), GLOB_MAX, "matches");
        assert_eq!(out, "no matches");
    }

    #[test]
    fn list_dir_dirs_first_with_suffix() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("zfile.txt"), "").unwrap();
        fs::write(tmp.path().join("afile.txt"), "").unwrap();
        fs::create_dir_all(tmp.path().join("zdir")).unwrap();
        fs::create_dir_all(tmp.path().join("adir")).unwrap();
        let ctx = ToolCtx { cwd: tmp.path().to_path_buf(), bash_timeout: Duration::from_secs(BASH_TIMEOUT_SECS) };
        let result = dispatch(&ctx, "list_dir", &json!({}));
        assert!(!result.is_error, "{}", result.content);
        let lines: Vec<&str> = result.content.lines().collect();
        assert_eq!(lines, vec!["adir/", "zdir/", "afile.txt", "zfile.txt"]);
    }

    #[test]
    fn list_dir_subdirectory_and_cap() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("sub")).unwrap();
        fs::write(tmp.path().join("sub/f.txt"), "").unwrap();
        let ctx = ToolCtx { cwd: tmp.path().to_path_buf(), bash_timeout: Duration::from_secs(BASH_TIMEOUT_SECS) };
        let result = dispatch(&ctx, "list_dir", &json!({"path": "sub"}));
        assert!(!result.is_error, "{}", result.content);
        assert_eq!(result.content, "f.txt");

        let lines: Vec<String> = (0..600).map(|i| format!("e{i:03}")).collect();
        let out = cap_lines(lines, LIST_DIR_MAX);
        let shown: Vec<&str> = out.lines().collect();
        assert_eq!(shown.len(), LIST_DIR_MAX + 1);
        assert!(shown[LIST_DIR_MAX].contains("showing first 500 of 600"));
        assert_eq!(cap_lines(Vec::new(), LIST_DIR_MAX), "(empty directory)");
    }

    #[test]
    fn truncate_middle_keeps_head_and_tail() {
        let s = "a".repeat(40_000);
        let out = truncate_middle(&s, 20_000, 10_000);
        assert!(out.contains("...[output truncated]..."));
        assert!(out.len() < 40_000);
    }

    #[test]
    fn truncate_middle_noop_under_cap() {
        assert_eq!(truncate_middle("short", 100, 50), "short");
    }

    // ---- T31: wall-clock-sensitive run_shell tests serialize on one lock ----

    // T151: the T31 lock moved to the ONE shared crate-visible timing
    // domain (crate::testsupport) so the run_shell windows can no longer
    // overlap ANY other file's timing test either (the cycle-70/71 flake
    // storms were all cross-file co-occurrences). The three tests below
    // keep passing byte-identical in behavior: same guard semantics, same
    // elapsed bounds, only the acquisition path changed. The old
    // `RUN_SHELL_TIMING_LOCK` static + `timing_guard` fn were deleted with
    // the move — re-declaring them here would re-introduce a second
    // independent lock (the T151 req-1 finding).
    //
    // The original T31 rationale, still true: each of the three tests
    // asserts an upper bound on real elapsed time around a
    // `timeout + READER_GRACE` window; under parallel load the scheduler
    // can starve a test thread while sibling tests run, stretching
    // `elapsed` past the bound even though run_shell behaved correctly
    // (two one-off sightings, both under parallel load, both green
    // isolated). Holding the shared lock for the clocked window removes
    // that co-occurrence by construction. The elapsed bounds themselves
    // are deliberately untouched: they must keep dying when run_shell
    // stops returning promptly.

    /// Regression: a backgrounded grandchild in the shell's own process group
    /// must be killed with the GROUP at timeout — before the fix only the
    /// direct `sh` child died, the orphan held the stdout pipe, and the reader
    /// join blocked the driver forever. The call must return shortly after
    /// timeout + reader grace, with a timeout error.
    #[test]
    fn run_shell_timeout_kills_process_group_and_returns() {
        // T31: wall-clock ceiling below — hold the timing lock so a sibling
        // timing test cannot stretch `elapsed` (crate::testsupport, T151).
        // Same mechanism as the two named flake sites, so it serializes too.
        let _timing = crate::testsupport::timing_guard();
        let tmp = tempfile::tempdir().unwrap();
        let timeout = Duration::from_secs(1);
        let start = Instant::now();
        let outcome = run_shell(tmp.path(), "sleep 300 & wait", timeout).unwrap();
        let elapsed = start.elapsed();

        assert!(outcome.timed_out);
        assert!(outcome.exit_code.is_none());
        assert!(
            outcome.output.contains("timed out after 1s (process group killed)"),
            "output: {}",
            outcome.output
        );
        // Well under the 300s the orphan would have kept us blocked for; the
        // group kill closed the pipes, so the readers drained immediately
        // (well inside timeout + grace + slack).
        assert!(
            elapsed < timeout + READER_GRACE + Duration::from_secs(5),
            "run_shell blocked for {elapsed:?}"
        );
    }

    /// T151 (new residual mechanism, named in the commit message): the setsid
    /// leg's escapee must boot python3 and call `os.setsid()` BEFORE the 1s
    /// group kill fires. Unpolluted that is ~30ms of CPU against a 1s
    /// deadline — but macOS charges the FIRST exec of an interpreter after a
    /// short idle gap ~1s of wall clock (measured on the dev host, both
    /// Homebrew's and /usr/bin's python3: 0.8–1.2s cold, 0.02–0.05s within
    /// ~3s of a previous exec of the same binary, then cold again). A suite
    /// run usually keeps python3 warm via the mcp stub tests; when the leg
    /// lands outside that window (isolated runs, quiet stretches at default
    /// parallelism) the cold exec LOSES the race to the kill, the escapee
    /// dies WITH its group, the inherited pipe closes, the readers drain,
    /// and the truncation note this test pins never appears — a red shaped
    /// exactly like a real run_shell regression (observed deterministic-red
    /// in isolation). Two mechanisms, both race-eliminating and
    /// constant-preserving:
    /// 1. WARM-UP (primary): an untimed, best-effort `python3 -c "import os"`
    ///    spawn immediately before the timed leg — the leg's exec then sits
    ///    inside the warm window by construction (the gap is spawn+fork
    ///    syscalls, milliseconds; the measured warm window is ~3s). The race
    ///    is eliminated, not widened; no timeout constant moves.
    /// 2. T59-STYLE BOUNDED RETRY (defense in depth): the slow-detach shape
    ///    is DETECTED (`timed_out` + group-killed message + missing
    ///    truncation note — no other outcome produces that triple; a broken
    ///    group kill leaves `sleep 300` alive holding the pipe, so the note
    ///    IS present and the leg is never retried) and the whole leg retries
    ///    with a FRESH escapee, bounded by [`SETSID_DETACH_RETRY_ATTEMPTS`].
    ///    On exhaustion the LAST outcome is returned so the caller's asserts
    ///    keep their byte-identical messages — a genuine regression (python3
    ///    missing, kill broken) still lands red, unmasked. The per-attempt
    ///    `elapsed` is what the caller asserts on, so a retry can never
    ///    stretch the asserted window.
    const SETSID_DETACH_RETRY_ATTEMPTS: usize = 3;

    /// Untimed, best-effort warm-up exec of `program` (see
    /// [`SETSID_DETACH_RETRY_ATTEMPTS`] for the mechanism): a spawn failure
    /// (interpreter absent) just leaves the timed leg exactly as it was.
    fn warm_interpreter_exec(program: &str) {
        let _ = Command::new(program).arg("-c").arg("import os").output();
    }

    /// One setsid-escapee leg with the warm-up + bounded retry-on-slow-detach
    /// (see [`SETSID_DETACH_RETRY_ATTEMPTS`]). Returns the surviving
    /// attempt's outcome plus ITS OWN elapsed window.
    fn run_shell_setsid_escapee_holds_pipe_once(timeout: Duration) -> (ShellOutcome, Duration) {
        warm_interpreter_exec("python3");
        let mut last = None;
        for attempt in 1..=SETSID_DETACH_RETRY_ATTEMPTS {
            let tmp = tempfile::tempdir().unwrap();
            let escapee =
                "python3 -c \"import os, time; os.setsid(); print('held'); time.sleep(60)\"";
            let command = format!("{escapee} & sleep 300");
            let start = Instant::now();
            let outcome = run_shell(tmp.path(), &command, timeout).unwrap();
            let elapsed = start.elapsed();
            let slow_detach = outcome.timed_out
                && outcome
                    .output
                    .contains("timed out after 1s (process group killed)")
                && !outcome
                    .output
                    .contains("(output truncated: reader did not drain after kill)");
            if !slow_detach {
                return (outcome, elapsed);
            }
            eprintln!(
                "run_shell setsid leg: attempt {attempt}/{SETSID_DETACH_RETRY_ATTEMPTS} hit a \
                 slow escapee boot (detached after the group kill — pipe drained, no reader \
                 note, {elapsed:?}); retrying with a fresh escapee"
            );
            last = Some((outcome, elapsed));
        }
        last.expect("retry loop ran at least once")
    }

    /// Regression: a setsid-escaped grandchild keeps the stdout pipe open after
    /// the process group is killed, so the reader never sees EOF. run_shell must
    /// still return (capped reader wait), reporting partial output plus a
    /// truncation note. (macOS ships no `setsid` binary, so the escapee detaches
    /// via python's os.setsid(); the mechanism under test — a session leader
    /// outside the killed group holding the inherited pipe — is identical.)
    #[test]
    fn run_shell_returns_when_setsid_grandchild_holds_pipe() {
        // T31 (named flake): the ceiling below is exactly the shape parallel
        // scheduler stretch violates — hold the timing lock so no sibling
        // timing test's clocked window can overlap ours and starve this
        // thread (crate::testsupport, T151). Bound itself unchanged.
        let _timing = crate::testsupport::timing_guard();
        let timeout = Duration::from_secs(1);
        let (outcome, elapsed) = run_shell_setsid_escapee_holds_pipe_once(timeout);
        assert!(outcome.timed_out);
        assert!(
            outcome
                .output
                .contains("timed out after 1s (process group killed)"),
            "output: {}",
            outcome.output
        );
        // The shell (foreground `sleep 300`) stayed alive past the timeout — the
        // group kill fired — and the setsid escapee kept the pipe open, so the
        // readers hit the grace cap instead of EOF. Must return right around
        // timeout + grace, not hang for the escapee's lifetime.
        assert!(
            elapsed >= timeout,
            "returned before the timeout fired: {elapsed:?}"
        );
        assert!(
            elapsed < timeout + READER_GRACE + Duration::from_secs(3),
            "run_shell blocked for {elapsed:?}"
        );
        assert!(
            outcome
                .output
                .contains("(output truncated: reader did not drain after kill)"),
            "output: {}",
            outcome.output
        );
    }

    /// Fast path sanity: a normal command still completes with its real exit
    /// code and full output (readers drain via the channel without EOF issues).
    #[test]
    fn run_shell_normal_path_unchanged() {
        // T31 (named flake): no explicit elapsed assert here, but the load
        // sensitivity is the same family one level down — the driver's 10s
        // timeout can fire spuriously when this thread is starved between
        // spawn and its next poll, and the suite's own heavyweight timing
        // tests (multi-second kill+grace windows) are the co-occurrence that
        // produced the one-off sighting. Serialize against them (see
        // crate::testsupport, T151); every assertion below is byte-identical.
        let _timing = crate::testsupport::timing_guard();
        let tmp = tempfile::tempdir().unwrap();
        let outcome = run_shell(tmp.path(), "echo hi; exit 3", Duration::from_secs(10)).unwrap();
        assert!(!outcome.timed_out);
        assert_eq!(outcome.exit_code, Some(3));
        // run_shell returns the raw combined output (the bash tool wrapper
        // appends the exit-code line); stdout keeps its trailing newline.
        assert_eq!(outcome.output, "hi\n");
    }

    // ---- T185: already-read output survives a grandchild holding the pipe ----

    // The orphan legs below are each capped at ~READER_GRACE of wall clock by
    // construction (the direct `sh` exits immediately; only the reader grace
    // remains), and each asserts that bound — the same load-sensitivity
    // family as the T31 legs above, so they serialize on the one shared
    // timing domain (crate::testsupport, T151). The `sleep 60` orphans
    // outlive the test on purpose (the shape under test: a process the
    // command failed to redirect keeps the pipe open) and die on their own,
    // mirroring the setsid escapee's bounded `time.sleep(60)` above.

    /// T185 orphan leg, stdout held: a grandchild inherits the stdout pipe and
    /// outlives the direct `sh` (the alarm-killed-cargo/rustc shape — an
    /// alarm kill only SIGKILLs the direct child, and a backgrounded process
    /// the command forgot to redirect does the same on the fast path). The
    /// result must CONTAIN the bytes the command wrote before exiting AND
    /// carry the drain note. RED-proof: against the pre-fix receive path
    /// (`recv_capped`'s timeout leg returned `(Vec::new(), false)`) the
    /// bytes assertion below fails — the whole buffer was discarded.
    #[test]
    fn run_shell_orphan_holding_stdout_keeps_already_read_bytes() {
        let _timing = crate::testsupport::timing_guard();
        let tmp = tempfile::tempdir().unwrap();
        // The orphan's stderr is pointed at /dev/null so ONLY the stdout pipe
        // stays held; stderr drains clean via sh's exit.
        let start = Instant::now();
        let outcome = run_shell(
            tmp.path(),
            "printf 't185-stdout-bytes\\n'; sleep 60 2>/dev/null &",
            Duration::from_secs(10),
        )
        .unwrap();
        let elapsed = start.elapsed();
        assert!(!outcome.timed_out);
        assert_eq!(outcome.exit_code, Some(0));
        // Already-read bytes survive the grace expiry (the RED-proof leg).
        assert!(
            outcome.output.contains("t185-stdout-bytes"),
            "already-read stdout lost: {:?}",
            outcome.output
        );
        // The undrained side still carries the note (exact bytes, T185 req 3).
        assert!(
            outcome
                .output
                .contains("(output truncated: reader did not drain)"),
            "missing drain note: {:?}",
            outcome.output
        );
        // The orphan never EOFs, so the wait is capped at the grace plus
        // scheduler slack — never the orphan's lifetime.
        assert!(
            elapsed < READER_GRACE + Duration::from_secs(5),
            "run_shell blocked for {elapsed:?}"
        );
    }

    /// T185 orphan leg, stderr held (the symmetric shape): the grandchild
    /// keeps only the stderr pipe open. The drained stdout side stays
    /// byte-identical, the stderr side's already-read bytes survive (the
    /// RED-proof leg — pre-fix the stderr buffer was discarded whole), and
    /// the note is present exactly once for the undrained side.
    #[test]
    fn run_shell_orphan_holding_stderr_keeps_already_read_bytes() {
        let _timing = crate::testsupport::timing_guard();
        let tmp = tempfile::tempdir().unwrap();
        // The orphan's stdout is pointed at /dev/null so ONLY the stderr pipe
        // stays held; stdout drains clean via sh's exit.
        let outcome = run_shell(
            tmp.path(),
            "printf 't185-stdout-clean\\n'; printf 't185-stderr-bytes\\n' >&2; \
             sleep 60 >/dev/null &",
            Duration::from_secs(10),
        )
        .unwrap();
        assert!(!outcome.timed_out);
        assert_eq!(outcome.exit_code, Some(0));
        // The clean side is untouched by the fix.
        assert!(
            outcome.output.contains("t185-stdout-clean"),
            "clean stdout lost: {:?}",
            outcome.output
        );
        // Already-read stderr bytes survive the grace expiry (RED-proof leg).
        assert!(
            outcome.output.contains("t185-stderr-bytes"),
            "already-read stderr lost: {:?}",
            outcome.output
        );
        assert!(
            outcome
                .output
                .contains("(output truncated: reader did not drain)"),
            "missing drain note: {:?}",
            outcome.output
        );
    }

    /// T185 fast-path leg: when both sides reach EOF inside the grace, the
    /// output is byte-identical to the pre-fix shape and carries NO note —
    /// the chunked-send restructure must not disturb the clean path.
    #[test]
    fn run_shell_clean_drain_fast_path_has_no_note_and_identical_bytes() {
        let _timing = crate::testsupport::timing_guard();
        let tmp = tempfile::tempdir().unwrap();
        let outcome = run_shell(
            tmp.path(),
            "printf 'out-line\\n'; printf 'err-line\\n' >&2",
            Duration::from_secs(10),
        )
        .unwrap();
        assert!(!outcome.timed_out);
        assert_eq!(outcome.exit_code, Some(0));
        // Byte-identical to the pre-fix fast path — combine_out_err's
        // separator (`"\n--- stderr ---\n"`) after a newline-terminated
        // stdout doubles the newline by long-standing design; the exact
        // bytes pin that the restructure changes nothing on this path.
        assert_eq!(outcome.output, "out-line\n\n--- stderr ---\nerr-line\n");
    }

    /// T185 bound leg: a never-EOF orphan (writes nothing, holds the stdout
    /// pipe forever) must not deadlock or stretch the caller — run_shell
    /// returns within a small multiple of [`READER_GRACE`], with the note
    /// and an empty-but-honest result. The leaked reader thread remains the
    /// accepted tradeoff (blocking the driver is the non-negotiable).
    #[test]
    fn run_shell_never_eof_orphan_returns_within_reader_grace_bound() {
        let _timing = crate::testsupport::timing_guard();
        let tmp = tempfile::tempdir().unwrap();
        let start = Instant::now();
        let outcome = run_shell(
            tmp.path(),
            "sleep 60 2>/dev/null &",
            Duration::from_secs(10),
        )
        .unwrap();
        let elapsed = start.elapsed();
        assert!(!outcome.timed_out);
        assert_eq!(outcome.exit_code, Some(0));
        assert!(
            outcome
                .output
                .contains("(output truncated: reader did not drain)"),
            "missing drain note: {:?}",
            outcome.output
        );
        assert!(
            elapsed < READER_GRACE + Duration::from_secs(5),
            "run_shell blocked for {elapsed:?}"
        );
    }

    // ---- T144: driver-spawned shells must not inherit CARGO_TARGET_DIR ----

    /// The legs below seed the process-global `CARGO_TARGET_DIR` /
    /// `CARGO_BUILD_TARGET_DIR` (the loopd.sh per-invocation prefix shape),
    /// so they serialize on the one lock that already owns process-global
    /// env mutation in this test binary — `DELEGATE_ENV_LOCK` (T129 made it
    /// pub(crate) for exactly this cross-module sharing; the delegate-launch
    /// env leg in delegate::tests seeds the same two variables).
    ///
    /// The invariant (specs/t144-check-harness-target-dir-scrub.md): a shell
    /// the driver spawns must never inherit a target dir other checkouts
    /// also write. Cargo's artifact filename excludes the checkout path, so
    /// an inherited shared dir is last-builder-wins — a worktree's goal gate
    /// can execute a FOREIGN worktree's test binary. The scrub lives in
    /// `run_shell`, the one spawn point shared by the bash tool and the
    /// goal-gate check.
    #[test]
    fn run_shell_scrubs_cargo_target_dir_and_its_alias() {
        let _guard = crate::delegate::tests::DELEGATE_ENV_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::tempdir().unwrap();
        // SAFETY: serialized by DELEGATE_ENV_LOCK (one env, one lock); both
        // process values restored before the asserts (a panic below must not
        // leak the seed into sibling tests).
        let saved_target = std::env::var_os("CARGO_TARGET_DIR");
        let saved_alias = std::env::var_os("CARGO_BUILD_TARGET_DIR");
        unsafe {
            std::env::set_var("CARGO_TARGET_DIR", "/tmp/t144-foreign-shared-target");
            std::env::set_var("CARGO_BUILD_TARGET_DIR", "/tmp/t144-foreign-alias-target");
        }
        let outcome = run_shell(
            tmp.path(),
            "printf '%s|%s' \"${CARGO_TARGET_DIR:-UNSET}\" \"${CARGO_BUILD_TARGET_DIR:-UNSET}\"",
            Duration::from_secs(10),
        )
        .unwrap();
        restore_var("CARGO_TARGET_DIR", saved_target);
        restore_var("CARGO_BUILD_TARGET_DIR", saved_alias);
        assert!(!outcome.timed_out);
        assert_eq!(outcome.exit_code, Some(0));
        // Both spellings arrive unset: after the scrub a bare `cargo test`
        // in the gate builds `<cwd>/target` — content-correct by
        // construction.
        assert_eq!(outcome.output, "UNSET|UNSET");
    }

    /// The explicit in-command prefix must still win: the scrub removes the
    /// INHERITED variable at the spawn boundary and never touches a
    /// `CARGO_TARGET_DIR=… ` prefix inside the command string — the
    /// goal-carried `export …` / per-command prefix is the warm shared-cache
    /// mechanism (T52/T57 role-keyed dirs) and must keep working.
    ///
    /// `printenv`, not `echo`: a POSIX shell expands `$VAR` before a leading
    /// assignment of the same simple command takes effect, so
    /// `CARGO_TARGET_DIR=x echo $CARGO_TARGET_DIR` prints the stale value in
    /// ANY shell. printenv reads the exec'd child's real environment —
    /// exactly the level the scrub acts on.
    #[test]
    fn run_shell_explicit_in_command_target_dir_prefix_still_wins() {
        let _guard = crate::delegate::tests::DELEGATE_ENV_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::tempdir().unwrap();
        // SAFETY: serialized by DELEGATE_ENV_LOCK; restored before the asserts.
        let saved_target = std::env::var_os("CARGO_TARGET_DIR");
        unsafe { std::env::set_var("CARGO_TARGET_DIR", "/tmp/t144-process-env-value") };
        let outcome = run_shell(
            tmp.path(),
            "CARGO_TARGET_DIR=/tmp/t144-explicit-prefix printenv CARGO_TARGET_DIR",
            Duration::from_secs(10),
        )
        .unwrap();
        restore_var("CARGO_TARGET_DIR", saved_target);
        assert_eq!(outcome.exit_code, Some(0));
        assert_eq!(outcome.output, "/tmp/t144-explicit-prefix\n");
    }

    /// Restore one process env var to its pre-leg value (the T144 legs'
    /// shared undo half — set_var/remove_var are unsafe in edition 2024).
    fn restore_var(key: &str, saved: Option<std::ffi::OsString>) {
        match saved {
            Some(v) => unsafe { std::env::set_var(key, v) },
            None => unsafe { std::env::remove_var(key) },
        }
    }

    /// T69 doc pins (T41/T63 convention): the README delegate paragraph names
    /// the `collect` action with its user-facing semantics, and LOOP-SPEC §2
    /// step 3 carries the adoption sentence (the T23→T24 lesson: a capability
    /// without a doctrine sentence doesn't get called). Whitespace-normalized
    /// so markdown rewrapping cannot unpin them. T97 split the paragraph into
    /// per-action sub-bullets — the collect needle carries the bullet's
    /// ` — ` label/verb separator (proven RED against the pre-T97 paragraph).
    #[test]
    fn readme_and_loop_spec_name_collect() {
        let root = std::env::current_dir().expect("cargo sets the test cwd to the package root");
        let flat = |path: &str| {
            fs::read_to_string(root.join(path))
                .unwrap_or_else(|e| panic!("reading {path} from the crate root: {e}"))
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        };
        let readme = flat("README.md");
        assert!(
            readme.contains("**`collect`** — returns the child's structured result in one bounded, non-blocking read"),
            "README delegate paragraph lost the collect clause: {readme}"
        );
        assert!(
            readme.contains("every git failure degrades to a note, never an error"),
            "README collect clause must name the git degrade: {readme}"
        );
        assert!(
            readme.contains("it never blocks or waits, so long-poll with `status` first"),
            "README collect clause must state the non-blocking contract: {readme}"
        );
        assert!(
            !readme.contains("Two actions: **`launch`**"),
            "stale two-actions phrasing must be gone from the README: {readme}"
        );
        let spec = flat("LOOP-SPEC.md");
        assert!(
            spec.contains("the review's first look is one `delegate{action:\"collect\", cwd, pid}` call"),
            "LOOP-SPEC §2 step 3 lost the collect adoption sentence: {spec}"
        );
    }

    /// T22: the bash description must carry the macOS `timeout` mirage note.
    /// Models reach for GNU `timeout` (absent on macOS) and exit 127 — twice
    /// in one day across two model families — and the one surface every
    /// session of every role sees is the tool description itself. The platform
    /// fact and the `perl -e 'alarm` idiom are pinned against the LIVE schema
    /// (`tool_schemas()`, not a copied literal), so reverting the description
    /// or corrupting the idiom fails here.
    #[test]
    fn bash_description_warns_macos_has_no_timeout_and_pins_perl_alarm_idiom() {
        let schemas = tool_schemas();
        let entries: Vec<&Value> = schemas
            .iter()
            .filter(|s| s.get("name").and_then(Value::as_str) == Some("bash"))
            .collect();
        assert_eq!(entries.len(), 1, "exactly one bash schema");
        let desc = entries[0]
            .get("description")
            .and_then(Value::as_str)
            .expect("bash schema has a description");
        // The warning, in its warning context (not just any `timeout` token —
        // the driver's own cap mentions that word too), and the idiom, whose
        // load-bearing prefix is `perl -e 'alarm`.
        assert!(
            desc.contains("no `timeout` command"),
            "bash description lost the macOS `timeout` warning: {desc}"
        );
        assert!(
            desc.contains("perl -e 'alarm"),
            "bash description lost the `perl -e 'alarm` idiom: {desc}"
        );
        // Appended as exactly ONE sentence (3 → 4); T181's BSD-sed sibling
        // later made it 4 → 5, placed before this note so it stays final.
        assert_eq!(
            desc.split(". ").count(),
            5,
            "description did not gain exactly one sentence: {desc}"
        );
        assert!(desc.ends_with("instead."), "note is not the final sentence: {desc}");
        // The 120s phrase is the DRIVER's kill cap, not the model's command
        // budget — that wording must survive untouched.
        assert!(
            desc.contains("120s timeout; long output is truncated"),
            "driver 120s cap wording changed: {desc}"
        );
    }

    /// T181: the bash description must carry the BSD-sed range-form trap.
    /// The cycle-83 eval caught the T22 pattern one level down: two
    /// orchestrator streams (glm and kimi) reached for GNU sed's `,+N`
    /// range form ("print N lines from the match") on macOS, where sed is
    /// BSD and rejects it (`sed: N: ",+Np`). Both recovered in 1–2
    /// iterations; T22's precedent says a recurring cross-platform trap
    /// seen across BOTH model families gets one sentence in the tool
    /// description — the only universal surface every child sees. The trap
    /// (`,+N`), the platform fact (BSD), and the working alternatives
    /// (`awk` / `sed -n 'N,Mp'` with absolute line numbers) are pinned
    /// against the LIVE schema (`tool_schemas()`, not a copied literal), so
    /// reverting the sentence fails here.
    #[test]
    fn bash_description_pins_bsd_sed_no_gnu_plus_n_range_form() {
        let schemas = tool_schemas();
        let entries: Vec<&Value> = schemas
            .iter()
            .filter(|s| s.get("name").and_then(Value::as_str) == Some("bash"))
            .collect();
        assert_eq!(entries.len(), 1, "exactly one bash schema");
        let desc = entries[0]
            .get("description")
            .and_then(Value::as_str)
            .expect("bash schema has a description");
        // The GNU-only range form, the platform fact, and the working
        // alternatives (with the "absolute line numbers" qualifier that
        // makes the `sed -n 'N,Mp'` form actually portable).
        for token in [
            "`,+N`",
            "BSD",
            "`awk`",
            "`sed -n 'N,Mp'`",
            "absolute line numbers",
        ] {
            assert!(
                desc.contains(token),
                "bash description lost the BSD-sed token {token:?}: {desc}"
            );
        }
    }

    // ---- T26: read_file `offset`/`limit` pagination ----

    /// An `n`-line fixture (line i is `L{i}`, trailing newline) at `dir/big.txt`,
    /// mirroring the >2000-line source files that motivated T26.
    fn write_big_fixture(dir: &Path, n: usize) {
        let body: String = (1..=n).map(|i| format!("L{i}\n")).collect();
        fs::write(dir.join("big.txt"), body).unwrap();
    }

    fn read_dispatch(cwd: &Path, input: Value) -> ToolResult {
        let ctx = ToolCtx {
            cwd: cwd.to_path_buf(),
            bash_timeout: Duration::from_secs(BASH_TIMEOUT_SECS),
        };
        dispatch(&ctx, "read_file", &input)
    }

    /// Lines `a..=b` of the `L{i}` fixture, joined with newlines.
    fn fixture_lines(a: usize, b: usize) -> String {
        (a..=b)
            .map(|i| format!("L{i}"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// T26 default leg (over the cap): with neither param the output must be
    /// byte-identical to pre-T26 — head 2000 lines plus the LEGACY truncation
    /// note. Pinned as one exact string so any note wording drift fails here.
    #[test]
    fn read_file_default_over_cap_is_byte_identical_head_and_legacy_note() {
        let tmp = tempfile::tempdir().unwrap();
        write_big_fixture(tmp.path(), 2894);
        let result = read_dispatch(tmp.path(), json!({"path": "big.txt"}));
        assert!(!result.is_error, "{}", result.content);
        let expected = format!(
            "{}\n\n[truncated: showing lines 1-2000 of 2894]",
            fixture_lines(1, 2000)
        );
        assert_eq!(result.content, expected);
    }

    /// T26 default leg (under the cap): the whole file verbatim, no note.
    #[test]
    fn read_file_default_under_cap_returns_file_verbatim() {
        let tmp = tempfile::tempdir().unwrap();
        write_big_fixture(tmp.path(), 3);
        let result = read_dispatch(tmp.path(), json!({"path": "big.txt"}));
        assert!(!result.is_error, "{}", result.content);
        assert_eq!(result.content, "L1\nL2\nL3\n");
    }

    /// T26 window leg: `offset: 2001` (no `limit`) starts at the file's true
    /// line 2001 and runs to EOF, with a note naming the real window.
    #[test]
    fn read_file_offset_pages_to_true_window_through_eof() {
        let tmp = tempfile::tempdir().unwrap();
        write_big_fixture(tmp.path(), 2894);
        let result = read_dispatch(tmp.path(), json!({"path": "big.txt", "offset": 2001}));
        assert!(!result.is_error, "{}", result.content);
        assert!(
            result.content.starts_with("L2001\n"),
            "window must start at the file's line 2001: {}",
            result.content.lines().next().unwrap_or_default()
        );
        let expected = format!(
            "{}\n\n[showing lines 2001-2894 of 2894]",
            fixture_lines(2001, 2894)
        );
        assert_eq!(result.content, expected);
    }

    /// T26 window leg: `offset`+`limit` shows exactly that slice, note names it.
    #[test]
    fn read_file_offset_limit_shows_exact_window() {
        let tmp = tempfile::tempdir().unwrap();
        write_big_fixture(tmp.path(), 2894);
        let result = read_dispatch(
            tmp.path(),
            json!({"path": "big.txt", "offset": 2001, "limit": 50}),
        );
        assert!(!result.is_error, "{}", result.content);
        let expected = format!(
            "{}\n\n[showing lines 2001-2050 of 2894]",
            fixture_lines(2001, 2050)
        );
        assert_eq!(result.content, expected);
    }

    /// T26: `limit` may exceed the 2000-line cap (explicit paging is the
    /// point), and a window covering the whole file gets NO note.
    #[test]
    fn read_file_limit_may_exceed_cap_whole_file_window_has_no_note() {
        let tmp = tempfile::tempdir().unwrap();
        write_big_fixture(tmp.path(), 2894);
        let result = read_dispatch(
            tmp.path(),
            json!({"path": "big.txt", "offset": 1, "limit": 5000}),
        );
        assert!(!result.is_error, "{}", result.content);
        assert_eq!(result.content, fixture_lines(1, 2894));
        assert!(!result.content.contains("[showing lines"));
        assert!(!result.content.contains("[truncated:"));
    }

    /// T26: `limit` alone pages from the top (offset defaults to 1).
    #[test]
    fn read_file_limit_alone_pages_from_the_top() {
        let tmp = tempfile::tempdir().unwrap();
        write_big_fixture(tmp.path(), 10);
        let result = read_dispatch(tmp.path(), json!({"path": "big.txt", "limit": 3}));
        assert!(!result.is_error, "{}", result.content);
        assert_eq!(result.content, "L1\nL2\nL3\n\n[showing lines 1-3 of 10]");
    }

    /// T26 edge: `offset: 0` is a tool error naming the 1-based convention.
    #[test]
    fn read_file_offset_zero_is_tool_error_naming_one_based_convention() {
        let tmp = tempfile::tempdir().unwrap();
        write_big_fixture(tmp.path(), 10);
        let result = read_dispatch(tmp.path(), json!({"path": "big.txt", "offset": 0}));
        assert!(result.is_error);
        assert!(
            result.content.contains("1-based"),
            "error must name the 1-based convention: {}",
            result.content
        );
    }

    /// T26 edge: `offset` past EOF is NOT an error — short content naming the
    /// file length, so paging loops stop cleanly instead of crashing.
    #[test]
    fn read_file_offset_past_eof_names_length_without_error() {
        let tmp = tempfile::tempdir().unwrap();
        write_big_fixture(tmp.path(), 2894);
        let result = read_dispatch(tmp.path(), json!({"path": "big.txt", "offset": 3000}));
        assert!(!result.is_error, "{}", result.content);
        assert!(result.content.contains("2894"), "{}", result.content);
    }

    /// T26 edge (EOF boundary): `offset` == line_count shows the LAST line —
    /// the EOF test is strictly greater-than, not `>=`.
    #[test]
    fn read_file_offset_at_last_line_shows_it() {
        let tmp = tempfile::tempdir().unwrap();
        write_big_fixture(tmp.path(), 2894);
        let result = read_dispatch(tmp.path(), json!({"path": "big.txt", "offset": 2894}));
        assert!(!result.is_error, "{}", result.content);
        assert_eq!(
            result.content,
            "L2894\n\n[showing lines 2894-2894 of 2894]"
        );
    }

    /// T26 edge: `limit: 1` shows exactly one line.
    #[test]
    fn read_file_limit_one_shows_exactly_one_line() {
        let tmp = tempfile::tempdir().unwrap();
        write_big_fixture(tmp.path(), 2894);
        let result = read_dispatch(
            tmp.path(),
            json!({"path": "big.txt", "offset": 7, "limit": 1}),
        );
        assert!(!result.is_error, "{}", result.content);
        assert_eq!(result.content, "L7\n\n[showing lines 7-7 of 2894]");
    }

    /// T26 schema pin (T22 convention): the LIVE `tool_schemas()` read_file
    /// entry carries integer `offset`/`limit` properties, documented, and
    /// `required` stays exactly `["path"]` — the params are optional.
    #[test]
    fn read_file_schema_pins_optional_offset_and_limit() {
        let schemas = tool_schemas();
        let entries: Vec<&Value> = schemas
            .iter()
            .filter(|s| s.get("name").and_then(Value::as_str) == Some("read_file"))
            .collect();
        assert_eq!(entries.len(), 1, "exactly one read_file schema");
        let schema = &entries[0];
        let props = schema
            .get("input_schema")
            .and_then(|s| s.get("properties"))
            .expect("input_schema.properties");
        for key in ["offset", "limit"] {
            let prop = props
                .get(key)
                .unwrap_or_else(|| panic!("{key} property missing from read_file schema"));
            assert_eq!(
                prop.get("type").and_then(Value::as_str),
                Some("integer"),
                "{key} must be typed integer"
            );
        }
        let required: Vec<&str> = schema["input_schema"]["required"]
            .as_array()
            .expect("required list")
            .iter()
            .filter_map(Value::as_str)
            .collect();
        assert_eq!(
            required,
            vec!["path"],
            "required must stay exactly [path] (offset/limit optional)"
        );
        let desc = schema
            .get("description")
            .and_then(Value::as_str)
            .expect("description");
        assert!(
            desc.contains("Use `offset`/`limit` to page beyond the cap."),
            "description lost the paging clause: {desc}"
        );
    }

    // ---- T41: candid cwd-confinement wording in the filesystem tool schemas ----

    /// The five filesystem tools whose schemas must name the refusal.
    const CWD_REFUSAL_TOOLS: [&str; 5] = ["read_file", "write_file", "edit_file", "glob", "list_dir"];

    /// T41: the filesystem tools' descriptions must be candid about the cwd
    /// sandbox — paths outside the cwd are REFUSED, naming the live error
    /// string (`path escapes cwd`), with `bash` named as the escape hatch for
    /// cross-tree reads/writes. The bites that motivated this (cycle-16, two in
    /// one run): `read_file /tmp/chug-loop-t37/src/webfetch.rs` while reviewing
    /// a child worktree, and `write_file /tmp/eval-head.md` for scratch space —
    /// each cost an iteration plus a bash-heredoc workaround. The tool
    /// description is the only surface every session of every role sees
    /// (T22 precedent: impl children never read META-SPEC). Pinned against the
    /// LIVE `tool_schemas()` output, not a copied literal, so reverting any one
    /// description to its pre-T41 text fails here.
    #[test]
    fn filesystem_tool_descriptions_name_the_cwd_refusal_and_bash_escape_hatch() {
        let schemas = tool_schemas();
        for name in CWD_REFUSAL_TOOLS {
            let entries: Vec<&Value> = schemas
                .iter()
                .filter(|s| s.get("name").and_then(Value::as_str) == Some(name))
                .collect();
            assert_eq!(entries.len(), 1, "{name}: exactly one schema");
            let desc = entries[0]
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or_else(|| panic!("{name}: schema has a description"));
            assert!(
                desc.contains("refused"),
                "{name}: description does not say outside paths are refused: {desc}"
            );
            assert!(
                desc.contains("path escapes cwd"),
                "{name}: description does not name the live error string: {desc}"
            );
            assert!(
                desc.contains("go through `bash`"),
                "{name}: description does not name the bash escape hatch: {desc}"
            );
            // Appended as exactly ONE sentence, at the end.
            assert!(
                desc.ends_with("go through `bash`."),
                "{name}: confinement clause is not the final sentence: {desc}"
            );
        }
    }

    /// T41 (T22 pattern): each touched description gains exactly ONE sentence
    /// — so the pre-T41 part counts (via `split(". ")`; glob's pre-existing
    /// "(e.g. …)" contributes its own split, folded into the expectation) each
    /// rise by one — and the load-bearing tokens survive the append: caps,
    /// defaults, and `offset`/`limit` notes are unchanged.
    #[test]
    fn filesystem_tool_descriptions_gain_one_sentence_and_keep_load_bearing_tokens() {
        let expected: &[(&str, usize, &[&str])] = &[
            (
                "read_file",
                5,
                &[
                    "Output is capped at 2000 lines",
                    "Use `offset`/`limit` to page beyond the cap.",
                ],
            ),
            (
                "write_file",
                3,
                &["Parent directories are created automatically."],
            ),
            ("edit_file", 3, &["must occur exactly once"]),
            ("glob", 4, &["capped at 200 with a truncation note"]),
            ("list_dir", 3, &["Capped at 500 with a truncation note"]),
        ];
        let schemas = tool_schemas();
        for (name, parts, tokens) in expected.iter() {
            let desc = schemas
                .iter()
                .find(|s| s.get("name").and_then(Value::as_str) == Some(*name))
                .expect("schema present (pinned elsewhere)")
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or_else(|| panic!("{name}: schema has a description"));
            assert_eq!(
                desc.split(". ").count(),
                *parts,
                "{name}: description did not gain exactly one sentence: {desc}"
            );
            for token in tokens.iter() {
                assert!(
                    desc.contains(*token),
                    "{name}: load-bearing token lost: {token:?} — {desc}"
                );
            }
        }
    }

    /// T41: the per-param `path` descriptions stop underselling the sandbox —
    /// each names the refusal and the `bash` escape hatch, and the pre-T41
    /// wording stays ("relative to cwd"; glob's "Optional base directory";
    /// list_dir's "default: cwd").
    #[test]
    fn filesystem_tool_path_param_descriptions_name_the_refusal() {
        let schemas = tool_schemas();
        for name in CWD_REFUSAL_TOOLS {
            let schema = schemas
                .iter()
                .find(|s| s.get("name").and_then(Value::as_str) == Some(name))
                .expect("schema present (pinned elsewhere)");
            let path_desc = schema["input_schema"]["properties"]["path"]
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or_else(|| panic!("{name}: path property has a description"));
            assert!(
                path_desc.contains("refused") && path_desc.contains("path escapes cwd"),
                "{name}: path description does not name the refusal: {path_desc}"
            );
            assert!(
                path_desc.contains("`bash`"),
                "{name}: path description does not name the bash escape hatch: {path_desc}"
            );
            assert!(
                path_desc.contains("relative to cwd"),
                "{name}: path description lost the relative-to-cwd token: {path_desc}"
            );
        }
    }

    /// T41: the error string the new wording names is the LIVE one —
    /// `resolve_safe` fails with the `path escapes cwd: <path>` prefix
    /// (pre-T41 wording, byte-identical; behavior is also pinned by
    /// `glob_rejects_path_escape` and the `path_safety_*` tests). T61: the
    /// message names `bash` as the escape hatch and stays single-line, at
    /// BOTH refusal sites (the `..`-past-root pop and the outside-cwd
    /// prefix check).
    #[test]
    fn resolve_safe_error_names_path_escapes_cwd() {
        let tmp = tempfile::tempdir().unwrap();
        for err in [
            resolve_safe(tmp.path(), "/etc/passwd").unwrap_err(),
            resolve_safe(tmp.path(), "../out.txt").unwrap_err(),
            // Relative cwds reach both refusal sites: leg 3 (`.` + `..`)
            // pops an empty prefix (unreachable with an absolute cwd);
            // leg 4 (`a/b` + `../..`) normalizes into the prefix check.
            resolve_safe(Path::new("."), "../out.txt").unwrap_err(),
            resolve_safe(Path::new("a/b"), "../../out.txt").unwrap_err(),
        ] {
            assert!(err.starts_with("path escapes cwd: "), "{err}");
            assert!(err.contains("bash"), "{err}");
            assert!(!err.contains('\n'), "refusal must stay single-line: {err}");
        }
    }

    /// T41: the README Tools intro must name the sandbox exceptions —
    /// `delegate` (absolute child-worktree paths), `web_fetch` (network, not
    /// filesystem) and, since T180, `web_search` (network too). It said
    /// `delegate` was "the one documented exception", stale the moment T37
    /// landed `web_fetch`, and "two" was stale the moment T180 landed
    /// `web_search` — a cold reader saw the intro contradict the
    /// `web_fetch`/`web_search` paragraphs one screen below. T134 (codex
    /// review 20260928 drift §3): "All paths sandboxed" was FALSE — bash is
    /// not filesystem-confined and symlinks bypassed the old lexical-only
    /// check — so the intro must now name the symlink-confinement guarantee,
    /// bash's honest limits, and all three exceptions, and the old blanket
    /// claim must be gone. Whitespace is normalized so the pin is
    /// independent of markdown line wrapping.
    #[test]
    fn readme_tools_intro_names_the_three_sandbox_exceptions() {
        // T48: cargo runs test binaries with cwd = the package root; the compile-time env! path is wrong under the T47 shared cache (cycle-21) — resolve at runtime.
        let readme = fs::read_to_string(
            std::env::current_dir()
                .expect("cargo sets the test cwd to the package root")
                .join("README.md"),
        )
        .expect("README.md readable from the crate root");
        let flat: String = readme.split_whitespace().collect::<Vec<_>>().join(" ");
        // The corrected file-tool claim: sandboxing includes symlink
        // resolution, and all exceptions stay named.
        assert!(
            flat.contains(
                "All file-tool paths are sandboxed to `--cwd` — `..` traversal, absolute paths outside it, AND symlinks resolving outside it are refused (T134:"
            ),
            "README Tools intro does not state the T134 symlink-confinement claim: {flat}"
        );
        assert!(
            flat.contains(
                "`delegate`, `web_fetch`, and `web_search` remain the three documented exceptions"
            ),
            "README Tools intro does not name the three exceptions: {flat}"
        );
        assert!(
            flat.contains(
                "`web_fetch` and `web_search` are network, not filesystem). `bash` is NOT filesystem-confined: it starts in `--cwd`"
            ),
            "README Tools intro lost the web_fetch/web_search wording or the honest bash limits: {flat}"
        );
        // The stale singular and the stale pair are gone.
        assert!(
            !flat.contains("is the one documented exception"),
            "README still calls delegate the one documented exception: {flat}"
        );
        assert!(
            !flat.contains("remain the two documented exceptions"),
            "README still says two documented exceptions: {flat}"
        );
        assert!(
            !flat.contains("sandbox exceptions stay exactly two"),
            "README still says the sandbox exceptions stay exactly two: {flat}"
        );
        // The pre-T134 blanket claim is gone — it was false.
        assert!(
            !flat.contains("All paths sandboxed to `--cwd`"),
            "README still claims every path (bash included) is sandboxed: {flat}"
        );
    }

    // ---- T91: read_file image leg ----

    /// RFC 4648 §10 test vectors — the hand-rolled encoder pinned before it
    /// is trusted with real payloads.
    #[test]
    fn image_base64_encode_matches_rfc4648_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    /// The extension → media-type map, every supported extension, including
    /// an uppercase one (`X.PNG`): the lowercased extension is what matches.
    #[test]
    fn image_extension_maps_to_media_type_including_uppercase() {
        let tmp = tempfile::tempdir().unwrap();
        for (name, media_type) in [
            ("a.png", "image/png"),
            ("a.jpg", "image/jpeg"),
            ("a.jpeg", "image/jpeg"),
            ("a.gif", "image/gif"),
            ("a.webp", "image/webp"),
            ("X.PNG", "image/png"),
        ] {
            let path = tmp.path().join(name);
            fs::write(&path, b"\x89PNG\r\n\x1a\nfix").unwrap();
            let result = read_dispatch(tmp.path(), json!({"path": name}));
            assert!(!result.is_error, "{name}: {result:?}");
            assert!(
                result.content.contains(media_type),
                "{name}: note must name {media_type}: {}",
                result.content
            );
            assert_eq!(result.images.len(), 1, "{name}: exactly one image block");
            assert_eq!(result.images[0].media_type, media_type, "{name}");
        }
    }

    /// A hand-rolled minimal PNG (real signature + stub body) written by the
    /// test: the image leg returns the short note naming path/bytes/media
    /// type, exactly one image block whose base64 is the exact base64 of the
    /// file's bytes, and no error.
    #[test]
    fn image_fixture_png_returns_note_and_exact_base64_block() {
        let tmp = tempfile::tempdir().unwrap();
        let png: &[u8] = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDRminimal";
        fs::write(tmp.path().join("fixture.png"), png).unwrap();

        let result = read_dispatch(tmp.path(), json!({"path": "fixture.png"}));
        assert!(!result.is_error, "{result:?}");
        assert_eq!(
            result.content,
            format!(
                "[image: {} ({} bytes, image/png)]",
                tmp.path().join("fixture.png").display(),
                png.len()
            )
        );
        assert_eq!(result.images.len(), 1, "exactly one image block");
        assert_eq!(result.images[0].media_type, "image/png");
        assert_eq!(
            result.images[0].data,
            base64_encode(png),
            "the block carries the exact base64 of the file bytes"
        );
    }

    /// A file with an unknown binary extension keeps the text path
    /// byte-identical: same read_to_string output, zero image blocks. This
    /// pin is the RED proof that widening image detection to non-image
    /// extensions (e.g. treating any extension as an image) turns it red.
    #[test]
    fn image_unknown_binary_extension_keeps_text_path_byte_identical() {
        let tmp = tempfile::tempdir().unwrap();
        let body = "plain-ish binary \u{1}\u{2} but valid UTF-8\n";
        fs::write(tmp.path().join("blob.bin"), body).unwrap();

        let result = read_dispatch(tmp.path(), json!({"path": "blob.bin"}));
        assert!(!result.is_error, "{result:?}");
        assert_eq!(result.content, body, "text path byte-identical");
        assert!(
            result.images.is_empty(),
            "an unknown extension never carries image blocks"
        );
    }

    /// The size guard: an image larger than 5 MiB is a tool error naming the
    /// cap (5242880) and the actual size, with zero base64 in the error text.
    #[test]
    fn image_oversize_file_is_tool_error_naming_cap_without_base64() {
        let tmp = tempfile::tempdir().unwrap();
        let oversized = vec![0u8; 5 * 1024 * 1024 + 1];
        fs::write(tmp.path().join("big.png"), &oversized).unwrap();

        let result = read_dispatch(tmp.path(), json!({"path": "big.png"}));
        assert!(result.is_error, "{result:?}");
        assert!(
            result.content.contains("5 MiB"),
            "error names the cap: {}",
            result.content
        );
        assert!(
            result.content.contains("5242880"),
            "error names the cap in bytes: {}",
            result.content
        );
        assert!(
            result.content.contains("5242881"),
            "error names the actual size: {}",
            result.content
        );
        assert!(
            result.content.len() < 300,
            "error stays short — no base64 payload: {}",
            result.content
        );
        assert!(result.images.is_empty(), "no image block on the error leg");
    }

    /// The sandbox gates the image leg too: an image path escaping cwd is
    /// refused (`path escapes cwd` shape) BEFORE any read — the would-be image
    /// outside cwd is never returned.
    #[test]
    fn image_path_escaping_cwd_is_refused_before_any_read() {
        let tmp = tempfile::tempdir().unwrap();
        // A valid image placed OUTSIDE the cwd, reachable only by traversal.
        let outside = tmp.path().parent().unwrap().join("t91-outside.png");
        fs::write(&outside, b"\x89PNG\r\n\x1a\noutside").unwrap();

        let result = read_dispatch(tmp.path(), json!({"path": "../t91-outside.png"}));
        assert!(result.is_error, "{result:?}");
        assert!(
            result.content.contains("path escapes cwd"),
            "refusal shape unchanged: {}",
            result.content
        );
        assert!(
            !result.content.contains("[image:"),
            "the outside image was never read: {}",
            result.content
        );
        assert!(result.images.is_empty());
        let _ = fs::remove_file(&outside);
    }

    // ---- T94: `get_str` miss errors name the received keys (alias self-correction) ----

    /// The exact t88 fumble (cycle-53 eval §2 I3): the Anthropic-canonical
    /// `old_string`/`new_string` aliases sent to `edit_file`, which expects
    /// `old`/`new`. The error must name the missing field AND list the
    /// received keys sorted, so one-iteration self-correction is possible
    /// instead of the model re-sending the same shape three times and
    /// self-reporting a tool bug. Fires before any file CONTENT is touched
    /// (the missing-`old` check precedes the path resolution), so no fixture
    /// file is needed.
    #[test]
    fn received_edit_file_alias_fumble_names_missing_key_and_sorted_received_keys() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = ToolCtx {
            cwd: tmp.path().to_path_buf(),
            bash_timeout: Duration::from_secs(BASH_TIMEOUT_SECS),
        };
        let result = dispatch(
            &ctx,
            "edit_file",
            &json!({"path": "f.txt", "old_string": "a", "new_string": "b"}),
        );
        assert!(result.is_error, "{}", result.content);
        assert!(
            result
                .content
                .contains("missing or non-string field: old"),
            "the missing field is named: {}",
            result.content
        );
        assert!(
            result
                .content
                .contains("(received keys: new_string, old_string, path)"),
            "received keys named in sorted order: {}",
            result.content
        );
    }

    /// Non-object input names the received JSON type instead (the T88 req-3
    /// shape) — every non-object variant.
    #[test]
    fn received_non_object_input_names_the_json_type() {
        for (input, ty) in [
            (json!(["a", "b"]), "array"),
            (json!("just a string"), "string"),
            (json!(null), "null"),
            (json!(42), "number"),
            (json!(true), "boolean"),
        ] {
            let err = get_str(&input, "old").unwrap_err().to_string();
            assert_eq!(
                err,
                format!("missing or non-string field: old (received: {ty})"),
                "input: {input}"
            );
        }
    }

    /// An empty object has no keys to name: `(received keys: none)`.
    #[test]
    fn received_keys_none_for_empty_object() {
        let err = get_str(&json!({}), "old").unwrap_err().to_string();
        assert_eq!(err, "missing or non-string field: old (received keys: none)");
    }

    /// The key is present but not a string — still an object-input miss, so
    /// the received-keys leg applies uniformly (the list shows the key IS
    /// there under the right name, pointing at the value's type).
    #[test]
    fn received_key_present_but_non_string_still_names_received_keys() {
        let err = get_str(&json!({"old": 42, "path": "f.txt"}), "old")
            .unwrap_err()
            .to_string();
        assert_eq!(err, "missing or non-string field: old (received keys: old, path)");
    }

    /// Bound requirement: a 500-key object names the first 12 sorted keys,
    /// then `, … (+N more)` — the error never balloons with the full list.
    #[test]
    fn received_keys_capped_at_twelve_with_more_suffix() {
        let mut obj = serde_json::Map::new();
        for i in 0..500 {
            obj.insert(format!("k{i:03}"), json!(i));
        }
        let err = get_str(&Value::Object(obj), "old").unwrap_err().to_string();
        assert!(
            err.contains(
                "(received keys: k000, k001, k002, k003, k004, k005, k006, k007, k008, k009, k010, k011, … (+488 more))"
            ),
            "{err}"
        );
        assert!(!err.contains("k012"), "list stops at the cap: {err}");
        assert!(!err.contains("k499"), "tail keys omitted: {err}");
    }

    /// Exactly at the cap (12 keys): the full sorted list, no suffix.
    #[test]
    fn received_keys_at_exactly_twelve_have_no_suffix() {
        let mut obj = serde_json::Map::new();
        for i in 0..12 {
            obj.insert(format!("k{i:02}"), json!(i));
        }
        let err = get_str(&Value::Object(obj), "old").unwrap_err().to_string();
        assert_eq!(
            err,
            "missing or non-string field: old (received keys: k00, k01, k02, k03, k04, k05, k06, k07, k08, k09, k10, k11)"
        );
    }

    /// Success path: present-and-string returns the value untouched — no
    /// diagnostic anywhere, end-to-end edit_file still edits byte-identically.
    /// (The byte-identical requirement is enforced suite-wide by the
    /// pre-existing tools.rs tests, all green unmodified.)
    #[test]
    fn received_success_path_is_unchanged_clean_value_no_hint() {
        assert_eq!(get_str(&json!({"old": "x"}), "old").unwrap(), "x");
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("f.txt"), "a b c").unwrap();
        let ctx = ToolCtx {
            cwd: tmp.path().to_path_buf(),
            bash_timeout: Duration::from_secs(BASH_TIMEOUT_SECS),
        };
        let result = dispatch(
            &ctx,
            "edit_file",
            &json!({"path": "f.txt", "old": "b", "new": "X"}),
        );
        assert!(!result.is_error, "{}", result.content);
        assert_eq!(fs::read_to_string(tmp.path().join("f.txt")).unwrap(), "a X c");
    }

    // ---- T145: update_ledger writes through fsatomic::write_atomic ----
    //
    // LEDGER.md is the orchestrator's and every child's external memory,
    // rewritten wholesale by update_ledger on every bookkeeping step. T136
    // routed the transcript and the todo store through the atomic
    // temp+fsync+rename helper but left the ledger on the truncating
    // `fs::write` — a crash mid-write could leave a torn or empty ledger.
    // These tests pin the same crash class closed for the ledger, mirroring
    // the T136 todos.rs test family.

    /// Happy-path regression: the ledger is replaced byte-exactly, the tool
    /// result text is unchanged, and no temp sibling lingers in cwd.
    #[test]
    fn t145_update_ledger_replaces_byte_exactly_and_leaves_no_temp() {
        let tmp = tempfile::tempdir().unwrap();
        let ledger = tmp.path().join("LEDGER.md");
        fs::write(&ledger, "# Ledger\n\n## Done\n- old\n").unwrap();
        let ctx = ToolCtx {
            cwd: tmp.path().to_path_buf(),
            bash_timeout: Duration::from_secs(BASH_TIMEOUT_SECS),
        };
        let new = "# Ledger\n\n## Done\n- new\n\n## Next\n- x\n\n## Blockers\n- none\n";
        let r = dispatch(&ctx, "update_ledger", &json!({"content": new}));
        assert!(!r.is_error, "{}", r.content);
        assert_eq!(r.content, format!("ledger updated ({} bytes)", new.len()));
        assert_eq!(
            fs::read(&ledger).unwrap(),
            new.as_bytes(),
            "the ledger is replaced byte-exactly"
        );
        let entries: Vec<String> = fs::read_dir(tmp.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(entries, vec!["LEDGER.md".to_string()], "no temp sibling: {entries:?}");
    }

    /// RED leg: the write must go through the atomic temp+rename seam. The
    /// pid-suffixed temp path `LEDGER.md.<pid>.tmp` is obstructed with a
    /// directory, so write_atomic's temp `File::create` fails while the live
    /// LEDGER.md itself stays a perfectly writable file — a failure only the
    /// atomic path can reach (the direct `fs::write` never touches the temp
    /// name: pre-fix this call succeeded and destroyed the previous bytes).
    /// The failed atomic write must surface as a tool error and leave the
    /// PREVIOUS ledger byte-intact — no torn file, no empty file.
    #[test]
    fn t145_failed_atomic_write_leaves_previous_ledger_byte_intact() {
        let tmp = tempfile::tempdir().unwrap();
        let ledger = tmp.path().join("LEDGER.md");
        let previous = "# Ledger\n\n## Done\n- previous bytes stay\n";
        fs::write(&ledger, previous).unwrap();
        // Obstruct exactly the temp path write_atomic will use.
        let seam = tmp.path().join(format!("LEDGER.md.{}.tmp", std::process::id()));
        fs::create_dir(&seam).unwrap();
        let ctx = ToolCtx {
            cwd: tmp.path().to_path_buf(),
            bash_timeout: Duration::from_secs(BASH_TIMEOUT_SECS),
        };
        let r = dispatch(&ctx, "update_ledger", &json!({"content": "replacement"}));
        assert!(
            r.is_error,
            "the failed atomic write surfaces as a tool error: {}",
            r.content
        );
        assert_eq!(
            fs::read(&ledger).unwrap(),
            previous.as_bytes(),
            "the failed write left the previous ledger byte-intact"
        );
        // The obstructed temp path was never renamed over the target.
        assert!(seam.is_dir(), "the seam was consumed by a direct write");
        fs::remove_dir(&seam).unwrap();
    }

    /// Mirrors the T136 todos.rs error-injection leg: the ledger path being
    /// a directory makes the write fail; the error propagates as a tool
    /// error (the call errors, the run never aborts), the target is
    /// untouched, and after clearing the fault a clean write replaces the
    /// content with no temp sibling left behind.
    #[test]
    fn t145_failed_write_error_surface_and_recovery_match_t136_pattern() {
        let tmp = tempfile::tempdir().unwrap();
        let ledger = tmp.path().join("LEDGER.md");
        fs::write(&ledger, "# Ledger\n\n## Done\n- old\n").unwrap();
        let backup = tmp.path().join("LEDGER.md.bak");
        fs::rename(&ledger, &backup).unwrap();
        fs::create_dir(&ledger).unwrap();
        let ctx = ToolCtx {
            cwd: tmp.path().to_path_buf(),
            bash_timeout: Duration::from_secs(BASH_TIMEOUT_SECS),
        };
        let r = dispatch(&ctx, "update_ledger", &json!({"content": "second"}));
        assert!(r.is_error, "the failed write surfaces as a tool error");
        assert!(
            r.content.starts_with("tool error: "),
            "the error rides the usual tool-error surface: {}",
            r.content
        );
        assert!(ledger.is_dir(), "the target was never touched by the failed write");
        fs::remove_dir(&ledger).unwrap();
        fs::rename(&backup, &ledger).unwrap();

        // Recovery: a clean write replaces the previous bytes and leaves
        // exactly one file in cwd (no temp sibling).
        let r = dispatch(&ctx, "update_ledger", &json!({"content": "third"}));
        assert!(!r.is_error, "{}", r.content);
        assert_eq!(fs::read(&ledger).unwrap(), b"third");
        let entries: Vec<String> = fs::read_dir(tmp.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(entries, vec!["LEDGER.md".to_string()], "no temp siblings: {entries:?}");
    }
}
