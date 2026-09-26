# T46 — Eval digest: pre-computed Phase-1 corpus summary

check: cargo test

## Concern

Phase 1 (evaluation) costs ~45–55 orchestrator iterations, mostly reading
raw corpus: `jq` over events archives one query at a time, tailing ledgers,
skimming transcripts. Late-cycle orchestrator context is ~300–500k input
tokens per poll cycle. A mechanical pre-digest shrinks Phase 1 to reading
ONE compact file — the model evaluates, it should not be doing ETL.

## Repo context

- `META-META-SPEC.md` corpus list: TODO.md, ledgers, `.chug/events*.jsonl`,
  specs, src layout, README.
- `.chug/events*.jsonl` is jq-mineable by design (T10): iteration counts,
  tool distributions, error rates, token curves, aborts, budget_low fires.
- Harvest convention (T19/K2): child streams land in main `.chug/` as
  `events-t<N>-<role>-<ts>.jsonl`.

## Requirements

1. New `scripts/eval-digest.sh` (or `chug digest` subcommand — implementer's
   call, script is fine) that scans `.chug/` and writes
   `.chug/eval-digest.md`: per events file — iterations, wall time, tool
   distribution, error classes w/ counts, token totals + late-cycle curve,
   aborts w/ reasons, budget_low fires; plus TODO status counts and days
   since last EVALUATION.md.
2. Deterministic, no LLM calls, runs in <5s. Invoked by loopd before each
   cycle (one line in loopd.sh) so the digest is always fresh.
3. META-META-SPEC corpus item 1 becomes: read `.chug/eval-digest.md` FIRST;
   raw events only for drilling into a specific incident the digest raised.
4. Digest flags its own staleness (generated-at vs newest events mtime).

## Tests

- Script runs against the current `.chug/` and produces a file containing
  per-file iteration counts that match `jq -r '.type' | grep -c iteration`
  for ≥3 sampled archives (pin in a shell test or Rust integration test).
- Empty `.chug/` produces a valid empty digest (no crash).
- Acceptance: next fresh evaluation's events show the digest read early and
  Phase 1 iteration count visibly below the ~45–55 baseline (recorded in
  that cycle's Outcomes).

## Out of scope

- LLM summarization; changing what EVALUATION.md contains; transcript mining
  beyond what events.jsonl already captures.
