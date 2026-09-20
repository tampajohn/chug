# chug

Autonomous coding harness. Given a spec and a goal, it keeps on chugging.

Design rule #1: **the loop is code, not conversation.** The model never decides
whether to continue — the driver does. Goal and progress live in files on disk,
re-read every iteration, so context trimming can never kill the run.

## Usage

```bash
export ANTHROPIC_BASE_URL=...   # or leave default for api.anthropic.com
export ANTHROPIC_AUTH_TOKEN=... # or ANTHROPIC_API_KEY

chug run --spec SPEC.md --goal "Build X and make the check pass" \
  --model anthropic-system.ai.glm-5-2 --max-iters 40 --max-minutes 120

chug run --resume   # continue an aborted run from .chug/transcript.jsonl
chug ledger         # print current LEDGER.md
```

Put a `check: <shell command>` line in your spec — `goal_complete` is only
accepted when the check exits 0.

## How it stays alive

- **Anti-stall kick** — if the model stops without `goal_complete`, the driver
  injects "consult the ledger, continue" and keeps going
- **LEDGER.md** — external memory the model updates each iteration; injected
  into every turn
- **Verification** — `goal_complete` re-runs the spec's `check:` command;
  failure rejects the claim and the loop continues
- **Stuck tripwire** — 3 identical consecutive tool errors → abort, ledger
  intact, resumable
- **Transcript trimming** — old tool outputs collapse to `[trimmed]` past a
  token estimate; the ledger carries durable state

Tools: `read_file`, `write_file`, `edit_file`, `bash`, `grep`,
`update_ledger`, `goal_complete`. All paths sandboxed to `--cwd`.

See [SPEC.md](SPEC.md) for the full design.
