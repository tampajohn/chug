# T116 — todo_update rejects empty titles (todo_add symmetry)

check: cargo test --bin chug todos

## Repo context

Cycle-60's T111 kimi validator (REQUIRED PASS 30/50, 6/6 mutants)
carried three non-blocking findings to this eval; this is the one
actionable inconsistency (the other two — prompt_text corrupt-swallow
and no `deny_unknown_fields` — are by design, rejected with reasons in
EVALUATION.md cycle-61 §2/handoff):

`todo_add` rejects an empty title, but `todo_update` with
`{"id": "t1", "title": ""}` ACCEPTS it — the two title-setting paths
disagree, so an empty title enters the store and rides the `## Todos`
prompt section as a blank line. One rule, both paths: a title being set
must be non-empty (after trimming).

`src/todos.rs` is the only file touched; its tests live in the same
file's unit-test module (bin target), and nothing in `tests/`
integration binaries pins todos.rs behavior beyond the Development
layout line (which lists the file name — untouched here), so the
`--bin chug todos` filter sees every test this change can break
(T114-style breadth reasoning, stated per the spec bar).

estimate: ~30 changed lines (≈8 production + ≈20 tests)

## Requirements

1. `todo_update` rejects a `title` argument that is empty or
   whitespace-only with a tool error of the same shape `todo_add` uses
   for the same offense (name the rule: titles must be non-empty).
2. `todo_add`'s behavior is byte-identical (this row only aligns
   `todo_update`).
3. Whitespace-only is rejected exactly like empty (trim-then-check —
   the two paths share one validation helper so they cannot drift
   again).

## Tests

4. `todo_update` with `""` title → error naming the non-empty rule;
   with `"   "` → same error.
5. Symmetry leg: the same two inputs against `todo_add` produce the
   same error shape (regression pin against future drift).
6. Happy path unchanged: `todo_update` with a real title still updates
   (existing tests cover; keep them green untouched).

## Acceptance

- `cargo test --bin chug todos` green including the new legs; clippy
  clean. Orchestrator review gates run the full suite regardless.

## Out of scope

- `deny_unknown_fields` on the store schema (by design — forward-compat;
  the corrupt-file error leg already covers the failure class; rejection
  logged in EVALUATION.md cycle-61).
- Title length caps, dedup, or removal semantics (F8 phase 2 territory,
  deferred pending organic use).
