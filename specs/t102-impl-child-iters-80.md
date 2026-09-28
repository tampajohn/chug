# T102 — LOOP-SPEC impl-child template `--max-iters 65→80`

check: grep -q 'max_iters:   80' LOOP-SPEC.md && ! grep -q 'max_iters:   65' LOOP-SPEC.md && grep -q '80/35 impl, 50/30 validate' LOOP-SPEC.md && cargo test --test loop_spec_recovery

## Repo context

Cycle-58 eval §2 I1 (EVALUATION.md): T92's written measure clause FIRED.
Post-T92 impl-class children (launch order, digest-verified): T93 23/65,
T91-run1 **65/65 DIED**, T94 17/65, T98 38/65, T95 26/65, T96 23/65,
T97 37/65, T99-run1 **65/65 DIED**, T99-fixup 28/65, T100-run1 **65/65
DIED**, T100-fixup 41/65, T101 53/65 — 3 of 12 died at the ceiling, and
any rolling 6-window over children 5–10 (T95…T100-run1) holds 2 deaths,
tripping the clause's `>1 of the next 6` trigger. All three deaths are
the big-feature class with the work DONE (T91 +1012 lines / 84 total
iters, T99 +816/−10 / 93, T100 +2008/−12 / 67); every one was recovered
by a T63 resume in minutes (21/21 all-time). This is the T21/T92 class
at the 65 cap. The fix is the same one-number doctrine step: +23%
(65→80), the T21 proportional increment (+30% would be 84; 80 is the
round step). Honest sizing note carried from the eval: 2 of the 3 deaths
(84, 93) exceed 80 too — this step buys the 65–80 band (T100's 67
finishes first-try); the >80 band stays resume-served. Minutes are never
binding (t90 used 8m24s of 35 for 60 iterations; the 35-min ceiling and
the token budget remain the wedge guards).

Edit surface (LOOP-SPEC.md, verified line numbers at filing):
- line 99: the step-2 delegate-launch template `max_iters:   65` (THREE
  spaces — the step-4 validator template `max_iters: 50` with ONE space
  is UNCHANGED: validators finish 16–48/50, zero validator deaths).
- lines 102–105: the explicit-budget sentence gains the new number and
  the `(65, not 50: …)` rationale parenthetical is replaced with the new
  census AND the new measure clause (req 3).
- line 138: the T63 resume sentence `same budgets (65/35 impl, 50/30
  validate)` → `(80/35 impl, 50/30 validate)` (resume keeps parity with
  the launch budget; fix-up arcs ride the step-2 template per doctrine).
- Step 2's `max_minutes: 35` UNCHANGED (minutes never binding).
- Pin: `tests/loop_spec_recovery.rs:141` pins `"65/35 impl, 50/30
  validate"` — update to `"80/35 impl, 50/30 validate"` and prove it RED
  by reverting the LOOP-SPEC edit. `tests/eval_digest.rs` `max_iters":50`
  occurrences are EVENT-STREAM FIXTURES — do not touch.
- Sweep `tests/` for any other pin that breaks (`cargo test` tells you);
  update minimally and prove each RED. Do NOT touch META-SPEC.md.

## Requirements

1. The three LOOP-SPEC.md edits above, byte-exact per the surface list;
   every other line byte-identical (validator 50/30, anti-sprint-burn
   guard, T63 resume mechanics otherwise untouched).
2. Pin updates minimal + each proven RED against the pre-edit LOOP-SPEC.
3. The replacement rationale parenthetical carries the new census (3 of
   12 post-T92 impl children died at 65/65 with the work done —
   T91/T99/T100 run1s, totals 84/93/67; T63 resumes 21/21) AND the new
   measure clause: **if >1 of the next 6 impl children still dies at
   80/80 with the work done, the next eval considers a spec-size cap (a
   ~500-line estimate ceiling that forces a split) instead of further
   iteration raises.**

## Tests

- `cargo test --test loop_spec_recovery` green after the pin update; the
  RED proof (revert LOOP-SPEC → pin fails) recorded in the commit message.
- Full `cargo test` green — the sweep found no other casualty.

## Acceptance

- The `check:` line above passes verbatim in the worktree.
- Diff is LOOP-SPEC.md + tests/loop_spec_recovery.rs (+ any swept pin)
  ONLY.
