//! T71 — the `delegate` tool: launch, observe, and collect a bounded child
//! `chug run`, in a child worktree this loop created. Moved here verbatim from
//! `src/tools.rs`, which had grown to 5,233 lines with eight delegate features
//! stacked in it (T23/T28/T29/T39/T58/T61/T68/T69). Registration stays in
//! `src/tools.rs`: the schema json! in `tool_schemas()` (the one cross-cutting
//! surface other tools' pins reference) and the `"delegate" =>` arm in
//! `inner()`, which calls [`delegate`] across the module boundary.

use std::ffi::OsString;
use std::fs;
use std::io::{Read, Seek};
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, anyhow, bail};
use serde_json::Value;

use crate::tools::{ToolCtx, ToolResult, get_str};

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
/// is harmless and returns promptly on a significant change (T68:
/// `last_event` churn is filtered out before the wake fires).
const DELEGATE_WAIT_POLL: Duration = Duration::from_millis(2500);
/// T69: the commit-refs block of `collect` is a bounded `git log --oneline` —
/// this is the hard line cap, for both the default range and a `<base>..HEAD`
/// range, so a child worktree with a huge history can never flood the result.
const DELEGATE_COLLECT_COMMIT_CAP: usize = 20;

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
pub(crate) fn delegate(_ctx: &ToolCtx, input: &Value) -> anyhow::Result<ToolResult> {
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
        "collect" => {
            // T69: the wait knob is status-only here too — launch-leg parity,
            // naming that beats silently ignoring it. `collect` NEVER blocks
            // (spec req 4): the caller long-polls with `status` + `wait_secs`
            // first, then collects the structured result in one bounded read.
            if input.get("wait_secs").is_some() {
                bail!(
                    "delegate: `wait_secs` applies to the status action only — collect never blocks or waits; long-poll with status first, then collect"
                );
            }
            delegate_collect(input)
        }
        other => bail!(
            "delegate: unknown action {other:?} (expected \"launch\", \"status\", or \"collect\")"
        ),
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
///
/// T58: `resume == true` appends the bare `--resume` flag as the new tail
/// (after any `--max-tokens` pair), purely additive to the argv — the abort
/// output's own resume line shows spec/goal/model (and any budgets) are all
/// still passed on a resume. Absent/false produces the pre-T58 argv
/// byte-for-byte (whole-list pinned).
fn delegate_child_argv(
    spec: &Path,
    goal: &str,
    model: &str,
    max_iters: u64,
    max_minutes: u64,
    max_tokens: Option<u64>,
    resume: bool,
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
    if resume {
        argv.push(OsString::from("--resume"));
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

/// T58: parse the optional launch-only `resume` (continue the child's prior
/// run from its `.chug/transcript.jsonl` via `--resume`, instead of starting
/// fresh). Absent or `false` → `false` (child starts fresh, exactly as
/// before); `true` → `true`. A non-boolean value is a tool error, never a
/// silent ignore — a caller that asked to resume and got a fresh child would
/// silently throw away the prior run's context, which is the one failure this
/// flag exists to prevent.
fn delegate_resume(input: &Value) -> anyhow::Result<bool> {
    match input.get("resume") {
        None | Some(Value::Null) => Ok(false),
        Some(value) => value.as_bool().ok_or_else(|| {
            anyhow!("delegate: `resume` must be a boolean (true = continue the child's prior run with --resume)")
        }),
    }
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
    let resume = delegate_resume(input)?;

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
        resume,
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

    // T39/T58: the configured token budget and the resume leg are echoed back
    // only when set — absent, the return text is byte-identical to pre-T39.
    let tokens_note = match max_tokens {
        Some(tokens) => format!(" max_tokens: {tokens}"),
        None => String::new(),
    };
    let resume_note = if resume { " resume: true" } else { "" };
    Ok(ToolResult {
        content: format!(
            "launched: pid {pid}\nlog: {}\nevents: {}\nmodel: {model} max_iters: {max_iters} max_minutes: {max_minutes}{tokens_note}{resume_note}",
            log_path.display(),
            chug_dir.join("events.jsonl").display(),
        ),
        is_error: false,
    })
}

/// Observe a previously launched child. Without `wait_secs` (or with `0`)
/// this is the pre-T29 instant render, byte-identical for identical state
/// (pinned by test); with `wait_secs > 0` it is a bounded long-poll that
/// blocks until the first significant change (T68 — iteration advance or
/// verdict/budget-low flag; `last_event` churn never wakes), a liveness
/// flip, or the deadline.
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

/// One non-blocking bounded tail read of the child's events log — the shared
/// source for both the `status` summary and the T69 `collect` parse, so
/// neither can grow an unbounded read by accident.
fn read_events_tail(events_path: &Path) -> anyhow::Result<Vec<String>> {
    read_tail_lines(events_path, DELEGATE_EVENTS_TAIL_BYTES)
}

/// One non-blocking read of the child's events tail, shared by both status
/// legs. A missing/unreadable events log is the normal state before a child's
/// first write — reported as an empty summary plus the `events: nothing read`
/// note, never an error (T23 behavior, unchanged).
fn read_events(events_path: &Path) -> (DelegateSummary, Option<String>) {
    match read_events_tail(events_path) {
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

/// T29 + T68: the bounded long-poll. Block until the FIRST of:
/// (a) the child's events-derived state changes SIGNIFICANTLY vs. the
///     snapshot at entry — the wake set is `max_iters`, `last_iteration`,
///     `budget_low_seen`, `goal_seen`, `abort_seen`, `abort_reason`
///     ([`DelegateSummary::significant_ne`]) — or the events file's
///     creation when it was missing at entry (the launch→build window is
///     exactly this state). `last_event_type`/`last_event_ts` churn is
///     deliberately NOT wake-worthy (T68): an active child appends a
///     `tool_result` event every 2–10 s, so the pre-T68 any-field wake
///     fired at the first poll tick almost every time and every
///     `wait_secs: 90–110` long-poll collapsed back into per-tool-call
///     polling. Churn is still RENDERED — the deadline leg's final read
///     carries it — it just never wakes the wait;
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
        // Req 2(a) + T68: a SIGNIFICANT summary-field diff (an iteration
        // advance, max_iters appearing, a budget-low/goal/abort flag or the
        // abort reason — [`DelegateSummary::significant_ne`]), or the events
        // file appearing when it was missing at entry. A diff confined to
        // `last_event_type`/`last_event_ts` (per-tool-call churn) must NOT
        // wake — it would fire at the first poll tick nearly every time.
        let state_changed = now_summary.significant_ne(&entry_summary)
            || (now_existed && !entry_existed);
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
                // T58: a new `run_start` begins a NEW run segment — a resumed
                // child appends a fresh `run_start` + iterations to the same
                // stream after its prior segment died. The verdict latches are
                // segment-scoped, so reset them here: the summary must
                // describe the LATEST segment (a pre-resume abort must not
                // keep a healthy resumed child reporting `aborted`, the
                // cycle-18 bite). `max_iters` and `last_iteration` deliberately
                // keep their last-seen values (the latter until the new
                // segment writes its first iteration) — they are stream-scope
                // observations, not verdicts.
                s.budget_low_seen = false;
                s.goal_seen = false;
                s.abort_seen = false;
                s.abort_reason = None;
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

    /// T68: whether the SIGNIFICANT fields differ from `other` — the wake set
    /// of the `status` long-poll: `max_iters`, `last_iteration`,
    /// `budget_low_seen`, `goal_seen`, `abort_seen`, `abort_reason`.
    /// Deliberately EXCLUDES `last_event_type`/`last_event_ts`: an active
    /// child appends a `tool_result` event every 2–10 s, so the pre-T68
    /// any-field wake fired at the first poll tick almost every time
    /// (cycles 29–30: every `wait_secs: 90–110` long-poll woke at 2–7 s and
    /// pacing fell back to bash sleeps + instant status). Churn stays
    /// RENDERED (the deadline leg's final read) — it just never wakes.
    fn significant_ne(&self, other: &Self) -> bool {
        self.max_iters != other.max_iters
            || self.last_iteration != other.last_iteration
            || self.budget_low_seen != other.budget_low_seen
            || self.goal_seen != other.goal_seen
            || self.abort_seen != other.abort_seen
            || self.abort_reason != other.abort_reason
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

// ---- T69: the `collect` action — a child's structured result ----

/// What `collect` can say about a child's event stream: a SEPARATE
/// collect-side parse of the same bounded tail the `status` summary reads.
/// T68 constraint: nothing here enters [`DelegateSummary`] or its pinned
/// six-field `significant_ne` wake set — the `status` render and long-poll
/// wake behavior stay byte-identical.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct CollectSummary {
    /// The latest segment's verdict latch, in stream order (a later verdict
    /// overwrites an earlier one — a rejected verdict followed by a later
    /// accepted one leaves the segment accepted). `Some` = a verdict line
    /// was seen in the segment.
    verdict_latch: Option<&'static str>,
    /// `summary` of the LATEST accepted `goal` line in the segment — the
    /// child's own account of what it did.
    goal_summary: Option<String>,
    /// `reason` of the latest-segment `abort` line (rendered with the
    /// `aborted` verdict).
    abort_reason: Option<String>,
    /// `cmd` of the LATEST `verifying` line in the segment — the check that
    /// gated the verdict.
    check_cmd: Option<String>,
    /// Any complete, parsable line was seen in the segment (the `running`
    /// signal — vs. `starting` when nothing was read).
    saw_any: bool,
}

impl CollectSummary {
    /// The LATEST segment's terminal state: the verdict latch when one was
    /// seen (`goal-accepted` / `goal-rejected` / `aborted`), `running` when
    /// events exist without a verdict, `starting` when nothing was read.
    fn verdict(&self) -> &'static str {
        match self.verdict_latch {
            Some(v) => v,
            None if self.saw_any => "running",
            None => "starting",
        }
    }
}

/// The parsing/verdict logic of `collect`, with no I/O: every edge case
/// (empty stream, torn last line, missing fields, segment reset) is
/// unit-tested through here. Malformed lines are skipped, never fatal —
/// collecting must be safe at ANY child lifecycle moment.
fn summarize_collect(lines: &[&str]) -> CollectSummary {
    let mut c = CollectSummary::default();
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
        c.saw_any = true;
        match ev_type {
            "run_start" => {
                // T58 segment reset, collect-side: a resumed child appends a
                // fresh `run_start` to the same stream — the verdict, its
                // summary, the abort reason, and the check cmd all describe
                // the LATEST segment, so reset them here (a pre-resume abort
                // or pre-resume gate must not leak into the resume's result).
                c.verdict_latch = None;
                c.goal_summary = None;
                c.abort_reason = None;
                c.check_cmd = None;
            }
            "verifying" => {
                // LATEST `verifying` line wins: the last one is the gate that
                // actually decided the verdict.
                if let Some(cmd) = obj.get("cmd").and_then(Value::as_str) {
                    c.check_cmd = Some(cmd.to_string());
                }
            }
            "goal" => match obj.get("outcome").and_then(Value::as_str) {
                // A rejected verdict latches `goal-rejected` — and a LATER
                // accepted verdict in the same segment flips it (the child's
                // loop continues after a rejection).
                Some("accepted") => {
                    c.verdict_latch = Some("goal-accepted");
                    c.goal_summary = obj
                        .get("summary")
                        .and_then(Value::as_str)
                        .map(str::to_string);
                }
                Some("rejected") => {
                    c.verdict_latch = Some("goal-rejected");
                }
                _ => {}
            },
            "abort" => {
                c.verdict_latch = Some("aborted");
                if let Some(reason) = obj.get("reason").and_then(Value::as_str) {
                    c.abort_reason = Some(reason.to_string());
                }
            }
            _ => {}
        }
    }
    c
}

/// T69: parse the optional collect-only `base` (a git ref scoping the
/// commit-refs range as `<base>..HEAD`). Absent → `None` (the bounded
/// default range over `HEAD`). A non-string value is a tool error, never a
/// silent ignore — a caller that asked for a range must not silently get the
/// default range (T39 `max_tokens` / T58 `resume` parse precedent).
fn delegate_base(input: &Value) -> anyhow::Result<Option<String>> {
    match input.get("base") {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_str()
            .map(|s| Some(s.to_string()))
            .ok_or_else(|| {
                anyhow!(
                    "delegate: `base` must be a string git ref (e.g. \"origin/main\") scoping the commit range <base>..HEAD"
                )
            }),
    }
}

/// The `<base>..HEAD` (or bounded default `HEAD`) range string shared by the
/// git spawn and the render header, so the rendered range always names what
/// was actually queried.
fn commit_range(base: Option<&str>) -> String {
    match base {
        Some(b) => format!("{b}..HEAD"),
        None => "HEAD".to_string(),
    }
}

/// T69: one best-effort bounded `git log --oneline` in the child's cwd — the
/// commit-refs block of `collect`. With `base`, the range is
/// `<base>..HEAD`; without, the bounded default range over `HEAD` (last
/// [`DELEGATE_COLLECT_COMMIT_CAP`] commits). Plain `Command` spawn with
/// captured output (T20 `resolve_head` precedent): EVERY failure leg — no
/// git binary, not a repo, bad ref, nonzero exit — degrades to an
/// `Err(one-line note)`, never a panic, never a tool error, never blocking.
fn collect_git_commits(cwd: &Path, base: Option<&str>) -> Result<Vec<String>, String> {
    let range = commit_range(base);
    let out = Command::new("git")
        .args([
            "log",
            "--oneline",
            "-n",
            &DELEGATE_COLLECT_COMMIT_CAP.to_string(),
            &range,
        ])
        .current_dir(cwd)
        .output()
        .map_err(|e| format!("git not available ({e})"))?;
    if !out.status.success() {
        // The one-line note carries git's own stderr (clipped) when it has
        // one — "not a git repository", "ambiguous argument" — else the
        // bare exit code.
        let stderr = String::from_utf8_lossy(&out.stderr);
        let why: String = stderr.trim().chars().take(200).collect();
        return Err(if why.is_empty() {
            format!(
                "git exited {}",
                out.status.code().unwrap_or(-1)
            )
        } else {
            why
        });
    }
    let text = String::from_utf8_lossy(&out.stdout);
    Ok(text
        .lines()
        .map(str::to_string)
        .filter(|l| !l.trim().is_empty())
        .collect())
}

/// T69: `collect` — a finished (or in-progress) child's structured result in
/// ONE bounded, non-blocking read. The four fields: the LATEST segment's
/// verdict (with the abort reason when aborted), the accepted goal's summary
/// (the child's own account of what it did), the segment's latest check cmd
/// (the gate that decided the verdict), and best-effort commit refs of the
/// child's cwd. Never blocks, never waits — the caller long-polls with
/// `status` + `wait_secs` first. Safe at ANY child lifecycle moment: a
/// missing events log is `starting`, mid-run is `running`, and every git or
/// parse failure leg degrades to a note, never an error.
fn delegate_collect(input: &Value) -> anyhow::Result<ToolResult> {
    let cwd = delegate_cwd(input)?;
    // Same liveness leg `status` has — absent pid → no liveness claim at all.
    let pid = input.get("pid").and_then(Value::as_u64);
    let base = delegate_base(input)?;
    let alive = pid.and_then(reap_and_alive);

    let (summary, events_note) = match read_events_tail(&cwd.join(".chug").join("events.jsonl")) {
        Ok(lines) => {
            let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
            (summarize_collect(&refs), None)
        }
        Err(e) => (
            CollectSummary::default(),
            Some(format!("events: nothing read ({e:#})")),
        ),
    };
    let commits = collect_git_commits(&cwd, base.as_deref());
    Ok(ToolResult {
        content: render_collect(
            &summary,
            alive,
            base.as_deref(),
            &commits,
            events_note.as_deref(),
        ),
        is_error: false,
    })
}

/// The `collect` text body: one `key: value` per field so the caller can grep
/// it. `alive` is rendered ONLY when a pid was given (no pid → no liveness
/// claim, unlike `status`'s always-rendered `alive:` line). The commits
/// header names the queried range, so the caller sees what was bounded.
fn render_collect(
    summary: &CollectSummary,
    alive: Option<bool>,
    base: Option<&str>,
    commits: &Result<Vec<String>, String>,
    events_note: Option<&str>,
) -> String {
    let mut out = format!("verdict: {}", summary.verdict());
    match alive {
        Some(true) => out.push_str("\nalive: true"),
        Some(false) => out.push_str("\nalive: false"),
        None => {}
    }
    if let Some(text) = &summary.goal_summary {
        // The FULL accepted summary, verbatim (it may wrap lines).
        out.push_str("\nsummary: ");
        out.push_str(text);
    }
    if let Some(cmd) = &summary.check_cmd {
        out.push_str(&format!("\ncheck_cmd: {cmd}"));
    }
    match commits {
        Ok(lines) if lines.is_empty() => {
            out.push_str(&format!("\ncommits: (none in range {})", commit_range(base)));
        }
        Ok(lines) => {
            out.push_str(&format!(
                "\ncommits (range {}, up to {DELEGATE_COLLECT_COMMIT_CAP}):",
                commit_range(base)
            ));
            for line in lines {
                out.push_str(&format!("\n  {line}"));
            }
        }
        Err(why) => out.push_str(&format!("\ncommits: (unavailable: {why})")),
    }
    if let Some(note) = events_note {
        out.push_str(&format!("\n{note}"));
    }
    if summary.verdict_latch == Some("aborted")
        && let Some(reason) = &summary.abort_reason
    {
        out.push_str(&format!("\nabort_reason: {reason}"));
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;
    use std::sync::Mutex;
    use serde_json::json;
    use crate::tools::{BASH_TIMEOUT_SECS, dispatch, kill_pid_group, tool_schemas};

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

    /// T58 (a): a resumed child appends a NEW `run_start` + iterations to the
    /// same stream after its first segment aborted — the summary must describe
    /// the LATEST segment (`running`, `abort_seen` reset), not keep the
    /// pre-resume abort latched (the cycle-18 bite that had the orchestrator
    /// fall back to `ps`). `last_iteration` keeps last-seen values: here the
    /// new segment's own iteration 2.
    #[test]
    fn delegate_summary_two_segment_abort_then_run_start_reports_running() {
        let lines = [
            // Segment 1: ran, then died mid-arc.
            "{\"type\":\"run_start\",\"ts\":\"t0\",\"max_iters\":40}",
            "{\"type\":\"iteration\",\"ts\":\"t1\",\"n\":12}",
            "{\"type\":\"abort\",\"ts\":\"t2\",\"reason\":\"llm request failed\",\"model\":\"kimi\"}",
            // Segment 2: the resume — fresh run_start, fresh iterations.
            "{\"type\":\"run_start\",\"ts\":\"t3\",\"max_iters\":40}",
            "{\"type\":\"iteration\",\"ts\":\"t4\",\"n\":1}",
            "{\"type\":\"iteration\",\"ts\":\"t5\",\"n\":2}",
        ];
        let s = summarize_events(&lines);
        assert_eq!(s.state(), "running", "resumed child mid-run is running, not aborted");
        assert!(!s.abort_seen, "pre-resume abort must not stay latched");
        assert_eq!(s.abort_reason, None, "pre-resume abort reason must reset");
        assert_eq!(s.max_iters, Some(40));
        assert_eq!(s.last_iteration, Some(2), "iteration takes the last-seen value");
        assert_eq!(s.last_event_type.as_deref(), Some("iteration"));
        assert_eq!(s.last_event_ts.as_deref(), Some("t5"));
        assert!(!s.goal_seen && !s.budget_low_seen);
    }

    /// T58 (b): goal in the second segment after an abort in the first —
    /// `done`, not `aborted`.
    #[test]
    fn delegate_summary_goal_in_second_segment_after_abort_reports_done() {
        let lines = [
            "{\"type\":\"run_start\",\"ts\":\"t0\",\"max_iters\":40}",
            "{\"type\":\"iteration\",\"ts\":\"t1\",\"n\":7}",
            "{\"type\":\"abort\",\"ts\":\"t2\",\"reason\":\"model stream cut\",\"model\":\"kimi\"}",
            "{\"type\":\"run_start\",\"ts\":\"t3\",\"max_iters\":40}",
            "{\"type\":\"iteration\",\"ts\":\"t4\",\"n\":1}",
            "{\"type\":\"goal\",\"ts\":\"t5\",\"outcome\":\"accepted\",\"summary\":\"VERDICT PASS\"}",
        ];
        let s = summarize_events(&lines);
        assert_eq!(s.state(), "done");
        assert!(s.goal_seen);
        assert!(!s.abort_seen, "segment-1 abort must not outlive the resume");
        assert_eq!(s.abort_reason, None);
        assert_eq!(s.last_event_type.as_deref(), Some("goal"));
    }

    /// T58 (c): `budget_low` only in the first segment — a resumed child that
    /// has not gone budget-low again must not report the stale latch.
    #[test]
    fn delegate_summary_budget_low_in_first_segment_only_resets_on_resume() {
        let lines = [
            "{\"type\":\"run_start\",\"ts\":\"t0\",\"max_iters\":40}",
            "{\"type\":\"iteration\",\"ts\":\"t1\",\"n\":33}",
            "{\"type\":\"budget_low\",\"ts\":\"t2\",\"remaining_iters\":8,\"remaining_secs\":100}",
            "{\"type\":\"abort\",\"ts\":\"t3\",\"reason\":\"iteration budget exceeded\",\"model\":\"glm\"}",
            // The resume: fresh segment, budget_low never fires again.
            "{\"type\":\"run_start\",\"ts\":\"t4\",\"max_iters\":40}",
            "{\"type\":\"iteration\",\"ts\":\"t5\",\"n\":1}",
        ];
        let s = summarize_events(&lines);
        assert!(!s.budget_low_seen, "segment-1 budget_low must reset at the new run_start");
        assert!(!s.abort_seen && !s.goal_seen);
        assert_eq!(s.state(), "running");
    }

    /// T64 pin (b) — the t58-validate over-reset survivor. The T58
    /// two-segment tests above all place segment 2's iterations AFTER its
    /// `run_start`, so a mutant that over-resets `max_iters`/`last_iteration`
    /// to `None` at `run_start` survives: the new segment's own lines refill
    /// both fields before any assertion looks. T58 spec req 4 sentence 2
    /// pins the other half — the fields "already take the last-seen values
    /// and keep doing so" — so in the GAP window (after segment 2's
    /// `run_start`, before its first iteration) the summary must still
    /// report meaningful numbers, which is exactly what a resumed child's
    /// `status` shows while the relaunched process spins up.
    #[test]
    fn delegate_summary_run_start_gap_keeps_last_seen_max_iters_and_iteration() {
        // Segment 1: ran to its iteration-budget abort (max_iters 40, last
        // iteration 40). Segment 2: the resume — a fresh `run_start`
        // carrying its own max_iters (deliberately 50, so the assertions
        // prove WHICH line each field came from), and NOTHING else yet:
        // the gap window.
        let gap = [
            "{\"type\":\"run_start\",\"ts\":\"t0\",\"max_iters\":40}",
            "{\"type\":\"iteration\",\"ts\":\"t1\",\"n\":39}",
            "{\"type\":\"iteration\",\"ts\":\"t2\",\"n\":40}",
            "{\"type\":\"abort\",\"ts\":\"t3\",\"reason\":\"iteration budget exceeded\",\"model\":\"glm\",\"budget_kind\":\"iterations\",\"budget_max\":40}",
            "{\"type\":\"run_start\",\"ts\":\"t4\",\"max_iters\":50}",
        ];
        let s = summarize_events(&gap);
        // The verdict latches describe segment 2: the pre-resume abort is
        // gone, and a stream ending in a fresh `run_start` is `running`.
        assert_eq!(
            s.state(),
            "running",
            "segment-2's run_start resets the abort latch — the gap reports \
             running, not aborted"
        );
        assert!(
            !s.abort_seen,
            "pre-resume abort must not outlive the new run_start"
        );
        assert_eq!(
            s.abort_reason, None,
            "pre-resume abort reason must reset at the new run_start"
        );
        assert!(!s.goal_seen && !s.budget_low_seen);
        assert_eq!(s.last_event_type.as_deref(), Some("run_start"));
        // The stream-scope fields KEEP LAST-SEEN across the boundary — NOT
        // null/0: `max_iters` from the latest `run_start` seen (segment 2's
        // 50), `last_iteration` still segment 1's final iteration (40) until
        // segment 2 writes its first.
        assert_eq!(
            s.max_iters,
            Some(50),
            "max_iters keeps last-seen across the run_start boundary (the \
             latest run_start's value) — an over-reset to null is the \
             t58-validate survivor mutant"
        );
        assert_eq!(
            s.last_iteration,
            Some(40),
            "last_iteration keeps last-seen across the run_start boundary \
             (segment 1's final iteration) — an over-reset to null/0 is the \
             t58-validate survivor mutant"
        );

        // After segment 2's first iteration event the fields reflect
        // segment 2: its own iteration takes over, its max_iters stands.
        let after = [
            "{\"type\":\"run_start\",\"ts\":\"t0\",\"max_iters\":40}",
            "{\"type\":\"iteration\",\"ts\":\"t1\",\"n\":39}",
            "{\"type\":\"iteration\",\"ts\":\"t2\",\"n\":40}",
            "{\"type\":\"abort\",\"ts\":\"t3\",\"reason\":\"iteration budget exceeded\",\"model\":\"glm\",\"budget_kind\":\"iterations\",\"budget_max\":40}",
            "{\"type\":\"run_start\",\"ts\":\"t4\",\"max_iters\":50}",
            "{\"type\":\"iteration\",\"ts\":\"t5\",\"n\":1}",
        ];
        let s2 = summarize_events(&after);
        assert_eq!(s2.state(), "running");
        assert_eq!(
            s2.max_iters,
            Some(50),
            "segment 2's run_start max_iters wins once seen"
        );
        assert_eq!(
            s2.last_iteration,
            Some(1),
            "segment 2's own iteration takes over from segment 1's last-seen 40"
        );
        assert_eq!(s2.last_event_type.as_deref(), Some("iteration"));
        assert!(!s2.abort_seen && !s2.goal_seen && !s2.budget_low_seen);
    }

    /// T58 (d) regression pin: a SINGLE-segment stream — the only kind before
    /// resume existed — summarizes exactly as before T58. This is the same
    /// event sequence as the two-segment test minus the second `run_start`:
    /// the abort stays latched and the state is `aborted`.
    #[test]
    fn delegate_summary_single_segment_stream_unchanged_by_t58() {
        let lines = [
            "{\"type\":\"run_start\",\"ts\":\"t0\",\"max_iters\":40}",
            "{\"type\":\"iteration\",\"ts\":\"t1\",\"n\":12}",
            "{\"type\":\"abort\",\"ts\":\"t2\",\"reason\":\"llm request failed\",\"model\":\"kimi\"}",
        ];
        let s = summarize_events(&lines);
        assert!(s.abort_seen);
        assert_eq!(s.abort_reason.as_deref(), Some("llm request failed"));
        assert_eq!(s.state(), "aborted");
        assert_eq!(s.max_iters, Some(40));
        assert_eq!(s.last_iteration, Some(12));
        assert!(!s.goal_seen && !s.budget_low_seen);
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
        // T69: the error names ALL THREE actions verbatim, so a caller
        // reading the error learns the full surface at the moment of need.
        assert!(result.content.contains("\"launch\""), "{}", result.content);
        assert!(result.content.contains("\"status\""), "{}", result.content);
        assert!(result.content.contains("\"collect\""), "{}", result.content);
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
    /// T58: the resumed segment's fresh `run_start` — same shape, later ts.
    const T58_RESUME_RUN_START: &str =
        "{\"type\":\"run_start\",\"ts\":\"t3\",\"max_iters\":40}";

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

    /// T68: `significant_ne` is EXACTLY the six-field wake set. A diff
    /// confined to `last_event_type`/`last_event_ts` (the per-tool-call
    /// churn an active child emits every 2–10 s) is NOT significant; a diff
    /// in each of the six significant fields IS. Pinned field-by-field so a
    /// field cannot silently migrate between the wake set and the churn set.
    #[test]
    fn delegate_summary_significant_ne_is_exactly_the_six_field_wake_set() {
        let base = DelegateSummary {
            max_iters: Some(50),
            last_iteration: Some(7),
            last_event_type: Some("iteration".to_string()),
            last_event_ts: Some("t1".to_string()),
            budget_low_seen: true,
            goal_seen: false,
            abort_seen: false,
            abort_reason: Some("iteration budget exceeded".to_string()),
        };
        // Churn only (new last_event): never significant — the T68 defect
        // was exactly this diff waking the wait at the first poll tick.
        let churn = DelegateSummary {
            last_event_type: Some("tool_result".to_string()),
            last_event_ts: Some("t9".to_string()),
            ..base.clone()
        };
        assert!(!base.significant_ne(&churn), "last_event churn must not be significant");
        assert!(!churn.significant_ne(&base), "significance must be symmetric");
        // Identical: not significant.
        assert!(!base.significant_ne(&base.clone()));
        // Each significant field alone IS significant.
        let significant = [
            DelegateSummary { max_iters: None, ..base.clone() },
            DelegateSummary { last_iteration: Some(8), ..base.clone() },
            DelegateSummary { budget_low_seen: false, ..base.clone() },
            DelegateSummary { goal_seen: true, ..base.clone() },
            DelegateSummary { abort_seen: true, ..base.clone() },
            DelegateSummary { abort_reason: None, ..base.clone() },
        ];
        for sig in &significant {
            assert!(
                base.significant_ne(sig),
                "a diff in a significant field must wake: {base:?} vs {sig:?}"
            );
        }
    }

    /// T68 (churn pin): a mid-wait `tool_result` append — the `last_event`
    /// churn an active child emits every 2–10 s — must NOT wake the wait.
    /// Timing: the writer lands at ~0.3 s; with `wait_secs: 3` and the 2.5 s
    /// cadence there is exactly one mid-wait poll tick (~2.5 s), where the
    /// churn IS visible — so the pre-T68 any-field wake returned there
    /// (~2.5 s, `waited: 2`) while the significant wake set must run to the
    /// deadline. The payload still carries the NEW last_event: the deadline
    /// leg's final read renders the churn it declined to wake on.
    ///
    /// NON-VACUOUSNESS (validator-recorded): reverting the wake condition to
    /// any-field-diff turns this red — the wake fires at the ~2.5 s tick,
    /// under the elapsed lower bound and with `waited: 2 < 3`. Gutting the
    /// significant comparison so NOTHING wakes keeps this green but turns
    /// T29's iteration-append pin and the goal-append pin below red.
    #[test]
    fn delegate_status_wait_last_event_churn_does_not_wake() {
        let tmp = tempfile::tempdir().unwrap();
        write_events_fixture(tmp.path(), &[T29_RUN_START]);
        let events = tmp.path().join(".chug/events.jsonl");
        // The realistic churn line: name/ok/is_error/duration_ms/preview.
        // It moves ONLY last_event_type/ts — no significant field changes
        // (state stays `running`, no iteration, no verdict flag).
        let writer = thread::spawn(move || {
            thread::sleep(Duration::from_millis(300));
            append_events_line(
                &events,
                "{\"type\":\"tool_result\",\"ts\":\"t2\",\"name\":\"edit\",\"ok\":true,\"is_error\":false,\"duration_ms\":3,\"preview\":\"edited\"}",
            );
        });
        let ctx = delegate_ctx(tmp.path());
        let started = Instant::now();
        let result = dispatch(
            &ctx,
            "delegate",
            &json!({"action": "status", "cwd": tmp.path(), "wait_secs": 3}),
        );
        let elapsed = started.elapsed();
        writer.join().unwrap();
        assert!(!result.is_error, "{}", result.content);
        // NOT early: the churn was on disk before the ~2.5 s poll tick, so an
        // any-field wake returns there — under this bound, which only the
        // deadline leg (≥ 3 s) can satisfy.
        assert!(
            elapsed >= Duration::from_millis(2900),
            "last_event churn woke the wait early: {elapsed:?}"
        );
        assert!(elapsed < Duration::from_secs(30), "churn wait hung: {elapsed:?}");
        // `waited:` ≈ the request — the pre-T68 any-field wake reports 2.
        let waited = waited_secs_of(&result.content).expect("waited: line present");
        assert!(waited >= 3, "waited: {waited}s — churn woke the wait early");
        // Churn is RENDERED: the deadline's final read carries the new
        // last_event (not an entry-snapshot render).
        assert!(result.content.contains("last_event: tool_result t2"), "{}", result.content);
        // …while every significant field is unchanged.
        assert!(result.content.contains("state: running"), "{}", result.content);
        assert!(result.content.contains("last_iteration: none"), "{}", result.content);
        assert!(result.content.contains("goal_seen: false"), "{}", result.content);
    }

    /// T68 (significant-wake pin): a mid-wait `goal` append — a verdict flag
    /// flipping with NO `last_iteration` movement — must still wake the wait
    /// EARLY carrying the flag. T29's early-return pin covers the
    /// `last_iteration` advance; this covers the verdict-flag half of the
    /// significant set, so the T68 narrowing cannot over-correct into
    /// "nothing wakes".
    ///
    /// NON-VACUOUSNESS: the over-correction mutant (significant comparison
    /// gutted so nothing wakes) fails the elapsed bound — the goal is on
    /// disk at the ~2.5 s tick and the 30 s deadline is far; an
    /// entry-snapshot render fails the `goal_seen: true` / `state: done`
    /// pins. (The pre-T68 any-field revert passes this test — it is the
    /// churn pin above that kills it.)
    #[test]
    fn delegate_status_wait_wakes_early_on_goal_flag() {
        let tmp = tempfile::tempdir().unwrap();
        write_events_fixture(tmp.path(), &[T29_RUN_START]);
        let events = tmp.path().join(".chug/events.jsonl");
        let writer = thread::spawn(move || {
            thread::sleep(Duration::from_millis(300));
            append_events_line(
                &events,
                "{\"type\":\"goal\",\"ts\":\"t2\",\"outcome\":\"accepted\",\"summary\":\"all done\"}",
            );
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
        assert!(
            elapsed < Duration::from_secs(15),
            "goal flag did not wake the wait early: {elapsed:?}"
        );
        // The verdict flag, rendered from the woken state — not the entry
        // snapshot (`goal_seen: false`, `state: running`).
        assert!(result.content.contains("goal_seen: true"), "{}", result.content);
        assert!(result.content.contains("state: done"), "{}", result.content);
        assert!(result.content.contains("last_event: goal t2"), "{}", result.content);
        let waited = waited_secs_of(&result.content).expect("waited: line present");
        assert!(waited < 30, "waited: {waited}s");
    }

    /// T68 schema pin (T22/T41 convention): the LIVE `tool_schemas()` delegate
    /// entry names the significant-change wake set in BOTH the tool
    /// description's wait_secs sentence and the `wait_secs` property
    /// description — and the stale any-field phrasing ("events state
    /// changes" / "child state change") is gone. Doc honesty: the schema is
    /// what a cold orchestrator reads; describing an any-field wake would
    /// promise more than the (deliberately narrowed) code delivers.
    #[test]
    fn delegate_schema_describes_significant_wake_set() {
        let schemas = tool_schemas();
        let schema = schemas
            .iter()
            .find(|s| s.get("name").and_then(Value::as_str) == Some("delegate"))
            .expect("exactly one delegate schema (pinned elsewhere)");
        let tool_desc = schema
            .get("description")
            .and_then(Value::as_str)
            .expect("delegate tool description");
        assert!(
            tool_desc.contains("iteration advances"),
            "tool description must name the iteration-advance wake: {tool_desc}"
        );
        assert!(
            tool_desc.contains("verdict or budget-low flag"),
            "tool description must name the verdict/budget-low wake: {tool_desc}"
        );
        assert!(
            tool_desc.contains("never wakes it"),
            "tool description must say last_event churn never wakes: {tool_desc}"
        );
        assert!(
            !tool_desc.contains("events state changes"),
            "stale any-field phrasing must be gone from the tool description: {tool_desc}"
        );
        let wait_desc = schema["input_schema"]["properties"]["wait_secs"]
            .get("description")
            .and_then(Value::as_str)
            .expect("wait_secs property carries a description");
        assert!(
            wait_desc.contains("significant child change"),
            "wait_secs description must name the significant set: {wait_desc}"
        );
        assert!(
            wait_desc.contains("never wakes"),
            "wait_secs description must exclude last_event churn: {wait_desc}"
        );
        assert!(
            !wait_desc.contains("child state change"),
            "stale any-field phrasing must be gone from the wait_secs description: {wait_desc}"
        );
    }

    /// T68 README pin (T41 convention): the `delegate` paragraph's wait_secs
    /// clause names the significant wake set — no stale "state changes"
    /// phrasing. Whitespace-normalized so markdown rewrapping cannot unpin it.
    #[test]
    fn readme_delegate_wait_clause_names_significant_wake_set() {
        let readme = fs::read_to_string(
            std::env::current_dir()
                .expect("cargo sets the test cwd to the package root")
                .join("README.md"),
        )
        .expect("README.md readable from the crate root");
        let flat: String = readme.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(
            flat.contains(
                "it returns early when the child's iteration advances, a verdict or budget-low flag appears, or its liveness flips to dead"
            ),
            "README wait_secs clause lost the significant wake set: {flat}"
        );
        assert!(
            flat.contains("per-tool-call `last_event` churn renders at the deadline but never wakes it"),
            "README wait_secs clause must exclude last_event churn: {flat}"
        );
        assert!(
            !flat.contains("when the child's state changes or"),
            "stale any-field phrasing must be gone from the README clause: {flat}"
        );
    }

    /// Manual smoke, permanent: `collect` against the REAL checkout the test
    /// runs in — a live git repo with real commits (and, while this loop
    /// itself runs, a live `.chug/events.jsonl`) — returns the verdict plus
    /// the resolved commit-refs block in one bounded non-blocking call (the
    /// build_info T20 integration-pin pattern).
    #[test]
    fn delegate_collect_against_the_real_checkout_resolves_commit_refs() {
        let root = std::env::current_dir().expect("cargo sets the test cwd to the package root");
        let result = dispatch(
            &delegate_ctx(&root),
            "delegate",
            &json!({"action": "collect", "cwd": root}),
        );
        assert!(!result.is_error, "{}", result.content);
        // This checkout's live `.chug/events.jsonl` can be in ANY run state
        // when the suite runs — mid-run `running`, a clean checkout with no
        // events `starting`, or a FINISHED run's `goal-accepted` /
        // `goal-rejected` / `aborted` (the t69 child's own goal-accepted
        // stream red-fired a running|starting-only pin at the orchestrator's
        // review gates) — so pin the verdict LINE's shape (one of the five
        // known verdicts), never the state. Either way the git leg resolves
        // the REAL checkout's commits.
        let verdict = result
            .content
            .lines()
            .find_map(|l| l.strip_prefix("verdict: "))
            .expect("verdict line present");
        assert!(
            matches!(
                verdict,
                "goal-accepted" | "goal-rejected" | "aborted" | "running" | "starting"
            ),
            "{}",
            result.content
        );
        assert!(
            result.content.contains("commits (range HEAD, up to 20):"),
            "{}",
            result.content
        );
        // Real commit lines: `<7+ hex sha> <subject>`, newest first.
        let commit_lines: Vec<&str> = result
            .content
            .lines()
            .filter(|l| l.starts_with("  ") && !l.trim().is_empty())
            .collect();
        assert!(!commit_lines.is_empty(), "{}", result.content);
        let first = commit_lines[0].trim();
        let sha = first.split(' ').next().unwrap_or_default();
        assert!(sha.len() >= 7 && sha.chars().all(|c| c.is_ascii_hexdigit()), "{first}");
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
    fn delegate_schema_registers_exactly_one_entry_with_all_three_actions() {
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
        // T69: the enum gains `collect` — launch/status behavior untouched,
        // so the enum is the pre-T69 list plus exactly the new tail.
        assert_eq!(actions, vec!["launch", "status", "collect"]);
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

    /// T39/T58: the pure argv builder — with a token budget the flag pair is
    /// appended at the tail (after `--max-minutes`); without one the argv is
    /// byte-identical to pre-T39 (also pre-T58: `resume` false), pinned as
    /// the whole list.
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
        // Absent: byte-identical to pre-T39/pre-T58 — no `--max-tokens`
        // anywhere, no `--resume`.
        assert_eq!(
            delegate_child_argv(&spec, "g", "m", 40, 35, None, false),
            base,
            "absent max_tokens must not change the child argv"
        );
        // T15 parity: the flag pair lands at the tail, verbatim.
        let mut with = base.clone();
        with.push(OsString::from("--max-tokens"));
        with.push(OsString::from("250000"));
        assert_eq!(
            delegate_child_argv(&spec, "g", "m", 40, 35, Some(250_000), false),
            with
        );
        // Boundary: 1 is accepted and passes through verbatim.
        let mut one = base;
        one.push(OsString::from("--max-tokens"));
        one.push(OsString::from("1"));
        assert_eq!(
            delegate_child_argv(&spec, "g", "m", 40, 35, Some(1), false),
            one
        );
    }

    /// T58: the pure argv builder with `resume` — `true` appends the bare
    /// `--resume` flag as the very last argument (after any `--max-tokens`
    /// pair), purely additive: spec/goal/model and the budget flags pass
    /// through exactly as today. Absent and `false` both produce the pre-T58
    /// argv byte-for-byte (whole-list pin, T39 style) — children keep their
    /// fresh-start behavior unless the orchestrator opts in.
    #[test]
    fn delegate_child_argv_resume_appends_flag_last_absent_false_byte_identical() {
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
        // Absent (parsed to false upstream) == false == pre-T58, whole list.
        assert_eq!(
            delegate_child_argv(&spec, "g", "m", 40, 35, None, false),
            base,
            "resume=false must not change the child argv"
        );
        // With resume: `--resume` is the last argument, nothing else moves.
        let mut resumed = base.clone();
        resumed.push(OsString::from("--resume"));
        assert_eq!(
            delegate_child_argv(&spec, "g", "m", 40, 35, None, true),
            resumed,
            "--resume must be appended after all other flags"
        );
        // Resume composes with the token budget: still the very tail.
        let mut tokens_then_resume = base;
        tokens_then_resume.push(OsString::from("--max-tokens"));
        tokens_then_resume.push(OsString::from("250000"));
        tokens_then_resume.push(OsString::from("--resume"));
        assert_eq!(
            delegate_child_argv(&spec, "g", "m", 40, 35, Some(250_000), true),
            tokens_then_resume,
            "--resume must stay last even after the --max-tokens pair"
        );
    }

    /// T58: the parse — absent → `false`; `false` → `false`; `true` → `true`;
    /// a non-boolean (string "true", number 1) is a tool error naming the
    /// boolean requirement, never a silent fresh start (a caller that asked to
    /// resume and silently got a fresh child would lose the prior run).
    #[test]
    fn delegate_resume_parse_absent_false_true_and_rejects_non_boolean() {
        assert!(!delegate_resume(&json!({})).unwrap());
        assert!(!delegate_resume(&json!({"resume": false})).unwrap());
        assert!(delegate_resume(&json!({"resume": true})).unwrap());
        for bad in [json!("true"), json!(1), json!(0)] {
            let err = delegate_resume(&json!({"resume": bad}))
                .unwrap_err()
                .to_string();
            assert!(err.contains("boolean"), "{bad} → {err}");
        }
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

    /// T58 schema pin (T22/T39 convention): the LIVE `tool_schemas()` delegate
    /// entry names the `resume` property for launch — optional boolean, and
    /// its description says what it does (append `--resume`, continue the
    /// child's prior run). The tool description also names the resume leg and
    /// the latest-segment status summary; `required` stays unchanged.
    #[test]
    fn delegate_schema_pins_optional_resume() {
        let schemas = tool_schemas();
        let schema = schemas
            .iter()
            .find(|s| s.get("name").and_then(Value::as_str) == Some("delegate"))
            .expect("exactly one delegate schema (pinned elsewhere)");
        let prop = schema["input_schema"]["properties"]["resume"]
            .as_object()
            .expect("resume property missing from the delegate schema");
        assert_eq!(prop.get("type").and_then(Value::as_str), Some("boolean"));
        let desc = prop
            .get("description")
            .and_then(Value::as_str)
            .expect("resume property carries a description");
        assert!(
            desc.contains("--resume"),
            "resume description must name the appended flag: {desc}"
        );
        assert!(
            desc.contains("prior run"),
            "resume description must say it continues the child's prior run: {desc}"
        );
        let tool_desc = schema
            .get("description")
            .and_then(Value::as_str)
            .expect("delegate tool description");
        assert!(
            tool_desc.contains("resume optional"),
            "tool description lost the resume clause: {tool_desc}"
        );
        assert!(
            tool_desc.contains("latest run segment"),
            "tool description lost the latest-segment status clause: {tool_desc}"
        );
        let required: Vec<&str> = schema["input_schema"]["required"]
            .as_array()
            .expect("required list")
            .iter()
            .filter_map(Value::as_str)
            .collect();
        assert_eq!(required, vec!["action", "cwd"], "required list must be unchanged");
    }

    /// T58 end-to-end: launch with `resume: true` → the child's argv ends
    /// with `--resume` (after any `--max-tokens` pair), and the return text
    /// names the resume leg (`resume: true`, T39 echo pattern). NON-VACUOUSNESS:
    /// dropping the argv append fails the dump assert; dropping the echo fails
    /// the content assert.
    #[test]
    fn delegate_launch_with_resume_appends_flag_to_child_argv() {
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
                "resume": true,
            }),
        );
        assert!(!launch.is_error, "{}", launch.content);
        let argv = wait_for_argv_dump(child_dir.path());
        assert_eq!(
            argv.last().map(String::as_str),
            Some("--resume"),
            "--resume must be the child argv's last argument: {}",
            argv.join(" | ")
        );
        let pos = argv
            .iter()
            .position(|a| a == "--max-tokens")
            .expect("--max-tokens in the child argv");
        assert_eq!(argv[pos + 1], "250000", "{}", argv.join(" | "));
        // The return text names the resume leg alongside the other budgets.
        assert!(
            launch
                .content
                .contains("max_iters: 40 max_minutes: 35 max_tokens: 250000 resume: true"),
            "{}",
            launch.content
        );
        let pid = spawn_pid_of(&launch);
        kill_pid_group(pid);
        // SAFETY: serialized by DELEGATE_ENV_LOCK; no other test reads this var.
        unsafe { std::env::remove_var("CHUG_DELEGATE_BIN") };
    }

    /// T58 end-to-end through the status action over a synthetic two-segment
    /// events file (abort in segment 1, `run_start` + iterations in segment
    /// 2): the summary describes the LATEST segment — `state: running`,
    /// `abort_seen: false` — not the latched pre-resume abort.
    #[test]
    fn delegate_status_two_segment_events_reports_latest_segment() {
        let tmp = tempfile::tempdir().unwrap();
        write_events_fixture(
            tmp.path(),
            &[
                "{\"type\":\"run_start\",\"ts\":\"t0\",\"max_iters\":40}",
                "{\"type\":\"iteration\",\"ts\":\"t1\",\"n\":12}",
                "{\"type\":\"abort\",\"ts\":\"t2\",\"reason\":\"llm request failed\",\"model\":\"kimi\"}",
                T58_RESUME_RUN_START,
                "{\"type\":\"iteration\",\"ts\":\"t4\",\"n\":1}",
            ],
        );
        let result = dispatch(
            &delegate_ctx(tmp.path()),
            "delegate",
            &json!({"action": "status", "cwd": tmp.path()}),
        );
        assert!(!result.is_error, "{}", result.content);
        assert!(result.content.contains("state: running"), "{}", result.content);
        assert!(result.content.contains("abort_seen: false"), "{}", result.content);
        assert!(!result.content.contains("abort_reason"), "{}", result.content);
        assert!(result.content.contains("last_iteration: 1"), "{}", result.content);
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

    // ---- T69: the `collect` action — structured child result ----

    /// A `goal` event line with the given outcome and payload field.
    fn goal_line(outcome: &str, field: &str, text: &str) -> String {
        format!("{{\"type\":\"goal\",\"ts\":\"t2\",\"outcome\":\"{outcome}\",\"{field}\":\"{text}\"}}")
    }

    #[test]
    fn delegate_collect_parse_accepted_yields_verdict_summary_and_cmd() {
        let lines = [
            "{\"type\":\"run_start\",\"ts\":\"t0\",\"max_iters\":40}",
            "{\"type\":\"iteration\",\"ts\":\"t1\",\"n\":4}",
            "{\"type\":\"verifying\",\"ts\":\"t1\",\"cmd\":\"cargo test --bin chug delegate\"}",
            &goal_line("accepted", "summary", "T69 done: collect shipped, gates green"),
        ];
        let s = summarize_collect(&lines);
        assert_eq!(s.verdict(), "goal-accepted");
        assert_eq!(
            s.goal_summary.as_deref(),
            Some("T69 done: collect shipped, gates green")
        );
        assert_eq!(s.check_cmd.as_deref(), Some("cargo test --bin chug delegate"));
        assert_eq!(s.abort_reason, None);
    }

    /// The accepted summary is the FULL text — multi-line summaries survive
    /// verbatim (the child's own account, not a one-line clip).
    #[test]
    fn delegate_collect_parse_keeps_multiline_summary_verbatim() {
        let lines = [
            "{\"type\":\"run_start\",\"ts\":\"t0\",\"max_iters\":40}",
            "{\"type\":\"goal\",\"ts\":\"t2\",\"outcome\":\"accepted\",\"summary\":\"line one\\nline two\"}",
        ];
        let s = summarize_collect(&lines);
        assert_eq!(s.verdict(), "goal-accepted");
        assert_eq!(s.goal_summary.as_deref(), Some("line one\nline two"));
    }

    /// A rejected verdict latches `goal-rejected` — and a LATER accepted
    /// verdict in the SAME segment flips it (the child's loop continues
    /// after a rejection).
    #[test]
    fn delegate_collect_parse_rejected_then_later_accepted_flips() {
        let rejected_only = [
            "{\"type\":\"run_start\",\"ts\":\"t0\",\"max_iters\":40}",
            &goal_line("rejected", "reason", "check failed: 3 tests red"),
        ];
        let s = summarize_collect(&rejected_only);
        assert_eq!(s.verdict(), "goal-rejected");
        assert_eq!(s.goal_summary, None, "no summary on a rejected verdict");
        assert_eq!(s.abort_reason, None);

        // The flip: acceptance after rejection, same segment.
        let start: &str = "{\"type\":\"run_start\",\"ts\":\"t0\",\"max_iters\":40}";
        let flipped = [
            start,
            rejected_only[1],
            &goal_line("accepted", "summary", "fixed and green"),
        ];
        let s2 = summarize_collect(&flipped);
        assert_eq!(s2.verdict(), "goal-accepted");
        assert_eq!(s2.goal_summary.as_deref(), Some("fixed and green"));
    }

    /// An `abort` line latches `aborted` and carries the reason.
    #[test]
    fn delegate_collect_parse_abort_carries_reason() {
        let lines = [
            "{\"type\":\"run_start\",\"ts\":\"t0\",\"max_iters\":40}",
            "{\"type\":\"abort\",\"ts\":\"t2\",\"reason\":\"iteration budget exceeded\",\"model\":\"glm\",\"budget_kind\":\"iterations\",\"budget_max\":40}",
        ];
        let s = summarize_collect(&lines);
        assert_eq!(s.verdict(), "aborted");
        assert_eq!(s.abort_reason.as_deref(), Some("iteration budget exceeded"));
        assert_eq!(s.goal_summary, None);
    }

    /// Events without a verdict → `running`; nothing read at all →
    /// `starting`.
    #[test]
    fn delegate_collect_parse_running_vs_starting() {
        let mid_run = [
            "{\"type\":\"run_start\",\"ts\":\"t0\",\"max_iters\":40}",
            "{\"type\":\"iteration\",\"ts\":\"t1\",\"n\":7}",
        ];
        let s = summarize_collect(&mid_run);
        assert_eq!(s.verdict(), "running");
        assert_eq!(s.check_cmd, None);

        let empty = summarize_collect(&[]);
        assert_eq!(empty.verdict(), "starting");
        // A torn-only stream parses to nothing → still `starting`.
        let torn = ["{\"type\":\"iteration\",\"ts\":\"t1\",\"n\":7,\"trunc"];
        assert_eq!(summarize_collect(&torn).verdict(), "starting");
    }

    /// T58's segment reset, collect-side: a pre-resume `abort` must not
    /// outlive the resume's `run_start` — post-resume acceptance reports
    /// `goal-accepted`, and the segment's check cmd is the RESUMED segment's.
    /// The verdict-latch reset has its own tooth (finding-2 pin): a resume
    /// that has NOT reached a verdict yet must render `running`, not leak
    /// segment 1's verdict or summary — a post-resume-accepted leg alone
    /// would overwrite the latch and leave the reset vacuous (the
    /// mutant-kill leg: deleting the `run_start` latch reset turns this red).
    #[test]
    fn delegate_collect_parse_run_start_resets_verdict_and_check() {
        let lines = [
            // Segment 1: gated by check A, then died.
            "{\"type\":\"run_start\",\"ts\":\"t0\",\"max_iters\":40}",
            "{\"type\":\"verifying\",\"ts\":\"t1\",\"cmd\":\"check A\"}",
            "{\"type\":\"abort\",\"ts\":\"t2\",\"reason\":\"llm request failed\",\"model\":\"kimi\"}",
            // Segment 2 (the resume): fresh gate, fresh verdict.
            "{\"type\":\"run_start\",\"ts\":\"t3\",\"max_iters\":40}",
            "{\"type\":\"verifying\",\"ts\":\"t4\",\"cmd\":\"check B\"}",
            &goal_line("accepted", "summary", "resumed run passed"),
        ];
        let s = summarize_collect(&lines);
        assert_eq!(s.verdict(), "goal-accepted", "pre-resume abort must not outlive the resume");
        assert_eq!(s.abort_reason, None, "pre-resume abort reason must reset");
        assert_eq!(s.check_cmd.as_deref(), Some("check B"), "the segment's LATEST gate wins");
        assert_eq!(s.goal_summary.as_deref(), Some("resumed run passed"));

        // Post-resume-NO-verdict leg: segment 1 reached an ACCEPTED verdict
        // (with its summary), then the resume's segment holds only ordinary
        // tool events — no goal, no abort. The pre-resume verdict and
        // summary must NOT leak: the verdict latch resets to `running`.
        let resumed_mid_run = [
            "{\"type\":\"run_start\",\"ts\":\"t0\",\"max_iters\":40}",
            "{\"type\":\"verifying\",\"ts\":\"t1\",\"cmd\":\"check A\"}",
            &goal_line("accepted", "summary", "pre-resume summary must not leak"),
            T58_RESUME_RUN_START,
            "{\"type\":\"iteration\",\"ts\":\"t4\",\"n\":1}",
        ];
        let s2 = summarize_collect(&resumed_mid_run);
        assert_eq!(
            s2.verdict(),
            "running",
            "a pre-resume verdict must not leak into a verdict-less resume"
        );
        assert_eq!(
            s2.goal_summary, None,
            "a pre-resume summary must not leak either"
        );
        assert_eq!(
            s2.check_cmd, None,
            "a pre-resume check cmd must not leak into a verdict-less resume"
        );
    }

    /// Torn/malformed lines are skipped, never fatal — collecting is safe at
    /// ANY child lifecycle moment (mid-write, mid-run, post-cleanup). A
    /// complete verdict line still counts; a torn NEXT write contributes
    /// nothing and changes no verdict.
    #[test]
    fn delegate_collect_parse_torn_lines_skipped_not_fatal() {
        let lines = [
            "{\"type\":\"run_start\",\"ts\":\"t0\",\"max_iters\":40}",
            &goal_line("accepted", "summary", "done"),
            // Torn final write (partial JSON) and non-JSON noise: skipped.
            "{\"type\":\"iteration\",\"ts\":\"t3\",\"n\":8,\"trunc",
            "not json at all",
            "",
        ];
        let s = summarize_collect(&lines);
        assert_eq!(s.verdict(), "goal-accepted");
        assert_eq!(s.goal_summary.as_deref(), Some("done"));
        // A torn VERDICT line must not produce a phantom verdict either —
        // the segment stays `running` on its earlier complete lines.
        let torn_verdict = [
            "{\"type\":\"run_start\",\"ts\":\"t0\",\"max_iters\":40}",
            "{\"type\":\"goal\",\"ts\":\"t2\",\"outcome\":\"accepted\",\"summary\":\"do",
        ];
        let s2 = summarize_collect(&torn_verdict);
        assert_eq!(s2.verdict(), "running");
        assert_eq!(s2.goal_summary, None);
    }

    /// Multiple `verifying` lines in one segment: the LATEST cmd wins (the
    /// last one is the gate that actually decided the verdict).
    #[test]
    fn delegate_collect_parse_latest_verifying_cmd_wins() {
        let lines = [
            "{\"type\":\"run_start\",\"ts\":\"t0\",\"max_iters\":40}",
            "{\"type\":\"verifying\",\"ts\":\"t1\",\"cmd\":\"first gate\"}",
            "{\"type\":\"verifying\",\"ts\":\"t2\",\"cmd\":\"second gate\"}",
            "{\"type\":\"verifying\",\"ts\":\"t3\",\"cmd\":\"final gate\"}",
            &goal_line("accepted", "summary", "s"),
        ];
        let s = summarize_collect(&lines);
        assert_eq!(s.check_cmd.as_deref(), Some("final gate"));
    }

    /// The full dispatch render over a synthetic accepted child: the
    /// four-field result (verdict + summary + check cmd + commits block) plus
    /// the pid liveness line, in one bounded non-blocking call.
    #[test]
    fn delegate_collect_dispatch_renders_full_accepted_result() {
        let tmp = tempfile::tempdir().unwrap();
        write_events_fixture(
            tmp.path(),
            &[
                "{\"type\":\"run_start\",\"ts\":\"t0\",\"max_iters\":40}",
                "{\"type\":\"verifying\",\"ts\":\"t1\",\"cmd\":\"cargo test --bin chug delegate\"}",
                "{\"type\":\"goal\",\"ts\":\"t2\",\"outcome\":\"accepted\",\"summary\":\"T69: collect shipped\"}",
            ],
        );
        let result = dispatch(
            &delegate_ctx(tmp.path()),
            "delegate",
            &json!({
                "action": "collect",
                "cwd": tmp.path(),
                "pid": std::process::id(),
            }),
        );
        assert!(!result.is_error, "{}", result.content);
        assert!(result.content.contains("verdict: goal-accepted"), "{}", result.content);
        assert!(result.content.contains("alive: true"), "{}", result.content);
        assert!(result.content.contains("summary: T69: collect shipped"), "{}", result.content);
        assert!(
            result.content.contains("check_cmd: cargo test --bin chug delegate"),
            "{}",
            result.content
        );
        // Not-a-repo tempdir: the git leg DEGRADES to a note, never an error.
        assert!(
            result.content.contains("commits: (unavailable:"),
            "{}",
            result.content
        );
        assert!(!result.content.contains("abort_reason"), "{}", result.content);
    }

    /// No pid → NO liveness line at all (collect's contract, unlike
    /// status's always-rendered `alive: unknown`).
    #[test]
    fn delegate_collect_without_pid_renders_no_liveness_claim() {
        let tmp = tempfile::tempdir().unwrap();
        let result = dispatch(
            &delegate_ctx(tmp.path()),
            "delegate",
            &json!({"action": "collect", "cwd": tmp.path()}),
        );
        assert!(!result.is_error, "{}", result.content);
        assert!(result.content.contains("verdict: starting"), "{}", result.content);
        assert!(
            result.content.contains("events: nothing read"),
            "a missing events log degrades to the note: {}",
            result.content
        );
        assert!(!result.content.contains("alive:"), "{}", result.content);
        assert!(
            result.content.contains("commits: (unavailable:"),
            "not-a-repo git leg degrades to a note: {}",
            result.content
        );
    }

    /// The abort render path END-TO-END at dispatch level (finding-1 pin):
    /// a stream whose latest segment ends in an `abort` line carrying a
    /// reason renders BOTH the `aborted` verdict AND the `abort_reason:`
    /// line — the bail story is the failure surface the caller greps, and
    /// without the render leg the parse pin alone leaves the render block
    /// dead per the suite (the mutant-kill leg: deleting the render block
    /// turns this red).
    #[test]
    fn delegate_collect_dispatch_renders_aborted_verdict_and_reason() {
        let tmp = tempfile::tempdir().unwrap();
        write_events_fixture(
            tmp.path(),
            &[
                T29_RUN_START,
                T29_ITERATION,
                "{\"type\":\"verifying\",\"ts\":\"t1\",\"cmd\":\"cargo test --bin chug delegate\"}",
                "{\"type\":\"abort\",\"ts\":\"t2\",\"reason\":\"iteration budget exceeded\",\"model\":\"glm\",\"budget_kind\":\"iterations\",\"budget_max\":50}",
            ],
        );
        let result = dispatch(
            &delegate_ctx(tmp.path()),
            "delegate",
            &json!({"action": "collect", "cwd": tmp.path()}),
        );
        assert!(!result.is_error, "{}", result.content);
        assert!(result.content.contains("verdict: aborted"), "{}", result.content);
        assert!(
            result.content.contains("abort_reason: iteration budget exceeded"),
            "{}",
            result.content
        );
        // The gate that ran before the bail is still the segment's check cmd.
        assert!(
            result.content.contains("check_cmd: cargo test --bin chug delegate"),
            "{}",
            result.content
        );
        // An aborted segment carries no accepted-goal summary.
        assert!(!result.content.contains("summary:"), "{}", result.content);
    }

    /// Mid-run child: events exist without a verdict → `running`, without
    /// blocking (the call returns immediately; a verdict would say so).
    #[test]
    fn delegate_collect_mid_run_reports_running() {
        let tmp = tempfile::tempdir().unwrap();
        write_events_fixture(tmp.path(), &[T29_RUN_START, T29_ITERATION]);
        let started = Instant::now();
        let result = dispatch(
            &delegate_ctx(tmp.path()),
            "delegate",
            &json!({"action": "collect", "cwd": tmp.path()}),
        );
        // Non-blocking by contract: well under any plausible poll interval.
        assert!(started.elapsed() < Duration::from_secs(5), "{:?}", started.elapsed());
        assert!(!result.is_error, "{}", result.content);
        assert!(result.content.contains("verdict: running"), "{}", result.content);
        assert!(!result.content.contains("summary:"), "{}", result.content);
    }

    /// Req 4: `wait_secs` with `collect` is a tool error naming that the wait
    /// knob is status-only (launch-leg parity — naming beats silently
    /// ignoring).
    #[test]
    fn delegate_collect_rejects_wait_secs_as_status_only() {
        let tmp = tempfile::tempdir().unwrap();
        let result = dispatch(
            &delegate_ctx(tmp.path()),
            "delegate",
            &json!({"action": "collect", "cwd": tmp.path(), "wait_secs": 5}),
        );
        assert!(result.is_error, "{}", result.content);
        assert!(result.content.contains("status action only"), "{}", result.content);
        assert!(result.content.contains("collect"), "{}", result.content);
    }

    /// Req 3: a non-string `base` is a tool error naming the constraint —
    /// never a silent ignore (T39/T58 parse precedent).
    #[test]
    fn delegate_collect_rejects_non_string_base() {
        let tmp = tempfile::tempdir().unwrap();
        for bad in [json!(7), json!(true), json!(["origin/main"])] {
            let result = dispatch(
                &delegate_ctx(tmp.path()),
                "delegate",
                &json!({"action": "collect", "cwd": tmp.path(), "base": bad}),
            );
            assert!(result.is_error, "{bad}: {}", result.content);
            assert!(result.content.contains("must be a string git ref"), "{bad}: {}", result.content);
        }
    }

    /// A real temp repo: commit refs render, the `base` range is honored,
    /// an unresolvable ref degrades to a note, and an empty self-range says
    /// so — every leg `is_error: false`.
    #[test]
    fn delegate_collect_git_legs_refs_base_and_degrades() {
        let tmp = tempfile::tempdir().unwrap();
        let git = |args: &[&str]| {
            let out = Command::new("git")
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
        let second = git(&["rev-parse", "--short", "HEAD"]);
        assert_ne!(first, second, "two distinct commits");

        // Default range: BOTH commits, newest first, each as `sha msg`.
        let refs = collect_git_commits(tmp.path(), None).expect("refs resolve in a real repo");
        assert_eq!(refs.len(), 2, "{refs:?}");
        assert!(refs[0].starts_with(&second) && refs[0].contains("second"), "{refs:?}");
        assert!(refs[1].starts_with(&first) && refs[1].contains("first"), "{refs:?}");

        // `base` scopes the range: first..HEAD holds ONLY the second commit.
        let scoped = collect_git_commits(tmp.path(), Some(&first)).expect("base range resolves");
        assert_eq!(scoped.len(), 1, "{scoped:?}");
        assert!(scoped[0].starts_with(&second), "{scoped:?}");

        // A self-range is empty: git exits 0 with no commits in range.
        let none = collect_git_commits(tmp.path(), Some("HEAD")).expect("self-range is empty");
        assert!(none.is_empty(), "{none:?}");

        // Unresolvable base ref: degrade note, never an error.
        let err = collect_git_commits(tmp.path(), Some("definitely-not-a-ref-xyz"))
            .expect_err("bad ref degrades");
        assert!(err.contains("definitely-not-a-ref-xyz"), "{err}");

        // Not a repo: degrade note, never an error, never a panic.
        let bare = tempfile::tempdir().unwrap();
        let err = collect_git_commits(bare.path(), None).expect_err("not a repo degrades");
        assert!(err.contains("not a git repository"), "{err}");
    }

    /// The dispatch-level git legs end-to-end: `base` scopes the rendered
    /// block, the range header names the queried range, and every degrade
    /// keeps `is_error: false`.
    #[test]
    fn delegate_collect_dispatch_renders_commit_refs_with_base() {
        let tmp = tempfile::tempdir().unwrap();
        let git = |args: &[&str]| {
            let out = Command::new("git")
                .args(args)
                .current_dir(tmp.path())
                .output()
                .expect("git available for the integration pin");
            assert!(out.status.success(), "git {args:?} failed");
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        };
        git(&["init", "-q"]);
        git(&["-c", "user.email=t@t", "-c", "user.name=t", "commit", "--allow-empty", "-qm", "first"]);
        let first = git(&["rev-parse", "--short", "HEAD"]);
        git(&["-c", "user.email=t@t", "-c", "user.name=t", "commit", "--allow-empty", "-qm", "second"]);
        let second = git(&["rev-parse", "--short", "HEAD"]);

        let result = dispatch(
            &delegate_ctx(tmp.path()),
            "delegate",
            &json!({"action": "collect", "cwd": tmp.path(), "base": first}),
        );
        assert!(!result.is_error, "{}", result.content);
        assert!(
            result.content.contains(&format!("commits (range {first}..HEAD, up to 20):")),
            "{}",
            result.content
        );
        // Commit LINES carry the two-space indent — pin on those, so the
        // base sha's appearance in the range HEADER is not confused with a
        // listed commit.
        assert!(result.content.contains(&format!("\n  {second} second")), "{}", result.content);
        assert!(
            !result.content.contains(&format!("\n  {first} first")),
            "the base commit itself must be outside the range: {}",
            result.content
        );

        // An empty range renders the one-line note, still not an error.
        let result = dispatch(
            &delegate_ctx(tmp.path()),
            "delegate",
            &json!({"action": "collect", "cwd": tmp.path(), "base": "HEAD"}),
        );
        assert!(!result.is_error, "{}", result.content);
        assert!(
            result.content.contains("commits: (none in range HEAD..HEAD)"),
            "{}",
            result.content
        );

        // An unresolvable ref renders the degrade note, still not an error.
        let result = dispatch(
            &delegate_ctx(tmp.path()),
            "delegate",
            &json!({"action": "collect", "cwd": tmp.path(), "base": "definitely-not-a-ref-xyz"}),
        );
        assert!(!result.is_error, "{}", result.content);
        assert!(
            result.content.contains("commits: (unavailable:"),
            "{}",
            result.content
        );
    }

    /// T69 schema pins: the action enum carries all three, the tool
    /// description names the collect action truthfully, and the `base`
    /// property is a described optional string.
    #[test]
    fn delegate_schema_pins_collect_and_base() {
        let schemas = tool_schemas();
        let schema = schemas
            .iter()
            .find(|s| s.get("name").and_then(Value::as_str) == Some("delegate"))
            .expect("exactly one delegate schema (pinned elsewhere)");
        let tool_desc = schema
            .get("description")
            .and_then(Value::as_str)
            .expect("delegate tool description");
        // All three actions named in the description's action sentences.
        assert!(tool_desc.contains("action=launch"), "{tool_desc}");
        assert!(tool_desc.contains("action=status"), "{tool_desc}");
        assert!(tool_desc.contains("action=collect"), "{tool_desc}");
        // The collect sentence names its four fields truthfully.
        assert!(tool_desc.contains("verdict"), "{tool_desc}");
        assert!(tool_desc.contains("summary"), "{tool_desc}");
        assert!(tool_desc.contains("check cmd"), "{tool_desc}");
        assert!(tool_desc.contains("commit refs"), "{tool_desc}");
        // The never-blocks contract covers collect too, and the wait knob's
        // rejection is named for both non-waiting actions.
        assert!(tool_desc.contains("collect reads tails only"), "{tool_desc}");
        assert!(tool_desc.contains("launch and collect reject it"), "{tool_desc}");

        let base = schema["input_schema"]["properties"]["base"]
            .as_object()
            .expect("base property");
        assert_eq!(base.get("type").and_then(Value::as_str), Some("string"));
        let desc = base
            .get("description")
            .and_then(Value::as_str)
            .expect("base property carries a description");
        assert!(desc.contains("collect only"), "{desc}");
        assert!(desc.contains("<base>..HEAD"), "{desc}");
        let action_desc = schema["input_schema"]["properties"]["action"]
            .get("description")
            .and_then(Value::as_str)
            .expect("action property carries a description");
        assert!(action_desc.contains("collect"), "{action_desc}");
        let required: Vec<&str> = schema["input_schema"]["required"]
            .as_array()
            .expect("required list")
            .iter()
            .filter_map(Value::as_str)
            .collect();
        assert_eq!(required, vec!["action", "cwd"], "required list must be unchanged");
    }
}
