# T59 — mcp_http `dead_port` tests: bounded retry on port-theft (T31 residual flake)

check: cargo test

## Repo context

T31 (`e765de2`) deflaked the parallel-load test family with mechanism
fixes: `dead_port()` (`src/mcp_http.rs:1258`) probe-verifies a
connection-refused port (bounded 32-attempt re-bind) and
`assert_dead_port()` (`src/mcp_http.rs:1281`) re-verifies the handout
immediately before a phase connects. The residual race is structural:
between the probe and the client's connect, a PARALLEL test (same or
sibling test binary) or the OS ephemeral-port allocator can claim the
port — "port-theft under parallel load". Three organic sightings on
record: cycle-16 (~1/8 rate estimate), the t48 validator stream, and
the T55 validator's gates (cycle-25 carry (c)iii: "passes isolated
3/3"). Every sighting false-reds a gate run and erodes trust in green;
it is the last known organic flake in the suite.

## Requirements

1. Where a `dead_port()` handout feeds a connect phase, a detected
   port-theft (the `assert_dead_port` re-verify failing, or the connect
   unexpectedly succeeding against the supposedly-dead port) retries the
   WHOLE acquire-probe-connect sequence with a fresh `dead_port()` —
   bounded (3 attempts) — instead of panicking on the first theft.
2. The retry helper lives in the test module next to `dead_port` /
   `assert_dead_port`; the panic message after exhaustion names the
   attempts and the theft mechanism (so a REAL regression is still
   distinguishable from a flake storm).
3. Production code is untouched — every hunk is inside
   `#[cfg(test)] mod tests` (or a test-only helper it uses).
4. No timeout/sleep-based masking: the fix is retry-on-theft, not
   waiting (T31 doctrine: mechanism, not timeouts).

## Tests

- A unit test for the retry helper itself: with a scripted "steal"
  injected on the first N attempts (a seam closure or a fake
  acquire/probe pair — implementer's choice of the cheapest honest
  seam), the helper succeeds on attempt N+1; with theft on ALL attempts
  it panics/errors naming the attempt count.
- Non-vacuousness hand-check by the implementer: the current failure
  mode (single-attempt) would not exercise the retry path.
- Full suite green under `--test-threads=4` (the configuration that
  produces the sightings) — run it at least twice in the worktree.

## Acceptance

- `check:` passes in the impl worktree.
- The diff is tests-only; `git diff --stat` shows only `src/mcp_http.rs`
  test-module hunks.
- No `|` in the TODO row's notes cell (T40).
