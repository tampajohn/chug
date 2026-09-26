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

case "${1:-run}" in
  stop)
    touch "$STOP"
    echo "stop requested — supervisor exits after the current cycle"
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
  *) echo "usage: loopd.sh [run|stop|status]" >&2; exit 2 ;;
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
  cargo build >> "$LOG" 2>&1
  # T46: refresh the Phase-1 corpus digest so every cycle's evaluation reads
  # .chug/eval-digest.md instead of re-mining raw events archives.
  scripts/eval-digest.sh >> "$LOG" 2>&1
  cycle_log="$STATE/cycle-$(date -u +%Y%m%d-%H%M%S).log"
  echo "$(ts) cycle start -> $cycle_log" >> "$LOG"
  # cycle budget: fresh Phase 1 ≈45–55 iters + ~28–35/item + ~10 wrap (cycle-16 eval Q1); 160 fits eval + 3 items + wrap; minutes never binding (56–117 of 240)
  # T47: the shared cache reaches the cycle as a per-invocation env prefix on
  # the chug call — NEVER as a bare `export`. An export inside this while loop
  # would persist in the supervisor's environment across iterations, so from
  # cycle 2 on the `cargo build` above would run with the var set, land in
  # target-shared, and never refresh ./target/debug/chug (stale supervisor
  # binary). The prefix is visible only to this cycle's orchestrator process
  # and the delegate children that inherit its launch env; the supervisor's
  # own build above always stays in ./target, so ./target/debug/chug keeps
  # resolving to a freshly built binary.
  CARGO_TARGET_DIR="$ROOT/target-shared" ./target/debug/chug run --spec LOOP-SPEC.md \
    --goal "Run the full self-improvement cycle per LOOP-SPEC: evaluate or skip per the freshness rule, work the queue (features are first-class per the amended doctrine — close capability gaps, not only harden), adversarial validation for core-logic items, you own all bookkeeping, push after each item lands green + remainder at wrap. Your wrap IS the next cycle's input — leave TODO.md, EVALUATION.md and specs/ such that a cold next cycle needs zero human words." \
    --model anthropic-system.ai.kimi-k3 --max-iters 160 --max-minutes 240 \
    > "$cycle_log" 2>&1
  if grep -q "chug: goal complete" "$cycle_log"; then
    summary=$(grep "^summary:" "$cycle_log" | head -1 | cut -c1-200)
    echo "$(ts) cycle OK: $summary" >> "$LOG"
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
