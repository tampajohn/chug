# T232 — T225's real-clock timing pins flake under host load: synthetic-clock conversion (tests-only, robustness)

check: set -o pipefail; export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared; touch src/*.rs tests/*.rs; cargo test --bin chug

estimate: ~180 changed lines all-in (pin-closure row kind — the
cycle-103..105 calibration band for pin sweeps is 1.7–3.0x the
narrative: narrative ~70 lines of leg rewrites + one smoke leg + spec;
two wall-clock legs converted + one smoke + family sweep)

## Concern

`t225_slow_progress_surface_double_outlives_the_absolute_fence`
(src/testsupport.rs:920) failed THREE times inside one impl-child
stream (t230-impl, cycle 105: 00:24:43, 00:25:39, 00:30:45 UTC,
`.chug/events-t230-impl-20261005-005005.jsonl`) — each time in a
`cargo test` bin run whose binary wall was 43–46s under the child's
concurrent cold-compile load. The same child verified the failure is
environmental, not diff-induced (its commit message: "passed in
isolation (2.29s) and full-binary (5.58s) on a quiet machine —
environmental"; its diff touched only loopd.sh +
tests/loopd_model_routing.rs). The indictment is verified per the
T228 bar: the leg asserts REAL wall-clock windows hold — advance the
surface every 100ms and the 600ms silence fence must never trip —
but host load under a compile storm dilates the process's own
sleep+write loop past the silence base (an ~8x observed slowdown:
5.58s quiet vs 45s loaded binary wall), so `Stalled` trips and the
leg panics on exactly the message it carries ("this is the cycle-101
flake shape the old absolute fence blew"). The fence MACHINERY is
load-scaled by design (`load_scaled_deadline`, `BACKSTOP_FACTOR`,
the pure `trip_decision` seam); the TEST's driving clock is not.
Cost class: the T152/T172/T225 false-red lineage — a red suite the
child must diagnose and re-run (three ~45s loaded re-runs here)
inside a 50-minute budget that this very friction helped exhaust
(the child died minutes-bound, committed, T55-finished). Worse than
the minutes: flake fatigue teaches children to dismiss red, which is
how a real regression in this family would get waved through.

## Repo context

- The T225 pin family lives in src/testsupport.rs (cfg(test)-only
  module, compiled into the bin's test build): legs at L694
  (`t225_trip_decision_pure_legs` — pure, safe), L795
  (`t225_observe_resets_on_advance_and_treats_unreadable_as_progress`),
  L859 (`t225_surface_fingerprint_moves_on_advance_…` — mtime/write
  driven, no sleep windows), L879
  (`t225_stalled_surface_double_trips_in_about_base` — wall-clock:
  base 500ms + 25ms sleep loop), L920 (the flaked leg — wall-clock:
  base 600ms + 100ms sleep loop over base*3), L981
  (`t225_no_bare_verdict_fence_construction_…` — grep pin, safe).
- The synthetic seam ALREADY EXISTS — no production change needed:
  `ProgressDeadline::observe_at(seen, instant)` and
  `tripped_at(instant)` take the clock as a parameter (the pub
  wrappers `observe`/`tripped` are one-liners passing
  `Instant::now()`); `trip_decision` is pure; T229's M7 pin already
  drives the mtime arm synthetically via `File::set_modified`.
- `load_scaled_deadline(base)` (src/testsupport.rs:143) and
  `BACKSTOP_FACTOR` (= 4, T229-pinned) give the load-scaled window
  arithmetic the smoke leg needs.
- The flake evidence: three `FAILED` previews in
  `.chug/events-t230-impl-20261005-005005.jsonl` (00:24:43 / 00:25:39
  / 00:30:45 — `25 passed; 1 failed … finished in 43–46s`), plus the
  child's quiet-machine re-verification recorded in commit 6760a72's
  message.

## Requirements

1. **Convert the two wall-clock legs to synthetic-instant driving.**
   Rewrite `t225_slow_progress_surface_double_outlives_the_absolute_fence`
   and `t225_stalled_surface_double_trips_in_about_base` to drive
   `observe_at`/`tripped_at` with an explicit synthetic instant
   sequence (start + k·step), zero `thread::sleep`, zero
   `Instant::now()` in the assertion path: (a) the outlives leg —
   advances at base/6 synthetic steps keep `tripped_at` == None
   through base×3, and the Backstop (never Stalled) trips at ≥
   `load_scaled_deadline(base) * BACKSTOP_FACTOR`-equivalent synthetic
   silence; (b) the stalled leg — synthetic silence ≥ base trips
   `Stalled`, < base does not. All assertions become deterministic
   functions of the injected instants; load cannot stretch them.
2. **Keep ONE real-clock smoke leg** asserting only the LOAD-ROBUST
   direction through the real wrappers (`ProgressDeadline::new` +
   `observe` + `tripped`, NOT the `_at` seam — the smoke exists to
   prove the wrappers wire through): a genuinely stalled surface
   trips `Stalled` within a wall-clock budget of
   `load_scaled_deadline(base) * BACKSTOP_FACTOR` (generous by
   construction). The smoke MUST NOT assert any sub-scaled no-trip
   window (that is the load-fragile half this row removes); its
   comment says so, naming the synthetic legs as the owners of
   no-trip precision.
3. **Sweep-the-family (T69)**: grep the whole testsupport module for
   `Instant::now()` / `thread::sleep` in timing legs (incl. L795 and
   the t229-region clock use near L1165) and convert EVERY leg whose
   assertion is a sub-scaled-window wall-clock fact to the same
   synthetic driving; legs already load-scaled or pure stay, and the
   sweep's verdict per inspected leg is named in the commit message.
4. **RED-proofs, one named mutant per converted leg**: e.g. silence
   base doubled → the stalled leg dies; reset-on-advance removed →
   the outlives leg dies (it must trip Stalled at the second synthetic
   step); the smoke leg dies if the real wrapper stops calling the
   `_at` seam (wrapper decoupled by test-only indirection or a named
   stub). Existing T229 legs (BACKSTOP_FACTOR == 4, mtime arm) stay
   green — do not duplicate them.
5. **Scope**: src/testsupport.rs ONLY (cfg(test) module). No
   production code, no other test file, no doctrine. If the sweep
   (req 3) finds a wall-clock timing leg OUTSIDE testsupport.rs, name
   it in the commit message and leave it — one concern per row.

## Tests

- The converted legs + smoke green; the full bin unit suite green
  (`cargo test --bin chug` — the check line runs it; testsupport is
  the bin's cfg(test) module so this filter covers every surface the
  change can break).
- RED-proof log per req 4 in the commit message.

## Acceptance

- Zero `thread::sleep`/unscaled `Instant::now()` windows remaining in
  the T225 family's assertion paths; the synthetic legs are
  deterministic under `cargo test --bin chug testsupport -- --test-threads=1`
  AND the default threaded run; the smoke leg's load-robust-only
  comment present; check line green.

## Out of scope

- Production fence behavior (ProgressDeadline semantics unchanged —
  this row changes TEST DRIVING, never the fence), the loopd/launch
  fence adoption sites, any sleep-based leg outside the T225 family
  beyond the req-3 naming, nextest-vs-libtest scheduling questions.
