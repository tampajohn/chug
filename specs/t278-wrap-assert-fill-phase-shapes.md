check: cd "$(git rev-parse --show-toplevel)" && export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-validate-a && touch src/*.rs tests/*.rs && python3 -m unittest scripts/test_wrap_assert.py -v && cargo test --release --test todo_consistency --test internal_info_lint

# T278 — wrap_assert fill-phase shapes: window-scoped substitution count + bare-token scan

**One concern:** give the fill phase's two recurring verification shapes an
import target in `scripts/wrap_assert.py` — a window-scoped placeholder
count assertion and a window-scoped bare-token scan — so fill helpers stop
re-authoring file-wide count asserts inline (the T273 banned shape) and
misfiring on prose that legitimately quotes the token names.

## Why

The T273 clause (META-META-SPEC: import-don't-re-author) already covers
wrap/fill-phase verification helpers, but the helper has no WINDOW-SCOPED
shape, so the fill phase keeps hand-rolling file-wide counts. Two post-T273
fill-phase wrong-expectation fires, both self-caught pre-write with zero
casualty:

1. trip-84 fill, 13:59:17Z (cycle-466 stream, census 12→13 at the trip-85
   eval, d1791643063-3) — a substitution needle embedding a CONFABULATED
   time constant (`'~07:01–WRAP_TIME UTC'`, found 0, expected 1) against a
   structurally correct artifact.
2. trip-85 fill, 14:51:37Z + 14:51:43Z (cycle-470 stream, census 13→14 at
   the trip-86 eval) — `AssertionError: 'WRAP_HASH': found 2, expected 1`
   twice, then two more assert legs while debugging: a hand-rolled heredoc
   asserted a FILE-WIDE count == 1 against the state file's TWO legitimate
   WRAP_HASH mentions — the pacing-line placeholder (the fill target) and
   the `open-threads` bullet's PROSE QUOTE of the token name. The standing
   lesson ("the carried entries' fill-instruction prose quotes the tokens
   by name") was applied to the EVALUATION.md entry window but not to the
   state-file substitution; the 14:51:46Z grep probe (found 2: lines 9 and
   34) was the self-correction, and the fill landed correctly at 49a95e5.

The trip-85 eval's re-arm fired verbatim: a SECOND post-T273 fill-phase
wrong-expectation fire in a hand-rolled helper weighs the fill-phase import
sweep. This row is the weighed remedy — the same import-starvation move
T273 made for the wrap phase, now for the fill phase's two shapes.

## Repo context (read these first)

- `scripts/wrap_assert.py` — six carried helpers:
  `read_key_values(text)`, `assert_fields_non_empty(source, keys)`
  (path-or-mapping per T277), `assert_count_delta(before, after, added)`,
  `assert_byte_carry(prev_lines, new_lines)`,
  `assert_substituted(path, needle, expected_present=True)`,
  `assert_count_exact(actual, expected, label="count")`. Each def line
  carries a one-line signature crib comment (the T277 convention).
- `scripts/test_wrap_assert.py` — the carried unittest suite; every
  helper gets a PASS path and a FAIL path, and each fail path asserts the
  AssertionError names the probed actuals (the T270 reporting
  requirement).
- The firing surface: the wrap's fill heredocs substitute the placeholders
  (WRAP_TIME / WRAP_HASH / PROBE_RESULT / COLOR_STREAM / GATES_COLOR /
  AUDIT_COLOR) into (a) the new Outcomes entry in EVALUATION.md — window =
  the `### Cycle N` section through the next `### ` heading — and (b) the
  `pacing-streak:` line in `.chug/eval-state.md` — window = that single
  line. Token names also appear as prose quotes OUTSIDE those windows
  (carried entries' fill instructions, the state's open-threads bullets),
  which is exactly why file-wide counts misfire.
- The class boundary: AssertionError wrong-expectation is the
  python-assertion-slip census (14) — this row changes no census; the
  T277 TypeError call-shape class (3, discharged) is untouched.

## Requirements

1. `assert_window_count(path, placeholder, start, expected, end=None)`:
   read the file at `path`; compute the window = from the FIRST line
   containing the literal substring `start` (inclusive) through the first
   SUBSEQUENT line containing the literal substring `end` (exclusive), or
   EOF when `end is None`; assert the placeholder's occurrence count
   within the window == `expected`. Failures name the probed actual
   count, the expected, the placeholder, the anchor, and the window's
   line span. A window that matches NO line raises AssertionError naming
   the missing anchor (a genuinely-wrong input — the guard firing
   correctly, not a wrong-expectation miscalibration).
2. `assert_window_clean(path, tokens, start, end=None)`: same window
   mechanics; assert NONE of `tokens` appears within the window (the
   post-fill bare-placeholder scan). The failure names EVERY found token
   with its window-relative line number in one AssertionError (the
   all-failures-in-one-error convention).
3. Both def lines carry one-line signature crib comments per the T277
   convention, naming arg shapes with a minimal example call.
4. Diff touches ONLY `scripts/wrap_assert.py` +
   `scripts/test_wrap_assert.py`. No caller edits (the fill heredocs
   compose fresh each wrap; the helpers are purely additive).

## Tests

- `python3 -m unittest scripts/test_wrap_assert.py -v` green (the carried
  suite + the new cases), zero collection errors.
- New cases, pass + fail paths for each helper, including the regression
  pin of THIS row's firing shape: a file holding a placeholder ONCE in
  the `pacing-streak:` line and ONCE in a later prose bullet PASSES
  `assert_window_count(path, "WRAP_HASH", "pacing-streak:", 1)` while a
  file-wide count would read 2; the anchor-miss fail names the anchor;
  the clean-scan pass (tokens only outside the window) and fail (a bare
  token in-window named with its line number).
- Review RED-proof: reverting `assert_window_count`'s window scoping to a
  file-wide count turns ONLY the new regression test red while the
  carried suite stays green; revert byte-clean.
- `cargo test --release --test todo_consistency --test internal_info_lint`
  green (the scripts tree carries doctrine-adjacent lint surfaces).

## Acceptance

- Both helpers land with cribs; suite green; diff only the two named
  files.
- T189 low-stakes lane EXPECTED (no core-list file, ≤ ~150 lines, no
  new tool/command surface, no check:-line change in tracked workflows)
  — the lane's four inputs are computed from the actual diff at review;
  the orchestrator RED-proof above runs regardless of lane.

## Estimate

- estimate: ~120 changed lines (two helpers ~45 LOC + docstrings/cribs
  ~15 + ~7 test methods ~60 LOC), scripts + tests only
  (`scripts/wrap_assert.py` + `scripts/test_wrap_assert.py`).
