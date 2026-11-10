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
# ONLY the allowlisted keys are applied (the T205 judge keys, then the T332
# decision-provider pass-throughs — a secret file must not inject arbitrary
# env into every cycle), and only when the operator has
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

# T330 — the LLM profile (docs/install-profiles.md): `install.sh --profile`
# writes the operator's chosen profile to $HOME/.chug/profile.env and the
# supervisor loads it ONCE at startup (the T47 lesson — no env mutation
# inside the cycle loop). The loader translates the profile's owned keys to
# the loop's native env BEFORE the model defaults below apply:
# CHUG_LLM_BASE_URL/CHUG_LLM_API_KEY(_CMD) land on ANTHROPIC_BASE_URL/
# ANTHROPIC_AUTH_TOKEN (src/auth.rs reads those), CHUG_MODEL_REASONING/
# CHUG_MODEL_CODING land on LOOP_ORCH_MODEL/LOOP_ROUTINE_MODEL, and
# CHUG_LANGFUSE_* lands on LANGFUSE_*. Already-shaped env wins everywhere
# (an operator-exported LOOP_ORCH_MODEL or ANTHROPIC_AUTH_TOKEN is never
# overridden) and an absent profile is a silent no-op — today's behavior,
# no flag day. Override the path with CHUG_PROFILE_ENV. The fragment is
# resolved relative to THIS script's own path, never the cwd.
PROFILE_ENV_FILE="${CHUG_PROFILE_ENV:-$HOME/.chug/profile.env}"
. "$(dirname "$0")/scripts/loopd_profile_loader.sh"
loopd_load_profile

# T352 — the opt-in self-update check (CHUG_AUTO_UPDATE=1 in the profile).
# The fragment defines loopd_auto_update_check: the daily `chug update
# --check` + the between-cycles apply, best-effort end to end (an update
# check must never gate a cycle). Resolved relative to THIS script's own
# path, never the cwd.
. "$(dirname "$0")/scripts/loopd_auto_update.sh"

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

# T326 — the ideation child's model: glm-5-3-flash (the spec's call — the
# ideation leg is a ~2k-token structured task, never kimi). The T81 knob
# shape: one env override, the FULL provider-routed id (model ids go
# verbatim into the API request body — there is no alias layer), never a
# bare shorthand.
LOOP_IDEATION_MODEL="${LOOP_IDEATION_MODEL:-anthropic-system.ai.glm-5-3-flash}"

# The mechanical half of LOOP-SPEC Phase 1's freshness rule ("skip
# evaluation if TODO.md has `todo` rows AND EVALUATION.md is fresh (same
# UTC day)"), evaluated by the supervisor BEFORE launch so the
# routine-vs-eval switch is the RULE, not a model's opinion — and so the
# orchestrator's own Phase-1 decision (same predicate, same files, one
# single-driver writer) always agrees with it. CHUG_ROUTINE_TODAY pins the
# "today" side for tests, the eval-digest.sh CHUG_DIGEST_NOW pattern.
todo_rows() { # count of TODO.md data rows whose status cell is `todo` or
  # `open` (the operator's dormant-era active-backlog state, T325 — an
  # open row is real queue work and must gate the cheap-exit routing).
  [ -f "$1" ] || { echo 0; return; }
  awk -F'|' '/^[|]/ {
    id=$2; gsub(/[ \t]/, "", id)
    if (id ~ /^T[0-9]+$/) { s=$6; gsub(/[ \t]/, "", s); if (s == "todo" || s == "open") n++ }
  } END { print n + 0 }' "$1"
}
top_open_item() { # the queue's top open row as "Txxx — <title, 60 chars>";
  # empty when there is none (or TODO.md is missing/unreadable). The T356
  # heartbeat's item hint: LOOP-SPEC works the queue strictly serially in
  # file order (merges serial in queue order), so the FIRST open row is the
  # item a starting cycle will work. Best-effort like todo_rows above.
  [ -f "${1:-TODO.md}" ] || return 0
  awk -F'|' '/^[|]/ {
    id=$2; gsub(/[ \t]/, "", id)
    if (id ~ /^T[0-9]+$/) {
      s=$6; gsub(/[ \t]/, "", s)
      if (s == "todo" || s == "open") {
        t=$3; gsub(/^[ \t]+/, "", t); gsub(/[ \t]+$/, "", t)
        print id " — " substr(t, 1, 60)
        exit
      }
    }
  }' "${1:-TODO.md}"
  return 0
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

closed_todo_rows() { # baseline `todo`/`open` rows no longer `todo` (closed,
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
      if (id ~ /^T[0-9]+$/) { s = $6; gsub(/[ \t]/, "", s); if (s == "todo" || s == "open") was[id] = 1 }
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

# T332 — the decision-provider selection (src/decision's table, shell
# mirror; scripts/ideate.py's noul_provider is the third copy of the same
# contract). The triage consults exactly ONE provider per cycle; the choice
# must never be silent — an unknown mode is refused into the fail-open, not
# mapped to whatever the operator did not ask for.
decision_provider() { # prints `local` | `proxy` — the ONE transport this
  # triage consults — or fails (rc 1, nothing printed) on an unknown
  # CHUG_DECISION_PROVIDER mode (the caller fail-opens). Unset/blank is
  # auto. auto collapses to the local daemon when no proxy is configured —
  # today's default shape byte for byte, no health probe, so the provider
  # change is invisible in every fixture that does not configure a proxy —
  # and lets the socket's /healthz decide when a proxy IS configured:
  # warm daemon wins, else the proxy.
  local mode="${CHUG_DECISION_PROVIDER:-}"
  mode=$(printf '%s' "$mode" | tr -d '[:space:]' | tr '[:upper:]' '[:lower:]')
  case "$mode" in
    ''|auto)
      if [ -n "${CHUG_DECISION_PROXY_URL:-}" ]; then
        if decision_healthz_ok; then echo local; else echo proxy; fi
      else
        echo local
      fi ;;
    local) echo local ;;
    proxy) echo proxy ;;
    *) return 1 ;;
  esac
}

