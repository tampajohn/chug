# Ledger

## Done
- Cycle-3209 the DORMANT WEEKLY AUDIT (T325, the dormant era's FIRST): timer fired 605,834s >= 604,800 since git anchor 577ea4f0; routing d1795024352-1 (kimi, DORMANT=1).
- Audit legs ALL run READ-ONLY: gates GREEN at HEAD 0ba24318 (T242 bg pid 89001: build 7.87s + clippy -D warnings ZERO + nextest 2289/2289 in 137.166s); pins verified (canonical 12-target battery 225/225 in 17.834s, composition 33+30+29+28+27+21+17+12+10+9+8+1 = 225 re-derived; python battery 225/225 = 198 wrap_assert + 27 todo_flip); daemon healthz OK (rl-agent laya warm); Langfuse /api/public/health OK (v3.225.5); site reachable (chug.sh + /docs.html 200).
- ONE observed breakage filed per the audit doctrine: the site's latest-release panel v0.26.0 STALE vs v0.26.1 pushed 2026-11-11T12:41:55-05:00 (the T361 flush pushes the tag but never runs site-sync.sh; the sync rides only the launched-cycle-OK path dormancy suppresses) -> T366 (pri 2, the flush gains a post-push-verify best-effort site-sync leg), spec t366-dormant-flush-site-sync.md, filing ad69bd51, todo_consistency 21/21. The filing EXITS dormancy per T325 (open=1).
- Audit record: EVALUATION.md Outcomes cycle-3209 entry (full) + cycle 3194 compacted to the one-liner (6-full set kept); battery re-verified 225/225 post-edit.
- Audit disposition 5d82c234 (eval:-prefixed class line: gates=green pins=225/225 site=stale daemon=ok langfuse=ok filings=1) — touches EVALUATION.md so the weekly timer restarts; pushed main a9d9e288..5d82c234 (the 237 dormant cheap-exit dispositions rode along).
- Outcome backfill d1795024352-2 (routing landed-clean) logged.
- NO TAG: TRUE 0 distinct flip-ids since v0.26.1 (mechanical recount, ANCESTOR-OK per T245, describe v0.26.1-308-gad69bd51).

## Next
- Cold next cycle: dormancy EXITED on the T366 filing (open=1) — the normal T258/T259/T247 arms resume; T366 (pri 2, loopd.sh flush site-sync leg, spec ready, SOLO doctrine) tops the queue; the delta since the audit commit is non-bookkeeping (spec file) so the next real eval routes kimi when the staleness arm fires.
- loopd's cycle-OK leg runs scripts/site-sync.sh at this goal_complete — the stale panel heals (v0.26.1 publishes) at THIS wrap; T366 prevents recurrence.

## Blockers
- none
