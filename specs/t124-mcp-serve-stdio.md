# T124 — F10 phase 1: `chug mcp-serve` stdio MCP server skeleton + `chug_status`

check: cargo test

## Repo context

FEATURES.md F10 ("chug as MCP server") is the top unworked roadmap
item (Tier 3; every Tier 1/2 item is landed or deferred with a
standing written reason — re-verified cycle-64 eval). Today chug is an
MCP CLIENT only: `src/mcp.rs` (1,304 lines) spawns/consumes stdio +
streamable-HTTP servers; `src/mcp_http.rs` is the HTTP transport.
Nothing exposes chug TO another agent — Claude Code, the bridge fleet,
or a second chug cannot launch or even OBSERVE a chug run except by
shelling out and mining `.chug/` by hand. F10 is the fleet primitive
that closes that.

The client side already fixes the protocol shape: newline-delimited
JSON-RPC 2.0 on stdio (one object per line — `src/mcp.rs:600` writes
"newline to mcp stdin"), `PROTOCOL_VERSION = "2025-06-18"`
(`src/mcp.rs:17`), handshake `initialize` → `notifications/initialized`
→ `tools/list` (`src/mcp.rs:628`), tool results shaped
`{"content": [{"type": "text", "text": …}], "isError": bool}`. The
python echo-server fake in `src/mcp.rs:688` (`echo_server_body`) is the
exact server-side shape this item implements in Rust.

F10 is SPLIT (cycle-64 eval, per the working-rules shrink clause):

- **Phase 1 (this row)** — `chug mcp-serve` subcommand: the stdio
  server skeleton (framing, handshake, dispatch, error taxonomy,
  stdout purity, EOF lifecycle) plus ONE read-only tool, `chug_status`
  (events summary of any chug cwd). Read-only by design: no process
  spawning, no writes anywhere.
- Phase 2 (deferred, filed after phase 1 lands): `chug_collect`
  (delegate.rs's collect read path) + `chug_launch` (the write leg —
  delegate-launch parity, permissions/hooks interaction must be
  thought through).
- Phase 3 (deferred): server log file, `tools/listChanged`
  notifications, cancellation, resources.

Read-side seams to reuse (do NOT re-implement): `src/delegate.rs`
`read_events` (:470) → `summarize_events` (:618) produce the
`DelegateSummary` the orchestrator's `delegate status` renders. Both
are private today; this item makes the minimal `pub(crate)`
visibility changes and adds a COMPACT renderer for MCP output — the
existing `render_status` (:832) is pinned by delegate tests and must
not change behavior.

estimate: ~485 changed lines (≈200 production: new `src/mcp_serve.rs`
~170 incl. compact summary renderer + `src/main.rs` ~15 +
`src/delegate.rs` visibility ~5; ≈250 tests: bin-internal unit legs
~170 + one `tests/mcp_serve.rs` end-to-end spawn ~80; ≈35 README).
Under the ~500 filing ceiling BY the split — phase 1+2 together
estimated ~750+ and would have forced it.

## Requirements

