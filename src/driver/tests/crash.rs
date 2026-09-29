// T136 family: crash recovery. A kill mid-tool-batch leaves the transcript's
// last assistant message holding unanswered `tool_use` blocks — the batch's
// tool_result user message is appended only after the WHOLE batch finishes
// (assistant appended at driver.rs:930, results only at :1214) — and a torn
// trailing JSONL line (crash mid-append) used to abort transcript loading
// entirely. Resume must repair BOTH shapes: every unanswered tool_use gets an
// is_error tool_result ("interrupted; effects unknown") so the next request
// never carries missing tool_results for the endpoint to reject, and a torn
// tail is dropped and physically truncated so later appends cannot merge into
// the torn bytes. transcript.rs's truncating rewrite got the same class sweep
// (temp+rename; see transcript.rs tests).
    use super::*; // the shared harness (driver::tests) + driver's own imports
    use crate::api::{RawResponse, Transport, TransportError};
    use std::sync::Mutex;

    /// Transport that records every request body AND scripts the responses —
    /// the test asserts the exact message list a real endpoint would judge.
    struct RecordingTransport {
        bodies: Mutex<Vec<String>>,
        responses: Mutex<std::collections::VecDeque<Value>>,
    }

    impl RecordingTransport {
        fn new(responses: Vec<Value>) -> Self {
            RecordingTransport {
                bodies: Mutex::new(Vec::new()),
                responses: Mutex::new(responses.into()),
            }
        }

        fn bodies(&self) -> Vec<String> {
            self.bodies.lock().unwrap().clone()
        }
    }

    impl Transport for RecordingTransport {
        fn send(
            &self,
            _url: &str,
            _headers: &[(String, String)],
            body: &str,
        ) -> Result<RawResponse, TransportError> {
            self.bodies.lock().unwrap().push(body.to_string());
            let next = self
                .responses
                .lock()
                .unwrap()
                .pop_front()
                .ok_or_else(|| TransportError::Fatal("scripted transport exhausted".into()))?;
            Ok(RawResponse {
                status: 200,
                headers: Vec::new(),
                body: next.to_string(),
            })
        }
    }

    fn text_response(text: &str) -> Value {
        json!({
            "stop_reason": "end_turn",
            "usage": {"input_tokens": 10, "output_tokens": 5},
            "content": [{"type": "text", "text": text}],
        })
    }

    /// A resume-flavored RunConfig: no abort, `--resume`, literal goal.
    fn resumed_run_config(tmp: &tempfile::TempDir, spec: &Path) -> RunConfig {
        let (stx, srx) = mpsc::channel();
        drop(stx);
        RunConfig {
            cwd: tmp.path().to_path_buf(),
            spec_path: spec.to_path_buf(),
            goal: "x".to_string(),
            model: "test-model".to_string(),
            max_iters: 5,
            max_minutes: 10,
            max_tokens: 0,
            resume: true,
            controls: Controls {
                abort: Arc::new(AtomicBool::new(false)),
                steering_rx: srx,
            },
            risk_gate: false,
            bash_timeout: Duration::from_secs(tools::BASH_TIMEOUT_SECS),
            mcp_config: None,
            mcp_off: true,
            goal_pack: None,
        }
    }

    /// Seed the on-disk shape a kill mid-tool-batch leaves: the goal message,
    /// then the assistant message with its tool_use — and NO tool_result user
    /// message, because the crash landed before driver.rs's post-batch append.
    fn seed_crashed_batch(tmp: &tempfile::TempDir) {
        transcript::append(
            tmp.path(),
            &Message::user(vec![ContentBlock::text_block(
                "Goal: x\n\nThe spec, goal, and ledger are in your system prompt. Start working.",
            )]),
        )
        .unwrap();
        transcript::append(
            tmp.path(),
            &Message::assistant(vec![ContentBlock::Known(KnownBlock::ToolUse {
                id: "tu_crash".into(),
                name: "bash".into(),
                input: json!({ "command": "sleep 100" }),
            })]),
        )
        .unwrap();
    }

    /// The endpoint's pairing rule, simulated over a serialized request's
    /// `messages`: every assistant tool_use must be answered by a tool_result
    /// in the IMMEDIATELY following user message. Returns the rejection
    /// reason a real endpoint would give, or None when the request is valid.
    fn endpoint_rejection(messages: &[Value]) -> Option<String> {
        let mut owed: Option<Vec<String>> = None;
        for m in messages {
            match m["role"].as_str() {
                Some("assistant") => {
                    if owed.is_some() {
                        return Some("assistant message while tool results are still owed".into());
                    }
                    let ids: Vec<String> = m["content"]
                        .as_array()
                        .map(|blocks| {
                            blocks
                                .iter()
                                .filter(|b| b["type"] == "tool_use")
                                .filter_map(|b| b["id"].as_str().map(str::to_string))
                                .collect()
                        })
                        .unwrap_or_default();
                    if !ids.is_empty() {
                        owed = Some(ids);
                    }
                }
                Some("user") => {
                    if let Some(ids) = &owed {
                        for id in ids {
                            let answered = m["content"]
                                .as_array()
                                .map(|blocks| {
                                    blocks.iter().any(|b| {
                                        b["type"] == "tool_result"
                                            && b["tool_use_id"].as_str() == Some(id.as_str())
                                    })
                                })
                                .unwrap_or(false);
                            if !answered {
                                return Some(format!("tool_result missing for {id}"));
                            }
                        }
                    }
                    owed = None;
                }
                _ => {}
            }
        }
        owed.map(|_| "unanswered tool_use at the end of the request".to_string())
    }

    fn parse_messages(body: &str) -> Vec<Value> {
        serde_json::from_str::<Value>(body)
            .expect("request body parses as JSON")["messages"]
            .as_array()
            .expect("request body carries a messages array")
            .clone()
    }

    /// THE T136 trigger: chug was killed after the assistant tool_use message
    /// was appended but before the batch's tool_result message. A resumed run
    /// must send the endpoint a request in which the crashed batch's tool_use
    /// is answered (an is_error "interrupted" result — the tool may already
    /// have changed files, so its real result is gone), both in the request
    /// and on disk, and then run to goal-acceptance.
    #[test]
    fn resumed_run_answers_crashed_batch_tool_use() {
        let tmp = tempfile::tempdir().unwrap();
        let spec = write_spec(&tmp);
        seed_crashed_batch(&tmp);

        let client = Client::with_transport_for_tests(
            Arc::new(RecordingTransport::new(vec![
                text_response("the interrupted tool left state unknown; re-checking"),
                tool_use_response("goal_complete", json!({ "summary": "recovered" })),
            ])),
            "test-model",
        );
        let mut sink = RecordingSink::default();
        let code = run_loop(
            resumed_run_config(&tmp, &spec),
            client,
            None,
            &mut sink,
            &observ::Sink::Noop,
        )
        .unwrap();
        assert_eq!(code, 0, "the repaired resume runs to goal-acceptance");

        // (checked below via the transport's recorded bodies)
    }

    /// Same trigger, pinned against the RECORDED request bodies + the on-disk
    /// transcript (split from the exit-code test so a failure names the leg).
    #[test]
    fn resumed_request_carries_no_unanswered_tool_use() {
        let tmp = tempfile::tempdir().unwrap();
        let spec = write_spec(&tmp);
        seed_crashed_batch(&tmp);

        let transport = Arc::new(RecordingTransport::new(vec![
            text_response("re-checking state after the interrupted tool"),
            tool_use_response("goal_complete", json!({ "summary": "recovered" })),
        ]));
        let client = Client::with_transport_for_tests(transport.clone(), "test-model");
        let mut sink = RecordingSink::default();
        run_loop(
            resumed_run_config(&tmp, &spec),
            client,
            None,
            &mut sink,
            &observ::Sink::Noop,
        )
        .unwrap();

        let bodies = transport.bodies();
        assert!(!bodies.is_empty(), "the run made at least one LLM call");
        let first = parse_messages(&bodies[0]);
        assert_eq!(
            endpoint_rejection(&first),
            None,
            "first resumed request must be endpoint-valid: {first:?}"
        );
        // The repair is an is_error result naming the interruption, so the
        // model knows the tool's real effect is unknown (it may already have
        // changed files before the kill).
        let repair = first
            .iter()
            .find_map(|m| {
                m["content"]
                    .as_array()?
                    .iter()
                    .find(|b| b["type"] == "tool_result" && b["tool_use_id"] == "tu_crash")
                    .cloned()
            })
            .expect("the crashed batch's tool_use is answered");
        assert_eq!(repair["is_error"], json!(true), "repair is an error result");
        assert!(
            repair["content"]
                .as_str()
                .unwrap_or_default()
                .to_lowercase()
                .contains("interrupt"),
            "repair names the interruption: {repair}"
        );

        // On disk: the transcript is repaired too — the message immediately
        // following the crashed assistant answers it. Pre-fix, the next
        // append (the anti-stall kick, plain text) sat there instead and the
        // endpoint would still reject the next resume.
        let on_disk = transcript::load(tmp.path()).unwrap();
        let crash_idx = on_disk
            .iter()
            .position(|m| {
                m.role == "assistant"
                    && m.content
                        .iter()
                        .any(|b| b.tool_use().is_some_and(|(id, _, _)| id == "tu_crash"))
            })
            .expect("crashed assistant message still in the transcript");
        let follower = &on_disk[crash_idx + 1];
        assert!(
            follower.content.iter().any(|b| matches!(
                b,
                ContentBlock::Known(KnownBlock::ToolResult { tool_use_id, is_error: true, .. })
                    if tool_use_id == "tu_crash"
            )),
            "the message right after the crashed assistant answers it: {follower:?}"
        );
    }

    /// resume_messages (the run+chat --resume choke point) appends the
    /// interrupted tool_results in memory AND on disk, and is idempotent — a
    /// second resume must not stack a second repair message.
    #[test]
    fn resume_messages_repair_appends_once_in_memory_and_on_disk() {
        let tmp = tempfile::tempdir().unwrap();
        seed_crashed_batch(&tmp);

        let messages = resume_messages(tmp.path()).unwrap();
        assert_eq!(messages.len(), 3, "goal + crashed assistant + repair");
        let repair = &messages[2];
        assert_eq!(repair.role, "user");
        let results: Vec<&ContentBlock> = repair
            .content
            .iter()
            .filter(|b| matches!(b, ContentBlock::Known(KnownBlock::ToolResult { .. })))
            .collect();
        assert_eq!(results.len(), 1, "one repair result for the one tool_use");
        match &results[0] {
            ContentBlock::Known(KnownBlock::ToolResult {
                tool_use_id,
                is_error,
                content,
            }) => {
                assert_eq!(tool_use_id, "tu_crash");
                assert!(is_error);
                assert!(
                    content.as_str().unwrap_or_default().contains("interrupt"),
                    "repair names the interruption: {content}"
                );
            }
            other => panic!("expected a tool_result block, got {other:?}"),
        }

        // On disk: the appended repair line is the third message.
        let on_disk = transcript::load(tmp.path()).unwrap();
        assert_eq!(on_disk, messages, "repair persisted to the transcript");

        // Idempotent: the repaired transcript has no unanswered tool_use, so
        // a second resume is a no-op (same messages, same file bytes).
        let before = std::fs::read(transcript::transcript_path(tmp.path())).unwrap();
        let again = resume_messages(tmp.path()).unwrap();
        assert_eq!(again, messages, "second resume repairs nothing");
        let after = std::fs::read(transcript::transcript_path(tmp.path())).unwrap();
        assert_eq!(before, after, "second resume leaves the file byte-identical");
    }

    /// A fully answered history is untouched: no repair message, no writes.
    #[test]
    fn resume_messages_leaves_answered_history_untouched() {
        let tmp = tempfile::tempdir().unwrap();
        transcript::append(
            tmp.path(),
            &Message::user(vec![ContentBlock::text_block("Goal: x")]),
        )
        .unwrap();
        transcript::append(
            tmp.path(),
            &Message::assistant(vec![ContentBlock::Known(KnownBlock::ToolUse {
                id: "tu_ok".into(),
                name: "bash".into(),
                input: json!({ "command": "true" }),
            })]),
        )
        .unwrap();
        transcript::append(
            tmp.path(),
            &Message::user(vec![ContentBlock::tool_result_block(
                "tu_ok",
                "done".to_string(),
                false,
            )]),
        )
        .unwrap();
        let before = std::fs::read(transcript::transcript_path(tmp.path())).unwrap();

        let messages = resume_messages(tmp.path()).unwrap();
        assert_eq!(messages.len(), 3, "nothing appended");
        let after = std::fs::read(transcript::transcript_path(tmp.path())).unwrap();
        assert_eq!(before, after, "answered history is byte-identical on disk");
    }

    /// The torn-tail leg: a crash mid-append leaves a final line that is not
    /// valid JSON. Loading must drop the torn tail (not abort), resume must
    /// physically truncate it so the next append cannot merge into the torn
    /// bytes, and the repaired history still gets the tool_use repair.
    #[test]
    fn resume_messages_truncates_torn_trailing_line() {
        let tmp = tempfile::tempdir().unwrap();
        seed_crashed_batch(&tmp);
        let path = transcript::transcript_path(tmp.path());
        // Simulate the torn final append: half of a tool_result user message.
        let torn = r#"{"role":"user","content":[{"type":"tool_res"#;
        let mut data = std::fs::read_to_string(&path).unwrap();
        data.push_str(torn);
        std::fs::write(&path, data).unwrap();

        // Loading tolerates the torn tail: the two intact messages survive.
        let messages = resume_messages(tmp.path()).unwrap();
        assert_eq!(messages.len(), 3, "goal + assistant + repair (torn tail dropped)");
        assert!(messages[1].content.iter().any(|b| b.tool_use().is_some()));

        // The file no longer carries the torn bytes: a subsequent append
        // cannot merge into them, and the file parses whole.
        let on_disk = std::fs::read_to_string(&path).unwrap();
        assert!(!on_disk.contains("tool_res\""), "torn bytes physically gone: {on_disk}");
        assert!(on_disk.ends_with('\n'), "file ends on a line boundary");
        let reloaded = transcript::load(tmp.path()).unwrap();
        assert_eq!(reloaded.len(), 3, "file loads whole after truncation");

        // And a plain append lands as its own clean line.
        transcript::append(tmp.path(), &Message::user(vec![ContentBlock::text_block("next")]))
            .unwrap();
        let reloaded = transcript::load(tmp.path()).unwrap();
        assert_eq!(reloaded.len(), 4);
        assert_eq!(reloaded[3].content[0].text(), Some("next"));
    }

    /// A torn tail can also swallow the whole batch tool_result line, which
    /// is the same shape the batch-repair test covers — this pin keeps the
    /// tail-drop rule from ever regressing to a hard load error.
    #[test]
    fn load_drops_torn_trailing_line_and_still_errors_mid_file() {
        let tmp = tempfile::tempdir().unwrap();
        let path = transcript::transcript_path(tmp.path());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let good1 = serde_json::to_string(&Message::user(vec![ContentBlock::text_block("one")]))
            .unwrap();
        let good2 = serde_json::to_string(&Message::user(vec![ContentBlock::text_block("two")]))
            .unwrap();
        // Torn LAST line: load keeps the two intact messages.
        std::fs::write(&path, format!("{good1}\n{good2}\n{{\"role\":\"user\",\"con")).unwrap();
        let loaded = transcript::load(tmp.path()).unwrap();
        assert_eq!(loaded.len(), 2, "torn tail dropped, intact lines kept");

        // Corrupt MIDDLE line: real corruption stays loud — the middle line
        // is data loss, not a torn write, and silently skipping it would
        // hide a broken transcript.
        std::fs::write(&path, format!("{good1}\ngarbage{{\n{good2}\n")).unwrap();
        assert!(
            transcript::load(tmp.path()).is_err(),
            "mid-file corruption must still abort loading"
        );
    }
