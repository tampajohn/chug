// T188 family: autospec — the auto-spec draft phase + acceptance gate
// (src/autospec.rs `draft_and_gate`) and the drafted-spec driver wiring
// (PlanKind::SpecDraft prompt/first-message/kick selection in driver.rs).
// The fix-up classes: the gate path had NO e2e coverage and the SpecDraft
// wiring was unpinned (m6: 1-attempt-instead-of-redraft-once, m7:
// kick→PLAN_KICK both survived the full suite). Every test here lives in
// exactly one family file.
use super::*; // the shared harness (driver::tests) + driver's own imports

// ===================== T188: auto-spec =====================

/// The complete draft fixture: every piece `validate_spec` requires, with a
/// check that is green in a cwd holding `marker.txt` and real (non-vacuous).
fn draft_fixture(check: &str) -> String {
    format!(
        "\
# T-e2e — wire the draft gate

check: {check}

estimate: ~5 changed lines

## Concern

A drafted spec must be gated before it runs.

## Requirements

- The check executes and passes the dry-run.

## Tests

- The e2e leg drives draft → run → goal_complete.

## Acceptance

- `{check}` exits 0.
"
    )
}

const E2E_GOAL: &str = "wire the draft gate end to end";

/// The draft-phase config `draft_and_gate` builds (mirrored here only to
/// build the SAME shape for the wiring pins below — the assertion targets
/// are the loop's own artifacts, never this mirror).
fn spec_draft_cfg(tmp: &tempfile::TempDir, goal: &str) -> PlanConfig {
    PlanConfig {
        cwd: tmp.path().to_path_buf(),
        kind: crate::plan::PlanKind::SpecDraft,
        spec_path: None,
        goal: goal.to_string(),
        model: "scripted-model".to_string(),
        max_iters: 5,
        max_minutes: 10,
        max_tokens: 0,
        max_tokens_per_request: crate::api::DEFAULT_MAX_TOKENS,
        out_path: Some(tmp.path().join(crate::autospec::AUTO_SPEC_REL)),
        goal_pack: None,
    }
}

fn empty_registry(tmp: &tempfile::TempDir) -> McpRegistry {
    McpRegistry::new(tmp.path(), true, None).unwrap()
}

// ---------- F2: the SpecDraft driver wiring (kills m7) ----------

/// THE m7 KILLER. A drafted-spec session whose model stalls (answers with
/// text, no tool call) must be kicked with `autospec::DRAFT_KICK` — the
/// draft kick ("the complete drafted spec"), never the plan kick ("the
/// complete plan"). The mutant flips the `PlanKind::SpecDraft` arm to
/// `plan::PLAN_KICK`; the transcript then carries the plan kick and loses
/// the draft kick, and both assertions below go RED. The full-string
/// assertions are safe both ways: neither kick is a substring of the other,
/// and a fresh session transcript contains no other kick.
#[test]
fn spec_draft_stall_is_kicked_with_the_draft_kick_not_the_plan_kick() {
    let tmp = tempfile::tempdir().unwrap();
    let goal = "Operator request: draft a spec for renaming a param";
    let spec_text = draft_fixture("test -f marker.txt");
    std::fs::write(tmp.path().join("marker.txt"), "x").unwrap();
    let mut llm = crate::api::ScriptedLlm::new(vec![
        // The stall: text only, no tool call → the anti-stall kick fires.
        text_only_response("let me look around first"),
        // The recovery: the complete spec lands via submit_plan → exit 0.
        tool_use_response("submit_plan", json!({ "plan": spec_text })),
    ]);
    let mut sink = RecordingSink::default();
    let code = run_plan_loop(
        spec_draft_cfg(&tmp, goal),
        &mut llm,
        &mut sink,
        &observ::Sink::Noop,
        empty_registry(&tmp),
    )
    .unwrap();
    assert_eq!(code, 0, "the draft session exits 0 on submit_plan");
    assert_eq!(
        llm.calls.len(),
        2,
        "two model calls: the stalled one and the kicked one"
    );
    // The loop's own transcript carries the DRAFT kick — and never the
    // plan kick (m7: the SpecDraft arm flipped to PLAN_KICK turns this RED:
    // the transcript then holds PLAN_KICK and the DRAFT_KICK assertion dies).
    let transcript = fs::read_to_string(tmp.path().join(".chug/transcript.jsonl")).unwrap();
    assert!(
        transcript.contains(crate::autospec::DRAFT_KICK),
        "a stalled spec-draft session must be kicked with DRAFT_KICK; transcript:\n{transcript}"
    );
    assert!(
        !transcript.contains(crate::plan::PLAN_KICK),
        "a spec-draft session must NEVER be kicked with PLAN_KICK; transcript:\n{transcript}"
    );
    // The kicked call's message history shows the kick as a USER message —
    // the same pin on the seam the model actually sees.
    let kicked_messages = format!("{:?}", llm.calls[1].1);
    assert!(
        kicked_messages.contains(crate::autospec::DRAFT_KICK),
        "the kick reaches the model as a user message: {kicked_messages}"
    );
    assert!(
        !kicked_messages.contains(crate::plan::PLAN_KICK),
        "the plan kick must not reach a spec-draft model: {kicked_messages}"
    );
}

