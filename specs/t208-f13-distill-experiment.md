# T208 — F13 phase 2b: the first distillation experiment (measure-first, no routing wiring)

check: python3 -m py_compile scripts/distill_experiment.py && test -f docs/distill-f13-2b.md && grep -q "held-out" docs/distill-f13-2b.md && grep -qE "tau|τ" docs/distill-f13-2b.md

## Repo context

Feature row — the FEATURES.md roadmap pull (F13 phase 2b, cycle-95 eval §4).

F13's payoff: the loop's own judgment corpus
(`.chug/decisions.jsonl` — 858 records at filing: eval-triage 253,
outcome 216, validation-routing 142, validation-verdict 131,
recovery-routing 108, plus singletons) trains a Laya decision head so
first-pass routings (does-this-need-kimi, PASS/FAIL smell, recovery
routing) go to the cheap model at confidence ≥τ and escalate otherwise —
classification ONLY, never completion/stuck/verdict-final judgments
(SPEC-3 doctrine; the T204 daemon is classification-only for the same
reason).

Phase 2a LANDED (v0.14.0): `scripts/decisions-audit.sh` (T199 — write-time
hygiene gate) + `scripts/decisions-export.sh` (T200 — training JSONL
export). The co-located inference constraint is gone: T204's baked daemon
runs candle ModernBERT locally. What has NEVER been measured: whether 858
records (216 outcome-labeled) can train a head that beats the mechanical
baseline at any τ. Phase 2b is that measurement. Phase 3 (confidence-gated
routing wiring) is OUT OF SCOPE here — it needs this experiment's numbers
first, and its endpoint is F15 phase 2 (deferred "no consumer yet" — this
experiment is the consumer's feasibility study).

Environment (pre-staged by the T204 arc): venv
`/Users/jadams/models/laya/venv` (torch + transformers), HF snapshots
cached under `~/.cache/huggingface` (base laya ModernBERT @55cf4c4e) —
run with `TRANSFORMERS_OFFLINE=1` `HF_HUB_OFFLINE=1` to prove no network.
The corpus lives in the MAIN repo checkout
(`/Users/jadams/workspace/chug/.chug/decisions.jsonl`) — the worktree's
own `.chug/` is a fresh child corpus; the script takes the corpus path as
an argument and the report analyzes the main one.

estimate: ~460 changed lines (scripts/distill_experiment.py ~350 +
docs/distill-f13-2b.md ~100 + README/DEPENDENCIES one-liners ~10).
Deliberately under the ~500 ceiling — do NOT grow scope (no routing
wiring, no src/ changes, no daemon changes).

## Requirements

1. `scripts/distill_experiment.py` — one reproducible script:
   export (via `scripts/decisions-export.sh` or direct parse) → dataset →
   train → evaluate → write the report. Seed pinned; every run byte-identical
   metrics modulo hardware nondeterminism (document the tolerance).
2. Leakage controls (the honesty spine): the split is by RECORD ORDER
   (time), never random — train on the first ~80%, held-out on the last
   ~20%; near-duplicate templated records (same class+subject shape) are
   deduped or the report names the inflation; no field that is only known
   post-decision leaks into the features (name the feature set: class,
   inputs text, options, model — NOT choice/confidence).
3. Target task: the implementer picks ONE and justifies in the report —
   recommended: `validation-routing` (REQUIRED vs lane-eligible, 142
   records, mechanically checkable labels from T189's four inputs) or
   `validation-verdict` PASS/FAIL (131 records, imbalanced — report the
   baseline). A mechanical-majority baseline MUST be reported next to the
   trained head's numbers.
4. The τ-curve: coverage vs held-out accuracy at escalating confidence
   thresholds (the F13 operating curve — what fraction of decisions the
   head could take at ≥95% accuracy). Rendered as a small table in the
   report.
5. Training runs on the venv, offline, bounded ≤30 min wall (a 2–4 epoch
   fine-tune of a ModernBERT classifier on <1k examples is minutes on
   MPS; CPU acceptable). Artifacts (checkpoint) live OUTSIDE git (e.g.
   `/Users/jadams/models/laya/distill-f13-2b/`); the report records the
   path + the git commit of the corpus snapshot used.
6. `docs/distill-f13-2b.md` — the report: dataset shape, feature set,
   leakage controls, baseline vs trained numbers, the τ-curve, and a
   go/no-go judgment for phase-3 wiring with the measured reason either
   way.
7. DEPENDENCIES.md: the venv/torch experiment dependency is recorded as
   operator-host-only (not a chug build dep) if the doc's env surface
   rules cover it — one row or a named exemption, editor's pick.

## Tests

`python3 -m py_compile` clean (the check line); the decisions-audit gate
stays green (the corpus is read, never written); the report exists with
the required sections (the check line greps one). No Rust tests — no src/
changes.

## Acceptance

- The script runs end-to-end on the operator host from a cold worktree
  (given the venv + the main corpus path) and reproduces the report's
  headline numbers.
- The report carries: baseline vs trained held-out accuracy/F1, the
  τ-curve table, leakage controls named, go/no-go with measured reason.
- No `src/` diff, no routing wiring, no network fetches (offline env
  honored), no secrets in the report or artifacts.
