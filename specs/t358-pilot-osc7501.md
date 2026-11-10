# T358 — OSC 7501 emission in loopd-pilot.sh (the supervisor spawned loops actually run)

estimate: ~80 changed lines (src/loop_cmd/loopd-pilot.sh + emission helper sharing + fixtures)

check: CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared cargo test --release --test loopd_osc7501

## Why

T354 landed its OSC 7501 emission in the repo's `loopd.sh` (K7's own
supervisor — 20 references, working). But Fleet Primary's live proof on
F94 showed a bare `chug loop run` reports `status: None` in herdr —
because `chug loop run` does NOT use the repo loopd.sh: it embeds
`src/loop_cmd/loopd-pilot.sh` at build time (`include_str!` in
src/loop_cmd.rs:71) and that template (last touched by T334) has ZERO
7501 references. Every fleet/herdr/pilot spawn runs the pilot template,
so T354's promise — "kills the harness shim for every terminal harness" —
is currently true only for K7 itself. This item closes the coverage gap.

## What

1. **Port the emission** into `src/loop_cmd/loopd-pilot.sh`: the same
   state map (cycle/item start → working; cycle-OK/goal-complete/dormant
   → idle; error/delivery-fail → blocked+msg), the same TTY gate
   (stdout TTY + CHUG_OSC7501 unset-or-1, =0 kills), the same msg text
   source (the T340 blocked-reason vocabulary).
2. **Share the helper**: the emission function must not be a second copy
   that drifts — extract it where both supervisor sources source it
   (scripts/osc7501.sh materialized beside both, or generated from one
   template; implementer's choice, but a fixture proves the two sources
   emit byte-identical sequences for the same transition).
3. **Acceptance (external)**: Fleet Primary re-runs its live
   verification on F94 against a release carrying this item — bare
   `chug loop run` (no wrapper) shows in herdr's Agents list with
   working/idle transitions exact and blocked carrying the msg. Cite
   their run as the closing evidence.

## Tests

- Pilot-template fixtures (pty): each transition emits; piped stdout →
  zero bytes; CHUG_OSC7501=0 → zero bytes.
- Parity fixture: the same canned transition through loopd.sh and
  loopd-pilot.sh produces byte-identical 7501 output (the drift guard).
- Compat: the emitted bytes parse identically under the reference
  parser in herdr-fleet-mods docs/osc-7501.md (the T354 fixture pattern).

## Compat

Same gating contract as T354: TTY-only, env-killable, zero bytes when
redirected. No change to non-harness operation.

## Out of scope

- herdr-side changes (the wrapper's 7501 leg retires on FP's schedule
  once this ships and verifies).
- The next release cut (the loop's normal release cadence carries it).
- The heartbeat's pilot port (T356's line surface stays loopd.sh-only;
  the shared emitter keeps the 7501 bytes ONLY — loopd.sh's
  `loopd_osc7501_emit` stays the T356 composition wrapper that calls the
  shared emitter AND the heartbeat legs, so the pilot sourcing the shared
  file emits dot-only. This shape is what makes parity provable by
  construction: ONE function body produces the bytes in both supervisors).

## Fixture home + helper shape (orchestrator note at dispatch)

- Fixtures EXTEND `tests/loopd_osc7501.rs` (reuse the T354 pty/parse
  harness — the parity leg needs both supervisors in one test anyway);
  do not duplicate the harness into a new target.
- The shared file is `scripts/osc7501.sh`, materialized beside the pilot
  supervisor by `write_supervisor_assets` exactly like the T205/T330
  loader fragments (`include_str!("../scripts/osc7501.sh")`, no copy to
  drift); loopd.sh sources `$(dirname "$0")/scripts/osc7501.sh`.
- The pilot gains the `osc` probe subcommand (the T354 pattern: fixtures
  drive the real gate, never a reimplementation).
- Blocked derivation = spec's option (a): on a failed cycle, scan the
  NEWEST `cycle-*.log` for the failure patterns (the msg carries the last
  matched line, headed by its T340 blocked-reason word); the
  failure-marker-only-in-cycle-log fixture is RED-proven against a
  supervisor-stream-only derivation.

## Field finding (Fleet Primary transition suite, 2026-11-10)

**Failure markers do NOT reach the supervisor stream.** 'observability
delivery failing', errors, and panics land in the per-cycle log
(`.chug/loopd/cycle-*.log`), NOT in `loopd.log` — FP's wrapper only
fired blocked+msg after scanning the NEWEST cycle log (3a802e4). A
blocked-state derivation that reads only loopd.log will silently never
fire. REQUIRED in this item (pick one, test either way):
  (a) the emission scans the newest cycle-*.log for the failure
      patterns in addition to the supervisor stream, OR
  (b) loopd mirrors failure lines into loopd.log (rides T356's
      heartbeat — one mirrored line per failure, source string shared).
Fixture REQUIRED either way: a failure marker that exists ONLY in a
cycle-*.log produces blocked+msg (RED-proven against a
supervisor-only derivation).
