# T198 — loopd cycle budget --max-minutes 240→360 (the fleet outgrew the wall)

check: export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared && cargo test --test loopd_model_routing --test loopd_reexec && grep -qF -- "--max-minutes 360" loopd.sh && ! grep -qF -- "--max-minutes 240" loopd.sh

## Repo context

loopd.sh launches each cycle with `--max-iters 200 --max-minutes 240`
(the minutes figure is untouched since the supervisor's first form;
iterations took the T27/T36 raises, 80→120→160→200). The cycle-85–90
delta measured the wall: cycle 85 3h55m, cycle 86 3h23m, cycle 87
3h56m, **cycle 88 DIED at the 240-minute wall mid-arc** (six items
landed across its two segments, wrap finished by cycle 89), cycle 89
2h47m, cycle 90 5m42s. Three of six cycles ran within 5 minutes of
the cap and one hit it — the wall is now the binding budget on any
≥5-item cycle, and T194's 3-child fleet lands MORE items per cycle,
not fewer. The death is absorbed (split-arc continuation works —
cycle 89 reconciled cleanly) but costs a wrap + a cold restart +
reconciliation bookkeeping per occurrence.

Arithmetic for 360: p95 observed cycle wall ≈ 4h (cycle 88's death);
4h × 1.5 headroom = 6h = 360. Iterations stay 200 (cycle 89 used
195/200 — close, but iteration raises are a separate lever with a
separate measure history; this row moves minutes only). Bounded
blast-radius reasoning is unchanged from T27: loopd relaunches a
dead cycle, and the 3-consecutive-failures HALT still caps a wedged
fleet.

This row is DOCTRINE (loopd.sh) → SOLO: no other child in flight
while it runs.

estimate: ~15 changed lines (one number + the comment arithmetic +
one in-commit pin amendment).

## Requirements

1. loopd.sh's cycle launch line: `--max-minutes 240` →
   `--max-minutes 360`, and the adjacent comment gains the arithmetic
   (cycle-85–90 walls 3h23m–4h01m, cap hit once, 360 = p95 × 1.5;
   iterations stay 200).
2. **Pin amended in-commit:** `tests/loopd_model_routing.rs:74` pins
   the exact launch argv fragment
   `--model "$orch_model" --max-iters 200 --max-minutes 240` — amend
   to the 360 form, the amendment justified in the commit message
   (the T187 pin-follows-carrier pattern).
3. Sweep `grep -rn "max-minutes 240" loopd.sh tests/ docs/ README.md
   runbooks/ 2>/dev/null` — any other carrier of the 240 figure is
   amended in the same commit or named NOT-a-carrier in the commit
   message. (README's Continuous-mode section names no number today —
   keep it that way.)
4. Nothing else in loopd.sh changes.

## Tests

- `cargo test --test loopd_model_routing --test loopd_reexec` green
  with the amended pin.
- The spec check's grep legs.

## Acceptance

- Check line green; clippy green (trivially).
- Diff confined to loopd.sh + tests/loopd_model_routing.rs.

## Out of scope

- Iteration-budget raises (200 stays; cycle 89's 195/200 is a watch
  item for the next eval, not this row).
- Per-phase minute budgets inside the cycle.
