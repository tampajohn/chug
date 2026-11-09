# Ledger

## Done
- Phase 1: routine-skip logged (eval fresh 13847880, same UTC day 09:57Z; todo_rows=2) → glm-routed; eval-routing record d1794218347-1
- Phase 2 disposition: T344 re-probed — all three resolution paths still closed (installed @anthropic-ai/claude-code 2.1.235 exports [], npm latest 2.1.319 has no exports field, `claude plugin` has no test subcommand, chug-watch has no package.json) → dispatch refused (the cycle-3155 shape d1794189727-2); T328 re-verified BLOCKED same-root; ZERO dispatchable rows
- T207 wrap-state note: zero-item corridor — no merged-but-unflipped rows, no worktrees (main-only), no unpushed commits, missing decision records: 3 T261 backfills owed at the batch (d1794218347-1 this-cycle + the inherited t339 pair d1794192449-3/-4)

## Next
- Wrap cycle-3158: Outcomes entry + compact 3151 → prep commit → eval-state rewrite → token-free notes + T257 color → probe → fill (T309/T310/T321/T323/T311 legs) → push → flips → goal_complete

## Blockers
- T328/T344: no chug-watch host-runnable test harness on this host (npm pointer stub, no testing export, no plugin test subcommand, no package.json in chug-watch) — the RED-proven unblock probe lives in specs/t344-chugwatch-host-test-harness.md; decision rests with the operator per the spec's hand-off clause
