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
/// Verification (`check:`) commands get a more generous ceiling than the bash tool.
pub const CHECK_TIMEOUT_SECS: u64 = 600;
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
        // T70: schema lives in decisions.rs (single source of truth for the
        // description the model sees), registered here alongside the builtins.
        crate::decisions::schema(),
        json!({
            "name": "delegate",
            "description": "Launch, observe, or collect a bounded child `chug run` (e.g. in a worktree you created). action=launch: spawns a detached child with its working directory at `cwd` (absolute), spec/goal/model required, max_iters/max_minutes optional (defaults 40/35), max_tokens optional (child token ceiling; omitted = unlimited), resume optional (true = append --resume, continue the child's prior run instead of starting fresh); returns immediately with the child pid and the log/events paths — it never waits on the child. action=status: reports the child's liveness (when you pass the `pid` from launch), a summary of its .chug/events.jsonl (state, last_iteration, budget-low/goal/abort flags — covering the child's latest run segment), and the tail of its console log. action=collect: returns the child's structured result in ONE bounded non-blocking read — the latest run segment's verdict (goal-accepted / goal-rejected / aborted with reason / running / starting), the accepted goal's summary, the segment's latest check cmd, and best-effort commit refs of the child's cwd (optional `base` scopes the range <base>..HEAD; every git failure degrades to a note, never an error). Never blocks: launch returns at spawn, status reads tails only, collect reads tails only. Optionally pass `wait_secs` on status (0/absent = instant, max 600) to block up to that many seconds, returning early when the child's iteration advances, a verdict or budget-low flag appears, or its liveness flips to dead — per-tool-call last_event churn renders at the deadline but never wakes it (status only — launch and collect reject it).",
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
                    "base": {"type": "string", "description": "collect only (optional): a git ref scoping the commit-refs range as <base>..HEAD (e.g. \"origin/main\"); absent = the bounded default range over HEAD (last 20 commits). Non-string → tool error"},
                    "pid": {"type": "integer", "description": "The pid launch returned (status and collect, optional): status reports liveness, collect adds the same alive line; omit → status reports liveness unknown and collect renders no liveness line"},
                    "wait_secs": {"type": "integer", "minimum": 0, "maximum": 600, "description": "Seconds to block on status waiting for a significant child change (iteration advance, verdict or budget-low flag, liveness flip to dead; last_event churn renders at the deadline but never wakes) or this deadline (0/absent = instant; status only — launch and collect reject it: collect never blocks)"}
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
        "delegate" => crate::delegate::delegate(ctx, input),
        "web_fetch" => crate::webfetch::web_fetch(input),
        "decision_log" => crate::decisions::decision_log(&ctx.cwd, input),
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

pub(crate) fn get_str<'a>(input: &'a Value, key: &str) -> anyhow::Result<&'a str> {
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

/// Both refusal messages of `resolve_safe` carry a suffix naming the `bash`
/// escape hatch (T61): the tool descriptions are read once at turn 0, but
/// the error string is what the model sees at the moment of need — it
/// must name the fallback.
const PATH_ESCAPES_CWD_SUFFIX: &str = " — cross-tree paths go through bash";

/// Resolve `path` lexically against `cwd`, rejecting anything that escapes it
/// (`..` traversal, absolute paths outside cwd). No filesystem access, no
/// symlink resolution: purely lexical, per spec. Both refusal messages carry
/// the T61 `bash` suffix above.
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
                    return Err(format!("path escapes cwd: {path}{PATH_ESCAPES_CWD_SUFFIX}"));
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
        return Err(format!("path escapes cwd: {path}{PATH_ESCAPES_CWD_SUFFIX}"));
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

    /// T69 doc pins (T41/T63 convention): the README delegate paragraph names
    /// the `collect` action with its user-facing semantics, and LOOP-SPEC §2
    /// step 3 carries the adoption sentence (the T23→T24 lesson: a capability
    /// without a doctrine sentence doesn't get called). Whitespace-normalized
    /// so markdown rewrapping cannot unpin them.
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
            readme.contains("**`collect`** returns the child's structured result in one bounded, non-blocking read"),
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
