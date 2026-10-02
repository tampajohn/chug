# Runbook: feasibility eval of an external repo

**Use when** you want to know whether a loop harness can drive some other
repository before committing one to it: build system, test gates, layout,
and what would trip the harness up. This is the internal-monorepo pattern.
**Arc: ~30–90 minutes.**

## 1. Clone, then aim chug at the clone

`--cwd` is the whole sandbox — every file/bash tool lands inside it — so
the eval reads the external repo and writes its deliverable there, never
touching your own checkout:

```bash
git clone --depth 1 https://github.com/owner/repo /tmp/eval-target
chug run --spec eval-spec.md \
  --goal "Evaluate this repo for loop-driven work per the spec; write CHUG-FEASIBILITY.md; change no file but it" \
  --cwd /tmp/eval-target \
  --model anthropic-system.ai.kimi-k3 --max-iters 40 --max-minutes 60
```

The eval spec is ~15 lines, written by hand in the repo that runs chug:
requirement 1 = produce the deliverable; a non-negotiation = no writes
outside it; tests = the deliverable names the build system, the test
command, and measured gate cost. The `check:` gates the deliverable's
existence and verdict line:

```markdown
check: test -f CHUG-FEASIBILITY.md && grep -q '^verdict:' CHUG-FEASIBILITY.md

estimate: ~1 changed line (the deliverable)
```

## 2. Deliverable and verdict format

One file, first line machine-greppable:

```markdown
verdict: FEASIBLE
blockers: none            # or the named gaps, each with evidence
build: <command + observed wall-clock>
test gate: <command + pass count + wall-clock>
layout: <where things live, one screenful>
risks: <what a loop would trip on — cold build times, flaky gates, ...>
```

`FEASIBLE` means a loop could run here with a sane cycle budget;
`NOT-FEASIBLE` names the blockers precisely. Skim `.chug/events.jsonl` in
the clone (`jq -r '.type' .chug/events.jsonl | sort | uniq -c`) to see
where the eval's time actually went.

## 3. If FEASIBLE

One loop per repo is the repo-level lever (the dashd/internal-monorepo pattern):
give the repo its own TODO.md, specs/, and `./loopd.sh` — don't merge it
into another repo's cycle.
