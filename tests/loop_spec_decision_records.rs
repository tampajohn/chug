//! T75 — doctrine pins: LOOP-SPEC's Phase-3 wrap checklist carries the
//! decision-records sentence (the zero-call adoption gap), and (T246) the
//! wrap-audit step the outcome-backfill defect class forced.
//!
//! T70 landed the `decision_log` tool + `.chug/decisions.jsonl` AND wired
//! adoption at four named points (Phase-1 `eval-triage`, step-2
//! `recovery-routing` / `model-fallback`, step-4 `validation-routing` /
//! `validation-verdict`, step-5 `outcome` backfills) — yet two cycles
//! later the corpus shows ZERO organic calls (cycles 34+35 shipped nine
//! routing/verdict decisions with none recorded, and the file did not
//! exist on disk). The adoption sentences named the moments but nothing in
//! the arc ASKED for the records at the moment of accounting — the T23→T24
//! zero-calls lesson repeating: `delegate` landed T23 and sat uncalled
//! until T24 wired it into LOOP-SPEC's workflow. The remedy is one
//! checklist line at Phase 3 (Wrap), the accounting surface: the wrap
//! cannot be called complete while a worked-item cycle shipped zero
//! `decision_log` records.
//!
//! These pins assert the sentence exists and sits where the spec requires:
//! inside the Phase-3 wrap window (between the Phase-3 heading and the
//! Hard-rules heading, headings matched loosely — the T64 heading-scope
//! pattern), with the load-bearing needles each occurring EXACTLY once in
//! LOOP-SPEC.md. No production code changes; this file is doctrine-only.
//! The pin file greps LOOP-SPEC.md only — never a spec prose copy (the T67
//! self-match lesson).
//!
//! T78 idiom: multi-word needles are matched against whitespace-collapsed
//! text because the doctrine prose wraps mid-phrase (the T246 wrap-audit
//! needle spans a line break in the bullet, so its pin is asserted against
//! flat text — the pre-T246 needles all sat on one line and pinned raw).
//!
//! T48 doctrine: every pin resolves LOOP-SPEC.md from the checkout the
//! binary RUNS against (`std::env::current_dir()`; cargo runs test binaries
//! with cwd = the package root), never via the compile-time manifest-dir
//! macro — under the T47 shared cache a compile-time path can point at a
//! since-removed worktree.

/// (a) The carries-records token — `.chug/decisions.jsonl` named as the
/// corpus that carries the cycle's records, in one contiguous run. Must
/// occur EXACTLY once in LOOP-SPEC.md: zero means the checklist line was
/// dropped, more than one means it is stated twice.
const CARRIES_RECORDS: &str = "`.chug/decisions.jsonl` carries the cycle's records";

/// (b) The incomplete-wrap token — the accounting consequence (a
/// worked-item cycle with zero `decision_log` records is an INCOMPLETE
/// wrap). Must occur EXACTLY once in LOOP-SPEC.md.
const INCOMPLETE_WRAP: &str = "is an incomplete wrap";

/// (c) The zero-calls citation token — the T23→T24 lesson receipt. The
/// non-ASCII arrow is spelled as an escape so an editor normalization
/// cannot silently unpin it (\u{2192} = →). Must occur EXACTLY once in
/// LOOP-SPEC.md.
const ZERO_CALLS: &str = "the T23\u{2192}T24 zero-calls lesson";

/// The cycles-34+35 receipt — the measured zero-call evidence (nine
/// routing/verdict decisions shipped with none recorded). Scoped to the
/// wrap window by the scope leg.
const CYCLES_34_35: &str = "cycles 34+35";
const NINE_DECISIONS: &str = "nine routing/verdict decisions";

/// The Phase-3 heading and the Hard-rules heading, matched LOOSELY — the
/// T64 heading-scope pattern: number/word + section marker only, so a
/// wording tweak of either heading's text must not break the scope leg.
/// Each marker is unique in the file today; `find` + `find`-after scopes
/// the wrap window.
const PHASE3_HEADING: &str = "## Phase 3";
const HARD_RULES_HEADING: &str = "## Hard";

