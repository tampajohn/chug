# T173 — LOOP-SPEC child budgets: impl max_minutes 35→50, validator 40→50

check: export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared && cargo test --test loop_spec_recovery

## Repo context

LOOP-SPEC.md §2 step 2 launches impl children with `max_minutes: 35`
(delegate's default) and step 4 launches validators with `max_minutes: 40`.
The cycle-79 eval (EVALUATION.md §2.1) found minutes are now the BINDING
child budget: 5 of 7 impl children in the cycles-76–78 delta died at the
35-minute wall with iterations to spare (t162 54/80, t163 46/80, t164
50/80 uncommitted→resume, t165 65/80, t167 47/80) and 3 of 5 validators
finished within ~1–4.5 min of the 40-min wall (t167-val 39m16s, t168-val
38m22s, t162-val 35m46s). glm throughput is ~1.3–1.9 iters/min, so 80
iters needs 42–62 min; kimi validators need ~45–50 min for a full 60-iter
run. The pins live in tests/loop_spec_recovery.rs — its header comment
(:124-139) and needles `80/35 impl, 60/40 validate` (:286),
STEP4_BUDGET_NUMBERS "`max_iters: 60`, `max_minutes: 40`" (:1103), and the
measure-clause window (:1132-1198) pin the CURRENT numbers and must move
with the doctrine. META-SPEC.md is NOT edited (the T21/T24 precedent:
LOOP-SPEC overrides §6 launch mechanics).

estimate: ~120 changed lines (LOOP-SPEC.md ~4 hunks + measure-clause
re-key ~40; pin-file needle + comment updates ~80 — the cycle-79 eval's
sentence+pin-file calibration).

## Requirements

1. LOOP-SPEC.md §2 step 2: the delegate template's `max_minutes: 35`
   becomes `max_minutes: 50`, and the following parenthetical
   ("`max_iters: 80` and `max_minutes: 35` are explicit — delegate's
   defaults are 40/35 …") is reworded to name 50 (delegate's defaults
   stay 40/35 — the template overrides minutes explicitly).
2. The T63 budget-death-recovery paragraph: "same budgets (80/35 impl,
   60/40 validate)" becomes "(80/50 impl, 60/50 validate)".
3. §2 step 4: "`max_iters: 60`, `max_minutes: 40` (§6's budgets …)" —
   minutes becomes 50; the parenthetical "minutes is 40, still ABOVE
   delegate's 35 default" is reworded (50, above delegate's 35 default);
   the census measure clause is re-keyed: "if >1 of the next 8 validator
   runs still dies at 60/40 with the verdict unannounced …" → "… at
   60/50 …", with the evidence sentence (4-of-4 deaths in cycles 66–69)
   extended by the cycle-79 evidence (3 of 5 validators in cycles 76–78
   finished within ~1–4.5 min of the 40-min wall — the raise's reason).
4. Add the impl-side measure sentence to step 2 (the T21-class pattern):
   the cycles-76–78 census (5 of 7 impl children died at 35 min, 4
   committed + 1 resume) is the raise's evidence; if >2 of the next 8
   impl children still die at the 50-minute budget with the goal
   unaccepted, the next eval considers spec-size discipline instead of
   further raises.
5. tests/loop_spec_recovery.rs: every needle/comment pinning 35/40
   budgets moves to the new numbers IN THE SAME COMMIT — the pin file's
   own header census (:124-139) is updated to describe this change
   (50-minute raise, both roles) so the comment stays truthful.
6. Nothing else changes: max_iters stay 80/60; delegate's defaults stay
   40/35; META-SPEC.md, SELF-SPEC.md, loopd.sh untouched.

## Tests

- `cargo test --test loop_spec_recovery` green with the UPDATED needles
  (the pin file is the check).
- RED-proof: reverting any one of the three LOOP-SPEC surfaces (step-2
  template, T63 echo, step-4 numbers) while keeping the updated pins
  fails the pin file — state the proof in the commit message.

## Acceptance

- Spec check green; full `cargo test` green; clippy `-D` clean.
- `grep -n "80/35\|60/40\|max_minutes: 35\|max_minutes: 40" LOOP-SPEC.md`
  returns nothing; the new "80/50 impl, 60/50 validate" and step-4 50
  appear exactly once each.

## Out of scope

- Raising max_iters, delegate's built-in defaults, loopd's orchestrator
  budgets (200/240), META-SPEC.md edits, the T178 bash-timeout row.
