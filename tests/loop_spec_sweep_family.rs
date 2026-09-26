//! T72 — doctrine pins: LOOP-SPEC §2 step 4's FAIL arc carries the
//! sweep-the-family directive.
//!
//! Cycle 33's T69 validation arc took THREE kimi rounds where one should
//! have sufficed: each round's fix-up pinned exactly the vacuous leg the
//! findings named (run_start latch-reset survivors of the abort_reason /
//! verdict_latch / check_cmd class), and the next round found one more leg
//! of the SAME class — one round per leg, ~15 min per round. The class only
//! closed when the final round's goal carried explicit sweep-the-family
//! guidance: every run_start latch reset swept, each with its own
//! RED-proven killing test. The structural pattern: a fix-up that pins only
//! the named leg guarantees the next round finds the next instance. The
//! remedy is evaluator/orchestrator-side and one sentence long — at the
//! fix-up dispatch point, when a finding names ONE instance of a class, the
//! fix-up goal must ALSO name the class and require EVERY instance swept.
//!
//! These pins assert the directive exists and sits where the spec requires:
//! inside §2 step 4, extending the FAIL arc's fix-up sentence in place
//! ("FAIL → fix-up child … then re-validate"), before step 5's heading — no
//! step renumbering (T19/T30 rule). No production code changes; this file
//! is doctrine-only.
//!
//! T48 doctrine: every pin resolves LOOP-SPEC.md from the checkout the
//! binary RUNS against (`std::env::current_dir()`; cargo runs test binaries
//! with cwd = the package root), never via the compile-time manifest-dir
//! macro — under the T47 shared cache a compile-time path can point at a
//! since-removed worktree.

/// (a) The directive's name phrase — the sweep-the-family token the fix-up
/// goal must carry when a finding names a class. Must occur EXACTLY once
/// in LOOP-SPEC.md: zero means the directive was dropped, more than one
/// means it is stated twice (the T67 self-match lesson — the pin file
/// greps LOOP-SPEC, never a spec prose copy).
const SWEEP_FAMILY: &str = "sweep-the-family";

/// (b) The requirement carrier — the every-instance/RED-proven leg in one
/// contiguous run. Must occur EXACTLY once in LOOP-SPEC.md. (The needle is
/// single-line by construction; a rewrap that splits it goes red, the same
/// byte-identity stance as the T63 nohup-fallback pin.)
const EVERY_INSTANCE_RED: &str = "EVERY instance swept with its own RED-proven killing test";

/// (c) The cycle-33 receipt tokens — the lesson citation that justifies the
/// directive (T69's run_start latch resets took three rounds one leg at a
/// time; the sweep-the-family goal closed it in one). Each must occur
/// EXACTLY once in LOOP-SPEC.md.
const CYCLE33: &str = "cycle-33";
const T69: &str = "T69";

/// Step 4's and step 5's headings, matched LOOSELY — number + bold marker
/// only (the T64 heading-scope pattern): a wording tweak of either heading's
/// text must not break the scope leg. Each marker is unique in the file
/// today; `find` + `find`-after scopes the step-4 window.
const STEP4_HEADING: &str = "4. **";
const STEP5_HEADING: &str = "5. **";

/// The FAIL-arc sentence the directive extends IN PLACE (the non-ASCII
/// arrow is spelled as an escape so an editor normalization cannot silently
/// unpin it: \u{2192} = →). The inserted clause sits between the pasted-
/// findings half and the re-validate half.
const FAIL_ARC_ANCHOR: &str = "FAIL \u{2192} fix-up child";
const FINDINGS_PASTED: &str = "with the findings pasted into its goal";
const THEN_REVALIDATE: &str = "then re-validate";

fn loop_spec() -> String {
    let root = std::env::current_dir().expect("cargo sets the test cwd to the package root");
    std::fs::read_to_string(root.join("LOOP-SPEC.md"))
        .unwrap_or_else(|e| panic!("reading LOOP-SPEC.md from the runtime checkout: {e}"))
}

/// (a) The sweep-the-family name phrase occurs in LOOP-SPEC.md exactly
/// once. Delete the inserted clause and this goes red (count 0); a
/// duplicate statement of the directive elsewhere also goes red.
#[test]
fn sweep_family_phrase_occurs_exactly_once() {
    // Needle self-check (T48 idiom): a mangled needle must not let this
    // pin pass silently.
    assert!(
        SWEEP_FAMILY.contains("sweep") && SWEEP_FAMILY.contains("family"),
        "the needle must carry the sweep-the-family name phrase"
    );
    let spec = loop_spec();
    assert_eq!(
        spec.matches(SWEEP_FAMILY).count(),
        1,
        "LOOP-SPEC must name sweep-the-family exactly once — zero means the \
         FAIL-arc directive was dropped, more than one means it is stated \
         twice"
    );
}

