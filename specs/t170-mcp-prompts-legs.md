# T170 — F11 phase 1b-ii: prompts/list + prompts/get consume legs (stdio)

check: export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-impl-a && cargo test --bin chug mcp

## Repo context

Mirrors T162's resources shape for the prompts capability. The MCP client
lives in src/mcp.rs (McpRegistry / McpServer stdio JSON-RPC; the
`McpBackend` trait at ~:200 carries `capabilities()` plus the resources
legs with phase-1b default bails) — the `McpCapabilities` catalog
(src/mcp.rs:46) already parses advertised capabilities; today `prompts`
presence is captured but never listed or gotten (grep: no `prompts/list`
in src/). Test style: hermetic stub servers in the mcp.rs / mcp_http.rs
test modules (T6 harness). `MAX_MCP_TOOLS` / `MAX_MCP_RESOURCES = 200`
(src/mcp.rs:36) are the warn-and-cap precedents. Roadmap: FEATURES.md F11
phase 1b (second split; T169 landed the model surface for resources,
T171 wires the HTTP transport for resources+prompts).

estimate: ~300 changed lines (protocol legs + types ~100, registry
surface ~40, tests ~140, README ~15 — feature-row calibration band).

## Requirements

1. **Protocol legs (stdio)**: `prompts/list` (same session/framing
   discipline as tools/list + resources/list) and `prompts/get`
   (name + optional arguments map; map the response — description +
   messages array with role/content — into internal
   McpPrompt / McpPromptMessage types). Bound advertised prompts with
   `MAX_MCP_PROMPTS` (mirror the MAX_MCP_RESOURCES pattern, same
   warn-and-cap semantics, same per-server .chug log line).
2. **Capability-gated**: a server that did not advertise `prompts` is
   never sent prompts/list or prompts/get (the T162 resources discipline;
   the stub asserts zero such requests).
3. **Registry surface (internal, no model tool yet)**:
   `McpRegistry::list_prompts()` / `get_prompt(server, name, args)`
   returning structured per-server outcomes where non-capable/down
   servers are named errors, never panics or hangs (timeouts reuse the
   existing LIST/CALL consts — the T162 list_resources/read_resource
   shape). The legs join the shared `McpBackend` trait with default
   "phase 1b" bails so the stdio transport overrides now and the HTTP
   transport overrides in T171 (no signature fork).
4. **Types placed for reuse**: the prompt/argument/message mapping lives
   where the HTTP transport can call it in T171 (the T162 "shared
   response-mapping" precedent — no fork between transports).
5. **README**: the MCP section gains one honest line: prompts are
   consumed (list/get) from servers that advertise them; a model-facing
   prompts surface (slash-pack surfacing) is a later F11/F9 phase.

## Tests

In the mcp.rs test module (the `mcp` filter): (a) capability captured —
prompts present/absent at initialize; (b) list returns parsed prompts,
capped at MAX_MCP_PROMPTS with the warning; (c) get returns mapped
messages (roles + content) for a stub prompt with arguments; (d) get of
an unknown prompt / dead server → named error; (e) a server WITHOUT the
capability is never sent prompts/list (stub-side witness). RED-prove
(b)-uncapped and (e)-always-query — state the proofs in the commit
message.

## Acceptance

- Spec check green; full `cargo test` green; clippy `-D` clean.
- `grep -rn "prompts/list\|prompts/get" src/` shows the real legs (not
  only tests).

## Out of scope

- Any model-facing prompts surface (slash commands from prompts — a
  later phase riding F9), HTTP transport legs (T171), prompt argument
  completion, changes to T162's resources legs or T169's tool.
