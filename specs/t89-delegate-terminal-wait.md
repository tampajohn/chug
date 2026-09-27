# T89 — delegate status terminal-wait mode + LOOP-SPEC adoption

## Repo context

`src/delegate.rs` (~3,500 lines) implements `delegate` (T23 launch/status,
T29 `wait_secs` long-poll, T68 significant-change wake). The wait's wake set
(`DelegateSummary::significant_ne`) is `max_iters`, `last_iteration`,
`budget_low_seen`, `goal_seen`, `abort_seen`, `abort_reason` — so a long-poll
on an actively-working child **wakes on every child iteration advance**
(~15–90s for a glm child). `last_event` churn deliberately never wakes (T68).

The loop-level economics of that wake set are now the measured problem:

- Cycle 48 (`events-20260927-210537.jsonl`): **116 delegate calls out of 157
  iterations**; cycle 49 (`events-20260927-215944.jsonl`): **123 of 158**.
  ~75% of the orchestrator's iterations are polls.
- Every poll is one orchestrator iteration = one full-context LLM round trip
  (265–470k cumulative input tokens late-cycle).
- Ceiling pressure at the 160 cap (T36 raised 120→160 in cycle 16): cycle 44
  ran 158/160 (budget_low fired), **cycle 45 died 160/160 with goal never
  claimed** (wrap lost; cycle 46 reconstructed the T81/T82 state), cycle 48
  157/160, cycle 49 158/160 (both budget_low-forced wraps with children in
  flight). Cycle 50 ran 78/160 only because it was a pure mechanical
  recovery.
- T29/T68 collapsed the WALL-clock cost of waiting; the ITERATION cost is
  untouched: with wake-on-advance, waiting on a 40-iteration child costs the
  orchestrator ~40 of its own iterations even with `wait_secs: 600`.

What the orchestrator actually needs while a child flies is exactly four
facts: the child claimed its goal, the child aborted, the child died, or the
wait deadline passed. Iteration advances and budget-low flags are progress
telemetry the orchestrator does not act on mid-child (review happens
post-exit; budget-death recovery starts at the abort). This spec adds an
opt-in wait mode that wakes on exactly those four, then makes it the loop's
default wait posture.

## Requirements

1. **New optional field `terminal`** (boolean, status action only, default
   false). With `terminal: true` AND `wait_secs > 0`, the wait blocks until
   the FIRST of: (a) `goal_seen` flips true, (b) `abort_seen` flips true
   (including `abort_reason` appearing), (c) observed liveness flips
   alive→dead, (d) the events file is created when it was missing at entry
   (the launch→build window), (e) the deadline elapses. Iteration advances,
   `budget_low_seen` flips, and `max_iters` appearance do **NOT** wake a
   terminal wait.
2. **Rejection legs**: `terminal: true` with `wait_secs` absent or 0 is a
   tool error naming that terminal waits need `wait_secs > 0` (the instant
   leg already exists — a terminal instant poll is a contradiction; corrective
   error, not silent degradation). `terminal` on `launch` or `collect` is the
   same style of rejection the `wait_secs` arm uses today.
3. **Default byte-identical**: `terminal` absent/false keeps the pre-T89
   significant-change semantics exactly (pinned — T29/T68's contract is
   load-bearing for active progress monitoring).
4. **Render unchanged**: same status payload plus the one `waited: <n>s`
   line; the wake cause is carried by the summary flags themselves
   (`goal_seen: true` etc.), no new render lines.
5. **LOOP-SPEC adoption** (this item IS doctrine — it runs alone, no other
   child in flight): §2 step 2's polling paragraph gains the terminal-wait
   posture as the DEFAULT wait — after launch, long-poll with
   `terminal: true` + a long `wait_secs` (up to the 600 cap); significant-wake
   and instant polls remain for active monitoring (e.g. watching a known-
   flaky child); the paragraph names the iteration-economics rationale (one
   orchestrator iteration per child run, not per child iteration) in one
   clause. The anti-sprint-burn guard is untouched.
6. **README**: the `delegate` paragraph gains one clause for terminal waits
   (integrated, same density as the `wait_secs` clause).

**Pin sweep (impl child's duty)**: the LOOP-SPEC §2 step-2 polling sentence
may be pinned in `tests/loop_spec_*.rs` — grep the pin carriers before
editing, update any pin that wraps the edited sentence in the SAME commit
(the T72 in-place-hunk + pin-file pattern), and RED-prove each changed pin.

## Tests

In `src/delegate.rs`'s test module, RED-proven:

- terminal wait does NOT wake on an iteration advance (fixture events stream
  whose `last_iteration` bumps between poll ticks; the wait runs to a short
  deadline) — and the same fixture DOES wake a significant-mode wait
  (non-vacuousness).
- terminal wait wakes on `goal_seen` flip; on `abort_seen` flip; on
  liveness alive→dead (real-process leg per the existing liveness tests);
  on events-file creation when missing at entry.
- terminal wait does NOT wake on a `budget_low_seen` flip (deadline leg).
- `terminal: true` without `wait_secs` → tool error naming the requirement;
  `terminal` on launch → the rejection arm.
- default (no `terminal`) significant-wait behavior byte-identical
  (existing T29/T68 pins green unmodified).
- schema legs: `terminal` described as status-only, boolean, default false.

## Acceptance

- `cargo build && cargo clippy --all-targets -- -D warnings && cargo test`
  green; LOOP-SPEC/README edits land with any wrapped-pin updates in the
  same commit; the spec's own check passes.

check: cargo test --bin chug delegate
