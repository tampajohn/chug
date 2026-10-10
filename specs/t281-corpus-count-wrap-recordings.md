# T281 — corpus-count clause extends to wrap-side recordings: wc -l at that write, composed after the last append or re-probed (META-META-SPEC clause)

check: cd "$(git rev-parse --show-toplevel)" && export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared && touch src/*.rs tests/*.rs && grep -q 'Corpus counts at wrap (T281)' META-META-SPEC.md && grep -q -- 'composed AFTER the cycle' META-META-SPEC.md && grep -q 'Corpus counts (T265)' META-META-SPEC.md && cargo test --release --test todo_consistency --test eval_outcomes_carry --test eval_state_delta --test loop_spec_cheap_exit --test loop_spec_check_wall --test loop_spec_doctrine_prune --test loop_spec_recovery --test loop_spec_validation_lane

## Repo context

- The T265 clause (`**Corpus counts (T265).**` in META-META-SPEC.md's
  `## Write EVALUATION.md` section) pinned the eval-time method: the
  corpus count quoted in EVALUATION.md is `wc -l < .chug/decisions.jsonl`
  measured at eval-write, derived figures use shell arithmetic, and the
  eval-state health line carries the same two numbers.
- The recording-drift class kept firing on the surfaces the clause does
  NOT name — wrap-side recordings (commit messages, Outcomes entries,
  wrap notes). The census reached its SIXTH fire at the cycle-482 wrap
  (d1791656512-12): the wrap-prep commit f22f8cb named all THREE of the
  cycle's wrap supplements yet recorded `corpus 2,047` — stale-by-one-leg
  against the mechanical `wc -l` 2,048 at its own composition (labels 488
  right). Root (knowable): the figure was composed BEFORE the cycle's
  last decision-corpus append, and no re-probe ran at composition.
- The trigger's letter, recorded in the class's open-thread: "the SIXTH
  fire MET the trigger's letter → **trip 89 FILES the doctrine-text row**
  (the mechanical wc -l + enumerated derivation is the only authoring
  shape); root: compose the figure AFTER the cycle's last append, or
  re-probe wc -l at composition". This row is that discharge, filed on
  schedule at trip 89 (the T274/T280 handoff-discharge precedent).
- Zero casualty to date across all six fires: the audit greps the corpus
  file directly and the F13 GO precondition reads mechanical counts —
  but the recorded text is the chain's own memory, and a ±1 there is a
  permanent mis-record handed to every later eval.
- estimate: ~30 changed lines all-in (one META-META-SPEC.md paragraph
  ~8 lines + this spec + the TODO row; the child's own diff is the
  ~8-line paragraph only). Mechanical doctrine addition — no code, no
  test changes.
- **Pin-carrier discipline (T80):** META-META-SPEC.md is a pinned doctrine
  carrier — EIGHT test binaries read it: `tests/todo_consistency.rs`,
  `tests/eval_outcomes_carry.rs`, `tests/eval_state_delta.rs`,
  `tests/loop_spec_cheap_exit.rs`, `tests/loop_spec_check_wall.rs`,
  `tests/loop_spec_doctrine_prune.rs`, `tests/loop_spec_recovery.rs`, and
  `tests/loop_spec_validation_lane.rs` (some with exact-count needles).
  The new paragraph is append-only and must NOT quote any existing pinned
  needle verbatim; the `check:` line runs every reader binary, so a
  broken pin fails the goal gate, not just review. The T265 paragraph's
  own needles (`Corpus counts (T265)`,
  `wc -l < .chug/decisions.jsonl`) must survive intact — the check's
  third grep pins that.

## Requirements

1. Add ONE paragraph to `META-META-SPEC.md`, placed immediately after the
   existing `**Corpus counts (T265).**` paragraph (before the
   `Sections:` list), bold-led `**Corpus counts at wrap (T281).**`,
   carrying the rule:
   - The T265 method binds EVERY wrap-side recording of the corpus size:
     an eval/wrap commit message (subject or body), an Outcomes entry,
     and a wrap-notes line quote only a
     `wc -l < .chug/decisions.jsonl` count measured at THAT write —
     composed AFTER the cycle's last decision-corpus append, or
     re-probed at composition — with the derivation enumerated when the
     figure bundles prior recordings (the `2,050 = 2,035 + 15 enumerated`
     shape).
   - The evidence sentence: the cycle-482 wrap-prep fire is the class's
     sixth — the commit named all three wrap supplements yet recorded
     the pre-append figure (2,047 against the mechanical 2,048 at its
     own composition); compose the figure after the last append and the
     slip class has no window.
2. No other repo file changes (this spec and the TODO row are the
   orchestrator's bookkeeping, not the child's).
3. The paragraph must not duplicate any existing META-META-SPEC sentence
   verbatim (the exact-count pins), must not alter the T265 paragraph
   (its needles survive — the check's third grep), and must not name
   operator-identifying paths (the internal-info lint):
   `.chug/decisions.jsonl` is the only path the rule needs.

## Tests

- The `check:` line's three greps (the new bold lead, the
  `composed AFTER the cycle` needle, the T265 paragraph's survival).
- Every pin binary that reads META-META-SPEC.md green (the `check:`
  line's eight `--test` legs) — the T80 pinned-carrier discipline for an
  md-only edit.

## Acceptance

- META-META-SPEC.md carries the wrap-side corpus-counts paragraph;
  `check:` green.
- Review/merge gates per the docs-only classification (the diff touches
  ONLY `*.md`): the T80 guard floor
  (`cargo test --release --test todo_consistency`) PLUS the eight pin
  binaries named above; the full suite skipped per the T80 letter.
- Validation: kimi REQUIRED (doctrine edit — META-META-SPEC.md is on the
  step-4 core list by construction).
