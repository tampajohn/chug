#!/usr/bin/env python3
"""test_wrap_assert.py — T273: unittest coverage for scripts/wrap_assert.py.

Every helper gets a PASS path and a FAIL path; each fail path asserts the
AssertionError fires AND names the probed actuals (never a bare constant
mismatch — the T270 clause's reporting requirement). The two post-adoption
fires are pinned as regression cases:

  - d1791616594-11 (the cycle-442 wrap): the 1-char `schema: 1` value the
    banned fixed-length-threshold shape fired on PASSES
    assert_fields_non_empty, because the split-read probes the value back,
    not the line's length.
  - d1791618821-7 / d1791619025-10 (the cycle-446 wrap): the grown-block
    count (78+ blocks against a remembered `== 76`) PASSES
    assert_count_exact when the expectation is probed from the same read,
    and the stale-constant fail names the probed actual.

T288 adds the probe-deriving, section-scoped pair for the class's two NEW
sub-shapes (both pinned as regression legs):

  - d1791684611-3 (the cycle-508 wrap, fire 3): the remembered constant
    rode the helper's imported expected-count ARG — a stale ARG passed
    INTO assert_ring_count cannot override the capacity probed from the
    live header.
  - the cycle-512 wrap's 03:02:56Z leg (fire 4): the probe split the WHOLE
    temp file on `startswith('- d')` (17 = 12 ring lines + 5 open-threads
    bullets) and fired on structurally correct content — the ring count
    now scopes to the `## decisions` ring section (line-anchored per
    T266) and reads 12 on the same bytes.

T289 adds the ring's uniqueness + position-aware splice legs (the
cycle-520 wrap's corrupted rewrite pinned as regression shapes):

  - the cycle-520 shape (12 ring lines / 8 unique decision ids, the tail
    4 byte-duplicates of the head 4): FAILS assert_ring_unique naming the
    duplicated ids and both offsets, while the count-only
    assert_ring_count and the position-free assert_byte_carry still pass
    it (the documented gaps the new teeth close).
  - the well-formed prepend+carry splices (8+4 young-ring growth, and the
    full-capacity rotation that drops the previous ring's TAIL only)
    PASS assert_ring_splice; the vacuous-pass shape (the "carried"
    window a copy of the new batch's own head) fails it positionally.

T295 adds the validate_state facade legs (the cycle-686 five-fire
cluster's canonical fix — ONE call owning the surface conversions a wrap
gate previously hand-matched across ~7 call sites):

  - the d1791921121-12 / d1791921226-14 pair pinned end-to-end: the RAW
    composed text into the path-walking helpers still errors (a str walks
    as a PATH — `OSError: File name too long`, the interface defense
    intact, the facade widens nothing), while validate_state on the SAME
    text passes;
  - the realistic fixture (the nine `state_field` fields, the sections, a
    `last 12` ring of 12) passes ONE-call, with prev_ring_lines +
    corpus_tail_ids riding the same call;
  - each sub-surface's failure mode fails the facade naming probed
    actuals: a dropped / empty field, a byte-duplicated ring line, a
    mis-spliced carry (prev supplied), a corpus-tail mismatch (ids
    supplied);
  - the capacity is read from the header: a `last 8` fixture ring gates
    against 8, not a hardcoded 12.

T298 adds the zero-carry refusal legs (the cycle-1042 wrap's
prepend-12/carry-0 write the facade passed GREEN): a full-ring write
whose batch fills the entire capacity computes keep = 0 and the
carried-window assertion passed VACUOUSLY — the write-time gate now
REFUSES the shape (T192: rotate at most 11 so one prior line always
survives verbatim; the remedy is prepend-11/carry-1), while the
under-capacity append path and the empty-previous-ring `m == 0` path
stay GREEN.

T306 adds the eval-commit probed-equality legs (the cycle-2035 filing
fire's write-time half): the 2031 wrap's state recompose wrote
`eval-commit: 3930452b1f9e0e05a12e93d69a2d6ba95d7cbe47` — the 7-char
prefix TRUE, the suffix memory-composed (the TRUE full hash is
`39304522865f99ca95b6e8b8750e1714e627e7fb`; `git cat-file -t` on the
falsified hash rc-128) — and the facade's field check passed it, the
field gated NON-EMPTY only. The facade gains `expected_eval_commit`:
the CALL SITE probes `git rev-parse <commit>` and passes the probed
full hash in, the facade asserts the parsed field EQUALS it (the
T270/T273 probed-arg shape `prev_ring_lines` / `corpus_tail_ids`
already ride). Legs: a fabricated-suffix eval-commit FAILS naming both
the probed field value and the supplied probed actual; an honest
eval-commit PASSES; without the keyword the leg is INERT (the
fabricated text still passes a keyword-less gate — the boot-time
fail-closed marker check in eval-delta.sh stays the other half).

T307 adds the state-drift first-token legs (the T304 arc's write-side
half, the cycle-2039 filing fire): the 2035 wrap's state FULL recompose
healed the eval-commit field (T306's leg rode green) but composed the
drift field with the healed event's NARRATIVE in the flag position —
the field is non-empty (the facade gated it NON-EMPTY only), and the
2039 boot's delta re-flagged FULL-READ REQUIRED (state-drift-flagged)
with the marker HEALED and reachable. The facade now refuses a drift
field whose FIRST whitespace-token is not exactly `none` — an
INTRINSIC leg, no new keyword. Legs: the prose-after-token shape
(`none (the boot STATE-HIT: …)` — the flag token first, the narrative
AFTER it) PASSES; the narrative-first shape FAILS naming both the
received first token and the full field value.

T309 adds the fill-datum leg coverage (the corridor-figure falsification
class's 4th sighting, the cycle-2240 wrap): fill 8ea1079's committed FINAL
datum block (72 iters / ~1,225.624s wall [-> 12:28:48.056Z]) vs the TRUE
helper record at the stamp (73 / ~1,223.542s [-> 12:28:45.974Z]) — composed
from the PRIOR probe plus a projected delta BEFORE the same leg's
scripts/stream_datum.py run printed the actuals. Legs: the fill text
carrying the helper's exact figures (the corpus's comma-grouped wall
rendering, the uncommaed form, a wrapped marker) PASSES; the eae4d90 shape
FAILS pre-write naming every divergent figure (iters, wall, the end anchor
— the matching start anchor is NOT reported) with the tempdir
byte-unchanged (zero mutation); the PLAIN display parenthetical (it
derives from the wall figure) and the block's other adjudication figures
(ok:false / trim) are OUTSIDE the asserted surface — divergent ones stay
GREEN; a datum-less fill FAILS named-missing (never a silent pass); a
degenerate datum_output (a `none` figure, a missing line, a non-digit
iters) FAILS closed; non-str arguments TypeError naming the accepted
shape; the correction-quote shape (both blocks in one text) stays GREEN —
a FRESH run's stamp-figures cannot equal any earlier probe's block.

T309 adds the fill-datum leg coverage (the corridor-figure falsification
class's 4th sighting, the cycle-2240 wrap): fill 8ea1079's committed FINAL
datum block (72 iters / ~1,225.624s wall [-> 12:28:48.056Z]) vs the TRUE
helper record at the stamp (73 / ~1,223.542s [-> 12:28:45.974Z]) — composed
from the PRIOR probe plus a projected delta BEFORE the same leg's
scripts/stream_datum.py run printed the actuals. Legs: the fill text
carrying the helper's exact figures (the corpus's comma-grouped wall
rendering, the uncommaed form, a wrapped marker) PASSES; the eae4d90 shape
FAILS pre-write naming every divergent figure (iters, wall, the end anchor
— the matching start anchor is NOT reported) with the tempdir
byte-unchanged (zero mutation); the PLAIN display parenthetical (it
derives from the wall figure) and the block's other adjudication figures
(ok:false / trim) are OUTSIDE the asserted surface — divergent ones stay
GREEN; a datum-less fill FAILS named-missing (never a silent pass); a
degenerate datum_output (a `none` figure, a missing line, a non-digit
iters) FAILS closed; non-str arguments TypeError naming the accepted
shape; the correction-quote shape (both blocks in one text) stays GREEN —
a FRESH run's stamp-figures cannot equal any earlier probe's block.

T310 adds the fill-enumeration leg coverage (the T272 false-at-write
class's newest fire, the cycle-2244 wrap fill 5b5e38f): the committed
FINAL datum block reads "ok:false 4 == 4 adjudicated" at its named
2026-10-28T14:03:44Z stamp while the mechanical whole-final enumeration
finds FIVE `ok:false` legs at that stamp — the fifth the 14:03:24.421Z
assert_fill_datum AssertionError (the T309 guard's first production fire,
itself the designed pre-commit catch). The recompose-after-guard-fire
updated the datum figures (the T309 leg forced that) but NOT the
enumeration — a read taken earlier, not re-run before the write. Legs:
the TRUE enumeration figure (5 == 5, the mechanical count) PASSES (also
the bare `ok:false K` head-figure shape, the wrapped rendering, and a
legitimate 0 count); the 5b5e38f falsified figure (4 against the probed
5) FAILS naming BOTH figures with the tempdir byte-unchanged (zero
mutation); a datum-less fill FAILS named-missing (never a silent pass);
a degenerate probed_count (a str, None, a bool, negative) FAILS closed
naming the probed value; non-str text / probed_stamp TypeError naming
the accepted shapes (the pre-split line list is the text slip shape);
the any-block semantics (a stale + TRUE block pair) stays GREEN — the
TRUE block redeems the text, the freshness contract riding the compose
leg's SAME-leg re-probe (the T282 channel half) rather than the datum
leg's stamp-moves argument; RED-provability hand-proved by mutants run
on a copy (the count compare deleted → the falsified leg goes red; the
zero-block fire silent-passed → the named-missing leg goes red), the
module restored byte-identical after each run.

T311 adds the on-disk state-schema leg coverage (the 2515 false-GREEN
class's durable gate, the T309/T310 fill-assert family's third leg): the
cycle-2515 fill a66f604 claimed "state write FACADE-GREEN first attempt"
while `validate_state` on the COMMITTED `.chug/eval-state.md` bytes
raises AssertionError at the FIELDS leg — the claim composed without a
real run over the on-disk bytes. assert_state_on_disk IS that run,
wired into the fill compose leg's pre-commit assert (the SAME leg as
the commit). Legs: the schema-conformant fixture on disk PASSES with and
without expected_eval_commit; the 2515-shape prose fixture
(`last-eval-commit:` substituted, zero schema fields, `## Delta pointer`
/ `## Outcome-id ring` sections only) FAILS from the FIELDS leg naming
the missing keys; a wrong probed hash FAILS naming BOTH the probed field
value and the supplied actual (the T270 reporting requirement); a
nonexistent path FAILS naming the probed path and the OS error (never an
uncaught OSError); a non-str path TypeErrors naming the accepted shape
(the call-signature-slip convention); the on-disk read is a REAL disk
read (a mutate-after-write state fails the leg — the committed bytes,
not a remembered draft, are what the gate runs).

Run (the check line): python3 -m unittest scripts/test_wrap_assert.py -v
"""

import ast
import os
import sys
import tempfile
import unittest

# The check line imports this file as `scripts.test_wrap_assert` (a
# namespace-package path with the repo root on sys.path), so THIS module's
# own directory is not importable from there — insert it explicitly or
# `import wrap_assert` fails. Also makes a direct
# `python3 scripts/test_wrap_assert.py` run work.
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import wrap_assert  # noqa: E402


def _write(tmpdir, name, text):
    path = os.path.join(tmpdir, name)
    with open(path, "w", encoding="utf-8") as f:
        f.write(text)
    return path


class StdlibOnlyTest(unittest.TestCase):
    """The module is dependency-free: zero imports (trivially stdlib-only)."""

    def test_zero_imports(self):
        with open(wrap_assert.__file__, "r", encoding="utf-8") as f:
            tree = ast.parse(f.read())
        imports = [
            node
            for node in ast.walk(tree)
            if isinstance(node, (ast.Import, ast.ImportFrom))
        ]
        self.assertEqual(imports, [], "wrap_assert.py must stay import-free")


