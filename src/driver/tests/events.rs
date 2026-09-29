// T104 family: events — .chug/events.jsonl + abort-path pins (T10/T11/T12), incl. the unwritable-log leg. Moved bytes byte-identical (T84 rule)
// from driver.rs's test module; every test here lives in exactly one family
// file.
    use super::*; // the shared harness (driver::tests) + driver's own imports
    #[test]
    fn abort_flag_aborts_at_boundary_like_budget_abort() {
        let tmp = tempfile::tempdir().unwrap();
        let spec = tmp.path().join("s.md");
        std::fs::write(&spec, "spec text\ncheck: true\n").unwrap();

        let (stx, srx) = mpsc::channel();
        drop(stx);
        let controls = Controls {
            abort: Arc::new(AtomicBool::new(true)),
            steering_rx: srx,
        };
        let cfg = RunConfig {
            cwd: tmp.path().to_path_buf(),
            spec_path: spec,
            goal: "x".to_string(),
            model: "test-model".to_string(),
            max_iters: 5,
            max_minutes: 10,
            max_tokens: 0, // no token budget: pre-T15 behavior
            max_tokens_per_request: crate::api::DEFAULT_MAX_TOKENS,
            resume: false,
            controls,
            risk_gate: false,
            bash_timeout: Duration::from_secs(tools::BASH_TIMEOUT_SECS),
            mcp_config: None,
            // Tests must never pick up the developer's ~/.config/chug/mcp.json.
            mcp_off: true,
            // T117: a literal test goal — no pack expansion (None wiring leg).
            goal_pack: None,
        };
        let client = Client::new_without_credentials("test-model", crate::api::DEFAULT_MAX_TOKENS).unwrap();
        let mut sink = RecordingSink::default();
        // Noop sink: observability off → the run path must be untouched.
        let code = run_loop(cfg, client, None, &mut sink, &observ::Sink::Noop).unwrap();

        assert_eq!(code, 1);
        assert!(matches!(
            sink.0.iter().find(|e| matches!(e, Event::Aborted { .. })),
            Some(Event::Aborted { reason, .. }) if reason == "operator abort"
        ));
        // identical abort path to budgets: freshest ledger pushed, then abort
        let aborted_idx = sink
            .0
            .iter()
            .position(|e| matches!(e, Event::Aborted { .. }))
            .unwrap();
        assert!(matches!(&sink.0[aborted_idx - 1], Event::LedgerChanged(_)));
        // aborted at the boundary before any LLM call
        assert!(!sink.0.iter().any(|e| matches!(e, Event::ModelText(_))));
        assert!(!sink.0.iter().any(|e| matches!(e, Event::Iteration { .. })));

        // T11: the run's events log opens with the startup banner fields.
        let first: Value = serde_json::from_str(
            std::fs::read_to_string(tmp.path().join(".chug/events.jsonl"))
                .expect("events.jsonl written")
                .lines()
                .next()
                .expect("run_start line"),
        )
        .expect("first line parses");
        assert_eq!(first["type"], "run_start");
        assert_eq!(first["mode"], "run");
        assert_eq!(first["model"], "test-model");
        assert_eq!(first["version"], crate::build_info::VERSION);
        assert_eq!(first["commit"], crate::build_info::GIT_COMMIT);
        // T17: the run's configured ceilings ride the banner.
        assert_eq!(first["max_iters"], 5);
        assert_eq!(first["max_minutes"], 10);
        assert!(first["max_tokens"].is_null(), "no token budget → null");
        // T20: the cwd's checkout HEAD rides along; this tempdir is not a
        // repo, so both stay null (and the run itself was never touched by
        // the failed resolution).
        assert!(first["head_branch"].is_null(), "{first}");
        assert!(first["head_commit"].is_null(), "{first}");
    }

    /// T117 wiring leg (the Some↔None survivor class applied at the RUN call
    /// site): the loop records `goal_pack` from the config on its own
    /// `run_start` line — `Some` names the pack, and `goal_sha256` hashes the
    /// EXPANDED goal text the config carries (transmission truth); `None`
    /// stays null but the field is present.
    #[test]
    fn run_loop_run_start_carries_goal_pack_from_config_both_ways() {
        for (goal_pack, goal) in [
            (Some("smoke".to_string()), "Say hello to hello.".to_string()),
            (None, "a literal goal".to_string()),
        ] {
            let tmp = tempfile::tempdir().unwrap();
            let spec = tmp.path().join("s.md");
            std::fs::write(&spec, "spec text\ncheck: true\n").unwrap();
            let (stx, srx) = mpsc::channel();
            drop(stx);
            let cfg = RunConfig {
                cwd: tmp.path().to_path_buf(),
                spec_path: spec,
                goal: goal.clone(),
                model: "test-model".to_string(),
                max_iters: 5,
                max_minutes: 10,
                max_tokens: 0,
                max_tokens_per_request: crate::api::DEFAULT_MAX_TOKENS,
                resume: false,
                // Abort at the first boundary: the startup path (including
                // run_start) runs, no LLM call is ever made.
                controls: Controls {
                    abort: Arc::new(AtomicBool::new(true)),
                    steering_rx: srx,
                },
                risk_gate: false,
                bash_timeout: Duration::from_secs(tools::BASH_TIMEOUT_SECS),
                mcp_config: None,
                mcp_off: true,
                goal_pack: goal_pack.clone(),
            };
            let client = Client::new_without_credentials("test-model", crate::api::DEFAULT_MAX_TOKENS).unwrap();
            let mut sink = RecordingSink::default();
            run_loop(cfg, client, None, &mut sink, &observ::Sink::Noop).unwrap();

            let first: Value = serde_json::from_str(
                std::fs::read_to_string(tmp.path().join(".chug/events.jsonl"))
                    .expect("events.jsonl written")
                    .lines()
                    .next()
                    .expect("run_start line"),
            )
            .expect("first line parses");
            let obj = first.as_object().expect("run_start is an object");
            match goal_pack {
                Some(pack) => assert_eq!(first["goal_pack"], pack, "{first}"),
                None => {
                    assert!(
                        first["goal_pack"].is_null(),
                        "a literal goal stays null: {first}"
                    );
                    assert!(
                        obj.contains_key("goal_pack"),
                        "the field must be PRESENT even when null: {first}"
                    );
                }
            }
            // The hash is over the goal text the loop was given (the expanded
            // body when a pack fired) — never the pre-expansion invocation.
            assert_eq!(
                first["goal_sha256"],
                crate::eventlog::goal_sha256(&goal),
                "{first}"
            );
        }
    }

    // ---------- T10: .chug/events.jsonl ----------

    /// A scripted autonomous run leaves a jq-mineable events log: iteration
    /// lines carry cumulative tokens, tool results carry ok/is_error/
    /// duration_ms and a ≤200-char preview, and the terminal goal event is
    /// recorded. The tee is transparent to the real sink.
    #[test]
    fn drive_loop_writes_events_jsonl() {
        let tmp = tempfile::tempdir().unwrap();
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for(&tmp, Mode::Autonomous, &controls, &urx, None, &observ::Sink::Noop);
        let mut knobs = knobs_with(5);
        let loud = format!("printf '{}'", "x".repeat(500));
        let mut llm = ScriptedLlm::new(vec![
            tool_use_response("bash", json!({"command": "false"})),
            tool_use_response("bash", json!({"command": loud})),
            tool_use_response("goal_complete", json!({"summary": "all done"})),
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
        // Transparent tee: the real sink still saw the terminal event.
        assert!(
            sink.0.iter().any(|e| matches!(e, Event::GoalAccepted { .. })),
            "inner sink receives events through the tee"
        );

        let lines = events_jsonl(&tmp);
        // Iteration lines: n + cumulative tokens from the Usage merge.
        let iters: Vec<&Value> = lines.iter().filter(|l| l["type"] == "iteration").collect();
        assert_eq!(iters.len(), 3, "one per iteration: {lines:?}");
        assert_eq!(iters[0]["n"], 1);
        assert_eq!(iters[0]["input_tokens"], 10);
        assert_eq!(iters[2]["input_tokens"], 30);
        // Tool results: the failing bash is an error; previews cap at 200.
        let tools: Vec<&Value> = lines.iter().filter(|l| l["type"] == "tool_result").collect();
        assert_eq!(tools.len(), 3, "{lines:?}");
        assert_eq!(tools[0]["name"], "bash");
        assert_eq!(tools[0]["ok"], false);
        assert_eq!(tools[0]["is_error"], true);
        assert!(tools[0]["duration_ms"].is_u64(), "duration recorded");
        assert_eq!(tools[1]["is_error"], false);
        for t in &tools {
            let p = t["preview"].as_str().unwrap();
            assert!(p.chars().count() <= 200, "preview ≤200 chars, got {}", p.chars().count());
        }
        assert_eq!(tools[1]["preview"].as_str().unwrap().chars().count(), 200);
        // The check ran and the terminal goal verdict is on record.
        assert!(lines.iter().any(|l| l["type"] == "verifying"));
        let goal = lines.iter().find(|l| l["type"] == "goal").expect("goal line");
        assert_eq!(goal["outcome"], "accepted");
        assert_eq!(goal["summary"], "all done");
        // The goal line is the last event of the run.
        assert_eq!(lines.last().unwrap()["type"], "goal");
    }

    /// The terminal abort event lands in the log too (budget death here).
    #[test]
    fn drive_loop_events_jsonl_records_abort() {
        let tmp = tempfile::tempdir().unwrap();
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for(&tmp, Mode::Autonomous, &controls, &urx, None, &observ::Sink::Noop);
        let mut knobs = knobs_with(1); // die on the iteration budget after one pass
        let mut llm = ScriptedLlm::new(vec![text_only_response("thinking")]);
        let mut gate = None;
        let mut messages = Vec::new();
        let outcome = drive_loop(
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
        assert!(matches!(outcome, DriveOutcome::RunFinished(1)));
        let lines = events_jsonl(&tmp);
        assert_eq!(lines.last().unwrap()["type"], "abort");
        assert_eq!(
            lines.last().unwrap()["reason"],
            "iteration budget exceeded"
        );
        // T12: the abort line names the model and the exhausted budget.
        assert_eq!(lines.last().unwrap()["model"], "scripted-model");
        assert_eq!(lines.last().unwrap()["budget_kind"], "iterations");
        assert_eq!(lines.last().unwrap()["budget_max"], 1);
    }

    /// T12: the Aborted event itself carries the dying model + the exhausted
    /// budget, so sinks can render the resume-with-fallback hint.
    #[test]
    fn budget_abort_event_names_model_and_exhausted_budget() {
        let tmp = tempfile::tempdir().unwrap();
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for(&tmp, Mode::Autonomous, &controls, &urx, None, &observ::Sink::Noop);
        let mut knobs = knobs_with(1); // die on the iteration budget after one pass
        let mut llm = ScriptedLlm::new(vec![text_only_response("thinking")]);
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
        let abort = sink
            .0
            .iter()
            .find_map(|e| match e {
                Event::Aborted {
                    reason,
                    model,
                    budget,
                } => Some((reason.clone(), model.clone(), *budget)),
                _ => None,
            })
            .expect("abort event emitted");
        assert_eq!(abort.0, "iteration budget exceeded");
        assert_eq!(abort.1, "scripted-model", "the dying model is named");
        assert_eq!(
            abort.2,
            Some(BudgetExceeded::Iterations { max: 1 }),
            "the exhausted iteration budget is named"
        );
    }

    /// T12: operator aborts share the model line but carry no budget (the
    /// fallback hint is a budget-death feature).
    #[test]
    fn operator_abort_event_has_model_but_no_budget() {
        let tmp = tempfile::tempdir().unwrap();
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls {
            abort: Arc::new(AtomicBool::new(true)),
            steering_rx: mpsc::channel().1,
        };
        let ctx = ctx_for(&tmp, Mode::Autonomous, &controls, &urx, None, &observ::Sink::Noop);
        let mut knobs = knobs_with(5);
        let mut llm = ScriptedLlm::new(vec![]);
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
        let abort = sink
            .0
            .iter()
            .find_map(|e| match e {
                Event::Aborted { model, budget, .. } => Some((model.clone(), *budget)),
                _ => None,
            })
            .expect("abort event emitted");
        assert_eq!(abort.0, "scripted-model");
        assert_eq!(abort.1, None);
    }

    /// An unwritable events log never aborts the run: poison the path with
    /// a directory so every append fails, and the scripted run still
    /// completes through the real sink.
    #[test]
    fn drive_loop_survives_unwritable_events_log() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".chug").join("events.jsonl")).unwrap();
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for(&tmp, Mode::Autonomous, &controls, &urx, None, &observ::Sink::Noop);
        let mut knobs = knobs_with(5);
        let mut llm = ScriptedLlm::new(vec![
            tool_use_response("bash", json!({"command": "true"})),
            tool_use_response("goal_complete", json!({"summary": "finished anyway"})),
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
        assert!(
            sink.0.iter().any(|e| matches!(e, Event::GoalAccepted { .. })),
            "run completes with the events log failing underneath"
        );
    }

