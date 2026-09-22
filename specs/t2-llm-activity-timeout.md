# T2 — Per-request activity timeout on LLM calls

check: cargo test

## Status

DONE-RECORD. Shipped in `9840aba` (closed by TODO row T2). Backfilled
under T8 (EVALUATION.md I10).

## Concern

A round-2 child hung 5+ minutes at 0% CPU on a stale HTTP connection; only
the 600s total read timeout saved it (and the wedge-kill got there first).
A ~180s no-bytes timeout was needed, distinct from the total read timeout.

## What shipped

- `src/api.rs:15` — `ACTIVITY_TIMEOUT_SECS = 180`: zero-bytes activity
  watchdog on response bodies (`read_body_with_watchdog`,
  `src/api.rs:166`,`:193`): any 180s silent gap fails the attempt.
- A watchdog failure classifies as connection-level (`src/api.rs:149`),
  so T1's retry schedule applies instead of aborting the run.

## Regression tests

`src/api.rs` tests with injected timeouts:
`body_watchdog_aborts_silent_stream`, `body_watchdog_allows_slow_steady_stream`.
Run `cargo test api::tests`.
