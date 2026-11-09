# Ledger

## Done
- T342 arc LANDED + PUSHED (merge e19cfde6 + remedy 47406e10 + README 37c1e4e2 + flip 76cab715; kimi PASS d1794211960-4; harvest byte-verified; worktree removed)
- T343 arc LANDED + PUSHED (merge 65ee407f + README + flip 010145fa; main == origin/main): impl glm 80/80 died uncommitted → T63 resume d1794213527-6 accepted 56; kimi PASS d1794215189-8 (7/7 mutants); gates 2046/2046 + clippy ZERO post-merge; harvest byte-verified (events-t343-impl runs:2 + events-t343-validate + verdict + LEDGER-t343-validate)

## Wrap-state (T207 — crossed the stop-dispatch boundary)
- Merged-but-unflipped rows: NONE (T342+T343 both flipped+pushed)
- Unharvested worktrees: NONE (t342 removed; t343 harvest done, removal next)
- Unpushed commits: 0 (main == origin/main 010145fa)
- Missing decision records: NONE (8 landed: d1794207660-1, d1794209590-2, d1794211960-3, d1794212248-4, d1794212606-5, d1794213527-6, d1794215189-7, d1794216762-8 — outcome backfills batched at the wrap)
- Release state: v0.20.0 tagged+pushed cycle-3156; this cycle 2 distinct flip-ids (T342+T343) < 3 → NO TAG expected

## Next
- Remove /tmp/chug-loop-t343 (zombie-exact pids 88630/90993/52982) + delete branch
- Release recount (mechanical, v0.20.0 ancestry) → NO TAG
- T261 batch (8 outcome records, ONE pass) + decisions-audit REPORT
- Outcomes 3157 + compact 3150 one-line; eval-state rewrite (T311 FULL, validate_state GREEN)
- token-free eval: notes + T257 color + post-commit sleep-ok probe; fill (T309/T310/T311/T317/T321/T323 asserts); push; goal_complete

## Blockers
- T328 BLOCKED (parked); T344 UNBLOCKABLE (same root)
