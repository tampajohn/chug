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
            // T89: the terminal wait flag is status-only for the same reason
            // (same presence-based rejection style — even `false` names the
            // constraint rather than being silently ignored).
            if input.get("terminal").is_some() {
                bail!(
                    "delegate: `terminal` applies to the status action only — launch returns at spawn and never waits on the child"
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
            // T89: launch-leg parity for the terminal flag as well.
            if input.get("terminal").is_some() {
                bail!(
                    "delegate: `terminal` applies to the status action only — collect never blocks or waits; long-poll with status first, then collect"
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
///
/// T103: the cwd must also EXIST as a directory (probed here, riding the same
/// exemption — status/collect re-render it the same way). A
/// corrupted-but-absolute cwd previously made it to spawn and died inside the
/// child; the error now names the received path verbatim at the call site.
fn delegate_cwd(input: &Value) -> anyhow::Result<PathBuf> {
    let raw = get_str(input, "cwd")?;
    let cwd = PathBuf::from(raw);
    if !cwd.is_absolute() {
        bail!("delegate: cwd must be an absolute directory, got {raw:?}");
    }
    if !cwd.is_dir() {
        bail!(
            "delegate: cwd does not exist or is not a directory: {}",
            cwd.display()
        );
    }
    Ok(cwd)
}

/// The child's spec path: absolute (it is read by the child, whose cwd is the
/// worktree, not by us — a relative path would mean something else there).
///
/// T103: the absoluteness bail validates only SHAPE — a corrupted-but-absolute
/// spec (goal text glued to a path, a hallucinated slug, a truncated path)
/// used to pass here and spawn a doomed child that burned setup iterations
/// before dying on its own spec read. The probe below refuses the payload at
/// the call site instead, so the caller self-corrects in ONE iteration (the
/// T94 thesis applied to the launch surface) and the received path is named
/// verbatim, making the corruption visible in the error.
///
/// TOCTOU (deliberate, per spec req 3): the path can still vanish between this
/// probe and the child's own read — the probe exists to catch MALFORMED
/// payloads, not to guarantee the child succeeds; the child's own spec-read
/// error remains the backstop. `File::open` (not just metadata) is the
/// readability ground truth for this user; a regular file that exists but
/// cannot be opened is refused with the same error.
fn delegate_spec(input: &Value) -> anyhow::Result<PathBuf> {
    let raw = get_str(input, "spec")?;
    let spec = PathBuf::from(raw);
    if !spec.is_absolute() {
        bail!("delegate: spec must be an absolute path, got {raw:?}");
    }
    // `File::open` alone would admit a directory (open(2) on a directory
    // succeeds) — the is_file leg is what refuses directories here, and
    // open() is the readability ground truth for this user.
    let readable = spec.is_file() && fs::File::open(&spec).is_ok();
    if !readable {
        bail!(
            "delegate: spec does not exist or is not a readable file: {}",
            spec.display()
        );
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
        images: Vec::new(),
    })
}

/// Observe a previously launched child. Without `wait_secs` (or with `0`)
/// this is the pre-T29 instant render, byte-identical for identical state
/// (pinned by test); with `wait_secs > 0` it is a bounded long-poll that
/// blocks until the first significant change (T68 — iteration advance or
/// verdict/budget-low flag; `last_event` churn never wakes), a liveness
/// flip, or the deadline. With `terminal: true` AND `wait_secs > 0` (T89)
/// the same long-poll narrows its wake set to the terminal facts — the
/// goal/abort verdict, a liveness flip to dead, events-file creation, or
/// the deadline — never iteration advances or budget-low flags; the
/// default (`terminal` absent/false) keeps the pre-T89 semantics exactly
/// (pinned by test).
fn delegate_status(input: &Value) -> anyhow::Result<ToolResult> {
    // Parse the knobs before any I/O so a bad value errors instantly even
    // when `cwd` is also bad.
    let wait_secs = parse_wait_secs(input)?;
    let terminal = parse_terminal(input)?;
    // T89 req 2: a terminal wait with no wait window is a contradiction —
    // the instant leg already exists, so a `terminal: true` poll without
    // `wait_secs > 0` is a caller error, named as such (corrective error,
    // never silent degradation into the instant leg).
    if terminal && wait_secs.unwrap_or(0) == 0 {
        bail!(
            "delegate: `terminal: true` needs `wait_secs > 0` — a terminal instant poll is a contradiction; the terminal wait blocks until the goal/abort verdict, a liveness flip to dead, events-file creation, or this wait_secs deadline (max {DELEGATE_WAIT_MAX_SECS})"
        );
    }
    let cwd = delegate_cwd(input)?;
    let pid = input.get("pid").and_then(Value::as_u64);
    match wait_secs {
        Some(secs) if secs > 0 => delegate_status_wait(&cwd, pid, secs, terminal),
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

/// T89: parse the optional `terminal` flag (status only). Absent → `false`;
/// boolean → its value; anything else → tool error, never a silent ignore
/// (the `resume`/`max_tokens` parse precedent).
fn parse_terminal(input: &Value) -> anyhow::Result<bool> {
    match input.get("terminal") {
        None => Ok(false),
        Some(Value::Bool(b)) => Ok(*b),
        Some(other) => bail!("delegate: `terminal` must be a boolean (status only), got {other}"),
    }
}

/// The instant leg: bounded tail reads only, no waiting anywhere.
fn delegate_status_now(cwd: &Path, pid: Option<u64>) -> anyhow::Result<ToolResult> {
    let alive = pid.and_then(reap_and_alive);
    let (summary, events_note) = read_events(&cwd.join(".chug").join("events.jsonl"));
    let log_tail = read_log_tail(&cwd.join(".chug").join("delegate.log"));
    Ok(ToolResult {
        content: render_status(&summary, alive, &log_tail, events_note.as_deref()),
        is_error: false,
        images: Vec::new(),
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

/// T29 + T68 + T89: the bounded long-poll. Block until the FIRST of:
/// (a) the child's events-derived state changes SIGNIFICANTLY vs. the
///     snapshot at entry — the default (pre-T89, significant) wake set is
///     `max_iters`, `last_iteration`, `budget_low_seen`, `goal_seen`,
///     `abort_seen`, `abort_reason` ([`DelegateSummary::significant_ne`]) —
///     or the events file's creation when it was missing at entry (the
///     launch→build window is exactly this state). `last_event_type`/
///     `last_event_ts` churn is deliberately NOT wake-worthy (T68): an
///     active child appends a `tool_result` event every 2–10 s, so the
///     pre-T68 any-field wake fired at the first poll tick almost every
///     time and every `wait_secs: 90–110` long-poll collapsed back into
///     per-tool-call polling. Churn is still RENDERED — the deadline leg's
///     final read carries it — it just never wakes the wait;
/// (b) the observed liveness flips alive → dead;
/// (c) the deadline elapses (`wait_secs`, already hard-capped at 600).
///
/// T89 `terminal` mode narrows the (a) wake set to the terminal facts —
/// `goal_seen`/`abort_seen` present in the current read, plus events-file
/// creation — so an actively-working child does NOT wake the wait on every
/// iteration advance or budget-low flip (loop-level economics: one
/// orchestrator iteration per child RUN, not per child iteration). The
/// verdict flags are PRESENCE-based, not transition-based: a fact already
/// observable at entry is returned immediately (the re-attach case — the
/// wait has nothing left to wait for); in the normal launch→wait flow both
/// flags are false at entry, so this coincides with the spec's "flips true"
/// wording. The T58 segment reset keeps the flags describing the LATEST
/// segment, so a resumed child re-arms them truthfully. Liveness and
/// file-creation stay transition-based (entry snapshot).
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
    terminal: bool,
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
                images: Vec::new(),
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
                images: Vec::new(),
            });
        }
        // Req 2(a) + T68: a SIGNIFICANT summary-field diff (an iteration
        // advance, max_iters appearing, a budget-low/goal/abort flag or the
        // abort reason — [`DelegateSummary::significant_ne`]), or the events
        // file appearing when it was missing at entry. A diff confined to
        // `last_event_type`/`last_event_ts` (per-tool-call churn) must NOT
        // wake — it would fire at the first poll tick nearly every time.
        // T89: terminal mode narrows this to the terminal facts — the
        // goal/abort verdict flags (presence semantics, see the fn doc) and
        // events-file creation; iteration advances, `budget_low_seen` flips,
        // and `max_iters` appearance are progress telemetry that never wake
        // a terminal wait. The wake cause is carried by the rendered flags
        // themselves (`goal_seen: true` …) — no new render lines.
        let state_changed = if terminal {
            now_summary.goal_seen
                || now_summary.abort_seen
                || (now_existed && !entry_existed)
        } else {
            now_summary.significant_ne(&entry_summary) || (now_existed && !entry_existed)
        };
        // Req 2(b): the observed liveness flipped alive → dead.
        let liveness_flipped = entry_alive == Some(true) && now_alive == Some(false);
        if state_changed || liveness_flipped {
            return Ok(ToolResult {
                content: render(&now_summary, now_alive, now_note.as_deref()),
                is_error: false,
                images: Vec::new(),
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
        images: Vec::new(),
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
mod tests;
