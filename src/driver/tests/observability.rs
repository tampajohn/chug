// T104 family: observability — SPEC-8 wiring: spans, scores, gate verdicts, steering notes. Moved bytes byte-identical (T84 rule)
// from driver.rs's test module; every test here lives in exactly one family
// file.
use super::*; // the shared harness (driver::tests) + driver's own imports
// ---------- observability wiring (SPEC-8) ----------

/// A live observability sink wired to a counting transport: `sink.shutdown()`
/// flushes everything, then `transport.events()` returns what was emitted.
fn test_obs() -> (
    std::sync::Arc<observ::testing::CountingTransport>,
    observ::Sink,
) {
    let transport = observ::testing::CountingTransport::new();
    let sink = observ::testing::test_sink(transport.clone());
    (transport, sink)
}

fn events_of_kind(transport: &observ::testing::CountingTransport, kind: &str) -> Vec<Value> {
    transport
        .events()
        .into_iter()
        .filter(|e| e["type"] == kind)
        .collect()
}

// ---------- observability tests (SPEC-8) ----------

#[test]
fn observability_run_emits_span_goal_event_and_completion_scores() {
    let tmp = tempfile::tempdir().unwrap();
    let (transport, sink) = test_obs();
    let (_utx, urx) = mpsc::channel::<SlashUpdate>();
    let controls = Controls::detached();
    let ctx = ctx_for(
        &tmp,
        Mode::Autonomous,
        &controls,
        &urx,
        Some("chug-test0001"),
        &sink,
    );
    let mut knobs = knobs_with(40);
    let mut llm = ScriptedLlm::new(vec![
        tool_use_response("read_file", json!({"path": "missing.txt"})),
        tool_use_response("goal_complete", json!({"summary": "did it"})),
    ]);
    let mut gate = None;
    let mut messages = Vec::new();
    let outcome = drive_loop(
        &ctx,
        &mut knobs,
        &mut llm,
        &mut gate,
        &mut messages,
        Some("check: true".to_string()),
        &mut RecordingSink::default(),
        // Forced-off registry: MCP is a strict no-op in these tests.
        &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
    )
    .unwrap();
    assert!(matches!(outcome, DriveOutcome::RunFinished(0)));
    sink.shutdown();

    // One span per tool call — read_file (missing file → error result)
    // and goal_complete (ok) — with ok / is_error metadata.
    let spans = events_of_kind(&transport, "span-create");
    assert_eq!(spans.len(), 2);
    assert_eq!(spans[0]["body"]["name"], "read_file");
    assert_eq!(spans[0]["body"]["metadata"]["ok"], false);
    assert_eq!(spans[0]["body"]["metadata"]["is_error"], true);
    assert_eq!(spans[1]["body"]["name"], "goal_complete");
    assert_eq!(spans[1]["body"]["metadata"]["ok"], true);
    assert_eq!(spans[1]["body"]["metadata"]["is_error"], false);

    // Goal accepted event carrying the summary.
    let accepted = events_of_kind(&transport, "event-create");
    assert_eq!(accepted.len(), 1);
    assert_eq!(accepted[0]["body"]["name"], "goal_accepted");
    assert_eq!(accepted[0]["body"]["metadata"]["summary"], "did it");

    // Outcome (completed) + iterations (2 LLM iterations) scores.
    let scores = events_of_kind(&transport, "score-create");
    assert_eq!(scores.len(), 2);
    assert_eq!(scores[0]["body"]["name"], "outcome");
    assert_eq!(scores[0]["body"]["stringValue"], "completed");
    assert_eq!(scores[0]["body"]["dataType"], "CATEGORICAL");
    assert_eq!(scores[1]["body"]["name"], "iterations");
    assert_eq!(scores[1]["body"]["value"], 2);
    assert_eq!(scores[1]["body"]["dataType"], "NUMERIC");

    // The run trace is finished via the trace-create upsert.
    let finishes = events_of_kind(&transport, "trace-create");
    assert_eq!(finishes.len(), 1);
    assert_eq!(finishes[0]["body"]["id"], "chug-test0001");
    assert_eq!(finishes[0]["body"]["metadata"]["outcome"], "completed");
    assert_eq!(finishes[0]["body"]["metadata"]["iterations"], 2);
}

#[test]
fn observability_budget_abort_scores_budget_outcome_and_finishes_trace() {
    let tmp = tempfile::tempdir().unwrap();
    let (transport, sink) = test_obs();
    let (_utx, urx) = mpsc::channel::<SlashUpdate>();
    let controls = Controls::detached();
    let ctx = ctx_for(
        &tmp,
        Mode::Autonomous,
        &controls,
        &urx,
        Some("chug-test0002"),
        &sink,
    );
    let mut knobs = knobs_with(0); // budget exhausted before iteration 1
    let mut llm = ScriptedLlm::new(vec![]);
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
        // Forced-off registry: MCP is a strict no-op in these tests.
        &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
    )
    .unwrap();
    assert!(matches!(outcome, DriveOutcome::RunFinished(1)));
    sink.shutdown();

    let aborts = events_of_kind(&transport, "event-create");
    assert_eq!(aborts.len(), 1);
    assert_eq!(aborts[0]["body"]["name"], "abort");
    assert_eq!(
        aborts[0]["body"]["metadata"]["reason"],
        "iteration budget exceeded"
    );
    let scores = events_of_kind(&transport, "score-create");
    assert_eq!(scores[0]["body"]["stringValue"], "budget");
    assert_eq!(scores[1]["body"]["value"], 0);
    let finishes = events_of_kind(&transport, "trace-create");
    assert_eq!(finishes[0]["body"]["metadata"]["outcome"], "budget");
}

