#!/bin/sh
# meta-usage hook — T343, the usage-events emitter pack (docs/meta-harness.md).
#
# Claude Code pipes one compact-JSON hook payload per event to stdin. The
# pack wires on Stop — the turn-complete event — and appends ONE JSONL line
# per turn to:
#
#     <claude-dir>/meta-usage.jsonl
#     { "ts": "…", "session_id": "…",
#       "model_requested": "…", "model_served": "…",
#       "input_tokens": n, "output_tokens": n,
#       "cache_read_tokens": n, "cache_creation_tokens": n,
#       "contract_version": "1" }
#
# PRESENCE CONTRACT (docs/meta-harness.md, the T343 section): a field is in
# the line only when the EVENT carries it — an absent value is an omitted
# key, never a fabricated zero, never the requested name copied into the
# served slot. Today's Claude Code Stop payloads carry none of the model or
# usage fields (the docs pin `last_assistant_message` as the only turn-final
# fact, and the transcript is deliberately NOT read here: it is written
# asynchronously and may not yet include the current turn, so mining it
# would attribute the PREVIOUS turn's usage to this one — exactly the
# wrongness this pack exists to end). A payload that does carry the fields —
# a harness re-emitting enriched events on the T327 pattern, or a future
# Claude build — is transcribed in full.
#
# Extraction contract (each rule is a pin in tests/meta_usage_hook.rs):
#   session_id      <- "session_id". A payload without a safe one emits no
#                      line at all: an unattributable turn is no row, not a
#                      blank one.
#   model_requested <- "model_requested" when the payload carries it, else
#                      "model" (the session's configured name — what was
#                      ASKED for).
#   model_served    <- "model_served" only. NEVER derived from "model": the
#                      served slot is filled only by an explicit as-served
#                      value (the T343 conflation rule, chug side too).
#   usage fields    <- the first `"usage":{…}` object's input_tokens,
#                      output_tokens, cache_read_input_tokens and
#                      cache_creation_input_tokens (the Messages-API usage
#                      shape), renamed to the line's cache_read_tokens /
#                      cache_creation_tokens. Extracted within the object
#                      only; a usage-less payload omits all four.
#
# Hygiene (the T342 rules, same reasons): every extracted value rides inside
# a JSON string AND inside a date format string, so a value carrying a
# percent sign, a backslash, or a non-printable byte is OMITTED — never
# half-escaped — as is anything over 200 chars.
#
# The write (the interesting part): the line IS a date format string and the
# shell `exec`s into `date`, so the whole handler is shell builtins plus a
# single exec, appending one open+write — no fork beyond date, no network,
# no shipping (Langfuse stays on the OTel path, NOT here), well under the
# spec's <2ms-per-turn budget (measured in tests/meta_usage_hook.rs).
# Appends are one write under PIPE_BUF with O_APPEND, so concurrent hook
# writers cannot interleave mid-line; every write is one valid JSONL line.
#
# Silent by contract: any payload that is not a Stop event, carries no
# session id, or yields nothing to transcribe exits 0 having written
# nothing. A non-zero exit can only come from an unwritable target, which
# Claude treats as a non-blocking warning (exit 2, the only blocking code
# for Stop-class hooks, is never produced).
#
# stdin is drained on every path (builtin loop, no fork) so Claude's pipe
# write never sees EPIPE. Run it by hand with `echo '{}' | hook.sh` — a
# tty stdin is never blocked on.

claude_dir=${CLAUDE_CONFIG_DIR:-$HOME/.claude}
usage_file=$claude_dir/meta-usage.jsonl

# Silent when unwatched: the log's parent IS the claude dir (the file is
# created on first append; the hook itself never mkdirs — the hot path pays
# no directory syscall beyond the append). A missing claude dir means the
# pack's target is gone: exit 0 having written nothing (the T342 posture —
# never a per-turn hook-error notice for an unwatched session).
[ -d "$claude_dir" ] || exit 0

# The payload's first line is the compact JSON Claude pipes; anything after
# it is drained for EPIPE safety only (a compact payload is always one
# line; a multi-line body is not transcribed rather than guessed at).
payload=
if [ ! -t 0 ]; then
  IFS= read -r payload || :
  while IFS= read -r _; do :; done
fi

