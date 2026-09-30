# T169 — F11 phase 1b-i: model-facing `mcp_resource` tool (list/read server resources)

check: export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared && cargo test --bin chug mcp && cargo test --test mcp_serve

## Repo context

F11 phase 1a (T162, merge 8120b00) built the consume legs:
`McpRegistry::list_resources()` / `read_resource(server, uri)`
(src/mcp.rs:630/654) returning structured per-server outcomes, the
`McpCapabilities` catalog parsed at initialize (src/mcp.rs:46), and
`MAX_MCP_RESOURCES = 200` warn-and-cap — all INTERNAL: no tool in
`tool_schemas()` (src/tools.rs) exposes resources to the model. Tool
dispatch: src/tools.rs:157+ (`delegate`, `web_fetch` schemas) and the
dispatch match at src/tools.rs:221; the driver holds
`mcp: &mut McpRegistry` through the dispatch path (src/driver.rs:780/857)
— mcp__ tool calls already route through it with the T90 permissions
check FIRST (src/tools.rs:1194) and the risk-gate posture for mcp__ tools
(README: MCP tools bypass the laya risk gate). Roadmap: FEATURES.md F11
phase 1b (first of three splits; T170 prompts legs, T171 HTTP legs
follow).

estimate: ~350 changed lines (tool schema + dispatch + registry wiring
~120, tests ~180, README ~20, permissions/risk-gate notes ~30 — the
cycle-79 eval's feature-row calibration band).

## Requirements

1. **One new builtin tool `mcp_resource`** with two actions:
   `{"action": "list", "server"?} ` returns the resource catalog (all
   servers when `server` is omitted, one server when given — names, uris,
   descriptions, mimeTypes as compact text); `{"action": "read",
   "server": <name>, "uri": <uri>}` returns the resource's contents
   (text contents inline; blob contents base64 with the mimeType named,
   size-capped consistent with existing tool-result limits). Missing
   params → corrective tool errors naming the missing field (T88
   pattern). Unknown action → tool error listing the two actions.
2. **Capability-gated honesty**: a server that did not advertise
   `resources`, an unknown server name, or a dead/unreachable server
   produces a named tool error (never a panic, never a hang — the T162
   LIST/CALL timeout consts); `list` on zero configured servers says so
   plainly. No-servers-configured (`--mcp-off`) → the tool still exists
   and returns the named error (the delegate/web_fetch posture).
3. **Policy chain placement**: the tool is a builtin (name
   `mcp_resource`, NOT an `mcp__`-prefixed server tool) — the T90
   deny-list can match `mcp_resource` explicitly; read-only, so it shares
   the mcp__ read posture on the laya risk gate (bypass) — state the
   chosen posture in the commit message and keep it consistent with how
   mcp__ tools are treated today.
4. **Result shape**: text the model can act on (one resource per line for
   list: `server uri — description (mimeType)`; read: the contents with a
   `server/uri` header line). Bounded: list output capped (the
   MAX_MCP_RESOURCES catalog is already bounded); read contents capped
   with a truncation note consistent with web_fetch's cap behavior.
5. **README**: the MCP section's Resources bullet is updated — the
   surface is no longer "internal for now": the `mcp_resource` tool is
   documented (list/read, capability-gated), and prompts + HTTP
   transport legs stay named as the remaining F11 phases.

## Tests

In the mcp test modules (the `mcp` filter catches src/mcp.rs +
src/mcp_http.rs; mcp_serve integration for the end-to-end leg if added):
(a) schema pin — `mcp_resource` appears in LIVE `tool_schemas()` output
with both actions documented (T22 precedent: assert the live schema, not
a copy); (b) list against a hermetic stub advertising resources →
parsed lines; (c) read text contents from a stub; (d) capability-gated:
a stub WITHOUT the resources capability → named error, and the stub
asserts it was NEVER queried; (e) unknown server / dead server → named
errors; (f) permissions: a `mcp_resource` deny rule blocks the call
before dispatch (T90 chain). RED-prove (b) and (d) at minimum — state
the proofs in the commit message.

## Acceptance

- Spec check green; full `cargo test` green; clippy `-D` clean.
- The tool works end to end against a stub server (the (b)/(c) tests).
- `grep -n "mcp_resource" README.md` shows the documented surface.

## Out of scope

- prompts/list+get (T170), HTTP transport legs (T171), resources/templates,
  subscriptions/notifications, chat-only surfaces (resource autocomplete),
  any mcp__ server-tool behavior change, changes to T162's registry method
  signatures (reuse them as-is; widen only if a signature gap is real and
  named in the commit message).