/// F2, the prompt half of the wiring: a SpecDraft session's system prompt
/// is built on DRAFT_PREAMBLE (the draft contract), never PLAN_PREAMBLE —
/// read off the model seam (ScriptedLlm records the system string), so a
/// preamble-selection mutant is RED on the bytes the model received. The
/// first user message (the draft brief) carries the operator request and
/// the submit_plan-ONCE instruction.
#[test]
fn spec_draft_system_prompt_is_the_draft_preamble_with_the_draft_brief() {
    let tmp = tempfile::tempdir().unwrap();
    let goal = "Operator request: draft a spec for renaming a param";
    let spec_text = draft_fixture("test -f marker.txt");
    std::fs::write(tmp.path().join("marker.txt"), "x").unwrap();
    let mut llm = crate::api::ScriptedLlm::new(vec![tool_use_response(
        "submit_plan",
        json!({ "plan": spec_text }),
    )]);
    let mut sink = RecordingSink::default();
    let code = run_plan_loop(
        spec_draft_cfg(&tmp, goal),
        &mut llm,
        &mut sink,
        &observ::Sink::Noop,
        empty_registry(&tmp),
    )
    .unwrap();
    assert_eq!(code, 0);
    assert_eq!(llm.calls.len(), 1);
    // The system prompt the model received: the DRAFT preamble, not the
    // plan one.
    let system = &llm.calls[0].0;
    assert!(
        system.starts_with(crate::autospec::DRAFT_PREAMBLE),
        "a SpecDraft session's system prompt must start with DRAFT_PREAMBLE; got:\n{system}"
    );
    assert!(
        !system.contains(crate::plan::PLAN_PREAMBLE),
        "the plan preamble must never reach a spec-draft model; system:\n{system}"
    );
    // The first user message is the draft brief: the operator request
    // verbatim plus the submit_plan-ONCE spec instruction.
    let first = format!("{:?}", llm.calls[0].1);
    assert!(
        first.contains(goal),
        "the first user message carries the draft brief (the goal verbatim): {first}"
    );
    assert!(
        first.contains("complete drafted spec"),
        "the first user message names the drafted-spec submit contract: {first}"
    );
    assert!(
        !first.contains("The goal (and spec, when given) are in your system prompt"),
        "a spec-draft brief is never the plan-mode brief: {first}"
    );
    assert!(
        first.contains("not an implementation plan"),
        "the brief states the submit carries the spec, not an implementation plan: {first}"
    );
}

