# T39 — `delegate` launch gains optional `max_tokens` (T15 parity for children)

check: cargo test

## Concern

One concern: close the knob gap — T15 gave `chug run`/`chug chat` a
token-denominated budget, but `delegate` cannot set it on the children it
spawns, so a runaway child's token spend has no ceiling.

## Repo context

- T15 (`12a6d20`) added `--max-tokens N` to `chug run` and `chug chat`:
  cumulative input+output ceiling, `BudgetExceeded::Tokens` abort, one-shot
  50k-remaining warning. Its rationale: "a cheap watch-and-wait loop burns
  neither [iterations nor minutes] while racking up tokens."
- `delegate` launch (`src/tools.rs:676-708`) builds the child argv with only
  `--spec --goal --model --max-iters --max-minutes` — no `--max-tokens`.
  A delegated child doing `wait_secs` long-polls or stuck in a read-heavy
  loop is exactly T15's watch-and-wait class, one level down.
- Evidence of scale: the T37 validator child burned 151,300 in / 17,673 out
  (≈169k tokens) in 48 iterations
  (`.chug/events-t37-validate-20260926-043809.jsonl`).
- The launch schema (`src/tools.rs:156-166`) documents `max_iters` /
  `max_minutes` as optional integers with defaults 40/35.

## Requirements

1. `delegate` input schema gains an optional integer `max_tokens`
   (launch-only; ignored — not rejected — on `status`, consistent with how
   `max_iters` behaves there today if that is the existing behavior;
   otherwise match existing budget-arg handling exactly).
2. When present, launch appends `--max-tokens <N>` to the child argv. When
   absent, the argv is byte-identical to today (no flag) — children keep
   their current no-token-ceiling behavior unless the orchestrator opts in.
3. Validation: `max_tokens < 1` → tool error naming the constraint;
   non-integer rejected by the schema; zero/negative never reaches the
   child. Boundary test at 1.
4. `status` output shape unchanged; no new events; launch's return text may
   mention the configured token budget alongside the existing budgets if
   that is today's pattern for iters/minutes (match it; otherwise leave the
   return text alone).
5. README: the `delegate` paragraph names the new optional knob in one
   clause (integrated into the existing launch sentence, matching how
   `max_iters`/`max_minutes` are presented).

## Tests

- Launch with `max_tokens: 250000` → child argv contains
  `--max-tokens 250000` (assert against the argv seam used by the existing
  delegate tests).
- Launch without it → argv has no `--max-tokens` (byte-identical-argv
  control).
- `max_tokens: 0` and `max_tokens: -5` → tool error, no spawn.
- Schema round-trip: the tool schema advertises `max_tokens` as an optional
  integer in `launch`'s properties.

## Acceptance

- `cargo build`, `cargo clippy --all-targets -- -D warnings`, `cargo test`
  all green.
- Mutation-ready: dropping the argv append, or accepting `0`, fails at least
  one test.
- Commit message cites T15 parity + the 169k-token validator datum.

## Out of scope

- Changing the loop's child-launch budgets (LOOP-SPEC passes
  `max_iters`/`max_minutes` explicitly; adopting `max_tokens` in LOOP-SPEC's
  template is a separate doctrine decision for a later evaluation).
- Token *telemetry* in `delegate status` (the events tail already shows
  cumulative tokens per iteration).
