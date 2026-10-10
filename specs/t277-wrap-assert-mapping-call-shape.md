check: cd "$(git rev-parse --show-toplevel)" && export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared && touch src/*.rs tests/*.rs && python3 -m unittest scripts/test_wrap_assert.py -v && cargo test --release --test todo_consistency --test internal_info_lint

# T277 — wrap_assert call-shape ergonomics: mapping-accepting assert_fields_non_empty + signature cribs

**One concern:** root-fix the recurring wrap_assert call-signature slip —
`assert_fields_non_empty` accepts an already-parsed mapping as well as a
path, and every helper's def line carries a one-line signature crib — so
the call site that "feels right" stops raising TypeError.

## Why

The T273 helper's call shape has slipped THREE times in two cycles, every
time self-caught pre-write with zero casualty (the T270/T266 guards held):

1. trip-81 wrap state rewrite, 10:31:18Z —
   `assert_fields_non_empty` TypeError `expected str … not dict` at line
   74 (the caller passed the `read_key_values` result where the helper
   re-opens a path);
2. trip-81 wrap state rewrite, 10:31:22Z —
   `assert_count_delta() takes 3 positional arguments but 4 were given`
   (the caller passed the path first by analogy with the other helpers);
3. trip-83 eval corpus assembly (this filing's own probe leg) —
   `assert_fields_non_empty` TypeError `expected str … not dict` at line
   74 again: the natural composition
   `assert_fields_non_empty(read_key_values(path), keys)` feels right and
   is wrong on TWO counts (`read_key_values` takes the TEXT, not the
   path; `assert_fields_non_empty` re-opens the path itself).

The cycle-458 eval's first-sighting record (d1791630092-3, census at 2,
one authoring episode) armed the trigger: **a third instance files the
counted census and weighs a signature-ergonomics note**. The third
instance fired at the trip-83 eval's own ingress probe (same
expected-str-at-line-74 shape, self-corrected one leg later by passing
the path, zero casualty) — the census files at 3 and this row is the
weighed remedy. The remedy goes to the root rather than documenting
around the trap: the helper accepts the mapping callers keep handing it,
and every def line names its arg shapes where the caller reads them.

## Repo context (read these first)

- `scripts/wrap_assert.py` — six helpers:
  `read_key_values(text)`, `assert_fields_non_empty(path, keys)`,
  `assert_count_delta(before, after, added)`,
  `assert_byte_carry(prev_lines, new_lines)`,
  `assert_substituted(path, needle, expected_present=True)`,
  `assert_count_exact(actual, expected, label="count")`.
- `scripts/test_wrap_assert.py` — 17 tests at filing (the T274 reorder
  pin included); the file's convention is `unittest` classes with
  probed-actual failure messages.
- The class boundary: TypeError ≠ AssertionError — the
  python-assertion-slip census (12) is untouched by this row; this is
  the ADJACENT call-signature sub-shape, counted at 3 by this filing.

## Requirements

1. `assert_fields_non_empty(source, keys)` accepts EITHER a path
   (`str`/`os.PathLike` — the carried behavior: open + read_key_values)
   OR an already-parsed mapping (`collections.abc.Mapping` — used
   directly, no re-read). Dispatch on isinstance; any other type raises
   TypeError whose message names BOTH accepted shapes. The assertion
   behavior (missing-key and empty-value failures, the all-failures-in-
   one-error naming with probed actuals) is unchanged in both modes.
2. Every helper's def line gains a one-line signature crib comment
   (immediately above or on the def line, matching the file's comment
   style) naming the arg shapes with a minimal example call — e.g.
   `# read_key_values(text) — text, NOT a path` and
   `# assert_count_delta(before, after, added) — 3 probed values, no path`.
   The cribs are comments; no behavior changes beyond req 1.
3. Tests in `scripts/test_wrap_assert.py`: mapping-input pass case;
   mapping-input fail case (the missing key named in the error);
   a wrong-type argument (e.g. an int) raising TypeError naming the
   accepted shapes; the carried path-input cases stay green untouched.
4. Diff touches ONLY `scripts/wrap_assert.py` +
   `scripts/test_wrap_assert.py`. No caller edits (the ad-hoc wrap
   heredocs compose the helpers fresh each wrap; both input shapes stay
   valid, zero breakage).

## Tests

- `python3 -m unittest scripts/test_wrap_assert.py -v` green (the
  carried 17 + the new cases), zero collection errors.
- Review RED-proof: reverting the isinstance dispatch (path-only
  `assert_fields_non_empty`) turns ONLY the new mapping tests red while
  the carried suite stays green; revert byte-clean.
- `cargo test --release --test todo_consistency --test internal_info_lint`
  green (the scripts tree carries doctrine-adjacent lint surfaces).

## Acceptance

- Mapping and path inputs both pass the same assertion semantics;
  TypeError message names both accepted shapes; cribs on all six def
  lines; suite green.
- Diff touches only the two named files, ≤ ~35 changed lines.
- T189 low-stakes lane EXPECTED (no core-list file, ≤ ~150 lines, no
  new tool/command surface, no check:-line change in tracked workflows)
  — the lane's four inputs are computed from the actual diff at review;
  the orchestrator RED-proof above runs regardless of lane.

## Estimate

- estimate: ~35 changed lines (the isinstance branch + TypeError +
  six crib comments + ~3 test methods), scripts + tests only
  (`scripts/wrap_assert.py` + `scripts/test_wrap_assert.py`).
