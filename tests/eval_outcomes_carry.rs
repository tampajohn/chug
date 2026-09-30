//! T166 — the eval template must carry forward ALL existing Outcomes sections.
//!
//! Doctrine pins for META-META-SPEC.md's Write-EVALUATION.md section (the
//! `## Write `EVALUATION.md`` heading). The cycle-72 eval commit (`1206d93`)
//! REGENERATED EVALUATION.md and dropped the Cycle 69/70/71 Outcomes sections
//! that existed at the cycle-71 wrap — the narrative survived only because
//! the wrap restored it verbatim from `9c1d6bd:EVALUATION.md`. The cycle-73
//! AND cycle-75 wraps named the remedy ("the eval-commit template must carry
//! forward ALL existing Outcomes sections") and nothing pinned it, so every
//! fresh eval could silently amputate the history again. The pinned sentence
//! makes the carry-forward a written MUST with a pre-commit verification
//! step: the eval rewrites the assessment body only, never the Outcomes
//! ledger.
//!
//! Three legs: (a) the sentence's load-bearing tokens occur exactly-once —
//! `carry forward` and `verbatim` whole-file, `Outcomes` with the census the
//! required sentence dictates, window-scoped (see the test's doc comment);
//! (b) non-vacuousness: deleting the sentence drops every needle's count to
//! zero and the red message says so — the deletion hand-check (GREEN →
//! delete sentence → RED → restore → GREEN) was run before committing and is
//! stated in the commit message, per the loop_spec_* deletion-proof
//! convention; (c) the current EVALUATION.md still carries a `## Outcomes`
//! heading — cheap structural sanity, NOT a full history check (the
//! mechanical history-DIFF guard was ruled out as fragile across cycle
//! numbering).
//!
//! T48 doctrine: every pin resolves its file from the checkout the binary
//! RUNS against (`std::env::current_dir()`; cargo runs test binaries with
//! cwd = the package root), never via the compile-time manifest-dir macro —
//! under the T47 shared cache a compile-time path can point at a
//! since-removed worktree. T78 idiom: multi-word needles are matched against
//! whitespace-collapsed text because the doctrine prose wraps mid-phrase
//! (every multi-word needle in this file crosses a wrap point in the
//! committed sentence).

fn meta_meta_spec() -> String {
    let root = std::env::current_dir().expect("cargo sets the test cwd to the package root");
    std::fs::read_to_string(root.join("META-META-SPEC.md"))
        .unwrap_or_else(|e| panic!("reading META-META-SPEC.md from the runtime checkout: {e}"))
}

fn evaluation_md() -> String {
    let root = std::env::current_dir().expect("cargo sets the test cwd to the package root");
    std::fs::read_to_string(root.join("EVALUATION.md"))
        .unwrap_or_else(|e| panic!("reading EVALUATION.md from the runtime checkout: {e}"))
}

