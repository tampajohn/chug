use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::{Read, Seek};
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
/// Verification (`check:`) commands get a more generous ceiling than the bash tool.
pub const CHECK_TIMEOUT_SECS: u64 = 600;
const READ_MAX_LINES: usize = 2000;
const OUTPUT_KEEP_HEAD: usize = 20_000;
const OUTPUT_KEEP_TAIL: usize = 10_000;
const GLOB_MAX: usize = 200;
const LIST_DIR_MAX: usize = 500;

/// T23 (`delegate`): child budget defaults, matching the LOOP-SPEC
/// impl-child template of the loop that motivated the tool.
pub(crate) const DELEGATE_DEFAULT_MAX_ITERS: u64 = 40;
pub(crate) const DELEGATE_DEFAULT_MAX_MINUTES: u64 = 35;
/// `status` reads only the last ≤64 KiB of a child's events log: the file
/// grows unboundedly over a run and every poll must stay bounded.
const DELEGATE_EVENTS_TAIL_BYTES: u64 = 64 * 1024;
/// Same idea for the child's console log before its tail lines are taken.
const DELEGATE_LOG_TAIL_BYTES: u64 = 8 * 1024;
const DELEGATE_LOG_TAIL_LINES: usize = 3;
const DELEGATE_LOG_LINE_MAX: usize = 200;
/// T29: hard cap of the `status` long-poll — the wait is strictly bounded so
/// one call can never outlive the caller's patience (schema maximum too).
const DELEGATE_WAIT_MAX_SECS: u64 = 600;
/// T29: internal poll cadence of the wait loop (spec: 2–5 s). Each poll is
/// the same cheap bounded tail read the instant leg does, so polling often
/// is harmless and returns promptly on a state change.
const DELEGATE_WAIT_POLL: Duration = Duration::from_millis(2500);

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
}

/// JSON schemas for the tools, in registration order.
pub fn tool_schemas() -> Vec<Value> {
    vec![
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
            "description": "Run a shell command via `sh -c` in the working directory. Captures stdout+stderr and the exit code. 120s timeout; long output is truncated (head+tail kept). On macOS there is no `timeout` command; bound long commands with `perl -e 'alarm N; exec @ARGV' <cmd>` instead.",
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
            "description": "Match file paths under the working directory with a glob pattern (e.g. src/**/*.rs). Returns sorted relative paths, capped at 200 with a truncation note. Paths outside the cwd are refused (`path escapes cwd`); cross-tree reads/writes (such as a child worktree in /tmp) go through `bash`.",
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
        json!({
            "name": "delegate",
            "description": "Launch or observe a bounded child `chug run` (e.g. in a worktree you created). action=launch: spawns a detached child with its working directory at `cwd` (absolute), spec/goal/model required, max_iters/max_minutes optional (defaults 40/35), max_tokens optional (child token ceiling; omitted = unlimited); returns immediately with the child pid and the log/events paths — it never waits on the child. action=status: reports the child's liveness (when you pass the `pid` from launch), a summary of its .chug/events.jsonl (state, last_iteration, budget-low/goal/abort flags), and the tail of its console log. Never blocks: launch returns at spawn, status reads tails only. Optionally pass `wait_secs` on status (0/absent = instant, max 600) to block up to that many seconds, returning early when the child's events state changes or its liveness flips to dead.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "action": {"type": "string", "enum": ["launch", "status"], "description": "launch spawns a detached child chug run; status observes a previously launched one"},
                    "cwd": {"type": "string", "description": "Absolute directory the child runs in (launch and status; the worktree you created — NOT confined to your cwd)"},
                    "spec": {"type": "string", "description": "Absolute path to the spec file (launch only, required)"},
                    "goal": {"type": "string", "description": "Goal text for the child (launch only, required)"},
                    "model": {"type": "string", "description": "Model id the child runs with (launch only, required — routing stays your explicit choice)"},
                    "max_iters": {"type": "integer", "description": "Child iteration budget (launch only; default 40)"},
                    "max_minutes": {"type": "integer", "description": "Child wall-clock budget in minutes (launch only; default 35)"},
                    "max_tokens": {"type": "integer", "minimum": 1, "description": "Child token budget: cumulative input+output tokens across the child run (launch only; omitted = no token ceiling)"},
                    "pid": {"type": "integer", "description": "The pid launch returned (status only, optional; omit → liveness is reported unknown)"},
                    "wait_secs": {"type": "integer", "minimum": 0, "maximum": 600, "description": "Seconds to block on status waiting for a child state change, a liveness flip to dead, or this deadline (0/absent = instant; status only — launch rejects it)"}
                },
                "required": ["action", "cwd"]
            }
        }),
        // T37: schema lives in webfetch.rs (single source of truth for the
        // description the model sees), registered here alongside the builtins.
        crate::webfetch::schema(),
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
    ]
}

