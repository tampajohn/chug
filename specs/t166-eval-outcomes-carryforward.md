# T166 — the eval template must carry forward ALL existing Outcomes sections

check: export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared && cargo test --test eval_outcomes_carry

## Repo context

META-META-SPEC.md's "Write EVALUATION.md" section defines the eval document
shape; LOOP-SPEC.md Phase 3 owns the Outcomes lifecycle (per-item entries,
last-6-full compaction). Doctrine pins live in tests/loop_spec_*.rs and
tests/todo_consistency.rs (one concern per file).

estimate: ~45 changed lines (one doctrine sentence + one pin file).

## Evidence

Cycle-73 wrap bookkeeping incident: the cycle-72 eval commit (1206d93)
REGENERATED EVALUATION.md and dropped the Cycle 69/70/71 Outcomes sections
that existed at the cycle-71 wrap — the narrative survived only because the
wrap restored it verbatim from `9c1d6bd:EVALUATION.md`. The wrap named the
remedy: "the eval-commit template must carry forward ALL existing Outcomes
sections (next-eval candidate)". Cycle-75 named it again. Nothing pins the
carry-forward, so every fresh eval can silently amputate the history.

## Requirements

1. META-META-SPEC.md's "Write EVALUATION.md" section gains ONE sentence:
   a regenerated EVALUATION.md MUST carry forward every existing
   `## Outcomes` content verbatim (per-cycle sections and per-item
   entries) — the eval rewrites the assessment body only, never the
   Outcomes ledger; before committing, verify the newest pre-existing
   cycle's section is still present.
2. New pin file `tests/eval_outcomes_carry.rs`:
   (a) the sentence's load-bearing tokens appear exactly-once in
   META-META-SPEC.md (`carry forward`, `Outcomes`, `verbatim`);
   (b) non-vacuousness: deleting the sentence fails the pin (hand-check
   stated in the commit message, loop_spec_* deletion-proof convention);
   (c) the current EVALUATION.md still contains a `## Outcomes` heading
   (cheap structural sanity, not a full history check).

## Tests

The pin file is the tests. Follow the loop_spec_* pin-file conventions
(read-the-doctrine-file, exactly-once needles).

## Acceptance

- The spec check green; full `cargo test` green.

## Out of scope

- Any mechanical history-DIFF guard (fragile across cycle numbering);
  LOOP-SPEC edits (its Outcomes rules are already correct).
