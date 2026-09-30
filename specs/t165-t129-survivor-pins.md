# T165 — pin the T129 validator's two weak-test survivors (M7 CLI plumbing, M8 isError arm)

check: export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-impl-a && cargo test --test mcp_serve && cargo test --bin chug mcp_serve

## Repo context

T129 landed `chug_launch` (F10 phase 2b) flag-gated by `mcp-serve
--allow-launch`. Its kimi validator PASSed with two weak-test survivors,
recorded in `.chug/LEDGER-t129-validate-20260929-064334.md`:
- **M7** — CLI plumbing `cmd_mcp_serve(false)` (flag OFF path): mutant
  SURVIVED with 938 bin + 3 integration green.
- **M8** — launch-failure isError arm flipped true→false: mutant SURVIVED
  with 13/13 integration green.
The launch leg lives in `src/mcp_serve.rs` (cmd_mcp_serve wiring, the
--allow-launch policy boundary) with tests in `src/mcp_serve/tests.rs`
(bin) and `tests/mcp_serve.rs` (integration). The T153/T157 rows later
extended the same flag-gate shape; do not regress their pins.

estimate: ~110 changed lines (tests-only: 2-4 pin tests + fixture tweaks).

## Requirements

1. **M7 pin**: a test that calls the CLI wiring with the flag OFF and
   asserts the boundary behavior — `chug_launch` is NOT advertised AND a
   direct call is rejected with the policy error (the exact current
   refusal text/shape; read the T129 implementation). The pin must die if
   `cmd_mcp_serve(false)` stops wiring the OFF path (RED-prove by
   reverting the plumbing to the pre-T129 or always-on shape).
2. **M8 pin**: a test that drives a launch failure end to end (stub the
   spawn to fail) and asserts the tool result's isError arm is TRUE with
   the failure payload — dies if the arm flips to false (RED-prove).
3. Both pins name the survivor they kill in a comment (M7/M8 + the
   validator ledger filename).
4. Optional stretch (only if cheap, else note as deferred): the flag-ON
   wire e2e — `chug_launch` advertised ⇔ callable with `--allow-launch`
   (the T129 descope note). The T153/T157 flag e2e tests are the pattern.

## Tests

Tests-only: no production src/ edits. Zero timeout bumps. The pins live in
whichever of the two mcp_serve test surfaces matches each leg (M7 is CLI
plumbing — likely integration; M8 — wherever the isError arm is produced).

## Acceptance

- The spec check green; full `cargo test` green.
- Commit message states both RED-proofs (mutant applied → pin failed →
  restored green).

## Out of scope

- Any behavior change to the launch leg; the stub-spawn events-ordering
  nit (rejected at the cycle-76 eval, cosmetic).
