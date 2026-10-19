# T300 — wrap-side handoff text quotes the eval body's committed figures (copy-don't-derive)

## Repo context

The eval body's `## Handoff` section (authored at eval time per
META-META-SPEC's "Handoff section in EVALUATION.md") carries the cycle's
forward-looking committed figures: the re-arm due-label pairings
("trip-282's 68/1,168 (196th, ~300)"), the backstop ordinals, the
establishment ordinal, the trip-timing prediction. The wrap then quotes
that handoff in TWO derived surfaces: the Outcomes entry's handoff
paragraph (composed at the wrap prep) and the wrap-notes commit's
handoff clause. Today nothing forbids the wrap-side text re-deriving
those figures from memory instead of copying the eval body's committed
text — and the class has now fired TWICE, both zero-casualty only
because the machine never reads the prose:

1. FIRST sighting (named at the cycle-1332 eval §2): the cycle-1328
   wrap-notes handoff read "rides to the 253rd establishment" — the
   close ordinal mis-carried as the establishment ordinal against the
   TRUE 254th (the eval body read EXACT).
2. SECOND sighting (named at the cycle-1336 eval §2): the cycle-1332
   wrap's Outcomes-entry handoff read "reading trip-282's 80/1,223" —
   trip-264's figure paired with trip-282's label against the eval
   body's committed 68/1,168 (the figure the 196th re-arm then
   discharged EXACT against the digest).

The cycle-1332 eval's §2 watch carried the trigger: "a second variant
sighting files the doctrine sentence (the wrap-notes handoff quotes the
eval body's committed ordinal — copy, never re-derive)". This row is
that trigger's file disposition.

Doctrine home: META-META-SPEC.md's "## Handoff section in EVALUATION.md"
section (the handoff's authoring doctrine — the eval body authors the
figures once; the wrap-side quotations copy them).

## Requirements

- Add ONE doctrine paragraph to META-META-SPEC.md inside the
  "## Handoff section in EVALUATION.md" section, AFTER the existing
  "End with a recommended execution order: ..." paragraph. The
  paragraph names the discipline **copy-don't-derive** and carries,
  in substance: the eval body's handoff authors its committed figures
  ONCE (the re-arm due-label pairings, the backstop ordinals, the
  establishment ordinal, the trip-timing prediction); any wrap-side
  quotation of the handoff (the Outcomes entry's handoff paragraph,
  the wrap-notes commit's handoff clause) copies those figures
  VERBATIM from the eval body's committed text, never re-derives
  them; both sightings named as the evidence class (the 253rd-vs-254th
  ordinal mis-carry; the trip-282/80/1,223-vs-68/1,168 label-figure
  mispairing), zero-casualty because the machine never reads the
  prose; the sentence is the second-sighting trigger's file
  disposition.
- The paragraph must carry the literal token `copy-don't-derive` at
  least once (the check-line needle below).
- Add ONE pin to `tests/loop_spec_recovery.rs` (the doctrine carrier's
  pin file): the literal token `copy-don't-derive` occurs EXACTLY
  ONCE in META-META-SPEC.md (count 1; delete the paragraph → count 0
  red; duplicate it → count 2 red), with the T48 needle self-check
  idiom (the needle asserted to contain a distinguishing substring
  before the count assertion), the pin placed with the other
  META-META-SPEC needle pins, and a RED-proof run named in the commit
  message (the pin fails with the paragraph absent).
- No other repo file changes. META-META-SPEC.md stays within its
  existing section structure (no new headings; the paragraph extends
  the Handoff section's body).

## Tests

- The new pin in `tests/loop_spec_recovery.rs` (RED-proven: fails
  before the META-META-SPEC.md edit, passes after).
- The full suite stays green (`cargo nextest run --release`).

## Acceptance

- META-META-SPEC.md's Handoff section carries the copy-don't-derive
  paragraph with both sightings named.
- The pin occurs-exactly-once GREEN with the paragraph present.
- No structural drift: `## Handoff section in EVALUATION.md` heading
  byte-identical, still after `## Extend `TODO.md`` (the existing
  window pins stay green).
- estimate: ~60 changed lines all-in (the doctrine paragraph ~12
  lines + the pin ~30 lines + commit; no src changes).

check: cd "$(git rev-parse --show-toplevel)" && export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared && touch src/*.rs tests/*.rs && grep -qF "copy-don't-derive" META-META-SPEC.md && cargo test --release --test loop_spec_recovery --test todo_consistency --test internal_info_lint
