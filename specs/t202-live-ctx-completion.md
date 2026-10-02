# T202 — T192 completion: genuinely-free-turn budget leg + digest ctx-edit surfacing

check: touch src/*.rs tests/*.rs; export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-validate-a && cargo test --bin chug live_ctx && cargo test --test eval_digest

## Repo context

Two small completion legs for the F14 phase-2 surface, both carried
by the cycle-88/89 wrap:

1. **The carried weak test.** T192's kimi validator finding 1
   (non-blocking, explicitly carried "for the next eval"):
   `tests/live_ctx.rs`'s `token_budget_binds_on_free_edit_turns`
   scripts edits that are all REJECTED, so no genuinely-free turn
   ever exercises the usage-accumulation skip the test claims to pin
   — the leg is vacuous for its named behavior. The free-turn
   accounting (≤3 consecutive free edit turns, the 4th counts;
   `--max-tokens` always binds) deserves a scripted ACCEPTED edit
   that produces a real free turn and then observes the budget
   binding anyway.
2. **The invisible adoption metric.** T192's spec named `ctx_edit`
   event lines "for the T184-class digest" — the events land (2
   `ctx-edit` markers in the cycle-89 orchestrator stream) but
   `scripts/eval-digest.sh` never surfaces them (grep-verified: no
   ctx handling), so the loop's own context-economy telemetry (T184's
   whole point) cannot see the feature it is meant to measure. The
   digest gains a per-file `ctx-edit fires: N` line beside the
   existing `trim fires: N`.

estimate: ~120 changed lines (test rework ~50, digest line ~20 +
golden pins ~50).

## Requirements

1. **Genuinely-free leg (tests/live_ctx.rs):** rework
   `token_budget_binds_on_free_edit_turns` (or add a sibling leg, the
   old one kept if it pins a distinct behavior — child's call, named
   in the commit) so the scripted CtxEditorLlm performs an ACCEPTED
   edit (whole blocks, pinned content untouched, strictly smaller) →
   the turn is genuinely free (iteration not consumed) → and the
   cumulative usage STILL crosses `--max-tokens` and aborts. The leg
   must be RED against a mutant that drops the always-binds rule (or
   that makes free turns skip usage accumulation) — the RED-proof is
   stated in the commit message.
2. **Digest surfacing (scripts/eval-digest.sh):** each events-file
   block gains `ctx-edit fires: N` (count of `ctx_edit` event
   lines), placed adjacent to the existing `trim fires:` field; zero
   renders as `0` (shape-stable).
3. **Golden pins (tests/eval_digest.rs):** the golden-section harness
   gains the ctx-edit field in its field-order/label pins, plus a
   fixture leg with 2 ctx-edit lines rendering `2`.
4. **No src/ behavior changes** — if leg 1's RED-proof reveals a
   REAL bug in free-turn accounting, the child stops and reports
   instead of fixing (scope escalation back to the orchestrator).

## Tests

- The reworked/new live_ctx leg with its RED-proof.
- eval_digest golden legs for the new field.
- `cargo test --test live_ctx --test eval_digest` green.

## Acceptance

- Check line green; `cargo clippy --all-targets -- -D warnings`
  green.
- `scripts/eval-digest.sh` run at review shows `ctx-edit fires: 2`
  on the cycle-89 stream (`.chug/events-20261002-075715.jsonl`) —
  evidence in the commit message.

## Out of scope

- Token-cost accounting for ctx edits (before/after tokens are
  already on the event lines; aggregating them is a later digest
  pass).
- plan/chat-mode LIVE_CTX behavior (unchanged).
