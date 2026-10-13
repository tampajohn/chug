# T295 — wrap_assert validate_state facade: one-call state-write gate owning the surface conversions

## Repo context

Doctrine row (scripts/wrap_assert.py + scripts/test_wrap_assert.py + ONE
doctrine sentence in META-META-SPEC.md's T273 paragraph), pri 3, SOLO
(doctrine items never overlap), kimi validation REQUIRED (the loop/spec
doctrine itself is on LOOP-SPEC step 4's REQUIRED list).

The eval-state open-threads carries the **wrap_assert call-signature
slip** census (35 fires at filing) and its parent **python-assertion-slip**
census (105 at filing) — every fire caught PRE-WRITE by the helpers' own
interface defense, zero casualty across the classes' histories. The
cycle-686 wrap produced the classes' first FIVE-fire single-stream
cluster: the parent's 103rd/104th/105th (inline whole-file asserts firing
on structurally correct content) + this class's 34th/35th
(d1791921121-12 / d1791921226-14 — `assert_fields_non_empty(new, keys)`
called with the RAW COMPOSED TEXT where the helper walks path-or-MAPPING
(wrap_assert.py line ~113: `open(source)`) → `OSError: File name too
long`, caught pre-write; then `assert_ring_count(new, 12)` called with
the RAW TEXT where the helper walks path-or-LINES → the SAME OSError one
call later on the sibling helper). One meta-root: call-site surfaces not
swept against their consumers' walks — and the confession shape THREE
times at the same write: the standing correction's own sentence ("the
count/unique helpers take the FULL text's lines") quoted VERBATIM in the
compose text and violated in the same breath. When the in-context
correction does not prevent the fire, the surface is wrong, not the
reader.

The 686 wrap's state bullet NAMES the canonical fix and routes it here:
*"the canonical fix NAMED for trip-140's queue-weighing (a
`wa.validate_state(text)` wrapper owning the surface conversions
internally — read_key_values' mapping for fields, full lines for
count/unique, extracted lines for splice — a helper-API improvement, not
a brittle-awk compose-lint; rows file at evals, never from a wrap)."*
This row executes that naming.

The surface today: a wrap's state-write gate composes ~7 helper calls and
must match each helper's walk semantics by hand —
`assert_fields_non_empty(source, keys)` takes path-or-MAPPING;
`assert_ring_count(path_or_lines, …)` / `assert_ring_unique(…)` take
path-or-LINES with the capacity probed from the ring header's own
`last N` (`_ring_capacity`, never a remembered arg per T288);
`assert_ring_splice(prev_ring_lines, new_ring_lines, …)` takes EXTRACTED
ring lines (`ring_section_lines`); the corpus-tail identity read goes
through the `ring_section_lines` accessor (the 105th fire slipped past it
with a bare whole-file `startswith` scan). Every ad-hoc gate re-derives
that mapping; the cluster is the re-derivation failing.

estimate: ~120 changed lines (the facade + `__all__` export ~45, the
RED-proven test legs ~65, the doctrine sentence ~5, row bookkeeping)

## Requirements

1. **The facade.** `scripts/wrap_assert.py` gains
   `validate_state(text, prev_ring_lines=None, corpus_tail_ids=None)` —
   ONE call owning the full state-write gate surface:

   - parses fields via `read_key_values(text)` and runs
     `assert_fields_non_empty` on the MAPPING (the nine `state_field`
     keys the module already names), never on raw text;
   - splits `text` into lines and runs `assert_ring_count` +
     `assert_ring_unique` on the LINES, the capacity probed from the ring
     header's own `last N` via `_ring_capacity` (never a remembered
     12 — the T288 letter);
   - when `prev_ring_lines` is supplied, extracts the new ring via
     `ring_section_lines` and runs `assert_ring_splice` on the EXTRACTED
     lines;
   - when `corpus_tail_ids` is supplied, asserts the corpus-tail
     identity: the extracted ring ids == the supplied tail ids, the
     mismatch message naming the probed actuals;
   - every failure raises AssertionError naming the PROBED actuals (the
     module's invariant — never a bare constant mismatch);
   - exported in `__all__`; the module docstring's coverage line names
     the facade.

2. **Tests.** `scripts/test_wrap_assert.py` gains RED-proven legs:

   (a) the fire shapes: raw text passed to `assert_fields_non_empty` /
   `assert_ring_count` still errors (the interface defense stays
   intact — the facade does not widen those helpers' accepted shapes),
   while `validate_state` on the SAME text passes a correct fixture;
   (b) a realistic state fixture (schema line, nine `key: value` fields,
   a `## decisions ring (last 12, newest first …)` header + ring lines)
   passes;
   (c) each sub-surface's failure mode fails the facade with a
   probed-actuals message: a dropped/empty field, a duplicated ring line,
   a mis-spliced carry (prev supplied), a corpus-tail mismatch (ids
   supplied);
   (d) the capacity is read from the header: a `last 8` fixture ring
   gates against 8, not a hardcoded 12.

3. **The doctrine sentence.** META-META-SPEC.md's T273 paragraph ("The
   probed-assertion helper (T273, import-don't-re-author)") gains ONE
   sentence naming `validate_state` as the preferred surface for the
   full state-write gate — the facade owns the surface conversions;
   per-helper calls stay for the narrower surfaces (window counts,
   byte-carries, substitutions).

## Acceptance

- `python3 -m unittest scripts/test_wrap_assert.py -v` green (old + new
  legs).
- The facade's first dogfood: the NEXT wrap's state-write gate runs ONE
  `validate_state` call where the cycle-686 gate ran seven call-site
  surfaces.
- **Pin-surface declaration (T285, probed at filing):**
  `grep -rn 'matches("' tests/*.rs` over the T273 paragraph's window
  finds ZERO needles covering it — the three `loopd_*.rs` hits name the
  T273 pair in code COMMENTS (not needles), and
  `tests/loop_spec_doctrine_prune.rs`'s needles target LOOP-SPEC's T261
  region. The doctrine sentence perturbs ZERO pins.

check: cd "$(git rev-parse --show-toplevel)" && export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared && touch src/*.rs tests/*.rs; set -o pipefail; python3 -m unittest scripts/test_wrap_assert.py 2>&1 | tail -5 && cargo test --release --test loop_spec_doctrine_prune --test eval_state_delta --test todo_consistency 2>&1 | tail -8
