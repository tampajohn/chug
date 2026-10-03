# T209 — dispatch-time spec-size gate (the T173 measure clause resolves)

check: export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-validate-a; touch src/*.rs tests/*.rs; cargo test --test todo_consistency --test loop_spec_recovery

## Repo context

Doctrine row — LOOP-SPEC.md + META-META-SPEC.md edit; runs SOLO.

The T110 filing-time ~500-line estimate ceiling exists, but nothing enforces
it AT DISPATCH, and the cycles-91–94 census says the cost is now the
binding one:

- t197 impl: died 50m29s at 54/80 (minutes-bound), goal unaccepted.
- t203 impl: died 50m35s at 57/80 (minutes-bound, a DOCS row whose spec
  `check:` is the full `cargo test` — each check run costs minutes),
  goal unaccepted.
- T204 arc: 6 glm impl/fixup segments — aborts: iteration ×4, time ×2,
  stuck ×1 — plus both kimi validators died 60/60 (verdicts written first;
  the written-verdict doctrine held). Child B was ONE HALF of the T110
  split and still landed ~2,100 added lines — the split was by COMPONENT
  (inference core / daemon surface), not by acceptance surface, so the
  half was 4× the ceiling.
- Delta tally: 10 T204 child segments, 9 aborts, ZERO goal-accepted
  children; the arc landed entirely on recovery doctrine (T63 resumes,
  orchestrator-finish, verdict-file harvesting).

This resolves LOOP-SPEC's T173 measure clause ("if >2 of the next 8 impl
children still die at the 50-minute budget with the goal unaccepted, the
next eval considers spec-size discipline instead of further raises"):
t197 + t203 minutes-bound + the T204 iteration-bound deaths trip it. The
remedy is dispatch-time size discipline, NOT further budget raises.

estimate: ~70 changed lines (LOOP-SPEC.md Phase-2 step 2 + the T173
measure-clause resolution + META-META-SPEC.md spec quality bar + one pin).

## Requirements

1. LOOP-SPEC Phase 2 step 2 (implementation child) gains a dispatch-time
   gate, stated BEFORE the launch template: the orchestrator re-reads the
   spec's `estimate:` line at dispatch; an estimate > ~500 lines (or a
   missing estimate on a non-trivial row) is NOT dispatched — the row is
   re-split first (2–3 rows) and the specs rewritten, by the orchestrator
   or via a quick filing pass.
2. The split rule gains "by acceptance surface, not by component": each
   split half's own diff estimate must be ≤ ~500 lines — a split whose
   half still exceeds the ceiling (T204 child B: ~2,100 lines after a
   by-component split) is re-split again. Naming rule: each half must have
   its own independently-gateable `check:` surface.
3. The T173 measure clause in LOOP-SPEC (the minutes-raise paragraph) is
   marked RESOLVED with this row's ref — census tripped (t197/t203
   minutes-bound, T204 iteration-bound), remedy enacted = the dispatch
   gate, no further budget raises.
4. META-META-SPEC's spec quality bar gains one sentence: the estimate is a
   DISPATCH-TIME contract, re-checked by the orchestrator — filing-time
   honesty is not enforcement (the T204 row said "~800" and the arc
   landed ~3,000+ all-in across both halves).
5. Validator note recorded in the same LOOP-SPEC paragraph that carries
   the validator budget measure clause: both t204 validators died 60/60
   WITH verdicts written (the verdict-file doctrine absorbed it) — the
   clause's letter (verdict UNANNOUNCED) is not tripped; no validator
   budget change; the next trip trims default mutation-leg counts.
6. Pin the dispatch-gate sentence and the by-acceptance-surface rule in an
   existing `tests/loop_spec_*.rs` or `tests/todo_consistency.rs` leg
   (editor's pick), RED-proven.

## Tests

The req-6 pin; doctrine carriers stay green (the check line).

## Acceptance

- LOOP-SPEC.md carries the dispatch-time gate + by-acceptance-surface rule
  + the T173 RESOLVED marker + the validator note.
- META-META-SPEC.md carries the estimate-is-a-dispatch-contract sentence.
- Pins green; each pin dies on its deletion mutant.