1. **`chug mcp-serve` subcommand** (clap, beside Run/Plan/Fork/Chat in
   `src/main.rs`): runs the server on stdio until stdin EOF, then
   exits 0. No banner, no ledger, no driver lock, no events.jsonl
   writes — stdout carries ONLY protocol messages (a stdio MCP
   server's stdout IS the wire; any stray byte corrupts framing).
   Diagnostics, if any, go to stderr.
2. **Framing**: newline-delimited JSON-RPC 2.0 — one request object
   per stdin line, one response object per stdout line. Blank lines
   skipped. Serial dispatch (read-only tools are fast; no
   concurrency in phase 1).
3. **Methods**: `initialize` → result with `protocolVersion:
   "2025-06-18"` (reuse the `PROTOCOL_VERSION` const — export it
   `pub(crate)` from mcp.rs rather than duplicating the literal),
   `capabilities: {"tools": {}}`, `serverInfo: {"name": "chug",
   "version": <the binary's version>}`. `ping` → `{}`. `tools/list` →
   the single `chug_status` schema. `tools/call` → dispatch.
   `notifications/initialized` (and any `notifications/*`) → NO
   response (notifications never get one).
4. **Error taxonomy** (JSON-RPC standard codes): line fails to parse
   as JSON → `-32700` with `"id": null`; unknown method on a request
   (has id) → `-32601`; `tools/call` naming an unlisted tool →
   `-32602`; a request missing `method` → `-32600`. Errors never kill
   the loop — the server keeps reading.
5. **`chug_status` tool**: input `{"cwd": "<absolute path>"}`
   (required, must be absolute, must exist, must contain `.chug/` —
   each violation is a tool result with `isError: true` and text
   naming the received path verbatim, the delegate-launch fail-fast
   shape). Output: one text content block with a compact,
   self-describing summary of `<cwd>/.chug/events.jsonl`'s LATEST run
   segment (state, last_iteration vs max_iters, goal/abort/budget-low
   flags with the abort reason when present) built on the
   `read_events`/`summarize_events` seams — missing/unreadable events
   file → `isError: true` text naming the path, never a panic.
6. **Refactor discipline**: delegate.rs changes are visibility-only
   (`pub(crate)`) plus the additive seam(s) the renderer needs;
   `render_status`, `summarize_events`, `read_events` behavior
   byte-identical (existing delegate tests stay green unmodified —
   a needed edit to an existing delegate test is a smell to surface,
   not make silently).
7. **README**: the MCP section gains a `chug as MCP server
   (chug mcp-serve)` subsection — phase-1 scope (one read-only
   `chug_status`, phases 2–3 named as deferred), the Claude
   Code-compatible client config (`{"mcpServers": {"chug":
   {"command": "chug", "args": ["mcp-serve"]}}}`), and the
   stdout-purity rule (never run it expecting chatty output — the
   wire is the stdout). The Development layout line gains
   `mcp_serve.rs` (the `tests/readme_layout.rs` pin parses that line —
   update in the same commit).
8. **FEATURES.md**: the F10 row gains the SPLIT annotation with
   phase-1 → T124 (the orchestrator checks the box language at merge
   per the working rules — the child edits only if the spec's repo
   context section is stale; row flips stay the orchestrator's).

## Tests

Bin-internal (`src/mcp_serve.rs` test module), each a pure
`handle_message(&str) -> Option<String>`-style leg (no real stdio):

- `initialize` result shape: protocolVersion `2025-06-18`,
  serverInfo.name `chug`, capabilities.tools present; response id
  echoes the request id.
- `notifications/initialized` and an arbitrary `notifications/foo`
  produce NO response (None).
- `ping` → `{}` result; `tools/list` → exactly one tool named
  `chug_status` whose inputSchema requires `cwd`.
- Parse error → `-32700` with id null; unknown method → `-32601`;
  `tools/call` unknown tool → `-32602`; request missing `method` →
  `-32600`; a notification with an unknown method still gets NO
  response.
- `chug_status` happy path: tempdir with a synthetic
  `.chug/events.jsonl` (a run_start + a few iteration lines + a goal
  line) → `isError` absent/false, text names the state and iteration
  counts; missing `.chug` / missing events file / relative `cwd` /
  nonexistent `cwd` → `isError: true` naming the received path; a
  malformed final line does not panic (best-effort summary or a
  named-degrade note — pin whichever the implementation chooses).
- Stdout purity, structural: `mcp-serve`'s code path never calls the
  banner/`run_start` writers (pin by construction — the subcommand
  dispatches straight to `mcp_serve::serve`; a comment + a test that
  greps the module for `println!` outside the protocol writer).
- EOF: feeding no lines / closing input exits cleanly (unit-shape:
  the serve loop returns Ok on EOF).

Integration (`tests/mcp_serve.rs`, one end-to-end leg): spawn
`CARGO_BIN_EXE_chug mcp-serve` with piped stdio; write `initialize` →
`notifications/initialized` → `tools/list` → `tools/call chug_status`
against a tempdir fixture `.chug/`; read the four responses in order;
assert the handshake shape, the tool listing, and a summary text
naming the fixture's state; close stdin and assert exit success.
Bounded reads (deadlines like the T6 stub harness — a wedged server
must fail the test in seconds, not hang the suite).

## Acceptance

- `cargo test` green (the spec touches README + tests/ + src — plain
  `cargo test`, never `--bin chug` alone: the readme_layout pin and
  the new integration binary must both run).
- `cargo clippy --all-targets -- -D warnings` clean.
- Manual smoke shape (the e2e test IS this): a Claude
  Code-compatible stdio client config can complete the handshake and
  call `chug_status`.

## Out of scope

`chug_collect`, `chug_launch`, any write/spawn tool (phase 2);
notifications beyond the no-reply rule, `tools/listChanged`,
cancellation, resources/prompts capabilities (phase 3); server log
file (phase 2 candidate); HTTP transport for the server side;
changes to the MCP CLIENT (mcp.rs/mcp_http.rs behavior untouched
beyond the `PROTOCOL_VERSION` export).
