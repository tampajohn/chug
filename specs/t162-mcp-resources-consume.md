# T162 — MCP resources consume legs, phase 1a (F11): resources/list + resources/read on stdio

check: export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared && cargo test --bin chug mcp

## Repo context

The MCP client lives in `src/mcp.rs` (McpRegistry / McpServer, stdio
JSON-RPC, `call()` at ~:95, handshake in `start()` ~:300) and
`src/mcp_http.rs` (HTTP+SSE transport; handshake advertises
`"capabilities": {}` then `tools/list` ~:221-243, `tools/call` ~:283).
Today chug consumes TOOLS only — `resources` and `prompts` server
capabilities are never listed or called (grep: no `resources/list` anywhere
in src/). Test style: hermetic stub servers in the mcp_http.rs test module
(T6 harness; framing helpers around :1824-1990) and mcp.rs unit tests.
`MAX_MCP_TOOLS` caps tool advertisements (mcp_http.rs:243) — resources
need the same bound. Roadmap: FEATURES.md F11.

estimate: ~350 changed lines (src/mcp.rs ~120 protocol + capability
catalog, shared response-mapping ~40, tests ~160, README ~20).

## Requirements

1. **Capability catalog**: at initialize, parse and store the server's
   advertised capabilities (`resources`, `prompts` presence + any
   `listChanged` flags) on the stdio McpServer (and in a shape the HTTP
   transport can reuse later — a small shared struct, not a fork). A
   server advertising no `resources` capability is never queried for them.
2. **Protocol legs (stdio)**: `resources/list` (with the same
   session/framing discipline as tools/list) and `resources/read` (uri
   parameter; map the response — text and blob contents — into an
   internal Resource/ResourceContents type). Bound advertised resources
   with `MAX_MCP_RESOURCES` (mirror the MAX_MCP_TOOLS pattern, same
   warn-and-cap semantics).
3. **Registry surface (internal, no model tool yet)**: McpRegistry gains
   `list_resources()` / `read_resource(server, uri)` methods returning
   structured results with errors that name the server; unreachable or
   non-capable servers degrade to a named error, never a panic or hang
   (timeouts reuse the existing LIST/CALL timeout consts).
4. **Tests**: hermetic stub stdio/HTTP-style servers following the
   existing test-module patterns: (a) capability captured when advertised,
   absent when not; (b) list returns parsed resources, capped at
   MAX_MCP_RESOURCES with the warning; (c) read returns text contents;
   (d) read of an unknown uri / dead server produces the named error;
   (e) a server WITHOUT the capability is never sent resources/list
   (assert at the stub: zero such requests seen).
5. **README**: the `MCP servers (mcp.json)` section gains one honest
   bullet: chug consumes tools AND reads resources (list/read) from
   servers that advertise them; prompts + the model-facing resource
   surface are a later phase (F11 phasing stated).

## Tests

All in `src/mcp.rs` / `src/mcp_http.rs` test modules (the check filter
`mcp` catches both). RED-prove at least (b)-uncapped and (e)-always-query
mutants; state them in the commit message.

## Acceptance

- The spec check green; full `cargo test` green; clippy `-D` clean.
- `grep -rn "resources/list" src/` shows the real legs (not only tests).

## Out of scope

- Any model-facing tool/prompt surface, resources/templates,
  subscriptions/notifications, prompts/list+get, HTTP-transport resource
  legs (phase 1b — reuse the mapping), changes to tools/call semantics.
