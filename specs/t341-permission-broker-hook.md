# T341 — permission-broker hook pack: meta-harness as approver-of-record (opt-in, fail-open)

estimate: ~180 changed lines (hooks/meta-broker/ pack + install.sh --with-meta-pack leg + broker socket protocol doc + fixtures)
check: set -o pipefail; CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared cargo test --release --test install_sh --test capabilities_contract

## Why

Meta-harness map item 1 (permission brokering) — the single most-wanted
capability. Today a harness that wants to adjudicate Claude's permission
requests edits state.json respawnFlags and kills the worker
(bg-job-permission-mode-respawnflags scar). Claude's PreToolUse hook
protocol already supports permissionDecision — so the broker is buildable
TODAY as an opt-in hook pack: when a harness is listening, decisions route
to it with context; when not, behavior is byte-identical to stock.

## What

1. **hooks/meta-broker/** (new pack, shipped in-repo and installed by
   `install.sh --with-meta-pack`): a PreToolUse hook that, when
   `~/.claude/meta-broker.sock` exists and answers, POSTs
   `{tool_name, input_summary, session_id, cwd, contract_version}` and
   honors the reply `{decision: allow|deny|ask, reason, ttl}` — `ask`
   falls through to the stock prompt path unchanged.
2. **Fail-open by construction**: socket absent, timeout (>800ms), or
   malformed reply → the hook exits 0 with no output (stock behavior,
   zero interference). A broker crash can NEVER wedge a session.
3. **Audit leg**: every brokered decision appends one JSONL line to
   `~/.claude/meta-broker-audit.jsonl` (decision, reason, broker latency)
   — the harness audit trail from the design map.
4. **Protocol doc**: docs/meta-harness.md gains the broker section
   (request/response shapes, the fail-open table, the audit schema);
   `chug capabilities` reports `permission_broker: true` when the pack is
   installed (detected by hook presence, not socket presence).

## Tests

- Hook fixture battery: allow/deny/ask replies honored; timeout → stock
  path; malformed JSON → stock path; socket absent → stock path with zero
  latency added (<5ms).
- Audit fixture: brokered decisions append valid JSONL; stock-path
  decisions do NOT append (no noise).
- Install fixture: `--with-meta-pack` wires the hook into settings.json
  additively (existing hooks untouched); uninstall removes exactly its
  own block.

## Compat (the operator's constraint, explicit)

Default OFF: the pack installs only via the explicit flag; without the
socket the hook is a <5ms no-op; stock permission prompts are the
unaltered default in every non-harness session. No global settings change
beyond the opt-in hook entry.

## Shipping mechanic (discovered at dispatch — the tarball carries the pack)

Today the release tarball carries ONLY the `chug` binary
(.github/workflows/release.yml package step). "Shipped in-repo and
installed by install.sh --with-meta-pack" therefore implies: the workflow
package step also copies `hooks/` into the tarball (one line), and
`tests/release_workflow.rs`'s pin updates to match; install.sh
--with-meta-pack then extracts the pack from the SAME verified tarball it
already downloads (no second fetch, no raw-URL fragility). Repo-checkout
users install from the working tree.

## Out of scope

- The harness-side broker implementation (herdr/orca build theirs; we
  ship a reference `chug broker --stdio` stub for dogfooding only if it
  rides free in the fixtures).
- Auto-mode / server-side review integration (separate lane, existing).
