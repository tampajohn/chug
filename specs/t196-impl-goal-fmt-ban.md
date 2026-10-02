# T196 — Impl-goal template bans tree-wide formatters (the T188 fmt-noise class)

check: export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared && cargo test --test loop_spec_recovery --test todo_consistency && grep -qF "tree-wide" LOOP-SPEC.md && grep -qF "cargo fmt" LOOP-SPEC.md

## Repo context

Cycle-87 wrap incident (handed to the next eval verbatim: "impl-child
tree-wide formatters need a goal-template ban"): the t188 round-3 glm
impl child ran a mass `cargo fmt` across the whole repo — 83 files,
+12.6k/−9.2k uncommitted fmt noise on top of the real change. The T63
resume had to spend its first act stripping the noise to the 775 real
insertions before the fix-up could proceed. Cost: most of a resume
child plus review confusion (the diff was unreviewable until
stripped). The LOOP-SPEC §2 step-2 impl goal template currently says
nothing about formatting, so a child that decides to "tidy up" has
license — glm defaults to repo-hygiene gestures when a task leaves it
slack iterations.

This row is DOCTRINE-ONLY (LOOP-SPEC.md + one pin leg) → SOLO.

estimate: ~25 changed lines (one template sentence + one pin leg +
check greps).

## Requirements

1. LOOP-SPEC §2 step-2's impl goal template gains ONE sentence, woven
   into the goal text (not a footnote): never run tree-wide
   formatters (`cargo fmt` across the repo, mass whitespace or
   import-ordering passes) — format nothing you did not rewrite; a
   diff touching files outside the spec's named targets is a review
   red flag on its face.
2. If step 4's fix-up goal shares the template's wording it is
   covered by the same sentence; if it is a separate text, it gains
   the same clause.
3. No other LOOP-SPEC text changes (no renumbering, no reflow).

## Tests

- ONE new exactly-once pin leg in `tests/loop_spec_recovery.rs` (the
  step-2 doctrine carrier): the ban's load-bearing tokens
  (`tree-wide` + `cargo fmt`) occur exactly once in LOOP-SPEC.md;
  RED-proven by deleting the sentence, the deletion proof stated in
  the commit message.
- `cargo test --test loop_spec_recovery --test todo_consistency`
  green.

## Acceptance

- Check line green; `cargo clippy --all-targets -- -D warnings` green
  (trivially — no src).
- Diff confined to LOOP-SPEC.md + tests/loop_spec_recovery.rs.

## Out of scope

- Enforcement beyond doctrine (a pre-commit fmt-detector hook is a
  later-eval call if the class recurs after the ban).
- rustfmt config changes.
