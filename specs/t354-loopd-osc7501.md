# T354 — loopd emits OSC 7501 status natively (kills the harness shim for every terminal harness)

estimate: ~110 changed lines (loopd.sh status-emission legs + fixtures + LOOP-SPEC note)

## Why

Fleet Primary live-verified 2026-11-09 (first real herdr spawn,
chug-f94dvv2hkw-1223): herdr's agent model = terminals whose program
reports status via OSC 7501. A bare `chug loop run` emits nothing —
status null, no status dot, absent from the Agents list. FP shipped a
fleet-side shim (`chug-loop-agent`, herdr-fleet-mods 252a6f8) that parses
loopd.log and re-emits 7501 — it works (both loops now report, and the
idle/working distinction is exact), but it is a wrapper every harness
must ship. If loopd emits 7501 natively, the shim dies for herdr AND for
every future terminal harness. This is the meta-harness map's
observation lane in its terminal-native form — sibling to T342's
file-based status mod, which serves harnesses that read files; 7501
serves harnesses that watch terminals.

## What

1. **Status emission points** in loopd.sh (map to the shim's proven
   semantics): cycle start / item start → `working`; cycle OK, goal
   complete, dormant → `idle`; delivery failing, gate red, error →
   `blocked` WITH the msg channel carrying the one-line reason (the
   T340 blocked-reason vocabulary — one source of truth for both
   surfaces).
2. **Wire format**: the id-less-root OSC 7501 format Fleet Primary's
   shim implements (herdr-fleet-mods repo 252a6f8 is the reference
   implementation; FP offered the exact bytes — the spec author should
   pull the shim's emission function and match it byte-for-byte).
3. **Gating (the compat contract)**: emit ONLY when (a) stdout is a TTY
   AND (b) CHUG_OSC7501 is unset or =1. Redirected logs (nohup, launchd,
   pipes) get ZERO escape bytes — log parsers must never see them.
   CHUG_OSC7501=0 disables even on a TTY.
4. **Docs**: docs/meta-harness.md gains the 7501 section (when to use
   7501 vs the T342 status files vs the T340 socket endpoints — the
   three observation channels and their audiences).

## Tests

- Emission fixtures: each loopd state transition emits the right
  sequence (asserted against a captured pty); the msg channel carries
  the blocked reason.
- Gating fixtures: piped stdout → zero bytes (byte-count assert);
  CHUG_OSC7501=0 on a pty → zero bytes; TTY + unset → emitted.
- Wire compat fixture: the emitted bytes parse under the shim's own
  parser (pull it from herdr-fleet-mods as the test fixture — the
  shim's parse of our native emission must equal its parse of the log).

## Compat

Default-safe: TTY-gated, env-killable, zero bytes in any redirected
context. Non-harness operation is byte-identical.

## Out of scope

- herdr-side changes (the shim retires on FP's schedule, not ours).
- OSC 7501 in the chug binary's one-shot runs (loopd only — one-shot
  `chug run` has its own lifecycle UX).

check: CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-validate-a cargo test --release --test loopd_osc7501
