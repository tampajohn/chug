//! T83 — F3 hooks phase 1: `.chug/hooks.json` (PreToolUse veto + PostToolUse
//! advisory).
//!
//! Policy-as-config: operator shell commands fire around every tool call.
//! `PreToolUse` runs before a tool executes — a non-zero hook exit vetoes the
//! call with a tool error the model routes around (risk-gate shape);
//! `PostToolUse` runs after (ok or error) and its stdout+stderr is appended
//! to the tool result as an advisory the model reads. Stop/GoalComplete
//! events, arg-glob matchers, and the Laya stop-hook consumer are phase 2.
//!
//! Hard rules, all fail-open — hooks are policy, never a run-killer:
//! - absent/empty config or empty event lists → zero hooks, zero cost;
//! - malformed/unreadable config → warn once per run on stderr + one
//!   events.jsonl error line, then run with zero hooks;
//! - spawn failure / timeout → warn (latched) + one error line, call allowed.
//!
//! Every hook runs `sh -c <command>` in its OWN process group (the T6
//! bash-tool pattern: the timeout SIGKILLs the whole group, so a hook's
//! grandchildren cannot outlive it), cwd = the run cwd, a JSON payload on
//! stdin, bounded by a 10s wall cap.

use std::fs;
use std::io::{Read, Write};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::anyhow;
use serde_json::{Value, json};

use crate::events::{Event, EventSink};
use crate::tools::kill_process_group;

/// The hook wall cap: a hook that runs longer is killed (whole group) and
/// the call fails open.
const HOOK_TIMEOUT: Duration = Duration::from_secs(10);
/// Hook output (veto stderr / advisory text) is tail-anchored to this many
/// chars — the T25 shape (failure bytes cluster at the end).
const HOOK_OUTPUT_MAX_CHARS: usize = 2000;
/// Grace for the pipe-reader threads after the hook exits or is killed
/// (same discipline as run_shell: leak the reader, never block the driver).
const HOOK_READER_GRACE: Duration = Duration::from_secs(5);
/// Poll interval while waiting for the hook to exit.
const HOOK_POLL: Duration = Duration::from_millis(25);
/// `sh -c` exit code for "command not found": the hook does not exist —
/// a hook-side error (fail-open), never a veto. A legitimate hook could
/// exit 127 by coincidence, but 127 is the shell's own not-found convention.
const EXIT_NOT_FOUND: i32 = 127;

/// One configured hook: a tool-name glob and the command to run.
#[derive(Debug, Clone, PartialEq)]
pub struct HookEntry {
    /// Glob on the tool name (`*`/`?`; `mcp__*` matches MCP tools by their
    /// registered `mcp__<name>__<tool>` name like any other tool).
    pub match_glob: String,
    pub command: String,
}

/// The loaded hooks config for one run. Phase-1 events only: PreToolUse and
/// PostToolUse. Zero-value = zero hooks = every fire site is one bool check.
#[derive(Debug, Clone)]
pub struct Hooks {
    pre_tool_use: Vec<HookEntry>,
    post_tool_use: Vec<HookEntry>,
    /// Wall cap per hook fire. Test-only seam to shrink (the api.rs
    /// `retry_delays` seam pattern — never a slow wall-clock test).
    timeout: Duration,
    /// stderr warn-once latch, per run: the first hook-side failure warns;
    /// every failure gets its own events.jsonl error line.
    warn_count: usize,
}

impl Default for Hooks {
    fn default() -> Self {
        Hooks {
            pre_tool_use: Vec::new(),
            post_tool_use: Vec::new(),
            timeout: HOOK_TIMEOUT,
            warn_count: 0,
        }
    }
}

impl Hooks {
    /// Zero hooks (plan mode's structural exclusion; also the fail-open
    /// landing spot).
    pub fn empty() -> Self {
        Hooks::default()
    }

