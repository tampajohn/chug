#!/usr/bin/env bash
# loopd.sh — continuous self-improvement supervisor for chug.
#
# Runs LOOP-SPEC cycles back-to-back: each cycle's wrap (TODO.md,
# EVALUATION.md, specs/) is the next cycle's input — phases chain through
# files, not through a human. The loop is code, not conversation.
#
#   ./loopd.sh           run the supervisor loop (foreground; detach with
#                        nohup/setsid for a persistent daemon)
#   ./loopd.sh stop      ask the supervisor to exit after the current cycle
#   ./loopd.sh status    supervisor liveness + recent activity
#
# State lives in .chug/loopd/ (gitignored): loopd.pid, loopd.log,
# cycle-<timestamp>.log per cycle, HALTED if it gave up.
set -euo pipefail
# T137 — the -e/pipefail half of "failed builds do not prevent running an old
# binary": no command's failure may silently sail past the launch of
# ./target/release/chug, and a failing pipeline member (eval-digest, the
# verdict greps) may neither kill the supervisor mid-success nor flip its
# verdict. Every deliberately-best-effort command below carries its own
# `|| …` guard — the ones that don't are load-bearing and MUST fail loudly.
# T137 fix-up pipeline sweep (validator F1 class: pipeline legs whose rc
# semantics change under `set -o pipefail`) — every pipeline audited:
#   • single-driver probe   → DE-PIPELINED (captured, then grepped; rc latched)
#   • verdict marker        → DE-PIPELINED (herestring grep, no pipe)
#   • summary + status `ls` → rc-masked `|| true`; data survives because
#     `head -1` reads its line before any writer can die (proofs inline)
#   • eval-digest, site-sync, build, cycle → not pipelines; rc latched or
#     best-effort-guarded line by line.
ROOT="$(cd "$(dirname "$0")" && pwd)"
cd "$ROOT"
STATE=.chug/loopd
mkdir -p "$STATE"
STOP=.chug/STOP-LOOP
PIDFILE=$STATE/loopd.pid
LOG=$STATE/loopd.log

ts() { date -u +%Y-%m-%dT%H:%M:%SZ; }
# T219 — the supervisor's start instant, echoed as the /sessions registry
# entry's `started` (captured once here, sent at every cycle start; the
# daemon-side upsert keeps the FIRST-seen value, so the entry's age tracks
# the supervisor across cycles).
LOOPD_STARTED="$(ts)"

# T205 — the laya judge's private-checkpoint env reaches every cycle from
# ONE out-of-repo env file: default $HOME/.chug/loopd.env, override with
# CHUG_LOOPD_ENV. K7 wiring: the supervisor loads it ONCE at startup (the
# T47 lesson — no env mutation inside the cycle loop; a token rotation is
# edit + restart), and the orchestrator + delegate children + the spawned
# judge daemon all inherit the exports. Format: KEY=VALUE lines, `#`
# comments, optional `export ` prefix and one pair of surrounding quotes.
# ONLY the allowlisted judge keys are applied (a secret file must not
# inject arbitrary env into every cycle), and only when the operator has
# not already set them in the environment (explicit env wins). The file's
# CONTENTS are never echoed or logged (the T205 hygiene gate — tokens were
# once spilled by an env dump); only the path and a skipped-line COUNT may
# reach the log. Absent file is a silent no-op: public chug keeps the
# public base laya, no auth wall. (T213: the loader lives in scripts/loopd_env_loader.sh.)
LOOPD_ENV_FILE="${CHUG_LOOPD_ENV:-$HOME/.chug/loopd.env}"
# T213: the loader block moved BYTE-IDENTICAL into scripts/loopd_env_loader.sh
# (the sourceable seam tests/loopd_env_loader.rs drives — the M4 loader pin).
# The fragment is resolved relative to THIS script's own path, never the cwd
# (loopd.sh may be invoked or re-exec'd from anywhere).
. "$(dirname "$0")/scripts/loopd_env_loader.sh"
loopd_load_env_file

# T81 — per-phase model routing (operator-approved 2026-09-26). The
# ORCHESTRATOR's model is per-cycle and the switch is the freshness rule —
# never model judgment: when the queue holds `todo` rows AND EVALUATION.md
# is fresh (same UTC day), Phase 1 would skip evaluation, so the cycle is
# routine queue-working and launches on LOOP_ROUTINE_MODEL
# (anthropic-system.ai.glm-5-3-flash — ~85 tok/s, 11+ consecutive clean
# impl rounds); every other cycle is a fresh-eval cycle and launches on
# LOOP_ORCH_MODEL (anthropic-system.ai.kimi-k3 — today's behavior).
# Judgment stays kimi: fresh evaluations are kimi-only by construction
# (a routine cycle is exactly one where Phase 1 is skipped), and the
# spec's validation children are ALWAYS kimi — family independence, a
# glm-orchestrated cycle never lets glm validate glm.
# ROLLBACK: one env var restores single-model operation —
#   LOOP_ROUTINE_MODEL=anthropic-system.ai.kimi-k3 ./loopd.sh
LOOP_ORCH_MODEL="${LOOP_ORCH_MODEL:-anthropic-system.ai.kimi-k3}"
LOOP_ROUTINE_MODEL="${LOOP_ROUTINE_MODEL:-anthropic-system.ai.glm-5-3-flash}"

# The mechanical half of LOOP-SPEC Phase 1's freshness rule ("skip
# evaluation if TODO.md has `todo` rows AND EVALUATION.md is fresh (same
# UTC day)"), evaluated by the supervisor BEFORE launch so the
# routine-vs-eval switch is the RULE, not a model's opinion — and so the
# orchestrator's own Phase-1 decision (same predicate, same files, one
# single-driver writer) always agrees with it. CHUG_ROUTINE_TODAY pins the
# "today" side for tests, the eval-digest.sh CHUG_DIGEST_NOW pattern.
todo_rows() { # count of TODO.md data rows whose status cell is `todo`
  [ -f "$1" ] || { echo 0; return; }
  awk -F'|' '/^[|]/ {
    id=$2; gsub(/[ \t]/, "", id)
    if (id ~ /^T[0-9]+$/) { s=$6; gsub(/[ \t]/, "", s); if (s == "todo") n++ }
  } END { print n + 0 }' "$1"
}
eval_fresh() { # EVALUATION.md was written today (same UTC day)
  [ -f "$1" ] || return 1
  today="${CHUG_ROUTINE_TODAY:-$(date -u +%Y-%m-%d)}"
  if epoch=$(stat -f %m "$1" 2>/dev/null); then
    mday=$(TZ=UTC date -r "$epoch" +%Y-%m-%d)
  elif epoch=$(stat -c %Y "$1" 2>/dev/null); then
    mday=$(date -u -d "@$epoch" +%Y-%m-%d)
  else
    return 1
  fi
  [ "$mday" = "$today" ]
}
# route <TODO.md> <EVALUATION.md> — prints "<mode> <orchestrator-model>".
# The ONE place the switch is decided; both fields are echoed so callers
# can log the model next to the mode.
route() {
  if [ "$(todo_rows "$1")" -gt 0 ] && eval_fresh "$2"; then
    echo "routine $LOOP_ROUTINE_MODEL"
  else
    echo "eval $LOOP_ORCH_MODEL"
  fi
}

# T237 — empty-cycle backoff. The launch cadence had durably exceeded the
# work-arrival rate: with the queue drained, every cycle wrapped with an
# empty delta, and the flat 60s cycle-OK sleep kept the no-op cadence at
# ~20 minutes (~10M input tokens/day of pure burn, measured cycles
# 110–113). The signal is already in the git record,
# machine-greppable: each empty-delta wrap's commit subject carries the
# literal token the awk needle below matches (LOOP-SPEC Phase 3 makes
# the token load-bearing doctrine — a disposition wrap whose subject
# lost it would silently read as a reset and defeat the pacing). The
# walk counts consecutive empty-delta wraps newest-first, SKIPS pure
# bookkeeping (`eval:` commits that are not wrap notes — Outcomes
# compaction and the like) without stopping, and stops at anything else:
# a real wrap (an `eval:` wrap-notes subject without the token) or any
# landed work (a non-`eval:` subject). POSIX awk only (the BSD-sed/awk
# doctrine — no GNU-isms), and a non-git cwd, a missing git, or an empty
# log degrades to 0, never an error under set -e/pipefail (the rc is
# latched, the T142 house style).
empty_wrap_streak() { # consecutive empty-delta wrap commits, newest-first
  local subjects rc=0
  subjects=$(git log --format=%s -30 2>/dev/null) || rc=$?
  if [ "$rc" -ne 0 ] || [ -z "$subjects" ]; then
    echo 0
    return 0
  fi
  printf '%s\n' "$subjects" | awk '
    /^eval:/ {
      if (index($0, "empty-delta disposition") > 0) { n++; next }
      if (index($0, "wrap notes") > 0) { exit } # a real wrap: reset
      next                                      # eval bookkeeping: skip
    }
    { exit }                                    # landed work: reset
    END { print n + 0 }
  '
}

