//! T10: append-only local event log at `<cwd>/.chug/events.jsonl`.
//!
//! Postmortems and meta-evaluations mine the transcript, which is lossy
//! (`[trimmed]` lines) and session-spliced; this log persists the useful
//! slice of the driver's structured event stream — run start, one line per
//! iteration with cumulative tokens, tool results with bounded previews,
//! verifications, goal verdicts, aborts — so a later session has a cheap,
//! untrimmed source of truth (`jq`-mineable, one JSON object per line).
//!
//! Hard rule: best-effort telemetry. Any open/write failure warns ONCE on
//! stderr and is otherwise ignored — the log must never change run
//! behavior. Previews are of tool results only; never request bodies, auth
//! headers, or ledger contents.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::archive;
use crate::events::{BudgetExceeded, Event, EventSink};
use crate::observ::now_rfc3339;

pub fn events_path(cwd: &Path) -> PathBuf {
    cwd.join(".chug").join("events.jsonl")
}

/// Fresh-run housekeeping: rotate a non-empty events log left by a previous
/// session to `.chug/events-<timestamp>.jsonl` BEFORE the new run's first
/// append — same lifecycle as the transcript (T7), called from the same
/// site so both files rotate together. `--resume` and chat keep appending.
pub fn rotate_fresh(cwd: &Path) -> archive::Outcome {
    let path = events_path(cwd);
    match fs::metadata(&path) {
        Ok(meta) if meta.len() > 0 => {
            archive::rotate(&path, &cwd.join(".chug"), "events", ".jsonl")
        }
        _ => archive::Outcome::Skipped,
    }
}

/// T115: SHA-256 over a goal's UTF-8 bytes, lowercase hex — the ONE shared
/// primitive for both ends of the delegate integrity comparison: the parent
/// hashes the exact goal string it passes to the child argv (the launch
/// result's `goal_sha256`), the child hashes the goal it actually received
/// (its `run_start` line's `goal_sha256`). Equal hashes mean the argv/pipe
/// delivered the goal byte-intact (the transmission class); unequal hashes
/// mean corruption in between. Pure so the `run_start` call sites stay thin.
/// `sha2` over a bespoke hash: the hex is cross-checkable with external
/// `shasum -a 256` for out-of-band verification.
pub(crate) fn goal_sha256(goal: &str) -> String {
    sha256_hex(goal.as_bytes())
}

/// T146: SHA-256 over raw bytes (lowercase hex) — the shared hasher behind
/// [`goal_sha256`] and the `--approve` plan file's `plan_sha256` (the file's
/// raw bytes, not its decoded text, is what is hashed: the on-disk artifact
/// is the approved thing).
pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        hex.push_str(&format!("{byte:02x}"));
    }
    hex
}

/// The run-start record: version, commit, model, spec path, cwd, mode —
/// the same fields as the T11 startup banner — plus the configured budget
/// ceilings (T17), so a post-hoc `jq` pass can ask "how close to the ceiling
/// did this run sail" even for runs that never aborted. `max_tokens` is
/// `null` when unset (0), never a phantom number. Written once per
/// autonomous run and once per chat session.
///
/// T115: `goal_sha256` is the SHA-256 ([`goal_sha256`]) of the goal's UTF-8
/// bytes when the run has a goal (run and plan modes), `null` when it
/// doesn't (chat opens goal-less) — the field is always present in the
/// serialized line, matching how `max_tokens` serializes null. This is the
/// child-side half of the delegate integrity comparison: equal to the
/// parent's launch-result `goal_sha256` iff the goal arrived byte-intact.
///
/// T117: `goal_pack` names the `.chug/commands/` pack the goal was expanded
/// from (F9 phase 2a — the CLI boundary replaced `--goal "/name args"` with
/// the pack body), `null` when the goal is literal. The field is always
/// present (same pattern as `goal_sha256`). `goal_sha256` hashes the
/// EXPANDED text — what the model actually received — so when `goal_pack`
/// is non-null the child's hash will NOT match a parent's delegate-launch
/// echo of the literal argv: that difference is the expansion, not a
/// transmission garble (the parent's `goal_tail` remains its garble
/// surface). Chat passes `null` (chat-side expansion is per-turn, T113).
///
/// T20: `head_branch`/`head_commit` carry the **cwd's** checkout identity
/// (see [`crate::build_info::resolve_head`]) so a harvested child stream
/// names the worktree it ran in, not just the binary's build commit. Both
/// are `null` when the identity couldn't be resolved (not a repo, git
/// missing) — null means unresolved, never a phantom string.
///
/// T143: `max_tokens_per_request` records the configured per-request output
/// cap (always present — the default 32768 applies even when unconfigured),
/// distinct from the cumulative `max_tokens` budget beside it. A post-hoc
/// `jq` pass can now ask whether a truncating run was riding the old 8192
/// shape.
/// T146: `approve` names the operator-approved plan file passed to
/// `chug run --approve <path>` (the path exactly as the operator typed it),
/// with `plan_sha256` the SHA-256 of that file's raw bytes — the F2 phase-2a
/// execution-contract pair, sitting next to the goal provenance fields above.
/// Both fields are ALWAYS present, `null` when the flag is absent (the T117
/// honesty shape; chat and plan mode never carry one and pass `null`).
#[allow(clippy::too_many_arguments)] // one line per field, same shape as the banner
pub fn run_start(
    cwd: &Path,
    mode: &str,
    spec: Option<&Path>,
    model: &str,
    max_iters: u32,
    max_minutes: u64,
    max_tokens: u64,
    max_tokens_per_request: u32,
    head: Option<(&str, &str)>,
    goal: Option<&str>,
    goal_pack: Option<&str>,
    approve: Option<&str>,
    plan_sha256: Option<&str>,
) {
    append_line(
        cwd,
        json!({
            "type": "run_start",
            "ts": now_rfc3339(),
            "mode": mode,
            "model": model,
            "spec": spec.map(|p| p.display().to_string()),
            "cwd": cwd.display().to_string(),
            "version": crate::build_info::VERSION,
            "commit": crate::build_info::GIT_COMMIT,
            "head_branch": head.map(|(b, _)| b),
            "head_commit": head.map(|(_, c)| c),
            "max_iters": max_iters,
            "max_minutes": max_minutes,
            "max_tokens": (max_tokens > 0).then_some(max_tokens),
            "max_tokens_per_request": max_tokens_per_request,
            "goal_sha256": goal.map(goal_sha256),
            "goal_pack": goal_pack,
            "approve": approve,
            "plan_sha256": plan_sha256,
        }),
    );
}

