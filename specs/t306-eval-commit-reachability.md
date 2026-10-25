# T306 — wrap_assert validate_state: eval-commit probed-equality leg

## Repo context

`.chug/eval-state.md`'s `eval-commit:` field is the staleness marker the loopd
delta builder (`scripts/eval-delta.sh`) resolves against the repo at every
boot: an unreachable marker flips the read-path verdict to
`FULL-READ REQUIRED (marker-unreachable)` — fail-closed by design. The
write-time gate `scripts/wrap_assert.py`'s `validate_state(text, ...)` (the
T295 facade) checks the field NON-EMPTY only (`STATE_FIELD_KEYS`, nine
`state_field` extractions) — it never checks the field names a REAL commit.

Evidence (the filing fire, cycle-2035): the 2031 wrap's state FULL recompose
wrote `eval-commit: 3930452b1f9e0e05a12e93d69a2d6ba95d7cbe47` — the 7-char
prefix TRUE, the suffix memory-composed. The TRUE full hash is
`39304522865f99ca95b6e8b8750e1714e627e7fb` (`git cat-file -t` on the falsified
hash: rc-128 `could not get object info`; `git merge-base --is-ancestor`
rc-128). The falsification sat latent across three supervisor cheap-exit
dispositions and surfaced only at the next launched eval's boot delta read —
detection one chain late, zero casualty by design (a TRIP eval full-reads
regardless of the verdict; the state file is untracked). The T272 mis-fill
class's 73rd fire, its FIRST in the eval-commit surface; triage
d1792968241-2.

The facade already takes probed actuals as arguments (the T270/T273 shape:
`prev_ring_lines`, `corpus_tail_ids`) — this item adds the eval-commit
surface in the same shape: the CALL SITE probes, the facade ASSERTS equality.

estimate: ~70 changed lines (scripts/wrap_assert.py ~15 + scripts/test_wrap_assert.py ~45 + spec ~10)

## Requirements

1. `validate_state` gains an optional keyword (e.g. `expected_eval_commit=None`):
   when supplied, the gate asserts the parsed `eval-commit` field EQUALS the
   supplied probed full hash, raising AssertionError naming BOTH the field
   value and the probed actual on mismatch (the T270 reporting requirement —
   never a bare constant mismatch). When not supplied, the leg is inert
   (backwards-compatible with every existing call site and test).
2. The docstring names the intended call shape: the wrap's state-write helper
   probes `git rev-parse <commit>` for the hash the state is being written
   against and passes the probed value in — never composing the hash from
   memory. (The helper itself lives in the wrap corridor's shell, outside
   this repo's tracked code; the function contract is what lands here.)
3. RED-proven: the new FAIL-path test goes red without the leg (a
   fabricated-suffix eval-commit passes the facade today) and green with it.

## Tests

`scripts/test_wrap_assert.py` (python unittest, the existing home of
`ValidateStateTest`):
- PASS path: a state text whose eval-commit equals the supplied probed hash
  passes `validate_state`.
- FAIL path: a state text whose eval-commit is a fabricated-suffix variant
  (7-char prefix shared, tail altered) raises AssertionError naming both the
  field value and the probed actual.
- INERT path: `validate_state` without the new keyword behaves exactly as
  today (the existing `ValidateStateTest` legs stay green unmodified).

## Acceptance

`python3 scripts/test_wrap_assert.py` green (all pre-existing legs + the three
new ones); the RED proof demonstrated by stashing the leg (the new FAIL-path
test red without it, green with it). No change to `scripts/eval-delta.sh`'s
marker check (the boot-time fail-closed catch stays as-is — this item is the
write-time half).

check: python3 scripts/test_wrap_assert.py
