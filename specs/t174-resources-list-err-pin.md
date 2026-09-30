# T174 — Pin the resources/list error-reply message surface (T162 informational survivor)

check: export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-impl-a && cargo test --bin chug mcp

## Repo context

T162's kimi verdict (cycle 77, d1790766799-4) carried ONE informational
survivor: the list-err-drop mutant — no test pins the `resources/list`
error-reply message surface. The fallback error still names the server,
so the Req-3 named-error contract held; the verdict said "a future eval
may file the one-test pin". This is that pin. The legs live in src/mcp.rs
(`McpServer::list_resources` ~:364, `McpRegistry::list_resources` ~:630
returning structured per-server outcomes); the tests live in the mcp.rs
test module (hermetic stub servers, T6 harness).

estimate: ~60 changed lines (1-2 tests + a fixture arm; tests-only —
production src/mcp.rs logic untouched).

## Requirements

1. A stub-server leg where `resources/list` gets a JSON-RPC ERROR reply
   (method-not-found or a server-defined error): assert the surfaced
   error/outcome (a) names the server and (b) carries the server's error
   message text (the T162 response-mapping surface) — the exact
   assertions mirror how the registry's structured outcome exposes it
   (per-server outcome naming the server).
2. The mutant the pin kills is stated: dropping/mangling the server's
   error message from the mapped outcome (the list-err-drop class).
3. Tests-only: no production edits. If the existing surface LOSES the
   message so the pin cannot be written, that is a finding — stop and
   report instead of editing production; the row is priced as a pin,
   not a fix.
4. RED-proof: apply the list-err-drop mutant (drop the message from the
   mapping) in the worktree and show the new test fails; revert and show
   green. State both runs in the commit message.

## Tests

`cargo test --bin chug mcp` green with the new leg(s).

## Acceptance

- Spec check green; full `cargo test` green; clippy `-D` clean.
- `git diff main -- 'src/mcp.rs'` shows test-module hunks only (nothing
  outside `#[cfg(test)]`/`mod tests`).

## Out of scope

- Production error-mapping changes, prompts legs (T170), the T169 tool
  surface, any other survivor class.
