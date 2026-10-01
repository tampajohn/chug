# T182 — decision_log validation errors always carry the full contract

check: export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-impl-a && cargo test --bin chug decision_log

## Repo context

Cycle-83 eval §3: T168's success criterion FAILED — glm's
`options must be a string, got missing` corrective-error class was
supposed to go to zero and instead fired FIVE times in the delta
(cycle-80 orchestrator ×2, t169-impl, t173-impl, t179-impl). Root
cause found by code read: `validation_message` (src/decisions.rs:285)
appends `CONTRACT_REMINDER` ("one record per call; required: class,
subject, inputs, options, choice, confidence") ONLY when the input
has NO required key or HAS an unrecognized key
(`!has_required || has_unrecognized`). The exact recurring fumble
shape — glm sends 5 of the 6 required fields, all recognized —
takes the legs-only path: the error names the ONE missing field but
never re-states the full contract, so the retry can drop a
DIFFERENT field (the ×2 in the cycle-80 stream). T88 built the
reminder, T168 the description; the gap is the conditional.

estimate: ~30 changed lines (one condition flip + comment + test-leg
updates/additions in decisions.rs's test module).

## Requirements

1. `validation_message` ALWAYS appends `; {CONTRACT_REMINDER}` to the
   error, for every validation-failure shape. The `received keys:
   [...]` list stays conditional on `has_unrecognized` (it exists for
   the unknown-keys diagnostic; a merely-missing-field call needs the
   contract, not its own keys echoed — but if the impl judges
   received-keys useful in the missing-field case too, keeping it is
   acceptable; the contract reminder is the load-bearing change).
2. Update the const-adjacent comment (the T88 req-4 comment above
   `validation_message`) to state the new invariant: every failure
   message carries the full required-field list because the
   5-of-6-fields fumble shape previously bypassed it (name the
   cycle-83 census: 5 instances in 3 cycles).
3. The success path stays byte-identical (validation failures never
   write — the existing `decision_log_validation_legs_name_the_field`
   no-write assertion stands); only failure message text changes.

## Tests

- New leg: input with exactly 5 of 6 required fields (all recognized,
  `options` missing) → error contains BOTH `options must be a string,
  got missing` AND the full `required: class, subject, inputs,
  options, choice, confidence` reminder.
- Existing legs updated where they assert exact message text
  (decisions.rs tests at :809/:833 assert multi-leg messages — extend
  them to the always-reminder shape in the same commit).
- RED-proof: revert req 1 (restore the conditional) → the new leg
  fails; record in the commit message.

## Acceptance

- `cargo test --bin chug decision_log` green; clippy `-D warnings`
  clean. One commit; no TODO.md/LEDGER.md edits.

## Out of scope

- Schema/description changes (T168's surface stands), any relaxation
  of the required-fields rule, example-payload additions (weighed at
  the eval: the contract list + the field-naming leg is the
  proportionate fix; an example JSON is the next lever ONLY if the
  fumble class survives this fix — measure next delta).
