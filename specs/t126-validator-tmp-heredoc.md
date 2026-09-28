# T126 — META-SPEC §6 validator template: write /tmp helper scripts via bash heredoc

check: cargo test --test loop_spec_recovery

## Repo context

META-SPEC §6's validator goal template already teaches the
cross-tree READ rule ("Read worktree files via bash —
read_file/grep/glob/list_dir/edit_file are cwd-confined and refuse
cross-tree paths with `path escapes cwd`; cross-tree reads go through
bash") but says nothing about WRITES. Validators keep discovering the
write half live: the cycle-61/62 streams carry TEN
`path escapes cwd … cross-tree paths go through bash` tool errors
across three validators, all on the same shape — writing the mutation
helper script to /tmp with write_file/edit_file:

- t112-validate ×7 (`/tmp/t112-mutate-*.py`, `/tmp/t112-leg.sh`)
- t113-validate ×2 (`/tmp/chug-mut-apply.py`, `/tmp/chug-mut-t113-run.sh`)
- t115-validate ×1 (`/tmp/t115-mut-leg.sh`)

T79's parallel-mutant legs (routine since cycle 61) multiply exactly
this write — one apply/run script pair per leg, cap 3 — so the class
is growing, not shrinking (the cycle-62 eval weighed it at 6 fires
and rejected; the T79 datum is new evidence, and the rejection is
superseded, not ignored). Each fire costs ~1 validator iteration
(self-corrects to a bash heredoc) — small, but the fix is one
sentence in the template every validator already reads.

Doctrine item: touches META-SPEC.md + its pin carrier
tests/loop_spec_recovery.rs ONLY. Runs ALONE; kimi REQUIRED
(loop/spec doctrine).

estimate: ~25 changed lines (≈5 doctrine + ≈20 pin loader + leg)

## Requirements

1. META-SPEC §6's goal template, immediately after the existing
   cross-tree-reads sentence (extending the same guidance, where the
   validator reads it), gains the write half — one sentence to the
   effect of: write /tmp helper scripts (the mutant apply/run legs)
   with bash heredocs; write_file and edit_file are cwd-confined the
   same way and refuse /tmp paths with `path escapes cwd`. The
   existing read sentence stays byte-identical.
2. `tests/loop_spec_recovery.rs` gains a `meta_spec()` loader (the
   `meta_meta_spec()` pattern, reading META-SPEC.md from the runtime
   checkout) and ONE new pin leg — next letter in the leg series —
   asserting the new heredoc sentence's load-bearing tokens appear
   EXACTLY ONCE in META-SPEC.md (stable needles: `heredoc` and the
   `/tmp` helper-scripts clause as written). RED-prove the leg
   against the pre-edit template.
3. No other META-SPEC content changes: the §6 budgets, model, verdict
   format, T79 parallel-mutant mandate, and every other sentence
   byte-identical. LOOP-SPEC's §6-override paragraph is NOT edited
   (it overrides launch mechanics only; the template text lives in
   META-SPEC).

## Tests

The new loader + pin leg are the test change; the full
`cargo test --test loop_spec_recovery` suite is the regression set
(it already carries LOOP-SPEC and META-META-SPEC pins; this is the
first META-SPEC needle — the loader pattern is identical).

## Acceptance

- `cargo test --test loop_spec_recovery` green; the new leg
  RED-proven against the old template.
- The sentence sits adjacent to the existing cross-tree-reads
  sentence in the §6 goal text.
- `cargo clippy --all-targets -- -D warnings` clean.

## Out of scope

LOOP-SPEC edits; changing §6 budgets/model/verdict shape; any
attempt to make the tools themselves accept /tmp (the sandbox is
working as designed — this row teaches the remedy, it does not move
the boundary); retro-fixing validator ledgers.
