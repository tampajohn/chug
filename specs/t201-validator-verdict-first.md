# T201 — Validator verdict-first: VERDICT lands in .chug/verdict.md at decision time

check: touch src/*.rs tests/*.rs; export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared && cargo test --test loop_spec_recovery --test todo_consistency && grep -qF "verdict.md" META-SPEC.md && grep -qF "verdict.md" LOOP-SPEC.md

## Repo context

The unannounced-verdict class: a validator that reaches its verdict
but dies before announcing it burns a recovery cycle — census this
delta: t185-validate (50-min wall, verdict recovered via T63
resume), t188-round4 (60/60, verdict recovered from transcript),
t192-validate (60/60, verdict recovered from the log — "4th of the
unannounced-verdict class" per the cycle-88/89 wrap). The T173-era
measure clause (>1 of the next 8 dying with verdict unannounced →
trim mutation-leg counts) sits at 2 of the last 10 — NOT tripped, so
no mutation-leg trim; but every instance costs the orchestrator a
transcript-archaeology pass or a resume (~10–20 min) because the
verdict exists ONLY inside the dying child's context. The t186
validator's post-goal_complete WEDGE (14 min silent, verdict fully
rendered in its log) is the same shape one step further: the
artifact existed, the process would not exit.

One sentence kills the class for every future death mode: the moment
the verdict is DECIDED — before wrap-up gates, before the summary
polish — it lands in a file in the worktree. A budget death, a
wedge, a SIGKILL: the verdict is already on disk.

This row is DOCTRINE-ONLY (META-SPEC.md §6 goal template + LOOP-SPEC
step-4 recovery sentence + one pin leg) → SOLO.

estimate: ~30 changed lines.

## Requirements

1. META-SPEC §6's validation goal template gains the verdict-first
   sentence: when you reach your verdict, FIRST write it —
   `VERDICT: PASS|FAIL` + the numbered findings list — to
   `.chug/verdict.md` in your worktree cwd (one bash heredoc), THEN
   do any wrap-up gates or summary; a budget death after the write
   loses nothing.
2. LOOP-SPEC §2 step 4 gains the recovery sentence: on a validator
   budget death or wedge, read `.chug/verdict.md` in the worktree
   BEFORE spending a T63 resume — a written verdict IS the verdict
   (provenance note in the ledger; the resume is then only for
   missing mutation legs, named in the routing record).
3. No other text changes; no renumbering (§6 stays §6).

## Tests

- ONE new exactly-once pin leg in `tests/loop_spec_recovery.rs`:
  `verdict.md` occurs exactly once in META-SPEC.md AND exactly once
  in LOOP-SPEC.md; RED-proven by deleting either sentence, the proof
  stated in the commit message.
- `cargo test --test loop_spec_recovery --test todo_consistency`
  green.

## Acceptance

- Check line green; clippy green (trivially).
- Diff confined to META-SPEC.md, LOOP-SPEC.md,
  tests/loop_spec_recovery.rs.

## Out of scope

- Rotated/segmented verdict files (one file, last write wins — a
  fix-up round's verdict overwrites; the events stream already
  carries the round structure).
- Harvesting verdict.md into the main repo (step-5's harvest rule
  covers `.chug/` events; the verdict's content lands in the merge
  commit message as today).
