# Judge-parity bake-off — the verdict (T223, T220's measurement half)

Run: `scripts/judge-parity.sh` (fail-closed, idempotent — T217 doctrine), 2026-10-04,
release `chug 0.16.2` daemon, commit `cf365743129eff99ee0c62ab1599a0bc0ff534b8`.
Every response came through the real served `/judge` path (Unix-socket HTTP, the
exact wire risk-gate/notify clients see), one daemon per pinned checkpoint, every
raw response stored verbatim in the run artifacts.

## Verdict: **Laya stays.** The default checkpoint is unchanged.

The **default-checkpoint decision**: Laya (`convaiinnovations/laya`) remains the
shipped default. T220's margin rule — swap only if kev wins a class by >2 points
top-1 / AUROC — is **vacuous this round: the bake-off ended one contestant short**
(no kev number exists to compare, for two independent reasons recorded below). A
vacuous margin rule can never elect a swap, so Laya stays by the rule's own
letter, and the kev loader remains an opt-in (currently refusing) checkpoint.

## Contestants and their provenance (T205 pins)

| Contestant | Pin | Status this run |
|---|---|---|
| Laya (default) | `convaiinnovations/laya@55cf4c4ebb4ebe31b2550e8bdf3bd21b9975385` | **measured** — served both corpora |
| Laya (stop-judge fine-tune) | `tampajohn/laya-stop-completion-judge@c1931d4ffbb90c3584ef936b5d8eae7bbe8c82d` | **measured** — served its golden fixtures |
| kev-0.8b | `jaredpalmer/kev-0.8b@bf75a6a8848ea6960ff2ed108d9ed44c2941174f` (base `Qwen/Qwen3.5-0.8B-Base@dc7cdfe2ee4154fa7e30f5b51ca41bfa40174e68`) | **refused to load** (classified) — see gap 1 |
| openJev-verdict-2.0 | `Heman10x-NGU/openJev-verdict-2.0` | **id does not resolve** — re-probed live this run: HTTP **401** — see gap 2 |