/// (b) The every-instance/RED-proven requirement occurs in LOOP-SPEC.md
/// exactly once. Delete the inserted clause and this goes red (count 0);
/// rewording the requirement (e.g. dropping "its own" or "RED-proven")
/// also goes red.
#[test]
fn every_instance_red_proven_requirement_occurs_exactly_once() {
    assert!(
        EVERY_INSTANCE_RED.contains("EVERY instance")
            && EVERY_INSTANCE_RED.contains("RED-proven killing test"),
        "the needle must carry both the every-instance sweep and the \
         RED-proven killing-test requirement"
    );
    let spec = loop_spec();
    assert_eq!(
        spec.matches(EVERY_INSTANCE_RED).count(),
        1,
        "LOOP-SPEC must require EVERY instance swept with its own RED-proven \
         killing test exactly once — zero means the requirement was dropped \
         or reworded"
    );
}

/// (c) The cycle-33/T69 citation occurs in LOOP-SPEC.md exactly once —
/// both tokens, each exactly once. The receipt is load-bearing: it tells a
/// future orchestrator WHY the directive exists (one leg per round burned
/// three kimi rounds; the class-sweep goal closed it in one), and a
/// directive stripped of its receipt is free to regress.
#[test]
fn cycle33_t69_citation_occurs_exactly_once() {
    assert!(
        CYCLE33.starts_with("cycle-") && CYCLE33.ends_with("33"),
        "the cycle token must name cycle 33"
    );
    assert!(
        T69.starts_with('T') && T69[1..].chars().all(|c| c.is_ascii_digit()),
        "the T-token must name a T-item"
    );
    let spec = loop_spec();
    for (needle, what) in [
        (CYCLE33, "the cycle-33 lesson citation"),
        (T69, "the T69 receipt"),
    ] {
        assert_eq!(
            spec.matches(needle).count(),
            1,
            "LOOP-SPEC must cite {what} exactly once — zero means the \
             receipt was dropped, more than one means it is cited twice"
        );
    }
}

/// (d) The needles live INSIDE §2 step 4: the whole window between step 4's
/// heading and step 5's heading (T64 heading-scope pattern, headings
/// matched loosely as number + bold marker so wording tweaks survive), and
/// each needle sits AFTER the FAIL-arc anchor — the directive extends the
/// fix-up sentence in place ("FAIL → fix-up child … then re-validate"),
/// never renumbering steps (T19/T30 rule).
#[test]
fn sweep_directive_lives_inside_step_4_at_the_fail_arc() {
    let spec = loop_spec();
    let start = spec
        .find(STEP4_HEADING)
        .expect("LOOP-SPEC step-4 heading (`4. **`) present");
    let end = start
        + spec[start..]
            .find(STEP5_HEADING)
            .expect("LOOP-SPEC step-5 heading (`5. **`) present after step 4");
    let window = &spec[start..end];
    // The window is step 4's FAIL arc: the sentence the directive extends
    // is inside it, with both halves of the original sentence intact.
    let anchor = window
        .find(FAIL_ARC_ANCHOR)
        .unwrap_or_else(|| panic!("step-4 window must carry the FAIL-arc anchor {FAIL_ARC_ANCHOR:?} (got:\n{window})"));
    assert!(
        window.contains(FINDINGS_PASTED) && window.contains(THEN_REVALIDATE),
        "the FAIL-arc sentence's existing bytes must survive intact — \
         {FINDINGS_PASTED:?} and {THEN_REVALIDATE:?} both inside step 4"
    );
    // Every needle is inside the window, after the FAIL-arc anchor, and
    // exactly once there.
    for needle in [
        SWEEP_FAMILY,
        EVERY_INSTANCE_RED,
        CYCLE33,
        T69,
    ] {
        let at = window.find(needle).unwrap_or_else(|| {
            panic!("step-4 window must carry the directive needle {needle:?}")
        });
        assert!(
            at > anchor,
            "the directive needle {needle:?} must sit AFTER the FAIL-arc \
             anchor — the directive extends the fix-up sentence in place"
        );
        assert_eq!(
            window.matches(needle).count(),
            1,
            "the directive needle {needle:?} must occur exactly once inside \
             step 4"
        );
    }
}