decision_healthz_ok() { # the auto matrix's warm-daemon probe: GET /healthz
  # over the daemon socket (the T204 resolution order), short bound —
  # src/decision LocalDaemon::healthz_ok's shell twin. 200 means warm (the
  # model loads BEFORE the socket binds, so healthz never lies).
  local sock="${CHUG_DAEMON_SOCK:-${CHUG_HOME:-$HOME/.chug}/daemon.sock}"
  local status
  status=$(curl --unix-socket "$sock" --max-time 1 -s -o /dev/null \
    -w '%{http_code}' http://localhost/healthz 2>/dev/null) || return 1
  [ "$status" = "200" ]
}

decision_proxy_post() { # the T332 proxy leg: POST $1 to the configured
  # <base>/v1/systemone — the model field added (the proxy contract
  # REQUIRES it; the daemon never sees this field), `Authorization: Bearer`
  # when a token is configured (a tokenless call is legal at the wire level
  # and reads its 401 as the error), 2s bound (the triage's own fail-open
  # bound, not the provider's 10s). Echoes the response in the daemon leg's
  # exact parse shape — body + '\n' + status — or sets DECISION_POST_ERR and
  # returns 1 on ANY failure: the caller fail-opens, same as a dead daemon.
  DECISION_POST_ERR=""
  local base="${CHUG_DECISION_PROXY_URL:-}"
  [ -n "$base" ] || { DECISION_POST_ERR="decision proxy not configured (set CHUG_DECISION_PROXY_URL)"; return 1; }
  local wire
  wire=$(printf '%s' "$1" | jq -c '. + {model: "jev"}' 2>/dev/null) || { DECISION_POST_ERR="proxy request body build failed (jq?)"; return 1; }
  local cmd
  cmd=(curl --max-time 2 -s -w '\n%{http_code}' -X POST -H 'Content-Type: application/json')
  if [ -n "${CHUG_DECISION_PROXY_TOKEN:-}" ]; then
    cmd+=(-H "Authorization: Bearer ${CHUG_DECISION_PROXY_TOKEN}")
  fi
  local resp status
  resp=$("${cmd[@]}" -d "$wire" "${base%/}/v1/systemone" 2>/dev/null) || { DECISION_POST_ERR="decision proxy unreachable at ${base%/}"; return 1; }
  status=${resp##*$'\n'}
  [ "$status" = "200" ] || { DECISION_POST_ERR="decision proxy HTTP $status"; return 1; }
  printf '%s\n' "$resp"
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
  # needs-eval answer + confidence over the T332-selected provider:
  # CHUG_DECISION_PROVIDER=auto|local|proxy (auto default) — local is the
  # existing daemon socket (the T204 resolution: $CHUG_DAEMON_SOCK, then
  # $CHUG_HOME, then ~/.chug) UNCHANGED, proxy is decision_proxy_post (the
  # /v1/systemone contract, docs/decision-providers.md), auto prefers a
  # warm socket and falls back to a configured proxy. Sets
  # LAYA_VERDICT/LAYA_CONF/LAYA_ROUTE — an empty route means FAIL-OPEN and
  # the caller keeps the T258 arm verbatim, whatever the transport.
  # Bounded at 2s per leg (req 3); the answer's confidence is normalized to
  # a number (an unreadable one IS zero confidence, and zero routes kimi).
  LAYA_VERDICT=""; LAYA_CONF=""; LAYA_ROUTE=""
  local sock="${CHUG_DAEMON_SOCK:-${CHUG_HOME:-$HOME/.chug}/daemon.sock}"
  local body resp status answer choice conf provider
  body=$(triage_request_body "$@") || { laya_failopen "state pack build failed (jq?)"; return 0; }
  provider=$(decision_provider) || { laya_failopen "unknown CHUG_DECISION_PROVIDER (expected auto|local|proxy) — no provider consulted"; return 0; }
  if [ "$provider" = proxy ]; then
    # NOT command substitution: DECISION_POST_ERR is set inside
    # decision_proxy_post, and a $(...) subshell would swallow it — the
    # fail-open note would read an EMPTY reason (the T332 validator's F1).
    # stdout goes to a tempfile, the rc + the global var reach the caller.
    DECISION_POST_ERR=""
    local proxy_out
    proxy_out=$(mktemp "${TMPDIR:-/tmp}/decision-post.XXXXXX") || { laya_failopen "decision proxy tempfile failed"; return 0; }
    # the if-guard keeps set -e from aborting the run on a dead proxy — the
    # fail-open must REACH laya_failopen, never die before it.
    if decision_proxy_post "$body" > "$proxy_out"; then
      resp=$(cat "$proxy_out")
    else
      laya_failopen "${DECISION_POST_ERR:-decision proxy failed (no reason recorded)}"
      rm -f "$proxy_out"; return 0
    fi
    rm -f "$proxy_out"
  else
    resp=$(curl --unix-socket "$sock" --max-time 2 -s -w '\n%{http_code}' \
      -X POST -H 'Content-Type: application/json' -d "$body" \
      http://localhost/judge 2>/dev/null) || { laya_failopen "daemon unreachable at $sock"; return 0; }
  fi
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
    if [ "$provider" = proxy ]; then
      # the hosted leg names itself — proxy spend is never invisible spend
      echo "$(ts) laya triage (T259): needs_eval=$LAYA_VERDICT conf=$LAYA_CONF (HIGH=$TRIAGE_HIGH) -> route=$LAYA_ROUTE (provider=proxy jev)" >> "$LOG"
    else
      echo "$(ts) laya triage (T259): needs_eval=$LAYA_VERDICT conf=$LAYA_CONF (HIGH=$TRIAGE_HIGH) -> route=$LAYA_ROUTE" >> "$LOG"
    fi
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
  # T263 — the outcome record mints its OWN id, the triage writer's exact
  # convention (d<epoch>-loopd<SEQ> off the shared LAYA_SEQ counter), and
  # carries the parked triage id as its SUBJECT only. The pre-fix writer
  # passed the triage id as the outcome's own id too: the corpus then held
  # a laya-triage and an outcome with the SAME id, the outcome's subject
  # naming itself (duplicate ids + a malformed chain, measured on the live
  # corpus) — the id collision secondarily mis-flagged correctly-targeted
  # outcomes through the audit's last-wins id→class map.
  # ONE outcome per triage id, ever: the pending park is consumed exactly
  # once (the rm in laya_record, right after this backfill) — a look that
  # backfills but parks nothing (a mechanical skip, a fail-open) leaves no
  # stale record behind, so no later look can ever double-backfill the same
  # id (pinned by a_pending_surviving_an_intervening_non_triage_cycle_
  # backfills_exactly_once).
  # T296 — that invariant covers writers beyond loopd too. An OUT-OF-BAND
  # outcome appended straight to .chug/decisions.jsonl (the wrap's T261
  # batch discharge is the observed racer) never touches the park file, so
  # the rm alone cannot see it: the next look would backfill a consult that
  # already carries an outcome — a duplicate subject in the F13 corpus,
  # measured twice on the live corpus (d1791919548-loopd4, both verdicts
  # agreeing; d1792039974-loopd92, found at the cycle-850 trip-180 eval).
  # The probe below runs BEFORE the append: a hit consumes the park
  # silently (one loopd.log note, no record written), a miss runs the
  # designed backfill unchanged. The guard is writer-agnostic — whichever
  # writer (this look or the wrap's batch) lands first wins — and the
  # tally of outcome records stays == unique triage subjects, so the
  # double is impossible regardless of which writer races and LOOP-SPEC's
  # T261 batch clause stands unedited.
  local pend=$1 gate=$2 id out_id now route pbase cur_base cur_book choice what probe
  id=$(jq -r '.id // ""' "$pend" 2>/dev/null) || id=""
  if [ -z "$id" ]; then
    echo "$(ts) laya triage (T259): unreadable pending record — backfill skipped" >> "$LOG"
    return 0
  fi
  # T296 double-discharge guard: one bounded read of the corpus for an
  # outcome already naming the parked triage id. Fail-open to APPEND on
  # any probe error — an unreadable/missing corpus or a jq failure never
  # blocks the designed backfill (the same tolerance shape as the
  # pending-record reads above).
  probe=$(jq -r --arg id "$id" \
    'select(.class == "outcome" and .subject == $id) | .subject' \
    .chug/decisions.jsonl 2>/dev/null) || probe=""
  if [ -n "$probe" ]; then
    echo "$(ts) laya triage (T259): outcome for $id already backfilled out-of-band — consuming the park silently" >> "$LOG"
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
  now=$(date -u +%s)
  LAYA_SEQ=$((LAYA_SEQ + 1))
  out_id="d${now}-loopd${LAYA_SEQ}"
  # T263 self-subject tripwire: should the id about to be written ever
  # equal its subject again (a future regression of this exact class), log
  # ONE WARN naming the collision and skip the write — the corpus write
  # stays best-effort and never blocks the cycle. The sequence counter has
  # already advanced (the skipped mint still consumed it), so the cycle's
  # own triage id below can never collide with anything already written.
  if [ "$out_id" = "$id" ]; then
    echo "$(ts) laya triage (T259): WARN outcome id == subject ($out_id) — record write SKIPPED" >> "$LOG"
    return 0
  fi
  if laya_record_write "$out_id" "$now" "outcome" "$id" \
      "what the loop found when it looked: $what; bookkeeping=$cur_book base=${cur_base:-none}" \
      "landed-clean | fixed-up | reverted" "$choice" 1; then
    echo "$(ts) laya triage (T259): outcome backfill $id -> $choice ($what) [outcome id $out_id]" >> "$LOG"
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

# T325 — dormant mode. When the queue is empty (zero `todo`/`open` rows),
# the loop is DORMANT: the T247 forced-eval leg is suspended (the valve's
# 4th-TRUE trip was the leak — with an empty backlog the forced eval audits
# nothing outward and files meta-work about the loop's own machinery) and
# is replaced by a WEEKLY AUDIT TIMER: a real evaluation launches only when
# the last evaluation commit is DORMANT_AUDIT_SECS (7d) or more old. The
# timer's anchor is GIT, never a new authority: the newest commit touching
# EVALUATION.md — the T258 baseline, the last examination of loop state, an
# `eval:` commit in every production shape. The loop's own dormancy
# dispositions are EMPTY commits and never touch the file, so the loop can
# never refresh its own timer; the audit's eval moves it, restarting the
# week. State rides in .chug/loopd/dormant-state.json (`last_eval_ts`,
# written from that git epoch — the file is the RECORD, the routing always
# re-derives). T325-F1: the re-derivation takes the NEWER of the record
# and the git anchor (dormant_timer_ts below) — the record is rewritten
# before the audit's own eval: commit lands, so a record-only read lags a
# landed audit by the whole elapsed week and re-fires a duplicate; the
# anchor half stops the duplicate the very next cycle, while a crashed
# audit (no eval: commit, both halves stale) simply re-audits. An unknown
# half — an unreadable record, an unanchorable clock — degrades to 0, the
# due side: an unknown clock must never suppress the audit.
DORMANT_AUDIT_SECS=604800 # 7d — the weekly audit timer's only knob
dormant_newest_eval_ts() { # the git anchor: newest EVALUATION.md-touching
  # commit's committer epoch; empty = unknown (non-git cwd, never examined)
  git log -1 --format=%ct -- EVALUATION.md 2>/dev/null || true
}
dormant_state_ts() { # the latch's last_eval_ts — a clean base-10 integer
  # (leading zeros stripped: bash arithmetic would read them as octal), or
  # EMPTY when the record is missing/unreadable (an unknown half)
  [ -f "$STATE/dormant-state.json" ] || return 0
  awk '{
    if (match($0, /"last_eval_ts"[ \t]*:[ \t]*[0-9]+/)) {
      v = substr($0, RSTART, RLENGTH)
      sub(/.*:[ \t]*/, "", v)
      printf "%d\n", v
      exit
    }
  }' "$STATE/dormant-state.json"
}
dormant_timer_ts() { # the timer's EFFECTIVE anchor (T325-F1): the newer of
  # the record (the latch's last_eval_ts) and the git source (the newest
  # EVALUATION.md-touching commit). The record is rewritten before the
  # audit's eval: commit lands, so a record-only read lags a landed audit
  # by the whole elapsed week and the next cycle re-fires the audit — the
  # duplicate. The git half stops it the moment the commit lands (git is
  # the source, the record the cache) and leaves the crashed audit's
  # re-fire untouched (no eval: commit, both halves stale, the timer stays
  # due). Either half unknown degrades to 0 — the due side; an unknown
  # clock never suppresses the audit.
  local lt gt
  lt=$(dormant_state_ts)
  case "$lt" in ''|*[!0-9]*) lt=0 ;; esac
  gt=$(dormant_newest_eval_ts)
  case "$gt" in ''|*[!0-9]*) gt=0 ;; esac
  if [ "$gt" -gt "$lt" ]; then printf '%s\n' "$gt"; else printf '%s\n' "$lt"; fi
}
dormant_state_write() { # (re)arm the latch's timer: last_eval_ts = the git
  # anchor's epoch, read from the record, never invented — called at the
  # dormant ENTRY (the anchor the empty queue starts from) and AFTER the
  # audit child returns (the re-arm: fresh when the audit's eval: commit
  # landed, unchanged when it crashed); NEVER at the audit launch, where
  # the anchor is still the pre-audit commit a record-only restart would
  # keep stale (the T325-F1 duplicate). An unanchorable clock writes 0
  # (due) — an unknown anchor must never suppress the audit. Best-effort
  # like every state write: a latch that cannot land leaves the loop
  # awake, which errs toward MORE launches (the safe side).
  local ts ige
  ts=$(dormant_newest_eval_ts)
  case "$ts" in ''|*[!0-9]*) ts=0 ;; esac
  # T326 — the ideation half rides the SAME latch file: this write would
  # clobber a recorded last_ideation_ts, so the half is read BEFORE the
  # write and carried through; absent stays absent (the ideation timer
  # then reads its own git anchor — the newest specs/proposed commit —
  # and an absent half over a missing anchor leaves the timer unarmed).
  ige=$(ideation_state_ts)
  case "$ige" in ''|*[!0-9]*)
    printf '{"last_eval_ts": %s}\n' "$ts" > "$STATE/dormant-state.json" || true
    return 0 ;;
  esac
  printf '{"last_eval_ts": %s, "last_ideation_ts": %s}\n' "$ts" "$ige" > "$STATE/dormant-state.json" || true
}

