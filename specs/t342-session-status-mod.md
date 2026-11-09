# T342 — session-status mod: typed status surface for any Claude session (no-op when unwatched)

estimate: ~150 changed lines (hooks/meta-status/ pack sharing the T341 install leg + status writer + fixtures)
check: set -o pipefail; CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-validate-a cargo test --release --test install_sh --test capabilities_contract

## Why

Meta-harness map item 3's Claude-side half: herdr's sidebar and orca's
worker view both scrape terminals to learn what a session is doing — the
blocked-signal scraping (claude-bg-agent-blocked-signal) is our own scar.
Claude's hook events already SEE the truth (turn ends, permission asks,
errors, user questions). A tiny mod can maintain a typed status file any
harness reads: zero scraping, zero TTY coupling.

## What

1. **hooks/meta-status/** (same `--with-meta-pack` install leg as T341):
   hook handlers on turn/permission/error/question events that rewrite
   `~/.claude/meta-status/<session-id>.json` atomically:
   `{ status: working|blocked|done, blocked_reason: permission|question|
   error|null, detail, updated_at, contract_version }` — the T339 enum,
   exactly.
2. **Reader contract** (docs/meta-harness.md): harnesses poll the file
   (inotify/fswatch optional); absent file = not-a-meta session; stale
   (>10min) = treat as unknown. The mod never writes anything else and
   never touches the session's own state.
3. **Cost ceiling**: <2ms per hook event, no network, no LLM calls — pure
   state transcription. Budget asserted in fixtures.
4. **Capability wiring**: `chug capabilities` reports `status_mod: true`
   when installed.

## Tests

- Event fixtures: each hook event maps to the right status/blocked_reason
  (permission ask → blocked/permission; turn end with pending question →
  blocked/question; Stop → done; tool error surfaced → blocked/error).
- Atomicity fixture: readers never observe a partial file (tmp+rename).
- Silent-when-unwatched fixture: no reader, no socket — the mod's total
  footprint is one small file per session; nothing else changes.
- Cleanup fixture: session end leaves the file with status done (no
  reaper needed; documented).

## Compat

Same install leg and same OFF-by-default rule as T341. No-op mod: it
observes and transcribes; it cannot alter session behavior, hook order,
or output. Non-meta sessions are unaffected whether or not the pack is
installed.

## Out of scope

- The herdr/orca readers (their side of the contract).
- Aggregation of many status files into a dashboard (harness job; chugd
  MAY read them for T340's children view in a follow-up).
