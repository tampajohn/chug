# T143 — max_tokens too small for thinking models (GLM truncation class)

check: cargo test

## Concern

Operator report 2026-09-28 (live session, Swift app build): glm-5-3-flash
died at iteration 9 with an unrecoverable tool-call parse error — the
write_file JSON was truncated mid-stream. Root cause: request max_tokens
is hardcoded 8192 (src/api.rs), and GLM thinking blocks consume the SAME
budget as the response content — a ~7KB file write plus a thinking block
overflows the cap, truncating the JSON. This is the REQUEST-side sibling
of T141 (SSE truncation acceptance, landed — that was detection, this is
prevention). Related: T38's truncated-response advisory (names
stop_reason=max_tokens) and glm-thinking-block-eats-max-tokens (upstream
behavior; thinking cannot be disabled via the proxy — probed 2026-09-26).

## Repo context

- `src/api.rs`: MAX_TOKENS constant baked into every request.
- `src/driver.rs`: BudgetExceeded::Tokens (T15) — cumulative budget, a
  DIFFERENT knob. The new per-request cap must not collide with
  T15's `--max-tokens` flag.
- T38: budget-abort advisory names stop_reason=max_tokens — the signal
  exists; the cap is just too low for thinking models on large writes.
- Operator's local workaround (F94 checkout, now stashed): 8192 → 32768
  fixed the live run. Supersede it properly — do not take the diff
  verbatim.

## Requirements

1. Per-request max_tokens becomes configurable: default 32768 (the
   operator-proven value); env `CHUG_MAX_TOKENS` override; CLI flag
   override (name chosen by implementer — MUST NOT collide with T15's
   cumulative-budget `--max-tokens`; suggest `--max-tokens-per-request`).
2. The flag/env must apply to both run and chat modes; unit tests
   against the mock Llm (no network).
3. T38's advisory text gains a remedy line when stop_reason=max_tokens
   AND the configured cap is below 32768: "raise CHUG_MAX_TOKENS".
4. eventlog run_start records the configured request cap.

## Tests

- Default is 32768; env override wins; CLI wins over env; T15's
  cumulative-budget flag unchanged (pin both in one test).
- Mock Llm asserts the configured value lands in the request body.
- Acceptance: a glm child writing a ~7KB file with thinking no longer
  truncates (the operator's session is the live proof for the default).

## Out of scope

- Per-model caps; thinking-budget request params (proxy ignores them —
  probed); T141's stream-side rejection (landed, untouched).