    /// Load `.chug/hooks.json` from the run cwd. No search chain, no CLI
    /// flag this phase. Every failure leg fails open with a warn + one
    /// events.jsonl error line; absent/empty config returns zero hooks.
    pub fn load(cwd: &Path, sink: &mut dyn EventSink) -> Hooks {
        let path = hooks_path(cwd);
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            // Absent file: the normal zero-cost leg.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Hooks::empty(),
            Err(e) => {
                return Self::fail_open(
                    &format!("hooks config unreadable {}: {e}", path.display()),
                    sink,
                );
            }
        };
        // Empty file: zero hooks, zero cost, no warn.
        if text.trim().is_empty() {
            return Hooks::empty();
        }
        let value: Value = match serde_json::from_str(&text) {
            Ok(value) => value,
            Err(e) => {
                return Self::fail_open(
                    &format!("hooks config malformed {}: {e}", path.display()),
                    sink,
                );
            }
        };
        match parse_config(&value) {
            Ok((pre, post)) => Hooks {
                pre_tool_use: pre,
                post_tool_use: post,
                ..Hooks::default()
            },
            Err(e) => Self::fail_open(
                &format!("hooks config invalid {}: {e}", path.display()),
                sink,
            ),
        }
    }

    /// The config-error landing spot: warn once on stderr + one error line,
    /// then run with zero hooks.
    fn fail_open(detail: &str, sink: &mut dyn EventSink) -> Hooks {
        eprintln!("chug: warning: {detail}; running with zero hooks");
        sink.emit(Event::HookError {
            detail: detail.to_string(),
        });
        let mut hooks = Hooks::empty();
        hooks.warn_count = 1;
        hooks
    }

    pub fn is_empty(&self) -> bool {
        self.pre_tool_use.is_empty() && self.post_tool_use.is_empty()
    }

    /// PreToolUse: before a tool executes, fire every matching hook. `Ok(())`
    /// = allow; `Err(message)` = veto (the tool does NOT execute; the
    /// message is the tool-error text the model sees). Every matching hook
    /// fires even after one vetoes; the first veto's message wins.
    pub fn pre_tool_use(
        &mut self,
        cwd: &Path,
        tool: &str,
        input: &Value,
        sink: &mut dyn EventSink,
    ) -> Result<(), String> {
        let entries = self.pre_tool_use.clone();
        let mut veto: Option<String> = None;
        for entry in entries {
            if !glob_matches(&entry.match_glob, tool) {
                continue;
            }
            let payload = json!({
                "event": "PreToolUse",
                "tool": tool,
                "input": input,
                "cwd": cwd.display().to_string(),
            });
            let t0 = Instant::now();
            match run_hook(cwd, &entry.command, &payload, self.timeout) {
                Ok(outcome) if !outcome.timed_out && outcome.exit == Some(0) => {
                    Self::emit_fired(
                        sink,
                        "PreToolUse",
                        tool,
                        &entry.command,
                        outcome.exit,
                        false,
                        t0,
                    );
                }
                Ok(outcome)
                    if !outcome.timed_out
                        && outcome.exit.is_some_and(|c| c != 0 && c != EXIT_NOT_FOUND) =>
                {
                    // Non-zero exit → VETO: the tool does not execute.
                    Self::emit_fired(
                        sink,
                        "PreToolUse",
                        tool,
                        &entry.command,
                        outcome.exit,
                        true,
                        t0,
                    );
                    if veto.is_none() {
                        veto = Some(format!(
                            "[hook veto] {}",
                            tail_chars(outcome.stderr.trim_end(), HOOK_OUTPUT_MAX_CHARS)
                        ));
                    }
                }
                // 127 = `sh` could not find the command (the hook does not
                // exist); a `None` exit = the hook died by signal. Both are
                // hook-side errors → fail-open, allow the call.
                Ok(outcome) if !outcome.timed_out => {
                    let detail = if outcome.exit == Some(EXIT_NOT_FOUND) {
                        format!(
                            "hook not found (exit {EXIT_NOT_FOUND}): {} (PreToolUse {tool}); allowing call",
                            entry.command
                        )
                    } else {
                        format!(
                            "hook exited without a code: {} (PreToolUse {tool}); allowing call",
                            entry.command
                        )
                    };
                    self.hook_error(sink, &detail);
                }
                // Timeout: the group was killed; fail open, allow the call.
                Ok(_) => {
                    self.hook_error(
                        sink,
                        &format!(
                            "hook timed out after {}s: {} (PreToolUse {}); allowing call",
                            self.timeout.as_secs(),
                            entry.command,
                            tool
                        ),
                    );
                }
                Err(e) => self.hook_error(
                    sink,
                    &format!("hook failed: {e} (PreToolUse {} {tool}); allowing call", entry.command),
                ),
            }
        }
        match veto {
            Some(message) => Err(message),
            None => Ok(()),
        }
    }

    /// PostToolUse: after a tool executed (ok or error), fire every matching
    /// hook. Advisory only: non-empty stdout+stderr is appended to the
    /// result content as `\n\n[hook] <text>` (with ` (exit <n>)` when
    /// non-zero). Never blocks, never changes ok/is_error, never re-fires
    /// (a hook does not trigger hooks).
    pub fn post_tool_use(
        &mut self,
        cwd: &Path,
        tool: &str,
        input: &Value,
        is_error: bool,
        result_content: &mut String,
        sink: &mut dyn EventSink,
    ) {
        let entries = self.post_tool_use.clone();
        for entry in entries {
            if !glob_matches(&entry.match_glob, tool) {
                continue;
            }
            let payload = json!({
                "event": "PostToolUse",
                "tool": tool,
                "input": input,
                "cwd": cwd.display().to_string(),
                "is_error": is_error,
            });
            let t0 = Instant::now();
            match run_hook(cwd, &entry.command, &payload, self.timeout) {
                Ok(outcome) if !outcome.timed_out => {
                    Self::emit_fired(
                        sink,
                        "PostToolUse",
                        tool,
                        &entry.command,
                        outcome.exit,
                        false,
                        t0,
                    );
                    // A nonexistent hook (sh exit 127) is a hook-side error,
                    // not a note: fail open, advisory dropped.
                    if outcome.exit == Some(EXIT_NOT_FOUND) {
                        self.hook_error(
                            sink,
                            &format!(
                                "hook not found (exit {EXIT_NOT_FOUND}): {} (PostToolUse {tool}); advisory dropped",
                                entry.command
                            ),
                        );
                        continue;
                    }
                    let text = combine_out_err(&outcome.stdout, &outcome.stderr);
                    let text = text.trim_end();
                    if !text.is_empty() {
                        let note = tail_chars(text, HOOK_OUTPUT_MAX_CHARS);
                        result_content.push_str("\n\n[hook] ");
                        result_content.push_str(&note);
                        match outcome.exit {
                            // Zero-exit advisory: the note, verbatim.
                            Some(0) => {}
                            Some(code) => {
                                result_content.push_str(&format!(" (exit {code})"));
                            }
                            None => result_content.push_str(" (exit none)"),
                        }
                    }
                }
                // Timeout: the group was killed; fail open, advisory dropped.
                Ok(_) => {
                    self.hook_error(
                        sink,
                        &format!(
                            "hook timed out after {}s: {} (PostToolUse {}); advisory dropped",
                            self.timeout.as_secs(),
                            entry.command,
                            tool
                        ),
                    );
                }
                Err(e) => self.hook_error(
                    sink,
                    &format!(
                        "hook failed: {e} (PostToolUse {} {tool}); advisory dropped",
                        entry.command
                    ),
                ),
            }
        }
    }

    /// One line per hook fire: the fire record lands in
    /// `.chug/events.jsonl` (type "hook") with the event, tool, command,
    /// exit code, veto flag, and duration.
    fn emit_fired(
        sink: &mut dyn EventSink,
        event: &str,
        tool: &str,
        command: &str,
        exit: Option<i32>,
        veto: bool,
        t0: Instant,
    ) {
        sink.emit(Event::HookFired {
            event: event.to_string(),
            tool: tool.to_string(),
            command: command.to_string(),
            exit,
            veto,
            duration_ms: t0.elapsed().as_millis() as u64,
        });
    }

    /// A hook-side failure: warn ONCE per run on stderr (latched), one
    /// events.jsonl error line per occurrence, call allowed (fail-open).
    fn hook_error(&mut self, sink: &mut dyn EventSink, detail: &str) {
        if self.warn_count == 0 {
            self.warn_count += 1;
            eprintln!("chug: warning: hooks: {detail}");
        }
        sink.emit(Event::HookError {
            detail: detail.to_string(),
        });
    }

    /// Test-only seam: shrink the hook wall cap so the timeout leg kills
    /// without a slow wall-clock test (the api.rs retry_delays pattern).
    #[cfg(test)]
    pub(crate) fn set_timeout_for_tests(&mut self, timeout: Duration) {
        self.timeout = timeout;
    }

    /// Test-only: how many stderr warns fired (asserts the once latch).
    #[cfg(test)]
    pub(crate) fn warn_count(&self) -> usize {
        self.warn_count
    }

    /// Test-only: the configured PreToolUse entries in config order.
    #[cfg(test)]
    pub(crate) fn pre_entries(&self) -> &[HookEntry] {
        &self.pre_tool_use
    }

    /// Test-only: the configured PostToolUse entries in config order.
    #[cfg(test)]
    pub(crate) fn post_entries(&self) -> &[HookEntry] {
        &self.post_tool_use
    }
}

