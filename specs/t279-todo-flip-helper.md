check: cd "$(git rev-parse --show-toplevel)" && export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-validate-a && touch src/*.rs tests/*.rs && python3 -m unittest scripts.test_todo_flip -v && cargo test --release --test todo_consistency --test internal_info_lint --test shared_target_dir --test loop_spec_recovery

# T279 — checked todo-flip helper: rebuild-from-cells structural flip (scripts/todo_flip.py)

**One concern:** give the TODO.md row flip an invoke target —
`scripts/todo_flip.py`, a stdlib-only python helper that rebuilds the
target row FROM CELLS and proves the structural invariants pre-write —
so orchestrators stop hand-rolling string-splice edits around the
trailing pipe (the shape that has now fired three times), and name the
helper in LOOP-SPEC's flip instructions.

## Why

The todo-flip structural-slip census stands at 3 and its armed trigger's
letter binds verbatim: "trip 87 WEIGHS the checked flip-helper row
(rebuild the row from cells, never string-splice around the trailing
pipe; assert single-physical-line post-edit)". The three fires, every one
a hand-rolled splice edit around the trailing pipe:

1. The census's origin fire (pre-counting): a flip's glued note split the
   row's cell count; the T8 guard caught it.
2. d1791632010-15 (the T275 flip): python glued the LANDED note AFTER the
   row's trailing pipe → a 7-cell row → the T8 guard FAILED pre-commit
   (`expected exactly 6 cells, got 7`) — the designed catch, zero
   casualty, repaired one leg later.
3. d1791648496-11 (the T278 flip): the edit appended the newline INSIDE
   the notes cell while the join re-added the trailing pipe → the row
   split across two physical lines; exit-0-masked, self-caught by the
   same-command verification grep, rejoined byte-exact, guard green
   pre-commit.

This is the T273 import-starvation shape one surface over: the recurring
edit re-authors its structural checks inline each flip (or skips them),
and the failures are all pre-commit, zero-casualty, and recurring.

## Repo context (read these first)

- `TODO.md` — the table: `| id | title | spec | pri | status | notes |`,
  six cells. Splitting a row line on `|` yields 8 fragments for 6 cells
  (leading + trailing empties). The T8 guard splits every row on `|` — a
  stray pipe in ANY cell splits one cell into two and fails the guard.
- `tests/todo_consistency.rs` — the T8 guard (21 tests); the flip's
  post-edit gate, run by the orchestrator per LOOP-SPEC step 5.
- `scripts/wrap_assert.py` — the carried helper library; this row's
  helper IMPORTS `assert_byte_carry(prev_lines, new_lines)` from it (the
  T273 move: import the probed-actuals carry assertion, never re-author).
- `scripts/test_wrap_assert.py` — the unittest conventions: every helper
  gets PASS + FAIL paths; each fail path asserts the AssertionError names
  the probed actuals (the T270 reporting requirement); run from repo root
  as `python3 -m unittest scripts.test_todo_flip -v`.
- `LOOP-SPEC.md` line ~721 — the flip instructions sentence ("the TODO
  row to `done` **with the merge commit ref in the same commit** ...").
  The substring "the TODO row to `done`" is PINNED by
  `tests/shared_target_dir.rs` (~line 642): the doctrine clause lands as
  a NEW sentence after the pinned sentence (or otherwise preserves the
  pinned needle byte-exact — the T107/T275 insert-without-breaking
  precedent), never by editing inside the pinned text.

## Requirements

1. `scripts/todo_flip.py` (new, stdlib-only), CLI:
   `python3 scripts/todo_flip.py TODO.md T279 --status done --notes "..."`
   — `--notes` optional (a status-only flip keeps the notes cell). The
   helper:
   (a) locates the target row by FIRST-cell id match (strip-compared);
   probed match count != 1 → AssertionError naming the id and the probed
   count (0 or N), file untouched;
   (b) parses the row into exactly 6 cells; probed cell count != 6 →
   AssertionError naming the probed count, file untouched;
   (c) rejects `--notes` containing `|` (the T8 split hazard) or `\n`
   (the single-physical-line hazard) pre-write, the AssertionError naming
   the offending character and its probed position, file untouched;
   (d) rebuilds the row FROM CELLS — `| ` + ` | `.join(cells) + ` |` —
   never string-splices around the trailing pipe;
   (e) asserts the rebuilt row is ONE physical line (no `\n`) — by
   construction after (c), asserted anyway;
   (f) asserts every OTHER line of the file is byte-identical pre/post
   via the IMPORTED `assert_byte_carry` (the import line
   `from wrap_assert import assert_byte_carry` appears exactly once);
   (g) writes the file only after every assert passes; any AssertionError
   leaves the file byte-identical (write-then-verify is the banned
   order). Exit 0 on success, non-zero with the probed actuals on
   failure.
2. `scripts/test_todo_flip.py` (new): unittest per the carried
   conventions — PASS + FAIL paths per assert above, including the TWO
   regression pins of THIS row's firing shapes: notes containing `|`
   (the T275 fire shape) and notes containing `\n` (the T278 fire shape)
   are both rejected pre-write with the file left byte-identical; the
   byte-carry PASS case flips one row of a 5-row fixture with the other
   four rows provably byte-identical; the id-count failures (0 and 2)
   name the probed counts.
3. `LOOP-SPEC.md` doctrine clause: ONE new sentence in the Phase-2 step-5
   flip instructions naming the helper — row flips go through
   `python3 scripts/todo_flip.py` (or re-derive the same structural
   asserts from probed properties — the T273 carve-out shape), never
   string-splice around the trailing pipe — placed to preserve the
   `shared_target_dir.rs` pinned needle byte-exact (new sentence after
   the pinned sentence, or the pin-preserving equivalent).
4. Diff touches ONLY `scripts/todo_flip.py` + `scripts/test_todo_flip.py`
   + `LOOP-SPEC.md`. No other files.

## Tests

- `python3 -m unittest scripts.test_todo_flip -v` green, zero collection
  errors; the two regression pins included.
- Review RED-proof: a helper with assert (c) removed (notes accepted
  unfiltered) turns EXACTLY the two regression pins red while the rest of
  the suite stays green; revert byte-clean.
- `cargo test --release --test todo_consistency --test internal_info_lint
  --test shared_target_dir --test loop_spec_recovery` green (the
  doctrine-pin surfaces covering the edited carriers).

## Acceptance

- Helper + suite + clause land; suite green; diff only the three named
  files; the pinned "the TODO row to `done`" needle still present
  byte-exact in LOOP-SPEC.md.
- T189 lane INELIGIBLE by the core-list leg (the LOOP-SPEC doctrine
  itself) — FULL adversarial validation REQUIRED (kimi), and the item
  runs SOLO (doctrine; no other child in flight).

## Estimate

- estimate: ~380 changed lines (helper ~140 + tests ~220 + doctrine
  clause ~3 + def-line cribs), three named files only.
