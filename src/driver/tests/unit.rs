// T104 family: unit — driver pure-function pins: parse_check_command, system prompt, tool_use extraction, is_allow_destructive. Moved bytes byte-identical (T84 rule)
// from driver.rs's test module; every test here lives in exactly one family
// file.
    use super::*; // the shared harness (driver::tests) + driver's own imports
    #[test]
    fn check_line_parsing() {
        assert_eq!(
            parse_check_command("goal: x\ncheck: cargo test\n"),
            Some("cargo test".to_string())
        );
        assert_eq!(parse_check_command("no check here"), None);
        assert_eq!(
            parse_check_command("check:   echo hi  "),
            Some("echo hi".to_string())
        );
        assert_eq!(
            parse_check_command("check: first\ncheck: second"),
            Some("first".to_string())
        );
        assert_eq!(parse_check_command("check:"), None);
        assert_eq!(parse_check_command("  check: go build ./..."), Some("go build ./...".to_string()));
    }

    #[test]
    fn system_prompt_contains_all_sections() {
        let prompt = build_system_prompt("SPEC TEXT", "do the thing", "# Ledger\n...");
        assert!(prompt.contains("## Spec"));
        assert!(prompt.contains("SPEC TEXT"));
        assert!(prompt.contains("## Goal"));
        assert!(prompt.contains("do the thing"));
        assert!(prompt.contains("## Ledger"));
        assert!(prompt.contains("goal_complete"));
    }

    #[test]
    fn tool_use_blocks_extracted_from_assistant_content() {
        let msg = Message::assistant(vec![
            ContentBlock::text_block("thinking out loud"),
            ContentBlock::Known(KnownBlock::ToolUse {
                id: "tu_1".into(),
                name: "bash".into(),
                input: json!({"command": "ls"}),
            }),
            ContentBlock::Other(json!({"type": "mystery"})),
        ]);
        let uses: Vec<_> = msg.content.iter().filter_map(ContentBlock::tool_use).collect();
        assert_eq!(uses.len(), 1);
        assert_eq!(uses[0].0, "tu_1");
        assert_eq!(uses[0].1, "bash");
    }

    #[test]
    fn allow_destructive_note_matching() {
        assert!(is_allow_destructive("allow destructive"));
        assert!(is_allow_destructive("  ALLOW DESTRUCTIVE  "));
        assert!(!is_allow_destructive("allow destructively"));
        assert!(!is_allow_destructive("please allow destructive commands"));
    }

