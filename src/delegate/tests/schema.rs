// T109 family: schema — schema + README pins (T23/T28/T39/T58/T69/T89).
// Moved bytes byte-identical (T84 rule) from delegate.rs's test
// module; every test here lives in exactly one family file.
// T109 req 4 count-pin anchor (see mod.rs's pin): this family's
// #[test] fn count — a dropped `mod schema;` line fails the pin's
// reference to this const to compile.
pub(super) const TEST_COUNT: usize = 10;
    use super::*; // the shared harness (delegate::tests) + delegate's own imports

    /// T29 test 5 (schema pins): the live delegate schema gains optional
    /// integer `wait_secs` (min 0, max 600), the required list is unchanged,
    /// and the launch action rejects `wait_secs` with an error naming that
    /// it is status-only.
    #[test]
    fn delegate_schema_pins_wait_secs_and_launch_rejects_it() {
        let schemas = tool_schemas();
        let schema = schemas
            .iter()
            .find(|s| s.get("name").and_then(Value::as_str) == Some("delegate"))
            .expect("exactly one delegate schema (pinned elsewhere)");
        let wait = schema["input_schema"]["properties"]["wait_secs"]
            .as_object()
            .expect("wait_secs property");
        assert_eq!(wait.get("type").and_then(Value::as_str), Some("integer"));
        assert_eq!(wait.get("minimum").and_then(Value::as_u64), Some(0));
        assert_eq!(wait.get("maximum").and_then(Value::as_u64), Some(600));
        let required: Vec<&str> = schema["input_schema"]["required"]
            .as_array()
            .expect("required list")
            .iter()
            .filter_map(Value::as_str)
            .collect();
        assert_eq!(required, vec!["action", "cwd"], "required list must be unchanged");

        // Status-only: the launch rejection names the status action.
        let tmp = tempfile::tempdir().unwrap();
        let result = dispatch(
            &delegate_ctx(tmp.path()),
            "delegate",
            &json!({
                "action": "launch",
                "cwd": tmp.path(),
                "spec": "/tmp/chug-stub-spec.md",
                "goal": "g",
                "model": "m",
                "wait_secs": 5,
            }),
        );
        assert!(result.is_error, "{}", result.content);
        assert!(
            result.content.contains("status action only"),
            "rejection must name that wait_secs is status-only: {}",
            result.content
        );
    }

    /// T89 schema pin (T41 convention): the `terminal` property is described
    /// as status-only, boolean, default false, and names its wake set
    /// (terminal facts only — no iteration/budget-low wake).
    #[test]
    fn delegate_schema_pins_terminal_property() {
        let schemas = tool_schemas();
        let schema = schemas
            .iter()
            .find(|s| s.get("name").and_then(Value::as_str) == Some("delegate"))
            .expect("exactly one delegate schema (pinned elsewhere)");
        let terminal = schema["input_schema"]["properties"]["terminal"]
            .as_object()
            .expect("terminal property");
        assert_eq!(terminal.get("type").and_then(Value::as_str), Some("boolean"));
        let desc = terminal
            .get("description")
            .and_then(Value::as_str)
            .expect("terminal property carries a description");
        assert!(
            desc.contains("status only"),
            "terminal must be described as status-only: {desc}"
        );
        assert!(
            desc.contains("default false"),
            "terminal must name its default: {desc}"
        );
        assert!(
            desc.contains("goal_seen") && desc.contains("abort_seen"),
            "terminal must name the verdict-flag wake set: {desc}"
        );
        assert!(
            desc.contains("wait_secs > 0"),
            "terminal must name its wait_secs > 0 requirement: {desc}"
        );
        // The tool description names the terminal mode too (doc honesty).
        let tool_desc = schema
            .get("description")
            .and_then(Value::as_str)
            .expect("delegate tool description");
        assert!(
            tool_desc.contains("terminal"),
            "tool description must name the terminal wait mode: {tool_desc}"
        );
    }

    /// T68 schema pin (T22/T41 convention): the LIVE `tool_schemas()` delegate
    /// entry names the significant-change wake set in BOTH the tool
    /// description's wait_secs sentence and the `wait_secs` property
    /// description — and the stale any-field phrasing ("events state
    /// changes" / "child state change") is gone. Doc honesty: the schema is
    /// what a cold orchestrator reads; describing an any-field wake would
    /// promise more than the (deliberately narrowed) code delivers.
    #[test]
    fn delegate_schema_describes_significant_wake_set() {
        let schemas = tool_schemas();
        let schema = schemas
            .iter()
            .find(|s| s.get("name").and_then(Value::as_str) == Some("delegate"))
            .expect("exactly one delegate schema (pinned elsewhere)");
        let tool_desc = schema
            .get("description")
            .and_then(Value::as_str)
            .expect("delegate tool description");
        assert!(
            tool_desc.contains("iteration advances"),
            "tool description must name the iteration-advance wake: {tool_desc}"
        );
        assert!(
            tool_desc.contains("verdict or budget-low flag"),
            "tool description must name the verdict/budget-low wake: {tool_desc}"
        );
        assert!(
            tool_desc.contains("never wakes it"),
            "tool description must say last_event churn never wakes: {tool_desc}"
        );
        assert!(
            !tool_desc.contains("events state changes"),
            "stale any-field phrasing must be gone from the tool description: {tool_desc}"
        );
        let wait_desc = schema["input_schema"]["properties"]["wait_secs"]
            .get("description")
            .and_then(Value::as_str)
            .expect("wait_secs property carries a description");
        assert!(
            wait_desc.contains("significant child change"),
            "wait_secs description must name the significant set: {wait_desc}"
        );
        assert!(
            wait_desc.contains("never wakes"),
            "wait_secs description must exclude last_event churn: {wait_desc}"
        );
        assert!(
            !wait_desc.contains("child state change"),
            "stale any-field phrasing must be gone from the wait_secs description: {wait_desc}"
        );
    }

    /// T68 README pin (T41 convention): the `delegate` paragraph's wait_secs
    /// clause names the significant wake set — no stale "state changes"
    /// phrasing. Whitespace-normalized so markdown rewrapping cannot unpin it.
    #[test]
    fn readme_delegate_wait_clause_names_significant_wake_set() {
        let readme = fs::read_to_string(
            std::env::current_dir()
                .expect("cargo sets the test cwd to the package root")
                .join("README.md"),
        )
        .expect("README.md readable from the crate root");
        let flat: String = readme.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(
            flat.contains(
                "it returns early when the child's iteration advances, a verdict or budget-low flag appears, or its liveness flips to dead"
            ),
            "README wait_secs clause lost the significant wake set: {flat}"
        );
        assert!(
            flat.contains("per-tool-call `last_event` churn renders at the deadline but never wakes it"),
            "README wait_secs clause must exclude last_event churn: {flat}"
        );
        assert!(
            !flat.contains("when the child's state changes or"),
            "stale any-field phrasing must be gone from the README clause: {flat}"
        );
    }

    #[test]
    fn delegate_schema_registers_exactly_one_entry_with_all_three_actions() {
        let schemas = tool_schemas();
        let entries: Vec<&Value> = schemas
            .iter()
            .filter(|s| s.get("name").and_then(Value::as_str) == Some("delegate"))
            .collect();
        assert_eq!(entries.len(), 1, "exactly one delegate schema");
        let schema = &entries[0];
        let action = schema
            .get("input_schema")
            .and_then(|s| s.get("properties"))
            .and_then(|p| p.get("action"))
            .expect("action property");
        let actions: Vec<&str> = action
            .get("enum")
            .and_then(Value::as_array)
            .expect("action enum")
            .iter()
            .filter_map(Value::as_str)
            .collect();
        // T69: the enum gains `collect` — launch/status behavior untouched,
        // so the enum is the pre-T69 list plus exactly the new tail.
        assert_eq!(actions, vec!["launch", "status", "collect"]);
        let required: Vec<&str> = schema["input_schema"]["required"]
            .as_array()
            .expect("required list")
            .iter()
            .filter_map(Value::as_str)
            .collect();
        assert!(required.contains(&"action"), "action must be required");
        assert!(required.contains(&"cwd"), "cwd must be required");
        // No other schema may shadow or duplicate the name.
        assert!(schemas.iter().any(|s| s.get("name").and_then(Value::as_str) == Some("goal_complete")));
    }

    /// T39: the delegate schema advertises `max_tokens` as an OPTIONAL integer
    /// (minimum 1) in launch's properties, and the required list is unchanged.
    #[test]
    fn delegate_schema_pins_optional_max_tokens() {
        let schemas = tool_schemas();
        let schema = schemas
            .iter()
            .find(|s| s.get("name").and_then(Value::as_str) == Some("delegate"))
            .expect("exactly one delegate schema (pinned elsewhere)");
        let prop = schema["input_schema"]["properties"]["max_tokens"]
            .as_object()
            .expect("max_tokens property");
        assert_eq!(prop.get("type").and_then(Value::as_str), Some("integer"));
        assert_eq!(prop.get("minimum").and_then(Value::as_u64), Some(1));
        let required: Vec<&str> = schema["input_schema"]["required"]
            .as_array()
            .expect("required list")
            .iter()
            .filter_map(Value::as_str)
            .collect();
        assert_eq!(required, vec!["action", "cwd"], "required list must be unchanged");
    }

    /// T58 schema pin (T22/T39 convention): the LIVE `tool_schemas()` delegate
    /// entry names the `resume` property for launch — optional boolean, and
    /// its description says what it does (append `--resume`, continue the
    /// child's prior run). The tool description also names the resume leg and
    /// the latest-segment status summary; `required` stays unchanged.
    #[test]
    fn delegate_schema_pins_optional_resume() {
        let schemas = tool_schemas();
        let schema = schemas
            .iter()
            .find(|s| s.get("name").and_then(Value::as_str) == Some("delegate"))
            .expect("exactly one delegate schema (pinned elsewhere)");
        let prop = schema["input_schema"]["properties"]["resume"]
            .as_object()
            .expect("resume property missing from the delegate schema");
        assert_eq!(prop.get("type").and_then(Value::as_str), Some("boolean"));
        let desc = prop
            .get("description")
            .and_then(Value::as_str)
            .expect("resume property carries a description");
        assert!(
            desc.contains("--resume"),
            "resume description must name the appended flag: {desc}"
        );
        assert!(
            desc.contains("prior run"),
            "resume description must say it continues the child's prior run: {desc}"
        );
        let tool_desc = schema
            .get("description")
            .and_then(Value::as_str)
            .expect("delegate tool description");
        assert!(
            tool_desc.contains("resume optional"),
            "tool description lost the resume clause: {tool_desc}"
        );
        assert!(
            tool_desc.contains("latest run segment"),
            "tool description lost the latest-segment status clause: {tool_desc}"
        );
        let required: Vec<&str> = schema["input_schema"]["required"]
            .as_array()
            .expect("required list")
            .iter()
            .filter_map(Value::as_str)
            .collect();
        assert_eq!(required, vec!["action", "cwd"], "required list must be unchanged");
    }

    /// T69 schema pins: the action enum carries all three, the tool
    /// description names the collect action truthfully, and the `base`
    /// property is a described optional string.
    #[test]
    fn delegate_schema_pins_collect_and_base() {
        let schemas = tool_schemas();
        let schema = schemas
            .iter()
            .find(|s| s.get("name").and_then(Value::as_str) == Some("delegate"))
            .expect("exactly one delegate schema (pinned elsewhere)");
        let tool_desc = schema
            .get("description")
            .and_then(Value::as_str)
            .expect("delegate tool description");
        // All three actions named in the description's action sentences.
        assert!(tool_desc.contains("action=launch"), "{tool_desc}");
        assert!(tool_desc.contains("action=status"), "{tool_desc}");
        assert!(tool_desc.contains("action=collect"), "{tool_desc}");
        // The collect sentence names its four fields truthfully.
        assert!(tool_desc.contains("verdict"), "{tool_desc}");
        assert!(tool_desc.contains("summary"), "{tool_desc}");
        assert!(tool_desc.contains("check cmd"), "{tool_desc}");
        assert!(tool_desc.contains("commit refs"), "{tool_desc}");
        // The never-blocks contract covers collect too, and the wait knob's
        // rejection is named for both non-waiting actions.
        assert!(tool_desc.contains("collect reads tails only"), "{tool_desc}");
        assert!(tool_desc.contains("launch and collect reject it"), "{tool_desc}");

        let base = schema["input_schema"]["properties"]["base"]
            .as_object()
            .expect("base property");
        assert_eq!(base.get("type").and_then(Value::as_str), Some("string"));
        let desc = base
            .get("description")
            .and_then(Value::as_str)
            .expect("base property carries a description");
        assert!(desc.contains("collect only"), "{desc}");
        assert!(desc.contains("<base>..HEAD"), "{desc}");
        let action_desc = schema["input_schema"]["properties"]["action"]
            .get("description")
            .and_then(Value::as_str)
            .expect("action property carries a description");
        assert!(action_desc.contains("collect"), "{action_desc}");
        let required: Vec<&str> = schema["input_schema"]["required"]
            .as_array()
            .expect("required list")
            .iter()
            .filter_map(Value::as_str)
            .collect();
        assert_eq!(required, vec!["action", "cwd"], "required list must be unchanged");
    }

    /// T115: the delegate schema surface is UNCHANGED — the launch
    /// goal-integrity echoes (`goal_bytes`/`goal_sha256`/`goal_tail`) are a
    /// result-text-only change, so no input property was added and the
    /// required list is untouched. Pinning the exact property list means a
    /// future edit that silently widens the schema trips here instead of
    /// shipping (T104's mutant lesson, applied to the schema surface).
    #[test]
    fn delegate_schema_property_surface_is_unchanged() {
        let schemas = tool_schemas();
        let schema = schemas
            .iter()
            .find(|s| s.get("name").and_then(Value::as_str) == Some("delegate"))
            .expect("exactly one delegate schema (pinned elsewhere)");
        let mut props: Vec<&str> = schema["input_schema"]["properties"]
            .as_object()
            .expect("properties object")
            .keys()
            .map(String::as_str)
            .collect();
        props.sort_unstable();
        assert_eq!(
            props,
            vec![
                "action", "base", "cwd", "env", "goal", "max_iters", "max_minutes", "max_tokens",
                "model", "pid", "resume", "spec", "terminal", "wait_secs",
            ],
            "delegate schema properties drifted — T115 was result-text-only and T183 \
             added `env` with its own pin; any further property needs its own spec \
             and its own pin update"
        );
        let required: Vec<&str> = schema["input_schema"]["required"]
            .as_array()
            .expect("required list")
            .iter()
            .filter_map(Value::as_str)
            .collect();
        assert_eq!(
            required,
            vec!["action", "cwd"],
            "required list must be unchanged"
        );
    }

    /// T183 schema pin (T22/T39/T58 convention): the launch `env` property is
    /// an OPTIONAL object of strings, its description carries the allowlist
    /// regex, the caps, the explicit-wins-over-scrub ordering, and the
    /// fallback sentence (the goal-carried `export` stays the fallback when
    /// `env` is absent); the tool description names the env clause and the
    /// keys-only payload echo. Doc honesty: a cold orchestrator reads the
    /// schema, so it must state the ordering the spawn actually applies.
    #[test]
    fn delegate_schema_pins_optional_env_property() {
        let schemas = tool_schemas();
        let schema = schemas
            .iter()
            .find(|s| s.get("name").and_then(Value::as_str) == Some("delegate"))
            .expect("exactly one delegate schema (pinned elsewhere)");
        let prop = schema["input_schema"]["properties"]["env"]
            .as_object()
            .expect("env property missing from the delegate schema");
        assert_eq!(prop.get("type").and_then(Value::as_str), Some("object"));
        let desc = prop
            .get("description")
            .and_then(Value::as_str)
            .expect("env property carries a description");
        // The allowlist regex, verbatim.
        assert!(
            desc.contains("^(CARGO_|CHUG_|RUST)[A-Z0-9_]*$"),
            "env description must carry the allowlist regex: {desc}"
        );
        // The caps.
        assert!(
            desc.contains("at most 16 entries"),
            "env description must name the entry cap: {desc}"
        );
        assert!(
            desc.contains("4 KiB") && desc.contains("no NUL"),
            "env description must name the value caps: {desc}"
        );
        // The explicit-wins-over-scrub ordering.
        assert!(
            desc.contains("WINS over the scrub"),
            "env description must state explicit-wins-over-scrub: {desc}"
        );
        // The fallback sentence.
        assert!(
            desc.contains("the goal-carried `export` remains the fallback"),
            "env description must state the goal-carried-export fallback: {desc}"
        );
        // The tool description names the clause and the keys-only echo.
        let tool_desc = schema
            .get("description")
            .and_then(Value::as_str)
            .expect("delegate tool description");
        assert!(
            tool_desc.contains("env optional"),
            "tool description lost the env clause: {tool_desc}"
        );
        assert!(
            tool_desc.contains("applied env keys (keys only, never values)"),
            "tool description must name the keys-only env echo: {tool_desc}"
        );
        let required: Vec<&str> = schema["input_schema"]["required"]
            .as_array()
            .expect("required list")
            .iter()
            .filter_map(Value::as_str)
            .collect();
        assert_eq!(
            required,
            vec!["action", "cwd"],
            "required list must be unchanged"
        );
    }
