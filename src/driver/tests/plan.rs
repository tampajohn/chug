// T104 family: plan — T73 plan mode: advertised list, exclusions, real run_plan_loop legs, submit_plan. Moved bytes byte-identical (T84 rule)
// from driver.rs's test module; every test here lives in exactly one family
// file.
    use super::*; // the shared harness (driver::tests) + driver's own imports
    // ===================== T73: plan mode =====================

    /// Schema-filter pin: the tool list a plan-mode run sends to the API is
    /// EXACTLY the five names (set compare with exact cardinality — a sixth
    /// added or one dropped turns this RED).
    #[test]
    fn plan_mode_advertises_exactly_the_five_tool_schemas() {
        let tmp = tempfile::tempdir().unwrap();
        let out = tmp.path().join("plan.md");
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for_plan(&tmp, Some(&out), &controls, &urx);
        let mut knobs = knobs_with(5);
        let mut llm = ToolRecordingLlm::new(vec![tool_use_response(
            "submit_plan",
            json!({"plan": "# Plan\n\n- step one\n- step two\n"}),
        )]);
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
            &mut McpRegistry::new(ctx.cwd, true, None).unwrap(),
        )
        .unwrap();
        assert!(matches!(outcome, DriveOutcome::RunFinished(0)), "{outcome:?}");
        let mut names: Vec<String> = llm.recorded_tools[0]
            .iter()
            .filter_map(|t| t.get("name").and_then(Value::as_str))
            .map(String::from)
            .collect();
        names.sort();
        assert_eq!(names.len(), 5, "exact cardinality five: {names:?}");
        assert_eq!(
            names,
            vec!["glob", "grep", "list_dir", "read_file", "submit_plan"],
            "the plan surface is exactly the five read-only tools + submit_plan"
        );
    }

    /// Rejection sweep through the LOOP (T72 sweep-the-family doctrine): one
    /// leg per excluded registered tool. Scripted tool_use of the excluded
    /// name in plan mode → tool error naming the allowed set; the loop
    /// CONTINUES (the scripted follow-up submit_plan runs); the tool never
    /// executed (per-leg side-effect pin).
    #[test]
    fn plan_mode_rejects_every_excluded_tool_and_the_loop_continues() {
        let excluded = [
            "write_file",
            "edit_file",
            "bash",
            "delegate",
            "web_fetch",
            "update_ledger",
            "goal_complete",
            "decision_log",
        ];
        let plan_text = "# Plan\n\nthe plan body\n";
        for name in excluded {
            let tmp = tempfile::tempdir().unwrap();
            // edit_file leg: a target the scripted edit would change.
            fs::write(tmp.path().join("target.txt"), "original").unwrap();
            // delegate leg: a would-be child dir whose delegate.log proves
            // whether a child was spawned.
            let child_dir = tempfile::tempdir().unwrap();
            let input = match name {
                "write_file" => json!({"path": "escape.md", "content": "mutated"}),
                "edit_file" => json!({"path": "target.txt", "old": "original", "new": "mutated"}),
                "bash" => json!({"command": "touch pwned-by-bash.txt"}),
                "delegate" => json!({
                    "action": "launch",
                    "cwd": child_dir.path().display().to_string(),
                    "spec": child_dir.path().join("s.md").display().to_string(),
                    "goal": "g",
                    "model": "m"
                }),
                "web_fetch" => json!({"url": "http://127.0.0.1:1/x"}),
                "update_ledger" => json!({"content": "MUTATED LEDGER"}),
                "goal_complete" => json!({"summary": "claim done"}),
                "decision_log" => json!({
                    "class": "outcome", "subject": "T73", "inputs": "i",
                    "options": "o", "choice": "landed-clean", "confidence": 0.5
                }),
                other => unreachable!("{other}"),
            };
            let out = tmp.path().join("plan.md");
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for_plan(&tmp, Some(&out), &controls, &urx);
            let mut knobs = knobs_with(5);
            let mut llm = ScriptedLlm::new(vec![
                tool_use_response(name, input),
                tool_use_response("submit_plan", json!({"plan": plan_text})),
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
                &mut McpRegistry::new(ctx.cwd, true, None).unwrap(),
            )
            .unwrap();
            assert!(
                matches!(outcome, DriveOutcome::RunFinished(0)),
                "{name}: the loop must continue past the rejection and exit on submit_plan, got {outcome:?}"
            );
            // The loop continued: BOTH scripted calls were consumed.
            assert_eq!(llm.calls.len(), 2, "{name}: two model calls expected");
            // The rejection error reached the transcript, naming the allowed set.
            let transcript = fs::read_to_string(tmp.path().join(".chug/transcript.jsonl")).unwrap();
            assert!(
                transcript.contains("plan mode") && transcript.contains("submit_plan"),
                "{name}: transcript must carry the plan-gate rejection naming the allowed set: {transcript}"
            );
            // The plan still landed (the follow-up submit_plan executed).
            assert_eq!(
                fs::read_to_string(tmp.path().join("plan.md")).unwrap(),
                plan_text,
                "{name}: leg must not disturb the plan exit"
            );
            // Per-leg side-effect pins: the excluded tool never executed.
            match name {
                "write_file" => assert!(!tmp.path().join("escape.md").exists()),
                "edit_file" => assert_eq!(
                    fs::read_to_string(tmp.path().join("target.txt")).unwrap(),
                    "original"
                ),
                "bash" => assert!(!tmp.path().join("pwned-by-bash.txt").exists()),
                "delegate" => assert!(!child_dir.path().join(".chug/delegate.log").exists()),
                "web_fetch" => {} // the plan-gate message assert above is the leg
                "update_ledger" => assert!(!tmp.path().join("LEDGER.md").exists()),
                // goal_complete: rejected as a TOOL, not honored as the exit —
                // the second scripted call proves the loop moved past it.
                "goal_complete" => {}
                "decision_log" => {
                    assert!(!tmp.path().join(".chug/decisions.jsonl").exists())
                }
                other => unreachable!("{other}"),
            }
        }
    }

    /// Helpers for the LOOP-level plan tests: these drive the REAL
    /// `run_plan_loop` (driver lock, fresh rotation, the loop's own
    /// `run_start` event, budget enforcement, the submit_plan exit) with a
    /// scripted `&mut dyn Llm` — the run_turn seam pattern — so the plan
    /// startup/exit guarantees are pinned on the lines the loop actually
    /// executes, not on hand-written mirrors.
    fn plan_cfg(tmp: &tempfile::TempDir, out: Option<PathBuf>, max_iters: u32) -> PlanConfig {
        PlanConfig {
            cwd: tmp.path().to_path_buf(),
            spec_path: None,
            goal: "draft a plan".to_string(),
            model: "scripted-model".to_string(),
            max_iters,
            max_minutes: 20,
            max_tokens: 0,
            out_path: out,
        }
    }

    fn empty_plan_registry(tmp: &tempfile::TempDir) -> McpRegistry {
        // The prod shape: plan mode never initializes MCP servers.
        McpRegistry::new(tmp.path(), true, None).unwrap()
    }

    fn events_lines(cwd: &Path) -> Vec<Value> {
        fs::read_to_string(cwd.join(".chug/events.jsonl"))
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    /// (a) The run_start event the REAL loop emits carries mode "plan" —
    /// read from the events.jsonl the loop itself wrote. The pre-fix tests
    /// hand-wrote this line, so a mode drift inside run_plan_loop was
    /// invisible; here the loop's own line is the pin.
    #[test]
    fn plan_loop_run_start_event_names_the_plan_mode() {
        let tmp = tempfile::tempdir().unwrap();
        let mut llm = ScriptedLlm::new(vec![tool_use_response(
            "submit_plan",
            json!({"plan": "# Plan\n\nloop-level run_start\n"}),
        )]);
        let mut sink = RecordingSink::default();
        let code = run_plan_loop(
            plan_cfg(&tmp, None, 5),
            &mut llm,
            &mut sink,
            &observ::Sink::Noop,
            empty_plan_registry(&tmp),
        )
        .unwrap();
        assert_eq!(code, 0);
        let lines = events_lines(tmp.path());
        assert_eq!(lines[0]["type"], "run_start", "{lines:?}");
        assert_eq!(
            lines[0]["mode"], "plan",
            "the loop's own run_start must name mode \"plan\""
        );
    }

    /// (b, ensure_seeded mutant) A plan loop run in a cwd with NO ledger
    /// leaves it absent: plan mode never seeds LEDGER.md (the run path's
    /// `ledger::ensure_seeded` is deliberately not in the plan startup).
    #[test]
    fn plan_loop_run_leaves_an_absent_ledger_absent() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(!tmp.path().join("LEDGER.md").exists());
        let mut llm = ScriptedLlm::new(vec![tool_use_response(
            "submit_plan",
            json!({"plan": "# Plan\n\nno ledger writes\n"}),
        )]);
        let mut sink = RecordingSink::default();
        let code = run_plan_loop(
            plan_cfg(&tmp, None, 5),
            &mut llm,
            &mut sink,
            &observ::Sink::Noop,
            empty_plan_registry(&tmp),
        )
        .unwrap();
        assert_eq!(code, 0);
        assert!(
            !tmp.path().join("LEDGER.md").exists(),
            "a plan run must never seed LEDGER.md"
        );
    }

    /// (b, archive_stale mutant) A plan loop run leaves a PRE-EXISTING
    /// ledger and TODO.md byte-untouched: no archive rotation into .chug,
    /// no seeding, no rewrite. The run path archives any non-seed ledger at
    /// startup; plan mode must not.
    #[test]
    fn plan_loop_run_leaves_preexisting_ledger_and_todo_bytes_untouched() {
        let tmp = tempfile::tempdir().unwrap();
        let ledger = "# Ledger\n\n## Done\n- custom content\n";
        let todo = "# TODO\n\n| T1 | something | pending |\n";
        fs::write(tmp.path().join("LEDGER.md"), ledger).unwrap();
        fs::write(tmp.path().join("TODO.md"), todo).unwrap();
        let mut llm = ScriptedLlm::new(vec![tool_use_response(
            "submit_plan",
            json!({"plan": "# Plan\n\nread-only bookkeeping\n"}),
        )]);
        let mut sink = RecordingSink::default();
        let code = run_plan_loop(
            plan_cfg(&tmp, None, 5),
            &mut llm,
            &mut sink,
            &observ::Sink::Noop,
            empty_plan_registry(&tmp),
        )
        .unwrap();
        assert_eq!(code, 0);
        assert_eq!(
            fs::read_to_string(tmp.path().join("LEDGER.md")).unwrap(),
            ledger,
            "a plan run must never touch LEDGER.md"
        );
        assert_eq!(
            fs::read_to_string(tmp.path().join("TODO.md")).unwrap(),
            todo,
            "a plan run must never touch TODO.md"
        );
        let archived: Vec<String> = fs::read_dir(tmp.path().join(".chug"))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with("LEDGER"))
            .collect();
        assert!(
            archived.is_empty(),
            "a plan run must never archive the ledger: {archived:?}"
        );
    }

    /// (c) The tool list the REAL loop sends to the API is exactly the five
    /// plan tools EVEN WITH a live non-empty MCP registry present — the
    /// plan branch must never extend the advertised list with mcp schemas
    /// (an empty prod registry would make that extension invisible).
    #[test]
    fn plan_loop_advertises_exactly_five_tools_with_a_live_mcp_registry() {
        let tmp = tempfile::tempdir().unwrap();
        write_echo_server(tmp.path());
        let mcp = McpRegistry::new(tmp.path(), false, None).expect("live fake registry");
        assert!(
            !mcp.tool_schemas().is_empty(),
            "leg premise: the registry must be non-empty (fake server must be up)"
        );
        let mut llm = ToolRecordingLlm::new(vec![tool_use_response(
            "submit_plan",
            json!({"plan": "# Plan\n\nfive tools only\n"}),
        )]);
        let mut sink = RecordingSink::default();
        let code = run_plan_loop(
            plan_cfg(&tmp, None, 5),
            &mut llm,
            &mut sink,
            &observ::Sink::Noop,
            mcp,
        )
        .unwrap();
        assert_eq!(code, 0);
        let mut names: Vec<String> = llm.recorded_tools[0]
            .iter()
            .filter_map(|t| t.get("name").and_then(Value::as_str))
            .map(String::from)
            .collect();
        names.sort();
        assert_eq!(
            names,
            vec!["glob", "grep", "list_dir", "read_file", "submit_plan"],
            "the live MCP schemas must never reach a plan-mode API call: {names:?}"
        );
    }

    /// (d) A REJECTED submit_plan keeps the loop UP — no goal/accepted
    /// event, no exit 0, the conversation continues to the next model call.
    /// Two legs: an empty plan, and a sandbox-escaping --out. Killing the
    /// is_error-guard mutant: without the guard a rejected submit_plan
    /// latches, exits 0, and records a goal/accepted with a bogus summary.
    #[test]
    fn plan_loop_rejected_submit_plan_keeps_the_loop_up() {
        let plan_text = "# Plan\n\nthe real plan\n";
        let legs: Vec<(&str, Value, Option<PathBuf>, String)> = vec![
            (
                "empty plan",
                json!({"plan": ""}),
                None,
                "must not be empty".to_string(),
            ),
            (
                "sandbox escape",
                json!({"plan": plan_text}),
                Some(PathBuf::from("../plan-escape.md")),
                "escapes the cwd sandbox".to_string(),
            ),
        ];
        for (leg, input, out, err_needle) in legs {
            let tmp = tempfile::tempdir().unwrap();
            let mut llm = ScriptedLlm::new(vec![
                tool_use_response("submit_plan", input),
                // The conversation continues: the model gets the tool error
                // and answers again.
                text_only_response("let me fix that"),
            ]);
            let mut sink = RecordingSink::default();
            let code = run_plan_loop(
                plan_cfg(&tmp, out.clone(), 2),
                &mut llm,
                &mut sink,
                &observ::Sink::Noop,
                empty_plan_registry(&tmp),
            )
            .unwrap();
            // NOT exit 0: with the rejection, the run dies on the iteration
            // budget via the existing abort path.
            assert_ne!(
                code, 0,
                "{leg}: a rejected submit_plan must not exit 0 (got {code})"
            );
            // The conversation continued past the rejection: the model was
            // called again after the tool error.
            assert_eq!(llm.calls.len(), 2, "{leg}: the loop must continue");
            // The rejection reached the transcript with its specific wording.
            let transcript =
                fs::read_to_string(tmp.path().join(".chug/transcript.jsonl")).unwrap();
            assert!(
                transcript.contains(&err_needle),
                "{leg}: rejection must reach the transcript: {err_needle}"
            );
            // The events stream has NO goal/accepted — the rejection is not
            // a completion — and does record the abort.
            let lines = events_lines(tmp.path());
            assert!(
                !lines
                    .iter()
                    .any(|l| l["type"] == "goal" && l["outcome"] == "accepted"),
                "{leg}: a rejected submit_plan must not record goal/accepted: {lines:?}"
            );
            assert!(
                lines
                    .iter()
                    .any(|l| l["type"] == "abort" && l["reason"] == "iteration budget exceeded"),
                "{leg}: the run must end through the existing abort path: {lines:?}"
            );
            // And nothing was written: not inside cwd…
            assert!(
                !tmp.path().join("plan.md").exists()
                    && !tmp.path().join("plan-escape.md").exists(),
                "{leg}: no plan file may be written on a rejected submit"
            );
            // … nor outside it (the ../ traversal stays lexical-only).
            if out.is_some() {
                assert!(
                    !tmp.path().parent().unwrap().join("plan-escape.md").exists(),
                    "{leg}: the plan must never land outside the sandbox"
                );
            }
        }
    }

    /// submit_plan end-to-end through the REAL loop with `--out`: the file's
    /// bytes are exactly the plan string; exit 0; the loop's own events
    /// stream carries the plan-completed outcome (run_start mode "plan", a
    /// goal line accepted with the plan as summary) — the same machinery a
    /// run's goal acceptance uses.
    #[test]
    fn submit_plan_end_to_end_writes_out_file_and_records_the_outcome() {
        let tmp = tempfile::tempdir().unwrap();
        let out = tmp.path().join("scratch/plan.md");
        let plan_text = "# Plan\n\n1. add the flag\n2. pin the parse\n";
        let mut llm = ScriptedLlm::new(vec![tool_use_response(
            "submit_plan",
            json!({"plan": plan_text}),
        )]);
        let mut sink = RecordingSink::default();
        let code = run_plan_loop(
            plan_cfg(&tmp, Some(out.clone()), 5),
            &mut llm,
            &mut sink,
            &observ::Sink::Noop,
            empty_plan_registry(&tmp),
        )
        .unwrap();
        assert_eq!(code, 0, "an accepted submit_plan exits 0");
        // Verbatim bytes, parent dirs created.
        assert_eq!(fs::read(&out).unwrap(), plan_text.as_bytes());
        // The events stream carries the completion outcome.
        let lines = events_lines(tmp.path());
        assert_eq!(lines[0]["type"], "run_start");
        assert_eq!(
            lines[0]["mode"], "plan",
            "the loop's own run_start names the plan mode"
        );
        let goal = lines
            .iter()
            .find(|l| l["type"] == "goal" && l["outcome"] == "accepted")
            .expect("events stream must record the plan-completed outcome");
        assert_eq!(goal["summary"], plan_text, "the plan rides the goal event");
        // And the submit_plan tool call itself is on the stream as ok.
        assert!(
            lines
                .iter()
                .any(|l| l["type"] == "tool_result" && l["name"] == "submit_plan" && l["ok"] == true),
            "submit_plan tool_result missing: {lines:?}"
        );
    }

    /// stdout leg: without `--out`, the plan surfaces on stdout (via the
    /// GoalAccepted summary the ConsoleSink prints) and NO file is created.
    #[test]
    fn submit_plan_without_out_prints_the_plan_and_creates_no_file() {
        let tmp = tempfile::tempdir().unwrap();
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for_plan(&tmp, None, &controls, &urx);
        let mut knobs = knobs_with(5);
        let plan_text = "# Plan\n\nprinted, not written\n";
        let mut llm = ScriptedLlm::new(vec![tool_use_response(
            "submit_plan",
            json!({"plan": plan_text}),
        )]);
        let mut gate = None;
        let mut messages = Vec::new();
        let out_buf: std::sync::Arc<std::sync::Mutex<Vec<u8>>> = Default::default();
        let err_buf: std::sync::Arc<std::sync::Mutex<Vec<u8>>> = Default::default();
        struct SharedWriter(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);
        impl std::io::Write for SharedWriter {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(buf);
                Ok(buf.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let mut sink = crate::events::ConsoleSink::with_writers(
            Box::new(SharedWriter(out_buf.clone())),
            Box::new(SharedWriter(err_buf.clone())),
            ctx.cwd.to_path_buf(),
        );
        let outcome = drive_loop(
            &ctx,
            &mut knobs,
            &mut llm,
            &mut gate,
            &mut messages,
            None,
            &mut sink,
            &mut McpRegistry::new(ctx.cwd, true, None).unwrap(),
        )
        .unwrap();
        assert!(matches!(outcome, DriveOutcome::RunFinished(0)), "{outcome:?}");
        let stdout = String::from_utf8(out_buf.lock().unwrap().clone()).unwrap();
        assert!(
            stdout.contains("printed, not written"),
            "the plan must surface on stdout: {stdout:?}"
        );
        // No-file pin: nothing but .chug/ exists in the cwd.
        let created: Vec<String> = fs::read_dir(tmp.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(created, vec![".chug"], "no plan file may be created: {created:?}");
    }

    /// Budget exhaustion without submit_plan uses the existing abort path
    /// unchanged: nonzero exit, Aborted event naming model + budget.
    #[test]
    fn plan_mode_budget_death_uses_the_existing_abort_path() {
        let tmp = tempfile::tempdir().unwrap();
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for_plan(&tmp, None, &controls, &urx);
        let mut knobs = knobs_with(1);
        let mut llm = ScriptedLlm::new(vec![text_only_response("still thinking…")]);
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
            &mut McpRegistry::new(ctx.cwd, true, None).unwrap(),
        )
        .unwrap();
        assert!(matches!(outcome, DriveOutcome::RunFinished(1)), "{outcome:?}");
        let events = fs::read_to_string(tmp.path().join(".chug/events.jsonl")).unwrap();
        assert!(events.contains("\"type\":\"abort\""), "{events}");
        assert!(events.contains("iteration budget exceeded"), "{events}");
        assert!(events.contains("scripted-model"), "{events}");
        assert!(events.contains("\"budget_kind\":\"iterations\""), "{events}");
    }

    /// Regression pin: the run-mode advertised tool list equals the exact
    /// pre-change set — sixteen tools today (T73 added plan mode's surface
    /// without touching this list; T76 added tgrep and updated this pin in
    /// the same diff; T111 added the three todo tools beside update_ledger
    /// and updated this pin in the same diff). If a rebase changes the set,
    /// update this pin in the same diff and say so.
    #[test]
    fn run_mode_advertised_tool_list_is_exactly_the_pre_change_set() {
        let schemas = crate::tools::tool_schemas();
        let mut names: Vec<&str> = schemas
            .iter()
            .filter_map(|t| t.get("name").and_then(Value::as_str))
            .collect();
        names.sort_unstable();
        let mut expected = [
            "read_file",
            "write_file",
            "edit_file",
            "bash",
            "grep",
            "tgrep",
            "glob",
            "list_dir",
            "update_ledger",
            "todo_add",
            "todo_update",
            "todo_list",
            "goal_complete",
            "delegate",
            "web_fetch",
            "decision_log",
        ];
        expected.sort_unstable();
        assert_eq!(names, expected, "run-mode tool list changed — update this pin in the same diff and say so");
    }

    /// submit_plan absence pin: the run-mode and chat-mode advertised tool
    /// lists do NOT contain submit_plan; the plan-mode list does.
    /// (One test, three assertions.)
    #[test]
    fn submit_plan_is_absent_from_run_and_chat_lists_present_in_plan() {
        // Run surface: the builtin registry (drive_loop extends it with MCP
        // only, never submit_plan).
        let run_names: Vec<String> = crate::tools::tool_schemas()
            .iter()
            .filter_map(|t| t.get("name").and_then(Value::as_str))
            .map(String::from)
            .collect();
        assert!(
            !run_names.contains(&"submit_plan".to_string()),
            "run-mode list must not advertise submit_plan: {run_names:?}"
        );
        // Chat surface: drive a real run_turn (the chat path) with a
        // tool-recording LLM and inspect the advertised array.
        let tmp = tempfile::tempdir().unwrap();
        let mut client = ToolRecordingLlm::new(vec![text_only_response("hi")]);
        let mut messages = vec![Message::user(vec![ContentBlock::text_block(
            "hello".to_string(),
        )])];
        let mut gate = None;
        run_turn(
            tmp.path(),
            &mut client,
            &mut gate,
            &mut messages,
            &Controls::detached(),
            &mpsc::channel().1,
            &mut knobs_with(5),
            Duration::from_secs(tools::BASH_TIMEOUT_SECS),
            &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
            None,
            &observ::Sink::Noop,
            &mut RecordingSink::default(),
        )
        .unwrap();
        let chat_names: Vec<String> = client.recorded_tools[0]
            .iter()
            .filter_map(|t| t.get("name").and_then(Value::as_str))
            .map(String::from)
            .collect();
        assert!(
            !chat_names.contains(&"submit_plan".to_string()),
            "chat-mode list must not advertise submit_plan: {chat_names:?}"
        );
        // Plan surface: the five, submit_plan included.
        let plan_names: Vec<String> = crate::plan::tool_schemas()
            .iter()
            .filter_map(|t| t.get("name").and_then(Value::as_str))
            .map(String::from)
            .collect();
        assert!(
            plan_names.contains(&"submit_plan".to_string()),
            "plan-mode list must advertise submit_plan: {plan_names:?}"
        );
    }

