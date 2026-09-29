// T139 family: spec_check_gate — the goal_complete verification check passes
// the SAME pre-execution gates as an ordinary bash tool call. The spec is
// re-read every iteration, so its `check:` line is model-influenceable text:
// a model that may edit files must never be able to turn the spec check into
// an ungated shell execution (CODEX-REVIEW-20260928 §2 HIGH), and a gated-
// blocked check must never verify a goal (fail-closed).
    use super::*; // the shared harness (driver::tests) + driver's own imports

    /// Write the run spec into the cwd and point the knobs at it — the
    /// re-read-every-iteration path the trigger rides.
    fn spec_file(tmp: &tempfile::TempDir, text: &str) -> PathBuf {
        let path = tmp.path().join("spec.md");
        std::fs::write(&path, text).unwrap();
        path
    }

    fn no_accepted_goal(lines: &[Value]) -> bool {
        !lines
            .iter()
            .any(|l| l["type"] == "goal" && l["outcome"] == "accepted")
    }

    /// An allow-judging judge double that RECORDS whether it was ever
    /// consulted. The order-only signal for the pair-order pins below: when
    /// a gate leg is wrongly dispatched BEFORE the permissions deny or the
    /// hook veto, the visible outcome is identical (the later leg still
    /// blocks), but the judge is asked for a verdict on a command policy
    /// already refused — a real judge call (latency, tokens, transcript).
    /// Only the flag separates the two orders, so the pins assert on it.
    struct RecordingAllowJudge {
        called: std::sync::Arc<std::sync::atomic::AtomicBool>,
    }

    impl crate::riskgate::Judge for RecordingAllowJudge {
        fn judge(&mut self, _command: &str) -> Result<crate::riskgate::Verdict, String> {
            self.called
                .store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(crate::riskgate::Verdict {
                choice: "safe".to_string(),
                p_destructive: 0.1,
            })
        }
    }

    /// THE TRIGGER (reviews/CODEX-REVIEW-20260928.md §2 HIGH): deny the bash
    /// tool but permit file editing. The model edits its own spec to contain
    /// an arbitrary `check:` shell command, then calls `goal_complete` the
    /// next iteration. The check must be REFUSED — never executed, the goal
    /// never accepted, the loop alive. Before the fix the check ran via
    /// `run_shell` outside the permission chain: the file was created and the
    /// goal was accepted on the strength of the attacker-chosen command.
    #[test]
    fn edited_spec_check_is_refused_when_bash_is_denied() {
        let tmp = tempfile::tempdir().unwrap();
        write_permissions_json(tmp.path(), json!([{"tool": "bash"}]));
        let spec = spec_file(&tmp, "original spec\n");
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for(&tmp, Mode::Autonomous, &controls, &urx, None, &observ::Sink::Noop);
        let mut knobs = knobs_with(3);
        knobs.spec_path = Some(spec.clone());
        let mut llm = ScriptedLlm::new(vec![
            // Iteration 1: the model edits its own spec (edit_file is NOT
            // denied — the injection is the edited text becoming the check).
            tool_use_response(
                "edit_file",
                json!({
                    "path": spec.to_string_lossy(),
                    "old": "original spec",
                    "new": "check: touch pwned-by-check.txt",
                }),
            ),
            // Iteration 2: goal_complete rides the edited check line.
            tool_use_response("goal_complete", json!({"summary": "claim done"})),
            // Iteration 3: the loop must still be alive after the rejection.
            text_only_response("routing around"),
        ]);
        let mut gate = None;
        let mut messages = Vec::new();
        let outcome = drive_loop(
            &ctx,
            &mut knobs,
            &mut llm,
            &mut gate,
            &mut messages,
            Some("original spec\n".to_string()),
            &mut RecordingSink::default(),
            &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
        )
        .unwrap();
        // The attacker-chosen command never executed.
        assert!(
            !tmp.path().join("pwned-by-check.txt").exists(),
            "the spec check must not execute while bash is denied"
        );
        // The goal was never accepted: no exit-0 finish (the loop runs to the
        // iteration budget instead).
        assert!(
            matches!(outcome, DriveOutcome::RunFinished(1)),
            "a bash-deny must block verification, not accept the goal: {outcome:?}"
        );
        let lines = events_jsonl(&tmp);
        assert!(
            no_accepted_goal(&lines),
            "no accepted goal on a gated-blocked check: {lines:?}"
        );
        // The model is told the check was BLOCKED by policy, not that it
        // failed — it never ran, so "fix the failure" would send the model
        // chasing a nonexistent failure.
        let rejection: String = messages
            .iter()
            .flat_map(|m| m.content.iter())
            .filter_map(|b| b.text())
            .collect();
        assert!(rejection.contains("blocked"), "{rejection}");
        assert!(!rejection.contains("Fix the failure and try again"), "{rejection}");
    }

    /// The command-matcher leg: a `deny bash` rule with a command glob judges
    /// the CHECK command exactly like a bash call — a matching check is
    /// refused, a non-matching check still runs and can verify.
    #[test]
    fn spec_check_command_glob_deny_judges_the_check_command() {
        let tmp = tempfile::tempdir().unwrap();
        write_permissions_json(
            tmp.path(),
            json!([{"tool": "bash", "command": "*secret*"}]),
        );
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for(&tmp, Mode::Chat, &controls, &urx, None, &observ::Sink::Noop);

        // Matching check command: blocked, never executed, goal rejected.
        let mut knobs = knobs_with(5);
        knobs.check_cmd = Some("echo leaking > secret.txt".to_string());
        let mut llm = ScriptedLlm::new(vec![
            tool_use_response("goal_complete", json!({"summary": "claim done"})),
            text_only_response("routed around"),
        ]);
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
        assert!(
            !tmp.path().join("secret.txt").exists(),
            "a check command matching the deny glob must not execute"
        );
        assert!(
            matches!(outcome, DriveOutcome::TurnEnded(TurnEndReason::Completed)),
            "the loop stays alive past the block: {outcome:?}"
        );
        assert!(no_accepted_goal(&events_jsonl(&tmp)));

        // Non-matching check command: the same config lets it run and verify.
        let ctx = ctx_for(&tmp, Mode::Chat, &controls, &urx, None, &observ::Sink::Noop);
        let mut knobs = knobs_with(5);
        knobs.check_cmd = Some("echo ran > check-ran.txt".to_string());
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
            None,
            &mut RecordingSink::default(),
            &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
        )
        .unwrap();
        assert!(
            tmp.path().join("check-ran.txt").exists(),
            "a check command the deny glob does not match still runs"
        );
        assert!(
            matches!(outcome, DriveOutcome::TurnEnded(TurnEndReason::GoalAccepted)),
            "a permitted check verifies the goal: {outcome:?}"
        );
    }

    /// The risk gate judges the check command too: a destructive verdict
    /// blocks verification (the command never runs, the goal is rejected),
    /// and an allowed verdict lets it run and verify — the gate's own
    /// semantics (fail-open on judge failure, `allow destructive` override)
    /// are unchanged.
    #[test]
    fn spec_check_is_risk_gate_judged_like_a_bash_command() {
        let tmp = tempfile::tempdir().unwrap();
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for(&tmp, Mode::Chat, &controls, &urx, None, &observ::Sink::Noop);
        let mut knobs = knobs_with(5);
        knobs.check_cmd = Some("echo ran > gate-check.txt".to_string());

        // Destructive verdict → blocked before execution, goal rejected.
        let mut gate = Some(RiskGate::new(
            Box::new(CannedJudge("destructive", 0.9)),
            tmp.path(),
        ));
        let mut llm = ScriptedLlm::new(vec![
            tool_use_response("goal_complete", json!({"summary": "claim done"})),
            text_only_response("routed around"),
        ]);
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
        assert!(
            !tmp.path().join("gate-check.txt").exists(),
            "a gate-blocked check must not execute"
        );
        assert!(
            matches!(outcome, DriveOutcome::TurnEnded(TurnEndReason::Completed)),
            "the loop stays alive past the gate block: {outcome:?}"
        );
        assert!(no_accepted_goal(&events_jsonl(&tmp)));

        // Safe verdict → the check runs and verifies.
        let ctx = ctx_for(&tmp, Mode::Chat, &controls, &urx, None, &observ::Sink::Noop);
        let mut knobs = knobs_with(5);
        knobs.check_cmd = Some("echo ran > gate-check.txt".to_string());
        let mut gate = Some(RiskGate::new(
            Box::new(CannedJudge("safe", 0.1)),
            tmp.path(),
        ));
        let mut llm = ScriptedLlm::new(vec![tool_use_response(
            "goal_complete",
            json!({"summary": "claim done"}),
        )]);
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
        assert!(
            tmp.path().join("gate-check.txt").exists(),
            "a gate-allowed check runs"
        );
        assert!(
            matches!(outcome, DriveOutcome::TurnEnded(TurnEndReason::GoalAccepted)),
            "a gate-allowed check verifies the goal: {outcome:?}"
        );
    }

    /// A PreToolUse hook that vetoes bash also vetoes the check: the check
    /// is a bash-shaped execution, so hook-side gating applies to it.
    #[test]
    fn pre_tool_use_veto_blocks_spec_check() {
        let tmp = tempfile::tempdir().unwrap();
        write_hooks_json(
            tmp.path(),
            json!([hook_entry("bash", "echo vetoing-hook-stderr >&2; exit 2")]),
            json!([]),
        );
        let spec = spec_file(&tmp, "check: touch vetoed-by-hook.txt\n");
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for(&tmp, Mode::Autonomous, &controls, &urx, None, &observ::Sink::Noop);
        let mut knobs = knobs_with(2);
        knobs.spec_path = Some(spec);
        let mut llm = ScriptedLlm::new(vec![
            tool_use_response("goal_complete", json!({"summary": "claim done"})),
            text_only_response("routing around"),
        ]);
        let mut gate = None;
        let mut messages = Vec::new();
        let outcome = drive_loop(
            &ctx,
            &mut knobs,
            &mut llm,
            &mut gate,
            &mut messages,
            Some("check: touch vetoed-by-hook.txt\n".to_string()),
            &mut RecordingSink::default(),
            &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
        )
        .unwrap();
        assert!(
            !tmp.path().join("vetoed-by-hook.txt").exists(),
            "a vetoed check must not execute"
        );
        assert!(
            matches!(outcome, DriveOutcome::RunFinished(1)),
            "a vetoed check cannot verify the goal: {outcome:?}"
        );
        let rejection: String = messages
            .iter()
            .flat_map(|m| m.content.iter())
            .filter_map(|b| b.text())
            .collect();
        assert!(rejection.contains("vetoing-hook-stderr"), "{rejection}");
        assert!(rejection.contains("blocked"), "{rejection}");
    }

    /// The legitimate operator path survives: with no deny rules (default
    /// permissions) the check still executes through the same run_shell
    /// wrapper and a passing check accepts the goal.
    #[test]
    fn spec_check_still_runs_and_accepts_under_default_permissions() {
        let tmp = tempfile::tempdir().unwrap();
        let spec = spec_file(&tmp, "spec text\ncheck: echo ran > check-ran.txt\n");
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for(&tmp, Mode::Autonomous, &controls, &urx, None, &observ::Sink::Noop);
        let mut knobs = knobs_with(3);
        knobs.spec_path = Some(spec);
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
            Some("spec text\ncheck: echo ran > check-ran.txt\n".to_string()),
            &mut RecordingSink::default(),
            &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
        )
        .unwrap();
        assert!(
            tmp.path().join("check-ran.txt").exists(),
            "the check must still execute under default permissions"
        );
        assert!(
            matches!(outcome, DriveOutcome::RunFinished(0)),
            "a passing check accepts the goal: {outcome:?}"
        );
        let lines = events_jsonl(&tmp);
        assert!(
            !no_accepted_goal(&lines),
            "the accepted goal is recorded: {lines:?}"
        );
    }

    // ---------------------------------------------------------------------
    // ORDER PINS (T139 fix-up F1): verify()'s gate chain dispatches in the
    // bash chain's order — permissions deny → PreToolUse veto → risk gate —
    // and every ADJACENT pair is pinned by its own killing test. A reorder
    // mutant leaves the block outcome identical (the later leg still
    // blocks), so each pin asserts on the leg-ORDER signal: the winning
    // leg's text and, where the loser is the gate, the judge's consultation
    // flag. Each pin below was RED-proven against its reorder mutant (M11,
    // M4, and the hook/gate swap) and passes on the clean tree.
    // ---------------------------------------------------------------------

    /// PIN 1 — permissions.check deny BEFORE the PreToolUse hook veto.
    /// Both gates configured against the same check: the deny reason wins
    /// the model-visible message and ZERO hook events fire (mirrors
    /// permission_deny_fires_no_hooks_before_or_after on the bash dispatch
    /// path). Kills M11 (hook veto moved before permissions.check): there
    /// the veto hook fires and its text wins the message instead.
    #[test]
    fn spec_check_permission_deny_wins_over_pre_tool_use_hook() {
        let tmp = tempfile::tempdir().unwrap();
        write_permissions_json(tmp.path(), json!([{"tool": "bash"}]));
        write_hooks_json(
            tmp.path(),
            json!([hook_entry("bash", "echo HOOK-VETO-MUST-NOT-WIN >&2; exit 2")]),
            json!([]),
        );
        let spec = spec_file(&tmp, "check: touch deny-hook-order.txt\n");
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for(&tmp, Mode::Autonomous, &controls, &urx, None, &observ::Sink::Noop);
        let mut knobs = knobs_with(2);
        knobs.spec_path = Some(spec);
        let mut llm = ScriptedLlm::new(vec![
            tool_use_response("goal_complete", json!({"summary": "claim done"})),
            text_only_response("routing around"),
        ]);
        let mut gate = None;
        let mut messages = Vec::new();
        let outcome = drive_loop(
            &ctx,
            &mut knobs,
            &mut llm,
            &mut gate,
            &mut messages,
            Some("check: touch deny-hook-order.txt\n".to_string()),
            &mut RecordingSink::default(),
            &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
        )
        .unwrap();
        // The check never executed under either gate.
        assert!(
            !tmp.path().join("deny-hook-order.txt").exists(),
            "a denied check must not execute"
        );
        // The DENY reason wins the message (permissions dispatch first) —
        // not the hook's veto text.
        let rejection: String = messages
            .iter()
            .flat_map(|m| m.content.iter())
            .filter_map(|b| b.text())
            .collect();
        assert!(rejection.contains("[permission denied]"), "{rejection}");
        assert!(
            !rejection.contains("HOOK-VETO-MUST-NOT-WIN"),
            "the hook veto text must not win over the deny: {rejection}"
        );
        // Fail-closed: no accepted goal, the loop stays alive.
        assert!(
            matches!(outcome, DriveOutcome::RunFinished(1)),
            "a denied check cannot verify the goal: {outcome:?}"
        );
        let lines = events_jsonl(&tmp);
        assert!(no_accepted_goal(&lines), "no accepted goal on a deny: {lines:?}");
        // ZERO hook events: a denied check fires no PreToolUse hook (and
        // verify() fires no PostToolUse hook by design) — the order signal
        // on the events stream.
        assert!(
            !lines.iter().any(|l| l["type"] == "hook"),
            "a denied check must fire NO hook: {lines:?}"
        );
    }

    /// PIN 2 — permissions.check deny BEFORE the risk gate. The deny fires
    /// with an allow-judging gate present: the deny still wins and the
    /// judge is NEVER consulted. Kills M4 (risk gate moved before
    /// permissions.check): there the judge judges (and allows) the command
    /// first — same visible block, wrong order.
    #[test]
    fn spec_check_permission_deny_wins_over_risk_gate() {
        let tmp = tempfile::tempdir().unwrap();
        write_permissions_json(tmp.path(), json!([{"tool": "bash"}]));
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for(&tmp, Mode::Chat, &controls, &urx, None, &observ::Sink::Noop);
        let mut knobs = knobs_with(5);
        knobs.check_cmd = Some("echo ran > deny-gate-order.txt".to_string());
        let consulted = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let mut gate = Some(RiskGate::new(
            Box::new(RecordingAllowJudge {
                called: std::sync::Arc::clone(&consulted),
            }),
            tmp.path(),
        ));
        let mut llm = ScriptedLlm::new(vec![
            tool_use_response("goal_complete", json!({"summary": "claim done"})),
            text_only_response("routed around"),
        ]);
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
        // The deny wins: never executed, deny text in the model's message.
        assert!(
            !tmp.path().join("deny-gate-order.txt").exists(),
            "a denied check must not execute"
        );
        let rejection: String = messages
            .iter()
            .flat_map(|m| m.content.iter())
            .filter_map(|b| b.text())
            .collect();
        assert!(rejection.contains("[permission denied]"), "{rejection}");
        assert!(rejection.contains("blocked"), "{rejection}");
        assert!(
            matches!(outcome, DriveOutcome::TurnEnded(TurnEndReason::Completed)),
            "the loop stays alive past the deny: {outcome:?}"
        );
        assert!(no_accepted_goal(&events_jsonl(&tmp)));
        // THE ORDER SIGNAL: the risk judge was never consulted — the deny
        // fires first, so a gate leg after it is unreachable for a denied
        // command.
        assert!(
            !consulted.load(std::sync::atomic::Ordering::SeqCst),
            "the risk judge must never be consulted when the permission deny fires first"
        );
    }

    /// PIN 3 — PreToolUse hook veto BEFORE the risk gate. The veto fires
    /// with an allow-judging gate present: the veto text wins and the judge
    /// is NEVER consulted. Kills the third adjacency (risk gate moved
    /// before the hooks leg): there the judge judges (and allows) first —
    /// same visible veto, wrong order.
    #[test]
    fn spec_check_hook_veto_wins_over_risk_gate() {
        let tmp = tempfile::tempdir().unwrap();
        write_hooks_json(
            tmp.path(),
            json!([hook_entry("bash", "echo HOOK-VETO-WINS >&2; exit 2")]),
            json!([]),
        );
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for(&tmp, Mode::Chat, &controls, &urx, None, &observ::Sink::Noop);
        let mut knobs = knobs_with(5);
        knobs.check_cmd = Some("echo ran > hook-gate-order.txt".to_string());
        let consulted = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let mut gate = Some(RiskGate::new(
            Box::new(RecordingAllowJudge {
                called: std::sync::Arc::clone(&consulted),
            }),
            tmp.path(),
        ));
        let mut llm = ScriptedLlm::new(vec![
            tool_use_response("goal_complete", json!({"summary": "claim done"})),
            text_only_response("routed around"),
        ]);
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
        // The veto wins: never executed, the hook's stderr rides the
        // model's message.
        assert!(
            !tmp.path().join("hook-gate-order.txt").exists(),
            "a vetoed check must not execute"
        );
        let rejection: String = messages
            .iter()
            .flat_map(|m| m.content.iter())
            .filter_map(|b| b.text())
            .collect();
        assert!(rejection.contains("HOOK-VETO-WINS"), "{rejection}");
        assert!(rejection.contains("blocked"), "{rejection}");
        assert!(
            matches!(outcome, DriveOutcome::TurnEnded(TurnEndReason::Completed)),
            "the loop stays alive past the veto: {outcome:?}"
        );
        assert!(no_accepted_goal(&events_jsonl(&tmp)));
        // Non-vacuousness: the vetoing hook DID fire — exactly one
        // PreToolUse fire line with the veto flag.
        let lines = events_jsonl(&tmp);
        let hook_lines: Vec<&Value> = lines.iter().filter(|l| l["type"] == "hook").collect();
        assert_eq!(
            hook_lines.len(),
            1,
            "the vetoing hook must have fired exactly once: {lines:?}"
        );
        assert_eq!(hook_lines[0]["event"], "PreToolUse");
        assert_eq!(hook_lines[0]["veto"], true);
        // THE ORDER SIGNAL: the risk judge was never consulted — the veto
        // fires first, so a gate leg after it is unreachable for a vetoed
        // command.
        assert!(
            !consulted.load(std::sync::atomic::Ordering::SeqCst),
            "the risk judge must never be consulted when the hook veto fires first"
        );
    }
