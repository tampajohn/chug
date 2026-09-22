# T13 — Budget-low warning before abort

check: cargo test

## Concern

Budget deaths keep landing in the **wrap phase** — after the code is done but
before the commit / gates / TODO flip — because the loop enforces budgets at
the top of each iteration with no advance signal, so the model cannot
reprioritize toward committing and bookkeeping while budget remains
(EVALUATION.md J1, 2026-09-21 cycle 2):

- T10 budget-aborted with complete, green, **uncommitted** work; the operator
  harvested it post-mortem (`a115a71` todo note).
- The T11/T12 self session budget-aborted at 40 iterations, was resumed, and
  budget-aborted **again** 30 iterations later, immediately after the T12
  code commit — the operator ran the gates and flipped the row
  (`.chug/events-20260922-032901.jsonl` aborts at `03:15:40Z` / `03:19:09Z`;
  `.chug/LEDGER-20260922-032901.md`).
- LOOP-SPEC §2.5 ("orchestrator owns the books") mitigates this
  process-side; this item fixes it in-harness.

## Repo context

- `src/driver.rs`: `drive_loop` checks `iteration >= knobs.max_iters` and
  `start.elapsed() >= max_minutes` at the top of the loop (~lines 374-398)
  and calls `abort_exit` — with no prior notice to the model.
- The loop already injects user messages mid-run (steering notes,
  `append_steering_notes`; goal-rejection feedback) — the warning rides the
  same mechanism: push a `Message::user`, `transcript::append` it.
- Chat mode shares `drive_loop` (budgets are per-turn there) — the warning
  applies to both modes with no special-casing.

## Requirements

1. When the remaining **iteration** budget first drops to
   `<= WARN_REMAINING_ITERS` (new const, `5`) OR the remaining **wall-clock**
   budget first drops to `<= WARN_REMAINING_SECS` (new const, `300`), inject
   exactly one user message before the next LLM call, e.g.:
   `chug: budget low — N iteration(s) and M minute(s) remain. Stop starting
   new work: commit what is done, run the gates, and finish bookkeeping now.`
2. **One-shot per budget kind** per drive_loop invocation (two flags; each
   fires at most once — never spam the transcript every iteration).
3. The message states the actual remaining counts at fire time.
4. Both autonomous and chat mode get the warning (shared loop); no new CLI
   flags, no config, no new `Event` variants, no changes to abort behavior,
   exit codes, or the events.jsonl schema.
5. Put the fire/no-fire decision in a small pure helper (e.g.
   `budget_low_notice(remaining_iters, remaining_secs, already_warned_iter,
   already_warned_time) -> Option<String>`) so the time half is unit-testable
   without sleeping.

## Tests

- Scripted `drive_loop` (existing `ScriptedLlm` harness) with
  `--max-iters 8` (or similar): the transcript gains exactly one budget-low
  user message, first appearing at the iteration where remaining == 5, none
  before that boundary, and never a second one.
- Pure-helper unit tests: iteration-threshold boundary (6 remaining → None,
  5 → Some), time-threshold boundary, one-shot flags suppress repeats, the
  message interpolates the actual remaining counts.
- Existing abort tests stay green (abort behavior unchanged).

## Acceptance

- `cargo build`, `cargo clippy --all-targets -- -D warnings`, `cargo test`
  all green.
- A scripted run reaching the threshold shows the notice in its transcript;
  abort output and exit codes are otherwise identical to before.
