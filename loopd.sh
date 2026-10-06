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
  *) echo "usage: loopd.sh [run|stop|status|routing] [sleep-ok]" >&2; exit 2 ;;
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
  routing="$(route TODO.md EVALUATION.md)"
  mode=${routing%% *}
  orch_model=${routing#* }
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
