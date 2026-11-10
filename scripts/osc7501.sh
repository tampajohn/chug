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
# address; T360 adds the constant `app=chug` label so herdr's Agents list
# groups the native bytes with the wrapper's). The state map lives at the
# CALL SITES (each supervisor's own transitions); this file owns only the
# bytes:
#     cycle/item start            -> working
#     cycle OK / dormant          -> idle  (T360: surfaced as `blocked` when
#                                    the newest cycle log carries a failure
#                                    marker — the scan below — the marker
#                                    line riding the msg channel verbatim)
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
# Wire (one sequence): ESC ] 7501 ; state=<s>[:app=chug][:msg=<base64>] ESC \
#   app=chug rides EVERY record (T360 — the wrapper's bytes carry it, so the
#   native bytes must too: herdr Agents-list grouping parity). msg is ONE
#   human-readable line, base64'd (the tr strips GNU wrapping);
#   the decoded msg stays <=2048 bytes and the whole sequence <=4096 (the
#   protocol's limits — one-line reasons sit far under both).
#   base64 absent -> the msg channel degrades away, the state rides alone.

# T360 — the blocked leg's CYCLE-LOG scan (the 2535e8af field finding): the
# failure markers a cycle leaves behind — the observability delivery
# failure, an error, a panic, a red gate — land in the PER-CYCLE log
# (.chug/loopd/cycle-<ts>.log, the launch's 2>> redirect), NEVER in the
# supervisor stream. A supervisor-stream-only blocked derivation silently
# never fires (Fleet Primary acceptance, 2026-11-10: the dead-langfuse
# cycle completed "cycle OK" and went dormant while its log carried
# `observability delivery failing (run continues): ... Connection refused`
# verbatim). BOTH supervisors' emission wrappers run this scan at an idle
# transition (the ONE composition point — a match rewrites the transition
# idle->blocked with the marker msg BEFORE the surfaces compose, so the 7501
# dot and the heartbeat line carry the same string). The LAST matching line
# wins (the most recent failure signal — the same last-marker-wins doctrine
# the T358 death derivation pins), bounded to 200 chars so the msg channel
# stays one short line. The four patterns, exactly as specified: the two
# phrases as substrings, `error` and `panic` as WHOLE words (bounded regex —
# a clean cycle's `errors: 0` summary or a Rust `panicked at` unwind line
# can never trip the block). Prints the marker line and rc 0 on a match;
# rc 1 when nothing matches, no cycle log exists, or the scan itself fails
# — the caller's `if` treats every non-zero as "no block", so a scan
# failure degrades to the plain idle emission, never silence of the dot
# (the T142 house style: this can never trip set -e/pipefail, and it
# writes nothing anywhere — stdout is the only channel and the caller
# captures it).
osc7501_cycle_block() { # — the newest cycle log's failure marker, if any
  local log
  log=$(ls -t .chug/loopd/cycle-*.log 2>/dev/null | head -1) || true
  [ -n "$log" ] && [ -f "$log" ] || return 1
  awk '
    {
      if ($0 ~ /(^|[^A-Za-z0-9_])error([^A-Za-z0-9_]|$)/ ||
          $0 ~ /(^|[^A-Za-z0-9_])panic([^A-Za-z0-9_]|$)/ ||
          index($0, "observability delivery failing") ||
          index($0, "gate red")) r = $0
    }
    END { if (r != "") { print substr(r, 1, 200); exit 0 } exit 1 }
  ' "$log"
}

osc7501_emit() { # <state> [one-line reason] — TTY-gated, env-killable
  local state="${1:-working}" msg="${2:-}" b64 seq rc=0
  [ -t 1 ] || return 0 # (a) a TTY stdout or nothing — the whole contract
  case "${CHUG_OSC7501-}" in # (b) unset (or empty) and exactly 1 emit
    ''|1) ;;
    *) return 0 ;;
  esac
  msg=${msg//$'\n'/ } # msg is ONE human-readable line, never a multiline
  # T360 — the app label rides EVERY record: the wrapper's bytes carry
  # app=chug, so the native bytes must too (herdr Agents-list grouping).
  seq="state=$state:app=chug"
  if [ -n "$msg" ]; then
    b64=$(printf '%s' "$msg" | base64 | tr -d '\n') || rc=$? # rc latched (T142)
    if [ "$rc" -eq 0 ] && [ -n "$b64" ]; then
      seq="$seq:msg=$b64"
    fi
  fi
  printf '\033]7501;%s\033\\' "$seq" || true # a closed stdout is silence, never a set -e death
  return 0
}
