# Runbook: loopd operations

**Use when** you run the continuous self-improvement loop — start it, read
its state, stop it, recover from a HALT, clean up worktrees. **Arc: a
daemon (cycles of ~1–4 h each); the ops commands below are seconds.**

## Start / probe / status / stop

```bash
nohup ./loopd.sh > /dev/null 2>&1 &   # start (detached; plain ./loopd.sh = foreground)
./loopd.sh routing                    # what the NEXT cycle would run: mode + orchestrator model (launches nothing)
./loopd.sh status                     # liveness + recent activity + last cycle tail
./loopd.sh stop                       # graceful: touches .chug/STOP-LOOP, exits after the current cycle
```

State lives in gitignored `.chug/loopd/`: `loopd.pid`, `loopd.log`
(supervisor events — routing decisions, build gate, verdicts),
`cycle-<UTC-timestamp>.log` (one per cycle; raw orchestrator and child
output), and `HALTED` when it gave up.

## HALT recovery

Two triggers write `.chug/loopd/HALTED` and exit 1: 3 consecutive build
failures (launching a stale binary is refused) and 3 consecutive cycles
without goal complete.

```bash
cat .chug/loopd/HALTED                        # names the trigger + the last cycle log
tail -30 .chug/loopd/loopd.log                # the failures leading up to it
cargo build --release                         # build trigger: fix the tree first
grep -c "verdict: no goal complete" .chug/loopd/cycle-*.log   # cycle trigger: read the last cycle log
nohup ./loopd.sh > /dev/null 2>&1 &           # restart — startup clears the HALTED marker
```

A restart re-runs the release build gate; a still-broken tree halts again
after three tries, so fix before restarting.

## Worktree cleanup

Children work in `/tmp/chug-loop-t<N>` worktrees. `git worktree remove`
deletes a worktree's untracked `.chug/` silently — harvest first:

```bash
ls -d /tmp/chug-loop-t*                       # what exists
cp /tmp/chug-loop-t<N>/.chug/events.jsonl .chug/events-t<N>-impl-$(date -u +%Y%m%d-%H%M%S).jsonl
cp /tmp/chug-loop-t<N>/LEDGER.md .chug/LEDGER-t<N>-impl-$(date -u +%Y%m%d-%H%M%S).md
git worktree remove /tmp/chug-loop-t<N>       # then `git worktree prune` for stale rows
LOOP_REAPER_DRY_RUN=1 scripts/orphan-reaper.sh   # list leftover loop processes; kills nothing
```

The reaper also runs automatically before every cycle (SIGTERM only,
fail-closed); `LOOP_REAPER=0` opts out.