# T237 — the cycle-OK sleep. The T137 test seam wins first and
# byte-identically: LOOPD_SLEEP_OK set non-empty is echoed verbatim (four
# behavioral legs pin `LOOPD_SLEEP_OK=1`; explicit-set-wins). Otherwise
# the sleep scales with the empty-delta streak — 60s doubled once per
# consecutive empty wrap, capped at LOOPD_EMPTY_SLEEP_CAP (default 1800):
# 60 → 120 → 240 → 480 → 960 → 1800, so the fourth consecutive no-op
# cycle parks ~16 minutes and the cadence floor becomes one launch per
# 30 minutes; any real work resets the streak to 0 and the sleep to 60.
# The FAIL path is untouched (LOOPD_SLEEP_FAIL stays 300 flat). A
# POSIX-safe doubling loop, no GNU-isms; a non-numeric cap degrades to
# the base (the comparisons fail closed inside the loop guard, never a
# set -e death — condition contexts are exempt).
ok_sleep_seconds() {
  if [ -n "${LOOPD_SLEEP_OK:-}" ]; then
    echo "$LOOPD_SLEEP_OK"
    return 0
  fi
  local streak cap s
  streak=$(empty_wrap_streak)
  cap="${LOOPD_EMPTY_SLEEP_CAP:-1800}"
  s=60
  while [ "$s" -lt "$cap" ] && [ "$streak" -gt 0 ]; do
    s=$((s * 2))
    streak=$((streak - 1))
    if [ "$s" -gt "$cap" ]; then s=$cap; fi
  done
  if [ "$s" -gt "$cap" ]; then s=$cap; fi
  echo "$s"
}

# T259 — the Laya triage layer: the judge daemon (T204, the host-scoped 0600
# unix socket) gates the would-be eval launches the mechanical layer cannot
# settle. Three layers, the routing LOOP-SPEC Phase 1 names (mechanical ->
# Laya -> System Two): the cheap exit below settles an empty predicate at
# $0; when the predicate is non-empty the supervisor asks the daemon ONE
# classification question — needs-eval: yes/no — over a compact state pack
# every field of which is supervisor-computed (the same git/loopd.log
# record the gate and the valve read, never an LLM's say-so); the launched
# evaluation stays System Two. SPEC-3 constraint (quoted): laya does text
# classification ONLY — no counting, negation, or completion judgments —
# and "does this delta need a full eval" is a routing classification,
# never a quality verdict. FAIL-OPEN everywhere (req 3): the daemon
# absent, any error, or a >2s timeout falls back to EXACTLY the T258
# routing — one note per cycle in loopd.log, never a storm. Every triage
# is recorded to the decision corpus (.chug/decisions.jsonl, class
# laya-triage: the state pack verbatim, the verdict, the confidence, the
# route taken) and the next look backfills the outcome — the F13
# distillation corpus (scripts/decisions-export.sh joins them).
# The confidence threshold lives HERE, the one place (the LOOP-SPEC
# triage-layer paragraph points back at this constant):
TRIAGE_HIGH=0.85

triage_route() { # the pure cascade: <choice> <confidence> -> skip | glm | kimi
  # Confidence < TRIAGE_HIGH is unsure — the hard judgment escalates to
  # kimi whatever the choice says. At/above it the choice decides:
  # no -> skip ($0), yes -> glm (~$0.05). An unreadable confidence and an
  # unreadable choice are the same kind of unknown: kimi (the safe side —
  # a fail-open router must never fabricate a cheap skip).
  awk -v c="$1" -v conf="$2" -v high="$TRIAGE_HIGH" 'BEGIN {
    if (conf !~ /^[0-9]*\.?[0-9]+([eE][+-]?[0-9]+)?$/ || conf + 0 < high) { print "kimi"; exit }
    if (c == "no") print "skip"
    else if (c == "yes") print "glm"
    else print "kimi"
  }'
}

closed_todo_rows() { # baseline `todo` rows no longer `todo` (closed,
  # re-statused, or removed) — the pack's rows-closed half. Same shape as
  # new_todo_rows: the baseline copy from git, an unknown baseline
  # degrading to 0 (the bookkeeping leg catches the dirty-delta shape).
  local base=$1 rc=0
  [ -n "$base" ] || { echo 0; return 0; }
  [ -f TODO.md ] || { echo 0; return 0; }
  git show "$base:TODO.md" > "$STATE/triage-baseline-TODO.md" 2>/dev/null || { echo 0; return 0; }
  awk -F'|' '
    NR == FNR {
      id = $2; gsub(/[ \t]/, "", id)
      if (id ~ /^T[0-9]+$/) { s = $6; gsub(/[ \t]/, "", s); if (s == "todo") was[id] = 1 }
      next
    }
    {
      id = $2; gsub(/[ \t]/, "", id)
      if (id in was) seen[id] = 1
    }
    END { for (id in was) if (!(id in seen)) n++; print n + 0 }
  ' "$STATE/triage-baseline-TODO.md" TODO.md
}

changed_files_by_class() { # files changed since the $1 baseline, counted by
  # class — the pack's delta shape (where the changes landed, not just that
  # they did). An unknown baseline degrades to `unknown` (never a quiet 0).
  local base=$1 files f rc=0 src=0 tests=0 specs=0 docs=0 book=0 other=0
  [ -n "$base" ] || { echo unknown; return 0; }
  files=$(git diff --name-only "$base" HEAD 2>/dev/null) || rc=$?
  if [ "$rc" -ne 0 ]; then echo unknown; return 0; fi
  # T137 de-pipelined shape: captured above, then walked — no
  # printf-to-while pipe whose writer leg could SIGPIPE-flip under pipefail.
  while IFS= read -r f; do
    [ -n "$f" ] || continue
    case "$f" in
      EVALUATION.md|TODO.md) book=$((book + 1)) ;;
      src/*) src=$((src + 1)) ;;
      tests/*) tests=$((tests + 1)) ;;
      specs/*) specs=$((specs + 1)) ;;
      docs/*|runbooks/*|*.md) docs=$((docs + 1)) ;;
      *) other=$((other + 1)) ;;
    esac
  done <<EOF
$files
EOF
  echo "src=$src tests=$tests specs=$specs docs=$docs book=$book other=$other"
}

digest_stats() { # the T46 digest's corpus stats, compact — the pack's
  # digest half. A missing or unreadable digest degrades to `missing`
  # fields (never an error: the pack stays sendable, the judge sees the
  # degradation).
  local d=".chug/eval-digest.md" headln counts days
  [ -f "$d" ] || { echo missing; return 0; }
  headln=$(awk 'NR == 3 {
    for (i = 1; i <= NF; i++) {
      if ($i == "files:") f = $(i + 1)
      if ($i == "iterations:") t = $(i + 1)
    }
    print "files=" f + 0 " iters=" t + 0; exit
  }' "$d")
  counts=$(awk '/^- TODO\.md status counts: / { sub(/^- TODO\.md status counts: /, ""); print; exit }' "$d")
  days=$(awk '/^- days since last EVALUATION\.md write: / { sub(/^- days since last EVALUATION\.md write: /, ""); sub(/ .*/, ""); print; exit }' "$d")
  echo "${headln:-files=0 iters=0} ${counts:-statuses=unknown} days_since_eval=${days:-unknown}"
}

triage_request_body() { # the /judge request: the compact state pack + the
  # ONE needs-eval question. jq builds it — valid JSON or the triage fails
  # open (req 3); the field order here is the wire order the judge reads.
  command -v jq >/dev/null 2>&1 || return 1
  jq -cn \
    --arg rows_added "$1" --arg rows_closed "$2" --arg child_deaths "$3" \
    --arg files "$4" --arg digest "$5" --arg streak "$6" \
    --arg bookkeeping "$7" --arg fresh "$8" --arg tripped "$9" \
    '{
      state: {
        context: "A coding-loop supervisor (loopd) must decide, before every evaluation cycle, whether the delta since the last evaluation needs a full LLM evaluation launch or can be skipped at zero cost. Every input below is supervisor-computed from the git record and the loop log.",
        rows_added: $rows_added,
        rows_closed: $rows_closed,
        child_deaths: $child_deaths,
        files_changed: $files,
        digest_stats: $digest,
        empty_streak: $streak,
        bookkeeping_only_delta: $bookkeeping,
        evaluation_fresh: $fresh,
        valve_tripped: $tripped
      },
      questions: {
        needs_eval: {
          type: "choice",
          instructions: "Routing classification only, never a quality verdict: does the delta since the last evaluation need a full evaluation launch now? Bookkeeping-only deltas (dispositions, wrap notes, digest refreshes) are noise; landed work, queue movement, or child deaths are signal.",
          criteria: {
            no: "effectively empty - nothing a full evaluation would need to examine right now; skipping costs nothing",
            yes: "real signal - landed changes, queue movement, or child deaths that a full evaluation should examine"
          }
        }
      }
    }'
}

laya_gate_field() { # the value of one "<key>=" field in a gate line (empty
  # when absent) — the record writer and the disposition tag read the gate
  # line's fields back out of the one decision string. $1 carries its own
  # "=" (e.g. `laya=`): appending another would double it and never match.
  awk -v key="$1" '{
    for (i = 1; i <= NF; i++)
      if (index($i, key) == 1) { print substr($i, length(key) + 1); exit }
  }' <<<"$2"
}

laya_failopen() { # req 3: one note per cycle, never a storm — T258
  # behavior stands. The predicate probe sets LAYA_TRIAGE_QUIET (it writes
  # nothing); run mode notes once per cycle, which is once per triage.
  LAYA_VERDICT=""; LAYA_CONF=""; LAYA_ROUTE=""
  if [ -z "${LAYA_TRIAGE_QUIET:-}" ]; then
    echo "$(ts) laya triage (T259): FAIL-OPEN ($1) — T258 routing stands" >> "$LOG"
  fi
}

