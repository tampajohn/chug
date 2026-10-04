# T231 — LOOP-SPEC step 5's TODO-edit guard floor names the T57 main-dedicated target dir (doctrine, SOLO)

check: set -o pipefail; export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared; touch src/*.rs tests/*.rs; cargo test --test loop_spec_recovery --test todo_consistency

estimate: ~120 changed lines all-in (doctrine+pin carrier row — the
cycle-105 calibration: narrative ~40 lands ~3x with the window-anchor
pin leg machinery and spec)

## Concern

LOOP-SPEC step 5's TODO.md-edit guard sentence runs
`cargo test --test todo_consistency` "(seconds)" and names NO target
dir. The "(seconds)" assumption silently depends on whichever cache
the invocation happens to hit — and the DEFAULT `target/` dir is cold
for the whole crate after EVERY version bump by construction (a
manifest-only change stales every artifact). The cycle-104 wrap paid
the measured cost: four consecutive guard/gate runs died
`timed out after 300s (process group killed)` with
`Compiling chug v0.17.1 (/Users/jadams/workspace/chug)` as the last
line (23:08, 23:17, 23:22, 23:27 UTC,
`.chug/events-20261004-233804.jsonl`) — ~20 minutes of wrap wall
burned on cold-compile timeouts immediately after the v0.17.1 bump,
recovered only as partial compiles warmed the cache. The T57 ALWAYS
rule (main gates run under `target-shared-main`, never conditionally)
already governs the post-merge and final gates; the TODO-edit GUARD
FLOOR is the one main-tree cargo run the rule's text does not reach —
its sentence sits two paragraphs away from the guard sentence and the
guard's own "(seconds)" framing reads as exempt-from-cargo-discipline.
This is the cycle-98/100 goal-gate cold-cache class (67e11fc, 7f94da2)
one level down, and it recurs structurally: every tag makes the next
wrap's guard runs cold.

## Repo context

- The guard sentence (LOOP-SPEC Phase 2 step 5, mid-paragraph): "…and
  the orchestrator runs `cargo test --test todo_consistency` (seconds)
  after every TODO.md edit, before committing." No `CARGO_TARGET_DIR`,
  no touch guard.
- Two paragraphs later the same step states the T57 main-dedicated-dir
  rule for post-merge re-runs
  (`CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-main`,
  ALWAYS) — the guard floor is a main-tree cargo run too, so the same
  dir is its correct slot: every builder in that dir is a main
  checkout, the TODO-edit guard only ever runs in main, and the dir is
  warm across cycles (the version bump stales the artifacts there too,
  but the warm-cache rebuild is one crate's test binary, not a cold
  default-dir full compile — and it shares the rebuild ACROSS the
  wrap's gate runs instead of four separate cold deaths).
- The T195 touch guard rides every shared-slot cargo invocation
  (`touch src/*.rs tests/*.rs;` first) — main-dedicated gates are
  EXEMPT per the exemption clause (every builder there is a main
  checkout), so the guard floor needs the env prefix, NOT the touch.
- The pin carrier is tests/loop_spec_recovery.rs — T227's leg "am"
  (the unpiped/pipefail sentence) lives there; this row's needles are
  the same sentence family (step 5's TODO-edit guard), one new leg.

## Requirements

1. **LOOP-SPEC step 5's guard sentence amended**: the
   `cargo test --test todo_consistency` invocation gains the
   `CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-main`
   env prefix, with the one-line why IN THE SENTENCE's flow (the T57
   main-dedicated dir; a version bump colds the default target by
   construction, the cycle-104 4x300s wrap timeouts named as the
   evidence) and the unpiped/pipefail rule unchanged (the T227
   sentence stays verbatim — this row amends the TARGET-DIR shape of
   the invocation, adjacent to it).
2. **Carrier pin leg** in tests/loop_spec_recovery.rs (new leg,
   T227-style): the amended invocation needle
   (`CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-main cargo test --test todo_consistency`)
   exactly-once, in-window (the step-5 TODO-edit guard neighborhood —
   after the T227 unpiped sentence's anchor region, within the same
   guard paragraph), and the T227 sentence's needles still green
   (amendment must not disturb the adjacent doctrine).
3. **RED-proof**: revert the env prefix from the sentence → the new
   leg dies; revert the whole guard sentence → the leg dies (not
   vacuous).
4. Scope: step 5's TODO-edit guard ONLY. Step 3's docs-only guard
   floor already carries "same T195 touch guard + env prefix as the
   full gate" — no edit there; the spec names this so the child does
   not wander.

## Tests

- The new pin leg green + RED-proven per req 3; the existing
  loop_spec_recovery legs (incl. T227's leg "am") stay green;
  todo_consistency green (the check line itself is linted).

## Acceptance

- The amended sentence lands with the evidence named; pin leg
  RED-proven; `cargo test --test loop_spec_recovery --test
  todo_consistency` green; no other LOOP-SPEC text touched; no TODO.md
  row-format change beyond the one row.

## Out of scope

- Any mechanical carrier (a scripts/ guard-floor wrapper — the §3
  watch-item's remedy if the BEHAVIORAL recurrence count reaches two;
  this row is the text fix), any change to the guard floor's test
  selection, step 3's docs-only floor, the T57 rule's own text.
