# T161 — Two-impl overlap (T44 extension: {1 impl + 1 validator} → ≤2 impls)

check: cargo test

estimate: ~120 lines (doctrine + guard tests)

## Concern

Measured 2026-09-29 (operator request): ~75% of cycle wall time is the
orchestrator blocked in delegate waits on children (delegate 6,875s of
9,222s tool time in a sampled cycle). Cycles run 53–247 min (median ~3h).
The biggest remaining serialization is impl children running one at a time
when their target files are disjoint. T44 already computes the
disjointness gate — extend it one level: a second impl child may launch
while the first is still flying.

## Repo context

- LOOP-SPEC §2 + Hard rules (T44, 41f61a1): at most 2 children in flight
  (1 validator + 1 impl, never 2 impls); disjoint spec-named target files
  gate; merges strictly serial; doctrine items never overlap.
- T52 role-keyed target dirs: two impls building concurrently MUST NOT
  share an artifact slot — target-shared-impl-a / target-shared-impl-b
  (validator keeps target-shared-validate, gates keep target-shared-gates).
- T63 resume + recovery recipes handle mid-flight child death; two live
  arcs means two live recipes — the recipe format already keys on the item.
- Measured child shape: impl children 65–80 iters (30–60 min), validators
  50 (30–45 min); most impl children finish by iteration 48.

## Requirements

1. Amend LOOP-SPEC §2 + Hard rules: at most 2 children in flight, of
   which **at most 1 validator**; **2 impl children allowed iff** the two
   items' spec-named target files are disjoint (the existing gate, both
   specs read, file lists compared). Never 3+; never 2 validators.
2. Merge order unchanged: strictly serial in queue order (N before N+1).
   A FAIL on N's validator pauses N+1's MERGE (not its impl) until the
   fix-up arc resolves; the orchestrator owns any rebase.
3. Role-keyed build slots: impl children export
   CARGO_TARGET_DIR=target-shared-impl-a / target-shared-impl-b (T52
   lesson — never one shared slot for concurrent impls).
4. Doctrine items (LOOP/META/META-META/SELF-SPEC, TODO.md format, loopd.sh)
   NEVER overlap with anything (existing rule, restated for 2-impl).
5. When the two queued items are NOT disjoint, the orchestrator falls
   back to the current {1 impl + 1 validator} overlap — the new rule
   widens, never narrows, and never forces overlap.

## Tests

- Doctrine item: validation REQUIRED (kimi) — review checks the
  disjointness gate wording, the ≤2-children/≤1-validator cap, the
  role-keyed build slots, the FAIL-pauses-merge (not impl) clause, and
  the fallback-to-old-rule sentence.
- Pin: hard-rules invariant text ("at most 2 children, ≤1 validator,
  disjoint-gated") exists exactly once; the old "never 2 impls" clause
  is gone.
- Acceptance: a later multi-item cycle's events show TWO impl children
  with run_start overlap and disjoint specs (recorded in Outcomes) —
  measured against the ~75%-delegate baseline.

## Out of scope

- 3+ children; 2 validators; overlapping doctrine edits; changing merge
  order; any change to delegate/child budgets.
