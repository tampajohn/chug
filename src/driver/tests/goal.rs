// T104 family: goal — goal-rejection environment honesty (T9). Moved bytes byte-identical (T84 rule)
// from driver.rs's test module; every test here lives in exactly one family
// file.
use super::*; // the shared harness (driver::tests) + driver's own imports
// ---------- T9: goal-rejection environment honesty ----------

#[test]
fn goal_rejected_message_states_check_environment() {
    let msg = goal_rejected_message("$ cargo test\nexit code: 127\nsh: cargo: command not found");
    // The three key facts (T9): the check shares the bash tool's
    // run_shell wrapper, the cargo PATH prepend, and the out-of-cwd
    // prohibition. The literals are derived from the shell wrapper's own
    // constants, so drift between the message and reality breaks here.
    assert!(
        msg.contains("same shell wrapper as your bash tool"),
        "{msg}"
    );
    assert!(
        msg.contains(&format!("~/{}", tools::CARGO_BIN_REL)),
        "{msg}"
    );
    assert!(
        msg.contains(&format!("{}s timeout", tools::CHECK_TIMEOUT_SECS)),
        "{msg}"
    );
    assert!(
        msg.contains("do NOT create or modify files outside the run cwd"),
        "{msg}"
    );
    // T144: the note names the target-dir scrub — both spellings — and
    // states that a shared cache is an explicit in-check choice.
    assert!(msg.contains("`CARGO_TARGET_DIR`"), "{msg}");
    assert!(msg.contains("`CARGO_BUILD_TARGET_DIR`"), "{msg}");
    assert!(msg.contains("not an inheritance accident"), "{msg}");
    // The original guidance and the failing output are preserved.
    assert!(msg.contains("Fix the failure and try again"), "{msg}");
    assert!(msg.contains("sh: cargo: command not found"), "{msg}");
}

#[test]
fn goal_rejection_includes_environment_note_in_tool_result() {
    // End to end through drive_loop: a goal_complete with a failing
    // check puts the T9 environment note in the user message the model
    // sees next iteration.
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
    let mut knobs = knobs_with(1); // abort right after the first iteration
    let mut llm = ScriptedLlm::new(vec![tool_use_response(
        "goal_complete",
        json!({"summary": "claim done"}),
    )]);
    let mut gate = None;
    let mut messages = Vec::new();
    let outcome = drive_loop(
        &ctx,
        &mut knobs,
        &mut llm,
        &mut gate,
        &mut messages,
        Some("check: false".to_string()),
        &mut RecordingSink::default(),
        &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
    )
    .unwrap();
    assert!(matches!(outcome, DriveOutcome::RunFinished(1)));
    let rejection: String = messages
        .last()
        .expect("rejection user message")
        .content
        .iter()
        .filter_map(|b| b.text())
        .collect();
    assert!(rejection.contains("goal_complete rejected"), "{rejection}");
    assert!(
        rejection.contains("same shell wrapper as your bash tool"),
        "{rejection}"
    );
    assert!(
        rejection.contains("do NOT create or modify files outside the run cwd"),
        "{rejection}"
    );
}

/// T144 integration leg, goal-gate-shaped: with BOTH target-dir
/// spellings seeded in the driver process env (the loopd.sh
/// per-invocation prefix shape — this is exactly the env
/// `tools::run_shell` inherits), a spec `check:` that echoes them into a
/// file must record them UNSET. The check shares the bash tool's
/// `run_shell`, and run_shell scrubs the inherited variables at the
/// spawn boundary: a bare `cargo test` check builds `<cwd>/target`
/// (content-correct by construction), while an in-command prefix stays
/// the warm shared-cache path. Pre-fix this leg is RED (the file records
/// the foreign paths); it also dies when only ONE of the two spellings
/// is scrubbed.
#[test]
fn goal_check_runs_without_inherited_target_dir() {
    let _guard = crate::delegate::tests::DELEGATE_ENV_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let tmp = tempfile::tempdir().unwrap();
    // SAFETY: serialized by DELEGATE_ENV_LOCK (one env, one lock); both
    // values restored before the asserts.
    let saved_target = std::env::var_os("CARGO_TARGET_DIR");
    let saved_alias = std::env::var_os("CARGO_BUILD_TARGET_DIR");
    unsafe {
        std::env::set_var("CARGO_TARGET_DIR", "/tmp/t144-foreign-shared-target");
        std::env::set_var("CARGO_BUILD_TARGET_DIR", "/tmp/t144-foreign-alias-target");
    }
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
    let mut knobs = knobs_with(3); // accepted on iteration 1; slack unused
    let check = "printf '%s|%s' \"${CARGO_TARGET_DIR:-UNSET}\" \
                     \"${CARGO_BUILD_TARGET_DIR:-UNSET}\" > t144-check-env.txt";
    let mut llm = ScriptedLlm::new(vec![tool_use_response(
        "goal_complete",
        json!({"summary": "t144 claims the goal"}),
    )]);
    let mut gate = None;
    let mut messages = Vec::new();
    let outcome = drive_loop(
        &ctx,
        &mut knobs,
        &mut llm,
        &mut gate,
        &mut messages,
        Some(format!("check: {check}")),
        &mut RecordingSink::default(),
        &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
    )
    .unwrap();
    // SAFETY: serialized by DELEGATE_ENV_LOCK; restore before asserting
    // (a panic below must not leak the seed into sibling tests).
    match saved_target {
        Some(v) => unsafe { std::env::set_var("CARGO_TARGET_DIR", v) },
        None => unsafe { std::env::remove_var("CARGO_TARGET_DIR") },
    }
    match saved_alias {
        Some(v) => unsafe { std::env::set_var("CARGO_BUILD_TARGET_DIR", v) },
        None => unsafe { std::env::remove_var("CARGO_BUILD_TARGET_DIR") },
    }
    // The check exited 0 (its output was redirected), so the goal is
    // accepted — and the file it wrote records BOTH spellings unset.
    assert!(
        matches!(outcome, DriveOutcome::RunFinished(0)),
        "{outcome:?}"
    );
    let recorded = std::fs::read_to_string(tmp.path().join("t144-check-env.txt"))
        .expect("check wrote the env file");
    assert_eq!(
        recorded, "UNSET|UNSET",
        "the goal-gate check must see both target-dir spellings unset"
    );
}
