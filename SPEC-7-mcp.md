# SPEC 7 — MCP client (stdio, tools only)

chug gains the ability to consume tools from MCP servers. v1 scope is
deliberately narrow: stdio transport, tools capability only (no resources,
prompts, sampling, roots, or HTTP/SSE transports).

check: cargo test

## Config

Claude Code-compatible JSON, discovered in this order (first found wins):

1. `--mcp-config <path>` flag (new, on `run` and `chat`)
2. `./mcp.json` (cwd)
3. `~/.config/chug/mcp.json`

```json
{"mcpServers": {"<name>": {"command": "...", "args": ["..."], "env": {"K": "V"}}}}
```

Absent config everywhere = MCP off, zero behavior change. `--mcp-off` flag
forces off even when config exists. Server names: `[a-z0-9-]` (validate).

## Protocol (MCP stdio transport)

Newline-delimited JSON-RPC 2.0 (NOT LSP-style Content-Length framing). Per
server, at run start:

1. Spawn `command args` with piped stdin/stdout; stderr appended to
   `<cwd>/.chug/mcp-<name>.log`. A reader thread per server demultiplexes
   responses by `id` into a pending-request map (`HashMap<u64, Sender>`);
   notifications (no id) are logged and dropped.
2. Send `initialize` (`protocolVersion: "2025-06-18"`, `capabilities: {}`,
   `clientInfo: {name: "chug", version}`), await result (10s timeout).
3. Send `notifications/initialized`.
4. `tools/list` (10s) → register each tool as `mcp__<name>__<tool>` with the
   server's `description` + `inputSchema` passed through verbatim.

If ANY step fails for a server: log to the mcp log + emit one event line,
continue WITHOUT that server (fail-soft — never abort the run).

## Dispatch

Tool-use for `mcp__<name>__<tool>` → `tools/call` with the input object as
`arguments`, 60s timeout. Result content blocks: `text` blocks concatenated
into the tool_result content; `isError: true` from the server maps to
`is_error: true`. Server died mid-run (reader EOF) → subsequent calls to its
tools return an error tool_result `mcp server <name> is down` (no respawn in
v1). MCP tool results count toward the stuck tripwire like any tool error.
MCP tools bypass the laya risk gate (it judges bash only) — note this in the
schema docs comment.

## Lifecycle

All servers spawn lazily at run/chat start (after config load), all killed
on ANY exit path (normal, abort, panic-guard path): kill process, reap. No
orphans — this matters (see the bash process-group bug history); use the
same process-group discipline as run_shell.

## Files

- `src/mcp.rs` — new: config load/discovery, `McpServer` (spawn, reader
  thread, request map, call), registry (`Vec<McpServer>` → tool schemas +
  dispatch table).
- `src/tools.rs` — merge MCP schemas into the tools array; route
  `mcp__`-prefixed names to the registry.
- `src/main.rs` — `--mcp-config`, `--mcp-off`.
- No new external crates beyond what's present (serde_json, std::process,
  std::sync). NO async.

## Tests (no network)

- Fake server: a `python3 -c` (or `sh`) script emitted to a tempdir by the
  test that speaks NDJSON: answers `initialize`, `tools/list` (one echo
  tool), `tools/call` (echoes arguments). Drives the full path: spawn →
  handshake → list → call → result text.
- Config discovery precedence (temp files); invalid server name rejected;
  malformed JSON → MCP off with warning, run continues.
- Dead server: start, kill -9 it, call its tool → error tool_result, run
  continues.
- Framing: response with a notification interleaved between request and
  result is handled (skipped, result still matched by id).

## Acceptance

- Gates clean: build / clippy -D warnings / test.
- With no mcp.json anywhere, `chug run` behaves byte-identically to before.
- With a fake echo server configured, a run can call `mcp__fake__echo` and
  the model sees the echoed content.