/// The five decision classes + the backfill half the checklist line must
/// name, per T70's four named adoption points. Scoped to the wrap window
/// (the same tokens also appear at their named points elsewhere in the
/// spec — Phase 1, §2 steps 2/4/5 — which is fine; the checklist line
/// restates them at the accounting surface).
const WINDOW_CLASS_TOKENS: [&str; 6] = [
    "`eval-triage`",
    "`recovery-routing`",
    "`model-fallback`",
    "`validation-routing`",
    "`validation-verdict`",
    "`outcome` backfills",
];

/// (f) The T246 wrap-audit token — the wrap step the decisions bullet
/// carries after the outcome-backfill defect class fired in BOTH of the
/// last two wraps (cycle-115's three missing backfills, cycle-116's
/// unbackfilled verdict id + outcome-subjecting-outcome chain) and was
/// caught only by the NEXT eval's manual jq spot-check. The sentence wraps
/// mid-phrase in the doctrine prose, so it pins against FLAT text.
const WRAP_AUDIT: &str = "The wrap runs `scripts/decisions-audit.sh`";

fn loop_spec() -> String {
    let root = std::env::current_dir().expect("cargo sets the test cwd to the package root");
    std::fs::read_to_string(root.join("LOOP-SPEC.md"))
        .unwrap_or_else(|e| panic!("reading LOOP-SPEC.md from the runtime checkout: {e}"))
}

