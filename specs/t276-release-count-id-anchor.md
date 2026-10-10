check: cd "$(git rev-parse --show-toplevel)" && export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-validate-a && touch src/*.rs tests/*.rs && test "$(grep -cF "sed -E 's/^todo: (file |fix )?(T[0-9]+( *[+,] *T[0-9]+)*).*/\2/'" LOOP-SPEC.md)" = 1 && test "$(grep -cF "a flip subject's tail names no uppercase T-token beyond the flipped row id" LOOP-SPEC.md)" = 1 && cargo test --release --test loop_spec_release_count_method --test todo_consistency --test eval_outcomes_carry --test eval_state_delta --test loop_spec_doctrine_prune --test loop_spec_recovery --test loop_spec_decision_records --test internal_info_lint

# T276 — release-count method id-list anchor + flip-subject hygiene (LOOP-SPEC COUNT METHOD clause)

**One concern:** anchor the T269 release-count pipeline to the flip
subject's LEADING id-list (so a stray T-token anywhere in the subject's
trailing prose never inflates the distinct-id recount), and write the
flip-subject hygiene rule that keeps stray uppercase T-tokens out of the
tail in the first place.

## Why

The T269 COUNT METHOD clause names the mechanical recount
`git log <tag>..HEAD --format='%s' | grep -E '^todo:' | grep -oE 'T[0-9]+' | sort -u | wc -l`.
The bare `grep -oE 'T[0-9]+'` extracts EVERY T-token in each flip
subject, including trailing-prose references that are not flipped rows.
Three sightings, the third CONSEQUENTIAL (census d1791632020-16, the
cycle-458 wrap): the db5870f flip subject (`todo: T274 done … T189
gates-only lane d1791627721-5 …`) rides the T189 lane-name token, so the
v0.17.13..HEAD window recount reads **3 tokens {T189, T274, T275} ≥ the
3-item tag trigger while the TRUE flip-id count is 2** — the first
consequential false crossing of the release trigger, adjudicated by hand
(TRUE governs → NO TAG). The wrap's armed letter delegates the
flip-subject-template fix to this eval (the cycle-462 eval, trip 83;
triage record rides the eval commit).

The fix has two composing halves, both verified live at filing:

1. **The pipeline anchors on the leading id-list.** The flip template
   puts the flipped row id(s) FIRST — `todo: T<a> done …`,
   `todo: T<a> + T<b> done …` (bundled, the Trivial-row rule),
   `todo: file T<a> …` / `todo: fix T<a> …` — and every stray token
   lives in the trailing prose. Extracting only the leading id-list
   before the `-oE` pass removes the class at the root:

   `git log <tag>..HEAD --format='%s' | grep -E '^todo:' | sed -E 's/^todo: (file |fix )?(T[0-9]+( *[+,] *T[0-9]+)*).*/\2/' | grep -oE 'T[0-9]+' | sort -u | wc -l`

   Live verification at filing: the v0.17.13..HEAD window reads **2**
   under the anchored pipeline vs 3 bare (the TRUE count), and the
   bundled-history window v0.17.11..v0.17.12 holds **5** both ways (the
   `T<a> + T<b>` shape still extracts every bundled id).

2. **Flip-subject hygiene.** A flip subject's tail names no uppercase
   T-token beyond the flipped row id(s): record references ride their
   d-ids (which never match `T[0-9]+`), and lane/doctrine references
   write the name ("the gates-only lane") rather than the id. The
   pipeline anchor is the mechanical guard; the hygiene is the
   authoring rule that keeps the tail clean for human readers too.

## Repo context (read these first)

- `LOOP-SPEC.md` Phase 3's release trigger — the **COUNT METHOD (T269)**
  paragraph names the pipeline verbatim (the text this item amends);
  the same phase's flip-template text (step 5's `todo:` commit rule)
  carries the subject shape.
- `tests/loop_spec_release_count_method.rs` — the T269 pin: its
  `COMMAND_SHAPE` constant pins the pipeline text verbatim and MUST
  follow the amendment (the pin re-pins the new shape; the check line
  runs the pin).
- The evidence record: d1791632020-16 (the third sighting, census 3),
  and the cycle-458 wrap notes (the adjudication: TRUE 2 < 3 → NO TAG).

## Requirements

1. LOOP-SPEC.md's COUNT METHOD paragraph: the named pipeline gains the
   id-list anchor — `| sed -E 's/^todo: (file |fix )?(T[0-9]+( *[+,] *T[0-9]+)*).*/\2/' |`
   inserted between `grep -E '^todo:'` and `grep -oE 'T[0-9]+'` — with
   one sentence naming why (the third sighting: stray T-tokens in
   flip-subject tails inflated the recount to a false 3-vs-2 trigger
   crossing, d1791632020-16; the anchor extracts only the leading
   id-list, bundled `T<a> + T<b>` shapes intact).
2. The same amendment adds the hygiene sentence, verbatim or
   near-verbatim: "a flip subject's tail names no uppercase T-token
   beyond the flipped row id(s) — record references ride their d-ids
   (which never match the extraction), and lane or doctrine references
   write the name, not the id" (the check line's second needle is the
   lead clause; keep it intact).
3. `tests/loop_spec_release_count_method.rs`: `COMMAND_SHAPE` follows
   the amended pipeline exactly (whitespace-collapsed matching per the
   pin's own convention); any pin leg quoting the old bare pipeline is
   updated to quote the new shape.
4. Diff touches ONLY LOOP-SPEC.md + tests/loop_spec_release_count_method.rs.
   No other doctrine paragraph changes; no re-wrapping of surrounding
   text (pinned tokens in the region must survive — the pin floor runs).

## Tests

- Both needles exactly once in LOOP-SPEC.md (the check line enforces).
- `cargo test --release --test loop_spec_release_count_method` green
  (the re-pinned shape); the 7-suite pin floor green (the check line
  runs all 8 suites).
- Review/validator mutation legs: drop the sed step from the doctrine
  text → the first needle grep AND the pin's shape assertion both fail;
  corrupt the id-list group (`(T[0-9]+( *[+,] *T[0-9]+)*)` → `(T[0-9]+)`)
  → both fail.
- Live RED-proof (review evidence, window-dependent, not a pinned test):
  on the v0.17.13..HEAD window the bare pipeline reads 3 while the
  anchored pipeline reads 2 (the TRUE flip count); on the bundled
  v0.17.11..v0.17.12 window both read 5.

## Acceptance

- Needles exactly once; `loop_spec_release_count_method` + the 7-suite
  pin floor green; the live-window reads reproduced at review.
- Diff touches only the two named files, ≤ ~15 changed lines.
- kimi adversarial validation REQUIRED (LOOP-SPEC.md is the loop/spec
  doctrine itself — the core-list leg makes the T189 lane ineligible).

## Estimate

- estimate: ~15 changed lines (one pipeline substitution + one hygiene
  sentence + one pinned constant + its comment), doctrine + one pin
  file (LOOP-SPEC.md + tests/loop_spec_release_count_method.rs).
