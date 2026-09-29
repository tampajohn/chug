# T150 — pin: every `todo`-status TODO row's spec carries an `estimate:` line

check: cargo test

estimate: ~70 changed lines (tests only)

## Repo context

META-META-SPEC's spec quality bar requires every specs/t<N>-*.md to
carry an `estimate: ~N changed lines` line — the T110 filing-time
ceiling (~500 hard, ~400 should-split) depends on it. The cycle-65
codex-intake filed 10 rows (T134–T143) whose specs carry NO estimate
line (verified: `grep -L 'estimate:' specs/t13[4-9]*.md specs/t14*.md`
= 10/10), and 4 of those 10 rows then died 80/80 mid-work (T136/T138/
T139/T143 — all absorbed by T63 resume, ~4 resume spin-ups of tax).
Doctrine existed; nothing enforced it at filing time.

A universal retroactive pin is infeasible — 121 of 141 existing spec
files predate the rule — so the pin binds exactly where it matters:
**specs referenced by `todo`-status rows in TODO.md** (at filing time
every new row is `todo`; done rows are exempt history). When the queue
is empty the leg is vacuously green — the synthetic unit legs below
keep the guard non-vacuous.

## Requirements

1. Extend the existing TODO/spec guard surface (tests/todo_consistency.rs
   — follow its parsing; a sibling test file is acceptable if the child
   finds the guard's structure fights the addition): for every TODO.md
   row whose status cell is `todo`, the named spec file must contain a
   line matching `estimate:` with a `~<number>` in it.
2. Structure the check as a pure function over (table text, spec-reader
   closure) so the unit legs below need no repo fixture edits; the
   repo-live leg calls it on the real TODO.md and asserts zero misses.
3. Synthetic unit legs (each RED-proven against a broken implementation):
   a `todo` row whose spec lacks the line → flagged; a `todo` row whose
   spec has `estimate: ~120` → clean; a `done` row whose spec lacks the
   line → NOT flagged (history exempt); an `estimate:` line with no
   `~N` number → flagged (a bare word is not an estimate).
4. No TODO.md, META-META-SPEC.md, or spec-file edits in this row — the
   requirement already exists in doctrine; this row is the mechanical
   guard only. (That keeps the row out of the doctrine-never-overlaps
   class.)

## Acceptance

- `cargo build && cargo clippy --all-targets -- -D warnings && cargo test`
  all green in the worktree (the repo-live leg is vacuously green at an
  empty queue; the synthetic legs carry the weight).
- Validation routing: tests-only tooling guard — kimi SKIPPED per
  LOOP-SPEC step 4 (T130/T131 precedent); the RED proofs are the
  evidence.
