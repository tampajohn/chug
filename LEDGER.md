# Ledger

## Done
- Read spec + T339 contract (docs/meta-harness.md, src/capabilities.rs, daemon.rs routes, T219 harness).
- src/daemon.rs: T340 status section (LoopStatus/BlockedReason/StatusChild, bounded derivation from loopd state files, /status/loop + /status/items bodies, ?repo= query parsing + percent-decode, CHUG_DAEMON_REPO env, stub refusal, route/serve threading serve_status flag). Blocked/external names the missing path. Children = .git/worktrees enumeration (impl/validate/worktree lanes).
- 16 unit tests in daemon.rs test mod — all green; cargo build --release green.

## Next
- capabilities.rs: bounded daemon probe (GET /status/loop, 2s budget) + document merge (daemon_status true + lifecycle_events "1" only when a live daemon answers) + unit tests.
- tests/daemon_status_contract.rs: wire battery (fixture repos, each status enum value, blocked/external names path, children impl+validate worktrees, exact-keys contract fixture tests/fixtures/daemon_status/status-loop-v1.json, socket-absent degrade <2s, capabilities flips, stub refuses).
- Gates: build, clippy -D warnings, cargo test --release --test daemon_status_contract, T339 suites (capabilities_contract, daemon_lifecycle, daemon_sessions_registry, daemon_feature_off), commit.

## Blockers
- none

## Decisions recorded (for the wrap)
- Stub serves /healthz ONLY; /status refused there (keeps T339's with-daemon test + fixture posture valid, no fabrication).
- Health UNTOUCHED (compat section) → versions.daemon stays null in T340; probe reads /status/loop's contract_version only.
- Loop surface derives external/budget/error; permission/question carried as vocabulary (documented, allow(dead_code) with reason).
- Children = git worktree admin dir (.git/worktrees) via pure file reads, no subprocess.
