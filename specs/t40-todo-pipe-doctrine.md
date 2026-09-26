# T40 — LOOP-SPEC: TODO row edits must stay pipe-free + verify with todo_consistency

check: grep -Fq 'must contain no `|`' LOOP-SPEC.md && grep -Fq 'cargo test --test todo_consistency' LOOP-SPEC.md && grep -Fq 'cycle-16' LOOP-SPEC.md

## Concern

One concern: LOOP-SPEC's bookkeeping step never warns that a single `|` in a
TODO.md notes cell breaks the T8 consistency guard — and that failure mode
killed a cycle at the goal gate.

## Repo context

- The T8 guard (`tests/todo_consistency.rs`) splits every table row on
  `|`; a notes cell containing a pipe yields 7 cells ("expected exactly 6
  cells, got 7") and fails `cargo test` — which is also LOOP-SPEC's
  `check:` command, so the break surfaces as a **goal_complete rejection**.
- **The bite (cycle-16, `.chug/events-20260926-043119.jsonl`):** the
  orchestrator annotated the T37 row with a recovery recipe whose text
  included a `|`. At iteration ~118 (of 120) its `goal_complete` was
  REJECTED (`goal` event `04:25:54`, "check command failed" —
  `todo_md_is_consistent_with_specs_on_disk`, "row 41 (T37): expected
  exactly 6 cells, got 7"). The fix (`75da192`) and push consumed the
  remaining iterations; the run aborted at 120/120 (`04:26:07`) without
  goal complete. loopd recorded "cycle ended WITHOUT goal complete".
- LOOP-SPEC §2 step 5 ("Harvest, then merge + close — you own the books")
  is where TODO rows get flipped/annotated; it says nothing about the pipe
  hazard or verifying the guard after edits.

## Requirements

1. LOOP-SPEC §2 step 5 gains one sentence (woven into the existing row-flip
   directive, not a new step): when editing TODO.md (row flips, notes
   annotations), the notes cell **must contain no `|`** — the T8 guard
   splits rows on it (cycle-16's goal-gate death) — and the orchestrator
   runs `cargo test --test todo_consistency` (seconds) after every TODO.md
   edit, before committing.
2. The sentence must contain the exact tokens the check greps:
   `must contain no ` + backtick + `|` + backtick,
   `cargo test --test todo_consistency`, and `cycle-16`.
3. Every other line of LOOP-SPEC.md byte-identical (one-hunk change; no
   renumbering — t13's §2.5 reference must keep resolving).
4. No other file touched.

## Tests

- The spec `check:` line above (worktree-relative greps).
- Non-vacuousness: the check fails on the parent commit's LOOP-SPEC.md.
- `git diff` review: exactly one hunk, additive prose only.

## Acceptance

- Check green in the worktree; `git diff --stat` vs merge-base shows
  LOOP-SPEC.md only, one hunk.
- Commit message cites the cycle-16 death (goal rejection at ~118/120,
  abort 120/120, fix `75da192`).

## Out of scope

- Changing the guard itself (its error already names row + cell count; the
  failure was orchestrator discipline, not guard quality).
- A pre-commit hook (the loop has no hook machinery; the doctrine line is
  the proportionate fix).
