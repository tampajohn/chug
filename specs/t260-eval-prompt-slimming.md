# T260 — eval prompt slimming: LIVE_CTX state + delta replaces the 1.7M re-read

check: cargo test

estimate: ~400 lines (state file + delta builder + doctrine + pins)

## Concern

Operator 2026-10-07: "evaluate ways of speeding this up." Measured
(2026-10-07 telemetry): every orchestrator iteration re-reads ~134k
fresh input + ~1.7M cache-read (the eval digest + EVALUATION.md +
TODO wholesale) at ~$1/iteration. The eval does not need the full
corpus — it needs its own maintained state plus the delta since the
last eval. T192's LIVE_CTX machinery (model-edited context file with
shrink gate, LANDED) is the vehicle.

## Repo context

- T192/T202 (CLM port): .chug/LIVE_CTX.md model-edited with fit/shrink
  gate and free compaction turns — exists, proven.
- T184 telemetry: cache-token fields on iteration events — the
  measurement exists; this item is the structural fix it was FOR.
- META-META-SPEC's corpus list (digest, EVALUATION.md, TODO, specs,
  events) is the re-read source. The delta shape (rows, deaths,
  changed files by class, cycle summaries since last eval) is
  computable by loopd/eval-digest mechanically.
- Cache-read at ~0.1x input price is cheap per token but not free;
  fresh input is the expensive half.

## Requirements

1. Eval state file (.chug/eval-state.md, schema-pinned): current
   health, open threads, last N (say 12) decisions with refs, pacing
   streak state, and the last-eval hash/marker it was computed from.
   The eval READS state + the delta (loopd-built, mechanical) instead
   of the full digest/EVALUATION re-read; it REWRITES the state at
   wrap (LIVE_CTX gate applies: verbatim splice, never summarization).
2. The full corpus read happens only on state-stale (marker behind >
   3 evals, schema change, or the eval itself flags state drift) —
   measured per cycle in the wrap's Outcomes (reads-fresh vs
   reads-state ratio is the acceptance metric).
3. Fresh input per eval iteration drops by >= 5x on state-hit cycles
   (pinned via the T184 fields; the wrap records before/after).
4. Doctrine: META-META-SPEC gains the state+delta read path with the
   full-read fallback; the state file is NEVER the eval's only input
   on a trip eval (trip = full read, always).
5. Pins: state rewrite is verbatim-splice (T192); stale-marker
   fallback fires; fresh-input drop measurable from events.

## Tests

- The three pins above; existing eval/pacing pins stay green.
- Acceptance: a state-hit eval cycle's fresh-input total drops >= 5x
  vs the cycle-195-198 measured baseline, recorded in Outcomes.

## Out of scope

- Work-cycle orchestration prompts (a separate lever); trimming the
  digest itself; changing what the eval DECIDES (read-path only).
