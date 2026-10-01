# T192 — Live-context editing: model-edited .chug/LIVE_CTX.md with shrink gate + free compaction turns (F14 phase 2)

check: cargo test

estimate: ~450 lines (mirror + parse-back + gate + transcript marker + free-turn accounting + nudge + events, ~15 test legs)

## Concern

Operator 2026-10-01 (evaluation of facebookresearch/context-language-models,
the "Context Language Models" paper): chug's context management is one-way
and position-based — T77's segment-frozen trim drops OLD turns on a
deterministic schedule, blind to what the model still needs. CLM demonstrates
the alternative: the harness mirrors the message list to a file, the model
edits that file with ordinary tools, edits are parsed back with an acceptance
gate, and edit-only turns are free. Measured there: +5% score at 59% fewer
FLOPs on 12-hour EdgeBench, +11.4% accuracy at 21.5% fewer FLOPs on
BrowseComp-Plus, 65% greater improvement at equal compute on a 24-hour
multi-repo agent task. For chug through tools-proxy, self-curated context is
fewer billed input tokens per iteration, and compaction keyed to model
judgment instead of position.

F14's cycle-85 reframe deferred "summarization-quality compaction" because a
summarization pass breaks T77's byte-stable prompt-cache prefix. This item
is NOT a summarization pass: edits splice whole verbatim turn blocks, so the
pre-edit prefix stays byte-stable and the new suffix freezes after one
re-prefill. The deferral's stated objection does not apply.

## Repo context

- T77 src/trim.rs: cache-stable segment-frozen trim; persists [trimmed:]
  markers INTO transcript.jsonl so resume re-derives an identical context.
  Trim stays as the deterministic backstop — this item adds a voluntary,
  model-driven path alongside it, never replacing it.
- F14 (FEATURES.md): phase 1 → T184 (context-economy telemetry, todo).
  T184's cache-token fields are how this item's effect gets measured;
  sequencing: T184 first.
- driver.rs builds the prompt from transcript.jsonl every iteration; the
  model already has read_file/write_file/edit_file/bash — no new editing
  tool is needed (CLM gives the model a single bash tool; chug is richer).
- T13's one-shot steering-note injection is the nudge mechanism to reuse;
  T15's token budget is the anti-abuse bound (an edit-only turn still costs
  one LLM call).
- .chug/ is the session state dir (driver lock T55, transcript rotation T7).
- License note: the CLM repo is CC BY-NC 4.0 — port the mechanism, copy no
  code. The mechanism is small and our transcript model differs anyway.

## Requirements

1. Mirror: each iteration, pre-LLM-call, the driver writes the current
   message list to .chug/LIVE_CTX.md as [[CTX_TURN i role=...]] blocks
   (post-system-prompt messages only; system prompt and spec are pinned and
   never mirrored as editable content).
2. Parse-back + shrink gate: after each turn, if the file differs from the
   mirrored state, parse it back into a message list. Accept iff (a) it
   parses into whole turn blocks, (b) no pinned content is touched, (c) the
   result is strictly smaller in tokens (shrink gate). On reject: transcript
   untouched, a one-line reason is surfaced to the model, and the file is
   re-mirrored at the next iteration.
3. Persistence: an accepted edit replaces the affected transcript segment
   and is recorded in transcript.jsonl as a [ctx-edit:] marker (T77's
   [trimmed:] discipline) — resume re-derives the identical context.
4. Free compaction turn: a turn whose ONLY effect is an accepted LIVE_CTX
   edit does not count against --max-iters. Hard caps: at most 3 consecutive
   free edit turns (the 4th counts normally); --max-tokens ALWAYS still
   binds.
5. Occupancy nudge: one-shot steering note when pre-call context crosses a
   --ctx-warn-at-tokens threshold (default 0 = off), naming LIVE_CTX editing
   as the remedy; T13 one-shot latch pattern.
6. Observability: Event::CtxEdit {accepted, before_tokens, after_tokens}
   event lines for the T184-class digest; README bullet.

Honesty requirement: an accepted edit invalidates the prompt-cache prefix
from the edit point (one re-prefill of the surviving suffix). The bet is
that a sustained smaller context beats a one-time re-prefill in long runs —
T184's telemetry is the arbiter. Acceptance of this spec is mechanical
(gates + tests); the value claim is measured after landing, not asserted.

## Tests

- Mirror round-trip: messages -> LIVE_CTX -> parse -> identical list.
- Gate: growth rejected; equal-size rejected; shrink accepted; malformed
  file rejected with reason; pinned-block tamper rejected.
- Persistence: accepted edit -> [ctx-edit:] marker -> resume re-derives an
  identical context (T77-pattern test).
- Free turns: an edit-only turn does not advance the iteration counter; the
  4th consecutive free edit counts; token budget still aborts.
- Nudge: fires once at the threshold, silent below, absent when 0.
- Events: ctx_edit accepted/rejected line shapes.

## Out of scope

- Serving-side KV reuse after mid-context edits (the CLM repo's
  suffix_cache_reuse SGLang patch) — that is the spark spike track, not
  chug code.
- Graduated 25/50/75% nudge ladder — phase 2, after T184 telemetry shows
  the occupancy distribution.
- chat-mode surface; MCP exposure; driver-initiated compaction (the driver
  never edits on its own — T77 trim behavior is unchanged).