**Gap 1 — kev-0.8b is candle-blocked (T222's premise defect #2, re-confirmed by
this run's daemon).** Pointing `CHUG_LAYA_CHECKPOINT` at the pinned sha, the
daemon exits before binding the socket with the classified refusal:
`the kev-layout checkpoint … cannot serve yet: candle-blocked: the LoRA base
Qwen/Qwen3.5-0.8B-Base is a Gated DeltaNet hybrid (qwen3_5: linear-attention
projections in_proj_*/out_proj among the targets, merge scale 2) —
candle-transformers 0.11 … has no qwen3_5 model, and substituting the dense
qwen3 would serve a different architecture.` No architecture was substituted
(SPEC-3); there is no kev number to report. First blocker for a full bake-off:
the gated-DeltaNet prefill port for candle.

**Gap 2 — the second contestant never existed as written.** The researched id
`Heman10x-NGU/openJev-verdict-2.0` still returns 401 (absent/private as
written; probed live during this run, one GET, recorded verbatim). T223 req 3
names this case: the doc records the gap rather than fabricating a comparison.

**Gap 3 — external anchor not local.** The LocalLLaMA typed-decisions suite was
not found on this host; per req 4 it was **not** fetched mid-run. Skipped.

## Corpus (a): the stop-judge golden vectors — the PARITY panel

8 committed fixtures (4x `riskgate_base`, `readme_billing_base`, 3x
`stop_judge_*`), each asked in the canonical and the option-flipped ordering;
fixtures routed to the checkpoint their record names. Measured: does the served
checkpoint reproduce the Python-SDK-recorded probabilities (tolerance 1e-3, the
live-parity tolerance)?

| Metric | Value |
|---|---|
| Golden parity, max \|Δp\| over all compared options | **0.00e+00** (0 violations of 1e-3) |
| Choice top-1 agreement with the recorded answers | **8/8 (1.0)** |
| Score MAE vs the recorded score | **0.0001** |
| noul parity | exact (both noul fixtures reproduce the recorded value) |

The served Laya **is** the recorded Laya, bit-for-bit at round-4 precision — the
bake-off's laya numbers are the published checkpoint's numbers, not a drift.

## Corpus (b): the labeled decision-log holdout — the ACCURACY panel

Derived by T200's `scripts/decisions-export.sh` over the main checkout's
decision corpus (sha256 `50bc4f06…a7dd84`, export sha256 `0809d0c3…8a186d`):
**708 rows, 216 labeled, 7 grandfathered choice-violations** (the export's own
summary line). 216 labeled rows − 5 prose-grandfathered outcome labels =
**211 closed-set labels** `{landed-clean, fixed-up, reverted}`; holdout = the
time-ordered last-20% tail = **42 records** (the F13 split doctrine; features
are the pre-decision fields only — class, subject, inputs, options — the F13
leakage rule; the recorded choice/confidence are excluded). Task: outcome
prediction, one 3-way choice question + one noul (clean-vs-not) question per
record, both orderings.

| Metric (T222 set, per class) | Laya (default checkpoint) |
|---|---|
| Choice top-1 (3-way outcome) | **24/42 = 0.5714** |
| noul AUROC (clean vs not, midranks) | **0.374** |
| Option-order flip rate (choice, paired) | **17/42 = 0.4048** |
| Mean \|p_diff\| between orderings (choice) | **0.0658** |
| Score MAE | n/a — the holdout defines no score question |

Honest reading: the shipped checkpoint does **not** separate decision outcomes
— AUROC 0.374 is *below* coin-flip, and top-1 0.5714 sits near the 0.55+ you
get from always guessing the majority class in this tail. That is the expected
deep-OOD result (this checkpoint was fine-tuned for stop/completion and
riskgate questions, not outcome prediction) and it is exactly the number the
F13 fine-tune path exists to move; until a contestant beats it, corpus (b)
offers **no evidence for a swap** — and no gating signal either (SPEC-3: this
was measurement, never wiring).

## The kev card's caveat, measured on Laya itself

Option order flips answers — the kev card's warning — holds for Laya too:
**5/9 (0.556)** of the golden choice/score questions flip their argmax when the
options are presented in the reverse order (mean per-option \|Δp\| 0.130), and
**40.5%** of holdout records flip. The goldens' canonical ordering is what the
SDK and the daemon serve, so parity is unaffected; but any future
cross-contestant comparison must pin option order per fixture (this runner
measures both orderings for every record by construction).

## Reproducibility

`bash scripts/judge-parity.sh /path/to/checkout` rebuilds all of the above:
same corpora + pinned revisions → same numbers (the runner is idempotent, no
wall-clock fields in `metrics.json`; the golden parity reproduced 0.00e+00 on
three consecutive runs). It exits **nonzero** while any contestant fails to
load — today that is the kev refusal — and stores the partial verdict
(`metrics.json`, `kev-refusal.txt`, `verdict2-probe.txt`, the responses, and
the corpus sha256 pins) outside git under
`/Users/jadams/models/laya/judge-parity-t223`. The committed script differs
from the run that produced these numbers only in a metrics-phase variable
rename (`fx_by_name` → `fx_by_key`, the index-keying fix for the four
same-named `riskgate_base` fixtures); that exact code was re-executed against
this run's stored responses to produce `metrics.json` as committed above.

## What would change the verdict

1. A gated-DeltaNet prefill port for candle (or a candle-runnable kev
   checkpoint) → the runner's kev leg loads and the same corpus discipline
   produces the missing column; the margin rule then applies with real numbers.
2. A resolvable verdict-2.0 repo id (same ModernBERT family as Laya — the
   existing loader carries it once pinned) → a third column, no new loader.
3. The F13 fine-tune beating these holdout numbers → a *better default
   checkpoint*, decided on the same runner's output.
