# T343 — usage events emitter + model-identity truth (additive telemetry leg)

estimate: ~120 changed lines (hooks/meta-usage/ in the meta pack + chug event stamping + docs + fixtures)
check: set -o pipefail; CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-validate-a cargo test --release --test install_sh --test capabilities_contract

## Why

Meta-harness map item 4. Every harness builds its own wrong cost
dashboard from side-channels; model identity through proxies is actively
unreliable (serving names are not model ids; GLM cache accounting reads
zero). Two cheap fixes: (a) a hook that emits a per-turn usage line any
harness can tail, and (b) chug stamping the AS-SERVED model identity into
its own event stream so downstream consumers stop guessing.

## What

1. **hooks/meta-usage/** (same `--with-meta-pack` leg as T341/T342): on
   each turn-complete event append one JSONL line to
   `~/.claude/meta-usage.jsonl`: `{ ts, session_id, model_requested,
   model_served, input_tokens, output_tokens, cache_read_tokens,
   cache_creation_tokens, contract_version }` — fields present only when
   the event actually carries them (no fabricated zeros; absent = key
   omitted, documented).
2. **Model-identity stamping (chug side)**: the loop's events/decisions
   streams gain `model_served` alongside the existing model field,
   populated from the response's served-model metadata when the provider
   returns it (proxy and daemon both); absent → key omitted, never the
   requested name copied into the served slot (that conflation is the
   bug being fixed).
3. **Docs**: docs/meta-harness.md gains the usage schema + the
   presence/absence contract; `chug capabilities` reports
   `usage_events: true` when the pack is installed.

## Tests

- Usage fixture: a turn event with full fields → complete line; partial
  event → only present keys; the line is single-line valid JSONL.
- Stamping fixtures: provider returning served-model → both fields
  recorded distinctly; provider silent → model_served absent; existing
  events/decisions consumers parse unchanged (additive field).
- Budget fixture: hook adds <2ms per turn; no network in the hook path
  (Langfuse shipping stays with the existing OTel path, NOT this hook).

## Compat

Additive everywhere: new file on disk, new optional keys. No existing
event shapes change; no telemetry is removed or rerouted; non-meta
sessions see zero behavioral change (the file just grows).

## Out of scope

- Dashboards/aggregation over the JSONL (harness job; the T327 pattern is
  the recipe).
- Retroactive model-identity correction of historical telemetry.
