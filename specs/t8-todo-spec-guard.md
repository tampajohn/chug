# T8 — TODO↔spec consistency guard + t1–t6 backfill

check: cargo test

## Concern

SELF-SPEC mandates one spec file per TODO row (`specs/t<N>-<slug>.md`), but
`specs/` has never existed in git (`git log --all -- specs/` is empty) while
TODO.md T1–T6 reference it; the commit that closed T1/T2/T4/T5 (`9840aba`)
touched only `src/api.rs` + `src/tools.rs`. The self-improvement loop violated
its own protocol and nothing noticed (EVALUATION.md I10). Without a guard,
TODO.md — the meta loops' shared memory — can silently drift from the repo.

## Repo context

- `TODO.md` (repo root): markdown table, header `| id | title | spec | pri |
  status | notes |`, ids `T<int>`, statuses `todo | in-progress | blocked |
  done`, spec column `specs/t<N>-<slug>.md`.
- `SELF-SPEC.md`: owns the format; do not edit it (human spec).
- No existing test reads repo-root files; use `env!("CARGO_MANIFEST_DIR")` in
  an integration test (`tests/`) so the path is stable regardless of cwd.

## Requirements

1. New integration test (e.g. `tests/todo_consistency.rs`) that parses
   TODO.md's table and asserts, for every data row:
   - exactly 6 cells;
   - id matches `^T[0-9]+$` and ids are unique;
   - pri parses as an integer;
   - status ∈ {todo, in-progress, blocked, done};
   - spec cell matches `^specs/t<N>-[a-z0-9-]+\.md$` where `<N>` equals the
     row's id number (case-insensitive on the `t`);
   - the referenced spec file exists on disk.
   Header and separator rows are skipped; blank trailing lines ignored.
2. Backfill the missing specs: create `specs/t1-api-retry-endpoint-restart.md`,
   `specs/t2-llm-activity-timeout.md`, `specs/t3-stale-ledger-detection.md`,
   `specs/t4-bash-cargo-path.md`, `specs/t5-edit-disambiguate.md`,
   `specs/t6-stub-test-timeouts.md`. t1/t2/t4/t5 are done-record specs: brief
   concern, what shipped (commit `9840aba`; api.rs retry schedule + 180s
   activity watchdog; tools.rs cargo-PATH prepend; edit_file match-line
   errors), and where the regression tests live. t3/t6 are real forward specs
   written from their TODO notes (concern, repo context, requirements, tests,
   acceptance, `check: cargo test`) — t3 covers `ensure_seeded` /
   `driver.rs:185` ledger-vs-goal mismatch; t6 covers stub-internal timeouts +
   bounded gate commands.
3. The guard test must FAIL on the repo state before the backfill (dangling
   spec refs) and PASS after.

## Tests

- The consistency test itself, plus a unit-testable parser: a fixture string
  with a malformed row (bad status, missing spec file, id/spec-number
  mismatch) is rejected with a useful message naming the row.

## Acceptance

- `cargo build`, `cargo clippy --all-targets -- -D warnings`, `cargo test` all
  green.
- Deleting any `specs/t*.md` file makes `cargo test` fail.
