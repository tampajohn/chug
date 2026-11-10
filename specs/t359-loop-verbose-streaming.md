# T359 — loop verbose streaming: turn-detail tiers for the PTY (CHUG_LOOP_VERBOSE levels)

estimate: ~100 changed lines (loopd.sh verbose legs + cycle-log tee + fixtures + LOOP-SPEC note)

## Why

Operator 2026-11-10: "I would like the chug PTY interface exposed to
herdr to have a flag that shows verbose details about the turns, maybe
streaming the logs to the interface if folks want to see more of what's
going on?" T356 gave the terminal compact heartbeat lines (cycle/item/
milestone/flip/blocked). That answers "is it alive" but not "what is it
actually doing" — the turn-level content (tool calls, actions, model
text) still lives only in cycle-*.log files. The herdr tab is a PTY;
humans watching it want a dial between silent (redirected), compact
(heartbeat), and full detail (the turn stream).

## What

1. **Verbosity levels** (one knob, `chug loop run --verbose[=<n>]` or
   CHUG_LOOP_VERBOSE=<n>):
   - **0** — silent (current redirected behavior everywhere)
   - **1** — compact heartbeat (the T356 default on a TTY)
   - **2** — verbose: level 1 PLUS per-turn lines — each tool call
     (`[loop] T042 iter 37 — tool bash: <command summary, 80 chars>`),
     each model text block truncated to ~3 lines, errors in full
   - **3** — trace: the cycle's full output streamed (tee of the
     cycle-*.log content to stdout as it is written)
2. **Streaming mechanism**: the cycle's chug run already writes
   cycle-*.log; at level ≥2 the supervisor tees NEW lines to stdout as
   they land (tail-follow of the cycle log is acceptable; direct
   pass-through if the exec shape allows — implementer's choice, but
   latency ≤2s and no double-writing when the run itself prints to the
   TTY already).
3. **Gating (the standing contract)**: stdout must be a TTY; redirected
   output gets level 0 regardless of the flag (logs never polluted).
   The T354 7501 emission is unaffected (dot + line stay in sync — the
   heartbeat's source-string rule extends to level 2 summaries).
4. **Docs**: docs/meta-harness.md observation section gains the levels
   table; the loopd.sh header comment documents the default (1 on a
   TTY, 0 otherwise).

## Tests

- Pty fixtures per level: 0 = zero lines; 1 = compact only (≤15/cycle);
  2 = tool-call + text lines present with truncation markers; 3 = full
  stream matches the cycle log.
- Gating fixtures: piped stdout at level 3 → zero bytes; the 7501 msg
  text still equals the level-1 blocked line.
- Latency fixture: a cycle-log line reaches stdout ≤2s after write at
  level ≥2.

## Compat

Default behavior is byte-identical to today (level 1 on TTY, 0
redirected); the flag/env is the only way in. No change to cycle logs,
7501 emission, or the supervisor's own log file.

## Out of scope

- Filtering/redaction of the streamed content (the loop's existing
  no-secrets discipline applies; a scrub pass is a separate item if
  ever needed).
- Per-turn interactive control (pause/steer from the terminal —
  separate feature).

## Check

check: CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-validate-a cargo test --release --test loopd_verbose --test loopd_heartbeat --test loopd_osc7501
