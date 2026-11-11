# T361 — dormant release flush: tags must not wait on a dormant queue (release-only cycle)

estimate: ~80 changed lines (loopd dormant-mode release leg + fixtures)

check: sh -n loopd.sh && CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-validate-a cargo test --release --test loopd_dormant_mode --test loopd_model_routing --test loopd_laya_triage

## Why

Live 2026-11-11: T360 (blocked leg) is done in main, T359 and T328 are
done too — three distinct flip-ids on v0.25.0, so a release is due by
the loop's own rule. But the queue is empty and the loop is DORMANT
(T325: cheap-exit always, suspended valve, 1800s sleeps), and release
recounts happen inside EVAL cycles — which dormancy suppresses. Result:
v0.25.0 is the latest tag while main is 27 commits ahead with a release
that will never cut until new work arrives. Fleet Primary's acceptance
suite is armed on that tag, so the blocked leg can't verify or the shim
retire. Dormancy must not freeze the release channel.

## What

1. **Release flush leg in loopd's dormant path**: when dormant AND
   `git describe`'s distinct flip-id count since the last tag ≥ 3 (the
   existing T245 recount), loopd runs a RELEASE-ONLY cycle instead of a
   full eval: compute the tag per the normal rule, cut + push it, log
   `dormant release flush (T361): vX.Y.Z (3 flips: T###, T###, T###)`.
   No orchestrator, no LLM call — the tag is a mechanical operation;
   this is the T258 cheap-exit's sibling, not a paid eval.
2. **Dormant evals (T325's weekly audit) gain the recount too** so the
   flush also happens on any natural audit cycle (belt and suspenders).
3. **No flush when < 3 flips** (rule unchanged); no flush when the
   current HEAD's tree differs from the tag candidate's (the existing
   anchor-safety rules apply).

## Tests

- loopd fixture: dormant + 3 distinct flips → tag cut+pushed with the
  flush log line, zero LLM calls (assert no model invocation).
- dormant + 2 flips → no tag, normal cheap-exit.
- dormant + 3 flips + anchor-unsafe candidate → no tag, named skip.
- Wake semantics unchanged: an open row still exits dormancy first.

## Compat

Only the dormant path changes; active-queue release logic is untouched.
The flush is mechanical and free — no new paid cycles in dormancy.

## Out of scope

- Backfilling older missed tags (this item's own landing flushes the
  current one: T359 + T328 + T360 + itself).
- Changing the 3-flip threshold (existing T245 rule).
