# T225 — progress-reset liveness fences for child-spawn test surfaces

check: set -o pipefail; export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared; touch src/*.rs tests/*.rs; cargo test --bin chug && cargo test --test loopd_orphan_reaper --test loopd_spoof_guard --test loopd_daemon_ensure

estimate: ~350 changed lines all-in (testsupport seam + pure legs + adoption across ~5 test surfaces + RED-proof fixtures — filed at the cycle-103 calibration for a multi-surface pin row)

## Repo context

The loop's spawn-heavy TEST fences are wall-clock deadlines over real
child processes. Under suite fan-out (nextest ~17-way) plus the loop's
own concurrent children, child-spawn walls stretch ~9x (T152's
measurement: 30.8s at 17-way vs 3.36s solo) while the fences do not
move. Two measured costs in the cycle 98-102 delta:

- cycle-100: t219-impl run-1 died 80/80 mid-debug after **10 delegate
  launch bin-tests flaked** under concurrent loopd load; the child
  A/B-proved (stash/re-run) its diff not-causal — the family passes at
  `--test-threads=1`. A full 80-iteration child burned on a
  test-infrastructure false-red.
- cycle-101: `loopd_orphan_reaper::a_failing_driver_probe_means_no_sweep`
  expired its T214 load-scaled fence IDENTICALLY on main mid-cycle under
  ambient load 9.68, then passed at post-merge. T214 scales by
  `clamp(loadavg_1m / cores, 1, 4)` — K7 has **18 cores**, so 9.68 reads
  0.54/core → factor 1.0 → NO scaling. The per-core variable is blind
  to suite-fan-out contention on many-core hosts.

Current state:

- `src/testsupport.rs:142 load_scaled_deadline(base)` — adopted by
  `tests/loopd_orphan_reaper.rs:445` (30s), `tests/loopd_spoof_guard.rs:183`
  (30s), `tests/loopd_daemon_ensure.rs:284` (90s); pure `scale_factor`
  legs + fail-safe seam pinned.
- `src/delegate/tests/{launch,status,wait,wait_terminal,collect}.rs` —
  ~30 bare `elapsed < Duration::from_secs(5..30)` asserts over real stub
  children (launch.rs alone has 29 test fns).
- A fence whose child is making progress is a LIVENESS fence; its trip
  condition should be "no progress for N seconds", never "wall clock
  exceeded while progress continued".

## Requirements

1. `src/testsupport.rs`: a progress-reset deadline — e.g.
   `ProgressDeadline::new(base)` whose poll-loop caller `reset()`s it
   whenever the watched surface advances (mtime / byte length / iteration
   count); it trips only after `base` of NO progress. An absolute outer
   backstop (`load_scaled_deadline(base)` scaled by a named constant, ~4x)
   keeps a genuinely hung child failing. The reset/trip computation is
   PURE (the `scale_factor` pattern — testable without host state) and
   fail-safe on any read error (an unreadable surface reads as PROGRESS,
   never a false trip).
2. Adopt at the three loopd families' verdict fences where the poll
   already observes an advancing surface (events file mtime, log growth,
   child stdout): the deadline resets on observed advance. A fence with
   NO observable progress surface keeps `load_scaled_deadline` — name it
   in the compliance notes with the reason.
3. Adopt at the delegate bin-test fences that poll real children
   (launch/status/collect legs). SEMANTIC timing asserts STAY ABSOLUTE —
   `elapsed >= from_secs(1)` lower bounds and the wait_secs round-trip
   bounds pin call semantics, not liveness; converting them would gut
   the pin. The compliance notes list which asserts converted and which
   stayed, one line of reason each.
4. RED-proof (T69 sweep doctrine): (a) a stalled-fixture leg — a child
   double that never advances trips in ~base; (b) a slow-progress
   fixture — a double that advances every ~2s for 3x base PASSES under a
   base the old absolute fence would have blown (this IS the flake
   reproduction: the old shape fails it, the new shape passes); (c) the
   T214 pure clamp legs and fail-safe seam legs stay green untouched.
5. The adoption grep pin (T214's T48-runtime-needle pattern) is extended:
   zero bare `Instant::now() + Duration::from_secs(30|90)` verdict-fence
   constructions remain on the converted surfaces.
6. Tests + testsupport only — no production behavior change, no doctrine
   file change. If a production edit appears necessary, STOP and report.

## Tests

- Pure legs: reset computation (advance → not expired; base of silence →
  expired; backstop reached despite continuous progress → expired);
  fail-safe legs (unreadable surface treated as progress).
- Fixture legs per req 4 (stalled trips; slow-progress passes).
- The existing T214 pins (`scale_factor` clamp legs, fail-safe seam) and
  the three loopd families' full suites stay green.

## Acceptance

- cargo build + clippy --all-targets -D warnings green.
- The check line green; the three loopd families green in ONE
  full-suite run (no `--test-threads=1` isolation) at review.
- Old-vs-new discrimination demonstrated once in the child's notes: the
  slow-progress fixture fails under the pre-row fence shape, passes
  under the new.
- Compliance notes naming converted vs stayed-absolute asserts.

## Out of scope

- Re-opening T214's `load_scaled_deadline` formula (it stays the
  backstop).
- nextest configuration changes; test edits outside the named surfaces.
