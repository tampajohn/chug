# T175 — LOOP-SPEC: overlap dispatch re-keys the spec's check-line CARGO_TARGET_DIR

check: export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared && cargo test --test loop_spec_recovery --test todo_consistency

## Repo context

LOOP-SPEC.md's Pipeline-overlap paragraph (T161) teaches: an impl child
launched INTO the 2-impl overlap swaps its goal's export for the
role-keyed slot (`target-shared-impl-a`, or `-impl-b` when impl-a is
held). It does NOT mention the SPEC's `check:` line — and spec check
lines hardcode a dir (e.g. specs/t162 names `target-shared`). Cycle 77
bit: t165's goal gate ran its check (naming the DEFAULT `target-shared`)
while T162's impl child actively built into that dir — cargo lock
contention plus the T52 same-artifact-name class (a gate can execute a
binary compiled from a foreign checkout's source) rejected the child's
goal on green work; fixed mid-flight by role-keying the check to
`target-shared-impl-a` (commit 607e877, routing d1790763842-1).
T144's scrub (src/tools.rs:1028-1099) makes a bare `cargo` content-correct
but cold; explicit exports keep the gate warm but must name the child's
assigned slot. EVALUATION.md §2.3 (cycle 79) weighed the structural
alternative (driver-injected per-worktree check dir) and REJECTED it —
this row is the proportionate doctrine fix. Pins live in
tests/loop_spec_recovery.rs (the T63/recovery paragraph pins) and the
loop_spec_* family conventions.

estimate: ~150 changed lines (one LOOP-SPEC.md sentence + one pin leg /
pin-file update — the cycle-79 eval's sentence+pin-file calibration).

## Requirements

1. LOOP-SPEC.md's Pipeline-overlap paragraph (the T161 sentence naming
   `target-shared-impl-a` / `-impl-b`) gains ONE sentence: at dispatch
   into the overlap, the orchestrator ALSO rewrites the spec's `check:`
   line export to the SAME role-keyed slot before launch (sed + grep
   verify per the sed-assert doctrine; commit the re-keyed spec on the
   branch) — the goal's export and the check's export must always name
   one dir, because T144's scrub means the check's own export is the
   only target dir the goal gate sees, and the T52 artifact-name class
   makes a foreign dir a correctness hazard, not only contention. The
   cycle-77 bite (t165's rejection + 607e877) is named as the evidence.
2. The solo default is unchanged: spec authors keep writing check lines
   against the DEFAULT `target-shared`; the re-key is a dispatch-time
   orchestrator act, only when slotting a child into impl-a/impl-b.
3. One pin leg (a new test in tests/loop_spec_recovery.rs or the nearest
   loop_spec_* file, family conventions): the sentence's load-bearing
   tokens (re-key the check line / same role-keyed slot / goal gate)
   appear exactly once in the overlap paragraph's window — non-vacuous
   (the pin fails if the sentence is deleted or moved out of the
   paragraph).
4. No step renumbering, no other paragraph touched; META-SPEC.md,
   META-META-SPEC.md untouched.

## Tests

`cargo test --test loop_spec_recovery --test todo_consistency` green;
RED-prove the new pin leg by deleting the sentence (fails) and
duplicating it (fails) — state the proofs in the commit message.

## Acceptance

- Spec check green; full `cargo test` green; clippy `-D` clean.
- The overlap paragraph names the check-line re-key; solo-dispatch
  doctrine unchanged.

## Out of scope

- Structural check-env changes (driver-side CARGO_TARGET_DIR injection —
  weighed and rejected at the cycle-79 eval), spec-author-side re-keying
  (specs keep the default dir), T173/T178 budget rows, any src/ code.
