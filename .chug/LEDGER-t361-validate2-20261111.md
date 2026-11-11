# Ledger

## Done
- Scope check: branch diff e2ec1121..HEAD = exactly the 4 named targets (loopd.sh, tests/loopd_dormant_mode.rs, tests/loopd_osc7501.rs, specs/t361-dormant-release-flush.md); fix-up 55d72b64 tests-only +147; zero TODO.md/LEDGER.md commits
- Gates ALL GREEN (validate-a slot): sh -n OK; spec check 27+30+12=69 SPEC_RC=0; nextest 2275/2275 (1 leaky = pre-existing chug::update_cmd mark); clippy --all-targets --release -D warnings rc=0 zero warnings
- Mutant M4 (tag-delete neuter, loopd.sh:855) KILLED: exactly 2 failures (static pin + failed-push fixture), 25/27 pass — RED-proof reproduced
- Mutant M5 (audit rider -ge 3→-ge 4, loopd.sh:2483) KILLED: exactly 2 failures (static pin + flush-before-audit fixture), 25/27 pass — RED-proof reproduced; gate-side -ge 3 (1206) untouched
- Main worktree byte-clean (only live loop's own LEDGER.md runtime state, mtime predating session)
- VERDICT: PASS written to .chug/verdict.md (5 numbered findings); throwaway worktrees + role-keyed target dirs removed (cap 3 honored: 2 used)

## Next
- (round-2 validation complete — nothing further)

## Blockers
- none