# T326 — ideation mode. While DORMANT, the loop proposes outward-facing
# features instead of only going quiet: when `now - last_ideation_ts` is
# 7d or more (state in the same .chug/loopd/dormant-state.json latch: one
# extra key, `last_ideation_ts`), the supervisor launches ONE glm ideation
# run (never kimi — a ~2k-token structured task) executing
# scripts/ideate.py, the deterministic ideas-packet builder (FEATURES.md
# parked items, deferred/blocked threads in LEDGER + docs, the eval-triage
# class census from decisions.jsonl, site/mod staleness facts) whose every
# proposal passes the Laya noul meta-class gate (fail CLOSED — daemon
# unavailable rejects the cycle's filings) before it may land in the
# `## Proposed` lane. The operator flips `proposed` rows to `open`; the
# loop never self-flips — EXCEPT the 2026-11-06 amendment's
# auto-ratification, which ideate.py itself performs on the 7d-old rows
# the operator blanket-approved (never the internal-artifact rows: the
# never-dispatch marker is never flipped, never dispatched).
IDEATION_SECS=604800 # 7d — the ideation timer's only knob
IDEATION_SPEC=specs/ideation-cycle.md
IDEATION_GOAL="T326 ideation cycle, exactly per the spec: (1) run python3 scripts/ideate.py — it is deterministic (no network); it auto-ratifies eligible Proposed rows (the amendment's 7d sweep) and files at most 3 noul-gated outward-facing stubs; (2) run python3 scripts/ideate.py --check — it must exit 0; (3) commit ONLY what the script changed (TODO.md and/or specs/proposed/) with subject: eval: ideation cycle (T326): <the script's printed disposition lines, joined by ;> ; (4) push. NEVER hand-edit a proposal, NEVER flip a proposed row yourself (the loop never self-flips), NEVER implement a proposal — a rejected proposal stays rejected."
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
  #         closed= laya= conf= route= dormant=
  #   laya=none          the triage layer did not run (the mechanical layer
  #                      settled the cycle, or a stale evaluation is due)
  #   laya=down          fail-open (daemon absent/error/timeout) — T258
  #                      routing stands verbatim
  #   laya=yes|no conf=<c> route=skip|glm|kimi — the triage verdict, its
  #                      confidence, and the route TAKEN
  #   dormant=no         the loop is awake (T325) — the T258/T259 arms below
  #   dormant=suspended-valve   dormant cheap exit: the T247 valve is
  #                      suspended, the skip fires whatever the streak
  #   dormant=audit      the weekly audit timer is due: the one launch
  #                      dormancy schedules, read-only (DORMANT=1 in env)
  local base new deaths book fresh streak verb model
  base=$(last_eval_commit)
  new=$(new_todo_rows "$base")
  deaths=$(cycle_deaths "$base")
  if delta_bookkeeping_only "$base"; then book=yes; else book=no; fi
  if eval_fresh EVALUATION.md; then fresh=yes; else fresh=no; fi
  streak=$(empty_wrap_streak)
  local closed laya="laya=none"
  closed=$(closed_todo_rows "$base")
  # T325 — DORMANCY FIRST, before the T247 valve leg: with the queue empty
  # (zero `todo`/`open` rows — the same count the field echo carries as
  # rows=) AND the dormancy latch present (the run loop arms it at cycle
  # routing time), the cheap exit fires on EVERY cycle whatever the TRUE
  # streak and the valve's 4th-TRUE trip is replaced by the weekly audit
  # timer: a real eval (the FULL orchestrator-model id) launches only when
  # the timer is due, and the audit never consults the triage layer — the
  # launch is mandated mechanically, and a confident-empty verdict must
  # not cancel the one evaluation dormancy schedules. The due decision
  # reads the NEWER of the record and the git anchor (T325-F1: a landed
  # audit's own eval: commit restarts the week for the very next cycle);
  # an unknown half degrades to the due side — the audit — never to a
  # skip.
  local dormant="no" dts
  if [ "$(todo_rows TODO.md)" -eq 0 ] && [ -f "$STATE/dormant-state.json" ]; then
    dts=$(dormant_timer_ts)
    if [ -z "$dts" ] || [ "$(( $(date -u +%s) - dts ))" -ge "$DORMANT_AUDIT_SECS" ]; then
      verb=launch; model=$LOOP_ORCH_MODEL; dormant=audit
    else
      verb=skip; model=-; dormant=suspended-valve
    fi
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
  elif [ "$streak" -ge 3 ]; then
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
  echo "$verb $model rows=$(todo_rows TODO.md) new=$new deaths=$deaths bookkeeping=$book fresh=$fresh base=${base:-none} streak=$streak closed=$closed $laya dormant=$dormant"
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
  local t1="empty-delta" t2="disposition" streak snext fields rc=0 verdict conf reason dormant
  # strip "skip - " (the verb + the no-model placeholder) — the fields line
  # in the subject starts at the predicate inputs
  fields=${1#skip - }
  verdict=$(laya_gate_field laya= "$fields")
  conf=$(laya_gate_field conf= "$fields")
  dormant=$(laya_gate_field dormant= "$fields")
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
  # T325: the dormant skip's disposition names the suspension — the valve
  # did NOT bind this cycle; the T237 token and the TRUE-streak handoff
  # still ride the subject, so the chain's bookkeeping stays continuous
  # through dormancy (the streak walk counts it; the wake cycle resumes
  # the valve from the carried count).
  if [ "$dormant" = "suspended-valve" ]; then
    reason="(T325 dormant: streak=$streak suspended-valve — the weekly audit timer replaces the valve)"
  fi
  snext=$((streak + 1))
  git commit --allow-empty -m "eval: loopd cheap-exit ${t1} ${t2} ${reason} — TRUE streak ${streak}→${snext}; ${fields}; next cycle re-checks, the T247 valve still binds on the TRUE count" >/dev/null 2>&1 || rc=$?
  return "$rc"
}

