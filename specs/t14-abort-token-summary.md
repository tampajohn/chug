# T14 — Cumulative tokens in console abort/goal output

check: cargo test

## Concern

`ConsoleSink` drops `Event::Usage` entirely (`src/events.rs`:
`Event::Usage { .. } => {}`), so a headless `chug run` prints **no token
totals** when it ends — not on `goal_complete`, not on abort. The data exists
(the driver emits cumulative `Usage` after every response; T10's
`.chug/events.jsonl` persists it per iteration), but an operator wrapping a
budget-dead run has to `jq` the events file to learn what the run cost. That
is exactly what the 2026-09-21 cycle-2 evaluation had to do to discover the
T11/T12 session burned **8,683,323 input + 1,243,749 output tokens** over 70
iterations (EVALUATION.md J2). TUI mode already shows live cumulative tokens
in the title bar; only the console sink is blind.

## Repo context

- `src/events.rs`: `ConsoleSink` already caches `last_ledger` from
  `Event::LedgerChanged` and prints it at `GoalAccepted` / `Aborted` — the
  same cache-and-print pattern applies to `Usage`.
- `Event::Usage { input, output }` values are **cumulative** totals for the
  run/turn (documented on the TUI arm: "The driver sends cumulative totals
  after each response").
- `ConsoleSink::with_writers` (existing `#[cfg(test)]` constructor) enables
  output-capture unit tests.

## Requirements

1. `ConsoleSink` caches the latest `Event::Usage` totals (latest wins).
2. On `Event::GoalAccepted`, print one line to stdout after the summary:
   `tokens: <input> in / <output> out (cumulative)`.
3. On `Event::Aborted`, print the same line to stdout alongside the existing
   `model:` / `budget:` lines.
4. If no `Usage` event was ever received (e.g. an abort before the first API
   response), omit the tokens line entirely.
5. No changes to the eventlog sink, TUI sink, chat mode, the `Event` enum, or
   the events.jsonl schema; no new flags or config.

## Tests

- `ConsoleSink::with_writers`: emit `Usage{input,output}` then
  `GoalAccepted` → stdout contains both totals.
- Emit `Usage` twice with different values, then `Aborted` → the printed
  totals are the **latest** ones.
- Emit `Aborted` with no prior `Usage` → output contains no `tokens:` line.

## Acceptance

- `cargo build`, `cargo clippy --all-targets -- -D warnings`, `cargo test`
  all green.
- README's abort-output / goal-complete description updated if it enumerates
  the printed lines.
