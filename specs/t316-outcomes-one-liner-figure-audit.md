# t316 — wrap_assert Outcomes one-liner figure-audit leg

One concern: the wrap's Outcomes-compaction one-liner figures are composed
from memory at wrap time, and the corridor-figure falsification class's
second COMMITTED sighting landed at the cycle-2947 wrap — the compacted
2919 one-liner duplicated the 2923 figures across 5 groups and rode the
tree FALSE for 4 cycles (2943→2947) before an ad-hoc audit caught it. The
class's own watch pre-authorized this row: a second committed sighting
weighs a guard row. This spec lands the mechanical compose-time check for
the one-liner surface: a new `assert_outcomes_one_liner` leg in
`scripts/wrap_assert.py` + RED-proof tests, callable from the fill's
pre-commit assert battery whenever a wrap one-lines a cycle.

## Repo context

- The class evidence (the falsification-watch bullet in `.chug/eval-state.md`):
  T305's origin (three committed corridor texts falsified at their own
  named stamps, cycle-1986 eval); the cycle-2826 first committed sighting
  (prep-ENTRY prose surface, corrected forward per T70); the cycle-2947
  wrap's SECOND and THIRD committed sightings — (1) the committed 2919
  one-liner (the 2943 wrap's compaction) duplicated the 2923 figures across
  5 groups [quiet-TRIP 43→44 FALSE (TRUE 42→43), net 530 FALSE (TRUE 529),
  the close ordinal+stream FALSE (wrote the 666th/2903 stream; TRUE the
  665th/2899 stream 80/1,182), the establishment ordinal+stream+census
  FALSE (wrote the 669th/2915 stream; TRUE the 668th/2911 stream 122/1,333
  census 38→39), the re-arm ordinal FALSE (wrote 598th; TRUE 597th)] —
  caught by the 2947 wrap's own ad-hoc one-liner audit and corrected
  forward per T70 in the same prep edit (c600bc5), the audit finding the
  defect isolated; (2) the T184 fresh-input mis-quote riding three
  committed texts (the 2943 eval body + the 2943 Outcomes entry + the
  2947 eval body), corrected forward per T70 in the 2947 Outcomes entry.
  The near-miss history (self-caught compose slips, designed refusals) is
  enumerated in the state file's watch bullet and is NOT re-litigated here.
- The module shape: `scripts/wrap_assert.py` is dependency-free /
  stdlib-only (the T312 marker leg reaches subprocess through the builtin
  `__import__` to keep the AST import-free shape the stdlib-only leg pins —
  the new leg needs NO subprocess, pure text extraction + comparison).
  Exports ride the module's `__all__` (16 entries today). The module
  docstring's coverage sentence ("Coverage (run by the check line):
  python3 -m unittest scripts/test_wrap_assert.py -v — the pre-T295 legs
  plus the validate_state facade legs, the T309 fill-datum legs, and the
  T310 fill-enumeration legs, the T311 on-disk state-schema legs, and the
  T312 always-on marker legs.") is the pin-surface this spec reconciles —
  it is EXTENDED, not re-shaped (append ", and the T316 Outcomes
  one-liner figure-audit legs" in the same sentence).
- The one-liner's figure groups (the compaction template's regular shape,
  from the committed one-liners): each `### Cycle N (date) —` one-line
  entry carries (1) the quiet-TRIP/zero-row count pair (`zero-row A/B →
  C/D` or `quiet-TRIP X→Y`), (2) the second-net figure (`net →K`), (3)
  the backstop close group (`the <ord> close [stream id + figures]`),
  (4) the establishment group (`the <ord> establishment [stream id +
  figures + census move]`), (5) the re-arm group (`re-arm <ord>`). The
  leg compares the SAME five groups between the committed FULL entry (the
  cycle's last full `### Cycle N … —` section before compaction) and the
  one-liner replacing it.
- The T285 pin-surface probe: `grep -rn 'matches("' tests/*.rs` — no Rust
  pin names wrap_assert's `__all__` or the docstring coverage sentence
  (the only wrap_assert mentions in tests/ are comments:
  tests/loopd_daemon_ensure.rs:204, tests/loopd_spoof_guard.rs:131,
  tests/loopd_stale_binary.rs:146, tests/stream_datum.rs:135). The
  python battery is unittest (`python3 -m unittest discover -s scripts
  -p 'test_*.py'` — 147 tests at filing). No exact-count census pins the
  test total; the docstring sentence above is the only text pin perturbed.
- Scoping decision (recorded at filing): the eval-body T184 mis-quote
  subclass is NOT covered by this leg — that surface is backstopped by
  the state file's own `fresh-input-last-eval:` line (the catch that
  fired at the 2947 wrap prep) and stays prose-guarded per T282/T305.
  This leg covers the Outcomes one-liner surface only.

## Requirements

1. New leg `assert_outcomes_one_liner(full_text, one_liner_text)` in
   `scripts/wrap_assert.py`: extract the five figure groups named in
   repo-context from BOTH texts; assert equality group-by-group; on any
   mismatch raise AssertionError naming the group, the full-entry value,
   and the one-liner value (the module's established refusal style —
   `_fire_*`-style named failure, never a bare traceback). A group that
   cannot be extracted from EITHER text raises AssertionError naming the
   group and the extraction failure (fail-closed — a silent skip would
   re-create the vacuous-pass class).
2. Export the leg in the module's `__all__` (16 → 17) and extend the
   docstring coverage sentence per repo-context.
3. Tests in `scripts/test_wrap_assert.py`: (a) GREEN — a fixture full
   entry + its matching one-liner passes; (b) RED-proofs — one perturbed
   fixture per group (at least 3 of the 5 groups) must raise
   AssertionError naming the perturbed group; (c) a malformed-input leg —
   a one-liner missing a group raises the named extraction failure, never
   a bare exception.
4. The leg is stdlib-only, pure-python text work (no subprocess, no file
   IO — both texts are passed in, per the module's read_key_values
   TEXT-not-path convention).

## Tests

`python3 -m unittest discover -s scripts -p 'test_*.py'` from the worktree
root — all green (147 + the new tests).

## Acceptance

- The leg exists, is exported, and the docstring coverage sentence names
  the T316 legs.
- The python battery is green with the new tests; each RED-proof fixture
  kills (AssertionError naming the perturbed group).
- No other repo file changes.

check: python3 -m unittest discover -s scripts -p 'test_*.py'

estimate: ~140 changed lines (helper ~70 + tests ~70)
