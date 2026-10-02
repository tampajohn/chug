# T204 — baked-in Laya judge: `chug daemon` inference server, phase 1 (F15)

check: cargo test

estimate: ~800 lines (daemon + candle judge + RLAgent loader + parity goldens + lifecycle)

## Concern

Operator 2026-10-02: "our laya models are hosted externally from chug —
we should look at having this baked in… by having the inference server
hosted in a chug daemon (rust)." Today two chug surfaces depend on a
Python layad daemon at LAYA_URL (default 127.0.0.1:8420): the risk
gate's LayaJudge (SPEC-3 semantic layer) and notify's layad sink
(T190). layad is F94-only (venv + launchd + an S1-quarantine history),
so on K7 — where the loop lives — the judge is connection-refused dead:
risk-gated runs silently lose the semantic layer, the layad sink is
unusable, and F13's payoff (decision-log distillation -> fast local
routing judgments) has no co-located inference to build on.

## Repo context

- Laya (convaiinnovations/laya, Apache 2.0): 421M-param ModernBERT-large
  + decision head, ENCODER-ONLY single forward pass (never generates),
  ~23ms warm on MPS, ~650-810MB safetensors + tokenizer.json.
  Fine-tunes share the RLAgent layout (tampajohn/laya-stop-completion-
  judge shipped; F13 will produce loop-decision fine-tunes likewise).
- sidecar-model-plan (operator memory, 2026-08): candle + hf-hub +
  tokenizers is the blessed Rust inference stack — pure Rust (clean for
  the 3 release targets), Metal on Apple Silicon, HF cache at
  ~/.cache/huggingface. Laya is SIMPLER than that plan's generative
  case: no autoregressive loop.
- DAEMON-NOT-IN-PROCESS (operator 2026-10-02): the 650MB load happens
  ONCE per host in the daemon, not per chug run; every run/child shares
  the warm model; the client keeps its lean 9-crate footprint (no
  candle in the hot path compile).
- chug already self-spawns: CHUG_DELEGATE_BIN (43 refs) is the
  one-binary/subcommand pattern — `chug daemon` is the same shape.
  Single-instance via the driver-lock pattern; daemon lock distinct
  from the run lock (.chug/daemon.lock or per-user /tmp path — impl
  detail, pin one).
- src/riskgate.rs: LayaJudge POSTs {LAYA_URL}/judge, 2s fail-fast,
  fail-open logged. The HTTP client is the seam: a daemon speaking the
  SAME /judge protocol is a DROP-IN — zero client changes beyond the
  default URL. SPEC-3 constraint carried verbatim: classification only,
  never completion/stuck/verdict-final judgments.
- Weights are NOT in the release tarball (~650MB): daemon downloads via
  hf-hub on first load into the standard HF cache; offline = clear
  error + clients fail-open as today.

## Requirements

1. `chug daemon` subcommand (same binary, CHUG_DELEGATE_BIN self-exe
   pattern): hosts candle inference (candle-core/-nn/-transformers +
   hf-hub + tokenizers behind a `daemon` cargo feature — metal on
   macos-14, cpu on linux release builds) and serves HTTP on
   127.0.0.1:8421 (distinct from layad's 8420 so both can run during
   migration): `GET /healthz`, `POST /judge` — request/response shape
   IDENTICAL to layad's (drop-in: LayaJudge pointed at 8421 works
   unmodified).
2. RLAgent-layout checkpoint loader: convaiinnovations/laya AND RLAgent
   fine-tunes (stop-judge layout), from HF hub or local dir;
   CHUG_LAYA_CHECKPOINT overrides; daemon preloads configured
   checkpoints at startup (one load, shared by all requests).
   ModernBERT forward + decision head(s) in candle — if
   candle-transformers lacks ModernBERT at impl time, port it
   (encoder-only, bounded); do NOT substitute another architecture.
3. Packing parity: state_pack.py featurization ported to Rust with
   GOLDEN VECTORS committed (input state -> expected probabilities,
   generated once from the Python SDK, incl. the billing 0.9607 case)
   asserted within 1e-3; plus a live A/B script hitting layad:8420 and
   chug-daemon:8421 on the same fixtures (dev-time, not CI).
4. Lifecycle: single-daemon lock; auto-spawn on first judge call when
   CHUG_JUDGE=daemon (detached self-exe spawn, healthz wait with
   bounded budget, clear stderr line); `chug daemon --stop|--status`;
   loopd ensures the daemon at cycle start (like build warmth).
   Fail-open preserved: daemon absent/unreachable -> clients degrade
   logged exactly as the HTTP path does today.
5. Client selection: CHUG_JUDGE=daemon|http|off — daemon defaults the
   judge URL to 127.0.0.1:8421 (LAYA_URL still overrides); http keeps
   today's behavior byte-for-byte. Judgment latency budget ~50ms warm.
6. FEATURES.md gains F15 (baked-in judge daemon) with this row as
   phase 1; README risk-gate/notify sections updated (one paragraph
   each, no append-sprawl); DEPENDENCIES.md (T203) lists the daemon as
   the judge provider replacing the external layad note.

## Tests

- Golden-vector parity (committed fixtures, 1e-3).
- Protocol drop-in: fixture /judge request -> response shape matches
  layad's (fields, types, error shape) — pinned by contract test.
- Lifecycle: second `chug daemon` exits on the lock; auto-spawn brings
  up a healthy daemon; --stop leaves no orphan.
- Feature-off build compiles with zero candle deps (cargo tree pin);
  release-matrix builds compile (metal mac, cpu linux).

## Out of scope

- /hook/* layad parity (Claude Code hooks migration off Python layad —
  phase 2); F13 routing endpoint + hot checkpoint reload (phase 2+);
  CUDA; sub-F16 quantization; removing the HTTP fallback path.
