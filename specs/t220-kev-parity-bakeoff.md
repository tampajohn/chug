# T220 — Kev parity bake-off: kev-0.8b in the daemon vs Laya, per-class verdict

check: cargo test

estimate: ~450 lines (kev loader + eval harness + report + pins)

## Concern

Operator 2026-10-04: file the standing item from the System One
evaluation. The research (completed 2026-10-03) found Kev
(jaredpalmer/kev-*, Apache-2.0, prefill-only Qwen3 + pointer head,
choice/score/noul primitives identical to Jev's API) is the one
credible open-weights alternative to Laya in the daemon slot. Laya won
the six-axis review (speed, candle-done, proven fine-tune path) — but
the honest test is calibrated accuracy on OUR corpora, and that was
never measured. The daemon's model-agnostic socket makes the outcome a
checkpoint swap either way.

## Repo context

- T204 daemon: RLAgent-layout loader + candle ModernBERT serving /judge
  on the 0600 socket; CHUG_LAYA_CHECKPOINT selects the checkpoint.
  Kev-0.5b/0.6b/0.8b are LoRA adapters on frozen Qwen3 bases
  (candle-transformers HAS qwen3); the pointer head is two projections
  + softmax (~50-100 lines). kev-4b+ sit on Qwen3.5 DeltaNet hybrids
  candle cannot run — bake-off uses kev-0.8b only.
- Golden corpora (both committed/derivable in-repo): (a) T204's
  stop-judge golden vectors (input state -> expected probabilities,
  generated from the Python SDK), (b) a labeled decision-log holdout
  from T200's scripts/decisions-export.sh (decisions joined with
  outcome labels — the F13 distillation corpus itself).
- Kev card caveats to carry into the harness: option order can flip
  answers (test both orderings of a fixture), small-kev OOD transfer is
  weak (0.62 acc on their bench), single fitted temperature can't
  reorder confidences; kev-27b trails hosted Jev (52.3 vs 54.0
  breadth-v1). Provenance: jaredpalmer is a personal HF account —
  pin the checkpoint by revision sha (T205's pinning requirement).

## Requirements

1. Kev loader behind the existing checkpoint mechanism:
   CHUG_LAYA_CHECKPOINT=jaredpalmer/kev-0.8b@<pinned-sha> loads the
   qwen3 backbone + LoRA + pointer head and serves the SAME /judge
   request/response shape as the RLAgent path (drop-in: risk-gate and
   notify clients unmodified). Feature-off build stays candle-free.
2. Eval harness (cargo test target or scripts/judge-parity.sh): runs
   both judges over (a) and (b); reports per-primitive calibrated
   accuracy (choice top-1, score MAE, noul AUROC), mean |p_diff|, and
   the option-order flip rate on a pinned fixture set.
3. Verdict report committed at docs/judge-parity.md (or the eval
   digest): per-class winner named with the numbers; the default
   checkpoint changes ONLY if kev wins a class by a margin (>2 points
   top-1 / AUROC with the corpus size named); otherwise Laya stays and
   the kev loader remains as an opt-in checkpoint.
4. No live-traffic shadow in this item: offline corpora only (the
   SPEC-3 classification-only constraint stands; this is measurement,
   not gating).

## Tests

- Loader: kev-0.8b loads and answers a fixture state within shape
  (fields, types, probability sums within 1e-3).
- Harness: both judges produce per-class metrics over the two corpora
  (pinned fixture counts; deterministic given pinned checkpoints).
- Option-order pin: the flip-rate fixture runs both orderings.
- Acceptance: docs/judge-parity.md exists with per-class verdicts and
  the default-checkpoint decision recorded (recorded in Outcomes).

## Out of scope

- kev-4b+ (DeltaNet, candle-blocked); hosted Jev as a third contestant
  (API key + cost path, a later row if the operator wants the
  reference); fine-tuning kev on our corpus (that IS the follow-up if
  kev wins); any change to gating behavior.