# Only the turn-complete event transcribes (the pack is wired on Stop and
# never on anything else; both spacings are matched — the compact payload
# is the contract, the spaced form is defense in depth).
case $payload in
  *'"hook_event_name":"Stop"'*|*'"hook_event_name": "Stop"'*) ;;
  *) exit 0 ;;
esac

# session id: the line's attribution key. UUID-shaped in practice; anything
# outside the safe alphabet (or absurdly long) is skipped — an
# unattributable turn is no line, never a wrong one.
session=${payload#*'"session_id":"'}
session=${session%%'"'*}
case $session in
  ''|*[!A-Za-z0-9._-]*) exit 0 ;;
esac
[ ${#session} -le 100 ] || exit 0

# extract_str NEEDLE PAYLOAD -> REPLY: the first string value after NEEDLE,
# cut at its closing quote; empty when the needle is absent (the junk
# prefix a plain #* strip would leave behind is never mistaken for a value).
extract_str() {
  case $2 in
    *"$1"*)
      REPLY=${2#*"$1"}
      REPLY=${REPLY%%'"'*}
      ;;
    *) REPLY= ;;
  esac
}

# extract_num NEEDLE SOURCE -> REPLY: the leading digit run after NEEDLE;
# empty when the needle is absent or the value is not numeric (null, a
# string) — omitted, never fabricated. A real 0 rides: a zero the event
# CARRIES is data, not absence.
extract_num() {
  case $2 in
    *"$1"*)
      v=${2#*"$1"}
      REPLY=${v%%[!0-9]*}
      ;;
    *) REPLY= ;;
  esac
}

# hygiene: the value rides inside a JSON string AND a date format string.
# Percent signs and backslashes would be interpreted by date; non-printable
# or non-ASCII text cannot be written as one clean JSONL line — an impure
# value is OMITTED (the contract's absent-values rule), as is anything over
# 200 chars. Empty already-omitted values pass through.
sanitize() {
  case $REPLY in
    ''|*'%'*|*'\'*|*[!\ -~]*) REPLY= ;;
  esac
  [ ${#REPLY} -le 200 ] || REPLY=
}

extract_str '"model_requested":"' "$payload"
model_requested=$REPLY
if [ -z "$model_requested" ]; then
  # No explicit requested field: the payload's plain `model` is the
  # session's configured name — what was ASKED for. It fills the requested
  # slot and NEVER the served slot (the T343 rule).
  extract_str '"model":"' "$payload"
  model_requested=$REPLY
fi
sanitize
model_requested=$REPLY

extract_str '"model_served":"' "$payload"
model_served=$REPLY
sanitize
model_served=$REPLY

# The usage object: the first `"usage":{` in the payload (the Messages-API
# usage shape; `message.usage` — the transcript-line embedding — matches the
# same needle). Everything after the object's first closing brace is cut, so
# the extractions below cannot reach past it.
usage_body=
case $payload in
  *'"usage":{'*)
    usage_body=${payload#*'"usage":{'}
    usage_body=${usage_body%%\}*}
    ;;
esac

extract_num '"input_tokens":' "$usage_body"
input_tokens=$REPLY
extract_num '"output_tokens":' "$usage_body"
output_tokens=$REPLY
extract_num '"cache_read_input_tokens":' "$usage_body"
cache_read=$REPLY
extract_num '"cache_creation_input_tokens":' "$usage_body"
cache_creation=$REPLY

# The line, in the documented key order, present keys only.
line="{\"ts\":\"%Y-%m-%dT%H:%M:%SZ\",\"session_id\":\"$session\""
case $model_requested in
  ?*) line="$line,\"model_requested\":\"$model_requested\"" ;;
esac
case $model_served in
  ?*) line="$line,\"model_served\":\"$model_served\"" ;;
esac
case $input_tokens in
  ?*) line="$line,\"input_tokens\":$input_tokens" ;;
esac
case $output_tokens in
  ?*) line="$line,\"output_tokens\":$output_tokens" ;;
esac
case $cache_read in
  ?*) line="$line,\"cache_read_tokens\":$cache_read" ;;
esac
case $cache_creation in
  ?*) line="$line,\"cache_creation_tokens\":$cache_creation" ;;
esac
line="$line,\"contract_version\":\"1\"}"

# The write: this shell becomes `date`, whose format string IS the line.
# One append (O_APPEND, one write under PIPE_BUF), one trailing newline —
# one valid JSONL line per turn.
exec date -u +"$line" 1>> "$usage_file" 2>/dev/null
