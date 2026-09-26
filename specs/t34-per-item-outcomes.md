# T34 — LOOP-SPEC: Outcomes entries are written per-item at landing, not deferred to wrap

check: grep -q 'per-item' LOOP-SPEC.md && grep -qi 'cycle-12' LOOP-SPEC.md && ! grep -q 'EVALUATION.md gains an \*\*Outcomes\*\* section: what landed, what was' LOOP-SPEC.md

## Repo context

LOOP-SPEC Phase 3 currently makes the cycle's wrap the sole author of
EVALUATION.md's Outcomes section ("EVALUATION.md gains an **Outcomes**
section: what landed, what was skipped/deferred, what the validators
caught." — `LOOP-SPEC.md:121-122`). Cycle 12 ended mid-arc at budget
before its wrap; its Outcomes entry was never written by cycle 12 and
had to be RECONSTRUCTED from the git record at cycle-13's wrap (the
entry is titled "RECONSTRUCTED at cycle-13 wrap"; eval commit `697a6b6`
records the lesson: "deferred-wrap loss lesson: write Outcomes
per-item"). This is the same ownership class as the T10/T12 row-flip
fix (children die between the code commit and the bookkeeping; the
orchestrator now owns the row AT LANDING): the narrative bookkeeping
gets the same treatment.

## Requirements

1. **LOOP-SPEC.md §2 step 5** ("Harvest, then merge + close — you own
   the books.") gains a per-item Outcomes directive: after the TODO row
   flip, append the item's Outcomes entry to EVALUATION.md in the SAME
   commit as the row flip (or an immediately following `eval:` commit),
   so a mid-cycle death loses no narrative. The sentence must name the
   cycle-12 bite (reconstruction from git at cycle-13 wrap).
2. **LOOP-SPEC.md Phase 3's Outcomes bullet is re-scoped** from
   authorship to assembly + gap-check, e.g.: "EVALUATION.md's
   **Outcomes** section is complete and truthful — per-item entries
   were written at each landing (§2 step 5); wrap adds the
   skipped/deferred rows, the cycle-level notes (what the validators
   caught, final state), and fills any gap a mid-arc death left."
   (Wording may vary; the re-scope from wrap-authorship to
   wrap-assembly is the requirement.)
3. Every other byte of LOOP-SPEC.md unchanged — step numbering, the
   harvest directive, the push-after-each-item clause all untouched.
4. Commit message cites the cycle-12 evidence and the T10/T12
   ownership-class analogy.

## Tests

- Doctrine-only change: no code tests apply. The `check:` line pins
  the new per-item language, the cycle-12 citation, and the absence of
  the old wrap-authorship bullet.
- Run `cargo test -- --test-threads=4` once to confirm the tree is
  green (you changed no code).

## Acceptance

- `check:` passes verbatim from the worktree.
- `git diff` shows hunks in LOOP-SPEC.md only.

## Boundaries

- Implement TODO item t34 ONLY. DO NOT touch TODO.md, LEDGER.md, or
  EVALUATION.md in the main tree — bookkeeping is the orchestrator's.
  Do not edit any file other than LOOP-SPEC.md.
