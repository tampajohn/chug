# T107 — LOOP-SPEC child goal template: worktree-discipline commitment clause

check: grep -q 'commit ONLY from your worktree cwd' LOOP-SPEC.md && grep -q 'never run git add or git commit with the main repo as cwd' LOOP-SPEC.md && cargo test --test loop_spec_recovery

## Repo context

Cycle-58 INCIDENT (EVALUATION.md cycle-58 Outcomes, T104 entry): the
T104 impl child (glm, 75/80, goal-accepted) **committed its work to the
MAIN repo** — it `cd`'d from its worktree (`/tmp/chug-loop-t104`) to the
main checkout to run byte-identity checks against main's tree, then ran
`git add` + `git commit` from there. First worktree-discipline breach in
58 cycles. The commit (`ee3943e`) sat unpushed on local main; recovery
was branch-at-commit + main reset to the pushed state + worktree
re-point, and the standard arc then ran untouched (all recorded in the
cycle-58 Outcomes). Root cause: the step-2 goal template's commitment
clause is **ambiguous once a child cd's out** — LOOP-SPEC.md's template
says `Commit your work here.` where "here" was written to mean the
worktree cwd the child launched in, but a child that has cd'd to the
main repo for legitimate read-only checks can resolve "here" to its
CURRENT directory. Nothing else in the template re-anchors it. The
doctrine around it is already strict (children never touch main's
TODO.md/LEDGER.md; merges are the orchestrator's alone) — the one
unwritten rule is *where `git commit` may run*.

Edit surface (LOOP-SPEC.md, Phase 2 step 2's goal template — the
`goal:` block naming `Implement TODO item t<N> ONLY`): the sentence
`Keep cargo build + clippy + test green. Commit your work here. DO NOT
touch TODO.md or LEDGER.md — bookkeeping is the orchestrator's.` gains
the discipline clause. The rest of the template (T47 export line,
budgets, model, polling) is byte-identical. Step 4's fix-up arc shares
this template by reference (T21-era cross-ref: "FAIL → fix-up child
with the findings pasted into its goal"), so one edit covers fix-up
children too; the step-4 VALIDATOR template is untouched (validators
commit nothing — their contract is tree-restored, a different surface).

## Requirements

1. One in-place edit inside the step-2 goal template: after
   `Commit your work here.` insert the discipline clause —
   `Commit ONLY from your worktree cwd (the /tmp/chug-loop-t<N> you were
   launched in): if you cd to the main repo for read-only checks, cd
   back before committing — never run git add or git commit with the
   main repo as cwd.` (exact wording is the impl's to fit the template's
   line-wrap style, but the two check: needles above must appear
   verbatim).
2. Everything else in the goal template byte-identical: the T47
   `CARGO_TARGET_DIR` export sentence, the `DO NOT touch TODO.md or
   LEDGER.md` sentence, the budgets comment, the model line.
3. Pin (new leg in `tests/loop_spec_recovery.rs`, the step-2-template
   pin carrier per the T63/T102 precedent): the two check: needles each
   appear EXACTLY once in LOOP-SPEC.md, inside the step-2 window (the
   T64 loose-heading scope-leg pattern), and the pre-existing
   `Commit your work here.` sentence is still present exactly once
   (the clause EXTENDS it, never replaces it — a replacement mutant
   dies).
4. RED-proof the new pin: delete the inserted clause → the new pin legs
   fail (record the RED run in the commit message), revert → green.
5. Non-goals: no edit to the step-4 validator template, no META-SPEC.md
   edit (its §6 goal template is the validator's tree-restored
   contract), no renumbering of steps, no other LOOP-SPEC text.

## Tests

- `cargo test --test loop_spec_recovery` green with the new legs
  (broad enough — runs the whole pin file the change touches, T96
  rule).
- The full suite stays green (`cargo test --bin chug` in the worktree):
  doctrine-only edit, but the pin file is code.
- Non-vacuousness: the req-3 pin fails on the pre-edit LOOP-SPEC.md
  (the needles are absent on the parent — proven by the RED leg).

## Acceptance

- The `check:` line passes in the worktree.
- Diff is LOOP-SPEC.md (one hunk) + tests/loop_spec_recovery.rs (new
  legs) ONLY.
- Doctrine item → runs ALONE (no T44 overlap), kimi REQUIRED
  validation (loop doctrine is on the §2 step-4 REQUIRED list).
