# Ledger

## Done
- Cycle-3074 opened as a RECOVERY cycle (T251 read-first): cycle-3073's wrap was already CLOSED+PUSHED per git record (fill a31b499 == origin/main at ff2e628) — the stale t4324 zombie-repair todo flipped done per T255; the dirty LEDGER.md is the launch harness's template reseed (prior ledger lives in HEAD's corridor-tail commit ff2e628).
- Phase-1 disposition: routine glm skip per freshness rule — routing record d1794031572-1 (predicate HOLDS: 4 rows + EVALUATION.md fresh, the 739th eval e59f52b same UTC day; loopd routed glm at 06:02:14Z).
- T330 arc (recovery recipe on the row, per T255): worktree /tmp/chug-loop-t330 rebased onto main (732611b impl + 42af7ae review fix); child's uncommitted LEDGER harvested → .chug/LEDGER-t330-impl-20261107-035623.md; the THREE loopd_model_routing reds from the recipe diagnosed as FIVE targets/132 tests (loopd_laya_triage 44, loop_spec_cheap_exit 38, loopd_dormant_mode 24, loopd_empty_backoff 14, loopd_ideation_mode 12 + loopd_model_routing 5): T330's new `scripts/loopd_profile_loader.sh` sourcing (loopd.sh:81) dies under `set -euo pipefail` in every fixture running a COPIED loopd.sh with a hand-listed fragment set. FIX (44a3381): tests/common/mod.rs copy_loopd_fixture DERIVES the fragment set from the copied loopd.sh text (fails loudly on a missing fragment) — all six fixture builders rewired (daemon_ensure/stale_binary immune: wholesale copies). One edit_file slip self-caught: the first fix landed in the MAIN tree (edit_file is cwd-confined) — copied into the worktree, main restored byte-identical before merge.
- Worktree gates GREEN: build + clippy --all-targets --release -D warnings ZERO + nextest 1903/1903 (90.6s, target-shared).
- kimi validator REQUIRED (loopd.sh doctrine) launched: pid 20926, /tmp/chug-loop-t330, 60/50, validate-a slot — routing record d1794034366-3. VERDICT PASS (record d1794034366-4): 9/9 mutants KILLED (4 install.sh + 4 loader + 1 lint, parallel disjoint-file legs); 7 findings ALL minor/nit none gate-blocking; finding-6 (double loopd.sh copy from my refactor) fixed on-branch b4183d7.
- Merged loop-t330 → main (1cf0588); post-merge gates launched (bg pid 38054, /tmp/postmerge-t330-gates.log, target-shared-main, T242 window); T330 flipped done + README install section gains the profile contract (3693a25); todo_consistency 21/21 GREEN; harvest complete (events-t330-impl 265 lines / events-t330-validate 118 lines / verdict-t330 / LEDGER-t330-validate, all UNTRACKED per the 614f7a7 lint rule); worktree removed post-liveness-check.

## Next (wrap, cycle-3074)
- Post-merge gates GREEN check (bg pid 38054) → push T330 commits (main == origin/main restore).
- Phase 3 wrap: release recount (1 item since v0.18.0 → NO TAG expected), decisions-audit + T261 batch (routing d1794031572-1 + validation-routing/verdict + outcome backfills), eval-state rewrite, Outcomes entry, token-free wrap-notes + T257 color, post-commit probe, fill, push, goal_complete.

## Blockers
- none (T328 stays BLOCKED on the chug-watch tree; T331/T332 operator rows queue behind T330)
