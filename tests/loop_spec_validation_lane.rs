//! T189 — doctrine pins: LOOP-SPEC §2 step 4's low-stakes validation lane.
//!
//! kimi adversarial validation costs 15–30 min per item — right for core
//! logic, overkill for chores. The lane generalizes step 4's docs/tests
//! skip into an explicit, MECHANICAL predicate: gates-only when ALL four
//! hold — (a) no core-list file touched, (b) ≤ ~150 changed lines, (c) no
//! new tool/command surface, (d) no CI/workflow or spec `check:` line
//! change — otherwise full adversarial validation. The predicate's inputs
//! are computed from the diff (never the model's say-so), the routing is
//! recorded via decision_log naming the four inputs, and the lane changes
//! review DEPTH only: the gates and the quality floor are never weakened.
//!
//! These pins assert the lane is stated where the spec requires — inside
//! step 4's opening paragraph, which also keeps step 4's REQUIRED list and
//! the docs-only escape intact — with no step renumbering (the T19/T30
//! rule). The same conjunction's reference implementation + pins live in
//! src/valroute.rs; this file pins the DOCTRINE side (the two cannot drift
//! apart without one of the two pin sets going red).
//!
//! T48 doctrine: every pin resolves LOOP-SPEC.md from the checkout the
//! binary RUNS against (`std::env::current_dir()`; cargo runs test binaries
//! with cwd = the package root), never via the compile-time manifest-dir
//! macro — under the T47 shared cache a compile-time path can point at a
//! since-removed worktree.

/// Step 4's opening and step 5's heading, matched LOOSELY (the T64
/// heading-scope pattern): number + bold marker only, so a wording tweak
/// of a heading's text cannot break the scope legs. Each is unique in the
/// file today; `find` + `find`-after scopes the step-4 window.
const STEP4: &str = "4. **Adversarial validation";
const STEP5: &str = "5. **Harvest";

/// The lane's name-and-scope sentence: decided per item, BEFORE dispatch
/// (so the T44/T161 overlap gate reads the same file lists), and
/// MECHANICAL. Each needle must occur EXACTLY once in LOOP-SPEC.md.
const LANE_NAMED: &str = "low-stakes lane (T189) is decided per item BEFORE dispatch";

/// The conjunctive shape: ALL four must hold for gates-only.
const ALL_HOLD: &str = "gates-only when ALL hold";

/// Clause (a): the core-list file half and the doctrine clause — any
/// doctrine edit stays FULL validation.
const NO_CORE_FILE: &str = "the diff touches NO core-list file";
const DOCTRINE_CLAUSE: &str =
    "any LOOP-SPEC/META-SPEC/META-META-SPEC/SELF-SPEC edit stays FULL validation";

/// Clause (b): the line budget.
const LINE_BUDGET: &str = "~150 changed lines";

/// Clause (c): the new-surface categories.
const NEW_SURFACE: &str = "no new tool/command surface";

/// Clause (d): the CI/workflow + spec `check:` half.
const CHECK_OR_CI: &str = "no CI/workflow or spec `check:` line change";

/// Requirement 2: the inputs are computed from the diff, never the model's
/// say-so, and the routing record names all four inputs plus the verdict.
const FROM_DIFF: &str = "computed from the diff";
const NOT_SAY_SO: &str = "never the model's say-so";
const RECORD_NAMED: &str = "names all four inputs plus the verdict";

/// Requirement 3 + 5: the lane changes review depth, never the gates or
/// the quality floor — the gates-still-required floor, and the fix-up arc
/// survives for lane-eligible diffs the gates catch red.
const REVIEW_DEPTH: &str = "review depth, never the gates or the quality floor";
const GATES_STILL: &str = "the lane skips the kimi child, never the gates";
const FIXUP_STILL: &str = "lane-eligible diff the gates catch red";

/// Requirement 4: the auto-spec default + the two operator overrides.
const AUTO_SPEC_DEFAULT: &str = "default to the lane predicate";
const VALIDATE_FLAG: &str = "`--validate` forces full adversarial";
const NO_VALIDATE_FLAG: &str = "`--no-validate` forces gates-only";
const OVERRIDE_RECORDED: &str = "either operator override is recorded";

fn loop_spec() -> String {
    let root = std::env::current_dir().expect("cargo sets the test cwd to the package root");
    std::fs::read_to_string(root.join("LOOP-SPEC.md"))
        .unwrap_or_else(|e| panic!("reading LOOP-SPEC.md from the runtime checkout: {e}"))
}