# T259 — the triage layer's per-run state: the record-sequence suffix
# (ids stay unique per supervisor run) and the quiet flag (the predicate
# probe sets it — the probe writes nothing; run mode leaves it empty so
# each cycle notes its one triage line). T263: the outcome backfill mints
# from the SAME counter, so the outcome's own id and the triage id it
# names as subject can never collide.
LAYA_SEQ=0
LAYA_TRIAGE_QUIET=""

# T354 — the OSC 7501 program-status emitter (the terminal-harness channel).
# loopd reports its own state to the terminal it runs on the terminal-native
# way: one OSC 7501 record on STDOUT — Mitchell Hashimoto's Program Status
# Protocol, the id-less ROOT record the fleet shim re-emits (state + msg, no
# id key — the harness's terminal IS the record's address). The legs map the
# shim's proven semantics onto loopd's own transitions:
#     cycle start / ideation (item) start    -> working
#     cycle OK (= goal complete) / dormant   -> idle
#     build-gate red / delivery (disposition commit) failing / cycle death /
#     HALT                                   -> blocked, with the ONE-line
#                                               reason on the msg channel,
#                                               headed by its T340
#                                               blocked-reason word
#                                               (budget|error — the same
#                                               vocabulary the T340 socket
#                                               surface reports)
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
#   protocol's limits — loopd's one-line reasons sit far under both).
#   base64 absent -> the msg channel degrades away, the state rides alone.
# Probe surface: `./loopd.sh osc <state> [one-line reason]` (the
# routing/sleep-ok/predicate pattern) — the operator's and the tests'
# behavioral leg, driven through THIS function so the fixtures exercise the
# real gate, never a reimplementation.
# T356 — the function is the ONE composition point for BOTH surfaces of a
# transition: the 7501 dot (msg channel, below) and the human heartbeat line
# (loopd_hb_emit, CHUG_LOOP_VERBOSE-gated). One call, one state, one reason —
# the dot and the line can never disagree. An optional 3rd argument carries
# the line's extra detail (cycle number, routing, item, pacing seconds); a
# blocked transition with no detail composes its line FROM the msg, so the
# reason text is byte-identical on both surfaces by construction.
loopd_osc7501_emit() { # <state> [one-line reason] [hb-detail] — TTY-gated, env-killable
  local state="${1:-working}" msg="${2:-}" hb="${3:-}" b64 seq rc=0
  [ -t 1 ] || return 0 # (a) a TTY stdout or nothing — the whole contract
  # T356 — the human leg of the same transition, BEFORE the dot's env gate:
  # the line's knob (CHUG_LOOP_VERBOSE) is the line's own; the dot's knob
  # below never silences it and vice versa. The detail is the line VERBATIM
  # (the T356 fix-up's F2 shapes: the `[loop HH:MM:SS]` clock is the
  # cycle-start line's own composition — the spec shows it there ONLY; every
  # other line is a bare `[loop] …`).
  if [ -n "$hb" ]; then
    loopd_hb_emit "$hb"
  elif [ "$state" = blocked ]; then
    loopd_hb_emit "[loop] BLOCKED: $msg" # the msg IS the line's reason — one source
  fi
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

# T356 — the terminal heartbeat: ONE appended line per supervisor transition
# on stdout, the human-readable sibling of the 7501 dot above (the operator
# report 2026-11-09: the terminal view of a running loop showed nothing —
# every surface wrote files). Same TTY gate as the dot — a redirected
# context (nohup, launchd, a pipe, a log file) receives ZERO lines, log
# parsers must never see one — but its OWN env knob: CHUG_LOOP_VERBOSE
# unset-or-1 emits, any other value silences even on a TTY. Lines are
# appended, never carriage-return redraws (herdr keeps scrollback).
# Best-effort end to end: a closed stdout (EPIPE) degrades to silence, and
# nothing here can trip set -e/pipefail (the T142 house style).
loopd_hb_emit() { # <one line, no trailing newline> — TTY-gated, env-killable
  [ -t 1 ] || return 0 # a TTY stdout or nothing — the whole contract
  case "${CHUG_LOOP_VERBOSE-}" in # unset (or empty) and exactly 1 emit
    ''|1) ;;
    *) return 0 ;;
  esac
  printf '%s\n' "$1" || true # a closed stdout is silence, never a set -e death
  return 0
}

