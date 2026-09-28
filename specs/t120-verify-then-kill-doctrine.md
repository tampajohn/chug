# T120 — LOOP-SPEC: verify-then-kill SEQUENTIAL + assert-after-sed orchestrator self-checks

check: grep -q 'verify-then-kill is SEQUENTIAL' LOOP-SPEC.md && grep -q 'Never kill in the same breath' LOOP-SPEC.md && grep -q 'sed exits 0 on no-match' LOOP-SPEC.md && cargo test --test loop_spec_recovery

## Repo context

Cycle-61 incidents (orchestrator self-inflicted, recorded in the
cycle-61 Outcomes entry and wrap commit ccd4b0a):

1. **The healthy-validator SIGKILL.** Reading a RENDER-ONLY garble in
   its own console view (a duplicated-tail rendering artifact in the
   delegate status output), the orchestrator concluded the T112
   validator's launch goal was corrupt and SIGKILLed it 35s in — in
   the SAME breath as the check, before any read-back. Transcript
   read-back afterwards proved the 2166-byte payload intact; the
   validator had to be relaunched (a full validator spin-up burned,
   ~11 min wall). The lesson recorded at wrap: "verify-then-kill must
   be SEQUENTIAL — read first, kill after". It lives only in
   EVALUATION.md today; orchestrators read LOOP-SPEC every cycle.
2. **The anchor-typo silent no-op.** A `sed -i` bookkeeping edit with
   a typo'd anchor exited 0 changing nothing (sed never fails on
   no-match); only a later read-back caught it. `edit_file` errors on
   no-match; sed does not — the discipline gap is unwritten.

Doctrine home: LOOP-SPEC Phase-2 step 2 (child management — the kill
rule, beside the poll-posture/budget-death-recovery paragraphs) and
step 5 (bookkeeping — the sed-assertion rule, near "you own the
books"). Pin pattern follows T110 leg h / T114 legs i-j in
tests/loop_spec_recovery.rs (needle exactly-once + window/ordering).

estimate: ~55 changed lines (≈25 doctrine + ≈30 pin legs)

## Requirements

1. **Kill rule (step 2), verbatim needles included:** a short
   paragraph establishing that **verify-then-kill is SEQUENTIAL** —
   before killing any child over a suspected payload garble (a
   delegate goal echo, a log tail, a status render that LOOKS
   corrupt), the orchestrator reads the payload back from the child's
   on-disk artifacts (transcript / events / files via bash) in a
   SEPARATE completed step, and kills only when the read-back proves
   the payload itself corrupt. **Never kill in the same breath** as
   the check: render-only garbles (terminal/preview artifacts) are
   the common case and kill nothing; the cycle-61 T112-validator
   SIGKILL is named as evidence (payload proven intact by read-back
   AFTER the kill; relaunch cost).
2. **Edit-assertion rule (step 5), verbatim needles included:** after
   any `sed`/in-place bash edit to repo files (bookkeeping, harvest,
   gates scripting), grep-verify the intended needle in the same
   command line or the immediately following one — **sed exits 0 on
   no-match**, so an anchor typo is a silent no-op (the cycle-61
   anchor-typo incident named); prefer `edit_file` (errors on
   no-match) for repo files, and when sed is necessary, assert.
3. **Pin legs (tests/loop_spec_recovery.rs)** beside T114's legs i-j:
   leg (k) the two kill-rule needles (`verify-then-kill is
   SEQUENTIAL`, `Never kill in the same breath`) each appear exactly
   once in LOOP-SPEC.md, inside the step-2 window (after the step-2
   heading, before step 3); leg (l) the sed needle
   (`sed exits 0 on no-match`) appears exactly once, inside the step-5
   window. Each leg RED-proven by a temporary mutation (delete /
   duplicate / move), reverted.
4. No other LOOP-SPEC text changes; all pre-existing doctrine pins
   green unmodified (the child runs the full loop_spec_recovery file
   plus the other loop_spec_* pin files to prove it).

## Tests

The pin legs k-l are the tests (exactly-once + windowed), RED-proven.

## Acceptance

- The spec's own `check:` line green (greps + the named integration
  test).
- `cargo test` green at review (doctrine rows run the full suite at
  review regardless; the docs-only-gates classification does NOT apply
  — LOOP-SPEC.md is a pinned doctrine carrier, and step 3's floor
  rules name this the ambiguity-default-to-full case).

## Out of scope

- META-SPEC §6 validator-side verdict discipline (a separate observed
  gap — validator verdicts must be self-contained — was weighed and
  rejected this eval; one occurrence, template already requires a
  numbered findings list).
- Any code change.
