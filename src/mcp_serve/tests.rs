// ---------------------------------------------------------------------------
// Tests — bin-internal, pure handle_message legs (no real stdio)
// ---------------------------------------------------------------------------

    use super::*;
    use serde_json::json;

    /// The flagless dispatch — the pre-T129 entry, byte-identical read-only
    /// behavior, and the call site every phase-1/2a leg already rides
    /// (unchanged by T129). The flag-ON legs call
    /// [`handle_message_with`](super::handle_message_with) directly.
    fn handle_message(line: &str) -> Option<String> {
        super::handle_message_with(line, false)
    }

    /// The flagless serve loop — the pre-T129 entry for the framing legs.
    fn serve_from(read: &mut impl BufRead, out: &mut impl Write) -> anyhow::Result<()> {
        super::serve_from_with(read, out, false)
    }

    /// Parse a response line into its envelope parts.
    fn parts(line: &str) -> (Value, Option<Value>, Option<Value>, Option<Value>) {
        let v: Value = serde_json::from_str(line).expect("response is valid JSON");
        assert_eq!(v["jsonrpc"], "2.0", "envelope jsonrpc: {line}");
        (
            v.clone(),
            v.get("id").cloned(),
            v.get("result").cloned(),
            v.get("error").cloned(),
        )
    }

    /// The `result` of a tools/call response, as (text, isError).
    fn tool_result(line: &str) -> (String, Option<bool>) {
        let (_, _, result, _) = parts(line);
        let result = result.expect("tools/call returns a result, not an error");
        let text = result["content"][0]["text"].as_str().expect("text block").to_string();
        assert_eq!(result["content"][0]["type"], "text", "content type");
        (text, result.get("isError").and_then(Value::as_bool))
    }

    // ---------- handshake ----------

    #[test]
    fn initialize_shape_protocol_version_serverinfo_and_capabilities() {
        let line = handle_message(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#)
            .expect("initialize responds");
        let (_, id, result, error) = parts(&line);
        assert_eq!(id, Some(json!(1)), "response id echoes the request id");
        assert!(error.is_none(), "no error: {line}");
        let result = result.expect("initialize result");
        // The version literal is REUSED from the client module, not duplicated.
        assert_eq!(result["protocolVersion"], crate::mcp::PROTOCOL_VERSION);
        assert_eq!(result["protocolVersion"], "2025-06-18");
        assert!(
            result["capabilities"].get("tools").is_some(),
            "capabilities.tools present: {result}"
        );
        assert_eq!(result["serverInfo"]["name"], "chug");
        assert_eq!(result["serverInfo"]["version"], crate::build_info::VERSION);
    }

    #[test]
    fn initialize_echoes_string_and_null_ids_verbatim() {
        for id in [json!("abc"), json!(null), json!(42)] {
            let req = json!({"jsonrpc":"2.0","id":id,"method":"initialize"}).to_string();
            let line = handle_message(&req).expect("responds");
            let (_, echoed, _, _) = parts(&line);
            assert_eq!(echoed, Some(id), "id echoed verbatim");
        }
    }

    #[test]
    fn ping_replies_with_empty_result() {
        let line = handle_message(r#"{"jsonrpc":"2.0","id":7,"method":"ping"}"#)
            .expect("ping responds");
        let (_, id, result, error) = parts(&line);
        assert_eq!(id, Some(json!(7)));
        assert!(error.is_none(), "{line}");
        assert_eq!(result, Some(json!({})));
    }

    #[test]
    fn notifications_get_no_response() {
        // The handshake notification…
        assert_eq!(
            handle_message(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#),
            None
        );
        // …an arbitrary unknown notification…
        assert_eq!(
            handle_message(r#"{"jsonrpc":"2.0","method":"notifications/foo","params":{}}"#),
            None
        );
        // …and a notifications/* method that illegally carries an id — the
        // prefix means the client does not want a reply, so none comes.
        assert_eq!(
            handle_message(r#"{"jsonrpc":"2.0","id":3,"method":"notifications/initialized"}"#),
            None
        );
    }

    // ---------- tools/list ----------

    #[test]
    fn tools_list_exposes_both_read_only_tools_requiring_cwd() {
        let line = handle_message(r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#)
            .expect("tools/list responds");
        let (_, id, result, _) = parts(&line);
        assert_eq!(id, Some(json!(2)));
        let tools = result.expect("result")["tools"]
            .as_array()
            .expect("tools array")
            .clone();
        assert_eq!(tools.len(), 2, "phases 1+2a ship exactly two tools");
        assert_eq!(tools[0]["name"], "chug_status");
        assert_eq!(tools[1]["name"], "chug_collect");
        for tool in &tools {
            assert_eq!(tool["inputSchema"]["type"], "object");
            let required: Vec<&str> = tool["inputSchema"]["required"]
                .as_array()
                .expect("required array")
                .iter()
                .filter_map(Value::as_str)
                .collect();
            assert!(
                required.contains(&"cwd"),
                "inputSchema requires cwd: {tools:?}"
            );
        }
        // T128: `chug_collect` advertises its three params — cwd required,
        // pid + base optional (properties, NOT required).
        let collect = &tools[1];
        let props = collect["inputSchema"]["properties"]
            .as_object()
            .expect("properties object");
        assert_eq!(props.len(), 3, "cwd + pid + base: {props:?}");
        assert_eq!(props["pid"]["type"], "integer", "{props:?}");
        assert_eq!(props["base"]["type"], "string", "{props:?}");
        assert_eq!(collect["inputSchema"]["required"], json!(["cwd"]));
    }

    // ---------- error taxonomy ----------

    #[test]
    fn unparseable_line_is_parse_error_with_null_id() {
        for garbage in ["{ not json !!!", "", "   ", "[1,2,"] {
            // Blank lines are skipped at the serve layer; here a blank line
            // still must not panic (it parses as no JSON → -32700).
            let line = handle_message(garbage).expect("parse error responds");
            let (_, id, _, error) = parts(&line);
            assert_eq!(id, Some(Value::Null), "id null on parse error: {line}");
            assert_eq!(error.expect("error object")["code"], -32700, "{line}");
        }
    }

    #[test]
    fn unknown_method_on_a_request_is_method_not_found() {
        let line =
            handle_message(r#"{"jsonrpc":"2.0","id":9,"method":"resources/list"}"#)
                .expect("responds");
        let (_, id, _, error) = parts(&line);
        assert_eq!(id, Some(json!(9)));
        let error = error.expect("error object");
        assert_eq!(error["code"], -32601, "{line}");
        assert!(error["message"].as_str().unwrap().contains("resources/list"));
    }

    #[test]
    fn request_missing_method_is_invalid_request() {
        let line = handle_message(r#"{"jsonrpc":"2.0","id":4,"params":{}}"#).expect("responds");
        let (_, id, _, error) = parts(&line);
        assert_eq!(id, Some(json!(4)));
        assert_eq!(error.expect("error object")["code"], -32600, "{line}");
    }

    #[test]
    fn methodless_idless_object_gets_no_response() {
        // Without a method AND without an id there is nothing to route and
        // nobody to reply to — silence, not a broadcast error.
        assert_eq!(handle_message(r#"{"jsonrpc":"2.0"}"#), None);
    }

    #[test]
    fn non_object_json_is_invalid_request_with_null_id() {
        for not_a_request in ["5", "\"hi\"", "[1,2,3]", "true"] {
            let line = handle_message(not_a_request).expect("responds");
            let (_, id, _, error) = parts(&line);
            assert_eq!(id, Some(Value::Null));
            assert_eq!(error.expect("error object")["code"], -32600, "{line}");
        }
    }

    #[test]
    fn tools_call_unknown_tool_is_invalid_params() {
        let line = handle_message(
            r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"bash","arguments":{}}}"#,
        )
        .expect("responds");
        let (_, id, _, error) = parts(&line);
        assert_eq!(id, Some(json!(5)));
        let error = error.expect("error object");
        assert_eq!(error["code"], -32602, "{line}");
        assert!(error["message"].as_str().unwrap().contains("bash"));
    }

    #[test]
    fn tools_call_missing_name_is_invalid_params() {
        let line = handle_message(
            r#"{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{}}"#,
        )
        .expect("responds");
        let (_, _, _, error) = parts(&line);
        assert_eq!(error.expect("error object")["code"], -32602, "{line}");
    }

    // ---------- chug_status ----------

    /// A synthetic latest-segment events fixture: run_start + a few
    /// iterations + a goal line.
    fn write_fixture(cwd: &Path, lines: &[&str]) {
        std::fs::create_dir_all(cwd.join(".chug")).unwrap();
        let body: String = lines.iter().map(|l| format!("{l}\n")).collect();
        std::fs::write(cwd.join(".chug/events.jsonl"), body).unwrap();
    }

    /// Borrow a `Vec<String>` fixture as the `&[&str]` `write_fixture`
    /// takes (the same ref-map the `read_events` seam does).
    fn as_str_refs(lines: &[String]) -> Vec<&str> {
        lines.iter().map(String::as_str).collect()
    }

    fn run_then_iterations_then_goal() -> Vec<String> {
        [
            r#"{"type":"run_start","ts":"t0","mode":"run","model":"m","max_iters":50,"max_minutes":35,"max_tokens":null}"#,
            r#"{"type":"iteration","ts":"t1","n":3,"input_tokens":1,"output_tokens":1}"#,
            r#"{"type":"iteration","ts":"t2","n":4,"input_tokens":1,"output_tokens":1}"#,
            r#"{"type":"tool_result","ts":"t2b","tool":"bash","ok":true}"#,
            r#"{"type":"goal","ts":"t3","outcome":"accepted","summary":"did the thing"}"#,
        ]
        .into_iter()
        .map(str::to_string)
        .collect()
    }

    #[test]
    fn chug_status_happy_path_names_state_and_iteration_counts() {
        let tmp = tempfile::tempdir().unwrap();
        write_fixture(tmp.path(), &as_str_refs(&run_then_iterations_then_goal()));
        let (text, is_error) = chug_status(&json!({ "cwd": tmp.path().display().to_string() }));
        assert!(!is_error, "{text}");
        // State + iteration counts present and correct.
        assert!(text.contains("state: done"), "{text}");
        assert!(text.contains("iteration: 4/50"), "{text}");
        assert!(text.contains("goal_seen: true"), "{text}");
        assert!(text.contains("abort_seen: false"), "{text}");
        assert!(text.contains("budget_low_seen: false"), "{text}");
        // Self-describing: names the cwd and the events file it read.
        assert!(text.contains(&tmp.path().display().to_string()), "{text}");
        assert!(text.contains(".chug/events.jsonl"), "{text}");
    }

    #[test]
    fn chug_status_renders_abort_reason_and_budget_low_flags() {
        let tmp = tempfile::tempdir().unwrap();
        write_fixture(
            tmp.path(),
            &[
                r#"{"type":"run_start","ts":"t0","max_iters":10}"#,
                r#"{"type":"iteration","ts":"t1","n":9}"#,
                r#"{"type":"budget_low","ts":"t2","budget_kind":"iterations","budget_max":10}"#,
                r#"{"type":"abort","ts":"t3","reason":"iteration budget exhausted","model":"m"}"#,
            ],
        );
        let (text, is_error) = chug_status(&json!({ "cwd": tmp.path().display().to_string() }));
        assert!(!is_error, "{text}");
        assert!(text.contains("state: aborted"), "{text}");
        assert!(text.contains("iteration: 9/10"), "{text}");
        assert!(text.contains("budget_low_seen: true"), "{text}");
        assert!(text.contains("abort_seen: true"), "{text}");
        assert!(text.contains("abort_reason: iteration budget exhausted"), "{text}");
    }

    #[test]
    fn chug_status_segment_reset_reports_the_latest_segment() {
        // T58 semantics ride the shared seam: a resumed child's fresh
        // run_start resets the verdict latches — the summary describes the
        // LATEST segment, not the pre-resume abort.
        let tmp = tempfile::tempdir().unwrap();
        let lines: Vec<String> = [
            r#"{"type":"run_start","ts":"t0","max_iters":40}"#,
            r#"{"type":"abort","ts":"t1","reason":"old death"}"#,
            r#"{"type":"run_start","ts":"t2","max_iters":20}"#,
            r#"{"type":"iteration","ts":"t3","n":1}"#,
        ]
        .into_iter()
        .map(str::to_string)
        .collect();
        write_fixture(tmp.path(), &as_str_refs(&lines));
        let (text, is_error) = chug_status(&json!({ "cwd": tmp.path().display().to_string() }));
        assert!(!is_error, "{text}");
        assert!(text.contains("state: running"), "{text}");
        assert!(text.contains("iteration: 1/20"), "{text}");
        assert!(text.contains("abort_seen: false"), "{text}");
        assert!(!text.contains("old death"), "{text}");
    }

    #[test]
    fn chug_status_with_no_events_seen_reports_starting_state() {
        // A `.chug/events.jsonl` that exists but is empty: a successful read
        // of nothing — not an error.
        let tmp = tempfile::tempdir().unwrap();
        write_fixture(tmp.path(), &[]);
        let (text, is_error) = chug_status(&json!({ "cwd": tmp.path().display().to_string() }));
        assert!(!is_error, "{text}");
        assert!(text.contains("state: starting"), "{text}");
        assert!(text.contains("iteration: none/-"), "{text}");
    }

    #[test]
    fn chug_status_missing_chug_dir_names_received_path() {
        let tmp = tempfile::tempdir().unwrap();
        let (text, is_error) = chug_status(&json!({ "cwd": tmp.path().display().to_string() }));
        assert!(is_error, "{text}");
        assert!(
            text.contains(&tmp.path().display().to_string()),
            "names the received path verbatim: {text}"
        );
        assert!(text.contains(".chug"), "{text}");
        // T130 (the T124 M3 survivor): the three assertions above are ALL
        // substring-satisfied by the downstream events-unreadable message
        // (it embeds `<tmp>/.chug/events.jsonl`, is an error, and contains
        // the literal `.chug`), so deleting the `.chug/`-existence check
        // used to survive. The DISTINCTIVE phrase of this leg's own
        // message — plus the negative guard against the shadow message —
        // makes the mutant die.
        assert!(text.contains("no .chug/ directory in"), "{text}");
        assert!(
            !text.contains("events file unreadable"),
            "must be the missing-dir leg, not the shadowing events-unreadable one: {text}"
        );
    }

    #[test]
    fn chug_status_missing_events_file_names_the_path() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".chug")).unwrap();
        let (text, is_error) = chug_status(&json!({ "cwd": tmp.path().display().to_string() }));
        assert!(is_error, "{text}");
        let events = tmp.path().join(".chug/events.jsonl");
        assert!(
            text.contains(events.display().to_string().as_str()),
            "names the events path: {text}"
        );
        // T130 sweep: the DISTINCTIVE phrase of this leg's own message (a
        // reworded message used to survive — the path assertion alone is
        // message-text-blind), plus the sibling-shadow guard: `.chug/`
        // EXISTS in this fixture, so the error must be the events one.
        assert!(text.contains("events file unreadable"), "{text}");
        assert!(
            !text.contains("no .chug/ directory in"),
            "must be the events-unreadable leg, not the shadowing missing-dir one: {text}"
        );
    }

    #[test]
    fn chug_status_relative_cwd_is_refused_naming_the_received_string() {
        let (text, is_error) = chug_status(&json!({ "cwd": "some/relative/path" }));
        assert!(is_error, "{text}");
        // The delegate-launch fail-fast shape: the RAW received string,
        // quoted, not a silently-resolved path.
        assert!(text.contains("\"some/relative/path\""), "{text}");
        // T130 sweep: the DISTINCTIVE phrase of this leg's own message
        // (a reworded message used to survive — the quoted-raw assertion
        // is message-text-blind), plus the fall-through shadow guard:
        // with the absolute-check deleted the nonexistent-dir leg fires,
        // and its display() output is unquoted.
        assert!(text.contains("must be an absolute directory"), "{text}");
        assert!(
            !text.contains("does not exist or is not a directory"),
            "must be the relative-cwd leg, not the fall-through nonexistent-dir shadow: {text}"
        );
    }

    #[test]
    fn chug_status_nonexistent_cwd_is_refused_naming_the_path() {
        let bogus = "/definitely/not/a/chug/cwd-t124";
        let (text, is_error) = chug_status(&json!({ "cwd": bogus }));
        assert!(is_error, "{text}");
        assert!(text.contains(bogus), "{text}");
        // T130 sweep (the M3 shadow class again): the no-`.chug/` message
        // for the SAME bogus path also contains it verbatim, so deleting
        // the is_dir() check used to survive on the path assertion alone.
        assert!(
            text.contains("does not exist or is not a directory"),
            "{text}"
        );
        assert!(
            !text.contains("no .chug/ directory in"),
            "must be the nonexistent-cwd leg, not the shadowing missing-dir one: {text}"
        );
    }

    #[test]
    fn chug_status_missing_cwd_argument_is_an_error_result() {
        let (text, is_error) = chug_status(&json!({}));
        assert!(is_error, "{text}");
        assert!(text.contains("cwd"), "{text}");
        // T130 sweep: "cwd" alone is substring-satisfied by the
        // relative-cwd message a neutered guard falls through to — pin
        // the DISTINCTIVE phrase and guard against that shadow.
        assert!(text.contains("missing required argument: cwd"), "{text}");
        assert!(
            !text.contains("must be an absolute directory"),
            "must be the missing-argument leg, not the fall-through relative-cwd shadow: {text}"
        );
        // And via the full protocol path it is a tool RESULT, not a
        // JSON-RPC error (the tool ran; its input was bad).
        let line = handle_message(
            r#"{"jsonrpc":"2.0","id":8,"method":"tools/call","params":{"name":"chug_status","arguments":{}}}"#,
        )
        .expect("responds");
        let (_, _, _, error) = parts(&line);
        assert!(error.is_none(), "tool failure is a result, not a JSON-RPC error: {line}");
        let (text2, is_error2) = tool_result(&line);
        assert_eq!(is_error2, Some(true), "{text2}");
    }

    #[test]
    fn chug_status_non_string_cwd_is_an_error_result() {
        let (text, is_error) = chug_status(&json!({ "cwd": 17 }));
        assert!(is_error, "{text}");
        assert!(text.contains("cwd"), "{text}");
        // T130 sweep: same guard as the missing-argument leg (as_str()
        // yields None either way) — pin its DISTINCTIVE phrase and guard
        // against the same relative-cwd fall-through shadow.
        assert!(text.contains("missing required argument: cwd"), "{text}");
        assert!(
            !text.contains("must be an absolute directory"),
            "must be the missing-argument leg, not the fall-through relative-cwd shadow: {text}"
        );
    }

    #[test]
    fn chug_status_malformed_final_line_degrades_best_effort_without_panicking() {
        // The torn-final-write case: the good lines still summarize; the
        // garbage line is skipped (the shared summarize_events contract).
        let tmp = tempfile::tempdir().unwrap();
        let mut body = String::new();
        for line in run_then_iterations_then_goal() {
            body.push_str(&line);
            body.push('\n');
        }
        body.push_str("{\"type\":\"iteration\",\"ts\":\"t4\",\"n\":"); // torn
        std::fs::create_dir_all(tmp.path().join(".chug")).unwrap();
        std::fs::write(tmp.path().join(".chug/events.jsonl"), body).unwrap();
        let (text, is_error) = chug_status(&json!({ "cwd": tmp.path().display().to_string() }));
        assert!(!is_error, "best-effort summary, not an error: {text}");
        assert!(text.contains("state: done"), "{text}");
        assert!(text.contains("iteration: 4/50"), "{text}");
    }

    // ---------- chug_collect (T128) ----------

    /// A synthetic latest-segment events fixture in the collect shape:
    /// run_start + a verifying gate + an accepted goal.
    fn collect_fixture(cwd: &Path) {
        write_fixture(
            cwd,
            &[
                r#"{"type":"run_start","ts":"t0","mode":"run","model":"m","max_iters":50}"#,
                r#"{"type":"verifying","ts":"t2","cmd":"cargo test"}"#,
                r#"{"type":"goal","ts":"t3","outcome":"accepted","summary":"did the thing, verified"}"#,
            ],
        );
    }

    #[test]
    fn chug_collect_happy_path_names_verdict_summary_check_cmd_and_degraded_commits() {
        let tmp = tempfile::tempdir().unwrap();
        collect_fixture(tmp.path());
        let (text, is_error) = chug_collect(&json!({ "cwd": tmp.path().display().to_string() }));
        assert!(!is_error, "{text}");
        assert!(text.contains("verdict: goal-accepted"), "{text}");
        assert!(text.contains("summary: did the thing, verified"), "{text}");
        assert!(text.contains("check_cmd: cargo test"), "{text}");
        // Self-describing: names the cwd and the events file it read.
        assert!(text.contains(&tmp.path().display().to_string()), "{text}");
        assert!(text.contains(".chug/events.jsonl"), "{text}");
        // Not-a-repo tempdir: the git leg DEGRADES to a note, never fails
        // the call (the delegate collect rule, mirrored by this renderer).
        assert!(text.contains("commits: (unavailable:"), "{text}");
    }

    #[test]
    fn chug_collect_aborted_segment_renders_verdict_with_reason() {
        let tmp = tempfile::tempdir().unwrap();
        write_fixture(
            tmp.path(),
            &[
                r#"{"type":"run_start","ts":"t0","max_iters":10}"#,
                r#"{"type":"iteration","ts":"t1","n":9}"#,
                r#"{"type":"abort","ts":"t2","reason":"iteration budget exhausted","model":"m"}"#,
            ],
        );
        let (text, is_error) = chug_collect(&json!({ "cwd": tmp.path().display().to_string() }));
        assert!(!is_error, "{text}");
        assert!(text.contains("verdict: aborted"), "{text}");
        assert!(text.contains("abort_reason: iteration budget exhausted"), "{text}");
        // No accepted verdict → no summary and no check_cmd lines.
        assert!(!text.contains("summary:"), "{text}");
        assert!(!text.contains("check_cmd:"), "{text}");
    }

    #[test]
    fn chug_collect_missing_events_file_names_the_path() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".chug")).unwrap();
        let (text, is_error) = chug_collect(&json!({ "cwd": tmp.path().display().to_string() }));
        assert!(is_error, "{text}");
        let events = tmp.path().join(".chug/events.jsonl");
        assert!(
            text.contains(events.display().to_string().as_str()),
            "names the events path: {text}"
        );
        assert!(text.contains("events file unreadable"), "{text}");
    }

    #[test]
    fn chug_collect_relative_cwd_is_refused_naming_the_received_string() {
        // One leg per violation class on the SHARED validator (the
        // nonexistent-cwd class is pinned chug_status-side; both tools run
        // the same validator, so together the matrices cover it).
        let (text, is_error) = chug_collect(&json!({ "cwd": "some/relative/path" }));
        assert!(is_error, "{text}");
        assert!(text.contains("\"some/relative/path\""), "{text}");
        assert!(text.contains("must be an absolute directory"), "{text}");
    }

    #[test]
    fn chug_collect_missing_chug_dir_is_refused_naming_the_path() {
        let tmp = tempfile::tempdir().unwrap();
        let (text, is_error) = chug_collect(&json!({ "cwd": tmp.path().display().to_string() }));
        assert!(is_error, "{text}");
        assert!(text.contains(&tmp.path().display().to_string()), "{text}");
        assert!(text.contains("no .chug/ directory in"), "{text}");
    }

    #[test]
    fn chug_collect_missing_cwd_argument_is_an_error_result() {
        let (text, is_error) = chug_collect(&json!({}));
        assert!(is_error, "{text}");
        assert!(text.contains("missing required argument: cwd"), "{text}");
        assert!(text.contains(CHUG_COLLECT_TOOL), "{text}");
    }

    #[test]
    fn chug_collect_non_string_base_is_an_error_result() {
        // The T69 rule on the new tool: a caller that asked for a range must
        // not silently get the default range.
        let tmp = tempfile::tempdir().unwrap();
        collect_fixture(tmp.path());
        let (text, is_error) = chug_collect(&json!({
            "cwd": tmp.path().display().to_string(), "base": 17
        }));
        assert!(is_error, "{text}");
        assert!(text.contains("`base` must be a string git ref"), "{text}");
        assert!(!text.contains("commits"), "no silent default-range fallback: {text}");
    }

    #[test]
    fn chug_collect_pid_renders_liveness_line_absent_pid_renders_none() {
        let tmp = tempfile::tempdir().unwrap();
        collect_fixture(tmp.path());
        // Absent pid → NO liveness claim at all (the delegate collect rule).
        let (text, is_error) = chug_collect(&json!({ "cwd": tmp.path().display().to_string() }));
        assert!(!is_error, "{text}");
        assert!(!text.contains("alive:"), "{text}");
        // A pid that cannot exist (macOS caps pids far below this; Linux's
        // pid_max caps at 2^22) probes dead — the liveness line renders.
        let (with_pid, is_error) = chug_collect(&json!({
            "cwd": tmp.path().display().to_string(), "pid": 2_000_000_000u64
        }));
        assert!(!is_error, "{with_pid}");
        assert!(with_pid.contains("alive: false"), "{with_pid}");
    }

    #[test]
    fn chug_collect_base_is_forwarded_into_the_commit_range_seam() {
        // A real git fixture: the distinguishing observable is WHICH commits
        // the spawn returned — `<first>..HEAD` lists only the second, so a
        // mutant that dropped the forward (and silently queried the default
        // range) lists both and dies. The git plumbing itself stays pinned
        // delegate-side (delegate_collect_git_legs_refs_base_and_degrades).
        let tmp = tempfile::tempdir().unwrap();
        let git = |args: &[&str]| {
            let out = std::process::Command::new("git")
                .args(args)
                .current_dir(tmp.path())
                .output()
                .expect("git available for the integration pin");
            assert!(
                out.status.success(),
                "git {args:?} failed: {}",
                String::from_utf8_lossy(&out.stderr)
            );
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        };
        git(&["init", "-q"]);
        git(&["-c", "user.email=t@t", "-c", "user.name=t", "commit", "--allow-empty", "-qm", "first"]);
        let first = git(&["rev-parse", "--short", "HEAD"]);
        git(&["-c", "user.email=t@t", "-c", "user.name=t", "commit", "--allow-empty", "-qm", "second"]);
        collect_fixture(tmp.path());

        // Absent base: the bounded default range over HEAD lists both.
        let (text, is_error) = chug_collect(&json!({ "cwd": tmp.path().display().to_string() }));
        assert!(!is_error, "{text}");
        assert!(text.contains("commits (range HEAD, up to 20):"), "{text}");
        assert!(text.contains(" second"), "{text}");
        assert!(text.contains(" first"), "{text}");

        // base = the first commit: `<first>..HEAD` lists ONLY the second —
        // the forwarded range reached the real git spawn.
        let (scoped, is_error) = chug_collect(&json!({
            "cwd": tmp.path().display().to_string(), "base": first
        }));
        assert!(!is_error, "{scoped}");
        assert!(
            scoped.contains(&format!("commits (range {first}..HEAD, up to 20):")),
            "{scoped}"
        );
        assert!(scoped.contains(" second"), "{scoped}");
        assert!(
            !scoped.contains(&format!("{first} first")),
            "the first commit is outside the forwarded range: {scoped}"
        );
    }

    // ---------- chug_launch (T129) ----------

    /// Route a `chug_launch` tools/call through the FULL dispatch with the
    /// given flag, returning (text, isError) — the same shape the wire
    /// serves. A routed call must always be a tool RESULT (a JSON-RPC error
    /// would mean the tool was not routable at all).
    fn launch_call(allow_launch: bool, arguments: &Value) -> (String, Option<bool>) {
        let req = json!({
            "jsonrpc": "2.0", "id": 129, "method": "tools/call",
            "params": {"name": "chug_launch", "arguments": arguments}
        });
        let line = handle_message_with(&req.to_string(), allow_launch).expect("responds");
        let (_, id, _, error) = parts(&line);
        assert_eq!(id, Some(json!(129)), "id echoed: {line}");
        assert!(
            error.is_none(),
            "a routed launch call is a tool result, not a JSON-RPC error: {line}"
        );
        tool_result(&line)
    }

    /// A real spec file in `dir` (the launch spec probe needs one).
    fn write_launch_spec(dir: &Path) -> PathBuf {
        let spec = dir.join("t129-spec.md");
        fs::write(&spec, "# t129 launch-matrix spec\n").unwrap();
        spec
    }

    /// The launch-matrix payload with every required param valid.
    fn full_launch_args(cwd: &Path, spec: &Path) -> Value {
        json!({
            "cwd": cwd.display().to_string(),
            "spec": spec.display().to_string(),
            "goal": "t129 matrix goal",
            "model": "matrix-model",
        })
    }

    #[test]
    fn chug_launch_flagless_call_is_the_unknown_tool_error() {
        // Without the flag the write leg DOES NOT EXIST: the unknown-tool
        // error, the same -32602 a never-existing tool gets — a read-only
        // deployment cannot be probed into revealing a hidden tool.
        let line = handle_message(
            r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"chug_launch","arguments":{}}}"#,
        )
        .expect("responds");
        let (_, id, _, error) = parts(&line);
        assert_eq!(id, Some(json!(5)), "{line}");
        let error = error.expect("error object");
        assert_eq!(error["code"], -32602, "{line}");
        let message = error["message"].as_str().unwrap();
        assert!(message.contains("chug_launch"), "{line}");
        assert!(message.contains("unknown tool"), "{line}");
        // And the flagless tools/list does not advertise it — the read-only
        // tool set is byte-identical to pre-T129.
        let list = handle_message(r#"{"jsonrpc":"2.0","id":6,"method":"tools/list"}"#)
            .expect("responds");
        let (_, _, result, _) = parts(&list);
        let tools = result.expect("result")["tools"]
            .as_array()
            .expect("tools array")
            .clone();
        let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
        assert_eq!(names, ["chug_status", "chug_collect"], "{list}");
    }

    #[test]
    fn chug_launch_flag_on_advertised_with_the_exact_schema() {
        let line = handle_message_with(r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#, true)
            .expect("responds");
        let (_, _, result, _) = parts(&line);
        let tools = result.expect("result")["tools"]
            .as_array()
            .expect("tools array")
            .clone();
        assert_eq!(tools.len(), 3, "two read-only tools + the write leg: {tools:?}");
        assert_eq!(tools[0]["name"], "chug_status");
        assert_eq!(tools[1]["name"], "chug_collect");
        assert_eq!(tools[2]["name"], "chug_launch");
        let schema = &tools[2]["inputSchema"];
        // The required list is EXACT: cwd, spec, goal, model.
        assert_eq!(schema["required"], json!(["cwd", "spec", "goal", "model"]));
        let props = schema["properties"].as_object().expect("properties object");
        assert_eq!(props.len(), 6, "cwd + spec + goal + model + two budgets: {props:?}");
        for key in ["cwd", "spec", "goal", "model"] {
            assert_eq!(props[key]["type"], "string", "{props:?}");
        }
        // The loopd ceilings are IN the schema: 1..=200 and 1..=240.
        assert_eq!(props["max_iters"]["type"], "integer", "{props:?}");
        assert_eq!(props["max_iters"]["minimum"], 1, "{props:?}");
        assert_eq!(props["max_iters"]["maximum"], 200, "{props:?}");
        assert_eq!(props["max_minutes"]["type"], "integer", "{props:?}");
        assert_eq!(props["max_minutes"]["minimum"], 1, "{props:?}");
        assert_eq!(props["max_minutes"]["maximum"], 240, "{props:?}");
    }

    #[test]
    fn chug_launch_flag_on_dispatch_is_a_tool_result_not_unknown_tool() {
        // Under the flag the tool is ROUTABLE: a validation failure is an
        // isError RESULT (the tool ran; the input was bad) — the same call
        // without the flag is the -32602 pinned above. Advertised ⇔
        // callable, both directions.
        let (text, is_error) = launch_call(true, &json!({ "cwd": "relative/cwd" }));
        assert_eq!(is_error, Some(true), "{text}");
    }

    #[test]
    fn chug_launch_missing_required_arguments_are_is_error_results() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".chug")).unwrap();
        let spec = write_launch_spec(tmp.path());
        // Each required param absent → an isError naming the argument.
        for (args, key) in [
            (json!({}), "cwd"),
            (json!({ "cwd": tmp.path() }), "spec"),
            (
                json!({ "cwd": tmp.path(), "spec": spec.display().to_string() }),
                "goal",
            ),
            (
                json!({
                    "cwd": tmp.path(),
                    "spec": spec.display().to_string(),
                    "goal": "g"
                }),
                "model",
            ),
        ] {
            let (text, is_error) = launch_call(true, &args);
            assert_eq!(is_error, Some(true), "{text}");
            assert!(
                text.contains(&format!("missing required argument: {key}")),
                "{key}: {text}"
            );
        }
    }

    #[test]
    fn chug_launch_relative_cwd_is_refused_naming_the_received_string() {
        let tmp = tempfile::tempdir().unwrap();
        let spec = write_launch_spec(tmp.path());
        let (text, is_error) = launch_call(
            true,
            &json!({
                "cwd": "relative/cwd",
                "spec": spec.display().to_string(),
                "goal": "g",
                "model": "m"
            }),
        );
        assert_eq!(is_error, Some(true), "{text}");
        // The SAME validator and error-text parity as chug_status/chug_collect:
        // the RAW received string, quoted.
        assert!(text.contains("\"relative/cwd\""), "{text}");
        assert!(text.contains("must be an absolute directory"), "{text}");
        assert!(text.starts_with("chug_launch:"), "tool-named error: {text}");
    }

    #[test]
    fn chug_launch_cwd_without_chug_dir_is_refused_naming_the_path() {
        let empty = tempfile::tempdir().unwrap(); // exists, but no .chug/
        let scratch = tempfile::tempdir().unwrap();
        let spec = write_launch_spec(scratch.path());
        let (text, is_error) = launch_call(true, &full_launch_args(empty.path(), &spec));
        assert_eq!(is_error, Some(true), "{text}");
        assert!(text.contains(&empty.path().display().to_string()), "{text}");
        assert!(text.contains("no .chug/ directory in"), "{text}");
    }

    #[test]
    fn chug_launch_nonexistent_spec_is_refused_naming_the_path() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".chug")).unwrap();
        let bogus = "/definitely/not/t129-spec.md";
        let (text, is_error) = launch_call(
            true,
            &json!({
                "cwd": tmp.path().display().to_string(),
                "spec": bogus,
                "goal": "g",
                "model": "m"
            }),
        );
        assert_eq!(is_error, Some(true), "{text}");
        assert!(text.contains(bogus), "names the received path: {text}");
        assert!(
            text.contains("spec does not exist or is not a readable file"),
            "{text}"
        );
    }

    #[test]
    fn chug_launch_relative_spec_is_refused_naming_the_received_string() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".chug")).unwrap();
        let (text, is_error) = launch_call(
            true,
            &json!({
                "cwd": tmp.path().display().to_string(),
                "spec": "t129-relative-spec.md",
                "goal": "g",
                "model": "m"
            }),
        );
        assert_eq!(is_error, Some(true), "{text}");
        assert!(text.contains("\"t129-relative-spec.md\""), "{text}");
        assert!(text.contains("spec must be an absolute path"), "{text}");
    }

    #[test]
    fn chug_launch_empty_and_whitespace_goals_are_refused() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".chug")).unwrap();
        let spec = write_launch_spec(tmp.path());
        for goal in ["", "   \t "] {
            let (text, is_error) = launch_call(
                true,
                &json!({
                    "cwd": tmp.path().display().to_string(),
                    "spec": spec.display().to_string(),
                    "goal": goal,
                    "model": "m"
                }),
            );
            assert_eq!(is_error, Some(true), "{goal:?}: {text}");
            assert!(
                text.contains("goal must be a non-empty string after trim"),
                "{goal:?}: {text}"
            );
            // Validation precedes the spawn seam: a refused payload must not
            // have launched anything (no delegate.log was opened).
            assert!(
                !tmp.path().join(".chug/delegate.log").exists(),
                "a refused goal must not spawn: {goal:?}"
            );
        }
    }

    #[test]
    fn chug_launch_empty_model_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".chug")).unwrap();
        let spec = write_launch_spec(tmp.path());
        let (text, is_error) = launch_call(
            true,
            &json!({
                "cwd": tmp.path().display().to_string(),
                "spec": spec.display().to_string(),
                "goal": "g",
                "model": ""
            }),
        );
        assert_eq!(is_error, Some(true), "{text}");
        assert!(text.contains("model must be a non-empty string"), "{text}");
        assert!(!tmp.path().join(".chug/delegate.log").exists(), "no spawn: {text}");
    }

    #[test]
    fn chug_launch_budgets_above_the_ceilings_are_refused_naming_the_ceiling() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".chug")).unwrap();
        let spec = write_launch_spec(tmp.path());
        // 201 > 200: REJECTED (not clamped), naming the received value and
        // the 200 ceiling (loopd's own).
        let (text, is_error) = launch_call(
            true,
            &json!({
                "cwd": tmp.path().display().to_string(),
                "spec": spec.display().to_string(),
                "goal": "g",
                "model": "m",
                "max_iters": 201
            }),
        );
        assert_eq!(is_error, Some(true), "{text}");
        assert!(text.contains("`max_iters` must be at most 200"), "{text}");
        assert!(text.contains("got 201"), "{text}");
        // 241 > 240: the same reject-above semantics.
        let (text, is_error) = launch_call(
            true,
            &json!({
                "cwd": tmp.path().display().to_string(),
                "spec": spec.display().to_string(),
                "goal": "g",
                "model": "m",
                "max_minutes": 241
            }),
        );
        assert_eq!(is_error, Some(true), "{text}");
        assert!(text.contains("`max_minutes` must be at most 240"), "{text}");
        assert!(text.contains("got 241"), "{text}");
        // Neither refusal spawned anything.
        assert!(!tmp.path().join(".chug/delegate.log").exists(), "no spawn: {text}");
    }

    #[test]
    fn chug_launch_budgets_parse_reject_above_accept_boundary_and_default_to_none() {
        // Boundary legs 200/240 ACCEPTED (at the validation layer — the
        // stub-spawn leg covers the full path with these values).
        let parsed = parse_launch_budgets(&json!({ "max_iters": 200, "max_minutes": 240 }))
            .expect("boundary values accepted");
        assert_eq!(parsed, (Some(200), Some(240)));
        // Reject-above names the received value and the ceiling.
        let err = parse_launch_budgets(&json!({ "max_iters": 201 })).unwrap_err();
        assert!(err.contains("max_iters") && err.contains("at most 200") && err.contains("201"), "{err}");
        let err = parse_launch_budgets(&json!({ "max_minutes": 241 })).unwrap_err();
        assert!(err.contains("max_minutes") && err.contains("at most 240") && err.contains("241"), "{err}");
        // Below 1 and non-integers are refused too — never clamped, never
        // silently defaulted.
        for bad in [json!(0), json!(-5), json!("200"), json!(7.5), json!(true)] {
            let err = parse_launch_budgets(&json!({ "max_iters": bad })).unwrap_err();
            assert!(err.contains("max_iters"), "{bad}: {err}");
        }
        // Absent (or null) → None: the delegate defaults (40/35) apply
        // seam-side, exactly like a flagless human launch.
        assert_eq!(parse_launch_budgets(&json!({})).unwrap(), (None, None));
        assert_eq!(
            parse_launch_budgets(&json!({ "max_iters": null, "max_minutes": null })).unwrap(),
            (None, None)
        );
    }

    /// Poll (deadline-bounded) for the stub's atomically-published dump —
    /// launch returns at spawn, so the dump lands milliseconds later.
    #[cfg(unix)]
    fn wait_for_stub_dump(path: &Path) -> Vec<String> {
        use std::time::{Duration, Instant};
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Ok(text) = fs::read_to_string(path) {
                let dump: Vec<String> = text.lines().map(str::to_string).collect();
                if !dump.is_empty() {
                    return dump;
                }
            }
            assert!(
                Instant::now() < deadline,
                "stub never wrote {}",
                path.display()
            );
            std::thread::sleep(Duration::from_millis(25));
        }
    }

    /// The stub-spawn leg: `CHUG_DELEGATE_BIN` pointed at a tiny shell stub
    /// (the T126 idiom — written in the test's tempdir, records its argv and
    /// cwd, sleeps as a fake child) that the launch seam substitutes for the
    /// real binary. Pins the EXACT child argv order/values, the child's cwd,
    /// the boundary budgets (200/240) reaching the argv verbatim, and that
    /// the returned pid/log/events name real paths. The stub is killed at
    /// leg end. The goal carries spaces and double quotes — the delegate
    /// argv mechanism delivers it byte-exact with no quoting code.
    #[cfg(unix)]
    #[test]
    fn chug_launch_stub_spawn_pins_exact_argv_cwd_and_return_paths() {
        use std::os::unix::fs::PermissionsExt;
        use std::time::Duration;
        // The env var is process-global and the delegate tests mutate it too
        // (same test binary) — one env, one lock.
        let _guard = crate::delegate::tests::DELEGATE_ENV_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let target = tempfile::tempdir().unwrap();
        let scratch = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(target.path().join(".chug")).unwrap();
        let spec = write_launch_spec(scratch.path());
        let stub = scratch.path().join("chug-launch-stub.sh");
        std::fs::write(
            &stub,
            concat!(
                "#!/bin/sh\n",
                "pwd -P > cwd.txt\n",
                "printf '%s\\n' \"$@\" > argv.tmp && mv argv.tmp argv.txt\n",
                ": > .chug/events.jsonl\n",
                "sleep 60\n",
            ),
        )
        .unwrap();
        std::fs::set_permissions(&stub, fs::Permissions::from_mode(0o755)).unwrap();
        // SAFETY: serialized by the delegate env lock; removed before return.
        unsafe { std::env::set_var("CHUG_DELEGATE_BIN", &stub) };
        let goal = "t129 goal with spaces and \"quotes\"";
        let (text, is_error) = launch_call(
            true,
            &json!({
                "cwd": target.path().display().to_string(),
                "spec": spec.display().to_string(),
                "goal": goal,
                "model": "stub-model",
                "max_iters": 200,
                "max_minutes": 240,
            }),
        );
        assert_eq!(is_error, Some(false), "{text}");
        // The return shape mirrors delegate launch: pid, log, events.
        let pid: u32 = text
            .lines()
            .find_map(|l| l.strip_prefix("launched: pid "))
            .expect("pid line in launch result")
            .trim()
            .parse()
            .expect("pid parses");
        let log_path = target.path().join(".chug/delegate.log");
        let events_path = target.path().join(".chug/events.jsonl");
        assert!(text.contains(&format!("log: {}", log_path.display())), "{text}");
        assert!(text.contains(&format!("events: {}", events_path.display())), "{text}");
        // Real paths: the log exists (the parent opened it for the child's
        // stdout+stderr before spawn).
        assert!(log_path.is_file(), "log must exist: {text}");
        // The EXACT child argv, in order, with the boundary budgets verbatim.
        let argv = wait_for_stub_dump(&target.path().join("argv.txt"));
        let expected = [
            "run",
            "--spec",
            spec.to_str().unwrap(),
            "--goal",
            goal,
            "--model",
            "stub-model",
            "--max-iters",
            "200",
            "--max-minutes",
            "240",
        ];
        assert_eq!(argv, expected, "exact child argv");
        // The child ran IN the target cwd: `pwd -P` names it, and its
        // relative events touch landed in that cwd's .chug/.
        let cwd_txt = fs::read_to_string(target.path().join("cwd.txt")).expect("cwd.txt");
        let expected_cwd = fs::canonicalize(target.path()).unwrap();
        assert_eq!(cwd_txt.trim(), expected_cwd.to_str().unwrap(), "child cwd");
        assert!(events_path.is_file(), "events file created by the stub: {text}");
        // Reap/kill the stub at leg end (its own process group), then
        // restore the seam.
        crate::tools::kill_pid_group(pid);
        std::thread::sleep(Duration::from_millis(50));
        // SAFETY: serialized by the delegate env lock.
        unsafe { std::env::remove_var("CHUG_DELEGATE_BIN") };
    }

    // ---------- structural stdout purity ----------

    /// The subcommand's code path must never call the banner / run_start
    /// writers, and must have no stdout writer except the protocol
    /// `writeln!`. Pinned by construction here (the module greps clean) AND
    /// by the dispatch shape in main.rs (`CliCommand::McpServe =>` goes
    /// straight to `mcp_serve::serve`, not through cmd_run/cmd_chat).
    ///
    /// The needles are built with `concat!` so this test's own source does
    /// not contain them — a literal needle would self-match and the grep
    /// could never pass.
    #[test]
    fn stdout_purity_module_has_no_stdout_writers_and_no_banner_calls() {
        let src = include_str!("../mcp_serve.rs");
        // No stdout writers anywhere — including inside tests (a test
        // println! is noise, but the grep is cheap and absolute).
        let println_needle = concat!("print", "ln!(");
        let print_needle = concat!("print", "!(");
        assert!(!src.contains(println_needle), "no stdout print allowed: the wire is stdout");
        assert!(!src.contains(print_needle), "no stdout print allowed: the wire is stdout");
        // The one protocol writer exists (the assertion keeps the grep from
        // passing vacuously after a rewrite of the I/O layer).
        let writeln_needle = concat!("write", "ln!(out, \"{response}\")");
        assert!(src.contains(writeln_needle), "protocol writer present");
        // The production half only: the test module below the `#[cfg(test)]`
        // boundary legitimately builds fixtures (tempdir mkdirs, doc-mention
        // needles), which are not the serve path. Everything up to the test
        // boundary must be free of the banner / events-writer / driver-lock
        // / filesystem-write surface `chug run` starts with.
        let prod = src
            .split("#[cfg(test)]")
            .next()
            .expect("the module always has a non-test half");
        let banner_needle = concat!("print_startup", "_banner");
        assert!(!prod.contains(banner_needle), "no banner call on the mcp-serve path");
        let run_start_needle = concat!("eventlog::", "run_start");
        assert!(!prod.contains(run_start_needle), "no run_start call on the mcp-serve path");
        let lock_needle = concat!("driver_", "lock");
        assert!(!prod.contains(lock_needle), "no driver lock on the mcp-serve path");
        let mkdir_needle = concat!("create_dir", "_all");
        let write_needle = concat!("fs::", "write(");
        let open_needle = concat!("Open", "Options");
        for needle in [mkdir_needle, write_needle, open_needle] {
            assert!(!prod.contains(needle), "phase 1 writes nothing ({needle})");
        }
    }

    // ---------- EOF / framing ----------

    #[test]
    fn serve_loop_returns_ok_on_immediate_eof() {
        // No lines at all → Ok (the "closed stdin exits cleanly" leg).
        let mut out: Vec<u8> = Vec::new();
        serve_from(&mut "".as_bytes(), &mut out).expect("EOF is a clean exit");
        assert!(out.is_empty(), "no output without input: {out:?}");
    }

    #[test]
    fn serve_loop_writes_one_line_per_request_and_skips_blank_lines_and_notifications() {
        let input = "\
{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{}}

{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}
{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"ping\"}
";
        let mut out: Vec<u8> = Vec::new();
        serve_from(&mut input.as_bytes(), &mut out).expect("EOF is a clean exit");
        let text = String::from_utf8(out).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2, "exactly the two request responses, one per line: {text}");
        let (_, id1, _, _) = parts(lines[0]);
        let (_, id2, _, _) = parts(lines[1]);
        assert_eq!(id1, Some(json!(1)), "initialize first");
        assert_eq!(id2, Some(json!(2)), "the notification produced no response");
    }

    #[test]
    fn serve_loop_keeps_reading_after_error_responses() {
        // Errors never kill the loop: garbage, then a valid ping, then EOF.
        let input = "\
{ not json !!!
{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"ping\"}
";
        let mut out: Vec<u8> = Vec::new();
        serve_from(&mut input.as_bytes(), &mut out).expect("keeps reading");
        let text = String::from_utf8(out).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2, "{text}");
        let (_, _, _, error) = parts(lines[0]);
        assert_eq!(error.expect("parse error")["code"], -32700);
        let (_, id, result, _) = parts(lines[1]);
        assert_eq!(id, Some(json!(3)));
        assert_eq!(result, Some(json!({})));
    }
