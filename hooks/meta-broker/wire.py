#!/usr/bin/env python3
"""wire.py — the meta-broker pack's settings.json surgeon (T341).

install.sh calls this to wire the pack's PreToolUse hook into Claude's
settings.json and to remove exactly its own block again:

    wire.py add    <settings.json> <hook-command>
    wire.py remove <settings.json> <hook-command>

`add` is additive and idempotent: existing hooks are untouched (values are
preserved verbatim — only the pack's own entry is appended), and a second
run is a no-op. `remove` drops only the hook objects carrying the pack's
exact command, then prunes the containers it emptied; everything else rides
untouched. The write is atomic (tmp + rename) and preserves the file's mode;
a malformed settings.json is named and left alone, never rewritten.
"""

import json
import os
import sys
import tempfile

PACK_MATCHER = "*"


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
    fd, tmp_path = tempfile.mkstemp(prefix=".meta-broker-wire-", dir=directory)
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


def hook_commands(settings):
    """Every command string under hooks.PreToolUse (whatever the entry
    shapes) — the idempotence and removal scan."""
    commands = []
    entries = settings.get("hooks", {}).get("PreToolUse", [])
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


def make_entry(command):
    return {
        "matcher": PACK_MATCHER,
        "hooks": [{"type": "command", "command": command}],
    }


def add(path, command):
    settings = load_settings(path)
    if command in hook_commands(settings):
        print("wire.py: meta-broker hook already wired in %s (no change)" % path)
        return
    hooks = settings.setdefault("hooks", {})
    if not isinstance(hooks, dict):
        fail("%s: hooks is not an object — wire the pack by hand; the file "
             "was left untouched" % path)
    pre = hooks.setdefault("PreToolUse", [])
    if not isinstance(pre, list):
        fail("%s: hooks.PreToolUse is not a list — wire the pack by hand; "
             "the file was left untouched" % path)
    pre.append(make_entry(command))
    save_settings(path, settings)
    print("wire.py: meta-broker hook wired into %s" % path)


def remove(path, command):
    settings = load_settings(path)
    hooks = settings.get("hooks")
    removed = 0
    if isinstance(hooks, dict) and isinstance(hooks.get("PreToolUse"), list):
        kept_entries = []
        for entry in hooks["PreToolUse"]:
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
        hooks["PreToolUse"] = kept_entries
        if not kept_entries:
            del hooks["PreToolUse"]
        if not hooks:
            del settings["hooks"]
    if removed:
        save_settings(path, settings)
        print("wire.py: meta-broker hook removed from %s" % path)
    else:
        print("wire.py: meta-broker hook not present in %s (no change)" % path)


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
