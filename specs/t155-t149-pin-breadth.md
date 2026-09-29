# T155 — pin the two T149 unpinned-text gaps (tests-only doctrine-pin breadth)

check: cargo test

## Repo context

T149 (8699a70, cycle 71) raised the LOOP-SPEC validator-child budgets
50/30 → 60/40 across three surfaces: LOOP-SPEC §2 step 4, the T63-resume
echo in step 2, and META-SPEC §6's launch template. Its kimi validator
PASSed with **two non-blocking unpinned-text observations** (verdict
d1790688874-9, named "candidate pin-breadth rows for a future eval";
carried by the cycle-71 wrap ledger): text-revert mutants on
(a) LOOP-SPEC step 4's budget RATIONALE text — the census sentence
("4 of the last 4 validator children died at budget in cycles 66–69")
and the measure clause (">1 of the next 8 validator runs still dies at
60/40 … trim default mutation-leg counts") — and
(b) META-SPEC §6's validator-template budget text —
stayed full-suite GREEN. Only the bare numbers were pinned
(tests/loop_spec_recovery.rs, updated + RED-proven by T149). A future
edit can now silently drop the rationale/measure text (the teeth that
make the budgets self-governing) with every gate green.

The doctrine-pin family lives in tests/loop_spec_*.rs +
tests/shared_target_dir.rs + tests/nextest_gate_runner.rs (the pinned
doctrine carriers; T150's estimate pin lives in
tests/todo_consistency.rs). This row extends the EXISTING carrier that
already pins T149's numbers — verify at implementation time which file
pins the 60/40 pair (tests/loop_spec_recovery.rs per the T149 row
notes) and extend THAT file, not a new one.

estimate: ~90 changed lines (tests only)

## Requirements

1. **Pin the step-4 rationale tokens** (LOOP-SPEC.md): the pin asserts
   the presence, in §2 step 4's validator-budget text, of (i) the
   `max_iters: 60` + `max_minutes: 40` pair (already pinned — keep,
   do not duplicate), (ii) the census clause's stable core tokens —
   "4 of the last 4 validator children died at budget" — and (iii) the
   measure clause's stable core tokens — ">1 of the next 8 validator
   runs" AND "trimming default mutation-leg counts". Tokens are
   verbatim substrings; each token set is asserted in the SAME
   paragraph region as the 60/40 numbers (adjacency, not two disjoint
   file-wide greps — the T24 whole-file-grep non-localizing
   observation).
2. **Pin the META-SPEC §6 template tokens** (META-SPEC.md): the
   validator launch template's `--max-iters 60 --max-minutes 40`
   pair, asserted as one contiguous argv fragment (not two separate
   greps), in the file that carries the §6 template.
3. **RED-prove every new pin**: for each, the text-revert mutant
   (restore the pre-T149 text or delete the token) is shown RED on the
   pin, with the byte-restore verified (sha or `git diff` empty) —
   the T150-era RED-proof discipline.
4. **No doctrine text edits.** This row touches tests only; LOOP-SPEC.md
   and META-SPEC.md are byte-identical before/after (any wording drift
   the pin exposes is reported, not fixed — wording changes are T156's
   sibling class and would make this row doctrine-touching).

## Acceptance

- `cargo build`, `cargo clippy --all-targets -- -D warnings`, `cargo test`
  all green.
- RED proofs in the commit message: per-pin mutant → the specific pin
  leg that dies (test name), and the restore-verified line.
- The todo_consistency guard stays green (no TODO.md changes).

## Out of scope

- Pinning the T63-resume echo sentence's budgets (already covered by
  the numbers pin per T149's row notes — verify at implementation
  time; if NOT covered, add the one token leg and say so).
- Broader doctrine-text pinning sweeps (each eval names its own gaps).
- LOOP-SPEC/META-SPEC wording changes (see requirement 4).
