# T76 — tgrep: token-budgeted ranked context retrieval

check: cargo test

## Concern

Context economics (operator 2026-09-26, Langfuse telemetry): 9.5M input /
1.56M output tokens over 2 days; kimi 79% of input. The per-call context is
dominated by TOOL RESULT bulk — full-file reads (tools.rs was ~55k tokens
before T71; driver.rs ~40k today) and wide bash output. Children routinely
read whole files to find 20 relevant lines. Prompt caching hides the dollar
cost (avg uncached ~6k/call) but not the latency, the trim pressure, or the
thinking dilution. Operator named the fix class: "tgrep or something to aid
in getting relevant context."

## Repo context

- `src/tools.rs` (post-T71 `src/delegate.rs` pattern): tool schemas +
  dispatch; `read_file` gained offset/limit (T26) but the model must know
  WHERE to look first — pagination without relevance is still blind.
- `grep` exists but returns raw matches without ranking, context windows,
  or a token budget — a 200-hit grep dumps everything.
- Precedent: T46 eval digest (pre-compute beats model-ETL).

## Requirements

1. New `tgrep` tool: query (one or more terms, ranked AND-ish), optional
   path glob, `budget` (default 2000 tokens, clamp 8k). Returns ranked
   match CLUSTERS: file:line + ±3-line window, best-first, with per-cluster
   score; output stops at budget with `[more: N clusters omitted]`.
2. Ranking: exact-phrase > all-terms-in-window > term frequency-density;
   path basename boost; deterministic (no LLM, no embeddings — keep it
   fast, <100ms on the repo).
3. Also gains a `symbols` mode (`tgrep --symbols <file>`): signatures/types
   skeleton of a Rust file (fn/struct/impl lines, no bodies) — orientation
   before deep reads.
4. Tool description teaches the workflow: tgrep → targeted read_file with
   offset/limit. Sandbox-candor (T41): names the cwd confinement.
5. No new dependencies (hand-rolled ranking; rust signature mode is
   line-oriented heuristics, not a parser).

## Tests

- Unit: ranking order (exact-phrase beats scatter), budget truncation with
  omission marker, cluster merging (overlapping windows merge once),
  symbols mode extracts fn signatures not bodies.
- Integration: scripted driver run where tgrep output stays under budget
  for a 300-hit query and the omission marker fires.
- Acceptance (telemetry): a later cycle's events show tgrep adopted in
  place of ≥1 full-file read per child (recorded in Outcomes).

## Out of scope

- Semantic/embedding search; other languages' symbol parsers; changing
  read_file defaults.