laya_triage() { # the System One triage call: the state pack -> ONE
  # needs-eval answer + confidence over the existing daemon socket (the
  # T204 resolution: $CHUG_DAEMON_SOCK, then $CHUG_HOME, then ~/.chug).
  # Sets LAYA_VERDICT/LAYA_CONF/LAYA_ROUTE — an empty route means
  # FAIL-OPEN and the caller keeps the T258 arm verbatim. Bounded at 2s
  # (req 3); the answer's confidence is normalized to a number (an
  # unreadable one IS zero confidence, and zero routes kimi).
  LAYA_VERDICT=""; LAYA_CONF=""; LAYA_ROUTE=""
  local sock="${CHUG_DAEMON_SOCK:-${CHUG_HOME:-$HOME/.chug}/daemon.sock}"
  local body resp status answer choice conf
  body=$(triage_request_body "$@") || { laya_failopen "state pack build failed (jq?)"; return 0; }
  resp=$(curl --unix-socket "$sock" --max-time 2 -s -w '\n%{http_code}' \
    -X POST -H 'Content-Type: application/json' -d "$body" \
    http://localhost/judge 2>/dev/null) || { laya_failopen "daemon unreachable at $sock"; return 0; }
  status=${resp##*$'\n'}
  resp=${resp%$'\n'*}
  [ "$status" = "200" ] || { laya_failopen "daemon HTTP $status"; return 0; }
  answer=$(printf '%s' "$resp" | jq -r '.answers.needs_eval | ((.choice // "?") + " " + ((.confidence // 0) | tostring))' 2>/dev/null) || answer=""
  choice=${answer%% *}
  conf=${answer#* }
  if [ -z "$choice" ] || [ "$choice" = "$answer" ]; then
    laya_failopen "unparsable judge answer"
    return 0
  fi
  conf=$(awk -v c="$conf" 'BEGIN {
    print (c ~ /^[0-9]*\.?[0-9]+([eE][+-]?[0-9]+)?$/) ? c + 0 : 0
  }')
  LAYA_VERDICT=$choice
  LAYA_CONF=$conf
  LAYA_ROUTE=$(triage_route "$choice" "$conf")
  if [ -z "${LAYA_TRIAGE_QUIET:-}" ]; then
    echo "$(ts) laya triage (T259): needs_eval=$LAYA_VERDICT conf=$LAYA_CONF (HIGH=$TRIAGE_HIGH) -> route=$LAYA_ROUTE" >> "$LOG"
  fi
}

laya_fields() { # the gate line's triage fields for the verdict laya_triage
  # left behind — one shape at both call sites (a real verdict carries
  # laya/conf/route; a fail-open reads laya=down).
  if [ -n "$LAYA_ROUTE" ]; then
    echo "laya=$LAYA_VERDICT conf=$LAYA_CONF route=$LAYA_ROUTE"
  else
    echo "laya=down conf=- route=-"
  fi
}

laya_record_write() { # append ONE decision record — jq builds it, so the
  # line is valid JSON or nothing lands (the corpus stays parseable, the
  # T199/T200 tooling never sees a torn line from this writer).
  jq -cn \
    --arg id "$1" --argjson ts "$2" --arg class "$3" --arg subject "$4" \
    --arg inputs "$5" --arg options "$6" --arg choice "$7" --argjson confidence "$8" \
    '{id: $id, ts: $ts, class: $class, subject: $subject, inputs: $inputs,
      options: $options, choice: $choice, confidence: $confidence}' \
    >> .chug/decisions.jsonl
}

laya_backfill() { # $1 = the parked pending record, $2 = THIS cycle's gate
  # line — the outcome label for the parked triage id (req 4: what the
  # loop found when it looked, backfilled next cycle; the T200 export
  # joins them into the accuracy census). Labels, supervisor-observable
  # only, documented in LOOP-SPEC: a skip held (landed-clean) while the
  # delta stayed bookkeeping-only; a skip overtaken by real work (the
  # delta went non-bookkeeping) did not (fixed-up); a launch held when its
  # evaluation landed the artifacts (the baseline moved) and did not when
  # the next look still finds the baseline unchanged. `reverted` is never
  # emitted: the supervisor has no revert path.
  # ONE outcome per triage id, ever: the pending park is consumed exactly
  # once (the rm in laya_record, right after this backfill) — a look that
  # backfills but parks nothing (a mechanical skip, a fail-open) leaves no
  # stale record behind, so no later look can ever double-backfill the same
  # id (pinned by a_pending_surviving_an_intervening_non_triage_cycle_
  # backfills_exactly_once).
  local pend=$1 gate=$2 id route pbase cur_base cur_book choice what
  id=$(jq -r '.id // ""' "$pend" 2>/dev/null) || id=""
  if [ -z "$id" ]; then
    echo "$(ts) laya triage (T259): unreadable pending record — backfill skipped" >> "$LOG"
    return 0
  fi
  route=$(jq -r '.route // ""' "$pend" 2>/dev/null) || route=""
  pbase=$(jq -r '.base // ""' "$pend" 2>/dev/null) || pbase=""
  cur_base=$(laya_gate_field base= "$gate")
  cur_book=$(laya_gate_field bookkeeping= "$gate")
  if [ "$route" = skip ]; then
    if [ "$cur_book" = no ]; then
      choice="fixed-up"; what="the delta went non-bookkeeping after the skip"
    else
      choice="landed-clean"; what="the delta stayed bookkeeping-only through the next look"
    fi
  else
    if [ -n "$cur_base" ] && [ "$cur_base" != "$pbase" ]; then
      choice="landed-clean"; what="the launched evaluation landed its artifacts (the baseline moved)"
    else
      choice="fixed-up"; what="no evaluation artifacts since the launch (the baseline is unchanged)"
    fi
  fi
  if laya_record_write "$id" "$(date -u +%s)" "outcome" "$id" \
      "what the loop found when it looked: $what; bookkeeping=$cur_book base=${cur_base:-none}" \
      "landed-clean | fixed-up | reverted" "$choice" 1; then
    echo "$(ts) laya triage (T259): outcome backfill $id -> $choice ($what)" >> "$LOG"
  else
    echo "$(ts) laya triage (T259): outcome backfill write FAILED — continuing" >> "$LOG"
  fi
}

laya_record() { # T259 req 4 — the decision_log record per triage + the
  # previous triage's outcome backfill. RUN MODE ONLY (the predicate probe
  # never writes), best-effort throughout: a jq-less or unwritable host
  # skips the corpus, never the cycle.
  local gate=$1 pending="$STATE/triage-pending.json"
  command -v jq >/dev/null 2>&1 || return 0
  if [ -f "$pending" ]; then
    laya_backfill "$pending" "$gate"
    rm -f "$pending"
  fi
  local verdict conf route streak tripped new closed deaths book fresh
  verdict=$(laya_gate_field laya= "$gate")
  case "$verdict" in
    yes|no) ;;
    *) return 0 ;; # laya=none (the mechanical layer settled it) or laya=down (fail-open)
  esac
  conf=$(laya_gate_field conf= "$gate")
  route=$(laya_gate_field route= "$gate")
  streak=$(laya_gate_field streak= "$gate")
  new=$(laya_gate_field new= "$gate")
  closed=$(laya_gate_field closed= "$gate")
  deaths=$(laya_gate_field deaths= "$gate")
  book=$(laya_gate_field bookkeeping= "$gate")
  fresh=$(laya_gate_field fresh= "$gate")
  if [ "$streak" -ge 3 ]; then tripped=yes; else tripped=no; fi
  # The pack verbatim, re-derived from the SAME inputs the gate just sent
  # (pure functions of the gate line + the repo state; the record carries
  # byte-for-byte what the judge answered about).
  local pack
  pack=$(triage_request_body "$new" "$closed" "$deaths" \
    "$(changed_files_by_class "$(laya_gate_field base= "$gate")")" \
    "$(digest_stats)" "$streak" "$book" "$fresh" "$tripped" | jq -c '.state' 2>/dev/null) || pack="{}"
  local now id
  now=$(date -u +%s)
  LAYA_SEQ=$((LAYA_SEQ + 1))
  id="d${now}-loopd${LAYA_SEQ}"
  if laya_record_write "$id" "$now" "laya-triage" \
      "cycle-$(date -u +%Y%m%d-%H%M%S) eval triage (T259)" \
      "${pack:-{}} | verdict=needs_eval:$verdict conf=$conf route=$route" \
      "skip | glm | kimi" "$route" "$conf"; then
    # park the id for the next look's outcome backfill
    jq -cn --arg id "$id" --arg route "$route" \
      --arg base "$(laya_gate_field base= "$gate")" \
      --arg deaths "$deaths" \
      '{id: $id, route: $route, base: $base, deaths: $deaths}' > "$pending" 2>/dev/null \
      || rm -f "$pending"
  else
    echo "$(ts) laya triage (T259): decision record write FAILED — continuing" >> "$LOG"
  fi
}

