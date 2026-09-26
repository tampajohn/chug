# T33 — Align META-META-SPEC's priority line with LOOP-SPEC's amended doctrine

check: tr -s ' \n' ' ' < META-META-SPEC.md | tr -d '*' | grep -q 'bugs > robustness > features > DX friction > performance' && ! tr -s ' \n' ' ' < META-META-SPEC.md | tr -d '*' | grep -q 'bugs > robustness > DX friction > performance > features'

## Repo context

`META-META-SPEC.md:69` (the "Extend TODO.md" section) reads:
"Priority doctrine: bugs > robustness > DX friction > performance >
features."

LOOP-SPEC §2 reads: "Priority order: bugs > robustness > **features** >
DX friction > performance." plus "Features are first-class: the mandate
is closing capability gaps … When the queue holds both a credible
feature row and a DX-friction row of the same pri, work the feature
first."

The evaluator (META-META role, filing order) and the orchestrator
(LOOP-SPEC role, work order) are the SAME loop session reading two
disagreeing doctrines — the cycle-13 wrap handed this to this eval for
adjudication. **Adjudication (cycle-14 eval, EVALUATION.md §2 P3):**
LOOP-SPEC wins — it post-dates, carries the explicitly amended
doctrine, and its preamble asserts override ("They apply in full except
where this spec overrides"). The README's continuous-mode paragraph
already matches LOOP-SPEC; only META-META-SPEC drifted. The T30
precedent sanctions a loop item editing META-META-SPEC.md (its
validator adjudicated: the evaluator-role hard-rule list does not
enumerate META-META-SPEC.md).

## Requirements

1. In `META-META-SPEC.md`, replace the priority-doctrine sentence with
   the LOOP-SPEC-aligned order AND the first-class clause, e.g.:
   "Priority doctrine: bugs > robustness > features > DX friction >
   performance — features are first-class (LOOP-SPEC §2): at equal
   pri, a credible feature row is worked before a DX-friction row."
   (Wording may vary; the ORDER and the first-class clause are the
   requirement.)
2. Every other byte of META-META-SPEC.md unchanged — including the
   adjacent T30 worktree-relative `check:` sentence and its bites
   parenthetical.
3. Do NOT edit LOOP-SPEC.md (already correct) or README.md (already
   correct) — this is a one-file alignment.
4. Commit message cites the contradiction, the adjudication, and the
   T30 precedent.

## Tests

- Doctrine-only change: no code tests apply. The `check:` line pins
  the new order and the absence of the old one.
- Run `cargo test -- --test-threads=4` once to confirm the tree is
  green (you changed no code).

## Acceptance

- `check:` passes verbatim from the worktree.
- `git diff` shows one hunk in META-META-SPEC.md and nothing else.

## Boundaries

- Implement TODO item t33 ONLY. DO NOT touch TODO.md or LEDGER.md in
  the main tree — bookkeeping is the orchestrator's. Do not edit any
  file other than META-META-SPEC.md.
