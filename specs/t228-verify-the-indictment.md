# T228 — META-META-SPEC: verify the indictment before filing a bug row (doctrine, SOLO)

check: set -o pipefail; export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-validate-a; touch src/*.rs tests/*.rs; cargo test --test loop_spec_recovery

estimate: ~50 changed lines (one doctrine clause + one carrier pin leg)

## Concern

The cycle-98 eval filed T212 on a FALSE INDICTMENT: "the T197 drift
advisory false-positives on a correctly pre-keyed branch" — asserted
from secondhand Outcomes text without reading the code path. The WARNs
were TRUE positives: the goal gate reads the spec ARG path every
iteration (`src/driver.rs:1171`), so the on-branch T175 re-keys never
reached it. The filing's premise directed round 1's fix (874c620 —
"fix" the truthful advisory), the round-1 kimi validator FAILed it with
three-way proof (`main.rs:87`, `driver.rs:462/1171/1556`, the t209/t210
verifying events running the un-keyed check), and the entire round was
reverted sha256-byte-identical before the row was re-scoped (the t212
spec's "CORRECTED PREMISE" section is the record). Cost: one full impl
round (70/80) + one validation round (48/60) + the revert — the most
expensive eval-quality defect in the corpus.

META-META-SPEC's filing bar already says "Verify T1/T2/T4/T5 actually
worked before filing anything adjacent (read the code, run the relevant
tests if cheap)" — that clause covers fix-ADJACENCY, not a fresh bug row
whose premise indicts a specific component.

## Requirements

1. META-META-SPEC's "Extend TODO.md" section gains the clause: a bug row
   whose premise INDICTS a specific component (X is broken /
   false-positive / dead code) MUST be verified against the code before
   filing — read the component, run the cheap repro when one exists;
   when verification is not feasible at eval time, the spec files the
   SYMPTOM + evidence and labels the mechanism a HYPOTHESIS in
   repo-context, never the row's premise (cite the T212 lesson: the
   indictment was inverted, a full round reverted).
2. One carrier pin leg in tests/loop_spec_recovery.rs (the META-META
   window — the file already reads META-META-SPEC.md at ~line 660):
   asserts the clause exists inside the Extend-TODO.md window, exactly
   once. RED-proof: delete the clause → the pin goes RED; restore
   byte-clean.
3. Doctrine row → SOLO dispatch (no other child in flight), kimi
   REQUIRED validation.

## Acceptance

- cargo test --test loop_spec_recovery green; clippy -D warnings clean.
- The RED-proof recorded in the commit message.

## Out of scope

- Editing the estimate/threshold numbers (standing doctrine: calibration
  lives in eval text, never edited in passing).
- Any LOOP-SPEC change (T227 carries that file this cycle).
