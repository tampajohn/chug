check: cd "$(git rev-parse --show-toplevel)" && export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared && touch src/*.rs tests/*.rs && python3 -m unittest scripts/test_wrap_assert.py -v && cargo test --release --test todo_consistency

# T274 — assert_byte_carry reorder fail-case pin

**One concern:** add ONE reorder fail-case test method to the
`AssertByteCarryTest` class in `scripts/test_wrap_assert.py`, pinning the
ordered-search semantics of `assert_byte_carry` in `scripts/wrap_assert.py`.

## Why

The T273 kimi validation verdict (d1791624468-8, finding 1) exercised 6
mutants against the wrap_assert helper; mutant m6 — relaxing the ordered
carry search to unordered membership — SURVIVED the suite, and the verdict
named the remedy: "a one-line fail-case kills it — survivor-pin candidate
for the next eval". The trip-81 eval verified the indictment before filing
(the verify-the-indictment doctrine): the class carries 4 cases —
`test_pass_byte_identical_carry_at_any_offset`,
`test_pass_identical_lines_pass`,
`test_fail_names_the_mutated_carry_line_and_both_probed_reprs`,
`test_fail_names_a_dropped_carry_line` — pass-at-offset, pass-identical,
fail-mutated, fail-dropped; NO case exercises reorder. Under the current ordered
implementation, carrying `["a","b"]` as `["b","a"]` raises at the first
carry line today, so the pin is non-vacuous AND kills any mutant that
relaxes the ordered search to unordered membership (m6's exact shape).

## Requirements

- ONE new test method in `AssertByteCarryTest` (name it
  `test_fail_reordered_lines` or close): construct a carried block whose
  lines are ALL PRESENT in the new section but in a DIFFERENT order (the
  minimal case: two lines swapped), call the assertion helper the same way
  the existing fail-cases do, and assert it raises (the same assertion
  style the neighboring fail-cases use — `assertRaises` or the probed
  helper per T273's shape; match the file's own convention).
- Tests-only: the diff touches ONLY `scripts/test_wrap_assert.py`. No
  change to `scripts/wrap_assert.py` — the implementation already raises
  on reorder; this pins that behavior.
- Follow the existing test bodies' construction style (list-of-lines
  fixtures, the same helper invocation the other AssertByteCarryTest
  cases use).

## Tests

- The new test FAILS when `scripts/wrap_assert.py`'s ordered carry search
  is mutated to unordered membership (the review's RED-proof: a
  perl -i mutant relaxing the ordered search — e.g. replacing the
  positional scan with a membership check — must turn the new test RED
  while the 4 carried cases stay green).
- The new test PASSES against the unmodified implementation (the
  non-vacuity leg: the current code already raises on reorder, verified
  at filing).

## Acceptance

- `python3 -m unittest scripts/test_wrap_assert.py -v` green (17 tests:
  the carried 16 + the new case), zero collection errors.
- Reviewer RED-proof: unordered-membership mutant of
  `scripts/wrap_assert.py` fails the new test; revert byte-clean.
- Diff touches only `scripts/test_wrap_assert.py`.

## Estimate

- estimate: ~12 changed lines (one test method with a short docstring),
  tests-only (`scripts/test_wrap_assert.py` only).
