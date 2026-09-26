# T64 — Pin the two validator survivor classes (t57 ALWAYS-form, t58 over-reset)

One concern: two adversarial-validation rounds this cycle-triple each left
exactly one non-blocking weak-test survivor; the standing survivor-class to
pin pipeline (T54, T62 precedents) closes them with tests-only pins.

## Repo context

- **Survivor (a) — t57-validate M5** (`.chug/LEDGER-t57-validate-20260926-100640.md`):
  "step-5 rule's ALWAYS-form unpinned — window assertion satisfied by the
  mechanism sentence's 'ALWAYS main checkouts'." LOOP-SPEC §2 step 5's rule
  that post-merge gates re-run in main under
  `CARGO_TARGET_DIR=.../target-shared-main` is an ALWAYS rule ("ALWAYS, never
  conditionally on the T44 overlap" — distinct from step 3's conditional
  role-keyed split); a mutant rewording/dropping the ALWAYS-form survives
  because `tests/shared_target_dir.rs`'s current step-5 pins are satisfied by
  the mechanism sentence instead.
- **Survivor (b) — t58-validate** (`.chug/LEDGER-t58-validate-20260926-064150.md`):
  "over-reset of max_iters/last_iteration at run_start (weak test on spec
  req 4 sentence 2)". T58 made `summarize_events` (src/tools.rs, near :975)
  reset the verdict latches (goal/abort/budget_low + abort_reason) at each
  new `run_start` while `max_iters`/`last_iteration` KEEP LAST-SEEN across
  the boundary (so a resumed child's status in the gap between the new
  segment's `run_start` and its first iteration event still reports
  meaningful numbers). The two-segment test
  (`delegate_summary_budget_low_in_first_segment_only_resets_on_resume`,
  src/tools.rs:2110) covers the latch reset but not the keep-last-seen
  fields, so a mutant that over-resets them survives.
- Both validators VERDICT: PASS (survivors non-blocking, code spec-correct);
  this row is pin-strengthening only — zero production behavior change.

## Requirements

1. **Pin (a)** — `tests/shared_target_dir.rs` gains a pin that kills the
   named M5 mutant: assert LOOP-SPEC step 5's ALWAYS-form explicitly — the
   step-5 window (between step 5's heading and step 6's) contains both
   `target-shared-main` and the ALWAYS language ("ALWAYS, never
   conditionally"), exact-count per the T47-carrier doctrine (the
   ALWAYS-form carrier for the main-gates dir occurs exactly once).
   Non-vacuousness: reword step 5's ALWAYS-form (the validator's M5 shape)
   then the new pin must go RED; revert then green.
2. **Pin (b)** — src/tools.rs test module gains a two-segment
   `summarize_events` test, following the synthesis pattern of
   `delegate_summary_budget_low_in_first_segment_only_resets_on_resume`
   (src/tools.rs:2110): segment 1 ends aborted at max_iters N /
   last_iteration N; segment 2 begins with a `run_start`. Assert: in the gap
   window (after segment 2's `run_start`, before its first iteration event)
   the summary still reports the last-seen max_iters/last_iteration (NOT
   null/0) while the state/abort latches describe segment 2; and after
   segment 2's first iteration event the fields reflect segment 2.
   Non-vacuousness: the over-reset mutant (reset max_iters/last_iteration at
   `run_start`) must go RED; revert then green.
3. Tests-only: no production code, no doctrine text edits (the pins assert
   today's true text/behavior). Non-vacuousness evidence for BOTH pins in
   the commit message (mutant goes red, revert goes green), per the T54/T62
   pattern.

## Tests

The two new pins ARE the tests. Full suite + clippy green.

## Acceptance

- Both named mutants die on the new pins (evidence in commit message).
- All pre-existing pins untouched and green.
- `cargo test` + `cargo clippy --all-targets -- -D warnings` green.
- Adversarial validation OPTIONAL (tests-only — T16/T31/T59/T62 precedent);
  orchestrator gates + the non-vacuousness evidence suffice.

check: cargo test --test shared_target_dir && cargo test --bin chug
