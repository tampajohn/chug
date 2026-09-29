# T148 — chug_launch wire e2e over real stdio (T129 descoped follow-up; kills M7 + M8)

check: cargo test

estimate: ~220 changed lines (tests ~200, docs ~20) — T129's estimate
line named this follow-up explicitly when the descope clause fired
(diff 989 → 814 after dropping it)

## Repo context

T129 (cycle 69, 2b4490b) landed `chug mcp-serve --allow-launch` with the
`chug_launch` write tool. Its estimate-line descope clause dropped the
optional wire-level e2e to this follow-up, and the validator's two
non-blocking survivors (verdict `d1790664201-2`) map onto exactly that
gap:

- **M7**: `--allow-launch` CLI plumbing unpinned end-to-end (flag →
  advertised in tools/list ⇔ callable in tools/call over a real wire).
- **M8**: the launch-failure `isError` arm unpinned.
- Carried nit (reject, do not chase): the unit tests' sub-ms stub-spawn
  events-ordering assertion is inherently racy — this e2e must NOT
  assert sub-ms ordering; poll with a bounded deadline for eventual
  effects instead.

Existing seams: tests/mcp_serve.rs already has the T124-era stdio e2e
harness (spawn the built server, ndjson JSON-RPC over pipes,
deadline-bounded). src/delegate.rs:305 has the `CHUG_DELEGATE_BIN` env
lock for spawn-stubbing in tests. Use both: the e2e spawns the REAL
`chug mcp-serve` binary over REAL stdio, with `CHUG_DELEGATE_BIN`
pointing at a stub "child" script that records its argv+env to a file
and exits 0 — real wire, real server, real spawn, stubbed child (no
model endpoint needed).

## Requirements

1. **Happy-path leg (kills M7)**: server spawned WITH `--allow-launch`;
   over the wire: initialize → tools/list contains `chug_launch` →
   tools/call `chug_launch` with a valid absolute spec path, goal,
   model, budgets → response is not isError and carries a pid/liveness
   payload → the stub's argv record appears within a bounded poll
   (~5 s) showing the exact spec/goal/model/budget values the wire
   carried (the CLI plumbing, asserted end to end).
2. **Failure leg (kills M8)**: a `chug_launch` call that must fail
   validation (e.g. relative spec path, or budget above the 200/240
   ceilings — pick the leg T129's unit tests cover least) → the wire
   response is the `isError` arm with the received-value error text,
   and NO stub spawn record appears.
3. **Default-deny leg**: server spawned WITHOUT the flag → tools/list
   omits `chug_launch` and tools/call `chug_launch` returns the
   unknown-tool error — a read-only deployment cannot probe the flag
   into revealing the tool (the T129 policy boundary, proven over the
   wire).
4. Harness hygiene: every spawned server gets a unique temp cwd +
   stdio pipes, bounded total leg deadline (the T6 rules), cleanup of
   stub files; no port use (stdio). Parallel-test safe (unique tempdirs,
   the `CHUG_DELEGATE_BIN` env lock serialized per the existing
   pattern).
5. README's `chug mcp-serve` section gains one line naming the wire e2e
   coverage.

## Acceptance

- `cargo build && cargo clippy --all-targets -- -D warnings && cargo test`
  all green in the worktree.
- Commit message states the M7/M8 kill proofs (mutant shape → killing
  leg).
- Validation routing: tests-only on the young MCP wire surface — kimi
  OPTIONAL (T130/T131 precedent leans skip; the orchestrator decides at
  work time; the RED proofs are the evidence either way).
