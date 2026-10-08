# T259 — loopd Laya triage: the judge daemon gates non-trip eval launches

check: cd "$(git rev-parse --show-toplevel)" && export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared && touch src/*.rs tests/*.rs && cargo test --release

estimate: ~350 lines (triage call + state-pack + thresholds + records + pins)

## Concern

Operator 2026-10-07: "use our laya chugd to make this $0/day if
there's nothing to do." T258's cheap exit already zeroes the
MECHANICALLY empty case; the remaining empty-day burn is borderline
predicates (technically non-empty, usually nothing) and valve-trip
kimi evals. The judge daemon (T204, live on the loop host, ~23ms
CPU inference, $0 marginal cost) is the System One triage layer for
those cases: confidence-gated routing — confident-empty skips (>$0),
confident-work launches glm (~$0.05), unsure escalates to kimi. This
is F13's confidence-gated cascade in production for the first time,
and every triage decision becomes distillation corpus.

## Repo context

- T258 (filed same day): mechanical cheap exit (loopd writes the
  disposition itself when the predicate is empty) + glm for borderline
  non-trip evals. This item inserts Laya between those two layers.
- The daemon serves /judge on a 0600 unix socket, host-scoped,
  fail-open (T204): daemon down or any error -> current T258 behavior,
  never a blocked loop.
- SPEC-3 constraint (classification only) fits: "does this delta need
  a full eval" is a routing classification, never a quality verdict.
- decision_log (T182 always-reminder): every Laya triage is recorded
  with inputs + verdict + confidence; outcomes backfill from what the
  loop actually did next (the T199/T200 export path) — the F13
  distillation corpus for a future fine-tuned triage judge (the
  stop-judge fine-tune recipe, AUROC 0.64 -> 0.85 precedent).
- CMU "JEV-as-a-Judge" (2026): confidence-gated cascade keeps ~99%
  accuracy at ~57% cost — the published shape of this pattern.

## Requirements

1. Triage call in loopd's eval-routing step: a compact state pack
   (rows added/closed, child deaths, files changed by class, digest
   stats, streak position) -> one `choice` question
   (needs-eval: yes/no) + confidence, over the existing daemon
   socket. Response cached in the cycle's disposition line.
2. Routing: confidence >= HIGH (0.85) + no -> skip eval, loopd writes
   the disposition ($0); yes -> glm eval; confidence < HIGH -> kimi
   eval (the hard judgment stays System Two). Valve trips route
   through the SAME triage (a valve trip Laya calls confidently empty
   writes the trip disposition with the verdict recorded; no kimi
   stream).
3. Fail-open everywhere: daemon absent/error/timeout (>2s) -> T258
   behavior exactly; one events note per cycle, never a storm.
4. decision_log record per triage: inputs (the state pack verbatim),
   verdict, confidence, route taken, and (backfilled next cycle) what
   the loop found when it looked — the accuracy census rides T200's
   export.
5. Doctrine: LOOP-SPEC routing names the three layers (mechanical ->
   Laya -> System Two) with the SPEC-3 constraint quoted; the
   thresholds live in one place with a comment pointing here.
6. Pins: fixture state packs route correctly (empty->skip, work->glm,
   low-confidence->kimi); daemon-down falls to T258; the trip
   disposition records the Laya verdict.

## Tests

- The pins above; existing pacing (T237/T247) and cheap-exit (T258)
  pins stay green.
- Acceptance: a fully empty day costs ~$0-5 (valve trips mostly
  triaged empty, recorded in Outcomes with the Laya census).

## Out of scope

- Fine-tuning the triage judge (F13's distillation phase — this item
  uses the BASE laya checkpoint; the corpus accumulates first);
  Laya gating WORK cycles (eval launches only); changing the valve
  trip count.
