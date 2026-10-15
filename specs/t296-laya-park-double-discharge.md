# T296 — loopd T259 park-discharge probes the corpus before appending (double-discharge guard)

## Repo context

`loopd.sh`'s T259 triage layer parks each `laya-triage` consult record
(`.chug/loopd/laya-pending.json` — one parked record at a time) and the next
look backfills its outcome via `laya_backfill`, which consumes the park (the
`rm` in `laya_record` right after the backfill). The designed invariant —
"ONE outcome per triage id, ever" (the `laya_backfill` comment, pinned
loopd-internally by
`a_pending_surviving_an_intervening_non_triage_cycle_backfills_exactly_once`)
— covers only loopd's OWN double-backfill. An OUT-OF-BAND outcome appended
straight to `.chug/decisions.jsonl` never touches the park file, so the
supervisor's next look backfills a consult that already carries an outcome:
a duplicate subject in the F13 corpus.

Measured twice on the live corpus (the class's two sightings, both
zero-casualty, both verdicts agreeing):

- `d1791919548-loopd4` — outcomes `d1791920473-7` (the cycle-686 wrap's
  T261 batch, "the next-look park convention") AND `d1791921551-loopd5`
  (the supervisor's own next-look discharge).
- `d1792039974-loopd92` — outcomes `d1792040908-7` (the cycle-846 wrap's
  T261 batch) AND `d1792042010-loopd93` (the supervisor's cycle-850
  pre-launch discharge). Found at the cycle-850 trip-180 eval; the audit's
  duplicate census names it (the second sighting files this row — the
  first was grandfathered).

The fix is loopd-side and mechanical: `laya_backfill` probes the corpus
for an existing outcome naming the parked triage id BEFORE appending; a
hit consumes the park silently (a `loopd.log` note, no record appended).
The guard is writer-agnostic — whichever writer (the wrap's T261 batch or
the supervisor's next look) lands first wins, and the tally of outcome
records stays == unique triage subjects. LOOP-SPEC's T261 batch clause is
NOT edited (the guard makes the double impossible regardless of which
writer races).

Files: `loopd.sh` (the `laya_backfill` guard leg + its comment),
`tests/loopd_laya_triage.rs` (the behavioral pin + any needle-count
reconciliation). Pin surface (T285): the `count_eq` needles over loopd.sh
in `tests/loopd_laya_triage.rs` (the TRIAGE_HIGH / consult-sites /
function-scoped pins) and the "park" doctrine mentions in
`tests/loop_spec_eval_read_path_pointer.rs` +
`tests/loop_spec_loopd_log_pointer.rs` — the change adds one function body
leg and must not perturb those counts; any count the new text moves is
reconciled with the pin's own contract-change comment.

estimate: ~60 changed lines (loopd.sh ~25 + tests ~35).

## Requirements

1. `laya_backfill` in `loopd.sh`, after extracting the parked triage `id`
   and before `laya_record_write` appends the outcome, probes
   `.chug/decisions.jsonl` for an existing record with `class=="outcome"`
   and `subject == "$id"` (a jq select over the corpus — one bounded read,
   fail-open to APPEND on any probe error: an unreadable/missing corpus or
   a jq failure must never block the designed backfill, matching the
   function's existing `|| id=""` tolerance shape).
2. Probe hit: log ONE line to `loopd.log` ("already backfilled out-of-band
   — consuming the park silently" or equivalent wording naming the triage
   id) and return 0 WITHOUT appending — the existing `rm` consumption in
   `laya_record` still runs, so no stale park survives.
3. Probe miss: the existing backfill path runs unchanged (labels,
   id-minting, T263 conventions untouched).
4. The `laya_backfill` comment gains the out-of-band rationale: the "ONE
   outcome per triage id" invariant now covers writers beyond loopd (the
   wrap's T261 batch discharge is the observed racer; the park-file `rm`
   alone cannot see it).
5. New behavioral pin in `tests/loopd_laya_triage.rs`: a fixture cycle
   where the parked consult's id ALREADY carries an outcome record in the
   fixture's `.chug/decisions.jsonl` → the next look appends ZERO new
   outcome records (the corpus's outcome count for that subject stays 1),
   the park file is consumed, and the log carries the silent-consumption
   line. The existing
   `a_pending_surviving_an_intervening_non_triage_cycle_backfills_exactly_once`
   pin stays green unchanged.
6. No other repo file changes; the T249 lint passes.

## Acceptance

- The new pin RED-proven: with the guard leg reverted, the fixture's
  double-discharge produces 2 outcome records for the parked subject and
  the test fails; with the guard, exactly 1.
- `cargo test --release --test loopd_laya_triage` green, plus the two
  loop_spec pointer targets green.
- `grep -c` reconciliation: every pre-existing `count_eq` needle over
  loopd.sh in `tests/loopd_laya_triage.rs` still passes (or its count moved
  with the pin's own contract-change comment).

check: export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared; touch src/*.rs tests/*.rs; cargo test --release --test loopd_laya_triage --test loop_spec_eval_read_path_pointer --test loop_spec_loopd_log_pointer
