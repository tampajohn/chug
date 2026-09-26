# T45 — Bundle trivial same-area rows into one round

check: cargo test

## Concern

Per-item fixed cost (worktree + fresh build + dispatch + validation cycle)
is ~15–30 min even when the change is a one-line pin or a doc sentence.
Cycle 18's queue (T38–T43) is mostly that class: advisory text, a pipe-char
rule, a candor pass, const pins, a pruning rule. Run serially that's ~2–3h
for <100 lines of diff. Same fix class as the pipeline's serial structure.

## Repo context

- `LOOP-SPEC.md` §2: one row → one worktree → one child → one validation.
- Precedent that batching is safe: T34's per-item Outcomes doctrine already
  expects multiple rows to close in a session; T8 guard validates rows
  independently of how commits group them.

## Requirements

1. Amend LOOP-SPEC §2 with a bundling rule: the orchestrator MAY dispatch
   ONE child for up to 3 rows in a single round when ALL hold:
   (a) each row's spec estimates ≤ ~30 changed lines (doc sentences, const
   pins, description text); (b) all rows touch the same 1–2 files or are
   docs/doctrine-only; (c) none touches driver.rs/api.rs core loop logic;
   (d) pri ≤ 3.
2. The child's goal lists each row's spec path and requires one commit PER
   ROW (not one squashed commit) so row flips reference their own commits.
3. Validation: ONE kimi round covering the bundle (mutation-test per row
   where feasible). Core-logic adjacency still forces REQUIRED validation;
   a bundle of docs/pins may use orchestrator gates per §2 step 4.
4. Row flips may ride one `todo:` commit naming all rows + refs.

## Tests

- Doctrine item: validation REQUIRED — review checks the bundle-eligibility
  predicate is conjunctive (all four conditions) and the per-row commit rule.
- Acceptance: a later cycle lands a ≥2-row bundle with per-row commits and
  one validation verdict covering the set.

## Out of scope

- Bundling features; bundling across file areas; bundles >3 rows.
