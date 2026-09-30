# T176 — LOOP-SPEC impl goal template: clippy runs with `-D warnings`

check: export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared && cargo test --test loop_spec_recovery --test todo_consistency

## Repo context

LOOP-SPEC.md §2 step 2's impl-child goal template says "Keep cargo build +
clippy + test green" — without `-D warnings`, so an impl child can
honestly claim clippy-green while warnings stand. Cycle 77 bit: the T166
impl committed with a `needless_lifetimes` warning and a "clippy clean"
claim; the kimi validator caught it (finding 1) and the fix landed
on-branch (7911b3c) — validator time spent on a mechanical nit the
child's own gate should have caught. The validator-side gates and the
orchestrator's post-merge gates already run clippy with `-D` clean; only
the child's self-check bar is unspecified. The cycle-79 eval
(EVALUATION.md §2.7(g)) weighed a lint-sweep row and rejected it — this
one-word template fix is the prevention half.

estimate: ~120 changed lines (one template sentence + one pin leg — the
cycle-79 eval's sentence+pin-file calibration).

## Requirements

1. LOOP-SPEC.md §2 step 2's goal template: "Keep cargo build +
   clippy + test green" becomes "Keep cargo build + clippy
   `-D warnings` + test green" (the child runs
   `cargo clippy --all-targets -- -D warnings`, zero warnings, not merely
   exit-0 clippy). The cycle-77 T166 instance is named as the evidence
   in one clause.
2. If the fix-up-child goal template (step 4's FAIL arc) or any other
   LOOP-SPEC goal template carries the same bare "clippy green" phrase,
   update it identically and name every surface in the commit message
   (the sweep-the-family doctrine — grep for the phrase).
3. One pin leg (loop_spec_* family conventions): the template window
   carries `-D warnings` exactly once per template surface — non-vacuous
   (fails on revert).
4. No other LOOP-SPEC change; no src/ code; META-SPEC.md untouched
   (§6's validator template already runs -D).

## Tests

`cargo test --test loop_spec_recovery --test todo_consistency` green;
RED-prove the pin by reverting the template sentence — state the proof
in the commit message.

## Acceptance

- Spec check green; full `cargo test` green; clippy `-D` clean.
- `grep -n "clippy" LOOP-SPEC.md` shows `-D warnings` at every impl/fixup
  goal-template surface.

## Out of scope

- Lint sweeps of src/ (no evidence of a wider class), validator-side
  gates (already -D), any code change.
