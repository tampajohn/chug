# Ledger

## Done
- Phase-1 disposition: routine freshness-skip (predicate HOLDS — todo rows open + eval fresh cd3e6003 same-UTC-day); routing d1794241440-1
- Read-first reconstruction (T251): operator filings T348-T351 specs-only; mid-cycle T352/T353/T354 filings; the operator's OWN T349 child (delegate 16:49:31Z) died 80/80 17:03:25Z with site-sync edits uncommitted — lane left to the operator, worktree /tmp/chug-loop-t349 preserved
- T347 arc: root-caused (macos-14 retired, live-API verified; loop tag-deletion ruled out) → glm impl 0331fc28 (26/80) → worktree gates GREEN (clippy ZERO + nextest 2049/2049) → kimi REQUIRED (d) d1794242900-2 → PASS d1794244759-4 (7 mutants 5 killed) → merge dd4407c2
- T348 arc: glm impl cd658edd (67/80) → worktree gates GREEN (clippy ZERO + nextest 2059/2059, 40/40 install_sh) → kimi REQUIRED (b) d1794242900-3 → PASS d1794245005-5 (7 mutants 5 killed) → merge 9e737d90
- Flips 8c68100c (ONE todo: commit, both rows + refs); harvest disk-only (8 artifacts); worktrees removed post-liveness (t349 operator lane preserved)
- THE BACKFILL: gh workflow run tag=v0.21.0 → run 42666475705 GREEN (macos-15 arm64 4m53s) → v0.21.0 PUBLISHED all 6 assets → releases/latest current
- RELEASE v0.22.0 CUT (extraction 3 == TRUE 3 ≥ 3; minor; pairing + notes + tag 223815c4 pushed; run 42669096203 provisioned)
- Final gates T242 bg GREEN at 606d71a6 (clippy ZERO + nextest 2060/2060); T261 batch FIVE; state write keyed to e40b192e (T311 GREEN)

## Next
- Wrap tail: fill (assert legs) → push → flips (this ledger's wrap todos) → goal_complete
- Next cycle: T349 operator-lane check (resume their 80/80-dead child or dispatch fresh if the operator's session is gone); T350-T354 operator-lane; T328 parked (A/B/C menu)

## Blockers
- T328 BLOCKED (operator A/B/C menu unanswered)
- Operator ACTIVE in the shared checkout (T354 filed 17:4xZ mid-cycle) — the loop must re-verify tree state read-first every boot
- LIVE_CTX: the compaction attempt REJECTED (turn-1 pairing) — file non-canonical on disk, never pushed, the driver's own trim supersedes (the 3173 precedent)
