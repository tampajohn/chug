# T301 — name the supervisor's T258 delta-anchor in LOOP-SPEC (the prediction-miss 3rd-sighting discharge)

## Repo context

The wrap-handoff prediction-miss class — "handoff prose asserting supervisor
mechanics contrary to the anchor rule" — fired its THIRD sighting at the
cycle-1340 trip-301 eval: the cycle-1336 wrap's handoff predicted "the chain
rule REFUSES the skip next cycle (a worked delta forces the real eval) →
1337 = the worked-delta real eval, kimi routes", and the supervisor
cheap-exited cycle 1337 anyway (disposition 17c044c, inputs rows=0 new=0
deaths=0 bookkeeping=yes fresh=yes), because its delta window anchors at the
newest EVALUATION.md-touching commit (`git log -1 -- EVALUATION.md`, the
`last_eval_commit()` in loopd.sh ~line 564) — the wrap prep c63d0ff — and
the T300 arc (merge ef89bad + flip 740424a) landed BEHIND that anchor, inside
the filing cycle. Prior sightings: cycle-458 (d1791630100-4, first-sighting
absorb, the armed trigger: "a SECOND instance files the counted census and
weighs a handoff-text rule") and ~cycle-1046 (the second sighting, census
counted, the rule never written). The class is zero-casualty every time (the
machine never reads the prose; the valve trips on the machine streak
regardless) but it re-fires on a known, named root cause: LOOP-SPEC's T258
paragraph says "since the last evaluation" without naming what the baseline
IS mechanically, so every handoff author predicts from the chain rule's
letter instead of the supervisor's anchor.

Fix: write the anchor rule into the T258 cheap-exit paragraph (one
sentence-pair) and pin it, so the mechanical behavior is load-bearing
doctrine and future handoff predictions compose from the anchor.

estimate: ~10 changed lines (LOOP-SPEC.md +~5 — the sentence-pair;
tests/loop_spec_cheap_exit.rs +~3 — one needle + comment; this spec).

check: cargo test --test loop_spec_cheap_exit --test todo_consistency --test internal_info_lint

## Pin-surface declaration (T285 — probed before filing)

- `tests/loop_spec_cheap_exit.rs::loop_spec_phase_1_carries_the_cheap_exit_clause`
  walks 12 needles with `count_eq(&spec, needle, 1, …)` over
  `flat(LOOP-SPEC.md)`. The mandated sentence-pair is INSERT-ONLY and
  reproduces none of the 12 needles verbatim (probed: none of its clauses
  match any needle), so every existing count stays 1. The edit ADDS one
  needle to that list — `"the baseline is the newest EVALUATION.md-touching commit"`
  (count 1) — guarding the new doctrine from silent rot.
- The ordering pin `the_cheap_exit_clause_sits_between_the_chain_rule_and_the_commit_artifacts`
  (same file) is untouched: the insertion sits INSIDE the paragraph.
- The RAW T237 count pin (`count_eq(&read("LOOP-SPEC.md"), "empty-delta disposition", 1, …)`):
  the mandated text writes around the contiguous token — it never contains
  the literal (quote discipline T248).
- Collision probes run at filing: no test file pins `"worked-delta"` (0
  hits), `"the last examination"` (0 hits), or a bare
  `"since the last evaluation"` (0 hits); `EVALUATION.md-touching` appears
  only in an eval_state_delta.rs comment; the `anchor` needles in
  loop_spec_cheap_exit.rs are fixture-text asserts, not LOOP-SPEC pins.
- The placement pins in tests/loop_spec_recovery.rs are unaffected (no
  edit to any pinned region they name).

## Requirements

1. In LOOP-SPEC.md's Phase-1 cheap-exit paragraph ("**The cheap exit
   (T258).**"), insert the following sentence-pair immediately AFTER the
   sentence ending "…and EVALUATION.md fresh (same UTC day)." and BEFORE
   the sentence beginning "When all four hold and the valve has not
   tripped", preserving the paragraph's bold-lead structure and reflowing
   line wraps to match the file's ~72-column style:

   "The window's baseline is mechanical: the newest EVALUATION.md-touching
   commit (`git log -1 -- EVALUATION.md` — the last examination of loop
   state, a real eval or a wrap's bookkeeping alike), so work landed and
   accounted inside the filing cycle (an arc closed into the wrap prep's
   Outcomes) sits BEHIND the baseline and never forces a worked-delta
   launch. Handoff prose that predicts supervisor behavior reads this
   anchor's mechanics, never the chain rule's letter — the wrap-handoff
   prediction-miss class (cycles 458, ~1046, 1340) is the evidence."

   (The impl MAY adjust the exact wording for flow, but the needle
   "the baseline is the newest EVALUATION.md-touching commit" MUST appear
   verbatim, exactly once, and the anchor's mechanism — the
   `git log -1 -- EVALUATION.md` baseline, the in-cycle arc sitting behind
   it, the handoff-prediction discipline — MUST survive intact.)
2. In `tests/loop_spec_cheap_exit.rs`, add the needle
   `"the baseline is the newest EVALUATION.md-touching commit"` to the
   `loop_spec_phase_1_carries_the_cheap_exit_clause` needle list with a
   one-line comment naming T301, keeping every existing needle's count at
   exactly 1.
3. No other repo file changes. Never reword any existing pinned needle.

## Tests

- The added needle lands in the existing pin (requirement 2) — the pin
  itself is the test: it fails if the doctrine sentence is dropped or
  reworded off the needle.
- RED-proof: with the LOOP-SPEC edit absent, the new needle count is 0
  and `cargo test --test loop_spec_cheap_exit
  loop_spec_phase_1_carries_the_cheap_exit_clause` fails; with the edit
  present it passes. (The orchestrator runs this left/right proof at
  review.)
- The full check: line above (three integration targets) green.

## Acceptance

- LOOP-SPEC.md carries the anchor sentence-pair inside the T258 paragraph,
  needle verbatim and exactly-once; the T237 contiguous token still
  exactly once RAW.
- `tests/loop_spec_cheap_exit.rs` carries the new needle; all 13 needles
  count 1.
- `cargo test --test loop_spec_cheap_exit --test todo_consistency --test internal_info_lint`
  green in the worktree.
- Validation: kimi REQUIRED (LOOP-SPEC.md is the loop doctrine itself —
  the doctrine-carrier rule; lane inputs (b) ≤150 lines ✓, (c) no new
  surface ✓, (d) no CI/check-line change ✓ are outweighed by (a)).
