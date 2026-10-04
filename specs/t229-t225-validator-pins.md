# T229 — close the T225 validator's pin-strength findings (tests-only)

check: set -o pipefail; export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-validate-a; touch src/*.rs tests/*.rs; cargo test --bin chug

estimate: ~60 changed lines (two pin legs, RED-proof each; tests-only)

## Concern

T225's kimi verdict (PASS — `.chug/verdict-t225-validate-20261004.md`)
left two predicted survivors behind (the T224 pattern — file them
forward):

- **M7 (finding 1)**: `surface_fingerprint`'s MTIME arm is unpinned — a
  fingerprint keyed on LENGTH ONLY passes all 6 t225 legs. Benign today
  (every adopted surface is length-monotonic: append-only supervisor
  logs, appended events.jsonl, one-shot env dumps) but a length-only
  fingerprint is one refactor from silently never resetting.
- **BACKSTOP_FACTOR upper bound (finding 2)**: the constant is pinned
  against REDUCTION (M8 died RED) but an INCREASE (4→N) survives —
  the pure legs pin the backstop STRUCTURE, not the constant's value.

## Requirements

1. **Mtime-arm pin**: a same-length-rewrite leg — write a surface file,
   capture the fingerprint, REWRITE it with different bytes of the SAME
   length (mtime advances), assert the fingerprint CHANGED (kills a
   len-only fingerprint). If the implementation's fingerprint is
   intentionally len-only for one-shot surfaces, pin the documented
   choice with a comment instead and say so in the commit.
2. **BACKSTOP_FACTOR pin**: pin the constant's VALUE (4) via the pure
   seam — a leg whose arithmetic fails if the factor moves in EITHER
   direction (e.g. backstop fires in the expected window for a fixed
   synthetic load, and a named `BACKSTOP_FACTOR == 4` assertion through
   the pure path — not a literal grep).
3. Sweep-the-family (T69): each pin RED-proven against its named mutant
   (len-only fingerprint; factor 4→8) in the worktree, reverted
   byte-clean (shasum-verified), mutant name → red test name in the
   commit message.
4. Tests-only: production diff must be EMPTY (testsupport.rs is
   cfg(test)-gated test support; the T189 gates-only lane expected).

## Acceptance

- cargo test --bin chug t225 green (all prior legs + the new pins);
  clippy --all-targets -D warnings clean.
- RED-proofs recorded in the commit message.

## Out of scope

- Re-opening the ProgressDeadline design or the fail-safe trade-off
  (verdict finding 4 — deliberate, spec'd).
- Any new adoption surface.
