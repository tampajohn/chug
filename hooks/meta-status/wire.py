#!/usr/bin/env python3
"""wire.py — the meta-status pack's settings.json surgeon (T342).

install.sh calls this to wire the pack's hooks into Claude's settings.json
and to remove exactly its own blocks again:

    wire.py add    <settings.json> <hook-command>
    wire.py remove <settings.json> <hook-command>

The pack is wired on six events (the truth surface the session-status mod
transcribes): PreToolUse matched to AskUserQuestion (the pending-question
fact), PostToolUse on all tools (the error fact), and UserPromptSubmit /
Notification / Stop / SessionEnd with no matcher (the working / permission
/ done facts). `add` is additive and idempotent per event: existing hooks
are untouched (values are preserved verbatim — only the pack's own entries
are appended), a partially-wired settings file gets completed, and a
second run is a no-op. `remove` walks EVERY event key and drops only the
hook objects carrying the pack's exact command, then prunes the containers
it emptied; everything else rides untouched. The write is atomic (tmp +
rename) and preserves the file's mode; a malformed settings.json is named
and left alone, never rewritten.
"""

import json
import os
import sys
import tempfile

# (event, matcher) pairs, in wiring order. `None` means no matcher key —
# the shape Claude's hooks schema expects for non-tool events.
PACK_EVENTS = (
    ("PreToolUse", "AskUserQuestion"),
    ("PostToolUse", "*"),
    ("UserPromptSubmit", None),
    ("Notification", None),
    ("Stop", None),
    ("SessionEnd", None),
)


def load_settings(path):
    """Read settings.json; a missing file is an empty document. A malformed
    one is a named failure — the surgeon never rewrites a file it cannot
    parse."""
    try:
        with open(path, "r", encoding="utf-8") as handle:
            text = handle.read()
    except FileNotFoundError:
        return {}
    except OSError as exc:
        fail("cannot read %s: %s" % (path, exc))
    if not text.strip():
        return {}
    try:
        parsed = json.loads(text)
    except ValueError as exc:
        fail("%s is not valid JSON (%s) — fix or remove it by hand; the "
             "pack was NOT wired and the file was left untouched" % (path, exc))
    if not isinstance(parsed, dict):
        fail("%s is not a JSON object — the pack was NOT wired" % path)
    return parsed


def save_settings(path, settings):
    """Atomic replace (tmp + rename), preserving the original file's mode."""
    directory = os.path.dirname(os.path.abspath(path)) or "."
    try:
        mode = os.stat(path).st_mode & 0o777
    except OSError:
        mode = 0o644
    fd, tmp_path = tempfile.mkstemp(prefix=".meta-status-wire-", dir=directory)
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as handle:
            handle.write(json.dumps(settings, indent=2) + "\n")
        os.chmod(tmp_path, mode)
        os.replace(tmp_path, path)
    except OSError as exc:
        try:
            os.unlink(tmp_path)
        except OSError:
            pass
        fail("cannot write %s: %s — check permissions and retry" % (path, exc))


def commands_under(settings, event):
    """Every command string under hooks.<event> (whatever the entry
    shapes) — the per-event idempotence scan."""
    commands = []
    entries = settings.get("hooks", {}).get(event, [])
    if isinstance(entries, list):
        for entry in entries:
            if not isinstance(entry, dict):
                continue
            hooks = entry.get("hooks")
            if not isinstance(hooks, list):
                continue
            for hook in hooks:
                if isinstance(hook, dict) and isinstance(hook.get("command"), str):
                    commands.append(hook["command"])
    return commands


def make_entry(matcher, command):
    entry = {"hooks": [{"type": "command", "command": command}]}
    if matcher is not None:
        entry["matcher"] = matcher
    return entry


def add(path, command):
    settings = load_settings(path)
    hooks = settings.setdefault("hooks", {})
    if not isinstance(hooks, dict):
        fail("%s: hooks is not an object — wire the pack by hand; the file "
             "was left untouched" % path)
    wired = 0
    for event, matcher in PACK_EVENTS:
        if command in commands_under(settings, event):
            continue  # idempotent per event: a partial wiring is completed
        entries = hooks.setdefault(event, [])
        if not isinstance(entries, list):
            fail("%s: hooks.%s is not a list — wire the pack by hand; the "
                 "file was left untouched" % (path, event))
        entries.append(make_entry(matcher, command))
        wired += 1
    if wired == 0:
        print("wire.py: meta-status hooks already wired in %s (no change)" % path)
        return
    save_settings(path, settings)
    print("wire.py: %d meta-status hook(s) wired into %s" % (wired, path))


def remove(path, command):
    settings = load_settings(path)
    hooks = settings.get("hooks")
    removed = 0
    if isinstance(hooks, dict):
        # Walk every event key: the pack wires six, and the exact-command
        # match makes anything found elsewhere ours to remove too.
        for event in list(hooks.keys()):
            entries = hooks[event]
            if not isinstance(entries, list):
                continue
            kept_entries = []
            for entry in entries:
                if not isinstance(entry, dict) or not isinstance(entry.get("hooks"), list):
                    kept_entries.append(entry)  # not ours to touch
                    continue
                kept_hooks = []
                for hook in entry["hooks"]:
                    if (
                        isinstance(hook, dict)
                        and hook.get("command") == command
                    ):
                        removed += 1  # exactly our block: the exact-command match
                    else:
                        kept_hooks.append(hook)
                if kept_hooks:
                    entry["hooks"] = kept_hooks
                    kept_entries.append(entry)
                # an entry left empty was ours (or empty before) — pruned
            if kept_entries:
                hooks[event] = kept_entries
            else:
                del hooks[event]
        if not hooks:
            del settings["hooks"]
    if removed:
        save_settings(path, settings)
        print("wire.py: %d meta-status hook(s) removed from %s" % (removed, path))
    else:
        print("wire.py: meta-status hooks not present in %s (no change)" % path)


def fail(message):
    sys.stderr.write("wire.py: %s\n" % message)
    sys.exit(1)


def main():
    args = sys.argv[1:]
    if len(args) != 3 or args[0] not in ("add", "remove"):
        fail("usage: wire.py add|remove <settings.json> <hook-command>")
    action, path, command = args
    if action == "add":
        add(path, command)
    else:
        remove(path, command)


if __name__ == "__main__":
    main()
