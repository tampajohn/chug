# T311 — the fill pre-commit assert gains the on-disk state-schema leg (the 2515 false-GREEN class's durable gate)

## Repo context

`.chug/eval-state.md` is schema-pinned (schema v1 — META-META-SPEC): the
loopd delta builder (`scripts/eval-delta.sh`) is the schema's enforcer and
fails CLOSED (`FULL-READ REQUIRED: schema-mismatch`) when any of the nine
pinned `state_field` extractions is absent. The write-time gate
`scripts/wrap_assert.py`'s `validate_state(text, ...)` (the T295 facade)
already refuses a non-conformant text at its unconditional FIELDS leg.

Evidence (the filing fire, cycle-2515 wrap): the wrap's state recompose
wrote the state in a free-form prose format — ZERO of the nine pinned
fields (`last-eval-commit:` substituted for `eval-commit:`, no `schema:`,
no `eval-at:`, no `state-drift:`, no telemetry fields, no `health:`, no
`open-threads:`, no `pacing-streak:`) and none of the doctrine's pinned
sections (only the non-schema `## Delta pointer` / `## Outcome-id ring`).
Every loopd build since reports `FULL-READ REQUIRED: schema-mismatch
(found: 'absent', want: 1)` (22:59:03Z / 23:02:59Z / 23:08:56Z /
23:18:50Z, the 706th-709th builds) — the enforcer worked. But the 2515
fill (a66f604) claims "state write FACADE-GREEN first attempt [the 46th
exercise]" — RED-proven at the 597th eval: `validate_state` on the
committed bytes raises AssertionError at the FIELDS leg. The facade
CANNOT have greened the committed text; the claim was composed without a
real run over the on-disk bytes (or over a draft a later leg overwrote).
The corridor has NO post-write mechanical check that reads the on-disk
state through its actual consumer, so a non-conformant write AND a false
GREEN claim both pass silently. Casualty to date: ZERO (the three builds
fed cheap-exit cycles whose predicate never consults the verdict; the
fourth fed a TRIP eval, which full-reads regardless) — the next non-trip
eval pays the full-corpus read T260 exists to avoid.

The T309/T310 pattern is the fix template: the fill compose leg's
pre-commit assert re-runs mechanical legs in the SAME leg as the commit
(`assert_fill_datum` re-runs `scripts/stream_datum.py` at the fill stamp;
`assert_fill_enumeration` re-enumerates the stream's `ok:false` legs) and
verifies the fill text against them — compose-time catches independent of
the writer's claim. This item adds the family's third leg: the ON-DISK
state file, validated through the EXISTING facade, at fill time.

estimate: ~120 changed lines (scripts/wrap_assert.py ~45 + scripts/test_wrap_assert.py ~60 + LOOP-SPEC doctrine sentence ~10 + spec ~8)

## Requirements

1. `scripts/wrap_assert.py` gains
   `assert_state_on_disk(path, expected_eval_commit=None)`: reads the
   state file's BYTES from `path` (a str — TypeError naming the accepted
   shape otherwise, the module's call-signature-slip convention) and runs
   the FULL existing `validate_state` gate on the read text
   (`expected_eval_commit` forwarded when supplied — probed by the caller
   via `git log -1 --format=%H -- EVALUATION.md`, never composed from
   memory; the splice/corpus-tail legs stay inert — the on-disk state
   stands alone). Failures propagate from the facade (each naming its
   PROBED actuals — the module's invariant); a missing/unreadable file
   raises AssertionError naming the probed path and the OS error (never a
   bare traceback).
2. The docstring names the class it closes: the cycle-2515 fill's
   "FACADE-GREEN first attempt" claim RED-proven against the committed
   bytes — a GREEN claim is never composed without the mechanical run
   over the on-disk file; this leg IS that run, wired into the fill
   compose leg's pre-commit assert (the same leg as the commit, per the
   T309/T310 pattern).
3. LOOP-SPEC.md Phase 3's wrap doctrine (the block naming
   `assert_fill_datum` / `assert_fill_enumeration`) names the third leg:
   after the wrap's state write, the fill compose leg's pre-commit
   assert runs `scripts/wrap_assert.py`'s on-disk state-schema leg
   against the committed `.chug/eval-state.md`; a refusal BLOCKS the
   fill commit — the state is rewritten as the full single write (the
   cycle-309 precedent) and re-gated before the fill push.
4. RED-proven: the new FAIL-path test goes red without the leg (a
   2515-shape prose state passes today's call surface — no such leg
   exists) and green with it.

## Tests

`scripts/test_wrap_assert.py` (python unittest, the existing home):
- PASS path: a schema-conformant fixture (all nine fields, `state-drift:
  none …`, the pinned sections, a 12-cap ring) written to a temp file
  passes `assert_state_on_disk` (with and without
  `expected_eval_commit`).
- FAIL path (the 2515 shape): a prose-format fixture (`last-eval-commit:`
  substituted, zero schema fields, `## Delta pointer` / `## Outcome-id
  ring` sections only) raises AssertionError from the FIELDS leg naming
  the missing keys.
- FAIL path (eval-commit mismatch): the conformant fixture + a wrong
  `expected_eval_commit` raises AssertionError naming BOTH the probed
  field value and the supplied actual (the T270 reporting requirement).
- FAIL path (missing file): a nonexistent path raises AssertionError
  naming the probed path (not an uncaught OSError).
- The existing battery stays green unmodified.

## Acceptance

`python3 scripts/test_wrap_assert.py` green (all pre-existing legs + the
new ones); the RED proof demonstrated by stashing the leg; the LOOP-SPEC
doctrine sentence landed; the full rust suite green (the doctrine
carriers' pins included).

check: python3 scripts/test_wrap_assert.py && cargo test --release
