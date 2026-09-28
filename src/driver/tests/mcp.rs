// T104 family: mcp — MCP schema merge + tool_use routing through the loop (echo server). Moved bytes byte-identical (T84 rule)
// from driver.rs's test module; every test here lives in exactly one family
// file.
    use super::*; // the shared harness (driver::tests) + driver's own imports
    #[test]
    fn mcp_schemas_merged_and_mcp_tool_use_routed_to_registry() {
        let tmp = tempfile::tempdir().unwrap();
        write_echo_server(tmp.path());
        let mut mcp =
            McpRegistry::new(tmp.path(), false, None).expect("registry with fake server");
        assert!(!mcp.tool_schemas().is_empty(), "fake server must register tools");

        let mut client = ToolRecordingLlm::new(vec![
            json!({
                "stop_reason": "tool_use",
                "usage": {"input_tokens": 1, "output_tokens": 1},
                "content": [{"type": "tool_use", "id": "tu_1", "name": "mcp__fake__echo", "input": {"text": "hello mcp"}}]
            }),
            json!({
                "stop_reason": "end_turn",
                "usage": {"input_tokens": 1, "output_tokens": 1},
                "content": [{"type": "text", "text": "done"}]
            }),
        ]);
        let mut messages = vec![Message::user(vec![ContentBlock::text_block(
            "call the echo tool".to_string(),
        )])];
        let mut sink = RecordingSink::default();
        let reason = run_turn(
            tmp.path(),
            &mut client,
            &mut None,
            &mut messages,
            &Controls::detached(),
            &mpsc::channel().1,
            &mut knobs_with(5),
            Duration::from_secs(tools::BASH_TIMEOUT_SECS),
            &mut mcp,
            None,
            &observ::Sink::Noop,
            &mut sink,
        )
        .unwrap();
        assert_eq!(reason, TurnEndReason::Completed);

        // The tools array the model saw merges MCP schemas with the built-ins.
        let offered = &client.recorded_tools[0];
        assert!(
            offered.iter().any(|t| t["name"] == "mcp__fake__echo"),
            "mcp schema missing: {offered:?}"
        );
        assert!(offered.iter().any(|t| t["name"] == "bash"));

        // The mcp__-prefixed tool_use was routed to the registry and the
        // model saw the echoed content.
        let (content, is_error) = tool_result_text(&messages).expect("tool result in transcript");
        assert!(!is_error, "{content}");
        assert!(content.contains(r#""text": "hello mcp""#), "{content}");
    }

    #[test]
    fn empty_registry_is_a_noop_on_the_tools_array() {
        let tmp = tempfile::tempdir().unwrap();
        // mcp_off: guaranteed-empty registry even if the developer's machine
        // has ~/.config/chug/mcp.json.
        let mut mcp = McpRegistry::new(tmp.path(), true, None).unwrap();
        let mut client = ToolRecordingLlm::new(vec![json!({
            "stop_reason": "end_turn",
            "usage": {"input_tokens": 1, "output_tokens": 1},
            "content": [{"type": "text", "text": "done"}]
        })]);
        let mut messages = vec![Message::user(vec![ContentBlock::text_block("hi")])];
        let mut sink = RecordingSink::default();
        run_turn(
            tmp.path(),
            &mut client,
            &mut None,
            &mut messages,
            &Controls::detached(),
            &mpsc::channel().1,
            &mut knobs_with(5),
            Duration::from_secs(tools::BASH_TIMEOUT_SECS),
            &mut mcp,
            None,
            &observ::Sink::Noop,
            &mut sink,
        )
        .unwrap();

        let offered = &client.recorded_tools[0];
        let names: Vec<&str> = offered
            .iter()
            .filter_map(|t| t["name"].as_str())
            .collect();
        let baseline: Vec<String> = tools::tool_schemas()
            .into_iter()
            .filter_map(|t| t["name"].as_str().map(str::to_string))
            .collect();
        // Byte-identical tool list: nothing added, nothing removed.
        assert_eq!(names, baseline);
        assert!(names.iter().all(|n| !n.starts_with("mcp__")));
    }

