# Quickstart

One command installs the latest release — platform detected, checksum
verified, no sudo:

```bash
curl -fsSL https://chug.sh/install.sh | sh
```

What it puts where:

- `~/.local/bin/chug` — the binary, and nothing else. No config files, no
  daemon, no dotfiles; every run's state lives in the repo it ran in,
  under `.chug/`.
- If `~/.local/bin` is not on your `PATH`, the installer prints the one
  `export` line to add.

No release for your platform, or you want to live on source:

```bash
git clone https://github.com/tampajohn/chug && cd chug
cargo install --path .
```

## First run

`chug run` takes a spec file with a `check:` line and a goal, then drives
a fixed loop until that check exits zero:

```bash
cat > SPEC.md <<'EOF'
# Fix the flaky login test

The login test passes alone but fails roughly one run in five.
Make it deterministic.

check: cargo test --test login
EOF

chug run --spec SPEC.md --goal "Make the login test deterministic" \
  --max-iters 40 --max-minutes 120
```

The `check:` line is the whole contract: a run only ends when the check
exits 0 on a re-run, never when the model *says* it is done. Point
`--model` at any provider that speaks the Anthropic Messages API —
`ANTHROPIC_BASE_URL` / `ANTHROPIC_AUTH_TOKEN` in the env, or your existing
`~/.claude/settings.json` (chug reads it when the env is empty).

No spec yet? `chug run --goal "Fix the flaky login test" --auto-spec`
drafts one for you, dry-runs its check, and refuses a check that cannot
pass on the current tree. `chug chat` is the interactive sibling; `--tui`
adds the live dashboard to either.

## The continuous loop

Single runs are one item. The supervisor makes it a queue that works
itself: clone the repo and start `loopd.sh` — cycles run back-to-back,
each wrap feeding the next cycle, all state in files:

```bash
git clone https://github.com/tampajohn/chug && cd chug
nohup ./loopd.sh > /dev/null 2>&1 &   # start (detached)
./loopd.sh status                     # liveness + recent cycle activity
./loopd.sh stop                       # exits after the current cycle
```

## What healthy looks like (first 10 minutes)

- the run prints `▪ iter NN …` lines — reading, editing, running bash —
  and every event appends to `.chug/events.jsonl`
- `chug: budget low — N iteration(s) remain` as the ceiling nears; a
  budget death leaves LEDGER.md intact and prints a resume line
- `goal_complete` appears only after the spec's `check:` re-runs and
  exits 0; a rejected claim keeps the loop in charge
- `chug ledger` prints LEDGER.md — the run's external memory
- under loopd: `.chug/loopd/loopd.log` gains one `cycle OK` line per
  completed cycle (plus a `cheap-exit disposition` line on empty days —
  a $0 cycle), `.chug/loopd/cycle-*.log` holds each cycle's stream, and
  `./loopd.sh status` answers "is it alive"
