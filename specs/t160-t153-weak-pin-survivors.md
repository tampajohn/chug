# T160 — pin the two T153 kimi weak-test survivors (needle `--special` row + escalation-effect pin)

Row: T160. Filed at the T153 landing (cycle 73) from the kimi REQUIRED verdict
d1790722244 (PASS, 9 mutants: 7 RED, 2 weak-test survivors). Both survivors are
one-line-ish test pins in the T153 test files; no product change.

estimate: ~10 changed lines (one table row + one assertion + comments)

## Finding M5 (low): the argv-needle table has no near-miss `--spec*` row

`chug_cancel_command_line_needle_matches_only_chug_run_invocations`
(src/mcp_serve/tests.rs) pins `command_names_chug_run` but has no row where a
`run` token is adjacent to a `--spec*` token that is NOT `--spec` (e.g.
`("tool run --special cfg", false)`). A mutant relaxing the needle to
`pair[1].starts_with("--spec")` ships green, though the code comment explicitly
claims "`--special` … must not match". Add the row; RED-prove it against the
relaxed mutant (temporarily relax → the new row fails → restore).

## Finding M6 (low-medium): the escalation fixture leg never proves the effect

`chug_cancel_term_ignoring_fixture_escalates_to_sigkill_payload_kill`
asserts the payload says `signaled: kill` but never proves the group actually
emptied — a mutant that sends no SIGKILL yet reports `kill` passes because
`FixtureGuard`'s cleanup kill masks it. Immediately after the cancel call
(before the guard drops), assert the group is gone (the same
reap-then-ESRCH probe shape the fix-up's convergence test uses — reuse
`group_gone` or an equivalent bounded probe). RED-prove: remove the SIGKILL
line in the product (temporary mutant) → the new assertion fails → restore.

## Constraints

- Tests-only: src/mcp_serve/tests.rs (or tests/mcp_serve.rs). NO product
  edits, NO doctrine edits, NO TODO.md/LEDGER.md (orchestrator-owned).
- Zero timeout bumps (T151 doctrine — mechanism, not timeouts).
- Keep cargo build + clippy + test green. Commit your work here. Always
  commit ONLY from your worktree cwd; never run git add or git commit with
  the main repo as cwd.
- export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared before
  every cargo command (T47 shared build cache — delegate cannot pass env).

check: cd /private/tmp/chug-loop-t160 && export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared && cargo clippy --release --all-targets 2>&1 | tail -3 && cargo test --release --test mcp_serve 2>&1 | tail -3 && cargo test --release --lib mcp_serve 2>&1 | tail -3
