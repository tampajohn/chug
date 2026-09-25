# T18 — Widen the budget-low iteration warning margin (WARN_REMAINING_ITERS 5 → 8)

check: cd /Users/jadams/workspace/chug && cargo test

## Why (evidence)

Cycle-4 evaluation watch item **J6 fired**: three of the last four child
runs died at the iteration ceiling with work complete but the wrap
unfinished (the J1 pattern, post-T13):

1. T15 glm implementation child (cycle 3): abort 40/40, implementation
   complete, **uncommitted** (harvested; `events-t15-impl-20260922-061957.jsonl`).
2. T17 glm implementation child (cycle 4): abort 40/40, diff complete +
   green, **uncommitted** (harvested as `0abc5de`;
   `events-t17-impl-20260925-170831.jsonl`). At iteration 34 it was still
   recovering from self-clobbered concurrent edits — the warning at 35
   left zero hiccup margin.
3. T17 kimi validation child (cycle 4): abort 40/40, review done, mutation
   matrix nearly done, **verdict unwritten** — it acknowledged the warning
   at iteration 36 and compressed, still ~3 turns short
   (`events-t17-validate-20260925-171639.jsonl`).

The wrap sequence (final gates 2–4 iterations + commit 1 +
verdict/goal_complete 1) needs 4–6 iterations; a warning that fires with
only 5 remaining leaves no margin for a slow test run or a self-inflicted
rework loop. The surviving validator this cycle finished at 17/20 only
because its goal carried an explicit hard schedule — doctrine can't rely
on every goal author remembering that.

## Repo context

- `src/driver.rs:40` — `const WARN_REMAINING_ITERS: u32 = 5;` (the only
  place the threshold lives; `budget_low_notice` at `:790+` and the latch
  at `:531` consume the constant).
- Existing pins that must flip (audit, don't blindly trust this list —
  grep for `WARN_REMAINING_ITERS`, `iteration(s)`, and
  `budget_low_notice(` across `src/`):
  - `:1893-1894` boundary unit pins: `(6, …) == None`, `(5, …) .is_some()`
    → become `(9, …) == None`, `(8, …) .is_some()`.
  - `:1933` doc comment, `:1972` / `:1978` scripted-loop pins ("5
    iteration(s)", "first seen on the 4th call"): with `max_iters=8` and
    WARN=8 the notice fires on the **1st** call with `remaining == 7` →
    "7 iteration(s)".
  - `:2029` T17 fire-time pin `remaining == 5` → `== 7` (same scripted
    run shape).
  - Any other test asserting an `iteration(s)` message literal whose
    counts derive from the threshold (`:1923`, `:1928`, `:2143` at time of
    writing — check whether each is iteration-leg or time/token-leg
    before touching it; time/token legs are unchanged).
- `README.md:61` — "when ≤5 iterations, …" → "≤8 iterations".
- Time (`WARN_REMAINING_SECS`) and token (`WARN_REMAINING_TOKENS`)
  thresholds are **unchanged**; warning text, one-shot latch semantics,
  and the abort path are byte-identical — this row changes ONE number and
  its pins.

## Requirements

1. `WARN_REMAINING_ITERS` becomes `8`; its doc comment (`:31-39` area)
   gains one line noting the J6 evidence (3 post-T13 child wrap deaths;
   wrap needs 4–6 iterations of runway).
2. Every pin that encodes the old threshold is updated to the new reality
   (boundary tests, scripted-loop fire-time counts, doc comments); no
   test may be weakened to make the change pass — counts move because the
   fire time moved, assertions stay exact.
3. README budget-low bullet updated.
4. No other behavior change: thresholds for minutes/tokens, the message
   bytes, latch semantics, and the abort path stay identical.

## Tests

- Updated boundary pins: `budget_low_notice(9, u64::MAX, None, …)` is
  `None`, `(8, …)` is `Some`.
- The scripted-loop warning test asserts the new fire-time counts exactly
  (first-seen call + "7 iteration(s)").
- T17's `budget_low_injection_lands_in_events_jsonl` fire-time pin updated
  (`remaining == 7`); the control test is untouched and must still pass.
- Non-vacuousness: reverting the constant to 5 must fail the new boundary
  pins (the implementer demonstrates this once during the round).

## Acceptance

- `cargo build`, `cargo clippy --all-targets -- -D warnings`, `cargo test`
  all green.
- Adversarial validation (REQUIRED — touches `src/driver.rs`): confirm
  the threshold moved exactly 5→8, no pin was weakened, and the
  revert-mutant (constant back to 5) dies.
