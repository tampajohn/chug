# T108 — F7 phase 1: streaming responses + console text deltas

check: cargo test --bin chug

## Repo context

FEATURES.md F7 (Streaming UX) — the cycle-59 mandatory roadmap pull
(Tier 2 top unworked after F6-p1 landed cycle 58): "Text deltas to sinks
as they arrive (TUI live typing, headless progress); watchdog gets
byte-level liveness for free." SPLIT per the working rules; **this spec
is phase 1** (transport streaming + accumulation + the console/headless
delta surface); phase 2 (TUI live typing) is deferred with a written
reason in EVALUATION.md cycle-59 §4.

The loop consumer is real: `delegate` children are headless — during a
long single generation (t104-impl: 74.7k output tokens in one run;
individual big write_file generations run minutes) the child's
`.chug/delegate.log` shows NOTHING between `tool` lines, and the
orchestrator's `delegate status` log tail has no liveness evidence.
Phase 1 makes the console stream live: model text appears as it
arrives.

Grounded architecture (verified at filing):
- `Client::complete` (src/api.rs:687) is explicitly "One non-streaming
  Messages API call" (api.rs:684 doc comment) — the request body has no
  `stream` field; the whole response body is read, then parsed.
- `Transport` (api.rs:293) is `pub(crate)` with in-crate impls only
  (ReqwestTransport + test fakes) — trait extension is cheap.
