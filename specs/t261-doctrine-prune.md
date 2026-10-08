# T261 — doctrine prune: triage-on-change, backfill batching, weekly README audit

check: cargo test

estimate: ~200 lines (three doctrine edits + pins)

## Concern

Operator 2026-10-07: "do we have doctrine that's no longer needed?"
Three candidates, each a cycle-20 rule now running as dead weight on
an empty or mature loop: (a) an eval-triage record for EVERY
weighed-and-rejected candidate (T70) — on a quiet eval that's dozens
of writes recording "no" repeatedly; (b) outcome backfills scattered
per item per cycle — bookkeeping iterations that batch cleanly at
wrap; (c) the README cold-read audit on EVERY eval (META-META §6) —
T184's digest already surfaces drift; per-eval is redundant.

## Repo context

- T70 decision_log: the always-reminder doctrine keeps RECORDS of
  decisions; the prune changes triage cadence, not record honesty.
- Outcome backfills exist because done-entries need Outcomes rows —
  batching at wrap keeps the record, drops the per-item iteration
  cost.
- META-META §6 README cold-read: valuable weekly; per-eval it is the
  same finding re-written (eval cycles keep noting it).
- The T189 lane's mechanical predicate is the model for pruning by
  rule, not by vibe: each prune keeps a mechanical trigger.

## Requirements

1. Triage records: write one per candidate whose disposition CHANGED
   since the last eval (new reject, reject->file, file->abandon),
   not one per candidate per eval. The decision_log record names the
   change; a quiet eval writes zero triage records.
2. Outcome backfills batch at wrap: one Outcomes entry per cycle
   listing items, not per-item scattered entries (the cycle-79-style
   one-line backfills become the wrap's table).
3. README cold-read audit moves to weekly (UTC-week marker in
   EVALUATION.md); the T184 digest's drift surfacing stays per-cycle.
4. Each prune keeps a mechanical trigger (change-detection, wrap
   boundary, UTC week) — doctrine text updated in META-META-SPEC and
   LOOP-SPEC with the triggers named.
5. Pins: quiet eval writes no triage records; changed disposition
   writes exactly one; weekly marker skips a same-week second audit.

## Tests

- The three pins above; decision_log record-shape pins stay green.

## Out of scope

- Removing decision_log or Outcomes themselves; pruning the T247/T237
  pacing doctrine (fresh, load-bearing); any validation requirement.
