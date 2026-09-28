// T104 family: budget — budget-low warning + token budgets (T13/T15/T17/T18). Moved bytes byte-identical (T84 rule)
// from driver.rs's test module; every test here lives in exactly one family
// file.
    use super::*; // the shared harness (driver::tests) + driver's own imports
    // ---------- T13: budget-low warning before abort ----------

    #[test]
    fn budget_low_notice_iteration_boundary() {
        // 9 remaining is above the threshold; 8 fires (T18: WARN_REMAINING_ITERS = 8).
        assert_eq!(budget_low_notice(9, u64::MAX, None, false, false, false), None);
        assert!(budget_low_notice(8, u64::MAX, None, false, false, false).is_some());
    }

    #[test]
    fn budget_low_notice_time_boundary() {
        // One second above the 5-minute threshold stays silent; at the
        // threshold the time half fires on its own (iters far from low).
        assert_eq!(
            budget_low_notice(u32::MAX, WARN_REMAINING_SECS + 1, None, false, false, false),
            None
        );
        assert!(
            budget_low_notice(u32::MAX, WARN_REMAINING_SECS, None, false, false, false).is_some()
        );
    }

    #[test]
    fn budget_low_notice_one_shot_flags_suppress_repeats() {
        // Both kinds latched: silent forever after, even deep in the low zone.
        assert_eq!(budget_low_notice(1, 30, None, true, true, false), None);
        // A latched kind never re-fires; the other still gets its one shot.
        assert!(budget_low_notice(1, 30, None, true, false, false).is_some());
        assert!(budget_low_notice(1, 30, None, false, true, false).is_some());
    }

    #[test]
    fn budget_low_notice_interpolates_actual_counts() {
        let msg = budget_low_notice(3, 150, None, false, false, false)
            .expect("fires below both thresholds");
        assert!(msg.contains("3 iteration(s)"), "{msg}");
        assert!(msg.contains("2 minute(s)"), "{msg}");
        assert!(msg.contains("commit what is done"), "{msg}");
        // The counts are the ones at fire time, not the thresholds.
        let msg = budget_low_notice(1, 60, None, false, true, false).expect("iter half still armed");
        assert!(msg.contains("1 iteration(s)"), "{msg}");
        assert!(msg.contains("1 minute(s)"), "{msg}");
    }

    /// T13, scripted run with an 8-iteration budget and WARN=8 (T18): the
    /// whole budget is warn-zone from the very first boundary, so exactly one
    /// budget-low user message — naming the 8 iterations that remain — goes
    /// out before the first call, and never a second one. The run itself
    /// ends exactly as before (accepted goal, exit 0).
    #[test]
    fn budget_low_warning_fires_once_at_threshold() {
        let tmp = tempfile::tempdir().unwrap();
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for(&tmp, Mode::Autonomous, &controls, &urx, None, &observ::Sink::Noop);
        let mut knobs = knobs_with(8);
        let mut responses = vec![text_only_response("working"); 7];
        responses.push(tool_use_response("goal_complete", json!({"summary": "wrapped up"})));
        let mut llm = ScriptedLlm::new(responses);
        let mut gate = None;
        let mut messages = Vec::new();
        let outcome = drive_loop(
            &ctx,
            &mut knobs,
            &mut llm,
            &mut gate,
            &mut messages,
            Some("check: true".to_string()),
            &mut RecordingSink::default(),
            &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
        )
        .unwrap();
        assert!(matches!(outcome, DriveOutcome::RunFinished(0)));

        let is_notice = |m: &Message| {
            m.role == "user"
                && m.content
                    .iter()
                    .any(|b| b.text().is_some_and(|t| t.starts_with("chug: budget low")))
        };

        // Exactly one notice in the transcript the driver holds…
        let notices: Vec<&Message> = messages.iter().filter(|m| is_notice(m)).collect();
        assert_eq!(notices.len(), 1, "exactly one budget-low warning");
        let text = notices[0].content[0].text().expect("notice is text");
        assert!(text.contains("8 iteration(s)"), "{text}");
        assert!(text.contains("commit what is done"), "{text}");
        // …and exactly one on disk.
        let on_disk = transcript::load(tmp.path()).unwrap();
        assert_eq!(on_disk.iter().filter(|m| is_notice(m)).count(), 1);

        // The model sees the single notice from the 1st call on (remaining
        // == 8: with max_iters == WARN the whole budget is warn-zone at the
        // first boundary) and no call ever sees more than that one message:
        // later calls still carry it as conversation history, never a second.
        assert_eq!(llm.calls.len(), 8);
        for (i, (_, seen)) in llm.calls.iter().enumerate() {
            let count = seen.iter().filter(|m| is_notice(m)).count();
            assert_eq!(
                count, 1,
                "call {} (1-based) carries {count} notice(s), expected the one",
                i + 1
            );
        }
    }

    /// T17: the injection lands in `.chug/events.jsonl` as exactly one
    /// `budget_low` line — the remaining counts at fire time (T18: with an
    /// 8-iteration budget and WARN=8 the iteration leg fires on the first
    /// boundary, with 8 remaining) and `remaining_tokens` null when no token
    /// budget is configured — recorded before the run ends.
    #[test]
    fn budget_low_injection_lands_in_events_jsonl() {
        let tmp = tempfile::tempdir().unwrap();
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for(&tmp, Mode::Autonomous, &controls, &urx, None, &observ::Sink::Noop);
        let mut knobs = knobs_with(8);
        let mut responses = vec![text_only_response("working"); 7];
        responses.push(tool_use_response("goal_complete", json!({"summary": "wrapped up"})));
        let mut llm = ScriptedLlm::new(responses);
        let mut gate = None;
        let mut messages = Vec::new();
        let outcome = drive_loop(
            &ctx,
            &mut knobs,
            &mut llm,
            &mut gate,
            &mut messages,
            Some("check: true".to_string()),
            &mut RecordingSink::default(),
            &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
        )
        .unwrap();
        assert!(matches!(outcome, DriveOutcome::RunFinished(0)));

        let lines = events_jsonl(&tmp);
        let lows: Vec<&Value> = lines
            .iter()
            .filter(|l| l["type"] == "budget_low")
            .collect();
        assert_eq!(lows.len(), 1, "one injection → one budget_low line: {lines:?}");
        let remaining = lows[0]["remaining_iters"].as_u64().unwrap();
        assert!(
            remaining <= u64::from(WARN_REMAINING_ITERS),
            "remaining at fire time is in the warn zone, got {remaining}"
        );
        assert_eq!(remaining, 8, "the fire-time count, not the threshold");
        assert!(
            lows[0]["remaining_tokens"].is_null(),
            "no token budget → null, never a phantom number"
        );
        // Telemetry of a mid-run injection: before the terminal goal line.
        let low_idx = lines
            .iter()
            .position(|l| l["type"] == "budget_low")
            .unwrap();
        let goal_idx = lines.iter().position(|l| l["type"] == "goal").unwrap();
        assert!(low_idx < goal_idx);
    }

    /// T17 control: a run that never approaches any budget writes no
    /// `budget_low` line at all.
    #[test]
    fn no_budget_low_line_when_far_from_budget() {
        let tmp = tempfile::tempdir().unwrap();
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for(&tmp, Mode::Autonomous, &controls, &urx, None, &observ::Sink::Noop);
        let mut knobs = knobs_with(50);
        let mut llm = ScriptedLlm::new(vec![
            text_only_response("working"),
            tool_use_response("goal_complete", json!({"summary": "wrapped up"})),
        ]);
        let mut gate = None;
        let mut messages = Vec::new();
        let outcome = drive_loop(
            &ctx,
            &mut knobs,
            &mut llm,
            &mut gate,
            &mut messages,
            Some("check: true".to_string()),
            &mut RecordingSink::default(),
            &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
        )
        .unwrap();
        assert!(matches!(outcome, DriveOutcome::RunFinished(0)));

        let lines = events_jsonl(&tmp);
        assert!(
            lines.iter().all(|l| l["type"] != "budget_low"),
            "no budget_low when budgets stay far away: {lines:?}"
        );
        // And no notice reached the transcript either.
        let on_disk = transcript::load(tmp.path()).unwrap();
        assert!(!on_disk.iter().any(|m| m.content.iter().any(|b| b
            .text()
            .is_some_and(|t| t.starts_with("chug: budget low")))));
    }

    // ---------- T15: token-denominated budget ----------

    #[test]
    fn budget_low_notice_tokens_boundary() {
        // One token above the threshold stays silent; at the threshold the
        // tokens leg fires on its own (iters/time far from low).
        assert_eq!(
            budget_low_notice(
                u32::MAX,
                u64::MAX,
                Some(WARN_REMAINING_TOKENS + 1),
                false,
                false,
                false
            ),
            None
        );
        assert!(
            budget_low_notice(u32::MAX, u64::MAX, Some(WARN_REMAINING_TOKENS), false, false, false)
                .is_some()
        );
    }

    #[test]
    fn budget_low_notice_tokens_none_means_unlimited() {
        // No token budget: the tokens leg never fires…
        assert_eq!(budget_low_notice(u32::MAX, u64::MAX, None, false, false, false), None);
        // …and the message never mentions tokens, even when another leg fires.
        let msg = budget_low_notice(3, 150, None, false, false, false).unwrap();
        assert!(!msg.contains("token"), "{msg}");
    }

    #[test]
    fn budget_low_notice_tokens_one_shot_latch() {
        // A latched tokens leg stays silent even deeper in the low zone…
        assert_eq!(budget_low_notice(u32::MAX, u64::MAX, Some(1), false, false, true), None);
        // …while the other legs still get their one shot (and vice versa).
        assert!(
            budget_low_notice(1, 30, Some(1), true, true, false).is_some(),
            "iters/time legs armed"
        );
        assert!(
            budget_low_notice(u32::MAX, u64::MAX, Some(1), false, false, false).is_some(),
            "tokens leg armed"
        );
    }

    #[test]
    fn budget_low_notice_tokens_interpolates_remaining() {
        let msg = budget_low_notice(u32::MAX, u64::MAX, Some(12_345), false, false, false)
            .expect("tokens leg fires");
        assert!(msg.contains("12345 token(s)"), "{msg}");
        assert!(msg.contains("commit what is done"), "{msg}");
        // The message names the remaining counts of every budget kind at
        // fire time, not the thresholds.
        let msg = budget_low_notice(2, 60, Some(100), false, false, false)
            .expect("tokens leg fires with iters low too");
        assert!(msg.contains("2 iteration(s)"), "{msg}");
        assert!(msg.contains("1 minute(s)"), "{msg}");
        assert!(msg.contains("100 token(s)"), "{msg}");
    }

    /// T15, scripted run: a tiny token budget aborts at the top of the
    /// iteration where cumulative usage (input+output) crosses it, naming
    /// the exhausted budget; the loop stops with script responses left.
    #[test]
    fn token_budget_aborts_when_cumulative_usage_crosses_max() {
        let tmp = tempfile::tempdir().unwrap();
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for(&tmp, Mode::Autonomous, &controls, &urx, None, &observ::Sink::Noop);
        // --max-tokens 25 against responses costing 10 in + 5 out = 15 each:
        // after two responses the cumulative 30 has crossed 25.
        let mut knobs = knobs_with_tokens(50, 25);
        let mut llm = ScriptedLlm::new(vec![
            text_only_response("working"),
            text_only_response("working"),
            text_only_response("never reached"),
        ]);
        let mut gate = None;
        let mut messages = Vec::new();
        let mut sink = RecordingSink::default();
        let outcome = drive_loop(
            &ctx,
            &mut knobs,
            &mut llm,
            &mut gate,
            &mut messages,
            None,
            &mut sink,
            &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
        )
        .unwrap();
        assert!(matches!(outcome, DriveOutcome::RunFinished(1)));
        // The loop stopped at the boundary with a scripted response unused.
        assert_eq!(llm.calls.len(), 2);
        let abort = sink
            .0
            .iter()
            .find_map(|e| match e {
                Event::Aborted { reason, budget, .. } => Some((reason.clone(), *budget)),
                _ => None,
            })
            .expect("abort event emitted");
        assert_eq!(abort.0, "token budget exceeded");
        assert_eq!(
            abort.1,
            Some(BudgetExceeded::Tokens { max: 25 }),
            "the exhausted token budget is named"
        );
        // The events log's abort line picks the new variant up unchanged.
        let lines = events_jsonl(&tmp);
        assert_eq!(lines.last().unwrap()["type"], "abort");
        assert_eq!(lines.last().unwrap()["reason"], "token budget exceeded");
        assert_eq!(lines.last().unwrap()["budget_kind"], "tokens");
        assert_eq!(lines.last().unwrap()["budget_max"], 25);
    }

    /// T15 control: the same shape of run with the knob unset (`0` =
    /// unlimited) completes naturally no matter how many tokens it burns —
    /// pre-T15 behavior exactly (no abort, no token warning leg).
    #[test]
    fn no_token_budget_runs_to_natural_completion() {
        let tmp = tempfile::tempdir().unwrap();
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for(&tmp, Mode::Autonomous, &controls, &urx, None, &observ::Sink::Noop);
        let mut knobs = knobs_with_tokens(50, 0);
        let mut llm = ScriptedLlm::new(vec![
            big_usage_text_response("working", (20_000, 5_000)),
            big_usage_text_response("working", (20_000, 5_000)),
            big_usage_text_response("working", (20_000, 5_000)),
            tool_use_response("goal_complete", json!({"summary": "wrapped up"})),
        ]);
        let mut gate = None;
        let mut messages = Vec::new();
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
        assert!(matches!(outcome, DriveOutcome::RunFinished(0)));
        assert!(!sink.0.iter().any(|e| matches!(e, Event::Aborted { .. })));
        // 60,010 cumulative input tokens burned with no ceiling, no warning leg.
        assert!(sink.0.iter().any(|e| matches!(
            e,
            Event::Usage {
                input: 60_010,
                ..
            }
        )));
        let on_disk = transcript::load(tmp.path()).unwrap();
        assert!(!on_disk.iter().any(|m| m.content.iter().any(
            |b| b.text().is_some_and(|t| t.starts_with("chug: budget low"))
        )));
    }

    /// T15, scripted run with a 120k-token budget and 25k-token responses:
    /// exactly one budget-low user message naming the remaining tokens, first
    /// seen by the model on the call after remaining drops to 45k, never a
    /// second one. The run itself ends exactly as before (accepted goal).
    #[test]
    fn token_budget_low_warning_fires_once_mid_run() {
        let tmp = tempfile::tempdir().unwrap();
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for(&tmp, Mode::Autonomous, &controls, &urx, None, &observ::Sink::Noop);
        let mut knobs = knobs_with_tokens(50, 120_000);
        let mut llm = ScriptedLlm::new(vec![
            big_usage_text_response("working", (20_000, 5_000)),
            big_usage_text_response("working", (20_000, 5_000)),
            big_usage_text_response("working", (20_000, 5_000)),
            tool_use_response("goal_complete", json!({"summary": "wrapped up"})),
        ]);
        let mut gate = None;
        let mut messages = Vec::new();
        let outcome = drive_loop(
            &ctx,
            &mut knobs,
            &mut llm,
            &mut gate,
            &mut messages,
            Some("check: true".to_string()),
            &mut RecordingSink::default(),
            &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
        )
        .unwrap();
        assert!(matches!(outcome, DriveOutcome::RunFinished(0)));

        let is_notice = |m: &Message| {
            m.role == "user"
                && m.content
                    .iter()
                    .any(|b| b.text().is_some_and(|t| t.starts_with("chug: budget low")))
        };

        // Exactly one notice, naming the 45k tokens remaining at fire time.
        let notices: Vec<&Message> = messages.iter().filter(|m| is_notice(m)).collect();
        assert_eq!(notices.len(), 1, "exactly one token budget-low warning");
        let text = notices[0].content[0].text().expect("notice is text");
        assert!(text.contains("45000 token(s)"), "{text}");
        assert!(text.contains("commit what is done"), "{text}");
        // The model first sees it on the 4th call (remaining crossed the 50k
        // threshold after the third response) and no call ever sees more
        // than that single message.
        assert_eq!(llm.calls.len(), 4);
        for (i, (_, seen)) in llm.calls.iter().enumerate() {
            let count = seen.iter().filter(|m| is_notice(m)).count();
            assert!(
                count == usize::from(i >= 3),
                "call {} (1-based) carries {count} notice(s)",
                i + 1
            );
        }
    }

    /// T15: a text-only response with an explicit (input, output) usage, so
    /// scripted runs can move the cumulative token counters in big steps.
    fn big_usage_text_response(text: &str, usage: (u64, u64)) -> Value {
        json!({
            "stop_reason": "end_turn",
            "usage": {"input_tokens": usage.0, "output_tokens": usage.1},
            "content": [{"type": "text", "text": text}],
        })
    }

    /// T15: knobs with a token budget (`0` = unlimited, like [`knobs_with`]).
    fn knobs_with_tokens(max_iters: u32, max_tokens: u64) -> TurnKnobs {
        TurnKnobs {
            max_tokens,
            ..knobs_with(max_iters)
        }
    }