/// Dispatch a tool call. Internal failures are converted into `is_error` results
/// so the model can see and recover from them.
pub fn dispatch(ctx: &ToolCtx, name: &str, input: &Value) -> ToolResult {
    match inner(ctx, name, input) {
        Ok(result) => result,
        Err(e) => ToolResult {
            content: format!("tool error: {e:#}"),
            is_error: true,
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
        "delegate" => delegate(ctx, input),
        "web_fetch" => crate::webfetch::web_fetch(input),
        "update_ledger" => update_ledger(ctx, input),
        "goal_complete" => Ok(ToolResult {
            content: "goal_complete acknowledged. Verification will run; do not assume acceptance until the loop confirms it.".to_string(),
            is_error: false,
        }),
        other => Ok(ToolResult {
            content: format!("unknown tool: {other}"),
            is_error: true,
        }),
    }
}

fn get_str<'a>(input: &'a Value, key: &str) -> anyhow::Result<&'a str> {
    input
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing or non-string field: {key}"))
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
    })
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
        })
    } else {
        let updated = apply_edit(&data, old, new)
            .map_err(|e| anyhow!("edit_file {}: {e}", path.display()))?;
        fs::write(&path, &updated).with_context(|| format!("writing {}", path.display()))?;
        Ok(ToolResult {
            content: format!("edited {}", path.display()),
            is_error: false,
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
    // Same path-safety rules as the other file tools: rejects `..` traversal
    // and absolute paths outside cwd, lexically, before any globbing happens.
    resolve_safe(&ctx.cwd, &joined.to_string_lossy()).map_err(|e| anyhow!("{e}"))?;
    let matched = glob::glob(&joined.to_string_lossy())
        .map_err(|e| anyhow!("invalid glob pattern: {e}"))?
        .collect::<Result<Vec<PathBuf>, _>>()
        .map_err(|e| anyhow!("glob error: {e}"))?;
    // Safety net: keep only entries lexically inside cwd, reported relative.
    let relative: Vec<String> = matched
        .into_iter()
        .filter(|p| p.starts_with(&ctx.cwd))
        .filter_map(|p| {
            p.strip_prefix(&ctx.cwd)
                .ok()
                .map(|rel| rel.to_string_lossy().into_owned())
        })
        .collect();
    Ok(ToolResult {
        content: format_sorted_capped(relative, GLOB_MAX, "matches"),
        is_error: false,
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
    }
}

fn update_ledger(ctx: &ToolCtx, input: &Value) -> anyhow::Result<ToolResult> {
    let content = get_str(input, "content")?;
    let path = ctx.cwd.join("LEDGER.md");
    fs::write(&path, content).with_context(|| format!("writing {}", path.display()))?;
    Ok(ToolResult {
        content: format!("ledger updated ({} bytes)", content.len()),
        is_error: false,
    })
}

/// T23: `delegate` — launch and observe a bounded child `chug run`.
///
/// Two actions. `launch` spawns a detached child
/// (`<binary> run --spec … --goal … --model … --max-iters … --max-minutes …`,
/// plus `--max-tokens …` only when the caller passes one — T39/T15 parity,
/// child cwd = the caller's `cwd`) and returns as soon as `spawn()` succeeds.
/// The caller supplied the worktree, so worktree creation, building,
/// harvest/merge, and killing the child stay with the caller's bash — this
/// tool only replaces the `nohup … &` line and the ps/tail/jq polling.
/// `status` reports the child's liveness plus a summary of its
/// `.chug/events.jsonl` and the tail of its console log.
///
/// Deliberately repo-agnostic — chug is not married to any one loop, so
/// there is no risk-gate integration here (the laya gate judges `bash`
/// commands only and is default-off) and no opinion about what the child is
/// for.
///
/// Path policy — deliberate exception, the ONLY tool exempt from
/// [`resolve_safe`]: `cwd` and `spec` must be absolute and are NOT confined
/// to the orchestrator's cwd, because children live in `/tmp` worktrees by
/// design; the confinement every other tool enforces would make this tool
/// useless for its one job.
fn delegate(_ctx: &ToolCtx, input: &Value) -> anyhow::Result<ToolResult> {
    // `_ctx` is deliberately unused: delegate is the one tool whose paths are
    // not confined to ctx.cwd (see the path-policy note above).
    match get_str(input, "action")? {
        "launch" => {
            // T29: launch returns at spawn by contract — the wait knob is
            // status-only, and naming that beats silently ignoring it.
            if input.get("wait_secs").is_some() {
                bail!(
                    "delegate: `wait_secs` applies to the status action only — launch returns at spawn and never waits on the child"
                );
            }
            delegate_launch(input)
        }
        "status" => delegate_status(input),
        other => bail!("delegate: unknown action {other:?} (expected \"launch\" or \"status\")"),
    }
}

/// The child's working directory. Absolute, and deliberately not confined to
/// the orchestrator's cwd — the one [`resolve_safe`] exemption (see
/// [`delegate`]).
fn delegate_cwd(input: &Value) -> anyhow::Result<PathBuf> {
    let raw = get_str(input, "cwd")?;
    let cwd = PathBuf::from(raw);
    if !cwd.is_absolute() {
        bail!("delegate: cwd must be an absolute directory, got {raw:?}");
    }
    if !cwd.is_dir() {
        bail!("delegate: cwd is not a directory: {}", cwd.display());
    }
    Ok(cwd)
}

/// The child's spec path: absolute (it is read by the child, whose cwd is the
/// worktree, not by us — a relative path would mean something else there).
fn delegate_spec(input: &Value) -> anyhow::Result<PathBuf> {
    let raw = get_str(input, "spec")?;
    let spec = PathBuf::from(raw);
    if !spec.is_absolute() {
        bail!("delegate: spec must be an absolute path, got {raw:?}");
    }
    Ok(spec)
}

/// The child's argv, in order, shared by `launch` and the tests that pin it.
/// Pure, so the exact flag list is assertable without spawning a process.
///
/// T39: `max_tokens` is the one optional tail — `Some(n)` appends
/// `--max-tokens n` (T15 parity for children), `None` produces the
/// pre-T39 argv byte-for-byte (no flag), so children keep their current
/// no-token-ceiling behavior unless the orchestrator opts in.
fn delegate_child_argv(
    spec: &Path,
    goal: &str,
    model: &str,
    max_iters: u64,
    max_minutes: u64,
    max_tokens: Option<u64>,
) -> Vec<OsString> {
    let mut argv = vec![
        OsString::from("run"),
        OsString::from("--spec"),
        spec.as_os_str().to_os_string(),
        OsString::from("--goal"),
        OsString::from(goal),
        OsString::from("--model"),
        OsString::from(model),
        OsString::from("--max-iters"),
        OsString::from(max_iters.to_string()),
        OsString::from("--max-minutes"),
        OsString::from(max_minutes.to_string()),
    ];
    if let Some(tokens) = max_tokens {
        argv.push(OsString::from("--max-tokens"));
        argv.push(OsString::from(tokens.to_string()));
    }
    argv
}

/// T39: parse the optional launch-only `max_tokens` (the child's cumulative
/// input+output token budget, `chug run --max-tokens` / T15 semantics).
/// Absent → `None` (child runs with no token ceiling, exactly as before).
/// Non-integer → tool error; `< 1` → tool error naming the constraint, so
/// zero/negative never reaches the child (0 would mean "unlimited" on the
/// child's CLI — silently launching an unbounded child when the caller asked
/// for a ceiling of 0 is the failure this guards).
fn delegate_max_tokens(input: &Value) -> anyhow::Result<Option<u64>> {
    let Some(value) = input.get("max_tokens") else {
        return Ok(None);
    };
    let n = value.as_i64().ok_or_else(|| {
        anyhow!("delegate: `max_tokens` must be an integer token count (at least 1)")
    })?;
    if n < 1 {
        bail!("delegate: `max_tokens` must be at least 1, got {n}");
    }
    Ok(Some(n as u64))
}

/// Spawn a detached `chug run` child and return immediately. Never waits on
/// the child — no sleeps, no retries, no waiting anywhere in this function.
fn delegate_launch(input: &Value) -> anyhow::Result<ToolResult> {
    let cwd = delegate_cwd(input)?;
    let spec = delegate_spec(input)?;
    let goal = get_str(input, "goal")?;
    let model = get_str(input, "model")?;
    let max_iters = input
        .get("max_iters")
        .and_then(Value::as_u64)
        .unwrap_or(DELEGATE_DEFAULT_MAX_ITERS);
    let max_minutes = input
        .get("max_minutes")
        .and_then(Value::as_u64)
        .unwrap_or(DELEGATE_DEFAULT_MAX_MINUTES);
    let max_tokens = delegate_max_tokens(input)?;

    // Binary resolution: the test seam wins, else the running chug itself —
    // children run the same binary, exactly like today's template line does.
    let binary = match std::env::var_os("CHUG_DELEGATE_BIN") {
        Some(override_bin) => PathBuf::from(override_bin),
        None => std::env::current_exe().context("resolving the chug binary (current_exe)")?,
    };

    // One fixed log location: `status` and the harvest step find it without a knob.
    let chug_dir = cwd.join(".chug");
    fs::create_dir_all(&chug_dir).with_context(|| format!("creating {}", chug_dir.display()))?;
    let log_path = chug_dir.join("delegate.log");

    let mut cmd = Command::new(&binary);
    cmd.args(delegate_child_argv(
        &spec,
        goal,
        model,
        max_iters,
        max_minutes,
        max_tokens,
    ))
    .current_dir(&cwd)
    .stdin(Stdio::null())
        // stdout AND stderr append to one log file.
        .stdout(Stdio::from(open_append(&log_path)?))
        .stderr(Stdio::from(open_append(&log_path)?));
    // Detached, `nohup … &` parity: the child gets its own process group and
    // ignores SIGHUP, so it survives both the orchestrator exiting and a
    // terminal hangup. Both are unix-only; non-unix falls back to a plain
    // detached spawn (the parent never waits on it either way).
    #[cfg(unix)]
    {
        cmd.process_group(0);
        // Ignored dispositions survive exec, handled ones do not — so the
        // child ends up SIG_IGN-ing SIGHUP without this process changing its
        // own disposition.
        unsafe {
            cmd.pre_exec(|| {
                libc::signal(libc::SIGHUP, libc::SIG_IGN);
                Ok(())
            });
        }
    }
    let child = cmd
        .spawn()
        .with_context(|| format!("spawning chug child: {}", binary.display()))?;
    let pid = child.id();
    // Detached by contract: the handle is dropped immediately, the child is
    // never waited on or reaped here.
    drop(child);

    // T39: the configured token budget is echoed back only when configured —
    // absent, the return text is byte-identical to pre-T39.
    let tokens_note = match max_tokens {
        Some(tokens) => format!(" max_tokens: {tokens}"),
        None => String::new(),
    };
    Ok(ToolResult {
        content: format!(
            "launched: pid {pid}\nlog: {}\nevents: {}\nmodel: {model} max_iters: {max_iters} max_minutes: {max_minutes}{tokens_note}",
            log_path.display(),
            chug_dir.join("events.jsonl").display(),
        ),
        is_error: false,
    })
}

/// Observe a previously launched child. Without `wait_secs` (or with `0`)
/// this is the pre-T29 instant render, byte-identical for identical state
/// (pinned by test); with `wait_secs > 0` it is a bounded long-poll that
/// blocks until the first state change, liveness flip, or deadline.
fn delegate_status(input: &Value) -> anyhow::Result<ToolResult> {
    // Parse the knob before any I/O so a bad value errors instantly even
    // when `cwd` is also bad.
    let wait_secs = parse_wait_secs(input)?;
    let cwd = delegate_cwd(input)?;
    let pid = input.get("pid").and_then(Value::as_u64);
    match wait_secs {
        Some(secs) if secs > 0 => delegate_status_wait(&cwd, pid, secs),
        // Absent and 0 are the same instant behavior — one code path, so the
        // byte-identical guarantee is structural, not hoped for.
        _ => delegate_status_now(&cwd, pid),
    }
}

/// T29: parse the optional `wait_secs` (status only). Absent → `None`; `0` →
/// `Some(0)`; `1..=600` → `Some(n)`. Negative, non-integer, or >600 → tool
/// error. Pinned choice (spec req 4): REJECT, never clamp — same style as
/// `read_file`'s handling of out-of-range params, and a clamped wait would
/// silently wait a different duration than the caller asked for.
fn parse_wait_secs(input: &Value) -> anyhow::Result<Option<u64>> {
    let Some(value) = input.get("wait_secs") else {
        return Ok(None);
    };
    let n = value.as_i64().ok_or_else(|| {
        anyhow!("delegate: `wait_secs` must be an integer number of seconds (0..={DELEGATE_WAIT_MAX_SECS})")
    })?;
    if n < 0 {
        bail!("delegate: `wait_secs` must be >= 0, got {n}");
    }
    // n >= 0 here, so the cast is lossless.
    let n = n as u64;
    if n > DELEGATE_WAIT_MAX_SECS {
        bail!("delegate: `wait_secs` must be at most {DELEGATE_WAIT_MAX_SECS}, got {n}");
    }
    Ok(Some(n))
}

/// The instant leg: bounded tail reads only, no waiting anywhere.
fn delegate_status_now(cwd: &Path, pid: Option<u64>) -> anyhow::Result<ToolResult> {
    let alive = pid.and_then(reap_and_alive);
    let (summary, events_note) = read_events(&cwd.join(".chug").join("events.jsonl"));
    let log_tail = read_log_tail(&cwd.join(".chug").join("delegate.log"));
    Ok(ToolResult {
        content: render_status(&summary, alive, &log_tail, events_note.as_deref()),
        is_error: false,
    })
}

/// One non-blocking read of the child's events tail, shared by both status
/// legs. A missing/unreadable events log is the normal state before a child's
/// first write — reported as an empty summary plus the `events: nothing read`
/// note, never an error (T23 behavior, unchanged).
fn read_events(events_path: &Path) -> (DelegateSummary, Option<String>) {
    match read_tail_lines(events_path, DELEGATE_EVENTS_TAIL_BYTES) {
        Ok(lines) => {
            let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
            (summarize_events(&refs), None)
        }
        Err(e) => (
            DelegateSummary::default(),
            Some(format!("events: nothing read ({e:#})")),
        ),
    }
}

/// T29: the bounded long-poll. Block until the FIRST of:
/// (a) the child's events-derived state changes vs. the snapshot at entry —
///     any [`DelegateSummary`] field differing (new last_event,
///     goal_seen/abort_seen flip, last_iteration advance, max_iters or
///     budget_low_seen appearing) or the events file's creation when it was
///     missing at entry (the launch→build window is exactly this state);
/// (b) the observed liveness flips alive → dead;
/// (c) the deadline elapses (`wait_secs`, already hard-capped at 600).
///
/// Never aborts the run (spec req 5): every internal error leg degrades to
/// the instant-style answer instead of hanging or erroring — a mid-wait read
/// failure returns immediately, and the deadline is honored even when nothing
/// ever changes. The poll cadence is [`DELEGATE_WAIT_POLL`]; each poll is the
/// same bounded tail read the instant leg does.
fn delegate_status_wait(
    cwd: &Path,
    pid: Option<u64>,
    wait_secs: u64,
) -> anyhow::Result<ToolResult> {
    let started = Instant::now();
    let deadline = started + Duration::from_secs(wait_secs);
    let events_path = cwd.join(".chug").join("events.jsonl");
    let log_path = cwd.join(".chug").join("delegate.log");

    let (entry_summary, entry_note) = read_events(&events_path);
    // A successful read is the existence signal: an empty-but-present file
    // still reads Ok with an empty summary, and its first content is then a
    // summary change; a failed read means the file was missing/unreadable at
    // entry, so its first successful read is the creation trigger.
    let entry_existed = entry_note.is_none();
    let entry_alive = pid.and_then(reap_and_alive);

    // The SAME render as the instant leg (same reads), plus the one
    // `waited:` line naming the actual elapsed seconds — the instant leg
    // prints no such line.
    let render = |summary: &DelegateSummary, alive: Option<bool>, note: Option<&str>| {
        let log_tail = read_log_tail(&log_path);
        let mut content = render_status(summary, alive, &log_tail, note);
        content.push_str(&format!("\nwaited: {}s", started.elapsed().as_secs()));
        content
    };

    loop {
        // Deadline first: never sleep past it, never poll past it. One FINAL
        // read before rendering: a change can land inside the last sleep
        // window (the cadence sleep is capped at the remaining time, so
        // nothing polls between the last check and the deadline — always the
        // case when wait_secs ≤ the cadence), and the rendered payload must
        // reflect the true final state, not the entry snapshot. This read IS
        // the instant leg's read, so the payload is "the same payload as the
        // instant leg" at the moment the wait returns.
        if Instant::now() >= deadline {
            let (final_summary, final_note) = read_events(&events_path);
            let final_alive = pid.and_then(reap_and_alive);
            return Ok(ToolResult {
                content: render(&final_summary, final_alive, final_note.as_deref()),
                is_error: false,
            });
        }

        let (now_summary, now_note) = read_events(&events_path);
        let now_alive = pid.and_then(reap_and_alive);
        let now_existed = now_note.is_none();

        // Req 5: an events read failing mid-wait (file deleted, worktree
        // cleaned) degrades to the instant-style answer immediately — never
        // a hang, never an error surfaced to the caller.
        if now_note.is_some() && entry_note.is_none() {
            return Ok(ToolResult {
                content: render(&now_summary, now_alive, now_note.as_deref()),
                is_error: false,
            });
        }
        // Req 2(a): any summary-field diff, or the events file appearing
        // when it was missing at entry.
        let state_changed =
            now_summary != entry_summary || (now_existed && !entry_existed);
        // Req 2(b): the observed liveness flipped alive → dead.
        let liveness_flipped = entry_alive == Some(true) && now_alive == Some(false);
        if state_changed || liveness_flipped {
            return Ok(ToolResult {
                content: render(&now_summary, now_alive, now_note.as_deref()),
                is_error: false,
            });
        }

        // Sleep at the poll cadence, but never past the deadline.
        let remaining = deadline.saturating_duration_since(Instant::now());
        thread::sleep(remaining.min(DELEGATE_WAIT_POLL));
    }
}

/// The parsing/flag logic of `status`, with no I/O: every edge case (empty
/// stream, torn last line, missing fields) is unit-tested through here.
/// Malformed lines are skipped, never fatal — a torn final write must not
/// blind the poll.
fn summarize_events(lines: &[&str]) -> DelegateSummary {
    let mut s = DelegateSummary::default();
    for line in lines {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let Some(obj) = value.as_object() else {
            continue;
        };
        let Some(ev_type) = obj.get("type").and_then(Value::as_str) else {
            continue;
        };
        // `last_event` is the last complete, parsable line — whatever it is.
        s.last_event_type = Some(ev_type.to_string());
        s.last_event_ts = obj.get("ts").and_then(Value::as_str).map(str::to_string);
        match ev_type {
            "run_start" => {
                if let Some(max) = obj.get("max_iters").and_then(Value::as_u64) {
                    s.max_iters = Some(max);
                }
            }
            "iteration" => {
                if let Some(n) = obj.get("n").and_then(Value::as_u64) {
                    s.last_iteration = Some(n);
                }
            }
            "budget_low" => s.budget_low_seen = true,
            "goal" => s.goal_seen = true,
            "abort" => {
                s.abort_seen = true;
                if let Some(reason) = obj.get("reason").and_then(Value::as_str) {
                    s.abort_reason = Some(reason.to_string());
                }
            }
            _ => {}
        }
    }
    s
}

/// What `status` can say about a child's event stream.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct DelegateSummary {
    /// `max_iters` from `run_start`, when that line was seen.
    max_iters: Option<u64>,
    /// `n` of the last `iteration` line.
    last_iteration: Option<u64>,
    last_event_type: Option<String>,
    last_event_ts: Option<String>,
    budget_low_seen: bool,
    goal_seen: bool,
    abort_seen: bool,
    /// `reason` of the abort line, when present.
    abort_reason: Option<String>,
}

impl DelegateSummary {
    /// `starting` = nothing read yet (child may not have written anything);
    /// `running` = events seen, no verdict; `done`/`aborted` = the stream
    /// ended in a goal or an abort.
    fn state(&self) -> &'static str {
        if self.abort_seen {
            "aborted"
        } else if self.goal_seen {
            "done"
        } else if self.last_event_type.is_some() {
            "running"
        } else {
            "starting"
        }
    }
}

/// The plain liveness probe: `kill(pid, 0)` delivers no signal but reports
/// existence (`EPERM` = exists, owned by someone else). No such probe on
/// non-unix → liveness reports unknown there.
///
/// This is the FALLBACK leg only — [`reap_and_alive`] layers the zombie reap
/// (T28) on top for the `status` poll. It stays standalone so its semantics
/// are exactly the pre-T28 ones: alive / not-alive / EPERM-means-alive.
fn process_alive(pid: u64) -> Option<bool> {
    #[cfg(unix)]
    {
        let rc = unsafe { libc::kill(pid as i32, 0) };
        Some(if rc == 0 {
            true
        } else {
            std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
        })
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        None
    }
}

/// The `status` liveness seam: reap our own exited children before probing
/// (T28). Launch drops the child handle without ever waiting on it, so the
/// orchestrator is a parent that never reaps — an exited child stays a
/// ZOMBIE, and `kill(pid, 0)` succeeds on zombies, which had `status`
/// reporting `alive: true` for children whose events stream already said
/// done (cycle-11 eval O2: three `state: done` children, three `alive:
/// true` polls). So before the plain probe, offer the pid a non-blocking
/// wait:
/// - returns `pid`: ours and had exited — now REAPED, truthfully `false`;
/// - returns `0`: ours, still running → plain probe (says alive);
/// - `-1` (`ECHILD`: foreign pid or already reaped — and any other errno):
///   plain probe, semantics UNCHANGED.
///
/// No panic paths: every waitpid leg degrades to the plain probe.
fn reap_and_alive(pid: u64) -> Option<bool> {
    #[cfg(unix)]
    {
        // A pid of 0 (or one too big for i32) is not a child pid — and
        // waitpid(0, …) would mean "any child in our process group", so the
        // reap leg is skipped for it and the plain probe answers unchanged.
        let pid_i = pid as i32;
        if pid_i > 0 {
            let mut status: libc::c_int = 0;
            // SAFETY: waitpid on our own child pid with a valid status
            // pointer; WNOHANG means it never blocks. It can only reap a
            // child of THIS process — a foreign pid just yields ECHILD.
            let rc = unsafe { libc::waitpid(pid_i, &mut status, libc::WNOHANG) };
            if rc == pid_i {
                // Our child had exited; this wait reaped it — it is gone.
                return Some(false);
            }
            // rc == 0 (ours, still running) and rc == -1 (ECHILD, or any
            // other errno) both fall through to the plain probe below.
        }
    }
    process_alive(pid)
}

/// The last `bound` bytes of `path`, as the complete lines inside that
/// window. Both files a child writes grow unboundedly, so `status` reads
/// tails only; a window that starts mid-line drops its first fragment, which
/// is not a complete line.
fn read_tail_lines(path: &Path, bound: u64) -> anyhow::Result<Vec<String>> {
    let mut file = fs::File::open(path).with_context(|| format!("reading {}", path.display()))?;
    let len = file.metadata()?.len();
    let start = len.saturating_sub(bound);
    let mut bytes = Vec::new();
    if start > 0 {
        file.seek(std::io::SeekFrom::Start(start))?;
    }
    file.read_to_end(&mut bytes)?;
    let text = String::from_utf8_lossy(&bytes);
    let mut lines: Vec<&str> = text.lines().collect();
    if start > 0 && !lines.is_empty() {
        lines.remove(0);
    }
    Ok(lines.into_iter().map(str::to_string).collect())
}

/// The child console log's tail: the last ≤3 non-empty lines, each clipped to
/// 200 chars, so a poll sees a crash line without reading the whole log.
/// Unreadable log → empty tail, never an error (the events summary is the
/// primary signal).
fn read_log_tail(path: &Path) -> Vec<String> {
    match read_tail_lines(path, DELEGATE_LOG_TAIL_BYTES) {
        Ok(lines) => lines
            .into_iter()
            .filter(|l| !l.trim().is_empty())
            .rev()
            .take(DELEGATE_LOG_TAIL_LINES)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .map(|l| l.chars().take(DELEGATE_LOG_LINE_MAX).collect())
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// The `status` text body: one `key: value` per line so a poll (model or
/// test) can grep it.
fn render_status(
    summary: &DelegateSummary,
    alive: Option<bool>,
    log_tail: &[String],
    events_note: Option<&str>,
) -> String {
    let mut out = format!("state: {}", summary.state());
    match alive {
        Some(true) => out.push_str("\nalive: true"),
        Some(false) => out.push_str("\nalive: false"),
        None => out.push_str("\nalive: unknown (no pid given)"),
    }
    if let Some(max) = summary.max_iters {
        out.push_str(&format!("\nmax_iters: {max}"));
    }
    match summary.last_iteration {
        Some(n) => out.push_str(&format!("\nlast_iteration: {n}")),
        None => out.push_str("\nlast_iteration: none"),
    }
    match (&summary.last_event_type, &summary.last_event_ts) {
        (Some(t), Some(ts)) => out.push_str(&format!("\nlast_event: {t} {ts}")),
        (Some(t), None) => out.push_str(&format!("\nlast_event: {t}")),
        (None, _) => out.push_str("\nlast_event: none"),
    }
    out.push_str(&format!(
        "\nbudget_low_seen: {}\ngoal_seen: {}\nabort_seen: {}",
        summary.budget_low_seen, summary.goal_seen, summary.abort_seen
    ));
    if let Some(reason) = &summary.abort_reason {
        out.push_str(&format!("\nabort_reason: {reason}"));
    }
    if let Some(note) = events_note {
        out.push_str(&format!("\n{note}"));
    }
    if log_tail.is_empty() {
        out.push_str("\nlog_tail: (none)");
    } else {
        out.push_str("\nlog_tail:");
        for line in log_tail {
            out.push_str(&format!("\n  {line}"));
        }
    }
    out
}

/// Open (creating) `path` for appending — the delegate log is opened twice so
/// stdout and stderr share one file.
fn open_append(path: &Path) -> anyhow::Result<fs::File> {
    fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("opening {}", path.display()))
}

/// Resolve `path` lexically against `cwd`, rejecting anything that escapes it
/// (`..` traversal, absolute paths outside cwd). No filesystem access, no
/// symlink resolution: purely lexical, per spec.
pub fn resolve_safe(cwd: &Path, path: &str) -> Result<PathBuf, String> {
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
                    return Err(format!("path escapes cwd: {path}"));
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
        return Err(format!("path escapes cwd: {path}"));
    }
    Ok(normalized)
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

/// Run `sh -c <command>` in `cwd`, capturing stdout+stderr and the exit code.
/// Drains both pipes on background threads to avoid pipe-buffer deadlock.
///
/// The child's PATH gets `~/.cargo/bin` prepended when that directory exists
/// (T4: cargo is chug's own toolchain; children should never have to discover
/// `cargo: command not found` themselves). An existing PATH is inherited
/// verbatim — the prepend never removes or reorders entries, and a PATH that
/// already contains `~/.cargo/bin` is left untouched.
///
/// The shell runs in its own process group; when `timeout` elapses the whole
/// group is SIGKILLed (a plain `child.kill()` would orphan grandchildren that
/// keep the pipes open and wedge the caller on join). Reader threads are never
/// joined without a deadline: if a reader has not seen EOF after the grace
/// period, whatever output was captured is returned with a truncation note.
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
    let mut child = shell_cmd
        .spawn()
        .with_context(|| format!("spawning sh -c {command}"))?;

    // Readers hand their buffers over a channel instead of being joined, so a
    // stuck reader (orphan holding the pipe) can never block the caller.
    let (out_tx, out_rx) = mpsc::channel::<Vec<u8>>();
    let (err_tx, err_rx) = mpsc::channel::<Vec<u8>>();
    let mut out_pipe = child.stdout.take().context("stdout not captured")?;
    let mut err_pipe = child.stderr.take().context("stderr not captured")?;
    thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = out_pipe.read_to_end(&mut buf);
        let _ = out_tx.send(buf);
    });
    thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = err_pipe.read_to_end(&mut buf);
        let _ = err_tx.send(buf);
    });

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

