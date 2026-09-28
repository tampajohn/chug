// T104 family: image — read_file image leg through the loop: degrade, previews, plan mode (T91). Moved bytes byte-identical (T84 rule)
// from driver.rs's test module; every test here lives in exactly one family
// file.
    use super::*; // the shared harness (driver::tests) + driver's own imports
// ---- T91: read_file image leg through the loop ----

/// Count image blocks in an outgoing request: standalone image blocks plus
/// image entries inside tool_result array content.
fn count_image_blocks(messages: &[Message]) -> usize {
    messages
        .iter()
        .flat_map(|m| m.content.iter())
        .filter(|b| match b {
            ContentBlock::Known(KnownBlock::Image { .. }) => true,
            ContentBlock::Known(KnownBlock::ToolResult {
                content: Value::Array(items),
                ..
            }) => items
                .iter()
                .any(|i| i.get("type").and_then(Value::as_str) == Some("image")),
            _ => false,
        })
        .count()
}

/// LLM double for the T91 degrade legs: call 1 replays scripted responses;
/// call 2 fails with an HTTP 400 whose body rejects image content (the
/// endpoint-rejection shape); later calls replay the remaining responses.
/// Every call's messages are recorded so the test can assert what the
/// retried request actually carried.
struct ImageDegradeLlm {
    responses: std::collections::VecDeque<Value>,
    calls: Vec<Vec<Message>>,
}

impl ImageDegradeLlm {
    fn new(responses: Vec<Value>) -> Self {
        ImageDegradeLlm {
            responses: responses.into(),
            calls: Vec::new(),
        }
    }
}

impl Llm for ImageDegradeLlm {
    fn complete(
        &mut self,
        _system: &str,
        messages: &[Message],
        _tools: &[Value],
        _obs: &crate::api::ObsCtx<'_>,
    ) -> anyhow::Result<crate::api::Response> {
        self.calls.push(messages.to_vec());
        if self.calls.len() == 2 {
            return Err(anyhow::anyhow!(
                "LLM request failed: HTTP 400: {{\"type\":\"error\",\"error\":{{\"type\":\"invalid_request_error\",\"message\":\"Requests must not contain image content blocks\"}}}}"
            ));
        }
        self.responses
            .pop_front()
            .map(|body| crate::api::Response { body })
            .ok_or_else(|| anyhow::anyhow!("no scripted response left"))
    }

    fn set_model(&mut self, _model: &str) {}

    fn model(&self) -> &str {
        "image-degrade-model"
    }
}

/// LLM double that always fails with an UNRELATED 400 body (no image/content
/// mention) — the control leg proving the degrade never fires on it.
struct UnrelatedErrLlm {
    calls: usize,
}

impl Llm for UnrelatedErrLlm {
    fn complete(
        &mut self,
        _system: &str,
        _messages: &[Message],
        _tools: &[Value],
        _obs: &crate::api::ObsCtx<'_>,
    ) -> anyhow::Result<crate::api::Response> {
        self.calls += 1;
        Err(anyhow::anyhow!(
            "LLM request failed: HTTP 400: {{\"error\":{{\"message\":\"max_tokens: field required\"}}}}"
        ))
    }

    fn set_model(&mut self, _model: &str) {}

    fn model(&self) -> &str {
        "unrelated-err-model"
    }
}

/// F7 hook-hygiene double (kimi finding 2): call 1 fails with the T91
/// image-rejection 400 (arming the degraded-retry path), call 2 — the
/// degraded retry — fails with an unrelated error, exercising the `?`
/// early return. Records whether a text-delta hook is installed at all
/// times so the test can assert the driver cleared it on that path.
struct HookLeakProbeLlm {
    calls: usize,
    hook_armed: bool,
}