class AssertFieldsNonEmptyTest(unittest.TestCase):
    def test_pass_reads_values_back(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = _write(tmp, "state.md", "purpose: carry the ring\nschema: 1\n")
            wrap_assert.assert_fields_non_empty(path, ["purpose", "schema"])

    def test_fail_reports_all_failing_keys_with_probed_actuals(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = _write(tmp, "state.md", "purpose:\nschema: 1\n")
            with self.assertRaises(AssertionError) as ctx:
                wrap_assert.assert_fields_non_empty(path, ["purpose", "schema", "health"])
        msg = str(ctx.exception)
        self.assertIn("2/3 named field(s) failed", msg)  # both fails, one pass
        self.assertIn("empty key 'purpose'", msg)  # the probed split-read ''
        self.assertIn("missing key 'health'", msg)  # the probed absence
        self.assertNotIn("'schema'", msg)  # the passing key is not reported

    def test_regression_1char_schema_value_passes_the_split_read(self):
        # d1791616594-11: the banned shape (`len(line) > len(key) + 2`)
        # fired on exactly this line; the split-read shape passes it.
        with tempfile.TemporaryDirectory() as tmp:
            path = _write(tmp, "state.md", "schema: 1\n")
            wrap_assert.assert_fields_non_empty(path, ["schema"])


# T277: the mapping-input shape — `assert_fields_non_empty(read_key_values(text),
# keys)` raised `expected str ... not dict` three times (trip-81 x2, trip-83 ingress).
class AssertFieldsNonEmptyMappingInputTest(unittest.TestCase):
    def test_pass_mapping_input_used_directly(self):
        # No path involved — a re-open (the pre-T277 shape) would TypeError here.
        fields = wrap_assert.read_key_values("purpose: carry the ring\nschema: 1\n")
        wrap_assert.assert_fields_non_empty(fields, ["purpose", "schema"])

    def test_fail_mapping_input_reports_the_missing_key_with_probed_actuals(self):
        with self.assertRaises(AssertionError) as ctx:
            wrap_assert.assert_fields_non_empty(
                {"purpose": "", "schema": "1"}, ["purpose", "health"]
            )
        msg = str(ctx.exception)
        self.assertIn("2/2 named field(s) failed", msg)  # same one-error shape
        self.assertIn("missing key 'health'", msg)  # the probed absence

    def test_fail_wrong_type_raises_type_error_naming_both_accepted_shapes(self):
        with self.assertRaises(TypeError) as ctx:
            wrap_assert.assert_fields_non_empty(7, ["purpose"])
        msg = str(ctx.exception)
        self.assertIn("PATH", msg)  # accepted shape 1 named
        self.assertIn("MAPPING", msg)  # accepted shape 2 named
        self.assertIn("type int", msg)  # the probed wrong type


class AssertCountDeltaTest(unittest.TestCase):
    def test_pass_computes_from_probed_actuals(self):
        wrap_assert.assert_count_delta(before=76, after=78, added=2)

    def test_fail_names_all_three_probed_actuals_and_the_arithmetic(self):
        with self.assertRaises(AssertionError) as ctx:
            wrap_assert.assert_count_delta(before=76, after=79, added=2)
        msg = str(ctx.exception)
        self.assertIn("after=79", msg)
        self.assertIn("before=76", msg)
        self.assertIn("added=2", msg)
        self.assertIn("79 != 76 + 2", msg)


class AssertByteCarryTest(unittest.TestCase):
    def test_pass_byte_identical_carry_at_any_offset(self):
        prev = ["## decisions", "d1 | older", "d2 | oldest"]
        new = ["d0 | newest", "d-1 | newer still"] + prev  # head prepends
        wrap_assert.assert_byte_carry(prev, new)

    def test_pass_identical_lines_pass(self):
        lines = ["a", "b", "c"]
        wrap_assert.assert_byte_carry(lines, list(lines))

    def test_fail_names_the_mutated_carry_line_and_both_probed_reprs(self):
        prev = ["d1 | older", "d2 | oldest"]
        new = ["d0 | newest", "d1 | older (rewritten)", "d2 | oldest"]
        with self.assertRaises(AssertionError) as ctx:
            wrap_assert.assert_byte_carry(prev, new)
        msg = str(ctx.exception)
        self.assertIn("carry line 0", msg)  # which carried line broke
        self.assertIn("'d1 | older'", msg)  # probed expected repr
        self.assertIn("'d0 | newest'", msg)  # probed candidate at the offset

    def test_fail_names_a_dropped_carry_line(self):
        with self.assertRaises(AssertionError) as ctx:
            wrap_assert.assert_byte_carry(["a", "b"], ["a", "c"])
        msg = str(ctx.exception)
        self.assertIn("carry line 1", msg)
        self.assertIn("'b'", msg)
        self.assertIn("'c'", msg)

    def test_fail_reordered_lines(self):
        """All carry lines PRESENT but REORDERED still raises (the m6 pin).

        The d1791624468-8 m6 mutant relaxed the positional scan to unordered
        membership — every line present, so it survived the suite. Under the
        ordered search the swap still fires: 'a' matches late (offset 1),
        so the scan for 'b' starts at offset 2, past the end.
        """
        with self.assertRaises(AssertionError) as ctx:
            wrap_assert.assert_byte_carry(["a", "b"], ["b", "a"])
        msg = str(ctx.exception)
        self.assertIn("carry line 1", msg)  # the line the ordered scan can't place
        self.assertIn("'b'", msg)  # probed expected repr
        self.assertIn("None", msg)  # probed candidate at offset 2: past the end


class AssertSubstitutedTest(unittest.TestCase):
    def test_pass_present_and_absent_paths(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = _write(tmp, "entry.md", "WRAP_HASH 22ced35 + PROBE_RESULT `60 0`\n")
            wrap_assert.assert_substituted(path, "WRAP_HASH")
            wrap_assert.assert_substituted(path, "PLACEHOLDER", expected_present=False)

    def test_fail_present_names_the_probed_zero_occurrences(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = _write(tmp, "entry.md", "unsubstituted body\n")
            with self.assertRaises(AssertionError) as ctx:
                wrap_assert.assert_substituted(path, "WRAP_HASH")
        msg = str(ctx.exception)
        self.assertIn("'WRAP_HASH'", msg)
        self.assertIn("expected PRESENT", msg)
        self.assertIn("found 0 occurrences", msg)

    def test_fail_absent_names_the_probed_occurrence_count(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = _write(tmp, "entry.md", "TOKEN x\nTOKEN y\n")
            with self.assertRaises(AssertionError) as ctx:
                wrap_assert.assert_substituted(path, "TOKEN", expected_present=False)
        msg = str(ctx.exception)
        self.assertIn("expected ABSENT", msg)
        self.assertIn("found 2 occurrence(s)", msg)


class AssertCountExactTest(unittest.TestCase):
    def test_pass_when_both_sides_are_probed(self):
        blocks = 78  # probed: the live transcript actually holds 78
        wrap_assert.assert_count_exact(blocks, blocks, "ctx-edit blocks")

    def test_fail_names_the_probed_actual_against_the_stale_constant(self):
        with self.assertRaises(AssertionError) as ctx:
            wrap_assert.assert_count_exact(78, 76, "ctx-edit blocks")
        msg = str(ctx.exception)
        self.assertIn("ctx-edit blocks", msg)  # the named count
        self.assertIn("probed actual 78", msg)  # the live read
        self.assertIn("expected 76", msg)  # the stale constant

    def test_regression_grown_block_count_passes_when_probed(self):
        # d1791618821-7 / d1791619025-10: the transcript grew 76 -> 78+;
        # the banned shape hardcoded `blocks == 76` and fired pre-write on
        # CORRECT content. The probed shape derives the expectation from
        # the same read and passes on the grown count.
        probed = 79  # what the live transcript holds at the wrap probe
        wrap_assert.assert_count_exact(probed, probed, "ctx-edit blocks")


class AssertWindowCountTest(unittest.TestCase):
    """T278: the window-scoped substitution count (the fill-phase shape)."""

    def test_pass_section_window_counts_within_it(self):
        # The EVALUATION.md shape: window = the `### Cycle N` section
        # through the next `### ` heading (exclusive). The carried 469
        # entry quotes a DIFFERENT token by name; the counted token exists
        # only in-window, so the window count and a file-wide count agree.
        with tempfile.TemporaryDirectory() as tmp:
            path = _write(
                tmp,
                "eval.md",
                "### Cycle 470\n"
                "- outcome: the fill landed — WRAP_TIME 13:37 UTC + "
                "PROBE_RESULT `60 0`\n"
                "### Cycle 469\n"
                "- carried entry: the fill-instruction prose quotes "
                "WRAP_HASH by name\n",
            )
            wrap_assert.assert_window_count(
                path, "PROBE_RESULT", "### Cycle 470", 1, end="### "
            )

    def test_fail_names_actual_expected_placeholder_anchor_and_span(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = _write(
                tmp,
                "eval.md",
                "### Cycle 470\n"
                "- outcome: PROBE_RESULT `60 0` first-try\n"
                "- second leg re-read PROBE_RESULT `60 0`\n"
                "### Cycle 469\n",
            )
            with self.assertRaises(AssertionError) as ctx:
                wrap_assert.assert_window_count(
                    path, "PROBE_RESULT", "### Cycle 470", 1, end="### "
                )
        msg = str(ctx.exception)
        self.assertIn("probed placeholder 'PROBE_RESULT'", msg)
        self.assertIn("count 2", msg)  # the probed actual
        self.assertIn("expected 1", msg)
        self.assertIn("'### Cycle 470'", msg)  # the anchor
        self.assertIn("lines 1-3", msg)  # the window's line span (end anchor at line 4)

    def test_fail_start_anchor_miss_names_the_missing_anchor(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = _write(
                tmp,
                "eval.md",
                "### Cycle 470\n- outcome: PROBE_RESULT `60 0`\n",
            )
            with self.assertRaises(AssertionError) as ctx:
                wrap_assert.assert_window_count(
                    path, "PROBE_RESULT", "### Cycle 999", 1
                )
        msg = str(ctx.exception)
        self.assertIn("'### Cycle 999'", msg)  # the missing anchor named
        self.assertIn("matches NO line", msg)  # the guard, not a miscount

    def test_fail_end_anchor_miss_names_the_missing_end_anchor(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = _write(
                tmp,
                "eval.md",
                "### Cycle 470\n- outcome: PROBE_RESULT `60 0`\n",
            )
            with self.assertRaises(AssertionError) as ctx:
                wrap_assert.assert_window_count(
                    path, "PROBE_RESULT", "### Cycle 470", 1, end="### Cycle 999"
                )
        msg = str(ctx.exception)
        self.assertIn("'### Cycle 999'", msg)  # the missing END anchor named
        self.assertIn("end anchor", msg)  # which anchor missed

    def test_regression_prose_quote_outside_the_window_never_miscounts(self):
        """The T278 firing shape (trip-85 fill, d1791646071-2), pinned.

        The state file holds WRAP_HASH twice — the pacing-line placeholder
        (the fill target) and an open-threads bullet's PROSE QUOTE of the
        token name — and the hand-rolled heredoc asserted a FILE-WIDE
        count == 1, firing twice pre-write on structurally correct
        content. The window-scoped count reads 1 in both geometries while
        the file-wide count reads 2; the end anchor's own line is excluded.
        """
        # Geometry 1: the prose quote BEFORE the anchor line — the 4-arg
        # call (window = the pacing line through EOF) already excludes it.
        with tempfile.TemporaryDirectory() as tmp:
            path = _write(
                tmp,
                "state.md",
                "schema: 1\n"
                "eval-commit: 49a95e5\n"
                "open-threads: the carried bullet quotes the tokens by "
                "name — the fill commit's substitutions (WRAP_TIME/"
                "WRAP_HASH/PROBE_RESULT) run window-scoped count-asserts\n"
                "pacing-streak: TRUE 0 (reset 2->0): WRAP_HASH + "
                "PROBE_RESULT `60 0`\n",
            )
            wrap_assert.assert_window_count(path, "WRAP_HASH", "pacing-streak:", 1)
            with open(path, "r", encoding="utf-8") as f:
                self.assertEqual(f.read().count("WRAP_HASH"), 2)  # the misfire
        # Geometry 2: the REAL trip-85 shape (the grep probe read lines 9
        # and 34) — the prose quote AFTER the pacing line, so the window
        # needs the end anchor to scope the single pacing line.
        with tempfile.TemporaryDirectory() as tmp:
            path = _write(
                tmp,
                "state.md",
                "schema: 1\n"
                "eval-commit: 49a95e5\n"
                "health: gates GREEN at the wrap\n"
                "open-threads: 3 open — see the section below\n"
                "pacing-streak: TRUE 0 (reset 2->0): WRAP_HASH + "
                "PROBE_RESULT `60 0`\n"
                "purpose: Cold-cycle entry reads this first\n"
                "\n"
                "## open-threads\n"
                "- wrap-notes placeholder backfill miss: the fill commit's "
                "substitutions (WRAP_TIME/WRAP_HASH/PROBE_RESULT) run "
                "window-scoped count-asserts pre-write\n",
            )
            wrap_assert.assert_window_count(
                path, "WRAP_HASH", "pacing-streak:", 1, end="purpose:"
            )
            with open(path, "r", encoding="utf-8") as f:
                self.assertEqual(f.read().count("WRAP_HASH"), 2)  # the misfire
        # Geometry 3: the end anchor's OWN line is exclusive — a token on
        # it is never counted (window = lines 1-1, the anchor line excluded).
        with tempfile.TemporaryDirectory() as tmp:
            path = _write(
                tmp,
                "state.md",
                "pacing-streak: TRUE 0 (the fill target line)\n"
                "purpose: Cold-cycle entry — the prose quotes WRAP_HASH\n",
            )
            wrap_assert.assert_window_count(
                path, "WRAP_HASH", "pacing-streak:", 0, end="purpose:"
            )


class AssertWindowCleanTest(unittest.TestCase):
    """T278: the window-scoped bare-token scan (the fill-phase shape)."""

    def test_pass_tokens_only_outside_the_window(self):
        # Post-fill: the window is fully substituted; the carried 469
        # entry's fill-instruction prose quotes the tokens by name
        # OUTSIDE the window and is never scanned.
        with tempfile.TemporaryDirectory() as tmp:
            path = _write(
                tmp,
                "eval.md",
                "### Cycle 470\n"
                "- outcome: the fill landed — 13:37–13:59 UTC, 49a95e5, "
                "`60 0`\n"
                "### Cycle 469\n"
                "- carried fill instructions quote WRAP_TIME / WRAP_HASH "
                "by name\n",
            )
            wrap_assert.assert_window_clean(
                path,
                ["WRAP_TIME", "WRAP_HASH", "PROBE_RESULT"],
                "### Cycle 470",
                end="### ",
            )

    def test_fail_names_the_bare_token_with_its_window_line(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = _write(
                tmp,
                "eval.md",
                "### Cycle 470\n"
                "- outcome: WRAP_TIME still bare in this line\n"
                "### Cycle 469\n",
            )
            with self.assertRaises(AssertionError) as ctx:
                wrap_assert.assert_window_clean(
                    path, ["WRAP_TIME", "WRAP_HASH"], "### Cycle 470", end="### "
                )
        msg = str(ctx.exception)
        self.assertIn("'WRAP_TIME'", msg)  # the found token named
        self.assertIn("window line 2", msg)  # window-relative line number
        self.assertIn("file line 2", msg)  # and the file line for the fix
        self.assertNotIn("'WRAP_HASH'", msg)  # the absent token not reported

    def test_fail_names_every_found_token_in_one_error(self):
        # The all-failures-in-one-error convention: all three bare tokens
        # named in ONE AssertionError with their window-relative lines.
        with tempfile.TemporaryDirectory() as tmp:
            path = _write(
                tmp,
                "eval.md",
                "### Cycle 470\n"
                "- WRAP_TIME and PROBE_RESULT both still bare\n"
                "- WRAP_HASH survives on a later window line\n"
                "### Cycle 469\n",
            )
            with self.assertRaises(AssertionError) as ctx:
                wrap_assert.assert_window_clean(
                    path,
                    ["WRAP_TIME", "WRAP_HASH", "PROBE_RESULT"],
                    "### Cycle 470",
                    end="### ",
                )
        msg = str(ctx.exception)
        self.assertIn("3 bare token(s)", msg)  # every finding, one error
        self.assertIn("'WRAP_TIME' at window line 2", msg)
        self.assertIn("'PROBE_RESULT' at window line 2", msg)
        self.assertIn("'WRAP_HASH' at window line 3", msg)


# T288 fixtures. The ring header is the format's pinned heading text (the
# live file carries a longer suffix; the anchor is the LINE-ANCHORED prefix).
RING_HEADER = (
    "## decisions ring (last 12, newest first — newest batch at the head)"
)

# The fire-4 open-threads bullets — all five start `- d`, which is exactly
# why the OLD whole-file `startswith('- d')` probe miscounted them as ring
# lines (17 = 12 ring lines + 5 bullets, the cycle-512 wrap's 03:02:56Z leg).
FIRE4_OPEN_THREADS_BULLETS = [
    "- decision_log-arg-slip census 4 + mis-class sub-shape 3 (UNCHANGED — "
    "the routing record classed CORRECTLY)",
    "- describe-count-slip census 2 (UNCHANGED — the count probe names the "
    "probed actual)",
    "- duplicate-subject backfill first-sighting (counted — subjects "
    "tail-probed first)",
    "- digest-backstop tail leg census 3 (UNCHANGED — the tail identified "
    "by jq on the rotated stream)",
    "- decision-census re-arm (a NEW fire of the class re-fires the census)",
]


def _ring_entries(count):
    return [
        f"- d1791683736-{i}: eval-routing trip 96 binds (kimi) — the probed "
        f"decision body {i}"
        for i in range(count)
    ]


def _fire4_state(ring_count=12, bullet_count=5, header=RING_HEADER, tail_sections=""):
    """A structurally-correct state file in the real section order: the
    fields block, then `## open-threads`, then the `## decisions` ring
    (running to EOF), optionally followed by more sections (tail_sections
    appended verbatim). header=None drops the ring header (the guard's
    genuinely-wrong-input shape)."""
    lines = [
        "schema: 1",
        "eval-commit: 4426e9b",
        "health: gates GREEN at the wrap",
        "open-threads: 5 open — see the section below",
        "pacing-streak: TRUE 0",
        "purpose: Cold-cycle entry reads this first",
        "",
        "## open-threads (censuses the next eval updates; each names its trigger)",
    ]
    lines += FIRE4_OPEN_THREADS_BULLETS[:bullet_count]
    if header is not None:
        lines.append(header)
        lines += _ring_entries(ring_count)
    if tail_sections:
        lines += tail_sections.splitlines()
    return "\n".join(lines) + "\n"


class RingSectionLinesTest(unittest.TestCase):
    """T288: the ring-section extraction primitive (the count's scope)."""

    def test_pass_extracts_the_body_between_the_header_and_the_next_section(self):
        text = (
            "schema: 1\n"
            "\n"
            "## open-threads (censuses the next eval updates)\n"
            "- decision_log-arg-slip census 4 (a `- d` bullet OUTSIDE the ring)\n"
            "\n"
            "## decisions ring (last 12, newest first — newest batch at the head)\n"
            "- d1791683736-1: eval-routing trip 96 binds (kimi)\n"
            "- d1791684607-2: eval-triage T287 filed\n"
            "\n"
            "## health (one line per load-bearing check)\n"
            "- gates: green\n"
        )
        self.assertEqual(
            wrap_assert.ring_section_lines(text),
            [
                "- d1791683736-1: eval-routing trip 96 binds (kimi)",
                "- d1791684607-2: eval-triage T287 filed",
                "",  # a blank line is a body line — not a `- d` line
            ],
        )

    def test_pass_lines_input_used_directly(self):
        lines = [RING_HEADER, "- d1: a", "- d2: b"]
        self.assertEqual(
            wrap_assert.ring_section_lines(lines), ["- d1: a", "- d2: b"]
        )

    def test_pass_ring_at_eof_runs_to_eof(self):
        text = (
            "schema: 1\n"
            "## decisions ring (last 12, newest first)\n"
            "- d1: a\n"
            "- d2: b\n"
        )
        self.assertEqual(wrap_assert.ring_section_lines(text), ["- d1: a", "- d2: b"])

    def test_pass_prose_mention_of_the_heading_never_anchors(self):
        # The T266 line-anchoring discipline: the anchor is `^## decisions
        # ring` — a bare substring mention inside another field's prose is
        # not an anchor (the cycle-305 splice shape). A substring anchor
        # would extract the `- d1791-orphan` line too.
        text = (
            "purpose: the bullet quotes ## decisions ring (last 12) in prose\n"
            "- d1791-orphan: NOT a ring line (no ring section here)\n"
            "## decisions ring (last 12, newest first)\n"
            "- d1791683736-1: the only real entry\n"
        )
        self.assertEqual(
            wrap_assert.ring_section_lines(text),
            ["- d1791683736-1: the only real entry"],
        )

    def test_fail_missing_header_raises_the_line_anchored_guard(self):
        with self.assertRaises(AssertionError) as ctx:
            wrap_assert.ring_section_lines("schema: 1\nopen-threads: none\n")
        msg = str(ctx.exception)
        self.assertIn("'## decisions ring'", msg)  # the missing anchor named
        self.assertIn("matches NO line start", msg)  # the guard, not a miscount


class AssertRingCountTest(unittest.TestCase):
    """T288: the section-scoped, probe-derived ring count (fires 3 + 4)."""

    def test_pass_fire4_fixture_section_scoped(self):
        # The fire-4 fixture: 12 ring `- d` lines AND 5 open-threads `- d`
        # bullets. The OLD whole-file shape reads 17 and mis-fires; the
        # section-scoped assertion reads the ring's 12 and passes.
        with tempfile.TemporaryDirectory() as tmp:
            path = _write(tmp, "state.md", _fire4_state())
            with open(path, "r", encoding="utf-8") as f:
                whole_file = sum(
                    1 for line in f.read().splitlines() if line.startswith("- d")
                )
            self.assertEqual(whole_file, 17)  # 12 ring + 5 bullets, the mis-read
            wrap_assert.assert_ring_count(path)

    def test_pass_lines_input_and_eof_ring(self):
        wrap_assert.assert_ring_count([RING_HEADER] + _ring_entries(12))

    def test_pass_dash_d_line_in_a_later_section_never_counts(self):
        # The next-`## `-header boundary: a `- d`-starting bullet in a
        # section AFTER the ring is outside the section (the whole-file
        # shape would read 18 here and mis-fire).
        tail = (
            "## wrap-notes (the next wrap's input)\n"
            "- d-scale prose quote: the ring holds the last 12 — NOT a ring line\n"
        )
        with tempfile.TemporaryDirectory() as tmp:
            path = _write(tmp, "state.md", _fire4_state(tail_sections=tail))
            wrap_assert.assert_ring_count(path)

    def test_remembered_arg_cannot_override_the_probed_capacity(self):
        # Fire 3 (d1791684611-3) pinned: the remembered constant rode the
        # helper's imported expected-count ARG. The capacity is PROBED from
        # the live header, so a stale ARG (6, the fire-3 value) cannot
        # mis-fire the gate on the correct 12-ring artifact.
        with tempfile.TemporaryDirectory() as tmp:
            path = _write(tmp, "state.md", _fire4_state())
            wrap_assert.assert_ring_count(path, expected_capacity=6)

    def test_capacity_probed_from_the_header_not_the_default(self):
        # The expectation derives from the live artifact: a header reading
        # `last 10` with a 10-line ring passes — under the old
        # remembered-constant shape (== 12) the same artifact mis-fires.
        with tempfile.TemporaryDirectory() as tmp:
            path = _write(
                tmp,
                "state.md",
                _fire4_state(
                    header=(
                        "## decisions ring (last 10, newest first — "
                        "newest batch at the head)"
                    ),
                    ring_count=10,
                ),
            )
            wrap_assert.assert_ring_count(path)

    def test_fail_over_capacity_13_ring_lines(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = _write(tmp, "state.md", _fire4_state(ring_count=13))
            with self.assertRaises(AssertionError) as ctx:
                wrap_assert.assert_ring_count(path)
        msg = str(ctx.exception)
        self.assertIn("count 13", msg)  # the probed actual
        self.assertIn("capacity 12", msg)
        self.assertIn("probed from the header", msg)  # the expectation's provenance

    def test_fail_under_capacity_11_ring_lines(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = _write(tmp, "state.md", _fire4_state(ring_count=11))
            with self.assertRaises(AssertionError) as ctx:
                wrap_assert.assert_ring_count(path)
        msg = str(ctx.exception)
        self.assertIn("count 11", msg)  # the probed actual
        self.assertIn("capacity 12", msg)

    def test_fail_names_the_whole_file_misfire_shape(self):
        # Fire 4 pinned end-to-end: the OLD probe (whole-file
        # `startswith('- d')` = 17) against the remembered 12 fires via
        # assert_count_exact naming the probed actual; the SAME bytes pass
        # section-scoped (test_pass_fire4_fixture_section_scoped).
        with tempfile.TemporaryDirectory() as tmp:
            path = _write(tmp, "state.md", _fire4_state())
            with open(path, "r", encoding="utf-8") as f:
                whole_file = sum(
                    1 for line in f.read().splitlines() if line.startswith("- d")
                )
            with self.assertRaises(AssertionError) as ctx:
                wrap_assert.assert_count_exact(
                    whole_file, 12, "ring lines in the temp file"
                )
        msg = str(ctx.exception)
        self.assertIn("ring lines in the temp file", msg)  # the fire's label
        self.assertIn("probed actual 17", msg)  # the unscoped mis-read
        self.assertIn("expected 12", msg)  # the remembered constant

    def test_fail_header_without_annotation_uses_the_named_fallback(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = _write(
                tmp,
                "state.md",
                _fire4_state(header="## decisions ring (newest first)"),
            )
            wrap_assert.assert_ring_count(path)  # the schema-default fallback 12
            path13 = _write(
                tmp,
                "state13.md",
                _fire4_state(
                    header="## decisions ring (newest first)", ring_count=13
                ),
            )
            with self.assertRaises(AssertionError) as ctx:
                wrap_assert.assert_ring_count(path13)
        msg = str(ctx.exception)
        self.assertIn("expected_capacity fallback arg 12", msg)  # provenance named
        self.assertIn("probed no `last N`", msg)

    def test_fail_missing_ring_header_raises_the_guard(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = _write(tmp, "state.md", _fire4_state(header=None))
            with self.assertRaises(AssertionError) as ctx:
                wrap_assert.assert_ring_count(path)
        msg = str(ctx.exception)
        self.assertIn("'## decisions ring'", msg)  # the missing anchor named
        self.assertIn("matches NO line start", msg)  # the guard, not a miscount

    def test_fail_wrong_type_raises_type_error_naming_both_shapes(self):
        with self.assertRaises(TypeError) as ctx:
            wrap_assert.assert_ring_count(7)
        msg = str(ctx.exception)
        self.assertIn("PATH", msg)  # accepted shape 1 named
        self.assertIn("LINE LIST", msg)  # accepted shape 2 named
        self.assertIn("type int", msg)  # the probed wrong type


# T289 fixtures: the cycle-520 wrap's corrupted ring shape — 12 ring lines,
# 8 unique decision ids, the tail 4 byte-duplicates of the head 4 (the new
# batch's own head written twice into the tail). The batch/carried ids are
# the corrupted ring's own 8 unique decision ids; the "older" lines are what
# the previous ring's tail held before rotation.
CYCLE520_BATCH_IDS = [
    "d1791696920-1",
    "d1791697568-2",
    "d1791697568-3",
    "d1791697575-4",
]
CYCLE520_CARRIED_IDS = [
    "d1791697575-5",
    "d1791697585-6",
    "d1791698000-7",
    "d1791698004-8",
]


def _ring_line(ring_id, tag):
    return f"- {ring_id}: eval-triage T289 {tag} (a probed ring line)"


def _batch_of(count):
    """A fresh new batch of `count` ring lines (date prefix disjoint from
    the cycle-520 fixtures' carried ids)."""
    return [
        _ring_line(f"d1791699100-{i}", f"new batch {i}")
        for i in range(1, count + 1)
    ]


def _cycle520_batch():
    return [
        _ring_line(ring_id, f"new batch {i}")
        for i, ring_id in enumerate(CYCLE520_BATCH_IDS)
    ]


def _cycle520_carried():
    return [
        _ring_line(ring_id, f"carried {i}")
        for i, ring_id in enumerate(CYCLE520_CARRIED_IDS)
    ]


def _cycle520_prev_ring():
    """The previous ring (12 lines): the 4 lines that survive the corruption
    as its middle block + 8 older lines a legal rotation drops from the
    TAIL."""
    older = [_ring_line(f"d1791683736-{i}", f"older {i}") for i in range(9, 17)]
    return _cycle520_carried() + older


def _cycle520_corrupted_ring():
    """The cycle-520 composed ring: 4 new + 4 carried + 4 byte-duplicates of
    the new batch's own head — 12 lines, 8 unique decision ids."""
    batch = _cycle520_batch()
    return batch + _cycle520_carried() + batch


class AssertRingUniqueTest(unittest.TestCase):
    """T289: the ring's uniqueness contract (the cycle-520 corruption)."""

    def test_fail_cycle520_shape_names_the_duplicated_ids_and_both_offsets(self):
        # The cycle-520 shape pinned: pre-T289 this helper did not exist and
        # every existing guard passed the corruption — pinned here with it
        # (the count gate reads 12 either way).
        lines = [RING_HEADER] + _cycle520_corrupted_ring()
        wrap_assert.assert_ring_count(lines)  # the count-only gate: vacuous
        with self.assertRaises(AssertionError) as ctx:
            wrap_assert.assert_ring_unique(lines)
        msg = str(ctx.exception)
        self.assertIn("8 duplicated ring element(s)", msg)  # 4 lines + 4 ids
        for ring_id in CYCLE520_BATCH_IDS:  # every duplicated id named
            self.assertIn(f"duplicated decision id '{ring_id}'", msg)
        for ring_id in CYCLE520_CARRIED_IDS:  # the carried ids are unique
            self.assertNotIn(f"duplicated decision id '{ring_id}'", msg)
        self.assertIn("ring lines 1 and 9", msg)  # BOTH offsets, ring-relative
        self.assertIn("file lines 2 and 10", msg)  # and file-anchored

    def test_fail_via_path_input_names_the_probed_source(self):
        with tempfile.TemporaryDirectory() as tmp:
            text = "\n".join([RING_HEADER] + _cycle520_corrupted_ring()) + "\n"
            path = _write(tmp, "state.md", text)
            with self.assertRaises(AssertionError) as ctx:
                wrap_assert.assert_ring_unique(path)
        msg = str(ctx.exception)
        self.assertIn("state.md'", msg)  # the probed source named (repr'd path)
        self.assertIn("duplicated decision id 'd1791697575-4'", msg)

    def test_fail_prose_quoted_header_never_anchors(self):
        # The T266 line-anchoring discipline carries over: a bare substring
        # mention of the ring header in prose is not the ring section (a
        # substring anchor would extract the bullet and pass vacuously).
        lines = [
            "purpose: prose quotes ## decisions ring (last 12) here",
            "- d1791699999-1: an open-threads bullet BEFORE any ring section",
        ]
        with self.assertRaises(AssertionError) as ctx:
            wrap_assert.assert_ring_unique(lines)
        msg = str(ctx.exception)
        self.assertIn("'## decisions ring'", msg)  # the missing anchor named
        self.assertIn("matches NO line start", msg)

    def test_pass_well_formed_ring_is_unique(self):
        # (b) the well-formed 12-line ring: 8 prepended + 4 carried, 12
        # unique ids — the same shape the corrupted fixture fails on.
        composed = _batch_of(8) + _cycle520_carried()
        wrap_assert.assert_ring_unique([RING_HEADER] + composed)

    def test_fail_wrong_type_raises_type_error_naming_both_shapes(self):
        with self.assertRaises(TypeError) as ctx:
            wrap_assert.assert_ring_unique(7)
        msg = str(ctx.exception)
        self.assertIn("PATH", msg)  # accepted shape 1 named
        self.assertIn("LINE LIST", msg)  # accepted shape 2 named
        self.assertIn("type int", msg)  # the probed wrong type


class AssertRingSpliceTest(unittest.TestCase):
    """T289: the position-aware ring-splice gate (prepend + carry; rotation
    drops from the previous ring's TAIL only)."""

    def test_pass_well_formed_12_line_ring_8_prepended_4_carried(self):
        # (b) 8 prepended + 4 carried from a previous 4-line ring; the
        # prepend count PROBED from the batch the caller composed.
        prev = _cycle520_carried()  # the previous ring: 4 lines
        batch = _batch_of(8)
        composed = batch + prev
        prepended = len(batch)  # probed, never a remembered constant
        self.assertEqual(len(composed), 12)
        wrap_assert.assert_ring_splice(prev, composed, prepended)
        wrap_assert.assert_ring_unique([RING_HEADER] + composed)

    def test_pass_full_capacity_rotation_drops_the_prev_tail_only(self):
        # (d) a legal prepend + tail-rotation splice at full capacity:
        # previous ring 12, batch 4, keep 8 — prev[8:] (the TAIL) drops,
        # prev[0:8] carries POSITION-ALIGNED at ring offsets 4-11.
        prev = _cycle520_prev_ring()
        self.assertEqual(len(prev), 12)
        batch = _cycle520_batch()
        wrap_assert.assert_ring_splice(prev, batch + prev[:8], len(batch))

    def test_fail_vacuous_pass_shape_byte_carry_passes_splice_fails(self):
        # (c) the cycle-520 gap, pinned end-to-end: the landing call verified
        # the BATCH's lines appear in the composed ring — the position-free
        # subsequence check the duplicated tail trivially satisfies — while
        # the position-aware gate fires at the first misaligned carried
        # position (ring offset 8: the previous ring's 5th line replaced by
        # a copy of the batch's own head).
        prev = _cycle520_prev_ring()
        batch = _cycle520_batch()
        composed = batch + _cycle520_carried() + batch
        wrap_assert.assert_byte_carry(batch, composed)  # the OLD gate: vacuous
        with self.assertRaises(AssertionError) as ctx:
            wrap_assert.assert_ring_splice(prev, composed, len(batch))
        msg = str(ctx.exception)
        self.assertIn("carried position 4", msg)
        self.assertIn("ring offset 8", msg)
        self.assertIn(repr(prev[4]), msg)  # probed expected repr
        self.assertIn(repr(batch[0]), msg)  # the batch-copy sitting there

    def test_fail_mutated_carried_line_names_both_reprs(self):
        prev = _cycle520_prev_ring()
        batch = _cycle520_batch()
        composed = batch + prev[:8]
        composed[5] = composed[5] + " (rewritten)"  # a byte-mutation
        with self.assertRaises(AssertionError) as ctx:
            wrap_assert.assert_ring_splice(prev, composed, len(batch))
        msg = str(ctx.exception)
        self.assertIn("carried position 1", msg)
        self.assertIn(repr(prev[1]), msg)
        self.assertIn("rewritten", msg)

    def test_fail_reordered_carry_fires_positionally(self):
        # Element-wise, never a subsequence: a swapped carried PAIR still
        # fires (every line present, the ORDER broken).
        prev = _cycle520_prev_ring()
        batch = _cycle520_batch()
        carried = prev[:8]
        carried[0], carried[1] = carried[1], carried[0]
        with self.assertRaises(AssertionError) as ctx:
            wrap_assert.assert_ring_splice(prev, batch + carried, len(batch))
        msg = str(ctx.exception)
        self.assertIn("carried position 0", msg)
        self.assertIn(repr(prev[0]), msg)
        self.assertIn(repr(prev[1]), msg)  # the swapped-in line probed there

    def test_fail_dropped_carry_names_the_splice_arithmetic(self):
        # The summarization shape: 11 composed lines where the splice
        # arithmetic demands 12 (one carried line dropped).
        prev = _cycle520_prev_ring()
        batch = _cycle520_batch()
        with self.assertRaises(AssertionError) as ctx:
            wrap_assert.assert_ring_splice(prev, batch + prev[:7], len(batch))
        msg = str(ctx.exception)
        self.assertIn("probed 11 ring line(s)", msg)  # the composed length
        self.assertIn("expected 4 + 8", msg)  # the splice arithmetic
        self.assertIn("previous ring 12 line(s)", msg)

    def test_fail_wrong_type_prepended_count_names_the_probed_arg(self):
        prev = _cycle520_prev_ring()
        batch = _cycle520_batch()
        with self.assertRaises(TypeError) as ctx:
            wrap_assert.assert_ring_splice(prev, batch + prev[:8], "4")
        msg = str(ctx.exception)
        self.assertIn("non-negative int", msg)
        self.assertIn("probed prepended_count '4'", msg)


class AssertRingSpliceZeroCarryTest(unittest.TestCase):
    """T298: the zero-carry full-ring write is REFUSED (the cycle-1042 gap).

    When the previous ring is non-empty and the composed ring is at
    capacity, keep = max(0, min(capacity - prepended, len(prev))) computes
    0 whenever the batch fills the whole ring — and the carried-window
    assertion passed VACUOUSLY (0 expected, 0 actual, nothing to compare)
    while T192's rule caps the rotation at capacity-1 so one prior line
    always survives verbatim. The refusal mirrors eval-delta.sh's post-hoc
    `k == maxd && j == 0` leg at write time; below capacity and on an
    empty previous ring the GREEN semantics are unchanged."""

    def test_fail_zero_carry_full_ring_refused_names_t192_and_arithmetic(self):
        # (a) 12 previous lines + 12 new disjoint lines, prepended 12:
        # keep computes 0 → REFUSED, naming T192's rule, the probed
        # arithmetic, and the remedy.
        prev = _cycle520_prev_ring()
        self.assertEqual(len(prev), 12)
        batch = _batch_of(12)
        self.assertEqual(len(batch), 12)
        with self.assertRaises(AssertionError) as ctx:
            wrap_assert.assert_ring_splice(prev, batch, len(batch))
        msg = str(ctx.exception)
        self.assertIn("zero-carry full-ring write is REFUSED", msg)
        self.assertIn("T192", msg)  # the governing rule named
        self.assertIn(
            "rotate at most 11 so one prior line always survives verbatim",
            msg,
        )
        self.assertIn("12 prepended line(s)", msg)  # probed prepended count
        self.assertIn("capacity-12", msg)  # probed capacity
        self.assertIn("composed 12-line ring", msg)  # probed new length
        self.assertIn("previous ring's 12 line(s)", msg)  # probed prev length
        self.assertIn("keep = max(0, ", msg)  # the probed arithmetic
        self.assertIn("prepend-11/carry-1", msg)  # the remedy shape

    def test_pass_prepend_11_carry_1_at_capacity_is_green(self):
        # (b) the remedy shape at capacity: 11 prepended + 1 carried —
        # keep = max(0, min(12 - 11, 12)) = 1, one prior line survives
        # verbatim as the tail. GREEN.
        prev = _cycle520_prev_ring()
        batch = _batch_of(11)
        composed = batch + prev[:1]
        self.assertEqual(len(composed), 12)
        wrap_assert.assert_ring_splice(prev, composed, len(batch))

    def test_pass_under_capacity_prepend_still_green(self):
        # (c) below capacity no new constraint: 6 previous lines + 3 new —
        # the composed 9-line ring keeps every previous line (keep = 6),
        # the under-capacity append path the eval-delta `k < maxd` leg
        # passes. GREEN.
        prev = _cycle520_prev_ring()[:6]
        self.assertEqual(len(prev), 6)
        batch = _batch_of(3)
        composed = batch + prev
        self.assertEqual(len(composed), 9)
        wrap_assert.assert_ring_splice(prev, composed, len(batch))

    def test_pass_empty_previous_ring_with_full_new_ring_is_green(self):
        # (d) the `m == 0` leg: an EMPTY previous ring with a full new ring
        # stays legal — a first-generation write carries nothing because
        # there is nothing to carry (eval-delta's `m == 0` → ok). GREEN.
        batch = _batch_of(12)
        self.assertEqual(len(batch), 12)
        wrap_assert.assert_ring_splice([], batch, len(batch))


# T295 fixtures: the validate_state facade's realistic state fixture,
# modeled on the live eval-state.md's shape — the nine `state_field`
# fields (the parser contract's exact list, scripts/eval-delta.sh: schema
# checked first, then the eight-key loop), a blank line, the `## health`
# and `## open-threads` sections, then the `## decisions` ring (last,
# running to EOF). The open-threads bullet STARTS `- d` on purpose: a
# whole-file scan would miscount it as a ring line (the fire-4 shape the
# section-scoped extraction exists for).
VS_FIELDS = [
    ("schema", "1"),
    ("eval-commit", "212bc03"),
    ("eval-at", "2026-10-13T19:40:14+00:00"),
    ("state-drift", "none"),
    (
        "fresh-input-last-eval",
        "37924676 fresh input tokens across 89 usage-bearing iteration "
        "events of 89 at this wrap write's read",
    ),
    (
        "cache-read-last-eval",
        "425566208 cache-read + 0 cache-creation + 6006789 output (same "
        "stream read, at the wrap write's read)",
    ),
    ("health", "gates GREEN through the wrap"),
    ("open-threads", "2 open — see the section below"),
    ("pacing-streak", "TRUE 0"),
]

# The previous state's ring (12 lines): the 4 lines the new write carries
# (the cycle-686 cluster's own records) + 8 older lines a full rotation
# drops from the TAIL.
VS_CARRIED = [
    "- d1791921121-12: wrap_assert call-signature 34 (a carried ring line)",
    "- d1791921226-14: wrap_assert call-signature 35 (a carried ring line)",
    "- d1791921022-10: python-assertion-slip 105 (a carried ring line)",
    "- d1791920768-8: python-assertion-slip 104 (a carried ring line)",
]


def _vs_batch(count=8, start=1):
    """The new batch's ring lines (fresh ids, disjoint from the carried set)."""
    return [
        f"- d1791950000-{i}: eval-triage T295 new batch {i} (a probed ring line)"
        for i in range(start, start + count)
    ]


def _vs_older(count=8):
    return [
        f"- d1791880000-{i}: an older decision body {i} (rotates off first)"
        for i in range(1, count + 1)
    ]


VS_PREV_RING = VS_CARRIED + _vs_older(8)


# T306 fixtures: the cycle-2035 filing fire's eval-commit pair — the TRUE
# full hash (probed) and the falsified variant the 2031 wrap's state
# recompose actually wrote (7-char prefix TRUE, the suffix memory-composed;
# `git cat-file -t` on it rc-128 `could not get object info`). The legs
# below pin the write-time probed-equality leg against exactly these.
T306_TRUE_HASH = "39304522865f99ca95b6e8b8750e1714e627e7fb"
T306_FABRICATED_HASH = "3930452b1f9e0e05a12e93d69a2d6ba95d7cbe47"


def _vs_fields_with_eval_commit(value):
    """VS_FIELDS with the `eval-commit` value swapped to *value* (T306)."""
    return [(k, value if k == "eval-commit" else v) for k, v in VS_FIELDS]


# T307 fixtures: the cycle-2039 filing fire's state-drift pair. The
# prose-after-token shape is the schema contract the boot delta builder's
# own comment names (`none — first write (bootstrap; …)`, the 1978 wrap's
# `none (the boot STATE-HIT: …)`: the flag token FIRST, the prose AFTER
# it). The narrative-first shape is what the 2035 wrap's state FULL
# recompose actually wrote — the healed event's NARRATIVE in the flag
# position — so the 2039 boot's delta re-flagged FULL-READ REQUIRED
# (state-drift-flagged) with the marker HEALED and reachable (fail-safe,
# zero routing consequence; the latch the write-side leg exists to close).
T307_PROSE_AFTER_TOKEN = (
    "none (the boot STATE-HIT: the slim path holds, re-arm 373rd discharged)"
)
T307_NARRATIVE_FIRST = (
    "FULL-READ REQUIRED (marker-unreachable) at the boot - the 2035 wrap's "
    "composition miss: the healed event's narrative in the flag position"
)


def _vs_fields_with_state_drift(value):
    """VS_FIELDS with the `state-drift` value swapped to *value* (T307)."""
    return [(k, value if k == "state-drift" else v) for k, v in VS_FIELDS]


def _vs_composed_ring():
    """The composed ring: 8 prepended + 4 carried = 12 (a full rotation)."""
    return _vs_batch(8) + VS_CARRIED


def _vs_ids(lines):
    return [
        line[2:].split(":", 1)[0].strip()
        for line in lines
        if line.startswith("- d")
    ]


def _vs_text(ring=None, fields=None, header=RING_HEADER, tail_sections=""):
    """The composed state TEXT: the nine fields, ## health, ## open-threads
    (one `- d` bullet OUTSIDE the ring), the ring (last, to EOF), optional
    tail sections after it. Per-field mutations ride the `fields` arg; ring
    mutations ride the `ring` arg."""
    lines = [
        f"{key}: {value}" for key, value in (VS_FIELDS if fields is None else fields)
    ]
    lines += [
        "",
        "## health (one line per load-bearing check)",
        "- gates: green",
        "",
        "## open-threads (censuses the next eval updates; each names its trigger)",
        "- decision_log-arg-slip census 5 (UNCHANGED — a `- d` bullet OUTSIDE the ring)",
    ]
    if header is not None:
        lines.append(header)
        lines += _vs_composed_ring() if ring is None else ring
    if tail_sections:
        lines += tail_sections.splitlines()
    return "\n".join(lines) + "\n"


class ValidateStateTest(unittest.TestCase):
    """T295: the ONE-call state-write gate facade (the cycle-686 cluster's
    canonical fix — the surface conversions owned internally, so no call
    site re-derives which helper walks path-or-MAPPING vs path-or-LINES vs
    EXTRACTED ring lines)."""

    def test_state_field_keys_match_the_parser_contract(self):
        # The nine `state_field` extractions, in the parser's check order
        # (schema first, then eval-delta.sh's eight-key loop) — the facade
        # gates exactly these, on the MAPPING, never on the raw text.
        self.assertEqual(
            list(wrap_assert.STATE_FIELD_KEYS),
            [key for key, _ in VS_FIELDS],
        )

    def test_pass_realistic_fixture_one_call(self):
        # (b) the realistic fixture: schema + the nine `state_field`
        # fields, the sections, a `last 12` ring of 12 — ONE call, zero
        # surface conversions at the call site.
        wrap_assert.validate_state(_vs_text())

    def test_pass_full_gate_prev_and_tail_ids(self):
        # The dogfood shape: prev + corpus-tail ids supplied — the splice
        # and the corpus-tail identity ride the SAME one call (the 686
        # gate's ~7 surfaces in one).
        ring = _vs_composed_ring()
        wrap_assert.validate_state(
            _vs_text(ring=ring),
            prev_ring_lines=VS_PREV_RING,
            corpus_tail_ids=_vs_ids(ring),
        )

    def test_pass_facade_on_the_fired_helpers_same_text(self):
        # (a) the d1791921121-12 / d1791921226-14 pair pinned end-to-end:
        # the RAW composed text into the field/count helpers still errors
        # (a str walks as a PATH — `OSError: File name too long`, the
        # interface defense intact, the facade widens nothing), while
        # validate_state on the SAME text passes.
        text = _vs_text()
        with self.assertRaises(OSError):
            wrap_assert.assert_fields_non_empty(
                text, list(wrap_assert.STATE_FIELD_KEYS)
            )
        with self.assertRaises(OSError):
            wrap_assert.assert_ring_count(text)
        wrap_assert.validate_state(text)

    def test_pass_prev_splice_with_a_section_after_the_ring(self):
        # The ring is not always last: a section after it leaves a blank
        # body line in the extraction — formatting noise, not a carried
        # line (the unique gate's own blank exemption) — and the splice
        # gate must not false-fire on it. The tail bullet starts `- d`:
        # still outside the section (the count gate's scoping holds).
        tail = (
            "",
            "## wrap-notes (the next wrap's input)",
            "- d-scale prose quote: NOT a ring line (after the ring section)",
        )
        wrap_assert.validate_state(
            _vs_text(tail_sections="\n".join(tail)),
            prev_ring_lines=VS_PREV_RING,
        )

    def test_fail_dropped_field_names_the_missing_key(self):
        # (c) the FIELDS sub-surface: a dropped `eval-at:` line fails the
        # facade naming the probed absence.
        fields = [f for f in VS_FIELDS if f[0] != "eval-at"]
        with self.assertRaises(AssertionError) as ctx:
            wrap_assert.validate_state(_vs_text(fields=fields))
        msg = str(ctx.exception)
        self.assertIn("missing key 'eval-at'", msg)
        self.assertIn("1/9 named field(s) failed", msg)

    def test_fail_empty_field_names_the_empty_key(self):
        # (c) the FIELDS sub-surface, the empty-value shape: the split-read
        # probed '' is named (the T270 shape, never a length threshold).
        fields = [(k, "" if k == "pacing-streak" else v) for k, v in VS_FIELDS]
        with self.assertRaises(AssertionError) as ctx:
            wrap_assert.validate_state(_vs_text(fields=fields))
        msg = str(ctx.exception)
        self.assertIn("empty key 'pacing-streak'", msg)
        self.assertIn("the split-read probed ''", msg)

    def test_fail_duplicated_ring_line_names_the_dup_and_both_offsets(self):
        # (c) the UNIQUENESS sub-surface: a byte-duplicated ring line (the
        # count still reads 12 — the T289 shape) fails the facade naming
        # the duplicated id and BOTH offsets.
        ring = _vs_composed_ring()
        ring[11] = ring[0]
        with self.assertRaises(AssertionError) as ctx:
            wrap_assert.validate_state(_vs_text(ring=ring))
        msg = str(ctx.exception)
        self.assertIn("duplicated line", msg)
        self.assertIn("duplicated decision id 'd1791950000-1'", msg)
        self.assertIn("ring lines 1 and 12", msg)
        self.assertIn("file lines 17 and 28", msg)  # the ring body's file anchor

    def test_fail_misspliced_carry_names_both_probed_reprs(self):
        # (c) the SPLICE sub-surface: a mutated carried line (its id kept,
        # so the probed prepended count stays honest) fails the facade
        # positionally, naming the probed expected/actual reprs.
        ring = _vs_composed_ring()
        ring[8] = ring[8] + " (rewritten)"  # the first carried line mutated
        with self.assertRaises(AssertionError) as ctx:
            wrap_assert.validate_state(
                _vs_text(ring=ring), prev_ring_lines=VS_PREV_RING
            )
        msg = str(ctx.exception)
        self.assertIn("carried position 0", msg)
        self.assertIn(repr(VS_CARRIED[0]), msg)  # the probed expected repr
        self.assertIn("rewritten", msg)  # the probed actual sitting there

    def test_fail_corpus_tail_mismatch_names_probed_vs_supplied(self):
        # (c) the IDENTITY sub-surface: a supplied tail id the ring does
        # not hold — the mismatch names the probed ring ids, the supplied
        # tail ids, and the first divergence (a WRONG id at index 3, then
        # a SHORT tail, both probed).
        ring = _vs_composed_ring()
        supplied = _vs_ids(ring)
        supplied[3] = "d1791999999-99"  # a tail id the ring does not hold
        with self.assertRaises(AssertionError) as ctx:
            wrap_assert.validate_state(
                _vs_text(ring=ring), corpus_tail_ids=supplied
            )
        msg = str(ctx.exception)
        self.assertIn("corpus-tail identity", msg)
        self.assertIn("'d1791999999-99'", msg)  # the supplied divergence
        self.assertIn("'d1791950000-4'", msg)  # the probed id at the index
        self.assertIn("first divergence at tail index 3", msg)
        short = _vs_ids(ring)[:-1]  # the supplied tail one id short
        with self.assertRaises(AssertionError) as ctx:
            wrap_assert.validate_state(
                _vs_text(ring=ring), corpus_tail_ids=short
            )
        msg = str(ctx.exception)
        self.assertIn("probed decision ids (12)", msg)
        self.assertIn("supplied tail ids (11)", msg)

    def test_pass_last8_header_gates_against_8(self):
        # (d) the capacity is read from the header: a `last 8` fixture ring
        # gates against 8, not a hardcoded 12 — and a 9-line ring on the
        # same header FAILS naming the probed capacity 8.
        header8 = (
            "## decisions ring (last 8, newest first — newest batch at the head)"
        )
        ring8 = _vs_batch(4) + VS_CARRIED  # 8 lines
        wrap_assert.validate_state(_vs_text(ring=ring8, header=header8))
        ring9 = _vs_batch(5) + VS_CARRIED  # 9 lines against a last-8 header
        with self.assertRaises(AssertionError) as ctx:
            wrap_assert.validate_state(_vs_text(ring=ring9, header=header8))
        msg = str(ctx.exception)
        self.assertIn("count 9", msg)  # the probed actual
        self.assertIn("capacity 8", msg)  # probed, never a remembered 12
        self.assertIn("probed from the header", msg)

    def test_pass_last8_header_with_prev_gates_splice_against_8(self):
        # The M7 pin (the kimi validator's named survivor): prev_ring_lines
        # COMBINED with a non-12 ring header — the splice leg's capacity is
        # probed from the header (8), never the fallback 12. keep =
        # min(8 - 2, 8) = 6 carried under the probe; a hardcoded 12 would
        # demand min(12 - 2, 8) = 8 carried and false-REJECT this correct
        # state (the fail-safe direction the verdict named, pinned shut).
        header8 = (
            "## decisions ring (last 8, newest first — newest batch at the head)"
        )
        prev8 = VS_CARRIED + _vs_older(4)  # the previous ring's 8 lines
        ring8 = _vs_batch(2) + prev8[:6]  # 2 prepended + 6 carried = 8
        wrap_assert.validate_state(
            _vs_text(ring=ring8, header=header8), prev_ring_lines=prev8
        )

    def test_fail_non_str_text_raises_type_error_naming_text(self):
        # The facade's own interface defense: *text* is the composed TEXT —
        # anything else is the call-signature slip shape this facade
        # exists to close, so it TypeErrors naming the accepted shape.
        with self.assertRaises(TypeError) as ctx:
            wrap_assert.validate_state(7)
        msg = str(ctx.exception)
        self.assertIn("TEXT", msg)
        self.assertIn("type int", msg)

    def test_fail_str_prev_ring_lines_raises_type_error(self):
        # The sibling slip shape: the PREVIOUS ring's LINES are a list —
        # raw text is the banned shape (a str would walk as CHARACTERS).
        with self.assertRaises(TypeError) as ctx:
            wrap_assert.validate_state(_vs_text(), prev_ring_lines="carried")
        msg = str(ctx.exception)
        self.assertIn("prev_ring_lines", msg)
        self.assertIn("LINES", msg)
        self.assertIn("type str", msg)

    def test_fail_str_corpus_tail_ids_raises_type_error(self):
        with self.assertRaises(TypeError) as ctx:
            wrap_assert.validate_state(
                _vs_text(), corpus_tail_ids="d1791950000-1"
            )
        msg = str(ctx.exception)
        self.assertIn("corpus_tail_ids", msg)
        self.assertIn("type str", msg)

    def test_fail_cycle1042_zero_carry_write_refused_at_the_facade(self):
        # T298 leg (e), the cycle-1042 regression: the wrap's first state
        # write prepended the cycle's 12 post-eval records onto the
        # previous 12-line ring — keep computed 0, the carried-window
        # assertion passed VACUOUSLY, and THIS facade waved the write GREEN
        # (12/12 same-generation ids, zero prior-generation lines
        # surviving; eval-delta caught it one generation later). The
        # write-time gate now refuses the same bytes.
        batch = _batch_of(12)  # the 12-record batch, disjoint ids
        self.assertEqual(len(VS_PREV_RING), 12)
        with self.assertRaises(AssertionError) as ctx:
            wrap_assert.validate_state(
                _vs_text(ring=batch), prev_ring_lines=VS_PREV_RING
            )
        msg = str(ctx.exception)
        self.assertIn("zero-carry full-ring write is REFUSED", msg)
        self.assertIn("T192", msg)
        self.assertIn(
            "rotate at most 11 so one prior line always survives verbatim",
            msg,
        )
        self.assertIn("12 prepended line(s)", msg)
        self.assertIn("previous ring's 12 line(s)", msg)
        self.assertIn("prepend-11/carry-1", msg)

    def test_pass_eval_commit_equals_the_probed_hash(self):
        # T306 PASS path: the state's `eval-commit` EQUALS the probed full
        # hash the call site passed in (probed via `git rev-parse <commit>`
        # at the wrap's state-write helper — never composed from memory);
        # an honest write rides the leg green, all other gates unchanged.
        text = _vs_text(fields=_vs_fields_with_eval_commit(T306_TRUE_HASH))
        wrap_assert.validate_state(text, expected_eval_commit=T306_TRUE_HASH)

    def test_fail_fabricated_suffix_eval_commit_names_both_values(self):
        # T306 FAIL path, the cycle-2035 filing fire's exact shape: the
        # 7-char prefix TRUE (`3930452`), the suffix memory-composed — the
        # field is non-empty (the pre-leg facade passed it, the field
        # gated NON-EMPTY only) and must NOT equal the probed hash. The
        # failure names BOTH the probed field value and the supplied
        # probed actual (the T270 reporting requirement — never a bare
        # constant mismatch), plus the probe discipline that closes it.
        text = _vs_text(
            fields=_vs_fields_with_eval_commit(T306_FABRICATED_HASH)
        )
        with self.assertRaises(AssertionError) as ctx:
            wrap_assert.validate_state(
                text, expected_eval_commit=T306_TRUE_HASH
            )
        msg = str(ctx.exception)
        self.assertIn(T306_FABRICATED_HASH, msg)  # the probed field value
        self.assertIn(T306_TRUE_HASH, msg)  # the supplied probed actual
        self.assertIn("eval-commit", msg)  # the surface named
        self.assertIn("git rev-parse", msg)  # the probe discipline named

    def test_inert_without_expected_eval_commit(self):
        # T306 INERT path: without the keyword the leg is inert — every
        # pre-existing call site behaves exactly as today, INCLUDING on
        # the fabricated text (the write-time half is opt-in; the
        # boot-time fail-closed FULL-READ marker check in eval-delta.sh
        # stays the other half, unchanged). This leg also pins the RED
        # shape: pre-T306, THIS call on THIS text was the whole gate —
        # the falsified hash sailed through.
        text = _vs_text(
            fields=_vs_fields_with_eval_commit(T306_FABRICATED_HASH)
        )
        wrap_assert.validate_state(text)
        wrap_assert.validate_state(
            _vs_text(fields=_vs_fields_with_eval_commit(T306_TRUE_HASH))
        )

    def test_fail_non_str_expected_eval_commit_raises_type_error(self):
        # The facade's interface defense, the new arg's shape: the probed
        # hash is a STR (`git rev-parse`'s output) — anything else is the
        # call-signature slip shape the facade exists to close.
        with self.assertRaises(TypeError) as ctx:
            wrap_assert.validate_state(
                _vs_text(), expected_eval_commit=3930452
            )
        msg = str(ctx.exception)
        self.assertIn("expected_eval_commit", msg)
        self.assertIn("PROBED full hash", msg)
        self.assertIn("type int", msg)

    def test_pass_state_drift_prose_after_none_token_is_green(self):
        # T307 PASS path: the prose-after-token shape the schema contract
        # names — the flag token `none` FIRST, the narrative AFTER it (the
        # 1978 wrap's `none (the boot STATE-HIT: …)` shape) — rides the
        # first-token leg GREEN: the T304 first-token semantics mirrored
        # write-side, prose after `none` stays legal.
        text = _vs_text(
            fields=_vs_fields_with_state_drift(T307_PROSE_AFTER_TOKEN)
        )
        wrap_assert.validate_state(text)

    def test_fail_narrative_first_state_drift_names_token_and_field(self):
        # T307 FAIL path, the cycle-2039 filing fire's exact shape: the
        # 2035 wrap's state FULL recompose wrote the healed event's
        # NARRATIVE in the flag position — the field is non-empty (the
        # pre-leg facade passed it, the field gated NON-EMPTY only) and
        # its FIRST whitespace-token is not `none`, so the 2039 boot's
        # delta re-flagged FULL-READ REQUIRED with the marker HEALED and
        # reachable (every genuine drift event re-flags the next boot —
        # the permanent latch). The failure names BOTH the received first
        # token and the full field value (the T270 reporting requirement —
        # never a bare constant), plus the composition contract that
        # closes it.
        text = _vs_text(
            fields=_vs_fields_with_state_drift(T307_NARRATIVE_FIRST)
        )
        with self.assertRaises(AssertionError) as ctx:
            wrap_assert.validate_state(text)
        msg = str(ctx.exception)
        self.assertIn("'FULL-READ'", msg)  # the received first token
        self.assertIn(T307_NARRATIVE_FIRST, msg)  # the full field value
        self.assertIn("state-drift", msg)  # the surface named
        self.assertIn("first whitespace-token", msg)  # the flag position
        self.assertIn("narrative AFTER the token", msg)  # the contract


# ---- T309: the fill-datum leg (assert_fill_datum) ----

T309_FILL_STAMP = "2026-10-28T12:28:47Z"
# A realistic scripts/stream_datum.py stdout (the pinned 13-line output,
# the fill stamp's figures the cycle-2240 wrap's helper run printed).
T309_TRUE_DATUM = (
    "iters: 73\n"
    "first-anchor: 2026-10-28T12:08:22.432Z\n"
    "last-anchor: 2026-10-28T12:28:45.974Z\n"
    "wall-secs: 1223.542\n"
    "ok-false: 1\n"
    "ok-false-legs: 2026-10-28T12:18:42.887Z\n"
    "ctx-edit: accepted 1 / rejected 0\n"
    "trim: 0\n"
    "trim-legs: none\n"
    "goal: none\n"
    "goal-ts: none\n"
    "bytes: 408057\n"
    "mtime: 2026-10-28T12:28:46Z\n"
)
# The corpus's FINAL-datum rendering: the TRUE block (fill eae4d90's
# corrected figures) and the falsified 8ea1079 block it corrected.
T309_TRUE_BLOCK = (
    "73 iters / ~1,223.542s wall [2026-10-28T12:08:22.432Z -> "
    "2026-10-28T12:28:45.974Z off the stream anchors; PLAIN display 1223 "
    "== 20*60+23]"
)
T309_FALSIFIED_BLOCK = (
    "72 iters / ~1,225.624s wall [2026-10-28T12:08:22.432Z -> "
    "2026-10-28T12:28:48.056Z off the stream anchors; PLAIN display 1225 "
    "== 20*60+25]"
)


def _t309_fill(block):
    """A fill text carrying *block* as its FINAL datum (the corpus shape)."""
    return (
        "eval: cycle-2240 wrap fill - the state FULL recompose landed "
        "[FACADE-GREEN]; FINAL datum at the fill stamp [GENERATED by "
        "scripts/stream_datum.py per the T305 doctrine, "
        f"{T309_FILL_STAMP}]: {block}: ok:false 1 == 1 adjudicated "
        "[[1] the battery-poll compound trailing-exit artifact, 0-REAL] "
        "+ trim 0 == 0 + goal 0 at the stamp"
    )


class AssertFillDatumTest(unittest.TestCase):
    def test_pass_true_helper_figures(self):
        # T309 PASS path: the fill text carrying the helper's exact figures
        # — the corpus's comma-grouped wall rendering (~1,223.542s) — rides
        # the datum leg green.
        wrap_assert.assert_fill_datum(
            _t309_fill(T309_TRUE_BLOCK), T309_TRUE_DATUM
        )

    def test_pass_helper_printed_wall_uncommaed(self):
        # The comma normalization is tolerance, not requirement: the wall
        # figure EXACTLY as the helper printed it (1223.542, no grouping)
        # passes too — the assert is against the helper's printed figure.
        wrap_assert.assert_fill_datum(
            _t309_fill(T309_TRUE_BLOCK.replace("~1,223.542s", "~1223.542s")),
            T309_TRUE_DATUM,
        )

    def test_pass_wrapped_marker_still_scans(self):
        # The scan is whitespace-tolerant: a commit message that wraps
        # between the marker's slash and the wall figure still reads as a
        # datum block (the wrap-anywhere prose hazard stays a non-issue).
        wrap_assert.assert_fill_datum(
            _t309_fill(T309_TRUE_BLOCK.replace("73 iters / ~", "73 iters /\n~")),
            T309_TRUE_DATUM,
        )

    def test_fail_eae4d90_shape_names_the_divergent_figures(self):
        # T309 FAIL path, the 4th sighting's exact shape: fill 8ea1079's
        # committed FINAL datum (72 iters / ~1,225.624s wall [-> 12:28:48.
        # 056Z]) against the TRUE helper record at the stamp (73 / 1,223.
        # 542 / -> 12:28:45.974Z) — the compose-from-projection fire. Every
        # divergent figure is named with BOTH values; the START anchor
        # matched and is NOT reported as divergent (the T270 reporting
        # requirement: the probed actuals of the failure, not a dump).
        text = _t309_fill(T309_FALSIFIED_BLOCK)
        with self.assertRaises(AssertionError) as ctx:
            wrap_assert.assert_fill_datum(text, T309_TRUE_DATUM)
        msg = str(ctx.exception)
        self.assertIn("iters '72' != probed '73'", msg)
        self.assertIn("1,225.624", msg)  # the composed (grouped) figure
        self.assertIn("1225.624", msg)  # the comma-normalized figure
        self.assertIn("!= probed '1223.542'", msg)
        self.assertIn("'2026-10-28T12:28:48.056Z'", msg)  # the end anchor
        self.assertIn("!= probed '2026-10-28T12:28:45.974Z'", msg)
        self.assertNotIn("first-anchor", msg)  # the matched figure: silent
        self.assertIn("block 1 of 1", msg)
        self.assertIn("8ea1079", msg)  # the fire named
        self.assertIn("T305", msg)  # the doctrine named

    def test_fail_start_anchor_divergence_named(self):
        # The start anchor's kill-proof (the sweep-the-family finding:
        # every asserted surface needs a RED-proven killing test). The
        # eae4d90 fixture's anchors MATCH by construction — iters / wall /
        # last-anchor carry their kill-proofs there, but the first-anchor
        # surface had none, so the mutant dropping the first-anchor
        # compare survived the suite green. A start-anchor-ONLY
        # divergence (a stale-probe anchor) fires naming it — and ONLY
        # it: the matched iters / wall / last-anchor are not reported
        # divergent (the T270 reporting requirement).
        text = _t309_fill(
            T309_TRUE_BLOCK.replace(
                "2026-10-28T12:08:22.432Z", "2026-10-28T12:07:00.000Z", 1
            )
        )
        with self.assertRaises(AssertionError) as ctx:
            wrap_assert.assert_fill_datum(text, T309_TRUE_DATUM)
        msg = str(ctx.exception)
        self.assertIn(
            "first-anchor '2026-10-28T12:07:00.000Z' != probed "
            "'2026-10-28T12:08:22.432Z'",
            msg,
        )
        self.assertEqual(msg.count("!= probed"), 1)  # one divergent figure
        self.assertNotIn("!= probed '73'", msg)
        self.assertNotIn("!= probed '1223.542'", msg)
        self.assertNotIn("!= probed '2026-10-28T12:28:45.974Z'", msg)

    def test_fail_fire_is_pre_write_zero_mutation(self):
        # The fire is designed pre-commit: both arguments are strs, the
        # function opens and writes NOTHING — the tempdir is byte-unchanged
        # through the raise.
        with tempfile.TemporaryDirectory() as tmp:
            before = sorted(os.listdir(tmp))
            with self.assertRaises(AssertionError):
                wrap_assert.assert_fill_datum(
                    _t309_fill(T309_FALSIFIED_BLOCK), T309_TRUE_DATUM
                )
            self.assertEqual(sorted(os.listdir(tmp)), before)

    def test_boundary_plain_display_and_adjudications_not_asserted(self):
        # The boundary the spec names: the PLAIN display parenthetical
        # derives FROM the wall figure — asserting it would double-count —
        # so a divergent (and an absent) parenthetical stays GREEN. The
        # block's other figures (ok:false, trim — the fill prose's own
        # adjudications) are likewise outside the datum leg's surface.
        wrap_assert.assert_fill_datum(
            _t309_fill(
                T309_TRUE_BLOCK.replace(
                    "PLAIN display 1223 == 20*60+23",
                    "PLAIN display 9999 == 20*60+999",
                )
            ),
            T309_TRUE_DATUM,
        )
        wrap_assert.assert_fill_datum(
            _t309_fill(
                T309_TRUE_BLOCK.replace(
                    "; PLAIN display 1223 == 20*60+23", ""
                )
            ),
            T309_TRUE_DATUM,
        )
        wrap_assert.assert_fill_datum(
            _t309_fill(T309_TRUE_BLOCK).replace("ok:false 1 == 1", "ok:false 9 == 9")
            .replace("trim 0 == 0", "trim 7 == 7"),
            T309_TRUE_DATUM,
        )

    def test_fail_absent_datum_block_named_missing(self):
        # A datum-less fill is NEVER a silent pass: zero blocks probed
        # fires named-missing — including when `iters` appears as prose or
        # in the establishment shape (`78/1,417`), neither of which is a
        # datum block.
        for bare in (
            "eval: cycle-2240 wrap fill - the state FULL recompose landed",
            "the iters figure rides the notes' T257 color",
            "the 2240 stream 78/1,417 the establishment shape",
        ):
            with self.assertRaises(AssertionError) as ctx:
                wrap_assert.assert_fill_datum(bare, T309_TRUE_DATUM)
            msg = str(ctx.exception)
            self.assertIn("no FINAL datum block found", msg)
            self.assertIn("named-missing", msg)
            self.assertIn("scripts/stream_datum.py", msg)

    def test_fail_degenerate_datum_output_fails_closed(self):
        # A datum_output without an assertable figure fails closed naming
        # the probed line — a wrong-stream probe (an unparsable-anchor
        # `none`, a missing key, a non-digit count) can never wave GREEN.
        for datum, frag in (
            (T309_TRUE_DATUM.replace("wall-secs: 1223.542", "wall-secs: none"),
             "the probed `wall-secs:` figure is 'none'"),
            (T309_TRUE_DATUM.replace(
                "last-anchor: 2026-10-28T12:28:45.974Z\n", ""),
             "no `last-anchor:` line in the supplied datum_output"),
            (T309_TRUE_DATUM.replace("iters: 73", "iters: seventy-three"),
             "the probed `iters:` figure 'seventy-three' is not a digit "
             "count"),
        ):
            with self.assertRaises(AssertionError) as ctx:
                wrap_assert.assert_fill_datum(_t309_fill(T309_TRUE_BLOCK), datum)
            self.assertIn(frag, str(ctx.exception))

    def test_fail_non_str_arguments_raise_type_error(self):
        # The interface defense: *text* is the composed TEXT and
        # *datum_output* is the helper's raw stdout STR — a pre-split line
        # list is the call-signature slip shape (the caller passes the
        # probed stdout verbatim), so it TypeErrors naming the shape.
        with self.assertRaises(TypeError) as ctx:
            wrap_assert.assert_fill_datum(7, T309_TRUE_DATUM)
        msg = str(ctx.exception)
        self.assertIn("TEXT", msg)
        self.assertIn("type int", msg)
        with self.assertRaises(TypeError) as ctx:
            wrap_assert.assert_fill_datum(_t309_fill(T309_TRUE_BLOCK), None)
        msg = str(ctx.exception)
        self.assertIn("FRESH", msg)
        self.assertIn("scripts/stream_datum.py", msg)
        self.assertIn("type NoneType", msg)
        with self.assertRaises(TypeError) as ctx:
            wrap_assert.assert_fill_datum(
                _t309_fill(T309_TRUE_BLOCK), T309_TRUE_DATUM.splitlines()
            )
        msg = str(ctx.exception)
        self.assertIn("13 probed line(s)", msg)
        self.assertIn("type list", msg)

    def test_pass_correction_quote_shape_multi_block(self):
        # The correction-message shape (eae4d90's own): one text quoting
        # BOTH the falsified block and the TRUE record stays GREEN — the
        # assertion needs ONE block carrying the fresh run's exact figures.
        # Safe by construction: a FRESH run's stamp-figures cannot equal
        # any earlier probe's block (the stamp moves), so the any-block
        # semantics cannot launder a compose-from-projection fire.
        both = _t309_fill(T309_FALSIFIED_BLOCK) + "\n" + _t309_fill(
            T309_TRUE_BLOCK
        )
        wrap_assert.assert_fill_datum(both, T309_TRUE_DATUM)
        # ...and the same text against a figure-less datum still fails
        # closed — the freshness contract binds the PROBED side, not the
        # text side.
        with self.assertRaises(AssertionError):
            wrap_assert.assert_fill_datum(
                both, T309_TRUE_DATUM.replace("wall-secs: 1223.542",
                                              "wall-secs: none")
            )


# ---- T310: the fill-enumeration leg (assert_fill_enumeration) ----

T310_FILL_STAMP = "2026-10-28T14:03:44Z"
# The mechanical enumeration the compose leg probes at the fill stamp (the
# T282 channel half: probed values ride argv/env into the script, never
# literal-composed) — the cycle-2244 stream's whole-final `ok:false`-leg
# count is FIVE: the four legs the fill adjudicates plus the fifth, the
# 14:03:24.421Z assert_fill_datum AssertionError (the T309 guard's own
# first production fire, itself the designed pre-commit catch).
T310_PROBED_COUNT = 5
# The corpus's FINAL-enumeration rendering: the TRUE figure (the mechanical
# 5) and the falsified 5b5e38f figure (the pre-fire probe's read, taken
# BEFORE the datum-leg guard fired).
T310_TRUE_BLOCK = (
    "ok:false 5 == 5 adjudicated [[1] the 13:44:45.635Z M4-reproof needle "
    "assert-miss [read-only, recomposed grep-verified the very next leg] + "
    "[2] the 13:52:18.630Z git-add gitignore refusal [the DESIGNED refusal] "
    "+ [3] the 14:00:55.442Z facade TypeError [read-only] + [4] the "
    "14:01:04.251Z assert_ring_splice AssertionError [the prepend-12/"
    "carry-0 REFUSAL, zero mutation] + [5] the 14:03:24.421Z "
    "assert_fill_datum AssertionError [the T309 datum-leg guard's first "
    "production fire, the designed pre-commit catch], 0-REAL]"
)
T310_FALSIFIED_BLOCK = (
    "ok:false 4 == 4 adjudicated [[1] the 13:44:45.635Z M4-reproof needle "
    "assert-miss [read-only, recomposed grep-verified the very next leg] + "
    "[2] the 13:52:18.630Z git-add gitignore refusal [the DESIGNED refusal] "
    "+ [3] the 14:00:55.442Z facade TypeError [read-only] + [4] the "
    "14:01:04.251Z assert_ring_splice AssertionError [the prepend-12/"
    "carry-0 REFUSAL, zero mutation], 0-REAL]"
)


# The corpus's FINAL-datum rendering is one physical line in the commit
# subject, but a fill BODY wraps — a multi-line TRUE fixture (the
# line-agnostic scan) doubling as the pre-split line-list slip shape's
# source (7 lines).
T310_MULTILINE_FILL = "\n".join(
    [
        "eval: cycle-2244 wrap fill - the state write landed [FACADE-GREEN]",
        "FINAL datum at the fill stamp [GENERATED by scripts/stream_datum.py",
        f"per the T305 doctrine, {T310_FILL_STAMP}]: 159 iters / ~4,147.136s",
        "wall [2026-10-28T12:54:37.339Z -> 2026-10-28T14:03:44.475Z off the",
        f"stream anchors]: {T310_TRUE_BLOCK}",
        "+ ctx_edit 1 accepted / 1 rejected + trim 1 == 1 + goal 0 at the "
        "stamp",
        "the wrap final todo flips land BEFORE the goal gate per T255",
    ]
)


def _t310_fill(enum_block):
    """A fill text carrying *enum_block* as its FINAL-datum enumeration."""
    return (
        "eval: cycle-2244 wrap fill - the state write landed "
        "[FACADE-GREEN]; the fill-stamp pins re-verify 236/236 PINS-EXIT 0; "
        "FINAL datum at the fill stamp [GENERATED by scripts/stream_datum.py "
        f"per the T305 doctrine, {T310_FILL_STAMP}]: 159 iters / ~4,147.136s "
        "wall [2026-10-28T12:54:37.339Z -> 2026-10-28T14:03:44.475Z off the "
        "stream anchors; PLAIN display 4147 == 69*60+7]: "
        f"{enum_block} + ctx_edit 1 accepted / 1 rejected + trim 1 == 1 + "
        "goal 0 at the stamp"
    )


def _mutant_wrap_assert(pristine, old, new):
    """A mutated COPY of wrap_assert.py exec'd in a fresh namespace.

    The module itself is never written: the mutation rides an in-memory
    exec of the mutated source string (the module is import-free, so the
    exec needs nothing from sys.modules). The surgical-mutation self-check
    (the T48 needle idiom) refuses a non-unique target.
    """
    assert pristine.count(old) == 1, "the mutation target must be unique"
    namespace = {"__name__": "wrap_assert_mutant"}
    exec(compile(pristine.replace(old, new), "<wrap_assert-mutant>", "exec"),
         namespace)
    return namespace


class AssertFillEnumerationTest(unittest.TestCase):
    def test_pass_true_enumeration_carried_exactly(self):
        # T310 PASS path: the fill text carrying the mechanical count
        # exactly (ok:false 5 == 5, the 5-after-fire figure) rides the
        # enumeration leg green — the one-line subject shape AND a
        # multi-line body shape (the scan is line-agnostic).
        wrap_assert.assert_fill_enumeration(
            _t310_fill(T310_TRUE_BLOCK), T310_PROBED_COUNT, T310_FILL_STAMP
        )
        wrap_assert.assert_fill_enumeration(
            T310_MULTILINE_FILL, T310_PROBED_COUNT, T310_FILL_STAMP
        )

    def test_pass_bare_head_figure_and_wrapped_shapes(self):
        # The bare `ok:false K` head-figure (the establishment prose's
        # shape, `ok:false 5, ctx 1, trim 0, ...`) and the wrapped
        # rendering (`ok:false 5 ==\n5` — a commit message wrapped
        # mid-rendering) both carry the probed count: green. A legitimate
        # 0 count is likewise green (0 is a count, never degenerate).
        wrap_assert.assert_fill_enumeration(
            _t310_fill(
                "ok:false 5, ctx 1, trim 0, goal 1, 45,786 bytes + mtime "
                "2026-10-28T14:03:46Z"
            ),
            T310_PROBED_COUNT,
            T310_FILL_STAMP,
        )
        wrap_assert.assert_fill_enumeration(
            _t310_fill(T310_TRUE_BLOCK.replace(
                "ok:false 5 == 5", "ok:false 5 ==\n5")),
            T310_PROBED_COUNT,
            T310_FILL_STAMP,
        )
        wrap_assert.assert_fill_enumeration(
            _t310_fill("ok:false 0 == 0 (none to adjudicate, 0-REAL)"),
            0,
            T310_FILL_STAMP,
        )

    def test_fail_falsified_count_names_both_figures(self):
        # T310 FAIL path, the 5b5e38f shape exactly: the committed figure
        # (4, the pre-fire probe's read) against the mechanical 5 (the
        # fifth leg the T309 datum guard's own first fire). BOTH figures
        # are named — the divergent block figure and the probed count —
        # with the stamp, the fire, and the T272 false-at-write shape.
        text = _t310_fill(T310_FALSIFIED_BLOCK)
        with self.assertRaises(AssertionError) as ctx:
            wrap_assert.assert_fill_enumeration(
                text, T310_PROBED_COUNT, T310_FILL_STAMP
            )
        msg = str(ctx.exception)
        self.assertIn("block figure '4' != probed 5", msg)
        self.assertIn(T310_FILL_STAMP, msg)  # the probed enumeration stamp
        self.assertIn("5b5e38f", msg)  # the fire named
        self.assertIn("false-at-write", msg)  # the class named
        self.assertIn("recompose", msg)  # the remedy named
        self.assertIn("wc -l", msg)  # the mechanical enumeration named
        self.assertIn("block 1 of 1", msg)
        self.assertIn("the full FINAL-enumeration rendering (== 4)", msg)

    def test_fail_zero_blocks_named_missing(self):
        # A datum-less fill is NEVER a silent pass: zero enumeration
        # blocks probed fires named-missing — including a fill carrying a
        # datum block whose enumeration figure was dropped entirely, and
        # prose that merely names the class (`ok:false count-0`,
        # `ok:false-legs`) without a count figure.
        for bare in (
            "eval: cycle-2244 wrap fill - the state write landed "
            "[FACADE-GREEN]; FINAL datum at the fill stamp: 159 iters / "
            "~4,147.136s wall [2026-10-28T12:54:37.339Z -> "
            "2026-10-28T14:03:44.475Z] + trim 1 == 1 + goal 0 at the stamp",
            "the ok:false count-0 falsified at its own stamp",
            "the ok:false-legs projection the compose leg re-runs",
        ):
            with self.assertRaises(AssertionError) as ctx:
                wrap_assert.assert_fill_enumeration(
                    bare, T310_PROBED_COUNT, T310_FILL_STAMP
                )
            msg = str(ctx.exception)
            self.assertIn("no `ok:false K` enumeration figure", msg)
            self.assertIn("named-missing", msg)
            self.assertIn("never a silent pass", msg)
            self.assertIn(T310_FILL_STAMP, msg)
            self.assertIn("wc -l", msg)

    def test_fail_degenerate_probed_count_fails_closed(self):
        # A degenerate probed_count fails closed naming the probed value
        # and its type — a str count, None, a bool (a bool is an int
        # subclass; it is not a count), and a negative all refuse. The
        # corpus's false-at-write shape: a literal-composed count riding
        # the ARG can never wave the leg GREEN.
        for bad, frag in (
            ("5", "probed probed_count '5' (type str)"),
            (None, "probed probed_count None (type NoneType)"),
            (True, "probed probed_count True (type bool)"),
            (-1, "probed probed_count -1 (type int)"),
        ):
            with self.assertRaises(AssertionError) as ctx:
                wrap_assert.assert_fill_enumeration(
                    _t310_fill(T310_TRUE_BLOCK), bad, T310_FILL_STAMP
                )
            msg = str(ctx.exception)
            self.assertIn("fails closed", msg)
            self.assertIn(frag, msg)

    def test_fail_non_str_arguments_raise_type_error(self):
        # The interface defense: *text* is the composed TEXT and
        # *probed_stamp* is the probed stamp STR — a pre-split line list
        # is the text call-signature slip shape (the caller passes the
        # composed text verbatim), so it TypeErrors naming the shape.
        with self.assertRaises(TypeError) as ctx:
            wrap_assert.assert_fill_enumeration(
                7, T310_PROBED_COUNT, T310_FILL_STAMP
            )
        msg = str(ctx.exception)
        self.assertIn("TEXT", msg)
        self.assertIn("type int", msg)
        with self.assertRaises(TypeError) as ctx:
            wrap_assert.assert_fill_enumeration(
                T310_MULTILINE_FILL.splitlines(),
                T310_PROBED_COUNT,
                T310_FILL_STAMP,
            )
        msg = str(ctx.exception)
        self.assertIn("7 probed line(s)", msg)
        self.assertIn("type list", msg)
        with self.assertRaises(TypeError) as ctx:
            wrap_assert.assert_fill_enumeration(
                _t310_fill(T310_TRUE_BLOCK), T310_PROBED_COUNT, None
            )
        msg = str(ctx.exception)
        self.assertIn("probed_stamp", msg)
        self.assertIn("type NoneType", msg)

    def test_pass_any_block_stale_plus_true_redeemed(self):
        # The any-block semantics (the datum leg's rule): one text quoting
        # BOTH the stale pre-fire figure and the TRUE recompose figure
        # stays GREEN — the TRUE block redeems the text. Unlike the datum
        # leg's stamp-moves safety argument, a bare count CAN repeat
        # across streams — the freshness contract rides the compose leg's
        # SAME-leg re-probe (the T282 channel half: the probed count is
        # the live reading, captured into a shell variable, never
        # literal-composed), not the any-block semantics.
        both = _t310_fill(T310_FALSIFIED_BLOCK) + "\n" + _t310_fill(
            T310_TRUE_BLOCK
        )
        wrap_assert.assert_fill_enumeration(
            both, T310_PROBED_COUNT, T310_FILL_STAMP
        )
        # ...and the same text rides green against the stale figure's own
        # count too (any-block), while a text carrying ONLY the stale
        # figure still fires (the falsified-count leg above).

    def test_fail_fire_is_pre_write_zero_mutation(self):
        # The fire is designed pre-commit: all arguments are values, the
        # function opens and writes NOTHING — the tempdir is byte-unchanged
        # through the raise.
        with tempfile.TemporaryDirectory() as tmp:
            before = sorted(os.listdir(tmp))
            with self.assertRaises(AssertionError):
                wrap_assert.assert_fill_enumeration(
                    _t310_fill(T310_FALSIFIED_BLOCK),
                    T310_PROBED_COUNT,
                    T310_FILL_STAMP,
                )
            self.assertEqual(sorted(os.listdir(tmp)), before)

    def test_red_provability_mutants_run_on_a_copy(self):
        # RED-provability, hand-proved by mutants run on a COPY: the
        # module itself is never written (the mutation rides an in-memory
        # exec of a mutated source string), and the on-disk module is
        # re-read byte-identical after each run.
        with open(wrap_assert.__file__, "r", encoding="utf-8") as f:
            pristine = f.read()
        falsified_text = _t310_fill(T310_FALSIFIED_BLOCK)
        datumless_text = "eval: cycle-2244 wrap fill - no enumeration figure"
        # The shipped module fires on both inputs (the legs above pin it;
        # re-asserted here so the mutant contrast is self-contained).
        with self.assertRaises(AssertionError):
            wrap_assert.assert_fill_enumeration(
                falsified_text, T310_PROBED_COUNT, T310_FILL_STAMP
            )
        with self.assertRaises(AssertionError):
            wrap_assert.assert_fill_enumeration(
                datumless_text, T310_PROBED_COUNT, T310_FILL_STAMP
            )
        # M1 — the count compare deleted (every block redeems): the
        # falsified-count fire goes red (no raise under the mutant, where
        # the shipped leg raises).
        m1 = _mutant_wrap_assert(
            pristine,
            "        if int(head) == probed_count:",
            "        if True:",
        )
        m1["assert_fill_enumeration"](
            falsified_text, T310_PROBED_COUNT, T310_FILL_STAMP
        )
        # M2 — the zero-block fire silent-passed: the named-missing fire
        # goes red (no raise under the mutant, where the shipped leg
        # raises).
        m2 = _mutant_wrap_assert(
            pristine,
            "        _fire_named_missing(probed_stamp)",
            "        return",
        )
        m2["assert_fill_enumeration"](
            datumless_text, T310_PROBED_COUNT, T310_FILL_STAMP
        )
        # The module restored byte-identical after each run (nothing was
        # ever written — re-read and compared against the pristine read).
        with open(wrap_assert.__file__, "r", encoding="utf-8") as f:
            self.assertEqual(f.read(), pristine)


# ---- T311: the on-disk state-schema leg (assert_state_on_disk) ----

# The 2515-shape prose state: the cycle-2515 wrap's state recompose wrote
# `.chug/eval-state.md` in a free-form prose format — `last-eval-commit:`
# substituted for `eval-commit:`, ZERO of the nine pinned `state_field`
# fields, and none of the doctrine's pinned sections (only the non-schema
# `## Delta pointer` / `## Outcome-id ring`). Every loopd delta build since
# fails CLOSED (`FULL-READ REQUIRED: schema-mismatch`, the enforcer
# working) — yet fill a66f604 claimed "state write FACADE-GREEN first
# attempt" while validate_state on these committed bytes raises
# AssertionError at the FIELDS leg (RED-proven at the 597th eval).
T311_PROSE_STATE = (
    "## Delta pointer\n"
    "\n"
    "last-eval-commit: a66f604 (the 2515 wrap's substituted field — the\n"
    "schema's eval-commit field never written)\n"
    "next delta: builds from EVALUATION.md at the next boot\n"
    "\n"
    "## Outcome-id ring (last 5, newest first)\n"
    "- o-2515: the wrap's outcome record, prose format (not the schema's\n"
    "decision ring)\n"
    "- o-2514: the prior cycle's outcome record\n"
)


class AssertStateOnDiskTest(unittest.TestCase):
    """T311: the ON-DISK state-schema leg — the fill compose leg's pre-commit
    assert re-runs the FULL validate_state gate over the COMMITTED bytes (the
    2515 false-GREEN class's durable gate: a GREEN state-write claim is never
    composed without the mechanical run over the on-disk file)."""

    def test_pass_conformant_state_on_disk_without_the_keyword(self):
        # PASS path, keyword-less: the schema-conformant fixture (all nine
        # `state_field` fields, `state-drift: none`, the pinned sections, a
        # 12-cap ring) written to a temp file rides the FULL gate green —
        # the same bytes validate_state passes on, now read FROM DISK.
        with tempfile.TemporaryDirectory() as tmp:
            path = _write(tmp, "eval-state.md", _vs_text())
            wrap_assert.assert_state_on_disk(path)

    def test_pass_conformant_state_on_disk_with_probed_eval_commit(self):
        # PASS path with the keyword: the T306 probed-equality leg forwards —
        # the CALL SITE probes the full hash (`git log -1 --format=%H --
        # EVALUATION.md`, never composed from memory) and passes it in; an
        # honest write rides green with the hash gate ACTIVE.
        with tempfile.TemporaryDirectory() as tmp:
            path = _write(
                tmp,
                "eval-state.md",
                _vs_text(fields=_vs_fields_with_eval_commit(T306_TRUE_HASH)),
            )
            wrap_assert.assert_state_on_disk(path, T306_TRUE_HASH)

    def test_fail_2515_prose_shape_fails_the_fields_leg_naming_missing_keys(self):
        # FAIL path, the 2515 shape exactly: the prose-format state
        # (`last-eval-commit:` substituted for `eval-commit:`, zero schema
        # fields, only the non-schema sections) fails the leg from the
        # FIELDS leg — all nine pinned keys named missing, the substituted
        # `last-eval-commit:` field satisfying nothing. RED-proven at the
        # 597th eval: validate_state on these committed bytes raises here,
        # so the fill's "FACADE-GREEN first attempt" claim could not have
        # been composed from a real run over them.
        with tempfile.TemporaryDirectory() as tmp:
            path = _write(tmp, "eval-state.md", T311_PROSE_STATE)
            with open(path, "r", encoding="utf-8") as f:
                self.assertIn("last-eval-commit:", f.read())  # the substitution
            with self.assertRaises(AssertionError) as ctx:
                wrap_assert.assert_state_on_disk(path)
        msg = str(ctx.exception)
        self.assertIn("assert_fields_non_empty", msg)  # the FIELDS leg fired
        self.assertIn("9/9 named field(s) failed", msg)  # ALL nine missing
        for key in wrap_assert.STATE_FIELD_KEYS:
            self.assertIn(f"missing key '{key}'", msg)  # each named
        # The substitute key is parsed (it rides the probed mapping's repr)
        # but is NOT one of the nine gated keys — the schema's
        # `eval-commit:` gate reads the schema's key, never the prose's.
        self.assertNotIn("missing key 'last-eval-commit'", msg)

    def test_fail_eval_commit_mismatch_names_both_values(self):
        # FAIL path, the T306 probed-equality leg forwarded: the conformant
        # fixture + a WRONG probed hash fails naming BOTH the probed field
        # value (what the on-disk bytes hold) and the supplied actual (the
        # T270 reporting requirement — never a bare constant mismatch).
        with tempfile.TemporaryDirectory() as tmp:
            path = _write(
                tmp,
                "eval-state.md",
                _vs_text(fields=_vs_fields_with_eval_commit(T306_TRUE_HASH)),
            )
            with self.assertRaises(AssertionError) as ctx:
                wrap_assert.assert_state_on_disk(path, T306_FABRICATED_HASH)
        msg = str(ctx.exception)
        self.assertIn(T306_TRUE_HASH, msg)  # the probed field value
        self.assertIn(T306_FABRICATED_HASH, msg)  # the supplied actual
        self.assertIn("eval-commit", msg)  # the surface named

    def test_fail_missing_file_names_probed_path_and_os_error(self):
        # FAIL path, the missing state: a nonexistent path raises
        # AssertionError (never an uncaught OSError) naming the probed path
        # and the OS error — a missing state can never wave the fill's
        # state-write claim GREEN.
        with tempfile.TemporaryDirectory() as tmp:
            missing = os.path.join(tmp, "absent-eval-state.md")
            with self.assertRaises(AssertionError) as ctx:
                wrap_assert.assert_state_on_disk(missing)
        msg = str(ctx.exception)
        self.assertIn("absent-eval-state.md", msg)  # the probed path
        self.assertIn("No such file or directory", msg)  # the OS error
        self.assertIn("assert_state_on_disk", msg)  # the leg named

    def test_fail_non_str_path_raises_type_error_naming_the_shape(self):
        # The interface defense: *path* is the ON-DISK state file's PATH (a
        # str — the bytes are read HERE); anything else is the
        # call-signature slip shape and TypeErrors naming the accepted
        # shape (the module's convention).
        with self.assertRaises(TypeError) as ctx:
            wrap_assert.assert_state_on_disk(7)
        msg = str(ctx.exception)
        self.assertIn("PATH", msg)
        self.assertIn("a str", msg)
        self.assertIn("type int", msg)

    def test_fail_composed_text_slips_into_the_unreadable_fire(self):
        # The composed-TEXT slip shape (the d1791921121-12 OSError shape,
        # re-pinned on this leg's surface): a str IS the accepted type for
        # *path*, so the composed state TEXT walks as a PATH — and fails
        # CLOSED at the unreadable-file fire naming the OS error (`File
        # name too long`), never a silent pass and never a bare traceback.
        # The composed text is never this leg's argument: the facade owns
        # the TEXT surface, this leg owns the ON-DISK surface.
        with self.assertRaises(AssertionError) as ctx:
            wrap_assert.assert_state_on_disk(_vs_text())
        msg = str(ctx.exception)
        self.assertIn("unreadable", msg)
        self.assertIn("File name too long", msg)
        self.assertIn("assert_state_on_disk", msg)

    def test_fail_non_str_expected_eval_commit_propagates_the_facade_error(self):
        # The keyword's interface defense propagates from the facade (the
        # T306 shape): the probed hash is a STR — a non-str TypeErrors
        # naming the accepted shape.
        with tempfile.TemporaryDirectory() as tmp:
            path = _write(tmp, "eval-state.md", _vs_text())
            with self.assertRaises(TypeError) as ctx:
                wrap_assert.assert_state_on_disk(path, 3930452)
        msg = str(ctx.exception)
        self.assertIn("expected_eval_commit", msg)
        self.assertIn("PROBED full hash", msg)
        self.assertIn("type int", msg)

    def test_fail_unreadable_file_fails_closed_naming_the_error(self):
        # A directory path is unreadable as a state file: the leg fails
        # closed naming the probed path and the OS error (never a bare
        # traceback) — the same fire shape as the missing file.
        with tempfile.TemporaryDirectory() as tmp:
            with self.assertRaises(AssertionError) as ctx:
                wrap_assert.assert_state_on_disk(tmp)
        msg = str(ctx.exception)
        self.assertIn("unreadable", msg)
        self.assertIn("IsADirectoryError", msg)

    def test_on_disk_read_is_a_real_disk_read_not_a_remembered_draft(self):
        # The class's whole point, pinned: the gate runs over the COMMITTED
        # bytes read fresh at call time — the same path asserting GREEN
        # before a later overwrite and RED after it. A GREEN claim composed
        # over a draft a later leg overwrote is exactly the 2515 shape.
        with tempfile.TemporaryDirectory() as tmp:
            path = _write(tmp, "eval-state.md", _vs_text())
            wrap_assert.assert_state_on_disk(path)  # GREEN on the good bytes
            with open(path, "w", encoding="utf-8") as f:
                f.write(T311_PROSE_STATE)  # a later leg overwrites the draft
            with self.assertRaises(AssertionError) as ctx:
                wrap_assert.assert_state_on_disk(path)  # RED on the new bytes
        msg = str(ctx.exception)
        self.assertIn("missing key 'schema'", msg)

    def test_fail_fire_is_read_only_zero_mutation(self):
        # The fire is designed pre-commit: the file is only ever READ — the
        # tempdir and the state's bytes are unchanged through the raise.
        with tempfile.TemporaryDirectory() as tmp:
            path = _write(tmp, "eval-state.md", T311_PROSE_STATE)
            with open(path, "rb") as f:
                before_bytes = f.read()
            before = sorted(os.listdir(tmp))
            with self.assertRaises(AssertionError):
                wrap_assert.assert_state_on_disk(path)
            self.assertEqual(sorted(os.listdir(tmp)), before)
            with open(path, "rb") as f:
                self.assertEqual(f.read(), before_bytes)


if __name__ == "__main__":
    unittest.main()
