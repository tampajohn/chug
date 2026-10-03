# T207 — orchestrator wrap-window doctrine (stop-dispatch 15 → 30 + ledger wrap-state note)

check: export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared; touch src/*.rs tests/*.rs; cargo test --test loop_spec_recovery --test loop_spec_decision_records --test todo_consistency

## Repo context

Doctrine row — LOOP-SPEC.md edit; runs SOLO (no other child in flight).

Cycle-94 seg-3 (glm, `.chug/events-20261003-044541.jsonl`): the
orchestrator ran 200/200 iterations over 5h01m and DIED at the iteration
ceiling with the work done but the wrap unwritten — T204's merge (dbce0e2)
was committed, but the row flip, the push (18 commits sat unpushed), the
Outcomes entry, the harvest, and the decision records were all missing.
The ledger at death was ~150 iterations stale (it named an impl pid from
the arc's middle); the cycle-95 orchestrator spent ~6 iterations
reconstructing the true state from the git record before any productive
work. This is the T10/T12 loss class one level up: children die between
the code commit and the row flip — and so do orchestrators.

Root cause: Phase 2 step 6's stop-dispatching rule is "Fewer than 15
iterations left" (LOOP-SPEC.md:540) — calibrated when orchestrator budgets
were 120 iterations. loopd now launches orchestrators at 200 iterations /
360 minutes, and the wrap tail (final gates ~5-10 min wall + row flips +
Outcomes + harvest + release check + push + goal gate) costs ~15–25
iterations. A 15-iteration margin at 200 is structurally too thin; seg-3
dispatched a validation round inside the margin and never came back.

estimate: ~45 changed lines (LOOP-SPEC.md + one loop_spec_* pin file).

## Requirements

1. Phase 2 step 6's budget check becomes: fewer than **30** iterations left
   → stop dispatching, go to wrap — with the calibration sentence (the
   number is sized to the wrap tail's measured ~15–25 iteration cost at
   the 200-iteration orchestrator budget loopd now launches).
2. New hard rule in Phase 3 (or Hard rules, editor's pick): when the
   orchestrator crosses the stop-dispatch boundary, its NEXT ledger write
   must carry a wrap-state note naming: every merged-but-unflipped row,
   every unharvested worktree, every unpushed commit count, and every
   missing decision record — so a mid-wrap death is recoverable from the
   ledger alone with zero git reconstruction.
3. Pin the new number and the wrap-state rule in one existing
   `tests/loop_spec_*.rs` file (editor's pick which; the pin fails if the
   threshold or the rule sentence is removed).
4. The T81 anti-sprint-burn guard is unaffected (do not weaken it — a
   wrap IS an act).

## Tests

The pin from req 3 (RED-proven: deleting the threshold sentence or the
wrap-state rule from LOOP-SPEC.md turns the pin red).

## Acceptance

- LOOP-SPEC.md carries the 30-iteration threshold + the wrap-state rule.
- The named pin file passes; the pin dies on each mutant (threshold
  deleted, rule deleted).
- Full doctrine test set green: `cargo test --test loop_spec_recovery
  --test loop_spec_decision_records --test todo_consistency` (the check
  line) plus any pin file the editor touched.
