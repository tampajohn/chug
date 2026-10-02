// T104 family: live-context editing (T192 — F14 phase 2). The model curates
// its own message list by editing the `.chug/LIVE_CTX.md` mirror the driver
// writes pre-call; the driver parses the edit back behind a gate and, on
// accept, splices the transcript.
use super::*;
use crate::api::Response;
use crate::live_ctx;

// ---------- the LLM double ----------

/// An LLM double whose every non-final call edits the on-disk mirror: it
/// reads the bytes the driver just wrote pre-call, drops the given turn
/// indices (whole blocks, the scripted-edit shape), and answers with a real
/// `write_file` tool_use targeting `.chug/LIVE_CTX.md`. The driver's
/// dispatch then really writes those bytes, so the end-of-turn check sees a
/// genuine model edit. The final scripted response is [`goal_complete`].
/// One scripted edit, relative to the mirror the driver just wrote (fixed
/// indices would shift as markers accumulate at the end of the prefix).
#[derive(Clone, Copy)]
enum Edit {
    /// Read-only turn: no edit, just a read_file call.
    None,
    /// Drop the last `n` mirrored turns.
    DropLast(usize),
}

struct CtxEditorLlm {
    cwd: PathBuf,
    /// The edit per call, in call order (exhausted → the scripted
    /// `responses` queue).
    drops: std::collections::VecDeque<Edit>,
    /// Usage to report per call (the default mirrors `tool_use_response`).
    usage: (u64, u64),
    /// Tool name for the non-edit calls preceding the edit rounds.
    calls: Vec<(String, Vec<Message>)>,
    responses: std::collections::VecDeque<Value>,
}

impl CtxEditorLlm {
    fn new(cwd: &Path, drops: Vec<Edit>) -> Self {
        CtxEditorLlm {
            cwd: cwd.to_path_buf(),
            drops: drops.into(),
            usage: (10, 5),
            calls: Vec::new(),
            responses: std::collections::VecDeque::new(),
        }
    }

    fn with_usage(mut self, input: u64, output: u64) -> Self {
        self.usage = (input, output);
        self
    }

    fn then_goal_complete(mut self) -> Self {
        self.responses.push_back(tool_use_response(
            "goal_complete",
            json!({"summary": "compacted and done"}),
        ));
        self
    }

    fn then_read_big_file(mut self, path: &str) -> Self {
        self.responses
            .push_back(tool_use_response("read_file", json!({"path": path})));
        self
    }
}

