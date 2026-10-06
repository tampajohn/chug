# T250 — machine-codename class: operator rules the internal-info lint's one open scope boundary

check: export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared; touch src/*.rs tests/*.rs; cargo test --test internal_info_lint --test todo_consistency

estimate: ~10-60 lines (either a redaction sweep + one pattern, or a doc line codifying the exclusion)

## Concern

T249's lint (tests/internal_info_lint.rs) guards the redaction classes
the operator's 2026-10-06 pass (47cd0d1) addressed. One class is
left OUTSIDE the pattern list, deliberately and documented: the
machine-codename class (the loop-host nickname and the checkout
codename — the lint source's class-5 note names the derivation). The
operator's own sweep is INTERNALLY INCONSISTENT on it: 47cd0d1
redacted two instances (FEATURES.md's "On <codename> — where the loop
lives" → "On the loop's host"; the checkout-codename mention →
"checkout") yet left ~45 word-bounded mentions across ~24 tracked
files (TODO.md rows, loopd.sh comments, scripts/, runbooks, ten
specs, src/testsupport.rs, seven test files).

The lint cannot resolve this itself: a pattern for the class would
flag the 45 live residues (the zero-hit-at-HEAD gate unsatisfiable
without touching files this arc's scope excluded), and redacting
operator-personal infra references without a mandate risks
over-redaction (the class may be public-acceptable — the operator's
GitHub identity and site footer already name them personally).

## Requirements (operator decides; the loop executes either half)

1. **IN-CLASS**: redact all word-bounded residues across tracked
   files to the operator's generalized vocabulary ("the loop host"),
   then add the class-5 pattern(s) to the lint with pins, and flip the
   lint's kept-class note. The TODO.md/loopd.sh residues are
   orchestrator bookkeeping (children never touch those files).
2. **OUT-CLASS**: one line in the lint's class-5 note (and this
   spec's resolution note) recording the operator's call that the
   machine-codename class is operator-personal infra, public-acceptable,
   and permanently excluded — the exclusion is already pinned
   load-bearing, so this is documentation only.

## Pins

- Either half keeps tests/internal_info_lint.rs green (8/8+) and the
  full suite green; the class-5 note matches the disposition.

## Out of scope

- History scrubbing (unchanged from T249 — operator call);
- the site repo and other repos.

## Resolution (2026-10-06 — loop-executed OUT-CLASS per the either-half mandate)

The machine-codename class (`K7` loop host, `F94` checkout codename;
53 word-bounded lines across 27 tracked files including this lint's own
self-excluded source) is ruled operator-personal infra and
public-acceptable — the operator's public identity and site footer
already name the machines personally, and the operator's own sweep
(47cd0d1) generalized the two prose spots while keeping the residues:
the class is not org info. The exclusion is permanent: no class-5
pattern is added, the kept-class shapes stay pinned passing so the
exclusion remains load-bearing, and the class-5 note records this
ruling. IN-CLASS was rejected on the record: redacting ~48 residues
would rewrite bookkeeping history (TODO.md rows cite the machines as
provenance for landed commits), touch doctrine files (loopd.sh), and
flip two load-bearing pins — churn with no protective value for a
public-acceptable class.
