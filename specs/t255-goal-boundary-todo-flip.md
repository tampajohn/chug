# T255 — LOOP-SPEC doctrine: the final todo flips BEFORE goal_complete; a completed-recovery cycle PROCEEDS, never re-claims (zombie-todo no-op class, 2nd fire)

check: touch src/*.rs tests/*.rs; export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared; cargo test --release

## repo-context

The zombie-todo no-op class fired its SECOND instance in the
cycles-170–172 chain — the standing re-file trigger from the
cycle-148 eval (d1791339250-2: "a SECOND fire files the combined
doctrine clause") executed at the cycle-173 trip evaluation.

- **First fire (census 1):** cycle 146's orchestrator exited
  goal-accepted with its final todo t289 ("goal_complete with
  cycle-146 summary") un-flipped — the goal-boundary race:
  acceptance ends the run before the flip — and the next launch
  (01:47:47Z) did recovery-complete-then-stop, re-claiming cycle
  146. Watch-listed at the cycle-148 eval; zero re-fires censused
  at trips 152/156/160/164/168.
- **Second fire (this chain):** cycle 169's first segment exited
  goal-accepted at 07:36:09Z with its final wrap todo t356
  un-flipped; the 07:40:19Z cold launch read the un-flipped todo as
  open bookkeeping, re-verified the already-complete wrap (wrap
  notes 00518ef pushed, `main == origin/main`, sleep-ok probe
  `60 0`, decisions-audit clean), retitled t356 ("Phase 3 wrap
  verified complete…"), and goal-completed in 5 iterations
  re-claiming cycle 169 — stream
  `.chug/events-20261007-074553.jsonl` (5 iters, 0 aborts, 0 failed
  tool results, goal accepted). Cost: one cycle slot (~4 min wall +
  one 60s backoff rung); zero correctness harm (the T237 machine
  walk correctly did NOT credit the commit-free echo — streak read
  0, 60s rung logged 07:43:11Z; pacing absorbed it).

Root structure, two facts:

1. **The goal-boundary race.** A todo whose completion condition IS
   the `goal_complete` call can never be flipped: the flip must
   precede the call, and the call's acceptance ends the run. Any
   cycle that files such a todo guarantees its own final todo ships
   un-flipped, and the next cold cycle reads it as open bookkeeping.
2. **The recovery-then-stop shape.** A cold cycle that reconstructs
   a COMPLETE previous cycle and stops (goal_completes on the
   verification alone) burns a cycle slot. The mandated shape
   observed working is cycle-150's recovery-then-full-disposition
   (reconstruct, repair the stale todo, then PROCEED into its own
   Phase-1 disposition in the same run).

Both clauses ride `LOOP-SPEC.md`: (a) a Phase-3 wrap doctrine
paragraph (the goal-boundary race), (b) a completion directive
appended to the "Read-first recovery against unremembered state
(T251)" hard rule. The pin home is `tests/loop_spec_recovery.rs`
(it already pins the T251 clauses at lines ~4041/~4346).

estimate: ~70 changed lines (two additive doctrine paragraphs ~20
lines + one new non-vacuous pin ~35–45 lines; doctrine-comment
density, the 1.5–3× band on the ~10-line text core).

## requirements

1. **Clause (a) — the goal-boundary race doctrine**, a new short
   paragraph in `LOOP-SPEC.md` Phase 3 (at the wrap's final-steps
   area, near the `goal_complete` mention): the wrap's final todo
   flip MUST land BEFORE the `goal_complete` call; NEVER file a
   todo whose completion condition IS `goal_complete` itself —
   acceptance ends the run, the queued flip never lands, and the
   next cold cycle re-closes an already-closed wrap. Both fires
   named with their todo ids (cycle-146 t289, cycle-169 t356) as
   the evidence anchors.
2. **Clause (b) — recovery-PROCEEDS-never-completes**, appended to
   the "Read-first recovery against unremembered state (T251)"
   hard rule: a cold cycle whose reconstruction finds the previous
   cycle's books CLOSED (wrap pushed, `main == origin/main`,
   probes/audit green) repairs the stale bookkeeping (flip/retitle
   the inherited todo) and then PROCEEDS into its own Phase-1
   disposition (or Phase-2 work) in the SAME run — it never
   `goal_complete`s on the verification alone. Names cycle-150's
   recovery-then-full-disposition as the mandated shape and the
   cycle-169 5-iteration echo as the counter-example (second fire
   of the class).
3. **Additive only, no renumbering, no edits to existing pinned
   sentences** — both clauses are new paragraphs; every existing
   loop_spec pin stays green unmodified (if a pin's premise
   genuinely moves, the pin update is part of the diff and named
   in the commit message).
4. **A new non-vacuous pin** in `tests/loop_spec_recovery.rs`
   asserting both clauses' load-bearing tokens in `LOOP-SPEC.md`
   (the impl chooses the exact needles; the pin MUST fail on the
   pre-T255 `LOOP-SPEC.md` — the impl demonstrates the RED leg
   before finalizing and names the RED proof in its goal summary).

## tests

- The new pin of requirement 4 (RED-proven against the parent
  `LOOP-SPEC.md`).
- The full loop_spec pin family stays green.
- The check line runs the full release suite (doctrine pins live
  across many test binaries; warm wall ~60–90s in the shared slot,
  well under the 300s gate).

## acceptance

- The check line is green.
- Both clauses present with the named evidence anchors; the new
  pin RED on the parent text, GREEN after.
- README: not user-visible (internal orchestration doctrine) —
  the merge-time README gate makes the final call.
