# SPEC 9 — MCP HTTP/SSE transport

Extend the SPEC-7 MCP client with remote transports so chug can consume
HTTP-hosted MCP servers. v1 scope: **streamable HTTP transport + static
bearer auth + reconnect**. OAuth flows are explicitly OUT (users paste a
token into config).

check: cargo test

## Config (extends SPEC-7's mcp.json)

```json
{"mcpServers": {
  "local-thing": {"command": "...", "args": ["..."]},
  "remote-thing": {
    "url": "https://mcp.example.com/mcp",
    "transport": "http",
    "headers": {"Authorization": "Bearer ${REMOTE_TOKEN}"}
  }
}}
```

- An entry with `url` is remote; `transport` may be `"http"` (default when
  `url` present) — no other values in v1.
- `${VAR}` in header values expands from process env; missing var → config
  error for that server (fail-soft: skip it, note in the log, continue).
- Server names get the same `mcp__<name>__<tool>` prefix; dispatch is
  transport-agnostic (registry routes by name, transport is per-server).

## Streamable HTTP transport (v1 semantics)

- **Requests**: `POST {url}` with the JSON-RPC message; body
  `application/json`; `Accept: application/json, text/event-stream`. A 200
  with JSON body = the response. A 202 with no body = accepted (used for
  notifications/responses-only messages).
- **Responses may be SSE**: when the POST returns `text/event-stream`, read
  `data:` lines until the matching-id response arrives (then the stream may
  be closed by the server; that's fine).
- **Notifications**: if the server sends requests/notifications over an SSE
  stream (from a POST response stream or a GET to the same endpoint), reply
  to server→client requests with JSON-RPC `method not found` error (same
  rule as stdio) and drop notifications.
- **Session headers**: if a response carries `Mcp-Session-Id`, send it on
  every subsequent request for that server. Honor `Last-Event-ID` on SSE
  reconnect when the server supplies ids.
- **Reconnect**: a dropped SSE stream retries with exponential backoff
  (1s→30s cap, jittered) for the life of the run; a POST that fails
  connection-level retries 3× (1s,2s,4s) then returns a tool error
  (fail-soft, same contract as stdio's dead server).
- **Timeouts**: connect 10s, first-byte 30s, per-call total 60s (same as
  stdio calls).
- **HTTP client**: reuse the existing `reqwest::blocking` style — a reader
  thread per remote server for SSE parsing; no async. SSE parsing is
  line-based (`event:`/`data:`/`id:`), UTF-8, tolerate comments (`:`).

## Lifecycle & gates

- Remote servers "spawn" at run start like stdio ones: initialize handshake
  (10s) → `notifications/initialized` → `tools/list` → register. Any failure
  = skip server with a log line, never abort the run.
- Shutdown: close streams, join reader threads with the same bounded-wait
  discipline as the stdio Drop fix (no unbounded joins).
- Risk gate: remote MCP tools bypass it (same as stdio MCP).

## Tests (no network beyond 127.0.0.1)

- Tiny local HTTP stub (python http.server or a rust TcpListener test
  helper) serving: initialize handshake, tools/list over JSON and over an
  SSE-framed response, tools/call, a server→client request expecting
  `method not found`, session-id echo assertion.
- `${VAR}` expansion (present + missing).
- SSE parser: multi-line data, comments, id lines, CRLF.
- Reconnect backoff schedule unit test (no real sleeping: inject clock).
- Dead-server fail-soft (connection refused → tool error, run continues).
- No async. SSE parsing is line-based (`event:`/`data:`/`id:`), UTF-8,
  tolerate comments (`:`).

## Acceptance

- Gates clean: build / clippy -D warnings / test.
- A remote stub server configured in mcp.json: its tools appear as
  `mcp__remote__*` and a chug run can call one end-to-end.
- stdio behavior byte-identical to before.