static WARNED: AtomicBool = AtomicBool::new(false);

/// Append one JSON object as a line, creating `.chug/` on demand. Failures
/// warn once and are dropped: telemetry never aborts a run.
fn append_line(cwd: &Path, line: Value) {
    if let Err(e) = append_line_inner(cwd, &line)
        && !WARNED.swap(true, Ordering::Relaxed)
    {
        eprintln!("chug: warning: events log write failed ({e}); continuing without it");
    }
}

fn append_line_inner(cwd: &Path, line: &Value) -> std::io::Result<()> {
    let dir = cwd.join(".chug");
    fs::create_dir_all(&dir)?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(events_path(cwd))?;
    writeln!(file, "{line}")?;
    Ok(())
}

/// An [`EventSink`] tee: mirrors the logged slice of the event stream to
/// `.chug/events.jsonl`, then forwards the event untouched to the inner
/// sink. Installed inside `drive_loop` so every mode (autonomous runs and
/// chat turns, scripted test runs included) is logged with zero config.
pub struct EventLogSink<'a> {
    cwd: PathBuf,
    inner: &'a mut dyn EventSink,
    /// The iteration number announced by the last `Iteration` event; merged
    /// into the iteration line written at the following `Usage` event.
    pending_iter: Option<u32>,
}

/// T25: the JSONL sink's error-leg preview window. The driver already
/// tail-selects error previews (see `driver::ERROR_PREVIEW_TAIL_CHARS`), so
/// the sink just passes up to this many chars through — a longer driver
/// window can never blow up the line. Ok results keep the T10-era 200-char
/// head, byte-identical to pre-T25.
const ERROR_PREVIEW_MAX_CHARS: usize = 2000;

impl<'a> EventLogSink<'a> {
    pub fn new(cwd: &Path, inner: &'a mut dyn EventSink) -> Self {
        EventLogSink {
            cwd: cwd.to_path_buf(),
            inner,
            pending_iter: None,
        }
    }
}