/// F2, the pure-function half: `build_plan_system_prompt` selects the
/// preamble BY KIND — SpecDraft → DRAFT_PREAMBLE, Plan → PLAN_PREAMBLE —
/// with the goal riding as `## Goal`. Both directions pinned so neither
/// kind can silently inherit the other's contract.
#[test]
fn build_plan_system_prompt_selects_the_preamble_by_kind() {
    let goal = "the draft brief";
    let draft = build_plan_system_prompt(None, goal, "ledger", crate::plan::PlanKind::SpecDraft);
    assert!(
        draft.starts_with(crate::autospec::DRAFT_PREAMBLE),
        "SpecDraft → DRAFT_PREAMBLE: {draft}"
    );
    assert!(!draft.contains(crate::plan::PLAN_PREAMBLE));
    assert!(draft.contains("## Goal") && draft.contains(goal), "{draft}");
    let plan = build_plan_system_prompt(None, goal, "ledger", crate::plan::PlanKind::Plan);
    assert!(
        plan.starts_with(crate::plan::PLAN_PREAMBLE),
        "Plan → PLAN_PREAMBLE: {plan}"
    );
    assert!(!plan.contains(crate::autospec::DRAFT_PREAMBLE));
    assert!(plan.contains("## Goal") && plan.contains(goal), "{plan}");
}

// ---------- F1: the acceptance gate (draft_and_gate) ----------

/// Run one draft_and_gate attempt with a scripted LLM; returns the result
/// and the double (for call-count/redraft-prompt assertions).
fn gate_with(
    tmp: &tempfile::TempDir,
    goal: &str,
    drafts: Vec<String>,
) -> (
    anyhow::Result<std::path::PathBuf>,
    crate::api::ScriptedLlm,
    RecordingSink,
) {
    let responses = drafts
        .into_iter()
        .map(|text| tool_use_response("submit_plan", json!({ "plan": text })))
        .collect();
    let mut llm = crate::api::ScriptedLlm::new(responses);
    let mut sink = RecordingSink::default();
    let result = crate::autospec::draft_and_gate(
        tmp.path(),
        goal,
        "scripted-model",
        crate::api::DEFAULT_MAX_TOKENS,
        &mut llm,
        &mut sink,
    );
    (result, llm, sink)
}

/// A first draft whose check FAILS the dry-run is redrafted ONCE with the
/// failure as feedback; the corrected second draft is accepted. THE m6
/// KILLER (leg 1): the 1-attempt mutant aborts after the first failing
/// draft — this test then gets an Err where it needs Ok, and one call
/// where it needs two.
#[test]
fn draft_and_gate_redrafts_a_failed_dry_run_once_and_accepts_the_fix() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("marker.txt"), "x").unwrap();
    let failing = draft_fixture("test -f t188-absent-marker.txt");
    let fixed = draft_fixture("test -f marker.txt");
    let (result, llm, _sink) = gate_with(
        &tmp,
        "draft a spec for the gate",
        vec![failing.clone(), fixed.clone()],
    );
    let spec_path = result.expect("the corrected second draft is accepted");
    assert_eq!(
        spec_path,
        tmp.path().join(crate::autospec::AUTO_SPEC_REL),
        "the accepted draft lives at .chug/auto-spec.md"
    );
    // THE m6 assertion: exactly TWO draft sessions ran (initial + one
    // redraft). The 1-attempt mutant stops at one call.
    assert_eq!(
        llm.calls.len(),
        2,
        "a dry-run-failed draft must be redrafted ONCE (m6: the 1-attempt mutant dies here)"
    );
    // The redraft session carried the failure as feedback in its brief.
    let redraft_brief = format!("{:?}", llm.calls[1].1);
    assert!(
        redraft_brief.contains("failed its dry-run"),
        "the redraft brief names the dry-run failure: {redraft_brief}"
    );
    assert!(
        redraft_brief.contains("t188-absent-marker"),
        "the redraft brief carries the failing check's output: {redraft_brief}"
    );
    // The ACCEPTED spec is the second draft's bytes — the failing check was
    // replaced by the redraft, never loosened.
    assert_eq!(
        fs::read_to_string(&spec_path).unwrap(),
        fixed,
        "the accepted file is the corrected redraft, byte-verbatim"
    );
}

