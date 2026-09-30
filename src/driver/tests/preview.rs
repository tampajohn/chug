// T104 family: preview — failure-aware event previews (T25). Moved bytes byte-identical (T84 rule)
// from driver.rs's test module; every test here lives in exactly one family
// file.
use super::*; // the shared harness (driver::tests) + driver's own imports
// ---------- T25: failure-aware event previews ----------

#[test]
fn error_preview_is_tail_anchored_long_content() {
    let marker = "failures:\n    tests::the_flaky_one";
    let content = format!("{}{marker}", "x".repeat(3000));
    let preview = tool_result_preview(&content, true);
    assert_eq!(preview.chars().count(), ERROR_PREVIEW_TAIL_CHARS);
    assert!(
        preview.ends_with(marker),
        "the failing test's name at the end survives: ...{}",
        preview
            .chars()
            .skip(ERROR_PREVIEW_TAIL_CHARS - 40)
            .collect::<String>()
    );
    // Exactly the last 2000 chars of the content, nothing else.
    let expected: String = content
        .chars()
        .skip(content.chars().count() - ERROR_PREVIEW_TAIL_CHARS)
        .collect();
    assert_eq!(preview, expected);
}

#[test]
fn error_preview_short_content_kept_whole() {
    let content = "FAILED tests::small_failure".to_string();
    assert_eq!(tool_result_preview(&content, true), content);
}

#[test]
fn ok_preview_keeps_500_char_head_byte_identical() {
    // Head/tail distinguishable: a head-take(500) is all 'a', a tail
    // window would end in 'b'.
    let content = format!("{}{}", "a".repeat(800), "b".repeat(200));
    let preview = tool_result_preview(&content, false);
    assert_eq!(preview, "a".repeat(500));
}

#[test]
fn ok_preview_short_content_kept_whole() {
    let content = "wrote 5 bytes".to_string();
    assert_eq!(tool_result_preview(&content, false), content);
}

/// T25, end to end through drive_loop: a failing bash command whose
/// output exceeds the window carries its unique end-of-output marker
/// into the emitted `Event::ToolResult` preview (pre-T25 the flat
/// 500-char head dropped it, which is what hid the flaky test's name).
#[test]
fn error_tool_result_preview_ends_with_output_tail() {
    let tmp = tempfile::tempdir().unwrap();
    let (_utx, urx) = mpsc::channel::<SlashUpdate>();
    let controls = Controls::detached();
    let ctx = ctx_for(
        &tmp,
        Mode::Autonomous,
        &controls,
        &urx,
        None,
        &observ::Sink::Noop,
    );
    let mut knobs = knobs_with(5);
    let marker = "T25-TAIL-MARKER-the-flaky-test";
    let cmd = format!("printf '%s' '{}{marker}'; exit 7", "x".repeat(2200));
    let mut llm = ScriptedLlm::new(vec![
        tool_use_response("bash", json!({"command": cmd})),
        tool_use_response("goal_complete", json!({"summary": "done"})),
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
    assert!(matches!(outcome, DriveOutcome::RunFinished(0)));

    let (ok, preview) = sink
        .0
        .iter()
        .find_map(|e| match e {
            Event::ToolResult {
                name, ok, preview, ..
            } if name == "bash" => Some((*ok, preview.clone())),
            _ => None,
        })
        .expect("bash tool_result event");
    assert!(!ok, "the failing command is an error result");
    assert_eq!(preview.chars().count(), ERROR_PREVIEW_TAIL_CHARS);
    assert!(
        preview.ends_with(&format!("{marker}\n[exit code: 7]")),
        "the emitted preview ends with the output tail"
    );
}

/// T25 control: a successful tool result keeps the pre-T25 500-char
/// head preview, byte-identical.
#[test]
fn ok_tool_result_preview_keeps_500_char_head() {
    let tmp = tempfile::tempdir().unwrap();
    let (_utx, urx) = mpsc::channel::<SlashUpdate>();
    let controls = Controls::detached();
    let ctx = ctx_for(
        &tmp,
        Mode::Autonomous,
        &controls,
        &urx,
        None,
        &observ::Sink::Noop,
    );
    let mut knobs = knobs_with(5);
    let cmd = format!("printf '%s' '{}'", "y".repeat(800));
    let mut llm = ScriptedLlm::new(vec![
        tool_use_response("bash", json!({"command": cmd})),
        tool_use_response("goal_complete", json!({"summary": "done"})),
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
    assert!(matches!(outcome, DriveOutcome::RunFinished(0)));

    let (ok, preview) = sink
        .0
        .iter()
        .find_map(|e| match e {
            Event::ToolResult {
                name, ok, preview, ..
            } if name == "bash" => Some((*ok, preview.clone())),
            _ => None,
        })
        .expect("bash tool_result event");
    assert!(ok, "the succeeding command is not an error result");
    assert_eq!(preview, "y".repeat(500), "ok preview is the 500-char head");
}