# T356 fix-up (validator F1, blocking) — the predecessor cycle's events log
# must be OFF the watcher's read surface before the watcher exists. The
# mechanism (reproduced by tests/loopd_heartbeat.rs's stale-events fixtures
# on the pre-fix tree): `loopd_hb_watch 200 &` is backgrounded microseconds
# before the chug launch, and the driver rotates .chug/events.jsonl only at
# its own launch (src/driver.rs, ~100ms+ in) — so the milestone leg's
# check-then-sleep FIRST poll read the predecessor's file and LATCHED its
# iteration count: a stale n=173 emitted a false `iter 150/200` line and
# suppressed every real milestone below 175; a stale n=200 emitted
# `iter 200/200` and went milestone-silent for the whole cycle. The flip
# leg primes its git read (first=1); this is the milestone leg's equivalent,
# taken at the READ SURFACE so no polling window remains at all: the stale
# file is rotated aside HERE, before `loopd_hb_watch` is born, and the
# driver's own rotate_fresh then finds nothing to do. The archive name is
# the driver's cycle-stream shape (`events-<YYYYmmdd>-<HHMMSS>.jsonl`,
# same-second collisions -2..-10 like src/archive.rs) so the wrap's
# cycle-stream enumeration (scripts/wrap_assert.py's streak derivation)
# sees the identical corpus it would have seen had the driver rotated
# ~100ms later. An absent or empty log is a no-op ([ -s ], the same
# len>0 rule as rotate_fresh); a failed move leaves the file for the
# driver's own rotation; every leg is rc-guarded — nothing here can trip
# set -e/pipefail (the T142 house style).
loopd_hb_prerotate() { # rotate a predecessor events log aside, best-effort
  [ -s .chug/events.jsonl ] || return 0
  local ts n dst
  ts=$(date -u +%Y%m%d-%H%M%S) || return 0
  for n in '' -2 -3 -4 -5 -6 -7 -8 -9 -10; do
    dst=".chug/events-$ts$n.jsonl"
    if [ ! -e "$dst" ]; then
      mv .chug/events.jsonl "$dst" 2>/dev/null || true
      return 0
    fi
  done
  return 0
}

