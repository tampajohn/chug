//! T81 — per-phase model routing: loopd.sh picks the ORCHESTRATOR's model
//! per cycle from the freshness rule (never model judgment), LOOP-SPEC
//! carries the model-agnostic anti-sprint-burn guard, and validation stays
//! kimi regardless of who orchestrates (family independence — glm never
//! validates glm).
//!
//! Two layers, the tests/shared_target_dir.rs + tests/eval_digest.rs
//! patterns:
//! - static pins on the wiring: the two env knobs with current-behavior
//!   defaults (LOOP_ORCH_MODEL=kimi-k3, LOOP_ROUTINE_MODEL=glm), the
//!   switch's DIRECTION (routine → LOOP_ROUTINE_MODEL, eval →
//!   LOOP_ORCH_MODEL), the rollback comment, the `routing` subcommand, and
//!   the `--model "$orch_model"` invocation (the hardcoded kimi is gone);
//! - BEHAVIORAL tests that run the real `loopd.sh routing` mode against
//!   fixture TODO.md / EVALUATION.md files in a temp dir (the script cd's
//!   to its own directory, so a copied script + fixtures is a faithful
//!   harness; no repo file is touched). CHUG_ROUTINE_TODAY pins the
//!   "today" side of the freshness comparison (the CHUG_DIGEST_NOW
//!   pattern) and file mtimes are set deterministically, so no test can
//!   flake on a UTC-midnight crossing.
//!
//! T48 doctrine: pins resolve files from the checkout the binary RUNS
//! against (`std::env::current_dir()`; cargo runs test binaries with cwd =
//! the package root), never via the compile-time manifest-dir macro.

#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

fn repo_root() -> PathBuf {
    std::env::current_dir().expect("cargo sets the test cwd to the package root")
}

fn read(rel: &str) -> String {
    std::fs::read_to_string(repo_root().join(rel))
        .unwrap_or_else(|e| panic!("reading {rel}: {e}"))
}

/// Per-carrier pin (the T47 count_eq pattern): `needle` must occur EXACTLY
/// `expected` times in `haystack`.
fn count_eq(haystack: &str, needle: &str, expected: usize, what: &str) {
    let found = haystack.matches(needle).count();
    assert_eq!(
        found, expected,
        "{what}: expected {needle:?} exactly {expected}×, found {found}×"
    );
}

// --- loopd.sh wiring pins ----------------------------------------------------

/// The eval-cycle default — TODAY's behavior (kimi-k3), so the knob changes
/// nothing until the operator opts in.
const ORCH_DEFAULT: &str =
    "LOOP_ORCH_MODEL=\"${LOOP_ORCH_MODEL:-anthropic-system.ai.kimi-k3}\"";
/// The routine-cycle default — the operator-approved knob (glm-5-3-flash).
const ROUTINE_DEFAULT: &str =
    "LOOP_ROUTINE_MODEL=\"${LOOP_ROUTINE_MODEL:-anthropic-system.ai.glm-5-3-flash}\"";
