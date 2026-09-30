# T167 — orchestrator RED-proofs apply mutants in the worktree cwd, never the main tree

check: export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared && cargo test --test loop_spec_worktree_mutants

## Repo context

LOOP-SPEC.md Phase 2 step 3 (Review) owns the orchestrator's personal
RED-proof practice for tests-only rounds (mutant → gate → restore).
`edit_file`/`write_file` are cwd-confined to the caller's tree — for the
orchestrator that is the MAIN repo, while the gates under proof run in the
child's `/tmp/chug-loop-t<N>` worktree (cross-tree edits go through bash —
the documented escape hatch). Doctrine pins live in tests/loop_spec_*.rs.

estimate: ~40 changed lines (one doctrine sentence + one pin file).

## Evidence

Cycle-75 wrap (T160 entry, commit f2d0977): the orchestrator applied a
mutant through `edit_file` — it edited MAIN's src/mcp_serve.rs while the
gates ran the WORKTREE copy, so the mutant "survived" trivially. Caught on
the first read-back; main restored byte-identical; worktree mutants
re-applied via bash `perl` in the worktree cwd. A read-back habit is all
that stopped a false RED-proof from shipping.

## Requirements

1. LOOP-SPEC.md Phase 2 step 3 gains ONE sentence (in the RED-proof /
   docs-only-guard neighborhood): orchestrator mutation legs for RED-proofs
   MUST be applied inside the worktree via bash (e.g. `perl -i` with cwd
   `/tmp/chug-loop-t<N>`) — `edit_file`/`write_file` are confined to the
   main tree and silently produce false survivors when the gates under
   proof run the worktree copy; after any main-tree edit during a round,
   verify main is restored byte-identical before merging.
2. New pin file `tests/loop_spec_worktree_mutants.rs`:
   (a) the sentence's load-bearing tokens exactly-once in LOOP-SPEC.md
   (`worktree`, `edit_file`, `byte-identical`);
   (b) deletion hand-check stated in the commit message (loop_spec_*
      convention).

## Tests

The pin file is the tests.

## Acceptance

- The spec check green; full `cargo test` green.

## Out of scope

- Any tool-side change to edit_file's confinement (it is a correct safety
  boundary — the gap was doctrine, not tooling).
