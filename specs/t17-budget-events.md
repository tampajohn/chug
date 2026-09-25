# T17 — Budget telemetry in `.chug/events.jsonl`: warning injections + configured budgets

check: cd /Users/jadams/workspace/chug && cargo test

## Why (evidence)

Cycle-4 evaluation (EVALUATION.md L1): the T15 glm child died at 40/40
iterations with its implementation complete but uncommitted
(`.chug/events-t15-impl-20260922-061957.jsonl`, abort event
2026-09-22T06:17:07Z). Whether the T13 budget-low warning ever fired for
that child — and with what remaining counts — is **unknowable**: the K2
harvest convention preserves child `events.jsonl` but not transcripts, and
the events stream records iterations/tool-results/aborts yet **not**
warning injections (the injection site has no `emit`). T13's one-shot
latches and T15's token leg are therefore empirically unverifiable per
session for exactly the sessions (children) whose transcripts don't
survive. Adjacent gap: `run_start` doesn't record the configured budgets,
so for *successful* runs (no abort event) even the ceilings are absent
from the jq-mineable record — "how close to the ceiling did this run
sail" is unanswerable post-hoc.

## Repo context

- `src/eventlog.rs` — the serializer. `EventLogSink::emit` (`:108`)
  matches `Event` variants → JSON lines; variants it doesn't care about
  return `None` and are dropped (see ModelText/ToolStart/LedgerChanged/
  SteeringQueued). `run_start(cwd, mode, spec, model)` (`:47`) writes the
  opening line. Tests at `:209+` (`read_lines` helper pattern).
- `src/driver.rs` — budget-low injection site `:495-516`: computes
  `remaining_iters/secs/tokens`, calls `budget_low_notice` (`:790+`), and
  on `Some(notice)` appends a user message to transcript + messages. The
  one-shot latches (`warned_iter/warned_time/warned_tokens`) update in the
  same block. `sink.emit(...)` is in scope throughout the loop (e.g.
  `:538-562`). `eventlog::run_start` is called at `:297`.
- `src/chat.rs` — chat's `run_start` call at `:175` (budgets live on the
  chat cfg knobs there); chat run_start test at `:538`.
- `src/events.rs` — `Event` enum (`:7+`) and `ConsoleSink` (`:167`);
  `src/tui.rs:57` `TuiSink`. Adding an enum variant will break exhaustive
  matches until each sink handles it — that's the compiler doing its job.
- Budget knobs: `max_iters: u32`, `max_minutes: u64`, `max_tokens: u64`
  with **0 = unset** for `max_tokens` (see `knobs.max_tokens > 0` at
  `src/driver.rs:499`).

## Requirements

1. **New event on injection.** Add an `Event` variant (suggested:
   `BudgetLow { remaining_iters: u32, remaining_secs: u64,
   remaining_tokens: Option<u64> }`). Emit it **iff a budget-low notice is
   actually injected** (same `if let Some(notice)` block), carrying the
   remaining counts at fire time. Because injections are one-shot per
   budget kind, a run can emit up to three such events; each injection
   emits exactly one.
2. **events.jsonl line.** `EventLogSink` serializes it as one line,
   e.g. `{"type":"budget_low","ts":…,"remaining_iters":N,"remaining_secs":N,
   "remaining_tokens":N|null}` — field naming consistent with the existing
   snake_case lines; `remaining_tokens` is `null` when no token budget is
   set (never a phantom number).
3. **No user-visible behavior change.** ConsoleSink and TuiSink ignore the
   new variant (the notice already reaches the user as a transcript
   message). Warning text, thresholds, and latch logic stay byte-identical
   — this row is telemetry only.
4. **Budgets on `run_start`.** Extend `run_start` so the opening line
   records the configured ceilings: `max_iters`, `max_minutes`, and
   `max_tokens` (`null` when unset — jq must distinguish unset from set).
   Both call sites (run `:297`, chat `:175`) pass their real values.
5. **README.** The "Events log" bullet gains the `budget_low` line and the
   new `run_start` budget fields.

## Tests

- eventlog unit: a `BudgetLow` emit lands as the specified line (incl.
  `remaining_tokens: null` when unset); the run_start test gains the
  budget fields (extend or add alongside
  `run_start_line_has_model_spec_cwd_mode`).
- scripted-loop test (driver.rs, pattern of the T13 `max_iters=8` scripted
  run): a warning-triggering run writes exactly one `budget_low` line with
  `remaining_iters <= WARN_REMAINING_ITERS` before it ends; a run that
  never approaches budget writes none.
- chat `run_start` budgets test analogous to
  `chat_session_opens_events_log_with_run_start`.
- All existing eventlog/driver/chat tests keep passing (the `run_start`
  signature change ripples to every caller and test).

## Acceptance

- `cargo build`, `cargo clippy --all-targets -- -D warnings`, `cargo test`
  all green.
- jq spot-check: a scripted tiny run shows both the budget fields on
  `run_start` and a `budget_low` line when the warning fires.
- Validation (LOOP-SPEC §2.4, REQUIRED — touches `src/driver.rs` +
  `src/events.rs`): mutation-test the new emit (dropping the emit,
  emitting-when-no-notice, wrong remaining values must each fail a test);
  as adjacency smoke, re-kill the carried T15 budget mutants (dropped
  `max_tokens > 0` guard; abort boundary).
