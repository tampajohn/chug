# chug meta-broker hook pack (T341)

The permission-broker hook pack: Claude's PreToolUse permission decisions
route to a meta-harness broker (herdr, orca) when one is listening, and are
untouched when one is not. Opt-in, fail-open. The wire contract lives in
`docs/meta-harness.md` (the T341 section); this README is the pack's
operating card.

## Files

- `hook.sh` — the hook entrypoint. Fast fail-open gate: no broker socket →
  exit 0 with no output in well under 5ms (shell builtins only, no fork).
  Socket present → hands the payload to `hook.py`.
- `hook.py` — the handler. POSTs the broker request over the unix socket
  (800ms wall budget), honors the reply, appends the audit line. Any
  failure prints nothing and exits 0.
- `wire.py` — the settings.json surgeon install.sh uses for the additive
  wiring and the exact-block uninstall.

## Install / uninstall

```console
$ install.sh --with-meta-pack      # wires the hook into ~/.claude/settings.json
$ install.sh --uninstall-meta-pack # removes exactly its own block again
```

A repo checkout installs the pack from the working tree; the one-liner
installs it from the same verified release tarball that carried the binary.
The pack lands in `<claude-dir>/hooks/meta-broker/`.

## What Claude sees

- broker says `allow`/`deny` → Claude's `permissionDecision` document on
  stdout (exit 0) — no prompt.
- broker says `ask`, or every fail-open case (socket absent, refused,
  >800ms, non-200, malformed, unknown decision, audit append failed) →
  no output, exit 0 — Claude's stock permission evaluation, byte-identical.

The broker socket is `<claude-dir>/meta-broker.sock`; every brokered
decision appends one JSONL line to `<claude-dir>/meta-broker-audit.jsonl`.
`<claude-dir>` is `$CLAUDE_CONFIG_DIR` when set, else `~/.claude`.

## Diagnosis

The hook is silent by contract (zero interference with the stock path).
`META_BROKER_DEBUG=1` keeps the handler's stderr visible, and the audit
file is the ground truth for what was brokered. Pipe a payload when running
the hook by hand: `echo '{}' | hook.sh`.
