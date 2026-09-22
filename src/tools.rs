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
            "description": "Read a text file. Paths are relative to the working directory. Output is capped at 2000 lines and truncation is noted.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "File path relative to cwd (must stay inside cwd)"}
                },
                "required": ["path"]
            }
        }),
        json!({
            "name": "write_file",
            "description": "Create or overwrite a file. Parent directories are created automatically.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "File path relative to cwd"},
                    "content": {"type": "string", "description": "Full file contents"}
                },
                "required": ["path", "content"]
            }
        }),
        json!({
            "name": "edit_file",
            "description": "Exact string replacement in a file. `old` must occur exactly once unless `replace_all` is true, which replaces every occurrence.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "File path relative to cwd"},
                    "old": {"type": "string", "description": "Exact text to replace"},
                    "new": {"type": "string", "description": "Replacement text"},
                    "replace_all": {"type": "boolean", "description": "Replace every occurrence (default false)"}
                },
                "required": ["path", "old", "new"]
            }
        }),
        json!({
            "name": "bash",
            "description": "Run a shell command via `sh -c` in the working directory. Captures stdout+stderr and the exit code. 120s timeout; long output is truncated (head+tail kept).",
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
            "description": "Match file paths under the working directory with a glob pattern (e.g. src/**/*.rs). Returns sorted relative paths, capped at 200 with a truncation note.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "pattern": {"type": "string", "description": "Glob pattern, relative to `path` (or cwd)"},
                    "path": {"type": "string", "description": "Optional base directory relative to cwd (must stay inside cwd)"}
                },
                "required": ["pattern"]
            }
        }),
        json!({
            "name": "list_dir",
            "description": "List the immediate entries of a directory (default cwd), one per line, directories suffixed with `/`, directories first, sorted. Capped at 500 with a truncation note.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Directory relative to cwd (default: cwd)"}
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

fn read_file(ctx: &ToolCtx, input: &Value) -> anyhow::Result<ToolResult> {
    let path = get_path(ctx, input)?;
    let data =
        fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let line_count = data.lines().count();
    let content = if line_count > READ_MAX_LINES {
        let head: Vec<&str> = data.lines().take(READ_MAX_LINES).collect();
        format!(
            "{}\n\n[truncated: showing lines 1-{READ_MAX_LINES} of {line_count}]",
            head.join("\n")
        )
    } else {
        data
    };
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

/// Kill the child's whole process group (the child is the group leader via
/// `process_group(0)`), falling back to killing just the direct child on
/// platforms without process groups. Shared with `mcp.rs`, which must honor
/// the same no-orphan discipline.
pub(crate) fn kill_process_group(child: &mut std::process::Child) {
    #[cfg(unix)]
    {
        let pgid = child.id() as i32;
        // Negative pid targets the entire process group.
        unsafe {
            libc::kill(-pgid, libc::SIGKILL);
        }
    }
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

    /// Regression: a backgrounded grandchild in the shell's own process group
    /// must be killed with the GROUP at timeout — before the fix only the
    /// direct `sh` child died, the orphan held the stdout pipe, and the reader
    /// join blocked the driver forever. The call must return shortly after
    /// timeout + reader grace, with a timeout error.
    #[test]
    fn run_shell_timeout_kills_process_group_and_returns() {
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
        let tmp = tempfile::tempdir().unwrap();
        let outcome = run_shell(tmp.path(), "echo hi; exit 3", Duration::from_secs(10)).unwrap();
        assert!(!outcome.timed_out);
        assert_eq!(outcome.exit_code, Some(3));
        // run_shell returns the raw combined output (the bash tool wrapper
        // appends the exit-code line); stdout keeps its trailing newline.
        assert_eq!(outcome.output, "hi\n");
    }
}