/// The config path: `<cwd>/.chug/hooks.json` (gitignored, per-checkout
/// operator config; a worktree child has its own).
pub fn hooks_path(cwd: &Path) -> PathBuf {
    cwd.join(".chug").join("hooks.json")
}

/// Parse the `{"hooks": {"PreToolUse": [...], "PostToolUse": [...]}}` shape.
/// An absent `hooks` key or absent/empty event lists = zero hooks. Wrong
/// shapes are config errors (fail-open upstream), never panics.
fn parse_config(value: &Value) -> anyhow::Result<(Vec<HookEntry>, Vec<HookEntry>)> {
    let obj = value
        .as_object()
        .ok_or_else(|| anyhow!("expected a JSON object"))?;
    let Some(hooks) = obj.get("hooks") else {
        return Ok((Vec::new(), Vec::new()));
    };
    let hooks = hooks
        .as_object()
        .ok_or_else(|| anyhow!("\"hooks\" must be an object"))?;
    let parse_list = |key: &str| -> anyhow::Result<Vec<HookEntry>> {
        let Some(list) = hooks.get(key) else {
            return Ok(Vec::new());
        };
        let list = list
            .as_array()
            .ok_or_else(|| anyhow!("\"hooks.{key}\" must be an array"))?;
        let mut out = Vec::with_capacity(list.len());
        for (i, entry) in list.iter().enumerate() {
            let entry = entry
                .as_object()
                .ok_or_else(|| anyhow!("hooks.{key}[{i}] must be an object"))?;
            let command = entry
                .get("command")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("hooks.{key}[{i}].command must be a string"))?;
            let match_glob = entry
                .get("match")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("hooks.{key}[{i}].match must be a string"))?;
            out.push(HookEntry {
                match_glob: match_glob.to_string(),
                command: command.to_string(),
            });
        }
        Ok(out)
    };
    Ok((parse_list("PreToolUse")?, parse_list("PostToolUse")?))
}

