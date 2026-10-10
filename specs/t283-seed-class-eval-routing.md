# T283 — `eval-routing` joins SEED_CLASSES in src/decisions.rs (schema names the class the convention already uses)
estimate: ~15 changed lines (two sibling edits in src/decisions.rs: the const + its hardcoded pin)

check: cd "$(git rev-parse --show-toplevel)" && export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared && touch src/*.rs tests/*.rs && cargo test --release --bin chug

## Repo context

- The `decision_log` tool's schema description renders its seed-class
  list from `SEED_CLASSES` (src/decisions.rs:81,
  `pub const SEED_CLASSES: [&str; 6]`) via
  `let seed_list = SEED_CLASSES.join(", ");` (src/decisions.rs:114) —
  the const is the single source, the description auto-renders.
- The loop's own class convention (LOOP-SPEC Phase 1: "one
  `eval-triage` record … per candidate"; the eval's own routing record)
  uses SEVEN recurring classes in production — the six seeds PLUS
  `eval-routing`, which the trip-eval orchestrator writes every trip
  (d1791664149-1-class records). `eval-routing` is NOT in the seed
  list, so the schema text the model reads never names it — and the
  orchestrator has mis-classed its own routing record `eval-triage`
  THREE times (the mis-class census: fires at trips 76, 84 — counted 2
  — and the third fire at trip 91, d1791664149-1, the trigger for this
  row). Naming the class in the schema removes the ambiguity the
  free-string escape hatch leaves to memory.
- The unit-test pin in the SAME file (src/decisions.rs, the
  `decision_log_schema_pins_required_six_and_seed_class_tokens` test at
  ~line 501) carries a HARDCODED `PINNED_SEED_CLASSES: [&str; 6]`
  (~line 543) — deliberately NOT `SEED_CLASSES`, so a rename inside the
  const breaks the pin (the M1a mutant lesson). Adding a class changes
  the rendered joined list, so the hardcoded pin must move in the SAME
  diff — the pin's own comment says "Changing a seed class is a
  contract change: update both lists consciously." This row IS that
  conscious change. The test fn NAME also encodes the count
  (`required_six`) — rename it
  `decision_log_schema_pins_required_seven_and_seed_class_tokens` in
  the same diff (the name is referenced nowhere else — verified by grep
  at filing).
- No other consumer pins the list: the description is built only at
  src/decisions.rs:114; the `validation-routing` hits in
  tests/decisions_export.rs / tests/decisions_audit.rs /
  tests/loop_spec_decision_records.rs are fixture record strings, not
  seed-list needles (verified by grep at filing).

## Requirements

1. In src/decisions.rs, change `pub const SEED_CLASSES: [&str; 6]` to
   `[&str; 7]` and insert `"eval-routing"` IMMEDIATELY BEFORE
   `"eval-triage"` — the rendered joined list becomes exactly
   `validation-routing, validation-verdict, recovery-routing, model-fallback, eval-routing, eval-triage, outcome`
   (the two eval classes adjacent, the outcome class last, no other
   reorder).
2. In the same file's test module, change the hardcoded
   `const PINNED_SEED_CLASSES: [&str; 6]` to `[&str; 7]` with
   `"eval-routing"` inserted in the SAME position (immediately before
   `"eval-triage"`), so the per-token legs and the exact joined-list
   needle expect the new seven-class rendering, AND rename the test fn
   `decision_log_schema_pins_required_six_and_seed_class_tokens` →
   `decision_log_schema_pins_required_seven_and_seed_class_tokens`
   (the name encodes the count; it is referenced nowhere else).
3. No other source or test file changes. The description text itself is
   NOT edited (it renders from the const at line 114); no fixture
   strings change; the free-string escape hatch sentence stays.
4. Keep `cargo clippy --all-targets --release -- -D warnings` at zero
   warnings.

## Tests

- The existing `decision_log_schema_pins_required_seven_and_seed_class_tokens`
  unit test (renamed from `..._required_six_...` in this diff) IS the
  acceptance surface: it fails pre-change on the new needle only if
  the pin is edited first (RED leg: update the pin, watch it fail
  against the un-edited const), then passes once the const gains the
  class (GREEN leg). Run the RED leg first and note it in the commit
  message or ledger.
- `cargo test --release --bin chug` green (the unit tests live in the
  bin target; the `check:` line runs exactly this).

## Acceptance

- SEED_CLASSES carries seven classes with `eval-routing` before
  `eval-triage`; the rendered schema description names it; the
  hardcoded pin matches; `check:` green; clippy zero warnings.
- Orchestrator review gates: the full T82 suite
  (`cargo nextest run --release` when nextest is on PATH, else
  `cargo test --release -- --test-threads=4`) + clippy, in the worktree
  — src change, so no docs-only floor.
- Validation lane: decided at dispatch from the diff's four mechanical
  inputs (T189) — src/decisions.rs is NOT on the step-4 core list
  (driver/api/tools/events/doctrine), the estimate is ~15 lines, the
  class list is free-string by design so no closed enum or new tool
  surface is added, and no CI/check-line change results; the
  orchestrator records the routing call with the computed inputs.
