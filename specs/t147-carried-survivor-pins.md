# T147 — pin the four carried validator weak-pin survivors (tests-only)

check: cargo test

estimate: ~150 changed lines (tests only)

## Repo context

Four adversarial-validation survivors have been carried across cycles
65–68, each individually non-blocking, collectively an accumulation of
unpinned behavior on core surfaces. This row kills all four with
RED-proven pins. Tests ONLY — no production change is expected; if a
pin reveals a real bug, the child stops and reports instead of fixing.

1. **T128 M4** (verdict `d1790623655-16`): alive-true-flip of the
   delegate-collect liveness render survived — the `Some(true)` arm of
   the liveness line in render_collect is unpinned. Add a unit test in
   src/delegate.rs's test module asserting the rendered collect output
   for a live child carries the distinct live rendering and that
   flipping the liveness bool flips the output (both arms pinned with
   distinctive text).
2. **T135 M2/F2** (verdict `d1790640546-3`): the driver_lock
   Guard::drop release-side sweep survived — the existing sweep test is
   weak (passes with the sweep removed). Strengthen: after the guard
   drops, the lock file must be ABSENT and a fresh acquire must succeed
   on its FIRST attempt (use the observe seam / attempt counter the
   T135 tests added — specs/t135-driver-lock-race.md has the shape).
3. **T138 plan-guard** (verdict `d1790652558-6`, "plan-guard test
   gap"): plan mode never starts MCP servers — verified by code read,
   never pinned. Add a behavioral leg: with an mcp.json whose command
   touches a flag file, drive the plan-mode path and assert no flag
   file appears AND no server process is spawned (follow the T138
   test's flag-file harness; plan-mode construction per src/plan.rs).
4. **T141 M1** (verdict `d1790628970-6`): the open-content-block
   truncation check in src/api.rs's StreamAccumulator::finish survives
   because the message_stop guard masks it. Isolate it: feed a stream
   that opens a tool_use content block, delivers partial input JSON,
   then delivers `message_stop` WITHOUT `content_block_stop` — the
   message_stop guard alone would ACCEPT this shape, so the open-block
   check is the only rejector. Assert finish() errors. Removing the
   open-block check must flip exactly this test RED (state that in the
   commit message as the M1 kill proof).

## Requirements

1. Four pins, each RED-proven against the pre-pin tree (state the RED
   command + failing assertion per pin in the commit message).
2. No production edits. If a pin fails against CURRENT production
   behavior for a reason other than the known mask (i.e. a real bug),
   STOP and report — do not fix production code in this row.
3. Follow each area's existing test harness (T6 stub rules for anything
   touching a server; env locks serialized where used).

## Acceptance

- `cargo build && cargo clippy --all-targets -- -D warnings && cargo test`
  all green in the worktree.
- Commit message carries the four RED proofs (one line each: mutant
  shape → killing test name).
- Validation routing: tests-only item on core-adjacent surfaces — kimi
  OPTIONAL per LOOP-SPEC step 4 (T130 precedent: skip, the RED proofs
  are the evidence). The orchestrator independently re-runs the four
  mutants at review.
