# T188 — auto-spec: chug drafts its own spec from a bare goal

check: export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared && cargo test

estimate: ~400 lines (draft step + UI + tests)

## Concern

Operator 2026-10-01 ("what's to stop chug by default for tasks?"): the
spec tax is the #1 barrier — every task needs a spec with a check: line,
and for small tasks authoring one costs more than doing the task.
Interactive harnesses need zero setup. chug should draft the spec itself:
`chug run --goal "..." --auto-spec` (or `chug quick`) does one kimi call
to draft spec+check from the goal, shows it, and runs it.

## Repo context

- SPEC format: check: line + concern/repo-context/requirements/tests —
  ~100 feature examples exist under specs/ (the loop's own bar: one
  concern, requirements, tests, acceptance, check:).
- T150 guard: every queued spec carries an estimate: line.
- plan mode (T73/T146): read-only drafting machinery + --approve gate
  already exist — the draft step reuses plan's read-only loop, not new I/O.
- The spec is a file: a drafted spec lands at .chug/auto-spec.md and the
  run proceeds against it — the operator can edit before acceptance.

## Requirements

1. `chug run --goal "<goal>" --auto-spec`: draft phase (one LLM call,
   read-only) writes .chug/auto-spec.md (concern, requirements, tests,
   acceptance, `check:` + `estimate:` lines) from the goal + repo scan.
2. Acceptance gate: headless runs accept the draft only if the check:
   line EXECUTES and passes dry-run (a vacuous check is rejected and
   redrafted once); chat mode shows the draft for operator edit/approve
   (T146's gate pattern) before the run.
3. The run proceeds with the drafted spec + the original goal verbatim;
   the drafted spec is committed in .chug (gitignored, resumable).
4. Doctrine sentence in SPEC.md/README: auto-spec is for task-class work
   (chores, small features); adversarial/loop work keeps hand-written
   specs. Auto-spec'd runs get the low-stakes validation lane (T189)
   unless --validate is passed.
5. Failure honesty: if the draft's check can't pass dry-run twice, abort
   with the draft + the failing output — never loosen the check to pass.

## Tests

- Draft produces all required sections + check: + estimate: (fixture).
- Dry-run rejection: a vacuous `check: true`-class line is caught and
  redrafted; a second failure aborts (no silent loosening).
- End-to-end: scripted goal → draft → run → goal_complete with the
  drafted spec's check actually run (mock Llm).

## Out of scope

- Editing hand-written specs; auto-spec for LOOP-SPEC/meta items (those
  keep hand-written doctrine, always); multi-goal batching.
