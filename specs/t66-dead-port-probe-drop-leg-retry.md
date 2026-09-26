# T66 — dead_port_probe drop→probe leg: T59 retry covers the last uncovered theft window

One concern: `dead_port_probe_distinguishes_live_from_dead`
(src/mcp_http.rs:1736) still false-reds gates under parallel load — the
first post-T59 organic sighting (cycle-30 goal gate, default-parallel
`cargo test`, port 50956: "dropped port did not refuse connections").
T59 wrapped only the test's third (acquire-verify) leg in
`dead_port_retry_with`; the second (bind→drop→re-probe) leg carries the
identical port-theft window with no retry.

## Repo context

- All machinery is in the `#[cfg(test)]` module (src/mcp_http.rs:1074+):
  `bind_stub()` (:1219), `port_refuses_connections()` (:1236),
  `DEAD_PORT_RETRY_ATTEMPTS = 3` (:1299),
  `check_dead_port(port) -> Result<(), PortTheft>` (:1312),
  `theft_or_regression(port, observed)` (:1329 — port still live at
  catch → theft, retryable; refuses at catch → regression, panic),
  `dead_port_retry_with(acquire, attempt)` (:1353).
- The failing assertion (src/mcp_http.rs:1745-1748): the test binds a
  stub, asserts the live probe reads live, `drop(listener)`, then asserts
  `port_refuses_connections(port)`. Between the drop and the re-probe
  connect, a parallel test or the OS ephemeral allocator can claim the
  just-freed port — the connect succeeds, the probe reads LIVE, the gate
  false-reds. This is the T31/T59 class ("Between the T31 probe and the
  client connect a parallel test or the OS ephemeral allocator claims
  the port" — T59 commit 062c175; sightings: cycle-16 ~1/8,
  t48-validate, t55-validate, and now cycle-30's goal gate).
- The live leg (probe-on-our-own-listener) has NO window — the port is
  held throughout. Only the drop→probe window is exposed. After a theft
  the old port stays poisoned (the thief holds it), so a retry must
  re-bind a FRESH stub, not re-probe the same port — the whole
  bind→live-probe→drop→dead-probe sequence is the retried unit.
- Evidence this is the flake, not a regression: passes isolated; 4 full
  green suite runs this cycle at `--test-threads=4`; the failure
  appeared only at default (max) parallelism; nothing this cycle touched
  mcp_http.rs (T64 tests in shared_target_dir.rs/tools.rs, T65 README).

## Requirements

1. **Wrap the drop→probe sequence in bounded theft-retry.** The
   live-probe + drop + dead-probe sequence runs as ONE attempt; on a
   dead-probe reading LIVE, discriminate via `theft_or_regression`
   semantics (re-probe at catch: still live → theft → fresh attempt
   with a freshly bound stub; refuses → regression → panic naming it —
   a real probe bug must NEVER be retried into green). Bound:
   `DEAD_PORT_RETRY_ATTEMPTS` (reuse the const, no new one); exhaustion
   panics naming the attempts. Reuse `dead_port_retry_with` if the
   shape fits honestly (its acquire returns a bare `u16` while this
   attempt must own the listener across the live probe — if that
   mismatch forces contortions, a small sibling helper or a hand-rolled
   loop with identical semantics/bound is acceptable; do NOT weaken
   `theft_or_regression`'s discrimination to make the shape fit).
2. **Scripted-theft deterministic unit pin** for the new retry path,
   mirroring `dead_port_retry_succeeds_after_scripted_theft`: a binder
   seam (injected closure) lets the test poison attempt 1's drop-window
   with a REAL thief listener (the probe genuinely reads live after the
   drop) while attempt 2 binds clean; an attempt counter pins that the
   retry executed exactly once — the pre-T66 shape (single attempt,
   panicking assert) cannot pass it. If req 1's shape makes a binder
   seam unnatural, the pin may instead drive the new helper with a
   scripted acquire/attempt pair — but SOME deterministic exercise of
   the new theft branch is required, not just the organic race.
3. **Non-vacuousness evidence in the commit message:** a gutted-probe
   mutant (e.g. `port_refuses_connections` forced to a constant) goes
   RED on this test family (the retry must not mask a real probe
   regression — `theft_or_regression` panics), revert → green. Paste
   before/after output.
4. **Tests-only** (src/mcp_http.rs `#[cfg(test)]` module only; zero
   production-behavior change; `dead_port()` at :1259 is `pub(crate)`
   production-adjacent — do not touch it). Existing T31/T59 pins
   (scripted-theft success, exhaustion, the acquire-verify leg) stay
   byte-identical and green.
5. **Verify at load:** full suite green at `--test-threads=4` AND at
   default parallelism (the goal-gate invocation), each stated in the
   commit message.

## Tests

The new scripted-theft pin IS the test for the branch; the retried
probe test is the fix. Full suite + clippy green.

## Acceptance

- The drop→probe leg tolerates detected theft with bounded retry;
  regression discrimination preserved (real probe bug panics).
- Deterministic scripted-theft pin proves the retry branch executes.
- All pre-existing mcp_http pins untouched and green.
- `cargo test` (default parallelism) + `cargo test --
  --test-threads=4` + `cargo clippy --all-targets -- -D warnings` green.
- Adversarial validation OPTIONAL (tests-only — T16/T31/T59/T62
  precedent); orchestrator gates + non-vacuousness evidence suffice.

check: cargo test -- --test-threads=4 && cargo test --bin chug mcp_http
