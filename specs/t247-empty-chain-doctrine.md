# T247 — LOOP-SPEC Phase 1: codify the empty-delta chain (disposition rule + eval-trip valve + human-counted streak)

check: export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-validate-a; touch src/*.rs tests/*.rs; cargo test --test loop_spec_empty_chain --test todo_consistency

estimate: ~70 lines all-in (Phase-1 clause ~30 + new pin file ~40 + row)

## Concern

The empty-delta chain has now run TWICE (cycles 110–113 and 119–122),
and both times its entire governance was carried in Outcomes/wrap-notes
prose — never in LOOP-SPEC:

1. **The disposition rule** — when the freshness predicate fails on the
   todo-rows half (queue drained) but the delta since the last eval is
   bookkeeping-only (zero children, zero items, eval/wrap commits only),
   the cycle skips the eval anyway and wraps with the T237 token.
   Practiced across SIX disposition cycles (110/111/112/119/120/121),
   pre-authorized as "the chain rule" in cycle-110's Outcomes — never
   written in Phase 1, whose skip predicate says nothing about the case
   where the predicate FAILS and the eval is skipped regardless.
2. **The eval-trip valve** — after ~3 consecutive dispositions the 4th
   empty cycle RUNS the real eval. Cycle 113 tripped on schedule and
   filed T237 (the backoff); cycle 122 tripped on schedule and filed
   this row. The valve is the termination condition that makes the T237
   pre-authorized chain safe — T237's own filing named the risk ("the
   disposition pre-authorization makes the chain self-sustaining").
   Prose-only until now.
3. **The human-counted streak** — the trip binds on the TRUE streak
   counted from the wrap-notes chain, never loopd's machine streak:
   loopd.sh `empty_wrap_streak` substring-matches the token in `eval:`
   subjects and counts NEGATED mentions (the cycle-118 false positive —
   machine read 4 vs TRUE 3 at the cycle-122 trip). The adjudication
   (d1791277274-2: divergence consequence-free for the backoff; the trip
   threshold is human-counted) lives only in Outcomes prose.

The handoff worked 6-for-6 (each disposition's Outcomes entry carried
the count forward), but the rule that decides WHEN a skipped-eval chain
must stop should not depend on every wrap correctly carrying prose. One
clause at the point of decision — Phase 1, immediately after the skip
predicate — makes the chain self-describing for a cold cycle.

## Repo context

- LOOP-SPEC Phase 1: the freshness predicate + skip rule + the
  glm-never-evaluates clause. The chain clause belongs right after the
  skip-predicate paragraph, before the commit-artifacts sentence.
- loopd.sh:136-150 `empty_wrap_streak`: awk substring walk over
  `git log --format=%s -30` counting `empty-delta disposition` in
  `eval:` subjects — code-verified to count negated mentions
  (loopd.sh:147 `index($0, "empty-delta disposition") > 0`; cycle-119
  reproduced the false positive at HEAD). The machine streak paces the
  BACKOFF only; this row does not touch it.
- LOOP-SPEC Phase 3's T237 token clause (the wrap-subject token rule the
  disposition relies on) is pinned by tests/loopd_empty_backoff.rs —
  untouched by this row.
- No existing test pins Phase-1 freshness text (grep-verified: zero
  tests match "Skip straight to Phase 2" or "re-evaluating for its own
  sake"). New pin file `tests/loop_spec_empty_chain.rs`, T78 idiom
  (exactly-once, whitespace-collapsed needles, runtime-cwd file
  resolution per T48).
- Doctrine row → SOLO (touches LOOP-SPEC.md; no other child in flight),
  kimi REQUIRED validation per LOOP-SPEC §2 step 4 (the loop/spec
  doctrine itself).

## Requirements

1. LOOP-SPEC Phase 1 gains the empty-delta chain clause in ONE paragraph
   after the skip-predicate paragraph, covering:
   (a) **the disposition rule** — predicate fails on the todo-rows half
   (queue drained) AND the delta since the last evaluation is
   bookkeeping-only (zero children launched, zero items landed, every
   commit an eval/wrap-notes commit) ⇒ the cycle MAY skip the
   evaluation and proceed to wrap; that is an empty-delta disposition;
   its wrap-notes subject carries the Phase-3 token; its Outcomes entry
   names the TRUE streak and hands it forward ("N empties away") so a
   cold next cycle reads the count from the newest Outcomes entry, not
   git archaeology;
   (b) **the trip valve** — after ~3 consecutive empty-delta
   dispositions, the next (4th) empty cycle RUNS the real evaluation;
   the chain converts itself into its own evaluation at the trip point;
   cycles 113 and 122 named as the two on-schedule trips;
   (c) **the human-counted streak** — the trip binds on the TRUE streak
   HUMAN-counted from the wrap-notes chain, NEVER on loopd's machine
   streak; the machine walk substring-matches the token in `eval:`
   subjects and counts NEGATED mentions too (the cycle-118 false
   positive named); the divergence is consequence-free for the backoff
   (bounded over-sleep, self-correcting) but the trip threshold reads
   the true count only.
2. `tests/loop_spec_empty_chain.rs` (new): one exactly-once
   whitespace-collapsed needle leg per part — (a) "zero children
   launched, zero items landed", (b) "next (4th) empty cycle RUNS the
   real evaluation", (c) "HUMAN-counted from the wrap-notes chain" —
   each RED-provable by reverting its needle phrase from LOOP-SPEC.md.
3. No edits to Phase 3's token clause, loopd.sh, META-META-SPEC.md, or
   tests/loopd_empty_backoff.rs.

## Tests

- Each pin leg RED-proven: revert its needle from LOOP-SPEC.md → the leg
  dies; restore byte-identical → green. The impl names the RED-proof
  per leg in its commit message.
- The check line above runs green (loop_spec_empty_chain +
  todo_consistency).

## Out of scope

- Changing loopd.sh's streak walk (the negation-quote fix stays REJECTED
  per the standing cycle-119 adjudication d1791277274-2 — consequence-
  free for the backoff; this row pins the human-counting rule, it does
  not re-weigh the machine fix).
- Mechanizing the trip in loopd.sh (the eval decision stays the
  orchestrator's judgment; loopd's token mechanism stays dumb per the
  T237 design).
- Editing the Phase-3 token clause or its loopd_empty_backoff.rs pins.
- Doctrine for one-off empty cycles (the disposition rule governs
  chains; a single empty cycle was never ambiguous).
