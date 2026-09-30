# T171 — F11 phase 1b-iii: resources + prompts legs over the HTTP transport

check: export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared && cargo test --bin chug mcp

## Repo context

The `McpBackend` trait (src/mcp.rs:~200) carries `list_resources` /
`read_resource` (T162, stdio-implemented) and — once T170 lands —
`list_prompts` / `get_prompt`, all with default bails naming "phase 1b".
The HTTP transport (src/mcp_http.rs:1019) inherits those defaults: its
comment says resources are not spoken over HTTP yet and "F11 phase 1b
reuses the shared" mapping. The HTTP handshake already captures the same
`McpCapabilities` struct (T162 req 1: "in a shape the HTTP transport can
reuse later — no fork"). HTTP test style: hermetic stub servers in the
mcp_http.rs test module (T6 harness; framing helpers ~:1824-1990) with
the tools/list-over-SSE legs as the framing precedent. Roadmap:
FEATURES.md F11 phase 1b (third split — closes 1b; worked AFTER T170,
whose prompt types it reuses).

estimate: ~320 changed lines (4 protocol methods over HTTP+SSE ~120,
capability gating ~30, tests ~150, README ~15 — feature-row calibration
band).

## Requirements

1. **HTTP legs for all four methods**: the HTTP McpServer overrides
   `list_resources`, `read_resource`, `list_prompts`, `get_prompt` using
   the SAME shared response mapping the stdio transport uses (no fork —
   call the shared parsers), with the HTTP transport's existing session
   discipline (Mcp-Session-Id replay, JSON-RPC over POST, SSE stream read
   until the matching-id response).
2. **Capability-gated parity**: an HTTP server that did not advertise
   `resources` / `prompts` is never sent the corresponding list/get —
   same discipline as stdio (stub-side witness legs).
3. **Error parity**: unreachable server / HTTP error / JSON-RPC error
   reply surface as the SAME named-error shape the stdio legs produce
   (the T162 Req-3 contract) — the transport name appears, never a panic
   or hang; timeouts mirror the existing HTTP consts (connect 10s,
   first-byte 30s, per-call total 60s).
4. **Registry honesty**: `McpRegistry`'s aggregated outcomes for HTTP
   servers name the transport when an outcome differs (the T162
   structured per-server outcome type is reused as-is unless a real gap
   is found — named in the commit message).
5. **README**: the MCP section's Resources bullet + prompts line are
   updated — resources and prompts are consumed over BOTH stdio and
   streamable HTTP; only the model-facing prompts surface remains
   deferred (F11/F9 later phase). F11 phase 1b closes here — the
   FEATURES.md F11 entry's "phase 1b" note is updated by the
   ORCHESTRATOR at merge (row-flip commit), not by the child.

## Tests

In the mcp_http.rs test module (the `mcp` filter): (a) resources/list
over HTTP returns parsed resources (SSE framing); (b) resources/read
returns text contents; (c) prompts/list + prompts/get return parsed
prompts/messages (T170 types reused); (d) capability-gated: a stub
without the capability is never queried (witness); (e) error parity:
JSON-RPC error reply and connection-refused legs produce the named
errors; (f) a mixed-registry leg (stdio + HTTP stubs) aggregates
per-server outcomes naming each server. RED-prove (a) and (d) — state
the proofs in the commit message.

## Acceptance

- Spec check green; full `cargo test` green; clippy `-D` clean.
- `grep -n "phase 1b" src/mcp_http.rs` shows the inherited-default
  comment is gone or reworded (the legs are real now).
- `grep -rn "resources/list\|prompts/list" src/mcp_http.rs` shows the
  real legs (not only tests).

## Out of scope

- The model-facing prompts surface, resources/templates, subscriptions/
  notifications, changes to the stdio legs (T162/T170) or T169's tool,
  GET-listen-stream behavior changes.
