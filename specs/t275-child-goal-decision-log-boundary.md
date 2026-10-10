check: cd "$(git rev-parse --show-toplevel)" && export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared && touch src/*.rs tests/*.rs && test "$(grep -c "stays EMPTY so a harvest never re-joins child-side records" LOOP-SPEC.md)" = 1 && test "$(grep -c "stays EMPTY so a harvest never re-joins child-side records" META-SPEC.md)" = 1 && cargo test --release --test todo_consistency --test eval_outcomes_carry --test eval_state_delta --test loop_spec_doctrine_prune --test loop_spec_recovery --test loop_spec_decision_records --test internal_info_lint

# T275 — the children-never-decision_log goal-template line (LOOP-SPEC step 2 + META-SPEC §6)

**One concern:** write the child-side decision-corpus boundary into the two
goal templates children actually read — LOOP-SPEC.md's step-2 impl goal and
META-SPEC §6's validator goal — so a child NEVER calls `decision_log` and a
worktree's own `.chug/decisions.jsonl` stays empty by instruction, not luck.

## Why

The F13 decision corpus (`.chug/decisions.jsonl` in the MAIN repo) has ONE
writer per record: the orchestrator. Four times now a child has written its
own records into its WORKTREE's `.chug/decisions.jsonl` — the cycle-450
wrap's three instances (the t272 impl's outcome record carrying the T246
malformed SHAPE in a throwaway corpus; the t273 impl's non-seed impl-design
class; the t273 validator's validation-verdict duplicating the orchestrator's
main-corpus d1791624468-8) plus the fourth: the t274 impl's
validation-verdict carrying the T189 lane call d1791627192-2, harvested
inside `.chug/events-t274-impl-20261010-101641.jsonl`. Every instance died
with its worktree per the zero-contamination precedent (events harvests
carry the stream; a DIFF-harvest of the worktree corpus would re-join
child-side records into the F13 corpus under the wrong provenance), but the
boundary today is prose-only and aimed at the wrong nouns: the impl goal's
"DO NOT touch TODO.md or LEDGER.md — bookkeeping is the orchestrator's"
names FILES, never the corpus, and the validator goal never mentions
`decision_log` at all. The cycle-450 wrap armed the trigger ("continued
sightings file the goal-template line", d1791624787-18); the cycle-454 wrap
declared it FIRED on the fourth instance; the cycle-458 eval (trip 82)
discharges it (triage d1791630092-2).

## Repo context (read these first)

- `LOOP-SPEC.md` step 2's goal template — the impl-goal block ends with
  "DO NOT touch TODO.md or LEDGER.md — bookkeeping is the orchestrator's."
  (the sentence this item extends; `tests/loop_spec_recovery.rs` pins text
  in this region — the addition must not disturb existing pinned tokens).
- `META-SPEC.md` §6's validator goal template — the VALIDATION-ONLY block
  (never mentions `decision_log` today).
- `.chug/decisions.jsonl` — the main corpus (1,953 records at filing); the
  F13 distillation joins records by id — child-side duplicates train
  unlabeled or double-joined rows.
- The harvest discipline: LOOP-SPEC Phase 2 step 5 (events + LEDGER
  harvests only; the rest of the worktree's `.chug/` dies with the
  removal).

## Requirements

1. LOOP-SPEC.md step 2's goal template gains ONE sentence immediately after
   the "DO NOT touch TODO.md or LEDGER.md — bookkeeping is the
   orchestrator's." sentence, verbatim:

   "NEVER call `decision_log` — routing, verdict, and recovery records are
   the orchestrator's writes, and your worktree's own
   `.chug/decisions.jsonl` stays EMPTY so a harvest never re-joins
   child-side records into the F13 corpus."

2. META-SPEC §6's validator goal template gains ONE sentence (adjacent to
   the VALIDATION-ONLY instruction), verbatim:

   "NEVER call `decision_log` — the verdict record is the orchestrator's
   write from your `.chug/verdict.md`, and your worktree's own
   `.chug/decisions.jsonl` stays EMPTY so a harvest never re-joins
   child-side records into the F13 corpus."

3. Both edits land inside the goal-template blocks (the text the child
   reads); no other template text changes; no re-wrapping of surrounding
   paragraphs; doctrine-only diff (two md carriers).
4. The shared tail "stays EMPTY so a harvest never re-joins child-side
   records" appears EXACTLY ONCE in each carrier (the check line's
   needles).

## Tests

- `grep -c` needles exactly once per carrier (the check line enforces).
- Pin floor green: todo_consistency + eval_outcomes_carry +
  eval_state_delta + loop_spec_doctrine_prune + loop_spec_recovery (pins
  the impl-goal template region) + loop_spec_decision_records +
  internal_info_lint.
- Validator mutation legs where feasible: drop either sentence → its
  needle grep fails; corrupt the shared tail ("stays EMPTY" → "stays
  empty") → both needle greps fail; the pin floor stays green without the
  sentences (the greps are the load-bearing guard, the suites the
  regression surface).

## Acceptance

- Both needles exactly once per carrier; the 7-suite pin floor green.
- Diff touches ONLY LOOP-SPEC.md + META-SPEC.md, ≤ ~15 changed lines; no
  TODO.md/LEDGER.md edits (the orchestrator's).
- kimi adversarial validation REQUIRED (the carriers are the loop/spec
  doctrine itself — the core-list leg makes the T189 lane ineligible).

## Estimate

- estimate: ~15 changed lines (two one-sentence template additions plus
  fence-safe line breaks), doctrine-only (LOOP-SPEC.md + META-SPEC.md).