/// Two dry-run failures abort with the draft + the failing output — the
/// check is never loosened to pass. THE m6 KILLER (leg 2): the 1-attempt
/// mutant consumes only ONE scripted draft; the two-call assertion dies,
/// and the abort wording (which names 2 attempts) is absent.
#[test]
fn draft_and_gate_aborts_after_two_failed_dry_runs_never_loosening() {
    let tmp = tempfile::tempdir().unwrap();
    let failing = draft_fixture("test -f t188-absent-marker.txt");
    let (result, llm, _sink) = gate_with(
        &tmp,
        "draft a spec for the gate",
        vec![failing.clone(), failing],
    );
    let err = result.expect_err("two failing dry-runs must abort");
    // Exactly two attempts ran — bounded redrafting, then the abort.
    assert_eq!(
        llm.calls.len(),
        2,
        "the gate runs exactly two draft sessions before aborting (m6: the 1-attempt mutant dies here)"
    );
    let err = err.to_string();
    assert!(
        err.contains("after 2 attempts"),
        "the abort names the attempt bound: {err}"
    );
    assert!(
        err.contains("never loosened"),
        "the abort states the no-loosening doctrine: {err}"
    );
    // The failing check and its output ride the error — the operator sees
    // WHY, not a silently-loosened pass.
    assert!(
        err.contains("test -f t188-absent-marker.txt"),
        "the abort names the failing check: {err}"
    );
    assert!(
        err.contains("must exit 0") || err.contains("exited 1"),
        "the abort carries the dry-run's failing output: {err}"
    );
    // The last draft is still on disk for inspection (failure honesty).
    let drafted = fs::read_to_string(tmp.path().join(crate::autospec::AUTO_SPEC_REL)).unwrap();
    assert!(
        drafted.contains("test -f t188-absent-marker.txt"),
        "the failing draft stays inspectable at .chug/auto-spec.md: {drafted}"
    );
}

/// A vacuous check is rejected and redrafted on the headless gate too (the
/// spec's dry-run-rejection test, on the draft_and_gate path): the first
/// draft's `check: true` is refused by the vacuous predicate (not just the
/// dry-run), the redraft's real check is accepted.
#[test]
fn draft_and_gate_redrafts_a_vacuous_check_once() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("marker.txt"), "x").unwrap();
    let vacuous = draft_fixture("true");
    let fixed = draft_fixture("test -f marker.txt");
    let (result, llm, _sink) = gate_with(
        &tmp,
        "draft a spec for the gate",
        vec![vacuous, fixed.clone()],
    );
    let spec_path = result.expect("the corrected redraft is accepted");
    assert_eq!(
        llm.calls.len(),
        2,
        "exactly one redraft for the vacuous check"
    );
    let redraft_brief = format!("{:?}", llm.calls[1].1);
    assert!(
        redraft_brief.contains("vacuous"),
        "the redraft brief names the vacuous rejection: {redraft_brief}"
    );
    assert_eq!(fs::read_to_string(&spec_path).unwrap(), fixed);
}

// ---------- F1: the end-to-end draft → run → goal_complete leg ----------

/// A transport double feeding scripted bodies through the REAL Client for
/// the run leg (the lock.rs family shape, family-local per the T84 rule:
/// helpers with callers in a single family live with that family).
struct E2eTransport(std::sync::Mutex<std::collections::VecDeque<Value>>);

impl crate::api::Transport for E2eTransport {
    fn send(
        &self,
        _url: &str,
        _headers: &[(String, String)],
        _body: &str,
    ) -> Result<crate::api::RawResponse, crate::api::TransportError> {
        let body = self
            .0
            .lock()
            .unwrap()
            .pop_front()
            .expect("e2e scripted transport exhausted");
        Ok(crate::api::RawResponse {
            status: 200,
            headers: Vec::new(),
            body: body.to_string(),
        })
    }
}

fn e2e_run_client(responses: Vec<Value>) -> Client {
    Client::with_transport_for_tests(
        Arc::new(E2eTransport(std::sync::Mutex::new(responses.into()))),
        "scripted-model",
        crate::api::DEFAULT_MAX_TOKENS,
    )
}

