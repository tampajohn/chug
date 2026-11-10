# osc7501.sh — the SHARED OSC 7501 program-status emitter (T358).
#
# ONE function body produces the 7501 bytes in BOTH supervisors: chug's own
# loopd.sh sources this file (resolved relative to ITS OWN path) and calls
# osc7501_emit from its T356 composition wrapper; the generated pilot
# supervisor (src/loop_cmd/loopd-pilot.sh — the script every `chug loop
# run` / fleet / herdr spawn actually executes, T334) sources the SAME file
# materialized beside it by `chug loop run` (write_supervisor_assets,
# include_str! from the live checkout — no copy to drift). Parity between
# the two supervisors is therefore parity by construction: there is no
# second implementation to drift.
#
# T354 — the wire and its semantics (unchanged): one OSC 7501 record on
# STDOUT — Mitchell Hashimoto's Program Status Protocol, the id-less ROOT
# record (state + msg, no id key — the harness's terminal IS the record's
# address). The state map lives at the CALL SITES (each supervisor's own
# transitions); this file owns only the bytes:
#     cycle/item start            -> working
#     cycle OK / dormant          -> idle
#     build-gate red / delivery (disposition commit) failing / cycle death /
#     HALT                        -> blocked, with the ONE-line reason on
#                                    the msg channel, headed by its T340
#                                    blocked-reason word (budget|error)
# THE COMPAT CONTRACT — read before touching (T354):
#   * emit ONLY when (a) stdout is a TTY *and* (b) CHUG_OSC7501 is unset or
#     =1. Any redirected context — nohup, launchd, a pipe, a log file —
#     receives ZERO escape bytes: log parsers must never see one.
#   * CHUG_OSC7501=0 (or any non-1 value) disables even on a TTY.
#   * Best-effort end to end, and it can never trip set -e/pipefail: every
#     leg is rc-guarded (the T142 house style), a closed stdout (EPIPE)
#     degrades to silence.
# Wire (one sequence): ESC ] 7501 ; state=<s>[:msg=<base64>] ESC \
#   msg is ONE human-readable line, base64'd (the tr strips GNU wrapping);
#   the decoded msg stays <=2048 bytes and the whole sequence <=4096 (the
#   protocol's limits — one-line reasons sit far under both).
#   base64 absent -> the msg channel degrades away, the state rides alone.

osc7501_emit() { # <state> [one-line reason] — TTY-gated, env-killable
  local state="${1:-working}" msg="${2:-}" b64 seq rc=0
  [ -t 1 ] || return 0 # (a) a TTY stdout or nothing — the whole contract
  case "${CHUG_OSC7501-}" in # (b) unset (or empty) and exactly 1 emit
    ''|1) ;;
    *) return 0 ;;
  esac
  msg=${msg//$'\n'/ } # msg is ONE human-readable line, never a multiline
  seq="state=$state"
  if [ -n "$msg" ]; then
    b64=$(printf '%s' "$msg" | base64 | tr -d '\n') || rc=$? # rc latched (T142)
    if [ "$rc" -eq 0 ] && [ -n "$b64" ]; then
      seq="$seq:msg=$b64"
    fi
  fi
  printf '\033]7501;%s\033\\' "$seq" || true # a closed stdout is silence, never a set -e death
  return 0
}
