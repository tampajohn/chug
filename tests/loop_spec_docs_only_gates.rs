//! T80 — doctrine pins: LOOP-SPEC §2's docs-only rounds skip the cargo gates.
//!
//! A docs-only round (a README clause, a doctrine sentence — the T22/T30/
//! T33/T34/T35/T40/T72 class) pays the full gate suite 2–4 times per item:
//! worktree review gates, the validator's independent re-run, and main's
//! post-merge gates — 500+ tests + clippy, ~10–30s each even warm, on a
//! diff the suite cannot go red for (guard suites excepted). The remedy is
//! orchestrator-side and mechanical: when the round diff touches ONLY
//! `*.md`, the gates shrink to the guard floor (`cargo test --test
//! todo_consistency`, plus `bash -n` on any `.sh` in the diff) at review
//! AND post-merge, with the classification stated as a template command so
//! no judgment call is needed; validators keep an explicit escape clause
//! (a doc diff that QUOTES commands or check lines — the T67 class, where a
//! spec `check:` line IS executable text — may still earn a full-suite
//! run), and any classification ambiguity defaults to full gates.
//!
//! These pins assert the override exists and sits where the spec requires:
//! the predicate + reduced gate set + ambiguity default inside §2 step 3
//! (where the gates are defined), the validator escape clause inside step 4,
//! and the post-merge shrink inside step 5 — no step renumbering (T19/T30
//! rule). No production code changes; this file is doctrine-only.
//!
//! T48 doctrine: every pin resolves LOOP-SPEC.md from the checkout the
//! binary RUNS against (`std::env::current_dir()`; cargo runs test binaries
//! with cwd = the package root), never via the compile-time manifest-dir
//! macro — under the T47 shared cache a compile-time path can point at a
//! since-removed worktree.

/// (a) The override's name phrase. Must occur EXACTLY once in LOOP-SPEC.md:
/// zero means the override was dropped, more than one means it is stated
/// twice (the T67 self-match lesson — the pin file greps LOOP-SPEC, never a
/// spec prose copy).
const DOCS_ONLY: &str = "Docs-only rounds skip the cargo gates";

/// (b) The mechanical classification command, verbatim (requirement: the
/// classification is mechanical and stated in the template). Must occur
/// EXACTLY once. In Rust source the needle spells the backslash and dollar
/// literally: it matches the file's `grep -qvE '\.md$'`.
const PREDICATE: &str = "grep -qvE '\\.md$'";

/// (c) The prose predicate is file-extension-exact — not "mostly docs" (a
/// "mostly docs" rule would let one .rs file through unguarded). Each must
/// occur EXACTLY once.
const ONLY_MD: &str = "ONLY `*.md`";
const FILE_EXT_EXACT: &str = "file-extension-exact";
const NOT_MOSTLY: &str = "not \"mostly docs\"";

/// (d) The exit-code semantics: any non-`.md` file → full gates (exit 0),
/// every file `.md` → the reduced set (exit 1). The arrow is spelled as an
/// escape so an editor normalization cannot silently unpin it
/// (\u{2192} = →). "→ full gates" must occur EXACTLY once: the ambiguity
/// default below states its fallback WITHOUT the arrow ("defaults to full
/// gates"), so a second arrow occurrence means a duplicate classification
/// statement.
const FULL_GATES_ARROW: &str = "\u{2192} full gates";

/// (e) The reduced gate set: the guard-floor phrase, the guard-suite floor
/// command, the `bash -n` leg, and the skip scope (full build/clippy/test
/// skipped at review AND post-merge). The phrase/command/leg needles must
/// each occur EXACTLY once in LOOP-SPEC.md. FLOOR_CMD is pinned
/// window-scoped, not whole-file: step 5 already carries one
/// `cargo test --test todo_consistency` occurrence (the T8 guard run after
/// every TODO.md edit), so the whole-file count is 2 — one per window.
const FLOOR: &str = "gates shrink to the guard floor";
const FLOOR_CMD: &str = "cargo test --test todo_consistency";
const BASH_N: &str = "bash -n";
const SKIP: &str = "skipped at review AND post-merge";