/// Wrap-insensitive copy (the T78 flat-readme idiom): the doctrine prose
/// wraps mid-phrase, so a multi-word needle that crosses a break point must
/// match whitespace-collapsed text or the pin goes red on the REAL doctrine.
fn flat(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ---- the sentence's load-bearing tokens ----

/// The carry-forward sentence's contiguous core — the MUST, the
/// every-existing scope, and the verbatim-ness in one phrase. Must occur
/// EXACTLY once in META-META-SPEC.md: zero means the sentence was deleted
/// or reworded, more than one means it is stated twice.
const CARRY_CORE: &str = "MUST carry forward every existing `## Outcomes` content verbatim";

/// The load-bearing verb phrase. Must occur EXACTLY once (whole file).
const CARRY_FORWARD: &str = "carry forward";

/// The verbatim-ness token: the Outcomes content is carried UNCHANGED, not
/// summarized or regenerated. Must occur EXACTLY once (whole file).
const VERBATIM: &str = "verbatim";

/// The pre-commit verification clause — the sentence's enforcement half:
/// before committing, the newest pre-existing cycle's section must still be
/// present. Must occur EXACTLY once (whole file).
const VERIFY_CLAUSE: &str = "verify the newest pre-existing cycle's section is still present";

/// The Outcomes token. NOT pinned whole-file: the Extend-TODO
/// spec-quality-bar paragraph already carries one legitimate occurrence
/// ("the Outcomes records", the T163-era estimate-calibration rule), so the
/// census is window-scoped to the Write-EVALUATION section, where the
/// required sentence dictates EXACTLY TWO (the `## Outcomes` heading
/// reference and "the Outcomes ledger").
const OUTCOMES: &str = "Outcomes";

/// Section-window markers (the T64 loose-heading pattern): the sentence
/// must live inside the Write-EVALUATION section, in its preamble BEFORE
/// the numbered section list (it governs the whole eval document, so it
/// sits where the doc shape is introduced, not buried in a numbered item).
const WRITE_EVAL_HEADING: &str = "## Write `EVALUATION.md`";
const EXTEND_TODO_HEADING: &str = "## Extend `TODO.md`";
const SECTIONS_MARKER: &str = "Sections:";

/// The Write-EVALUATION window: from its heading to the Extend-TODO
/// heading (the next `## ` section).
fn write_eval_window(spec: &str) -> &str {
    let start = spec
        .find(WRITE_EVAL_HEADING)
        .unwrap_or_else(|| panic!("META-META-SPEC must carry the {WRITE_EVAL_HEADING:?} heading"));
    let end = start
        + spec[start..]
            .find(EXTEND_TODO_HEADING)
            .unwrap_or_else(|| panic!("the Extend-TODO heading must follow the Write-EVALUATION heading"));
    &spec[start..end]
}

/// (a) The sentence's load-bearing tokens occur exactly-once in
/// META-META-SPEC.md: `carry forward` once whole-file, `verbatim` once
/// whole-file, the verification clause once whole-file, and `Outcomes` with
/// the census the required sentence dictates — TWO inside the
/// Write-EVALUATION window (one pre-existing in the Extend-TODO bar keeps
/// the whole-file count out of the pin). Delete the sentence and every
/// count goes red (zero — which also breaks the spec's own line-wise
/// grep); duplicate it and the counts grow past one/two; reword any one
/// instance and the census relation breaks.
#[test]
fn carry_forward_tokens_occur_exactly_once() {
    // Needle self-checks (T48 idiom): a mangled needle must not let this
    // pin pass silently.
    assert!(
        CARRY_FORWARD == "carry forward" && CARRY_FORWARD.contains(' '),
        "the verb-phrase needle must be the two-word phrase verbatim"
    );
    assert!(
        CARRY_CORE.contains("MUST") && CARRY_CORE.contains("every existing") && CARRY_CORE.ends_with("verbatim"),
        "the core needle must carry MUST + every-existing + verbatim"
    );
    assert!(
        VERIFY_CLAUSE.starts_with("verify the newest") && VERIFY_CLAUSE.ends_with("still present"),
        "the verification needle must carry the pre-commit check verbatim"
    );
    assert_eq!(OUTCOMES, "Outcomes", "the token needle must be the bare token");
    let spec_text = meta_meta_spec();
    let spec = flat(&spec_text);
    for (needle, what) in [
        (CARRY_FORWARD, "the carry-forward verb phrase"),
        (VERBATIM, "the verbatim-ness token"),
        (VERIFY_CLAUSE, "the pre-commit verification clause"),
    ] {
        assert_eq!(
            spec.matches(needle).count(),
            1,
            "META-META-SPEC must state {what} ({needle:?}) exactly once — zero \
             means the carry-forward sentence was dropped or reworded, more \
             than one means it is stated twice"
        );
    }
    let window = flat(write_eval_window(&spec_text));
    assert_eq!(
        window.matches(OUTCOMES).count(),
        2,
        "the Write-EVALUATION section must state {OUTCOMES:?} exactly twice — \
         the required sentence uses it in the `## Outcomes` heading reference \
         and in \"never the Outcomes ledger\"; zero means the sentence was \
         deleted, one means an instance was reworded away, more than two \
         means a duplicate statement drifted in"
    );
}

/// (b) Non-vacuousness core: the sentence is pinned as one contiguous
/// (wrap-insensitive) phrase, so deleting it — the exact regression the
/// cycle-72 eval commit performed on the Outcomes sections — fails this pin
/// with count 0, and so does rewording away the MUST, the every-existing
/// scope, or the verbatim-ness. A vacuous pin (a needle that matches
/// pre-existing text regardless of the sentence) would have stayed green
/// through the amputation; this leg cannot.
#[test]
fn carry_forward_sentence_is_pinned_non_vacuously() {
    let spec = flat(&meta_meta_spec());
    assert_eq!(
        spec.matches(CARRY_CORE).count(),
        1,
        "META-META-SPEC must state the carry-forward sentence core \
         ({CARRY_CORE:?}) exactly once — zero means the sentence was \
         deleted (the cycle-72 amputation class) or reworded, more than \
         one means it is stated twice"
    );
}

/// (b placement) The sentence lives INSIDE the Write-EVALUATION section,
/// in its preamble BEFORE the numbered section list — it governs the whole
/// eval document, so it sits where the doc shape is introduced. Delete the
/// sentence or move it into another section and this goes red.
#[test]
fn carry_forward_sentence_sits_in_the_write_evaluation_preamble() {
    let spec_text = meta_meta_spec();
    let flat_window = flat(write_eval_window(&spec_text));
    let core_needle = flat(CARRY_CORE);
    let core = flat_window
        .find(core_needle.as_str())
        .unwrap_or_else(|| panic!("the Write-EVALUATION window must carry the carry-forward sentence — it was deleted or moved out of the section"));
    let sections = flat_window
        .find(SECTIONS_MARKER)
        .unwrap_or_else(|| panic!("the Write-EVALUATION window must carry the {SECTIONS_MARKER:?} preamble marker"));
    assert!(
        core < sections,
        "the carry-forward sentence must sit in the section preamble \
         BEFORE the numbered section list, not after it"
    );
}

/// (c) Cheap structural sanity: the current EVALUATION.md still carries a
/// `## Outcomes` heading — the ledger the sentence protects still exists
/// for the eval to carry forward. This is NOT a full history check (no
/// per-cycle diffing; the mechanical history-DIFF guard was ruled out as
/// fragile across cycle numbering): amputating the heading, or the whole
/// Outcomes section with it, is what goes red here.
#[test]
fn eval_still_carries_the_outcomes_heading() {
    assert!(
        OUTCOMES.starts_with('O'),
        "the heading needle must be the bare Outcomes token"
    );
    let eval = evaluation_md();
    assert!(
        eval.contains("## Outcomes"),
        "EVALUATION.md must still carry a `## Outcomes` heading — zero means \
         the Outcomes ledger was amputated from the eval doc, the exact loss \
         the carry-forward sentence exists to prevent"
    );
}
