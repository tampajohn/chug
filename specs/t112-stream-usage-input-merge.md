# T112 — streaming path reports input_tokens=0: merge message_delta usage fields

check: cargo test

## Repo context

F7 phase 1 (T108, cycle 59) made `"stream": true` the default request
shape. The `StreamAccumulator` (src/api.rs ~590-950) synthesizes the
non-streaming response body from SSE events so `Response::usage()` is
identical either way — T15 budget enforcement, budget-low token warnings,
events.jsonl iteration lines, abort/goal token summaries, and Langfuse
usage all ride `usage()` unchanged.

**Bug**: on the production endpoint (`tools-proxy.videoamp-internal.com`),
streamed runs report `input_tokens: 0` for the whole run. Evidence:

- `.chug/events-20260928-091512.jsonl` (cycle-60 orchestrator, post-T108
  binary): every iteration line `input_tokens: 0` (output counts
  normally). Same for `events-t110-impl-*` and `events-t110-validate-*`.
- Pre-T108 streams are nonzero (`events-20260928-080130.jsonl`, cycle-59
  orchestrator: `input_tokens: 763543` at n=149).
- Live probe (cycle-61 eval, `max_tokens: 1`, `stream: true` against the
  configured endpoint) shows the proxy defers real usage to
  `message_delta`:

  ```
  message_start: ... "usage":{"input_tokens":0,"output_tokens":0}
  message_delta: ... "usage":{"input_tokens":257,"output_tokens":1}
  ```

- Root cause at src/api.rs ~929-945: the `message_delta` accumulator leg
  merges ONLY `output_tokens` from the delta's usage object. The real
  Anthropic API carries `input_tokens` in `message_start` (where the
  accumulator reads it), so the parity tests pass against real-API-shaped
  fixtures — but this proxy sends placeholder zeros in `message_start`
  and the real counts (including `input_tokens`) in `message_delta`,
  which the accumulator discards.

Impact: `--max-tokens` under-counts massively on the default path (input
is ~95% of agentic-loop tokens), the budget-low token leg never fires
with a real count, events.jsonl loses the input-context curve the eval
digest reads, and abort/goal token summaries print `0 in`. Interim
workaround exists (`CHUG_STREAM=0` — the non-streaming path reports real
usage), but the default path must be fixed.

estimate: ~80 changed lines (≈15 production + ≈60 tests + comments)

## Requirements

1. In `StreamAccumulator`'s `message_delta` leg, when the delta's `usage`
   object carries `input_tokens`, `cache_read_input_tokens`, or
   `cache_creation_input_tokens`, merge each present field over the
   accumulated usage skeleton (delta wins — latest is freshest).
   `output_tokens` behavior is unchanged (delta value overwrites).
2. `message_start` handling is unchanged. When a `message_delta` carries
   none of the input-side fields, behavior is byte-identical to today —
   on the real Anthropic API `message_delta.usage` carries only
   `output_tokens`, so the merge is a no-op there and the existing T108
   parity pins stay green UNTOUCHED.
3. No call-site changes: everything downstream keeps consuming
   `Response::usage()` as today (the fix is confined to the
   accumulator).

## Tests

4. **Proxy-shape fixture** (the regression pin): a stream whose
   `message_start` usage is `{"input_tokens":0,"output_tokens":0}` and
   whose `message_delta` usage is `{"input_tokens":257,"output_tokens":1}`
   — the exact observed production shape, cited in a comment — yields
   `usage().input == 257`, `usage().output == 1`.
5. **Cache-fields leg**: a `message_delta` carrying
   `cache_read_input_tokens` / `cache_creation_input_tokens` merges them
   over the skeleton (values asserted via `usage()`'s cache fields).
6. **No-input-in-delta leg**: a `message_start` with a real input count
   plus a `message_delta` carrying only `output_tokens` keeps the
   `message_start` input (today's real-API behavior — the no-op leg).

## Acceptance

- `cargo test` green including the three new legs; clippy clean.
- Existing T108 usage/parity pins pass UNMODIFIED (the fix is additive).
- Orchestrator-side live verification at review (not a check: leg):
  post-merge, a fresh run's `.chug/events.jsonl` iteration lines show
  `input_tokens > 0` (pre-fix streams read 0 — the evidence list above).

## Out of scope

- Proxy-side fixes (the proxy could send real usage in `message_start`;
  chug must be correct against what it actually sends).
- Token ESTIMATION when an endpoint sends no usage anywhere (no such
  endpoint observed; YAGNI until one appears).
