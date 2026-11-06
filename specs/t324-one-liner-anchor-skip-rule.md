# T324 — wrap_assert one-liner anchor skip-rule (the T316 extractor first-occurrence coverage gap)

estimate: ~120 changed lines (scripts/wrap_assert.py + its python fixture battery)

## Why

The T316 one-liner guard (`assert_outcomes_one_liner`) extracts five figure
groups from a full Outcomes entry by latching the FIRST anchor occurrence in
the text. Three production guard-fires prove the first-occurrence rule is
under-specified — each latched a non-anchor mention and failed closed
(extraction failure), adjudicated EQUAL BY HAND at zero casualty:

1. the 19th production exercise (cycle-3024 wrap, the 3000 one-liner): the
   re-arm group's first-`re-arm` rule latched the full entry's retrospective
   clause "the class re-armed at the FIFTH" (a `re-arm(ed)? at the` mention),
   probing (9,) vs the one-liner's (619,).
2. the 20th (cycle-3025 wrap, the 3004 one-liner): same re-arm
   clause-mention latch.
3. the 21st (cycle-3029 wrap, the 3008 one-liner): the establishment group's
   first-`establishment` rule latched the bare prose mention "the
   establishment figure 125/3,411 unaffected and TRUE" — no `<N>th` ordinal
   adjacent — before the group anchor "the 692nd establishment (3004 stream
   ...)".

The 3025 eval's triage d1793919184-5 pre-authorized this filing: "the 3rd
occurrence files the extractor skip-rule".

## What

In `scripts/wrap_assert.py`:

1. `_ol_establishment_anchor` (and any sibling first-occurrence anchor
   helper, the re-arm group's included): skip occurrences that are NOT the
   group anchor — concretely, an `establishment` occurrence with no adjacent
   `<N>th` ordinal before it, and a `re-arm` occurrence inside a
   retrospective `re-arm(ed)? at the` clause-mention (the class-accounting
   prose, never the discharge anchor). The scan tries subsequent occurrences
   until a real anchor is found; none → the existing fail-closed detail.
2. The re-arm extractor gets the same skip treatment (the 19th/20th fires):
   occurrences matching `re-arm(ed)? at the` (case-insensitive) are
   clause-mentions, never the group anchor.
3. Fixture legs (the python battery, RED-proven): for each fire's shape —
   (a) a full-entry text carrying the bare "the establishment figure ..."
   prose mention BEFORE the real group anchor (the extractor must probe the
   group anchor's ordinal/stream/figures/census, not fail); (b) a full-entry
   text carrying a retrospective "re-armed at the Nth" clause before the
   discharge anchor (the extractor must probe the discharge ordinal); (c) a
   text whose ONLY occurrences are non-anchors (the extractor still fails
   closed with the named detail — the vacuous-pass class stays closed);
   (d) the current 6-full-set's committed entries (the live-corpus smoke:
   every one-liner in the tree still extracts and compares EQUAL against its
   full entry where the guard is exercised).

## Acceptance

- `python3 -m pytest tests/` (or the repo's python battery invocation)
  green including the new fixture legs; each new leg RED-proven (fails
  against the pre-change extractor, passes after).
- The 3008-compaction reproduction: `assert_outcomes_one_liner(the 3008
  full entry, the committed 3008 one-liner)` passes MECHANICALLY (no
  by-hand adjudication) — the exact text pair that guard-fired at the
  cycle-3029 wrap.
- `cargo test --release --test todo_consistency` green.
- No behavior change for texts whose first occurrence IS the anchor (the
  existing fixtures pin that path).

## Notes

- Target files: `scripts/wrap_assert.py` + the python fixture battery that
  covers it. Doctrine files untouched.
- The adjudication convention (EQUAL BY HAND, zero-mutation, the
  observation rides) stays correct for the historical fires — this change
  closes the gap for FUTURE compactions; no committed text is edited.
