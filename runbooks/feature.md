# Runbook: spec'd feature work

**Use when** the change is a real feature — behavior worth pinning with
tests, a check gate, and adversarial validation. **Arc: 1–2 h per item**
(implementation, validation, a fix-up round if needed).

## 1. Write the spec

Hand-written specs live in `specs/t<N>-<slug>.md`; T191's own is a worked example:

```markdown
# T<N> — <one-line what>

check: <command that exits 0 exactly when this work is verifiably done>

estimate: ~<lines> changed lines

## Concern
<why this exists>

## Requirements
1. ...

## Tests
- <named behaviors that must pass>

## Out of scope
- <explicitly not this item>
```

Gate rules: `goal_complete` is accepted only when the `check:` line exits
0 (it runs with `CARGO_TARGET_DIR` scrubbed from the env — set the
variable inside the check line itself for a shared cache), and a spec on
a `todo` TODO row carries `estimate: ~N changed lines` (T150).

## 2. Run it

```bash
chug run --spec specs/t<N>-<slug>.md \
  --goal "Implement this item ONLY per the spec; commit in this worktree; \
you own LEDGER.md bookkeeping" \
  --model anthropic-system.ai.kimi-k3 --max-iters 80 --max-minutes 120
```

The goal should say: which item ONLY, any env to export before cargo, and
where to commit. The spec is re-read every iteration — you can sharpen it
mid-run.

## 3. Validation and verdict

Core-logic items get adversarial validation — an independent run that
re-derives requirements, re-runs gates, attacks with mutants. Handling:

- **PASS** — merge, close the row, done.
- **FAIL** — relaunch against the same worktree with the findings in the
  goal (a fix-up run starts fresh by design), then re-validate. One
  fix-up round is normal; three is a smell — re-scope the item.
- **Budget death, work complete-but-uncommitted** — one `--resume`
  attempt on the same worktree (T63); a second death: finish by hand
  from `LEDGER.md`.
- A surviving mutant gets a named follow-up on the row — never dropped.