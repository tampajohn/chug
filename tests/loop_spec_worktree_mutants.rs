//! T167 — orchestrator RED-proofs apply mutants in the worktree cwd,
//! never the main tree.
//!
//! LOOP-SPEC §2 step 3 (Review) owns the orchestrator's personal RED-proof
//! practice for tests-only rounds (mutant → gate → restore). The file
//! tools are cwd-confined to the CALLER's tree — for the orchestrator that
//! is the MAIN repo — while the gates under proof run the child's
//! `/tmp/chug-loop-t<N>` worktree copy, so a mutant applied through
//! `edit_file`/`write_file` lands in a file the gates never read and the
//! RED-proof "survives" trivially (cross-tree edits must go through bash —
//! the documented escape hatch). Cycle-75 wrap (T160 entry, commit
//! f2d0977): the orchestrator applied a mutant through `edit_file` — it
//! edited MAIN's src/mcp_serve.rs while the gates ran the WORKTREE copy,
//! so the mutant survived for real; caught on the first read-back, main
//! restored byte-identical, the worktree mutants re-applied via bash
//! `perl` in the worktree cwd. A read-back habit was all that stopped a
//! false RED-proof from shipping. The remedy is doctrine, not tooling:
//! `edit_file`'s confinement is a correct safety boundary (T167 out of
//! scope), so step 3 gains ONE sentence in the RED-proof / docs-only-guard
//! neighborhood mandating bash-applied mutation legs (cwd inside the
//! worktree) and a byte-identical main-restore check after any main-tree
//! edit during a round.
//!
//! Legs:
//! (a) the pinned sentence occurs EXACTLY once in LOOP-SPEC.md — the
//!     whole sentence as ONE flat needle, so deletion, duplication, AND
//!     any rewording of its bytes go red (the T78 flat idiom: the
//!     sentence wraps mid-phrase in the file);
//! (b) the sentence's load-bearing tokens are each pinned at the
//!     strongest exactly-once scope the token supports: `byte-identical`
//!     whole-file (it is unique in the file — 0 occurrences before this
//!     sentence); `edit_file` window-scoped to step 3 AND step 5 (its one
//!     other occurrence is step 5's "Prefer `edit_file` … over sed" rule,
//!     so the whole-file count is 2 — one per window; the FLOOR_CMD
//!     window-scoped precedent in loop_spec_docs_only_gates.rs);
//!     `worktree` scoped to the pinned sentence's own span (27
//!     pre-existing occurrences saturate the file — T19/T44/T52/T78/T79
//!     doctrine is built on the word — so no whole-file count can read
//!     exactly-once; the census is taken inside the sentence the token is
//!     load-bearing IN, the check_wall census idiom);
//! (c) placement: the sentence lives INSIDE step 3's window, AFTER the
//!     docs-only ambiguity default it neighbors and before step 4's
//!     heading (the RED-proof / docs-only-guard neighborhood), with no
//!     step renumbering (T19/T30 rule).
//!
//! T48 doctrine: every pin resolves LOOP-SPEC.md from the checkout the
//! binary RUNS against (`std::env::current_dir()`; cargo runs test
//! binaries with cwd = the package root), never via the compile-time
//! manifest-dir macro — under the T47 shared cache a compile-time path can
//! point at a since-removed worktree.

/// (a) The pinned sentence, whole, as ONE needle. Spelled single-spaced:
/// it is only ever matched against `flat()` text (the sentence wraps
/// across five line breaks in the file). The non-ASCII byte is spelled as
/// an escape so an editor normalization cannot silently unpin the needle
/// (\u{2014} = em dash — the sentence's one non-ASCII byte).
const SENTENCE: &str = "Orchestrator mutation legs for RED-proofs MUST be applied inside the worktree via bash (e.g. `perl -i` with cwd `/tmp/chug-loop-t<N>`) \u{2014} `edit_file`/`write_file` are confined to the main tree and silently produce false survivors when the gates under proof run the worktree copy; after any main-tree edit during a round, verify main is restored byte-identical before merging.";

/// (b) The three load-bearing tokens (the spec names them verbatim). The
/// sentence uses `worktree` twice ("applied inside the worktree via
/// bash", "run the worktree copy") — that census is pinned in
/// [`worktree_census_inside_the_pinned_sentence`].
const WORKTREE: &str = "worktree";
const EDIT_FILE: &str = "edit_file";
const BYTE_IDENTICAL: &str = "byte-identical";

/// (b) The worktree-census span anchors — the sentence's head and tail,
/// chosen token-free of `worktree` so the span derivation cannot
/// presuppose the census it measures. Each must occur EXACTLY once.
const SENTENCE_HEAD: &str = "Orchestrator mutation legs for RED-proofs";
const SENTENCE_TAIL: &str = "verify main is restored byte-identical before merging.";

/// (c) Step headings, matched LOOSELY — number + bold marker only (the
/// T64 heading-scope pattern): a wording tweak of a heading's text must
/// not break the scope legs. Each is unique in the file today.
const STEP3: &str = "3. **Review.**";
const STEP4: &str = "4. **Adversarial validation";
const STEP5: &str = "5. **Harvest";
const STEP6: &str = "6. **Budget check";

