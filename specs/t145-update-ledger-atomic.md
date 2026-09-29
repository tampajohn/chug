# T145 — update_ledger writes through fsatomic::write_atomic (completes T136's crash-safety class)

check: cargo test

estimate: ~60 changed lines (src ~15, tests ~45)

## Repo context

T136 (cycle 67, bf672b6) made crash-torn transcripts resumable and routed
`transcript::rewrite` + `todos::save` through the new
`fsatomic::write_atomic` (same-dir temp + fsync + rename). The round-1
validator's carried finding (decision `d1790643172-7`, non-blocking):
**`update_ledger`'s wholesale LEDGER.md write was NOT swept** — the same
crash class T136 fixed for transcripts/todos is still open for the
ledger. `fn update_ledger` lives at src/tools.rs:746 and writes the whole
file in one shot; a kill mid-write leaves a torn or empty LEDGER.md,
which is the orchestrator's and every child's external memory (and the
SPEC-9 saga's original motivation for ledger tooling). `src/fsatomic.rs`
already exists with the primitive and its own tests.

One concern only: the `update_ledger` write path. Chat `/ledger` edits or
any other LEDGER.md writer found during the sweep are named in the
commit message with their verdict, but only writers that do wholesale
replace-writes of the live ledger are in scope for conversion (append
paths are a different, safe shape).

## Requirements

1. `update_ledger` (src/tools.rs:746) writes via `fsatomic::write_atomic`
   instead of its current direct write.
2. Sweep: enumerate every wholesale LEDGER.md write site in src/ in the
   commit message (ledger.rs seed paths, tests excepted) with a
   converted / deliberately-excluded verdict. Seed-once creation paths
   (write-if-absent) MAY be excluded with the reason stated.
3. Behavior unchanged on the happy path: same content bytes, same tool
   result text, same error surface shape (a failing write still errors
   the tool call, never aborts the run).
4. No new dependencies; reuse `crate::fsatomic`.

## Tests

- RED first: a test that intercepts the write seam (or asserts on the
  implementation shape the T136 tests used for todos::save — follow that
  module's pattern) proving `update_ledger` goes through
  `fsatomic::write_atomic` — fails pre-fix.
- Happy-path regression: update_ledger still replaces the ledger
  byte-exactly and returns the same tool result.
- Crash-shape leg mirroring the T136 pattern (e.g. the RLIMIT_FSIZE or
  error-injection leg that module used): a failed atomic write leaves
  the PREVIOUS ledger byte-intact (no torn file, no empty file).
- Sweep-verdict leg: a grep-shaped test or module-level assertion is NOT
  required; the sweep is commit-message evidence.

## Acceptance

- `cargo build && cargo clippy --all-targets -- -D warnings && cargo test`
  all green in the worktree.
- Commit message names every LEDGER.md write site found + verdict.
- Validator REQUIRED (src/tools.rs is on the core list). Mutation
  candidates: revert to direct write (must die on the RED leg),
  atomic-write-without-fsync if distinguishable, wrong dir for the temp
  file (cross-device rename must not silently pass).
