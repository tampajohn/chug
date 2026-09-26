# T81 — Per-phase model routing (glm routine, kimi judgment)

check: cargo test

## Concern

kimi-k3 orchestrates EVERY cycle (~2.3s p50 + thinking tax on ~60% of
calls that are routing/bookkeeping, not judgment). glm-5-3-flash runs
85 tok/s and has 11+ consecutive clean impl rounds. Operator-approved knob
2026-09-26: glm orchestrates routine queue-working, kimi reserved for the
judgment phases. Validation independence is preserved: implementer and
validator stay different families regardless of who orchestrates.

## Repo context

- `loopd.sh`: launches the orchestrator with `--model
  anthropic-system.ai.kimi-k3` (single model for all phases).
- LOOP-SPEC: orchestrator duties = launch/poll/review/merge/bookkeeping
  (mechanical) + Phase 1 evaluation (judgment). Eval quality matters;
  routine dispatch is scripted by the spec.
- The muse lesson (M2/M3 sprint-burn) is WHY kimi orchestrates: a 30B
  orchestrator self-reviewed instead of delegating. glm-flash is also
  ~30B-class — the spec must inherit the delegation-forcing structure, not
  just swap the model name.

## Requirements

1. loopd.sh gains `LOOP_ORCH_MODEL` (default kimi-k3, current behavior) and
   `LOOP_ROUTINE_MODEL` (default glm-5-3-flash): when the queue is
   non-empty and the freshness rule would SKIP evaluation (routine cycle),
   the orchestrator launches on LOOP_ROUTINE_MODEL; fresh-eval cycles
   always launch kimi.
2. LOOP-SPEC gains the structural guard, model-agnostic: an orchestrator
   that spends >5 consecutive iterations without a delegate/bash child
   launch MUST act (launch, merge, or wrap) — the M2/M3 anti-sprint-burn
   rule made explicit for any model.
3. Validation stays kimi whenever the implementer is glm (family
   independence); a glm-orchestrated cycle NEVER lets glm validate glm.
4. Rollback: one env var (`LOOP_ROUTINE_MODEL=anthropic-system.ai.kimi-k3`)
   restores single-model operation; noted in loopd.sh comments.

## Tests

- Doctrine item: validation REQUIRED (kimi) — review checks the
  routine-vs-eval switch is driven by the freshness rule (not model
  judgment), the anti-sprint-burn guard, and family independence.
- Acceptance: two later cycles (one routine glm, one eval kimi) recorded in
  Outcomes with per-cycle wall time + outcome quality notes.

## Out of scope

- Changing validators' model; glm-orchestrated fresh evaluations; model
  fallback chains (existing glm→kimi child fallback unchanged).