/// (c) The docs-only ambiguity default — the sentence's NEIGHBOR: the
/// RED-proof sentence must sit after the guard block it extends, not
/// bolted onto another step.
const AMBIGUITY_DEFAULT: &str = "defaults to full gates.";

fn loop_spec() -> String {
    let root = std::env::current_dir().expect("cargo sets the test cwd to the package root");
    std::fs::read_to_string(root.join("LOOP-SPEC.md"))
        .unwrap_or_else(|e| panic!("reading LOOP-SPEC.md from the runtime checkout: {e}"))
}

/// Wrap-insensitive copy of the spec (the T78 flat idiom): the pinned
/// sentence wraps mid-phrase, so the whole-sentence needle must match
/// whitespace-collapsed text or the pin goes red on the REAL doctrine.
fn flat(spec: &str) -> String {
    spec.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Step-window helper (T64 heading-scope pattern): the raw text between
/// two loose step headings.
fn window_between<'a>(spec: &'a str, start_marker: &str, end_marker: &str, what: &str) -> &'a str {
    let start = spec
        .find(start_marker)
        .unwrap_or_else(|| panic!("LOOP-SPEC must carry {what}'s start marker {start_marker:?}"));
    let end = start
        + spec[start..]
            .find(end_marker)
            .unwrap_or_else(|| panic!("LOOP-SPEC must carry {what}'s end marker {end_marker:?} after it"));
    &spec[start..end]
}

/// (a) The pinned sentence occurs in LOOP-SPEC.md exactly once. Delete
/// the sentence and this goes red (count 0); duplicate the statement
/// elsewhere and it goes red (count 2); reword ANY byte of the sentence
/// (a token swap, a MUST → should softening, a dropped `perl -i` cwd
/// example) and the flat needle no longer matches — red. This is the
/// deletion hand-check's primary leg.
#[test]
fn pinned_sentence_occurs_exactly_once() {
    // Needle self-check (T48 idiom): a mangled needle must not let this
    // pin pass silently — it must still carry every load-bearing token
    // the sentence exists to pin.
    assert!(
        SENTENCE.starts_with("Orchestrator mutation legs")
            && SENTENCE.ends_with("before merging.")
            && SENTENCE.contains("MUST be applied inside the worktree via bash")
            && SENTENCE.contains("`perl -i` with cwd `/tmp/chug-loop-t<N>`")
            && SENTENCE.contains("`edit_file`/`write_file`")
            && SENTENCE.contains("silently produce false survivors")
            && SENTENCE.contains(WORKTREE)
            && SENTENCE.contains(EDIT_FILE)
            && SENTENCE.contains(BYTE_IDENTICAL),
        "the sentence needle must carry the mandate, the bash cwd example, \
         the confinement clause, and all three load-bearing tokens verbatim"
    );
    assert!(
        SENTENCE.contains('\u{2014}'),
        "the em dash must stay spelled as an escape so an editor \
         normalization cannot silently unpin the needle"
    );
    let spec = flat(&loop_spec());
    assert_eq!(
        spec.matches(SENTENCE).count(),
        1,
        "LOOP-SPEC step 3 must state the worktree-mutant RED-proof sentence \
         exactly once — zero means the sentence was deleted or reworded \
         (the false-survivor hazard this row closes is unpinned again), \
         more than one means it is stated twice"
    );
}

/// (b) `byte-identical` occurs in LOOP-SPEC.md exactly once — the one
/// load-bearing token unique enough for a whole-file exactly-once pin (0
/// occurrences before this sentence; the file's nearby "byte-clean" is a
/// different token and does not match). Rewording the restore clause away
/// from the hyphenated compound (e.g. to "byte-clean") goes red here.
#[test]
fn byte_identical_token_occurs_exactly_once() {
    // Needle self-check (T48 idiom).
    assert!(
        BYTE_IDENTICAL.starts_with("byte-") && BYTE_IDENTICAL.ends_with("-identical"),
        "the needle must be the hyphenated compound verbatim"
    );
    let spec = loop_spec();
    assert_eq!(
        spec.matches(BYTE_IDENTICAL).count(),
        1,
        "LOOP-SPEC must carry `byte-identical` exactly once — zero means \
         the main-restore clause was dropped or reworded to a non-compound \
         (the cycle-75 read-back check is unpinned), more than one means a \
         second statement of it drifted in"
    );
    // And it lives inside step 3's window, inside the pinned sentence's
    // tail (the restore clause is review-step doctrine, not a stray
    // mention elsewhere).
    let step3 = flat(window_between(&spec, STEP3, STEP4, "step 3"));
    assert!(
        step3.contains(SENTENCE_TAIL),
        "the byte-identical restore clause must live inside step 3's window"
    );
}

