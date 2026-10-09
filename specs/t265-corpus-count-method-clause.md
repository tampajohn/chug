# T265 — eval-text corpus-count method clause: mechanical count + shell-derived arithmetic, never mental arithmetic (META-META-SPEC clause)

check: cd "$(git rev-parse --show-toplevel)" && export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-validate-a && touch src/*.rs tests/*.rs && grep -q 'Corpus counts (T265)' META-META-SPEC.md && grep -q -- 'wc -l < .chug/decisions.jsonl' META-META-SPEC.md && cargo test --release --test todo_consistency --test eval_outcomes_carry --test eval_state_delta --test loop_spec_cheap_exit --test loop_spec_check_wall --test loop_spec_doctrine_prune --test loop_spec_recovery --test loop_spec_validation_lane

## Repo context

- The evaluation text quotes the decision-corpus size every trip (the
  delta bullet + §4's roadmap-pull precondition) and the eval-state health
  line carries the same number. Two recording-drift sightings of one
  class, both absorbed at zero casualty:
  1. trip-37 recorded 1,682 where the mechanical count was 1,685 — a +3
     append-window drift across recordings (triaged at trip 38,
     d1791498040-3, census 0→1). The trip-38 response NAMED the method:
     `wc -l` at eval-write — and named the trigger: "a 2nd sighting after
     the named method files a method-clause doctrine row".
  2. trip-40 recorded "1,698 mechanical pre-triage … 1,700 inclusive of
     this eval's three triage records" — but 1,698 + 3 = 1,701, and the
     next eval's `wc -l` closes the arithmetic exactly (1,701 + 3 later
     records = 1,704). The pre-triage count was mechanical and correct;
     the INCLUSIVE derivative was mental arithmetic and slipped by one
     (detected at trip 41, census 1→2 — the trigger's 2nd sighting).
- Root cause (knowable): the named method covers the MEASURED number only;
  derived figures (an inclusive-of-pending-records number, a +N delta
  against a prior recording) are still computed in the head, and mental
  arithmetic on four-digit counts is a ±1 class under eval-write load.
- Zero casualty to date: the audit greps the corpus file directly, the F13
  GO precondition reads mechanical counts, and the eval-state rewrite is
  verbatim-spliced — but the recorded text is the chain's own memory, and
  a ±1 there is a permanent mis-record handed to every later eval.
- estimate: ~30 changed lines (one META-META-SPEC.md paragraph + this spec
  + the TODO row). Mechanical doctrine addition — no code, no test
  changes.
- **Pin-carrier discipline (T80):** META-META-SPEC.md is a pinned doctrine
  carrier — EIGHT test binaries read it: `tests/todo_consistency.rs`,
  `tests/eval_outcomes_carry.rs`, `tests/eval_state_delta.rs`,
  `tests/loop_spec_cheap_exit.rs`, `tests/loop_spec_check_wall.rs`,
  `tests/loop_spec_doctrine_prune.rs`, `tests/loop_spec_recovery.rs`, and
  `tests/loop_spec_validation_lane.rs` (some with exact-count needles).
  The new paragraph is append-only and must NOT quote any existing pinned
  needle verbatim; the `check:` line runs every reader binary, so a
  broken pin fails the goal gate, not just review. (The T264 spec named 6
  of the 8 — zero needle overlap; this spec names all 8.)

## Requirements

1. Add ONE paragraph to `META-META-SPEC.md`, placed inside the
   `## Write EVALUATION.md` section (immediately after the carry-forward
   paragraph that ends "verify the newest pre-existing cycle's section is
   still present", before the "Sections:" list), bold-led
   `**Corpus counts (T265).**`, carrying the rule:
   - The decision-corpus count quoted anywhere in EVALUATION.md is
     `wc -l < .chug/decisions.jsonl` measured at eval-write, and the
     outcome-label count is
     `jq -c 'select(.class=="outcome")' .chug/decisions.jsonl | wc -l`
     measured in the same probe — never an eyeballed or remembered figure.
   - Every DERIVED figure (an inclusive-of-pending-records number, a +N
     delta against a prior eval's recording) is computed with shell
     arithmetic in the same probe (`echo $((pre + added))`), never mental
     arithmetic — the trip-40 inclusive slip (1,698 + 3 recorded as 1,700)
     is the class's evidence.
   - The `.chug/eval-state.md` health line's corpus field carries the same
     two `wc -l`-measured numbers.
2. No other repo file changes (this spec and the TODO row are the
   orchestrator's bookkeeping, not the child's).
3. The paragraph must not duplicate any existing META-META-SPEC sentence
   verbatim (the exact-count pins), and must not name operator-identifying
   paths (the internal-info lint): `.chug/decisions.jsonl` is the only
   path the rule needs.

## Tests

- The `check:` line's two greps (the bold lead + the `wc -l` needle).
- Every pin binary that reads META-META-SPEC.md green (the `check:`
  line's eight `--test` legs) — the T80 pinned-carrier discipline for an
  md-only edit.

## Acceptance

- META-META-SPEC.md carries the corpus-counts paragraph; `check:` green.
- Review/merge gates per the docs-only classification (the diff touches
  ONLY `*.md`): the T80 guard floor
  (`cargo test --release --test todo_consistency`) PLUS the eight pin
  binaries named above; the full suite skipped per the T80 letter.
- Validation: kimi REQUIRED (doctrine edit — META-META-SPEC.md is on the
  step-4 core list by construction).
