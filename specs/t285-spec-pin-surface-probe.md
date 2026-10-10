# T285 — spec-quality bar: probe the pin surface before mandating needles (META-META-SPEC clause)

Pri: 4 (spec-quality hygiene, doctrine; two zero-casualty fires, both caught in-flight by the goal gate as designed)

## Problem (two fires of the same root)

The spec-quality class: **a spec authored without probing the pin
surface it perturbs.** Both fires were zero-casualty because the goal
gate caught them — but both cost an in-flight reconciliation the
spec should have made unnecessary.

- **First fire (cycle-490 watch, counted):** a template-quoting spec
  carried a pinned needle verbatim into its own text, colliding with an
  exact-count pin the spec never declared.
- **Second fire (cycle 494, d1791668275-27):** the t282 spec MANDATED a
  case-sensitive bold-lead needle (`Outcomes-entry figures (T282)`)
  without probing the exact-count pin surface it perturbs — the
  Write-EVALUATION window's `Outcomes` census, held at exactly 2 by
  `tests/eval_outcomes_carry.rs` and `tests/loop_spec_doctrine_prune.rs`.
  The spec's own "no other repo file changes" requirement was
  unsatisfiable as written; the impl child reconciled in-flight by moving
  both census pins 2→3 (the kimi validator adjudicated the move MANDATED,
  not scope creep — d1791666415-9 finding 3).

The root is identical: the spec author composed acceptance text from the
doctrine's surface without running the mechanical probe that would have
named the pins in the blast radius.

## Fix

ONE paragraph in META-META-SPEC.md's spec quality bar (the section
carrying the T209 ~500-line estimate ceiling and the T249
internal-info gate), bold-led `**Spec pin-surface probe (T285).**`:

- Before a spec mandates a needle (a check-line grep target, a bold
  lead, a quoted template sentence) over a target file, the author
  runs the mechanical probe: `grep -rn 'matches("' tests/*.rs` plus a
  windowed read of the target file's surrounding pins — every
  exact-count census or window-scoped token pin whose window contains
  the mandated text is NAMED in the spec.
- Every pin the spec's mandated text perturbs is declared in the spec
  with its reconciliation (move the count with the pin's own
  contract-change comment, or reword the mandated text to dodge).
- A spec that declares the pin surface up front never hands the impl
  child an unsatisfiable pair of requirements.

## Acceptance

1. The paragraph lands in META-META-SPEC.md's spec quality bar section,
   bold-led, carrying all three bullets; `grep -q 'Spec pin-surface probe (T285)' META-META-SPEC.md`.
2. No other repo file changes unless a census collision forces one —
   and any forced move is declared in the commit message (the T282
   precedent).
3. Pin-carrier discipline: probe `grep -rn 'matches("' tests/*.rs` for
   the tokens the new paragraph introduces BEFORE finalizing its text.

## check:

cd "$(git rev-parse --show-toplevel)" && export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared && touch src/*.rs tests/*.rs && grep -q 'Spec pin-surface probe (T285)' META-META-SPEC.md && grep -q 'Outcomes-entry figures (T282)' META-META-SPEC.md && grep -q 'Probe hygiene (T284' META-META-SPEC.md && cargo test --release --test todo_consistency --test internal_info_lint --test eval_outcomes_carry --test loop_spec_doctrine_prune

estimate: ~25 lines (md-only doctrine paragraph)
