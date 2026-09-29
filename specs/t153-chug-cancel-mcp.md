# T153 — F10 phase 3a: `chug_cancel` MCP tool (flag-gated write leg)

check: cargo test

## Repo context

FEATURES.md F10 ("chug as MCP server") is the fleet primitive. Phase 1
(T124, d6264be) shipped the `chug mcp-serve` stdio JSON-RPC skeleton +
read-only `chug_status`; phase 2a (T128, 85ca4c1) added read-only
`chug_collect`; phase 2b (T129, 2b4490b) added the flag-gated write leg
`chug_launch` (default-deny via `--allow-launch`, advertised ⇔ callable,
spawn through the one delegate launch path, budgets 1..=200 / 1..=240
reject-above) and CLOSED phase 2 with the fleet shape proven end to end.
T148 (8cd59ac) added the real-wire e2e harness in tests/mcp_serve.rs
(REAL `chug mcp-serve` binary over REAL stdio, `CHUG_DELEGATE_BIN` stub
child recording argv+env).

The fleet gap a driver hits next: **a launched child cannot be stopped
over the wire.** Today cancel means ssh + kill by hand. This row is F10
phase 3a — `chug_cancel`, the second write leg, behind the SAME
`--allow-launch` policy boundary (the flag gates the write surface, not
individual tools). Phases 3b (resources / notifications / server log)
are deferred with a written reason in the cycle-72 EVALUATION §4.

Key mechanisms already in the tree: delegate launches children DETACHED
with `process_group(0)` (src/delegate.rs — the child IS its own process
group leader, so pgid == pid is the detached-spawn fingerprint);
`chug_launch` (src/mcp_serve.rs:522) spawns through that same path and
returns the pid; `chug_collect` already does best-effort `kill(pid, 0)`
liveness + waitpid-WNOHANG reaping (T28 seam).

estimate: ~380 changed lines (src/mcp_serve.rs ~150, its unit tests
~120, tests/mcp_serve.rs wire legs ~90, README ~20)

## Requirements

1. **Policy boundary.** `chug_cancel` appears in `tools/list` ONLY when
   the server was started with `chug mcp-serve --allow-launch`
   (advertised ⇔ callable, same as `chug_launch`). Without the flag a
   `tools/call` for it returns the unknown-tool error (`-32602`) and
   NOTHING is signalled.
2. **Input contract.** `{"cwd": "<absolute path>", "pid": <integer>}` —
   both required. `cwd` goes through the SAME
   `validate_chug_cwd`-class validation as the other tools (naming the
   received value on failure). `pid` must be a positive integer;
   non-integer / non-positive / missing → the validation-chain error
   naming what was received (T129's received-value honesty pattern).
3. **Positive ownership verification BEFORE any signal — fail-closed.**
   ALL legs must hold or the result is `isError: true` naming the FIRST
   failed leg and NO signal is sent:
   (a) pid is alive (`kill(pid, 0)` succeeds — an ESRCH leg answers
   `isError` naming "no such process", NOT an error crash);
   (b) pid is its own process-group leader (pgid == pid — the
   delegate detached-spawn fingerprint; a pid that is not a group
   leader was not launched through the delegate path);
   (c) the pid's command line (resolved via the platform's ps
   equivalent) names a `chug run` invocation. Unresolvable command
   line → fail closed (skip, name the leg).
   Rationale (state in a comment): the server is stateless across
   requests, so ownership is re-derived per call from process
   identity, never from a server-side launch log.
4. **Signal semantics.** On pass: SIGTERM to the process GROUP
   (negative-pid kill / killpg — the whole detached tree dies, not just
   the driver). Then ONE bounded escalation: wait up to ~5 s (polled,
   ≤100 ms cadence) for exit; if the group is still alive, SIGKILL the
   group. Payload: `{pid, signaled: "term"|"kill", waited_ms}` —
   `"term"` when the group exited within the grace, `"kill"` when
   escalation fired. An `isError` result is returned for every
   verification failure; the server loop survives every leg
   (byte-identical liveness posture to T129's refusal legs).
5. **stdout purity + error grammar.** Nothing but protocol messages on
   stdout; the existing `-32700/-32601/-32602/-32600` grammar unchanged;
   `chug_cancel` failures are `isError` tool results, never JSON-RPC
   errors, never fatal.
6. **Tests.**
   Unit (src/mcp_serve.rs test module): the ownership matrix with REAL
   fixture processes the test spawns and owns — (i) live group-leader
   fixture whose argv names `chug run` (spawn via the same detached
   seam, argv shaped by the test) → SIGTERM lands (fixture traps and
   records it), payload `"term"`; (ii) live non-group-leader fixture →
   `isError` naming the pgid leg, fixture provably UNSIGNALLED; (iii)
   dead pid → `isError` naming ESRCH-class; (iv) live group-leader
   whose argv is NOT `chug run` (e.g. `sleep`) → `isError` naming the
   command-line leg, fixture unsignalled; (v) escalate leg — a fixture
   that ignores SIGTERM → payload `"kill"` after the bounded grace
   (fixture reaped by the test; total leg wall < ~8 s).
   Wire (tests/mcp_serve.rs, T148 harness): (a) happy path — server
   with `--allow-launch`, `tools/list` advertises `chug_cancel`, a call
   against a live stub-launched child returns the payload and the stub
   provably dies; (b) default-deny — without the flag, unadvertised +
   call → `-32602`; (c) dead-pid call → `isError` true naming the leg,
   loop alive afterwards (ping round-trips).
7. **README** — the `chug mcp-serve` section: `chug_cancel` documented
   beside `chug_launch` (policy boundary, ownership legs, term→kill
   semantics); the "Two read-only tools ship" sentence and the phase-3
   deferral line updated to match (phase 3a landed; 3b still deferred).

## Acceptance

- `cargo build`, `cargo clippy --all-targets -- -D warnings`, `cargo test`
  all green.
- Every verification-failure leg proven to send NO signal (fixture
  aliveness asserted AFTER the call).
- Wire legs run deadline-bounded; no sub-ms ordering assertions (the
  T129 carried nit, honored by T148, holds here too).

## Out of scope

- Cancelling via the MCP `notifications/cancelled` protocol message
  (request-in-flight cancellation — 3b design territory).
- Server-side launch registry / durable state.
- `resources/*`, `notifications/*`, server log file (phase 3b).
- A `chug kill` CLI verb (the MCP surface is the fleet need; CLI cancel
  is a later row if a consumer pulls).