/// Wait up to [`READER_GRACE`] for one reader buffer; never blocks longer.
/// Returns `(buffer, drained)` where `drained` is false when the grace period
/// expired with the pipe still held open by an escaped process.
fn recv_capped(rx: mpsc::Receiver<Vec<u8>>) -> (Vec<u8>, bool) {
    match rx.recv_timeout(READER_GRACE) {
        Ok(buf) => (buf, true),
        Err(mpsc::RecvTimeoutError::Timeout) => (Vec::new(), false),
        Err(mpsc::RecvTimeoutError::Disconnected) => (Vec::new(), true),
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
    use std::io::Write as _;
    use std::sync::{Mutex, MutexGuard};

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

    /// Serializes the wall-clock-sensitive `run_shell` tests (T31). Each of
    /// the three tests below asserts an upper bound on real elapsed time
    /// around a `timeout + READER_GRACE` window; under `--test-threads=4` the
    /// scheduler can starve a test thread while sibling tests run, stretching
    /// `elapsed` past the bound even though run_shell behaved correctly (two
    /// one-off sightings, both under parallel load, both green isolated).
    /// Holding this lock for the clocked window removes that co-occurrence by
    /// construction: no two timing tests are ever in flight together, so the
    /// only stretch source left is whole-machine starvation, not the suite
    /// itself. std-only — no new dependencies. The elapsed bounds themselves
    /// are deliberately untouched: they must keep dying when run_shell stops
    /// returning promptly.
    static RUN_SHELL_TIMING_LOCK: Mutex<()> = Mutex::new(());

    /// Poison-tolerant acquisition: a panic inside one timing test must not
    /// cascade `PoisonError` failures into its siblings.
    fn timing_guard() -> MutexGuard<'static, ()> {
        RUN_SHELL_TIMING_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }

    /// Regression: a backgrounded grandchild in the shell's own process group
    /// must be killed with the GROUP at timeout — before the fix only the
    /// direct `sh` child died, the orphan held the stdout pipe, and the reader
    /// join blocked the driver forever. The call must return shortly after
    /// timeout + reader grace, with a timeout error.
    #[test]
    fn run_shell_timeout_kills_process_group_and_returns() {
        // T31: wall-clock ceiling below — hold the timing lock so a sibling
        // timing test cannot stretch `elapsed` (see RUN_SHELL_TIMING_LOCK).
        // Same mechanism as the two named flake sites, so it serializes too.
        let _timing = timing_guard();
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
        // thread (see RUN_SHELL_TIMING_LOCK). Bound itself unchanged.
        let _timing = timing_guard();
        let tmp = tempfile::tempdir().unwrap();
        let timeout = Duration::from_secs(1);
        let escapee = "python3 -c \"import os, time; os.setsid(); print('held'); time.sleep(60)\"";
        let command = format!("{escapee} & sleep 300");
        let start = Instant::now();
        let outcome = run_shell(tmp.path(), &command, timeout).unwrap();
        let elapsed = start.elapsed();

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
        // RUN_SHELL_TIMING_LOCK); every assertion below is byte-identical.
        let _timing = timing_guard();
        let tmp = tempfile::tempdir().unwrap();
        let outcome = run_shell(tmp.path(), "echo hi; exit 3", Duration::from_secs(10)).unwrap();
        assert!(!outcome.timed_out);
        assert_eq!(outcome.exit_code, Some(3));
        // run_shell returns the raw combined output (the bash tool wrapper
        // appends the exit-code line); stdout keeps its trailing newline.
        assert_eq!(outcome.output, "hi\n");
    }

    // ---- T23: delegate ----

    fn delegate_ctx(cwd: &Path) -> ToolCtx {
        ToolCtx {
            cwd: cwd.to_path_buf(),
            bash_timeout: Duration::from_secs(BASH_TIMEOUT_SECS),
        }
    }

    /// The tests that mutate `CHUG_DELEGATE_BIN` take this: the env is
    /// process-global and cargo runs test threads in parallel.
    static DELEGATE_ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn delegate_summary_empty_is_starting() {
        let s = summarize_events(&[]);
        assert_eq!(s.state(), "starting");
        assert_eq!(s.max_iters, None);
        assert_eq!(s.last_iteration, None);
        assert_eq!(s.last_event_type, None);
        assert!(!s.budget_low_seen && !s.goal_seen && !s.abort_seen);
    }

    #[test]
    fn delegate_summary_run_start_only_is_running_with_budget() {
        let lines = ["{\"type\":\"run_start\",\"ts\":\"2026-09-25T18:09:35.505Z\",\"mode\":\"run\",\"model\":\"kimi\",\"max_iters\":50,\"max_minutes\":35,\"max_tokens\":null}"];
        let s = summarize_events(&lines);
        assert_eq!(s.state(), "running");
        assert_eq!(s.max_iters, Some(50));
        assert_eq!(s.last_iteration, None);
        assert_eq!(s.last_event_type.as_deref(), Some("run_start"));
        assert_eq!(s.last_event_ts.as_deref(), Some("2026-09-25T18:09:35.505Z"));
        assert!(!s.budget_low_seen && !s.goal_seen && !s.abort_seen);
    }

    #[test]
    fn delegate_summary_mid_run_reports_iteration_and_last_event() {
        let lines = [
            "{\"type\":\"run_start\",\"ts\":\"t0\",\"max_iters\":40}",
            "{\"type\":\"iteration\",\"ts\":\"t1\",\"n\":3,\"input_tokens\":10,\"output_tokens\":5}",
            "{\"type\":\"tool_result\",\"ts\":\"t2\",\"name\":\"bash\",\"ok\":true,\"is_error\":false,\"duration_ms\":12,\"preview\":\"hi\"}",
        ];
        let s = summarize_events(&lines);
        assert_eq!(s.state(), "running");
        assert_eq!(s.max_iters, Some(40));
        assert_eq!(s.last_iteration, Some(3));
        assert_eq!(s.last_event_type.as_deref(), Some("tool_result"));
        assert_eq!(s.last_event_ts.as_deref(), Some("t2"));
        assert!(!s.budget_low_seen && !s.goal_seen && !s.abort_seen);
    }

    #[test]
    fn delegate_summary_budget_low_sets_flag() {
        let lines = [
            "{\"type\":\"run_start\",\"ts\":\"t0\",\"max_iters\":40}",
            "{\"type\":\"iteration\",\"ts\":\"t1\",\"n\":33}",
            "{\"type\":\"budget_low\",\"ts\":\"t2\",\"remaining_iters\":8,\"remaining_secs\":1785,\"remaining_tokens\":null}",
            "{\"type\":\"iteration\",\"ts\":\"t3\",\"n\":34}",
        ];
        let s = summarize_events(&lines);
        assert!(s.budget_low_seen);
        assert_eq!(s.state(), "running");
        assert_eq!(s.last_iteration, Some(34));
        assert!(!s.goal_seen && !s.abort_seen);
    }

    #[test]
    fn delegate_summary_goal_sets_flag_and_done_state() {
        let lines = [
            "{\"type\":\"run_start\",\"ts\":\"t0\",\"max_iters\":40}",
            "{\"type\":\"iteration\",\"ts\":\"t1\",\"n\":4}",
            "{\"type\":\"goal\",\"ts\":\"t2\",\"outcome\":\"accepted\",\"summary\":\"VERDICT PASS\"}",
        ];
        let s = summarize_events(&lines);
        assert!(s.goal_seen);
        assert!(!s.abort_seen && !s.budget_low_seen);
        assert_eq!(s.state(), "done");
        assert_eq!(s.last_event_type.as_deref(), Some("goal"));
    }

    #[test]
    fn delegate_summary_abort_sets_flag_reason_and_state() {
        let lines = [
            "{\"type\":\"run_start\",\"ts\":\"t0\",\"max_iters\":40}",
            "{\"type\":\"iteration\",\"ts\":\"t1\",\"n\":40}",
            "{\"type\":\"abort\",\"ts\":\"t2\",\"reason\":\"iteration budget exceeded\",\"model\":\"glm\",\"budget_kind\":\"iterations\",\"budget_max\":40}",
        ];
        let s = summarize_events(&lines);
        assert!(s.abort_seen);
        assert_eq!(s.abort_reason.as_deref(), Some("iteration budget exceeded"));
        assert_eq!(s.state(), "aborted");
        assert!(!s.goal_seen);
    }

    #[test]
    fn delegate_summary_malformed_lines_skipped_not_fatal() {
        let lines = [
            "{\"type\":\"iteration\",\"ts\":\"t1\",\"n\":7}",
            // A torn final write (partial JSON) and a non-JSON line must be
            // skipped; the summary keeps standing on the complete lines.
            "{\"type\":\"iteration\",\"ts\":\"t2\",\"n\":8,\"trunc",
            "not json at all",
            "",
        ];
        let s = summarize_events(&lines);
        assert_eq!(s.state(), "running");
        assert_eq!(s.last_iteration, Some(7));
        assert_eq!(s.last_event_type.as_deref(), Some("iteration"));
        assert_eq!(s.last_event_ts.as_deref(), Some("t1"));
    }

    #[test]
    fn delegate_status_without_chug_dir_is_starting_not_error() {
        let tmp = tempfile::tempdir().unwrap();
        let result = dispatch(
            &delegate_ctx(tmp.path()),
            "delegate",
            &json!({"action": "status", "cwd": tmp.path()}),
        );
        assert!(!result.is_error, "{}", result.content);
        assert!(result.content.contains("state: starting"), "{}", result.content);
        assert!(result.content.contains("alive: unknown"), "{}", result.content);
        assert!(result.content.contains("last_iteration: none"), "{}", result.content);
        assert!(result.content.contains("goal_seen: false"), "{}", result.content);
        assert!(result.content.contains("log_tail: (none)"), "{}", result.content);
    }

    #[test]
    fn delegate_status_summarizes_synthetic_events_file() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join(".chug")).unwrap();
        fs::write(
            tmp.path().join(".chug/events.jsonl"),
            concat!(
                "{\"type\":\"run_start\",\"ts\":\"t0\",\"mode\":\"run\",\"model\":\"m\",\"max_iters\":50,\"max_minutes\":35,\"max_tokens\":null}\n",
                "{\"type\":\"iteration\",\"ts\":\"t1\",\"n\":7,\"input_tokens\":1,\"output_tokens\":1}\n",
                "{\"type\":\"budget_low\",\"ts\":\"t2\",\"remaining_iters\":8,\"remaining_secs\":100,\"remaining_tokens\":null}\n",
                "{\"type\":\"abort\",\"ts\":\"t3\",\"reason\":\"iteration budget exceeded\",\"model\":\"m\",\"budget_kind\":\"iterations\",\"budget_max\":50}\n",
            ),
        )
        .unwrap();
        // The test process itself is a live pid for the kill(pid, 0) probe.
        let result = dispatch(
            &delegate_ctx(tmp.path()),
            "delegate",
            &json!({"action": "status", "cwd": tmp.path(), "pid": std::process::id()}),
        );
        assert!(!result.is_error, "{}", result.content);
        assert!(result.content.contains("state: aborted"), "{}", result.content);
        assert!(result.content.contains("alive: true"), "{}", result.content);
        assert!(result.content.contains("max_iters: 50"), "{}", result.content);
        assert!(result.content.contains("last_iteration: 7"), "{}", result.content);
        assert!(result.content.contains("last_event: abort"), "{}", result.content);
        assert!(result.content.contains("budget_low_seen: true"), "{}", result.content);
        assert!(result.content.contains("abort_seen: true"), "{}", result.content);
        assert!(
            result.content.contains("abort_reason: iteration budget exceeded"),
            "{}",
            result.content
        );
        assert!(result.content.contains("goal_seen: false"), "{}", result.content);
    }

    /// The deliberate `resolve_safe` exemption: the child worktree lives
    /// outside this process's sandbox (in /tmp by design) and `status` must
    /// work on it anyway.
    #[test]
    fn delegate_paths_may_lie_outside_the_orchestrator_cwd() {
        let ctx_cwd = tempfile::tempdir().unwrap();
        let child_dir = tempfile::tempdir().unwrap();
        let result = dispatch(
            &delegate_ctx(ctx_cwd.path()),
            "delegate",
            &json!({"action": "status", "cwd": child_dir.path()}),
        );
        assert!(!result.is_error, "{}", result.content);
        assert!(result.content.contains("state: starting"), "{}", result.content);
    }

    #[test]
    fn delegate_rejects_relative_cwd_and_nonexistent_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = delegate_ctx(tmp.path());
        let result = dispatch(&ctx, "delegate", &json!({"action": "status", "cwd": "relative/child"}));
        assert!(result.is_error);
        assert!(result.content.contains("absolute directory"), "{}", result.content);
        let result = dispatch(
            &ctx,
            "delegate",
            &json!({"action": "status", "cwd": tmp.path().join("nope")}),
        );
        assert!(result.is_error);
        assert!(result.content.contains("not a directory"), "{}", result.content);
    }

    #[test]
    fn delegate_unknown_action_errors() {
        let tmp = tempfile::tempdir().unwrap();
        let result = dispatch(
            &delegate_ctx(tmp.path()),
            "delegate",
            &json!({"action": "harvest", "cwd": tmp.path()}),
        );
        assert!(result.is_error);
        assert!(result.content.contains("unknown action"), "{}", result.content);
    }

    #[test]
    fn delegate_alive_probe_true_for_own_pid_false_for_reaped_exit() {
        assert_eq!(process_alive(u64::from(std::process::id())), Some(true));
        let mut child = Command::new("true").spawn().unwrap();
        let pid = child.id();
        assert!(child.wait().unwrap().success());
        assert_eq!(process_alive(u64::from(pid)), Some(false));
    }

    // ---- T28: status reaps zombie children ----

    /// T28 test 1. An exited own child is a ZOMBIE until reaped, and the
    /// orchestrator never reaps (launch drops the handle) — `kill(pid, 0)`
    /// answers "alive" for zombies, so `status` reported `alive: true` for
    /// children whose events stream already recorded done (cycle-11 eval O2).
    ///
    /// NON-VACUOUSNESS (T28 spec test item 5): this test FAILS pre-T28 —
    /// with no waitpid leg the zombie keeps answering the probe, so the
    /// bounded loop below times out holding `Some(true)` instead of ever
    /// seeing the `Some(false)` that only the reap produces.
    #[cfg(unix)]
    #[test]
    fn delegate_status_reaps_own_exited_child_and_reports_false_twice() {
        let child = Command::new("true").spawn().unwrap();
        let pid = child.id();
        drop(child); // the launch contract: detached, handle dropped, never waited

        // Wait for the child to exit WITHOUT reaping it ourselves: poll the
        // seam until it reports dead — the reap inside the seam is what turns
        // the zombie into a reaped, truly-gone pid.
        let deadline = Instant::now() + Duration::from_secs(10);
        let first = loop {
            match reap_and_alive(u64::from(pid)) {
                Some(false) => break Some(false),
                other => {
                    if Instant::now() >= deadline {
                        break other;
                    }
                    thread::sleep(Duration::from_millis(10));
                }
            }
        };
        assert_eq!(first, Some(false), "exited own child must reap to dead");
        // Second poll: already reaped, so waitpid says ECHILD and the plain
        // probe (ESRCH) still reports dead — no error, and not alive.
        assert_eq!(reap_and_alive(u64::from(pid)), Some(false));
    }

    /// T28 test 2. A still-running own child stays alive: the reap leg must
    /// not misreport it (waitpid WNOHANG returns 0 → plain probe → true).
    /// Cleanup kills and reaps, so the suite leaks no zombie or stray sleeper.
    #[cfg(unix)]
    #[test]
    fn delegate_status_reports_own_running_child_alive_then_cleans_up() {
        let mut child = Command::new("sleep").arg("30").spawn().unwrap();
        let pid = child.id();
        assert_eq!(reap_and_alive(u64::from(pid)), Some(true));
        child.kill().expect("kill the sleep child");
        assert!(!child.wait().expect("reap the sleep child").success());
    }

    /// T28 test 3. A pid that is NOT our child keeps the plain probe's
    /// semantics exactly: waitpid says ECHILD, so the kill(pid, 0)/EPERM
    /// answer is unchanged from pre-T28 — both for an existing foreign pid
    /// and for a provably dead never-our-child pid.
    #[cfg(unix)]
    #[test]
    fn delegate_status_foreign_pid_keeps_probe_semantics() {
        // pid 1 exists on every unix (init/launchd) and is never our child:
        // alive via the probe's ok/EPERM leg, same answer as pre-T28.
        assert_eq!(reap_and_alive(1), Some(true));
        // Provably dead and never an unreaped child of ours: fully reaped via
        // wait() first, so the seam's waitpid says ECHILD and the plain
        // probe's ESRCH reports false.
        let mut child = Command::new("true").spawn().unwrap();
        let pid = child.id();
        assert!(child.wait().expect("reap the true child").success());
        assert_eq!(reap_and_alive(u64::from(pid)), Some(false));
    }

    /// T28 test 4. Liveness without a pid stays `unknown (no pid given)` —
    /// the reap leg must not leak into the no-pid path.
    #[test]
    fn delegate_status_without_pid_still_reports_alive_unknown() {
        let tmp = tempfile::tempdir().unwrap();
        let result = dispatch(
            &delegate_ctx(tmp.path()),
            "delegate",
            &json!({"action": "status", "cwd": tmp.path()}),
        );
        assert!(!result.is_error, "{}", result.content);
        assert!(
            result.content.contains("alive: unknown (no pid given)"),
            "{}",
            result.content
        );
    }

    // ---- T29: status wait_secs long-poll ----

    /// Synthetic events lines shared by the T29 fixtures (no trailing
    /// newline; the fixture writer adds one per line).
    const T29_RUN_START: &str = "{\"type\":\"run_start\",\"ts\":\"t0\",\"mode\":\"run\",\"model\":\"m\",\"max_iters\":50,\"max_minutes\":35,\"max_tokens\":null}";
    const T29_ITERATION: &str =
        "{\"type\":\"iteration\",\"ts\":\"t1\",\"n\":7,\"input_tokens\":1,\"output_tokens\":1}";

    /// A fixed fake `.chug/` with the given events lines.
    fn write_events_fixture(cwd: &Path, lines: &[&str]) {
        fs::create_dir_all(cwd.join(".chug")).unwrap();
        let body: String = lines.iter().map(|l| format!("{l}\n")).collect();
        fs::write(cwd.join(".chug/events.jsonl"), body).unwrap();
    }

    /// Append one raw line to the fixture's events file (the mid-wait writer
    /// threads use this).
    fn append_events_line(path: &Path, line: &str) {
        let mut f = fs::OpenOptions::new().append(true).open(path).unwrap();
        f.write_all(line.as_bytes()).unwrap();
        f.write_all(b"\n").unwrap();
    }

    /// The `waited: <n>s` line's seconds — the wait leg's one extra line.
    fn waited_secs_of(content: &str) -> Option<u64> {
        content
            .lines()
            .find_map(|l| l.strip_prefix("waited: ")?.strip_suffix('s')?.parse().ok())
    }

    /// T29 test 1 (instant leg byte-identical) + test 6's `0` boundary: on a
    /// fixed fake `.chug/`, `wait_secs` absent and `wait_secs: 0` both render
    /// EXACTLY the pre-T29 payload — pinned as one whole string, so any
    /// render drift, any new field, or any `waited:` line leaking into the
    /// instant leg fails here.
    #[test]
    fn delegate_status_wait_secs_absent_and_zero_are_byte_identical_to_pre_t29() {
        let tmp = tempfile::tempdir().unwrap();
        write_events_fixture(tmp.path(), &[T29_RUN_START, T29_ITERATION]);
        let ctx = delegate_ctx(tmp.path());
        let absent = dispatch(&ctx, "delegate", &json!({"action": "status", "cwd": tmp.path()}));
        let zero = dispatch(
            &ctx,
            "delegate",
            &json!({"action": "status", "cwd": tmp.path(), "wait_secs": 0}),
        );
        let expected = "\
state: running
alive: unknown (no pid given)
max_iters: 50
last_iteration: 7
last_event: iteration t1
budget_low_seen: false
goal_seen: false
abort_seen: false
log_tail: (none)";
        assert_eq!(absent.content, expected, "{}", absent.content);
        assert_eq!(zero.content, absent.content, "wait_secs: 0 must be byte-identical to absent");
        assert!(!absent.content.contains("waited:"), "{}", absent.content);
        assert!(!absent.is_error && !zero.is_error);
    }

    /// T29 test 2 (early return on state change) AND test 7 (non-vacuousness).
    /// A writer thread appends an `iteration` line mid-wait; `wait_secs: 30`
    /// must return well under 30 s carrying the NEW state.
    ///
    /// NON-VACUOUSNESS TECHNIQUE: the assertions are on the returned CONTENT,
    /// not just timing. Gutting the wait loop to a fixed sleep (sleep to the
    /// deadline, then render) passes every timing assertion but returns the
    /// ENTRY state — `last_iteration: none` — so pinning `last_iteration: 7`
    /// and `last_event: iteration t1` kills it. The return-immediately
    /// mutant fails the same pins; a wait-the-full-30s mutant fails the
    /// elapsed bound below.
    #[test]
    fn delegate_status_wait_returns_early_on_state_change_with_new_state() {
        let tmp = tempfile::tempdir().unwrap();
        write_events_fixture(tmp.path(), &[T29_RUN_START]);
        let events = tmp.path().join(".chug/events.jsonl");
        let writer = thread::spawn(move || {
            thread::sleep(Duration::from_millis(300));
            append_events_line(&events, T29_ITERATION);
        });
        let ctx = delegate_ctx(tmp.path());
        let started = Instant::now();
        let result = dispatch(
            &ctx,
            "delegate",
            &json!({"action": "status", "cwd": tmp.path(), "wait_secs": 30}),
        );
        let elapsed = started.elapsed();
        writer.join().unwrap();
        assert!(!result.is_error, "{}", result.content);
        // Early: well under the 30 s deadline (CI slack).
        assert!(elapsed < Duration::from_secs(15), "wait did not return early: {elapsed:?}");
        // The NEW state, not the entry snapshot — the non-vacuousness pins.
        assert!(result.content.contains("last_iteration: 7"), "{}", result.content);
        assert!(result.content.contains("last_event: iteration t1"), "{}", result.content);
        assert!(result.content.contains("state: running"), "{}", result.content);
        // The wait leg's one extra line, naming the actual elapsed seconds.
        let waited = waited_secs_of(&result.content).expect("waited: line present");
        assert!(waited < 30, "waited: {waited}s");
    }

    /// T29 test 3 (deadline): a static `.chug/` and `wait_secs: 2` return
    /// after ~2 s with the entry state unchanged and the `waited:` line
    /// present (1 ≤ waited ≤ 10 for CI slack).
    #[test]
    fn delegate_status_wait_returns_at_deadline_with_unchanged_state() {
        let tmp = tempfile::tempdir().unwrap();
        write_events_fixture(tmp.path(), &[T29_RUN_START, T29_ITERATION]);
        let ctx = delegate_ctx(tmp.path());
        let started = Instant::now();
        let result = dispatch(
            &ctx,
            "delegate",
            &json!({"action": "status", "cwd": tmp.path(), "wait_secs": 2}),
        );
        let elapsed = started.elapsed();
        assert!(!result.is_error, "{}", result.content);
        assert!(elapsed >= Duration::from_secs(1), "returned before any wait could elapse: {elapsed:?}");
        assert!(elapsed < Duration::from_secs(30), "overshot the 2s deadline: {elapsed:?}");
        // Unchanged state fields — the same fields the instant leg renders.
        assert!(result.content.contains("last_iteration: 7"), "{}", result.content);
        assert!(result.content.contains("last_event: iteration t1"), "{}", result.content);
        assert!(result.content.contains("state: running"), "{}", result.content);
        let waited = waited_secs_of(&result.content).expect("waited: line present");
        assert!((1..=10).contains(&waited), "waited: {waited}s");
    }

    /// T29 test 4 (missing events file). Req 2(a) makes the file's CREATION a
    /// wake trigger, so a file missing at entry does not skip the wait — the
    /// two legs pin both halves and reconcile the spec's "returns
    /// immediately" wording with req 2(a):
    /// (a) no writer: the wait is strictly bounded by the deadline, never a
    ///     hang (req 5), and renders the starting/`events: nothing read`
    ///     note leg with no panic;
    /// (b) a creator thread: returns promptly — as soon as the file appears —
    ///     with the new state (the file-creation trigger, non-vacuously).
    #[test]
    fn delegate_status_wait_on_missing_events_file_bounded_then_created() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = delegate_ctx(tmp.path());
        // (a) No writer: bounded by the deadline, starting/note leg, no panic.
        let started = Instant::now();
        let result = dispatch(
            &ctx,
            "delegate",
            &json!({"action": "status", "cwd": tmp.path(), "wait_secs": 2}),
        );
        let elapsed = started.elapsed();
        assert!(!result.is_error, "{}", result.content);
        assert!(elapsed >= Duration::from_secs(1), "no-writer wait returned before its deadline: {elapsed:?}");
        assert!(elapsed < Duration::from_secs(30), "missing-file wait hung: {elapsed:?}");
        assert!(result.content.contains("state: starting"), "{}", result.content);
        assert!(result.content.contains("events: nothing read"), "{}", result.content);
        assert!(result.content.contains("last_event: none"), "{}", result.content);
        assert!(waited_secs_of(&result.content).is_some(), "{}", result.content);

        // (b) Creator thread: the file appearing mid-wait IS the state
        // change, so the wait returns promptly with the new state.
        let events = tmp.path().join(".chug/events.jsonl");
        let writer = thread::spawn(move || {
            thread::sleep(Duration::from_millis(300));
            fs::create_dir_all(events.parent().unwrap()).unwrap();
            fs::write(&events, format!("{T29_RUN_START}\n")).unwrap();
        });
        let started = Instant::now();
        let result = dispatch(
            &ctx,
            "delegate",
            &json!({"action": "status", "cwd": tmp.path(), "wait_secs": 30}),
        );
        let elapsed = started.elapsed();
        writer.join().unwrap();
        assert!(!result.is_error, "{}", result.content);
        assert!(
            elapsed < Duration::from_secs(15),
            "creation trigger did not wake the wait: {elapsed:?}"
        );
        assert!(result.content.contains("last_event: run_start t0"), "{}", result.content);
        assert!(result.content.contains("state: running"), "{}", result.content);
    }

    /// T29 test 5 (schema pins): the live delegate schema gains optional
    /// integer `wait_secs` (min 0, max 600), the required list is unchanged,
    /// and the launch action rejects `wait_secs` with an error naming that
    /// it is status-only.
    #[test]
    fn delegate_schema_pins_wait_secs_and_launch_rejects_it() {
        let schemas = tool_schemas();
        let schema = schemas
            .iter()
            .find(|s| s.get("name").and_then(Value::as_str) == Some("delegate"))
            .expect("exactly one delegate schema (pinned elsewhere)");
        let wait = schema["input_schema"]["properties"]["wait_secs"]
            .as_object()
            .expect("wait_secs property");
        assert_eq!(wait.get("type").and_then(Value::as_str), Some("integer"));
        assert_eq!(wait.get("minimum").and_then(Value::as_u64), Some(0));
        assert_eq!(wait.get("maximum").and_then(Value::as_u64), Some(600));
        let required: Vec<&str> = schema["input_schema"]["required"]
            .as_array()
            .expect("required list")
            .iter()
            .filter_map(Value::as_str)
            .collect();
        assert_eq!(required, vec!["action", "cwd"], "required list must be unchanged");

        // Status-only: the launch rejection names the status action.
        let tmp = tempfile::tempdir().unwrap();
        let result = dispatch(
            &delegate_ctx(tmp.path()),
            "delegate",
            &json!({
                "action": "launch",
                "cwd": tmp.path(),
                "spec": "/tmp/chug-stub-spec.md",
                "goal": "g",
                "model": "m",
                "wait_secs": 5,
            }),
        );
        assert!(result.is_error, "{}", result.content);
        assert!(
            result.content.contains("status action only"),
            "rejection must name that wait_secs is status-only: {}",
            result.content
        );
    }

    /// T29 test 6 (boundary pins): `600` accepted, `601` and negative
    /// rejected — the pinned req-4 choice is REJECT, never clamp. (`0` =
    /// instant is pinned byte-identically in the test-1 leg.)
    #[test]
    fn delegate_status_wait_secs_boundaries_reject_out_of_range() {
        let tmp = tempfile::tempdir().unwrap();
        write_events_fixture(tmp.path(), &[T29_RUN_START]);
        let ctx = delegate_ctx(tmp.path());

        // 600 is accepted. A writer thread cuts the wait short so the test
        // stays fast; acceptance means the parse did not reject the cap.
        let events = tmp.path().join(".chug/events.jsonl");
        let writer = thread::spawn(move || {
            thread::sleep(Duration::from_millis(300));
            append_events_line(&events, "{\"type\":\"iteration\",\"ts\":\"t9\",\"n\":8}");
        });
        let result = dispatch(
            &ctx,
            "delegate",
            &json!({"action": "status", "cwd": tmp.path(), "wait_secs": 600}),
        );
        writer.join().unwrap();
        assert!(!result.is_error, "600 must be accepted: {}", result.content);
        assert!(result.content.contains("last_iteration: 8"), "{}", result.content);

        // 601: rejected, naming the 600 cap — not clamped down to it.
        let result = dispatch(
            &ctx,
            "delegate",
            &json!({"action": "status", "cwd": tmp.path(), "wait_secs": 601}),
        );
        assert!(result.is_error, "{}", result.content);
        assert!(result.content.contains("600"), "{}", result.content);
        // Negative: rejected, naming the >= 0 floor.
        let result = dispatch(
            &ctx,
            "delegate",
            &json!({"action": "status", "cwd": tmp.path(), "wait_secs": -1}),
        );
        assert!(result.is_error, "{}", result.content);
        assert!(result.content.contains("-1"), "{}", result.content);
        // Non-integer: rejected (string and fractional alike).
        for bad in [json!("90"), json!(90.5)] {
            let result = dispatch(
                &ctx,
                "delegate",
                &json!({"action": "status", "cwd": tmp.path(), "wait_secs": bad}),
            );
            assert!(result.is_error, "{}", result.content);
            assert!(result.content.contains("integer"), "{}", result.content);
        }
    }

    /// T29 fix-up, FINDING 1 (spec req 2(b) shipped untested): the
    /// liveness-flip wake leg. A REAL own child (the T28 fixtures' pattern) is
    /// spawned with a static events fixture — so the ONLY thing that changes
    /// during the wait is liveness — and its handle is dropped per the launch
    /// contract, making the waitpid reap inside [`reap_and_alive`] the only
    /// observer of the exit. It is SIGKILLed mid-wait; the wait must wake well
    /// before the deadline with liveness flipped to dead.
    ///
    /// NON-VACUOUSNESS (validator mutant M7: the `liveness_flipped` check
    /// gutted): the events stream never changes, so the gutted wait sleeps to
    /// the full 30 s deadline — the elapsed bound below kills it. A mutant
    /// that skips the waitpid reap keeps answering the zombie's `alive: true`
    /// and fails the `alive: false` pin instead.
    #[cfg(unix)]
    #[test]
    fn delegate_status_wait_wakes_early_when_child_dies_liveness_flip() {
        let tmp = tempfile::tempdir().unwrap();
        write_events_fixture(tmp.path(), &[T29_RUN_START, T29_ITERATION]);
        let child = Command::new("sleep").arg("30").spawn().unwrap();
        let pid = child.id();
        drop(child); // launch contract: detached, never waited by the handle
        // The wait call blocks this thread, so the kill fires from a helper
        // thread: the child dies mid-wait and stays a zombie for the seam.
        let killer = thread::spawn(move || {
            thread::sleep(Duration::from_millis(300));
            unsafe { libc::kill(pid as libc::pid_t, libc::SIGKILL) };
        });
        let started = Instant::now();
        let result = dispatch(
            &delegate_ctx(tmp.path()),
            "delegate",
            &json!({"action": "status", "cwd": tmp.path(), "pid": pid, "wait_secs": 30}),
        );
        let elapsed = started.elapsed();
        killer.join().unwrap();
        assert!(!result.is_error, "{}", result.content);
        // The flip woke the wait: well under the 30 s deadline (CI slack).
        assert!(
            elapsed < Duration::from_secs(15),
            "liveness flip did not wake the wait early: {elapsed:?}"
        );
        // Liveness flipped to dead, with the wait leg's `waited:` line.
        assert!(result.content.contains("alive: false"), "{}", result.content);
        let waited = waited_secs_of(&result.content).expect("waited: line present");
        assert!(waited < 30, "waited: {waited}s");
        // The events state is unchanged (`last_iteration: 7`) — the wake came
        // from the liveness flip, not from a state change.
        assert!(result.content.contains("last_iteration: 7"), "{}", result.content);
    }

    /// T29 fix-up, FINDING 2 (spec req 2's "FIRST of" + "same payload as the
    /// instant leg"): the deadline leg must render the FINAL state, not the
    /// entry snapshot. With the 2.5 s cadence and `wait_secs: 2` there is
    /// exactly ONE poll — at entry; the cadence sleep is capped at the
    /// remaining 2 s, so nothing reads between the entry poll and the
    /// deadline — and a writer landing inside that window is therefore
    /// invisible to every intermediate poll: only a final read at the
    /// deadline can see it.
    ///
    /// NON-VACUOUSNESS: removing the final read (the pre-fix deadline leg,
    /// which rendered the entry snapshot) fails the `last_iteration: 8` pin —
    /// that render carries the entry state, `last_iteration: none`. An
    /// instant-return mutant fails the same pin (the writer has not run yet),
    /// and both pass no timing bound to hide behind.
    #[test]
    fn delegate_status_wait_deadline_renders_final_state_not_entry_snapshot() {
        let tmp = tempfile::tempdir().unwrap();
        write_events_fixture(tmp.path(), &[T29_RUN_START]);
        let events = tmp.path().join(".chug/events.jsonl");
        // Lands ~1 s in: after the entry poll (~0 ms) and before the 2 s
        // deadline wake, with a full second of scheduling slack each side.
        let writer = thread::spawn(move || {
            thread::sleep(Duration::from_millis(1000));
            append_events_line(&events, "{\"type\":\"iteration\",\"ts\":\"t8\",\"n\":8}");
        });
        let ctx = delegate_ctx(tmp.path());
        let started = Instant::now();
        let result = dispatch(
            &ctx,
            "delegate",
            &json!({"action": "status", "cwd": tmp.path(), "wait_secs": 2}),
        );
        let elapsed = started.elapsed();
        writer.join().unwrap();
        assert!(!result.is_error, "{}", result.content);
        // The deadline leg ran: at least the entry poll's window elapsed.
        assert!(
            elapsed >= Duration::from_millis(1500),
            "returned before the deadline could elapse: {elapsed:?}"
        );
        assert!(elapsed < Duration::from_secs(30), "overshot the 2s deadline: {elapsed:?}");
        // The deadline render carries the FINAL state, not the entry snapshot.
        assert!(result.content.contains("last_iteration: 8"), "{}", result.content);
        assert!(result.content.contains("last_event: iteration t8"), "{}", result.content);
        let waited = waited_secs_of(&result.content).expect("waited: line present");
        assert!((1..=10).contains(&waited), "waited: {waited}s");
    }

    #[test]
    fn delegate_log_tail_last_three_nonempty_clipped() {
        let tmp = tempfile::tempdir().unwrap();
        let log = tmp.path().join("delegate.log");
        fs::write(&log, "one\n\n".repeat(500) + "line-4\nline-5\nline-6\nline-7\n").unwrap();
        assert_eq!(read_log_tail(&log), vec!["line-5", "line-6", "line-7"]);
        let long = "y".repeat(500);
        fs::write(&log, format!("{long}\nlast\n")).unwrap();
        assert_eq!(read_log_tail(&log), vec!["y".repeat(200), "last".to_string()]);
        // Unreadable log → empty tail, never an error.
        assert!(read_log_tail(&tmp.path().join("missing.log")).is_empty());
    }

    /// Bound pin: the events log grows unboundedly, so `status` must read only
    /// the last [`DELEGATE_EVENTS_TAIL_BYTES`]. A goal line older than that
    /// window must not be reported — reading the whole file (bound removed)
    /// would see it and fail this test.
    #[test]
    fn delegate_status_reads_only_the_tail_window() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join(".chug")).unwrap();
        let mut body = String::from(
            "{\"type\":\"goal\",\"ts\":\"ancient\",\"outcome\":\"accepted\",\"summary\":\"old run\"}\n",
        );
        let filler = format!(
            "{{\"type\":\"iteration\",\"ts\":\"filler\",\"n\":1,\"pad\":\"{}\"}}\n",
            "x".repeat(80)
        );
        while body.len() < DELEGATE_EVENTS_TAIL_BYTES as usize + 4096 {
            body.push_str(&filler);
        }
        body.push_str("{\"type\":\"iteration\",\"ts\":\"recent\",\"n\":9}\n");
        fs::write(tmp.path().join(".chug/events.jsonl"), body).unwrap();

        let result = dispatch(
            &delegate_ctx(tmp.path()),
            "delegate",
            &json!({"action": "status", "cwd": tmp.path()}),
        );
        assert!(!result.is_error, "{}", result.content);
        assert!(
            !result.content.contains("goal_seen: true"),
            "pre-bound goal leaked into the tail window: {}",
            result.content
        );
        assert!(result.content.contains("last_iteration: 9"), "{}", result.content);
        assert!(result.content.contains("state: running"), "{}", result.content);
    }

    #[test]
    fn delegate_schema_registers_exactly_one_entry_with_both_actions() {
        let schemas = tool_schemas();
        let entries: Vec<&Value> = schemas
            .iter()
            .filter(|s| s.get("name").and_then(Value::as_str) == Some("delegate"))
            .collect();
        assert_eq!(entries.len(), 1, "exactly one delegate schema");
        let schema = &entries[0];
        let action = schema
            .get("input_schema")
            .and_then(|s| s.get("properties"))
            .and_then(|p| p.get("action"))
            .expect("action property");
        let actions: Vec<&str> = action
            .get("enum")
            .and_then(Value::as_array)
            .expect("action enum")
            .iter()
            .filter_map(Value::as_str)
            .collect();
        assert_eq!(actions, vec!["launch", "status"]);
        let required: Vec<&str> = schema["input_schema"]["required"]
            .as_array()
            .expect("required list")
            .iter()
            .filter_map(Value::as_str)
            .collect();
        assert!(required.contains(&"action"), "action must be required");
        assert!(required.contains(&"cwd"), "cwd must be required");
        // No other schema may shadow or duplicate the name.
        assert!(schemas.iter().any(|s| s.get("name").and_then(Value::as_str) == Some("goal_complete")));
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
        // Appended as exactly ONE sentence (3 → 4), at the end.
        assert_eq!(
            desc.split(". ").count(),
            4,
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

    /// End-to-end with a stub binary: `CHUG_DELEGATE_BIN` points at a script
    /// that writes a synthetic `run_start`+`iteration` into `$PWD/.chug/` then
    /// sleeps. Launch returns a pid immediately; bounded polling (≤5s) then
    /// sees the summary with `alive: true`.
    #[cfg(unix)]
    #[test]
    fn delegate_launch_stub_then_status_reports_summary_and_liveness() {
        use std::os::unix::fs::PermissionsExt;
        let _guard = DELEGATE_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let child_dir = tempfile::tempdir().unwrap();
        let ctx_cwd = tempfile::tempdir().unwrap();

        let stub = ctx_cwd.path().join("chug-stub.sh");
        fs::write(
            &stub,
            concat!(
                "#!/bin/sh\n",
                "mkdir -p .chug\n",
                "printf '%s\\n' '{\"type\":\"run_start\",\"ts\":\"stub-t0\",\"mode\":\"run\",\"model\":\"stub\",\"max_iters\":40,\"max_minutes\":35,\"max_tokens\":null}' >> .chug/events.jsonl\n",
                "printf '%s\\n' '{\"type\":\"iteration\",\"ts\":\"stub-t1\",\"n\":1,\"input_tokens\":7,\"output_tokens\":3}' >> .chug/events.jsonl\n",
                "echo stub child up\n",
                "sleep 60\n",
            ),
        )
        .unwrap();
        fs::set_permissions(&stub, fs::Permissions::from_mode(0o755)).unwrap();
        // SAFETY: serialized by DELEGATE_ENV_LOCK; no other test reads this var.
        unsafe { std::env::set_var("CHUG_DELEGATE_BIN", &stub) };

        let launch = dispatch(
            &delegate_ctx(ctx_cwd.path()),
            "delegate",
            &json!({
                "action": "launch",
                "cwd": child_dir.path(),
                "spec": "/tmp/chug-stub-spec.md",
                "goal": "stub goal",
                "model": "stub-model",
            }),
        );
        assert!(!launch.is_error, "{}", launch.content);
        assert!(
            launch.content.contains("max_iters: 40 max_minutes: 35"),
            "defaults not applied: {}",
            launch.content
        );
        assert!(
            launch
                .content
                .contains(&format!("log: {}", child_dir.path().join(".chug/delegate.log").display())),
            "{}",
            launch.content
        );
        assert!(
            launch
                .content
                .contains(&format!("events: {}", child_dir.path().join(".chug/events.jsonl").display())),
            "{}",
            launch.content
        );
        let pid: u32 = launch
            .content
            .lines()
            .find_map(|l| l.strip_prefix("launched: pid "))
            .expect("pid in launch output")
            .trim()
            .parse()
            .expect("pid parses");

        let deadline = Instant::now() + Duration::from_secs(5);
        let mut seen = None;
        while Instant::now() < deadline {
            let s = dispatch(
                &delegate_ctx(ctx_cwd.path()),
                "delegate",
                &json!({"action": "status", "cwd": child_dir.path(), "pid": pid}),
            );
            assert!(!s.is_error, "{}", s.content);
            if s.content.contains("last_iteration: 1") && s.content.contains("alive: true") {
                seen = Some(s);
                break;
            }
            thread::sleep(Duration::from_millis(100));
        }
        let status = seen.expect("stub summary + liveness within 5s");
        assert!(status.content.contains("state: running"), "{}", status.content);
        assert!(status.content.contains("max_iters: 40"), "{}", status.content);
        assert!(
            status.content.contains("last_event: iteration"),
            "{}",
            status.content
        );
        assert!(
            status.content.contains("stub child up"),
            "console log tail missing: {}",
            status.content
        );

        // The tool detached and dropped the handle, so cleanup only has the pid.
        kill_pid_group(pid);

        // Explicit budgets pass through to the child untouched (the launch
        // line echoes them back; the stub would receive them as argv).
        let launch2 = dispatch(
            &delegate_ctx(ctx_cwd.path()),
            "delegate",
            &json!({
                "action": "launch",
                "cwd": child_dir.path(),
                "spec": "/tmp/chug-stub-spec.md",
                "goal": "stub goal 2",
                "model": "stub-model",
                "max_iters": 7,
                "max_minutes": 9,
            }),
        );
        assert!(!launch2.is_error, "{}", launch2.content);
        assert!(
            launch2.content.contains("max_iters: 7 max_minutes: 9"),
            "{}",
            launch2.content
        );
        let pid2: u32 = launch2
            .content
            .lines()
            .find_map(|l| l.strip_prefix("launched: pid "))
            .expect("pid in launch output")
            .trim()
            .parse()
            .expect("pid parses");
        kill_pid_group(pid2);
        // SAFETY: serialized by DELEGATE_ENV_LOCK; no other test reads this var.
        unsafe { std::env::remove_var("CHUG_DELEGATE_BIN") };
    }

    /// Launch failure leg: a binary that does not exist must produce a tool
    /// error naming the path — never a panic, never a driver abort.
    #[test]
    fn delegate_launch_missing_binary_is_tool_error() {
        let _guard = DELEGATE_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::tempdir().unwrap();
        // SAFETY: serialized by DELEGATE_ENV_LOCK; no other test reads this var.
        unsafe { std::env::set_var("CHUG_DELEGATE_BIN", "/nonexistent/chug") };
        let result = dispatch(
            &delegate_ctx(tmp.path()),
            "delegate",
            &json!({
                "action": "launch",
                "cwd": tmp.path(),
                "spec": "/tmp/chug-spec.md",
                "goal": "g",
                "model": "m",
            }),
        );
        unsafe { std::env::remove_var("CHUG_DELEGATE_BIN") };
        assert!(result.is_error, "{}", result.content);
        assert!(
            result.content.contains("/nonexistent/chug"),
            "error must name the binary path: {}",
            result.content
        );
    }

    // ---- T39: delegate launch optional max_tokens passthrough ----

    /// T39: the pure argv builder — with a token budget the flag pair is
    /// appended at the tail (after `--max-minutes`); without one the argv is
    /// byte-identical to pre-T39, pinned as the whole list.
    #[test]
    fn delegate_child_argv_appends_max_tokens_only_when_present() {
        let spec = PathBuf::from("/tmp/spec.md");
        let base = vec![
            OsString::from("run"),
            OsString::from("--spec"),
            OsString::from("/tmp/spec.md"),
            OsString::from("--goal"),
            OsString::from("g"),
            OsString::from("--model"),
            OsString::from("m"),
            OsString::from("--max-iters"),
            OsString::from("40"),
            OsString::from("--max-minutes"),
            OsString::from("35"),
        ];
        // Absent: byte-identical to pre-T39 — no `--max-tokens` anywhere.
        assert_eq!(
            delegate_child_argv(&spec, "g", "m", 40, 35, None),
            base,
            "absent max_tokens must not change the child argv"
        );
        // T15 parity: the flag pair lands at the tail, verbatim.
        let mut with = base.clone();
        with.push(OsString::from("--max-tokens"));
        with.push(OsString::from("250000"));
        assert_eq!(
            delegate_child_argv(&spec, "g", "m", 40, 35, Some(250_000)),
            with
        );
        // Boundary: 1 is accepted and passes through verbatim.
        let mut one = base;
        one.push(OsString::from("--max-tokens"));
        one.push(OsString::from("1"));
        assert_eq!(delegate_child_argv(&spec, "g", "m", 40, 35, Some(1)), one);
    }

    /// T39: the parse — absent → `None`; valid → `Some`; `< 1` (0, negative)
    /// → error naming the constraint; non-integer (string, fractional) →
    /// error naming the integer requirement. Zero never parses into `Some(0)`
    /// (which the child CLI would read as "unlimited").
    #[test]
    fn delegate_max_tokens_parse_absent_valid_and_rejects() {
        assert_eq!(delegate_max_tokens(&json!({})).unwrap(), None);
        assert_eq!(
            delegate_max_tokens(&json!({"max_tokens": 250_000})).unwrap(),
            Some(250_000)
        );
        assert_eq!(delegate_max_tokens(&json!({"max_tokens": 1})).unwrap(), Some(1));
        for bad in [json!(0), json!(-5)] {
            let err = delegate_max_tokens(&json!({"max_tokens": bad}))
                .unwrap_err()
                .to_string();
            assert!(err.contains("at least 1"), "{bad} → {err}");
        }
        for bad in [json!("250000"), json!(250000.5)] {
            let err = delegate_max_tokens(&json!({"max_tokens": bad}))
                .unwrap_err()
                .to_string();
            assert!(err.contains("integer"), "{bad} → {err}");
        }
    }

    /// A stub child that records the argv it was invoked with, one argument
    /// per line, into `argv.txt` in its cwd (the child dir), then sleeps so
    /// the pid-group kill cleans it up.
    #[cfg(unix)]
    fn write_argv_stub(dir: &Path) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let stub = dir.join("chug-argv-stub.sh");
        fs::write(
            &stub,
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > argv.txt\nsleep 60\n",
        )
        .unwrap();
        fs::set_permissions(&stub, fs::Permissions::from_mode(0o755)).unwrap();
        stub
    }

    /// The stub's argv dump, polled for (launch returns at spawn; the stub
    /// writes the dump within milliseconds of exec).
    #[cfg(unix)]
    fn wait_for_argv_dump(child_dir: &Path) -> Vec<String> {
        let path = child_dir.join("argv.txt");
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Ok(text) = fs::read_to_string(&path) {
                return text.lines().map(str::to_string).collect();
            }
            assert!(
                Instant::now() < deadline,
                "stub never wrote argv.txt in {}",
                child_dir.display()
            );
            thread::sleep(Duration::from_millis(25));
        }
    }

    #[cfg(unix)]
    fn spawn_pid_of(launch: &ToolResult) -> u32 {
        launch
            .content
            .lines()
            .find_map(|l| l.strip_prefix("launched: pid "))
            .expect("pid in launch output")
            .trim()
            .parse()
            .expect("pid parses")
    }

    /// T39 spec test: launch with `max_tokens: 250000` → the child's argv
    /// contains `--max-tokens 250000`, asserted against the argv seam the
    /// other delegate tests use (`CHUG_DELEGATE_BIN`; the stub dumps its
    /// argv). NON-VACUOUSNESS: dropping the argv append leaves the dump
    /// without the flag and fails here.
    #[test]
    fn delegate_launch_with_max_tokens_appends_flag_to_child_argv() {
        let _guard = DELEGATE_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let child_dir = tempfile::tempdir().unwrap();
        let ctx_cwd = tempfile::tempdir().unwrap();
        // SAFETY: serialized by DELEGATE_ENV_LOCK; no other test reads this var.
        unsafe { std::env::set_var("CHUG_DELEGATE_BIN", write_argv_stub(ctx_cwd.path())) };
        let launch = dispatch(
            &delegate_ctx(ctx_cwd.path()),
            "delegate",
            &json!({
                "action": "launch",
                "cwd": child_dir.path(),
                "spec": "/tmp/chug-stub-spec.md",
                "goal": "g",
                "model": "m",
                "max_tokens": 250_000,
            }),
        );
        assert!(!launch.is_error, "{}", launch.content);
        let argv = wait_for_argv_dump(child_dir.path());
        let pos = argv
            .iter()
            .position(|a| a == "--max-tokens")
            .expect("--max-tokens in the child argv");
        assert_eq!(argv[pos + 1], "250000", "{}", argv.join(" | "));
        // The return text names the configured token budget alongside the
        // existing budgets (the iters/minutes echo pattern).
        assert!(
            launch
                .content
                .contains("max_iters: 40 max_minutes: 35 max_tokens: 250000"),
            "{}",
            launch.content
        );
        let pid = spawn_pid_of(&launch);
        kill_pid_group(pid);
        // SAFETY: serialized by DELEGATE_ENV_LOCK; no other test reads this var.
        unsafe { std::env::remove_var("CHUG_DELEGATE_BIN") };
    }

    /// T39 byte-identical-argv control: WITHOUT `max_tokens`, the child argv
    /// carries no `--max-tokens` and is exactly the pre-T39 list — children
    /// keep their current no-token-ceiling behavior unless the orchestrator
    /// opts in.
    #[test]
    fn delegate_launch_without_max_tokens_keeps_argv_byte_identical() {
        let _guard = DELEGATE_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let child_dir = tempfile::tempdir().unwrap();
        let ctx_cwd = tempfile::tempdir().unwrap();
        // SAFETY: serialized by DELEGATE_ENV_LOCK; no other test reads this var.
        unsafe { std::env::set_var("CHUG_DELEGATE_BIN", write_argv_stub(ctx_cwd.path())) };
        let launch = dispatch(
            &delegate_ctx(ctx_cwd.path()),
            "delegate",
            &json!({
                "action": "launch",
                "cwd": child_dir.path(),
                "spec": "/tmp/chug-stub-spec.md",
                "goal": "g",
                "model": "m",
            }),
        );
        assert!(!launch.is_error, "{}", launch.content);
        let argv = wait_for_argv_dump(child_dir.path());
        assert_eq!(
            argv,
            vec![
                "run".to_string(),
                "--spec".to_string(),
                "/tmp/chug-stub-spec.md".to_string(),
                "--goal".to_string(),
                "g".to_string(),
                "--model".to_string(),
                "m".to_string(),
                "--max-iters".to_string(),
                "40".to_string(),
                "--max-minutes".to_string(),
                "35".to_string(),
            ],
            "argv must be byte-identical to pre-T39 (no --max-tokens)"
        );
        // The return text is unchanged too: no token-budget echo when absent.
        assert!(
            !launch.content.contains("max_tokens"),
            "absent max_tokens must not appear in the launch text: {}",
            launch.content
        );
        let pid = spawn_pid_of(&launch);
        kill_pid_group(pid);
        // SAFETY: serialized by DELEGATE_ENV_LOCK; no other test reads this var.
        unsafe { std::env::remove_var("CHUG_DELEGATE_BIN") };
    }

    /// T39 boundary: `max_tokens: 1` is accepted and reaches the child
    /// verbatim (`--max-tokens 1`) — the floor is inclusive.
    #[cfg(unix)]
    #[test]
    fn delegate_launch_boundary_max_tokens_one_reaches_child() {
        let _guard = DELEGATE_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let child_dir = tempfile::tempdir().unwrap();
        let ctx_cwd = tempfile::tempdir().unwrap();
        // SAFETY: serialized by DELEGATE_ENV_LOCK; no other test reads this var.
        unsafe { std::env::set_var("CHUG_DELEGATE_BIN", write_argv_stub(ctx_cwd.path())) };
        let launch = dispatch(
            &delegate_ctx(ctx_cwd.path()),
            "delegate",
            &json!({
                "action": "launch",
                "cwd": child_dir.path(),
                "spec": "/tmp/chug-stub-spec.md",
                "goal": "g",
                "model": "m",
                "max_tokens": 1,
            }),
        );
        assert!(!launch.is_error, "{}", launch.content);
        let argv = wait_for_argv_dump(child_dir.path());
        let pos = argv
            .iter()
            .position(|a| a == "--max-tokens")
            .expect("--max-tokens in the child argv");
        assert_eq!(argv[pos + 1], "1", "{}", argv.join(" | "));
        let pid = spawn_pid_of(&launch);
        kill_pid_group(pid);
        // SAFETY: serialized by DELEGATE_ENV_LOCK; no other test reads this var.
        unsafe { std::env::remove_var("CHUG_DELEGATE_BIN") };
    }

    /// T39: `max_tokens: 0` and `max_tokens: -5` are tool errors naming the
    /// constraint — and no child is spawned (the stub's argv dump never
    /// appears in the child dir). NON-VACUOUSNESS: accepting 0 (e.g. by
    /// parsing with `as_u64` like `max_iters` does) fails the error asserts.
    #[test]
    fn delegate_launch_rejects_zero_and_negative_max_tokens_without_spawning() {
        let _guard = DELEGATE_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let child_dir = tempfile::tempdir().unwrap();
        let ctx_cwd = tempfile::tempdir().unwrap();
        // SAFETY: serialized by DELEGATE_ENV_LOCK; no other test reads this var.
        unsafe { std::env::set_var("CHUG_DELEGATE_BIN", write_argv_stub(ctx_cwd.path())) };
        for bad in [json!(0), json!(-5)] {
            let result = dispatch(
                &delegate_ctx(ctx_cwd.path()),
                "delegate",
                &json!({
                    "action": "launch",
                    "cwd": child_dir.path(),
                    "spec": "/tmp/chug-stub-spec.md",
                    "goal": "g",
                    "model": "m",
                    "max_tokens": bad,
                }),
            );
            assert!(result.is_error, "{bad}: {}", result.content);
            assert!(
                result.content.contains("at least 1"),
                "{bad} must name the constraint: {}",
                result.content
            );
        }
        // No spawn: the stub never ran, so its argv dump does not exist.
        assert!(
            !child_dir.path().join("argv.txt").exists(),
            "a rejected max_tokens must never spawn the child"
        );
        // SAFETY: serialized by DELEGATE_ENV_LOCK; no other test reads this var.
        unsafe { std::env::remove_var("CHUG_DELEGATE_BIN") };
    }

    /// T39: the delegate schema advertises `max_tokens` as an OPTIONAL integer
    /// (minimum 1) in launch's properties, and the required list is unchanged.
    #[test]
    fn delegate_schema_pins_optional_max_tokens() {
        let schemas = tool_schemas();
        let schema = schemas
            .iter()
            .find(|s| s.get("name").and_then(Value::as_str) == Some("delegate"))
            .expect("exactly one delegate schema (pinned elsewhere)");
        let prop = schema["input_schema"]["properties"]["max_tokens"]
            .as_object()
            .expect("max_tokens property");
        assert_eq!(prop.get("type").and_then(Value::as_str), Some("integer"));
        assert_eq!(prop.get("minimum").and_then(Value::as_u64), Some(1));
        let required: Vec<&str> = schema["input_schema"]["required"]
            .as_array()
            .expect("required list")
            .iter()
            .filter_map(Value::as_str)
            .collect();
        assert_eq!(required, vec!["action", "cwd"], "required list must be unchanged");
    }

    /// T39: `max_tokens` on the `status` action is IGNORED, not rejected —
    /// exactly how `max_iters`/`max_minutes` behave there today (launch-only
    /// inputs that status never reads). The status payload shape is
    /// unchanged.
    #[test]
    fn delegate_status_ignores_max_tokens() {
        let tmp = tempfile::tempdir().unwrap();
        write_events_fixture(tmp.path(), &[T29_RUN_START, T29_ITERATION]);
        let result = dispatch(
            &delegate_ctx(tmp.path()),
            "delegate",
            &json!({"action": "status", "cwd": tmp.path(), "max_tokens": 250_000}),
        );
        assert!(!result.is_error, "{}", result.content);
        assert!(result.content.contains("state: running"), "{}", result.content);
        assert!(result.content.contains("last_iteration: 7"), "{}", result.content);
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
    /// `resolve_safe` fails with exactly `path escapes cwd: <path>` (pre-T41
    /// wording, unchanged; behavior is also pinned by
    /// `glob_rejects_path_escape` and the `path_safety_*` tests).
    #[test]
    fn resolve_safe_error_names_path_escapes_cwd() {
        let tmp = tempfile::tempdir().unwrap();
        let err = resolve_safe(tmp.path(), "/etc/passwd").unwrap_err();
        assert!(err.starts_with("path escapes cwd: "), "{err}");
        let err = resolve_safe(tmp.path(), "../out.txt").unwrap_err();
        assert!(err.starts_with("path escapes cwd: "), "{err}");
    }

    /// T41: the README Tools intro must name BOTH sandbox exceptions —
    /// `delegate` (absolute child-worktree paths) and `web_fetch` (network,
    /// not filesystem). It said `delegate` was "the one documented exception",
    /// stale the moment T37 landed `web_fetch` — a cold reader saw the intro
    /// contradict the `web_fetch` paragraph one screen below. Whitespace is
    /// normalized so the pin is independent of markdown line wrapping.
    #[test]
    fn readme_tools_intro_names_both_sandbox_exceptions() {
        // T48: cargo runs test binaries with cwd = the package root; the compile-time env! path is wrong under the T47 shared cache (cycle-21) — resolve at runtime.
        let readme = fs::read_to_string(
            std::env::current_dir()
                .expect("cargo sets the test cwd to the package root")
                .join("README.md"),
        )
        .expect("README.md readable from the crate root");
        let flat: String = readme.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(
            flat.contains(
                "All paths sandboxed to `--cwd` (`delegate` and `web_fetch` are the two documented exceptions"
            ),
            "README Tools intro does not name both exceptions: {flat}"
        );
        assert!(
            flat.contains(
                "`web_fetch` is network, not filesystem). `bash` runs in its own process group"
            ),
            "README Tools intro lost the web_fetch wording or the byte-identical `bash` continuation: {flat}"
        );
        // The stale singular is gone.
        assert!(
            !flat.contains("is the one documented exception"),
            "README still calls delegate the one documented exception: {flat}"
        );
    }
}