# T258 — the eval-cycle gate: the disposition predicate, computed by the
# supervisor BEFORE any launch so an empty delta costs zero LLM calls. The
# T247 chain rule decides when a skipped-eval chain must STOP (the valve);
# this gate decides whether a would-be eval cycle needs to LAUNCH at all.
# Four mechanical inputs, every one supervisor-computed (the T237 streak
# and the T247 valve read the same git/loopd.log record — never an LLM's
# say-so):
#   new rows — TODO.md row ids gained since the baseline commit (the newest
#              commit touching EVALUATION.md: the last examination of loop
#              state, a real eval or a disposition alike — both examined);
#   deaths   — cycles that ended WITHOUT goal complete in loopd.log after
#              the baseline commit time: the child runs THIS supervisor
#              owns, the one death record loopd holds without an LLM;
#   delta    — bookkeeping-only since the baseline: every commit an `eval:`
#              commit AND the endpoint diff touching nothing outside the
#              bookkeeping pair {EVALUATION.md, TODO.md};
#   fresh    — EVALUATION.md fresh (same UTC day), the eval_fresh input the
#              T81 routing already reads.
# THE SAFE-SIDE RULE: the skip is the only decision that requires PROOF —
# every unknown (a non-git cwd, a missing baseline, a failed git probe)
# degrades to a launch, never to a skip. The gate can only ever add
# launches over the T81 routing, never suppress evidence from a real eval.
last_eval_commit() { # newest commit touching EVALUATION.md; empty = unknown
  git log -1 --format=%H -- EVALUATION.md 2>/dev/null || true
}
new_todo_rows() { # TODO.md row ids present now, absent from the $1 baseline
  local base=$1 rc=0
  [ -n "$base" ] || { echo 0; return 0; }
  [ -f TODO.md ] || { echo 0; return 0; }
  git show "$base:TODO.md" > "$STATE/predicate-baseline-TODO.md" 2>/dev/null || rc=$?
  if [ "$rc" -ne 0 ]; then echo 0; return 0; fi
  # (a TODO.md absent at the baseline degrades to 0 here; the bookkeeping
  # leg catches that shape anyway — the commits that added the file are
  # not `eval:` commits, so the delta reads dirty and the cycle launches.)
  awk -F'|' '
    NR == FNR {
      id = $2; gsub(/[ \t]/, "", id)
      if (id ~ /^T[0-9]+$/) seen[id] = 1
      next
    }
    {
      id = $2; gsub(/[ \t]/, "", id)
      if (id ~ /^T[0-9]+$/ && !(id in seen)) n++
    }
    END { print n + 0 }
  ' "$STATE/predicate-baseline-TODO.md" TODO.md
}
cycle_deaths() { # dead cycles in loopd.log strictly after the $1 baseline time
  local base=$1 epoch="" since=""
  if [ -n "$base" ]; then
    epoch=$(git log -1 --format=%ct "$base" 2>/dev/null) || epoch=""
  fi
  if [ -n "$epoch" ]; then
    # the log stamps UTC (date -u); the commit epoch converts the same way
    # (the eval_fresh dual-form pattern: BSD -r, GNU -d @)
    since=$(date -u -r "$epoch" +%Y-%m-%dT%H:%M:%S 2>/dev/null \
      || date -u -d "@$epoch" +%Y-%m-%dT%H:%M:%S 2>/dev/null \
      || true)
  fi
  [ -f "$LOG" ] || {
    # T258 fix-up (validator F3d): a MISSING log is an UNKNOWN record, not a
    # quiet one — the old `echo 0` was the one exception to the gate's own
    # every-unknown-degrades-to-launch rule (a removed or never-written log
    # would read as "no deaths ever" and cheap-exit over a real death the
    # record cannot show). The safe side: the caller sees `unknown`, the
    # skip arm's `deaths = 0` never fires, and the gate launches — on the
    # orchestrator model, since an unknown count is not borderline either.
    echo unknown
    return 0
  }
  if [ -n "$since" ]; then
    # same-second reads as a death (>=): the dangerous direction is the
    # false-negative skip over a real death, never the phantom launch
    awk -v since="$since" '
      /cycle ended WITHOUT goal complete/ {
        if (substr($1, 1, 19) >= since) n++
      }
      END { print n + 0 }
    ' "$LOG"
  else
    # no comparable baseline time: count every recorded death — an unknown
    # window never reads as quiet (the safe-side rule)
    awk '/cycle ended WITHOUT goal complete/ { n++ } END { print n + 0 }' "$LOG"
  fi
}
delta_bookkeeping_only() { # every commit since the $1 baseline is eval-bookkeeping
  local base=$1 rc=0 subjects files f
  if [ -z "$base" ]; then return 1; fi
  subjects=$(git log --format=%s "$base..HEAD" 2>/dev/null) || rc=$?
  if [ "$rc" -ne 0 ]; then return 1; fi
  # T137 de-pipelined shape: captured above, then walked — no
  # printf-to-grep pipe whose writer leg could SIGPIPE-flip under pipefail.
  if [ -n "$subjects" ]; then
    while IFS= read -r f; do
      case "$f" in eval:*) ;; *) return 1 ;; esac
    done <<EOF
$subjects
EOF
  fi
  files=$(git diff --name-only "$base" HEAD 2>/dev/null) || rc=$?
  if [ "$rc" -ne 0 ]; then return 1; fi
  if [ -n "$files" ]; then
    while IFS= read -r f; do
      case "$f" in EVALUATION.md|TODO.md) ;; *) return 1 ;; esac
    done <<EOF
$files
EOF
  fi
  return 0
}
eval_gate() { # the T258/T259 decision for a would-be eval cycle, one line:
  #   skip <fields>            — the cheap exit: launch skipped, loopd
  #                              writes the disposition itself (no LLM) —
  #                              either the mechanical predicate is empty
  #                              (T258) or the Laya triage is confidently
  #                              empty (T259)
  #   launch <model> <fields>  — launch the eval on <model>: the FULL
  #                              provider-routed id resolved from the T81
  #                              env, NEVER a bare shorthand (model ids go
  #                              verbatim into the API request body —
  #                              src/api.rs body.insert("model", ...) — and
  #                              there is no alias layer, so `--model glm`
  #                              would die at the first call)
  # fields: rows= new= deaths= bookkeeping= fresh= base= streak=
  #         closed= laya= conf= route=
  #   laya=none          the triage layer did not run (the mechanical layer
  #                      settled the cycle, or a stale evaluation is due)
  #   laya=down          fail-open (daemon absent/error/timeout) — T258
  #                      routing stands verbatim
  #   laya=yes|no conf=<c> route=skip|glm|kimi — the triage verdict, its
  #                      confidence, and the route TAKEN
  local base new deaths book fresh streak verb model
  base=$(last_eval_commit)
  new=$(new_todo_rows "$base")
  deaths=$(cycle_deaths "$base")
  if delta_bookkeeping_only "$base"; then book=yes; else book=no; fi
  if eval_fresh EVALUATION.md; then fresh=yes; else fresh=no; fi
  streak=$(empty_wrap_streak)
  local closed laya="laya=none"
  closed=$(closed_todo_rows "$base")
  # the valve FIRST (T247): the 4th consecutive empty cycle runs the real
  # evaluation whatever the delta says — the chain converts itself. A trip
  # is a REAL evaluation: the orchestrator model (the full $LOOP_ORCH_MODEL
  # id), never the cheap routine model.
  # T259: the trip consults the SAME triage before it launches — only a
  # confident-empty (needs-eval=no at/above $TRIAGE_HIGH) on a FRESH
  # evaluation cancels it (the trip disposition records the verdict, no
  # kimi stream). A confident-empty on a STALE evaluation cannot cancel:
  # the daily full evaluation is due, and the T258 doctrine keeps it on
  # the orchestrator model — the verdict is still recorded, the taken
  # route reads kimi. Every other answer, and a fail-open, launches.
  if [ "$streak" -ge 3 ]; then
    verb=launch; model=$LOOP_ORCH_MODEL
    laya_triage "$new" "$closed" "$deaths" "$(changed_files_by_class "$base")" "$(digest_stats)" "$streak" "$book" "$fresh" yes
    laya="$(laya_fields)"
    if [ "$LAYA_ROUTE" = skip ]; then
      if [ "$fresh" = yes ]; then
        verb=skip; model=-
      else
        # the daily full evaluation is due — the confident-empty cannot
        # cancel it; the verdict is recorded, the taken route reads kimi
        laya="laya=$LAYA_VERDICT conf=$LAYA_CONF route=kimi"
      fi
    elif [ -n "$LAYA_ROUTE" ]; then
      # the trip launches the real evaluation whatever the triage answered
      # (yes, unsure, anything) — the taken route reads kimi, the verdict
      # and confidence stay in the line and the record
      laya="laya=$LAYA_VERDICT conf=$LAYA_CONF route=kimi"
    fi
  elif [ "$new" = 0 ] && [ "$deaths" = 0 ] && [ "$book" = yes ] && [ "$fresh" = yes ]; then
    verb=skip; model=-
  elif [ "$book" = yes ] && [ "$fresh" = yes ] && [ "$deaths" != unknown ]; then
    # borderline (T258 req 2): non-empty only through deaths or new rows,
    # over a bookkeeping-only delta with a fresh evaluation — the routine
    # model runs the bounded eval; the corpus is unchanged and today's
    # evaluation already current, so no fresh-evaluation judgment is due.
    # $LOOP_ROUTINE_MODEL is the full id (glm by default), and the T81
    # rollback knob rides along: LOOP_ROUTINE_MODEL set to the kimi id
    # routes borderline evals to kimi too — single-model operation. A
    # deaths=unknown is NOT borderline: an unreadable record may hide a real
    # death, and the safe side never cheapens out on an unknown.
    # T259: the triage layer routes this would-be launch — the mechanical
    # inputs say "maybe nothing"; Laya asks the ONE question and the
    # confidence-gated cascade routes it: confident-empty -> the cheap
    # disposition ($0), yes -> glm, unsure (below $TRIAGE_HIGH) -> kimi
    # (the hard judgment stays System Two). Fail-open (an empty route)
    # keeps T258 exactly: glm.
    laya_triage "$new" "$closed" "$deaths" "$(changed_files_by_class "$base")" "$(digest_stats)" "$streak" "$book" "$fresh" no
    laya="$(laya_fields)"
    verb=launch; model=$LOOP_ROUTINE_MODEL
    case "$LAYA_ROUTE" in
      skip) verb=skip; model=- ;;
      kimi) verb=launch
            model=$LOOP_ORCH_MODEL ;; # the unsure escalation — System Two, full id never a shorthand
      *) ;;
    esac
  else
    # a non-bookkeeping delta (source/spec/doctrine work landed), a stale
    # evaluation, or an unknown deaths count: the fresh-evaluation boundary
    # stays on the orchestrator model (T81). System Two, no triage: these
    # are the shapes the T258 routing already decided mechanically.
    verb=launch; model=$LOOP_ORCH_MODEL
  fi
  echo "$verb $model rows=$(todo_rows TODO.md) new=$new deaths=$deaths bookkeeping=$book fresh=$fresh base=${base:-none} streak=$streak closed=$closed $laya"
}
loopd_write_disposition() { # T258 — the supervisor writes the one-line
  # disposition ITSELF (the cheap exit, no LLM): an empty commit whose
  # subject carries the T237 token (so the streak walk and the valve count
  # it), the TRUE streak handoff, and the gate's input fields. The token is
  # assembled from halves so this source keeps EXACTLY ONE contiguous
  # occurrence of it (the awk needle — a second literal in this template
  # would mask a needle-removal mutant under the T237 count pin).
  # T259: a triaged skip names its verdict in the subject — the layer that
  # cancelled the launch (the triage on a borderline cycle, or the triage
  # on a valve trip) must be legible in the git record; a mechanical skip
  # keeps the T258 subject byte-identical.
  local t1="empty-delta" t2="disposition" streak snext fields rc=0 verdict conf reason
  # strip "skip - " (the verb + the no-model placeholder) — the fields line
  # in the subject starts at the predicate inputs
  fields=${1#skip - }
  verdict=$(laya_gate_field laya= "$fields")
  conf=$(laya_gate_field conf= "$fields")
  streak=$(empty_wrap_streak)
  reason="(T258 mechanical: predicate empty, launch skipped)"
  case "$verdict" in
    yes|no)
      if [ "$streak" -ge 3 ]; then
        reason="(T259 laya trip: needs-eval=$verdict conf=$conf — the valve's launch cancelled)"
      else
        reason="(T259 laya triage: needs-eval=$verdict conf=$conf — launch skipped)"
      fi
      ;;
  esac
  snext=$((streak + 1))
  git commit --allow-empty -m "eval: loopd cheap-exit ${t1} ${t2} ${reason} — TRUE streak ${streak}→${snext}; ${fields}; next cycle re-checks, the T247 valve still binds on the TRUE count" >/dev/null 2>&1 || rc=$?
  return "$rc"
}

