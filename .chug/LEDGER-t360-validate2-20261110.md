# Ledger

## Done
- T360 fix-up COMPLETE (commit 7357c6de, tests/loopd_osc7501.rs only — loopd.sh and scripts/osc7501.sh untouched; the implementation passed 6 mutants on substance).
- F2 (m6 root cause) FIXED: parse_osc7501() seeds the NEUTRAL `Record7501 { state, ..Default::default() }` (app None); `rec()` documented as the EXPECTATION-side constructor only. All 34 `rec(` call sites are expectations — after the seed fix every one is a real app=chug assertion (a missing app key now parses as None and fails the equality).
- F2 fixture strengthened: `every_native_record_carries_the_app_chug_label` asserts BOTH the raw `state=<s>:app=chug` needle AND the parsed app == Some("chug") for working/idle/done/blocked.
- F2 RED PROOF (m6: app stripped ONLY on state=done in scripts/osc7501.sh): the dedicated app test FAILED at the done leg — `state=done: the raw bytes must carry the state=done:app=chug needle ... got [27, 93, 55, 53, 48, 49, 59, 115, 116, ...]` (wire = state=done:msg=…, no app key), exit 101. Restored → GREEN.
- F1 (m5 root cause) FIXED: `probe_pty_args` (the FULL-argument `osc` probe — the 4-arg form reaches the wrapper's `if [ -n "$hb" ]` heartbeat leg; both knobs env-removed) + `probe_pty` delegates to it + `heartbeat_lines` helper (mirrored from tests/loopd_heartbeat.rs).
- F1 fixture added: `the_scanned_block_drops_the_stale_ok_detail_on_the_heartbeat_line` — poisons the newest cycle-*.log with DELIVERY_MARKER, drives `osc idle "cycle ok" "[loop] cycle done — goal complete"` with CHUG_LOOP_VERBOSE=1 on a real pty, asserts BOTH surfaces: dot = blocked+marker msg, AND heartbeat lines == exactly `[loop] BLOCKED: <marker>` with the stale cycle-done line ABSENT.
- F1 RED PROOF (m5: loopd.sh `hb=""` reset dropped): the fixture FAILED on surface 2 — `assertion left == right failed ... left: ["[loop] cycle done — goal complete"] right: ["[loop] BLOCKED: observability delivery failing (run continues): ..."]`, exit 101 — while the dot's parse assert PASSED under the mutant (exactly why the round-1 dot-only fixtures were blind). Restored → GREEN. Both mutants applied/reverted in place; `git diff loopd.sh scripts/osc7501.sh` clean.
- Sweep: the ONLY fabricated-seed parse path repo-wide was loopd_osc7501.rs's parser (fixed). tests/loopd_heartbeat.rs and tests/loopd_verbose.rs parsers model ONLY state+msg with a neutral seed (no app field — not instances). The wrapper's remaining legs all carry real coverage: TTY gate (pty+piped), idle→blocked scan rewrite (T360 fixtures), elif-blocked composition (heartbeat real-supervisor dying arcs), dot wire (exact-byte pins). No real implementation defect surfaced — none of the emitters changed.
- GREEN: gated check cmd (loopd_osc7501 42 + loopd_heartbeat 52 + loopd_verbose 31 = 125 passed, was 124), cargo build --release clean, cargo clippy --all-targets --release -- -D warnings zero warnings. Committed (tests/loopd_osc7501.rs only; LEDGER.md and the pre-existing specs/t360 check re-key left for the orchestrator).

## Next
- none — fix-up closed; orchestrator round 2 can validate.

## Blockers
- none
