#!/bin/sh
# meta-broker PreToolUse hook — T341, the permission-broker hook pack.
#
# Claude Code runs this script before every tool call with the hook payload
# (JSON) on stdin. When a meta-harness broker is listening on
# ~/.claude/meta-broker.sock (or $CLAUDE_CONFIG_DIR/meta-broker.sock) the
# payload is forwarded to it and its decision is honored; otherwise the hook
# is a no-op and Claude's stock permission evaluation runs untouched.
#
# Fail-open by construction: socket absent, connect refused, >800ms, a
# malformed reply, a missing python3, or ANY handler error all land on the
# same path — exit 0 with no output. A broker crash can never wedge a
# session. The fast path (socket absent) is four shell builtins, well under
# the 5ms no-op budget; the handler lives in hook.py (python3) and only
# starts when a socket is actually present.
#
# stdin is drained on the fast path (builtin loop, no fork) so Claude's
# pipe write never sees EPIPE; the handler reads stdin itself. Run it by
# hand with `echo '{}' | hook.sh` — a tty stdin is never blocked on.

claude_dir=${CLAUDE_CONFIG_DIR:-$HOME/.claude}
if [ ! -S "$claude_dir/meta-broker.sock" ]; then
  # Socket absent: the stock path. Drain stdin so the pipe write cannot
  # EPIPE on Claude's side, then exit 0 with no output (<5ms).
  if [ ! -t 0 ]; then
    while IFS= read -r _; do :; done
  fi
  exit 0
fi

# Socket present: hand the payload to the handler. hook.py exits 0 on every
# path (decisions print to stdout; fail-opens print nothing); this script
# exits 0 regardless, so even a python3 crash cannot veto a tool call.
dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd) || exit 0
if command -v python3 >/dev/null 2>&1; then
  if [ -n "${META_BROKER_DEBUG:-}" ]; then
    python3 "$dir/hook.py"
  else
    # Zero interference: the handler's own stderr is silent by contract;
    # this only swallows a pathological python3 crash message.
    python3 "$dir/hook.py" 2>/dev/null
  fi
fi
exit 0