/// Glob match on the tool name (`*`/`?` semantics). An unparsable glob
/// matches nothing — deterministic, and fail-closed for that entry only.
fn glob_matches(pattern: &str, tool: &str) -> bool {
    match glob::Pattern::new(pattern) {
        Ok(p) => p.matches(tool),
        Err(_) => false,
    }
}

/// What one hook run produced.
struct HookOutcome {
    /// `None` when the hook died without an exit code (signal) or was
    /// killed at the wall cap.
    exit: Option<i32>,
    stdout: String,
    stderr: String,
    timed_out: bool,
}

/// Run one hook: `sh -c <command>` in its own process group, cwd = the run
/// cwd, JSON on stdin, bounded by `timeout` (expiry SIGKILLs the whole
/// group — a plain child kill would orphan grandchildren that hold the
/// pipes). `Err` = spawn/wait failure (hook-side error → fail-open).
fn run_hook(
    cwd: &Path,
    command: &str,
    stdin_json: &Value,
    timeout: Duration,
) -> Result<HookOutcome, String> {
    let mut cmd = Command::new("sh");
    cmd.arg("-c")
        .arg(command)
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    cmd.process_group(0);
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("spawning `sh -c {command}`: {e}"))?;

    // The payload can exceed the pipe buffer, so the write runs on its own
    // thread; once the hook exits (or is killed) the pipe closes and an
    // unfinished write fails with EPIPE instead of blocking the caller.
    let payload = serde_json::to_vec(stdin_json).unwrap_or_default();
    let mut stdin = child.stdin.take().ok_or("hook stdin not captured")?;
    let stdin_writer = thread::spawn(move || {
        let _ = stdin.write_all(&payload);
    });

    // Readers hand their buffers over a channel instead of being joined, so
    // a stuck reader (an orphan holding the pipe) can never block the caller.
    let (out_tx, out_rx) = mpsc::channel::<Vec<u8>>();
    let (err_tx, err_rx) = mpsc::channel::<Vec<u8>>();
    let mut out_pipe = child.stdout.take().ok_or("hook stdout not captured")?;
    let mut err_pipe = child.stderr.take().ok_or("hook stderr not captured")?;
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
    let exit;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                exit = status.code();
                break;
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    kill_process_group(&mut child);
                    let _ = child.wait();
                    exit = None;
                    timed_out = true;
                    break;
                }
                thread::sleep(HOOK_POLL);
            }
            Err(e) => return Err(format!("waiting for hook `sh -c {command}`: {e}")),
        }
    }
    let _ = stdin_writer.join();

    let out_waiter = thread::spawn(move || recv_capped(out_rx));
    let err_waiter = thread::spawn(move || recv_capped(err_rx));
    let (stdout, _) = out_waiter.join().unwrap_or((Vec::new(), true));
    let (stderr, _) = err_waiter.join().unwrap_or((Vec::new(), true));
    Ok(HookOutcome {
        exit,
        stdout: String::from_utf8_lossy(&stdout).into_owned(),
        stderr: String::from_utf8_lossy(&stderr).into_owned(),
        timed_out,
    })
}

/// Wait up to [`HOOK_READER_GRACE`] for one reader buffer; never blocks
/// longer. Returns the buffer (dropped when the grace expired with the pipe
/// still held open by an escaped process).
fn recv_capped(rx: mpsc::Receiver<Vec<u8>>) -> (Vec<u8>, bool) {
    match rx.recv_timeout(HOOK_READER_GRACE) {
        Ok(buf) => (buf, true),
        Err(mpsc::RecvTimeoutError::Timeout) => (Vec::new(), false),
        Err(mpsc::RecvTimeoutError::Disconnected) => (Vec::new(), true),
    }
}

