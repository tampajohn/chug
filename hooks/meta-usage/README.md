# meta-usage — the usage-events emitter pack (T343)

One hook, wired on `Stop` (the turn-complete event), appending ONE JSONL
line per turn to `<claude-dir>/meta-usage.jsonl` that any harness can tail:

```
{"ts":"…","session_id":"…","model_requested":"…","model_served":"…",
 "input_tokens":n,"output_tokens":n,"cache_read_tokens":n,
 "cache_creation_tokens":n,"contract_version":"1"}
```

Presence contract: a field is in the line only when the event carries it —
absent values are omitted keys, never fabricated zeros, and the served slot
is filled only by an explicit as-served value (never the requested name).
Today's Claude Code `Stop` payloads carry none of the model/usage fields, so
today's lines are `{ts, session_id, contract_version}` — honest per-turn
markers, not fabricated dashboard rows. The transcript is deliberately NOT
read (it lags the turn; mining it would attribute the previous turn's usage
to this one).

Install (with the other meta packs):
`install.sh --with-meta-pack` · uninstall: `install.sh --uninstall-meta-pack`
(the accumulated log is LEFT IN PLACE — it is a historical record, not
state). Detection: `chug capabilities` flips `usage_events: true` when the
pack's hook is wired. Full schema and rules: `docs/meta-harness.md`.

`wire.py add|remove <settings.json> <hook-command>` is the settings surgeon
install.sh drives; the handler is `hook.sh` (pure transcription, one exec,
no network — Langfuse shipping stays on the OTel path).