# T259 — the triage layer's per-run state: the record-sequence suffix
# (ids stay unique per supervisor run) and the quiet flag (the predicate
# probe sets it — the probe writes nothing; run mode leaves it empty so
# each cycle notes its one triage line).
LAYA_SEQ=0
LAYA_TRIAGE_QUIET=""

case "${1:-run}" in
  stop)
    touch "$STOP"
    echo "stop requested — supervisor exits after the current cycle"
    exit 0
    ;;
  routing)
    # T81: print the routing decision (mode + orchestrator model) for the
    # cycle that WOULD launch now, launching nothing — the operator's
    # probe and the test surface for the freshness predicate
    # (tests/loopd_model_routing.rs runs this mode against fixtures).
    route TODO.md EVALUATION.md
    exit 0
    ;;
  sleep-ok)
    # T237: print "<seconds> <streak>" — the cycle-OK sleep that WOULD
    # follow a successful cycle now, sleeping nothing (the `routing`
    # probe pattern: the operator's and the tests' behavioral surface
    # for the empty-delta backoff; tests/loopd_empty_backoff.rs runs
    # this mode against fixture git repos).
    echo "$(ok_sleep_seconds) $(empty_wrap_streak)"
    exit 0
    ;;
  predicate)
    # T258: print the eval-cycle gate decision for a WOULD-BE eval cycle,
    # launching nothing and writing nothing — the routing/sleep-ok probe
    # pattern and the operator's + tests' behavioral surface for the
    # cheap exit (req 5): "<verb> <model> rows=... new=... deaths=...
    # bookkeeping=... fresh=... base=... streak=... closed=... laya=...".
    # skip = the launch is skipped and loopd writes the disposition itself;
    # launch names the routed orchestrator model. Meaningful in eval mode
    # (the T81 routing decided eval); routine cycles never reach the gate.
    # T259: the triage call stays reachable from the probe (the operator's
    # dry-run surface), but QUIET — a probe notes nothing into loopd.log.
    LAYA_TRIAGE_QUIET=1
    eval_gate
    exit 0
    ;;
  status)
    if [ -f "$PIDFILE" ] && kill -0 "$(cat "$PIDFILE")" 2>/dev/null; then
      echo "loopd RUNNING (pid $(cat "$PIDFILE"))"
    else
      echo "loopd NOT running"
    fi
    # T137: status is a report, not a gate — every leg degrades to "nothing
    # to report" and must never trip `set -e` (a missing log/cycle file is
    # normal on a fresh install, `[ -f ] && echo` is nonzero without HALTED,
    # and under pipefail a no-match `ls` glob fails the pipeline).
    # T137 fix-up sweep: the ls-to-head pipeline below is rc-insensitive by
    # design — `|| true` masks EVERY leg's rc (the no-match glob's rc 2 AND
    # a SIGPIPE when `head -1` stops reading a long history early), and the
    # data survives because head has already read the first line (the newest
    # log) before any writer can die: SIGPIPE only strikes a write made
    # after the reader is gone. The report stays correct.
    [ -f "$STATE/HALTED" ] && echo "HALTED: $(cat "$STATE/HALTED")" || true
    echo "--- recent:"
    tail -5 "$LOG" 2>/dev/null || true
    latest=$(ls -t "$STATE"/cycle-*.log 2>/dev/null | head -1 || true)
    if [ -n "$latest" ]; then
      echo "--- latest cycle ($latest):"
      tail -3 "$latest" || true
    fi
    exit 0
    ;;
  run) ;;
  *) echo "usage: loopd.sh [run|stop|status|routing] [sleep-ok] [predicate]" >&2; exit 2 ;;
esac

# T50: same-pid pass. `exec` preserves the pid, so a re-exec'd self finds its
# own LIVE pid already in the pidfile — refusing on that would kill the
# re-exec at startup. Refuse only when the recorded pid is alive AND not ours
# (a foreign live supervisor keeps today's refusal); a stale (dead) pid keeps
# the fall-through it always had. On pass we re-write the pidfile and re-arm
# the EXIT trap below, as on the normal path.
if [ -f "$PIDFILE" ] && [ "$(cat "$PIDFILE")" != "$$" ] \
   && kill -0 "$(cat "$PIDFILE")" 2>/dev/null; then
  echo "loopd already running (pid $(cat "$PIDFILE"))" >&2
  exit 1
fi
echo $$ > "$PIDFILE"
rm -f "$STOP" "$STATE/HALTED"
trap 'rm -f "$PIDFILE"' EXIT
# T47: shared CARGO_TARGET_DIR for every worktree build (children + orchestrator
# gates) — a NEW dir, NOT the repo's own target/, so a poisoned cache can't
# touch the operator's daily builds. Clean policy: none automatic — reclaiming
# is the operator's call (`du -sh target-shared`; recovery from a poisoned
# cache is `rm -rf target-shared`, cheap, rebuilt once and warm for all).
# T194: validator children are SLOT-keyed — target-shared-validate-a /
# target-shared-validate-b, one dir per validator slot (a = the solo
# default, b = the pattern-(iv) second validator, which flies only when two
# items are simultaneously past gates, never two validators on one item) —
# so two concurrent validators never share a target dir (the T47 invariant:
# no two cargo processes share a target dir; sharing one would serialize on
# cargo's build lock and erase the parallelism). The supervisor never sets
# a slot dir itself: children receive theirs through their delegate goal
# export + env map (LOOP-SPEC step 4) — this script's own build stays pinned
# to ./target below, the cycle's orchestrator keeps the target-shared prefix
# on the chug invocation, and the wrap gates keep their own role-keyed dirs
# per LOOP-SPEC steps 3 and 5. Loop-SPEC doctrine says orchestrators key
# validator slots at dispatch; this comment records the convention where the
# T47 shared-cache discipline lives.
mkdir -p target-shared

echo "$(ts) loopd start (pid $$)" >> "$LOG"
# T82: nextest-first gates (LOOP-SPEC step 3, META-SPEC gates rule). The
# supervisor installs NOTHING — it only checks for the host tool and logs
# which gate runner this install's cycles use, so loopd.log always names
# the gate form; absence of cargo-nextest degrades to the unconditional
# fallback by construction (nextest is never a hard dependency).
if command -v cargo-nextest >/dev/null 2>&1; then
  echo "$(ts) gate runner: cargo nextest run --release (cargo-nextest on PATH)" >> "$LOG"
else
  echo "$(ts) gate runner: cargo test --release -- --test-threads=4 (fallback — cargo-nextest absent)" >> "$LOG"
