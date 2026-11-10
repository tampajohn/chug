# Ledger

## Done
- T360 COMPLETE — committed 545c2bcc (6 named-target files only; LEDGER.md left modified-uncommitted — bookkeeping is the orchestrator's).
- RED PROOF captured pre-implementation (supervisor-only derivation):
  - `a_cycle_log_marker_alone_blocks_the_idle_transition` FAILED: left=[Record{state:"idle", msg:"cycle ok", app:None}] right=[Record{state:"blocked", msg:"observability delivery failing (run continues): langfuse: dial tcp 127.0.0.1:3000: connect: connection refused"}] — the native emission reports idle while the cycle log carries the marker (the FP acceptance RED leg, exactly).
  - `every_native_record_carries_the_app_chug_label` FAILED: left=None right=Some("chug") — native bytes carry no app key (herdr shows app: None).
- Implemented: scripts/osc7501.sh `osc7501_cycle_block()` shared scan (newest .chug/loopd/cycle-*.log; `observability delivery failing`/`gate red` substrings, `error`/`panic` whole words; last match wins, 200-char cap; rc-guarded, degrades to plain idle) + `seq="state=$state:app=chug"` on EVERY record; loopd.sh wrapper + pilot wrapper integrate the scan at the ONE composition point (idle→blocked rewrite BEFORE both surfaces compose; stale OK hb detail dropped so the line composes `[loop] BLOCKED: $msg` from the same msg); fresh clean cycle start clears the block naturally (newest-log scan).
- Fixtures: 5 new in tests/loopd_osc7501.rs (cycle-log blocked, clean-clear, byte-identical parity through both supervisors, app=chug on all four states, pattern coverage incl. whole-word negatives `errors:`/`panicked` + the 200-char cap). Expectations updated: rec() app: Some("chug"), 3 exact-byte fixtures, `seq=` pin, pilot delegate pin, file-header wire comment; docs/meta-harness.md §T354 (wire shape + app sentence + T360 blocked-leg note); README 7501 bullet.
- Fixture-noise audit: all sandbox stub stderr lines in the trio are clean of the four patterns; build-gate stub stderr (whole-word `error` in tests/loopd_stale_binary.rs) goes to $LOG, never a cycle log, and those arcs never reach an idle transition — full-suite run confirms (2264/2264). Known intended behavior, not noise: a production cycle that hits a mid-run error and still completes OK surfaces blocked at the idle transition (the FP case is exactly a recovered delivery failure — that is the point of the leg).
- Gates: build --release GREEN; clippy --all-targets --release -D warnings rc=0 ZERO warnings; check: trio GREEN (loopd_osc7501 51/51, loopd_heartbeat 42/42, loopd_verbose 31/31); FULL suite nextest 2264/2264 GREEN.

## Next
- Orchestrator: harvest 545c2bcc; FP acceptance re-run on the carrying release is the live blocked-leg check.

## Blockers
- none
