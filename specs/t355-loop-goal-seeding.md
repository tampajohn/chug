# T355 — `chug loop init --goal`: seed the queue from the spawn's goal (empty-queue spawns go uselessly dormant)

estimate: ~120 changed lines (src/cli loop init goal leg + auto-spec reuse + scaffold writer + fixtures)

## Why

Operator report 2026-11-09 (first real herdr spawn, chug-f94dvv2hkw-1223):
"the queue stayed at 0 for the goal i spawned it with." Mechanism
confirmed: the fleet dialog passes the goal in the SPAWN PROMPT, then
runs `chug loop init && chug loop run` — init scaffolds an EMPTY TODO.md,
the goal never becomes a queue row, and the loop wakes, finds nothing,
and goes dormant. The automation "worked" (T354 reports it idle) while
doing absolutely nothing. A spawned loop that loses its goal is the
integration's core correctness bug.

## What

1. **`chug loop init --goal "text"`** (new flag; also accepted as
   `--goal-file <path>` for long goals): after scaffolding, seed TODO.md
   with row 1 — title from the goal's first sentence, a generated spec
   at `specs/t001-<slug>.md` (reuse the T188 auto-spec path: concern /
   requirements / tests derived from the goal text; if auto-spec can't
   run offline, the spec carries the goal verbatim + a `check:` line
   requiring an operator-visible artifact). Idempotency composes: on an
   already-scaffolded repo, `--goal` appends the row (it is the only
   mutation allowed on `kept` files; reported as `seeded`).
2. **`chug loop run --goal "text"`** convenience: if the queue is empty
   AND a goal is passed, seed-then-run (same seeding code); with a
   non-empty queue, `--goal` is an error (don't silently append to a
   queue you didn't read).
3. **Fleet contract note** (docs/meta-harness.md + the scaffold's
   LOOP-SPEC header comment): harnesses pass the spawn goal via
   `--goal`, not via the session prompt — the prompt is dead text to
   the loop.

## Tests

- init --goal fixture: TODO.md gains a parseable row (6 cells, todo_
  consistency clean) + spec file exists with a check: line.
- Idempotency fixture: init --goal twice → one row (second reported
  `kept`); init --goal on an existing repo with rows → appended as the
  next id.
- run --goal on empty queue → seeds and starts work; run --goal on
  non-empty queue → exit 2 naming the conflict.
- The seeded row's spec passes the goal-gate check extraction (the
  T335 header-anchored parser must find its check: line).

## Compat

No flags, no change: bare `chug loop init` behaves exactly as today
(empty queue + Proposed lane). Only the new flag mutates.

## Out of scope

- Fleet dialog changes (their side: pass the spawn goal as --goal once
  this ships — flagged to Fleet Primary).
- Goal decomposition (one row per goal; multi-row breakdowns are the
  eval cycle's existing job once the loop is alive).
