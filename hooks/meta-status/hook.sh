#!/bin/sh
# meta-status hook — T342, the session-status mod pack (docs/meta-harness.md).
#
# Claude Code pipes one compact-JSON hook payload per event (PreToolUse,
# PostToolUse, UserPromptSubmit, Notification, Stop, SessionEnd) to stdin.
# This handler transcribes the event into the typed status document the
# meta-harness reader contract defines:
#
#     <claude-dir>/meta-status/<session-id>.json
#     { "status": "working|blocked|done",
#       "blocked_reason": permission|question|error|null,
#       "detail": "…",                      <- omitted when unknown
#       "updated_at": "…", "contract_version": "1" }
#
# It is a PURE TRANSCRIPTION: it reads the payload, rewrites one file, and
# exits. It never prompts, never alters output, never touches hook order
# or the session's own state — a no-op mod on every path.
#
# Silent when unwatched: no socket is opened, no daemon is contacted, no
# handler is spawned. The pack's total footprint is the one status file
# per session (plus nothing else); a missing status dir (the mod was never
# installed, or was removed) exits 0 having written nothing. An unknown
# event, an unsanitary session id, or a payload without the needed fields
# is skipped the same way — never a wrong write, never an error for the
# session (every exit is 0 or the exec's own; non-zero here can only come
# from an unwritable target, which Claude treats as a non-blocking warning
# — exit 2, the only blocking code for PreToolUse, is never produced).
#
# The write mechanism (the interesting part):
#
#   The document body is padded with spaces to a CONSTANT length and
#   written with ONE open of the final path read-write WITHOUT truncation
#   (`1<>`, offset 0) followed by ONE write from `date` — the shell `exec`s
#   into date, whose format string IS the document, so the whole handler is
#   shell builtins plus a single exec and costs ~1.3ms added over a no-op
#   hook (the spec's <2ms per event; measured interleaved best-of-N — see
#   tests/meta_status_hook.rs). Consequences:
#
#   - Readers see the previous or the new COMPLETE document, never a
#     partial one: the rewrite is one equal-length overwrite of the same
#     inode (no truncate window, no rename gap). The trailing spaces are
#     legal JSON whitespace.
#   - tmp+rename was priced and rejected: a second forked process per
#     hook event costs ~2-3ms on a loaded box — the whole budget — for a
#     guarantee the equal-length overwrite already delivers.
#   - The timestamp comes from `date` itself (the only fork-free-honest
#     clock available to a POSIX shell is none), RFC3339 UTC, second
#     precision — the staleness the reader contract needs.
#
# stdin is drained on every path (builtin loop, no fork) so Claude's pipe
# write never sees EPIPE. Run it by hand with `echo '{}' | hook.sh` — a
# tty stdin is never blocked on.

claude_dir=${CLAUDE_CONFIG_DIR:-$HOME/.claude}
status_dir=$claude_dir/meta-status

# The payload's first line is the compact JSON Claude pipes; anything
# after it is drained for EPIPE safety only (a compact payload is always
# one line; a multi-line body is not transcribed rather than guessed at).
payload=
if [ ! -t 0 ]; then
  IFS= read -r payload || :
  while IFS= read -r _; do :; done
fi

[ -d "$status_dir" ] || exit 0

# session id: the status file's key. UUID-shaped in practice; anything
# outside the safe filename alphabet (or absurdly long) is skipped —
# never a traversal, never a weird file name.
session=${payload#*'"session_id":"'}
session=${session%%'"'*}
case $session in
  ''|*[!A-Za-z0-9._-]*) exit 0 ;;
