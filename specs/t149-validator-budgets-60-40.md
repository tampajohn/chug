# T149 — validator child budgets 50/30 → 60/40 (LOOP-SPEC doctrine)

check: cargo test

estimate: ~40 changed lines (doctrine ~10, pins ~30)

## Repo context

Four validator budget deaths in four cycles (66–69), one of them on the
MINUTES ceiling:

- t134-validate: 50/50 iterations mid-work (T63 resume finished).
- t137-validate: died at 30m08s — 8 seconds over the 30-minute ceiling —
  with the verdict already written but unannounced (round-2 re-launch
  cost a full validator spin-up, ~11 min wall).
- t138-validate: 50/50 mid-mutants (resumed, goal-accepted iter 4).
- t142-validate: 50/50 (re-validation round absorbed it).

T79 parallel mutants and the mutate→test→revert→re-test cycles under
cold `target-shared-validate` builds eat both iterations and wall time;
validators are doing MORE mutation legs than when 50/30 was set. The
T63 resume absorbs every death, but each resume is a 5–15 min spin-up —
four per four cycles is the tax, not the exception.

This is the validator-side analog of T102 (impl 65→80). Doctrine row:
LOOP-SPEC only (META-SPEC §6's launch mechanics are overridden by
LOOP-SPEC step 4's paragraph; if §6's template also names validator
budgets, align it in the same commit — grep first).

## Requirements

1. LOOP-SPEC.md step 4: the validation child launches with
   `max_iters: 60`, `max_minutes: 40` — update the sentence that
   currently says `max_iters: 50`, `max_minutes: 30` (and its
   parenthetical rationale: minutes stays ABOVE delegate's 35 default),
   and the T63-resume paragraph's `(80/35 impl, 50/30 validate)` budget
   echo. Add the one-line census evidence (4 deaths in cycles 66–69,
   one minutes-bound) to the rationale, T102-style.
2. Grep META-SPEC.md §6 for validator budget numbers; if named, align.
   Grep src/tools.rs's delegate tool description for validator budget
   text; align if present. Commit message lists every site found.
3. Update every pin that fixes the 50/30 numbers (tests/loop_spec_*.rs
   at minimum — find them with grep for the old numbers), RED-proving
   each updated pin against the pre-edit text.
4. No other doctrine change. The impl-child budgets (80/35) are
   untouched — the iteration-census remedy for impls stays the
   filing-time estimate ceiling (T110), not iteration raises.

## Acceptance

- `cargo build && cargo clippy --all-targets -- -D warnings && cargo test`
  all green in the worktree.
- Doctrine item: runs ALONE (no other child in flight), kimi REQUIRED
  validation per LOOP-SPEC step 4 (loop/spec doctrine is on the
  REQUIRED list).
- Measure clause for the next eval: if >1 of the next 8 validator runs
  still dies at 60/40 with the verdict unannounced, the next eval
  considers trimming default mutation-leg counts instead of further
  raises.
