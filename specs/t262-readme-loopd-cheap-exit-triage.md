# T262 — README Continuous-mode: integrate T258 cheap-exit + T259 laya triage + fix the build-cache count

estimate: ~60 lines (README.md only, docs-only)
**Priority:** 3.
**Target files:** `README.md` (only).
**Filed:** cycle-233 evaluation (T247 valve trip 26), per META-META-SPEC §6's per-eval docs-vs-reality drift audit. Triage: first sighting → file.

## Problem

README's **Continuous mode** section (the `loopd.sh` supervisor docs) was last
touched by T237 (commit e1d7d46, 2026-10-05) and describes the pre-T258 world:
every cycle launches an LLM orchestrator, with the kimi/glm routing decided by
the freshness rule alone. Two shipped behavior changes are invisible there:

1. **T258 — the empty-eval cheap exit** (landed 548b9ea, v0.17.9): when the
   mechanical predicate is empty (no new TODO rows since the last evaluation,
   no child deaths, a bookkeeping-only delta, EVALUATION.md fresh same UTC
   day) and the valve has not tripped, `loopd.sh` writes the one-line
   empty-delta disposition ITSELF (an empty commit whose subject carries the
   T237 token + TRUE streak + the predicate inputs) and skips the cycle launch
   entirely — no LLM call, $0 per empty cycle. The disposition is recorded in
   `.chug/loopd/loopd.log` with the predicate inputs (row count, last-change
   hash, freshness). Any single non-empty input forces the launch instead.
2. **T259 — the Laya triage layer** (landed 57752a8, v0.17.9): between the
   mechanical predicate and a launched evaluation sits a confidence-gated
   cascade — the judge daemon's System One answers ONE classification question
   (needs-eval: yes/no + confidence) over a compact state pack, gated by
   `TRIAGE_HIGH` (0.85) in loopd.sh. Confident-empty on a fresh evaluation
   skips the launch ($0); `yes` routes the borderline eval to glm; confidence
   below the threshold escalates to kimi. Fail-open everywhere: daemon absent,
   any error, or a >2s timeout falls back to exactly the T258 routing with one
   note per cycle in loopd.log. Every triage is recorded to
   `.chug/decisions.jsonl` (class `laya-triage`) with outcome backfills for
   the F13 distillation corpus.

Second leg (folded in, same section): the paragraph saying the supervisor
"creates five gitignored build caches, one per cargo-consumer role" — the role
dirs are now eight (`target-shared`, `target-shared-validate-a`,
`target-shared-validate-b`, `target-shared-gates`, `target-shared-main`,
`target-shared-impl-a`, `target-shared-impl-b`, `target-shared-impl-c`), plus
on-demand `target-shared-mut-<k>` for parallel mutation legs. The very next
sentence then says T194 "adds ... the second validator slot
`target-shared-validate-b/`" — validate-b is double-named (already counted in
the five). A cold reader counts eight dirs but reads "five".

## Required changes

All edits are inside README.md's Continuous-mode section, INTEGRATED into the
existing prose structure (the README gate's rule — no bullet stapled to the
nearest paragraph):

1. Where the section currently describes the per-cycle launch + freshness-rule
   routing, extend the flow description: the supervisor computes the
   mechanical predicate BEFORE any launch; on an empty predicate (and no valve
   trip) it writes the disposition itself and skips the launch ($0, the T258
   cheap exit); would-be launches the mechanical layer cannot settle
   (borderline evals, valve trips) consult the Laya triage (T259) — confident
   empty skips, `yes` routes glm, low confidence escalates to kimi, and the
   daemon's absence/error/>2s timeout falls back to the T258 routing
   (fail-open, one loopd.log note). Keep the existing per-phase model-routing
   prose (glm implements / kimi validates, T81) intact — the cheap exit and
   triage sit ABOVE it in the decision order.
2. Mention where the records live: the supervisor's disposition line in
   `.chug/loopd/loopd.log` (predicate inputs + streak) and the triage verdicts
   in `.chug/decisions.jsonl` (class `laya-triage`).
3. Fix the build-cache sentence: name the actual role dirs (or rephrase to
   "one per cargo-consumer role — five base roles plus the overlap-era impl
   and validator slots") so the count and the following T194 sentence agree;
   drop the double-naming of validate-b.

Do NOT touch: any other README section; any non-README file; TODO.md /
LEDGER.md / EVALUATION.md (bookkeeping is the orchestrator's). Do NOT run
tree-wide formatters.

## Acceptance

- `grep -q 'cheap exit' README.md` and `grep -q 'laya' README.md` (case-insensitive
  triage mention acceptable) in the Continuous-mode section's vicinity.
- The build-cache sentence's count agrees with the dirs it names (no "five"
  alongside eight named dirs; validate-b named once).
- `cargo test --release --test readme_layout` green (README is a pinned
  carrier — the pin family must keep passing).
- Diff touches README.md only; ≤ ~80 changed lines.

check: cd "$(git rev-parse --show-toplevel)" && export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared && grep -qi 'cheap exit' README.md && grep -qi 'laya' README.md && cargo test --release --test readme_layout
