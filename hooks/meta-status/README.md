# chug meta-status hook pack (T342)

The session-status mod: hook handlers on turn/permission/error/question
events maintain a typed status file any meta-harness can read — zero
scraping, zero TTY coupling. It is a pure transcription: it observes what
Claude's hook events already see and writes one small file per session,
nothing else. Opt-in (OFF until installed), silent when unwatched, and a
no-op mod on every path (it cannot alter session behavior, hook order, or
output). The wire contract lives in `docs/meta-harness.md` (the T342
section); this README is the pack's operating card.

## Files

- `hook.sh` — the handler. Shell builtins plus a single `exec date` whose
  format string IS the status document; costs a millisecond or so per
  event (the spec's <2ms budget, asserted by
  `tests/meta_status_hook.rs`), no network, no LLM, no python on the hot
  path.
- `wire.py` — the settings.json surgeon install.sh uses for the additive
  six-event wiring and the exact-block uninstall.

## Install / uninstall

```console
$ install.sh --with-meta-pack      # wires the hooks into ~/.claude/settings.json
$ install.sh --uninstall-meta-pack # removes exactly its own blocks again
```

A repo checkout installs the pack from the working tree; the one-liner
installs it from the same verified release tarball that carried the
binary. The pack lands in `<claude-dir>/hooks/meta-status/` and the status
directory `<claude-dir>/meta-status/` is created at install (the hook
never mkdirs on the hot path — a missing dir means skip, so the write
path stays fork-free). Manual wiring should create it too:
`mkdir -p ~/.claude/meta-status`.

## The status document

One file per session: `<claude-dir>/meta-status/<session-id>.json` —

```json
{ "status": "working", "blocked_reason": null, "detail": "Bash", "updated_at": "…", "contract_version": "1" }
```

- `status`: `working` | `blocked` | `done` (the T339 enum, exactly).
- `blocked_reason`: `permission` | `question` | `error` | `null`.
- `detail`: the tool name or notification message when known — otherwise
  the key is omitted (absent values are omitted keys, never fabricated).
- `updated_at`: RFC3339 UTC, second precision.
- The body is space-padded to a constant 512 bytes (legal JSON trailing
  whitespace) so every rewrite is one equal-length overwrite of the same
  inode — readers see the previous or the new complete document, never a
  partial one, with no rename and no truncate window.

## Event table

| event | condition | written |
| --- | --- | --- |
| `UserPromptSubmit` | — | `working` |
| `PreToolUse` | `tool_name` == `AskUserQuestion` | `blocked` / `question` |
| `PreToolUse` | any other tool | `working` (+ tool detail) |
| `PostToolUse` | response carries an error marker | `blocked` / `error` |
| `PostToolUse` | otherwise | `working` |
| `Notification` | message mentions `permission` | `blocked` / `permission` |
| `Notification` | otherwise | nothing (not a status fact) |
| `Stop` | file says `blocked`/`question` | nothing (the question stays pending) |
| `Stop` | otherwise | `done` |
| `SessionEnd` | — | `done` |

Session end leaves the file at `done` — no reaper needed; readers treat a
stale (>10min) file as unknown anyway. Anything unsanitary (unknown event,
unsafe session id, impure detail) is skipped silently — never a wrong
write.

## What Claude sees

Nothing. The hook prints nothing, exits 0, and never blocks: it cannot
alter session behavior, hook order, or output. Non-meta sessions are
unaffected whether or not the pack is installed. `<claude-dir>` is
`$CLAUDE_CONFIG_DIR` when set, else `~/.claude`.

## Diagnosis

The hook is silent by contract. Run it by hand with a payload on stdin
(`echo '{"session_id":"t1","hook_event_name":"Stop"}' | hook.sh`) and read
the file it wrote; `tests/meta_status_hook.rs` is the behavioral battery.