# T356 — the cycle watcher: while the orchestrator runs, emit ONE milestone
# line every LOOPD_HB_EVERY iterations (default 25 — against the launch's
# 200-iteration budget that bounds a cycle at 8 milestone lines; the volume
# guard is pinned in tests/loopd_heartbeat.rs) read from the cycle's OWN
# events log (.chug/events.jsonl — the predecessor's log is rotated aside by
# loopd_hb_prerotate before this watcher is born, so the first poll can only
# ever read THIS cycle's stream, and the child appends one merged iteration
# line per turn), plus ONE line per observed merge commit (the item flip;
# merges are the queue's serial delivery per LOOP-SPEC). The shapes are the
# spec's: `[loop] <item-id> iter K/max — <last tool, 60>` and
# `[loop] <item-id> done (merge <sha>) — <merge subject, 60>`; the item is
# the queue's top open row's ID — LOOP-SPEC works the queue strictly serially
# in file order. Best-effort end to end: a
# missing events file, a non-git cwd, or an unreadable TODO.md degrades to
# silence; every leg is rc-latched (T142) so neither set -e nor pipefail can
# kill the watcher mid-cycle; the parent kills it the moment the cycle
# capture returns, a dead supervisor (kill -0 $PPID) ends the loop, and the
# poll-count backstop below caps any leak that survives both.
loopd_hb_watch() { # <max_iters> — runs until killed, orphaned, or the backstop
  local max="${1:-200}" every poll n=0 shown=0 polls=0 first=1
  local sha="" last_sha="" subj="" act="" item="" item_id="" detail="" slept=0
  case "${LOOPD_HB_EVERY:-}" in ''|0|*[!0-9]*) every=25 ;; *) every=$LOOPD_HB_EVERY ;; esac
  case "${LOOPD_HB_POLL:-}" in ''|0|*[!0-9]*) poll=20 ;; *) poll=$LOOPD_HB_POLL ;; esac
  while :; do
    kill -0 "$PPID" 2>/dev/null || return 0 # the supervisor died: never outlive it
    polls=$((polls + 1))
    if [ "$polls" -ge 1440 ]; then return 0; fi # backstop: 1440 polls ≈ 8h at the 20s default
    n=$(sed -n 's/.*"type":"iteration".*"n":\([0-9][0-9]*\).*/\1/p' .chug/events.jsonl 2>/dev/null | tail -1) || n=""
    case "$n" in ''|*[!0-9]*) n=0 ;; esac
    # The item hint, read once per poll: the queue's top open row as
    # "T042 — <title>", whose ID half feeds BOTH watcher lines (the
    # milestone's `T042 iter K/max` and the flip's `T042 done (merge …)` —
    # the spec's shapes name the bare ID there; the full title rides only
    # the cycle-start line).
    item=$(top_open_item) || item=""
    item_id=${item%% — *}
    if [ "$n" -ge $((shown + every)) ] && [ "$shown" -lt "$max" ]; then
      shown=$(((n / every) * every))
      if [ "$shown" -gt "$max" ]; then shown=$max; fi
      act=$(grep '"type":"tool_result"' .chug/events.jsonl 2>/dev/null | tail -1 | sed -n 's/.*"name":"\([a-z_0-9]*\)".*/\1/p' | cut -c1-60) || act=""
      if [ -n "$item_id" ]; then
        loopd_hb_emit "[loop] $item_id iter $shown/$max — $act"
      else
        loopd_hb_emit "[loop] iter $shown/$max — $act"
      fi
    fi
    sha=$(git rev-parse --short HEAD 2>/dev/null) || sha=""
    if [ -n "$sha" ] && [ "$sha" != "$last_sha" ]; then
      if [ "$first" -eq 1 ]; then
        first=0 # prime: the pre-cycle HEAD is context, never a flip
      else
        subj=$(git log -1 --format=%s "$sha" 2>/dev/null) || subj=""
        case "$subj" in
          merge:*)
            # The spec's flip shape: `[loop] T042 done (merge abc1234) —
            # <merge subject, 60>` — the `merge:` prefix is redundant beside
            # `(merge $sha)` and is stripped (best-effort: an empty tail
            # omits the ` — `).
            detail=$(printf '%s' "$subj" | sed -n 's/^merge:[[:space:]]*//p' | cut -c1-60) || detail=""
            if [ -n "$detail" ]; then
              if [ -n "$item_id" ]; then
                loopd_hb_emit "[loop] $item_id done (merge $sha) — $detail"
              else
                loopd_hb_emit "[loop] done (merge $sha) — $detail"
              fi
            else
              if [ -n "$item_id" ]; then
                loopd_hb_emit "[loop] $item_id done (merge $sha)"
              else
                loopd_hb_emit "[loop] done (merge $sha)"
              fi
            fi
            ;;
        esac
      fi
      last_sha=$sha
    fi
    # The poll wait, sliced to 1s: a killed watcher must never leave a
    # long-sleeping orphan holding the pty stdout (the T354 fixtures' drain
    # bounds assume ≤1s stragglers), and a sliced wait stays responsive to
    # the parent's kill at every slice boundary.
    slept=0
    while [ "$slept" -lt "$poll" ]; do
      sleep 1 || return 0
      slept=$((slept + 1))
    done
  done
}

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
  update-check)
    # T352: run the auto-update check ONCE, now — the same call the while
    # body makes between cycles (the routing/sleep-ok probe pattern: the
    # operator's and the tests' behavioral surface for the opt-in
    # CHUG_AUTO_UPDATE=1 self-update). Writes what the real call writes
    # (the cadence latch + the log lines); launches nothing.
    loopd_auto_update_check
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
  osc)
    # T354: emit ONE status through the real gated emitter — the
    # routing/sleep-ok/predicate/update-check probe pattern: the operator's
    # and the tests' behavioral surface for the OSC 7501 legs. stdout IS the
    # channel, the gate decides (piped stdout stays byte-silent), nothing
    # launches, nothing is logged.
    #   loopd.sh osc <state> [one-line reason]
    # T356: the emitter is now the ONE composition point for BOTH surfaces
    # of a transition, so this probe drives both. No 4th argument → the
    # line is silenced (the T354 wire fixtures' exact dot-only captures);
    # a 4th argument asks for the human line too, composed in the same call.
    #   loopd.sh osc <state> [one-line reason] [hb-detail]
    case "${4:-}" in
      '') CHUG_LOOP_VERBOSE=0 ;;
    esac
    loopd_osc7501_emit "${2:-working}" "${3:-}" "${4:-}"
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
  *) echo "usage: loopd.sh [run|stop|status|routing] [sleep-ok] [predicate] [osc]" >&2; exit 2 ;;
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
cycles=0 # T356 — the heartbeat's cycle number: LAUNCHES this supervisor run
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
  # T352 — the opt-in self-update check, BETWEEN cycles: this sits after
  # the max-loops bound and BEFORE the single-driver probe and any cycle
  # launch, so no cycle is running here and the apply can never interleave
  # with one (the cycle-id capture in tests/update_cmd.rs asserts it).
  # Best-effort end to end — the function returns 0 on every leg.
  loopd_auto_update_check
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
    # T354 — the build gate went red: blocked until a green build (the
    # launch is refused, so no "working" ever follows on this iteration).
    loopd_osc7501_emit blocked "error: build failed (rc=$build_rc) — refusing to launch a stale binary"
    fails=$((fails + 1))
    if [ "$fails" -ge 3 ]; then
      echo "3 consecutive build failures — refusing to launch a stale binary; fix the tree or stop the loop (last build rc=$build_rc)" > "$STATE/HALTED"
      echo "$(ts) HALTED after 3 consecutive build failures" >> "$LOG"
      # T354 — the supervisor is stopping wedged: the last word is blocked.
      loopd_osc7501_emit blocked "error: HALTED after 3 consecutive build failures"
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
  # T260: build the state+delta read-path input beside the digest — the
  # read-path verdict (.chug/eval-delta.md) + the mechanical delta since the
  # eval-state marker, so a state-hit evaluation reads ~2 small files instead
  # of the full corpus. Best-effort like the digest: a nonzero exit must not
  # kill the supervisor under set -e, and the fail-closed default is a
  # FULL-READ cycle (a missing/failed delta never enables the slim path).
  scripts/eval-delta.sh >> "$LOG" 2>&1 \
    || echo "$(ts) eval-delta: nonzero exit (best-effort, ignored — the cycle full-reads)" >> "$LOG"
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
  # T326 — the ideation function definitions ride the main loop, AFTER the
# routing derivation: the T81 pin reads the FIRST chug-run invocation in
# the file text (the ideation launch inside ideation_cycle), so every
# launch-preceding computation (routing, the T178 bash-cap export) must
# precede this block in the file. Definitions are inert text — executing
# them per iteration is a harmless redefinition; the ideation_cycle CALL
# below still follows them.
ideation_state_ts() { # the latch's last_ideation_ts — a clean base-10
  # integer, or EMPTY when the record is missing/unreadable (an unknown
  # half; the timer then falls to the git anchor — the dormant_state_ts
  # shape, built on the SAME latch file T325 armed)
  [ -f "$STATE/dormant-state.json" ] || return 0
  awk '{
    if (match($0, /"last_ideation_ts"[ \t]*:[ \t]*[0-9]+/)) {
      v = substr($0, RSTART, RLENGTH)
      sub(/.*:[ \t]*/, "", v)
      printf "%d\n", v
      exit
    }
  }' "$STATE/dormant-state.json"
}
ideation_newest_lane_ts() { # the git anchor: newest commit touching the
  # Proposed lane (specs/proposed); empty = unknown (non-git cwd, no lane)
  git log -1 --format=%ct -- specs/proposed 2>/dev/null || true
}
ideation_timer_ts() { # the timer's EFFECTIVE anchor (the T325-F1 shape):
  # the newer of the record (last_ideation_ts) and the git source (the
  # newest specs/proposed commit). A landed ideation commit moves the
  # anchor half even when the record stamp failed, so the next cycle never
  # re-fires the same cycle; a crashed child (nothing landed, both halves
  # stale) simply re-fires. The T325 difference: both halves unknown is
  # UNARMED (printed empty) — ideation needs its lane; an unknown clock
  # must never launch into a nonexistent lane.
  local lt gt
  lt=$(ideation_state_ts)
  case "$lt" in ''|*[!0-9]*) lt=0 ;; esac
  gt=$(ideation_newest_lane_ts)
  case "$gt" in ''|*[!0-9]*) gt=0 ;; esac
  if [ "$lt" -eq 0 ] && [ "$gt" -eq 0 ]; then
    printf '%s\n' ""
    return 0
  fi
  if [ "$gt" -gt "$lt" ]; then printf '%s\n' "$gt"; else printf '%s\n' "$lt"; fi
}
ideation_due() { # the timer's due decision: now - anchor >= IDEATION_SECS.
  # Unarmed (both halves unknown) is never due.
  local its
  its=$(ideation_timer_ts)
  case "$its" in ''|*[!0-9]*) return 1 ;; esac
  [ "$(( $(date -u +%s) - its ))" -ge "$IDEATION_SECS" ]
}
ideation_state_write() { # stamp the ideation half AFTER a successful child:
  # last_ideation_ts = NOW (the ideation cycle has no eval: anchor of its
  # own; the specs/proposed git anchor is the newer-half corrective a
  # landed commit provides — the T325-F1 shape). The eval half is
  # PRESERVED from the record, never invented (0 = unknown, the due side
  # the eval timer's max-shape already tolerates). Best-effort like every
  # state write: a latch that cannot land leaves the ideation timer due,
  # which errs toward MORE launches (the safe side for the operator's
  # complaint was silence, never a proposal).
  local let_ts ige
  let_ts=$(dormant_state_ts)
  case "$let_ts" in ''|*[!0-9]*) let_ts=0 ;; esac
  ige=$(date -u +%s)
  printf '{"last_eval_ts": %s, "last_ideation_ts": %s}\n' "$let_ts" "$ige" > "$STATE/dormant-state.json" || true
}
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
# T178 (T326 amendment): the export rides the main loop immediately after
# the routing derivation and BEFORE the ideation + orchestrator launches —
# the T178 pin reads the FIRST chug-run invocation in the file text (the
# ideation launch inside ideation_cycle), so the export must sit before it
# for both the pin and the runtime guarantee (children inherit this env).
# Re-exporting per iteration is idempotent and harmless: it only affects
# chug bash-tool calls, never the supervisor's own ./target build.
export CHUG_BASH_TIMEOUT=300