/// Wrap-insensitive copy of the spec (the T78 flat idiom, hardened for the
/// step paragraphs' 3-space continuation indent): the lane sentence wraps
/// mid-phrase, so a multi-word needle must match whitespace-collapsed text
/// or the pin goes red on the REAL doctrine.
fn flat(spec: &str) -> String {
    spec.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Step-4 window helper (T64 heading-scope pattern): the text between
/// step 4's loose heading and step 5's, flat-collapsed for needle checks.
fn step4_window(spec: &str) -> String {
    let start = spec
        .find(STEP4)
        .unwrap_or_else(|| panic!("LOOP-SPEC must carry step 4's heading {STEP4:?}"));
    let end = start
        + spec[start..]
            .find(STEP5)
            .unwrap_or_else(|| panic!("LOOP-SPEC must carry step 5's heading {STEP5:?} after it"));
    flat(&spec[start..end])
}

/// The lane is named and scoped exactly once, inside step 4: decided per
/// item BEFORE dispatch (the overlap gate reads the same file lists), and
/// mechanical. Delete the lane block and this goes red (count 0); stating
/// it twice also goes red.
#[test]
fn lane_is_named_once_inside_step_4() {
    // Needle self-check (T48 idiom): a mangled needle must not let this
    // pin pass silently.
    assert!(
        LANE_NAMED.contains("low-stakes lane") && LANE_NAMED.contains("BEFORE dispatch"),
        "the needle must carry the lane's name and scope"
    );
    let spec = loop_spec();
    assert_eq!(
        flat(&spec).matches(LANE_NAMED).count(),
        1,
        "LOOP-SPEC must name the low-stakes lane exactly once — zero means \
         the lane was dropped, more than one means it is stated twice"
    );
    assert!(
        step4_window(&spec).contains(LANE_NAMED),
        "the lane statement must live inside step 4's opening paragraph"
    );
}

/// The four clauses are each stated exactly once, inside step 4's window:
/// the predicate is conjunctive (gates-only when ALL hold) and any single
/// flipped clause forces full adversarial validation.
#[test]
fn all_four_clauses_stated_once_inside_step_4() {
    assert!(
        NO_CORE_FILE.contains("NO core-list file")
            && LINE_BUDGET.contains("150")
            && NEW_SURFACE.contains("tool/command surface")
            && CHECK_OR_CI.contains("check:") ,
        "the needles must carry the four clauses"
    );
    let spec = loop_spec();
    for (needle, what) in [
        (ALL_HOLD, "the conjunctive ALL-hold shape"),
        (NO_CORE_FILE, "clause (a)'s core-list half"),
        (DOCTRINE_CLAUSE, "clause (a)'s doctrine half"),
        (LINE_BUDGET, "clause (b)'s line budget"),
        (NEW_SURFACE, "clause (c)'s new-surface guard"),
        (CHECK_OR_CI, "clause (d)'s check:/CI guard"),
    ] {
        assert_eq!(
            flat(&spec).matches(needle).count(),
            1,
            "LOOP-SPEC must state {what} exactly once — zero means it was \
             dropped or reworded, more than one means it is stated twice"
        );
        assert!(
            step4_window(&spec).contains(needle),
            "the clause {needle:?} must live inside step 4's window"
        );
    }
    // The otherwise-arm: any single flipped clause → full adversarial
    // validation (stated inside the window; not pinned exactly-once
    // whole-file — "full adversarial validation" appears elsewhere in the
    // doctrine legitimately).
    let window = step4_window(&spec);
    assert!(
        window.contains("full adversarial validation"),
        "step 4 must state the otherwise-arm: any single flipped clause → \
         full adversarial validation"
    );
}

/// Requirement 2: the predicate's inputs are computed from the diff, never
/// the model's say-so, and the routing record names all four inputs plus
/// the verdict. All three statements exactly once, inside step 4.
#[test]
fn inputs_are_diff_computed_and_recorded() {
    assert!(
        NOT_SAY_SO.contains("say-so") && RECORD_NAMED.contains("four inputs"),
        "the needles must carry the computed-from-diff + record shape"
    );
    let spec = loop_spec();
    for (needle, what) in [
        (FROM_DIFF, "the computed-from-the-diff rule"),
        (NOT_SAY_SO, "the never-the-model's-say-so rule"),
        (RECORD_NAMED, "the record-names-the-four-inputs rule"),
    ] {
        assert_eq!(
            flat(&spec).matches(needle).count(),
            1,
            "LOOP-SPEC must state {what} exactly once — zero means it was \
             dropped, more than one means it is stated twice"
        );
        assert!(
            step4_window(&spec).contains(needle),
            "the statement {needle:?} must live inside step 4's window"
        );
    }
}

/// Requirement 3 + 5: the lane changes review depth, never the gates or
/// the quality floor — the gates-still-required floor (the exact sentence
/// the spec names) and the fix-up arc for lane-eligible diffs the gates
/// catch red. Each exactly once, inside step 4.
#[test]
fn gates_still_required_and_fixup_arc_survive() {
    assert!(
        GATES_STILL.contains("skips the kimi child") && GATES_STILL.contains("never the gates"),
        "the needle must carry the gates-still-required floor"
    );
    let spec = loop_spec();
    for (needle, what) in [
        (REVIEW_DEPTH, "the review-depth-not-quality-floor stance"),
        (GATES_STILL, "the skips-the-kimi-child-never-the-gates floor"),
        (FIXUP_STILL, "the fix-up-arc-still-applies clause"),
    ] {
        assert_eq!(
            flat(&spec).matches(needle).count(),
            1,
            "LOOP-SPEC must state {what} exactly once — zero means it was \
             dropped, more than one means it is stated twice"
        );
        assert!(
            step4_window(&spec).contains(needle),
            "the statement {needle:?} must live inside step 4's window"
        );
    }
}

/// Requirement 4: auto-spec'd runs default to the lane predicate, and the
/// two operator overrides are stated with their flags — `--validate`
/// forces full adversarial, `--no-validate` forces gates-only, either is
/// recorded. Each exactly once, inside step 4.
#[test]
fn auto_spec_default_and_overrides_stated() {
    assert!(
        VALIDATE_FLAG.contains("--validate")
            && NO_VALIDATE_FLAG.contains("--no-validate")
            && OVERRIDE_RECORDED.contains("recorded"),
        "the needles must carry the override flags and the recording duty"
    );
    let spec = loop_spec();
    for (needle, what) in [
        (AUTO_SPEC_DEFAULT, "the auto-spec defaults-to-the-lane rule"),
        (VALIDATE_FLAG, "the --validate force-full override"),
        (NO_VALIDATE_FLAG, "the --no-validate force-gates-only override"),
        (OVERRIDE_RECORDED, "the operator-override-is-recorded duty"),
    ] {
        assert_eq!(
            flat(&spec).matches(needle).count(),
            1,
            "LOOP-SPEC must state {what} exactly once — zero means it was \
             dropped, more than one means it is stated twice"
        );
        assert!(
            step4_window(&spec).contains(needle),
            "the statement {needle:?} must live inside step 4's window"
        );
    }
}

/// The lane folds INTO step 4 — no step renumbering (the T19/T21/T30
/// rule): steps 1–6 keep their headings, each exactly once, and no
/// seventh step appeared. The insertion also must not have displaced
/// step 4's original opening (the REQUIRED list) or the docs-only escape.
#[test]
fn step_headings_survive_unrenumbered() {
    let spec = loop_spec();
    for (marker, what) in [
        ("1. **Worktree", "step 1"),
        ("2. **Implementation child", "step 2"),
        ("3. **Review.**", "step 3 (Review)"),
        (STEP4, "step 4 (Adversarial validation)"),
        (STEP5, "step 5 (Harvest, then merge + close)"),
        ("6. **Budget check", "step 6 (Budget check)"),
    ] {
        assert_eq!(
            spec.matches(marker).count(),
            1,
            "LOOP-SPEC §2 must keep {what}'s heading ({marker:?}) exactly \
             once — the T189 lane folds into step 4, never renumbers steps"
        );
    }
    assert_eq!(
        spec.matches("\n7. **").count(),
        0,
        "no seventh §2 step may appear — the lane is not a new step"
    );
    // Step 4 still OPENS with the REQUIRED list + the docs/tests escape:
    // the lane extends the opening paragraph, it does not replace it.
    let step4 = step4_window(&spec);
    assert!(
        step4.contains("REQUIRED** for any item touching"),
        "step 4's REQUIRED-list opening must survive the lane insertion"
    );
    assert!(
        step4.contains("optional for docs/tests-only items"),
        "step 4's docs/tests escape must survive the lane insertion"
    );
}
