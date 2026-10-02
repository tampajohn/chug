# T204 — baked-in Laya judge: candle port, phase 1 (F15)

check: cargo test

estimate: ~700 lines (candle judge + RLAgent loader + parity goldens + wiring)

## Concern

Operator 2026-10-02: "our laya models are hosted externally from chug —
we should look at having this baked in." Today two chug surfaces depend
on a layad daemon at LAYA_URL (default 127.0.0.1.8420): the risk gate's
LayaJudge (SPEC-3 semantic layer) and notify's layad sink (T190). The
daemon is F94-only (Python venv + launchd + an S1-quarantine history),
so on K7 — where the loop lives — the judge is connection-refused dead:
risk-gated runs silently lose the semantic layer and the layad sink is
unusable. F13's payoff (decision-log distillation -> confidence-gated
first-pass routing) needs inference co-located with the loop, not on one
operator box.

## Repo context

- Laya (convaiinnovations/laya, Apache 2.0): 421M-param ModernBERT-large
  + decision head, ENCODER-ONLY single forward pass (never generates),
  ~23ms warm on MPS, ~650-810MB safetensors + tokenizer.json.
  Fine-tunes share the RLAgent layout (tampajohn/laya-stop-completion-
  judge is the shipped example; F13 will produce loop-decision
  fine-tunes in the same layout).
- sidecar-model-plan (operator memory, 2026-08): candle + hf-hub +
  tokenizers is the blessed embed stack — pure Rust (clean for the 3
  release targets), Metal on Apple Silicon, HF cache at
  ~/.cache/huggingface. Laya is SIMPLER than that plan's generative
  case: no autoregressive loop.
- src/riskgate.rs: LayaJudge POSTs {LAYA_URL}/judge, 2s fail-fast,
  failure degrades logged (fail-open). The Judge trait is the seam —
  a local judge implements the same trait; selection via env.
- SPEC-3 hard constraint (carried, verbatim): laya does CLASSIFICATION
  ONLY — never completion/stuck/verdict-final judgments. The baked
  judge inherits this; routing/triage classes only.
- Weights are NOT in the binary (~650MB): first-use hf-hub download
  into the standard HF cache; offline fallback = HTTP judge.

## Requirements

1. Feature-flagged local judge: `judge-local` cargo feature pulling
   candle-core/-nn/-transformers + hf-hub + tokenizers (default OFF;
   release builds enable it per-target: metal on macos-14, cpu on
   linux). CHUG_JUDGE=local|http|off selects at runtime (default: local
   when the feature is compiled AND weights resolve, else http).
2. RLAgent-layout checkpoint loader: loads convaiinnovations/laya AND
   RLAgent fine-tunes (stop-judge layout) from HF hub or a local dir;
   CHUG_LAYA_CHECKPOINT overrides the model id/path. ModernBERT forward
   + decision head(s) in candle; if candle-transformers lacks
   ModernBERT at impl time, port it (encoder-only, bounded) — do NOT
   substitute a different architecture.
3. Packing parity: state_pack.py's featurization ported to Rust with
   GOLDEN VECTORS — committed fixtures (input state -> expected
   probabilities, generated once from the Python SDK, incl. the billing
   0.9607 case from operator memory) asserted within 1e-3 in tests.
   Train/inference packing sync is the documented failure mode; the
   goldens are the guard.
4. Wire the Judge trait: risk gate + notify layad sink run on the local
   judge when selected; LayaJudge (HTTP) stays as fallback and for
   parity A/B; judgment latency budget ~50ms local (vs 2s timeout
   today) — never blocks the agent loop.
5. FEATURES.md gains F15 (baked-in judge) with this row as phase 1;
   README risk-gate/notify sections updated (one paragraph each, no
   append-sprawl).
6. Release matrix: macos-14 arm64 build compiles with metal; linux
   builds cpu-only; install.sh unchanged (weights download at first
   use, with a clear stderr line + offline fallback note).

## Tests

- Golden-vector parity vs Python SDK outputs (the committed fixtures).
- Loader: base checkpoint + an RLAgent fine-tune both load and judge
  (fixture states).
- Selection: CHUG_JUDGE=http preserves today's behavior exactly;
  CHUG_JUDGE=local without weights -> clean fallback + one log line.
- Feature-off build compiles with zero candle deps (cargo tree pin).

## Out of scope

- Using the baked judge for NEW decision classes (F13's routing
  distillation is a later phase — this lands the engine); CUDA;
  quantizing below F16 (measure first); removing the HTTP judge path.
