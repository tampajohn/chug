check: cd "$(git rev-parse --show-toplevel)" && export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-validate-a && touch src/*.rs tests/*.rs && grep -q 'deferred-equality scope-or-re-probe' META-META-SPEC.md && grep -q 'scope the prediction to the probe' META-META-SPEC.md && cargo test --release --test todo_consistency --test eval_outcomes_carry --test eval_state_delta --test loop_spec_doctrine_prune --test internal_info_lint

# T271 — META-META-SPEC deferred-equality clause: scope-or-re-probe

## One concern

Trip wraps defer digest-final equalities — predictions of the form
"the digest whole-stream final backstops at the next eval: failed K ==
the K enumerated legs" — so the next evaluation can close the
change-detection net's accounting at the designed backstop read. Three
times now a deferred equality (or an equivalent whole-stream claim)
has been written from a PROBE-SCOPED enumeration, and the digest's
whole-stream final contradicted it:

1. trip 76 (d1791610441-3, the class's first sighting): the wrap
   deferred "failed 2 == the enumerated two" on an enumeration scoped
   to the wrap-notes probe; a tail leg fired between the probe and the
   state write; the digest read failed=3.
2. trip 77 (the trip-78 eval's seventh backstop): the wrap deferred
   "FIVE legs == the trip-78 digest's failed 5"; the digest reads 9 —
   four tail legs the wrap accounting never claimed (a write_file
   cross-tree refusal, a spec-grep exit, a helper traceback the wrap
   called exit-0-masked that the digest counts ok:false, a supplement
   composite with a traceback on stderr).
3. cycle 438 (the trip-78 eval's eighth backstop): the state file's
   open-threads header carried "ZERO ok:false legs this stream" — true
   at the wrap-notes probe (n≈47), false at the whole-stream final:
   the digest reads 6 (the n=53 sh fire + five state-write helper
   legs, all post-probe).

Every miss was caught one chain later by the digest backstop, exactly
the designed detection path; zero casualty each time. The class's
carried trigger — the SECOND deferred-read miss files this row and the
counted census — is the filing authority (the trip-77 miss is the
second; the cycle-438 miss the third; census count 3). The pin: a wrap
deferring a digest-final equality either SCOPES the prediction to the
probe's coverage or RE-PROBES the failed-leg count against the live
events at state-write time before deferring.

## Repo context

- Carrier: META-META-SPEC.md, adjacent to the T270
  probe-then-assert-delta paragraph (the eval/wrap text discipline
  cluster around the T266 splice-mechanics text) — the deferred
  equalities this clause governs are written by the same wrap arcs
  T266/T270 govern.
- The class boundary: DEFERRED equalities and whole-stream
  failed-leg/ctx-edit claims in wrap text (Outcomes entries, state
  open-threads) are this class; the at-commit T257 color (explicitly
  probe-stamped) is NOT — the color's probe stamp IS the scoping.
- Precedent clause shape: T270's paragraph — name the rule, name the
  firing shape, cite the fires by their records.

## Requirements

1. ONE new paragraph in META-META-SPEC.md, adjacent to the T270
   paragraph, carrying the literal token
   `deferred-equality scope-or-re-probe` and the literal phrase
   `scope the prediction to the probe`.
2. The paragraph states the rule: a wrap deferring a digest-final
   equality either (i) scopes the prediction to the probe's coverage
   (name the probe — "K ok:false legs THROUGH the wrap-notes probe at
   n=X; the tail re-probes at the next eval") or (ii) re-probes the
   failed-leg count against the live events at state-write time before
   deferring; an unscoped whole-stream claim written from a
   probe-scoped read is the class's firing shape.
3. The paragraph cites the fires (d1791610441-3's trip-76 first
   sighting; the trip-77 deferred-5 vs digest-9; the cycle-438
   zero-claim vs digest-6) and names the zero-casualty boundary (the
   digest backstop caught every miss one chain later by design; the
   clause is hygiene for the deferred text, not a gate).
4. No other file changes. No TODO.md/LEDGER.md edits by the child.

## Tests

- The check line's two grep needles against META-META-SPEC.md.
- `cargo test --release --test todo_consistency --test eval_outcomes_carry --test eval_state_delta --test loop_spec_doctrine_prune --test internal_info_lint`
  (the suites a META-META-SPEC text change can break).

## Acceptance

- Both needles present exactly once each in META-META-SPEC.md; the
  five check-line suites green; no diff outside META-META-SPEC.md.

- estimate: ~15 changed lines of doctrine (one paragraph) + this spec
