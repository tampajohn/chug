//! T100 — LOOP-SPEC Phase 3 tag doctrine pins.
//!
//! The loop cutting its own release tags is an OPERATOR OVERRIDE of the
//! earlier never-self-tag guardrail (2026-09-28) — exactly the kind of
//! doctrine sentence that drifts silently unless a pin file greps it back
//! (the T80/T87 class). The override's full shape must stay in §Phase 3
//! (where wrap happens): the trigger (≥3 items OR a FEATURES check-off since
//! the last tag), the bump rule, the verify-then-tag sequence, the hard
//! rules (never re-tag/move/force-push a tag, gates green at HEAD, one per
//! wrap, notes as the tag message), the failed-workflow row rule, and the
//! BOOTSTRAP clause — the FIRST tag is operator-cut, so the loop invents
//! nothing while no `v*` tag exists.
//!
//! T48 doctrine: LOOP-SPEC.md is resolved from the checkout the binary RUNS
//! against (`std::env::current_dir()`), never the compile-time manifest-dir
//! macro; T78 idiom: multi-word needles are matched against
//! whitespace-collapsed text because the doctrine prose wraps mid-phrase.

fn loop_spec() -> String {
    let root = std::env::current_dir().expect("cargo sets the test cwd to the package root");
    std::fs::read_to_string(root.join("LOOP-SPEC.md"))
        .unwrap_or_else(|e| panic!("reading LOOP-SPEC.md from the runtime checkout: {e}"))
}

fn flat(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// (a) The override's name phrase occurs exactly once, inside Phase 3 — zero
/// means the doctrine was dropped, two means it is stated twice.
#[test]
fn tag_doctrine_name_occurs_exactly_once_inside_phase_3() {
    let spec = loop_spec();
    const NAME: &str = "Tag at wrap (T100";
    assert_eq!(
        spec.matches(NAME).count(),
        1,
        "LOOP-SPEC must name the wrap-time tag doctrine exactly once"
    );
    let phase3 = spec
        .split("## Phase 3")
        .nth(1)
        .expect("LOOP-SPEC must carry `## Phase 3`")
        .to_string();
    let phase3 = phase3.split("## Hard rules").next().unwrap_or("").to_string();
    assert!(
        phase3.contains(NAME),
        "the tag doctrine must live in Phase 3 (wrap), not the hard-rules dump"
    );
}

/// (b) The trigger + bump rule, wrap-insensitive (the sentences wrap).
#[test]
fn trigger_and_bump_rule_are_stated() {
    let flat_spec = flat(&loop_spec());
    for (needle, what) in [
        (
            "≥3 items landed since the last `v*` tag OR any FEATURES.md check-off landed",
            "the wrap tag trigger",
        ),
        (
            "minor for a feature item, patch otherwise",
            "the minor/patch bump rule",
        ),
        ("commit `chore: release vX.Y.Z`", "the release commit subject"),
        (
            "scripts/check-tag-version.sh",
            "the divergence check the loop runs before tagging",
        ),
        ("scripts/release-notes.sh", "the mechanical notes generator"),
        ("git tag -a vX.Y.Z -F <notes-file>", "the annotated-tag command"),
    ] {
        assert_eq!(
            flat_spec.matches(needle).count(),
            1,
            "LOOP-SPEC must state {what} exactly once — zero means it was \
             dropped or reworded, more than one means it is duplicated"
        );
    }
}

/// (c) The hard rules: immutable tags (never re-tag/move/force-push), gates
/// green at HEAD, one tag per wrap max, notes as the tag message, and the
/// failed-workflow row rule (never delete the published tag).
#[test]
fn hard_rules_are_stated() {
    let flat_spec = flat(&loop_spec());
    for (needle, what) in [
        ("never re-tag or move a tag", "the no-re-tag rule"),
        ("never force-push tags", "the no-force-push-tags rule"),
        ("tag ONLY with gates green at HEAD", "the gates-green precondition"),
        ("one tag per wrap max", "the one-tag-per-wrap cap"),
        ("tag message = the generated notes since the previous tag", "the notes-as-message rule"),
        ("a published tag is immutable history, never deleted or moved", "the failed-workflow row rule"),
    ] {
        assert_eq!(
            flat_spec.matches(needle).count(),
            1,
            "LOOP-SPEC must state {what} exactly once"
        );
    }
}

/// (d) The BOOTSTRAP clause names the operator-cut first tag — without it a
/// cold loop facing "count since the last tag" with no tag on the books
/// would have to invent one.
#[test]
fn bootstrap_names_the_operator_cut_first_tag() {
    let flat_spec = flat(&loop_spec());
    for (needle, what) in [
        ("BOOTSTRAP: the FIRST tag is operator-cut", "the bootstrap clause"),
        ("until a `v*` tag exists, the loop never tags", "the no-invention rule"),
        (
            "once the operator's first tag lands, this doctrine is active from the next wrap on",
            "the activation clause",
        ),
    ] {
        assert_eq!(
            flat_spec.matches(needle).count(),
            1,
            "LOOP-SPEC must state {what} exactly once — the bootstrap is what \
             makes the override safe before the operator's first tag"
        );
    }
}
