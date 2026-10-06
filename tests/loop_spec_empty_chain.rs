//! T247 — LOOP-SPEC Phase 1 codifies the empty-delta chain: the disposition
//! rule, the eval-trip valve, and the human-counted streak.
//!
//! The chain ran TWICE (cycles 110–113 and 119–122) with its entire
//! governance carried in Outcomes/wrap-notes prose — never in LOOP-SPEC.
//! Phase 1's skip predicate said nothing about the case where the predicate
//! FAILS and the eval is skipped anyway (the queue-drained, bookkeeping-only
//! delta), the trip valve that terminates the chain lived only in prose, and
//! the cycle-118 negation-quote lesson (loopd's machine streak counts
//! NEGATED token mentions; machine 4 vs TRUE 3 at the cycle-122 trip) had no
//! written home. The handoff worked 6-for-6, but a rule that decides WHEN a
//! skipped-eval chain must stop should not depend on every wrap correctly
//! carrying prose — one clause at the point of decision makes the chain
//! self-describing for a cold cycle.
//!
//! Three exactly-once needle legs, one per part of the clause — (a) the
//! disposition rule, (b) the trip valve, (c) the human-counted streak — plus
//! a placement leg (the clause sits between the skip-predicate paragraph and
//! the commit-artifacts sentence, per the spec's insertion point) and a
//! coverage leg (the clause's remaining load-bearing tokens). Each needle
//! leg is RED-provable by reverting its needle phrase from LOOP-SPEC.md (the
//! deletion hand-check is recorded in the commit message, per the
//! loop_spec_* deletion-proof convention).
//!
//! T48 doctrine: the pin resolves LOOP-SPEC.md from the checkout the binary
//! RUNS against (`std::env::current_dir()`; cargo runs test binaries with
//! cwd = the package root), never via the compile-time manifest-dir macro.
//! T78 idiom: the doctrine prose wraps mid-phrase (twice inside the clause
//! itself, where the term `empty-delta` / `disposition` splits across a
//! wrap — the single-carrier pin on the raw token in
//! tests/loopd_empty_backoff.rs keeps LOOP-SPEC's contiguous occurrence at
//! exactly the Phase-3 clause), so multi-word needles match
//! whitespace-collapsed text.

fn loop_spec() -> String {
    let root = std::env::current_dir().expect("cargo sets the test cwd to the package root");
    std::fs::read_to_string(root.join("LOOP-SPEC.md"))
        .unwrap_or_else(|e| panic!("reading LOOP-SPEC.md from the runtime checkout: {e}"))
}

