# T230 — ctx-edit trigger calibration: put the `--ctx-warn-at-tokens` nudge live on the loopd orchestrator launch (measurement row)

check: set -o pipefail; export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-validate-a; touch src/*.rs tests/*.rs; cargo test --test loopd_model_routing --test readme_layout --test todo_consistency

estimate: ~150 changed lines all-in (doctrine+pin carrier row — the
cycle-105 calibration: narrative ~50 lands ~3x with the pin leg
machinery, README clause, and spec)

## Concern

T192's live-context-editing surface (F14 phase 2, landed cycle 89) has
ZERO `ctx-edit` fires across all 549 events files in the corpus
(`.chug/eval-digest.md`: 548 files carry the `ctx-edit fires:` line,
every one 0; the 549th is the evaluating cycle's own live stream). The
surface is dormant-by-INCENTIVE, not known-broken: the one-shot
occupancy nudge that names LIVE_CTX editing as the remedy
(`--ctx-warn-at-tokens`, src/driver.rs:1136, notice text in
src/live_ctx.rs `occupancy_notice`) defaults to 0 = off, and nothing in
the loop fleet sets it — so trim (src/trim.rs `TRIM_ABOVE_TOKENS =
120_000`) silently handles every overflow before any model is ever told
it could compact its own context for a free turn. The cycle-103 eval
(§3) pre-committed: "if the next eval still reads zero, file a
trigger-calibration measurement row — the surface's value claim is
measured-by-telemetry per its own doctrine." Still zero → this row.

The measurement question the row opens (it does NOT prejudge the
answer): with the nudge LIVE below the trim threshold, does any
orchestrator ctx-edit fire? Either verdict closes the question —
fires: T192's value claim gets its first production data; zero fires
across cycles whose trim fires prove 100k+ crossings happened: the
surface is recorded dormant-by-incentive and the per-eval re-check
ends.

## Repo context

- loopd.sh:513-516 is the orchestrator launch line
  (`./target/release/chug run --spec LOOP-SPEC.md --goal ... --model
  "$orch_model" --max-iters 200 --max-minutes 360`). It sets no
  `--ctx-warn-at-tokens`, so every orchestrator runs with the nudge
  off.
- src/driver.rs:1136-1144: the nudge fires once per run
  (`ctx_warned` latch) when the estimated context crosses the flag's
  value, injecting `occupancy_notice` pre-call. Default 0 = off
  (src/driver.rs:701, 855).
- src/trim.rs:12: `TRIM_ABOVE_TOKENS: usize = 120_000` — the automatic
  collapse. 100_000 gives the nudge a ~20k-token remedy window BEFORE
  trim can fire; the model either acts (a free edit-only turn) or trim
  proceeds exactly as today. Zero behavior change below 100k, zero
  behavior change when the model ignores the nudge.
- tests/loopd_model_routing.rs pins the launch line via
  `INVOCATION_MODEL` (`--model \"$orch_model\" --max-iters 200
  --max-minutes 360`) — the carrier pin for launch-argv changes (T198
  amended it for the 240→360 raise, T187-style: the pin follows the
  deliberately re-keyed carrier with the justification named).
- The digest already surfaces the measurement:
  `.chug/eval-digest.md` carries a per-file `ctx-edit fires:` line
  (T46/T192), and `trim fires:` is the proxy evidence that crossings
  happened (a cycle with trim fires > 0 crossed 120k, hence 100k).

## Requirements

1. **loopd.sh launch line** gains `--ctx-warn-at-tokens 100000` (after
   `--max-minutes 360`), with an in-comment rationale: why the nudge
   (T192's surface has never fired — measurement, not assertion), why
   100_000 (below trim's 120_000 so the remedy window precedes the
   collapse), and where the verdict lands (the digest's per-file
   ctx-edit line, read by the next evals).
2. **Carrier pin amended T187-style** in tests/loopd_model_routing.rs:
   `INVOCATION_MODEL` extends to include `--ctx-warn-at-tokens 100000`
   with the justification comment (the launch line was deliberately
   re-keyed by this row), and one NEW leg asserting the flag needle
   appears EXACTLY ONCE in loopd.sh (a duplicate flag would silently
   re-latch). RED-proof the amendment: launch line without the flag →
   the pin dies.
3. **README loopd section** gains ONE clause where the launch-time
   behavior is already documented (the `CHUG_BASH_TIMEOUT=300`
   paragraph's neighborhood): loopd launches the orchestrator with the
   100k occupancy nudge so LIVE_CTX compaction gets a pre-trim remedy
   window, and the digest's ctx-edit line is where the measurement
   lands. INTEGRATED, not an appended bullet.
4. **Scope discipline (stated, not code):** the flag goes on the
   ORCHESTRATOR launch only. Delegate child launches are unchanged —
   children's context curves and free-turn economics differ, and
   extending to them is a verdict-dependent follow-up, not this row.
5. **The verdict is eval-side (acceptance, not code):** the spec's
   measurement plan — within the next ~6 cycles the eval reads the
   digest: any ctx-edit fire > 0 with the nudge live closes the
   question one way (surface lives); zero fires across cycles with
   trim fires > 0 closes it the other (record dormant-by-incentive in
   EVALUATION.md; the per-eval re-check ends; the surface stays — it
   costs nothing unfired).

## Tests

- The amended `INVOCATION_MODEL` pin leg + the new exactly-once leg in
  tests/loopd_model_routing.rs, each RED-proven (flag removed → RED;
  flag duplicated → RED).
- readme_layout + todo_consistency stay green (README clause
  integrated; no TODO.md row-format touch beyond the one row).

## Acceptance

- loopd.sh launch line carries the flag with the rationale comment.
- Pins green and RED-proven; the full named test targets green.
- README clause integrated into the loopd section.
- The measurement plan (req 5) is stated in the commit message so the
  next eval reads the verdict surface without archaeology.

## Out of scope

- Delegate/child launch flags, chat-mode nudges, any change to trim's
  thresholds or live_ctx's gates, any retirement of the LIVE_CTX
  surface (that decision rides the verdict, not this row).
