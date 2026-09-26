# T42 — Pin webfetch connect/total timeout const values (T37 validator nit)

check: cargo test

## Concern

One concern: the two webfetch timeout constants' *values* are untested — a
const-only edit (10s→60s) would pass the whole suite while silently
contradicting the tool description's "connect 10s, total 30s".

## Repo context

- `src/webfetch.rs:54` — `const WEB_FETCH_CONNECT_TIMEOUT: Duration =
  Duration::from_secs(10);`
- `src/webfetch.rs:60` — `const WEB_FETCH_TOTAL_TIMEOUT: Duration =
  Duration::from_secs(30);`
- The user-facing surfaces carry the numbers as literals: the schema
  description (`src/webfetch.rs:77`, "connect 10s, total 30s") and the
  timeout error message (`:147`, same phrase).
- T37's kimi validator (PASS, 3 non-blocking observations — see
  `.chug/events-t37-validate-20260926-043809.jsonl` and the T37 TODO row)
  flagged: "connect-timeout const not value-pinned". The cycle-17 wrap
  carried it as a const-pin candidate for this evaluation.
- Tests-only item (all hunks in `#[cfg(test)]` or const-surface only) →
  adversarial validation optional per LOOP-SPEC §2 step 4 (T16/T31
  precedent); orchestrator gates + spot-check suffice.

## Requirements

1. Pin both values: `WEB_FETCH_CONNECT_TIMEOUT == Duration::from_secs(10)`
   and `WEB_FETCH_TOTAL_TIMEOUT == Duration::from_secs(30)` in
   `webfetch.rs`'s test module.
2. Coherence: a const-only edit must fail at least one test even though the
   description/message literals are unchanged — i.e. either (a) the
   description and timeout error message interpolate the consts
   (`Duration::as_secs`) instead of carrying literals, with the existing
   text assertions then pinning both ends, or (b) explicit assertions that
   the description and error text contain the numbers matching the const
   values. Implementation may choose (a) or (b); (a) is preferred — it makes
   drift structurally impossible. If (a), the produced strings must remain
   byte-identical to today ("connect 10s, total 30s" etc.), proven by the
   existing tests plus any new pin.
3. No behavioral change: timeouts, error text, and description render
   byte-identical.

## Tests

- The two const-value pins.
- The coherence leg (per the chosen option).
- Existing webfetch tests keep passing unmodified (except any that the (a)
  refactor makes redundant — removal must be justified in the commit
  message).

## Acceptance

- `cargo build`, `cargo clippy --all-targets -- -D warnings`, `cargo test`
  all green.
- Mutation-ready: setting either const to a different value fails at least
  one test with no other edit.
- Commit message cites the T37 validator observation.

## Out of scope

- Re-tuning the values themselves (10s/30s are spec-mandated by T37 req 2).
- mcp_http's similar consts (already test-pinned per T16).
