# Ledger

## Done
- Phase 1: routine freshness-skip routed (d1794155953-1) — predicate HOLDS on the todo-rows half (T328 open) + eval fresh same-UTC-day (d5c45e3c, 2026-11-08); STATE-HIT read-path; boot probe 60 0 machine==TRUE 0
- Phase 2: T328 re-verified BLOCKED external (~/workspace/chug-watch absent on host, ls fallback 16:39Z); P001/P003/P004 parked-operator lanes untouched — zero items dispatchable
- Wrap: battery v1 mis-filtered (test() vs binary() — 3/1942 matched, self-caught, superseded), v2 relaunched (bg pid 54700); Outcomes 3127 + one-line 3121; T261 batch; state splice; notes/fill/push pending

## Next
- Finish the cycle-3127 wrap corridor (battery green → prep → state → notes → probe → fill → push → goal_complete)

## Blockers
- none

<!-- T207 wrap-state note (cycle 3127, written at the stop-dispatch boundary) -->
## Wrap-state note (cycle 3127 — T207)

- Merged-but-unflipped rows: NONE (zero items landed — T328 re-verified BLOCKED external this cycle; the row stays open-blocked by its own recipe).
- Unharvested worktrees: NONE (`git worktree list` clean at main only; zero children this cycle — harvest NO-OP).
- Unpushed commits: 0 at boot (569ce1ff == origin/main); the corridor commits land and push at the wrap.
- Missing decision records: the routing d1794155953-1 recorded at 16:39Z; its T261 outcome backfill batches at THIS wrap (one append, DISK-ONLY per the 3078 correction).
- Gates: the pins battery relaunched in a T242 bg window (pid 54700, /tmp/eval-3127-pins-v2.log, target-shared-main; the v1 launch pid 53024 mis-filtered — test() keyed test-function names, matched 3/1942, self-caught and superseded by the binary() v2); result verified before the notes commit.
- Dirty-state note (T251): the boot-time dirty LEDGER.md was the fresh-run reseed template (byte-identical to the system-prompt template; the committed 569ce1ff:LEDGER.md wrap-state note lives in the 3126 wrap-notes record) — disposable, now overwritten by THIS wrap-state note.
