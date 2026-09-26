# T68 — `delegate status` `wait_secs` wakes on significant change only, not per-tool-call churn

## Repo context

T29 (cycle 13) added the `wait_secs` long-poll to `delegate status` because
"cycle 10 spent 9/68 iterations (~13%) on instant status polls… idle
waiting is the loop's largest remaining mechanical token sink". LOOP-SPEC
§2 step 2 tells orchestrators to pass `wait_secs: 90` "to collapse each
idle wait window into one blocking status call".

The implementation (`delegate_status_wait`, src/tools.rs ~:922) returns
early when `now_summary != entry_summary` — **any** `DelegateSummary`
field differing (src/tools.rs ~:986). But the struct (src/tools.rs
~:1065) includes `last_event_type` + `last_event_ts`, and an active child
appends `tool_result` events every 2–10 s (an impl child mid-edit emits
bursts of read/edit/bash results). The wake therefore fires at the first
2.5 s poll tick almost every time: observed across cycles 29 AND 30 —
every long-poll with `wait_secs: 90`–`110` woke at 2–7 s (cycle-29 eval
watch item; cycle-30 wrap note d: "fired on EVERY long-poll… pacing fell
back to bash sleeps + instant status"). The T29 premise is unrealized:
the loop still spends an orchestrator iteration per few seconds of child
waiting instead of one per wait window, and cycles wrap at 114–115/120
with `budget_low` at 8 — those iterations are the scarce budget.

The rendered payload is fine — `last_event` churn is worth SHOWING; it
just isn't worth WAKING for.

## Requirements

1. **Significant-change wake set.** The early-return condition compares
   only: `max_iters`, `last_iteration`, `budget_low_seen`, `goal_seen`,
   `abort_seen`, `abort_reason` (a `DelegateSummary`-level helper, e.g. a
   significant-fields sub-struct or a `significant_ne(&self, other)`).
   Changes confined to `last_event_type`/`last_event_ts` MUST NOT wake the
   wait. The three other return legs are unchanged: events-file creation
   when missing at entry, liveness flip alive→dead, deadline (with the
   final pre-render read, so the deadline payload still reflects the true
   final state — churn included).
2. **Rendered payload unchanged in shape.** Instant leg and wait leg render
   the same fields as today (T29's byte-identical instant-leg pin and the
   `waited: <n>s` line stay green UNCHANGED), including `last_event` — only
   the wake condition narrows.
3. **Doc honesty.** (a) the T29 doc comment above `delegate_status_wait`
   (which currently names "any DelegateSummary field differing (new
   last_event, …)") is reworded to the significant set; (b) the `delegate`
   tool schema description's wait_secs sentence ("returning early when the
   child's events state changes or its liveness flips to dead" /
   "waiting for a child state change") is reworded to name the significant
   set (e.g. "returning early when the child's iteration advances, a
   verdict or budget-low flag appears, or its liveness flips to dead") —
   keep T22/T41-style live-schema pins green or update them in the SAME
   diff with non-vacuousness; (c) the README `delegate` paragraph's
   wait_secs clause ("it returns early when the child's state changes or
   its liveness flips to dead") gets the same rewording.
4. **Do not regress T29/T58.** All existing delegate wait/status pins
   (byte-identical instant leg, early return on an `iteration` append —
   `last_iteration` advance IS significant — deadline return with
   unchanged entry state + `waited:` bounds, liveness flip, latest-segment
   latches, resume run_start keep-last-seen T64 pins) stay green.

## Tests

- **New churn pin:** fixture with run_start; a writer thread appends ONLY
  a `tool_result` line mid-wait (last_event churn, no significant-field
  change); `wait_secs: 2`–`3` must NOT return early — it returns at the
  deadline (elapsed ≥ the requested wait within slack, `waited:` ≈ the
  request) and the rendered payload carries the NEW last_event (proving
  the deadline's final read, not an entry-snapshot render).
- **New significant-wake pin (if not already covered):** a mid-wait
  `goal` (or `budget_low`) append returns early carrying the flag.
- **Non-vacuousness, recorded by the child:** reverting the condition to
  any-field-diff turns the churn pin red (returns at ~2.5 s, far under
  the asserted lower bound), revert → green; gutting the significant
  comparison so nothing wakes turns T29's existing iteration-append pin
  red.

## Acceptance

- Gates green in the worktree: build + clippy `--all-targets --
  -D warnings` + full `cargo test`.
- Both new pins land with the req-4/T29 regression evidence recorded.
- Tool schema description, doc comment, and README clause all name the
  significant set (no stale "state changes" phrasing at any of the three
  surfaces).
- TODO.md/LEDGER.md untouched (orchestrator's).

check: cargo test --bin chug delegate_status && cargo test --bin chug delegate_summary
