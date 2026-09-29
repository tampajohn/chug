// T104 family: hooks_policy — .chug/hooks.json PreToolUse veto / PostToolUse advisory chain (T83). Moved bytes byte-identical (T84 rule)
// from driver.rs's test module; every test here lives in exactly one family
// file.
    use super::*; // the shared harness (driver::tests) + driver's own imports
    // ---------- T83: .chug/hooks.json (PreToolUse veto + PostToolUse advisory) ----------

    /// THE VETO LEG (kills the allow-by-default mutant): a PreToolUse hook
    /// vetoes a `bash` call that WOULD have created a file — the file does
    /// not exist, the model receives a tool error carrying `[hook veto]` +
    /// the hook's stderr, and the loop stays alive (the next turn proceeds
    /// to completion).
    #[test]
    fn hook_veto_blocks_bash_execution_and_loop_continues() {
        // T151: hold the shared timing domain across the whole body (first
        // acquisition — see crate::testsupport's lock-order rule). Named T151 sighting + siblings asserting hook/bash child outcomes (HOOK_TIMEOUT spawn timing).
        let _timing = crate::testsupport::timing_guard();

        let tmp = tempfile::tempdir().unwrap();
        write_hooks_json(
            tmp.path(),
            json!([hook_entry("bash", "echo vetoing-hook-stderr >&2; exit 2")]),
            json!([]),
        );
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for(&tmp, Mode::Chat, &controls, &urx, None, &observ::Sink::Noop);
        let mut knobs = knobs_with(5);
        let mut llm = ScriptedLlm::new(vec![
            tool_use_response("bash", json!({"command": "echo created > veto-marker.txt"})),
            text_only_response("routed around the veto"),
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
        // The loop is alive: the next turn ran to natural completion.
        assert!(matches!(outcome, DriveOutcome::TurnEnded(TurnEndReason::Completed)), "{outcome:?}");
        // The tool did NOT execute.
        assert!(
            !tmp.path().join("veto-marker.txt").exists(),
            "a vetoed bash call must never execute"
        );
        // The model received a tool error with the veto text.
        let (content, is_error) = tool_result_text(&messages).expect("a tool result exists");
        assert!(is_error, "the veto is a tool error: {content:?}");
        assert!(content.starts_with("[hook veto] "), "{content:?}");
        assert!(content.contains("vetoing-hook-stderr"), "carries the hook stderr: {content:?}");
        // The veto is NOT in the transcript as executed output.
        let transcript = fs::read_to_string(tmp.path().join(".chug/transcript.jsonl")).unwrap();
        assert!(!transcript.contains("created\n"), "no execution output: {transcript:?}");
        // The events log carries the veto fire line.
        let lines = events_jsonl(&tmp);
        let hook_lines: Vec<&Value> = lines.iter().filter(|l| l["type"] == "hook").collect();
        assert_eq!(hook_lines.len(), 1, "{lines:?}");
        assert_eq!(hook_lines[0]["event"], "PreToolUse");
        assert_eq!(hook_lines[0]["tool"], "bash");
        assert_eq!(hook_lines[0]["exit"], 2);
        assert_eq!(hook_lines[0]["veto"], true);
        assert!(hook_lines[0]["duration_ms"].is_u64(), "{:?}", hook_lines[0]);
        assert!(hook_lines[0]["command"].as_str().unwrap().contains("vetoing-hook-stderr"));
    }

    /// CLASS-SWEEP killing test (T83 fix-up, kills the
    /// post-fires-on-vetoed-call mutant — the validator round-1 blocking
    /// finding): with BOTH a PreToolUse vetoing hook AND a PostToolUse hook
    /// configured, a vetoed call never executed, so NO PostToolUse hook
    /// fires — the vetoed result is EXACTLY `[hook veto] <stderr>` (req 2's
    /// shape, no `[hook]` advisory riding it) and events.jsonl carries no
    /// PostToolUse fire line for the call. Revert the driver's `!blocked`
    /// guard and this test fails (a phantom PostToolUse line appears and
    /// the advisory mutates the veto text).
    #[test]
    fn hook_veto_result_is_exact_and_fires_no_post_hook() {
        // T151: hold the shared timing domain across the whole body (first
        // acquisition — see crate::testsupport's lock-order rule). Named T151 sighting + siblings asserting hook/bash child outcomes (HOOK_TIMEOUT spawn timing).
        let _timing = crate::testsupport::timing_guard();

        let tmp = tempfile::tempdir().unwrap();
        write_hooks_json(
            tmp.path(),
            json!([hook_entry("bash", "echo vetoing-hook-stderr >&2; exit 2")]),
            json!([hook_entry("bash", "echo POST-MUST-NOT-FIRE-ON-VETO")]),
        );
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for(&tmp, Mode::Chat, &controls, &urx, None, &observ::Sink::Noop);
        let mut knobs = knobs_with(5);
        let mut llm = ScriptedLlm::new(vec![
            tool_use_response("bash", json!({"command": "echo created > veto-marker.txt"})),
            text_only_response("routed around the veto"),
        ]);
        let mut gate = None;
        let mut messages = Vec::new();
        let mut sink = RecordingSink::default();
        drive_loop(
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
        // The tool did NOT execute.
        assert!(!tmp.path().join("veto-marker.txt").exists());
        // The vetoed result is EXACTLY req 2's shape: `[hook veto] ` + the
        // hook's trimmed stderr — no PostToolUse advisory appended.
        let (content, is_error) = tool_result_text(&messages).expect("a tool result exists");
        assert!(is_error, "{content:?}");
        assert_eq!(content, "[hook veto] vetoing-hook-stderr", "{content:?}");
        // Exactly ONE hook fire line, and it is the PreToolUse veto — no
        // phantom PostToolUse line for a call whose tool never ran.
        let lines = events_jsonl(&tmp);
        let hook_lines: Vec<&Value> = lines.iter().filter(|l| l["type"] == "hook").collect();
        assert_eq!(hook_lines.len(), 1, "no PostToolUse fire on a vetoed call: {lines:?}");
        assert_eq!(hook_lines[0]["event"], "PreToolUse");
        assert_eq!(hook_lines[0]["veto"], true);
    }

    /// CLASS-SWEEP killing test (T83 fix-up, kills the
    /// post-fires-on-gate-blocked-call mutant): a risk-gate-blocked bash
    /// call never executed either (the gate returns the block error before
    /// dispatch), so NO PostToolUse hook fires — the blocked result carries
    /// no `[hook]` advisory and events.jsonl carries no PostToolUse fire
    /// line. Revert the driver's `!blocked` guard and this test fails.
    #[test]
    fn risk_gate_block_result_fires_no_post_hook() {
        let tmp = tempfile::tempdir().unwrap();
        write_hooks_json(
            tmp.path(),
            json!([]),
            json!([hook_entry("bash", "echo POST-MUST-NOT-FIRE-ON-GATE-BLOCK")]),
        );
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for(&tmp, Mode::Chat, &controls, &urx, None, &observ::Sink::Noop);
        let mut knobs = knobs_with(5);
        // A judge that always blocks (p_destructive 0.9 >= threshold).
        let mut gate = Some(RiskGate::new(Box::new(CannedJudge("destructive", 0.9)), tmp.path()));
        let mut llm = ScriptedLlm::new(vec![
            tool_use_response("bash", json!({"command": "echo created > gate-marker.txt"})),
            text_only_response("routed around the block"),
        ]);
        let mut messages = Vec::new();
        drive_loop(
            &ctx,
            &mut knobs,
            &mut llm,
            &mut gate,
            &mut messages,
            None,
            &mut RecordingSink::default(),
            &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
        )
        .unwrap();
        // The tool did NOT execute.
        assert!(!tmp.path().join("gate-marker.txt").exists());
        // The block result is the gate's message — no PostToolUse advisory
        // appended, and the result is still an error.
        let (content, is_error) = tool_result_text(&messages).expect("a tool result exists");
        assert!(is_error, "{content:?}");
        assert!(
            !content.contains("[hook]"),
            "a gate-blocked call never executed; no advisory may ride it: {content:?}"
        );
        // No hook fired at all: the only configured hook is PostToolUse, and
        // a never-executed call must not produce a fire line.
        let lines = events_jsonl(&tmp);
        let hook_lines: Vec<&Value> = lines.iter().filter(|l| l["type"] == "hook").collect();
        assert_eq!(hook_lines.len(), 0, "no PostToolUse fire on a gate-blocked call: {lines:?}");
    }

    /// The allow + advisory legs (kills the post-never-fires and
    /// post-blocks-result mutants): an exit-0 PreToolUse hook lets the tool
    /// execute, and the PostToolUse echo hook's note lands IN the tool
    /// result the model receives, without changing ok/is_error.
    #[test]
    fn hook_allow_executes_tool_and_post_note_lands_in_result() {
        // T151: hold the shared timing domain across the whole body (first
        // acquisition — see crate::testsupport's lock-order rule). Named T151 sighting + siblings asserting hook/bash child outcomes (HOOK_TIMEOUT spawn timing).
        let _timing = crate::testsupport::timing_guard();

        let tmp = tempfile::tempdir().unwrap();
        write_hooks_json(
            tmp.path(),
            json!([hook_entry("bash", "exit 0")]),
            json!([hook_entry("bash", "echo post-note-from-hook")]),
        );
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for(&tmp, Mode::Autonomous, &controls, &urx, None, &observ::Sink::Noop);
        let mut knobs = knobs_with(5);
        let mut llm = ScriptedLlm::new(vec![
            tool_use_response("bash", json!({"command": "echo created > allow-marker.txt"})),
            tool_use_response("goal_complete", json!({"summary": "done"})),
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
        assert!(matches!(outcome, DriveOutcome::RunFinished(0)), "{outcome:?}");
        // The tool executed.
        assert!(tmp.path().join("allow-marker.txt").exists(), "exit-0 hook must allow");
        // The advisory note rides the tool result, ok unchanged. (The bash
        // tool result itself is the `[exit code: n]` shape, not stdout.)
        let (content, is_error) = tool_result_text(&messages).expect("a tool result exists");
        assert!(!is_error);
        assert!(content.contains("\n\n[hook] post-note-from-hook"), "{content:?}");
        // Both fire lines, in order: Pre then Post, neither a veto.
        let lines = events_jsonl(&tmp);
        let hook_lines: Vec<&Value> = lines.iter().filter(|l| l["type"] == "hook").collect();
        assert_eq!(hook_lines.len(), 2, "{lines:?}");
        assert_eq!(hook_lines[0]["event"], "PreToolUse");
        assert_eq!(hook_lines[0]["veto"], false);
        assert_eq!(hook_lines[1]["event"], "PostToolUse");
        assert_eq!(hook_lines[1]["veto"], false);
        assert_eq!(hook_lines[1]["exit"], 0);
    }

    /// The once-per-run leg: a malformed config warns and records exactly
    /// ONE error line even when two tool calls happen — the load happens
    /// once per drive_loop invocation, not per tool call.
    #[test]
    fn malformed_hooks_config_error_line_exactly_once_despite_two_calls() {
        // T151: hold the shared timing domain across the whole body (first
        // acquisition — see crate::testsupport's lock-order rule). Named T151 sighting + siblings asserting hook/bash child outcomes (HOOK_TIMEOUT spawn timing).
        let _timing = crate::testsupport::timing_guard();

        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join(".chug")).unwrap();
        fs::write(hooks::hooks_path(tmp.path()), "{ not json !!!").unwrap();
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for(&tmp, Mode::Chat, &controls, &urx, None, &observ::Sink::Noop);
        let mut knobs = knobs_with(5);
        // Two tool calls in ONE response, then a clean finish.
        let mut llm = ScriptedLlm::new(vec![
            json!({
                "stop_reason": "tool_use",
                "usage": {"input_tokens": 1, "output_tokens": 1},
                "content": [
                    {"type": "tool_use", "id": "tu_1", "name": "bash", "input": {"command": "echo one"}},
                    {"type": "tool_use", "id": "tu_2", "name": "bash", "input": {"command": "echo two"}}
                ]
            }),
            text_only_response("done"),
        ]);
        let mut gate = None;
        let mut messages = Vec::new();
        let mut sink = RecordingSink::default();
        drive_loop(
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
        // Both tools executed (fail-open), and exactly one error line.
        let lines = events_jsonl(&tmp);
        let errors: Vec<&Value> = lines.iter().filter(|l| l["type"] == "hook_error").collect();
        assert_eq!(errors.len(), 1, "{lines:?}");
        assert!(errors[0]["detail"].as_str().unwrap().contains("malformed"));
        assert!(
            !lines.iter().any(|l| l["type"] == "hook"),
            "zero hooks configured → zero fire lines: {lines:?}"
        );
    }

    /// Plan mode NEVER fires hooks: a plan session with a veto-everything
    /// hooks.json present still executes its read-only tools and exits via
    /// submit_plan (the structural exclusion in drive_loop's load site).
    #[test]
    fn plan_mode_fires_no_hooks_even_with_config_present() {
        let tmp = tempfile::tempdir().unwrap();
        write_hooks_json(
            tmp.path(),
            json!([hook_entry("*", "exit 2")]),
            json!([hook_entry("*", "echo plan-post")]),
        );
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for_plan(&tmp, None, &controls, &urx);
        let mut knobs = knobs_with(5);
        let mut llm = ScriptedLlm::new(vec![
            tool_use_response("read_file", json!({"path": "note.txt"})),
            tool_use_response("submit_plan", json!({"plan": "the plan"})),
        ]);
        fs::write(tmp.path().join("note.txt"), "planning input").unwrap();
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
        assert!(matches!(outcome, DriveOutcome::RunFinished(0)), "{outcome:?}");
        // The read_file executed (the veto-everything hook did not fire).
        let (content, is_error) = tool_result_text(&messages).expect("tool results exist");
        assert!(!is_error, "read_file executed in plan mode: {content:?}");
        assert!(content.contains("planning input"), "{content:?}");
        assert!(!content.contains("[hook]"), "no advisory in plan mode: {content:?}");
        let lines = events_jsonl(&tmp);
        assert!(
            !lines.iter().any(|l| l["type"] == "hook" || l["type"] == "hook_error"),
            "plan mode fires NOTHING: {lines:?}"
        );
    }

    /// T140 (codex review finding 1, the PreToolUse veto sibling): a veto of
    /// `goal_complete` must not complete the run either. The veto result is
    /// an error the model routes around — but the goal_summary latch keyed
    /// on the tool NAME alone ignored it, so verification accepted (no
    /// check configured) and the turn ended as goal-accepted. Reverting the
    /// fix flips this red: `GoalAccepted` instead of `Completed`.
    #[test]
    fn vetoed_goal_complete_does_not_complete_the_run() {
        // T151: hold the shared timing domain across the whole body (first
        // acquisition — see crate::testsupport's lock-order rule). Named T151 sighting + siblings asserting hook/bash child outcomes (HOOK_TIMEOUT spawn timing).
        let _timing = crate::testsupport::timing_guard();

        let tmp = tempfile::tempdir().unwrap();
        write_hooks_json(
            tmp.path(),
            json!([hook_entry(
                "goal_complete",
                "echo no-exit-veto >&2; exit 2"
            )]),
            json!([]),
        );
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for(&tmp, Mode::Chat, &controls, &urx, None, &observ::Sink::Noop);
        let mut knobs = knobs_with(5); // check_cmd stays None: the unverified leg
        let mut llm = ScriptedLlm::new(vec![
            tool_use_response("goal_complete", json!({"summary": "claim done"})),
            text_only_response("routed around the veto"),
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
        // The veto keeps the turn alive: the next iteration ran to natural
        // completion — never a goal-accepted turn end.
        assert!(
            matches!(outcome, DriveOutcome::TurnEnded(TurnEndReason::Completed)),
            "{outcome:?}"
        );
        // The model received the veto as a tool error carrying the hook's
        // stderr.
        let (content, is_error) = tool_result_text(&messages).expect("a tool result exists");
        assert!(is_error, "the veto is a tool error: {content:?}");
        assert!(content.starts_with("[hook veto] "), "{content:?}");
        assert!(content.contains("no-exit-veto"), "{content:?}");
        // NO acceptance anywhere.
        assert!(
            !sink.0.iter().any(|e| matches!(e, Event::GoalAccepted { .. })),
            "a vetoed goal_complete must never be accepted: {:?}",
            sink.0
        );
        let lines = events_jsonl(&tmp);
        assert!(
            !lines.iter().any(|l| l["type"] == "goal" && l["outcome"] == "accepted"),
            "no accepted goal on a vetoed exit call: {lines:?}"
        );
        // The veto fire line names the exit tool.
        let hook_lines: Vec<&Value> = lines.iter().filter(|l| l["type"] == "hook").collect();
        assert_eq!(hook_lines.len(), 1, "{lines:?}");
        assert_eq!(hook_lines[0]["tool"], "goal_complete");
        assert_eq!(hook_lines[0]["veto"], true);
    }

    /// T140 POSITIVE CONTROL (kills the block-everything mutant): the veto
    /// is stateful — the payload on stdin carries the tool input, so
    /// `grep -q premature` rejects only the PREMATURE exit claim and allows
    /// the honest one. The block keeps the loop running (real work
    /// executes) and the later honest claim completes the run with ITS
    /// summary. With the name-only latch reverted, the premature claim is
    /// accepted immediately: the outcome lands before the bash call ever
    /// runs and the accepted summary is "premature claim" — red.
    #[test]
    fn vetoed_premature_goal_complete_loops_until_honest_claim() {
        // T151: hold the shared timing domain across the whole body (first
        // acquisition — see crate::testsupport's lock-order rule). Named T151 sighting + siblings asserting hook/bash child outcomes (HOOK_TIMEOUT spawn timing).
        let _timing = crate::testsupport::timing_guard();

        let tmp = tempfile::tempdir().unwrap();
        write_hooks_json(
            tmp.path(),
            json!([hook_entry(
                "goal_complete",
                "grep -q premature && { echo premature-exit-veto >&2; exit 2; } || exit 0"
            )]),
            json!([]),
        );
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for(
            &tmp,
            Mode::Autonomous,
            &controls,
            &urx,
            None,
            &observ::Sink::Noop,
        );
        let mut knobs = knobs_with(5); // no check: acceptance is unverified
        let mut llm = ScriptedLlm::new(vec![
            tool_use_response("goal_complete", json!({"summary": "premature claim"})),
            tool_use_response(
                "bash",
                json!({"command": "echo real-work > honest-work.txt"}),
            ),
            tool_use_response("goal_complete", json!({"summary": "honest claim"})),
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
        // The run completed — via the HONEST claim.
        assert!(matches!(outcome, DriveOutcome::RunFinished(0)), "{outcome:?}");
        // The intermediate work actually ran: the premature claim did not
        // end the run before iteration 2.
        assert!(
            tmp.path().join("honest-work.txt").exists(),
            "the veto must keep the loop running (real work executes)"
        );
        // The accepted summary is the honest one — the premature claim was
        // never accepted.
        let lines = events_jsonl(&tmp);
        let accepted: Vec<&Value> = lines
            .iter()
            .filter(|l| l["type"] == "goal" && l["outcome"] == "accepted")
            .collect();
        assert_eq!(accepted.len(), 1, "{lines:?}");
        assert_eq!(accepted[0]["summary"], "honest claim", "{lines:?}");
        // Both claims fired the hook (every matching hook fires even after
        // one vetoes); exactly one of the two was a veto.
        let hook_lines: Vec<&Value> = lines.iter().filter(|l| l["type"] == "hook").collect();
        assert_eq!(hook_lines.len(), 2, "{lines:?}");
        let vetoes: Vec<_> = hook_lines.iter().filter(|l| l["veto"] == true).collect();
        assert_eq!(vetoes.len(), 1, "{lines:?}");
    }

