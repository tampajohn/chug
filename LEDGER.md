# Ledger

## Wrap-state note (T207, written at the stop-dispatch boundary — cycle 3147)

- **Merged-but-unflipped rows:** none (zero items this cycle — T328 re-verified BLOCKED external; nothing dispatched, nothing merged).
- **Unharvested worktrees:** none (`git worktree list` = main only; harvest NO-OP).
- **Unpushed commits:** 0 at boot (71e711e0 == origin/main); the corridor commits land and push this wrap.
- **Missing decision records:** none outstanding — the routing d1794177197-1 is recorded; its T261 outcome backfill lands at the wrap batch per the standing shape.
- **Bg windows in flight:** gates pid 29061 (/tmp/wrap-gates-3147.log — clippy + nextest at target-shared-main, HEAD 71e711e0) + pins battery pid 29080 (/tmp/eval-3147-pins.log, the canonical 12-target battery); battery waits on the cargo lock by design.

## Done
- (nothing yet)

## Next
- Read the spec

## Blockers
- none
