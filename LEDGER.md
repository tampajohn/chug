# Ledger

## Done
- Phase-1 disposition: routine freshness-skip (predicate HOLDS — todo rows open + eval fresh cd3e6003 same-UTC-day); routing d1794241440-1
- Read-first reconstruction (T251): operator filings T348-T352 are specs-only (zero code); T328 re-verified BLOCKED; single-driver verified
- T347 arc: root-caused (macos-14 runner label retired — GH annotation on run 42606437875; loop never deleted tags, grep-proven) → glm impl 0331fc28 (26/80) → worktree gates GREEN (clippy ZERO + nextest 2049/2049) → lane FULL (d) d1794242900-2 → kimi validator pid 67222 FLYING
- T348 arc: glm impl cd658edd (67/80) → worktree gates GREEN (clippy ZERO + nextest 2059/2059) → lane FULL (b) d1794242900-3 → kimi validator pid 67419 FLYING (spec re-keyed validate-b pre-launch)
- T349 arc: worktree ready (rebased-baseline note in goal), check-line added (impl-c), glm impl pid 69872 FLYING

## Next
- T347 validator verdict → merge (serial first) → backfill dispatch v0.21.0 via workflow_dispatch → verify install
- T348 validator verdict → merge (serial second)
- T349 impl → review/gates → lane call → merge (third, after T348)
- T350 (site-sync stats leg — serial after T349, same file), T351 (README docs), T352 (deferred — spec ready), T328 (BLOCKED, operator menu pending)
- Phase-3 wrap: wrap-state note, final gates (T242 bg target-shared-main), Outcomes, state rewrite, token-free notes + T257, probe, fill, push, goal_complete

## Blockers
- T328 BLOCKED (operator A/B/C menu unanswered — chug-watch package.json ABSENT)
- Operator active in shared checkout (T352 filed 16:36Z mid-cycle — merges may need rebase onto newer main; filings are docs-only, no conflict expected)
- Advisory: T347 validator launched with spec check still keyed impl-a (both dirs quiescent + same content — safe; noted, not re-keyed mid-flight)