/// The rollback: one env var restores single-model operation (req 4). The
/// default-assignment lines use `:-`, so the `=` form is unique to the
/// rollback comment.
const ROLLBACK: &str = "LOOP_ROUTINE_MODEL=anthropic-system.ai.kimi-k3";
/// The switch's direction — the load-bearing wiring. A mutant that swaps
/// the two arms (routine→kimi, eval→glm) silently inverts the item.
const ROUTINE_ARM: &str = "routine $LOOP_ROUTINE_MODEL";
const EVAL_ARM: &str = "eval $LOOP_ORCH_MODEL";
/// The per-cycle routing computation in the while body, before the launch.
const ROUTE_CALL: &str = "routing=\"$(route TODO.md EVALUATION.md)\"";
/// The launch line now carries the routed model — the hardcoded kimi of
/// today's script is gone. The iters cap is pinned too: T121 raised it
/// 160→200 (cycle-61 died 160/160 post-wrap, pre-goal_complete). T198
/// raised minutes 240→360 (the fleet outgrew the wall: cycle-85–90 walls
/// 3h23m–4h01m, cap hit once at cycle 88; 360 = p95 × 1.5; iterations
/// stay 200). T230 re-keyed the carrier once more: the orchestrator
/// launch now ALSO carries the 100k occupancy nudge
/// (`--ctx-warn-at-tokens`, T192) — a measurement row, not a behavior
/// assertion (the LIVE_CTX surface had 0 production fires because
/// nothing ever set the flag); the pin follows the deliberately re-keyed
/// carrier (the T187 pattern), and the exactly-once leg in
/// `loopd_launches_the_ctx_warn_nudge_exactly_once` guards a duplicate
/// flag, which would silently re-latch the one-shot notice.
const INVOCATION_MODEL: &str = "--model \"$orch_model\" --max-iters 200 --max-minutes 360 --ctx-warn-at-tokens 100000";
/// The loopd.log routing line: every model change is explained in the log
/// (the T50 re-exec-log rule).
const ROUTING_LOG: &str = "routing: todo_rows=";

#[test]
fn loopd_carries_both_model_knobs_with_current_behavior_defaults() {
    let loopd = read("loopd.sh");
    count_eq(&loopd, ORCH_DEFAULT, 1, "LOOP_ORCH_MODEL default (T81)");
    count_eq(&loopd, ROUTINE_DEFAULT, 1, "LOOP_ROUTINE_MODEL default (T81)");
    count_eq(&loopd, ROLLBACK, 1, "rollback comment (T81 req 4)");
    // Both knobs precede the cycle loop: every cycle routes through them.
    let orch = loopd
        .find(ORCH_DEFAULT)
        .expect("LOOP_ORCH_MODEL default present");
    let routine = loopd
        .find(ROUTINE_DEFAULT)
        .expect("LOOP_ROUTINE_MODEL default present");
    let loop_top = loopd
        .find("while [ ! -f \"$STOP\" ]")
        .expect("loopd.sh has a supervisor loop");
    assert!(
        orch < loop_top && routine < loop_top,
        "both model knobs must be defined before the cycle loop (T81)"
    );
}

#[test]
fn loopd_switch_direction_and_predicate_wiring() {
    let loopd = read("loopd.sh");
    count_eq(&loopd, ROUTINE_ARM, 1, "routine arm routes to ROUTINE_MODEL");
    count_eq(&loopd, EVAL_ARM, 1, "eval arm routes to ORCH_MODEL");
    // The routine arm is gated on BOTH halves of the freshness predicate —
    // queue non-empty AND fresh evaluation — in one conjunctive condition.
    let route_fn = loopd
        .find("route() {")
        .expect("the route() decision function exists");
    let route_end = loopd[route_fn..]
        .find("\n}")
        .map(|i| route_fn + i)
        .expect("route() closes");
    let route = &loopd[route_fn..=route_end];
    assert!(
        route.contains("todo_rows \"$1\"") && route.contains("-gt 0"),
        "route() must gate the routine arm on a non-empty todo queue"
    );
    assert!(
        route.contains("eval_fresh \"$2\""),
        "route() must gate the routine arm on a fresh evaluation"
    );
    count_eq(&loopd, ROUTE_CALL, 1, "per-cycle route() call (T81)");
    count_eq(&loopd, ROUTING_LOG, 1, "loopd.log routing line (T81)");
    count_eq(
        &loopd,
        "usage: loopd.sh [run|stop|status|routing]",
        1,
        "usage line names the routing subcommand",
    );
}

