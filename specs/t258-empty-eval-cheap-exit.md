# T258 — empty-eval cheap exit: mechanical short-circuit + glm non-trip evals

check: cd "$(git rev-parse --show-toplevel)" && export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared && touch src/*.rs tests/*.rs && cargo test --release

estimate: ~250 lines (predicate + loopd gate + doctrine + pins)

## Concern

Operator 2026-10-07: "why do we need 20-30 kimi iterations to say
nothing to do?" Measured: every cycle — including non-trip empty
dispositions — runs a full kimi stream (20-30 iterations, ~134k fresh
input + ~1.7M cache-read per call, ≈$1/iteration ≈ $20-30/cycle, ≈
$800-1200/day at the T237 backoff cap). The breakdown (cycle-20261007-
161044): ~5 iterations are the actual Phase-1 disposition predicate;
the rest is doctrine bookkeeping (todo hygiene, triage records,
Outcomes, gates) on an empty delta. The doctrine has no cheap-exit
path: the T247 valve decides WHEN to eval, but every eval runs the
full META-META checklist regardless.

## Repo context

- T237 (backoff) spaces empty cycles to the 1800s cap; T247 (valve)
  trips a real eval on the 4th consecutive. Neither shrinks the
  per-cycle stream.
- The disposition predicate is already mechanical: no new TODO rows,
  no child deaths, no spec/source changes since the last eval,
  EVALUATION.md fresh. loopd computes todo_rows and eval_fresh for
  routing today — the inputs exist without an LLM.
- glm-5.3-flash is ~25x cheaper than kimi ($0.15/$0.50 vs $3/$15).

## Requirements

1. Cheap exit: when the mechanical predicate is empty (no new rows,
   no deaths, no source/spec/doctrine changes since the last eval,
   EVALUATION.md fresh same UTC day), loopd writes the one-line
   empty-delta disposition ITSELF and skips the chug eval launch
   entirely — no LLM call. The valve (4th consecutive) still trips a
   real eval, and any non-empty predicate input forces one.
2. Non-trip evals on glm: when an eval cycle DOES launch but the
   predicate was borderline (non-empty but routine), route glm, not
   kimi. Kimi stays for valve trips, work cycles, and validators
   (family-independence rule unchanged).
3. The one-line disposition records the predicate inputs (row count,
   last-change hash, freshness) in loopd.log — the streak math (T237)
   and valve (T247) read the same inputs, never the LLM's say-so.
4. Doctrine: LOOP-SPEC Phase-1 gets the cheap-exit rule verbatim
   (predicate listed, who may write the disposition); META-META-SPEC
   notes the full eval checklist applies only when the predicate is
   non-empty or the valve trips.
5. Pins (loop_spec_*): cheap exit skips the launch when inputs are
   empty; any single non-empty input forces the launch; the
   disposition line carries the inputs.

## Tests

- The three pins above; existing T237/T247 pacing pins stay green.
- Acceptance: an empty day costs <= ~$50 (mostly valve trips) — the
  cycle-116-class full evals become rare, recorded in Outcomes.

## Out of scope

- Shrinking REAL evals (work deltas keep the full checklist);
  changing the valve trip count; cache-size reduction (T184's domain).
