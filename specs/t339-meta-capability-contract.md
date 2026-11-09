# T339 — meta-harness capability contract: `chug capabilities` + versioned feature doc

estimate: ~110 changed lines (src/cli capabilities subcommand + docs/meta-harness.md + fixtures)

## Why

Operator direction 2026-11-09: extend Claude-side tooling to work well with
meta-harnesses (herdr, orca) — design map at
~/Documents/q4-chug-expansion/meta-harness-claude-integration.md. Every
harness today version-sniffs; the capability contract is the foundation
item: harnesses feature-detect instead, so each later meta item (T340-T343)
can land without breaking any consumer. This is the contract the rest of
the series implements against.

## What

1. **`chug capabilities`** (new subcommand, JSON to stdout):
   `{ contract_version, features: { daemon_status, lifecycle_events,
   permission_broker, usage_events, status_mod }, versions: { chug, daemon
   } }` — each feature boolean (or version string once shipped). T340-T343
   flip their feature on as they land; absent = false. Must be cheap (<50ms,
   no daemon required — a missing daemon just reports `daemon_status:
   false`).
2. **docs/meta-harness.md**: the contract doc — feature keys, the typed
   status enum (working / blocked / needs-input / done / parked), the
   blocked-reason vocabulary (permission, question, error, budget), event
   shapes as they land in T340-T343, and the versioning rule (additive
   within a contract_version; breaking changes bump it).
3. **Stability rule encoded**: the doc and the command both state the
   compat promise — reading unknown features MUST be treated as false by
   consumers; unknown keys MUST be ignored.

## Tests

- `chug capabilities` fixture: parses as JSON, required keys present,
  completes <50ms with and without a live daemon.
- Contract doc link-check: every feature key in the doc appears in the
  command output (scripted, runs in gates).
- Forward-compat fixture: an output with injected unknown keys/values
  parses identically (the consumer rule is testable on our own reader).

## Compat (non-meta-harness behavior)

Pure addition: new subcommand + new doc. No existing command, env var, or
output changes shape. Sessions without a meta-harness are byte-identical
in behavior.

## Out of scope

- The features themselves (T340-T343).
- Anthropic-side protocol asks (out of our control; the doc links them).
