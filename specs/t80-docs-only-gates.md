# T80 — Docs-only rounds skip the cargo gates

check: cargo test

## Concern

Docs-only rounds (markdown diff, zero .rs changes) currently pay the full
gate suite 2–4 times per item: worktree gates, validator gates, main
post-merge gates — 500+ tests, clippy, each ~10–30s even warm, plus the
validator's full re-run "independently". For a README clause or a doctrine
sentence the suite cannot go red (guard suites excepted), so the runs are
pure latency. Recent docs-class items: T22, T30, T33, T34, T35, T40, T72.

## Repo context

- `LOOP-SPEC.md` §2 steps 3–5: bounded `cargo test` gates at review,
  validation, and post-merge; §2 step 4 makes validation optional for
  docs/tests-only items but says nothing about gates.
- `tests/todo_consistency.rs` + `bash -n` cover the actual docs-only risk
  surface (table format, doctrine tokens, script syntax).

## Requirements

1. Amend LOOP-SPEC §2: when the round diff touches ONLY `*.md` (and no
   `*.rs`/`*.toml`/`*.sh`), gates shrink to `cargo test --test
   todo_consistency` (+ `bash -n` on any `.sh` in the diff). Full
   build/clippy/test is skipped at review AND post-merge.
2. The classification is mechanical and stated in the template:
   `git diff --name-only main...<branch> | grep -qvE '\.md$'` → full gates.
3. Validators retain the right to run the full suite anyway when the doc
   diff quotes commands/check lines (the T67 class: a spec check line IS
   executable text) — one sentence, judgment preserved.
4. Doctrine sentences naming the override stay: any ambiguity → full gates.

## Tests

- Doctrine item: validation REQUIRED — review checks the predicate is
  file-extension-exact (not "mostly docs"), the guard-suite floor is named,
  and the validator-escape clause exists.
- Acceptance: a later docs-only item's Outcomes records gate wall time vs
  the ~30–60s full-suite baseline.

## Out of scope

- Skipping validation verdicts (that's §2 step 4's existing rule);
  tests-only rounds (they run the suite — it IS their diff).
