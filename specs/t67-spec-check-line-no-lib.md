# T67 — spec `check:` lines never invoke `cargo test --lib` (binary-only crate) + mechanical guard

## Repo context

Every TODO work-item spec carries its own `check:` line (META-META-SPEC,
"Extend TODO.md": "its own `check:` line, which MUST be worktree-relative —
it runs in the impl child's worktree cwd"). `goal_complete` re-runs that
line as the child's goal gate; a failing check rejects the claim and the
loop continues burning the child's budget.

This crate is **binary-only**: `src/main.rs`, no `lib.rs`, no `[lib]`.
`cargo test --lib` therefore fails unconditionally:

```
$ cargo test --lib
error: no library targets found in package `chug`   (exit 101)
```

Nine child-stream sightings of spec check lines carrying `cargo test --lib`
(t22, t25, t26, t29, t39, t42, t58, t59, t64), each rejecting an otherwise
green child's `goal_complete`. Latest: t64-impl completed AND committed the
work, then died 50/50 with its goal gate unsatisfiable (leg 1,
`cargo test --test shared_target_dir`, had passed 14/14 first); the
orchestrator substituted review + gates + independent mutant reproduction
per T55/T62. The defect burns child budget on an unsatisfiable gate and
could mask real red behind a wrong-reason rejection. Reproduced at eval
time on main: exit 101, message above.

Correct forms already in use: `check: cargo test -- --test-threads=4 &&
cargo test --bin chug mcp_http` (specs/t66), plain `cargo test`,
`cargo test --test <integration>`.

## Requirements

1. **Convention (doctrine).** META-META-SPEC.md's check-line sentence in
   "Extend TODO.md" gains the rule: a spec's `check:` line MUST NOT invoke
   `cargo test --lib` — this crate is binary-only, so `--lib` exits 101
   ("no library targets found") at the goal gate; write plain `cargo test`
   or `cargo test --bin chug [<filter>]` instead. The sentence must carry
   the token `no library targets` (pinned, req 3).
2. **Historical repair.** specs/t64-validator-survivor-pins.md's check
   line: `cargo test --lib` → `cargo test --bin chug` (the only live spec
   still carrying the defect — `grep -l "cargo test --lib" specs/t*.md`
   returns exactly that file pre-fix, nothing post-fix). No other spec
   content changes.
3. **Mechanical guard.** tests/todo_consistency.rs (the T8 guard — already
   scans every `specs/t*.md` via the TODO table, already run by the
   orchestrator after every TODO.md edit) gains a lint leg: for every spec
   file on disk matching `specs/t*.md`, the spec's `check:` line must not
   contain the token sequence `cargo test --lib`. Failure message names the
   file and the rule (`binary-only crate — use cargo test or cargo test
   --bin chug`). Plus a doctrine pin: META-META-SPEC.md contains the token
   `no library targets` (so silently reverting the convention sentence
   fails the suite).
4. **Non-vacuousness evidence (in the child's ledger or commit message):**
   (a) re-adding `cargo test --lib` to any one spec's check line turns the
   new lint leg red, revert → green; (b) removing the META-META-SPEC
   convention sentence turns the doctrine pin red, revert → green; (c) the
   full pre-existing todo_consistency suite stays green (the T8 table
   validation is untouched).

## Tests

- New lint leg in tests/todo_consistency.rs covering req 3 (all
  `specs/t*.md` files scanned, `cargo test --lib` rejected, t64's repaired
  line accepted).
- Doctrine pin in the same file covering req 3's `no library targets`
  needle in META-META-SPEC.md.
- Both legs get the req-4 non-vacuousness run (mutate → red → revert →
  green) recorded by the child.

## Acceptance

- `grep -l "cargo test --lib" specs/t*.md` → no output.
- META-META-SPEC.md carries the convention sentence with `no library
  targets`.
- `cargo test --test todo_consistency` green; full gates (build + clippy
  `--all-targets -- -D warnings` + `cargo test`) green.
- TODO.md/LEDGER.md untouched (orchestrator's).

check: cargo test --test todo_consistency && grep -q "no library targets" META-META-SPEC.md && ! grep -l "cargo test --lib" specs/t*.md
