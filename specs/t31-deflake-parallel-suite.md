# T31 — Deflake the parallel-load test family (mechanism, not timeouts)

check: cargo test -- --test-threads=4

## Repo context

Three named tests flake under `--test-threads=4` parallel load and pass
in isolation. Four sightings across four cycles (all NAMED, post-T25):

1. `mcp_http::tests::dead_server_retries_then_tool_error_without_sleeping`
   (`src/mcp_http.rs:1482-…`) — binds `127.0.0.1:0`, **drops** the
   listener, and assumes the port stays free (`src/mcp_http.rs:1484`).
   Under parallel load another test's stub can claim the port between
   drop and connect (TOCTOU). The T28 validator's `goal_complete` was
   REJECTED by this flake (its verdict ledger names it; passes isolated
   in 0.01s).
2. `tools::tests::run_shell_returns_when_setsid_grandchild_holds_pipe`
   (`src/tools.rs:1756`) — asserts wall-clock `elapsed < timeout +
   READER_GRACE + 3s` (`src/tools.rs:1782`). Failed once in cycle-13's
   MAIN gates under parallel load; passes isolated in 6.10s; the T29
   fix-up child's verdict calls it "known flake" by name.
3. `tools::tests::run_shell_normal_path_unchanged`
   (`src/tools.rs:1797`) — same wall-clock assertion shape
   (`src/tools.rs:1744` is the sibling pin). Failed once in
   T29-validate2's suite run (414/415, 7.83s); passes isolated in
   0.13s.

Mechanism: wall-clock `elapsed` assertions stretch when the parallel
scheduler starves a test thread; the port probe is a TOCTOU. A suite
that lies red has already rejected one `goal_complete` and red-gated
two validators — bug-class reliability.

## Requirements

1. **Mechanism over timeouts.** It is FORBIDDEN to fix any of these by
   increasing a timeout, grace, or slack constant. Each fix must remove
   the load sensitivity by construction:
   - `dead_server_retries…`: remove the bind-:0-drop TOCTOU — e.g.
     probe-verify the port is actually connection-refused before use
     (attempt a connect; if it unexpectedly succeeds, re-bind a fresh
     port and retry, bounded attempts), or another mechanism you
     justify in a code comment.
   - the two `run_shell` tests: make the promptness assertions
     load-insensitive — e.g. serialize the timing-sensitive tests
     (a shared `static` `Mutex` in the `tools::tests` module locked for
     the duration of each timing-sensitive test; std-only, NO new
     dependencies) so scheduler stretch cannot co-occur, or re-base the
     assertion on mechanism rather than wall time. Pick per test and
     justify the choice in a code comment.
2. **No assertion weakened.** The fixed tests must still die when the
   behavior they guard regresses (a `run_shell` that stops returning
   promptly; a dead-server call that stops erroring fast). Document in
   the commit message at least one mutant you killed per fixed test
   (e.g. reintroduced hang, removed retry cap).
3. **Tests-only expected.** No production behavior change. If a
   production seam turns out to be required, keep it minimal, pinned,
   and call it out in the commit message.
4. `cargo build`, `cargo clippy --all-targets -- -D warnings`, and
   `cargo test -- --test-threads=4` all green.

## Tests

- The three named tests remain (modified, not deleted) and keep their
  original behavioral intent.
- Any new helper (probe/serialization primitive) is exercised by the
  fixed tests.
- Non-vacuousness: the commit message records the killed mutants
  (requirement 2).

## Acceptance

- **Five consecutive** full `cargo test -- --test-threads=4` suite runs
  green in the worktree (run them; ~15s each post-build) — record the
  five results in the commit message or ledger.
- No timeout/grace/slack constant increased anywhere in the diff
  (reviewer greps).
- `cargo clippy --all-targets -- -D warnings` clean.

## Boundaries

- Implement TODO item t31 ONLY. Keep `cargo build` + clippy + test
  green. Commit your work here. DO NOT touch TODO.md or LEDGER.md in
  the main tree — bookkeeping is the orchestrator's. Do not edit any
  spec/doctrine file (LOOP-SPEC.md, META-SPEC.md, META-META-SPEC.md,
  SPEC*.md).
