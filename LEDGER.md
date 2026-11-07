# Ledger

## Done
- (cycle start 2026-11-07T18:49:36Z, glm routine cycle, head a11e5e9)

## Next
- Phase 1: SKIPPED per routing (todo_rows=1 eval_fresh=yes -> routine glm; predicate holds, delta STATE-HIT bookkeeping-only) — record eval-routing
- Phase 2: P002 dispatch (the ONLY loop-workable row) — spec filed (specs/proposed/f16-unpark-decision.md ## Dispatch): docs/f16-unpark-memo.md, XS docs-only. Worktree /tmp/chug-loop-p002 -> warm build -> glm impl child -> review/T80 floor -> merge -> P-row note (operator pick pending)
- T328 re-verified BLOCKED (2026-11-07T18:50Z): /Users/jadams/workspace/chug-watch does NOT exist on host; blocker text still true
- P001/P003/P004 NOT dispatchable: operator-presence work (P001 3 operator steps on K7; P003 spark GPU headroom; P004 operator decision + chug-watch tree absent)
- Phase 3: wrap — T207 note, T80 docs-only floor if P002 is the only diff, T261 batch, release recount (TRUE count since v0.19.0), token-free notes + T257 color, probe, fill, push, goal_complete

## Blockers
- none (T328 external: chug-watch tree not on host, redispatch when reachable)
