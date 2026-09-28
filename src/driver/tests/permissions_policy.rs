// T104 family: permissions_policy — .chug/permissions.json deny-list chain (T90). Moved bytes byte-identical (T84 rule)
// from driver.rs's test module; every test here lives in exactly one family
// file.
    use super::*; // the shared harness (driver::tests) + driver's own imports
    // ---------- T90: .chug/permissions.json deny-list ----------

    /// Write a permissions.json deny config into `cwd/.chug/`.
    fn write_permissions_json(cwd: &Path, deny: Value) {
        let dir = cwd.join(".chug");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            permissions::permissions_path(cwd),
            json!({"permissions": {"deny": deny}}).to_string(),
        )
        .unwrap();
    }

    fn write_permissions_raw(cwd: &Path, text: &str) {
        let dir = cwd.join(".chug");
        fs::create_dir_all(&dir).unwrap();
        fs::write(permissions::permissions_path(cwd), text).unwrap();
    }

    /// Every tool result in the transcript, in order (the single-result
    /// `tool_result_text` only sees the last one).
    fn all_tool_results(messages: &[Message]) -> Vec<(String, bool)> {
        messages
            .iter()
            .filter_map(|m| match &m.content[0] {
                ContentBlock::Known(KnownBlock::ToolResult { content, is_error, .. }) => {
                    Some((content.as_str().unwrap_or_default().to_string(), *is_error))
                }
                _ => None,
            })
            .collect()
    }

    /// THE DENY LEG (kills the allow-by-default mutant): a permissions.json
    /// command-glob rule denies a bash call that WOULD have created a file —
    /// the file does not exist, the model receives a `[permission denied]`
    /// tool error, and the loop stays alive.
    #[test]
    fn permission_deny_blocks_bash_execution_and_loop_continues() {
        let tmp = tempfile::tempdir().unwrap();
        // One malformed sibling (command matcher on read_file) + the valid
        // rule: the sibling must be skipped with a permission_error line
        // while the valid sibling still denies (fail-open per rule,
        // fail-closed on match).
        write_permissions_json(
            tmp.path(),
            json!([
                {"tool": "read_file", "command": "*evil*"},
                {"tool": "bash", "command": "*deny-marker*"}
            ]),
        );
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for(&tmp, Mode::Chat, &controls, &urx, None, &observ::Sink::Noop);
        let mut knobs = knobs_with(5);
        let mut llm = ScriptedLlm::new(vec![
            tool_use_response("bash", json!({"command": "echo created > deny-marker.txt"})),
            text_only_response("routed around the deny"),
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
            !tmp.path().join("deny-marker.txt").exists(),
            "a denied bash call must never execute"
        );
        // The model received a tool error with the deny text naming the rule.
        let (content, is_error) = tool_result_text(&messages).expect("a tool result exists");
        assert!(is_error, "the deny is a tool error: {content:?}");
        assert!(
            content.starts_with("[permission denied] deny bash command \"*deny-marker*\""),
            "{content:?}"
        );
        // The events log carries one deny line per deny + one error line for
        // the skipped malformed sibling — and NO hook lines of any kind.
        let lines = events_jsonl(&tmp);
        let denies: Vec<&Value> = lines.iter().filter(|l| l["type"] == "permission_denied").collect();
        assert_eq!(denies.len(), 1, "{lines:?}");
        assert_eq!(denies[0]["tool"], "bash");
        assert_eq!(denies[0]["rule"], "deny bash command \"*deny-marker*\"");
        let errors: Vec<&Value> = lines.iter().filter(|l| l["type"] == "permission_error").collect();
        assert_eq!(errors.len(), 1, "{lines:?}");
        assert!(errors[0]["detail"].as_str().unwrap().contains("deny[0]"));
        assert!(
            !lines.iter().any(|l| l["type"] == "hook"),
            "a denied call fires no hooks: {lines:?}"
        );
    }

    /// POLICY ORDER, pinned (spec req 3): with a PreToolUse veto hook AND a
    /// PostToolUse hook both matching the same call, a permission deny fires
    /// NO hook — the result text is exactly the deny text (no `[hook veto]`,
    /// no `[hook]` advisory), and events.jsonl carries zero hook lines.
    /// Flip the dispatch order and the veto text wins instead; drop the
    /// `blocked` guard and a phantom PostToolUse line appears.
    #[test]
    fn permission_deny_fires_no_hooks_before_or_after() {
        let tmp = tempfile::tempdir().unwrap();
        write_permissions_json(
            tmp.path(),
            json!([{"tool": "bash", "command": "*deny-marker*"}]),
        );
        write_hooks_json(
            tmp.path(),
            json!([hook_entry("bash", "echo vetoing-hook-stderr >&2; exit 2")]),
            json!([hook_entry("bash", "echo POST-MUST-NOT-FIRE-ON-DENY")]),
        );
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for(&tmp, Mode::Chat, &controls, &urx, None, &observ::Sink::Noop);
        let mut knobs = knobs_with(5);
        let mut llm = ScriptedLlm::new(vec![
            tool_use_response("bash", json!({"command": "echo created > deny-marker.txt"})),
            text_only_response("routed around the deny"),
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
        assert!(!tmp.path().join("deny-marker.txt").exists());
        // The deny result is EXACTLY the deny text — no hook veto text, no
        // PostToolUse advisory riding it.
        let (content, is_error) = tool_result_text(&messages).expect("a tool result exists");
        assert!(is_error, "{content:?}");
        assert!(content.starts_with("[permission denied] "), "{content:?}");
        assert!(!content.contains("[hook"), "{content:?}");
        // Zero hook fire lines: not PreToolUse, not PostToolUse.
        let lines = events_jsonl(&tmp);
        assert!(
            !lines.iter().any(|l| l["type"] == "hook"),
            "a permission deny must fire NO hook: {lines:?}"
        );
    }

    /// NON-VACUOUSNESS (kills the deny-everything / matcher-inverted
    /// mutants): a non-matching command under a command rule executes
    /// normally, hooks still fire around it, and no deny line lands.
    #[test]
    fn non_matching_command_executes_and_hooks_still_fire() {
        let tmp = tempfile::tempdir().unwrap();
        write_permissions_json(
            tmp.path(),
            json!([{"tool": "bash", "command": "*forbidden*"}]),
        );
        write_hooks_json(
            tmp.path(),
            json!([hook_entry("bash", "exit 0")]),
            json!([hook_entry("bash", "echo post-note")]),
        );
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for(&tmp, Mode::Chat, &controls, &urx, None, &observ::Sink::Noop);
        let mut knobs = knobs_with(5);
        let mut llm = ScriptedLlm::new(vec![
            tool_use_response("bash", json!({"command": "echo fine > allow-marker.txt"})),
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
        // The tool executed.
        assert!(tmp.path().join("allow-marker.txt").exists(), "non-matching command must run");
        let (content, is_error) = tool_result_text(&messages).expect("a tool result exists");
        assert!(!is_error, "{content:?}");
        assert!(content.contains("\n\n[hook] post-note"), "{content:?}");
        let lines = events_jsonl(&tmp);
        assert!(
            !lines.iter().any(|l| l["type"] == "permission_denied"),
            "no deny on the allow path: {lines:?}"
        );
        let hook_lines: Vec<&Value> = lines.iter().filter(|l| l["type"] == "hook").collect();
        assert_eq!(hook_lines.len(), 2, "hooks fire on an allowed call: {lines:?}");
    }

    /// A whole-tool deny keeps the run alive too: the denied web_fetch call
    /// errors with the deny text (nothing is fetched) and the next turn
    /// completes.
    #[test]
    fn whole_tool_deny_blocks_web_fetch_and_loop_continues() {
        let tmp = tempfile::tempdir().unwrap();
        write_permissions_json(tmp.path(), json!([{"tool": "web_fetch"}]));
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for(&tmp, Mode::Chat, &controls, &urx, None, &observ::Sink::Noop);
        let mut knobs = knobs_with(5);
        let mut llm = ScriptedLlm::new(vec![
            tool_use_response("web_fetch", json!({"url": "https://example.com/x"})),
            text_only_response("routed around the deny"),
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
        assert!(matches!(outcome, DriveOutcome::TurnEnded(TurnEndReason::Completed)), "{outcome:?}");
        let (content, is_error) = tool_result_text(&messages).expect("a tool result exists");
        assert!(is_error, "{content:?}");
        assert_eq!(content, "[permission denied] deny web_fetch", "{content:?}");
    }

    /// PLAN MODE surfaces the deny (spec req 6): an in-process policy can
    /// only restrict further, so a `read_file *.key` rule denies that read
    /// inside a plan session while the other read-only tools keep working
    /// and the session still ends via submit_plan (the five-tool contract
    /// is unchanged).
    #[test]
    fn plan_mode_permission_deny_restricts_read_file_others_work() {
        let tmp = tempfile::tempdir().unwrap();
        write_permissions_json(
            tmp.path(),
            json!([{"tool": "read_file", "path": "*.key"}]),
        );
        fs::write(tmp.path().join("secret.key"), "PRIVATE").unwrap();
        fs::write(tmp.path().join("notes.md"), "planning input").unwrap();
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for_plan(&tmp, None, &controls, &urx);
        let mut knobs = knobs_with(5);
        let mut llm = ScriptedLlm::new(vec![
            tool_use_response("read_file", json!({"path": "secret.key"})),
            tool_use_response("read_file", json!({"path": "notes.md"})),
            tool_use_response("submit_plan", json!({"plan": "the plan"})),
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
        let results = all_tool_results(&messages);
        // submit_plan ends the session (its result need not land in the
        // transcript); the two reads are what this leg pins.
        assert!(results.len() >= 2, "{results:?}");
        // The denied read: tool error naming the rule; the key's contents
        // never reached the model.
        assert!(results[0].1, "the denied read is a tool error: {:?}", results[0]);
        assert!(
            results[0].0.starts_with("[permission denied] deny read_file path \"*.key\""),
            "{:?}",
            results[0]
        );
        // The allowed read executed and returned the file contents.
        assert!(!results[1].1, "notes.md read works in plan mode: {:?}", results[1]);
        assert!(results[1].0.contains("planning input"), "{:?}", results[1]);
        // One deny line, no hook lines (plan mode never fires hooks anyway).
        let lines = events_jsonl(&tmp);
        let denies: Vec<&Value> = lines.iter().filter(|l| l["type"] == "permission_denied").collect();
        assert_eq!(denies.len(), 1, "{lines:?}");
        assert_eq!(denies[0]["tool"], "read_file");
        assert!(!lines.iter().any(|l| l["type"] == "hook"), "{lines:?}");
    }

    /// The config fail-open leg at driver level: a malformed permissions
    /// config warns + records exactly ONE error line even with two tool
    /// calls in the run (the load happens once per drive_loop invocation),
    /// both calls execute, and the loop behaves exactly as with no config.
    #[test]
    fn malformed_permissions_config_fails_open_once_and_run_continues() {
        let tmp = tempfile::tempdir().unwrap();
        write_permissions_raw(tmp.path(), "{ not json !!!");
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
                    {"type": "tool_use", "id": "tu_1", "name": "bash", "input": {"command": "echo one > pm-one.txt"}},
                    {"type": "tool_use", "id": "tu_2", "name": "bash", "input": {"command": "echo two > pm-two.txt"}}
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
        assert!(tmp.path().join("pm-one.txt").exists() && tmp.path().join("pm-two.txt").exists());
        let lines = events_jsonl(&tmp);
        let errors: Vec<&Value> = lines.iter().filter(|l| l["type"] == "permission_error").collect();
        assert_eq!(errors.len(), 1, "{lines:?}");
        assert!(errors[0]["detail"].as_str().unwrap().contains("malformed"));
        assert!(
            !lines.iter().any(|l| l["type"] == "permission_denied"),
            "zero rules → zero denies: {lines:?}"
        );
    }

    /// Absent config = zero cost: a clean run produces no permission lines
    /// at all (the load leg never emits, and no per-call check runs).
    #[test]
    fn absent_permissions_config_produces_no_permission_lines() {
        let tmp = tempfile::tempdir().unwrap();
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for(&tmp, Mode::Chat, &controls, &urx, None, &observ::Sink::Noop);
        let mut knobs = knobs_with(5);
        let mut llm = ScriptedLlm::new(vec![
            tool_use_response("bash", json!({"command": "echo clean"})),
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
        let lines = events_jsonl(&tmp);
        assert!(
            !lines.iter().any(|l| l["type"].as_str().unwrap_or_default().starts_with("permission")),
            "absent config: zero permission lines: {lines:?}"
        );
    }