esac
[ ${#session} -le 100 ] || exit 0

event=${payload#*'"hook_event_name":"'}
event=${event%%'"'*}

status= reason=null detail=
case $event in
  PreToolUse)
    tool=${payload#*'"tool_name":"'}; tool=${tool%%'"'*}
    if [ "$tool" = AskUserQuestion ]; then
      # A question for the operator is pending until its PostToolUse (the
      # answer) or a turn end that leaves it pending — which Stop keeps.
      status=blocked reason='"question"'
    else
      status=working
    fi
    detail=$tool
    ;;
  PostToolUse)
    tool=${payload#*'"tool_name":"'}; tool=${tool%%'"'*}
    # A surfaced tool error is the blocked/error fact; the markers are
    # the ones Claude's own hook payloads carry (compact and spaced).
    case $payload in
      *'"isError":true'*|*'"isError": true'*|*'"is_error":true'*|*'"is_error": true'*)
        status=blocked reason='"error"' ;;
      *) status=working ;;
    esac
    detail=$tool
    ;;
  Notification)
    msg=${payload#*'"message":"'}; msg=${msg%%'"'*}
    case $msg in
      # Claude's permission-ask notification; other notifications (idle,
      # context) are not status facts and are not transcribed.
      *permission*|*Permission*)
        status=blocked reason='"permission"'
        detail=$msg
        ;;
      *) exit 0 ;;
    esac
    ;;
  UserPromptSubmit)
    status=working
    ;;
  SessionEnd)
    status=done
    ;;
  Stop)
    # A turn ending with a pending question is NOT done — the session
    # still waits on the operator, and the file is the only record of
    # that (PreToolUse AskUserQuestion wrote it). Read it back (builtin,
    # no fork) and keep it; any other turn end is done.
    current=
    if [ -f "$status_dir/$session.json" ]; then
      IFS= read -r current < "$status_dir/$session.json" || :
    fi
    case $current in
      *'"status":"blocked","blocked_reason":"question"'*) exit 0 ;;
    esac
    status=done
    ;;
  *)
    exit 0
    ;;
esac

# detail hygiene: the value rides inside a JSON string AND inside a date
# format string. The extraction already cuts at the first quote; percent
# signs and backslashes would be interpreted by date, and non-printable
# or non-ASCII text cannot be length-counted safely — an impure detail is
# OMITTED (never fabricated, never half-escaped), the contract's absent-
# values rule. Same for anything longer than 200 chars.
case $detail in
  *'%'*|*'\'*|*[!\ -~]*) detail= ;;
esac
[ ${#detail} -le 200 ] || detail=

case $detail in
  ?*) detail_field=",\"detail\":\"$detail\"" ;;
  *)  detail_field= ;;
esac

case $status in
  working|blocked|done) ;;
  *) exit 0 ;;
esac

doc="{\"status\":\"$status\",\"blocked_reason\":$reason${detail_field},\"updated_at\":\"%Y-%m-%dT%H:%M:%SZ\",\"contract_version\":\"1\"}"
# A detail long enough to crowd the 512-char body is dropped (absent,
# never truncated mid-escape); a document that STILL does not fit is a
# payload this handler does not transcribe.
if [ ${#doc} -gt 512 ]; then
  detail_field=
  doc="{\"status\":\"$status\",\"blocked_reason\":$reason,\"updated_at\":\"%Y-%m-%dT%H:%M:%SZ\",\"contract_version\":\"1\"}"
  [ ${#doc} -le 512 ] || exit 0
fi
# Pad to EXACTLY 512 chars with two expansions, not a per-character loop:
# the doubling chains build 512 spaces and 512 wildcards in ~15 cheap
# concatenations, then one trim expansion cuts doc+512spaces to its first
# 512 chars. A `while` pad loop costs ~4ms under load — the whole budget —
# because every append re-copies the string. 512 + date's %Y growth (+2)
# + date's trailing newline (+1) = the 515-byte document every rewrite
# produces.
sp='        '; sp=$sp$sp; sp=$sp$sp; sp=$sp$sp; sp=$sp$sp; sp=$sp$sp; sp=$sp$sp
q='?'; q=$q$q; q=$q$q; q=$q$q; q=$q$q; q=$q$q; q=$q$q; q=$q$q; q=$q$q; q=$q$q
doc="$doc$sp"
doc=${doc%"${doc#${q}}"}

# The write: this shell becomes `date`, whose format string IS the
# document. `1<>` opens the target read-write WITHOUT truncation at
# offset 0, so date's 512-byte output overwrites the previous body in one
# write — readers see the old or the new complete document, never a
# partial one (see the header).
exec date -u +"$doc" 1<> "$status_dir/$session.json" 2>/dev/null
