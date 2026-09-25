# T21 — LOOP-SPEC impl-child template: --max-iters 40 → 50

check: cd /Users/jadams/workspace/chug && grep -q -- '--max-iters 50' LOOP-SPEC.md && ! grep -q -- '--max-iters 40' LOOP-SPEC.md && cargo test

## Why (evidence)

Cycle-7 evaluation finding **M1**: the iteration ceiling is the loop's
recurring killer of otherwise-successful implementation children — 3 of
the last 5 glm impl children died at exactly 40/40 with the work done:

1. T15 impl (cycle 3): abort 40/40, implementation complete,
   **uncommitted** (`events-t15-impl-20260922-061957.jsonl`).
2. T17 impl (cycle 4): abort 40/40, diff complete + green,
   **uncommitted** (`events-t17-impl-20260925-170831.jsonl`).
3. T20 impl (cycle 6): abort 40/40, **committed `f08ea04` at ~iteration
   37** after T18's WARN=8 fired at 32, then lost 2 iterations to the
   `timeout` exit-127 mirage (T22) and died pre-`goal_complete`
   (`events-t20-impl-20260925-182346.jsonl`).

The children that fit in 40 (T18 at 26, T19 at ~20) were one-constant /
docs-only edits. Every **multi-file feature** child needed more than the
ceiling. The budget-low warning text is already directive ("Stop
starting new work: commit what is done, run the gates, and finish") and
partially worked (the T20 commit landed); what is missing is headroom,
not exhortation. Wall-clock minutes are NOT the binding constraint (T20
used 6 of 35; the deaths are all iteration deaths), so minutes stay put.

Sizing: the deaths needed +4–6 iterations (final gates 2–4 + commit 1 +
goal_complete 1); 50 covers the wrap plus one hiccup. Budget inflation
is not a risk: T18/T19 finished in 20–26 while 40 was available.

## Repo context

- `LOOP-SPEC.md:55` — Phase 2 step 2's implementation-child launch
  template is the ONLY occurrence of `--max-iters 40` / `--max-minutes
  35` in the file (verified by grep at eval time; re-verify before
  editing). It reads:
  `--model anthropic-system.ai.glm-5-3-flash --max-iters 40 --max-minutes 35 \`
- The same template covers fix-up children (§2 step 4 launches them from
  the §2 step 2 template), so one edit covers both.
- Validator children use META-SPEC §6's own template (`--max-iters 40
  --max-minutes 30`) — **unchanged by this row**: validators finished at
  10–29/40 in cycles 5–6 (the T17 validator death was cycle 4, before
  ground-truth-pointer goals), and META-SPEC.md is a human spec the loop
  does not amend here.
- This is a **doctrine edit** — no Rust code changes. `cargo test` must
  stay green trivially; the todo_consistency guard
  (`tests/todo_consistency.rs`) must stay green (this spec's filename
  matches the TODO row).

## Requirements

1. `LOOP-SPEC.md` §2 step 2's impl-child template: `--max-iters 40` →
   `--max-iters 50`. `--max-minutes 35` unchanged; model, goal wording,
   polling guidance, and every other byte of the template unchanged.
2. One short evidence sentence added adjacent to the template (in the
   style of the file's existing parentheticals) naming WHY 50: e.g. the
   3-of-5 glm impl wrap deaths at 40/40 (T15/T17/T20) and that minutes
   were never binding. Keep it to 1–2 lines; do not restructure the
   step.
3. No other edits to LOOP-SPEC.md, and NO edits to META-SPEC.md,
   META-META-SPEC.md, or any `specs/` file other than this one.

## Tests

- This row is doctrine-only; the executable checks are:
  - `grep -q -- '--max-iters 50' LOOP-SPEC.md`
  - `! grep -q -- '--max-iters 40' LOOP-SPEC.md` (no stale template left
    anywhere in the file)
  - `cargo test` green (todo_consistency guard passes: TODO row ↔ this
    spec file ↔ status consistent).

## Acceptance

- The three checks above all pass.
- Adversarial validation (REQUIRED — loop doctrine, LOOP-SPEC §2.4):
  the validator confirms (a) exactly one functional change (the
  threshold), (b) no cross-references broke (§2 step 2's template is
  referenced by step 4's fix-up path), (c) the evidence sentence is
  accurate against the named event streams, (d) validator/child model
  routing and all budgets other than impl `--max-iters` are untouched.
- README gate: no README change — child budgets are loop-internal
  doctrine, not user-facing surface.