#[test]
fn loopd_launches_the_routed_model_not_a_hardcoded_one() {
    let loopd = read("loopd.sh");
    count_eq(
        &loopd,
        INVOCATION_MODEL,
        1,
        "chug invocation carries the routed model (T81)",
    );
    assert!(
        !loopd.contains("--model anthropic-system.ai.kimi-k3"),
        "loopd.sh must not hardcode `--model anthropic-system.ai.kimi-k3` on \
         the cycle invocation any more — the orchestrator model is routed \
         per cycle (T81); the only kimi-k3 literals left are the knob \
         defaults and the rollback comment"
    );
    // The routing decision is computed BEFORE the launch it governs.
    let route_call = loopd
        .find(ROUTE_CALL)
        .expect("per-cycle routing computation present");
    let invocation = loopd
        .find("CARGO_TARGET_DIR=\"$ROOT/target-shared\" ./target/release/chug run")
        .expect("the env-prefixed cycle invocation (T47/T78) is present");
    assert!(
        route_call < invocation,
        "the per-cycle routing must be computed before the chug launch it \
         governs (T81)"
    );
}

/// T230 — the orchestrator launch carries the 100k occupancy nudge
/// EXACTLY ONCE in loopd.sh. The notice is a one-shot latch
/// (`ctx_warned`, src/driver.rs:1136) and argv holds the flag once, so a
/// duplicated flag would not error — the later value would silently win
/// and the measurement row's semantics would be re-keyed by accident.
/// The INVOCATION_MODEL pin above already proves the flag's adjacency to
/// the launch carrier (one contiguous needle), but it would PASS a
/// second flag with a different value; THIS leg counts the bare
/// space-form needle (` --ctx-warn-at-tokens ` — the comment's backticked
/// mention does not match), so a duplicate with ANY value — same or
/// different — counts 2 and dies. RED-proofs: removing the flag from the
/// launch line kills the INVOCATION_MODEL pin; duplicating it kills this
/// count_eq.
#[test]
fn loopd_launches_the_ctx_warn_nudge_exactly_once() {
    let loopd = read("loopd.sh");
    count_eq(
        &loopd,
        " --ctx-warn-at-tokens ",
        1,
        "the 100k occupancy nudge on the orchestrator launch (T230)",
    );
}

/// T178 — loopd exports `CHUG_BASH_TIMEOUT=300` beside the launch block so
/// the orchestrator AND every delegate child (children inherit the
/// orchestrator's process env — delegate has no env parameter) get the 300s
/// bash-tool cap instead of the 120s default. The export must sit BEFORE the
/// launch it governs (the T81 pre-launch shape, applied to an env knob), and
/// the comment must name the precedence chain's carrier (flag > env > 120,
/// src/main.rs:370). RED-proof: reverting the loopd.sh export or its comment
/// fails this pin.
#[test]
fn loopd_exports_bash_timeout_300_before_the_launch() {
    let loopd = read("loopd.sh");
    count_eq(
        &loopd,
        "export CHUG_BASH_TIMEOUT=300",
        1,
        "loopd.sh exports the fleet bash cap exactly once (T178)",
    );
    let export = loopd
        .find("export CHUG_BASH_TIMEOUT=300")
        .expect("the fleet bash-cap export is present");
    let invocation = loopd
        .find("CARGO_TARGET_DIR=\"$ROOT/target-shared\" ./target/release/chug run")
        .expect("the env-prefixed cycle invocation (T47/T78) is present");
    assert!(
        export < invocation,
        "the CHUG_BASH_TIMEOUT export must precede the chug launch it \
         governs (T178)"
    );
    assert!(
        loopd.contains("src/main.rs:370"),
        "the T178 comment must name the precedence carrier (flag > env > 120 \
         default, src/main.rs:370)"
    );
}

// --- LOOP-SPEC doctrine pins --------------------------------------------------

/// The anti-sprint-burn guard (req 2): model-agnostic, >5 consecutive
/// iterations without a child launch MUST act (launch, merge, or wrap) —
/// the M2/M3 anti-sprint-burn rule made explicit for any orchestrator
/// model, because glm-flash is also ~30B-class and must inherit the
/// delegation-forcing structure, not just the model name.
const GUARD: &str = "Anti-sprint-burn guard";
const GUARD_CAP: &str = "MORE than 5 consecutive iterations";
const GUARD_ACT: &str = "MUST act";
const GUARD_AGNOSTIC: &str = "model-agnostic";

