# Your first feature

The loop works a file-based queue, so filing a row *is* the assignment.
The whole arc:

1. **Write a spec** — `specs/tNNN-short-slug.md`: the concern, the
   requirements, the tests, and a mechanical `check:` line. The loop's
   goal gate re-runs that check and accepts nothing but exit 0.
2. **File the TODO row** — one line in `TODO.md`: id, title, spec path,
   priority, status `open`. That row is the contract; there is nothing
   to assign to anyone.
3. **Let the loop pick it up** — under `chug loop run` (after
   `chug loop init`) the next cycle takes the highest-priority open row;
   or run it directly:
   `chug run --spec specs/tNNN-short-slug.md --goal "Land X"`.
4. **Read the harvest** — the landed commit, the validation verdict, the
   merged LEDGER, and the artifacts under `.chug/` (events, ledgers,
   decision records). The wrap is the paper trail.

## What a good first item looks like

- **Small** — roughly 150 changed lines or less. Review time and the
  validator's low-stakes lane both scale with diff size.
- **Checkable** — the spec's `check:` is a command, not a vibe: a test
  that fails before and passes after, a script that exits nonzero on the
  bug and zero on the fix.
- **Outward** — docs, a runbook, a CLI surface, a rendered page: work
  whose result a reader can see. It is also the loop's own favorite food
  — this page and every runbook were filed and landed this way.

Bad first items: sweeping refactors (unbounded), anything without a
mechanical check (the loop cannot prove done), and infrastructure your
teammates have not agreed to (the queue is public — file it, propose it,
let it be picked up deliberately).
