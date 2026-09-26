# T77 — Cache-stable transcript trimming (segment-frozen prefix)

check: cargo test

## Concern

Prompt caching is the loop's invisible subsidy (avg ~6k uncached input per
call against a ~100k-token transcript; the proxy zeroes cache_read fields —
proxy-glm-cache-accounting-blind — so savings are real but unmeasured).
Transcript trimming today collapses old tool outputs to `[trimmed]` — a
mid-history byte edit that can invalidate the cache prefix on EVERY trim
event, re-sending the full remaining transcript. Make trim edits
cache-stable: freeze the prefix in segments.

## Repo context

- `src/transcript.rs`: trimming (collapse old tool outputs past a token
  estimate); rotation on fresh runs (T7); append pattern mirrored by
  eventlog (T10).
- `src/api.rs`: request assembly; usage accounting feeds events.jsonl
  iteration lines.

## Requirements

1. Trim in FIXED 16k-token segments: once a segment is collapsed its bytes
  NEVER change again (frozen); only whole oldest-complete-segments
  collapse. No partial-segment edits mid-stream — a segment younger than
  the threshold stays verbatim.
2. The collapse marker is segment-level: `[trimmed: ~16k tokens, N tool
  results]` replaces the whole segment (one edit per segment, once ever).
3. Measure what can be measured WITHOUT proxy cache fields: log
  per-call input_tokens (uncached) to events.jsonl (already present);
  acceptance looks for a drop in median per-iteration input growth on
  long cycles vs the pre-change baseline (recorded in Outcomes).
4. Behavior otherwise unchanged: ledger still carries durable state,
  resume/rotate semantics untouched.

## Tests

- Unit: segment boundaries — a frozen segment is byte-identical after
  later trims; marker format; threshold math at segment granularity.
- Integration: scripted long run (past 2 trim thresholds) — assert the
  prefix up to the last frozen boundary is byte-stable across 3
  consecutive API request assemblies.
- Acceptance: Outcomes of a later long cycle reports median per-iteration
  input-token growth vs the cycle-36 baseline (633k/104 iters).

## Out of scope

- Proxy-side cache_read forwarding (file against the proxy, not chug);
  changing the trim threshold value; summarization-instead-of-deletion.
