# T210 — launch-template placeholder guard (glm literalized `<N>` 4×)

check: export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-validate-a; touch src/*.rs tests/*.rs; cargo test --test todo_consistency

## Repo context

Doctrine row — LOOP-SPEC.md Phase-2 step-2 launch template edit; runs SOLO.

Cycle-94 seg-3 (glm orchestrator, `.chug/events-20261003-044541.jsonl`,
failed-tool classes): FOUR placeholder-literalization incidents in one
stream —
`tool error: delegate: spec does not exist or is not a readable file:
/Users/jadams/workspace/chug/specs/tN-dae…` (the delegate launch carried a
literal `tN` spec path),
`tool error: path escapes cwd: /tmp/chug-loop-tN/README.md` and
`…/loopd.sh` (literal `chug-loop-tN` paths), and a garbled commit attempt
`Nfe TN CHILD A: …`. glm followed the step-2 template's `<N>` placeholders
literally instead of substituting the row number. Each incident cost
tool-round trips inside a 200-iteration budget that later ran out
unwrapped.

estimate: ~30 changed lines (LOOP-SPEC.md step 2 + one pin leg in
tests/todo_consistency.rs or a loop_spec_* file).

## Requirements

1. The step-2 launch template's first `<N>` occurrence gains an inline
   marker sentence: every `<N>` in the template must be substituted with
   the real row number before dispatch — a literal `<N>`, `t<N>`, or
   `chug-loop-tN` in a launched goal, delegate spec path, or bash command
   is a dispatch defect (the cycle-94 glm evidence named).
2. The step-2 polling/review text (or the template, editor's pick) gains a
   pre-launch checklist line: after rendering the goal, grep it (and the
   `cwd`/`spec` arguments) for `<N>` / `tN` placeholders BEFORE the
   delegate call — one cheap look, not a tool.
3. Pin the marker sentence (not the absence of `<N>` — the template itself
   must keep its placeholders): a test leg asserts the marker exists in
   LOOP-SPEC.md and dies when the sentence is deleted.

## Tests

The req-3 pin (RED-proven by deletion).

## Acceptance

- LOOP-SPEC.md carries the marker + the pre-launch checklist line.
- The pin passes; it dies on the deletion mutant.
- `cargo test --test todo_consistency` green (the check line).