/// (f) The validator escape clause (T67 class): validators retain the right
/// to run the full suite when the doc diff quotes commands/check lines — a
/// spec `check:` line IS executable text. Both needles must occur EXACTLY
/// once in LOOP-SPEC.md (today T67 is named nowhere else in it).
const T67: &str = "T67";
const EXECUTABLE_TEXT: &str = "a spec `check:` line IS executable text";

/// (g) The ambiguity default: any classification ambiguity → full gates
/// (the conservative fallback — an unevaluable predicate must never shrink
/// the gates). Each needle must occur EXACTLY once.
const AMBIGUITY: &str = "Any ambiguity in the classification";
const DEFAULTS_FULL: &str = "defaults to full gates";

/// (h) The post-merge shrink: step 5 applies the same classification, the
/// guard floor replaces the full suite there too. Must occur EXACTLY once.
/// (Worded to avoid the T57/T64-guarded strings — no
/// `target-shared-main` env prefix, no `ALWAYS, never conditionally`, no
/// `applies here too` — so the step-5 pins in shared_target_dir.rs keep
/// their exact counts.)
const POST_MERGE_SHRINK: &str = "the guard floor replaces the full suite here too";

/// Step headings, matched LOOSELY — number + bold marker only (the T64
/// heading-scope pattern): a wording tweak of a heading's text must not
/// break the scope legs. Each is unique in the file today; `find` +
/// `find`-after scopes each step window.
const STEP3: &str = "3. **Review.**";
const STEP4: &str = "4. **Adversarial validation";
const STEP5: &str = "5. **Harvest";
const STEP6: &str = "6. **Budget check";

fn loop_spec() -> String {
    let root = std::env::current_dir().expect("cargo sets the test cwd to the package root");
    std::fs::read_to_string(root.join("LOOP-SPEC.md"))
        .unwrap_or_else(|e| panic!("reading LOOP-SPEC.md from the runtime checkout: {e}"))
}

/// Step-window helper (T64 heading-scope pattern): the text between two
/// loose step headings, plus the window's byte offsets for after-anchored
/// assertions.
fn window_between<'a>(
    spec: &'a str,
    start_marker: &str,
    end_marker: &str,
    what: &str,
) -> &'a str {
    let start = spec
        .find(start_marker)
        .unwrap_or_else(|| panic!("LOOP-SPEC must carry {what}'s start marker {start_marker:?}"));
    let end = start
        + spec[start..]
            .find(end_marker)
            .unwrap_or_else(|| panic!("LOOP-SPEC must carry {what}'s end marker {end_marker:?} after it"));
    &spec[start..end]
}

