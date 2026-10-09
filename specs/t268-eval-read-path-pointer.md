# T268 — LOOP-SPEC eval-read-path pointer clause (name `.chug/eval-delta.md` + `.chug/eval-state.md`)

## One concern

LOOP-SPEC.md never names the T260 evaluation read-path files — zero
mentions of `eval-delta` / `eval-state` (verified at filing) — so an eval
or orchestrator stream probing the evaluation state guesses a literal and
misses: the `guarded-path-probe` census's state-file sub-variant has fired
FOUR times across trips 47/48 (.json-variant state-path guesses), 53
(`.chug/eval-state.json` — d1791549131-2), and 54
(`.chug/loopd/eval-state.json` — wrong dir AND .json variant,
d1791554509-2). Every fire masked (`2>/dev/null` / `|| echo`), every fire
self-corrected one probe later via discovery (the `.chug/` listing or the
delta read itself), zero casualty — ~1 iteration per fire. META-META-SPEC
names the real paths correctly (4 mentions), but the misses keep firing:
the eval stream's own spec (LOOP-SPEC, the system prompt) is the surface
it reads first, and that surface is silent. The remedy mirrors T267
exactly (the loopd.log pointer, landed 00bc596 — its first live exercise
passed: the trip-54 stream's loopd.log probes hit first-try every time):
a ONE-CLAUSE pointer naming the real paths at the first relevant mention,
plus a drift pin.

## Repo context

- Carrier: LOOP-SPEC.md, Phase 1 opening paragraph — the sentence
  "**prefer `.chug/events.jsonl` over transcripts** — it is jq-mineable
  and untrimmed …". The read-path pointer rides the same sentence block:
  the evaluation inputs a stream probes at ingress are the events stream
  (already named) plus the T260 read-path pair (unnamed — this row).
- The real paths are load-bearing fact: `loopd.sh` rebuilds
  `.chug/eval-delta.md` before every cycle (loopd.sh line ~1149 comment),
  and the evaluation maintains `.chug/eval-state.md` (META-META-SPEC's
  T260/T262/T266 clauses). NO `.json` variant of either file exists — the
  sticky-guess shape is exactly the `.json`-variant guess (trips 47/48/53)
  now joined by a wrong-dir guess (trip 54).
- No test pins the strings `eval-delta` / `eval-state` in LOOP-SPEC.md's
  prose today (verified at filing: zero mentions, so nothing to break;
  the T267 pointer test tests/loop_spec_loopd_log_pointer.rs is the
  shape precedent for the pin).
- estimate: ~10 changed lines (LOOP-SPEC.md one clause + the new pin
  test file; the T267 lesson: the pin file carries the bulk — ~6 named
  → 159 all-in there — so the doctrine-text half stays ~4 lines and the
  pin file is the rest).
- Doctrine row: SOLO (no overlap with anything), kimi validation REQUIRED
  (LOOP-SPEC.md is on the core list).

## Requirements

1. LOOP-SPEC.md Phase 1's opening paragraph gains ONE clause naming the
   read-path pair — the literal strings `.chug/eval-delta.md` and
   `.chug/eval-state.md` must both appear — so a cold eval stream reads
   the written pointer instead of guessing (shape free, one sentence max;
   e.g. naming them as the T260 read-path files with a "no `.json`
   variant exists" honesty half-clause).
2. A NEW pin test file `tests/loop_spec_eval_read_path_pointer.rs`
   asserts LOOP-SPEC.md contains BOTH literals `.chug/eval-delta.md` and
   `.chug/eval-state.md` — the pointer cannot silently drift out in a
   future doctrine edit.
3. No other LOOP-SPEC.md text changes; the Phase-1 prose semantics are
   untouched (insert one clause, change nothing else).

## Tests

- The new pin (req 2) GREEN against the edited LOOP-SPEC.md.
- RED-proof (orchestrator, pre-merge): revert the LOOP-SPEC.md edit in
  the worktree → the pin FAILS; restore → GREEN.
- The affected LOOP-SPEC pin family still green:
  `cargo test --release --test loop_spec_eval_read_path_pointer --test loop_spec_loopd_log_pointer --test loop_spec_cheap_exit --test loop_spec_empty_chain --test loop_spec_doctrine_prune`
  (the doctrine carrier's existing pins must not break; the T267 pointer
  pin runs beside the new one as the family's shape proof).

## Acceptance

- `grep -c '\.chug/eval-delta\.md' LOOP-SPEC.md` ≥ 1 AND
  `grep -c '\.chug/eval-state\.md' LOOP-SPEC.md` ≥ 1.
- The pin test file exists and passes; the RED-proof was run and named
  in the merge/flip notes.
- Diff touches ONLY LOOP-SPEC.md + the one new test file (docs+pin only
  — the T80 docs-only classification does NOT shrink the gates: the edit
  touches a pinned doctrine carrier, so the editor ALSO runs the
  affected pin files' tests per LOOP-SPEC step 3's docs-only clause; the
  full suite is not required — zero src/ changes).

check: grep -q '\.chug/eval-delta\.md' LOOP-SPEC.md && grep -q '\.chug/eval-state\.md' LOOP-SPEC.md && export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-validate-a && touch src/*.rs tests/*.rs; cargo test --release --test loop_spec_eval_read_path_pointer --test loop_spec_loopd_log_pointer --test loop_spec_cheap_exit --test loop_spec_empty_chain --test loop_spec_doctrine_prune