/// Family independence (req 3): validation stays kimi whenever the
/// implementer is glm — spelled in the header AND at the step-4 launch
/// point, so the count is 2 spec-wide.
const NEVER_GLM_VALIDATES_GLM: &str = "never lets glm validate glm";
/// The header's routing mechanics (req 1's spec half): the switch is the
/// freshness rule, decided before launch — never model judgment.
const ROUTED_BEFORE_LAUNCH: &str = "picked by `loopd.sh` BEFORE";
/// The Phase-1 boundary that keeps glm-orchestrated fresh evaluations out
/// of scope: glm never evaluates; a mismatch wraps, it does not evaluate.
const GLM_NEVER_EVALUATES: &str = "glm never runs this phase";

#[test]
fn loop_spec_carries_the_model_agnostic_anti_sprint_burn_guard() {
    let spec = read("LOOP-SPEC.md");
    count_eq(&spec, GUARD, 1, "the anti-sprint-burn guard's name (T81 req 2)");
    count_eq(
        &spec,
        GUARD_CAP,
        1,
        "the >5-consecutive-iterations cap (T81 req 2)",
    );
    count_eq(
        &spec,
        GUARD_ACT,
        1,
        "the MUST-act remedy (launch, merge, or wrap) (T81 req 2)",
    );
    count_eq(
        &spec,
        GUARD_AGNOSTIC,
        1,
        "the model-agnostic stance (T81 req 2)",
    );
    // The remedy names all three acts.
    let guard_at = spec.find(GUARD).expect("guard present");
    let window = &spec[guard_at..(guard_at + 900).min(spec.len())];
    for act in ["launch the next dispatchable child", "merge a reviewed", "wrap"] {
        assert!(
            window.contains(act),
            "the guard's remedy must name {act:?} — launch, merge, or wrap \
             (T81 req 2); got:\n{window}"
        );
    }
    // The guard lives in the Hard rules section, where the caps it defers
    // to are stated.
    let hard_rules = spec
        .find("## Hard rules")
        .expect("LOOP-SPEC has a Hard rules section");
    assert!(
        guard_at > hard_rules,
        "the anti-sprint-burn guard must live in the Hard rules section (T81)"
    );
}

#[test]
fn loop_spec_carries_family_independence_and_the_routing_mechanics() {
    let spec = read("LOOP-SPEC.md");
    // Req 3: the header statement + the step-4 launch point.
    count_eq(
        &spec,
        NEVER_GLM_VALIDATES_GLM,
        2,
        "family-independence carriers: header + step-4 launch (T81 req 3)",
    );
    count_eq(
        &spec,
        "ALWAYS kimi whatever",
        2,
        "validation-is-always-kimi carriers: header + step-4 (T81 req 3)",
    );
    // Req 1's spec half: the orchestrator model is routed before launch
    // from the freshness rule, and glm never evaluates.
    count_eq(
        &spec,
        ROUTED_BEFORE_LAUNCH,
        1,
        "the routing-is-pre-launch-and-rule-driven mechanics (T81 req 1)",
    );
    count_eq(
        &spec,
        GLM_NEVER_EVALUATES,
        1,
        "the glm-never-evaluates boundary at Phase 1 (T81)",
    );
    // The step-4 carrier sits inside step 4's window (T64 heading-scope
    // pattern) — the independence rule at the point the validator is
    // launched, not only in the header.
    let step4 = spec
        .find("4. **Adversarial validation")
        .expect("LOOP-SPEC step-4 heading present");
    let step5 = spec[step4..]
        .find("5. **Harvest")
        .map(|i| step4 + i)
        .expect("LOOP-SPEC step-5 heading after step 4");
    let window = &spec[step4..step5];
    count_eq(
        window,
        NEVER_GLM_VALIDATES_GLM,
        1,
        "step-4 window family-independence carrier (T81 req 3)",
    );
}

// --- behavioral tests: the real `loopd.sh routing` against fixtures ----------

