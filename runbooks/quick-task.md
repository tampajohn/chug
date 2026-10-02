# Runbook: one small task, zero setup

**Use when** you have a single chore — a failing test, a missing helper, a
rename — and don't want to write a spec by hand. **Arc: ~5–20 minutes.**
Headless only (for interactive use, `chug chat`).

Before you start: the `chug` binary on PATH (`cargo install --path .` from
a checkout), model credentials in the env (`ANTHROPIC_BASE_URL` +
`ANTHROPIC_AUTH_TOKEN`, or the `~/.claude/settings.json` fallback), and
your shell sitting in the repo you want changed.

## Run it

```bash
chug quick --goal "<goal>" --model anthropic-system.ai.kimi-k3 \
  --max-iters 40 --max-minutes 60
```

What happens (T188): chug drafts the spec for you — one read-only plan
session writes `.chug/auto-spec.md` (concern, requirements, tests,
acceptance, `check:`, `estimate:`) — then dry-runs the draft's `check:`
line. A check that can't pass on the current tree is refused and redrafted
once; a second refusal aborts with the draft and the failing output. The
check is never loosened to pass.

Budget flags (all optional; defaults 40 iters / 120 min / unlimited
tokens): `--max-iters 40 --max-minutes 60 --max-tokens 2000000` are all
real values you can pass. Validation lane (T189): small diffs run
gates-only by default — the adversarial-validation child is skipped, the
gates are not. Override for this run: `--validate` forces full validation,
`--no-validate` forces gates-only; either is recorded as an operator
decision.

## What you get

- `LEDGER.md` — the model's external memory, `## Done` / `## Next` /
  `## Blockers`; `chug ledger` prints it anytime.
- `.chug/transcript.jsonl` — every message, append-only.
- `.chug/events.jsonl` — the forensics surface, jq-mineable:
  `jq -r '.type' .chug/events.jsonl | sort | uniq -c`
- Exit 0 = goal accepted (stdout carries a `chug: goal complete` block:
  `summary: …`, token cost, then the ledger dump). Exit 1 = budget or
  abort — stderr says `chug: abort: <reason>`, and stdout ends with a
  ready-to-paste `resume: chug run … --resume` hint. Exit 2 = stuck
  (repeated errors) — read the ledger and the transcript tail.

## If it dies at budget (T63)

Same directory, ONE resume attempt:

```bash
chug run --auto-spec --resume --goal "<goal>"
```

`--resume` reloads `.chug/transcript.jsonl` and continues where it died;
with `--auto-spec` it reuses the existing draft, never redrafts. Note the
command: `quick` has no `--resume`, so the resume path is `run`. If the
resume also dies at budget, stop resuming — read `LEDGER.md`; the work is
usually complete-but-uncommitted, so finish the last gate by hand and
commit.
