# Ledger

## Done
- Routing + batch landed: Phase-1 routine-skip glm d1794315571-1 (predicate held both halves — T328 open + eval 39d217d0 same-UTC-day STATE-HIT, evals-behind 0; loopd 12:58:10Z) + queue disposition d1794315574-2 (T328 BLOCKED re-verified: chug-watch HEAD 13eca32 unchanged, row shasum e9512f49 byte-unchanged since 6dc7a428; P-rows parked, zero dispatch) + T261 batch TWO d1794315577-3/-4 (both landed-clean, first attempt, no malformed fire).

## Next (T207 wrap-state note — cycle-3189, written BEFORE wrap duties)
- Merged-but-unflipped rows: NONE (zero items this cycle; TODO.md unchanged — T328 stays open BLOCKED).
- Unharvested worktrees: NONE (zero children dispatched; no /tmp/chug-loop-* created).
- Unpushed commits: corridor pending (prep + notes + fill to come).
- Missing decision records: NONE — routing 2 + outcomes 2 all recorded (ids d1794315571-1/-2 + -3/-4).
- Wrap duties: Outcomes 3189 entry + compact 3183 → prep commit → state rewrite keyed to prep (T311/T312) → token-free notes → probe → fill (assert legs) → push → release recount (expect NO TAG, TRUE 1 = T357) → close-out → goal_complete.
- Gates: T242 bg window pid 47546 (guard floor todo_consistency + battery 225, target-shared-main, log /tmp/wrap-gates-3189-080112.log).

## Blockers
- T328 BLOCKED (operator decision pending, A/B/C menu unanswered — chug-watch HEAD 13eca32 unchanged; re-verified THIS cycle).
- ctx-edit watch: LIVE_CTX write-path staleness rides (no ctx-edit consumed this cycle, under the 3-cap).
