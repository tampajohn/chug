// T104 family: goal — goal-rejection environment honesty (T9). Moved bytes byte-identical (T84 rule)
// from driver.rs's test module; every test here lives in exactly one family
// file.
    use super::*; // the shared harness (driver::tests) + driver's own imports
    // ---------- T9: goal-rejection environment honesty ----------

    #[test]
    fn goal_rejected_message_states_check_environment() {
        let msg = goal_rejected_message("$ cargo test\nexit code: 127\nsh: cargo: command not found");
        // The three key facts (T9): the check shares the bash tool's
        // run_shell wrapper, the cargo PATH prepend, and the out-of-cwd
        // prohibition. The literals are derived from the shell wrapper's own
        // constants, so drift between the message and reality breaks here.
        assert!(msg.contains("same shell wrapper as your bash tool"), "{msg}");
        assert!(msg.contains(&format!("~/{}", tools::CARGO_BIN_REL)), "{msg}");
        assert!(
            msg.contains(&format!("{}s timeout", tools::CHECK_TIMEOUT_SECS)),
            "{msg}"
        );
        assert!(
            msg.contains("do NOT create or modify files outside the run cwd"),
            "{msg}"
        );
        // The original guidance and the failing output are preserved.
        assert!(msg.contains("Fix the failure and try again"), "{msg}");
        assert!(msg.contains("sh: cargo: command not found"), "{msg}");
    }

    #[test]
    fn goal_rejection_includes_environment_note_in_tool_result() {
        // End to end through drive_loop: a goal_complete with a failing
        // check puts the T9 environment note in the user message the model
        // sees next iteration.
        let tmp = tempfile::tempdir().unwrap();
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for(&tmp, Mode::Autonomous, &controls, &urx, None, &observ::Sink::Noop);
        let mut knobs = knobs_with(1); // abort right after the first iteration
        let mut llm = ScriptedLlm::new(vec![tool_use_response(
            "goal_complete",
            json!({"summary": "claim done"}),
        )]);
        let mut gate = None;
        let mut messages = Vec::new();
        let outcome = drive_loop(
            &ctx,
            &mut knobs,
            &mut llm,
            &mut gate,
            &mut messages,
            Some("check: false".to_string()),
            &mut RecordingSink::default(),
            &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
        )
        .unwrap();
        assert!(matches!(outcome, DriveOutcome::RunFinished(1)));
        let rejection: String = messages
            .last()
            .expect("rejection user message")
            .content
            .iter()
            .filter_map(|b| b.text())
            .collect();
        assert!(rejection.contains("goal_complete rejected"), "{rejection}");
        assert!(
            rejection.contains("same shell wrapper as your bash tool"),
            "{rejection}"
        );
        assert!(
            rejection.contains("do NOT create or modify files outside the run cwd"),
            "{rejection}"
        );
    }

