check: cd "$(git rev-parse --show-toplevel)" && export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-validate-a && touch src/*.rs tests/*.rs && grep -q 'wrap_assert' META-META-SPEC.md && python3 -m unittest scripts/test_wrap_assert.py -v && cargo test --release --test todo_consistency --test eval_outcomes_carry --test eval_state_delta --test loop_spec_doctrine_prune --test internal_info_lint

# T273 — scripts/wrap_assert.py probed-assertion helper (the T270 clause's teeth)

## One concern

The T270 probe-then-assert-delta clause bans hardcoded-constant
expectations in fill/wrap-phase verification-helper assertions — the
assertion must read the property back from the artifact (split-read
non-empty) or compute the delta from the actuals in the same probe
(count == pre + added). As TEXT the clause works as hygiene — but the
helpers it governs are ad-hoc bash/python heredocs re-authored at
every wrap, and the old habit has now re-fired TWICE post-adoption,
each time re-authoring a constant the clause bans:

1. cycle-442 wrap (d1791616594-11, first post-adoption fire): the
   maiden state write's own helper re-authored
   `len(line) > len(key) + 2` — a fixed length threshold that fired on
   the legitimate 1-char `schema: 1` value, across two full-script
   attempts, while every other assertion verified the content correct.
2. cycle-446 wrap (d1791618821-7 / corrected d1791619025-10, second
   post-adoption fire): the ctx-edit-v1 helper hardcoded
   `blocks == 76` — a remembered constant against a live-grown
   transcript (78+ blocks); the guard fired pre-write on CORRECT
   content, the file never touched, corrected one leg later to probed
   counts.

Both fires were self-caught pre-write, zero casualty — the guard
shape doing its job. The carried re-arm — named at the trip-79 eval
(cycle-446 EVALUATION.md §2.2): "a SECOND post-adoption fire files it
(a helper-library or lint surface)" — is this row's filing authority.
The teeth: ONE shared probed-assertion helper the wrap/fill arcs
IMPORT instead of re-authoring assertions inline, plus the doctrine
clause pointing at it.

## Repo context

- New file `scripts/wrap_assert.py` — a small dependency-free python
  module (stdlib only) providing the recurring probed assertion
  shapes the wrap/fill helpers need. Sits beside scripts/eval-delta.sh
  and scripts/decisions-audit.sh (the wrap's other mechanical
  surfaces).
- New file `scripts/test_wrap_assert.py` — python unittest coverage
  for every helper function (the check line runs it; no cargo target
  needed for the helper itself).
- Carrier for the doctrine half: META-META-SPEC.md, inside/adjacent to
  the T270 "Verification-helper assertions" paragraph — the clause
  gains the pointer: wrap/fill-phase verification helpers IMPORT
  scripts/wrap_assert.py for the recurring shapes (or re-derive
  assertions from probed properties per the clause) — inline
  hardcoded-constant assertions are the banned shape; the two
  post-adoption fires cited.
- The recurring shapes the helper covers (from the fires and the
  standing wrap arcs): non-empty-field reads (the T266 nine-field
  check), count-delta assertions (the T265 shell-arithmetic
  discipline's python form), ring-carry byte-identity (the T262/T266
  splice gate), substitution verification (the T262 fill's token
  check), and count-exact enumerations (the T272 clause's jq-count
  assertion).

## Requirements

1. `scripts/wrap_assert.py` (stdlib-only) providing at minimum:
   - `assert_fields_non_empty(path, keys)` — for each `key:` line,
     `line.split(':', 1)[1].strip() != ''` (the T270 split-read
     shape); reports the failing keys.
   - `assert_count_delta(before, after, added)` —
     `after == before + added`, computed from the probed actuals.
   - `assert_byte_carry(prev_lines, new_lines)` — the carried segment
     is byte-identical (the ring-carry gate's shape).
   - `assert_substituted(path, needle, expected_present=True)` — the
     fill substitution's probed verification (needle present/absent).
   - `assert_count_exact(actual, expected, label)` — the
     enumeration-count assertion with the probed actual named in the
     failure message.
   Each function raises AssertionError with a message naming the
   PROBED actuals (never a bare constant mismatch).
2. `scripts/test_wrap_assert.py` — unittest coverage: each function's
   pass path AND fail path (the fail path asserts the AssertionError
   fires and names the probed actuals), including the two fire shapes
   as regression cases (the 1-char `schema: 1` value passes
   `assert_fields_non_empty`; a grown-block count passes
   `assert_count_exact` when the expectation is probed).
3. ONE paragraph in META-META-SPEC.md adjacent to the T270 paragraph,
   carrying the literal token `wrap_assert`, stating the pointer rule
   (import the helper or re-derive from probed properties; inline
   hardcoded constants banned), and citing the two post-adoption fires
   (d1791616594-11; d1791618821-7/d1791619025-10).
4. No other file changes. No TODO.md/LEDGER.md edits by the child.

## Tests

- `python3 -m unittest scripts/test_wrap_assert.py -v` (in the check
  line) — the helper's own coverage.
- The check line's `wrap_assert` grep needle against META-META-SPEC.md.
- `cargo test --release --test todo_consistency --test eval_outcomes_carry --test eval_state_delta --test loop_spec_doctrine_prune --test internal_info_lint`
  (the suites a META-META-SPEC text change can break).

## Acceptance

- The helper + its unittest exist and pass; the doctrine needle
  present exactly once in META-META-SPEC.md; the five cargo suites
  green; no diff outside {scripts/wrap_assert.py,
  scripts/test_wrap_assert.py, META-META-SPEC.md}.

- estimate: ~180 changed lines all-in (helper ~90 + tests ~70 +
  doctrine ~20) + this spec
