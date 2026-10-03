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

## The judge daemon (T204/T215)

The risk gate's judge (`CHUG_JUDGE=daemon`, the default) is a host-scoped
daemon: one per box, serving every run over `~/.chug/daemon.sock`. On loop
hosts it comes from the installed release binary — the release workflow
builds `--features daemon` into `~/.local/bin/chug`. The repo dev builds
are CLIENTS ONLY (T204 keeps them feature-lean): their `daemon` subcommand
is the fail-open stub, so never point the ensure at a repo build.

`loopd.sh` resolves the daemon binary ONCE per run (one startup line in
`.chug/loopd/loopd.log`) and ensures it at every cycle start, best-effort:

1. `$CHUG_DAEMON_BIN` — explicit override (wins when executable);
2. `~/.local/bin/chug` — when `chug daemon --help` exits 0 (a pre-T204
   release refuses the subcommand: the dogfood upgrade hasn't landed —
   upgrade the install; `loopd` never self-upgrades);
3. the repo release build — only when it hosts the judge (never loopd's own
   feature-lean build);

else the ensure is skipped for the whole run behind that one log line
(fail-open: the judge client degrades per command exactly as it did when
layad was down). `LOOP_DAEMON_ENSURE=0` opts out entirely.

**First run — pre-warm the weights (the pinned remedy, T215):** the daemon
loads the ~650 MB checkpoint BEFORE the socket binds, and the ensure's wait
budget stays bounded, so a cold first load exceeds it: the first cycle runs
fail-open while the detached daemon finishes downloading, and later cycles
find it warm. To pay the download outside a cycle, pre-warm once:

```bash
~/.local/bin/chug daemon      # foreground; Ctrl-C after "judge model warm"
```

The HF cache then serves every later spawn in seconds. `chug daemon
--status` reports running/starting/absent; `chug daemon --stop` stops the
lock holder. Diagnostics: `~/.chug/daemon.log`.
