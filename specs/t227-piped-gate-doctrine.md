# T227 — LOOP-SPEC: the TODO.md-edit gate must not swallow a red guard through a pipe (doctrine, SOLO)

check: set -o pipefail; export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-validate-a; touch src/*.rs tests/*.rs; cargo test --test loop_spec_recovery --test todo_consistency

estimate: ~60 changed lines (one doctrine sentence + one carrier pin leg)

## Concern

Cycle-101 pushed a RED main (c06a555, live on origin ~3 minutes): the
T220/T223 notes cells carried literal `|` characters that split the rows
into 8 cells (the T37/T8 class), `cargo test --test todo_consistency`
was red — and the orchestrator ran the guard INSIDE a pipe chain without
`set -o pipefail`, so the chain reported the filter's exit 0 and the red
was swallowed into a push (fixed forward in 6365b50, root cause named in
the commit).

LOOP-SPEC Phase-2 step 5 already says the orchestrator runs
`cargo test --test todo_consistency` (seconds) after every TODO.md edit,
before committing — what it does not say is that the run must not be
piped unguarded. The spec-`check:` pipe lint (T67→T164,
tests/todo_consistency.rs:205+) covers spec check lines; the
ORCHESTRATOR's ad-hoc gate chains have no carrier.

Cycle-103 added a THIRD instance while this row sat queued: the
filing cycle's own T225 flip ran `cargo test ... | grep "test result"`
— grep exits 0 on matching the FAILED line and the chain pushed a red
row-format guard live (71ca2c1, ~3 min, fixed d2bf500). Two of the
three recurrences are orchestrator filter chains, not model carelessness
— the carrier sentence must name the grep-filter shape explicitly.

## Requirements

1. LOOP-SPEC Phase-2 step 5's TODO.md-edit paragraph gains one sentence:
   the todo_consistency run is UNPIPED, or the chain begins
   `set -o pipefail;` — a piped gate whose filter exits 0 reports green
   on a red guard (the c06a555 lesson) — and the same rule binds ANY
   ad-hoc gate chain the orchestrator pipes through tail/head/grep.
2. One carrier pin leg (tests/loop_spec_recovery.rs's LOOP-SPEC window
   or tests/todo_consistency.rs — the impl picks): asserts the sentence
   exists in LOOP-SPEC.md inside the step-5 TODO.md-edit window, exactly
   once. RED-proof: delete the sentence → the pin goes RED; restore
   byte-clean.
3. Doctrine row → SOLO dispatch (no other child in flight), kimi
   REQUIRED validation. No other file changes (the TODO.md row flip is
   the orchestrator's, post-merge).

## Acceptance

- cargo test --test loop_spec_recovery --test todo_consistency green;
  clippy -D warnings clean (pin legs compile).
- The RED-proof recorded in the commit message.

## Out of scope

- Hooks/pre-commit automation (weighed and rejected at filing: the
  doctrine sentence + pin is the cheap fix; automation is a bigger
  surface for a 3-minute-red blast radius).
- Reformatting step 5 beyond the one sentence.