#[test]
fn observability_chat_turns_neither_score_nor_finish_the_session_trace() {
    let tmp = tempfile::tempdir().unwrap();
    let (transport, sink) = test_obs();
    let (_utx, urx) = mpsc::channel::<SlashUpdate>();
    let controls = Controls::detached();
    let ctx = ctx_for(
        &tmp,
        Mode::Chat,
        &controls,
        &urx,
        Some("chug-test0003"),
        &sink,
    );
    // Budget exhausted immediately: the turn aborts but the SESSION
    // trace stays open (per-session lifecycle, not per-turn).
    let mut knobs = knobs_with(0);
    let mut llm = ScriptedLlm::new(vec![]);
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
        // Forced-off registry: MCP is a strict no-op in these tests.
        &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        outcome,
        DriveOutcome::TurnEnded(TurnEndReason::BudgetExceeded)
    ));
    sink.shutdown();

    // The abort event is emitted, but no scores and no trace finish —
    // the session continues and finishing is the chat loop's job.
    let aborts = events_of_kind(&transport, "event-create");
    assert_eq!(aborts[0]["body"]["name"], "abort");
    assert!(events_of_kind(&transport, "score-create").is_empty());
    assert!(events_of_kind(&transport, "trace-create").is_empty());
}

#[test]
fn observability_risk_gate_verdicts_become_events() {
    let tmp = tempfile::tempdir().unwrap();
    let (transport, sink) = test_obs();
    let (_utx, urx) = mpsc::channel::<SlashUpdate>();
    let controls = Controls::detached();

    // Phase 1: judge blocks → verdict "blocked" event.
    let ctx = ctx_for(
        &tmp,
        Mode::Chat,
        &controls,
        &urx,
        Some("chug-test0004"),
        &sink,
    );
    let mut knobs = knobs_with(40);
    let mut gate = Some(RiskGate::new(
        Box::new(CannedJudge("destructive", 0.9)),
        tmp.path(),
    ));
    let mut llm = ScriptedLlm::new(vec![
        tool_use_response("bash", json!({"command": "rm -rf site"})),
        text_only_response("understood"),
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
        // Forced-off registry: MCP is a strict no-op in these tests.
        &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        outcome,
        DriveOutcome::TurnEnded(TurnEndReason::Completed)
    ));

    // Phase 2: judge allows → verdict "allowed" event (same trace).
    let ctx = ctx_for(
        &tmp,
        Mode::Chat,
        &controls,
        &urx,
        Some("chug-test0004"),
        &sink,
    );
    let mut knobs = knobs_with(40);
    let mut gate = Some(RiskGate::new(
        Box::new(CannedJudge("safe", 0.1)),
        tmp.path(),
    ));
    let mut llm = ScriptedLlm::new(vec![
        tool_use_response("bash", json!({"command": "ls"})),
        text_only_response("listed"),
    ]);
    let mut messages = Vec::new();
    drive_loop(
        &ctx,
        &mut knobs,
        &mut llm,
        &mut gate,
        &mut messages,
        None,
        &mut RecordingSink::default(),
        // Forced-off registry: MCP is a strict no-op in these tests.
        &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
    )
    .unwrap();
    sink.shutdown();

    let gate_events: Vec<Value> = transport
        .events()
        .into_iter()
        .filter(|e| e["type"] == "event-create" && e["body"]["name"] == "risk_gate")
        .collect();
    assert_eq!(gate_events.len(), 2);
    assert_eq!(gate_events[0]["body"]["metadata"]["verdict"], "blocked");
    assert_eq!(gate_events[0]["body"]["metadata"]["command"], "rm -rf site");
    assert_eq!(gate_events[1]["body"]["metadata"]["verdict"], "allowed");
    assert_eq!(gate_events[1]["body"]["metadata"]["command"], "ls");
}

#[test]
fn observability_steering_notes_become_events() {
    let tmp = tempfile::tempdir().unwrap();
    let (transport, sink) = test_obs();
    let (_utx, urx) = mpsc::channel::<SlashUpdate>();
    let (steer_tx, steer_rx) = mpsc::channel();
    steer_tx.send("focus on tests".to_string()).unwrap();
    let controls = Controls {
        abort: Arc::new(AtomicBool::new(false)),
        steering_rx: steer_rx,
    };
    let ctx = ctx_for(
        &tmp,
        Mode::Chat,
        &controls,
        &urx,
        Some("chug-test0005"),
        &sink,
    );
    let mut knobs = knobs_with(40);
    let mut llm = ScriptedLlm::new(vec![text_only_response("ok")]);
    let mut gate = None;
    let mut messages = Vec::new();
    drive_loop(
        &ctx,
        &mut knobs,
        &mut llm,
        &mut gate,
        &mut messages,
        None,
        &mut RecordingSink::default(),
        // Forced-off registry: MCP is a strict no-op in these tests.
        &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
    )
    .unwrap();
    sink.shutdown();

    let steering: Vec<Value> = transport
        .events()
        .into_iter()
        .filter(|e| e["type"] == "event-create" && e["body"]["name"] == "steering")
        .collect();
    assert_eq!(steering.len(), 1);
    assert_eq!(steering[0]["body"]["metadata"]["note"], "focus on tests");
}
