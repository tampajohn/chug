# T312 — the on-disk state-marker leg goes always-on + self-probing (the T306 composed-marker class's second fire)

## Repo context

`.chug/eval-state.md`'s `eval-commit:` marker is the delta window's
anchor: loopd's `scripts/eval-delta.sh` reads it at every boot and fails
CLOSED (`FULL-READ REQUIRED: marker-unreachable`) when the field is not
a commit in the repo. The write-time guard already exists but is OPT-IN:
`scripts/wrap_assert.py`'s `validate_state(text, ...,
expected_eval_commit=None)` (the T306 leg, filed after the class's first
fire at the cycle-2035 wrap) asserts the parsed field EQUALS a
caller-probed full hash — "without the keyword the leg is inert" — and
`assert_state_on_disk` (the T311 leg) merely forwards the keyword "when
supplied".

Evidence (the second fire, the cycle-2704 wrap): the wrap's state FULL
recompose wrote `eval-commit: 2f905d7e8c5b75eb1c2d4b2a3f0c5d6e7a8b9c0d`
— the 7-char prefix TRUE, the remaining 33 hex chars memory-composed
(the TRUE full hash of the 2704 eval commit is
`2f905d7f8cfcafbeff2894be019b2488b4cefc02`, re-probed via
`git rev-parse 2f905d7` at the cycle-2708 eval). Every write-side guard
passed it: the fill (0a8bf5b) claims "state FULL recompose + T311
assert_state_on_disk GREEN first attempt" while that run supplied NO
`expected_eval_commit` — the T306 leg inert by contract — so the run
validated schema-shape only. The consumer caught it one boot later,
fail-closed by design: the 2708 boot's delta build reported
`FULL-READ REQUIRED: marker-unreachable (2f905d7e8c… is not a commit in
this repo)`. Casualty: ZERO (the boot was a trip eval — always a full
read — and the slim path simply never engaged), the exact zero-casualty
shape of the class's first fire — proving the opt-in remedy does not
hold: the corridor's own call site left the leg off.

estimate: ~85 changed lines (scripts/wrap_assert.py ~30 + scripts/test_wrap_assert.py ~45 + spec ~10)

## Requirements

1. `scripts/wrap_assert.py`'s `assert_state_on_disk` gains an ALWAYS-ON
   marker leg at the on-disk surface: after the existing facade run, the
   function SELF-PROBES the expected marker —
   `git log -1 --format=%H -- EVALUATION.md` via subprocess in the repo
   cwd (the T311 documented probe convention, now OWNED by the function
   so no call site can leave it off) — and asserts the parsed
   `eval-commit:` field EQUALS the probed full hash, naming BOTH values
   on mismatch (the T270 reporting requirement). The existing
   `expected_eval_commit` keyword becomes an OVERRIDE (asserted against
   instead of the self-probe when supplied), never the leg's enabler —
   the leg runs by default.
2. A `probe_marker` injectable (a callable returning the 40-char hash,
   defaulting to the subprocess probe) keeps the leg testable; a probe
   FAILURE raises AssertionError naming the probe and the OS error,
   never a bare traceback.
3. The module docstring names the class's second fire: the 2704 wrap's
   fabricated-suffix marker (`2f905d7e8c…`, TRUE
   `2f905d7f8cfcafbeff2894be019b2488b4cefc02`) green-lit by the T311
   run with the opt-in leg inert — the on-disk surface now OWNS the
   probe so the leg cannot be left inert.
4. RED-proven: the new FAIL-path tests go red without the leg (a
   fabricated-suffix fixture passes today's call surface with
   `expected_eval_commit` omitted) and green with it.

## Tests

`scripts/test_wrap_assert.py` (python unittest, the existing home):
- PASS path: the schema-conformant fixture whose marker equals the
  test's OWN probe of `git log -1 --format=%H -- EVALUATION.md` (probed
  dynamically at test time, never hardcoded) passes
  `assert_state_on_disk` with no keyword.
- PASS path (override): the same fixture passes with
  `expected_eval_commit=<the same probed value>`.
- FAIL path (the 2704 shape): the fixture's marker replaced by its own
  7-char prefix + 33 invented hex chars raises AssertionError naming
  BOTH the field value and the probed actual.
- FAIL path (semantic drift): the fixture's marker replaced by a
  DIFFERENT reachable commit (any 40-hex != the probe) raises
  AssertionError — reachability alone is not the contract.
- FAIL path (probe failure): an injected `probe_marker` raising OSError
  surfaces AssertionError naming the probe, not a bare traceback.
- The existing battery stays green unmodified.

## Acceptance

`python3 scripts/test_wrap_assert.py` green (all pre-existing legs + the
new ones); the RED proof demonstrated by stashing the leg; the full rust
suite green.

check: python3 scripts/test_wrap_assert.py && cargo test