fi
# T215 — resolve the judge-daemon binary ONCE per loopd run (the probe cache).
# The daemon is HOST-SCOPED (one per box, serves any run over
# $CHUG_HOME/daemon.sock), so its carrier is the INSTALLED release binary —
# never the repo dev build: T204 keeps loopd's own builds feature-lean, so
# ./target/release/chug is a CLIENTS-ONLY binary whose `daemon` subcommand
# is the fail-open stub. That stub is the K7 diagnosis: every cycle-start
# ensure exited nonzero with the feature-off refusal (the literal the (c)
# leg greps for, below) for ~16h, fail-open, while the release workflow's
# daemon-capable binaries sat uninstalled in ~/.local/bin. Resolution order:
#   (a) $CHUG_DAEMON_BIN — the operator's explicit override, used as-is when
#       executable (no probe: an explicit choice is not second-guessed; a
#       set-but-unusable value is noted and the list falls through);
#   (b) $HOME/.local/bin/chug — install.sh's release install location — when
#       it REPORTS daemon support: `chug daemon --help` exit 0 (a pre-T204
#       release refuses the subcommand, so the probe doubles as the
#       dogfood-upgrade detector; the dashboard daemon owns those upgrades, never loopd);
#   (c) ./target/release/chug — only when it hosts the judge, detected by the
#       ABSENCE of the feature-off refusal literal (src/daemon.rs
#       real_backend's cfg(not(feature = "daemon")) bail — the same string
#       daemon.log logged six times). Under loopd this leg never fires (the
#       build gate rebuilds feature-lean every cycle) — it exists for a
#       pre-existing operator-built daemon-capable binary at startup.
# Nothing daemon-capable → the ensure is SKIPPED for the whole run: the one
# log line below is the record (fail-open — the judge client degrades per
# command exactly as before; repo dev builds are clients only). The probe
# runs ONCE here, never per cycle: the installed binary changes on dogfood
# upgrades, not on cycles — a re-exec (script changed on disk) is a fresh
# run and re-resolves.
DAEMON_BIN=""
DAEMON_SKIP_REASON=""
DAEMON_OVERRIDE_NOTE=""
judge_probe_ok() { # daemon-capable? the probe: subcommand recognized (exit 0)
  "$1" daemon --help >/dev/null 2>&1
}
resolve_daemon_bin() { # sets DAEMON_BIN / DAEMON_SKIP_REASON; always rc 0
  DAEMON_BIN=""
  DAEMON_SKIP_REASON=""
  DAEMON_OVERRIDE_NOTE=""
  if [ -n "${CHUG_DAEMON_BIN:-}" ]; then
    if [ -x "$CHUG_DAEMON_BIN" ]; then
      DAEMON_BIN="$CHUG_DAEMON_BIN"
      return 0
    fi
    DAEMON_OVERRIDE_NOTE="CHUG_DAEMON_BIN=$CHUG_DAEMON_BIN is not executable — ignored; "
    DAEMON_SKIP_REASON="CHUG_DAEMON_BIN=$CHUG_DAEMON_BIN is set but not executable"
  fi
  local installed="${HOME:-}/.local/bin/chug"
  if [ -x "$installed" ] && judge_probe_ok "$installed"; then
    DAEMON_BIN="$installed"
    return 0
  fi
  local repo_bin="$ROOT/target/release/chug" grep_rc=0
  if [ -x "$repo_bin" ]; then
    # T142 house style: the grep's rc is latched, never sailed past — 0 =
    # the feature-off stub literal compiled in (clients only), 1 = no match
    # (the binary hosts the judge), 2 = unreadable (unusable, fail-open).
    grep -aq "built without the judge daemon" "$repo_bin" 2>/dev/null || grep_rc=$?
    if [ "$grep_rc" -eq 1 ]; then
      DAEMON_BIN="$repo_bin"
      return 0
    fi
    DAEMON_SKIP_REASON="the repo release build is feature-lean (T204: clients only)"
  elif [ -z "$DAEMON_SKIP_REASON" ]; then
    DAEMON_SKIP_REASON="no daemon-capable binary found (no usable CHUG_DAEMON_BIN, no daemon-capable $installed, no repo build)"
  fi
  return 0
}
resolve_daemon_bin
if [ -n "$DAEMON_BIN" ]; then
  echo "$(ts) judge daemon binary: $DAEMON_BIN (${DAEMON_OVERRIDE_NOTE}probe cached for this run — T215; the ensure spawns this, not the repo dev build)" >> "$LOG"
else
  echo "$(ts) judge daemon: ensure skipped for this run — $DAEMON_SKIP_REASON (fail-open, T215; the judge client degrades per command as before)" >> "$LOG"
fi
# T50: content fingerprint of the running script, recorded BEFORE the cycle
# loop. POSIX cksum is content-based — `touch` or a git checkout that
# preserves content must not trigger a spurious re-exec; only a real content
# change does.
SELF_CKSUM="$(cksum "$ROOT/loopd.sh")"
fails=0
# T254 — the fixture-leak fail-safe. LOOPD_MAX_LOOPS, when set to a positive
# integer, bounds the supervisor's main-loop iterations: after N full passes
# the supervisor logs one line naming the knob and exits 0 (clean — the EXIT
# trap removes the pidfile). Production never sets it: unset, empty, zero, or
# any non-numeric value leaves today's unbounded loop byte-identical (the
# T137 LOOPD_SLEEP_OK seam is the precedent — a knob production never sets).
# Why a knob and not a reaper leg: the T152 orphan reaper matches argv
# needles (/tmp/chug-loop-t*, /tmp/chug-mut-*) and must NEVER kill an
# unresolved identity — a needle broad enough to see `bash loopd.sh run`
# would risk the production supervisor. The cure is fixture-side
# self-termination (fail-safe by construction): every test harness that
# spawns `loopd.sh run` exports a bound comfortably above its observed
# iteration need, so the bound only fires when the HARNESS died and the
# fixture leaked — the cycle-168 instance (a `bash loopd.sh run` orphaned to
# launchd for six days, immortal-but-inert on its probe-fail skip path,
# which never reaches the 3-strike HALT) is the indictment. The arming is
# resolved ONCE here, not per iteration (the T47 lesson: no env reads whose
# failure could surprise inside the loop), and a malformed value degrades to
# unbounded — the knob must never break production.
LOOPD_MAX_LOOPS_N=0
case "${LOOPD_MAX_LOOPS:-}" in
  ''|0|*[!0-9]*) ;;                       # unset/empty/zero/non-numeric: unbounded (today)
  *) LOOPD_MAX_LOOPS_N=$LOOPD_MAX_LOOPS ;; # a positive integer arms the bound
