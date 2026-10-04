# T224 — close the T222 validator's pin-strength findings (tests-only)

check: set -o pipefail; export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared; touch src/*.rs tests/*.rs; cargo test --test kev_loader

estimate: ~120 lines (three pin families, no production diff)

## Concern

T222's kimi verdict (PASS — `.chug/verdict-t222-validate-20261004.md`)
predicted three surviving mutants via static analysis, budget-truncated
before execution. The implementation verified CORRECT; the PINS are the
gap:

- m8: `daemon.rs` kev-routing (`real_backend` Dir arm) is deletable —
  every daemon lifecycle test uses `CHUG_DAEMON_STUB` and
  `env_remove(CHUG_LAYA_CHECKPOINT)`, so no test exercises the routing
  end-to-end.
- m9: the harness AUROC midrank tie-handling is value-identical to plain
  ranks on the committed corpus (no ties) — untested.
- m12: `parse_record`'s refusal legs (bad probs sum, label out of range,
  non-permutation flipped options) have no negative test.

The cycle-33 lesson (T69): sweep the WHOLE family in one row, one
RED-proven killing test each — not one leg at a time.

## Requirements

1. Daemon-routing Dir-arm pin (feature-gated): point
   `CHUG_LAYA_CHECKPOINT` at a kev-layout tempdir (T222's fixture shape —
   adapter config + provenance + head, no weights needed; the classified
   refusal fires BEFORE any big download) and assert the classified kev
   refusal surfaces (daemon start fails with it or /judge returns it) —
   kills m8 with no network and no real weights.
2. Tie-handling AUROC pin: a corpus with tied noul scores where midrank
   and plain-rank diverge; pin the midrank value exactly — kills m9.
3. Negative-path pins for `parse_record`: each refusal leg (bad probs
   sum, label out of range, non-permutation flip) named-error refused —
   kills m12.
4. Tests-only row: production diff must be empty (T189 gates-only lane
   expected; RED-proof each pin by applying its named mutant in a
   worktree and watching its test go red, then reverting byte-clean).

## Out of scope

- The gated-DeltaNet port (T223's first blocker).
- Any new refusal behavior or wire change.
