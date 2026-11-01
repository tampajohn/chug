# T315 — `scripts/eval-delta.sh` splice-check: accept the corpus-tail identity for the all-records carry-0 ring

check: export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-validate-a && touch src/*.rs tests/*.rs; cargo test --release --test eval_state_delta --test eval_outcomes_carry --test loop_spec_cheap_exit --test loop_spec_check_wall --test loop_spec_doctrine_prune --test loop_spec_eval_read_path_pointer --test loop_spec_recovery --test loop_spec_scoped_walk --test loop_spec_validation_lane --test todo_consistency --test internal_info_lint -- --test-threads=4

## Repo context

`scripts/eval-delta.sh`'s `splice_verdict` (awk over the PREV ring
snapshot and the current STATE ring) declares
`violated: nothing carried verbatim from the previous ring (summarization?)`
when the ring is full (k == maxd == 12) and zero lines carry (j == 0).
The ALL-RECORDS wrap — a cycle whose new decision-corpus records fill
the ring exactly (cycle 2729's 12: six decisions + six outcome
backfills) — writes prepend-12/carry-0 BY DESIGN, and the wrap verifies
the STRONGER anchor at fill time: the state's ring == the decision
corpus's tail-12 REVERSED (the wrap fills' standing all-records
verification; the corpus `.chug/decisions.jsonl` is append-only per
T70, LOCAL-ONLY/untracked per the cycle-132 design 8df655b, but
host-present — the delta builder runs on the host and already reads
`.chug/` files for its aborts/goals probes, and `jq` is already a
script dependency). The splice check knows only the weaker
state-vs-state anchor, so EVERY carry-0 wrap forces a spurious
`FULL-READ REQUIRED (splice-violated)` on the next launched eval — the
fail-safe direction, a bounded burn: the cycle-2730 eval is the
evidence (the delta verdict read FULL-READ on a state whose ring
verified EXACT against the corpus tail-13-minus-newest reversed at the
eval-write probe). META-META-SPEC's rotate-at-most-11 sentence
implements splice-verifiability against the PREVIOUS state snapshot;
the all-records practice verifies against the append-only corpus tail —
strictly stronger and host-verifiable. The fix teaches the check the
corpus-tail acceptance leg and names the case in the doctrine.

estimate: ~90 changed lines (the script ~22 + the doctrine rider ~8 +
the pin legs ~55 + the spec).

## Requirements

1. **`scripts/eval-delta.sh`** — the nothing-carried branch gains the
   corpus-tail acceptance leg:
   - The CALLER (not the awk) computes the corpus-tail identity when
     `splice_verdict` prints the nothing-carried violation: the last
     `MAX_DECISIONS` record ids of `.chug/decisions.jsonl`, each
     prefixed `- `, reversed with the portable awk reversal
     (`awk '{a[NR]=$0} END{for(i=NR;i>=1;i--) print a[i]}'` — never
     `tail -r` [BSD-only] and never `tac(1)` [GNU-only]; the T299
     compose-time rule binds).
   - If that reversed tail equals `state_decisions "$STATE"` exactly,
     the verdict DOWNGRADES to
     `ok (carried 0, rotated 0, appended N, corpus-tail-verified)` — an
     `ok*`-form the existing caller case accepts unchanged.
   - A mismatch keeps the violation verbatim (fail-closed: an
     unreadable or missing decisions.jsonl ALSO keeps the violation —
     never a false-green).
2. **The META-META-SPEC.md rider** (the ring-discipline paragraph in
   the Write-EVALUATION section): the rotate-at-most-11 sentence gains
   the all-records case — a cycle whose new corpus records fill the
   ring MAY write prepend-12/carry-0 when the wrap verifies ring ==
   corpus tail-12 reversed (the stronger anchor, append-only per T70),
   and the delta's splice-check accepts that case via the corpus-tail
   leg.
3. **The pin legs** (`tests/eval_state_delta.rs`):
   - (a) a fixture full-ring state whose ring == the fixture corpus's
     tail-12 reversed with ZERO carried vs the previous snapshot →
     verdict STATE-HIT (splice ok corpus-tail-verified);
   - (b) a fixture full-ring carry-0 state whose ring does NOT match
     the corpus tail → still FULL-READ (the existing pin's contract
     preserved).
   - The fixture gains a decisions.jsonl writer — inspect-first per
     T288: read the harness's existing fixture helpers
     (`tests/eval_state_delta.rs`) and reuse their fake-repo/state
     writers; add the corpus writer in the same style.
   **The T285 pin-surface declaration** (probed at filing): the splice
   pin family in `tests/eval_state_delta.rs`
   (`splice_plain_prepend_is_legal_below_the_cap`,
   `splice_full_ring_rotation_from_the_tail_is_legal`,
   `splice_violations_force_full_read`,
   `splice_empty_previous_ring_with_nonempty_current_is_a_legal_append`)
   asserts the existing verdict strings — the UNVERIFIED carry-0 leg's
   `nothing carried verbatim` needle STAYS (leg (b) pins it); the new
   `corpus-tail-verified` needle appears ONLY in the new ok-form and
   its pin leg (a). The doctrine rider edits the ring-discipline
   paragraph: `grep -rn "rotate at most" tests/*.rs` probed at filing
   returns META-META-SPEC.md only (zero test pins on the sentence), and
   the `loop_spec_doctrine_prune` §6-marker legs live outside this
   paragraph. The check line above runs every target that reads
   META-META-SPEC.md (grep -ln probed at filing:
   eval_outcomes_carry, eval_state_delta, loop_spec_cheap_exit,
   loop_spec_check_wall, loop_spec_doctrine_prune,
   loop_spec_eval_read_path_pointer, loop_spec_recovery,
   loop_spec_scoped_walk, loop_spec_validation_lane,
   todo_consistency) plus internal_info_lint.

## Tests

- Legs (a) and (b) green + the existing eval_state_delta functions
  green.
- RED-proofs (the orchestrator, pre-merge): leg (a)'s fixture with the
  acceptance leg stubbed out → red (FULL-READ); leg (b)'s fixture with
  the corpus deliberately corrupted → must STAY red-green
  (FULL-READ preserved).

## Acceptance

- The cycle-2730 case replays green: a state written prepend-12/carry-0
  with the corpus tail matching reads STATE-HIT (corpus-tail-verified)
  instead of the spurious FULL-READ; a corrupted carry-0 still reads
  FULL-READ.
- The META-META-SPEC rider lands with zero pin drift (the whole check
  line green).