- `read_body_with_watchdog` (api.rs:397) spawns the reader thread; its
  `rx.recv_timeout` loop runs ON THE CALLER'S THREAD (the thread
  blocked in `complete()` = the driver thread) and re-arms the T2
  watchdog per `BodyMsg::Progress` — a chunk hook invoked in that loop
  needs NO cross-thread synchronization, and the watchdog's byte-level
  liveness (F7's "free" benefit) ALREADY exists there. T74's margin
  pins (api.rs:1209+) must stay byte-identical.
- `src/sse.rs` already ships `SseParser::feed(&str) -> Vec<SseEvent>`
  — incremental, chunk-boundary-safe (internal carry buffer), unit-
  tested (mcp_http.rs's transport parses SSE with it today).
- `Event` (src/events.rs) is matched exhaustively at: events.rs
  `ConsoleSink::emit` (:228), eventlog.rs `EventlogSink`, tui.rs
  `apply` (:222), plus observ/riskgate-adjacent selective matches —
  a new variant is COMPILE-ENFORCED at every site.
- `Event::ModelText` is NOT written to events.jsonl (grep-verified: 0
  lines in the cycle-58 stream) — the console/TUI-only precedent for
  model text already exists.
- The driver calls `llm.complete()` through `&mut dyn Llm`
  (ScriptedLlm scripts full Responses — no network, no Transport).

Anthropic Messages SSE shapes (for the accumulator): `message_start`
(message skeleton + usage.input_tokens incl. cache fields),
`content_block_start` (block skeleton: text or tool_use with empty
input), `content_block_delta` (`text_delta.text` /
`input_json_delta.partial_json` string concatenation),
`content_block_stop`, `message_delta` (delta.stop_reason +
usage.output_tokens), `message_stop`, `ping`, `error`.

## Requirements

1. **Streaming request leg.** `Client::complete` sends `"stream": true`
   by default. Kill switch: `CHUG_STREAM=0` (process env) restores the
   byte-identical non-streaming request body AND parse path (both legs
   pinned against a request-body-capturing fake transport). README
   Tools or run-mode section documents the knob + the live-progress
   behavior, integrated in place.
2. **Incremental chunk hook.** `Transport` gains
   `send_with_progress(url, headers, body, on_chunk)` with a DEFAULT
   implementation that ignores the hook and delegates to `send`
   (existing fakes compile unchanged, byte-identical behavior);
   `ReqwestTransport` overrides it, passing the hook into the body
   read. `read_body_with_watchdog`'s `BodyMsg::Progress` gains the
   chunk bytes and the caller-thread receive loop invokes
   `on_chunk(&bytes)` when present. The existing 2-arg watchdog
   function keeps its signature as a thin wrapper (T74's pins
   untouched); the activity-timeout/retry classification is unchanged
   (a stalled SSE stream aborts at the SAME activity timeout — pinned).
3. **SSE accumulation → identical Response.** One `StreamAccumulator`
   in api.rs: fed chunks (incrementally via the hook when the transport
   supports it; once over the full body otherwise — BOTH legs tested,
   identical results), it drives `sse::SseParser`, emits text deltas
   to the delta hook as `text_delta` events arrive, and at
   `message_stop`/end-of-body synthesizes the NON-STREAMING body shape
   (content blocks incl. tool_use inputs from concatenated
   `input_json_delta` partials, stop_reason, usage) so the EXISTING
   Response parse path produces a Response byte-identical in contract:
   `content_blocks()`, `stop_reason()`, and `usage()` equal the
   non-streaming values for the same exchange (T15 budget enforcement
   untouched — usage equality pinned explicitly, incl. cache fields
   when present).
4. **Fallback leg.** Streaming requested but the response is not SSE
   (content-type not `text/event-stream` — a proxy downgrade): parse
   byte-identically to today and record ONE bounded
   `stream_fallback` line in events.jsonl, first-per-run latched
   (bounded telemetry for the tools-proxy compatibility question; no
   per-response spam).
5. **Delta surface.** The `Llm` trait gains
   `set_text_delta_hook(Option<Box<dyn FnMut(&str) + Send>>)` with a
   default NO-OP (ScriptedLlm untouched); `Client` overrides. The
   driver sets the hook before `complete()` to emit a NEW
   `Event::ModelTextDelta(String)` into its sink, and clears it after
   the call returns. Sink arms: `ConsoleSink` renders deltas live to
   stderr (raw incremental writes under a one-time `[chug] model: `
   prefix, terminated by a newline when the response completes — the
   existing per-response `ModelText` preview line is SUPPRESSED for a
   streamed response so text never double-prints; a non-streamed
   response keeps today's byte-identical preview line — both legs
   pinned); TUI `apply` arm is a phase-1 NO-OP with a comment naming
   phase 2 (the completing `ModelText` still lands in the activity
   panel exactly as today — pinned); the eventlog sink does NOT write
   delta lines (ModelText precedent — events.jsonl stays small; the
   `stream_fallback` line of req 4 is the only new eventlog surface);
   any other exhaustive match gains its deliberate arm.
6. **Error legs.** An SSE `error` event → `TransportError::Connection`
   (T1-retryable) naming the error type; a malformed `data:` payload
   classifies the SAME way today's unparseable-body leg classifies
   (name the parity, whatever it is); mid-stream connection failure
   retries per T1 exactly as a mid-body failure does today. No
   half-accumulated Response ever escapes: a failed attempt discards
   the accumulator (deltas already printed are console cosmetics only,
   never transcript state — the transcript only ever sees the final
   accumulated Response).
7. **ScriptedLlm / non-streaming surfaces byte-identical.** All
   existing tests stay green UNMODIFIED except: arms added for the new
   Event variant, and pins that assert the request body verbatim gain
   the `stream` field on the streaming leg (each such pin edit named
   in the commit message with its RED-then-green run).

## Tests

- Accumulator unit tests: text-only multi-chunk; tool_use via
  split `input_json_delta` partials; text+tool_use interleaved;
  usage equality incl. cache fields; stop_reason passthrough;
  chunk-boundary splits at every byte of a fixture (the sse.rs
  parser_feed_split_across_chunks pattern).
- Delta hook: receives the text deltas in order; `ModelText` still
  carries the full text; hook cleared after the call.
- Request legs: streaming body carries `"stream":true`; `CHUG_STREAM=0`
  body byte-identical to pre-change (fake transport captures the body).
- Fallback leg: stream requested, JSON body + `application/json`
  content-type returned → identical Response + one latched
  `stream_fallback` events.jsonl line.
- Stub-harness (T6 discipline) live legs: chunked SSE fixture with
  inter-chunk delays → deltas arrive BEFORE the body completes
  (ordering pinned, no wall-clock margin games — T74 doctrine:
  mechanism, not timeouts); stall fixture → activity-timeout abort
  (existing watchdog margins untouched); SSE `error` event → retry
  then scripted success (T1 retry path through a streamed attempt).
- Console sink: streamed response prints the prefix + raw deltas +
  terminating newline and SUPPRESSES the preview line; non-streamed
  response prints today's preview line byte-identically; eventlog
  silence pin (no delta lines); TUI no-op arm pin (apply of a delta
  leaves state unchanged).
- RED-proofs recorded in the commit message: accumulator-dropped-delta
  mutant dies; preview-suppression-reverted mutant dies; fallback-latch
  mutant dies.

## Acceptance

- `check:` green in the worktree (whole suite — the Event variant +
  api.rs legs are broad-surface).
- `cargo clippy --all-targets -- -D warnings` clean.
- Live smoke (child runs it): a real `chug run` or `chug chat` turn
  against the configured endpoint shows progressive `[chug] model: `
  text in the console output — or, if the endpoint downgrades, the
  `stream_fallback` line appears and behavior is byte-identical to
  today. Result recorded in the child's ledger.
- Diff confined to: src/api.rs, src/events.rs, src/eventlog.rs,
  src/driver.rs, src/tui.rs (+ its test arm), src/chat.rs ONLY if its
  sink arms require it, README.md (req 1 doc), and NO other files.
- Validation: api.rs + driver.rs + events.rs are ALL on the LOOP-SPEC
  §2 step-4 REQUIRED list → kimi REQUIRED; mutation legs named:
  accumulator delta-drop, usage-field swap, preview-suppression
  removal, fallback-latch removal, kill-switch removal.
- Out of scope (phase 2, deferred): TUI live typing / progressive
  tool-input rendering (rides the tui.rs redraw architecture; chat-only
  surface with no loop consumer), any chat-specific surface.
