# T168 — decision_log tool description names its required fields

check: export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared && cargo test --bin chug decision

## Repo context

The `decision_log` tool schema/description is built in `src/tools.rs`
(tool_schemas(); the decisions module is src/decisions.rs). T88 landed
corrective validation errors (`options must be a string, got missing`
etc.). Description-text pins follow the T22 precedent: assert the LIVE
tool_schemas() output (needle-in-context), not a copied literal.

estimate: ~25 changed lines (one description sentence + one pin).

## Evidence

Cycle-76 eval §2.3: `invalid decision_log call: options must be a string,
got missing` fired in 6 of 10 glm streams this delta (orchestrators ×6,
children ×3) plus `choice ... got missing` ×2 — T88's corrective errors
self-correct in one iteration, but the tax recurs every stream. The tool
description — the surface glm actually reads (the T22 bash-description
lesson) — never says the fields are required.

## Requirements

1. The `decision_log` tool description gains ONE sentence naming the
   required fields explicitly: every call must include `class`, `subject`,
   `inputs`, `options`, `choice`, and `confidence` — all strings except
   `confidence` (0..=1) — with no field omitted (match the real schema;
   read it first).
2. One pin test (bin tests, T22 precedent): the LIVE tool_schemas()
   description for decision_log contains the required-fields sentence's
   load-bearing tokens (`options`, `choice`, `required` or equivalent) —
   RED-proven by reverting the sentence (stated in the commit message).

## Tests

The description pin; existing decisions/tests stay green.

## Acceptance

- The spec check green (verify the filter catches the new pin); full
  `cargo test` green; clippy `-D` clean.

## Out of scope

- Schema shape changes (required-array edits); any glm-specific behavior
  branching.