/// THE SPEC'S END-TO-END TEST (Tests §3): a scripted goal → draft (one
/// submit_plan through a mock Llm) → the acceptance gate dry-runs the
/// drafted check for real → the run proceeds against the drafted spec with
/// the original goal verbatim → goal_complete triggers the drafted spec's
/// check, which actually EXECUTES (green) → the goal is accepted, exit 0.
/// The whole auto-spec surface in one leg, on the seam each stage really
/// uses (ScriptedLlm for the draft session, the real Client over a
/// scripted transport for the run).
#[test]
fn auto_spec_e2e_draft_gates_then_runs_to_goal_acceptance() {
    let tmp = tempfile::tempdir().unwrap();
    // The drafted check's premise, green on the current tree.
    std::fs::write(tmp.path().join("marker.txt"), "x").unwrap();
    let spec_text = draft_fixture("test -f marker.txt");

    // (1) DRAFT: one mock-Llm plan session hands the complete spec to
    // submit_plan; the gate validates + dry-runs the check and accepts.
    let mut llm = crate::api::ScriptedLlm::new(vec![tool_use_response(
        "submit_plan",
        json!({ "plan": spec_text }),
    )]);
    let mut sink = RecordingSink::default();
    let spec_path = crate::autospec::draft_and_gate(
        tmp.path(),
        E2E_GOAL,
        "scripted-model",
        crate::api::DEFAULT_MAX_TOKENS,
        &mut llm,
        &mut sink,
    )
    .expect("the complete draft passes the acceptance gate");
    assert_eq!(spec_path, tmp.path().join(crate::autospec::AUTO_SPEC_REL));
    assert_eq!(
        fs::read_to_string(&spec_path).unwrap(),
        spec_text,
        "the drafted spec is written verbatim to .chug/auto-spec.md"
    );
    assert_eq!(llm.calls.len(), 1, "one draft call, no redraft");
    // One dry-run of the drafted check really executed (the marker was
    // tested — a vacuous or non-executing check would not have run it).

    // (2) RUN: the run proceeds against the drafted spec with the ORIGINAL
    // goal verbatim; the mock-backed Client answers goal_complete; the
    // goal gate executes the drafted spec's check for real.
    let cfg = RunConfig {
        cwd: tmp.path().to_path_buf(),
        spec_path: spec_path.clone(),
        goal: E2E_GOAL.to_string(),
        model: "scripted-model".to_string(),
        max_iters: 5,
        max_minutes: 10,
        max_tokens: 0,
        max_tokens_per_request: crate::api::DEFAULT_MAX_TOKENS,
        resume: false,
        controls: Controls::detached(),
        risk_gate: false,
        bash_timeout: Duration::from_secs(tools::BASH_TIMEOUT_SECS),
        mcp_config: None,
        mcp_off: true,
        goal_pack: None,
        ctx_warn_at_tokens: 0,
        approve: None,
    };
    let mut run_sink = RecordingSink::default();
    let code = run_loop(
        cfg,
        e2e_run_client(vec![tool_use_response(
            "goal_complete",
            json!({ "summary": "wired the draft gate" }),
        )]),
        None,
        &mut run_sink,
        &observ::Sink::Noop,
    )
    .unwrap();
    assert_eq!(
        code, 0,
        "the run accepts the goal on the drafted spec's check"
    );

    // (3) The run's own events stream pins the whole chain: it started
    // against the DRAFTED spec with the ORIGINAL goal, the drafted check
    // was EXECUTED as the verification, and the goal was accepted on it.
    let lines: Vec<Value> = fs::read_to_string(tmp.path().join(".chug/events.jsonl"))
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let start = lines
        .iter()
        .find(|l| l["type"] == "run_start")
        .expect("the run's run_start line");
    assert_eq!(start["mode"], "run");
    assert_eq!(
        start["spec"],
        spec_path.display().to_string(),
        "the run proceeded against the drafted spec"
    );
    assert_eq!(
        start["goal_sha256"],
        crate::eventlog::goal_sha256(E2E_GOAL),
        "the run's goal is the original goal, verbatim"
    );
    let verifying = lines
        .iter()
        .find(|l| l["type"] == "verifying")
        .expect("the drafted check was actually executed as the verification");
    assert_eq!(
        verifying["cmd"], "test -f marker.txt",
        "the verification is the DRAFTED spec's check line"
    );
    assert!(
        lines
            .iter()
            .any(|l| l["type"] == "goal" && l["outcome"] == "accepted"),
        "the goal is accepted on the drafted spec's check: {lines:?}"
    );
}
