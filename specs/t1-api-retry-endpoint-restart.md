# T1 — API retry survives endpoint restarts

check: cargo test

## Status

DONE-RECORD. Shipped in `9840aba` (closed by TODO row T1). This spec was
backfilled under T8 after EVALUATION.md I10 found `specs/` had never
existed in git.

## Concern

Two meta sessions died to transient muse endpoint resets: the connection
dropped for ~30s but chug retried only ~15s (4 attempts, 1–8s backoff).
Connection-level errors (reset/refused/timeout) should retry for minutes;
4xx stays fail-fast.

## What shipped

- `src/api.rs:19` — `RETRY_DELAYS_SECS = [1, 2, 4, 8, 16, 32, 64, 120, 240]`
  (~8 minutes of retry budget) for connection-level failures.
- Retry loop (`src/api.rs` `:559-598` area): 4xx responses fail fast;
  `retry-after` response headers honored but capped at 120s.
- T2's activity watchdog failures classify as connection-level, so this
  schedule also covers stalled streams.

## Regression tests

`src/api.rs` tests (`:849+`): retry schedule across endpoint restarts,
4xx fail-fast, `retry-after` cap. Run `cargo test api::tests`.