/// Wrap-insensitive copy (the T78 idiom): the doctrine prose wraps
/// mid-phrase, so a multi-word needle that crosses a break point must match
/// whitespace-collapsed text or the pin goes red on the REAL doctrine.
fn flat(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ---- the three needle legs, one per part of the clause ----

/// (a) The disposition rule's delta test — the bookkeeping-only delta the
/// skip falls back on when the predicate's todo-rows half fails. Must occur
/// EXACTLY once in LOOP-SPEC.md: zero means the rule was deleted or
/// reworded, more than one means it is stated twice.
const DISPOSITION_NEEDLE: &str = "zero children launched, zero items landed";

/// (b) The trip valve — the termination condition that makes the T237
/// pre-authorized chain safe. Must occur EXACTLY once.
const VALVE_NEEDLE: &str = "next (4th) empty cycle RUNS the real evaluation";

/// (c) The human-counted streak — the trip binds on the TRUE count from the
/// wrap-notes chain, never loopd's machine streak. Must occur EXACTLY once.
const HUMAN_NEEDLE: &str = "HUMAN-counted from the wrap-notes chain";

/// (a) The disposition rule's delta test occurs EXACTLY once in
/// LOOP-SPEC.md. Delete the clause (or reword the delta test) and this goes
/// red at count 0; a duplicate statement of the rule elsewhere also goes
/// red; rewrapping the phrase cannot save a deletion (the needle matches
/// whitespace-collapsed text).
#[test]
fn disposition_rule_delta_needle_occurs_exactly_once() {
    // Needle self-check (T48 idiom): a mangled needle must not let this
    // pin pass silently.
    assert!(
        DISPOSITION_NEEDLE.starts_with("zero children")
            && DISPOSITION_NEEDLE.contains("zero items landed"),
        "the disposition needle must carry the bookkeeping-only delta test \
         verbatim — zero children AND zero items"
    );
    let spec = flat(&loop_spec());
    assert_eq!(
        spec.matches(DISPOSITION_NEEDLE).count(),
        1,
        "LOOP-SPEC must state the empty-delta disposition rule's delta test \
         ({DISPOSITION_NEEDLE:?}) exactly once — zero means the disposition \
         rule was deleted or reworded (the T247 text-revert mutant), more \
         than one means it is stated twice"
    );
}

/// (b) The trip valve occurs EXACTLY once in LOOP-SPEC.md. Delete the valve
/// sentence and this goes red at count 0 — the exact prose-only gap this
/// row closes: before T247, deleting the chain's termination condition from
/// the doctrine left every gate green.
#[test]
fn trip_valve_needle_occurs_exactly_once() {
    assert!(
        VALVE_NEEDLE.contains("(4th)") && VALVE_NEEDLE.ends_with("real evaluation"),
        "the valve needle must carry the ~3-then-trip shape verbatim — the \
         next (4th) empty cycle runs the real evaluation"
    );
    let spec = flat(&loop_spec());
    assert_eq!(
        spec.matches(VALVE_NEEDLE).count(),
        1,
        "LOOP-SPEC must state the eval-trip valve ({VALVE_NEEDLE:?}) exactly \
         once — zero means the chain's termination condition was deleted or \
         reworded, more than one means it is stated twice"
    );
}

/// (c) The human-counted streak occurs EXACTLY once in LOOP-SPEC.md. Delete
/// the sentence and this goes red at count 0; a revert to machine-counted
/// wording (dropping the HUMAN qualifier) dies the same way.
#[test]
fn human_counted_streak_needle_occurs_exactly_once() {
    assert!(
        HUMAN_NEEDLE.starts_with("HUMAN-counted")
            && HUMAN_NEEDLE.ends_with("wrap-notes chain"),
        "the human-counted needle must carry the TRUE-streak source verbatim \
         — counted from the wrap-notes chain, not the machine walk"
    );
    let spec = flat(&loop_spec());
    assert_eq!(
        spec.matches(HUMAN_NEEDLE).count(),
        1,
        "LOOP-SPEC must state the human-counted streak ({HUMAN_NEEDLE:?}) \
         exactly once — zero means the human-counting rule was deleted or \
         reworded, more than one means it is stated twice"
    );
}

// ---- placement: the clause sits at the point of decision ----

/// The clause must sit INSIDE Phase 1, AFTER the skip-predicate paragraph
/// (its glm-never-evaluates close included) and BEFORE the commit-artifacts
/// sentence — one clause at the point of decision, per the spec's insertion
/// point. Delete the clause, move it into Phase 3, or hoist it above the
/// skip predicate, and this goes red.
#[test]
fn chain_clause_sits_after_the_skip_predicate_before_the_commit_artifacts() {
    let spec = flat(&loop_spec());
    let p1 = spec
        .find("## Phase 1")
        .expect("LOOP-SPEC carries the `## Phase 1` heading");
    let p2 = p1
        + spec[p1..]
            .find("## Phase 2")
            .expect("the `## Phase 2` heading follows Phase 1");
    let window = &spec[p1..p2];
    let skip = window
        .find("re-evaluating for its own sake burns budget")
        .expect("Phase 1 carries the skip predicate (deleted or reworded?)");
    let glm = window
        .find("glm never runs this phase")
        .expect("Phase 1 carries the glm-never-evaluates clause (the T81 pin)");
    let disposition = window.find(DISPOSITION_NEEDLE).unwrap_or_else(|| {
        panic!(
            "Phase 1 must carry the disposition rule {DISPOSITION_NEEDLE:?} \
             (deleted, or moved out of Phase 1?)"
        )
    });
    let valve = window.find(VALVE_NEEDLE).unwrap_or_else(|| {
        panic!(
            "Phase 1 must carry the trip valve {VALVE_NEEDLE:?} (deleted, or \
             moved out of Phase 1?)"
        )
    });
    let human = window.find(HUMAN_NEEDLE).unwrap_or_else(|| {
        panic!(
            "Phase 1 must carry the human-counted streak {HUMAN_NEEDLE:?} \
             (deleted, or moved out of Phase 1?)"
        )
    });
    let commit = window
        .find("Commit the evaluation artifacts")
        .expect("Phase 1 closes with the commit-artifacts sentence");
    assert!(
        skip < glm && glm < disposition,
        "the chain clause sits AFTER the whole skip-predicate paragraph — \
         the predicate ({skip}), then its glm-never-evaluates close ({glm}), \
         then the chain clause ({disposition})"
    );
    assert!(
        disposition < valve && valve < human,
        "the clause reads in the written order — disposition rule \
         ({disposition}), trip valve ({valve}), human-counted streak \
         ({human})"
    );
    assert!(
        human < commit,
        "the chain clause sits BEFORE the commit-artifacts sentence it must \
         precede ({human} vs {commit}) — the commit instruction stays the \
         paragraph after the chain rule, where the spec's insertion point \
         puts it"
    );
}

// ---- coverage: the clause's remaining load-bearing tokens ----

/// The clause carries more than its three headline needles — the handoff
/// shape, the named trips, the false positive, and the standing
/// adjudication. Lose any one (a partial revert that keeps the headline
/// needles) and this goes red, inside the Phase-1 window only.
#[test]
fn chain_clause_carries_the_handoff_trips_and_adjudication() {
    let spec = flat(&loop_spec());
    let p1 = spec
        .find("## Phase 1")
        .expect("LOOP-SPEC carries the `## Phase 1` heading");
    let p2 = p1
        + spec[p1..]
            .find("## Phase 2")
            .expect("the `## Phase 2` heading follows Phase 1");
    let window = &spec[p1..p2];
    for (needle, what) in [
        (
            "the Phase-3 token",
            "the wrap-notes subject names the token rule the disposition \
             relies on",
        ),
        (
            "N empties away",
            "the Outcomes handoff names the TRUE streak forward",
        ),
        (
            "not git archaeology",
            "a cold cycle reads the count from the newest Outcomes entry",
        ),
        (
            "the chain converts itself into its own evaluation",
            "the valve's mechanism, not just its schedule",
        ),
        (
            "cycles 113 and 122",
            "the two on-schedule trips are named",
        ),
        (
            "counts NEGATED mentions too",
            "the machine walk's false-positive mechanism",
        ),
        (
            "the cycle-118 false positive",
            "the evidence the human-counting rule exists for",
        ),
        (
            "consequence-free for the backoff",
            "the standing adjudication's scope — the divergence is bounded \
             and self-correcting",
        ),
        (
            "d1791277274-2",
            "the adjudication's decision id (provenance, the T156 idiom)",
        ),
    ] {
        assert!(
            window.contains(needle),
            "the Phase-1 chain clause must carry {what:?} — {needle:?} is \
             missing (deleted, reworded, or moved out of Phase 1)"
        );
    }
}

// ---- the T248 Phase-3 legs: quote discipline + probe timing ----

/// (d) The quote-discipline rule (Phase 3's T237 token bullet) — the token
/// appears verbatim ONLY in a true disposition wrap subject; every other
/// mention in an `eval:` subject writes around it. Must occur EXACTLY once
/// in LOOP-SPEC.md (whitespace-collapsed).
const QUOTE_NEEDLE: &str = "verbatim ONLY in a true disposition wrap subject";

/// (e) The probe-timing rule (the same bullet) — the wrap's T237-watch
/// sleep-ok probe reads the streak after the wrap-notes commit lands (or
/// is explicitly named pre-commit). Must occur EXACTLY once.
const PROBE_NEEDLE: &str = "probe runs AFTER the wrap-notes commit lands";

/// (d) The quote-discipline rule occurs EXACTLY once in LOOP-SPEC.md.
/// Delete the clause (or reword the needle phrase) and this goes red at
/// count 0; a duplicate statement of the rule elsewhere also goes red;
/// rewrapping the phrase cannot save a deletion (the needle matches
/// whitespace-collapsed text).
#[test]
fn quote_discipline_needle_occurs_exactly_once() {
    // Needle self-check (T48 idiom): a mangled needle must not let this
    // pin pass silently.
    assert!(
        QUOTE_NEEDLE.starts_with("verbatim ONLY")
            && QUOTE_NEEDLE.ends_with("disposition wrap subject"),
        "the quote-discipline needle must carry the verbatim-token rule — \
         ONLY in a true disposition wrap subject"
    );
    let spec = flat(&loop_spec());
    assert_eq!(
        spec.matches(QUOTE_NEEDLE).count(),
        1,
        "LOOP-SPEC must state the quote-discipline rule ({QUOTE_NEEDLE:?}) \
         exactly once — zero means the rule was deleted or reworded (the \
         T248 text-revert mutant), more than one means it is stated twice"
    );
}

/// (e) The probe-timing rule occurs EXACTLY once in LOOP-SPEC.md. Delete
/// the clause (or reword the needle phrase) and this goes red at count 0;
/// a duplicate statement of the rule elsewhere also goes red.
#[test]
fn probe_timing_needle_occurs_exactly_once() {
    assert!(
        PROBE_NEEDLE.starts_with("probe runs AFTER")
            && PROBE_NEEDLE.ends_with("wrap-notes commit lands"),
        "the probe-timing needle must carry the post-commit probe rule — \
         the probe reads the streak after the wrap-notes commit lands"
    );
    let spec = flat(&loop_spec());
    assert_eq!(
        spec.matches(PROBE_NEEDLE).count(),
        1,
        "LOOP-SPEC must state the probe-timing rule ({PROBE_NEEDLE:?}) \
         exactly once — zero means the rule was deleted or reworded (the \
         T248 text-revert mutant), more than one means it is stated twice"
    );
}
