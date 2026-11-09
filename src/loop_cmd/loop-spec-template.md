# LOOP-SPEC — this repo's continuous loop contract

The loop runs cycles back-to-back; each cycle's wrap is the next cycle's
input. This file is the contract every launch obeys. It was scaffolded by
`chug loop init` for THIS repo — edit it freely; the loop re-reads it at
every cycle.

The loop's work comes from the QUEUE, never from a session prompt: a
spawned goal must be seeded into TODO.md (harnesses pass it as
`chug loop init --goal "<goal text>"`, which files the row + spec; a
prompt-carrying goal is dead text to the loop).

## The queue

- `TODO.md` is the ONE queue. A row is work: id, title, spec path,
  priority, status. Each cycle takes the highest-priority `open` or
  `todo` row; a landed row flips to `done` with a note naming what
  landed.
- Every row cites a spec file under `specs/` that states the concern, the
  requirements, the tests, and one mechanical `check:` line. No spec, no
  row — file the spec and the row together.
- Nothing is worked that is not a row; nothing is landed that is not
  green. Proposals are never self-filed into the queue: they go to the
  `## Proposed` lane in TODO.md and wait for the operator to activate.

## The check: contract

A run only ends when the spec's `check:` command re-runs and exits 0 —
never when a model says it is done. A rejected claim keeps the loop in
charge: fix, commit, and claim again.

## The gates

Before any wrap, on the tree the work lands in:

- the repo's own build and test gates are green — run them, never skip
  them, and never weaken a failing check to pass it;
- the changed files stay inside what the item's spec names — a diff
  touching files outside the spec's targets is a review red flag on its
  face;
- commits land on this repo's branches with honest messages, pushed when
  a remote is configured;
- the bookkeeping (TODO.md statuses, the ledger) is left such that a cold
  next cycle needs zero human words.

## Boundaries

- The loop works THIS repo only: the queue is this repo's TODO.md,
  worktrees live under this repo, commits land on this repo's branches.
- Tools are sandboxed to the run cwd. `.chug/hooks.json` and
  `.chug/permissions.json` are honored when present — the operator's
  guardrails outrank the loop's optimism.
- The loop never invents work to stay busy: an empty queue idles (the
  supervisor sleeps and re-probes), it does not manufacture rows.
