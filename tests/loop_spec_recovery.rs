//! T63 — doctrine pins: LOOP-SPEC §2 step 2 names the `delegate resume:true`
//! budget-death recovery leg.
//!
//! The loop's most common child failure is BUDGET death with the work
//! complete-but-uncommitted or unwrapped — t15/t17/t20 at 40/40, t28, t47,
//! t55, and t58 at 50/50 (the T58 row itself records the fifth occurrence
//! of the pattern). T58 shipped the in-tool recovery primitive (launch's
//! optional `resume: true`, plus `delegate status` reading the LATEST run
//! segment), but the DOCTRINE never named it — an orchestrator mid-arc
//! reads LOOP-SPEC, not the tool schema history, and cycle 18's actual
//! recovery was a hand-rolled bash `chug run --resume` under nohup (the
//! exact launch pattern T24 replaced) precisely because the doctrine had
//! no resume leg to follow.
//!
//! These pins assert the leg exists and sits where the spec requires:
//! inside §2 step 2's polling paragraph, directly after "Exit of the pid =
//! child done; then review." and BEFORE the hand-rolled-nohup
//! tool-failure fallback — the resume leg supplements that fallback, never
//! replaces it. No production code changes; this file is doctrine-only.
//!
//! T48 doctrine: every pin resolves LOOP-SPEC.md from the checkout the
//! binary RUNS against (`std::env::current_dir()`; cargo runs test binaries
//! with cwd = the package root), never via the compile-time manifest-dir
//! macro — under the T47 shared cache a compile-time path can point at a
//! since-removed worktree.

/// The leg's signature phrase: "resume" + the one-attempt cap language in
/// one contiguous run. Must occur EXACTLY once in LOOP-SPEC.md.
const RESUME_CAP: &str = "ONE resume attempt per child";

/// Step 2's polling anchor — the leg must sit directly after this sentence.
const STEP2_ANCHOR: &str = "Exit of the pid = child done";

/// Step 3's heading — the leg must not cross the step boundary (step
/// numbering is unchanged; the leg folds INTO step 2).
const STEP3_HEADING: &str = "3. **Review.**";

/// The pre-existing hand-rolled-nohup fallback sentence, byte-identical
/// INCLUDING its wrapped line breaks (the two non-ASCII bytes are spelled
/// as escapes so an editor normalization cannot silently unpin them:
/// \u{a7} = section sign, \u{2014} = em dash). The resume leg is inserted
/// BEFORE it; these bytes must survive untouched.
const NOHUP_FALLBACK: &str = concat!(
    "If\n",
    "   `delegate` itself errors persistently (the tool, not the child),\n",
    "   META-SPEC \u{a7}4's hand-rolled nohup launch template remains the fallback\n",
    "   launch path \u{2014} note the fallback in your ledger."
);

fn loop_spec() -> String {
    let root = std::env::current_dir().expect("cargo sets the test cwd to the package root");
    std::fs::read_to_string(root.join("LOOP-SPEC.md"))
        .unwrap_or_else(|e| panic!("reading LOOP-SPEC.md from the runtime checkout: {e}"))
}

/// (a) The leg's signature needle — "resume" + the one-attempt cap language
/// — occurs in LOOP-SPEC.md exactly once. Delete the leg and this goes red
/// (count 0); a duplicate statement of the cap elsewhere also goes red.
#[test]
fn resume_cap_needle_occurs_exactly_once() {
    // Needle self-check (T48 idiom): a mangled needle must not let this
    // pin pass silently.
    assert!(
        RESUME_CAP.contains("ONE")
            && RESUME_CAP.contains("resume")
            && RESUME_CAP.contains("attempt"),
        "the needle must carry both the one-attempt cap language and \"resume\""
    );
    let spec = loop_spec();
    assert_eq!(
        spec.matches(RESUME_CAP).count(),
        1,
        "LOOP-SPEC must state the ONE-resume-attempt cap exactly once — \
         zero means the recovery leg was deleted, more than one means it is \
         stated twice"
    );
}

/// (b) The leg sits INSIDE step 2: after the polling anchor, before the
/// hand-rolled-nohup fallback sentences, and before step 3's heading —
/// step numbering unchanged, the fallback supplemented not replaced.
#[test]
fn recovery_leg_sits_inside_step_2_before_the_nohup_fallback() {
    let spec = loop_spec();
    let anchor = spec
        .find(STEP2_ANCHOR)
        .expect("step-2 polling anchor (\"Exit of the pid = child done\") present");
    let leg = spec
        .find(RESUME_CAP)
        .expect("the recovery leg's cap needle present (leg deleted?)");
    let fallback = spec
        .find(NOHUP_FALLBACK)
        .expect("the hand-rolled-nohup fallback sentence present (pin (c) covers its bytes)");
    let step3 = spec
        .find(STEP3_HEADING)
        .expect("step-3 heading (\"3. **Review.**\") present");
    assert!(
        anchor < leg && leg < fallback && fallback < step3,
        "the recovery leg must sit inside step 2's polling paragraph, after \
         \"Exit of the pid = child done\" and BEFORE the nohup fallback and \
         step 3's heading (anchor {anchor}, leg {leg}, fallback {fallback}, \
         step3 {step3})"
    );
}

/// (c) The pre-existing nohup-fallback sentence survives BYTE-IDENTICAL
/// (its wrapped line breaks included) — the resume leg supplements the
/// tool-failure fallback, never replaces or rewraps it.
#[test]
fn nohup_fallback_sentence_survives_byte_identical() {
    let spec = loop_spec();
    assert_eq!(
        spec.matches(NOHUP_FALLBACK).count(),
        1,
        "the hand-rolled-nohup fallback sentence must survive byte-identical \
         (wrapping included) exactly once — the resume leg supplements it, \
         never replaces it"
    );
}

/// (d) The leg names its mechanics and its scope guards, inside step 2:
/// the `resume: true` relaunch shape (same worktree / spec / goal / model /
/// budgets), why it works (T19 — the worktree is never removed pre-harvest
/// so the untracked `.chug/` transcript persists; T58 — status reads the
/// LATEST run segment), and the fix-up-children exclusion (step 4's FAIL
/// arc starts fresh by design).
#[test]
fn leg_names_mechanics_and_scope_guards_inside_step_2() {
    let spec = loop_spec();
    let anchor = spec
        .find(STEP2_ANCHOR)
        .expect("step-2 polling anchor present");
    let step3 = spec
        .find(STEP3_HEADING)
        .expect("step-3 heading present");
    let window = &spec[anchor..step3];
    for needle in [
        "`resume: true`",
        "SAME worktree",
        "re-carries the T47",
        "50/35 impl, 50/30 validate",
        "continues the child's",
        "never removed pre-harvest (T19)",
        "LATEST run segment (T58)",
        "start fresh by design",
    ] {
        assert!(
            window.contains(needle),
            "the step-2 recovery leg must name {needle:?} — the leg names its \
             mechanics (same worktree/spec/goal/model/budgets; T19 transcript \
             persistence; T58 latest-segment status) and its scope guards \
             (fix-up children start fresh by design)"
        );
    }
}
