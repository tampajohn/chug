# T252 — load-fence composition test: guard the double-read race (goal-gate flake)

check: cd "$(git rev-parse --show-toplevel)" && export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-main && touch src/*.rs tests/*.rs && cargo test --release --test loopd_daemon_ensure

## Repo context

Cycle-130 wrap close: the goal gate (full nextest --release at 9fcc036,
cold recompile + 42 parallel test binaries starting up) FAILED
`chug::loopd_daemon_ensure testsupport::t214_live_deadline_stays_within_base_and_4x`
with `left: 98.95s, right: 94.6s` — a 4.6% divergence. The test
(src/testsupport.rs) computes `load_scaled_deadline(base)` (which reads the
1-minute load average INTERNALLY) and then reads the load average AGAIN to
build the expected pure composition, asserting equality. When the load
average updates between the two reads — a busy host crossing a loadavg
update boundary — the equality false-reds though the fence is correct.
Confirmed a flake, not a regression: 20/20 + 30/30 solo/loaded runs green;
T251 (the cycle's only landed item) touched LOOP-SPEC.md + one test file
only and cannot affect the fence.

estimate: ~15 lines (one test restructured + comment)

Orchestrator-direct (tests-only, one function, no core-list file): T189
gates-only lane per the mechanical predicate.

## Requirements

1. In `t214_live_deadline_stays_within_base_and_4x`, bracket the live call
   with two load reads (`before`/`after`) and assert the composition
   equality ONLY when `before == after` — the seam was stable across the
   live call (the near-universal case: loadavg quantizes and updates on a
   multi-second cadence, while the bracket reads are microseconds apart),
   so the pin stays load-bearing in the steady state and never false-reds
   at an update boundary. The `[base, 4x base]` invariant legs are
   unchanged — they carry the run under churn.
2. Comment names the evidence (the cycle-130 goal-gate fire, left/right
   values) and the non-vacuity argument (the equality leg fires whenever
   the seam is stable — the near-universal path).
3. RED-prove the equality leg still has teeth post-fix: temporarily break
   the expected value (e.g. scale it), watch the test fail SOLO, restore
   byte-identical.
4. Scope: src/testsupport.rs ONLY. `load_scaled_deadline`'s signature and
   the loopd fence seams untouched.

## Acceptance

- The targeted test green solo (repeatedly) and the full
  loopd_daemon_ensure binary green.
- RED-proof executed (step 3) with restore verified.
- Full nextest --release green in target-shared-main.
- `git diff --name-only` shows exactly src/testsupport.rs (plus this
  cycle's bookkeeping: TODO.md row + EVALUATION.md Outcomes addendum).
