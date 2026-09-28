// T104 family: truncated — T38 truncated-response advisory (+T76 tgrep scripted run). Moved bytes byte-identical (T84 rule)
// from driver.rs's test module; every test here lives in exactly one family
// file.
    use super::*; // the shared harness (driver::tests) + driver's own imports
    // ---------- T38: truncated-response advisory ----------

    /// The advisory pinned as a LITERAL (not the const), so corrupting any
    /// load-bearing token of [`crate::driver::OUTPUT_TRUNCATED_ADVISORY`] —
    /// `max_tokens`, `stop_reason`, `write_file`, `edit_file` — fails here.
    const T38_ADVISORY: &str = "chug: output truncated — the previous response hit the API output-token ceiling (stop_reason=max_tokens). If you were writing a file, split it: write_file the first chunk, then append with edit_file (or bash heredoc) in smaller pieces.";

    /// A truncated response carrying one tool call — the T37 shape: a big
    /// `write_file` cut off by the output-token ceiling.
    fn truncated_tool_use_response(name: &str, input: Value) -> Value {
        json!({
            "stop_reason": "max_tokens",
            "usage": {"input_tokens": 10, "output_tokens": 8192},
            "content": [{"type": "tool_use", "id": "tu_1", "name": name, "input": input}],
        })
    }

    /// A truncated pure-text response (no tool calls at all).
    fn truncated_text_response(text: &str) -> Value {
        json!({
            "stop_reason": "max_tokens",
            "usage": {"input_tokens": 10, "output_tokens": 8192},
            "content": [{"type": "text", "text": text}],
        })
    }

    /// Drive one autonomous scripted run to an accepted goal, returning the
    /// LLM double, the recorded events, and the run's parsed events log.
    fn run_t38(
        tmp: &tempfile::TempDir,
        responses: Vec<Value>,
    ) -> (ScriptedLlm, Vec<Event>, Vec<Value>) {
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for(tmp, Mode::Autonomous, &controls, &urx, None, &observ::Sink::Noop);
        // 20 iterations: comfortably clear of WARN_REMAINING_ITERS (8), so
        // the T13 budget-low notice never fires inside these 2-3-call runs —
        // it would land after the advisory and break the last-message
        // position assertions (the advisory must be observable as the most
        // recent injection before the next call).
        let mut knobs = knobs_with(20);
        let mut llm = ScriptedLlm::new(responses);
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
        (llm, sink.0, events_jsonl(tmp))
    }

    fn advisory_count(messages: &[Message]) -> usize {
        messages
            .iter()
            .filter(|m| m.content.iter().any(|b| b.text() == Some(T38_ADVISORY)))
            .count()
    }

    /// T38: no advisory message, no event, no log line — the control for
    /// every non-truncated stop reason (`tool_use`, `end_turn`, absent).
    fn assert_untouched(llm: &ScriptedLlm, events: &[Event], lines: &[Value]) {
        for (_, seen) in &llm.calls {
            assert_eq!(
                advisory_count(seen),
                0,
                "no truncation advisory may reach the model"
            );
        }
        assert!(
            !events.iter().any(|e| matches!(e, Event::OutputTruncated)),
            "no OutputTruncated event expected"
        );
        assert!(
            !lines.iter().any(|l| l["type"] == "output_truncated"),
            "no output_truncated log line expected: {lines:?}"
        );
    }

    /// T76 integration: a scripted driver run where the model calls `tgrep`
    /// on a 300-hit corpus. The tool result the NEXT LLM call sees must stay
    /// under the requested budget and carry the omission marker.
    #[test]
    fn tgrep_scripted_run_stays_under_budget_and_marks_omissions() {
        let tmp = tempfile::tempdir().unwrap();
        let mut body = String::new();
        // 300 hits, one every 10 lines, windows never touching: 300 clusters.
        for i in 1..=3000 {
            let line = if i % 10 == 0 {
                format!("needle line {i}\n")
            } else {
                format!("filler line {i} padding padding padding\n")
            };
            body.push_str(&line);
        }
        fs::write(tmp.path().join("hay.rs"), body).unwrap();
        // Round-3 sweep (blocking class 1, driver leg): the <=budget claim
        // must EXERCISE the omitted-marker band the reserve guards, so the
        // budget is calibrated at runtime from measured cluster sizes (the
        // fixed 600-token budget left ~200 chars of headroom — a
        // reserve-deletion mutant ran green). band::calibrate picks the
        // budget where the shipped reserve stays under budget but a
        // delete/shrink mutant (reserve → 0/8/16/32) re-packs one more
        // cluster and overflows.
        // Round-4 timing sweep (T72 family): this leg has NO wall-clock
        // asserts — every pin here is SIZE-based (chars / budgets / cluster
        // counts) and band::calibrate measures rendered cluster sizes,
        // never time — so the leg is load-immune by construction. The only
        // timing assert on the tgrep surface is deterministic_and_fast's
        // SPEED leg (src/tgrep.rs, median-of-5 vs a load-robust bound).
        let measure_ctx = ToolCtx {
            cwd: tmp.path().to_path_buf(),
            bash_timeout: Duration::from_secs(crate::tools::BASH_TIMEOUT_SECS),
        };
        let measured = crate::tools::dispatch(
            &measure_ctx,
            "tgrep",
            &json!({"query": "needle", "budget": 8000u64}),
        );
        assert!(!measured.is_error, "{}", measured.content);
        // The 300-cluster measure pass at the 8000-token ceiling itself
        // truncates (300 x ~300 chars > 32000); calibrate only needs the
        // header's total plus the first ~100 rendered sizes — packing is
        // prefix-based, and the picked budgets show < 20 clusters.
        let (budget, shown96, body96, over0, header_len) =
            crate::tgrep::band::calibrate(&measured.content, 3);
        let (_llm, _events, _lines) = run_t38(
            &tmp,
            vec![
                tool_use_response("tgrep", json!({"query": "needle", "budget": budget})),
                tool_use_response("goal_complete", json!({"summary": "done"})),
            ],
        );
        // The second LLM call is the first to see the tgrep tool result.
        assert_eq!(_llm.calls.len(), 2);
        let (content, is_error) = tool_result_text(&_llm.calls[1].1).unwrap();
        assert!(!is_error, "{content}");
        // Marker fires; shown + omitted = 300 with at least one cluster shown.
        let marker_at = content
            .find("[more: ")
            .unwrap_or_else(|| panic!("no omission marker: {content}"));
        let marker_end = content[marker_at..].find(']').unwrap() + marker_at;
        let omitted: usize = content[marker_at..marker_end]
            .trim_start_matches("[more: ")
            .trim_end_matches(" clusters omitted")
            .parse()
            .unwrap();
        let shown = content.matches(" (exact-phrase)").count();
        assert_eq!(shown + omitted, 300, "{content}");
        assert!(shown > 0, "{content}");
        assert_eq!(
            shown, shown96,
            "reserve pushed out exactly one cluster: {content}"
        );
        assert!(
            content.contains("300 clusters in 1 file"),
            "header names the corpus: {content}"
        );
        // The output is EXACTLY header + reserve-limited body + marker, and
        // the band pins hold: without the reserve the packing takes one more
        // cluster and overflows by `over0` chars — the fixture straddles the
        // marker band, so the invariant below is genuinely exercised.
        assert_eq!(
            content.chars().count(),
            header_len + 2 + body96 + 59 + omitted.to_string().len(), // line1\n + marker\n
            "packing drifted: {content}"
        );
        assert!(over0 > 0, "fixture drifted out of the reserve band (over0 = {over0})");
        assert!(over0 <= 24, "band too loose: over0 = {over0}");
        assert!(
            content.chars().count() <= budget * 4,
            "tgrep output {} chars exceeds the {}-token budget: {}",
            content.chars().count(),
            budget,
            content
        );
    }

    /// T38: a truncated response (`stop_reason=max_tokens`) injects the
    /// pinned advisory as the last user message before the next LLM call —
    /// transcript + memory — and records exactly one `output_truncated`
    /// event and log line.
    #[test]
    fn truncated_response_injects_pinned_advisory_and_event() {
        let tmp = tempfile::tempdir().unwrap();
        let (llm, events, lines) = run_t38(
            &tmp,
            vec![
                truncated_tool_use_response(
                    "write_file",
                    json!({"path": "src/big.rs", "content": "…"}),
                ),
                tool_use_response("goal_complete", json!({"summary": "done"})),
            ],
        );
        assert_eq!(llm.calls.len(), 2);
        // The first call predates any response: no advisory anywhere in it.
        assert_eq!(advisory_count(&llm.calls[0].1), 0);

        // The next call ends with the pinned advisory, right after the tool
        // result of the truncated response.
        let seen = &llm.calls[1].1;
        let last = seen.last().unwrap();
        assert_eq!(last.role, "user");
        assert_eq!(last.content.len(), 1);
        assert_eq!(last.content[0].text(), Some(T38_ADVISORY));
        let second_last = &seen[seen.len() - 2];
        assert_eq!(second_last.role, "user");
        assert!(matches!(
            second_last.content[0],
            ContentBlock::Known(KnownBlock::ToolResult { .. })
        ));

        // Transcript on disk carries the same advisory.
        let on_disk = transcript::load(tmp.path()).unwrap();
        assert_eq!(advisory_count(&on_disk), 1);

        // Exactly one event on the recorded stream, exactly one
        // `output_truncated` line on the log, both before the terminal goal.
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, Event::OutputTruncated))
                .count(),
            1
        );
        let truncs: Vec<&Value> = lines
            .iter()
            .filter(|l| l["type"] == "output_truncated")
            .collect();
        assert_eq!(truncs.len(), 1, "{lines:?}");
        assert!(truncs[0]["ts"].as_str().unwrap().ends_with('Z'));
        let trunc_idx = lines.iter().position(|l| l["type"] == "output_truncated").unwrap();
        let goal_idx = lines.iter().position(|l| l["type"] == "goal").unwrap();
        assert!(trunc_idx < goal_idx);
    }

    /// T38 control: `tool_use`, `end_turn`, and an absent stop_reason are
    /// byte-identical to pre-T38 — no advisory message, no event, no line.
    #[test]
    fn non_truncated_responses_inject_nothing() {
        // stop_reason: "tool_use"
        let tmp = tempfile::tempdir().unwrap();
        let (llm, events, lines) = run_t38(
            &tmp,
            vec![
                tool_use_response("bash", json!({"command": "true"})),
                tool_use_response("goal_complete", json!({"summary": "done"})),
            ],
        );
        assert_untouched(&llm, &events, &lines);

        // stop_reason: "end_turn" (natural stop → anti-stall kick → retry)
        let tmp = tempfile::tempdir().unwrap();
        let (llm, events, lines) = run_t38(
            &tmp,
            vec![
                text_only_response("all done"),
                tool_use_response("goal_complete", json!({"summary": "done"})),
            ],
        );
        assert_untouched(&llm, &events, &lines);

        // stop_reason absent entirely.
        let tmp = tempfile::tempdir().unwrap();
        let (llm, events, lines) = run_t38(
            &tmp,
            vec![
                json!({
                    "usage": {"input_tokens": 10, "output_tokens": 5},
                    "content": [{"type": "text", "text": "working"}],
                }),
                tool_use_response("goal_complete", json!({"summary": "done"})),
            ],
        );
        assert_untouched(&llm, &events, &lines);
    }

    /// T38: two consecutive truncated responses → two advisories — no
    /// one-shot latch (contrast: the T13 budget-low warning). History
    /// accumulates, so the third call carries both.
    #[test]
    fn two_truncated_responses_inject_two_advisories() {
        let tmp = tempfile::tempdir().unwrap();
        let (llm, events, lines) = run_t38(
            &tmp,
            vec![
                truncated_tool_use_response("bash", json!({"command": "echo one"})),
                truncated_tool_use_response("bash", json!({"command": "echo two"})),
                tool_use_response("goal_complete", json!({"summary": "done"})),
            ],
        );
        assert_eq!(llm.calls.len(), 3);
        assert_eq!(advisory_count(&llm.calls[0].1), 0);
        assert_eq!(advisory_count(&llm.calls[1].1), 1);
        assert_eq!(advisory_count(&llm.calls[2].1), 2);
        // Each truncated turn's advisory is the LAST message of the
        // following call.
        assert_eq!(llm.calls[1].1.last().unwrap().content[0].text(), Some(T38_ADVISORY));
        assert_eq!(llm.calls[2].1.last().unwrap().content[0].text(), Some(T38_ADVISORY));
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, Event::OutputTruncated))
                .count(),
            2
        );
        assert_eq!(
            lines
                .iter()
                .filter(|l| l["type"] == "output_truncated")
                .count(),
            2
        );
    }

    /// T38: a truncation with no tool calls (pure text cut short) still
    /// injects the same advisory — the chunking remedy sentence applies
    /// regardless of what was cut.
    #[test]
    fn pure_text_truncation_still_injects_advisory() {
        let tmp = tempfile::tempdir().unwrap();
        let (llm, events, lines) = run_t38(
            &tmp,
            vec![
                truncated_text_response("I'll write the file star"),
                tool_use_response("goal_complete", json!({"summary": "done"})),
            ],
        );
        // Autonomous: the text-only truncated response also takes the
        // anti-stall kick; the advisory lands after it, last before the
        // next call.
        let seen = &llm.calls[1].1;
        let last = seen.last().unwrap();
        assert_eq!(last.role, "user");
        assert_eq!(last.content[0].text(), Some(T38_ADVISORY));
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, Event::OutputTruncated))
                .count(),
            1
        );
        assert_eq!(
            lines
                .iter()
                .filter(|l| l["type"] == "output_truncated")
                .count(),
            1
        );
    }

