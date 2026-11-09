# Ledger

## Done
- Phase 1 skipped per T81 (glm routine; routing d1794248276-1)
- T349 arc DONE+PUSHED: resume → review fix 62c47d60 → kimi PASS d1794249438-3 → merge 52ff78e1 → flip 383e2b40 (harvest 3 artifacts disk-only)
- T355 arc DONE+PUSHED: transport-death resume → kimi PASS d1794249931-4 → merge 39abe72b → flip 5a09b9a0 → post-merge gates GREEN 2093/2093; operator docs 4fb53b2b rode along
- T352 arc: budget-death resume (d1794254673-7) pid 55774 ACCEPTED 72/80 → review caught 5 reds the child skipped (clippy nits 7de7c892; loader CHUG_LANGFUSE_* wildcard clobbered + README layout line 52d36e20) → worktree gates GREEN clippy ZERO + 2127/2127 → kimi REQUIRED d1794258303-8 (loopd.sh doctrine carrier + new CLI surface + 2,369 lines) → validator pid 40507 FLYING (validate-a)
- T350 arc: budget-death resume (d1794254673-7) pid 55827 ACCEPTED 27/80 → worktree gates GREEN clippy ZERO + 2094/2094 → kimi REQUIRED d1794259493-9 (check-line flip, ~200 lines) → validator pid 71028 FLYING (validate-b, pattern iv legal)
- LIVE_CTX compaction attempt REJECTED (5th driver-side refusal; file grew via re-escaping) — per standing discipline NO RETRY; file disk-only never pushed; driver's own trim supersedes

## Next
- Poll both validators → verdicts (validation-verdict records) → harvest t352/t350 → serial merges (T352 FIRST, then T350) → T242 bg post-merge gates per merge → flips (T352 ref 9c52b79d+fixes, T350 ref 25970392) → push
- Budget check: if <30 iters left → wrap instead of T353/T351/T354/T356 (they stay `open`, specs ready)
- Wrap: T261 batch (d1794248276-1, d1794249054-2, d1794249438-3, d1794249931-4, d1794252043-5, d1794252276-6, d1794254673-7, d1794258303-8, d1794259493-9 + verdicts), Outcomes (4 items this cycle), release recount (T349+T355 pushed + T352/T350 pending — ≥3 since v0.22.0 → TAG LIKELY), state rewrite, T237 probe (worked wrap → token-free per T248), push, goal_complete
- T352 doctrine-note: spec's item-3 loopd integration wasn't in the named-targets list — loopd.sh carrier ran overlapped (disjoint files, serial merges); no further dispatches until this arc resolves; next eval weighs a named-targets-honesty row

## Blockers
- T328 BLOCKED (operator A/B/C menu unanswered) — re-verify at wrap
- LIVE_CTX: NO RETRY (5 refusals) — driver's own trim supersedes