esac
loops=0
while [ ! -f "$STOP" ]; do
  # T50: re-exec self when this script changed on disk. FIRST statement in
  # the body, so a re-exec only ever happens BETWEEN cycles, never
  # mid-cycle; `exec` replaces the process image, so the fresh process reads
  # the new script from disk with zero incremental-read exposure (the hazard
  # that made signalling a running supervisor unsafe — T27). The re-exec is
  # logged so .chug/loopd/loopd.log always explains a budget/behavior change.
  # The operator's `loopd.sh stop` during a cycle still lands at the next
  # while-condition check, re-exec or not: a pending STOP is seen by the
  # while condition BEFORE this body runs, so STOP is never cleared by this
  # path. Known tradeoffs, both intentional: (a) the consecutive-failure
  # counter resets across a re-exec — failures against old code don't count
  # against new code; (b) the re-exec'd process re-runs the
  # `rm -f "$STOP" "$STATE/HALTED"` above, clearing a HALTED marker from a
  # prior crash streak — acceptable, because a re-exec only happens after a
  # merge changed the code (new code, fresh start).
  if [ "$(cksum "$ROOT/loopd.sh")" != "$SELF_CKSUM" ]; then
    echo "$(ts) loopd: script changed on disk — re-exec (pid $$)" >> "$LOG"
    exec "$ROOT/loopd.sh" run
  fi
  # T254 — the iteration bound (armed above the loop): N full body passes
  # run, and the pass AFTER the Nth logs the knob by name and exits 0. The
  # counter sits at the TOP of the body — after the T50 re-exec block (which
  # stays FIRST) and before every `continue` — so every path counts toward
  # it: the cycle launch AND the skip paths (the driver-active and probe-
  # fail skips `continue` past a bottom-of-body counter, and the probe-fail
  # skip is exactly where the cycle-168 leaked fixture sat, immortal).
  loops=$((loops + 1))
  if [ "$LOOPD_MAX_LOOPS_N" -gt 0 ] && [ "$loops" -gt "$LOOPD_MAX_LOOPS_N" ]; then
    echo "$(ts) LOOPD_MAX_LOOPS=$LOOPD_MAX_LOOPS_N reached — self-terminating after $((loops - 1)) main-loop iterations (T254 fixture-leak fail-safe)" >> "$LOG"
    exit 0
  fi
  # Single-driver: never overlap another LOOP-SPEC run (e.g. a manual one).
  # ps, never pgrep: on this host pgrep persistently fails to enumerate the
  # launchd-spawned loopd tree — pgrep -f/-l/-P all miss a live in-tree
  # driver while ps -ax lists it every time (argv intact) — so a pgrep-based
  # guard fails OPEN and duplicate drivers become possible (cycle-24 eval
  # I1). The [c]hug bracket keeps the grep's own argv out of the match; the
  # supervisor's own argv (`bash .../loopd.sh`) holds no needle and
  # `chug chat` must not match by design.
  # T137 fix-up (validator F1): the listing is CAPTURED, then the capture is
  # grepped — never a ps-to-grep pipeline. Under `set -o pipefail` that
  # pipeline fails OPEN on a real driver: grep -q exits at the FIRST match,
  # the still-writing ps leg gets SIGPIPE, and pipefail makes the writer's
  # 141 the pipeline's rc — so a LIVE driver read as "no driver" and a
  # second driver would launch (EARLY-MATCH-FLIPPED-FALSE, proven by the
  # validator with real 83KB ps output on this host and behaviorally by
  # tests/loopd_stale_binary.rs; T135's flock only degrades-with-warning).
  # Grepping the capture (the verdict-marker pattern below) has no pipe and
  # no SIGPIPE leg: the match alone decides.
  ps_rc=0
  ps_out="$(ps -ax -o command=)" || ps_rc=$?
  # T137 fix-up sweep: the capture's own rc is load-bearing, so it is
  # latched, never sailed past. Under set -e a bare capture would kill the
  # whole supervisor the first time ps hiccuped; swallowing it (`|| true`)
  # would read as "no driver" — both wrong. An unknown enumeration must
  # fail CLOSED: skip the cycle, say why, retry. This does not count toward
  # the 3-strikes guard — a transient ps failure must not HALT a healthy
  # supervisor, it must merely refuse to risk a duplicate driver.
  if [ "$ps_rc" -ne 0 ]; then
    echo "$(ts) driver probe FAILED (ps rc=$ps_rc) — enumeration unknown; refusing to risk a duplicate driver, skipping" >> "$LOG"
    sleep 120
    continue
  fi
  if grep -q "[c]hug run --spec LOOP-SPEC.md" <<<"$ps_out"; then
    echo "$(ts) another LOOP-SPEC driver active; skipping" >> "$LOG"
    sleep 120
    continue
  fi
  # T152 — pre-cycle orphan-process reaper. THE ONE SAFE INSTANT: the
  # single-driver probe just passed, so no driver, no orchestrator, and no
  # delegate children can legitimately exist — every chug process of a cycle
  # descends from the driver argv that probe would have matched — so any
  # leftover loop-artifact process (a T79 mutant leg whose validator died
  # mid-leg, a test binary that outlived its bash-120s-capped parent, a
  # removed /tmp/chug-loop-t*/chug-mut-* worktree's spinner) is
  # definitionally orphaned. The sweep is identity-based and fail-closed —
  # it never runs mid-cycle, never signals its own process group, never
  # kills an unresolved identity, SIGTERM only — and bounded to ~64
  # examinations per cycle; scripts/orphan-reaper.sh carries the doctrine.
  # Best-effort guard (the eval-digest shape, for the set -e regime): a
  # reaper failure must never block the launch.
  LOOP_REAPER_PAGE_FILE="$STATE/reaper-page" scripts/orphan-reaper.sh >> "$LOG" 2>&1 \
    || echo "$(ts) orphan-reaper: nonzero exit (best-effort, ignored — the cycle proceeds)" >> "$LOG"
  # T137 — the build is a GATE, not a best-effort step. The cycle below
  # launches ./target/release/chug, so a failed build must REFUSE the launch:
  # otherwise a merged change that fails compilation leaves the PREVIOUS
  # release binary in ./target/release/chug and the supervisor keeps running
  # cycles, pushing, and syncing site stats on code the merge never changed.
  # The rc is latched (`|| build_rc=$?` — the T142 house style; both `if !`
  # and command substitution would mask the real rc), and the build PINS
  # CARGO_TARGET_DIR="$ROOT/target": an inherited CARGO_TARGET_DIR must not
  # send a successful build elsewhere while the launch still execs the fixed
  # ./target/release/chug path (the same per-invocation-prefix rule T47
  # established for the cycle's shared cache).
  build_rc=0
  CARGO_TARGET_DIR="$ROOT/target" cargo build --release >> "$LOG" 2>&1 || build_rc=$?
  if [ "$build_rc" -ne 0 ]; then
    echo "$(ts) build FAILED (rc=$build_rc) — refusing to launch: ./target/release/chug is the stale previous release; not starting a cycle on it" >> "$LOG"
    fails=$((fails + 1))
    if [ "$fails" -ge 3 ]; then
      echo "3 consecutive build failures — refusing to launch a stale binary; fix the tree or stop the loop (last build rc=$build_rc)" > "$STATE/HALTED"
      echo "$(ts) HALTED after 3 consecutive build failures" >> "$LOG"
      exit 1
    fi
    echo "$(ts) build failure counts toward the consecutive-failure guard ($fails/3); will retry" >> "$LOG"
    sleep "${LOOPD_SLEEP_FAIL:-300}"
    continue
  fi
  # T78: the loop runs the RELEASE binary — this build produces
  # ./target/release/chug, the cycle invocation below launches it, delegate
  # children re-launch the orchestrator's own executable (current_exe), and
  # the review/validation gates run the T82 gate runner — `cargo nextest
  # run --release` when cargo-nextest is on PATH, else `cargo test
  # --release` (LOOP-SPEC step 3, META-SPEC gates rule). First cycle after
  # this lands pays a cold release build here AND into the shared cache;
  # every later one is warm.
  # T204/F15 — ensure the baked-in judge daemon (build warmth for the risk
  # gate): spawn it NOW, before the cycle, so the first risk-gated bash
  # command does not pay the model load itself (the daemon loads ONCE per
  # host and every run/child shares the warm model). Best-effort like the
  # reaper and the digest — a daemon that will not come up must never block
  # the launch: the client's fail-open degrade is the same shape as today's
  # unreachable-layad path. `chug daemon --ensure` is idempotent (healthy
  # daemon -> instant exit 0) and bounded (spawn + wait-for-socket with a
  # fixed budget). LOOP_DAEMON_ENSURE=0 opts out (the LOOP_REAPER pattern).
  # T215 — the spawned binary is $DAEMON_BIN, resolved ONCE at loopd startup
  # (the probe cache): the installed release binary when daemon-capable, not
  # the repo dev build (feature-lean by T204 — its `daemon` subcommand is
  # the fail-open stub that made every ensure exit nonzero). When nothing
  # daemon-capable resolved, the ensure is skipped silently here: the
  # startup resolution already logged the one skip line for this run. A
  # cold FIRST weight load (~650 MB HF download) exceeds the ensure's
  # bounded wait budget — the detached daemon keeps loading and the cycle
  # proceeds fail-open (warm by the next cycles); the pinned remedy is the
  # runbook pre-warm one-liner (`<daemon binary> daemon` foreground once,
  # runbooks/loop-ops.md), not a bigger budget.
  if [ "${LOOP_DAEMON_ENSURE:-1}" = "1" ] && [ -n "$DAEMON_BIN" ]; then
    # stdout/stderr to /dev/null — the supervisor log carries SUPERVISOR
    # lines only (the spoof-guard invariant: chug output reaches the log
    # only through the sanctioned cycle-child record at verdict time);
    # daemon diagnostics live in <chug home>/daemon.log.
    "$DAEMON_BIN" daemon --ensure >/dev/null 2>&1 \
      || echo "$(ts) daemon ensure: nonzero exit (best-effort, ignored — the judge fails open)" >> "$LOG"
  fi
  # T219 — register/heartbeat this loopd run in the daemon's /sessions
  # registry (the same host-scoped 0600 socket the ensure just warmed; the
  # one host-local answer to "what chug runs are alive on this box"). One
  # curl line at cycle start, exactly like the ensure's house style:
  # best-effort (`|| true` — a failed POST is a no-op, never a set -e/pipefail
  # trip), bounded (--max-time 2, the judge client's own socket bound). The
  # id is the SUPERVISOR pid ($$ — stable across cycles: one entry, refreshed
  # at each cycle start; the daemon-side upsert keeps the first-seen started,
  # so the session age tracks the supervisor, not the cycle). Socket
  # resolution matches the daemon's own ($CHUG_DAEMON_SOCK overrides, then
  # $CHUG_HOME, then ~/.chug).
  curl --unix-socket "${CHUG_DAEMON_SOCK:-${CHUG_HOME:-$HOME/.chug}/daemon.sock}" \
    --max-time 2 -s -o /dev/null -X POST -H 'Content-Type: application/json' \
    -d "{\"id\":\"loopd-$$\",\"role\":\"loopd-cycle\",\"started\":\"$LOOPD_STARTED\",\"last_event_ts\":\"$(ts)\",\"status\":\"running\"}" \
    http://localhost/sessions || true
  # T46: refresh the Phase-1 corpus digest so every cycle's evaluation reads
  # .chug/eval-digest.md instead of re-mining raw events archives. T137:
  # best-effort — a nonzero exit must not kill the supervisor under set -e;
  # the cycle degrades to reading the previous digest.
  scripts/eval-digest.sh >> "$LOG" 2>&1 \
    || echo "$(ts) eval-digest: nonzero exit (best-effort, ignored — the cycle reads the previous digest)" >> "$LOG"
  cycle_log="$STATE/cycle-$(date -u +%Y%m%d-%H%M%S).log"
  # T81: route THIS cycle before launch — the freshness predicate (the same
  # mechanical rule LOOP-SPEC Phase 1 gives the orchestrator) picks the
  # mode, the mode picks the orchestrator model. Logged to loopd.log so it
  # always explains a model change (the T50 re-exec-log rule) and echoed as
  # the cycle log's first line, so every cycle record names who ran it.
  # T258: in eval mode the gate below may re-route the model (borderline
  # non-trip evals to $LOOP_ROUTINE_MODEL) or skip the launch entirely (the
  # cheap exit), so the routing line is logged AFTER the gate and names the
  # FINAL model.
  routing="$(route TODO.md EVALUATION.md)"
  mode=${routing%% *}
  orch_model=${routing#* }
  if [ "$mode" = eval ]; then
    gate="$(eval_gate)"
    gate_verb=${gate%% *}
    gate_rest=${gate#* }
    gate_model=${gate_rest%% *}
    # T259: the triage's decision record (the state pack verbatim, the
    # verdict, the confidence, the route taken) + the previous triage's
    # outcome backfill — run mode only, best-effort, before the skip or
    # launch the record describes.
    laya_record "$gate"
    if [ "$gate_verb" = skip ]; then
      echo "$(ts) cheap-exit disposition (T258): ${gate#skip } — launch skipped" >> "$LOG"
      if loopd_write_disposition "$gate"; then
        fails=0
        skip_secs=$(ok_sleep_seconds)
        echo "$(ts) cheap-exit pacing: sleeping ${skip_secs}s (empty streak $(empty_wrap_streak))" >> "$LOG"
        sleep "$skip_secs"
        continue
      fi
      # The disposition commit failed (identity, hook, read-only fs) — the
      # safe side is the launch: a skip whose git record never landed would
      # be invisible to the T237 streak walk and the T247 valve, and the
      # chain would cheap-exit forever unpaced.
      echo "$(ts) cheap-exit disposition commit FAILED — falling through to a real $LOOP_ORCH_MODEL eval" >> "$LOG"
      orch_model=$LOOP_ORCH_MODEL
    elif [ "$gate_model" != "$LOOP_ORCH_MODEL" ]; then
      # the borderline re-route (req 2): the gate resolved $LOOP_ROUTINE_MODEL
      # (the full id) and it differs from the orchestrator model — launch it.
      # Under the T81 rollback (LOOP_ROUTINE_MODEL = the kimi id) the two are
      # equal: the re-route line stays silent and single-model operation
      # holds. The comparison keys on the env, never a bare literal — a
      # literal `kimi` here would log a phantom re-route on every kimi arm.
      echo "$(ts) eval gate (T258): ${gate#launch } -> $gate_model orchestrator" >> "$LOG"
      orch_model=$gate_model
    fi
  fi
  echo "$(ts) routing: todo_rows=$(todo_rows TODO.md) eval_fresh=$(eval_fresh EVALUATION.md && echo yes || echo no) -> $mode cycle on $orch_model" >> "$LOG"
  echo "$(ts) cycle start -> $cycle_log ($mode cycle, orchestrator $orch_model)" >> "$LOG"
  echo "[loopd $(ts)] T81 routing: $mode cycle — orchestrator model $orch_model" > "$cycle_log"
  # cycle budget: eval ≈45–55 + items ≈28–35 each + wrap ≈10 ⇒ a 5-item eval
  # cycle wants ≈195–240 — 200 fits it with a slim margin (T121). Cycle-61
  # (2026-09-28) died 160/160 POST-wrap, pre-goal_complete (5-item eval
  # queue; minutes 171/240 — never binding). The cap is also the stuck-cycle
  # detector, so 200 costs ≤40 extra stuck iterations per stuck cycle before
  # the HALT guard (3 consecutive failed cycles) trips.
  # T198: minutes raised 240→360 — the fleet outgrew the wall: cycle-85–90
  # walls measured 3h23m–4h01m, cap hit once (cycle 88 DIED at the
  # 240-minute wall mid-arc, absorbed by a wrap + cold restart + the
  # cycle-89 reconciliation; three of the six ran within 5 minutes of the
  # cap). 360 = p95 ≈ 4h × 1.5 headroom; iterations stay 200 (cycle 89
  # used 195/200 — a watch item for the next eval, not this row; separate
  # lever with a separate measure history). Bounded blast-radius reasoning
  # is unchanged from T27: loopd relaunches a dead cycle, and the
  # 3-consecutive-failures HALT still caps a wedged fleet.
  # T47: the shared cache reaches the cycle as a per-invocation env prefix on
  # the chug call — NEVER as a bare `export`. An export inside this while loop
  # would persist in the supervisor's environment across iterations, so from
  # cycle 2 on the `cargo build --release` above would run with the var set,
  # land in target-shared, and never refresh ./target/release/chug (stale
  # supervisor binary). The prefix is visible only to this cycle's
  # orchestrator process and the delegate children that inherit its launch
  # env; the supervisor's own build always stays in ./target, so
  # ./target/release/chug keeps resolving to a freshly built binary. T137
  # now ENFORCES that invariant: the build itself pins
  # CARGO_TARGET_DIR="$ROOT/target" as a per-invocation prefix, so an
  # inherited CARGO_TARGET_DIR can send a successful build elsewhere while
  # the supervisor still launches the fixed ./target/release/chug path.
  # T142: the cycle verdict is the child's EXIT STATUS, never a log grep.
  # Model text reaches the cycle log verbatim (raw stderr deltas, the F7
  # raw-bytes doctrine), so a run that died on verification or budget while
  # SAYING "chug: goal complete" used to satisfy the old log grep: the cycle
  # was recorded OK, the failure counter reset, and site sync ran with no
  # accepted goal. driver.rs maps run-mode exit 0 ⟺ accepted goal
  # (RunFinished(0) only on the verified path; budget/abort exit 1, stuck 2),
  # so the rc decides. The child's stdout — the only stream the
  # goal-complete block is printed on — is captured apart from the mixed
  # stderr log, so a model-forged `chug: goal complete` or `summary:` line
  # can neither satisfy the marker grep nor shadow the recorded summary; the
  # capture is appended back to the cycle log so the forensic record keeps
  # the block. The supervisor then stamps its own rc-based verdict line into
  # the cycle log — the only thing downstream consumers (site-sync's cycle
  # count) may count, never raw child bytes.
  # T178: lift the 120s bash-tool cap for the whole fleet — cycles-76–78
  # showed 3-4 `timed out after Ns (process group killed)` bash deaths per
  # stream (cold/warm nextest gates and cold cargo builds exceed 120s), and
  # every death buys a retry whose wall time feeds the minutes-death census.
  # ONE export here covers the orchestrator AND every delegate child
  # (children inherit this process env — delegate has no env parameter, the
  # T47 lesson — and T144's scrub removes only target-dir vars). Precedence:
  # `--bash-timeout` flag > $CHUG_BASH_TIMEOUT env > the 120 default
  # (src/main.rs:370), so an explicit flag still wins. Unlike the T47
  # CARGO_TARGET_DIR prefix below, persisting this export across supervisor
  # iterations is harmless: it only affects chug bash-tool calls, never the
  # supervisor's own ./target build.
  export CHUG_BASH_TIMEOUT=300
  # T230 — the orchestrator launch carries the 100k context-occupancy nudge
  # (`--ctx-warn-at-tokens`, the T192/LIVE_CTX surface). This is a
  # MEASUREMENT, not an assertion of value: the one-shot notice has never
  # fired in production (0 ctx-edit fires across all 549 events files in
  # the cycle-103 eval corpus) because nothing in the loop fleet ever set
  # the flag — the default is 0 = off (src/driver.rs), so trim
  # (TRIM_ABOVE_TOKENS = 120_000, src/trim.rs) silently collapsed every
  # overflow before any model was told it could compact its own context
  # for a free turn. 100_000 sits ~20k tokens BELOW trim's 120_000, so the
  # remedy window (one free edit-only turn) PRECEDES the automatic
  # collapse: the model either acts, or trim proceeds exactly as before —
  # zero behavior change below 100k, zero when the nudge is ignored. The
  # verdict lands in the digest's per-file `ctx-edit fires:` line
  # (scripts/eval-digest.sh), which the next evals read; ORCHESTRATOR
  # launch only — delegate children keep their own launch (their context
  # curves and free-turn economics differ; extending is verdict-dependent,
  # not this row).
  chug_rc=0
  chug_out="$(CARGO_TARGET_DIR="$ROOT/target-shared" ./target/release/chug run --spec LOOP-SPEC.md \
    --goal "Run the full self-improvement cycle per LOOP-SPEC: evaluate or skip per the freshness rule, work the queue (features are first-class per the amended doctrine — close capability gaps, not only harden), adversarial validation for core-logic items, you own all bookkeeping, push after each item lands green + remainder at wrap. Your wrap IS the next cycle's input — leave TODO.md, EVALUATION.md and specs/ such that a cold next cycle needs zero human words." \
    --model "$orch_model" --max-iters 200 --max-minutes 360 --ctx-warn-at-tokens 100000 \
    2>> "$cycle_log")" || chug_rc=$?
  printf '%s\n' "$chug_out" >> "$cycle_log"
  if [ "$chug_rc" -eq 0 ]; then
    echo "[loopd $(ts)] verdict: goal complete (rc=$chug_rc)" >> "$cycle_log"
  else
    echo "[loopd $(ts)] verdict: no goal complete (rc=$chug_rc)" >> "$cycle_log"
  fi
  # T137 pipefail sweep: the marker is grepped from the captured stdout
  # DIRECTLY, not through a `printf | grep -q` pipeline — under pipefail a
  # writer SIGPIPE (large stdout, grep -q exits at the match) would flip a
  # real goal-complete verdict into a failure.
  if [ "$chug_rc" -eq 0 ] && grep -q "chug: goal complete" <<<"$chug_out"; then
    # T137 pipefail sweep: `head -1` closes the pipe early — under
    # pipefail+set -e a second `summary:` line would SIGPIPE grep and kill
    # the supervisor mid-success. Best-effort by design, so guard it.
    # T137 fix-up sweep: the `|| true` masks EVERY leg's rc — a writer
    # SIGPIPE when head stops reading after the first match, and grep's own
    # no-match rc 1 — and the DATA survives (head has already read the first
    # summary line before any writer can die); a no-match degrades to an
    # empty summary exactly as it did before pipefail existed. rc-masked and
    # data-insensitive by design; the verdict itself is decided by the
    # herestring grep above, never by this extraction.
    summary=$(printf '%s\n' "$chug_out" | grep "^summary:" | head -1 | cut -c1-200 || true)
    echo "$(ts) cycle OK: $summary" >> "$LOG"
    # T98: best-effort site stats sync — one line, failure-tolerant. The
    # script itself never fails a cycle (missing clone / rejected push /
    # marker-less page all warn and exit 0; a nonzero exit here is logged
    # and ignored — site sync is observability, not a gate).
    scripts/site-sync.sh >> "$LOG" 2>&1 \
      || echo "$(ts) site-sync: nonzero exit (best-effort, ignored)" >> "$LOG"
    fails=0
    # T237 — the cycle-OK sleep scales with consecutive empty-delta wraps
    # (ok_sleep_seconds above): 60s doubling to the LOOPD_EMPTY_SLEEP_CAP
    # ceiling, the T137 LOOPD_SLEEP_OK seam still pinning any fixed
    # cadence byte-identically. The `cycle OK:` line above stays
    # byte-identical (site-sync and log greps read it); this ONE line
    # names the pacing decision next to it.
    ok_secs=$(ok_sleep_seconds)
    echo "$(ts) cycle-OK sleep ${ok_secs}s (empty streak $(empty_wrap_streak), cap ${LOOPD_EMPTY_SLEEP_CAP:-1800})" >> "$LOG"
    sleep "$ok_secs"
  else
    fails=$((fails + 1))
    echo "$(ts) cycle ended WITHOUT goal complete (consecutive failures: $fails)" >> "$LOG"
    if [ "$fails" -ge 3 ]; then
      echo "3 consecutive cycles without goal_complete — last: $cycle_log" > "$STATE/HALTED"
      echo "$(ts) HALTED after 3 consecutive failures" >> "$LOG"
      exit 1
    fi
    sleep "${LOOPD_SLEEP_FAIL:-300}"
  fi
done
echo "$(ts) stop file present — clean exit" >> "$LOG"
