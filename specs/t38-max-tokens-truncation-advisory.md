# T38 — Truncated-response advisory: `stop_reason=max_tokens` names the chunking remedy

check: cargo test

## Concern

One concern: when the API truncates a response at the output-token ceiling,
chug says nothing — the model burns iterations (or dies) discovering on its
own that its write was cut off.

## Repo context

- `src/api.rs:10` — `const MAX_TOKENS: u32 = 8192;` every request caps output
  at 8,192 tokens.
- `src/api.rs:414-419` — `Response::stop_reason()` exists and is passed to
  observability (`api.rs:485`), but the driver loop never acts on it ("the
  driver loop itself keys off tool_use blocks").
- **The bite (cycle-16, new child-death class):** the T37 glm impl child
  (`events-t37-impl-20260926-042452.jsonl`, 50/50 budget abort) tried to
  `write_file` a 987-line `src/webfetch.rs` (~30KB ≈ 8–10k tokens) in one
  call — impossible under an 8,192-token output ceiling. The truncated write
  saga cost the child its whole budget; the orchestrator had to
  harvest-complete 4 fixes. The model never got told *why* its file kept
  arriving short; it discovered chunked heredocs on its own, too late.
- T13 established the injection seam this item reuses: a one-shot user
  message injected in `drive_loop` before the next LLM call
  (`src/driver.rs:509-538`, pure helper `budget_low_notice` at
  `src/driver.rs:840`), recorded on the events stream (T17,
  `Event::BudgetLow`).
- `ScriptedLlm` (driver tests) already constructs responses with arbitrary
  `stop_reason` values (e.g. `src/driver.rs:2620`).

## Requirements

1. After each LLM response whose `stop_reason` is `"max_tokens"`, the driver
   injects a user message (same steering-note mechanism as T13: transcript +
   memory) before the next LLM call. Text (pin it):
   `chug: output truncated — the previous response hit the API output-token
   ceiling (stop_reason=max_tokens). If you were writing a file, split it:
   write_file the first chunk, then append with edit_file (or bash heredoc)
   in smaller pieces.`
   (Exact wording may be adjusted in implementation for house style, but
   every load-bearing token — `max_tokens`, `stop_reason`, the chunking
   remedy naming `write_file` + `edit_file` — must survive.)
2. Fires on *every* truncated response (no one-shot latch — each truncation
   is fresh, actionable information), at most one injection per response.
3. A truncation with no tool calls (pure text cut short) injects the same
   advisory — the remedy sentence still applies.
4. Events: a new `Event` variant (e.g. `OutputTruncated`) is emitted iff an
   advisory is injected, serialized on `.chug/events.jsonl` as one line
   (e.g. `{"type":"output_truncated",...}`), so future evaluations can count
   occurrences with `jq`. Console/TUI sinks render nothing new (silent, like
   the budget-low event legs — T17 precedent).
5. Non-truncated responses (`end_turn`, `tool_use`, absent): byte-identical
   behavior — no injection, no event.
6. Chat turns that route through the same generation path get the same
   treatment; if chat does not share the seam, scope to `run` and note the
   chat gap in the commit message.
7. README: one bullet under Autonomous mode (or extend the events-log
   bullet) documenting the advisory + its event line.

## Tests

- ScriptedLlm returns a response with `stop_reason: "max_tokens"` → the next
  user message in the loop is the advisory (text pinned); the event appears
  on the recorded event stream.
- ScriptedLlm returns `stop_reason: "tool_use"` / `"end_turn"` → no
  advisory, no event (non-vacuousness control).
- Two consecutive truncated responses → two advisories (no latch).
- Pure-text truncation (no tool_use blocks) → advisory still injected.
- Event serialization pin: the new variant writes the expected line shape.

## Acceptance

- `cargo build`, `cargo clippy --all-targets -- -D warnings`, `cargo test`
  all green.
- Mutation-ready: deleting the `stop_reason == "max_tokens"` check, or
  corrupting the advisory's load-bearing tokens, fails at least one test.
- Commit message cites the T37 glm truncated-write death as origin.

## Out of scope

- Retrying or auto-splitting the truncated call itself.
- Streaming-mode truncation handling.
- Changing `MAX_TOKENS` (an operator/operator-cost decision, not this row).
