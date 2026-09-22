# T6 — HTTP stub tests must not hang gates

check: cargo test

## Concern

Round-2's HTTP stub tests made a gates bash run 9 minutes (and a
`cargo test` 4 minutes), freezing the meta long enough to look wedged
(TODO T6 note; EVALUATION.md I6). Stub servers must fail fast on their own
and review/validation commands must be bounded, so a broken stub can never
hold a gate hostage again.

## Repo context

- `src/mcp_http.rs` — streamable-HTTP MCP transport + its test stub server.
  Partial mitigation landed: the stub sets a 10s read timeout
  (`src/mcp_http.rs:1110`) and the full suite currently runs in ~6s.
- `src/mcp.rs` — stdio MCP client; its fake-server tests use python3
  scripts (same family of risk: a stuck child blocks the suite).
- Meta loops (META-SPEC.md) run review/validation commands like
  `cargo test` via bash with no explicit `--test-threads` bound or
  wall-clock cap beyond the driver's bash timeout.

## Requirements

1. Every stub/fake server in `src/mcp_http.rs` and `src/mcp.rs` tests has
   internal timeouts (accept, read, write) so a stuck connection fails the
   test in seconds rather than hanging it — no reliance on the outer
   `cargo test` process being killed.
2. Tests that wait on stubs use bounded waits (deadline + poll), never
   unbounded blocking reads or `thread::sleep` chains that only pass on a
   fast machine.
3. Gate/review commands documented in META-SPEC.md and the specs use
   bounded invocations where hangs are possible (e.g.
   `cargo test -- --test-threads=N` and/or an explicit `timeout`-style
   wrapper), so a future slow stub degrades to a failure, not a freeze.
4. No test may rely on wall-clock sleeps longer than a few seconds to
   pass; prefer synchronization (channels, latches) over sleeping.

## Tests

- A deliberately unresponsive stub (accepts, never replies) makes the
  affected test FAIL within a small bounded time (assert elapsed < limit),
  not hang.
- Full `cargo test` completes in well under a minute on this machine
  (regression guard against re-introduced unbounded waits).

## Acceptance

- `cargo build`, `cargo clippy --all-targets -- -D warnings`, `cargo test`
  all green.
- `time cargo test` stays in the same order of magnitude as today (~6s).
