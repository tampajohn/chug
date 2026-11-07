# Ledger

## Done
- Reconstructed: the dead cycle-3073 run died at budget-low MID-WRAP. Books found: eval e59f52b (the 739th, non-trip kimi) pushed; T329 arc CLOSED (merge 6e14a5b + flip/harvest 89b42bf + untrack 614f7a7, kimi PASS d1794022067-7); T330 arc OPEN (impl e599640 goal-accepted + review fix 1334efa on preserved worktree /tmp/chug-loop-t330, 4 nextest reds diagnosed, recovery recipe on the row); T328 assessed BLOCKED (chug-watch tree not on this host); T261 batch 8 backfills ALREADY APPENDED to decisions.jsonl (d1794028828-9..d1794028829-16); wrap-state note + TODO row updates written but UNCOMMITTED; no Outcomes entry, no state rewrite, no notes/probe/fill/push.

## Next (wrap-state note, T207 — cycle-3073 zombie repair)
- Merged-but-unflipped rows: NONE (T329 flipped 89b42bf). Unharvested worktrees: /tmp/chug-loop-t330 (PRESERVED BY DESIGN — recovery recipe on the row; impl stream 2 segments ride its .chug/events.jsonl). Unpushed commits: 0 at baseline (main == origin/main at 614f7a7); the wrap bookkeeping lands and pushes next. Missing decision records: NONE (the T261 batch landed — 8/8 backfills present).
- Wrap sequence: commit inherited TODO/LEDGER bookkeeping → Outcomes cycle-3073 entry + one-line 3058 (kept-full 6 = 3073/3072/3071/3070/3066/3062) → prep commit (eval: prefix) → T242 bg full gates at HEAD (T329 touched site-sync.sh + site_sync.rs — NOT docs-only) → eval-state rewrite (marker = prep commit; T309/T311 asserts) → token-free wrap-notes + T257 color + post-notes probe → fill (T309/T310/T311 asserts &&-chained pre-commit) → push.
- Then Phase 1 (routine glm skip — 4 open rows T328-T332, eval fresh, predicate HOLDS) → Phase 2 T330 recovery recipe → Phase 3 cycle-3074 wrap.

## Blockers
- none