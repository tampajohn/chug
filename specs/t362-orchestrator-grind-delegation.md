# T362 — orchestrator grind delegation: fixture/gate iteration moves to glm children; inline bash budget + shape metric

estimate: ~120 changed lines (LOOP-SPEC doctrine + eval-notes shape metric + eval-triage class + fixtures)
check: CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-validate-a cargo test --release --test loop_spec_grind_delegation

## Why

Operator 2026-11-11 on the T361 cycle: "why did this run 96 iterations
to do a release, seems wonky." Breakdown of that cycle: 1 impl delegate,
1 edit, 8 todo_update, 2 update_ledger — and **95 inline bash calls by
the kimi orchestrator** (fixture battery iterations, gate windows,
consistency runs, heredoc file writes). The validation RIGOR is right
(RED-proven fixtures on the most load-bearing file in the repo); the
DRIVER is wrong — the most expensive model in the routing table is doing
mechanical per-fixture grind that the T81 coding model exists for. This
is the worked-cycle sibling of the empty-cycle overhead class (T257/
T258/T260 lineage): same money, different hiding place.

## What

1. **Doctrine (LOOP-SPEC)**: fixture iteration loops (write-RED →
   implement → verify → mutation rounds), gate window driving, and
   battery re-runs are delegated to glm children by default; the
   orchestrator reviews child OUTPUTS and owns decisions, never drives
   per-fixture bash itself. Explicit carve-outs: the verdict remains
   kimi's (validator independence is untouched), and items under ~20
   steps may stay inline (delegation overhead dominates there).
2. **Inline bash budget**: routine items target < 20 orchestrator
   inline-bash calls per cycle; over-budget requires the eval note to
   name the reason (e.g., fixture RED-proof that must stay orchestrator-
   side for independence — and if THAT is the recurring reason, the
   doctrine says how to delegate it without breaking independence).
3. **Shape metric in eval notes**: every eval cycle records its
   tool-call shape (`shape: bash=N delegate=N (impl/validate/fixup)`),
   so the class is measurable over time instead of discovered by the
   operator reading a cycle log by hand. One line per cycle, added to
   the wrap's existing record.
4. **New eval-triage class**: "orchestrator-inline-grind" — recorded
   when inline bash exceeds the budget with no delegate covering the
   same phase; recurrence drives the doctrine tune (the T261 channel).

## Tests

- Fixture: a synthetic item run shows inline bash below budget with the
  fixture phase delegated (assert the delegate call exists and inline
  bash < 20 in the cycle record).
- The shape metric line appears in the eval note and parses (count of
  shape lines == 1 per cycle).
- Validator independence fixture: the verdict's model of record is
  unchanged (kimi) after delegation of the grind phases.

## Compat

Validation rigor unchanged: same fixtures, same RED-proofs, same gates,
same kimi verdicts — only the cheap model drives the grind. No change
to routing (T81), dormancy (T325), or the release machinery.

## Out of scope

- Slimming the fixture batteries themselves (rigor is the point; this
  item is about WHO drives them).
- Gate parallelism (T242 bg windows already exist; this is about the
  orchestrator's inline driving of the remaining serial work).