/// stdout then stderr, newline-separated when both are non-empty (the
/// run_shell combine shape).
fn combine_out_err(stdout: &str, stderr: &str) -> String {
    match (stdout.is_empty(), stderr.is_empty()) {
        (true, true) => String::new(),
        (false, true) => stdout.to_string(),
        (true, false) => stderr.to_string(),
        (false, false) => format!("{stdout}\n{stderr}"),
    }
}

/// Tail-anchored window, char-boundary safe: always `chars()`, never byte
/// slicing (the T25 shape).
fn tail_chars(s: &str, max_chars: usize) -> String {
    let count = s.chars().count();
    if count <= max_chars {
        s.to_string()
    } else {
        s.chars().skip(count - max_chars).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Records emitted events so tests can assert on them.
    #[derive(Default)]
    struct RecordingSink(Vec<Event>);
    impl EventSink for RecordingSink {
        fn emit(&mut self, e: Event) {
            self.0.push(e);
        }
    }

    impl RecordingSink {
        fn fired(&self) -> Vec<&Event> {
            self.0.iter().filter(|e| matches!(e, Event::HookFired { .. })).collect()
        }
        fn errors(&self) -> Vec<String> {
            self.0
                .iter()
                .filter_map(|e| match e {
                    Event::HookError { detail } => Some(detail.clone()),
                    _ => None,
                })
                .collect()
        }
    }

    fn write_hooks_config(cwd: &Path, pre: Value, post: Value) {
        let dir = cwd.join(".chug");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            hooks_path(cwd),
            json!({"hooks": {"PreToolUse": pre, "PostToolUse": post}}).to_string(),
        )
        .unwrap();
    }

    fn entry(glob: &str, command: &str) -> Value {
        json!({"match": glob, "command": command})
    }

    // ---------- config load legs ----------

    #[test]
    fn absent_config_is_zero_hooks_and_zero_cost() {
        let tmp = tempfile::tempdir().unwrap();
        let mut sink = RecordingSink::default();
        let hooks = Hooks::load(tmp.path(), &mut sink);
        assert!(hooks.is_empty());
        assert!(hooks.pre_entries().is_empty() && hooks.post_entries().is_empty());
        assert!(sink.0.is_empty(), "no events for an absent config");
    }

    #[test]
    fn empty_config_file_is_zero_hooks_without_a_warn() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join(".chug")).unwrap();
        fs::write(hooks_path(tmp.path()), "").unwrap();
        let mut sink = RecordingSink::default();
        let hooks = Hooks::load(tmp.path(), &mut sink);
        assert!(hooks.is_empty());
        assert_eq!(hooks.warn_count(), 0, "empty file: zero cost, no warn");
        assert!(sink.0.is_empty());
    }

    /// Mutant killed: "malformed config aborts the run" and "malformed
    /// config silently ignored with no telemetry" both die — fail-open with
    /// exactly one error line (spec: warn ONCE + one events.jsonl line).
    #[test]
    fn malformed_config_fails_open_with_one_error_event() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join(".chug")).unwrap();
        fs::write(hooks_path(tmp.path()), "{ not json !!!").unwrap();
        let mut sink = RecordingSink::default();
        let hooks = Hooks::load(tmp.path(), &mut sink);
        assert!(hooks.is_empty(), "malformed config runs with zero hooks");
        let errors = sink.errors();
        assert_eq!(errors.len(), 1, "exactly one error line: {errors:?}");
        assert!(errors[0].contains("malformed"), "{errors:?}");
        assert_eq!(hooks.warn_count(), 1, "stderr warn fired once");
    }

    #[test]
    fn unreadable_config_fails_open_with_one_error_event() {
        let tmp = tempfile::tempdir().unwrap();
        // A DIRECTORY where the file should be: read fails, not NotFound.
        fs::create_dir_all(hooks_path(tmp.path())).unwrap();
        let mut sink = RecordingSink::default();
        let hooks = Hooks::load(tmp.path(), &mut sink);
        assert!(hooks.is_empty());
        assert_eq!(sink.errors().len(), 1);
        assert!(sink.errors()[0].contains("unreadable"), "{:?}", sink.errors());
    }

    #[test]
    fn absent_hooks_key_and_empty_lists_are_zero_hooks() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join(".chug")).unwrap();
        for text in ["{}", r#"{"hooks": {}}"#, r#"{"hooks": {"PreToolUse": []}}"#] {
            fs::write(hooks_path(tmp.path()), text).unwrap();
            let mut sink = RecordingSink::default();
            let hooks = Hooks::load(tmp.path(), &mut sink);
            assert!(hooks.is_empty(), "{text}");
            assert!(sink.0.is_empty(), "{text}");
        }
    }

    /// Shape errors are config errors (fail-open), never panics.
    #[test]
    fn wrong_shaped_config_is_an_error_not_a_panic() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join(".chug")).unwrap();
        for text in [
            r#"{"hooks": "no"}"#,
            r#"{"hooks": {"PreToolUse": "no"}}"#,
            r#"{"hooks": {"PreToolUse": [{"command": "exit 0"}]}}"#,
            r#"{"hooks": {"PreToolUse": [{"match": "bash"}]}}"#,
            r#"{"hooks": {"PreToolUse": ["no"]}}"#,
        ] {
            fs::write(hooks_path(tmp.path()), text).unwrap();
            let mut sink = RecordingSink::default();
            let hooks = Hooks::load(tmp.path(), &mut sink);
            assert!(hooks.is_empty(), "{text}");
            assert_eq!(sink.errors().len(), 1, "{text}");
        }
    }

    /// Config order is preserved (spec: "multiple entries same event in
    /// order").
    #[test]
    fn multiple_entries_load_in_config_order() {
        let tmp = tempfile::tempdir().unwrap();
        write_hooks_config(
            tmp.path(),
            json!([entry("bash", "echo one"), entry("edit_*", "echo two"), entry("*", "echo three")]),
            json!([entry("*", "echo post")]),
        );
        let mut sink = RecordingSink::default();
        let hooks = Hooks::load(tmp.path(), &mut sink);
        let cmds: Vec<&str> = hooks.pre_entries().iter().map(|e| e.command.as_str()).collect();
        assert_eq!(cmds, ["echo one", "echo two", "echo three"]);
        assert_eq!(hooks.post_entries().len(), 1);
    }

    // ---------- glob matcher ----------

    /// Fires a `bash`-named tool through a hook set with the spec's matcher
    /// legs; counts which entries fired via the HookFired events.
    fn fired_tools(pre: Value, tool: &str) -> usize {
        let tmp = tempfile::tempdir().unwrap();
        write_hooks_config(tmp.path(), pre, json!([]));
        let mut sink = RecordingSink::default();
        let mut hooks = Hooks::load(tmp.path(), &mut sink);
        hooks
            .pre_tool_use(tmp.path(), tool, &json!({}), &mut sink)
            .unwrap_or(());
        sink.fired().len()
    }

    #[test]
    fn glob_matcher_legs() {
        // exact name
        assert_eq!(fired_tools(json!([entry("bash", "exit 0")]), "bash"), 1);
        // non-match
        assert_eq!(fired_tools(json!([entry("bash", "exit 0")]), "edit_file"), 0);
        // edit_* prefix glob
        assert_eq!(fired_tools(json!([entry("edit_*", "exit 0")]), "edit_file"), 1);
        assert_eq!(fired_tools(json!([entry("edit_*", "exit 0")]), "bash"), 0);
        // mcp__* matches MCP tools by their registered name, no special case
        assert_eq!(fired_tools(json!([entry("mcp__*", "exit 0")]), "mcp__fake__echo"), 1);
        assert_eq!(fired_tools(json!([entry("mcp__*", "exit 0")]), "bash"), 0);
        // * matches everything, including mcp__ names
        assert_eq!(fired_tools(json!([entry("*", "exit 0")]), "bash"), 1);
        assert_eq!(fired_tools(json!([entry("*", "exit 0")]), "mcp__fake__echo"), 1);
        // ? single-char glob
        assert_eq!(fired_tools(json!([entry("bas?", "exit 0")]), "bash"), 1);
        // an unparsable glob matches nothing (deterministic, no panic)
        assert_eq!(fired_tools(json!([entry("[", "exit 0")]), "bash"), 0);
    }

    // ---------- PreToolUse ----------

    /// The allow leg: exit 0 → Ok(()), fire line with veto=false.
    #[test]
    fn pre_tool_use_exit_zero_allows() {
        let tmp = tempfile::tempdir().unwrap();
        write_hooks_config(tmp.path(), json!([entry("bash", "exit 0")]), json!([]));
        let mut sink = RecordingSink::default();
        let mut hooks = Hooks::load(tmp.path(), &mut sink);
        let decision = hooks.pre_tool_use(tmp.path(), "bash", &json!({"command": "ls"}), &mut sink);
        assert!(decision.is_ok(), "exit 0 must allow: {decision:?}");
        let &Event::HookFired { ref event, ref tool, exit, veto, .. } = sink.fired().into_iter().next().unwrap() else {
            panic!("expected HookFired, got {:?}", sink.0);
        };
        assert_eq!(event, "PreToolUse");
        assert_eq!(tool, "bash");
        assert_eq!(exit, Some(0));
        assert!(!veto);
    }

    /// The veto leg — kills the allow-by-default mutant: a non-zero hook
    /// exit must produce `[hook veto] ` + the hook's STDERR (stdout
    /// excluded).
    #[test]
    fn pre_tool_use_nonzero_exit_vetoes_with_stderr_tail() {
        let tmp = tempfile::tempdir().unwrap();
        write_hooks_config(
            tmp.path(),
            json!([entry("bash", "echo STDOUT-NOISE; echo no-marker-commands-allowed >&2; exit 2")]),
            json!([]),
        );
        let mut sink = RecordingSink::default();
        let mut hooks = Hooks::load(tmp.path(), &mut sink);
        let decision = hooks.pre_tool_use(tmp.path(), "bash", &json!({"command": "ls"}), &mut sink);
        let Err(veto_message) = decision else {
            panic!("non-zero exit must veto");
        };
        assert!(veto_message.starts_with("[hook veto] "), "{veto_message:?}");
        assert!(veto_message.contains("no-marker-commands-allowed"), "{veto_message:?}");
        assert!(!veto_message.contains("STDOUT-NOISE"), "veto text is stderr only: {veto_message:?}");
        let &Event::HookFired { exit, veto, .. } = sink.fired().into_iter().next().unwrap() else {
            panic!("expected HookFired, got {:?}", sink.0);
        };
        assert_eq!(exit, Some(2));
        assert!(veto);
    }

    /// The stdin contract: the hook receives
    /// `{"event","tool","input","cwd"}` JSON on stdin (the hook `cat`s its
    /// stdin into a file the test reads back).
    #[test]
    fn pre_tool_use_hook_receives_json_payload_on_stdin() {
        let tmp = tempfile::tempdir().unwrap();
        write_hooks_config(tmp.path(), json!([entry("bash", "cat > captured.json; exit 0")]), json!([]));
        let mut sink = RecordingSink::default();
        let mut hooks = Hooks::load(tmp.path(), &mut sink);
        let input = json!({"command": "echo hi", "timeout": 12});
        hooks.pre_tool_use(tmp.path(), "bash", &input, &mut sink).unwrap();
        let captured: Value = serde_json::from_str(
            &fs::read_to_string(tmp.path().join("captured.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(captured["event"], "PreToolUse");
        assert_eq!(captured["tool"], "bash");
        assert_eq!(captured["input"], input, "the tool call input object verbatim");
        assert_eq!(captured["cwd"], tmp.path().display().to_string());
    }

    /// Fail-open legs: a nonexistent hook command and a hook that dies
    /// without an exit code allow the call; the stderr warn latches at ONE
    /// while every occurrence gets its own error line.
    #[test]
    fn pre_tool_use_spawn_failure_fails_open_warns_once() {
        let tmp = tempfile::tempdir().unwrap();
        write_hooks_config(tmp.path(), json!([entry("bash", "/nonexistent/hook-xyz-0917")]), json!([]));
        let mut sink = RecordingSink::default();
        let mut hooks = Hooks::load(tmp.path(), &mut sink);
        for _ in 0..2 {
            let decision = hooks.pre_tool_use(tmp.path(), "bash", &json!({}), &mut sink);
            assert!(decision.is_ok(), "spawn failure fails open: {decision:?}");
        }
        assert_eq!(sink.errors().len(), 2, "one error line per failed fire");
        assert_eq!(hooks.warn_count(), 1, "stderr warn once per run");
    }

    /// The timeout leg — kills the unbounded-wait mutant AND the plain-spawn
    /// mutant: the wall cap SIGKILLs the whole process group, so the
    /// backgrounded grandchild cannot hold the pipes (a plain child kill
    /// would block the readers for the full 5s grace) and the call fails
    /// OPEN (allowed) with exactly one error event.
    #[test]
    fn pre_tool_use_timeout_kills_group_fails_open() {
        let tmp = tempfile::tempdir().unwrap();
        write_hooks_config(
            tmp.path(),
            json!([entry("bash", "sleep 60 & sleep 60")]), // grandchild + child
            json!([]),
        );
        let mut sink = RecordingSink::default();
        let mut hooks = Hooks::load(tmp.path(), &mut sink);
        hooks.set_timeout_for_tests(Duration::from_millis(300));
        let start = Instant::now();
        let decision = hooks.pre_tool_use(tmp.path(), "bash", &json!({}), &mut sink);
        let elapsed = start.elapsed();
        assert!(decision.is_ok(), "timeout fails open: {decision:?}");
        assert!(
            elapsed < Duration::from_secs(4),
            "group kill must return promptly, took {elapsed:?} (plain child kill blocks ~5s in reader grace)"
        );
        let errors = sink.errors();
        assert_eq!(errors.len(), 1, "one error event for the timeout: {errors:?}");
        assert!(errors[0].contains("timed out"), "{errors:?}");
        assert_eq!(hooks.warn_count(), 1);
        // No fire line for a timed-out hook (it never produced a verdict).
        assert!(sink.fired().is_empty());
    }

    // ---------- PostToolUse ----------

    /// The advisory leg: hook stdout lands in the result content as
    /// `\n\n[hook] <text>`; a non-zero exit is noted after the text.
    #[test]
    fn post_tool_use_appends_note_and_exit_suffix() {
        let tmp = tempfile::tempdir().unwrap();
        write_hooks_config(tmp.path(), json!([]), json!([entry("bash", "echo post-note-from-hook; exit 3")]));
        let mut sink = RecordingSink::default();
        let mut hooks = Hooks::load(tmp.path(), &mut sink);
        let mut content = "tool output".to_string();
        hooks.post_tool_use(tmp.path(), "bash", &json!({}), false, &mut content, &mut sink);
        assert_eq!(content, "tool output\n\n[hook] post-note-from-hook (exit 3)");
        let &Event::HookFired { ref event, exit, veto, .. } = sink.fired().into_iter().next().unwrap() else {
            panic!("expected HookFired, got {:?}", sink.0);
        };
        assert_eq!(event, "PostToolUse");
        assert_eq!(exit, Some(3));
        assert!(!veto, "PostToolUse never vetoes");
    }

    #[test]
    fn post_tool_use_zero_exit_note_has_no_suffix() {
        let tmp = tempfile::tempdir().unwrap();
        write_hooks_config(tmp.path(), json!([]), json!([entry("*", "echo clean-note")]));
        let mut sink = RecordingSink::default();
        let mut hooks = Hooks::load(tmp.path(), &mut sink);
        let mut content = "out".to_string();
        hooks.post_tool_use(tmp.path(), "bash", &json!({}), false, &mut content, &mut sink);
        assert_eq!(content, "out\n\n[hook] clean-note");
    }

    /// Empty hook output appends nothing — not even the separator.
    #[test]
    fn post_tool_use_empty_output_appends_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        write_hooks_config(tmp.path(), json!([]), json!([entry("bash", "exit 0")]));
        let mut sink = RecordingSink::default();
        let mut hooks = Hooks::load(tmp.path(), &mut sink);
        let mut content = "out".to_string();
        hooks.post_tool_use(tmp.path(), "bash", &json!({}), false, &mut content, &mut sink);
        assert_eq!(content, "out");
        assert_eq!(sink.fired().len(), 1, "the fire still records");
    }

    /// Both result legs fire (ok AND error), with `is_error` in the payload.
    #[test]
    fn post_tool_use_fires_on_ok_and_error_results_with_is_error() {
        let tmp = tempfile::tempdir().unwrap();
        write_hooks_config(tmp.path(), json!([]), json!([entry("bash", "cat > seen-POST.json; exit 0")]));
        let mut sink = RecordingSink::default();
        let mut hooks = Hooks::load(tmp.path(), &mut sink);
        let mut content = String::new();
        hooks.post_tool_use(tmp.path(), "bash", &json!({}), false, &mut content, &mut sink);
        hooks.post_tool_use(tmp.path(), "bash", &json!({}), true, &mut content, &mut sink);
        let seen: Value = serde_json::from_str(
            &fs::read_to_string(tmp.path().join("seen-POST.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(seen["event"], "PostToolUse");
        assert_eq!(seen["is_error"], true, "the second fire's payload");
        assert_eq!(sink.fired().len(), 2);
    }

    /// Advisory never changes the caller's ok/is_error: a veto-shaped
    /// post hook (non-zero exit, no output) leaves the content untouched.
    #[test]
    fn post_tool_use_cannot_veto_or_alter_ok() {
        let tmp = tempfile::tempdir().unwrap();
        write_hooks_config(tmp.path(), json!([]), json!([entry("bash", "exit 9")]));
        let mut sink = RecordingSink::default();
        let mut hooks = Hooks::load(tmp.path(), &mut sink);
        let mut content = "unchanged".to_string();
        hooks.post_tool_use(tmp.path(), "bash", &json!({}), false, &mut content, &mut sink);
        assert_eq!(content, "unchanged", "no output → nothing appended");
    }

    /// Tail anchoring is char-safe (the T25 shape).
    #[test]
    fn tail_chars_is_char_boundary_safe() {
        assert_eq!(tail_chars("short", 10), "short");
        let s: String = "é".repeat(2500);
        let t = tail_chars(&s, 2000);
        assert_eq!(t.chars().count(), 2000);
        assert!(t.starts_with("é"));
    }

    #[test]
    fn combine_out_err_shapes() {
        assert_eq!(combine_out_err("", ""), "");
        assert_eq!(combine_out_err("a", ""), "a");
        assert_eq!(combine_out_err("", "b"), "b");
        assert_eq!(combine_out_err("a", "b"), "a\nb");
    }
}
