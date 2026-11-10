# T356 — loop terminal heartbeat: compact live status lines on stdout (TTY-gated)

estimate: ~90 changed lines (loopd.sh heartbeat emission + fixtures + LOOP-SPEC note)

## Why

Operator report 2026-11-09 (watching the herdr-spawned loop): "in the
chug log / terminal view, i don't see any real info about what's going
on." Mechanism confirmed: loopd.sh writes everything to
`.chug/loopd/loopd.log` and prints nothing to stdout — a terminal
watcher (herdr tab, a human tail) sees a silent process. T354 (OSC
7501) gives herdr the status DOT; this gives the human the status LINE.
Observation is the integration's missing third channel (files T342,
socket T340, escape-status T354 — the terminal itself has nothing).

## What

1. **Heartbeat lines** on stdout at state transitions (appended lines,
   never carriage-return redraws — herdr keeps scrollback):
   - cycle start: `[loop 14:32:05] cycle 7 routing=eval (kimi) item=T042
     — <item title, 60 chars>`
   - iteration milestones: every 25 iterations, `[loop] T042 iter 50/200
     — <last tool/action, 60 chars>`
   - flips: `[loop] T042 done (merge abc1234) — validator PASS`
   - dormant: `[loop] queue empty — dormant, next probe 300s`
   - blocked: `[loop] BLOCKED: <reason>` (matches the T354 msg channel
     text — one source string, two destinations).
2. **Gating (same contract as T354)**: stdout TTY AND CHUG_LOOP_VERBOSE
   unset-or-1; redirected output gets NOTHING (logs stay clean);
   CHUG_LOOP_VERBOSE=0 silences even on a TTY.
3. **Single source**: the heartbeat text and the T354 7501-msg are
   composed by the same helper — the dot and the line never disagree.

## Tests

- Pty fixtures: each transition emits its line with cycle/item/content;
  piped stdout → zero lines (byte assert); verbose=0 → zero lines.
- Consistency fixture: for a blocked transition, the heartbeat text ==
  the 7501 msg text (same helper output).
- Volume guard: a 200-iteration cycle emits ≤ 15 lines (no scrollback
  spam).

## Compat

Zero output when redirected (today's behavior everywhere non-TTY);
opt-out on TTY via env. No other behavior change.

## Out of scope

- A full TUI (the heartbeat is lines, not a dashboard; a TUI is a
  separate, much larger item).
- Remote/streamed viewing (the T340 socket already serves machines).

## Field finding (Fleet Primary transition suite, 2026-11-10)

Failure markers land in the per-cycle log, not the supervisor stream
(see T357's identical finding). The heartbeat's BLOCKED line must
therefore source from the newest cycle-*.log (or loopd must mirror the
failure line — in which case the heartbeat carries the mirrored line).
Same RED-proven fixture required: cycle-log-only failure marker → the
blocked heartbeat fires.

check: CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared cargo test --release --test loopd_heartbeat
