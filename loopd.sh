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
set -u
ROOT="$(cd "$(dirname "$0")" && pwd)"
cd "$ROOT"
STATE=.chug/loopd
mkdir -p "$STATE"
STOP=.chug/STOP-LOOP
PIDFILE=$STATE/loopd.pid
LOG=$STATE/loopd.log

ts() { date -u +%Y-%m-%dT%H:%M:%SZ; }

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
  status)
    if [ -f "$PIDFILE" ] && kill -0 "$(cat "$PIDFILE")" 2>/dev/null; then
      echo "loopd RUNNING (pid $(cat "$PIDFILE"))"
    else
      echo "loopd NOT running"
    fi
    [ -f "$STATE/HALTED" ] && echo "HALTED: $(cat "$STATE/HALTED")"
    echo "--- recent:"
    tail -5 "$LOG" 2>/dev/null
    latest=$(ls -t "$STATE"/cycle-*.log 2>/dev/null | head -1)
    [ -n "$latest" ] && { echo "--- latest cycle ($latest):"; tail -3 "$latest"; }
    exit 0
    ;;
  run) ;;
  *) echo "usage: loopd.sh [run|stop|status|routing]" >&2; exit 2 ;;
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
  # I1). The [c]hug bracket keeps the grep pipeline's own argv out of the
  # match; the supervisor's own argv (`bash .../loopd.sh`) holds no needle
  # and `chug chat` must not match by design.
  if ps -ax -o command= | grep -q "[c]hug run --spec LOOP-SPEC.md"; then
    echo "$(ts) another LOOP-SPEC driver active; skipping" >> "$LOG"
    sleep 120
    continue
  fi
  cargo build --release >> "$LOG" 2>&1
  # T78: the loop runs the RELEASE binary — this build produces
  # ./target/release/chug, the cycle invocation below launches it, delegate
  # children re-launch the orchestrator's own executable (current_exe), and
  # the review/validation gates run the T82 gate runner — `cargo nextest
  # run --release` when cargo-nextest is on PATH, else `cargo test
  # --release` (LOOP-SPEC step 3, META-SPEC gates rule). First cycle after
  # this lands pays a cold release build here AND into the shared cache;
  # every later one is warm.
  # T46: refresh the Phase-1 corpus digest so every cycle's evaluation reads
  # .chug/eval-digest.md instead of re-mining raw events archives.
  scripts/eval-digest.sh >> "$LOG" 2>&1
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
  # T47: the shared cache reaches the cycle as a per-invocation env prefix on
  # the chug call — NEVER as a bare `export`. An export inside this while loop
  # would persist in the supervisor's environment across iterations, so from
  # cycle 2 on the `cargo build --release` above would run with the var set,
  # land in target-shared, and never refresh ./target/release/chug (stale
  # supervisor binary). The prefix is visible only to this cycle's
  # orchestrator process and the delegate children that inherit its launch
  # env; the supervisor's own build above always stays in ./target, so
  # ./target/release/chug keeps resolving to a freshly built binary.
  CARGO_TARGET_DIR="$ROOT/target-shared" ./target/release/chug run --spec LOOP-SPEC.md \
    --goal "Run the full self-improvement cycle per LOOP-SPEC: evaluate or skip per the freshness rule, work the queue (features are first-class per the amended doctrine — close capability gaps, not only harden), adversarial validation for core-logic items, you own all bookkeeping, push after each item lands green + remainder at wrap. Your wrap IS the next cycle's input — leave TODO.md, EVALUATION.md and specs/ such that a cold next cycle needs zero human words." \
    --model "$orch_model" --max-iters 200 --max-minutes 240 \
    >> "$cycle_log" 2>&1
  if grep -q "chug: goal complete" "$cycle_log"; then
    summary=$(grep "^summary:" "$cycle_log" | head -1 | cut -c1-200)
    echo "$(ts) cycle OK: $summary" >> "$LOG"
    # T98: best-effort site stats sync — one line, failure-tolerant. The
    # script itself never fails a cycle (missing clone / rejected push /
    # marker-less page all warn and exit 0; a nonzero exit here is logged
    # and ignored — site sync is observability, not a gate).
    scripts/site-sync.sh >> "$LOG" 2>&1 \
      || echo "$(ts) site-sync: nonzero exit (best-effort, ignored)" >> "$LOG"
    fails=0
    sleep 60
  else
    fails=$((fails + 1))
    echo "$(ts) cycle ended WITHOUT goal complete (consecutive failures: $fails)" >> "$LOG"
    if [ "$fails" -ge 3 ]; then
      echo "3 consecutive cycles without goal_complete — last: $cycle_log" > "$STATE/HALTED"
      echo "$(ts) HALTED after 3 consecutive failures" >> "$LOG"
      exit 1
    fi
    sleep 300
  fi
done
echo "$(ts) stop file present — clean exit" >> "$LOG"
