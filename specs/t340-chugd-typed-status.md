# T340 — chugd typed status endpoints: status enum, blocked-reason, children enumeration

estimate: ~160 changed lines (src/daemon status module + endpoint wiring + fixtures; rides T339 contract)

## Why

Meta-harness map item 3 (lifecycle/status). A harness supervising chug
today scrapes loopd logs and guesses. The daemon (T204, unix socket, live
since 2026-10-13) already serves /sessions (T219); this adds the typed
status model from the T339 contract so herdr/orca/bridge can ask, not
scrape: what is the loop doing, what is it blocked on, what children does
it have.

## What

1. **`/status/loop`** (new endpoint): `{ status: working|blocked|idle|
   dormant, cycle_id, iteration, item_id|null, blocked_reason: permission|
   question|error|budget|external|null, blocked_detail, children: [{kind:
   impl|validate|worktree, id, state}], as_of }` — sourced from the loopd
   state files the daemon already watches; the T328-relevant blocked
   external case (missing repo) must surface as `blocked/external` with
   the detail, not vanish.
2. **`/status/items`**: per open TODO row `{ id, title, pri, status,
   blocked_reason? }` — the harness's queue view without parsing TODO.md.
3. **Contract registration**: `chug capabilities` reports
   `daemon_status: true` + `lifecycle_events: "1"` once this lands (T339
   wiring).
4. **Versioned shapes**: every response carries `contract_version` from
   T339; fields are additive-only within a version.

## Tests

- Daemon fixture battery: each status enum value renders from canned
  loopd state; the blocked/external case names the missing path; children
  enumerate impl+validate worktrees from a fixture .chug dir.
- Socket-absent degrade: endpoints unreachable → `chug capabilities`
  reports daemon_status false, never hangs (>2s budget per probe).
- Contract fixture: response keys match docs/meta-harness.md exactly.

## Compat

New endpoints only; `/sessions`, `/status`, health, and the decision
providers (T332) are untouched. A client that never calls the new paths
sees zero change. Socket permissions (0600, host-scoped) unchanged — no
new exposure surface.

## Out of scope

- Writing/control endpoints (this is observation only; control is a
  separate, explicitly-approved future item).
- Cross-box federation (host-scoped, as today).

check: set -o pipefail; CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-validate-a cargo test --release --test daemon_status_contract