impl EventSink for EventLogSink<'_> {
    fn emit(&mut self, e: Event) {
        let line = match &e {
            // Held, not logged: the iteration line is written when Usage
            // arrives with the cumulative token counts.
            Event::Iteration { n, .. } => {
                self.pending_iter = Some(*n);
                None
            }
            Event::Usage { input, output } => Some(match self.pending_iter.take() {
                Some(n) => json!({
                    "type": "iteration",
                    "ts": now_rfc3339(),
                    "n": n,
                    "input_tokens": input,
                    "output_tokens": output,
                }),
                None => json!({
                    "type": "usage",
                    "ts": now_rfc3339(),
                    "input_tokens": input,
                    "output_tokens": output,
                }),
            }),
            Event::BudgetLow {
                remaining_iters,
                remaining_secs,
                remaining_tokens,
            } => Some(json!({
                "type": "budget_low",
                "ts": now_rfc3339(),
                "remaining_iters": remaining_iters,
                "remaining_secs": remaining_secs,
                "remaining_tokens": remaining_tokens,
            })),
            // T38: one line per injected truncation advisory — no dedup or
            // latch here, so `jq` counts truncations by counting lines.
            Event::OutputTruncated => Some(json!({
                "type": "output_truncated",
                "ts": now_rfc3339(),
            })),
            // T91: one line per degrade — the run is image-free from here on
            // (latched, so this fires at most once per run).
            Event::ImageDegraded => Some(json!({
                "type": "image_degraded",
                "ts": now_rfc3339(),
            })),
            Event::ToolResult {
                name,
                ok,
                duration_ms,
                preview,
            } => {
                // T25: failure-aware preview window. Ok results keep the
                // T10-era 200-char head of the driver's (500-char head)
                // preview, byte-identical to pre-T25. Error results pass up
                // to [`ERROR_PREVIEW_MAX_CHARS`] of the driver's
                // already-tail-selected preview through, so the failing
                // test's name at the end of the output survives the sink.
                let preview: String = if *ok {
                    preview.chars().take(200).collect()
                } else {
                    preview.chars().take(ERROR_PREVIEW_MAX_CHARS).collect()
                };
                Some(json!({
                    "type": "tool_result",
                    "ts": now_rfc3339(),
                    "name": name,
                    "ok": ok,
                    "is_error": !ok,
                    "duration_ms": duration_ms,
                    "preview": preview,
                }))
            }
            Event::HookFired {
                event,
                tool,
                command,
                exit,
                veto,
                duration_ms,
            } => Some(json!({
                "type": "hook",
                "ts": now_rfc3339(),
                "event": event,
                "tool": tool,
                "command": command,
                "exit": exit,
                "veto": veto,
                "duration_ms": duration_ms,
            })),
            // T83: one line per hooks problem (config load / spawn failure /
            // timeout); hooks fail open, so this is never a run killer.
            Event::HookError { detail } => Some(json!({
                "type": "hook_error",
                "ts": now_rfc3339(),
                "detail": detail,
            })),
            // T90: one line per permission deny (tool + the matched rule in
            // its config shape). A deny is fail-closed and never executes
            // the tool, so these lines are the postmortem record of what was
            // refused.
            Event::PermissionDenied { tool, rule } => Some(json!({
                "type": "permission_denied",
                "ts": now_rfc3339(),
                "tool": tool,
                "rule": rule,
            })),
            // T90: one line per permissions config problem (unreadable or
            // malformed config, or a malformed rule skipped while valid
            // siblings load); fail-open, never a run killer.
            Event::PermissionError { detail } => Some(json!({
                "type": "permission_error",
                "ts": now_rfc3339(),
                "detail": detail,
            })),
            Event::Verifying { cmd } => Some(json!({
                "type": "verifying",
                "ts": now_rfc3339(),
                "cmd": cmd,
            })),
            Event::GoalAccepted { summary } => Some(json!({
                "type": "goal",
                "ts": now_rfc3339(),
                "outcome": "accepted",
                "summary": summary,
            })),
            Event::GoalRejected { reason } => Some(json!({
                "type": "goal",
                "ts": now_rfc3339(),
                "outcome": "rejected",
                "reason": reason,
            })),
            Event::Aborted {
                reason,
                model,
                budget,
            } => {
                // T12: the abort line names the model that died, plus the
                // exhausted budget on budget deaths.
                let mut line = json!({
                    "type": "abort",
                    "ts": now_rfc3339(),
                    "reason": reason,
                    "model": model,
                });
                if let Some(budget) = budget {
                    let (kind, max) = match budget {
                        BudgetExceeded::Iterations { max } => ("iterations", u64::from(*max)),
                        BudgetExceeded::Minutes { max } => ("minutes", *max),
                        BudgetExceeded::Tokens { max } => ("tokens", *max),
                    };
                    line["budget_kind"] = json!(kind);
                    line["budget_max"] = json!(max);
                }
                Some(line)
            }
            // F7 phase 1 telemetry: a streaming request was answered with a
            // plain JSON body (proxy downgrade) and parsed byte-identically to
            // a non-streaming response. Latched first-per-session upstream
            // (one line max, no per-response spam) — the tools-proxy
            // compatibility probe.
            Event::StreamFallback => Some(json!({
                "type": "stream_fallback",
                "ts": now_rfc3339(),
            })),
            // F7 phase 1: live model-text deltas are console cosmetics only —
            // never transcript state, never event-log state (ModelText
            // precedent: model text stays out of the log, keeping events.jsonl
            // small). Pinned by silence tests on both sides.
            Event::ModelTextDelta(_) => None,
            // Everything else (model text, tool starts, ledger snapshots,
            // steering, risk verdicts, chat turn boundaries) stays out of
            // the log per the T10 spec.
            _ => None,
        };
        if let Some(line) = line {
            append_line(&self.cwd, line);
        }
        self.inner.emit(e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::Outcome;

    struct NullSink;
    impl EventSink for NullSink {
        fn emit(&mut self, _e: Event) {}
    }

    fn read_lines(cwd: &Path) -> Vec<Value> {
        fs::read_to_string(events_path(cwd))
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).expect("every line parses as JSON"))
            .collect()
    }

    #[test]
    fn run_start_line_has_model_spec_cwd_mode() {
        let tmp = tempfile::tempdir().unwrap();
        run_start(
            tmp.path(),
            "run",
            Some(Path::new("/repo/SPEC.md")),
            "test-model",
            40,
            120,
            0,
            crate::api::DEFAULT_MAX_TOKENS,
            None,
            // T115: the base-shape pin keeps the goal-less leg (null), so the
            // other fields' byte-identical representation is pinned both ways.
            // T117: a literal goal means no pack expansion (goal_pack null).
            None,
            None,
            // T146: approve/plan_sha256 ride every run_start line — null here.
            None,
            None,
        );
        let lines = read_lines(tmp.path());
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0]["type"], "run_start");
        assert_eq!(lines[0]["mode"], "run");
        assert_eq!(lines[0]["model"], "test-model");
        assert_eq!(lines[0]["spec"], "/repo/SPEC.md");
        assert_eq!(lines[0]["cwd"], tmp.path().display().to_string());
        // T11: the banner's build identification rides along.
        assert_eq!(lines[0]["version"], crate::build_info::VERSION);
        assert_eq!(lines[0]["commit"], crate::build_info::GIT_COMMIT);
        // T20, pinned representation: unresolved checkout → null, never a
        // phantom string (a tempdir is not a repo).
        assert!(
            lines[0]["head_branch"].is_null(),
            "unresolved head_branch stays null: {lines:?}"
        );
        assert!(
            lines[0]["head_commit"].is_null(),
            "unresolved head_commit stays null: {lines:?}"
        );
        // T17: the configured ceilings are on the record.
        assert_eq!(lines[0]["max_iters"], 40);
        assert_eq!(lines[0]["max_minutes"], 120);
        assert!(
            lines[0]["max_tokens"].is_null(),
            "no token budget → null, never a phantom 0"
        );
        assert!(lines[0]["ts"].as_str().unwrap().ends_with('Z'));
    }

    /// T17: a configured token budget is recorded as a number, so jq can
    /// distinguish "unset" from "set" on the opening line.
    #[test]
    fn run_start_records_token_budget_when_set() {
        let tmp = tempfile::tempdir().unwrap();
        run_start(tmp.path(), "run", None, "m", 8, 35, 250_000, crate::api::DEFAULT_MAX_TOKENS, None, None, None, None, None);
        let lines = read_lines(tmp.path());
        assert_eq!(lines[0]["max_iters"], 8);
        assert_eq!(lines[0]["max_minutes"], 35);
        assert_eq!(lines[0]["max_tokens"], 250_000);
    }

    /// T143: the configured per-request output cap rides the opening line —
    /// always present (the default 32768 applies even when unconfigured) and
    /// distinct from the cumulative `max_tokens` budget beside it (T15), so a
    /// post-hoc jq pass can tell a truncation-prone low cap from a run's
    /// total token budget.
    #[test]
    fn run_start_records_per_request_max_tokens_cap() {
        let tmp = tempfile::tempdir().unwrap();
        run_start(tmp.path(), "run", None, "m", 8, 35, 250_000, 8192, None, None, None, None, None);
        let lines = read_lines(tmp.path());
        assert_eq!(lines[0]["max_tokens_per_request"], 8192);
        assert_eq!(
            lines[0]["max_tokens"], 250_000,
            "the T15 cumulative budget is a different knob and must stay independent"
        );
        let obj = lines[0].as_object().expect("run_start is an object");
        assert!(
            obj.contains_key("max_tokens_per_request"),
            "the field must be PRESENT even at the default: {lines:?}"
        );
    }

    /// T20: when the caller resolved the cwd's checkout HEAD, the two
    /// strings ride the run_start line — the orientation a harvested child
    /// stream needs (branch@commit of the worktree it ran in).
    #[test]
    fn run_start_records_resolved_head_branch_and_commit() {
        let tmp = tempfile::tempdir().unwrap();
        run_start(
            tmp.path(),
            "run",
            None,
            "m",
            5,
            120,
            0,
            crate::api::DEFAULT_MAX_TOKENS,
            Some(("loop-t20", "9056c78")),
            None,
            None,
            None,
            None,
        );
        let lines = read_lines(tmp.path());
        assert_eq!(lines[0]["head_branch"], "loop-t20");
        assert_eq!(lines[0]["head_commit"], "9056c78");
        // The baked build commit stays untouched alongside the checkout's.
        assert_eq!(lines[0]["commit"], crate::build_info::GIT_COMMIT);
    }

    /// T115: the hash primitive against known vectors — `sha256("hello
    /// world")` and the empty string, both cross-checkable with external
    /// `shasum -a 256` (and confirmed against Python's hashlib pre-pin).
    #[test]
    fn goal_sha256_known_vectors() {
        assert_eq!(
            goal_sha256("hello world"),
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
        assert_eq!(
            goal_sha256(""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            "an empty goal still hashes (the child records the empty goal's digest)"
        );
    }

    /// T115: run_start records the goal's SHA-256 when the run has a goal —
    /// the child-side half of the delegate integrity comparison, pinned here
    /// against the same vector `shasum -a 256` gives for the fixed string.
    #[test]
    fn run_start_records_goal_sha256_when_goal_present() {
        let tmp = tempfile::tempdir().unwrap();
        run_start(
            tmp.path(),
            "run",
            None,
            "m",
            8,
            35,
            0,
            crate::api::DEFAULT_MAX_TOKENS,
            None,
            Some("hello world"),
            None,
            None,
            None,
        );
        let lines = read_lines(tmp.path());
        assert_eq!(
            lines[0]["goal_sha256"],
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9",
            "{lines:?}"
        );
    }

    /// T115: goal-less modes (chat) record null — but the FIELD is always
    /// present in the serialized line (matching how `max_tokens` serializes
    /// null), so jq can distinguish "no goal" from a truncated line.
    #[test]
    fn run_start_goal_sha256_null_but_always_present_without_goal() {
        let tmp = tempfile::tempdir().unwrap();
        run_start(tmp.path(), "chat", None, "m", 8, 35, 0, crate::api::DEFAULT_MAX_TOKENS, None, None, None, None, None);
        let lines = read_lines(tmp.path());
        assert!(
            lines[0]["goal_sha256"].is_null(),
            "no goal → null, never a phantom hash: {lines:?}"
        );
        let obj = lines[0].as_object().expect("run_start is an object");
        assert!(
            obj.contains_key("goal_sha256"),
            "the field must be PRESENT even when null: {lines:?}"
        );
    }

    /// T117: when the goal was expanded from a pack, `goal_pack` names it on
    /// the opening line — the provenance of the goal transformation.
    #[test]
    fn run_start_records_goal_pack_when_goal_was_expanded() {
        let tmp = tempfile::tempdir().unwrap();
        run_start(
            tmp.path(),
            "run",
            None,
            "m",
            8,
            35,
            0,
            crate::api::DEFAULT_MAX_TOKENS,
            None,
            Some("the expanded body"),
            Some("smoke"),
            None,
            None,
        );
        let lines = read_lines(tmp.path());
        assert_eq!(lines[0]["goal_pack"], "smoke", "{lines:?}");
    }

    /// T117: a literal goal records null — but the FIELD is always present
    /// (the T115 always-present pattern applied to the new field), so jq can
    /// distinguish "no pack expansion" from a truncated line.
    #[test]
    fn run_start_goal_pack_null_but_always_present_on_a_plain_goal() {
        let tmp = tempfile::tempdir().unwrap();
        run_start(tmp.path(), "run", None, "m", 8, 35, 0, crate::api::DEFAULT_MAX_TOKENS, None, Some("plain"), None, None, None);
        let lines = read_lines(tmp.path());
        assert!(
            lines[0]["goal_pack"].is_null(),
            "no expansion → null, never a phantom name: {lines:?}"
        );
        let obj = lines[0].as_object().expect("run_start is an object");
        assert!(
            obj.contains_key("goal_pack"),
            "the field must be PRESENT even when null: {lines:?}"
        );
    }

    /// T146: the approved-plan pair rides the opening line both ways — the
    /// path as typed + the file-bytes hash when `--approve` was given, both
    /// fields PRESENT but null when it wasn't (the T117 honesty shape).
    #[test]
    fn run_start_records_approve_fields_both_ways() {
        for (approve, plan_sha) in [
            (Some("plan.md"), Some("deadbeef")),
            (None, None),
        ] {
            let tmp = tempfile::tempdir().unwrap();
            run_start(
                tmp.path(),
                "run",
                None,
                "m",
                8,
                35,
                0,
                crate::api::DEFAULT_MAX_TOKENS,
                None,
                None,
                None,
                approve,
                plan_sha,
            );
            let lines = read_lines(tmp.path());
            match approve {
                Some(path) => {
                    assert_eq!(lines[0]["approve"], path, "{lines:?}");
                    assert_eq!(lines[0]["plan_sha256"], plan_sha.unwrap());
                }
                None => {
                    assert!(lines[0]["approve"].is_null(), "{lines:?}");
                    assert!(lines[0]["plan_sha256"].is_null(), "{lines:?}");
                }
            }
            let obj = lines[0].as_object().expect("run_start is an object");
            assert!(obj.contains_key("approve"), "{lines:?}");
            assert!(obj.contains_key("plan_sha256"), "{lines:?}");
        }
    }

    /// T117 known vector: `goal_sha256` hashes the EXPANDED text — what the
    /// model actually received (transmission truth) — so the recorded hash
    /// matches the expanded body's SHA-256 and does NOT match the literal
    /// `"/name args"` invocation the parent echoes. This is the designed
    /// parent/child hash difference when a pack fired (the expansion, not a
    /// transmission garble).
    #[test]
    fn run_start_goal_sha256_hashes_the_expanded_body_not_the_invocation() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = crate::commands::commands_dir(tmp.path());
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("smoke.md"), "Say hello to $ARGUMENTS.").unwrap();
        let crate::commands::Expansion::Body(expanded) =
            crate::commands::expand(tmp.path(), "smoke", Some("hello"))
        else {
            panic!("the smoke pack must expand");
        };
        assert_eq!(expanded, "Say hello to hello.");

        run_start(
            tmp.path(),
            "run",
            None,
            "m",
            8,
            35,
            0,
            crate::api::DEFAULT_MAX_TOKENS,
            None,
            Some(&expanded),
            Some("smoke"),
            None,
            None,
        );
        let lines = read_lines(tmp.path());
        assert_eq!(lines[0]["goal_pack"], "smoke");
        assert_eq!(
            lines[0]["goal_sha256"],
            goal_sha256("Say hello to hello."),
            "the hash is over the EXPANDED body: {lines:?}"
        );
        assert_ne!(
            lines[0]["goal_sha256"],
            goal_sha256("/smoke hello"),
            "the literal invocation's hash must NOT appear — the expansion rewrote the goal"
        );
    }

    /// T17: a BudgetLow event serializes as one jq-mineable line, with
    /// `remaining_tokens` null when no token budget is set.
    #[test]
    fn sink_logs_budget_low_without_token_budget() {
        let tmp = tempfile::tempdir().unwrap();
        let mut inner = NullSink;
        let mut sink = EventLogSink::new(tmp.path(), &mut inner);
        sink.emit(Event::BudgetLow {
            remaining_iters: 3,
            remaining_secs: 150,
            remaining_tokens: None,
        });
        let lines = read_lines(tmp.path());
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0]["type"], "budget_low");
        assert_eq!(lines[0]["remaining_iters"], 3);
        assert_eq!(lines[0]["remaining_secs"], 150);
        assert!(
            lines[0]["remaining_tokens"].is_null(),
            "unset token budget stays null: {lines:?}"
        );
        assert!(lines[0]["ts"].as_str().unwrap().ends_with('Z'));
    }

    /// T17: the same line carries the remaining tokens when a token budget
    /// is configured.
    #[test]
    fn sink_logs_budget_low_with_remaining_tokens() {
        let tmp = tempfile::tempdir().unwrap();
        let mut inner = NullSink;
        let mut sink = EventLogSink::new(tmp.path(), &mut inner);
        sink.emit(Event::BudgetLow {
            remaining_iters: 50,
            remaining_secs: 7_000,
            remaining_tokens: Some(49_000),
        });
        let lines = read_lines(tmp.path());
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0]["type"], "budget_low");
        assert_eq!(lines[0]["remaining_iters"], 50);
        assert_eq!(lines[0]["remaining_secs"], 7_000);
        assert_eq!(lines[0]["remaining_tokens"], 49_000);
    }

    /// T38: an OutputTruncated event serializes as one jq-mineable
    /// `output_truncated` line, and two emissions write two lines — the sink
    /// never latches, so `jq` counts truncations by counting lines.
    #[test]
    fn sink_logs_output_truncated_one_line_per_event() {
        let tmp = tempfile::tempdir().unwrap();
        let mut inner = NullSink;
        let mut sink = EventLogSink::new(tmp.path(), &mut inner);
        sink.emit(Event::OutputTruncated);
        sink.emit(Event::OutputTruncated);
        let lines = read_lines(tmp.path());
        assert_eq!(lines.len(), 2);
        for line in &lines {
            assert_eq!(line["type"], "output_truncated");
            assert!(line["ts"].as_str().unwrap().ends_with('Z'));
        }
    }

    #[test]
    fn rotate_fresh_archives_non_empty_events_log() {
        let tmp = tempfile::tempdir().unwrap();
        run_start(tmp.path(), "run", None, "m", 5, 120, 0, crate::api::DEFAULT_MAX_TOKENS, None, None, None, None, None);

        let out = rotate_fresh(tmp.path());
        let Outcome::Archived(dst) = out else {
            panic!("expected Archived, got {out:?}");
        };
        let name = dst.file_name().unwrap().to_string_lossy().to_string();
        assert!(name.starts_with("events-") && name.ends_with(".jsonl"), "{name}");
        assert!(fs::read_to_string(&dst).unwrap().contains("run_start"));
        assert!(!events_path(tmp.path()).exists(), "log moved away");

        // Absent now → skipped; empty file → skipped and left alone.
        assert_eq!(rotate_fresh(tmp.path()), Outcome::Skipped);
        fs::write(events_path(tmp.path()), "").unwrap();
        assert_eq!(rotate_fresh(tmp.path()), Outcome::Skipped);
        assert!(events_path(tmp.path()).exists());
    }

    #[test]
    fn sink_merges_iteration_and_usage_into_one_line() {
        let tmp = tempfile::tempdir().unwrap();
        let mut inner = NullSink;
        let mut sink = EventLogSink::new(tmp.path(), &mut inner);
        sink.emit(Event::Iteration {
            n: 3,
            max: 40,
            messages: 12,
        });
        sink.emit(Event::Usage {
            input: 1234,
            output: 56,
        });
        let lines = read_lines(tmp.path());
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0]["type"], "iteration");
        assert_eq!(lines[0]["n"], 3);
        assert_eq!(lines[0]["input_tokens"], 1234);
        assert_eq!(lines[0]["output_tokens"], 56);
    }

    /// The pre-T25 200-char pin, re-anchored by T25 to the **ok leg**: the
    /// fixture was `ok: false` before T25, but a 500-char error preview now
    /// passes through whole (≤2000), so the 200-char head-truncation pins
    /// ok results — exactly today's behavior, unchanged.
    #[test]
    fn sink_truncates_tool_preview_at_200_chars() {
        let tmp = tempfile::tempdir().unwrap();
        let mut inner = NullSink;
        let mut sink = EventLogSink::new(tmp.path(), &mut inner);
        sink.emit(Event::ToolResult {
            name: "bash".into(),
            ok: true,
            duration_ms: 7,
            preview: "x".repeat(500),
        });
        let lines = read_lines(tmp.path());
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0]["type"], "tool_result");
        assert_eq!(lines[0]["ok"], true);
        assert_eq!(lines[0]["is_error"], false);
        assert_eq!(lines[0]["duration_ms"], 7);
        let preview = lines[0]["preview"].as_str().unwrap();
        assert_eq!(preview.chars().count(), 200);
    }

    /// T25: the ok leg is head-anchored, not just capped — the FIRST 200
    /// chars of a long ok preview survive, never the tail.
    #[test]
    fn sink_ok_preview_keeps_head_not_tail() {
        let tmp = tempfile::tempdir().unwrap();
        let mut inner = NullSink;
        let mut sink = EventLogSink::new(tmp.path(), &mut inner);
        sink.emit(Event::ToolResult {
            name: "bash".into(),
            ok: true,
            duration_ms: 1,
            preview: format!("{}{}", "a".repeat(250), "z".repeat(250)),
        });
        let lines = read_lines(tmp.path());
        let preview = lines[0]["preview"].as_str().unwrap();
        assert_eq!(preview.chars().count(), 200);
        assert!(preview.starts_with(&"a".repeat(200)), "head kept: {preview}");
        assert!(!preview.contains('z'), "tail dropped: {preview}");
    }

    /// T25: the error leg's window at the sink is 2000 chars, not the
    /// T10-era 200 — a long error preview is capped at exactly 2000 chars.
    #[test]
    fn sink_error_preview_capped_at_2000_chars() {
        let tmp = tempfile::tempdir().unwrap();
        let mut inner = NullSink;
        let mut sink = EventLogSink::new(tmp.path(), &mut inner);
        sink.emit(Event::ToolResult {
            name: "bash".into(),
            ok: false,
            duration_ms: 7,
            preview: "x".repeat(2500),
        });
        let lines = read_lines(tmp.path());
        assert_eq!(lines[0]["ok"], false);
        assert_eq!(lines[0]["is_error"], true);
        let preview = lines[0]["preview"].as_str().unwrap();
        assert_eq!(preview.chars().count(), 2000);
    }

    /// T25, the non-vacuous heart: a SHORT error preview is kept whole.
    /// Pre-T25 the flat 200-char head-take truncated these 300 chars to 200,
    /// losing the `FAILED <test name>` tail.
    #[test]
    fn sink_keeps_short_error_preview_whole() {
        let tmp = tempfile::tempdir().unwrap();
        let mut inner = NullSink;
        let mut sink = EventLogSink::new(tmp.path(), &mut inner);
        let mut preview = "x".repeat(277);
        preview.push_str("FAILED tests::t25_flake"); // 23 chars → 300 total
        assert_eq!(preview.chars().count(), 300);
        sink.emit(Event::ToolResult {
            name: "bash".into(),
            ok: false,
            duration_ms: 7,
            preview,
        });
        let lines = read_lines(tmp.path());
        let out = lines[0]["preview"].as_str().unwrap();
        assert_eq!(out.chars().count(), 300, "all 300 chars kept: {out}");
        assert!(out.ends_with("FAILED tests::t25_flake"), "tail kept: {out}");
    }

    /// T25, production shape: the driver already tail-selected ≤2000 chars,
    /// so the sink must pass the whole error preview through — the
    /// `failures:` block at the END survives (pre-T25 it was cut to the
    /// first 200 chars).
    #[test]
    fn sink_passes_tail_selected_error_preview_through_whole() {
        let tmp = tempfile::tempdir().unwrap();
        let mut inner = NullSink;
        let mut sink = EventLogSink::new(tmp.path(), &mut inner);
        let mut preview = "x".repeat(1900);
        preview.push_str("failures:\n    tests::the_flaky_one"); // 34 chars → 1934
        sink.emit(Event::ToolResult {
            name: "bash".into(),
            ok: false,
            duration_ms: 7,
            preview,
        });
        let lines = read_lines(tmp.path());
        let out = lines[0]["preview"].as_str().unwrap();
        assert_eq!(out.chars().count(), 1934);
        assert!(
            out.ends_with("failures:\n    tests::the_flaky_one"),
            "tail-selected preview passes through whole: {out}"
        );
    }

    /// T25: a >2000-char error preview full of multibyte chars serializes
    /// without panic, stays within the char budget, and is cut on char
    /// boundaries (chars(), never byte slicing).
    #[test]
    fn sink_error_preview_multibyte_stays_in_char_budget() {
        let tmp = tempfile::tempdir().unwrap();
        let mut inner = NullSink;
        let mut sink = EventLogSink::new(tmp.path(), &mut inner);
        // "é中🦀" = 3 chars / 9 bytes; 800 repeats = 2400 chars, 7200 bytes.
        let unit = "é中🦀";
        let preview = unit.repeat(800);
        assert_eq!(preview.chars().count(), 2400);
        // The expected serialization: the first 2000 CHARS of the preview
        // (666 units + "é中") — never a mid-character byte cut.
        let expected: String = preview.chars().take(ERROR_PREVIEW_MAX_CHARS).collect();
        sink.emit(Event::ToolResult {
            name: "bash".into(),
            ok: false,
            duration_ms: 7,
            preview,
        });
        // read_lines parses every line as JSON: no panic, valid encoding.
        let lines = read_lines(tmp.path());
        let out = lines[0]["preview"].as_str().unwrap();
        assert_eq!(out, expected, "cut on char boundaries, within the budget");
        assert_eq!(out.chars().count(), 2000);
    }

    /// F7 phase 1: the streamed-response telemetry is exactly ONE line
    /// (latched upstream), and live deltas NEVER enter the log (ModelText
    /// precedent — events.jsonl stays small).
    #[test]
    fn stream_fallback_latches_one_line_and_deltas_stay_silent() {
        let tmp = tempfile::tempdir().unwrap();
        let mut inner = NullSink;
        let mut sink = EventLogSink::new(tmp.path(), &mut inner);
        sink.emit(Event::StreamFallback);
        sink.emit(Event::StreamFallback);
        sink.emit(Event::ModelTextDelta("typed".into()));
        let lines = read_lines(tmp.path());
        assert_eq!(lines.len(), 2, "two StreamFallback lines: the arm is not the latch");
        assert_eq!(lines[0]["type"], "stream_fallback");
        assert_eq!(lines[1]["type"], "stream_fallback");
    }

    #[test]
    fn sink_logs_goal_abort_and_verifying_but_not_other_events() {
        let tmp = tempfile::tempdir().unwrap();
        let mut inner = NullSink;
        let mut sink = EventLogSink::new(tmp.path(), &mut inner);
        sink.emit(Event::ModelText("hello".into()));
        sink.emit(Event::ToolStart { name: "bash".into() });
        sink.emit(Event::LedgerChanged("# Ledger".into()));
        sink.emit(Event::SteeringQueued("note".into()));
        sink.emit(Event::Verifying {
            cmd: "cargo test".into(),
        });
        sink.emit(Event::GoalAccepted {
            summary: "did it".into(),
        });
        sink.emit(Event::GoalRejected {
            reason: "check command failed".into(),
        });
        sink.emit(Event::Aborted {
            reason: "iteration budget exceeded".into(),
            model: "test-model".into(),
            budget: Some(BudgetExceeded::Iterations { max: 40 }),
        });
        let lines = read_lines(tmp.path());
        let types: Vec<&str> = lines.iter().map(|l| l["type"].as_str().unwrap()).collect();
        assert_eq!(types, ["verifying", "goal", "goal", "abort"]);
        assert_eq!(lines[0]["cmd"], "cargo test");
        assert_eq!(lines[1]["outcome"], "accepted");
        assert_eq!(lines[1]["summary"], "did it");
        assert_eq!(lines[2]["outcome"], "rejected");
        assert_eq!(lines[2]["reason"], "check command failed");
        assert_eq!(lines[3]["reason"], "iteration budget exceeded");
        // T12: model + exhausted budget ride the abort line.
        assert_eq!(lines[3]["model"], "test-model");
        assert_eq!(lines[3]["budget_kind"], "iterations");
        assert_eq!(lines[3]["budget_max"], 40);
    }

    #[test]
    fn unwritable_events_path_is_silently_ignored() {
        let tmp = tempfile::tempdir().unwrap();
        // Poison the log path: a directory where the file should be makes
        // every append open fail. Nothing may panic.
        fs::create_dir_all(events_path(tmp.path())).unwrap();
        let mut inner = NullSink;
        let mut sink = EventLogSink::new(tmp.path(), &mut inner);
        sink.emit(Event::Iteration {
            n: 1,
            max: 1,
            messages: 1,
        });
        sink.emit(Event::Usage { input: 1, output: 1 });
        sink.emit(Event::Aborted {
            reason: "operator abort".into(),
            model: "m".into(),
            budget: None,
        });
        run_start(tmp.path(), "run", None, "m", 5, 120, 0, crate::api::DEFAULT_MAX_TOKENS, None, None, None, None, None);
    }

    /// T91: an ImageDegraded event serializes as one jq-mineable
    /// `image_degraded` line (the driver latches, so at most one per run —
    /// pinned at the driver level).
    #[test]
    fn sink_logs_image_degraded_as_one_line() {
        let tmp = tempfile::tempdir().unwrap();
        let mut inner = NullSink;
        let mut sink = EventLogSink::new(tmp.path(), &mut inner);
        sink.emit(Event::ImageDegraded);
        let lines = read_lines(tmp.path());
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0]["type"], "image_degraded");
        assert!(lines[0]["ts"].as_str().unwrap().ends_with('Z'));
    }
}
