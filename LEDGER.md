# Ledger

## Done
- Phase 1: routine-skip disposition recorded (d1794221603-1) — predicate HOLDS (todo rows T344 + eval fresh cf96126a same UTC day), Phase 2 entered directly.
- Phase 2: T344 worked (orchestrator-finish, routing d1794221687-2): probe chain 2026-11-09 exhaustively dead on all three resolution paths (npm stub claude-code@1.0.0 pointer-only; official @anthropic-ai/claude-code@2.1.319 tarball 7 files, no testing entry, no exports; installed 2.1.235 exports []; 313MB binary strings 0 hits x4 variants; `claude plugin` 14 subcommands, no test runner; 4 plugin caches markdown-only). The spec's operator-handoff branch landed: chug-watch 13eca32 docs/testing-harness-investigation.md (+95, ff-merged to master, no remote). TODO: T344 → done (flip a74b8591); T328 notes refreshed (operator decision menu A/B/C). todo_consistency 21/21 GREEN.

## Next
- Release v0.21.0 (trigger FIRES: 3 distinct flip-ids since v0.20.0 = T342+T343+T344, ancestor-OK; T342/T343 features → minor bump).
- Wrap corridor: Outcomes 3161 + compact 3155, T261 batch (2 routing ids), audit, eval-state rewrite keyed to prep, token-free notes + probe, fill with T309/T310/T311/T317/T321/T323 legs, push (commit + tag together), goal_complete.

## Blockers
- none (T328 stays blocked pending the OPERATOR's decision — menu in chug-watch docs/testing-harness-investigation.md; not a loop blocker)
