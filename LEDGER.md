# Ledger

## Done
- Cycle-3208 CLOSED. Two items landed + one filed:
  - T363 (pri 1, operator-filed 07249f8b, LOOP-SPEC doctrine SOLO, kimi REQUIRED): loopd.sh flush gains the T100 push-verification leg (release_unpushed_tag probe + push + ls-remote-verify + keep-tag-retry-push-only; the retry check PRECEDES the recount trigger) + the LOOP-SPEC Tag-at-wrap push-verification sentence + tests/loopd_dormant_mode.rs + tests/loop_spec_tag_doctrine.rs. impl a4d480ba died 80/80 INCOMPLETE → T63 resume d1794412382-2 landed it. kimi r1 FAIL d1794414448-4 (4 findings) → orchestrator nit 2787e1f2 RED-proven. merge 8dc40bd0, flip 28ec5ee2, harvest + worktree removed.
  - T364 (pri 2, operator mid-cycle filing): release.yml aarch64 apt leg pins archive.ubuntu.com + 3-attempt DNS backoff (10s/30s/90s). kimi r1 PASS d1794416940-6. merge bb1c6ae0, flip 0b0c7ff4.
  - T365 filed (pri 3, spec ready): capabilities_contract daemon-probe tests read the host's REAL daemon socket — the loop's own daemon reds gates under clean-home isolation.
- Post-merge partition closed on HEAD 3cbc0ee7: full suite CHUG_HOME-clean 2288/2289 (only install_sh — the override artifact, natural-env 1/1) + capabilities override 3/3 with the daemon live (twice) + clippy ZERO → ALL 2289 GREEN across profiles. The 17:12 capabilities pair adjudicated a one-off env-plumbing flake.
- Corridor: prep 07390474 (Outcomes 3208 + compact 3193 needle-verified + T261 batch SIX d1794417705-1..-6) + notes c1e346ae (token-free, T257 color, post-commit probe 60 0 machine==TRUE 0) + fill 3cbc0ee7 (T309/T310/T317 asserts GREEN same-leg: datum 139/~7,052.055s, ok:false 2 == 2, trim 0). State keyed to prep 07390474 (T311 validate-state GREEN), ring PREPEND-12/CARRY-0. NO TAG (TRUE 2 < 3 since v0.26.0). main == origin/main through 3cbc0ee7.
- LIVE_CTX compaction landed this cycle: ONE contiguous span 410,398 → 17,970 bytes (turn 0 pinned, pair-integrity by construction) at the 100,752-token warn threshold.

## Next
- Cold next cycle: T365 open (pri 3, tests-only isolation — capabilities_contract daemon-probe tests isolate CHUG_HOME in-test; the loop's own daemon is the polluter). Predicate check per LOOP-SPEC Phase 1.
- The T362 corridor-over-budget watch carries its SECOND sighting (bash 60 then 107, delegate=5) — corridor delegation weighed at the next eval.

## Blockers
- none
