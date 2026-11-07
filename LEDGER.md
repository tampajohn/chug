# Ledger

## Done
- Cycle-3075 Phase-1 disposition: routine glm skip per freshness rule — routing record d1794035652-1 (predicate HOLDS: 3 open rows + EVALUATION.md 739th eb2dd5c same UTC day; loopd routed glm at 07:12:52Z). Flagged for the next eval: loopd's eval-delta read `FULL-READ REQUIRED: splice-violated (nothing carried verbatim from the previous ring)` at 07:12:25Z — the state ring may need a heal write at the wrap.
- T332 discovery DONE (orchestrator probe, live): the proxy /v1/systemone is a DROP-IN for the daemon /judge — same {state,questions} body, same {answers} envelope, choice/noul/score all verified, usage.cost present, model field REQUIRED, Bearer or x-api-key auth. Committed as docs/decision-providers.md on the arc branch (b0ed89b).
- T332 impl arc: 4 glm segments (seg1 80/80 iters → T63 resume seg2 77/80 minutes → fix-up seg3 80/80 → slice-4 seg4 60/60) + slice-4 ORCHESTRATOR-FINISH (T55/T150 precedent): 5 commits on loop-t332 (b0ed89b, 92c1989, b3252c7, bc05a59, e7aff68) — src/decision/ abstraction (LocalDaemon + ProxySystemOne, CHUG_DECISION_PROVIDER=auto|local|proxy), riskgate CHUG_JUDGE=proxy mode + decision-layer consult, the T330 profile contract (install.sh + both loaders + profiles/decision-proxy.env), the loopd.sh laya_triage rewire (fail-open preserved, provider-tagged note) + ideate.py noul leg (fail-closed preserved), batteries 30+17+8. My review fixes: 3 quiet-probe note asserts re-pinned at source level (loopd_fn_src); env_loader explicit-env leg count assert corrected. 11 delegate/mcp spawn reds adjudicated pre-existing (fail identically on main under load).
- T332 kimi validation REQUIRED (d1794044736-4): validator pid 29377 FLYING (validate-a slot).

## Next
- T332: collect verdict → PASS → merge/flip/push → harvest worktree → post-merge gates.
- T331 (pri 3, docs-only risk-gate story page) if budget permits.
- Phase-3 wrap: eval-state heal (the splice-violated flag), gates, release check, token-free notes + T257 color, probe, fill, push, goal_complete.

## Blockers
- none (T328 stays BLOCKED on the chug-watch tree, fleet-side)
