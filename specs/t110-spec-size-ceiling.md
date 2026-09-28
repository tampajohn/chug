# T110 — filing-time spec-size estimate ceiling (~500 lines); T102 measure clause resolved

check: grep -q 'estimate: ~N changed lines' META-META-SPEC.md && grep -q '~500 lines of new/modified logic' META-META-SPEC.md && grep -q 'filing-time ~500-line estimate ceiling' LOOP-SPEC.md && grep -q '~500-line estimate ceiling' FEATURES.md && cargo test --test loop_spec_recovery

## Repo context

T102 (cycle 58) raised the impl-child iteration budget 65→80 with a written
measure clause (LOOP-SPEC.md step 2, the `(80, not 65: ...)` parenthetical):
"if >1 of the next 6 impl children still dies at 80/80 with the work done,
the next eval considers a spec-size cap (a ~500-line estimate ceiling that
forces a split) instead of further iteration raises."

The clause TRIPPED in cycle 59 (wrap commit 67d1f5f, declared in-cycle):
2 of the last 4 impl children died at 80/80 —

- **t108-impl** (glm): abort event `iteration budget exceeded` at
  2026-09-28T06:40:44Z, died mid-implementation; T63 resume needed **61/80**
  more iterations to finish (141 total for one item — the most expensive
  child arc of the era). Evidence: `.chug/events-t108-impl-20260928-065545.jsonl`.
- **t108-fixup** (glm): died 80/80 post-commit (work done, goal gate not
  run); T63 resume accepted in 1/80. Evidence:
  `.chug/events-t108-fixup-20260928-073941.jsonl`.

Both deaths were on T108, the one row the cycle-59 eval sized at "~700–900
lines with tests (T91-class)" — and that sizing lived in the EVALUATION.md
prose, NOT in the spec, and no doctrine forced a split. The other six
post-raise impl children (61/75/43/10/31/47) all finished first-try. The
lesson is not "80 is too small" — it is "a ~700–900-line row is too big for
any single-child budget, and filing time is where that must be decided."

This is a DOCTRINE row (LOOP-SPEC.md + META-META-SPEC.md + FEATURES.md) —
it runs ALONE (no overlap), kimi validation REQUIRED.

## Requirements

1. **META-META-SPEC.md** — the spec quality bar paragraph in "Extend
   TODO.md" gains the estimate-and-ceiling rule, verbatim needles:
   - "Every spec carries an `estimate: ~N changed lines` line in its
     repo-context section"
   - "a row estimated above ~500 lines of new/modified logic MUST be split
     into 2–3 rows at filing time"
   - the mechanical-move exemption: "mechanical byte-identical move rows
     (the T104/T109 class) are exempt — the estimate line says so"
   (the exemption is the T104/T109 lesson: a 3,100-line byte-identical test
   move lands first-try at 75/80 — moved lines are not novel-logic lines).
2. **LOOP-SPEC.md** — the step-2 `(80, not 65: ...)` parenthetical keeps
   its history and gains the resolution, verbatim needle:
   "Measure clause RESOLVED at the cycle-60 eval (T110): the census tripped
   (2 of the last 4 impl children died 80/80 — t108-impl mid-impl at 141
   total iterations, t108-fixup post-commit); the remedy is the filing-time
   ~500-line estimate ceiling in META-META-SPEC's spec quality bar, not
   further iteration raises."
3. **FEATURES.md** — the working-rules shrink line's stale "50-iter child
   budget" reference is replaced by the ceiling, verbatim needle:
   "~500-line estimate ceiling" (note the staleness: the 50-iter reference
   predates the T21/T92/T102 raises to 80).
4. **Pin**: `tests/loop_spec_recovery.rs` (the step-2-template carrier)
   pins the LOOP-SPEC needle `filing-time ~500-line estimate ceiling` —
   RED-proven (pin fails before the LOOP-SPEC edit, passes after).
   META-META-SPEC.md and FEATURES.md carry no pin carrier today (most
   META-META/FEATURES text is unpinned); their needles are verified by the
   `check:` line above.
5. No other LOOP-SPEC text changes; the 80/35 impl budgets and 50/30
   validation budgets are unchanged.

## Tests

- The new pin leg in `tests/loop_spec_recovery.rs`: RED before the
  LOOP-SPEC edit (needle absent), GREEN after. Run
  `cargo test --test loop_spec_recovery` both ways and record the RED leg
  in the commit message.
- The `check:` line's grep needles verify the META-META/FEATURES edits.

## Acceptance

- All three doctrine files carry their needles verbatim; the pin leg is
  RED-proven; `check:` green in the worktree.
- The TODO.md row flip references this spec's commit.

estimate: ~80 changed lines (doctrine text + one pin leg; no code)
