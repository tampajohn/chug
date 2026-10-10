check: cd "$(git rev-parse --show-toplevel)" && export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared && touch src/*.rs tests/*.rs && grep -q 'probe-then-assert-delta' META-META-SPEC.md && grep -q 'never the hardcoded constant' META-META-SPEC.md && cargo test --release --test todo_consistency --test eval_outcomes_carry --test eval_state_delta --test loop_spec_doctrine_prune --test internal_info_lint

# T270 — META-META-SPEC verification-helper clause: probe-then-assert-delta, never the hardcoded constant

## One concern

The eval/wrap verification helpers (the python compositions that write
EVALUATION.md entries and rewrite `.chug/eval-state.md`) gate their
writes behind structural self-check assertions — the T266 discipline.
Twice now a helper's assertion has fired on STRUCTURALLY CORRECT
content because its expectation was a hardcoded constant that
mis-calibrated against the artifact's legitimate shape:

1. trip 75 (d1791605730-7, the sub-class's first sighting): the
   Outcomes-entry write helper's post-write count assertion expected a
   hardcoded `(6, 6)`; the artifact held `(6, 6)` only if pre-existing
   deliberate textual mentions were ignored — actual was the
   expectation plus those mentions; the write itself verified correct
   on every other assertion, zero casualty.
2. cycle-437 wrap (d1791611467-5, the second fire): the state-write
   helper's non-empty-fields check `len(line) > len(key) + 2` fired on
   the legitimate 1-char value `schema: 1` (9 > 9 is false) across two
   full-script attempts; the composed content verified correct on
   every OTHER assertion (10 key counts == 1, 3 section singles, ring
   12 exact, 5 byte-identical carries + 1 annotated transformation, 6
   rotated-out absent); the file never touched by any attempt, zero
   casualty.

Both fires were the WRONG-EXPECTATION sub-class the trip-75 record
named: the helper's constant, not the artifact, was wrong. The
sub-class's carried trigger — its SECOND fire files this row — is the
filing authority. The pin: an assertion about composed content must
PROBE the property it guards (read the value back, compute the delta
from the actuals at assertion time), never compare against a literal
the author happened to expect.

## Repo context

- Carrier: META-META-SPEC.md, beside the T266 splice-mechanics
  doctrine (the structural self-check paragraph in the eval-state
  rewrite discipline) — the helpers this clause governs are the ones
  T266 mandates.
- The class boundary is already carried in the decision corpus
  (d1791605730-7): fill/wrap-phase verification-helper assertions are
  counted; the anchor-guard's correctly-firing pre-write assertions
  and the ctx-edit pair-violation self-checks are NOT this class.
- Precedent clause shape: T265's "Corpus counts (T265)" paragraph —
  name the method, name the banned alternative, cite the fires.

## Requirements

1. ONE new paragraph in META-META-SPEC.md, adjacent to the T266
   splice-mechanics text, carrying the literal token
   `probe-then-assert-delta` and the literal phrase
   `never the hardcoded constant`.
2. The paragraph states the rule: a verification helper asserts
   PROBED properties — a value read back from the artifact
   (e.g. `line.split(':', 1)[1].strip() != ''` for non-empty), a delta
   computed from the actuals at assertion time (e.g.
   `count == pre_existing + added`) — never the hardcoded constant
   (a fixed length threshold, a fixed count) that can mis-calibrate
   against correct content.
3. The paragraph cites both fires by their records (d1791605730-7's
   (6,6)-count fire at trip 75, d1791611467-5's len-threshold fire at
   the cycle-437 wrap) and names the zero-casualty boundary (the
   assertion fired pre-write every time; the artifact untouched).
4. No other file changes. No TODO.md/LEDGER.md edits by the child.

## Tests

- The check line's two grep needles against META-META-SPEC.md.
- `cargo test --release --test todo_consistency --test eval_outcomes_carry --test eval_state_delta --test loop_spec_doctrine_prune --test internal_info_lint`
  (the suites a META-META-SPEC text change can break).

## Acceptance

- Both needles present exactly once each in META-META-SPEC.md; the
  five check-line suites green; no diff outside META-META-SPEC.md.

- estimate: ~15 changed lines of doctrine (one paragraph) + this spec
