# Ledger

## Done
- Read spec + T339 contract (docs/meta-harness.md, src/capabilities.rs, daemon.rs routes, T219 harness).
- src/daemon.rs: T340 status section (LoopStatus/BlockedReason/StatusChild, bounded derivation from loopd state files, /status/loop + /status/items bodies, ?repo= query parsing + percent-decode, CHUG_DAEMON_REPO env, stub refusal, route/serve threading serve_status flag). Blocked/external names the missing path. Children = .git/worktrees enumeration (impl/validate/worktree lanes).
- src/capabilities.rs: bounded probe (probe_daemon_status — ONE GET /status/loop on the 2s socket bound; absent/stale/refusing = all-off, never hangs) + document_with_daemon additive merge + document_json() now the probed command path. document() stays pure; versions.daemon stays null (health compat-protected).
- tests/daemon_status_contract.rs (9 wire tests) + tests/fixtures/daemon_status/status-loop-v1.json.
- COMMITTED aa636d08 on loop-t340 (worktree /tmp/chug-loop-t340).

## Next
- Orchestrator: gate + merge. Nothing pending in this worktree.

## Blockers
- none

## Decisions recorded (for the wrap)
- Stub serves /healthz ONLY; /status refused there (keeps T339's with-daemon test + fixture posture valid, no fabrication).
- Health UNTOUCHED (compat section) → versions.daemon stays null in T340; probe reads /status/loop's contract_version only.
- Loop surface derives external/budget/error; permission/question carried as vocabulary (documented, allow(dead_code) with reason).
- Children = git worktree admin dir (.git/worktrees) via pure file reads, no subprocess.
- Legacy routes match the RAW path (query on them still 404s — pre-T340 byte behavior); only /status/* carries ?repo=.

## Gates (all green)
- build --release; clippy --all-targets --release -- -D warnings ZERO
- daemon_status_contract 9/9 (the check target, run verbatim)
- capabilities_contract 7/7, daemon_lifecycle 3/3, daemon_sessions_registry 8/8, daemon_feature_off 5/5
- bin unit tests: daemon::tests + capabilities::tests 50/50
