# T19 — Codify the child events-harvest step in LOOP-SPEC (K2)

check: cd /Users/jadams/workspace/chug && cargo test

## Why (evidence)

The K2 events-harvest convention — copy a child run's
`.chug/events.jsonl` into the main repo's `.chug/` as
`events-t<N>-<role>-<ts>.jsonl` **before** `git worktree remove` — has
been practiced two cycles running and is the only reason child costs and
child behavior are jq-knowable (cycle-3/4 evaluations mined
`events-t15-impl-*`, `events-t16-impl-*`, `events-t17-*` x4).

Cycle 5 (2026-09-25) supplied the failure case: the orchestrator removed
`/tmp/chug-loop-t18` after merge **without harvesting** — both T18 child
streams (glm impl, kimi validator) plus their transcripts and the
impl child's LEDGER.md were deleted with the worktree, one cycle after
cycle-4 harvested 4/4 correctly by hand. A convention that lives only in
the previous cycle's ledger is one distraction away from data loss;
cycle-4's evaluation already carried "codify K2 in LOOP-SPEC §2.5" as a
human-decision item, and the failure now has evidence.

`git worktree remove` deletes untracked files without prompting, and
`.chug/` is untracked in the worktree — the loss is silent.

## Repo context

- `LOOP-SPEC.md` — Phase 2 step 5 ("Merge + close") and Phase 3 ("Wrap")
  are the text to amend; the single `git worktree remove` moment per item
  is where the harvest must happen **first**. This is a doctrine edit to
  the loop's own spec — the same class as T6's META-SPEC bounded-gates
  amendment (which landed as a normal row).
- Harvested examples to name as the convention's precedent:
  `.chug/events-t17-impl-20260925-170831.jsonl`,
  `.chug/events-t17-validate3-20260925.jsonl` (naming:
  `events-t<N>-<role>-<yyyymmdd>-<hhmmss>.jsonl`).
- Validation REQUIRED per LOOP-SPEC §2.4 (this row touches the loop/spec
  doctrine itself): the validator confirms the amended text is
  internally consistent with the rest of LOOP-SPEC (step numbering,
  the hard rules) and with META-SPEC's worktree rules.

## Requirements

1. LOOP-SPEC Phase 2 step 5 gains an explicit harvest step, ordered
   **before** any worktree removal: copy the child's
   `.chug/events.jsonl` to the main repo's
   `.chug/events-t<N>-<role>-<yyyymmdd>-<hhmmss>.jsonl`, and the child's
   `LEDGER.md` to `.chug/LEDGER-t<N>-<role>-<ts>.md` when it carried a
   verdict or non-trivial findings. Transcript harvest stays operator's
   choice (size) — the text says so explicitly rather than leaving it
   implicit.
2. The step names the failure mode it prevents (silent loss at
   `git worktree remove`; cycle-5 T18 reference) in one clause — no
   essay.
3. No other LOOP-SPEC text changes; step numbering/hard-rule references
   stay valid (adjust any "step N" cross-references the insertion
   breaks).
4. README untouched (internal doctrine, not user-visible).

## Tests

- Doctrine-only change: no code, no new tests. `cargo test` must stay
  green (the todo_consistency guard covers the TODO row/spec pairing).
- Non-vacuousness: the validator greps the merged LOOP-SPEC.md for the
  harvest step and confirms it precedes the removal instruction.

## Acceptance

- `cargo build`, `cargo clippy --all-targets -- -D warnings`, `cargo test`
  all green.
- LOOP-SPEC.md diff is exactly the harvest step (+ any cross-reference
  renumbering it forces).
- Adversarial validation (REQUIRED — doctrine edit): PASS with the
  consistency checks above.
