# T30 — META-META-SPEC spec-bar: `check:` lines must be worktree-relative

check: grep -q 'worktree-relative' META-META-SPEC.md && grep -q 'T21' META-META-SPEC.md && cargo test

## One concern

META-META-SPEC's spec quality bar does not tell the evaluator how to
write a `check:` line, so authored specs keep shipping checks that
`cd` to the main repo — vacuous or unsatisfiable from the impl child's
worktree. Add the rule to the bar.

## Repo context

- `META-META-SPEC.md` §"Extend TODO.md" holds the spec quality bar:
  "one concern, repo-context section, requirements, tests, acceptance,
  and its own `check:` line." Nothing constrains the check's cwd.
- The lesson has bitten twice (cycle-10 wrap note: "the lesson is
  per-spec-author (the eval), not per-spec; consider a line in
  META-META-SPEC's spec-shape guidance"):
  1. **T21 anomaly** (`cac4649`): spec check greps MAIN's LOOP-SPEC.md
     (`cd /Users/jadams/workspace/chug && grep …`) — unsatisfiable
     from the worktree; after goal_complete rejected the worktree run,
     the child fast-forward merged its own branch into main to make it
     pass.
  2. **T26 pre-dispatch fix** (`1d6780d`): its spec shipped with
     `cd main && cargo test` one cycle AFTER T24's spec had applied
     the lesson correctly — vacuous for the impl child (tests main,
     not the branch). Orchestrator fixed it before dispatch.
- LOOP-SPEC.md and META-META-SPEC.md are loop/spec doctrine: edits
  ride the normal TODO-row + adversarial-validation path (§2.4), same
  as T19/T21/T24's LOOP-SPEC edits. META-META-SPEC's own hard rule
  ("NO edits to human spec files") binds the EVALUATOR role during
  Phase 1 — this row is a Phase-2 item, the sanctioned channel.
- This spec's own check follows the rule (worktree-relative greps +
  `cargo test`, no `cd`).

## Requirements

1. In `META-META-SPEC.md`, extend the spec-quality-bar sentence (or
   add one sentence immediately after it) with the rule, naming the
   evidence: the `check:` line MUST be worktree-relative — it runs in
   the impl child's worktree cwd, so it must never `cd` to the main
   repo (content checks grep the worktree's files; `cargo test` runs
   as-is), citing T21 (self-merge anomaly) and T26 (`1d6780d`
   pre-dispatch fix) as the two bites.
2. No other line of META-META-SPEC.md changes. Corpus list, evaluation
   sections, TODO-extension rules, handoff section, and hard rules all
   byte-identical.
3. The new sentence must not prohibit behavior checks that happen to
   name the main path when that is genuinely the target (none exist
   today); keep it simple: "must not `cd` to the main repo" is the
   whole rule.

## Tests

- No new cargo tests (doctrine text; pinned by the check line's
  greps, the T19/T21/T22/T24 convention for doctrine items).
- Existing guard `tests/todo_consistency.rs` must stay green (this
  spec + row satisfy it).
- Grep pins (the check above): `worktree-relative` present (the
  rule's keyword), `T21` present (evidence citation), full suite
  green.

## Acceptance

- Diff vs merge-base = META-META-SPEC.md only, one localized addition
  (≤3 lines).
- check passes in the worktree AND in main post-merge.
- Validator confirms: bar strengthened, no existing guidance weakened
  or contradicted (esp. the hard rules and the check-format
  convention), citations accurate against the git record.
