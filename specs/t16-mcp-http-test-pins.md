# SPEC — T16: mcp_http test pins (SPEC-9 R4 carried validator gaps)

## Repo context

`src/mcp_http.rs` (SPEC-9 streamable-HTTP transport, landed `4ef81d0`)
passed its R4 adversarial acceptance validation with three **low-severity,
test-only gaps** the validator recommended pinning later. They were
recorded in `.chug/LEDGER-spec9-archive.md` ("Carried, low-severity
test-only gaps from R4 validator, could seed TODO: Accept-header pin test,
tools/list-over-SSE framing test, timeout-constant pins") but never made
it into TODO.md — five cycles of carried debt. This row files them.

Relevant code (verify line numbers before editing — they drift):

- **Deadline constants** (`src/mcp_http.rs` ~:31-52): `INIT_TIMEOUT` 10s,
  `LIST_TIMEOUT` 10s, `CALL_TIMEOUT` 60s, `FIRST_BYTE_TIMEOUT` 30s,
  `CONNECT_TIMEOUT` 10s, `REPLY_TIMEOUT` 30s, `LISTEN_JOIN_GRACE` 5s.
  `HttpMcpServer` stores `first_byte_timeout` / `call_timeout` fields
  (~:72-75) initialized from the constants in the constructor (~:182-183)
  and overridable in tests.
- **Accept headers:** POST requests send `accept: application/json,
  text/event-stream` (~:572 and ~:941); the GET listen stream sends
  `accept: text/event-stream` (~:797). The response-side checks lowercase-
  contains `text/event-stream` (~:422, ~:870).
- The module already has a stub-server test harness (T6 hardened it:
  `accept_conn` helper with 10s accept deadline + read/write timeouts).
  New tests must reuse that harness — no new bare `accept()` calls.

## Requirements

Three pins, all in `src/mcp_http.rs`'s test module (no production-code
changes expected; if a pin reveals a real bug, STOP and report instead of
fixing):

1. **Accept-header pin.** Stub-server test(s) asserting the exact Accept
   header the client sends: `application/json, text/event-stream` on POSTs
   (initialize/tools-list/tools-call path — one representative POST
   suffices) and `text/event-stream` on the GET listen stream. The stub
   captures request headers and the test asserts equality (not substring).
2. **tools/list-over-SSE framing test.** A test where the server's
   tools/list reply is delivered framed as SSE (`Content-Type:
   text/event-stream`, `data: {...}\n\n`) rather than a plain JSON body,
   exercising the SSE-framed response path (~:422) for the list operation
   — assert the parsed tool list matches the payload. If the current
   implementation only supports SSE framing on some paths, pin the
   narrowest path that does (and note the limitation in a test comment).
3. **Timeout-constant pins.** Tests that fail if the constants change:
   at minimum `CALL_TIMEOUT == 60s`, `FIRST_BYTE_TIMEOUT == 30s`,
   `CONNECT_TIMEOUT == 10s` (the R4 blocker was a body-read timeout
   killing silent SSE — these numbers are the fix's surface). Prefer
   asserting the constructor's default field values
   (`first_byte_timeout`/`call_timeout`) plus direct constant references
   for the rest, so a drive-by "bump the timeout" edit breaks the build's
   test suite deliberately.

## Tests

The three pins ARE the tests. All must live behind the existing stub
harness (accept/read/write deadlines per T6) so a hung stub degrades to a
test failure, never a frozen suite.

## Acceptance

- `cargo build`, `cargo clippy --all-targets -- -D warnings`, `cargo test`
  green; suite wall-clock stays in the same ~10s envelope.
- Mutating any pinned constant or Accept header value fails the
  corresponding test (mutants die).
- No production-code behavior change (diff should be test-module-only
  unless a pin exposes a bug — then report, don't fix).

check: cd /Users/jadams/workspace/chug && cargo test