/// Drop whole `[[CTX_TURN i]]` blocks (header + body lines) from mirror
/// text: either the named indices or the last `n` turns.
fn drop_turns(text: &str, edit: Edit) -> String {
    let indexes: Vec<usize> = match edit {
        Edit::None => return text.to_string(),
        Edit::DropLast(n) => {
            let total = text.lines().filter(|l| l.starts_with("[[CTX_TURN ")).count();
            (total.saturating_sub(n)..total).collect()
        }
    };
    let mut out = String::new();
    let mut skipping = false;
    for line in text.lines() {
        if let Some(inner) = line
            .strip_prefix("[[CTX_TURN ")
            .and_then(|s| s.strip_suffix("]]"))
        {
            let index = inner.split(' ').next().unwrap_or_default();
            skipping = index.parse::<usize>().is_ok_and(|i| indexes.contains(&i));
        }
        if !skipping {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

impl Llm for CtxEditorLlm {
    fn complete(
        &mut self,
        system: &str,
        messages: &[Message],
        _tools: &[Value],
        _obs: &ObsCtx<'_>,
    ) -> anyhow::Result<Response> {
        self.calls.push((system.to_string(), messages.to_vec()));
        if let Some(edit) = self.drops.pop_front() {
            if matches!(edit, Edit::None) {
                // Read-only turn: no edit, just look at a big file. The
                // usage is the CONFIGURED one (default 10/5, exactly what
                // `tool_use_response` hardcodes), so a leg can make the
                // counted read pay real tokens.
                let mut body = tool_use_response("read_file", json!({"path": "notes.txt"}));
                let (input, output) = self.usage;
                body["usage"] = json!({"input_tokens": input, "output_tokens": output});
                return Ok(Response { body });
            }
            // The driver mirrored the pre-call list to the file moments ago.
            let mirror = std::fs::read_to_string(live_ctx::live_ctx_path(&self.cwd))
                .expect("the driver wrote the mirror pre-call");
            let edited = drop_turns(&mirror, edit);
            assert_ne!(edited, mirror, "scripted edit must differ");
            let (input, output) = self.usage;
            return Ok(Response {
                body: json!({
                    "stop_reason": "tool_use",
                    "usage": {"input_tokens": input, "output_tokens": output},
                    "content": [{"type": "tool_use", "id": "tu_1", "name": "write_file",
                                 "input": {"path": ".chug/LIVE_CTX.md", "content": edited}}],
                }),
            });
        }
        self.responses
            .pop_front()
            .map(|body| Response { body })
            .ok_or_else(|| anyhow::anyhow!("CtxEditorLlm: no scripted response left"))
    }

    fn set_model(&mut self, _model: &str) {}

    fn model(&self) -> &str {
        "ctx-editor-model"
    }
}

fn is_ctx_marker(m: &Message) -> bool {
    live_ctx::is_ctx_edit_marker(m)
}

fn ctx_markers(messages: &[Message]) -> Vec<&Message> {
    messages.iter().filter(|m| is_ctx_marker(m)).collect()
}

fn ctx_events(sink: &RecordingSink) -> Vec<(bool, u64, u64, Option<String>)> {
    sink.0
        .iter()
        .filter_map(|e| match e {
            Event::CtxEdit {
                accepted,
                before_tokens,
                after_tokens,
                reason,
            } => Some((*accepted, *before_tokens, *after_tokens, reason.clone())),
            _ => None,
        })
        .collect()
}

// ---------- the free compaction turn (requirement 4) ----------

/// A turn whose only effect is an accepted LIVE_CTX edit does not advance
/// the iteration counter: with `max_iters = 3`, three free edit turns plus
/// the counted work turns still complete the run. Had the free turns
/// counted, the run would have aborted at the third boundary — 6 LLM calls
/// would be impossible.
#[test]
fn free_edit_turns_do_not_advance_the_iteration_counter() {
    let tmp = tempfile::tempdir().unwrap();
    let (_utx, urx) = mpsc::channel::<SlashUpdate>();
    let controls = Controls::detached();
    let ctx = ctx_for(&tmp, Mode::Autonomous, &controls, &urx, None, &observ::Sink::Noop);
    let mut knobs = knobs_with(3);
    // Seed the goal message the run loop would build (drive_loop starts
    // from an empty list; run_loop always has message 0).
    let messages = vec![Message::user(vec![ContentBlock::text_block(
        "Goal: compact me",
    )])];

    // Call 1 is read-only (empty drop entry); calls 2-5 are consecutive
    // accepted edits (drop the read pair, then each turn's own write pair).
    // With max_iters=3 the T17 budget-low notice also fires at the first
    // boundary (remaining 3 <= 8), landing at transcript index 1 — the
    // scripted drops target the read pair at turns [2,3] behind it.
    let mut llm = CtxEditorLlm::new(
        tmp.path(),
        vec![Edit::None, Edit::DropLast(2), Edit::DropLast(2), Edit::DropLast(2), Edit::DropLast(2)],
    )
    .then_goal_complete();
    // notes.txt exists (the read-only turn really reads it).
    std::fs::write(tmp.path().join("notes.txt"), "x".repeat(4_000)).unwrap();

    let mut gate = None;
    let mut messages = messages;
    let mut sink = RecordingSink::default();
    let outcome = drive_loop(
        &ctx,
        &mut knobs,
        &mut llm,
        &mut gate,
        &mut messages,
        Some("check: true".to_string()),
        &mut sink,
        &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
    )
    .unwrap();
    let kinds = sink
        .0
        .iter()
        .map(|e| match e {
            Event::CtxEdit { .. } => "CtxEdit".to_string(),
            Event::LedgerChanged(_) => "Ledger".to_string(),
            Event::Verifying { .. } => "Verifying".to_string(),
            Event::GoalAccepted { .. } => "Accepted".to_string(),
            Event::GoalRejected { .. } => "Rejected".to_string(),
            Event::Aborted { reason, .. } => format!("Aborted({reason})"),
            _ => "other".to_string(),
        })
        .collect::<Vec<_>>();
    let last_tools = llm
        .calls
        .iter()
        .filter_map(|(_, ms)| {
            ms.iter().rev().find_map(|m| {
                m.content.iter().find_map(|b| match b {
                    crate::api::ContentBlock::Known(crate::api::KnownBlock::ToolUse { name, .. }) => {
                        Some(name.clone())
                    }
                    _ => None,
                })
            })
        })
        .collect::<Vec<_>>();
    assert!(
        matches!(outcome, DriveOutcome::RunFinished(0)),
        "the run completed: three free edit turns + two counted turns fit in max_iters=3\n\
         outcome: {outcome:?}\ncalls: {}\nctx events: {:?}\nkinds: {:?}\nlast_tools: {:?}\nlast call messages: {}",
        llm.calls.len(),
        ctx_events(&sink),
        kinds,
        last_tools,
        llm.calls
            .last()
            .and_then(|(_, ms)| ms.last())
            .and_then(|m| m.content[0].text())
            .map(String::from)
            .unwrap_or_default()
    );
    assert_eq!(
        llm.calls.len(),
        6,
        "1 read + 4 edits + 1 goal_complete; free turns never burned an iteration"
    );
    // Four accepted edits (the read turn attempted none).
    let events = ctx_events(&sink);
    assert_eq!(events.len(), 4, "{events:?}");
    assert!(events.iter().all(|(accepted, ..)| *accepted));
    assert!(events.iter().all(|(_, _, _, reason)| reason.is_none()));
    // Strictly smaller every time.
    assert!(events
        .iter()
        .all(|(_, before, after, _)| after < before));

    // One marker per accepted edit's deleted run, in the final list.
    let markers = ctx_markers(&messages);
    assert_eq!(markers.len(), 4, "{:?}", markers.iter().map(|m| m.content[0].text().unwrap()).collect::<Vec<_>>());
}

/// The 4th consecutive free edit turn counts normally: with `max_iters = 4`
/// the run aborts on the iteration budget at the top of the 6th boundary
/// (1 counted read + 3 free edits + 1 counted 4th edit = 2 … the abort
/// lands when the counted total reaches the budget).
#[test]
fn fourth_consecutive_free_edit_counts_normally() {
    let tmp = tempfile::tempdir().unwrap();
    let (_utx, urx) = mpsc::channel::<SlashUpdate>();
    let controls = Controls::detached();
    let ctx = ctx_for(&tmp, Mode::Autonomous, &controls, &urx, None, &observ::Sink::Noop);
    // Budget: 1 (the read) + 3 (the free edits' re-arm ceiling) — the 4th
    // edit turn pushes the counter to 2; the 5th edit turn is refused at
    // the boundary (2 < 4 passes, 3 aborts)... the abort lands at the
    // boundary BEFORE the 6th call.
    let mut knobs = knobs_with(2);
    let messages = vec![Message::user(vec![ContentBlock::text_block("Goal: cap me")])];

    // Five consecutive edit turns: 1, 2, 3 free; 4, 5 counted. (The T17
    // budget-low notice fires at the first boundary too — remaining 2 <= 8 —
    // landing at index 1, ahead of the read pair the drops target.)
    let mut llm = CtxEditorLlm::new(
        tmp.path(),
        vec![Edit::None, Edit::DropLast(2), Edit::DropLast(2), Edit::DropLast(2), Edit::DropLast(2)],
    )
    .then_read_big_file("notes.txt");
    std::fs::write(tmp.path().join("notes.txt"), "x".repeat(4_000)).unwrap();

    let mut gate = None;
    let mut messages = messages;
    let mut sink = RecordingSink::default();
    let outcome = drive_loop(
        &ctx,
        &mut knobs,
        &mut llm,
        &mut gate,
        &mut messages,
        Some("check: true".to_string()),
        &mut sink,
        &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
    )
    .unwrap();
    // Aborted on the iteration budget, not the goal.
    assert!(matches!(outcome, DriveOutcome::RunFinished(1)), "{outcome:?}");
    // 1 (read) + 3 (free) + 1 (the counted 4th edit) = 5 calls; the 5th
    // edit turn is refused at the 6th boundary (counter would hit 3).
    assert_eq!(llm.calls.len(), 5, "free: 3, then the 4th edit counted");
    // Four accepted edits, one of which (the 4th) consumed an iteration.
    assert_eq!(ctx_events(&sink).len(), 4);
    // And the abort reason is the iteration budget.
    let aborted = sink.0.iter().any(|e| {
        matches!(
            e,
            Event::Aborted {
                reason,
                ..
            } if reason == "iteration budget exceeded"
        )
    });
    assert!(aborted, "the 4th free edit's iteration exhausted the budget");
}

/// The token budget always binds — even on a genuinely FREE edit turn
/// (T192's carried finding 1, reworked: the old leg scripted only REJECTED
/// edits, so every turn counted and the free-turn usage path was never
/// exercised; this one drives an ACCEPTED edit through the real gate).
///
/// The T15 check sits at the top of the loop over CUMULATIVE usage, which a
/// free turn's response already added to. The run is pinned by TWO budgets
/// at once: `max_iters = 2` convicts a COUNTED edit turn (the iteration
/// check precedes the token check at the boundary, so the abort reason
/// would read "iteration budget exceeded"), while a genuinely free edit
/// turn leaves the counter at 1 and the token check sees 80k+80k from the
/// counted read plus 80k+80k from the free edit = 320k >= 300k. Dying on
/// TOKENS is therefore the observable that the edit turn was free; crossing
/// 300k at all is the observable that its usage accumulated (always-binds).
#[test]
fn token_budget_binds_on_free_edit_turns() {
    let tmp = tempfile::tempdir().unwrap();
    let (_utx, urx) = mpsc::channel::<SlashUpdate>();
    let controls = Controls::detached();
    let ctx = ctx_for(&tmp, Mode::Autonomous, &controls, &urx, None, &observ::Sink::Noop);
    // Two budgets, one death: max_iters=2 would abort a counted second turn
    // at the third boundary; max_tokens=300k must be the thing that dies.
    // 80k/call keeps the T17 token-kind budget-low notice silent at the
    // second boundary (remaining 140k > the 50k warn threshold), so the read
    // pair the scripted drop targets stays at turns [2,3] behind the
    // boundary-1 iteration-kind notice.
    let mut knobs = knobs_with(2);
    knobs.max_tokens = 300_000;
    let messages = vec![Message::user(vec![ContentBlock::text_block("Goal: tokens")])];

    // Call 1 is a read-only turn (counted: iteration -> 1; the T17 budget-low
    // notice co-fires at the first boundary, landing at index 1). Call 2
    // drops the read pair behind it — an ACCEPTED edit, hence a genuinely
    // free turn (the counter stays at 1) whose usage still accumulates.
    let mut llm = CtxEditorLlm::new(tmp.path(), vec![Edit::None, Edit::DropLast(2)])
        .with_usage(80_000, 80_000)
        .then_goal_complete();
    // notes.txt exists (the read-only turn really reads it).
    std::fs::write(tmp.path().join("notes.txt"), "x".repeat(4_000)).unwrap();

    let mut gate = None;
    let mut messages = messages;
    let mut sink = RecordingSink::default();
    let outcome = drive_loop(
        &ctx,
        &mut knobs,
        &mut llm,
        &mut gate,
        &mut messages,
        Some("check: true".to_string()),
        &mut sink,
        &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
    )
    .unwrap();
    assert!(matches!(outcome, DriveOutcome::RunFinished(1)), "{outcome:?}");
    let aborted = sink.0.iter().any(|e| matches!(
        e,
        Event::Aborted { reason, budget: Some(BudgetExceeded::Tokens { max: 300_000 }), .. } if reason == "token budget exceeded"
    ));
    assert!(
        aborted,
        "the token budget, not the iteration budget, stopped the run\noutcome: {outcome:?}\nctx events: {:?}\ncalls: {}",
        ctx_events(&sink),
        llm.calls.len()
    );
    // Call 1 (read, counted) + call 2 (the free edit): 320k cumulative >=
    // 300k at the third boundary. A third call would mean the free turn's
    // usage (or its freedom) was skipped — the leg's RED mutants both
    // surface here as a reached `goal_complete`.
    assert_eq!(llm.calls.len(), 2);
    // And the edit turn was a genuinely accepted ctx edit (T192 finding 1:
    // the old leg's scripted edits were ALL rejected — no free turn existed
    // to exercise any of the above).
    let events = ctx_events(&sink);
    assert_eq!(events.len(), 1, "{events:?}");
    assert!(events[0].0, "the scripted edit was accepted: {events:?}");
    assert!(events[0].3.is_none(), "no rejection reason: {events:?}");
    assert!(events[0].2 < events[0].1, "strictly smaller: {events:?}");
}

// ---------- rejected edits (requirement 2) ----------

/// A malformed edit is rejected: the transcript is untouched, the model
/// receives a one-line reason, a rejected CtxEdit event lands, and the turn
/// still counts (no free ride on a failed edit).
#[test]
fn rejected_edit_surfaces_reason_and_counts() {
    let tmp = tempfile::tempdir().unwrap();
    let (_utx, urx) = mpsc::channel::<SlashUpdate>();
    let controls = Controls::detached();
    let ctx = ctx_for(&tmp, Mode::Autonomous, &controls, &urx, None, &observ::Sink::Noop);
    let mut knobs = knobs_with(5);
    let messages = vec![Message::user(vec![ContentBlock::text_block("Goal: reject me")])];

    // Call 1 grows the context; call 2 writes garbage into the mirror.
    let mut llm = CtxEditorLlm::new(tmp.path(), vec![Edit::None]).then_read_big_file("notes.txt");
    // Override call 2's edit with garbage by pushing a raw response: the
    // double consumes `drops` first, so pre-load garbage by hand.
    llm.drops.clear();
    llm.responses.clear();
    llm.responses
        .push_back(tool_use_response("read_file", json!({"path": "notes.txt"})));
    llm.responses.push_back(tool_use_response(
        "write_file",
        json!({"path": ".chug/LIVE_CTX.md", "content": "not a mirror at all\n"}),
    ));
    llm.responses
        .push_back(tool_use_response("goal_complete", json!({"summary": "done"})));
    std::fs::write(tmp.path().join("notes.txt"), "y".repeat(4_000)).unwrap();

    let mut gate = None;
    let mut messages = messages;
    let mut sink = RecordingSink::default();
    let outcome = drive_loop(
        &ctx,
        &mut knobs,
        &mut llm,
        &mut gate,
        &mut messages,
        Some("check: true".to_string()),
        &mut sink,
        &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
    )
    .unwrap();
    assert!(matches!(outcome, DriveOutcome::RunFinished(0)), "{outcome:?}");

    // One rejected CtxEdit event with a one-line reason; no marker anywhere.
    let events = ctx_events(&sink);
    assert_eq!(events.len(), 1, "{events:?}");
    assert!(!events[0].0, "rejected");
    let reason = events[0].3.as_deref().expect("a reason");
    assert!(reason.contains("content outside any [[CTX_TURN"), "{reason}");
    assert!(!reason.contains('\n'), "one line: {reason}");
    assert!(ctx_markers(&messages).is_empty());

    // The model saw the reason as its next call's last user message.
    let ( _, seen) = &llm.calls[2];
    let last = seen.last().unwrap();
    let text = last.content[0].text().unwrap_or_default();
    assert!(text.starts_with("[ctx-edit rejected] "), "{text}");
    assert!(text.contains("content outside any [[CTX_TURN"), "{text}");

    // The transcript on disk carries the reason message too (T77 discipline:
    // every injected message is transcript state).
    let on_disk = transcript::load(tmp.path()).unwrap();
    assert!(on_disk.iter().any(|m| {
        m.content[0]
            .text()
            .is_some_and(|t| t.starts_with("[ctx-edit rejected] "))
    }));
}

// ---------- persistence (requirement 3, the T77-pattern test) ----------

/// An accepted edit replaces the transcript segment and leaves a
/// `[ctx-edit:]` marker: a resume re-derives the identical context from
/// `transcript.jsonl` alone.
#[test]
fn accepted_edit_persists_and_resume_rederives_identical_context() {
    let tmp = tempfile::tempdir().unwrap();
    let (_utx, urx) = mpsc::channel::<SlashUpdate>();
    let controls = Controls::detached();
    let ctx = ctx_for(&tmp, Mode::Autonomous, &controls, &urx, None, &observ::Sink::Noop);
    let mut knobs = knobs_with(4);
    let mut messages = vec![Message::user(vec![ContentBlock::text_block(
        "Goal: persist me",
    )])];

    // One read turn (grows), one accepted edit (drops the read pair), then
    // another read turn so the run aborts mid-flight with a mixed list.
    let mut llm = CtxEditorLlm::new(tmp.path(), vec![Edit::None, Edit::DropLast(2), Edit::None])
        .then_read_big_file("notes.txt")
        .then_goal_complete();
    std::fs::write(tmp.path().join("notes.txt"), "z".repeat(4_000)).unwrap();
    // Reorder: read, edit, read, (never reached: max_iters). The T17
    // budget-low notice (remaining 2 <= 8) sits at index 1, so the read pair
    // the edit drops is turns [2,3].
    llm.responses.clear();
    llm.responses
        .push_back(tool_use_response("read_file", json!({"path": "notes.txt"})));
    llm.responses
        .push_back(tool_use_response("read_file", json!({"path": "notes.txt"})));

    let mut gate = None;
    let sink = &mut RecordingSink::default();
    // max_iters = 2: the read (1) + the free edit (still 1) + the counted
    // read (2) → the run aborts at the next boundary with the spliced list
    // on disk.
    knobs.max_iters = 2;
    let outcome = drive_loop(
        &ctx,
        &mut knobs,
        &mut llm,
        &mut gate,
        &mut messages,
        Some("check: true".to_string()),
        sink,
        &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
    )
    .unwrap();
    assert!(matches!(outcome, DriveOutcome::RunFinished(1)), "{outcome:?}");

    // The in-memory list after the splice: one marker standing in for the
    // dropped read pair.
    assert_eq!(ctx_markers(&messages).len(), 1);
    // The on-disk transcript re-derives EXACTLY that list (T77's resume
    // discipline: markers persist INTO transcript.jsonl, nothing hidden).
    let on_disk = transcript::load(tmp.path()).unwrap();
    assert_eq!(on_disk, messages, "resume re-derives the identical context");
    // And the marker text names the deleted run.
    let marker = ctx_markers(&messages)[0].content[0].text().unwrap();
    assert!(marker.contains("2 turns"), "{marker}");
}

// ---------- the occupancy nudge (requirement 5) ----------

/// One user message naming the LIVE_CTX remedy, exactly once, when the
/// pre-call context crosses `--ctx-warn-at-tokens`; silent below and
/// absent when the threshold is 0.
#[test]
fn ctx_warn_nudge_fires_once_at_threshold_only() {
    for (threshold, expect) in [(1u64, true), (u64::MAX, false), (0u64, false)] {
        let tmp = tempfile::tempdir().unwrap();
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let mut ctx = ctx_for(&tmp, Mode::Autonomous, &controls, &urx, None, &observ::Sink::Noop);
        ctx.ctx_warn_at_tokens = threshold;
        let mut knobs = knobs_with(5);
        let mut messages = vec![Message::user(vec![ContentBlock::text_block("Goal: warn me")])];

        // One read turn (grows the context), then goal_complete: two calls,
        // so the one-shot latch is observable across a boundary.
        let mut llm = CtxEditorLlm::new(tmp.path(), vec![]).then_goal_complete();
        llm.responses.clear();
        llm.responses
            .push_back(tool_use_response("read_file", json!({"path": "notes.txt"})));
        llm.responses
            .push_back(tool_use_response("goal_complete", json!({"summary": "done"})));
        std::fs::write(tmp.path().join("notes.txt"), "w".repeat(4_000)).unwrap();

        let mut gate = None;
        let sink = &mut RecordingSink::default();
        drive_loop(
            &ctx,
            &mut knobs,
            &mut llm,
            &mut gate,
            &mut messages,
            Some("check: true".to_string()),
            sink,
            &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
        )
        .unwrap();

        let is_notice = |m: &Message| {
            m.content[0]
                .text()
                .is_some_and(|t| t.starts_with("chug: context occupancy"))
        };
        let in_transcript = messages.iter().filter(|m| is_notice(m)).count();
        assert_eq!(in_transcript, usize::from(expect), "threshold {threshold}");
        if expect {
            // Exactly one, and every call from the first boundary on saw
            // that one (history), never a second. Identity is the PREFIX,
            // not a fixed index: the T13 budget-low notice co-fires with a
            // small max_iters knob (knobs_with(5) -> remaining 5 <= 8) and
            // may precede it, so both position asserts locate by prefix.
            assert!(is_notice(&messages[1]) || messages.iter().any(&is_notice));
            let notice_in_list = messages.iter().find(|m| is_notice(m)).unwrap();
            assert!(notice_in_list.content[0]
                .text()
                .unwrap()
                .starts_with("chug: context occupancy"));
            let count_in_call2 = llm.calls[1].1.iter().filter(|m| is_notice(m)).count();
            assert_eq!(count_in_call2, 1);
            let notice_in_call2 = llm.calls[1].1.iter().find(|m| is_notice(m)).unwrap();
            assert!(notice_in_call2
                .content[0]
                .text()
                .unwrap()
                .contains(".chug/LIVE_CTX.md"));
        }
        let on_disk = transcript::load(tmp.path()).unwrap();
        assert_eq!(
            on_disk.iter().filter(|m| is_notice(m)).count(),
            usize::from(expect),
            "threshold {threshold}: the notice is transcript state"
        );
    }
}

// ---------- events + observability (requirement 6) ----------

/// The T184-class digest: every end-of-turn check lands in events.jsonl as
/// a `ctx_edit` line — accepted edits with the token delta, rejected ones
/// with the reason.
#[test]
fn ctx_edit_events_land_in_events_jsonl() {
    let tmp = tempfile::tempdir().unwrap();
    let (_utx, urx) = mpsc::channel::<SlashUpdate>();
    let controls = Controls::detached();
    let ctx = ctx_for(&tmp, Mode::Autonomous, &controls, &urx, None, &observ::Sink::Noop);
    let mut knobs = knobs_with(5);
    let mut messages = vec![Message::user(vec![ContentBlock::text_block("Goal: log me")])];

    // One accepted edit, then a malformed one, then stop.
    let mut llm = CtxEditorLlm::new(tmp.path(), vec![Edit::None]).then_goal_complete();
    llm.responses.clear();
    llm.responses
        .push_back(tool_use_response("read_file", json!({"path": "notes.txt"})));
    // Tamper with the PINNED turn 0 body: parses fine, fails the
    // byte-identity gate.
    llm.responses.push_back(tool_use_response(
        "write_file",
        json!({"path": ".chug/LIVE_CTX.md", "content":
            "[[CTX_TURN 0 role=user]]\n{\"role\":\"user\",\"content\":[{\"type\":\"text\",\"text\":\"tampered\"}]}\n"}),
    ));
    llm.responses
        .push_back(tool_use_response("goal_complete", json!({"summary": "done"})));
    std::fs::write(tmp.path().join("notes.txt"), "q".repeat(4_000)).unwrap();

    let mut gate = None;
    let sink = &mut RecordingSink::default();
    drive_loop(
        &ctx,
        &mut knobs,
        &mut llm,
        &mut gate,
        &mut messages,
        Some("check: true".to_string()),
        sink,
        &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
    )
    .unwrap();

    let lines = events_jsonl(&tmp);
    let ctx_lines: Vec<&Value> = lines
        .iter()
        .filter(|l| l["type"] == "ctx_edit")
        .collect();
    assert_eq!(ctx_lines.len(), 1, "{ctx_lines:?}");
    // The rejected edit: the reason is the model-visible line, and the
    // candidate tokens ride after_tokens (the context did not change).
    assert_eq!(ctx_lines[0]["accepted"], false);
    assert_eq!(ctx_lines[0]["before_tokens"], ctx_lines[0]["after_tokens"]);
    assert_eq!(
        ctx_lines[0]["reason"],
        "pinned content of turn 0 was modified"
    );
}