impl Llm for HookLeakProbeLlm {
    fn complete(
        &mut self,
        _system: &str,
        _messages: &[Message],
        _tools: &[Value],
        _obs: &crate::api::ObsCtx<'_>,
    ) -> anyhow::Result<crate::api::Response> {
        self.calls += 1;
        if self.calls == 1 {
            return Err(anyhow::anyhow!(
                "LLM request failed: HTTP 400: {{\"type\":\"error\",\"error\":{{\"type\":\"invalid_request_error\",\"message\":\"Requests must not contain image content blocks\"}}}}"
            ));
        }
        Err(anyhow::anyhow!(
            "LLM request failed: HTTP 400: {{\"error\":{{\"message\":\"max_tokens: field required\"}}}}"
        ))
    }

    fn set_model(&mut self, _model: &str) {}

    fn model(&self) -> &str {
        "hook-leak-probe"
    }

    fn set_text_delta_hook(&mut self, hook: Option<crate::api::TextDeltaHook>) {
        self.hook_armed = hook.is_some();
    }
}

/// The full degrade arc: the request carrying a.png's image block is rejected
/// with a 400 image-rejection body → ONE retry whose request carries ZERO
/// image blocks and the placeholder text; ONE events note; and the latch
/// downgrades the SECOND image result at wrap time (never sent).
#[test]
fn image_endpoint_400_rejection_retries_once_with_placeholder_and_latches() {
    let tmp = tempfile::tempdir().unwrap();
    let a_bytes: &[u8] = b"\x89PNG\r\n\x1a\nAAA";
    let b_bytes: &[u8] = b"\x89PNG\r\n\x1a\nBBB";
    fs::write(tmp.path().join("a.png"), a_bytes).unwrap();
    fs::write(tmp.path().join("b.png"), b_bytes).unwrap();

    let (_utx, urx) = mpsc::channel::<SlashUpdate>();
    let controls = Controls::detached();
    let ctx = ctx_for(&tmp, Mode::Autonomous, &controls, &urx, None, &observ::Sink::Noop);
    let mut knobs = knobs_with(10);
    let mut llm = ImageDegradeLlm::new(vec![
        tool_use_response("read_file", json!({"path": "a.png"})),
        tool_use_response("read_file", json!({"path": "b.png"})),
        tool_use_response("goal_complete", json!({"summary": "did it"})),
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
    assert!(matches!(outcome, DriveOutcome::RunFinished(0)), "{outcome:?}");

    // Call 2 — the first request carrying a.png's image block — failed; the
    // retry is call 3; the final call is 4.
    assert_eq!(llm.calls.len(), 4, "failed call + retry + one per scripted turn");
    assert_eq!(count_image_blocks(&llm.calls[0]), 0, "kick request has no images");
    assert_eq!(
        count_image_blocks(&llm.calls[1]),
        1,
        "a.png's image block rode the request the endpoint rejected"
    );
    // Retry and every later request: zero image blocks, placeholder present.
    for (i, call) in llm.calls.iter().enumerate().skip(2) {
        assert_eq!(
            count_image_blocks(call),
            0,
            "call {i} must carry zero image blocks"
        );
        let wire = serde_json::to_string(call).unwrap();
        assert!(
            wire.contains(crate::api::IMAGE_REMOVED_PLACEHOLDER),
            "call {i} carries the placeholder text: {wire}"
        );
        assert!(!wire.contains(base64_of(a_bytes).as_str()), "call {i} leaks no a.png base64");
    }
    // The latch downgraded b.png's result at WRAP time: its tool_result is
    // the short note STRING, never an array with an image block.
    let b_result = llm.calls[3]
        .iter()
        .flat_map(|m| m.content.iter())
        .find_map(|b| match b {
            ContentBlock::Known(KnownBlock::ToolResult { content, is_error, .. }) => content
                .as_str()
                .filter(|s| s.contains("b.png"))
                .map(|s| (s, *is_error)),
            _ => None,
        })
        .expect("b.png tool result rides the final request as string content");
    assert!(!b_result.1, "the image read is not an error");
    assert!(
        b_result.0.starts_with("[image: ") && b_result.0.ends_with("image/png)]"),
        "the note rides: {}",
        b_result.0
    );

    // Exactly one events note for the whole run (the latch caps it).
    let degraded = sink
        .0
        .iter()
        .filter(|e| matches!(e, Event::ImageDegraded))
        .count();
    assert_eq!(degraded, 1, "exactly one image_degraded note, not per request");
}

/// Control leg: an unrelated 400 body keeps today's fail-fast error path —
/// no retry, no degrade note.
#[test]
fn image_unrelated_400_keeps_fail_fast_error_path() {
    let tmp = tempfile::tempdir().unwrap();
    let (_utx, urx) = mpsc::channel::<SlashUpdate>();
    let controls = Controls::detached();
    let ctx = ctx_for(&tmp, Mode::Autonomous, &controls, &urx, None, &observ::Sink::Noop);
    let mut knobs = knobs_with(10);
    let mut llm = UnrelatedErrLlm { calls: 0 };
    let mut gate = None;
    let mut messages = Vec::new();
    let mut sink = RecordingSink::default();
    let err = drive_loop(
        &ctx,
        &mut knobs,
        &mut llm,
        &mut gate,
        &mut messages,
        Some("check: true".to_string()),
        &mut sink,
        &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
    )
    .unwrap_err();
    assert!(err.to_string().contains("HTTP 400"), "{err}");
    assert_eq!(llm.calls, 1, "fail fast: no degrade retry on an unrelated 400");
    assert!(
        !sink.0.iter().any(|e| matches!(e, Event::ImageDegraded)),
        "no degrade note on an unrelated 400"
    );
}

/// F7 hook hygiene (kimi finding 2): the text-delta hook is cleared on EVERY
/// exit path of the LLM call — the T91 degraded-retry `?` early return
/// included. Call 1 errors with the image-rejection 400, the degraded retry
/// (call 2) errors again → drive_loop returns Err through the `?`; the hook
/// must be CLEARED, not left armed (a dangling raw-pointer hook into the
/// dropped sink borrow). RED on 53e4aed: the `?` propagated with the hook
/// still armed.
#[test]
fn degraded_retry_error_path_still_clears_the_text_delta_hook() {
    let tmp = tempfile::tempdir().unwrap();
    let (_utx, urx) = mpsc::channel::<SlashUpdate>();
    let controls = Controls::detached();
    let ctx = ctx_for(&tmp, Mode::Autonomous, &controls, &urx, None, &observ::Sink::Noop);
    let mut knobs = knobs_with(10);
    let mut llm = HookLeakProbeLlm { calls: 0, hook_armed: false };
    let mut gate = None;
    let mut messages = Vec::new();
    let mut sink = RecordingSink::default();
    let err = drive_loop(
        &ctx,
        &mut knobs,
        &mut llm,
        &mut gate,
        &mut messages,
        Some("check: true".to_string()),
        &mut sink,
        &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
    )
    .unwrap_err();
    assert!(err.to_string().contains("HTTP 400"), "{err}");
    assert_eq!(llm.calls, 2, "the image-rejection 400 + the degraded retry");
    assert!(
        !llm.hook_armed,
        "the hook is cleared on the degraded-retry error path"
    );
}

/// Events preview hygiene: an image tool result's events line carries the
/// short note — never more than a sliver of base64, in the event stream or
/// the `.chug/events.jsonl` line.
#[test]
fn image_result_events_preview_carries_short_note_no_base64() {
    let tmp = tempfile::tempdir().unwrap();
    let png: &[u8] = b"\x89PNG\r\n\x1a\nPREVIEWHYGIENE";
    fs::write(tmp.path().join("a.png"), png).unwrap();

    let (_utx, urx) = mpsc::channel::<SlashUpdate>();
    let controls = Controls::detached();
    let ctx = ctx_for(&tmp, Mode::Autonomous, &controls, &urx, None, &observ::Sink::Noop);
    let mut knobs = knobs_with(10);
    let mut llm = ScriptedLlm::new(vec![
        tool_use_response("read_file", json!({"path": "a.png"})),
        tool_use_response("goal_complete", json!({"summary": "did it"})),
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
    assert!(matches!(outcome, DriveOutcome::RunFinished(0)), "{outcome:?}");

    let note = format!(
        "[image: {} ({} bytes, image/png)]",
        tmp.path().join("a.png").display(),
        png.len()
    );
    let (ok, preview) = sink
        .0
        .iter()
        .find_map(|e| match e {
            Event::ToolResult { name, ok, preview, .. } if name == "read_file" => {
                Some((*ok, preview.clone()))
            }
            _ => None,
        })
        .expect("read_file tool_result event");
    assert!(ok, "the image read is ok");
    assert_eq!(preview, note, "the preview IS the short note");
    assert!(preview.len() < 100, "preview stays short: {preview}");
    assert!(!preview.contains(base64_of(png).as_str()), "no base64 in the preview");
    // The `.chug/events.jsonl` tool_result line rides the same short note.
    let lines: Vec<Value> = fs::read_to_string(tmp.path().join(".chug/events.jsonl"))
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let tr = lines
        .iter()
        .find(|l| l["type"] == "tool_result" && l["name"] == "read_file")
        .expect("events.jsonl records the read_file result");
    assert_eq!(tr["preview"], note, "events line carries the short note");
}

/// Plan-mode leg: read_file is one of the five plan tools — an image read
/// works there, riding the same array-content shape (five-tool contract is
/// pinned separately by `plan_mode_advertises_exactly_the_five_tool_schemas`).
#[test]
fn image_read_works_in_plan_mode() {
    let tmp = tempfile::tempdir().unwrap();
    let png: &[u8] = b"\x89PNG\r\n\x1a\nPLANIMG";
    fs::write(tmp.path().join("a.png"), png).unwrap();
    let out = tmp.path().join("plan.md");
    let (_utx, urx) = mpsc::channel::<SlashUpdate>();
    let controls = Controls::detached();
    let ctx = ctx_for_plan(&tmp, Some(&out), &controls, &urx);
    let mut knobs = knobs_with(5);
    let mut llm = ScriptedLlm::new(vec![
        tool_use_response("read_file", json!({"path": "a.png"})),
        tool_use_response("submit_plan", json!({"plan": "# Plan\n\nread the image\n"})),
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
        &mut McpRegistry::new(ctx.cwd, true, None).unwrap(),
    )
    .unwrap();
    assert!(matches!(outcome, DriveOutcome::RunFinished(0)), "{outcome:?}");
    assert_eq!(count_image_blocks(&messages), 1, "no degrade in plan mode: the image rides");
    let block = messages
        .iter()
        .flat_map(|m| m.content.iter())
        .find_map(|b| match b {
            ContentBlock::Known(KnownBlock::ToolResult { content, is_error, .. }) => {
                content.as_array().map(|items| (items, *is_error))
            }
            _ => None,
        })
        .expect("the image tool result uses array content");
    assert!(!block.1, "not an error");
    let items = block.0;
    assert_eq!(items.len(), 2, "image block first, then the text note");
    assert_eq!(items[0]["type"], "image");
    assert_eq!(items[0]["source"]["media_type"], "image/png");
    assert_eq!(items[0]["source"]["data"], base64_of(png));
    assert_eq!(items[1]["type"], "text");
    assert_eq!(
        items[1]["text"],
        format!(
            "[image: {} ({} bytes, image/png)]",
            tmp.path().join("a.png").display(),
            png.len()
        )
    );
}

/// Minimal PNG bytes for tests above need their base64; re-derive it from
/// the tool layer's encoder (private) via a dispatch round-trip is overkill —
/// compute it inline with the same standard alphabet.
fn base64_of(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b0 = u32::from(chunk[0]);
        let b1 = u32::from(chunk.get(1).copied().unwrap_or(0));
        let b2 = u32::from(chunk.get(2).copied().unwrap_or(0));
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[(n >> 18 & 63) as usize] as char);
        out.push(TABLE[(n >> 12 & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            TABLE[(n >> 6 & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}
