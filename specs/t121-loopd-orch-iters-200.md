# T121 — loopd orchestrator cap `--max-iters 160 → 200`

check: bash -n loopd.sh && cargo test --test loopd_model_routing --test loopd_reexec

## Repo context

`loopd.sh` launches each cycle's orchestrator with
`--model "$orch_model" --max-iters 160 --max-minutes 240`
(loopd.sh:218). The sizing comment (loopd.sh:206) reads "160 fits
eval + 3 items + wrap; minutes never binding (56–117 of 240)" — a
cycle-16-era estimate. The value is pinned verbatim by
`tests/loopd_model_routing.rs:73`
(`"--model \"$orch_model\" --max-iters 160 --max-minutes 240"`).
Prior raises: T27 (80→120, cycle 11), T36 (120→160, cycle 16) — same
mechanism each time: a watch criterion, then a death, then the raise.

**The criterion tripped in cycle 61** (2026-09-28): a fresh-eval cycle
worked a FIVE-item queue + wrap in 160/160 iterations and died at the
ceiling AFTER the wrap commit was pushed (ccd4b0a, 12:06:36Z) but
BEFORE `goal_complete` — loopd.log 12:06:57Z: "cycle ended WITHOUT
goal complete (consecutive failures: 1)". The wrap was complete
(gates green, pushed); the only casualty was the goal_complete call
and 1/3 of loopd's HALT budget (3 consecutive failures halt). Cost was
benign THIS time; the same 5-item-eval shape recurs (this very cycle
is one), and a death mid-wrap instead of post-wrap loses real work.
Minutes had headroom (171 of 240 for cycle 61), iterations did not.
Sizing: eval ≈45-55 + 5 items ≈28-35 each + wrap ≈10 ⇒ a 5-item eval
cycle wants ≈195-240; 200 fits it with the slim margin the T21/T92
era preferred to keep small (the cap is also the stuck-cycle detector
— loopd's HALT guard needs 3 consecutive failures, so 200 costs at
most 40 extra stuck iterations before relaunch, minutes still
non-binding).

Sweep (verified at filing): `160` appears ONLY at loopd.sh:206
(comment) + :218 (flag) + tests/loopd_model_routing.rs:73 (needle);
no LOOP-SPEC/README/META-SPEC reference. loopd re-execs itself
between cycles when its script changes (README loopd section), so the
merged raise activates at the next cycle automatically — no operator
restart.

estimate: ~25 changed lines (≈3 script + ≈2 test + comments)

## Requirements

1. loopd.sh: `--max-iters 160` → `--max-iters 200`; the sizing comment
   rewritten with the cycle-61 evidence (5-item eval cycle died
   160/160 post-wrap pre-goal_complete; minutes 171/240 non-binding;
   HALT-guard tradeoff named).
2. tests/loopd_model_routing.rs: the needle constant updated to
   `--max-iters 200 --max-minutes 240` (and any adjacent comment
   carrying the 160 rationale).
3. No other flag, model, or routing change; the freshness predicate
   and both model knobs untouched; `bash -n loopd.sh` clean.
4. RED-prove the pin: the updated test fails against the OLD loopd.sh
   value (or the child demonstrates the needle mismatch both ways).

## Tests

The existing loopd_model_routing + loopd_reexec suites are the
regression set; the needle update is the test change. No new legs
required beyond what the suites already assert (the child adds one
only if an existing leg's coverage gap appears).

## Acceptance

- The spec's own `check:` line green.
- Next cycle's loopd.log line shows the new cap (noted at wrap, not
  a gate).

## Out of scope

- `--max-minutes` changes (never binding: 171/240 worst observed).
- Child budgets (impl 80 / validator 50 — separate watch items, both
  zone-free this era).
- Any LOOP-SPEC text (the cap lives in loopd.sh alone).
