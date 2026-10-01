# T186 — LOOP-SPEC step 5: harvest ALL worktree events segments + never remove a worktree while a child pid lives

check: export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared && cargo test

## Repo context

Two cycle-84 incidents share one root: LOOP-SPEC §2 step 5's harvest +
removal mechanics are underspecified.

1. **Harvest gap ×2 (evidence lost).** The t183 impl child's two glm
   segments (an 80/80 death + its T63 resume) and the t181 impl child's
   glm segment (20/80 first-try) were never harvested into the main
   repo's `.chug/`; both worktrees were removed. Mechanism: a FRESH
   child launched into a reused worktree ROTATES the predecessor's
   `.chug/events.jsonl` to `.chug/events-<ts>.jsonl` (T10/T7), so
   copying only `.chug/events.jsonl` — the natural reading of step 5's
   "harvest every child run's `.chug/events.jsonl` from the worktree" —
   silently drops the rotated segment(s), and `git worktree remove`
   then deletes them (untracked). Cycle-84's glm orchestrator even
   named the t181 harvest `events-t181-impl-validate-…` believing both
   segments were inside; the file holds `runs: 1`, kimi only. t183's
   loss happened DESPITE the cycle-83 wrap note carrying an explicit
   correct recipe ("cp the worktree's .chug/events*.jsonl as
   events-t183-impl-* … both segments live in the worktree .chug/") —
   a written recipe is not a mechanism. (Cycle-83's kimi harvest of
   t180 did it right: `events-t180-impl-…` (runs: 2) AND
   `events-t180-validate-…` as separate files.)
2. **Zombie-collision (a gate raced, a worktree removed under a live
   child).** Cycle-84 seg-1 declared the t183 validator (pid 6260)
   dead via a TRUNCATED `ps` read, removed its worktree while it was
   alive; the zombie's cargo suites raced the wrap goal-gate →
   "check command failed" rejection, ~30 min diagnosis (accepted-goal
   summary, `.chug/events-20261001-121517.jsonl` tail). Code change
   already weighed and rejected (eval-triage d1790856539-8); the two
   doctrine lessons were explicitly handed to this eval: (a) never
   remove a worktree while any child pid launched in it is alive;
   (b) liveness comes from `delegate status` or `kill -0 <pid>`,
   never a truncated `ps … | head` read.

This row is DOCTRINE-ONLY (LOOP-SPEC.md + its pin tests) → SOLO: no
other child in flight while it runs (LOOP-SPEC hard rules).

estimate: ~70 changed lines (doctrine text + pin legs).

## Requirements

1. **Harvest-all mechanics (§2 step 5).** The harvest step requires
   copying ALL of the worktree's `.chug/events*.jsonl` files — the live
   stream AND every rotated `events-<ts>.jsonl` segment — one harvested
   file per source file, before any `git worktree remove`. Naming:
   each harvested file is named per the run segment(s) it ACTUALLY
   contains (inspect its `run_start` model/spec — e.g.
   `events-t<N>-impl-<ts>.jsonl` for the impl segment,
   `events-t<N>-validate-<ts>.jsonl` for the validator) — never a
   combined name like `impl-validate` for a single-segment file. The
   step names the rotation mechanism (a fresh child in a reused
   worktree rotates its predecessor's stream, T10/T7) so the mental
   model is one-file-per-segment, and names the cycle-84 t183/t181
   losses as evidence.
2. **Removal precondition (§2 step 5).** Before any
   `git worktree remove`, verify NO child pid launched in that
   worktree is still alive — via `delegate status` liveness or
   `kill -0 <pid>`, never a truncated `ps` pipeline. A live child
   blocks the removal: harvest what exists, leave the worktree, note
   the row for the next cycle. The cycle-84 zombie (pid 6260) is
   named as the evidence.
3. **Phase-3 wrap bullet** amended to match (harvest = all segments).
4. **Pins.** tests/loop_spec_recovery.rs (the step-5 doctrine carrier)
   gains legs pinning the new load-bearing tokens exactly-once inside
   the step-5 window (the T48 needle style already used there); any
   other pin file quoting the edited step-5 text is amended in-commit
   (sweep: `grep -n "harvest" tests/loop_spec_*.rs`).

## Tests

- The new pin legs go red if either doctrine sentence is reverted
  (RED-proof in the commit message: revert each sentence → the named
  leg fails).
- `cargo test --test loop_spec_recovery` green; full `cargo test` green
  (the check line runs it).

## Acceptance

- `cargo test` and `cargo clippy --all-targets -- -D warnings` green.
- LOOP-SPEC.md diff touches §2 step 5 + the Phase-3 harvest bullet
  only (plus whitespace-free context); no other doctrine file edited.
- Both evidence citations (t183/t181 losses; pid 6260 zombie) present.

## Out of scope

- META-SPEC.md (a separate row, T187, handles its alarm-600 drift).
- Code changes to the harvest tooling (none exist — the harvest is a
  documented manual step; this row sharpens the documentation).
