# T92 — LOOP-SPEC impl-child template `--max-iters 50→65`

check: grep -q 'max_iters:   65' LOOP-SPEC.md && ! grep -q 'max_iters:   50' LOOP-SPEC.md && grep -q '65/35 impl, 50/30 validate' LOOP-SPEC.md && cargo test --test loop_spec_recovery

## Repo context

Cycle-53 eval §2 I1 (EVALUATION.md): 4 of the last 6 glm impl children
died at 50/50 with the work done (T83 run1 → 2 resumes; T85-bundle run1;
T89 run1; T90 run1 — T84 finished at 48/50, one from death). Completed
work totals: 57/60/70/136 iterations. This is the T21 class at the new
cap (T21 went 40→50 on "3 of the last 5"). The fix is the T21/T32
one-number doctrine step: +30%, matching T21's proportional increment.

Edit surface (LOOP-SPEC.md, verified line numbers at filing):
- line ~99: the step-2 delegate-launch template `max_iters:   50` (THREE
  spaces — the step-4 validator template `max_iters: 50` at line ~229 with
  ONE space is UNCHANGED: validators finish 12–48/50).
- lines ~102–105: the explicit-budget sentence gains the new number and
  the `(50, not 40: …)` rationale parenthetical is replaced with the new
  census: `(65, not 50: 4 of the last 6 glm impl children died at 50/50
  with the work done — T83/T85/T89/T90; T84 at 48/50; minutes never
  binding, t90 used 8m24s of 35 for 60 iterations.)`
- line ~135: the T63 resume sentence `same budgets (50/35 impl, 50/30
  validate)` → `(65/35 impl, 50/30 validate)` (fix-up arcs ride the step-2
  template per doctrine; the resume keeps parity with the launch budget).
- Step 2's `max_minutes: 35` UNCHANGED (minutes were never binding).
- Pin: `tests/loop_spec_recovery.rs:141` pins `"50/35 impl, 50/30
  validate"` — update to `"65/35 impl, 50/30 validate"` and prove it RED
  by reverting the LOOP-SPEC edit. `tests/eval_digest.rs` `max_iters":50`
  occurrences are EVENT-STREAM FIXTURES, not template pins — do not touch.
- Sweep `tests/` for any other pin that breaks (`cargo test` tells you);
  update minimally and prove each RED. Do NOT touch META-SPEC.md.

## Requirements

1. The three LOOP-SPEC.md edits above, byte-exact per the surface list;
   every other line byte-identical (including the validator 50/30 and the
   anti-sprint-burn guard).
2. Pin updates minimal + each proven RED against the pre-edit LOOP-SPEC.
3. The rationale parenthetical carries the measure-clause: if >1 of the
   next 6 impl children still dies at 65/65, the next eval considers 80 or
   a work-splitting doctrine instead.

## Tests

- `cargo test --test loop_spec_recovery` green after the pin update; the
  RED proof (revert LOOP-SPEC → pin fails) recorded in the commit message.
- Full `cargo test` green — the sweep found no other casualty.

## Acceptance

- The `check:` line above passes verbatim in the worktree.
- Diff is LOOP-SPEC.md + tests/loop_spec_recovery.rs (+ any swept pin)
  ONLY.
