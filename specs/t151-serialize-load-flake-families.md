# T151 — serialize/isolate the default-parallelism load-flake test families (mechanism, not timeouts)

check: cargo test

## Repo context

The default-parallelism `cargo test` full-suite flake family is now the
loop's most expensive recurring test-infra hazard. Six fires in cycles
70–71, every one on a CLEAN base under machine load (parallel children +
nextest gates + mutation builds), every one in a spawn-timing or
wall-clock-sensitive test:

- t145-impl (cycle 70): TWO goal-gate rejections —
  `mcp_http::tests::dead_port_probe_retry_recovers_after_scripted_theft`
  (src/mcp_http.rs:1920) and a sibling dead-port leg, each 8/8 green in
  isolation, a different test each run (T31/T59/T66 residual class).
- t144-validate (cycle 70): first gate attempt hit the 600s check
  timeout under its own mutation-build load with ZERO test failures —
  wall-clock stretch, not red.
- t148-impl (cycle 71): `driver::tests::events::drive_loop_writes_events_jsonl`
  (src/driver/tests/events.rs:165),
  `driver::tests::hooks_policy::hook_allow_executes_tool_and_post_note_lands_in_result`
  (src/driver/tests/hooks_policy.rs:182),
  `driver::tests::permissions_policy::malformed_permissions_config_fails_open_once_and_run_continues`
  (src/driver/tests/permissions_policy.rs:425) — reproduced on the clean
  base with the tree stashed; the child fell back to `--test-threads=4`.
- t150-impl (cycle 71): 3 of 958 failed under default parallelism —
  `delegate::tests::launch::delegate_launch_boundary_max_tokens_one_reaches_child`
  (src/delegate/tests/launch.rs:475) + two sibling delegate-launch legs;
  the re-ran gates chased flakes until the child died on the 35-minute
  MINUTES budget with the work committed (d1790691515-11).
- cycle-70 T144 arc: `delegate::tests::launch::delegate_launch_stub_then_status_reports_summary_and_liveness`
  (src/delegate/tests/launch.rs:17) full-suite load flake, one sighting,
  assessed by the validator as timing-window stretch.

T31 (cycle 15) established the doctrine: **deflake by mechanism, not by
timeout/grace/slack bumps** — `RUN_SHELL_TIMING_LOCK` (src/tools.rs:1903,
a poison-tolerant static Mutex) serializes the 3 wall-clock run_shell
tests, and `dead_port()` got probe-verified acquisition. That lock covers
ONLY src/tools.rs's own tests; the driver/delegate/mcp_http families
above have no equivalent. Nextest-based orchestrator gates are unaffected
(per-process isolation); the victims are children's own `cargo test`
goal-gate runs at default parallelism.

estimate: ~260 changed lines (tests/test-support only — a shared lock
module ~40, lock acquisition + polling conversions across the named
families ~200, pins ~20)

## Requirements

1. **One shared serialization lock, crate-visible.** Introduce a
   `pub(crate)` test-support mutex (the RUN_SHELL_TIMING_LOCK pattern:
   static `Mutex<()>`, poison-tolerant acquisition helper that recovers
   the guard from a poisoned lock) in ONE location importable from
   `src/driver/tests/*`, `src/delegate/tests/*`, `src/mcp_http.rs`'s test
   module, and `src/tools.rs`'s test module (e.g. a small
   `src/testsupport.rs` `#[cfg(test)]`-gated module, or the existing
   pattern's natural home — the implementer picks, one location only).
   Re-implementing a second independent lock is a finding; the point is
   ONE serialization domain so no two wall-clock/spawn-timing tests in
   the process run concurrently. `RUN_SHELL_TIMING_LOCK` may be renamed/
   moved into the shared module; its 3 existing tests keep passing
   byte-identical in behavior.
2. **Serialize the named families.** Every test in the six named
   sightings above acquires the shared lock for its full body (guard
   held across the timing-sensitive region — spawn → assertion →
   cleanup), AND every sibling test in the same file that asserts on
   wall-clock windows, spawn timing, deadlines, or probe timing gets the
   same treatment (sweep the family, not just the sighted instances —
   the cycle-33 lesson: name the class, sweep every instance).
3. **Convert, don't widen.** Where a flaked leg asserts an absolute
   wall-clock bound (`elapsed < X` under load), replace with
   condition-polling against a deadline (poll for the expected state
   every ≤50ms until a generous-but-bounded deadline; the deadline is a
   liveness backstop, not the assertion). NO existing timeout, grace, or
   slack constant may be increased; deadline constants NEW to the
   polling legs are backstops and must be ≥10x the unpolluted expected
   duration (state the multiple in a comment).
4. **dead_port residual.** Re-read
   `dead_port_probe_retry_recovers_after_scripted_theft` and its
   neighbors: if the flake leg is the probe/toctou window, serialize +
   poll per (2)/(3); if a NEW mechanism is found, describe it in the
   commit message. Do NOT loosen the existing theft-retry attempt-count
   pins (T59/T66).
5. **Non-vacuousness pins.** One test proving the shared lock is a
   single domain: two threads overlapping acquisition attempt →
   serialized (e.g. a contended-acquisition ordering probe, itself
   immune to load-flake by construction — explain why in a comment).
   And the module must fail to compile if a second static lock instance
   is introduced next to it (one definition, by construction).

## Acceptance

- `cargo build`, `cargo clippy --all-targets -- -D warnings`, `cargo test`
  all green.
- The implementer runs the FULL suite at DEFAULT parallelism ≥6
  consecutive times green, at least once under induced load (e.g. a
  background `yes > /dev/null` spinner per core, named in the commit
  message), AND ≥2 more green runs with `--test-threads=4`.
- Zero timeout/grace/slack constant increases in the diff
  (`git diff -U0 | grep` evidence in the commit message).
- The six sighted tests each carry the lock (or a conversion note
  naming why serialization does not apply, approved at review).

## Out of scope

- Nextest gate behavior (already isolated; unchanged).
- loopd/orchestrator gate posture (unchanged — T82 runner rule stands).
- New flake families not yet sighted; watch item reports them to the
  next eval.
