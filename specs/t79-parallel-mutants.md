# T79 — Parallel mutation testing for validators

check: cargo test

## Concern

Validators run mutants strictly serially: edit → targeted test → revert →
next. A 10-mutant round at 10–30s per leg is 2–5 minutes of pure serial
wall time per validation, and validators are the longest single leg of the
pipeline. Operator-approved knob 2026-09-26 (parallel mutants, 3–4×
validation speedup). Mutants are independent by construction — one worktree
per mutant makes the legs parallel.

## Repo context

- `META-SPEC.md` §6 / `LOOP-SPEC.md` §2 step 4: validator goal text
  (mutation-testing instructions, VERDICT format, tree-restored requirement).
- T52 role-keyed target dirs: each parallel leg needs its OWN target dir to
  avoid the cross-checkout artifact race (the exact bug T52 fixed).
- The "tree restored byte-clean" requirement serializes cleanup at the END,
  not per leg.

## Requirements

1. Amend the validator goal template (LOOP-SPEC §2 step 4 + META-SPEC §6):
   after gates pass on the clean tree, the validator MAY run mutants in
   parallel — one throwaway worktree per mutant
   (`/tmp/chug-mut-<item>-<k>`), each with
   `CARGO_TARGET_DIR=.../target-shared-mut-<k>` (T52 lesson), each running
   its targeted test, results collected by the validator.
2. Cap parallelism at 3 mutant legs in flight (host has other work;
   validators are already 1 of max-2 children).
3. Serial remains the default when mutants touch overlapping files (the
   validator declares the overlap judgment in its verdict notes).
4. Tree-restored verification unchanged: main worktree byte-clean before
   verdict; throwaway worktrees removed after results are collected.
5. VERDICT format unchanged: findings reference mutant names, not leg dirs.

## Tests

- Doctrine item: validation REQUIRED — review checks the target-dir rule
  (mut-<k> dirs, never shared), the cap, the overlap judgment clause, and
  that tree-restored semantics are preserved.
- Acceptance: a later validator's events show ≥2 mutant legs overlapping in
  wall time; Outcomes records validation leg wall time vs serial baseline.

## Out of scope

- Changing what mutants are chosen (validator judgment); parallel fix-up
  rounds; mutant generation by the orchestrator.
