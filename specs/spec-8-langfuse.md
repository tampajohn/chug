# SPEC 8 — Langfuse observability (optional, zero-cost when off)

Send run telemetry to a self-hosted Langfuse v3. OFF unless configured;
when off there is no code-path cost and zero behavior change.

check: cargo test

## Config (all optional; ANY missing piece = observability off)

- `LANGFUSE_HOST` (e.g. `http://127.0.0.1:3002`), `LANGFUSE_PUBLIC_KEY`,
  `LANGFUSE_SECRET_KEY` from process env.
- Fallback files (first found): `~/.langfuse-keys-chug` (dedicated chug
  project — PREFERRED), then `~/.langfuse-keys`. Parse `pk-lf-…` and
  `sk-lf-…` tokens, and `LANGFUSE_HOST=` line if present (env still wins).
- Never log keys; never fail a run because observability is misconfigured —
  a one-line stderr note and OFF.

## What to emit (batched ingestion)

`POST {HOST}/api/public/ingestion` — Basic auth `pk:sk`, body
`{"batch": [ …typed events… ], "metadata": {}}`. Event types:
`trace-create`, `generation-create`, `span-create`, `event-create`,
`score-create`. Each item: `{id, timestamp, type, body}`.

- **One trace per run/chat session**: id `chug-{shortid}`, name = goal
  (truncated 80), `metadata: {model, cwd, mode: run|chat, spec_file}`,
  `tags: ["chug", model]`, `sessionId` = trace id.
- **One generation per LLM call**: trace-linked; `model`, `modelParameters:
  {maxTokens}`, `usage: {input, output, total, cache_read_input_tokens}` from
  the API response `usage` (map `cache_read_input_tokens` when present),
  `startTime`/`endTime` (latency), `metadata: {iteration, stop_reason}`.
- **One span per tool call**: name, start/end, `metadata: {ok, is_error}`.
- **Events**: goal accepted / rejected (with reason), abort (reason),
  risk-gate verdicts, operator steering notes.
- **One score per finished run**: `outcome` CATEGORICAL
  (`completed|aborted|budget|stuck`), plus NUMERIC `iterations`.

## Transport discipline (load-bearing)

- **Fire-and-forget**: a bounded channel (cap 1000 events; full = drop with a
  dropped-counter, never block the driver) + a flusher thread batching every
  2s or 50 events, 5s HTTP timeout.
- **Fail-open always**: any HTTP error → count, log once per run, keep
  running. Observability must NEVER change run behavior.
- **Flush on exit**: normal end, abort, and the panic-guard path all drain
  the queue (best effort, 5s cap) before process exit.

## Files

- `src/observ.rs` — new: config resolution, `Sink` (channel + flusher +
  batching + basic auth), event builders, `NoopSink` when off. Global
  `OnceLock` accessor like auth.
- `src/api.rs` — after each LLM response, hand `(model, usage, latency,
  stop_reason, iteration)` to the sink.
- `src/driver.rs` — trace create/finish, tool spans, outcome score, events.
- No new async; `reqwest::blocking` in the flusher thread only.

## Tests (no network)

- Config: env-only, file-only, missing → off; malformed file → off + note.
- Event builders produce the exact ingestion JSON shapes (golden).
- Batch flusher: fake `Sink`-transport trait impl counting sends; batches at
  50, flushes at 2s, drops when full, exit-drain delivers remainder.
- api.rs emits one generation per response with usage mapped incl.
  `cache_read_input_tokens`.

## Acceptance

- Gates clean.
- Default (no env): `chug run` behaves byte-identically to before.
- With a fake ingestion endpoint (tiny rust `TcpListener` in a test or a
  python one-liner): a 2-iteration run produces 1 trace, ≥2 generations, ≥1
  span, 1 score — assert the batch bodies.
