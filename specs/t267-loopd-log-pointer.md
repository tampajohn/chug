# T267 — LOOP-SPEC loopd-log pointer clause (name the real path `.chug/loopd/loopd.log`)

## One concern

LOOP-SPEC Phase 1's T258/T259 prose says "loopd.log" bare in three places
(the cheap-exit paragraph ×2, the triage-layer paragraph ×1), but the
supervisor log lives at `.chug/loopd/loopd.log`. Eval and orchestrator
streams probing the supervisor log guess the bare literal `.chug/loopd.log`,
miss, and self-correct via discovery (`find`/`ls`) one command later — the
`loopd.log-path-miss` census class: 0→2 first sighting at the cycle-241 eval
(d1791466562-2, trigger hung: ≥4 fires files this row), 2→3 at trip 52
(d1791545860-3), 3→4 at trip 53 (this filing's evidence, record id in the
trip-53 eval text). Every fire masked, every fire self-corrected, zero
casualty — ~1 iteration per fire. The trigger's own remedy: a ONE-CLAUSE
pointer naming the real path at the first bare mention.

## Repo context

- Carrier: LOOP-SPEC.md, Phase 1 "The cheap exit (T258)" paragraph — the
  sentence "The supervisor's disposition line records the predicate inputs
  (row count, last-change hash, freshness) in loopd.log — the streak math
  (T237) and the valve (T247) read the same git/loopd.log record, never an
  LLM's say-so." (lines ~110–111). First bare mention is "in loopd.log".
- The other two bare mentions ("git/loopd.log record" same sentence; "one
  note per cycle in loopd.log" in the T259 triage paragraph, ~line 138)
  read unambiguously once the first mention names the path — they stay
  byte-identical (one clause, minimal doctrine churn, per the trigger's
  letter).
- The real path is load-bearing fact: `loopd.sh` writes
  `.chug/loopd/loopd.log` (and per-cycle `.chug/loopd/cycle-<ts>.log`);
  tests/loop_spec_cheap_exit.rs's fixture uses `.chug/loopd/loopd.log`
  (line ~278). No test pins the bare "loopd.log" string in LOOP-SPEC.md's
  prose (verified at filing: the tests-side hits are fixtures/comments).
- estimate: ~6 changed lines (LOOP-SPEC.md only — one parenthetical clause
  + the pin test addition).
- Doctrine row: SOLO (no overlap with anything), kimi validation REQUIRED
  (LOOP-SPEC.md is on the core list).

## Requirements

1. LOOP-SPEC.md's first bare "loopd.log" mention gains the real path as a
   parenthetical — `in loopd.log (\`.chug/loopd/loopd.log\`)` — so a cold
   probe reads the written pointer instead of guessing. The other two
   mentions stay byte-identical.
2. A pin test (new `tests/loop_spec_loopd_log_pointer.rs` or an added
   assertion in an existing LOOP-SPEC pin file, author's choice) asserts
   LOOP-SPEC.md contains the literal `.chug/loopd/loopd.log` — the pointer
   cannot silently drift out in a future doctrine edit.
3. No other LOOP-SPEC.md text changes; the T258/T259 prose semantics are
   untouched.

## Tests

- The new/extended pin (req 2) GREEN against the edited LOOP-SPEC.md.
- RED-proof (orchestrator, pre-merge): revert the LOOP-SPEC.md edit in the
  worktree → the pin FAILS; restore → GREEN.
- The affected LOOP-SPEC pin family still green:
  `cargo test --release --test loop_spec_cheap_exit --test loop_spec_empty_chain --test loop_spec_doctrine_prune`
  (the doctrine carrier's existing pins must not break).

## Acceptance

- `grep -c '\.chug/loopd/loopd\.log' LOOP-SPEC.md` ≥ 1.
- The pin test exists and passes; the RED-proof was run and named in the
  merge/flip notes.
- Diff touches ONLY LOOP-SPEC.md + the one test file (docs+pin only — the
  T80 docs-only classification does NOT apply to the gates: the edit
  touches a pinned doctrine carrier, so the editor ALSO runs the affected
  pin files' tests per LOOP-SPEC step 3's docs-only clause; the full suite
  is not required — zero src/ changes).

check: grep -q '\.chug/loopd/loopd\.log' LOOP-SPEC.md && export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared && touch src/*.rs tests/*.rs; cargo test --release --test loop_spec_cheap_exit --test loop_spec_empty_chain --test loop_spec_doctrine_prune