/// (b) `edit_file` occurs exactly once per window — step 3's (the new
/// confinement sentence) and step 5's (the pre-existing "Prefer
/// `edit_file` … over sed" rule). The whole-file count is 2 — one per
/// window — so the pin is window-scoped (the FLOOR_CMD precedent): a
/// third occurrence elsewhere cannot break it, while the deletion of
/// EITHER window's occurrence goes red.
#[test]
fn edit_file_token_occurs_exactly_once_per_window() {
    // Needle self-check (T48 idiom).
    assert_eq!(EDIT_FILE, "edit_file", "the needle must be the token verbatim");
    let spec = loop_spec();
    let step3 = flat(window_between(&spec, STEP3, STEP4, "step 3"));
    let step5 = flat(window_between(&spec, STEP5, STEP6, "step 5"));
    assert_eq!(
        step3.matches(EDIT_FILE).count(),
        1,
        "step 3's worktree-mutant sentence must name `edit_file` exactly \
         once — zero means the confinement clause was dropped (the tool \
         whose cwd-confinement produces the false survivor is unnamed), \
         more than one means the sentence is duplicated"
    );
    assert_eq!(
        step5.matches(EDIT_FILE).count(),
        1,
        "step 5's pre-existing sed-rule preference (`edit_file` errors on \
         no-match) must stay intact — this pin guards both windows, so \
         neither occurrence can silently vanish"
    );
}

/// (b) `worktree`'s census, scoped to the pinned sentence's own span: the
/// sentence uses it exactly twice ("applied inside the worktree via
/// bash", "run the worktree copy"). A whole-file census is impossible —
/// 27 pre-existing occurrences saturate the file — so the exactly-once
/// (here: exactly-two) statement is taken inside the span the token is
/// load-bearing in, derived from token-free head/tail anchors (the
/// check_wall census idiom). Zero = the sentence was deleted; one = an
/// instance was reworded away; three+ = a second statement drifted in.
#[test]
fn worktree_census_inside_the_pinned_sentence() {
    // Needle self-check (T48 idiom): the anchors must not presuppose the
    // census — neither may contain the counted token.
    assert!(
        SENTENCE_HEAD.starts_with("Orchestrator") && !SENTENCE_HEAD.contains(WORKTREE),
        "the head anchor must be the sentence's opening phrase, worktree-free"
    );
    assert!(
        SENTENCE_TAIL.starts_with("verify main") && !SENTENCE_TAIL.contains(WORKTREE),
        "the tail anchor must be the sentence's closing clause, worktree-free"
    );
    let spec = flat(&loop_spec());
    for (anchor, what) in [
        (SENTENCE_HEAD, "the sentence's head anchor"),
        (SENTENCE_TAIL, "the sentence's tail anchor"),
    ] {
        assert_eq!(
            spec.matches(anchor).count(),
            1,
            "LOOP-SPEC must carry {what} exactly once — zero means the \
             pinned sentence was deleted or reworded, more than one means \
             the census span below is ill-defined"
        );
    }
    let start = spec.find(SENTENCE_HEAD).expect("head anchor present");
    let end = spec.find(SENTENCE_TAIL).expect("tail anchor present") + SENTENCE_TAIL.len();
    assert!(
        start < end,
        "the sentence's head must precede its tail — the sentence was \
         restructured out from under the census"
    );
    let span = &spec[start..end];
    assert_eq!(
        span.matches(WORKTREE).count(),
        2,
        "the pinned sentence must use `worktree` exactly twice — the \
         mandate leg (applied inside the worktree via bash) and the hazard \
         leg (the gates under proof run the worktree copy); any other \
         count means the sentence was deleted, reworded, or duplicated"
    );
}

/// (c) Placement: the sentence lives INSIDE step 3's window, AFTER the
/// docs-only ambiguity default it neighbors (the RED-proof /
/// docs-only-guard neighborhood the spec names) and before step 4's
/// heading — with both headings intact and unrenumbered (T19/T30 rule;
/// the T80 override folded into these same steps, never renumbered them).
#[test]
fn sentence_sits_in_step_3_after_the_docs_only_guard() {
    let spec = loop_spec();
    for (marker, what) in [
        (STEP3, "step 3 (Review)"),
        (STEP4, "step 4 (Adversarial validation)"),
    ] {
        assert_eq!(
            spec.matches(marker).count(),
            1,
            "LOOP-SPEC §2 must keep {what}'s heading ({marker:?}) exactly \
             once — the T167 sentence folds INTO step 3, never renumbers it"
        );
    }
    let step3_flat = flat(window_between(&spec, STEP3, STEP4, "step 3"));
    let sentence_at = step3_flat
        .find(SENTENCE)
        .unwrap_or_else(|| panic!("step 3's window must carry the pinned sentence"));
    let guard_at = step3_flat
        .find(AMBIGUITY_DEFAULT)
        .unwrap_or_else(|| panic!("step 3's window must carry the docs-only ambiguity default"));
    assert!(
        guard_at < sentence_at,
        "the RED-proof sentence must sit AFTER the docs-only guard block it \
         neighbors (ambiguity default at {guard_at}, sentence at \
         {sentence_at}) — the RED-proof / docs-only-guard neighborhood, \
         not a bolt-on elsewhere in the step"
    );
}
