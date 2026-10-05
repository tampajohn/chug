# T236 — loopd_orphan_reaper timing-fence reds under full-suite load (the fence class, not timeout bumps)

check: set -o pipefail; export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared; touch src/*.rs tests/*.rs; cargo test --test loopd_orphan_reaper && cargo test --bin chug

## Repo context

Tests-only robustness row (tests/loopd_orphan_reaper.rs plus, only
if the chosen remedy leg needs it, the existing testsupport scale
machinery in src/testsupport.rs — cfg(test)-gated).
estimate: ~250 changed lines (the T233 test-pin family band ~3x
priced in: fences and seams pre-exist — T214/T232/T233 — so the
spread is pin density, not new machinery).

The cycle-107 wrap handed "one pre-existing loopd_orphan_reaper
load-flake note" to the cycle-108 eval's census. The census
(events-stream grep across the delta window) MET the T233 filing
standard (2+ organic sightings, verified transient):

1. 2026-10-02 (.chug/events-20261002-050349.jsonl, glm
   orchestrator): `chug::loopd_orphan_reaper
   a_failing_driver_probe_means_no_sweep` FAIL [31.851s] under
   nextest release (113/1402);
2. 2026-10-03 (.chug/events-20261003-151057.jsonl): `--test
   loopd_orphan_reaper` 16 passed / 4 FAILED in 127.58s incl.
   `sandbox_scripts_copy_skips_non_regular_entries` panicked at
   tests/loopd_orphan_reaper.rs:361:65; green re-run in the same
   stream;
3. 2026-10-05 (.chug/events-t230-impl-20261005-005005.jsonl,
   t230-impl): the gate chain naming `--test
   loopd_orphan_reaper` SIGKILLed at the 300s bash cap under load;
   the suite then green standalone (34/34 in 45.3s) —
   family-adjacent load sighting;
4. 2026-10-05 (.chug/events-t234-validate-20261005T061.jsonl,
   t234 validator):
   `the_cwd_leg_identifies_an_orphan_through_loopd` FAIL [32.740s]
   under nextest release (254/1645); the verdict names it
   pre-existing with zero causal path to the diff.

The ~31–33s FAIL signature is the fence-TRIP class, not a logic
regression: every sighting re-ran green, and the failing legs vary
(probe-no-sweep, cwd-leg, sandbox-copy), which is the load
fingerprint, not a per-leg bug.

Mechanism — labeled HYPOTHESIS per the T228/T212 filing bar (the
child verifies per req 1 before remedying): the red legs ride
wall-clock fences — the suite's sweep-wait loop fences on a 30s
`ProgressDeadline` backstop (tests/loopd_orphan_reaper.rs:442-488,
the T225 machinery) that does not ride the T214 load-scale seam —
so under 17-way nextest parallelism plus compile storms the
sweep's log progress stalls past the fence and the deadline trips.

Remedy direction (mechanism, not timeout bumps — the T152
doctrine): route the red legs' fences through the proven
machinery — the T214/T233 scale seam (the `load_scaled_deadline`
family, clamp [1.0,4.0], NaN/0 fail-safe to base) or the T232
synthetic clock where the wait is real-clock — with factor-1 pins
keeping every scripted assertion byte-deterministic. The 30s base
stays BYTE-IDENTICAL: scale is multiplicative through the seam,
never an edited literal (the zero-timeout-bump doctrine).

## Requirements

1. **Verify the indictment per leg (T228 bar) BEFORE remedying:**
   for each census-red leg (`a_failing_driver_probe_means_no_sweep`,
   `the_cwd_leg_identifies_an_orphan_through_loopd`,
   `sandbox_scripts_copy_skips_non_regular_entries`, and every leg
   the sweep in req 3 names), READ the fence it actually rides and
   name it in the commit message. A leg whose fence already scales
   is excluded with the reason named; the hypothesis is per-leg
   until verified.
2. **Remedy each verified unscaled fence** through the existing
   machinery (scale seam or synthetic clock — the leg's fence shape
   chooses, per the T232/T233 precedents). Every remedy leg gets
   its own RED-proven killing test: the pin must go RED when the
   scale/synthetic wiring is reverted and GREEN with it (the T229
   pin-strength bar).
3. **Sweep the family:** every loopd_orphan_reaper leg riding the
   SAME fence class as a verified red leg is converted in the same
   arc; the sweep (converted + excluded-with-reason) is enumerated
   in the commit message (the T233 req-3 convention).
4. **Byte-stability pins:** the 30s `ProgressDeadline` base, the
   settle-window literals, and every scripted attempt-count
   assertion stay byte-identical; factor-1 pins prove the scripted
   legs are scale-invariant.
5. Zero production (non-cfg(test)) code changes. If a remedy leg
   needs a testsupport seam addition, it rides the EXISTING
   `load_scaled_deadline`-family surface the way T233's
   `dead_port_retry_budget_from_factor` did (pure seam, NaN/0
   fail-safe, drivers caller-compatible) — no edits to the existing
   testsupport behavior.

## Tests

- The per-leg RED-proven killing pins (req 2) and the factor-1
  byte-stability pins (req 4), all in tests/loopd_orphan_reaper.rs
  (plus src/testsupport.rs's test module only if a seam is added).
- The check line green; the reaper suite green in isolation AND
  under the full-suite runner (nextest release) — 3 consecutive
  full-suite runs green in the worktree before goal_complete (the
  T233 acceptance convention for a load-flake family).

## Acceptance

- Check line green: `cargo test --test loopd_orphan_reaper &&
  cargo test --bin chug`.
- 3 consecutive full-suite (`cargo nextest run --release`) green
  runs in the worktree.
- The commit message names: every census-red leg's verified fence,
  the remedy per leg, and the sweep enumeration (req 3).
- No timeout/grace/slack literal edited anywhere in the suite.

## Out of scope

- The other loopd integration suites (loopd_spoof_guard,
  loopd_stale_binary, …) — their census is clean in the window; a
  leg from ANOTHER suite that reds during this arc is evidence for
  the NEXT eval, not scope creep into this row.
- Production reaper logic changes (loopd.sh / src/daemon.rs
  non-test code).
