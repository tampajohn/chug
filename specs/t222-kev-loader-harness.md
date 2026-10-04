# T222 — kev-0.8b loader + judge-parity harness (T220 split, code half)

check: set -o pipefail; export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-validate-a; touch src/*.rs tests/*.rs; cargo test --test kev_loader

estimate: ~400 lines (qwen3 + LoRA + pointer-head loader, harness metrics module, fixture pins)

## Concern

T220's true surface was T204-class (new candle architecture + weight
downloads + live inference + a committed verdict doc) and its research
note carried a premise defect: the researched contestant id
`Heman10x-NGU/openJev-verdict-2.0` does NOT resolve on the HF API
("Invalid username or password" = repo absent/private as written —
probed 2026-10-04 ~08:2x UTC), while `jaredpalmer/kev-0.8b` resolves
clean (200; LoRA adapter + head.pt + tokenizer + provenance.json;
revision sha `bf75a6a8848ea6960ff2ed108d9ed44c2941174f`). This row is
the code half, deliverable against the CONFIRMED contestant only; the
run + verdict half is T223.

## Repo context

- T204 daemon (`src/judge_model.rs`): RLAgent-layout ModernBERT loader
  + candle serving /judge on the 0600 socket;
  `CHUG_LAYA_CHECKPOINT` selects the checkpoint (T205: org hosting,
  pinned revision sha).
- candle-transformers HAS qwen3; kev is prefill-only qwen3 + LoRA +
  pointer head (`head.pt`, two projections + softmax, ~50-100 lines);
  the adapter's `adapter_config.json` names its base — resolve it at
  impl time and pin BOTH the adapter sha and the base revision.
- kev card caveats to carry into the harness: option order can flip
  answers; small-kev OOD is weak; single fitted temperature; the
  provenance pin (T205) applies to a personal HF account.
- Golden corpora in-repo: T204's stop-judge golden vectors; T200's
  labeled decision-log export (scripts/decisions-export.sh).

## Requirements

1. `CHUG_LAYA_CHECKPOINT=jaredpalmer/kev-0.8b@bf75a6a8848ea6960ff2ed108d9ed44c2941174f`
   loads the qwen3 backbone + LoRA + pointer head and serves the SAME
   /judge request/response shape as the RLAgent path (drop-in: risk-gate
   and notify clients unmodified). Feature-off build stays candle-free.
2. Harness module (cargo test target): per-primitive metrics over a
   corpus dir — choice top-1, score MAE, noul AUROC, mean |p_diff|,
   option-order flip rate on a pinned fixture set (both orderings).
3. Offline-first: every test runs against committed fixtures; a
   `--features daemon`-gated live leg MAY hit the pinned checkpoint but
   must skip (not fail) when the fetch is unavailable.
4. No gating behavior change; no live traffic (SPEC-3 stands).

## Tests

- Fixture-state shape pin (fields, types, probability sums within 1e-3).
- Metrics determinism pins on committed corpora (pinned counts).
- Option-order flip-rate fixture runs both orderings.
- Wire-shape pin: kev path answers the same /judge JSON shape.

## Out of scope

- The actual bake-off RUN + docs/judge-parity.md verdict (T223).
- verdict-2.0 / GLiNER2.5-Decide (their ids need re-resolution — if the
  operator supplies a valid repo id, the existing ModernBERT loader
  already carries a same-family contestant; note it in T223).
- kev-4b+ (DeltaNet, candle-blocked); fine-tuning.
