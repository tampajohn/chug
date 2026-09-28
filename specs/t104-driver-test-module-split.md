# T104 — driver.rs test-module family split (pre-declared ~4500 trip line crossed again)

check: cargo test --bin chug

## Repo context

Cycle-58 eval §2 I3 (EVALUATION.md): the cycle-36 eval PRE-DECLARED the
mandate — "driver.rs … the ~4,500 trip line is PRE-DECLARED now (a
future crossing carries a T71-class extraction mandate)" — and T84
executed it at 5,488 lines (trim machinery → src/trim.rs, driver.rs
5,488→4,509). driver.rs is now **5,717 lines** (+1,208 over the
post-T84 size, +1,217 over the trip line): the mandate fires again.
This crossing's shape is NEW and the spec names it honestly: the
PRODUCTION half is healthy (driver.rs:1–1452 — 1,452 lines, 28 fns);
the growth is the TEST MODULE (`#[cfg(test)] pub(crate) mod tests` at
driver.rs:1453 → EOF: **4,264 lines, 139 of the file's 167 `fn`
tokens** — T83/T89/T90/T91 driver-integration families landed here).
Children editing driver.rs tests (every driver-touching row: T38, T89,
T90, T91) page a 5,717-line file three reads deep (T26 pagination).

The split (Rust-2018 idiom for a file+directory module pair):
- `src/driver.rs` keeps its `#[cfg(test)] pub(crate) mod tests;`
  DECLARATION (one line, attributes intact) and the module BODY moves to
  `src/driver/tests/mod.rs`.
- `src/driver/tests/mod.rs` holds the shared harness (the
  `ctx_for`/`knobs_with`/`tool_use_response`/`RecordingSink` family —
  driver.rs:1453's comment records that src/trim.rs's tests REUSE this
  harness: it stays `pub(crate)`-reachable exactly as today) plus one
  `mod <family>;` line per extracted family file.
- Each cohesive test family moves to its own
  `src/driver/tests/<family>.rs` (the impl picks the families by the
  test-name prefixes — e.g. `image_*` (~driver.rs:5355–5717),
  budget/budget-low, steering/kick, stuck, permissions/hooks policy
  chain, plan-mode, resume/rotation — the exact cut is the impl's
  judgment under T84's ONE rule: every test lands in exactly one family
  file; the harness and only the harness stays in mod.rs).
- T84's rules verbatim: byte-identical MOVES (no logic edits, no
  renames, no signature changes, test count before == after, recorded in
  the commit message); `use super::*;` per family file adjusted to the
  module path (`use super::super::*;` / `use crate::driver::tests::*;`
  as the cut requires); anything a cross-module caller uses (trim.rs's
  harness reuse) stays reachable via the existing `pub(crate)` seam.

Guard interplay (verified at filing): `tests/readme_layout.rs` walks
`std::fs::read_dir(src)` NON-recursively — its filesystem set is
top-level `src/*.rs` stems only, so `src/driver/tests/*.rs` files do NOT
enter the guard's set and the README layout line needs NO edit. The impl
proves this by keeping the guard green WITHOUT touching it (and records
the submodule layout in the commit message). `src/main.rs` needs no new
`mod` line (the tests module hangs off driver.rs itself).

## Requirements

1. The split above: driver.rs shrinks to ~1,455 lines (production + the
   one-line tests declaration + the harness comment if it stays with the
   declaration — impl's judgment, documented); every moved line
   byte-identical.
2. Test count before == after, both numbers in the commit message (the
   T84 rule); the full suite green from the new layout.
3. `src/trim.rs`'s tests still compile and pass against the harness via
   the unchanged `pub(crate)` seam — no trim.rs edits.
4. RED-proof (T84 shape): demonstrate one deletion leg failing LOUDLY
   (e.g. drop one `mod <family>;` line → that family's tests vanish from
   the run and a count pin fails; revert one moved test to a stub → it
   fails) — record the legs in the commit message or ledger.
5. Non-goals: no production-code moves, no behavior change, no README
   edit, no guard edit, no renames of test fns.

## Tests

- `cargo test --bin chug` green in the worktree; count equality proven
  (req 2); `cargo clippy --all-targets -- -D warnings` clean.
- `cargo test --test readme_layout` green UNMODIFIED (the guard's
  non-recursive contract absorbs subdirectory modules).
- Non-vacuousness: the split is a MOVE — the validator's angle is
  byte-identity + count equality + the req-4 loud legs (a mutant that
  drops a family-file `mod` line must fail the count; a mutant that
  edits moved logic dies against the moved tests).

## Acceptance

- The `check:` line passes in the worktree (the whole suite — the moved
  tests run from their new homes).
- Diff is src/driver.rs + new src/driver/tests/** ONLY.
- Validation: driver.rs is on the LOOP-SPEC §2 step-4 REQUIRED list →
  kimi REQUIRED (T84 precedent: byte-identity re-proof, count equality,
  parallel mutants on the moved families).
