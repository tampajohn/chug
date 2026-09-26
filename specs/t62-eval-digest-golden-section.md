# T62 — eval-digest golden-section pin (T46 validator's survivor class)

check: cargo test --test eval_digest

## Repo context

The T46 kimi validator (cycle 19, PASS with watch item) mutation-tested
`scripts/eval-digest.sh` and found a 7/7 survivor class: mutants in
digest OUTPUT FIELDS the tests don't pin survive silently. The existing
`tests/eval_digest.rs` (6 tests) pins behavioral counts against jq
baselines, empty/missing/malformed corpus legs, determinism, and the
TODO/EVALUATION-age corpus inputs — but never the stable OUTPUT SKELETON
(section headings, their order, the per-file block's field order). The
digest is the Phase-1 evaluator's primary corpus read (T46's acceptance
has now held for two fresh evals), so silent shape drift in what the
evaluator parses-by-eye is a real, if low-grade, risk. The T46
validator's recommendation verbatim: "one golden-section pin test per
archive would close the survivor class".

## Requirements

1. `tests/eval_digest.rs` gains a golden-section test that runs the real
   script (the existing `run_digest` harness, pinned clock) against a
   small synthetic corpus and pins the STABLE skeleton, exactly:
   (a) the digest header line's shape (`# Eval digest — T46 pre-computed
   Phase-1 corpus summary`);
   (b) the section headings in order: `## Events files` before
   `## Corpus inputs` before `## Staleness`;
   (c) within one fixture file's block, the field lines appear in the
   pinned order the script emits today (runs/model/spec line →
   iterations → wall → tokens → input-context curve → tools → failed
   tool results → goal → aborts → budget_low/output_truncated) —
   pinning ORDER and field LABELS, not volatile values;
   (d) the staleness block's field labels (`generated-at`,
   `newest-events-mtime`, `corpus age at generation`,
   `events-moved-during-generation`).
2. Volatile values (timestamps, token counts, durations) are matched by
   shape (presence/order/label), not literal — the test must not go red
   on a legitimate data change.
3. Tests-only: every hunk is in `tests/eval_digest.rs`; the script is
   untouched.

## Tests

- The new golden-section test fails when a fixture block's fields are
  reordered or a heading is renamed (implementer hand-checks ONE such
  mutation against the real script's fixture output — e.g. temporarily
  edit the expected order in the test — then reverts).
- All pre-existing eval_digest tests keep passing unmodified.

## Acceptance

- `check:` passes in the impl worktree.
- `git diff --stat` shows only `tests/eval_digest.rs`.
- No `|` in the TODO row's notes cell (T40).