/// Wrap-insensitive copy (the T78 flat idiom): the doctrine prose wraps
/// mid-phrase, so a multi-word needle must match whitespace-collapsed text
/// or the pin goes red on the REAL doctrine.
fn flat(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// (a) The carries-records token occurs in LOOP-SPEC.md exactly once.
/// Delete the inserted checklist line and this goes red (count 0); a
/// duplicate statement elsewhere also goes red.
#[test]
fn carries_records_token_occurs_exactly_once() {
    // Needle self-check (T48 idiom): a mangled needle must not let this
    // pin pass silently.
    assert!(
        CARRIES_RECORDS.contains("decisions.jsonl") && CARRIES_RECORDS.contains("carries"),
        "the needle must name .chug/decisions.jsonl as the corpus that \
         carries the cycle's records"
    );
    let spec = loop_spec();
    assert_eq!(
        spec.matches(CARRIES_RECORDS).count(),
        1,
        "LOOP-SPEC must state that .chug/decisions.jsonl carries the cycle's \
         records exactly once — zero means the wrap-checklist line was \
         dropped, more than one means it is stated twice"
    );
}

/// (b) The incomplete-wrap token occurs in LOOP-SPEC.md exactly once.
/// Delete the inserted checklist line and this goes red (count 0);
/// rewording the consequence (e.g. dropping "incomplete") also goes red.
#[test]
fn incomplete_wrap_token_occurs_exactly_once() {
    assert!(
        INCOMPLETE_WRAP.contains("incomplete") && INCOMPLETE_WRAP.contains("wrap"),
        "the needle must carry the incomplete-wrap consequence"
    );
    let spec = loop_spec();
    assert_eq!(
        spec.matches(INCOMPLETE_WRAP).count(),
        1,
        "LOOP-SPEC must state that a zero-record cycle is an incomplete \
         wrap exactly once — zero means the accounting consequence was \
         dropped, more than one means it is stated twice"
    );
}

/// (c) The zero-calls/T23→T24 citation occurs in LOOP-SPEC.md exactly
/// once. The receipt is load-bearing: it tells a future orchestrator WHY
/// the checklist line exists (adoption sentences sat uncalled for two
/// cycles — the same zero-calls pattern as delegate T23→T24), and a
/// sentence stripped of its receipt is free to regress.
#[test]
fn zero_calls_t23_t24_citation_occurs_exactly_once() {
    assert!(
        ZERO_CALLS.starts_with("the T23") && ZERO_CALLS.contains("\u{2192}T24"),
        "the citation token must name the T23→T24 handoff"
    );
    let spec = loop_spec();
    assert_eq!(
        spec.matches(ZERO_CALLS).count(),
        1,
        "LOOP-SPEC must cite the T23→T24 zero-calls lesson exactly once — \
         zero means the receipt was dropped, more than one means it is \
         cited twice"
    );
}

/// (d) The needles live INSIDE the Phase-3 wrap window: the whole window
/// between the Phase-3 heading and the Hard-rules heading (T64
/// heading-scope pattern, headings matched loosely as marker + name so
/// wording tweaks survive). Inside that window every needle sits exactly
/// once, together with the five decision classes + the backfill half and
/// the cycles-34+35 receipt — the checklist is the accounting surface, and
/// a line that drifted out of Phase 3 (or a Phase-3 rewrite that dropped
/// it) must go red.
#[test]
fn needles_live_inside_the_phase3_wrap_window() {
    let spec = loop_spec();
    let start = spec
        .find(PHASE3_HEADING)
        .expect("LOOP-SPEC Phase-3 heading (`## Phase 3`) present");
    let end = start
        + spec[start..]
            .find(HARD_RULES_HEADING)
            .expect("LOOP-SPEC Hard-rules heading (`## Hard`) present after Phase 3");
    let window = &spec[start..end];
    // The load-bearing needles: each inside the window, exactly once there.
    for needle in [
        CARRIES_RECORDS,
        INCOMPLETE_WRAP,
        ZERO_CALLS,
        CYCLES_34_35,
        NINE_DECISIONS,
    ] {
        window
            .find(needle)
            .unwrap_or_else(|| {
                panic!("Phase-3 wrap window must carry the needle {needle:?} (window:\n{window})")
            });
        assert_eq!(
            window.matches(needle).count(),
            1,
            "the needle {needle:?} must occur exactly once inside the \
             Phase-3 wrap window"
        );
    }
    // The five decision classes + the backfill half — T70's named adoption
    // points restated at the accounting surface.
    for token in WINDOW_CLASS_TOKENS {
        assert_eq!(
            window.matches(token).count(),
            1,
            "the Phase-3 wrap checklist must name {token:?} exactly once — \
             the records sentence restates T70's adoption points at the \
             accounting surface"
        );
    }
}

/// (f) The T246 wrap-audit step occurs in LOOP-SPEC.md exactly once,
/// whitespace-collapsed (T78 — the sentence wraps mid-phrase in the
/// bullet), and inside the Phase-3 wrap window (T64 heading-scope
/// pattern). Removing the wrap step from the bullet goes red (count 0);
/// restating it in a second place also goes red. The step is REPORT-only
/// doctrine — this pin guards its existence at the accounting surface,
/// never a merge gate.
#[test]
fn wrap_audit_token_occurs_exactly_once_whitespace_collapsed() {
    // Needle self-check (T48 idiom): a mangled needle must not let this
    // pin pass silently.
    assert!(
        WRAP_AUDIT.starts_with("The wrap runs")
            && WRAP_AUDIT.contains("scripts/decisions-audit.sh"),
        "the needle must name the wrap-time decisions-audit run"
    );
    let spec = loop_spec();
    assert_eq!(
        flat(&spec).matches(&flat(WRAP_AUDIT)).count(),
        1,
        "LOOP-SPEC must state that the wrap runs scripts/decisions-audit.sh \
         exactly once (whitespace-collapsed) — zero means the T246 wrap \
         step was dropped from the decisions bullet, more than one means \
         it is stated twice"
    );
    // ...and the step lives in the Phase-3 wrap window: the accounting
    // surface is where the audit + backfill + name step belongs.
    let start = spec
        .find(PHASE3_HEADING)
        .expect("LOOP-SPEC Phase-3 heading (`## Phase 3`) present");
    let end = start
        + spec[start..]
            .find(HARD_RULES_HEADING)
            .expect("LOOP-SPEC Hard-rules heading (`## Hard`) present after Phase 3");
    let flat_window = flat(&spec[start..end]);
    assert_eq!(
        flat_window.matches(&flat(WRAP_AUDIT)).count(),
        1,
        "the T246 wrap-audit step must sit exactly once inside the Phase-3 \
         wrap window (flat-matched)"
    );
}
