# T234 — delegate status goal latch is outcome-blind: terminal wake-set stale-latches on a goal-gate REJECTION

check: set -o pipefail; export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-validate-a; touch src/*.rs tests/*.rs; cargo test --bin chug; cargo test --test mcp_serve

estimate: ~150 changed lines all-in (delegate-row kind — summarize +
wake-set + render + schema text + ~6 test legs; the T124-era
renderer-adjacent changes landed ~2x the narrative: ~60-line
narrative priced at ~150)

## Concern

`delegate status`'s goal latch is outcome-blind. In
`summarize_events` (src/delegate.rs) the `"goal" =>` arm latches
`s.goal_seen = true` on ANY goal line and never parses the event's
`outcome` field — and the terminal wait's wake condition is absolute
presence (`now_summary.goal_seen || now_summary.abort_seen || …`).
So after a child's goal gate REJECTS (a `goal` line with
`outcome: "rejected"` — the driver then KEEPS RUNNING), every
subsequent `terminal: true` long-poll returns INSTANTLY, forever:
the doctrine's DEFAULT wait posture (LOOP-SPEC §2 step 2's terminal
long-poll) silently degrades to instant-polling exactly when the arc
gets interesting (the gate spoke, the child is steering and
re-verifying). Observed cost: the cycle-106 wrap's DX note — the
orchestrator "burned two instant polls before switching to
non-terminal waits"; and the latch was LIVE in both t232 children,
which each went rejected-then-accepted (digest: `goal: accepted 1 /
rejected 1` per stream). The sibling misrender — `state()` latching
"done" on a rejected line — is already defended at both production
consumers (mcp_serve's `chug_abort`/`chug_steer` consult LIVENESS
FIRST: src/mcp_serve.rs:1174's comment names the latch; the T157
fix-up pins), so this row changes the WAKE semantics and the status
payload's resolution, never the defended consumers' contract.

## Repo context

- The latch: `"goal" => s.goal_seen = true` in
  `summarize_events` (src/delegate.rs, ~line 950); the goal event
  carries `outcome` ("accepted" | "rejected") which the parser
  currently ignores. `summarize_collect` ALREADY distinguishes
  verdicts for the collect action — the status path is the only
  outcome-blind one.
- The wake-set: the terminal branch of the wait loop
  (`now_summary.goal_seen || now_summary.abort_seen || (now_existed
  && !entry_existed)`) — absolute presence, never a diff against the
  entry snapshot. The NON-terminal branch already uses the
  flip-detection shape (`significant_ne(&entry_summary)`).
- The T58 segment-reset: a new `run_start` clears
  `budget_low_seen` / `goal_seen` / `abort_seen` /
  `abort_reason` — the new flags must reset the same way.
- The render: `status` prints `state:`, `goal_seen:`, etc.; the
  DelegateSummary struct + `state()` are `pub(crate)` and consumed
  by src/mcp_serve.rs (compact renderer + the two control verbs)
  and pinned by tests/mcp_serve.rs (`goal_seen: true` at line 218)
  — `goal_seen` and `state()` semantics stay BYTE-COMPATIBLE.
- The schema text: the delegate tool description and the
  `terminal` property description live INLINE in src/tools.rs
  (lines ~158 and ~175) and are token-pinned in
  src/delegate/tests/schema.rs (line 84: `goal_seen` &&
  `abort_seen` presence).
- The delegate tests are a directory module:
  src/delegate/tests/{summary,status,wait,wait_terminal,schema}.rs
  (`mod tests` at src/delegate.rs:1489).

## Requirements

1. **Outcome-resolved latches (additive, never removing).**
   `DelegateSummary` gains `goal_accepted_seen: bool` and
   `goal_rejected_seen: bool`; `summarize_events` parses the goal
   line's `outcome` field and sets the matching flag. `goal_seen`
   KEEPS its any-goal latch (mcp_serve's LIVENESS-FIRST contract
   and `state()` are pinned consumers — both UNCHANGED). A goal
   line with a missing/unparseable `outcome` sets `goal_seen` only
   (fail-safe to today's behavior).
2. **T58 segment-reset covers the new flags** — a new `run_start`
   clears all five verdict flags (the resume case: a pre-resume
   rejection must not bleed into the resumed segment's summary).
3. **Terminal wake-set: rejections wake ONCE, stale rejections
   never relatch.** The terminal branch becomes: wake on
   `goal_accepted_seen` OR `abort_seen` OR
   (`goal_rejected_seen` && NOT the entry snapshot's
   `goal_rejected_seen`) OR events-file creation. A rejection
   already present AT ENTRY does not wake (it is stale news); a NEW
   rejection mid-wait wakes once (the orchestrator wants to steer
   immediately) — and because the waking payload becomes the next
   call's entry, subsequent terminal waits block normally.
   Non-terminal (`significant_ne`) semantics unchanged except the
   two new fields join the significant-field set (a rejection flip
   is significant telemetry there too).
4. **Render the resolution.** The status payload gains
   `goal_accepted_seen: true` / `goal_rejected_seen: true` lines
   when the corresponding flag is set (absent when false, matching
   the existing flag-render convention), so a polling orchestrator
   can distinguish "gate spoke: REJECTED, child still running"
   from "accepted, done" without reading the raw stream. Placement
   adjacent to the existing `goal_seen:` line; pinned.
5. **Schema text names the new semantics.** The delegate tool
   description's status sentence and the `terminal` property
   description (both src/tools.rs) are updated: the terminal wake
   set is "an ACCEPTED goal verdict, an abort, a NEW rejection
   since the wait began, liveness alive→dead, or events-file
   creation" — and the description keeps every load-bearing token
   the schema pins assert (update the pins deliberately, never
   delete an assertion to silence it).
6. **RED-proofs, named mutants**: (a) outcome ignored (the pre-row
   behavior — `goal_seen` drives the terminal wake) must DIE on the
   new "rejected-at-entry does not instant-return" wait leg;
   (b) `goal_accepted_seen` never set must die on the existing
   accepted-goal wake leg; (c) rejected-wakes-EVERY-poll (no entry
   gating) must die on a "second terminal wait blocks" leg;
   (d) segment-reset forgetting the new flags must die on a
   resumed-segment leg. RED outputs in the commit message.

## Tests

- src/delegate/tests/summary.rs: rejected-only stream →
  `goal_seen: true` (compat) + `goal_accepted_seen: false` +
  `goal_rejected_seen: true`; accepted stream → all-goal flags
  consistent; missing-outcome goal line → `goal_seen` only;
  run_start reset clears the new flags.
- src/delegate/tests/wait_terminal.rs: (i) rejected-at-entry +
  nothing further → NO early wake (blocks to a short deadline,
  does not instant-return — the req-6(a) non-vacuousness leg);
  (ii) a NEW rejection landing mid-wait wakes once; (iii) the
  existing accepted-goal / abort / liveness-flip /
  events-creation wake legs stay green byte-for-byte.
- src/delegate/tests/status.rs + schema.rs: the render lines (req
  4) and the schema text tokens (req 5).
- tests/mcp_serve.rs stays green UNMODIFIED unless a pin asserts
  text this row deliberately changes — any such edit is named in
  the commit message with its reason.
- Check line green (`cargo test --bin chug` covers the delegate
  directory module + tools.rs unit tests; `cargo test --test
  mcp_serve` covers the pinned compact-render consumer).

## Acceptance

- After a goal-gate rejection, a `terminal: true` status wait no
  longer instant-returns on the stale rejection; the wake set
  matches req 3 exactly; the payload distinguishes
  accepted-vs-rejected; `goal_seen`/`state()` byte-compatible;
  check line + clippy `-D warnings` clean.

## Out of scope

- `state()`'s any-goal "done" latch and the mcp_serve consumers'
  LIVENESS-FIRST contract (pinned, defended, unchanged — a
  semantics change there is its own row with the T157 pins on the
  table), `summarize_collect` verdict handling (already
  outcome-aware), the launch/collect actions, any LOOP-SPEC
  doctrine text (the §2 step-2 polling paragraph already says
  "goal or abort verdict" — no doctrine edit needed).
