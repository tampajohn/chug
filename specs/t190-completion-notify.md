# T190 — completion notifications (run end + validation verdicts)

check: cargo test

estimate: ~300 lines (notify mechanism + config + tests)

## Concern

Operator 2026-10-01: chug is async-first, but the operator only learns
outcomes by polling. Default use needs a nudge: run completed (goal or
abort), validation verdicts, release published. macOS has osascript
banners; the Laya notification hook (layad) already judges push/silent
for Claude Code hooks and is running on F94.

## Repo context

- `src/eventlog.rs` / events: GoalAccepted, Aborted (with model/budget),
  validation verdicts — the emit points exist; this adds a sink.
- SPEC-8 observability: fire-and-forget pattern (bounded queue, never
  blocks the run) — the notify path mirrors it.
- layad on F94: /hook/notification judges push-vs-silent and fires the
  macOS banner (separates 0.86 vs 0.40) — chug can POST the same shape,
  or shell `osascript -e 'display notification'` directly.
- config: .chug/notify.json (opt-in, off by default).

## Requirements

1. `.chug/notify.json`: {enabled, sink: "osascript"|"layad", events:
   [goal|abort|validation|release], min_duration_secs} — absent/disabled
   = zero behavior change (default off).
2. Fires on: goal accepted (with summary line), abort (with budget/cause),
   validation VERDICT PASS/FAIL (item + verdict), release tag pushed.
   Short-run suppression: runs under min_duration_secs don't notify goal.
3. Implementation: fire-and-forget (bounded channel + drop-on-full, like
   observability); a notify failure never changes run behavior and logs
   once to events.jsonl.
4. osascript sink: `display notification "<title>" with title "chug"`.
   layad sink: POST /hook/notification (reuses its push/silent judgment).
5. loopd integration: supervisor forwards nothing — notifications fire
   from the child chug processes themselves (per-run config).

## Tests

- Config parse: absent file = off; malformed = off + one warn (T90
  fail-open pattern).
- Fire legs: goal → one notification with summary; abort → cause named;
  short-run under min_duration → suppressed.
- Sink failure: notification dropped, run unaffected, one events.jsonl
  note (never repeated per run).

## Out of scope

- Phone push (bridge has no send route — parked); Slack/email sinks;
  per-iteration notifications (verdict-class events only).
EOF