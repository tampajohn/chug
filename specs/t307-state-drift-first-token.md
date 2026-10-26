# T307 — wrap_assert validate_state: state-drift first-token `none` leg

## Repo context

`.chug/eval-state.md`'s `state-drift:` field is the loopd delta builder's
human-flag surface: `scripts/eval-delta.sh:214` reads the field's FIRST
whitespace-token and flags `FULL-READ REQUIRED (state-drift-flagged)` when it
is not exactly `none` — the builder's own comment names the schema contract
(`none — first write (bootstrap; …)`, `none (the boot STATE-HIT: …)`: the
flag token first, the prose AFTER it). The write-time gate
`scripts/wrap_assert.py`'s `validate_state(text, ...)` (the T295 facade)
checks the field NON-EMPTY only (`STATE_FIELD_KEYS`, nine `state_field`
extractions) — it never checks the flag token occupies the flag position.

Evidence (the filing fire, cycle-2039): the 2035 wrap's state FULL recompose
healed the eval-commit field (the T306 arc, probed hash + the
expected_eval_commit dogfood PASSED) but composed the drift field with the
healed event's NARRATIVE in the flag position —
`state-drift: FULL-READ REQUIRED (marker-unreachable) at the boot - ...`.
The 2039 boot's delta build re-flagged `FULL-READ REQUIRED
(state-drift-flagged)` with the marker HEALED and reachable (`git cat-file
-t` commit; `git merge-base --is-ancestor` ANCESTOR-OK — both probed at the
476th eval). Fail-SAFE by design: a TRIP eval full-reads regardless of the
verdict, zero routing consequence, T260's metric HOLDS (no STATE-HIT
advance). The structural hazard if unpinned: EVERY genuine drift event
permanently latches the slim path off — each wrap's narrative re-flags the
next boot, so T260's re-arm path defeats itself silently after the first
drift. The T304 arc (2f01494) made the VERDICT side prose-tolerant
(first-token compare); this item is the WRITE-side half: the facade refuses
a drift field whose first token is not `none` AT write time. Triage
d1792981452-2.

The facade already takes probed actuals as arguments (the T270/T273/T306
shape) — this leg needs NO new argument: the contract is intrinsic to the
field (first whitespace-token IS `none`).

estimate: ~50 changed lines (scripts/wrap_assert.py ~12 + scripts/test_wrap_assert.py ~30 + spec ~8)

## Requirements

1. `validate_state` gains an intrinsic leg (no new keyword): the parsed
   `state-drift` field's first whitespace-token MUST equal `none`, raising
   AssertionError naming BOTH the received first token and the full field
   value on mismatch (the T270 reporting requirement — never a bare
   constant). The prose-after-`none` shape stays legal (the T304
   first-token semantics, mirrored write-side).
2. The docstring names the contract: a healed/adjudicated drift event writes
   `state-drift: none` with the narrative AFTER the token — never the
   narrative in the flag position (the 2035 wrap's composition miss is the
   named counter-example).
3. RED-proven: the new FAIL-path test goes red without the leg (a
   narrative-first drift field passes the facade today) and green with it.

## Tests

`scripts/test_wrap_assert.py` (python unittest, the existing home of
`ValidateStateTest`):
- PASS path: `state-drift: none (the boot STATE-HIT: …)` prose-after-token
  shape passes `validate_state`.
- FAIL path: `state-drift: FULL-READ REQUIRED (marker-unreachable) at the
  boot - …` (narrative in the flag position) raises AssertionError naming
  the received first token and the field value.
- The existing `ValidateStateTest` legs stay green unmodified (their fixture
  fields are already `none`-first — verify, never assume: if any fixture
  carries a narrative-first field it is corrected to the schema shape and
  the correction named in the commit message).

## Acceptance

`python3 scripts/test_wrap_assert.py` green (all pre-existing legs + the new
ones); the RED proof demonstrated by stashing the leg (the new FAIL-path
test red without it, green with it). No change to `scripts/eval-delta.sh`'s
first-token compare (the boot-time consumer stays as-is — this item is the
write-time half).

check: python3 scripts/test_wrap_assert.py
