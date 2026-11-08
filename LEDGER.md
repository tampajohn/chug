# Ledger

## Done
- Phase 1 ROUTING (cycle 3141): loopd 20:46:03Z `todo_rows=1 eval_fresh=yes -> routine cycle on glm` — predicate HOLDS on the todo-rows half, eval fresh (0ab5966f, same UTC day), delta bookkeeping-only → Phase 1 SKIPPED per the routing switch (glm never evaluates); routing record d1794171023-1. T328 re-verified BLOCKED external LIVE (~/workspace/chug-watch ABSENT at the ls probe; ssh chug-watch connect-timeout). Single-driver verified (ps census = THIS run only, pid 35860, glm-5-3-flash). Boot: HEAD 0ab5966f == origin/main, 0 unpushed; delta STATE-HIT (non-trip); describe v0.19.0-241-g0ab5966f.
- T207 WRAP-STATE (cycle 3141, pre-corridor): merged-but-unflipped rows NONE (T328 stays open-blocked, no merge this cycle); unharvested worktrees NONE (main only at boot — re-verify at harvest time); unpushed commits 0 at boot (0ab5966f == origin/main); missing decision records ONE (the routing d1794171023-1 — its outcome backfill rides the T261 batch at this wrap per the batching doctrine).

## Next
- Phase 2 disposition (T328 open-blocked, nothing dispatchable) → Phase 3 wrap: T207 note → battery bg (T242) → Outcomes 3141 + one-line 3135 → audit + T261 batch → release recount → prep commit → state splice rewrite → probe → notes → fill → push → flips → goal_complete.

## Blockers
- T328 blocked (chug-watch tree not on host — fleet-side; P001/P003/P004 operator-parked)