/// 2026-09-27T12:00:00Z — the pinned "fresh" mtime (mid-day, so the UTC-day
/// bucket is unambiguous) and the pinned TODAY that pairs with it.
const FRESH_EPOCH: u64 = 1_790_510_400;
const FRESH_DAY: &str = "2026-09-27";
/// The day after — pairs with the same FRESH_EPOCH mtime to make the
/// evaluation STALE.
const STALE_DAY: &str = "2026-09-28";
const KIMI: &str = "anthropic-system.ai.kimi-k3";
const GLM: &str = "anthropic-system.ai.glm-5-3-flash";

/// A TODO.md fixture in the real table shape (the todo_consistency parser's
/// format: 6 cells, `|`-separated, status in cell 5).
fn todo_table(rows: &[(&str, &str)]) -> String {
    let mut s = String::from(
        "| id | title | spec | pri | status | notes |\n\
         |----|-------|------|-----|--------|-------|\n",
    );
    for (id, status) in rows {
        s.push_str(&format!(
            "| {id} | title | specs/{id}-slug.md | 2 | {status} | notes |\n"
        ));
    }
    s
}

/// Fixture dir: a copy of the real loopd.sh (plus its T213 loader fragment)
/// plus TODO.md/EVALUATION.md.
/// The script cd's to its own directory, so the copy sees only the
/// fixtures. Returns the TempDir guard (RAII cleanup); use `.path()`.
fn fixture_dir(todo: Option<String>) -> tempfile::TempDir {
    let tmp = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        tmp.path().join("loopd.sh"),
        std::fs::read_to_string(repo_root().join("loopd.sh")).expect("loopd.sh readable"),
    )
    .expect("copy loopd.sh into fixture dir");
    // T213: loopd.sh sources scripts/loopd_env_loader.sh relative to its own
    // path (the M4 loader extraction) — the fixture copy needs the fragment
    // beside it or the sourcing dies under set -euo pipefail before any
    // mode dispatch (the reaper + daemon_ensure sandboxes copy scripts/
    // wholesale; this minimal fixture copies exactly what loopd.sh needs).
    let scripts = tmp.path().join("scripts");
    std::fs::create_dir_all(&scripts).expect("fixture scripts dir");
    std::fs::copy(
        repo_root().join("scripts/loopd_env_loader.sh"),
        scripts.join("loopd_env_loader.sh"),
    )
    .expect("copy loopd_env_loader.sh into fixture dir");
    if let Some(todo) = todo {
        std::fs::write(tmp.path().join("TODO.md"), todo).expect("write fixture TODO.md");
    }
    tmp
}

/// Write EVALUATION.md with its mtime pinned to FRESH_EPOCH (2026-09-27
/// 12:00 UTC) — deterministic regardless of when the suite runs.
fn write_fresh_evaluation(root: &Path) {
    use std::fs::FileTimes;
    let path = root.join("EVALUATION.md");
    std::fs::write(&path, "# Eval\n").expect("write fixture EVALUATION.md");
    let f = std::fs::File::options()
        .write(true)
        .open(&path)
        .expect("open fixture EVALUATION.md for mtime pinning");
    f.set_times(FileTimes::new().set_modified(
        std::time::SystemTime::UNIX_EPOCH + Duration::from_secs(FRESH_EPOCH),
    ))
    .expect("pin fixture mtime");
}

