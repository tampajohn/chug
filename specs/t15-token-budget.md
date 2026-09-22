# SPEC — T15: token-denominated budget (`--max-tokens`)

## Repo context

chug's run loop has exactly two budget ceilings, both checked at the top of
each iteration in `src/driver.rs` (~:386-408): `max_iters` and
`max_minutes`, surfaced as `Event::Aborted` with
`BudgetExceeded::Iterations { max }` / `BudgetExceeded::Minutes { max }`
(`src/events.rs` ~:70-101). T12 made the abort output name the model and
the exhausted budget (`budget: N iterations|minutes`); T13 added a
one-shot budget-low warning (`budget_low_notice`, `src/driver.rs` ~:748;
`WARN_REMAINING_ITERS = 5`, `WARN_REMAINING_SECS = 300`, latches
`warned_iter`/`warned_time` at ~:464-476); T14 prints cumulative tokens at
abort/goal-complete via ConsoleSink caching the latest `Event::Usage`.

The driver already maintains cumulative token counters: `usage_in` /
`usage_out` (`src/driver.rs` ~:380, accumulated per API response at
~:515-521 from the `usage` object, emitted as `Event::Usage { input,
output }`). CLI knobs are parsed in `src/main.rs` (`--max-iters`,
`--max-minutes`) and flow through `RunConfig`/knobs structs
(`src/driver.rs` ~:50-51, :112-114, :137-138).

Why: cycle 3 burned **712,215 input + 64,091 output tokens** on a no-op
watch-and-wait run (`.chug/events-20260922-060458.jsonl`) — the iteration
budget (80) was the only ceiling that caught it, after 2h; the 240-minute
wall-clock budget never came close. Token cost is the axis both existing
budgets can miss. EVALUATION.md cycle 3, K1.

## Requirements

1. **New CLI knob `--max-tokens <N>`** (u64, default `0` = unlimited —
   current behavior preserved exactly when unset). Threaded through
   main.rs parsing → RunConfig → loop knobs alongside max_iters/max_minutes.
2. **New `BudgetExceeded::Tokens { max: u64 }` variant** in
   `src/events.rs`; its `describe()` returns `"{max} tokens"` so T12's
   abort output (`budget: N tokens`) and the eventlog abort line pick it
   up with no further changes. `TurnEndReason::BudgetExceeded` mapping
   unchanged.
3. **Budget check at the same top-of-iteration point** as the existing two
   checks (`src/driver.rs` ~:386-408): if `--max-tokens` is set and the
   most recent cumulative `usage_in + usage_out` (0 before the first API
   response) is `>= max`, abort with `BudgetExceeded::Tokens { max }`.
   Check order: after iterations/minutes is fine — any order, but only one
   abort reason is emitted per run.
4. **Warning leg (T13 parity):** extend the budget-low path with a tokens
   kind — one-shot, only when `--max-tokens` is set. Suggested shape:
   extend `budget_low_notice` to also take `remaining_tokens:
   Option<u64>` (None when unlimited) and a `warned_tokens` latch, firing
   when remaining drops to `<= WARN_REMAINING_TOKENS` (a const; suggested
   value `50_000`). The user-message wording may keep the existing
   sentence style, e.g. `chug: budget low — N iteration(s), M minute(s),
   and K token(s) remain…` — but token(s) must appear only when a token
   budget is set, and the one-shot latch must be per-kind like the
   existing two.
5. **README** gains a bullet/line in the budgets area documenting
   `--max-tokens` (ceiling on cumulative input+output tokens, abort names
   `budget: N tokens`, low-warning leg included).
6. **No behavior change when the knob is unset**: existing tests must pass
   unmodified (add new tests; do not weaken old ones).

## Tests

- `budget_low_notice` unit tests: tokens leg fires at the boundary
  (remaining == WARN_REMAINING_TOKENS) and not above; one-shot latch
  (second call returns None); None input (unlimited) never fires; existing
  iters/minutes legs unchanged.
- `BudgetExceeded::Tokens` describe()/display test (`"50000 tokens"`).
- Scripted-harness drive_loop test (mirror the existing max_iters scripted
  tests, `src/driver.rs` test module ~:2043+): a run with a tiny
  `--max-tokens` (e.g. 25) against scripted responses carrying small
  `usage` objects aborts with `Event::Aborted { budget:
  Some(BudgetExceeded::Tokens { max: 25 }), .. }` after the cumulative
  usage crosses the limit — assert the abort event and that the loop
  stopped (script not fully consumed or turn ended).
- A no-budget control: identical scripted run with `--max-tokens 0`
  (unset) runs to natural completion (no Tokens abort).
- CLI parsing: `--max-tokens` lands in the knobs (main.rs test or via the
  drive_loop harness, whichever pattern the repo already uses for
  max_iters/max_minutes).

## Acceptance

- `cargo build`, `cargo clippy --all-targets -- -D warnings`, and
  `cargo test` all green.
- Deleting/negating the token check must fail at least one test (mutants
  die).
- README documents the knob.

check: cd /Users/jadams/workspace/chug && cargo test
