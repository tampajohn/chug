# P002 — F16 unpark decision: the keep/kill memo

Written 2026-11-07 (cycle-3083 dispatch). Docs-only; read-only toward the loop
state — this memo probes the corpus count, it never writes `.chug/decisions.jsonl`.

## The question

FEATURES.md stops carrying a parked F16 indefinitely — ship, kill, or keep
parked with a stated re-trigger. (P002 intent, verbatim.) The memo below lays
out the deciding facts; the operator picks the branch.

## The deciding facts

1. **The parked note's own unpark condition has two halves, both currently
   unmet.** `FEATURES.md` `## Parked`, F16 entry (parked 2026-10-05, operator
   call): "Un-parks when F13's first distilled judge lands and an
   alternate-runtime comparison has a consumer again." The same entry records
   the operator's judgment: "Not rejected: the precondition is real and near."

2. **F13 distillation rounds 1+2 both DECLINED the rollout — the first
   distilled judge has NOT landed.** Round 2 (T294, `docs/distill-f13-2c.md`
   §7): verdict **NO-GO** for phase-3 confidence-gated wiring — no τ operating
   point on either head reaches ≥95% covered accuracy at ≥50% coverage (probe
   best covered: 77.4% at 66.0% coverage); held-out n = 47, below the round-1
   wiring bar of n ≥ 60; at full coverage the probe beats the mechanical
   baseline (72.3% vs 55.3%) but nowhere near wiring grade. Round 1 (T208,
   `docs/distill-f13-2b.md`, compared in `distill-f13-2c.md` §8) returned
   NO-GO on the smaller corpus — its best ≥95%-confidence point covered 41.4%
   of held-out (12/29, FT τ=0.95).

3. **The alternate-runtime comparison lost its contestant at the T223
   bake-off — no consumer again yet.** `docs/judge-parity.md`, "Verdict: Laya
   stays": the margin rule is vacuous because the bake-off ended one contestant
   short. Gap 1: kev-0.8b refused to load — candle-blocked (its Qwen3.5 base is
   a Gated DeltaNet hybrid with no candle-transformers port; substituting the
   dense qwen3 would serve a different architecture). Gap 2:
   openJev-verdict-2.0's repo id never resolved — re-probed live during the
   run: HTTP 401. With zero contestant columns, a vacuous margin rule can never
   elect a swap.

4. **The corpus has grown well past round 2's pin — the condition's first half
   is re-measurable on demand.** Round 2 measured the corpus at 2,573 records
   (`docs/distill-f13-2c.md` §2/§9). The live count at memo-write time is
   **6,633 records** (`.chug/decisions.jsonl`, `wc -l`, probed 2026-11-07 — the
   corpus file is gitignored and append-only, so this is a dated probe, not a
   pin). The concrete step that could move the condition's first half:
   **distillation round 3** — re-run `scripts/distill_experiment.py` with no
   record limit (per `distill-f13-2c.md` §9, a no-limit re-run measures
   whatever the corpus then is) and apply the standing bar: a τ point with
   ≥95% covered accuracy at ≥50% coverage on held-out n ≥ 60. Named here, NOT
   run (out of scope for this memo).

5. **T332's decision-provider abstraction landed 2026-11-07** (`TODO.md` row
   T332, done, merge 350d66f; `docs/decision-providers.md`): the judge layer is
   now provider-pluggable — `CHUG_DECISION_PROVIDER=auto|local|proxy` selects
   between the local laya daemon and a hosted decision endpoint with a
   byte-compatible `/judge` wire contract. This broadens what an
   "alternate runtime" could mean (no longer only a candle-served local
   checkpoint) — but it satisfies NEITHER precondition half by itself: it lands
   no distilled judge (round 2's NO-GO stands regardless of who serves
   inference), and a provider option is not a benchmarked alternate-runtime
   comparison — the T223 margin rule still has no contestant column to compare.

## Recommendation

**RECOMMENDATION: KEEP PARKED**, with the re-trigger stated precisely. Un-park
F16 when BOTH halves of the parked condition are met:

1. F13's first distilled judge lands — concretely, a round-3 re-measurement
   (`scripts/distill_experiment.py`, no limit) reaching the τ bar: ≥95% covered
   accuracy at ≥50% coverage on held-out n ≥ 60 (`docs/distill-f13-2c.md` §7's
   own re-open condition) — or any distilled judge otherwise landing.
2. An alternate-runtime comparison has a consumer again — a benchmarked
   contestant producing a real column under the T223 margin rule
   (`docs/judge-parity.md`, "What would change the verdict": a
   candle-runnable kev, a resolvable verdict-2.0 id, or a benchmarked
   decision-provider contestant post-T332).

Killing now would erase an option the operator's own parked note calls
"real and near" (`FEATURES.md` `## Parked`, F16 entry). The corpus keeps
growing on its own (2,573 → 6,633 records since round 2) and the T332 provider
layer just widened the contestant surface; the cost of keeping the park is one
FEATURES.md bullet, while the cost of killing is re-litigating the feature from
scratch the day either trigger fires.

## The operator picks

- **(a) Keep parked with the stated re-trigger** — recommended. Edit the
  FEATURES.md F16 entry only to re-date this re-examination (2026-11-07) and
  restate the two-half trigger above.
- **(b) Kill** — record this memo as the written reason (F13 rounds 1+2 NO-GO;
  T223 one contestant short) and remove F16 from `## Parked`.
- **(c) Amend the parked condition** — e.g. drop the alternate-runtime half
  (un-park on the distilled judge alone, or on a benchmarked decision-provider
  comparison alone), keeping the rest of the entry intact.
