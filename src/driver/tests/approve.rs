// T146 family: approve — the F2 phase-2a `chug run --approve <plan.md>` gate
// at the run-loop level: the run_start provenance fields (always present,
// null without the flag) and the approved plan as the run's execution
// contract (prepended to the first outbound request).
use super::*; // the shared harness (driver::tests) + driver's own imports
use crate::api::{RawResponse, Transport, TransportError};
use std::sync::Mutex;

const PLAN: &str = "# Plan\n\n1. add the flag\n2. pin the parse\n";
const APPROVAL: &str =
    "The operator approved this implementation plan; implement it, then satisfy your goal's check.";

fn approved(path: &str) -> crate::driver::ApprovedPlan {
    crate::driver::ApprovedPlan {
        path: path.to_string(),
        sha256: crate::eventlog::goal_sha256(PLAN),
        text: PLAN.to_string(),
    }
}

/// Transport that records every request body AND scripts the responses —
/// the contract leg asserts the exact first message a real endpoint
/// would judge (crash.rs's RecordingTransport shape, family-local per
/// the T84 one-family rule).
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

/// T146 wiring leg (the Some↔None survivor class applied at the RUN call
/// site): `run_start` carries `approve` (the path as the operator typed
/// it) and `plan_sha256` (the file-bytes hash) from the config — and
/// with the flag absent both fields stay present-but-null (the T117
/// honesty shape).
#[test]
fn run_loop_run_start_carries_approve_fields_both_ways() {
    for approve in [Some(("plan.md", PLAN)), None] {
        let tmp = tempfile::tempdir().unwrap();
        let spec = tmp.path().join("s.md");
        std::fs::write(&spec, "spec text\ncheck: true\n").unwrap();
        let (stx, srx) = mpsc::channel();
        drop(stx);
        let cfg = RunConfig {
            cwd: tmp.path().to_path_buf(),
            spec_path: spec,
            goal: "a literal goal".to_string(),
            model: "test-model".to_string(),
            max_iters: 5,
            max_minutes: 10,
            max_tokens: 0,
            max_tokens_per_request: crate::api::DEFAULT_MAX_TOKENS,
            resume: false,
            // Abort at the first boundary: the startup path (including
            // run_start) runs, no LLM call is ever made.
            controls: Controls {
                abort: Arc::new(AtomicBool::new(true)),
                steering_rx: srx,
            },
            risk_gate: false,
            bash_timeout: Duration::from_secs(tools::BASH_TIMEOUT_SECS),
            mcp_config: None,
            ctx_warn_at_tokens: 0,
            mcp_off: true,
            goal_pack: None,
            approve: approve.map(|(path, text)| crate::driver::ApprovedPlan {
                path: path.to_string(),
                sha256: crate::eventlog::goal_sha256(text),
                text: text.to_string(),
            }),
        };
        let client =
            Client::new_without_credentials("test-model", crate::api::DEFAULT_MAX_TOKENS).unwrap();
        let mut sink = RecordingSink::default();
        let code = run_loop(cfg, client, None, &mut sink, &observ::Sink::Noop).unwrap();
        assert_eq!(code, 1);

        let first: Value = serde_json::from_str(
            std::fs::read_to_string(tmp.path().join(".chug/events.jsonl"))
                .expect("events.jsonl written")
                .lines()
                .next()
                .expect("run_start line"),
        )
        .unwrap();
        assert_eq!(first["type"], "run_start");
        // Always present, both ways — a missing key is the mutant shape.
        assert!(
            first.as_object().unwrap().contains_key("approve"),
            "approve field must always be present: {first}"
        );
        assert!(
            first.as_object().unwrap().contains_key("plan_sha256"),
            "plan_sha256 field must always be present: {first}"
        );
        match approve {
            Some((path, text)) => {
                assert_eq!(first["approve"], path, "{first}");
                assert_eq!(first["plan_sha256"], crate::eventlog::goal_sha256(text));
            }
            None => {
                assert!(first["approve"].is_null(), "{first}");
                assert!(first["plan_sha256"].is_null(), "{first}");
            }
        }
    }
}

/// T146 contract leg: with `--approve`, the first outbound request's
/// first user message opens with the operator-approval sentence and the
/// plan text verbatim — and the goal text still names the objective
/// (plan constrains HOW, goal names WHAT, the spec's check decides DONE).
/// The run then completes against the goal's check normally.
#[test]
fn run_loop_prepends_the_approved_plan_to_the_first_outbound_request() {
    let tmp = tempfile::tempdir().unwrap();
    let spec = write_spec(&tmp);
    let transport = Arc::new(RecordingTransport::new(vec![tool_use_response(
        "goal_complete",
        json!({"summary": "implemented the approved plan"}),
    )]));
    let client = Client::with_transport_for_tests(
        transport.clone(),
        "test-model",
        crate::api::DEFAULT_MAX_TOKENS,
    );
    let (stx, srx) = mpsc::channel();
    drop(stx);
    let cfg = RunConfig {
        cwd: tmp.path().to_path_buf(),
        spec_path: spec,
        goal: "land the flag".to_string(),
        model: "test-model".to_string(),
        max_iters: 5,
        max_minutes: 10,
        max_tokens: 0,
        max_tokens_per_request: crate::api::DEFAULT_MAX_TOKENS,
        resume: false,
        controls: Controls {
            abort: Arc::new(AtomicBool::new(false)),
            steering_rx: srx,
        },
        risk_gate: false,
        bash_timeout: Duration::from_secs(tools::BASH_TIMEOUT_SECS),
        mcp_config: None,
        mcp_off: true,
        goal_pack: None,
        ctx_warn_at_tokens: 0,
        approve: Some(approved("plan.md")),
    };
    let mut sink = RecordingSink::default();
    let code = run_loop(cfg, client, None, &mut sink, &observ::Sink::Noop).unwrap();
    assert_eq!(code, 0, "the run completes against the check");

    let bodies = transport.bodies();
    assert!(!bodies.is_empty(), "the run made at least one LLM call");
    let first: Value = serde_json::from_str(&bodies[0]).unwrap();
    let first_message = first["messages"][0]["content"][0]["text"]
        .as_str()
        .expect("first message is a text block")
        .to_string();
    assert!(
        first_message.starts_with(APPROVAL),
        "the approval sentence opens the first message: {first_message}"
    );
    assert!(
        first_message.contains(PLAN),
        "the plan text rides the first message verbatim: {first_message}"
    );
    assert!(
        first_message.contains("Goal: land the flag"),
        "the goal still names the objective: {first_message}"
    );
    // The plan block sits BEFORE the goal text (prepended, not appended).
    let plan_at = first_message.find(PLAN).unwrap();
    let goal_at = first_message.find("Goal: land the flag").unwrap();
    assert!(plan_at < goal_at, "plan block precedes the goal text");
}