/// Wrap-insensitive copy of the spec (the T78 flat-readme idiom, hardened
/// for the step paragraphs' 3-space continuation indent): the doctrine
/// prose wraps mid-phrase, so a multi-word needle that crosses a break
/// point must match whitespace-collapsed text or the pin goes red on the
/// REAL doctrine — the cycle-44 orchestrator-review catch, where FLOOR
/// ("gates shrink to the guard floor") and POST_MERGE_SHRINK ("the guard
/// floor replaces the full suite here too") both spanned their sentences'
/// wrap points and the committed pins failed against the committed text.
fn flat(spec: &str) -> String {
    spec.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// (a) The override's name phrase occurs in LOOP-SPEC.md exactly once.
/// Delete the inserted step-3 block and this goes red (count 0); a
/// duplicate statement of the override elsewhere also goes red.
#[test]
fn docs_only_override_name_occurs_exactly_once() {
    // Needle self-check (T48 idiom): a mangled needle must not let this
    // pin pass silently.
    assert!(
        DOCS_ONLY.contains("Docs-only") && DOCS_ONLY.contains("cargo gates"),
        "the needle must carry the override's name phrase"
    );
    let spec = loop_spec();
    assert_eq!(
        spec.matches(DOCS_ONLY).count(),
        1,
        "LOOP-SPEC must name the docs-only gate override exactly once — zero \
         means the override was dropped, more than one means it is stated twice"
    );
}

/// (b)+(c)+(d) The classification is mechanical, file-extension-exact, and
/// its exit-code semantics are stated: the template command occurs exactly
/// once, the prose predicate says ONLY `*.md` / file-extension-exact / not
/// "mostly docs", and the exit-0 leg's `→ full gates` occurs exactly once
/// (the ambiguity default states its fallback without the arrow).
#[test]
fn predicate_is_mechanical_and_file_extension_exact() {
    assert!(
        PREDICATE.contains("qvE") && PREDICATE.ends_with(".md$'"),
        "the needle must carry the inverted-grep predicate command"
    );
    assert!(
        ONLY_MD.contains("*.md") && FILE_EXT_EXACT.contains("extension") && NOT_MOSTLY.contains("mostly"),
        "the needles must carry the file-extension-exact predicate prose"
    );
    let spec = loop_spec();
    for (needle, what) in [
        (PREDICATE, "the mechanical classification command"),
        (ONLY_MD, "the ONLY-`*.md` prose predicate"),
        (FILE_EXT_EXACT, "the file-extension-exact stance"),
        (NOT_MOSTLY, "the not-\"mostly-docs\" guard"),
        (FULL_GATES_ARROW, "the exit-0 → full gates leg"),
    ] {
        assert_eq!(
            spec.matches(needle).count(),
            1,
            "LOOP-SPEC must state {what} exactly once — zero means it was \
             dropped or reworded, more than one means it is stated twice"
        );
    }
    // The command's exit-1 leg (every changed file ends `.md`) is stated
    // too — the reduced-set arm of the mechanical rule. Scoped to step 3's
    // window (step 5's classification cross-reference wraps the same words
    // across a line break, and is pinned by its own needle).
    let step3 = window_between(&spec, STEP3, STEP4, "step 3");
    assert!(
        step3.contains("every changed file ends `.md`"),
        "step 3 must state the exit-1 arm (every changed file ends `.md`) \
         → the reduced set"
    );
}

/// (e) The reduced gate set: guard-floor phrase, `bash -n` leg, and skip
/// scope each occur exactly once (whole file); the guard-suite floor
/// command occurs exactly once INSIDE step 3's window (the new override's
/// floor) and the pre-existing T8 occurrence stays exactly once inside
/// step 5's window (the guard run after every TODO.md edit) — window-scoped
/// so neither leg's deletion goes unnoticed while a third occurrence
/// elsewhere cannot break the pin.
#[test]
fn reduced_gate_set_names_the_guard_floor_and_skip_scope() {
    assert!(
        FLOOR.contains("guard floor") && SKIP.contains("review AND post-merge"),
        "the needles must carry the floor phrase and the skip scope"
    );
    // Wrap-insensitive: FLOOR's phrase crosses the step-3 paragraph's wrap
    // point ("gates\n   shrink to the guard floor"); the T78 flat idiom.
    let spec = flat(&loop_spec());
    for (needle, what) in [
        (FLOOR, "the gates-shrink-to-the-guard-floor phrase"),
        (BASH_N, "the `bash -n` leg"),
        (SKIP, "the skipped-at-review-AND-post-merge scope"),
    ] {
        assert_eq!(
            spec.matches(needle).count(),
            1,
            "LOOP-SPEC must state {what} exactly once — zero means it was \
             dropped, more than one means it is stated twice"
        );
    }
    let step3 = window_between(&spec, STEP3, STEP4, "step 3");
    let step5 = window_between(&spec, STEP5, STEP6, "step 5");
    assert_eq!(
        step3.matches(FLOOR_CMD).count(),
        1,
        "step 3's docs-only override must name the guard-suite floor \
         `cargo test --test todo_consistency` exactly once"
    );
    assert_eq!(
        step5.matches(FLOOR_CMD).count(),
        1,
        "step 5's pre-existing T8 guard run (`cargo test --test \
         todo_consistency` after every TODO.md edit) must stay intact"
    );
}

/// (f) The validator escape clause exists inside step 4: validators retain
/// the right to run the full suite when the doc diff quotes commands or
/// check lines — the T67 class (a spec `check:` line IS executable text).
/// Delete the sentence and both needles go red.
#[test]
fn validator_escape_clause_lives_inside_step_4() {
    assert!(
        EXECUTABLE_TEXT.contains("executable text"),
        "the needle must carry the executable-text escape"
    );
    let spec = loop_spec();
    for (needle, what) in [
        (T67, "the T67 class citation"),
        (EXECUTABLE_TEXT, "the executable-text escape clause"),
    ] {
        assert_eq!(
            spec.matches(needle).count(),
            1,
            "LOOP-SPEC must carry {what} exactly once — zero means the \
             validator escape clause was dropped, more than one means it is \
             stated twice"
        );
    }
    let step4 = window_between(&spec, STEP4, STEP5, "step 4");
    for needle in [T67, EXECUTABLE_TEXT] {
        assert!(
            step4.contains(needle),
            "the escape-clause needle {needle:?} must live inside step 4"
        );
    }
}

/// (g) The ambiguity default: any classification ambiguity defaults to full
/// gates — the conservative fallback that keeps the override honest. Both
/// needles occur exactly once, inside step 3's window (where the
/// classification lives).
#[test]
fn ambiguity_defaults_to_full_gates_inside_step_3() {
    assert!(
        AMBIGUITY.contains("Any ambiguity") && DEFAULTS_FULL.contains("full gates"),
        "the needles must carry the ambiguity default"
    );
    let spec = loop_spec();
    let step3 = window_between(&spec, STEP3, STEP4, "step 3");
    for (needle, what) in [
        (AMBIGUITY, "the any-ambiguity clause"),
        (DEFAULTS_FULL, "the defaults-to-full-gates fallback"),
    ] {
        assert_eq!(
            spec.matches(needle).count(),
            1,
            "LOOP-SPEC must state {what} exactly once — zero means the \
             ambiguity default was dropped, more than one means it is \
             stated twice"
        );
        assert!(
            step3.contains(needle),
            "the needle {needle:?} must live inside step 3's window — the \
             ambiguity default sits where the classification is defined"
        );
    }
}

/// (h) The post-merge shrink lives inside step 5: the same classification,
/// the guard floor replaces the full suite there too. Delete the sentence
/// and this goes red; duplicating it elsewhere also goes red.
#[test]
fn post_merge_gate_shrinks_for_docs_only_rounds_inside_step_5() {
    assert!(
        POST_MERGE_SHRINK.contains("guard floor") && POST_MERGE_SHRINK.contains("here too"),
        "the needle must carry the step-5 shrink clause"
    );
    // Wrap-insensitive: the shrink sentence wraps between "the guard
    // floor" and "replaces the full suite here too" in step 5's paragraph.
    let spec = flat(&loop_spec());
    assert_eq!(
        spec.matches(POST_MERGE_SHRINK).count(),
        1,
        "LOOP-SPEC must state the step-5 post-merge shrink exactly once — \
         zero means it was dropped, more than one means it is stated twice"
    );
    let step5 = window_between(&spec, STEP5, STEP6, "step 5");
    assert!(
        step5.contains(POST_MERGE_SHRINK),
        "the post-merge shrink must live inside step 5's window"
    );
    // The shrink sentence must not stand in for the T57 main-dedicated-dir
    // rule: step 5's window still carries the full-gates form the shrink
    // replaces (pinned exactly by shared_target_dir.rs; here only the
    // coexistence is asserted, so a shrink that DELETED the full-gate
    // sentence instead of adding alongside it goes red).
    assert!(
        step5.contains("cargo test --release -- --test-threads=4"),
        "step 5 must keep the T78 full-gate form alongside the docs-only \
         shrink — the shrink replaces it only for docs-only rounds"
    );
}

/// No step renumbering (T19/T21/T30 rule): all four §2 step headings are
/// still present, each exactly once — the override folds INTO steps 3/4/5,
/// never inserts a new numbered step.
#[test]
fn step_headings_survive_unrenumbered() {
    let spec = loop_spec();
    for (marker, what) in [
        (STEP3, "step 3 (Review)"),
        (STEP4, "step 4 (Adversarial validation)"),
        (STEP5, "step 5 (Harvest, then merge + close)"),
        (STEP6, "step 6 (Budget check)"),
    ] {
        assert_eq!(
            spec.matches(marker).count(),
            1,
            "LOOP-SPEC §2 must keep {what}'s heading ({marker:?}) exactly \
             once — the T80 override folds into the existing steps, never \
             renumbers them"
        );
    }
}
