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

if [ -f "$PIDFILE" ] && kill -0 "$(cat "$PIDFILE")" 2>/dev/null; then
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
fails=0
while [ ! -f "$STOP" ]; do
  # Single-driver: never overlap another LOOP-SPEC run (e.g. a manual one).
  if pgrep -f "chug run --spec LOOP-SPEC.md" > /dev/null 2>&1; then
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
