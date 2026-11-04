# t318 — T316 survivor fixture pair (m1 ordinal-adjacency + m6 zero-row-pair)

One concern: the T316 kimi validator (d1793798848-4, 6 mutants, 4 killed)
recorded TWO low-impact SURVIVOR findings against the
`assert_outcomes_one_liner` test surface — both weak-test gaps, both
verified LOW impact (the shipped behavior was verified correct via direct
adversarial probes) — and recommended "a follow-up fixture pair — a
zero-row-pair perturbation and an ordinal-dropped group slice — to pin
both sub-rules." This spec lands exactly that fixture pair, closing the
validator's recommendation with RED-proven killing tests.

## Repo context

- The validator's findings (verbatim from `.chug/verdict-t316-validate.md`,
  the 6-mutant run on the T316 branch):
  - **m1 (ordinal-adjacency-dropped) survived 16/16** — no committed
    fixture exercises the ordinal-adjacency contract; on the real corpus
    the mutant behaves identically to shipped, and on a synthetic
    ordinal-dropped close slice it still fails-closed (no stale ordinal
    in reach) — a false pass needs a stale ordinal within the window
    EQUAL to the true own ordinal (pathological, and any unequal stale
    read still diverges and fires).
  - **m6 (zero-row second-fraction dropped) survived 16/16** — group 1's
    RED fixtures perturb only the quiet-TRIP sub-figure, never the
    zero-row `A/B → C/D` pair; the validator verified the SHIPPED code
    FIRES on a C/D-only falsification (AssertionError naming
    'quiet-TRIP/zero-row pair'), so the gap is suite coverage, not
    behavior.
- The surface: `assert_outcomes_one_liner(full_text, one_liner_text)` in
  `scripts/wrap_assert.py` (the T316 leg) with its tests in
  `scripts/test_wrap_assert.py`. The five figure groups the leg compares
  (per the T316 spec): (1) the quiet-TRIP/zero-row count pair, (2) the
  second-net figure, (3) the backstop close group [ordinal + stream id +
  figures], (4) the establishment group [ordinal + stream id + figures +
  census move], (5) the re-arm group [ordinal].
- The sweep-the-family note (LOOP-SPEC step 4): this spec is the
  validator's own named family — both sub-rules pinned in ONE row, one
  RED-proven fixture each, exactly the validator's recommended pair.

## Requirements

1. New RED fixture for the m6 gap in `scripts/test_wrap_assert.py`: a
   full-entry fixture and its one-liner that agree on the quiet-TRIP
   sub-figure but DISAGREE on the zero-row `A/B → C/D` pair (the
   one-liner perturbs the zero-row second fraction only) — the leg MUST
   raise AssertionError naming the quiet-TRIP/zero-row pair group. The
   fixture is RED-proven: run it against the m6 mutant shape (the
   comparison with the zero-row second fraction dropped) and show it
   kills — i.e., the fixture FAILS (no fire) under the mutant and FIRES
   under shipped code; the test file documents the mutant shape in the
   fixture's comment.
2. New RED fixture for the m1 gap: a full-entry fixture whose close
   group carries an ordinal-adjacency VIOLATION the shipped leg rejects
   — the one-liner's close ordinal is a stale-but-in-window value
   UNEQUAL to the true own ordinal (the validator's divergence shape) —
   the leg MUST raise AssertionError naming the close group with both
   ordinals. RED-proven the same way: under the m1 mutant shape (the
   adjacency check dropped), the fixture does NOT fire; under shipped
   code it FIRES. (The validator's residual pathological case — a stale
   ordinal EQUAL to the true own ordinal — stays out of scope by the
   validator's own impact assessment.)
3. Both fixtures follow the T316 test file's established style (fixture
   texts as module-level constants or inline triple-quoted strings,
   `assertRaises(AssertionError)` with the group-name fragment matched
   in the message).
4. No changes to `scripts/wrap_assert.py` — the validator verified the
   shipped behavior correct; this row pins the two sub-rules with tests
   only. If a fixture surprises (fires on shipped code where the
   validator verified correct, or fails to fire), STOP and report — the
   premise is the validator's documented behavior verification.

## Tests

`python3 -m unittest discover -s scripts -p 'test_*.py'` from the worktree
root — all green (the current battery + the 2 new fixtures).

## Acceptance

- The two fixtures exist in `scripts/test_wrap_assert.py`, each
  RED-proven against its named mutant shape (m6 / m1) per the
  requirements, each raising AssertionError naming the correct group
  under shipped code.
- The python battery is green with the new tests.
- No other repo file changes (tests-only row).

check: python3 -m unittest discover -s scripts -p 'test_*.py'

estimate: ~60 changed lines (tests-only)
