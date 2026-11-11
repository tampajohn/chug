# Ledger

## Done
- Cycle-3208 CLOSED + the goal-gate arc repaired. Books closed at 5ddd2b50 (== origin/main).
- T363 (pri 1, operator-filed, LOOP-SPEC SOLO, kimi REQUIRED): loopd.sh flush push-verification leg (release_unpushed_tag probe + ls-remote-verify + keep-tag-retry-push-only, retry check precedes the recount trigger) + the LOOP-SPEC Tag-at-wrap push-verification sentence + 2 test files. impl a4d480ba died 80/80 → T63 resume d1794412382-2. kimi r1 FAIL d1794414448-4 → nit 2787e1f2 RED-proven. merge 8dc40bd0, flip 28ec5ee2.
- T364 (pri 2, operator filing): release.yml aarch64 apt leg pins archive.ubuntu.com + 10s/30s/90s DNS backoff. kimi r1 PASS d1794416940-6. merge bb1c6ae0, flip 0b0c7ff4.
- T365 (pri 3) LANDED AT THE GOAL GATE: the gate's natural-env check red exactly as T365 predicted (the two `..._without_any_socket` tests saw the loop's own live daemon). Fix: in-test CHUG_HOME scoping at all THREE wired-hook probe sites (class-swept; spec estimated 2). RED = the gate rejection itself; GREEN = full spec check 2289/2289 natural-env WITH the daemon live. flip in 5ddd2b50's parent (gate-fix commit) + todo flip; todo_consistency 21/21.
- CORRECTION (T70): the 17:12 capabilities "one-off env-plumbing flake" adjudication is FALSIFIED — it was the deterministic natural-env + live-daemon shape; corrected forward in EVALUATION.md (the 3208 entry) at the gate arc.
- Corridor: prep 07390474 (Outcomes 3208 + compact 3193 + T261 batch SIX d1794417705-1..-6) + notes c1e346ae (token-free, probe 60 0) + fill 3cbc0ee7 (T309/T310/T317 asserts GREEN same-leg: datum 139/~7,052.055s, ok:false 2==2, trim 0) + ledger close-out d0110037. State keyed to prep 07390474 (T311 GREEN), ring PREPEND-12/CARRY-0. NO TAG (TRUE 2 < 3 since v0.26.0 at the prep recount; T365's late flip makes TRUE 3 — the release recount re-runs at the next wrap's prep per the T269 mechanical rule, never quoted forward).
- LIVE_CTX compaction: ONE contiguous span 410,398 → 17,970 bytes (turn 0 pinned, pair-integrity by construction).

## Next
- Cold next cycle: predicate check per LOOP-SPEC Phase 1 (TODO has zero todo rows → the T325 dormancy question + the empty-delta chain; the release recount at the NEXT wrap's prep reads TRUE 3 = T363+T364+T365 since v0.26.0 → TAG v0.26.1 likely unless an eval cycle re-runs it first).
- The T362 corridor-over-budget watch carries its SECOND sighting (bash 60 then 107, delegate=5) — corridor delegation weighed at the next eval.
- The gate-check lesson for the corridor: the spec check runs natural-env with CARGO_TARGET_DIR scrubbed — any gate-red must be adjudicated against the host-daemon shape (T365's class) before calling it a flake.

## Blockers
- none