ideation_cycle() { # the ONE launch the ideation timer schedules: while
  # dormant, on a quiet cycle, ONE glm ideation run executes the
  # deterministic ideate.py (the ideas packet, the noul meta-class gate,
  # the lane filing, the amendment's auto-ratification sweep). The stamp
  # lands AFTER the child returns (the T325-F1 shape): a crashed child is
  # never stamped — the timer stays due and the cycle re-fires — while a
  # landed commit moves the anchor half and stops the duplicate even if
  # the stamp never ran.
  ideation_due || return 0
  local ilog="$STATE/ideation-$(date -u +%Y%m%d-%H%M%S).log" rc=0
  echo "$(ts) ideation cycle (T326): timer due — launching $IDEATION_SPEC on $LOOP_IDEATION_MODEL (log $ilog)" >> "$LOG"
  # T354 — item start: the ideation child is real work the supervisor owns;
  # the watching terminal sees working for its whole (bounded) run.
  # T356 — the ideation heartbeat rides the same call.
  loopd_osc7501_emit working "ideation cycle: proposing outward-facing features" "ideation cycle — proposing outward-facing features (T326)"
  CARGO_TARGET_DIR="$ROOT/target-shared" ./target/release/chug run \
    --spec "$IDEATION_SPEC" --goal "$IDEATION_GOAL" \
    --model "$LOOP_IDEATION_MODEL" --max-iters 16 --max-minutes 15 \
    > "$ilog" 2>&1 || rc=$?
  if [ "$rc" -eq 0 ]; then
    ideation_state_write
    echo "$(ts) ideation cycle (T326): child ok — timer stamped (last_ideation_ts=$(ideation_state_ts))" >> "$LOG"
  else
    echo "$(ts) ideation cycle (T326): child FAILED (rc=$rc) — timer NOT stamped, re-fires next cycle" >> "$LOG"
  fi
  return 0
}

  # T325 — dormancy latch maintenance, at cycle routing time BEFORE the
  # T247 valve leg (the gate below reads the latch). An empty queue arms
  # dormancy — the timer anchor is written ONCE, from git's newest eval:
  # commit, so the loop's own cheap-exit dispositions (empty commits) can
  # never refresh it and the week actually elapses; any open row disarms
  # it and logs the wake on the transition cycle. The latch is supervisor
  # state (the STOP/HALTED shape), never a routing authority: the gate
  # re-derives dormancy from the LIVE row count every cycle, so an armed
  # latch cannot out-vote real queue work.
  open_rows=$(todo_rows TODO.md)
  if [ "$open_rows" -eq 0 ]; then
    if [ ! -f "$STATE/dormant-state.json" ]; then
      dormant_state_write
      echo "$(ts) dormant entry (T325): open=0 — the weekly audit timer replaces the T247 valve (last_eval_ts=$(dormant_state_ts))" >> "$LOG"
    fi
  elif [ -f "$STATE/dormant-state.json" ]; then
    rm -f "$STATE/dormant-state.json"
    echo "$(ts) dormant exit (T325): open=$open_rows" >> "$LOG"
  fi
  # T325: the dormancy verdict rides the gate line (eval mode only — these
  # start empty so the launch-site export block below reads them safely on
  # routine cycles, where the gate is never consulted).
  gate_dormant=""
  gate_streak=""
  if [ "$mode" = eval ]; then
    gate="$(eval_gate)"
    gate_verb=${gate%% *}
    gate_rest=${gate#* }
    gate_model=${gate_rest%% *}
    # T325: the dormancy verdict rides the gate line — the skip arm names
    # the suspension in loopd.log, the audit arm carries DORMANT=1 to the
    # launch site below.
    gate_dormant=$(laya_gate_field dormant= "$gate")
    gate_streak=$(laya_gate_field streak= "$gate")
    # T259: the triage's decision record (the state pack verbatim, the
    # verdict, the confidence, the route taken) + the previous triage's
    # outcome backfill — run mode only, best-effort, before the skip or
    # launch the record describes.
    laya_record "$gate"
    if [ "$gate_verb" = skip ]; then
      # T325: the dormant skip names the suspension in its own line BEFORE
      # the T258 disposition record — the loop went quiet ON PURPOSE (the
      # valve is suspended behind the weekly audit timer), never because
      # the predicate happened to look empty.
      if [ "$gate_dormant" = "suspended-valve" ]; then
        echo "$(ts) dormant cheap-exit (T325): streak=$gate_streak suspended-valve" >> "$LOG"
        # T326 — the ideation cycle rides the quiet cycles: while dormant,
        # when the ideation timer is due, ONE glm launch runs
        # scripts/ideate.py (the audit arm never reaches here — the one
        # launch dormancy schedules for the EVALUATION side is the audit;
        # ideation launches only on a skipped cycle, never alongside it).
        ideation_cycle
      fi
      echo "$(ts) cheap-exit disposition (T258): ${gate#skip } — launch skipped" >> "$LOG"
      if loopd_write_disposition "$gate"; then
        fails=0
        skip_secs=$(ok_sleep_seconds)
        echo "$(ts) cheap-exit pacing: sleeping ${skip_secs}s (empty streak $(empty_wrap_streak))" >> "$LOG"
        # T354/T356 — a held skip is the loop at rest: idle for the pacing
        # sleep, and the same call carries the human line (the dormant one
        # names the next probe — the pacing the operator watches).
        if [ "$gate_dormant" = "suspended-valve" ]; then
          loopd_osc7501_emit idle "dormant: queue empty" "[loop] queue empty — dormant, next probe ${skip_secs}s"
        else
          loopd_osc7501_emit idle "cycle skipped: no evaluation needed now" "[loop] no evaluation needed — skipped, next probe ${skip_secs}s"
        fi
        sleep "$skip_secs"
        continue
      fi
      # The disposition commit failed (identity, hook, read-only fs) — the
      # safe side is the launch: a skip whose git record never landed would
      # be invisible to the T237 streak walk and the T247 valve, and the
      # chain would cheap-exit forever unpaced.
      echo "$(ts) cheap-exit disposition commit FAILED — falling through to a real $LOOP_ORCH_MODEL eval" >> "$LOG"
      # T354 — delivery failing: the git record the whole bookkeeping reads
      # (the T237 streak walk, the T247 valve) could not be written. The
      # blocked word is momentary — the launch that follows reports working.
      loopd_osc7501_emit blocked "error: disposition commit failed — falling through to a real eval"
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
  # T325 — the dormant weekly audit: the ONE launch dormancy schedules. The
  # timer does NOT restart here — the T325-F1 lesson: a launch-time rewrite
  # re-reads the anchor BEFORE the audit's eval: commit lands, so a
  # record-only restart keeps the stale week and the next cycle re-fires
  # the audit (the duplicate this fix removes). The restart is the audit's
  # OWN eval: commit; the gate reads the newer of record and anchor
  # (dormant_timer_ts), and the record re-arms AFTER the child returns
  # (the re-arm block below the launch). The child env carries
  # DORMANT=1 (LOOP-SPEC's dormant paragraph: the audit is READ-ONLY —
  # gates green, pins verified, site-sync freshness, daemon/Langfuse
  # liveness — filing rows only on observed breakage, never loop-tooling
  # meta-work), and every other cycle launches with DORMANT explicitly
  # unset — an export inside this loop would otherwise persist across
  # iterations (the T47 lesson; DORMANT affects no build, but the unset
  # keeps every non-audit child env honest).
  if [ "$gate_dormant" = "audit" ]; then
    echo "$(ts) dormant weekly audit (T325): due — read-only audit launching, DORMANT=1 (the restart is the audit's own eval: commit)" >> "$LOG"
    export DORMANT=1
  else
    unset DORMANT
  fi
  echo "$(ts) routing: todo_rows=$(todo_rows TODO.md) eval_fresh=$(eval_fresh EVALUATION.md && echo yes || echo no) -> $mode cycle on $orch_model" >> "$LOG"
  echo "$(ts) cycle start -> $cycle_log ($mode cycle, orchestrator $orch_model)" >> "$LOG"
  # T354 — item start: the cycle launch is the supervisor's one "working"
  # span (the orchestrator owns per-item turns inside it).
  # T356 — the cycle-start heartbeat: the ONE line the watching terminal
  # reads when a cycle opens — the UTC clock (the spec's cycle-start shape
  # carries it; every OTHER heartbeat line is a bare `[loop] …`), the cycle
  # number (launches this supervisor run), routing, the routed model's short
  # name, and the queue's top open row (the item a fresh cycle works, per
  # LOOP-SPEC's serial queue order). Composed BESIDE the dot in the same
  # call — one transition, both surfaces.
  cycles=$((cycles + 1))
  hb_item=$(top_open_item) || hb_item=""
  hb_detail="[loop $(date -u +%H:%M:%S)] cycle $cycles routing=$mode (${orch_model##*.})"
  if [ -n "$hb_item" ]; then
    hb_detail="$hb_detail item=$hb_item"
  fi
  loopd_osc7501_emit working "cycle start: $mode cycle on $orch_model" "$hb_detail"
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
  # T356 — the cycle watcher rides the launch: milestone lines (every
  # LOOPD_HB_EVERY iterations, read from the child's own events log) and
  # merge-flip lines while the child runs, killed the moment the capture
  # returns. The watcher's max is the launch's iteration budget — the same
  # 200 literal the T81/T230 pins hold byte-stable; the tandem (bump one,
  # bump both) is pinned in tests/loopd_heartbeat.rs.
  # T356 fix-up (validator F1) — the predecessor cycle's events log is
  # rotated aside BEFORE the watcher is backgrounded: the watcher's first
  # poll must never read the previous cycle's iteration count (the driver
  # rotates at its own launch, ~100ms after the watcher's birth — the
  # stale-events fixtures reproduce the false line the first poll used to
  # emit). See loopd_hb_prerotate above.
  loopd_hb_prerotate
  loopd_hb_watch 200 &
  hb_watch_pid=$!
  chug_rc=0
  chug_out="$(CARGO_TARGET_DIR="$ROOT/target-shared" ./target/release/chug run --spec LOOP-SPEC.md \
    --goal "Run the full self-improvement cycle per LOOP-SPEC: evaluate or skip per the freshness rule, work the queue (features are first-class per the amended doctrine — close capability gaps, not only harden), adversarial validation for core-logic items, you own all bookkeeping, push after each item lands green + remainder at wrap. Your wrap IS the next cycle's input — leave TODO.md, EVALUATION.md and specs/ such that a cold next cycle needs zero human words." \
    --model "$orch_model" --max-iters 200 --max-minutes 360 --ctx-warn-at-tokens 100000 \
    2>> "$cycle_log")" || chug_rc=$?
  kill "$hb_watch_pid" 2>/dev/null || true
  wait "$hb_watch_pid" 2>/dev/null || true
  kill -9 "$hb_watch_pid" 2>/dev/null || true # backstop: the watcher can never outlive the capture
  printf '%s\n' "$chug_out" >> "$cycle_log"
  # T325-F1 — the audit's record re-arm, AFTER the child returns: the
  # restart is the anchor the audit itself moved, so the record re-derives
  # from git here — fresh when the audit's eval: commit landed (the next
  # cycle's gate reads max(record, anchor) and the duplicate never fires),
  # unchanged when the child crashed (both halves stale — the timer stays
  # due and the audit simply re-fires next cycle). The value is read from
  # the anchor, never invented: a fabricated `now` would suppress a
  # crashed audit's re-fire for a week.
  if [ "$gate_dormant" = "audit" ]; then
    dormant_state_write
    echo "$(ts) dormant weekly audit (T325): timer re-armed from the git anchor (last_eval_ts=$(dormant_state_ts))" >> "$LOG"
  fi
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
    # T354 — cycle OK = goal complete: the loop is at rest between cycles.
    # T356 — the same call carries the human line, with the accepted goal's
    # own summary (capped) as the detail — the real info the operator asked
    # for, on the same line as the dot's state.
    hb_tail=""
    if [ -n "$summary" ]; then
      hb_tail=" — $(printf '%s' "${summary#summary: }" | cut -c1-80)"
    fi
    loopd_osc7501_emit idle "cycle ok" "[loop] cycle done — goal complete$hb_tail"
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
    # T354 — a cycle death is a blocked state: the msg channel carries the
    # ONE-line reason headed by its T340 blocked-reason word — rc 1 is the
    # budget/abort exit (driver.rs, the T142 verdict mapping), everything
    # else reads error.
    case "$chug_rc" in
      1) osc_word=budget ;;
      *) osc_word=error ;;
    esac
    loopd_osc7501_emit blocked "$osc_word: cycle ended without goal complete (rc=$chug_rc)"
    if [ "$fails" -ge 3 ]; then
      echo "3 consecutive cycles without goal_complete — last: $cycle_log" > "$STATE/HALTED"
      echo "$(ts) HALTED after 3 consecutive failures" >> "$LOG"
      # T354 — the supervisor is stopping wedged: the last word is blocked.
      loopd_osc7501_emit blocked "error: HALTED after 3 consecutive cycles without goal complete"
      exit 1
    fi
    sleep "${LOOPD_SLEEP_FAIL:-300}"
  fi
done
echo "$(ts) stop file present — clean exit" >> "$LOG"
