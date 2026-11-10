# T360 — OSC 7501 blocked leg: emission must scan cycle logs + native app= label

estimate: ~90 changed lines (loopd/pilot emission legs + fixtures)

## Why

Fleet Primary's pre-registered acceptance suite against v0.25.0
(2026-11-10): cycle-start=working GREEN, completion=done GREEN, but the
dead-langfuse blocked leg is RED — the cycle log carries
`observability delivery failing (run continues): ... Connection
refused` verbatim, the cycle still completes ("cycle OK") and goes
dormant, and the native emission (supervisor-stream-derived) reports
done, never blocked. This is EXACTLY the field finding encoded in
2535e8af (failure markers live in cycle-*.log, not the supervisor
stream) — the T358 implementation shipped the supervisor-stream path
only, without the required cycle-log scan and without T356's mirrored
line (which would heal the supervisor stream). The wrapper's 7501 leg
correctly stays load-bearing (its cycle-log scan is the only working
blocked+msg path today). Secondary: native bytes omit `app=`, so herdr
shows `app: None` vs the wrapper's `app=chug` (Agents-list grouping).

## What

1. **Blocked derivation honors 2535e8af** (both supervisor sources —
   repo loopd.sh AND loopd-pilot.sh, parity fixture still binding):
   the emission watches the newest `.chug/loopd/cycle-*.log` for the
   failure patterns (observability delivery failing, error, panic,
   gate red) IN ADDITION to the supervisor stream. On a match: emit
   `blocked` with the msg channel carrying the marker line (the same
   source string T356's blocked heartbeat uses). A fresh clean cycle
   start clears the block (the docs/osc-7501.md contract).
2. **RED fixture (the acceptance FP already wrote)**: a failure marker
   existing ONLY in a cycle-*.log produces blocked+msg; a
   supervisor-only derivation fails this test first. FP's dead-langfuse
   case is the live acceptance on the carrying release.
3. **app= label**: the native emission carries `app=chug` (herdr
   Agents-list grouping parity with the wrapper).
4. Note: T356's heartbeat mirror (option (b), failure lines into the
   supervisor stream) remains desirable but is NOT this item — this
   item takes option (a), the cycle-log scan, because the wrapper
   already proves it works.

## Tests

- Cycle-log fixture: marker only in cycle-*.log → blocked+msg with the
  marker line as msg; clean cycle start → block clears.
- Parity fixture: repo supervisor and pilot template produce identical
  blocked bytes for the same cycle-log content.
- RED proof: the fixture fails against a supervisor-only derivation.
- app= fixture: native bytes carry app=chug.

## Compat

Same TTY/redirect gating; no change to working/idle/done emission;
blocked is additive (never emitted today, so nothing changes until a
failure occurs).

## Out of scope

- T356's supervisor-stream mirror (option (b) — separate item if the
  scan proves costly).
- Wrapper retirement (FP's schedule, on 3/3 green).
