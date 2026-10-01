# T189 — low-stakes validation lane (gates-only for small diffs)

check: cargo test

estimate: ~200 lines (routing rule + pins + README)

## Concern

Operator 2026-10-01: kimi adversarial validation is 15–30 min per item —
right for core logic, overkill for chores. Using chug by default needs
validation proportional to stakes. §2 step 4 already skips validation for
docs/tests-only items; this generalizes into an explicit lane with a
mechanical predicate, so small-diff chores ship gates-only while core
logic keeps full adversarial review.

## Repo context

- LOOP-SPEC §2 step 4: validation REQUIRED for src/driver.rs, api.rs,
  tools.rs, events.rs, delegate.rs, mcp*.rs, permissions.rs, hooks.rs,
  trim.rs (the current core list) and doctrine; optional for docs/tests.
- T44/T161 overlap rules interact: the lane decides per item BEFORE
  dispatch (the overlap gate reads the same file lists).
- T70 decision_log: routing decisions already recorded (validation-
  routing class) — the lane's predicate outcome rides that record.

## Requirements

1. Mechanical lane predicate (stated in LOOP-SPEC step 4 verbatim):
   gates-only when ALL hold — (a) diff touches NO core-list file;
   (b) ≤ ~150 changed lines; (c) no new tool/command surface (schema
   enum, CLI flag, MCP tool, hook event); (d) no CI/workflow/spec check:
   line change. Otherwise full adversarial validation.
2. The predicate's inputs are computed from the diff, not the model's
   say-so; the orchestrator records the lane decision via decision_log
   (validation-routing) with the four inputs named.
3. Gates-only lane requirements unchanged and STILL REQUIRED: build +
   clippy + full suite in the worktree, byte-clean review, scope check.
   The lane skips the kimi child, never the gates.
4. Auto-spec'd runs (T188) default to the lane predicate; `--validate`
   forces full adversarial; `--no-validate` forces gates-only (operator
   override, recorded).
5. A lane-eligible diff that the gates catch red is still fixed via
   fix-up child (the lane changes review depth, not quality floor).

## Tests

- Predicate pins: each of the four inputs flips the lane correctly
  (core-file touch forces full, >150 lines forces full, new schema
  surface forces full, check:-line change forces full).
- decision_log record carries the four named inputs + verdict.
- Acceptance: a chore-class item lands gates-only with the routing
  record (recorded in a later cycle's Outcomes).

## Out of scope

- Weakening full validation for core logic (unchanged, REQUIRED);
  sampled/partial mutation testing; auto-escalation heuristics beyond
  the predicate.
