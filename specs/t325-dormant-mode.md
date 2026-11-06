# T325 — Dormant mode: empty queue = cheap-exit always + weekly read-only audit; no manufactured filings

estimate: ~90 changed lines (loopd.sh routing legs + fixtures + LOOP-SPEC doctrine paragraph)

## Why

The queue hit 319 done / 0 open and the loop did not go quiet — it ate itself.
Last-7-days commit mix: 1,043 eval bookkeeping commits vs 46 real commits, and
the "real" items (T317–T324) are self-referential tooling (wrap_assert legs,
validator-survivor fixture pairs, citation hygiene). The mechanism: the T247
valve forces a paid eval every 4th consecutive TRUE cheap-exit; with an empty
backlog the eval has nothing outward to audit, so it files micro-items about
its own assertion machinery and the loop "ships" them. Cost: ~$110/day of
perpetual-motion bookkeeping. The T258 cheap-exit made empty *stretches* free
but never addressed the valve's forced paid launches — that is the leak.

The operator's expectation is the design target: an empty backlog should make
the loop NEARLY FREE, not make it invent meta-work. (Outward-facing ideation
is the sibling item T326 — deliberately separate: dormancy first, ideation
second, so ideation launches into a quiet loop.)

## What

In `loopd.sh`, at cycle routing time, before the T247 valve leg:

1. **Dormancy predicate**: `open_rows == 0` (the same row count the cheap-exit
   predicate already computes). When dormant:
   - The cheap-exit fires on EVERY cycle regardless of the TRUE streak — the
     T247 forced-eval leg is suspended. Log the disposition as
     `dormant cheap-exit (T325): streak=N suspended-valve`.
   - The valve's 4th-TRUE trip is replaced by a **weekly audit timer**: a real
     eval launches only when `now - last_eval_commit_ts >= 7d`. State rides in
     `.chug/loopd/dormant-state.json` (`last_eval_ts`, written from the newest
     `eval:` commit timestamp — no new authority, git is the source).
2. **Dormant audit evals are read-only.** When the weekly audit fires, the
   launch passes `DORMANT=1` in the env; the eval spec (LOOP-SPEC) gains the
   dormant paragraph: audit = gates green, pins verified, site-sync freshness,
   daemon/Langfuse liveness. It files TODO rows ONLY on observed breakage
   (red gate, stale site, dead daemon) and MUST NOT file loop-tooling items
   (the wrap_assert/fixture/hygiene meta class). The audit's disposition line
   names the class: `dormant audit (T325): gates=green pins=23/23 site=fresh
   daemon=ok filings=0`.
3. **Wake condition**: any open row (operator-filed or breakage-filed) exits
   dormancy immediately; the normal T247 valve semantics resume. Log
   `dormant exit (T325): open=1` on the transition cycle.

## Tests

- loopd fixture battery: `open=0, streak=4, last_eval 2d ago` → cheap-exit,
  no launch, disposition names `suspended-valve`.
- `open=0, streak=4, last_eval 8d ago` → audit launch with `DORMANT=1`.
- `open=1` → normal routing (valve live) + `dormant exit` log line.
- Eval-side: a dormant audit with a red-gate fixture files exactly one
  breakage row; a green fixture files zero.

## Out of scope

- The ideation lane (T326) — separate spec, sequences after this one.
- Changing the T247 valve's non-dormant semantics (open>0 path untouched).
- Cost dashboards (T327) — this item only stops the spend.