/// Run the copied script's `routing` mode with the default env CLEARED of
/// the three routing variables, then the caller's overrides applied.
fn run_routing(root: &Path, envs: &[(&str, &str)]) -> String {
    let mut cmd = Command::new("bash");
    cmd.arg(root.join("loopd.sh"))
        .arg("routing")
        .current_dir(root)
        .env_remove("LOOP_ORCH_MODEL")
        .env_remove("LOOP_ROUTINE_MODEL")
        .env_remove("CHUG_ROUTINE_TODAY");
    for (k, v) in envs {
        cmd.env(k, v);
    }
    let out = cmd.output().expect("spawn loopd.sh routing");
    assert!(
        out.status.success(),
        "routing exited {:?}: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// Queue non-empty + evaluation fresh (same UTC day) → ROUTINE cycle on the
/// ROUTINE model: the freshness rule would skip Phase 1, so glm
/// orchestrates (req 1).
#[test]
fn routine_cycle_routes_to_the_routine_model() {
    let root = fixture_dir(Some(todo_table(&[("T1", "done"), ("T2", "todo")])));
    write_fresh_evaluation(root.path());
    let root = root.path();
    assert_eq!(
        run_routing(root, &[("CHUG_ROUTINE_TODAY", FRESH_DAY)]),
        format!("routine {GLM}"),
        "non-empty queue + same-UTC-day EVALUATION.md → routine glm"
    );
}

/// Each freshness leg is load-bearing: dropping the queue or going stale
/// flips the cycle to fresh-eval kimi.
#[test]
fn each_freshness_leg_is_load_bearing() {
    // (a) queue EMPTY (no todo rows) → fresh-eval kimi, even with a fresh
    //     evaluation — the freshness rule would NOT skip Phase 1.
    let root = fixture_dir(Some(todo_table(&[("T1", "done"), ("T2", "done")])));
    write_fresh_evaluation(root.path());
    let root = root.path();
    assert_eq!(
        run_routing(root, &[("CHUG_ROUTINE_TODAY", FRESH_DAY)]),
        format!("eval {KIMI}"),
        "empty queue → fresh-eval kimi (the todo-rows leg is load-bearing)"
    );
    // (b) evaluation STALE (mtime yesterday against a pinned tomorrow) →
    //     fresh-eval kimi, even with todo rows.
    let root = fixture_dir(Some(todo_table(&[("T1", "todo")])));
    write_fresh_evaluation(root.path());
    let root = root.path();
    assert_eq!(
        run_routing(root, &[("CHUG_ROUTINE_TODAY", STALE_DAY)]),
        format!("eval {KIMI}"),
        "stale EVALUATION.md → fresh-eval kimi (the freshness leg is \
         load-bearing)"
    );
}

/// Missing files default to the SAFE side: no queue knowledge → fresh-eval
/// kimi (a routine launch must never fire on missing evidence).
#[test]
fn missing_files_default_to_a_fresh_eval_cycle() {
    let root = fixture_dir(None);
    let root = root.path();
    assert_eq!(
        run_routing(root, &[]),
        format!("eval {KIMI}"),
        "missing TODO.md/EVALUATION.md → fresh-eval kimi (safe default)"
    );
}

/// Req 4's rollback, behaviorally: LOOP_ROUTINE_MODEL=kimi-k3 makes even a
/// routine cycle launch kimi — single-model operation restored with one env
/// var, no other change.
#[test]
fn rollback_env_restores_single_model_operation() {
    let root = fixture_dir(Some(todo_table(&[("T1", "todo")])));
    write_fresh_evaluation(root.path());
    let root = root.path();
    assert_eq!(
        run_routing(
            root,
            &[
                ("CHUG_ROUTINE_TODAY", FRESH_DAY),
                ("LOOP_ROUTINE_MODEL", KIMI),
            ]
        ),
        format!("routine {KIMI}"),
        "LOOP_ROUTINE_MODEL=kimi-k3 → routine cycles launch kimi (req 4)"
    );
}

/// The other knob is honored too (the eval arm routes through
/// LOOP_ORCH_MODEL, not a second hardcoded literal).
#[test]
fn orch_model_env_override_is_honored() {
    let root = fixture_dir(Some(todo_table(&[("T1", "todo")])));
    write_fresh_evaluation(root.path());
    let root = root.path();
    assert_eq!(
        run_routing(
            root,
            &[
                ("CHUG_ROUTINE_TODAY", STALE_DAY),
                ("LOOP_ORCH_MODEL", "test-orch-model"),
            ]
        ),
        "eval test-orch-model",
        "LOOP_ORCH_MODEL override reaches the eval arm"
    );
}
