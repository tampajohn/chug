# Ledger

## Done
- Cycle-3179 Phase 1: routine-skip glm (predicate holds; eval fresh 899eae9a 2026-11-10T09:27; d1794287559-1) → T358, T353, T351 arcs worked solo-serial.
- T358 arc: merge 4c62e1ef, flip 11b78bd9; kimi PASS 5/5 (d1794291547-5). OSC 7501 stage/heartbeat + dot in loopd_osc7501_emit.
- T353 arc: merge 88ff8d3f, flip b5d74ba5; kimi PASS 6/6 (d1794297778-7). T261/T262 remediated in one loopd.sh dispatch-time gate.
- T351 arc: merge a7151a4f, flip f358569d; kimi PASS 6/6 (d1794300667-10). README install-first + generic example.
- Release v0.24.0 cut: notes v0.22.0..v0.23.0+ since v0.23.0 (5 items TRUE ≥ 3), commit 66a7cb05, tag annotated, pairing ok. Pushes pending.

## Next
- Wrap: Outcomes cycle-3179 entry + compact oldest; decisions-audit + T261 batch (9 records this cycle); eval-state rewrite; token-free notes + T257 color; probe; fill; push (releases + v0.24.0 tag + corridor); flips; goal_complete.
- Remove worktrees /tmp/chug-loop-t358, t353, t351 after final harvest verify (t353+judge events already harvested).
- Next eval weighs: T359-era LIVE_CTX remirror staleness (re-surfaced live this cycle), T349 features.md telemetry, T328 BLOCKED re-verify.

## Blockers
- T328 BLOCKED (operator decision pending, A/B/C menu unanswered) — re-verified BLOCKED in 3175, re-verify next cycle.
- ctx-edit compaction was REJECTED twice this cycle then landed on the 3rd edit after the T359 remirror restored stale turns (watch: LIVE_CTX write-path).
