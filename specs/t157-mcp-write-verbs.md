# T157 — F10 phase 3: MCP write verbs (chug_abort + chug_steer)

check: export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared && cargo test --bin chug mcp_serve::

estimate: ~450 lines (2 verbs + gates + e2e)

## Concern

Operator 2026-09-29: "think about write mcp tools." F10 phase 2 proved
the fleet shape end-to-end (chug_status + chug_collect + flag-gated
chug_launch, T124/T128/T129/T148). Phase 3 was deferred pending a
consumer — the operator IS the consumer now. One write verb exists
(launch); the operator surface needs the other two natural verbs:
cancel and steer.

## Repo context

- `src/mcp_serve.rs`: the stdio server; CHUG_LAUNCH_TOOL gate pattern
  (default-deny, advertised ⇔ callable, --allow-launch).
- `src/delegate.rs`: launch/status/collect seams (T23/T28/T29/T69).
- Steering notes exist in the driver (chat dock `[operator]` notes,
  consumed at iteration boundaries) — chug_steer rides that mechanism,
  it does not invent one.
- Cancellation: T136's crash-repair + driver abort paths; a launched
  child is a process — chug_abort must be graceful-first (SIGTERM,
  then the T6 group-kill discipline).

## Requirements

1. `chug_abort`: cancel a running chug_launch'd child by id. Graceful
   SIGTERM first, SIGKILL group after a bounded grace (T6 discipline);
   result reports the terminal state (aborted | already-done | not-found
   — fail-closed per T130's distinctive phrases).
2. `chug_steer`: inject a steering note into a running child's input
   channel (the `[operator]` mechanism). Result reports queued vs
   undeliverable (child done/gone).
3. Policy: both verbs ride the SAME gate family as chug_launch — a new
   `--allow-control` flag (abort+steer together), default-deny, not
   advertised without it; permissions.json integration per T90
   (mcp__ prefix rules apply).
4. `chug_delegate` (spawn into an existing run) is EVALUATED and either
   specced or deferred with a written reason — the answer must be in
   the commit, not implied.
5. Wire e2e over real stdio (T148 pattern): launch → steer → observe
   the note land → abort → observe terminal state.

## Tests

- Gate legs: not advertised/callable without the flag (both verbs),
  advertised and callable with it.
- abort: not-found vs graceful vs escalated-to-KILL paths; the child's
  events.jsonl records the abort.
- steer: note appears in the child's next-iteration context (fixture
  assert via events or transcript); undeliverable when child is done.
- Acceptance: a fleet consumer (Claude Code or another chug) can
  launch → steer → abort a run over MCP without touching the host.

## Out of scope

- Notifications/resources (still phase-3-deferred, no consumer);
  multi-run batch verbs; chug_edit/TODO writes via MCP.
