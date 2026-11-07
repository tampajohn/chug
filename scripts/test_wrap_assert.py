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

T312 makes the marker leg ALWAYS-ON at the on-disk surface (the T306
composed-marker class's second fire, the cycle-2704 wrap): the 2704
state recompose wrote `eval-commit: 2f905d7e8c5b75eb1c2d4b2a3f0c5d6e7a8
b9c0d` — the 7-char prefix TRUE, the 33-hex suffix memory-composed (the
TRUE full hash 2f905d7f8cfcafbeff2894be019b2488b4cefc02) — and the T311
run green-lit it with the T306 expected_eval_commit leg INERT (the call
site supplied no keyword, the run validating schema-shape only); the
2708 boot's delta build caught it fail-closed (FULL-READ REQUIRED
marker-unreachable, zero casualty — a trip boot full-reads regardless),
the opt-in remedy's insufficiency proven. assert_state_on_disk now OWNS
the probe: after the facade run it self-probes `git log -1
--format=%H -- EVALUATION.md` in the repo cwd and asserts the parsed
field EQUALS the probed full hash — ALWAYS-ON, the keyword demoted to an
OVERRIDE (asserted against instead of the self-probe; the probe is not
called under an override), the `probe_marker` injectable keeping the leg
testable, a probe FAILURE surfacing AssertionError naming the probe and
the OS error (never a bare traceback). Legs: the keyword-less PASS rides
the test's OWN dynamic probe (`git log -1 --format=%H -- EVALUATION.md`
at the repo root, probed at test time, NEVER hardcoded); the same
fixture passes with the override (and an injected raising probe is
never called under it); the 2704 shape (the probed hash's own 7-char
prefix + 33 invented hex chars) FAILS naming BOTH the field value and
the probed actual; the semantic-drift shape (a DIFFERENT reachable
commit — the repo's root commit) FAILS the same way, reachability alone
not being the contract; an injected OSError probe FAILS closed naming
the probe; a non-callable probe_marker TypeErrors naming the shape; the
RED proof rides the in-suite mutant (the fire stashed on a COPY — the
fabricated fixture passes the keyword-less call, the 2704 GREEN again —
while the shipped module fires; the module byte-identical after). The
two T311-era keyword-less fixtures now carry the probed marker (the
always-on leg's contract) — their assertions unchanged.

T316 makes the Outcomes one-liner's five figure groups auditable at
compose time (the corridor-figure falsification class's second committed
sighting, the cycle-2947 wrap): the 2943 wrap's compaction of cycle 2919
wrote its one-liner with the 2923 entry's figures — quiet-TRIP 43→44
(TRUE 42→43), net →530 (TRUE 529), the 666th EXACT close [2903 stream
51/1,403] (TRUE the 665th/2899 stream 80/1,182), the 669th establishment
[2915 stream 108/2,095 census 39→40] (TRUE the 668th/2911 stream
122/1,333 census 38→39), re-arm 598th (TRUE 597th) — every existing guard
passed it, and the compacted line rode the tree FALSE for 4 cycles
(2943→2947) until the 2947 wrap's own ad-hoc one-liner audit caught it
(corrected forward per T70 in c600bc5); the class's own watch
pre-authorized this row (a second committed sighting weighs a guard row).
assert_outcomes_one_liner(full_text, one_liner_text) compares the SAME
five groups (the quiet-TRIP/zero-row pair, the second-net figure, the
backstop close, the establishment, the re-arm ordinal) between the
committed FULL entry and the one-liner replacing it — both probed strs
passed in (TEXT-not-path, no subprocess, no file IO), all group problems
reporting in ONE AssertionError naming the group + the full-entry value +
the one-liner value, extraction failures fail-closed (a group that cannot
be extracted from EITHER text fires the named failure — a silent skip
would re-create the vacuous-pass class). Legs: the GREEN pair rides the
cycle-2919 template shape (the corrected one-liner's exact figures); the
holds/RESETS pair and the ASCII-arrow paren-form pair ride the corpus's
other committed shapes (the 2951-era `HOLDS`/`RESETS` renderings, the
2798-era `->` arrows and `close EXACT (` close); a prose-only perturbation
(wrap-boilerplate digits) stays GREEN — the leg audits FIGURES, not prose;
each of the five groups RED-proved with ITS OWN falsified one-liner (the
eb7dd7a figures) firing the named group + both values; the full eb7dd7a
one-liner fires ALL FIVE in one AssertionError; extraction failures fire
closed (an establishment slice surgically removed, the census-less
2915-era one-liner rendering, a close-anchor-less 2907-era full — each
naming the side); non-str arguments TypeError naming the accepted shape;
the export + docstring coverage sentence pinned; the RED proof rides the
in-suite mutant (the fire stashed on a COPY — the falsified pair passes
the stash — while the shipped module fires; the module byte-identical
after). The eval-body T184 mis-quote subclass stays OUT of scope (the
state file's `fresh-input-last-eval:` line backstops that surface per
T282/T305).

Run (the check line): python3 -m unittest scripts/test_wrap_assert.py -v
"""

import ast
import inspect
import json
import os
import subprocess
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
        # T312: the marker leg is ALWAYS-ON (the keyword is no longer the
        # leg's enabler), so the keyword-less fixture's `eval-commit:`
        # marker carries the test's OWN probe of `git log -1 --format=%H
        # -- EVALUATION.md` — probed dynamically at test time, never
        # hardcoded — the self-probe the leg now owns matches it.
        with tempfile.TemporaryDirectory() as tmp:
            path = _write(
                tmp,
                "eval-state.md",
                _t312_state_with_marker(_t312_probe_marker()),
            )
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
        # T312: the GREEN call rides keyword-less through the ALWAYS-ON
        # marker leg, so the good bytes carry the test's OWN probed marker
        # (the self-probe the leg owns matches it).
        with tempfile.TemporaryDirectory() as tmp:
            path = _write(
                tmp,
                "eval-state.md",
                _t312_state_with_marker(_t312_probe_marker()),
            )
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


# ---- T312: the ALWAYS-ON eval-commit marker leg (assert_state_on_disk) ----

def _t312_repo_root():
    """The repo root — this test file lives at <root>/scripts/."""
    return os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def _t312_probe_marker():
    """The test's OWN probe of the eval-commit marker: `git log -1
    --format=%H -- EVALUATION.md` at the repo root, probed dynamically at
    test time — NEVER hardcoded (the T270 probe-then-assert discipline: the
    expectation is read back from the live repo, never remembered)."""
    proc = subprocess.run(
        ["git", "log", "-1", "--format=%H", "--", "EVALUATION.md"],
        cwd=_t312_repo_root(), capture_output=True, text=True,
    )
    probed = proc.stdout.strip()
    if proc.returncode != 0 or len(probed) != 40:
        raise AssertionError(
            f"the test's own marker probe failed — `git log -1 "
            f"--format=%H -- EVALUATION.md` rc {proc.returncode} probed "
            f"{probed!r} ({proc.stderr.strip()})"
        )
    return probed


def _t312_invented_suffix(probed):
    """33 invented hex chars GUARANTEED different from the true suffix:
    the per-char hex complement (every position maps to a different hex
    digit) — the 2704 shape's memory-composed suffix without any luck
    dependence."""
    return "".join("0" if c != "0" else "1" for c in probed[7:])


def _t312_state_with_marker(marker):
    """The schema-conformant fixture (_vs_text) with the `eval-commit:`
    marker swapped to *marker* (the _vs_fields_with_eval_commit shape)."""
    return _vs_text(fields=_vs_fields_with_eval_commit(marker))


class AssertStateOnDiskMarkerLegTest(unittest.TestCase):
    """T312: the ALWAYS-ON marker leg — the on-disk surface OWNS the probe
    (the T306 composed-marker class's second fire, the cycle-2704 wrap's
    fabricated-suffix marker green-lit by the T311 run with the opt-in leg
    inert; the expected_eval_commit keyword demoted to an OVERRIDE, the leg
    self-probing by default so no call site can leave it off)."""

    def test_pass_marker_equals_the_test_own_probe_no_keyword(self):
        # PASS path, no keyword: the schema-conformant fixture whose marker
        # EQUALS the test's OWN probe of `git log -1 --format=%H --
        # EVALUATION.md` (probed dynamically at test time, never hardcoded)
        # rides the ALWAYS-ON leg green — the self-probe the leg owns
        # matches it, no keyword required.
        probed = _t312_probe_marker()
        with tempfile.TemporaryDirectory() as tmp:
            path = _write(
                tmp, "eval-state.md", _t312_state_with_marker(probed)
            )
            wrap_assert.assert_state_on_disk(path)  # no keyword — self-probe

    def test_pass_override_equals_the_probed_marker_replaces_the_self_probe(
        self,
    ):
        # PASS path (override): the same fixture passes with
        # expected_eval_commit=<the same probed value> — the OVERRIDE is
        # asserted against INSTEAD of the self-probe, so an injected
        # raising probe_marker is never even called under it (the override
        # REPLACES the probe, it does not add a leg).
        probed = _t312_probe_marker()
        with tempfile.TemporaryDirectory() as tmp:
            path = _write(
                tmp, "eval-state.md", _t312_state_with_marker(probed)
            )
            wrap_assert.assert_state_on_disk(path, probed)

            def _raiser():
                raise OSError("the self-probe must not run under an override")

            wrap_assert.assert_state_on_disk(path, probed, probe_marker=_raiser)

    def test_fail_fabricated_suffix_2704_shape_names_both_values(self):
        # FAIL path, the 2704 shape exactly: the fixture's marker replaced
        # by its own 7-char prefix + 33 invented hex chars (the prefix TRUE,
        # the suffix memory-composed — the cycle-2704 wrap's composition)
        # raises AssertionError naming BOTH the field value and the probed
        # actual (the T270 reporting requirement), with the fire, the owned
        # probe, and the doctrine named. RED-proven by the stash leg below.
        probed = _t312_probe_marker()
        fabricated = probed[:7] + _t312_invented_suffix(probed)
        self.assertEqual(len(fabricated), 40)
        self.assertNotEqual(fabricated, probed)
        with tempfile.TemporaryDirectory() as tmp:
            path = _write(
                tmp, "eval-state.md", _t312_state_with_marker(fabricated)
            )
            with self.assertRaises(AssertionError) as ctx:
                wrap_assert.assert_state_on_disk(path)  # no keyword
        msg = str(ctx.exception)
        self.assertIn(fabricated, msg)  # BOTH values: the probed field value
        self.assertIn(probed, msg)  # ...and the probed actual
        self.assertIn("eval-commit", msg)  # the surface named
        self.assertIn("marker", msg)  # the ALWAYS-ON leg named
        self.assertIn("2704", msg)  # the class's second fire named
        self.assertIn("reachability alone is not the contract", msg)

    def test_fail_semantic_drift_reachable_commit_names_both_values(self):
        # FAIL path, semantic drift: the fixture's marker replaced by a
        # DIFFERENT REACHABLE commit (the repo's root commit — a real,
        # resolvable commit; any 40-hex != the probe satisfies the shape)
        # raises AssertionError — reachability alone is NOT the contract,
        # the field must equal THE probed commit, not merely a commit.
        probed = _t312_probe_marker()
        listing = subprocess.run(
            ["git", "rev-list", "--max-parents=0", "HEAD"],
            cwd=_t312_repo_root(), capture_output=True, text=True,
        )
        self.assertEqual(listing.returncode, 0, listing.stderr)
        drift = listing.stdout.split()[0]
        self.assertEqual(len(drift), 40)
        self.assertNotEqual(
            drift,
            probed,
            "the root commit IS the EVALUATION.md probe — pick another "
            "reachable commit for the drift fixture",
        )
        with tempfile.TemporaryDirectory() as tmp:
            path = _write(
                tmp, "eval-state.md", _t312_state_with_marker(drift)
            )
            with self.assertRaises(AssertionError) as ctx:
                wrap_assert.assert_state_on_disk(path)  # no keyword
        msg = str(ctx.exception)
        self.assertIn(drift, msg)  # BOTH values: the reachable field value
        self.assertIn(probed, msg)  # ...and the probed actual
        self.assertIn("reachability alone is not the contract", msg)

    def test_fail_probe_failure_names_the_probe_and_os_error(self):
        # FAIL path, probe failure: an injected probe_marker raising
        # OSError surfaces AssertionError naming the probe (the owned
        # command) and the OS error — never a bare traceback — even on an
        # otherwise-matching state (a failed probe fails CLOSED).
        probed = _t312_probe_marker()
        with tempfile.TemporaryDirectory() as tmp:
            path = _write(
                tmp, "eval-state.md", _t312_state_with_marker(probed)
            )

            def _raiser():
                raise OSError("git spawn refused (the injected probe)")

            with self.assertRaises(AssertionError) as ctx:
                wrap_assert.assert_state_on_disk(path, probe_marker=_raiser)
        msg = str(ctx.exception)
        self.assertIn("probe", msg)
        self.assertIn("git log -1 --format=%H -- EVALUATION.md", msg)
        self.assertIn("OSError", msg)
        self.assertIn("git spawn refused (the injected probe)", msg)
        self.assertIn("fails closed", msg)

    def test_fail_non_callable_probe_marker_raises_type_error(self):
        # The injectable's interface defense: probe_marker is a CALLABLE
        # returning the 40-char hash — anything else is the call-signature
        # slip shape and TypeErrors naming the shape (the module's
        # convention), never a bare `'int' object is not callable`.
        with self.assertRaises(TypeError) as ctx:
            wrap_assert.assert_state_on_disk(
                "unused-state-path.md", probe_marker=7
            )
        msg = str(ctx.exception)
        self.assertIn("probe_marker", msg)
        self.assertIn("callable", msg)
        self.assertIn("type int", msg)

    def test_red_proof_leg_stashed_the_fabricated_fixture_passes(self):
        # RED-provability, demonstrated by stashing the leg — the T310
        # mutant idiom (the fire stashed on an in-memory COPY; the module
        # itself never written, re-read byte-identical after): with the
        # marker fire stashed, the 2704-shape fabricated fixture PASSES the
        # keyword-less call (today's pre-T312 surface — the T306 leg inert
        # by contract, the GREEN the second fire rode); with the leg live
        # (the shipped module) the same fixture fires.
        with open(wrap_assert.__file__, "r", encoding="utf-8") as f:
            pristine = f.read()
        probed = _t312_probe_marker()
        fabricated = probed[:7] + _t312_invented_suffix(probed)
        with tempfile.TemporaryDirectory() as tmp:
            path = _write(
                tmp, "eval-state.md", _t312_state_with_marker(fabricated)
            )
            # The shipped module fires (the leg above pins it; re-asserted
            # here so the mutant contrast is self-contained).
            with self.assertRaises(AssertionError):
                wrap_assert.assert_state_on_disk(path)
            # The stash: the fire line swapped for `pass` on a copy — the
            # keyword-less call now PASSES (RED proven: the FAIL-path test
            # above goes red without the leg).
            stash_line = (
                "        _fire_marker_mismatch(path, field_value, "
                "expected_eval_commit)"
            )
            self.assertEqual(pristine.count(stash_line), 1)
            stash = _mutant_wrap_assert(pristine, stash_line, "        pass")
            stash["assert_state_on_disk"](path)  # no raise under the stash
        # The module restored byte-identical after the run (nothing was
        # ever written — re-read and compared against the pristine read).
        with open(wrap_assert.__file__, "r", encoding="utf-8") as f:
            self.assertEqual(f.read(), pristine)


# ---- T316: the Outcomes one-liner figure-audit legs (assert_outcomes_one_liner) ----

T316_FULL_2919 = (
    "### Cycle 2919 (2026-11-04) — trip-690 eval (kimi; the T247 valve's "
    "FIRST-attempt trip at TRUE 3 on an ALL-supervisor cheap-exit chain "
    "[2916 757ba63 / 2917 a4107d2 / 2918 726b222, all base c2d54d9 — "
    "grep-verified 0-files 3/3 at eval-write]): ZERO rows filed (the "
    "zero-row count moves 656/698 → 657/699, the queue EMPTY), 0 items "
    "worked; eval 259dd99 + the eval's push carried the 3 unpushed "
    "supervisor cheap-exit dispositions 2916-2918 = 757ba63/a4107d2/"
    "726b222 along. ZERO eval-triage records (the T261 change-detection "
    "trigger QUIET — the quiet trip: the bookkeeping-only window since the "
    "marker c2d54d9 [4 commits, all EMPTY by construction], every "
    "candidate's disposition unchanged; the fully-quiet-TRIP streak moves "
    "42 → 43); the second net re-derived THIS wrap exact → 529 = the 2915 "
    "wrap's 528 + the 665th EXACT close's increment; the T261 "
    "change-detection anchor the newest EVALUATION-md-touching commit "
    "259dd99 [THIS eval]. The eval: the 665th backstop closes EXACT on the "
    "carried-TRUE figure — reading the 2899 stream's whole-stream final "
    "80/1,182 [re-derivation off events-20261104-005801 per scripts/"
    "stream_datum.py CONCURS: iters=80, wall-secs 1181.763 → 1,182 "
    "[ROUND-anchor], ok:false 7, ctx 1/0, trim 0, goal 1] == the 665th "
    "establishment's committed figure [the 2907 eval, commit ba470da — "
    "re-grepped off the committed text at eval-write, count-3], NO carried "
    "tension; correct-accounting 478 → 479; INEXACT-close HOLDS 8; the "
    "quiet top HOLDS 139; the 666th backstop hands off [the 2903 stream "
    "51/1,403 — the 666th establishment's committed figure, the 2911 eval "
    "commit af0be77, events-20261103-234510 per the rotation mechanism — "
    "NO carried tension]. The 668th establishment — reading the 2911 "
    "stream's whole-stream final 122/1,333 [re-derivation off "
    "events-20261104-032712 per scripts/stream_datum.py CONCURS: iters=122, "
    "wall-secs 1332.579 → 1,333 [ROUND-anchor]]; ok:false 4 == 4 [the "
    "fill's committed FOUR byte-exact, ZERO post-stamp tail legs]; "
    "ctx_edit 1 accepted / 0 rejected [ZERO post-stamp ctx tail legs]; "
    "trim 0 == 0 [the TRUE streak reading at the 668th: 14 → 15]; goal 1 "
    "[accepted 2026-11-04T03:03:42.153Z]; the digest arithmetic CONCURS: "
    "104,324+108==104,432 EXACT; files 1,545 measured [ls-probed at "
    "eval-write]; the 102-bound FIRES [122 >= the bound — the census moves "
    "38 → 39, the non-fire streak BREAKS 14 → 0 [the first fire in "
    "fourteen readings]]; the quiet top HOLDS 139; the 669th establishment "
    "hands off [the 2915 stream's own final — re-derived THIS eval for the "
    "digest CONCURS: 108/2,095]. re-arm 597th DISCHARGED EXACT — reading "
    "the 2850 stream's whole-final 65/1,663 [the 651st establishment's "
    "committed figure [the 2854 eval, commit 6adb7a0]]; the pairing "
    "(597th, 699) lands on THIS the 699th evaluation EXACT [597 + 102 == "
    "699]; the re-arm spot-watch discharges its two-hundred-and-"
    "seventy-third; (b) NEGATIVE — 65 < the 102 bound [the watch HOLDS "
    "58]; the quiet top HOLDS 139 [65 < 139]; 1 rides [the TRUE count "
    "598th through the lattice's 598th [the 2858 slot = the 2858 stream "
    "103/2,005 committed 0af2f05 — (b) will be POSITIVE: 103 > the 102 "
    "bound, the watch TRIPS 58→59 at the 701st evaluation's discharge]]; "
    "the 597th hands off. The wrap: gates GREEN docs-only (batteries "
    "239/239 ×2), batch TWO (1,029th/1,030th), audit zero-growth, NO TAG "
    "(TRUE 1 = T315), fill 3e3ec30 pushed, main == origin/main. Handoff: "
    "the 666th backstop and the 669th establishment hand off; re-arm "
    "598th DUE reading the 2858 stream's 103/2,005 — (b) will be "
    "POSITIVE; the 700th evaluation is the next backstop close's slot."
)


def _t316_one_liner(
    quiet="42→43",
    net="529",
    close="the 665th EXACT close [2899 stream 80/1,182]",
    establishment=(
        "the 668th establishment [2911 stream 122/1,333, the 102-bound "
        "FIRES census 38→39]"
    ),
    rearm="re-arm 597th discharged EXACT",
):
    """The cycle-2919 one-liner template (the corrected c600bc5 figures).

    Each figure-group slice is a keyword — the RED fixtures swap ONE
    group's slice for the eb7dd7a falsification's figures.
    """
    return (
        "### Cycle 2919 (2026-11-04) — trip-690 eval 259dd99 (kimi, ZERO "
        "rows 656/698 → 657/699, ZERO triage, fully-quiet-TRIP " + quiet
        + ", second net → " + net + ", " + close + " + " + establishment
        + " + " + rearm + " [(b) NEGATIVE watch HOLDS 58]) + wrap landed: "
        "gates GREEN docs-only (batteries 239/239 ×2 [eval pid 76147 "
        "23.058s + prep pid 5074 22.720s] + inherited full gates at "
        "8a51ada), batch TWO (1,029th/1,030th, guard TRUE 678/678/0 the "
        "179th), audit zero-growth, NO TAG (TRUE 1 = T315), fill 3e3ec30 "
        "pushed, main == origin/main."
    )


T316_ONE_LINER_2919 = _t316_one_liner()

# The full eb7dd7a falsification: every figure group duplicated from the
# 2923 entry (the class's canonical shape — rode the tree FALSE 2943→2947).
T316_ONE_LINER_FALSIFIED = _t316_one_liner(
    quiet="43→44",
    net="530",
    close="the 666th EXACT close [2903 stream 51/1,403]",
    establishment=(
        "the 669th establishment [2915 stream 108/2,095, the 102-bound "
        "FIRES census 39→40]"
    ),
    rearm="re-arm 598th discharged EXACT",
)

# The T318 m6 survivor's killing fixture (the T316 validator
# d1793798848-4's second survivor, LOW impact: group 1's RED fixtures
# perturbed only the quiet-TRIP sub-figure, never the zero-row
# `A/B → C/D` pair). The one-liner agrees on quiet-TRIP (42→43) and on
# the zero-row FIRST fraction (656/698) and perturbs the SECOND fraction
# only (→ 658/700 where the full entry carries → 657/699) — the C/D-only
# falsification the validator verified the SHIPPED leg fires on
# (AssertionError naming the 'quiet-TRIP/zero-row pair' group). The m6
# MUTANT SHAPE it kills: the comparison with the zero-row second
# fraction dropped — the `zero_row = zero_row + (frac2[0], frac2[1])`
# extension in _ol_pair_figures deleted, both sides read as the bare
# `A/B` 2-tuple — under which the fixture compares (656, 698) ==
# (656, 698) and stays SILENT (the pre-T318 suite gap: nothing pinned
# the pair's second fraction).
T316_ONE_LINER_ZERO_ROW_SECOND_FALSIFIED = T316_ONE_LINER_2919.replace(
    "ZERO rows 656/698 → 657/699", "ZERO rows 656/698 → 658/700", 1
)

# The T318 m1 survivor's killing fixture (the T316 validator
# d1793798848-4's first survivor, LOW impact: no committed fixture
# exercised the ordinal-adjacency contract). The FULL entry's close
# region carries the ordinal-adjacency VIOLATION the shipped leg rejects
# — the handoff's forward announcement `the 611th backstop closes the
# 2692 stream at the NEXT real eval` (a `backstop closes` occurrence
# WITHOUT the EXACT adjacency — the NEXT close, the wrong ordinal AND
# the wrong stream, the 2704-era trap `_ol_close_anchor`'s docstring
# names) riding BEFORE the own `the 610th close EXACT` — and the
# falsified one-liner writes the close group with the trap's stale-but-
# in-window values (the 611th/2692 stream 61/1,318 — present in the full
# entry's close window, UNEQUAL to the true own 610th/2688 stream
# 84/1,251, the corridor class's exact divergence shape). The other four
# figure groups agree, so the fire is the close group ALONE with BOTH
# ordinals probed. The m1 MUTANT SHAPE it kills: the adjacency check
# dropped — the close anchor's first candidate scanned as bare
# `backstop closes` (shipped demands `backstop closes EXACT`) — under
# which the scan anchors the full side on the VIOLATING trap, reads
# (611, 2692, 61, 1318) == the one-liner's falsified close, and stays
# SILENT. The validator's residual pathological case (a stale ordinal
# EQUAL to the true own ordinal) stays out of scope per its own impact
# assessment.
T316_FULL_CLOSE_ADJACENCY_TRAP = (
    "### Cycle 2704 (2026-10-27) — trip-475 eval (kimi; the T247 valve's "
    "FIRST-attempt trip at TRUE 2 on a mixed chain): ZERO rows filed (the "
    "zero-row count moves 650/692 → 651/693, the queue EMPTY), 0 items "
    "worked; eval 7c4d2e9 + the eval's push carried the 2 unpushed "
    "supervisor dispositions 2702-2703 = b81a6f4/09c3e57 along; the 611th "
    "backstop closes the 2692 stream at the NEXT real eval [the rotation's "
    "forward slot — the 2692 stream's own final re-derived THIS eval "
    "CONCURS: 61/1,318 — the 611th establishment's committed figure, the "
    "2696 eval commit 5ec41a7, events-20261027-190412 per the rotation "
    "mechanism — NO carried tension]. ZERO eval-triage records (the T261 "
    "change-detection trigger QUIET — the fully-quiet-TRIP streak moves "
    "38 → 39); the second net re-derived THIS wrap exact → 521 = the 2700 "
    "wrap's 520 + the 610th EXACT close's increment; the T261 "
    "change-detection anchor the newest EVALUATION-md-touching commit "
    "7c4d2e9 [THIS eval]. The eval: the 610th close EXACT on the "
    "carried-TRUE figure — reading the 2688 stream's whole-stream final "
    "84/1,251 [re-derivation off events-20261027-171233 per scripts/"
    "stream_datum.py CONCURS: iters=84, wall-secs 1250.881 → 1,251 "
    "[ROUND-anchor], ok:false 5, ctx 1/0, trim 0, goal 1] == the 610th "
    "establishment's committed figure [the 2692 eval, commit 19bb40c — "
    "re-grepped off the committed text at eval-write, count-3], NO carried "
    "tension; correct-accounting 331 → 332; INEXACT-close HOLDS 8; the "
    "quiet top HOLDS 96. The 667th establishment — reading the 2696 "
    "stream's whole-stream final 96/1,478 [re-derivation off "
    "events-20261027-204455 per scripts/stream_datum.py CONCURS: iters=96, "
    "wall-secs 1477.412 → 1,478 [ROUND-anchor]]; ok:false 3 == 3; the "
    "102-bound NON-FIRES [96 < the bound — the census HOLDS 37, the "
    "non-fire streak 2 → 3]; the quiet top HOLDS 96; the 668th "
    "establishment hands off [the 2700 stream's own final]. re-arm 542nd "
    "DISCHARGED EXACT — reading the 2806 stream's whole-final 71/1,590 "
    "[the 531st establishment's committed figure [the 2810 eval, commit "
    "aa17e02]]; (b) NEGATIVE — 71 < the 102 bound [the watch HOLDS 49]; "
    "the quiet top HOLDS 96 [71 < 96]; the 542nd hands off. The wrap: "
    "gates GREEN docs-only (batteries 210/210 ×2), batch ONE (914th), "
    "audit zero-growth, NO TAG (TRUE 2 = T88 + T91), fill 40b9a17 pushed, "
    "main == origin/main. Handoff: the 611th backstop and the 668th "
    "establishment hand off; re-arm 543rd DUE reading the 2810 stream's "
    "own final."
)

T316_ONE_LINER_CLOSE_ADJACENCY_FALSIFIED = (
    "### Cycle 2704 (2026-10-27) — trip-475 eval 7c4d2e9 (kimi, ZERO rows "
    "650/692 → 651/693, ZERO triage, fully-quiet-TRIP 38→39, second net → "
    "521, the 611th EXACT close [2692 stream 61/1,318] + the 667th "
    "establishment [2696 stream 96/1,478, the 102-bound NON-FIRES census "
    "HOLDS 37] + re-arm 542nd discharged EXACT [(b) NEGATIVE watch HOLDS "
    "49]) + wrap landed: gates GREEN docs-only (batteries 210/210 ×2), "
    "audit zero-growth, NO TAG (TRUE 2 = T88 + T91), fill 40b9a17 pushed, "
    "main == origin/main."
)

T316_FULL_HOLDS = (
    "### Cycle 2951 (2026-11-04) — trip-694 eval (kimi; the T247 valve's "
    "FIRST-attempt trip at TRUE 1 on a bookkeeping window): ZERO rows "
    "filed (the zero-row count HOLDS 664/707, the queue EMPTY), 0 items "
    "worked. ZERO eval-triage records (the T261 change-detection trigger "
    "QUIET — the fully-quiet-TRIP streak RESETS 50 → 0; the second net "
    "HOLDS 537). The eval: the 673rd backstop closes EXACT (2931 stream "
    "80/1,678 re-derived CONCURS off events-20261104-080617 == the 673rd "
    "establishment's committed figure [the 704th eval 99565f6 re-grepped "
    "count-3]; the 2931 fill datum FINAL [76/~1,625.250, 9c7aea8] a "
    "strict PREFIX CONSISTENT [76 <= 80, +4]; ok:false 4==4 byte-exact, "
    "ctx 1/1, trim 0, goal 1; correct-accounting 486→487, INEXACT-close "
    "HOLDS 8); the 674th backstop hands off [the 2935 stream 91/1,958]. "
    "The 676th establishment (2943 stream 87/1,556 re-derived CONCURS off "
    "events-20261104-104607 — byte-identical to the 706th eval's "
    "re-derivation; the 102-bound NON-FIRES [census HOLDS 41, non-fire "
    "streak 5→6], trim-0 [TRUE streak 0→1 as projected], ok:false 2==2 "
    "byte-exact, ctx 1/0, digest CONCURS 105,074+97==105,171 + files "
    "1,553 measured); the 677th establishment hands off [the 2947 "
    "stream's own final]. re-arm 605th DISCHARGED EXACT (2882 stream "
    "100/1,910, (b) NEGATIVE — 100 < the 102 bound [the watch HOLDS 59], "
    "the 278th discharge); the 605th hands off. Handoff: the 674th "
    "backstop and the 677th establishment hand off; re-arm 606th DUE "
    "reading the 2886 stream's 98/1,972."
)

T316_ONE_LINER_HOLDS = (
    "### Cycle 2951 (2026-11-04) — trip-694 eval 99565f6 (kimi, ZERO rows "
    "664/707, ZERO triage, fully-quiet-TRIP 50→0, second net HOLDS 537, "
    "the 673rd EXACT close [2931 stream 80/1,678] + the 676th "
    "establishment [2943 stream 87/1,556, the 102-bound NON-FIRES census "
    "HOLDS 41] + re-arm 605th discharged EXACT [(b) NEGATIVE watch HOLDS "
    "59]) + wrap landed: gates GREEN docs-only (batteries 239/239 ×2 "
    "[eval pid 991 22.4s + prep pid 992 22.1s]), audit zero-growth, NO "
    "TAG (TRUE 1 = T315), fill pushed, main == origin/main."
)

T316_FULL_ASCII = (
    "### Cycle 2798 (2026-11-02) — trip-660 eval (kimi): ZERO rows filed "
    "(the zero-row count moves 625/667 -> 626/668, the queue EMPTY), 0 "
    "items worked. ZERO eval-triage records (the fully-quiet-TRIP streak "
    "moves 12 -> 13); the second net re-derived THIS wrap exact -> 499 = "
    "the 2794 wrap's 498 + the 634th close's increment. The eval: the "
    "634th backstop closes EXACT — reading the 2782 stream's whole-stream "
    "final 95/1,822 [re-derivation CONCURS: iters=95, wall-secs 1821.4 "
    "-> 1,822 [ROUND-anchor]] == the 634th establishment's committed "
    "figure; correct-accounting 451 -> 452; INEXACT HOLDS 8; the 635th "
    "backstop hands off [the 2786 stream 91/1,824]. The 637th "
    "establishment — reading the 2786 stream's whole-stream final "
    "91/1,824 [re-derivation CONCURS]; ok:false 3 == 3; the 102-bound "
    "NON-FIRES [91 < the bound — the census HOLDS 35, the non-fire "
    "streak moves 6 -> 7]; the 638th establishment hands off. re-arm "
    "566th DISCHARGED EXACT — reading the 2714 stream's whole-final "
    "92/1,876; (b) NEGATIVE — 92 < the 102 bound [the watch HOLDS 50 "
    "fires]; the 566th hands off. Handoff: the 635th backstop and the "
    "638th establishment hand off; re-arm 567th DUE."
)

T316_ONE_LINER_ASCII = (
    "### Cycle 2798 (2026-11-02) — trip-660 eval c0ffee1 (kimi, ZERO rows "
    "625/667 -> 626/668, ZERO triage, fully-quiet-TRIP 12->13, second net "
    "-> 499, the 634th close EXACT (2782 stream 95/1,822; "
    "correct-accounting 451 -> 452, INEXACT HOLDS 8) + the 637th "
    "establishment (2786 stream 91/1,824, the 102-bound NON-FIRES census "
    "HOLDS 35, streak 6 -> 7) + re-arm 566th DISCHARGED (b) NEGATIVE "
    "[watch HOLDS 50 fires]) + wrap landed: gates GREEN docs-only "
    "(batteries 239/239 ×2), audit zero-growth, NO TAG (TRUE 1 = T315), "
    "fill pushed, main == origin/main."
)


class AssertOutcomesOneLinerTest(unittest.TestCase):
    """T316: the five figure groups equal between full entry and one-liner."""

    def test_pass_true_one_liner_matches_full(self):
        # T316 PASS path: the corrected cycle-2919 one-liner (the c600bc5
        # figures) against its committed full entry — all five groups
        # equal, no fire.
        wrap_assert.assert_outcomes_one_liner(
            T316_FULL_2919, T316_ONE_LINER_2919
        )

    def test_pass_holds_and_resets_shapes(self):
        # The 2951-era template renderings: the zero-row count HOLDS (one
        # fraction, no arrow), the quiet-TRIP streak RESETS, the net
        # HOLDS, the paren-form full close/establishment, the census
        # HOLDS — the same five groups, the same numbers, green.
        wrap_assert.assert_outcomes_one_liner(
            T316_FULL_HOLDS, T316_ONE_LINER_HOLDS
        )

    def test_pass_ascii_arrow_paren_close_shapes(self):
        # The 2798-era renderings: ASCII `->` arrows, the `close EXACT (`
        # close shape, `the <id> stream's` possessives, the holds-shaped
        # census trailing a streak move — green.
        wrap_assert.assert_outcomes_one_liner(
            T316_FULL_ASCII, T316_ONE_LINER_ASCII
        )

    def test_pass_prose_only_perturbation_stays_green(self):
        # The leg audits FIGURES, not prose: perturbing the wrap
        # boilerplate's digits and the fill hash leaves all five figure
        # groups untouched — green (an over-matching leg would fire).
        prose_swapped = T316_ONE_LINER_2919.replace(
            "batteries 239/239 ×2", "batteries 238/238 ×2"
        ).replace("fill 3e3ec30", "fill d0d0d0d")
        self.assertNotEqual(prose_swapped, T316_ONE_LINER_2919)
        wrap_assert.assert_outcomes_one_liner(T316_FULL_2919, prose_swapped)

    def test_red_quiet_trip_pair_falsified_names_group_and_values(self):
        # The class's group-1 sighting: quiet-TRIP 43→44 written where the
        # full entry carries 42→43 — fires naming the group and BOTH
        # values.
        falsified = _t316_one_liner(quiet="43→44")
        self.assertNotEqual(falsified, T316_ONE_LINER_2919)
        with self.assertRaises(AssertionError) as caught:
            wrap_assert.assert_outcomes_one_liner(T316_FULL_2919, falsified)
        message = str(caught.exception)
        self.assertIn("'quiet-TRIP/zero-row pair'", message)
        self.assertIn("(42, 43)", message)
        self.assertIn("(43, 44)", message)

    def test_red_second_net_falsified_names_group_and_values(self):
        # Group 2: net →530 written where the full entry carries 529.
        falsified = _t316_one_liner(net="530")
        with self.assertRaises(AssertionError) as caught:
            wrap_assert.assert_outcomes_one_liner(T316_FULL_2919, falsified)
        message = str(caught.exception)
        self.assertIn("'second-net figure'", message)
        self.assertIn("(529,)", message)
        self.assertIn("(530,)", message)

    def test_red_backstop_close_falsified_names_group_and_values(self):
        # Group 3: the 666th/2903-stream close written where the full
        # entry carries the 665th/2899-stream close — the exact
        # eb7dd7a shape.
        falsified = _t316_one_liner(
            close="the 666th EXACT close [2903 stream 51/1,403]"
        )
        with self.assertRaises(AssertionError) as caught:
            wrap_assert.assert_outcomes_one_liner(T316_FULL_2919, falsified)
        message = str(caught.exception)
        self.assertIn("'backstop close'", message)
        self.assertIn("(665, 2899, 80, 1182)", message)
        self.assertIn("(666, 2903, 51, 1403)", message)

    def test_red_establishment_falsified_names_group_and_values(self):
        # Group 4: the 669th/2915-stream establishment with census 39→40
        # written where the full entry carries the 668th/2911-stream with
        # census 38→39 — the exact eb7dd7a shape.
        falsified = _t316_one_liner(
            establishment=(
                "the 669th establishment [2915 stream 108/2,095, the "
                "102-bound FIRES census 39→40]"
            )
        )
        with self.assertRaises(AssertionError) as caught:
            wrap_assert.assert_outcomes_one_liner(T316_FULL_2919, falsified)
        message = str(caught.exception)
        self.assertIn("'establishment'", message)
        self.assertIn("(668, 2911, 122, 1333, 38, 39)", message)
        self.assertIn("(669, 2915, 108, 2095, 39, 40)", message)

    def test_red_rearm_falsified_names_group_and_values(self):
        # Group 5: re-arm 598th written where the full entry carries 597th
        # — the exact eb7dd7a shape.
        falsified = _t316_one_liner(rearm="re-arm 598th discharged EXACT")
        with self.assertRaises(AssertionError) as caught:
            wrap_assert.assert_outcomes_one_liner(T316_FULL_2919, falsified)
        message = str(caught.exception)
        self.assertIn("'re-arm ordinal'", message)
        self.assertIn("(597,)", message)
        self.assertIn("(598,)", message)

    def test_red_zero_row_second_fraction_falsified_names_group_and_values(
        self,
    ):
        # The T318 m6 survivor's killing fixture: the one-liner agrees on
        # the quiet-TRIP sub-figure (42→43) and on the zero-row FIRST
        # fraction (656/698) but perturbs the zero-row SECOND fraction
        # only (→ 658/700 where the full entry carries → 657/699) — the
        # C/D-only falsification the T316 validator verified the shipped
        # leg fires on. The leg must fire naming the 'quiet-TRIP/zero-row
        # pair' group with both probed tuples.
        falsified = T316_ONE_LINER_ZERO_ROW_SECOND_FALSIFIED
        self.assertNotEqual(falsified, T316_ONE_LINER_2919)
        with self.assertRaises(AssertionError) as caught:
            wrap_assert.assert_outcomes_one_liner(T316_FULL_2919, falsified)
        message = str(caught.exception)
        self.assertIn("'quiet-TRIP/zero-row pair'", message)
        self.assertIn("(656, 698, 657, 699)", message)
        self.assertIn("(656, 698, 658, 700)", message)
        # C/D-only: exactly ONE group problem (the other four groups
        # agree — the perturbation never leaves the pair).
        self.assertIn("1 figure-group problem(s)", message)
        # RED provability against the m6 MUTANT SHAPE (run on a COPY, the
        # T310/T316 idiom): the comparison with the zero-row second
        # fraction dropped — the `zero_row = zero_row + (frac2[0],
        # frac2[1])` extension deleted, both sides read as the bare `A/B`
        # 2-tuple — the fixture then compares (656, 698) == (656, 698)
        # and stays SILENT (no raise under the mutant, where the shipped
        # leg raises above). The on-disk module is re-read byte-identical
        # after (the mutation rides an in-memory exec, never a write).
        with open(wrap_assert.__file__, "r", encoding="utf-8") as f:
            pristine = f.read()
        m6 = _mutant_wrap_assert(
            pristine,
            "            zero_row = zero_row + (frac2[0], frac2[1])",
            "            pass",
        )
        m6["assert_outcomes_one_liner"](T316_FULL_2919, falsified)
        with open(wrap_assert.__file__, "r", encoding="utf-8") as f:
            self.assertEqual(f.read(), pristine)

    def test_red_close_adjacency_violation_falsified_names_group_and_ordinals(
        self,
    ):
        # The T318 m1 survivor's killing fixture: the full entry's close
        # region carries the ordinal-adjacency VIOLATION the shipped leg
        # rejects — the handoff's forward announcement `the 611th backstop
        # closes the 2692 stream at the NEXT real eval` (a `backstop
        # closes` occurrence WITHOUT the EXACT adjacency — the NEXT close,
        # the wrong ordinal AND the wrong stream) riding BEFORE the own
        # `the 610th close EXACT` — and the one-liner's close ordinal is a
        # stale-but-in-window value UNEQUAL to the true own ordinal (the
        # 611th/2692 stream written where the full entry's own close is
        # the 610th/2688 — the corridor class's divergence shape). The leg
        # must fire naming the 'backstop close' group with BOTH ordinals.
        with self.assertRaises(AssertionError) as caught:
            wrap_assert.assert_outcomes_one_liner(
                T316_FULL_CLOSE_ADJACENCY_TRAP,
                T316_ONE_LINER_CLOSE_ADJACENCY_FALSIFIED,
            )
        message = str(caught.exception)
        self.assertIn("'backstop close'", message)
        self.assertIn("(610, 2688, 84, 1251)", message)
        self.assertIn("(611, 2692, 61, 1318)", message)
        # Only the close group diverged (the trap hijacks nothing else —
        # the other four figure groups agree).
        self.assertIn("1 figure-group problem(s)", message)
        # RED provability against the m1 MUTANT SHAPE (run on a COPY, the
        # T310/T316 idiom): the adjacency check dropped — the close
        # anchor's first candidate scanned as bare `backstop closes`
        # (shipped demands `backstop closes EXACT`, the 2704-era guard) —
        # the scan then anchors the full side on the VIOLATING trap and
        # reads (611, 2692, 61, 1318) == the one-liner's falsified close:
        # the fixture stays SILENT (no raise under the mutant, where the
        # shipped leg raises above). The on-disk module is re-read
        # byte-identical after (the mutation rides an in-memory exec,
        # never a write).
        with open(wrap_assert.__file__, "r", encoding="utf-8") as f:
            pristine = f.read()
        m1 = _mutant_wrap_assert(
            pristine,
            '_CLOSE_ANCHORS = ("backstop closes EXACT", "EXACT close", '
            '"close EXACT")',
            '_CLOSE_ANCHORS = ("backstop closes", "EXACT close", '
            '"close EXACT")',
        )
        m1["assert_outcomes_one_liner"](
            T316_FULL_CLOSE_ADJACENCY_TRAP,
            T316_ONE_LINER_CLOSE_ADJACENCY_FALSIFIED,
        )  # no raise under the mutant
        with open(wrap_assert.__file__, "r", encoding="utf-8") as f:
            self.assertEqual(f.read(), pristine)

    def test_red_full_falsified_one_liner_names_all_five_in_one_fire(self):
        # The class's canonical regression pin: the eb7dd7a one-liner (all
        # five figure groups duplicated from the 2923 entry) fires ALL
        # FIVE groups in ONE AssertionError (the all-failures-in-one-error
        # convention), each with its full-entry and one-liner values.
        with self.assertRaises(AssertionError) as caught:
            wrap_assert.assert_outcomes_one_liner(
                T316_FULL_2919, T316_ONE_LINER_FALSIFIED
            )
        message = str(caught.exception)
        self.assertIn("5 figure-group problem(s)", message)
        for group in (
            "'quiet-TRIP/zero-row pair'",
            "'second-net figure'",
            "'backstop close'",
            "'establishment'",
            "'re-arm ordinal'",
        ):
            self.assertIn(group, message)
        # Both values probed per group (the reporting requirement).
        for value in (
            "(42, 43)", "(43, 44)",
            "(529,)", "(530,)",
            "(665, 2899, 80, 1182)", "(666, 2903, 51, 1403)",
            "(668, 2911, 122, 1333, 38, 39)",
            "(669, 2915, 108, 2095, 39, 40)",
            "(597,)", "(598,)",
        ):
            self.assertIn(value, message)

    def test_red_missing_group_fail_closed_on_one_liner(self):
        # A one-liner missing a whole group fires the named extraction
        # failure — fail-closed (a silent skip would re-create the
        # vacuous-pass class).
        sliced = T316_ONE_LINER_2919.replace(
            " + the 668th establishment [2911 stream 122/1,333, the "
            "102-bound FIRES census 38→39]",
            "",
            1,
        )
        self.assertNotEqual(sliced, T316_ONE_LINER_2919)
        with self.assertRaises(AssertionError) as caught:
            wrap_assert.assert_outcomes_one_liner(T316_FULL_2919, sliced)
        message = str(caught.exception)
        self.assertIn("'establishment'", message)
        self.assertIn("could not be extracted from the one-liner", message)
        # The other four groups still compared (only the extraction
        # failure reported — the group that CANNOT be extracted is named,
        # the groups that CAN are either equal or named as divergent).
        self.assertNotIn("'backstop close'", message)

    def test_red_census_less_one_liner_fail_closed(self):
        # The 2915-era one-liner rendering dropped the establishment's
        # census figure while the current-template full carries it — the
        # named extraction failure fires (the census move is part of the
        # establishment group).
        census_less = _t316_one_liner(
            establishment="the 668th establishment [2911 stream 122/1,333]"
        )
        with self.assertRaises(AssertionError) as caught:
            wrap_assert.assert_outcomes_one_liner(T316_FULL_2919, census_less)
        message = str(caught.exception)
        self.assertIn("'establishment'", message)
        self.assertIn("no `census` figure", message)

    def test_red_anchor_less_full_entry_fail_closed_on_full_side(self):
        # The 2907-era compaction template carried NO close, establishment
        # or re-arm groups — a full entry lacking them fires the named
        # extraction failures on the FULL side (the side is named, not
        # guessed).
        anchor_less_full = (
            "### Cycle 2907 (2026-11-04) — trip-687 eval ba470da (kimi): "
            "ZERO rows filed (the zero-row count moves 653/695 → 654/696, "
            "the queue EMPTY), 0 items worked. ZERO eval-triage records "
            "(the fully-quiet-TRIP streak moves 39 → 40); the second net "
            "re-derived THIS wrap exact → 526 = the 2903 wrap's 525 + the "
            "663rd EXACT close's increment. The wrap: gates GREEN "
            "docs-only, audit zero-growth, NO TAG, fill pushed. Handoff: "
            "nothing rides."
        )
        with self.assertRaises(AssertionError) as caught:
            wrap_assert.assert_outcomes_one_liner(
                anchor_less_full, T316_ONE_LINER_2919
            )
        message = str(caught.exception)
        for group in ("'backstop close'", "'establishment'", "'re-arm ordinal'"):
            self.assertIn(group, message)
        self.assertIn(
            "the 'backstop close' group could not be extracted from the "
            "full entry", message
        )
        self.assertIn(
            "the 'establishment' group could not be extracted from the "
            "full entry", message
        )
        self.assertIn(
            "the 're-arm ordinal' group could not be extracted from the "
            "full entry", message
        )

    def test_signature_non_str_arguments_type_error(self):
        # The TEXT-not-path convention: both probed values are strs (the
        # section texts passed in — nothing opened, read, or written). A
        # non-str is the call-signature slip shape — TypeError naming the
        # accepted shape and the probed actual.
        for bad_full in (None, 42, b"### Cycle 2919", ["### Cycle 2919"]):
            with self.assertRaises(TypeError) as caught:
                wrap_assert.assert_outcomes_one_liner(
                    bad_full, T316_ONE_LINER_2919
                )
            message = str(caught.exception)
            self.assertIn("full_text", message)
            self.assertIn("TEXT-not-path", message)
        for bad_one in (None, 42, b"### Cycle 2919", ["### Cycle 2919"]):
            with self.assertRaises(TypeError) as caught:
                wrap_assert.assert_outcomes_one_liner(
                    T316_FULL_2919, bad_one
                )
            message = str(caught.exception)
            self.assertIn("one_liner_text", message)
            self.assertIn("TEXT-not-path", message)

    def test_export_and_docstring_coverage_sentence_pinned(self):
        # The leg rides the module's __all__ (16 -> 17) and the docstring
        # coverage sentence names the T316 legs (the pin-surface extended,
        # not re-shaped). The LAST-position pin moved with T321 and again
        # with T323 (the T323 leg now rides last).
        self.assertIn("assert_outcomes_one_liner", wrap_assert.__all__)
        self.assertEqual(len(wrap_assert.__all__), 21)
        self.assertEqual(
            wrap_assert.__all__[-1], "assert_fill_eval_state_untracked"
        )
        self.assertIn(
            "T316 Outcomes one-liner figure-audit legs", wrap_assert.__doc__
        )
        # The extension, not a re-shape: the T312 sentence tail still rides.
        self.assertIn("T312 always-on marker legs", wrap_assert.__doc__)

    def test_red_proof_fire_stashed_on_copy(self):
        # RED provability: the fire stashed on a COPY (the one-line call
        # swapped for `pass`) — the falsified pair then PASSES the leg;
        # the shipped module fires. The module byte-identical after (the
        # mutation rides an in-memory exec, never a write).
        with open(wrap_assert.__file__, "r", encoding="utf-8") as f:
            pristine = f.read()
        # The shipped module fires (the leg above pins it; re-asserted
        # here so the mutant contrast is self-contained).
        with self.assertRaises(AssertionError):
            wrap_assert.assert_outcomes_one_liner(
                T316_FULL_2919, T316_ONE_LINER_FALSIFIED
            )
        stash_line = "        _fire_one_liner_groups(problems)"
        self.assertEqual(pristine.count(stash_line), 1)
        stash = _mutant_wrap_assert(pristine, stash_line, "        pass")
        stash["assert_outcomes_one_liner"](
            T316_FULL_2919, T316_ONE_LINER_FALSIFIED
        )  # no raise under the stash
        # The module restored byte-identical after the run (nothing was
        # ever written — re-read and compared against the pristine read).
        with open(wrap_assert.__file__, "r", encoding="utf-8") as f:
            self.assertEqual(f.read(), pristine)


# ---- T317: the fill trim-enumeration leg (assert_fill_trim) ----

T317_FILL_STAMP = "2026-11-04T13:47:56.795Z"
# The mechanical trim enumeration the compose leg probes at the fill stamp
# (the T282 channel half: probed values ride argv/env into the script,
# never literal-composed) — the cycle-2951 stream's trim-fire count at the
# fill stamp is ONE: the trim leg 2026-11-04T13:46:55.188Z fired ~62s
# BEFORE the stamp (the notes' `trim 0 == 0` at ITS 13:43:57Z stamp was
# TRUE; the leg fired between the two stamps).
T317_PROBED_COUNT = 1
# The corpus's FINAL-enumeration rendering: the TRUE figure (the mechanical
# 1 at the fill stamp) and the falsified 51f2f7f figure (the notes'
# earlier trim-0 read, carried forward past the leg's fire).
T317_TRUE_BLOCK = "trim 1 == 1 on THIS stream's trim-fire at the stamp"
T317_FALSIFIED_BLOCK = "trim 0 == 0 [… on THIS stream's trim-0 at the stamp]"


# The corpus's FINAL-datum rendering is one physical line in the commit
# subject, but a fill BODY wraps — a multi-line TRUE fixture (the
# line-agnostic scan) doubling as the pre-split line-list slip shape's
# source (7 lines).
T317_MULTILINE_FILL = "\n".join(
    [
        "eval: cycle-2951 wrap fill - the state write landed [FACADE-GREEN]",
        "FINAL datum at the fill stamp [GENERATED by scripts/stream_datum.py",
        f"per the T305 doctrine, {T317_FILL_STAMP}]: 190 iters / ~5,022.1s",
        "wall [2026-11-04T12:14:02.318Z -> 2026-11-04T13:47:56.795Z off the",
        "stream anchors]: ok:false 3 == 3 adjudicated, 0-REAL + ctx_edit 1",
        f"accepted / 0 rejected + {T317_TRUE_BLOCK} + goal 0 at the stamp",
        "the wrap final todo flips land BEFORE the goal gate per T255",
    ]
)


def _t317_fill(trim_block):
    """A fill text carrying *trim_block* as its FINAL-datum enumeration."""
    return (
        "eval: cycle-2951 wrap fill - the state write landed "
        "[FACADE-GREEN]; the fill-stamp pins re-verify 236/236 PINS-EXIT 0; "
        "FINAL datum at the fill stamp [GENERATED by scripts/stream_datum.py "
        f"per the T305 doctrine, {T317_FILL_STAMP}]: 190 iters / ~5,022.1s "
        "wall [2026-11-04T12:14:02.318Z -> 2026-11-04T13:47:56.795Z off the "
        "stream anchors]: ok:false 3 == 3 adjudicated, 0-REAL + ctx_edit 1 "
        f"accepted / 0 rejected + {trim_block} + goal 0 at the stamp"
    )


class AssertFillTrimTest(unittest.TestCase):
    def test_pass_true_trim_carried_exactly(self):
        # T317 PASS path: the fill text carrying the mechanical trim count
        # exactly (trim 1 == 1, the 2951 stream's trim-fire at the fill
        # stamp) rides the trim leg green — the one-line subject shape AND
        # a multi-line body shape (the scan is line-agnostic).
        wrap_assert.assert_fill_trim(
            _t317_fill(T317_TRUE_BLOCK), T317_PROBED_COUNT, T317_FILL_STAMP
        )
        wrap_assert.assert_fill_trim(
            T317_MULTILINE_FILL, T317_PROBED_COUNT, T317_FILL_STAMP
        )

    def test_pass_bare_head_figure_and_wrapped_and_zero(self):
        # The bare `trim K` head-figure (the establishment prose's shape,
        # `... + trim 1, ctx 1, goal 0, ...`) and the wrapped rendering
        # (`trim 1 ==\n1` — a commit message wrapped mid-rendering) both
        # carry the probed count: green. A legitimate 0 count is likewise
        # green (0 is a count, never degenerate — a stream that fired no
        # trim leg, quoted at a stamp before any leg fired, rides green).
        wrap_assert.assert_fill_trim(
            _t317_fill(
                "trim 1, ctx 1, goal 0, 45,786 bytes + mtime "
                "2026-11-04T13:47:58Z"
            ),
            T317_PROBED_COUNT,
            T317_FILL_STAMP,
        )
        wrap_assert.assert_fill_trim(
            _t317_fill(T317_TRUE_BLOCK.replace(
                "trim 1 == 1", "trim 1 ==\n1")),
            T317_PROBED_COUNT,
            T317_FILL_STAMP,
        )
        wrap_assert.assert_fill_trim(
            _t317_fill("trim 0 == 0 (no trim fired on this stream)"),
            0,
            T317_FILL_STAMP,
        )

    def test_fail_falsified_count_names_both_figures(self):
        # T317 FAIL path, the 51f2f7f shape exactly: the committed figure
        # (0, the notes' earlier 13:43:57Z trim-0 read carried forward)
        # against the mechanical 1 (the 13:46:55.188Z leg fired between
        # the two stamps). BOTH figures are named — the divergent block
        # figure and the probed count — with the stamp, the fire, and the
        # T272 false-at-write shape.
        text = _t317_fill(T317_FALSIFIED_BLOCK)
        with self.assertRaises(AssertionError) as ctx:
            wrap_assert.assert_fill_trim(
                text, T317_PROBED_COUNT, T317_FILL_STAMP
            )
        msg = str(ctx.exception)
        self.assertIn("block figure '0' != probed 1", msg)
        self.assertIn(T317_FILL_STAMP, msg)  # the probed trim stamp
        self.assertIn("51f2f7f", msg)  # the fire named
        self.assertIn("false-at-write", msg)  # the class named
        self.assertIn("recompose", msg)  # the remedy named
        self.assertIn("wc -l", msg)  # the mechanical enumeration named
        self.assertIn("block 1 of 1", msg)
        self.assertIn("the full FINAL-enumeration rendering (== 0)", msg)

    def test_fail_zero_blocks_named_missing(self):
        # A datum-less fill is NEVER a silent pass: zero trim enumeration
        # blocks probed fires named-missing — including a fill carrying a
        # datum block whose trim figure was dropped entirely, and prose
        # that merely names the surface (`trim-fire`, `trim-0`) without a
        # count figure.
        for bare in (
            "eval: cycle-2951 wrap fill - the state write landed "
            "[FACADE-GREEN]; FINAL datum at the fill stamp: 190 iters / "
            "~5,022.1s wall [2026-11-04T12:14:02.318Z -> "
            "2026-11-04T13:47:56.795Z] + ok:false 3 == 3 + goal 0 at the "
            "stamp",
            "the trim-fire count the compose leg re-runs",
            "the trim-0 stream the notes adjudicated at their stamp",
        ):
            with self.assertRaises(AssertionError) as ctx:
                wrap_assert.assert_fill_trim(
                    bare, T317_PROBED_COUNT, T317_FILL_STAMP
                )
            msg = str(ctx.exception)
            self.assertIn("no `trim K` enumeration figure", msg)
            self.assertIn("named-missing", msg)
            self.assertIn("never a silent pass", msg)
            self.assertIn(T317_FILL_STAMP, msg)
            self.assertIn("wc -l", msg)

    def test_fail_degenerate_probed_count_fails_closed(self):
        # A degenerate probed_count fails closed naming the probed value
        # and its type — a str count, None, a bool (a bool is an int
        # subclass; it is not a count), and a negative all refuse. The
        # corpus's false-at-write shape: a literal-composed count riding
        # the ARG can never wave the leg GREEN.
        for bad, frag in (
            ("1", "probed probed_count '1' (type str)"),
            (None, "probed probed_count None (type NoneType)"),
            (True, "probed probed_count True (type bool)"),
            (-1, "probed probed_count -1 (type int)"),
        ):
            with self.assertRaises(AssertionError) as ctx:
                wrap_assert.assert_fill_trim(
                    _t317_fill(T317_TRUE_BLOCK), bad, T317_FILL_STAMP
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
            wrap_assert.assert_fill_trim(
                7, T317_PROBED_COUNT, T317_FILL_STAMP
            )
        msg = str(ctx.exception)
        self.assertIn("TEXT", msg)
        self.assertIn("type int", msg)
        with self.assertRaises(TypeError) as ctx:
            wrap_assert.assert_fill_trim(
                T317_MULTILINE_FILL.splitlines(),
                T317_PROBED_COUNT,
                T317_FILL_STAMP,
            )
        msg = str(ctx.exception)
        self.assertIn("7 probed line(s)", msg)
        self.assertIn("type list", msg)
        with self.assertRaises(TypeError) as ctx:
            wrap_assert.assert_fill_trim(
                _t317_fill(T317_TRUE_BLOCK), T317_PROBED_COUNT, None
            )
        msg = str(ctx.exception)
        self.assertIn("probed_stamp", msg)
        self.assertIn("type NoneType", msg)

    def test_pass_any_block_stale_plus_true_redeemed(self):
        # The any-block semantics (the datum leg's rule): one text quoting
        # BOTH the stale notes' figure (the 13:43:57Z trim-0 read, TRUE at
        # ITS stamp) and the TRUE fill figure stays GREEN — the TRUE block
        # redeems the text. Unlike the datum leg's stamp-moves safety
        # argument, a bare count CAN repeat across streams — the freshness
        # contract rides the compose leg's SAME-leg re-probe (the T282
        # channel half: the probed count is the live reading, captured
        # into a shell variable, never literal-composed), not the
        # any-block semantics.
        both = _t317_fill(T317_FALSIFIED_BLOCK) + "\n" + _t317_fill(
            T317_TRUE_BLOCK
        )
        wrap_assert.assert_fill_trim(
            both, T317_PROBED_COUNT, T317_FILL_STAMP
        )
        # ...and the same text rides green against the stale figure's own
        # count too (any-block), while a text carrying ONLY the stale
        # figure still fires (the falsified-count leg above).
        wrap_assert.assert_fill_trim(
            both, 0, T317_FILL_STAMP
        )

    def test_fail_fire_is_pre_write_zero_mutation(self):
        # The fire is designed pre-commit: all arguments are values, the
        # function opens and writes NOTHING — the tempdir is byte-unchanged
        # through the raise.
        with tempfile.TemporaryDirectory() as tmp:
            before = sorted(os.listdir(tmp))
            with self.assertRaises(AssertionError):
                wrap_assert.assert_fill_trim(
                    _t317_fill(T317_FALSIFIED_BLOCK),
                    T317_PROBED_COUNT,
                    T317_FILL_STAMP,
                )
            self.assertEqual(sorted(os.listdir(tmp)), before)

    def test_export_and_docstring_coverage_sentence_pinned(self):
        # The leg rides the module's __all__ (17 -> 18; T319 -> 19;
        # T321 -> 20; T323 -> 21) and the docstring coverage sentence
        # names the T317 legs (the pin-surface extended, not re-shaped).
        self.assertIn("assert_fill_trim", wrap_assert.__all__)
        self.assertEqual(len(wrap_assert.__all__), 21)
        self.assertIn("T317 fill trim-enumeration legs", wrap_assert.__doc__)
        self.assertIn("T321 fill trim-streak legs", wrap_assert.__doc__)
        # The extension, not a re-shape: the T316 sentence tail still rides.
        self.assertIn("T316 Outcomes one-liner figure-audit legs",
                      wrap_assert.__doc__)
        self.assertIn("T310 fill-enumeration legs", wrap_assert.__doc__)

    def test_red_provability_mutants_run_on_a_copy(self):
        # RED-provability, hand-proved by mutants run on a COPY: the
        # module itself is never written (the mutation rides an in-memory
        # exec of a mutated source string), and the on-disk module is
        # re-read byte-identical after each run.
        with open(wrap_assert.__file__, "r", encoding="utf-8") as f:
            pristine = f.read()
        falsified_text = _t317_fill(T317_FALSIFIED_BLOCK)
        datumless_text = (
            "eval: cycle-2951 wrap fill - no trim enumeration figure"
        )
        # The shipped module fires on both inputs (the legs above pin it;
        # re-asserted here so the mutant contrast is self-contained).
        with self.assertRaises(AssertionError):
            wrap_assert.assert_fill_trim(
                falsified_text, T317_PROBED_COUNT, T317_FILL_STAMP
            )
        with self.assertRaises(AssertionError):
            wrap_assert.assert_fill_trim(
                datumless_text, T317_PROBED_COUNT, T317_FILL_STAMP
            )
        # M1 — the trim count compare deleted (every block redeems): the
        # falsified-count fire goes red (no raise under the mutant, where
        # the shipped leg raises).
        m1 = _mutant_wrap_assert(
            pristine,
            "        if int(t_head) == probed_count:",
            "        if True:",
        )
        m1["assert_fill_trim"](
            falsified_text, T317_PROBED_COUNT, T317_FILL_STAMP
        )
        # M2 — the zero-block fire silent-passed: the named-missing fire
        # goes red (no raise under the mutant, where the shipped leg
        # raises).
        m2 = _mutant_wrap_assert(
            pristine,
            "        _fire_trim_named_missing(probed_stamp)",
            "        return",
        )
        m2["assert_fill_trim"](
            datumless_text, T317_PROBED_COUNT, T317_FILL_STAMP
        )
        # The module restored byte-identical after each run (nothing was
        # ever written — re-read and compared against the pristine read).
        with open(wrap_assert.__file__, "r", encoding="utf-8") as f:
            self.assertEqual(f.read(), pristine)


# ---- T319: the fill enumeration count-word leg (assert_fill_enumeration_word) ----

T319_FILL_STAMP = "2026-11-04T17:51:05.800Z"
# The mechanical `ok:false`-leg count the compose leg probes at the fill
# stamp (the SAME probe the T310 leg takes — the T282 channel half: probed
# values ride argv/env into the script, never literal-composed) — the
# cycle-2955 stream's whole-final `ok:false`-leg count at the fill stamp is
# TWELVE (jq `select(.type=="tool_result" and .ok==false)` count-12 — all
# twelve timestamps the fill itself enumerates).
T319_PROBED_COUNT = 12
# The corpus's FINAL-enumeration count-word renderings: the TRUE words (the
# mechanical TWELVE at the fill stamp — head `TWELVE legs` + tail `ALL
# TWELVE designed`) and the falsified 652fd7c words (head `ELEVEN legs` +
# tail `ALL ELEVEN designed` against the mechanical TWELVE — the class's
# FIFTH committed sighting, the FIRST on the enumeration count-word
# surface; the FIGURE `12 == 12` itself TRUE, the T310 guard-fire's
# designed catch having corrected it after the fire leg landed as the
# 12th, while the count-WORD rode stale, head and tail both).
T319_TRUE_BLOCK = (
    "ok:false 12 == 12 [the enumeration re-run off the stream's own flags "
    "per T272/T310 at the fill stamp: TWELVE legs — 14:43:00.698Z the "
    "Phase-1 corpus probe's trailing grep no-match exit-1 [recomposed] + "
    "16:16:59.628Z the 300s bash-cap timeout kill on the T317 harvest "
    "composite [no casualty, verified next leg] + 16:19:01.014Z the "
    "git-add .chug/* gitignore refusal [recomposed with TODO.md only "
    "GREEN] + 17:25:32.179Z/.180Z/.180Z/.181Z/.182Z the FIVE outcome-batch "
    "decision_log designed refusals [prose-suffixed choice, recomposed "
    "bare GREEN] + 17:35:26.002Z the tac command-not-found exit-127 "
    "[GNU-only on the BSD host, recomposed with tail -r GREEN] + "
    "17:38:16.919Z the prep-battery verify grep count-0 exit-1 [the "
    "Summary line the authority, recomposed GREEN] + ~17:49Z the T309 "
    "datum assert's FIRST guard-fire [the fill text's datum block carried "
    "the unicode arrow where the leg probes the ASCII form — the designed "
    "pre-commit fire, zero-commit consequence, recomposed GREEN the same "
    "wrap] + ~17:51Z the T310 enumeration assert's FIRST guard-fire [the "
    "recompose updated the datum but hardcoded the enumeration figure 10 "
    "against the mechanical 11 — the exact T272 shape the leg pins, the "
    "designed pre-commit fire, zero-commit consequence, recomposed with "
    "the probed count GREEN] — ALL TWELVE designed read-only zero-mutation "
    "0-REAL legs [the last two the assert family's own designed "
    "guard-fires dogfooding]]"
)
T319_FALSIFIED_BLOCK = T319_TRUE_BLOCK.replace(
    "TWELVE legs", "ELEVEN legs"
).replace("ALL TWELVE designed", "ALL ELEVEN designed")


# The corpus's FINAL-datum rendering is one physical line in the commit
# subject, but a fill BODY wraps — a multi-line TRUE fixture (the
# line-agnostic scan) doubling as the pre-split line-list slip shape's
# source (8 lines).
T319_MULTILINE_FILL = "\n".join(
    [
        "eval: cycle-2955 wrap fill - the wrap tail [the state write landed",
        "FACADE-GREEN]; Datum FINAL 195 iters / ~12554.650s wall [GENERATED",
        "by scripts/stream_datum.py per the T305 doctrine,",
        f"{T319_FILL_STAMP}]: ok:false 12 == 12 [the enumeration re-run off",
        "the stream's own flags per T272/T310 at the fill stamp: TWELVE",
        "legs — the ten timestamped legs plus the T309/T310 guard-fires —",
        "ALL TWELVE designed read-only zero-mutation 0-REAL legs",
        "[the last two the assert family's own designed guard-fires "
        "dogfooding]]",
    ]
)


def _t319_fill(word_block):
    """A fill text carrying *word_block* as its FINAL-datum enumeration."""
    return (
        "eval: cycle-2955 wrap fill - the wrap tail. Datum FINAL 195 iters "
        "/ ~12554.650s wall [2026-11-04T14:21:51.150Z -> "
        "2026-11-04T17:51:05.800Z off the stream anchors per "
        "scripts/stream_datum.py --at 2026-11-04T17:51:05Z at the fill "
        f"stamp] — {word_block} + masked-stderr 0 candidates + ctx 1 "
        "accepted / 0 rejected + trim 1 == 1 + goal 0 at the stamp"
    )


class AssertFillEnumerationWordTest(unittest.TestCase):
    def test_pass_true_count_words_carried_exactly(self):
        # T319 PASS path: the fill text whose enumeration count-words carry
        # the probed count (head `TWELVE legs` + tail `ALL TWELVE designed`
        # at the mechanical 12 — the figure the T310 leg already pins) rides
        # the count-word leg green — the one-line subject shape AND a
        # multi-line body shape (the scan is line-agnostic).
        wrap_assert.assert_fill_enumeration_word(
            _t319_fill(T319_TRUE_BLOCK), T319_PROBED_COUNT, T319_FILL_STAMP
        )
        wrap_assert.assert_fill_enumeration_word(
            T319_MULTILINE_FILL, T319_PROBED_COUNT, T319_FILL_STAMP
        )

    def test_pass_singular_and_zero_shapes(self):
        # The singular `ONE leg` head form (the count-1 shape) and the
        # `ZERO legs` head form (the count-0 shape — a stream that fired no
        # ok:false leg, quoted at a stamp before any leg fired) both carry
        # the probed count: green (0 is a count, never degenerate).
        wrap_assert.assert_fill_enumeration_word(
            _t319_fill(
                "ok:false 1 == 1 [the enumeration re-run off the stream's "
                "own flags per T272/T310 at the fill stamp: ONE leg — "
                "13:46:55.188Z the trim-leg AssertionError [the designed "
                "pre-commit fire, recomposed GREEN] — ALL ONE designed "
                "read-only zero-mutation 0-REAL legs]"
            ),
            1,
            T319_FILL_STAMP,
        )
        wrap_assert.assert_fill_enumeration_word(
            _t319_fill(
                "ok:false 0 == 0 [the enumeration re-run off the stream's "
                "own flags per T272/T310 at the fill stamp: ZERO legs — no "
                "ok:false leg fired on this stream — ALL ZERO designed "
                "read-only zero-mutation 0-REAL legs]"
            ),
            0,
            T319_FILL_STAMP,
        )

    def test_fail_2955_shape_names_word_mapped_probed_and_stamp(self):
        # T319 FAIL path, the 652fd7c shape exactly: the committed
        # count-WORD (ELEVEN, head and tail both) against the mechanical
        # TWELVE at the fill's own named stamp — the figure 12 == 12 itself
        # TRUE (the T310 guard-fire's designed catch corrected it) while
        # the word rode stale. The fire names BOTH offending words with
        # their mapped value, the probed count, and the stamp, with the
        # class and the remedy.
        text = _t319_fill(T319_FALSIFIED_BLOCK)
        with self.assertRaises(AssertionError) as ctx:
            wrap_assert.assert_fill_enumeration_word(
                text, T319_PROBED_COUNT, T319_FILL_STAMP
            )
        msg = str(ctx.exception)
        self.assertIn(
            "count-word 'ELEVEN' (head form) maps 11 != probed 12", msg
        )
        self.assertIn(
            "count-word 'ELEVEN' (tail form) maps 11 != probed 12", msg
        )
        self.assertIn(T319_FILL_STAMP, msg)  # the probed enumeration stamp
        self.assertIn("652fd7c", msg)  # the fire named
        self.assertIn("FIFTH committed sighting", msg)  # the class named
        self.assertIn("re-run", msg)  # the remedy named
        self.assertIn("wc -l", msg)  # the mechanical enumeration named
        self.assertIn("block 1 of 1", msg)

    def test_fail_word_mismatch_inside_one_block(self):
        # The head right and the tail wrong (`ALL ELEVEN designed` against
        # the mechanical TWELVE) STILL fires — every count-word occurrence
        # in the block must map, not merely one of them; only the offending
        # occurrence is named (the head word mapped true and rides).
        text = _t319_fill(
            T319_TRUE_BLOCK.replace(
                "ALL TWELVE designed", "ALL ELEVEN designed"
            )
        )
        with self.assertRaises(AssertionError) as ctx:
            wrap_assert.assert_fill_enumeration_word(
                text, T319_PROBED_COUNT, T319_FILL_STAMP
            )
        msg = str(ctx.exception)
        self.assertIn(
            "count-word 'ELEVEN' (tail form) maps 11 != probed 12", msg
        )
        self.assertNotIn("head form", msg)

    def test_fail_zero_word_bearing_blocks_named_missing(self):
        # A datum-less fill is NEVER a silent pass: enumeration blocks
        # probed with NO count-word rendering (the T310-era figure-only
        # shape, `ok:false 12 == 12 adjudicated, 0-REAL + ...` — no payload
        # bracket, no words) fire named-missing, as does a fill with no
        # enumeration figure at all.
        for bare in (
            "eval: cycle-2955 wrap fill - the wrap tail. Datum FINAL 195 "
            "iters / ~12554.650s wall [2026-11-04T14:21:51.150Z -> "
            "2026-11-04T17:51:05.800Z off the stream anchors]: ok:false 12 "
            "== 12 adjudicated, 0-REAL + trim 1 == 1 + goal 0 at the stamp",
            "eval: cycle-2955 wrap fill - the wrap tail [no enumeration "
            "figure probed]",
        ):
            with self.assertRaises(AssertionError) as ctx:
                wrap_assert.assert_fill_enumeration_word(
                    bare, T319_PROBED_COUNT, T319_FILL_STAMP
                )
            msg = str(ctx.exception)
            self.assertIn("no count-word rendering", msg)
            self.assertIn("named-missing", msg)
            self.assertIn("never a silent pass", msg)
            self.assertIn(T319_FILL_STAMP, msg)
            self.assertIn("wc -l", msg)

    def test_fail_unknown_word_fails_closed(self):
        # An unknown word in a count-word rendering fails closed naming it
        # — the vocabulary is the corpus's observed words (ZERO..TWENTY),
        # never a projection of what a fill MIGHT write.
        text = _t319_fill(
            T319_TRUE_BLOCK.replace("TWELVE legs", "THIRTY legs")
        )
        with self.assertRaises(AssertionError) as ctx:
            wrap_assert.assert_fill_enumeration_word(
                text, T319_PROBED_COUNT, T319_FILL_STAMP
            )
        msg = str(ctx.exception)
        self.assertIn(
            "count-word 'THIRTY' (head form) is not in the ZERO..TWENTY "
            "vocabulary", msg
        )
        self.assertIn("fails closed", msg)

    def test_fail_degenerate_probed_count_fails_closed(self):
        # A degenerate probed_count fails closed naming the probed value
        # and its type — a str count, None, a bool (a bool is an int
        # subclass; it is not a count), and a negative all refuse. The
        # corpus's false-at-write shape: a literal-composed count riding
        # the ARG can never wave the leg GREEN.
        for bad, frag in (
            ("12", "probed probed_count '12' (type str)"),
            (None, "probed probed_count None (type NoneType)"),
            (True, "probed probed_count True (type bool)"),
            (-1, "probed probed_count -1 (type int)"),
        ):
            with self.assertRaises(AssertionError) as ctx:
                wrap_assert.assert_fill_enumeration_word(
                    _t319_fill(T319_TRUE_BLOCK), bad, T319_FILL_STAMP
                )
            msg = str(ctx.exception)
            self.assertIn("fails closed", msg)
            self.assertIn(frag, msg)

    def test_fail_non_str_arguments_raise_type_error(self):
        # The interface defense: *text* is the composed TEXT and
        # *probed_stamp* is the probed stamp STR — a pre-split line list is
        # the text call-signature slip shape (the caller passes the
        # composed text verbatim), so it TypeErrors naming the shape.
        with self.assertRaises(TypeError) as ctx:
            wrap_assert.assert_fill_enumeration_word(
                7, T319_PROBED_COUNT, T319_FILL_STAMP
            )
        msg = str(ctx.exception)
        self.assertIn("TEXT", msg)
        self.assertIn("type int", msg)
        with self.assertRaises(TypeError) as ctx:
            wrap_assert.assert_fill_enumeration_word(
                T319_MULTILINE_FILL.splitlines(),
                T319_PROBED_COUNT,
                T319_FILL_STAMP,
            )
        msg = str(ctx.exception)
        self.assertIn("8 probed line(s)", msg)
        self.assertIn("type list", msg)
        with self.assertRaises(TypeError) as ctx:
            wrap_assert.assert_fill_enumeration_word(
                _t319_fill(T319_TRUE_BLOCK), T319_PROBED_COUNT, None
            )
        msg = str(ctx.exception)
        self.assertIn("probed_stamp", msg)
        self.assertIn("type NoneType", msg)

    def test_pass_any_block_stale_plus_true_redeemed(self):
        # The any-block semantics (the datum leg's rule): one text quoting
        # BOTH the falsified 652fd7c word shape (the stale ELEVEN read) and
        # the TRUE recompose shape stays GREEN — the TRUE block redeems the
        # text. The freshness contract rides the compose leg's SAME-leg
        # re-probe (the T282 channel half: the probed count is the live
        # reading, captured into a shell variable, never literal-composed),
        # not the any-block semantics.
        both = _t319_fill(T319_FALSIFIED_BLOCK) + "\n" + _t319_fill(
            T319_TRUE_BLOCK
        )
        wrap_assert.assert_fill_enumeration_word(
            both, T319_PROBED_COUNT, T319_FILL_STAMP
        )
        # ...and the same text rides green against the stale word's own
        # count too (any-block), while a text carrying ONLY the stale word
        # still fires (the 2955-shape leg above).
        wrap_assert.assert_fill_enumeration_word(both, 11, T319_FILL_STAMP)

    def test_fail_fire_is_pre_write_zero_mutation(self):
        # The fire is designed pre-commit: all arguments are values, the
        # function opens and writes NOTHING — the tempdir is byte-unchanged
        # through the raise.
        with tempfile.TemporaryDirectory() as tmp:
            before = sorted(os.listdir(tmp))
            with self.assertRaises(AssertionError):
                wrap_assert.assert_fill_enumeration_word(
                    _t319_fill(T319_FALSIFIED_BLOCK),
                    T319_PROBED_COUNT,
                    T319_FILL_STAMP,
                )
            self.assertEqual(sorted(os.listdir(tmp)), before)

    def test_export_and_docstring_coverage_sentence_pinned(self):
        # The leg rides the module's __all__ (18 -> 19; T321 -> 20;
        # T323 -> 21) and the docstring coverage sentence names the T319
        # legs (the pin-surface extended, not re-shaped).
        self.assertIn("assert_fill_enumeration_word", wrap_assert.__all__)
        self.assertEqual(len(wrap_assert.__all__), 21)
        self.assertIn(
            "T319 fill enumeration count-word legs", wrap_assert.__doc__
        )
        self.assertIn("T321 fill trim-streak legs", wrap_assert.__doc__)
        # The extension, not a re-shape: the T317 and T310 sentence tails
        # still ride.
        self.assertIn("T317 fill trim-enumeration legs", wrap_assert.__doc__)
        self.assertIn("T310 fill-enumeration legs", wrap_assert.__doc__)

    def test_red_provability_mutants_run_on_a_copy(self):
        # RED-provability, hand-proved by mutants run on a COPY: the module
        # itself is never written (the mutation rides an in-memory exec of
        # a mutated source string), and the on-disk module is re-read
        # byte-identical after each run.
        with open(wrap_assert.__file__, "r", encoding="utf-8") as f:
            pristine = f.read()
        falsified_text = _t319_fill(T319_FALSIFIED_BLOCK)
        wordless_text = _t319_fill(
            "ok:false 12 == 12 adjudicated, 0-REAL + trim 1 == 1 + goal 0 "
            "at the stamp"
        )
        # The shipped module fires on both inputs (the legs above pin it;
        # re-asserted here so the mutant contrast is self-contained).
        with self.assertRaises(AssertionError):
            wrap_assert.assert_fill_enumeration_word(
                falsified_text, T319_PROBED_COUNT, T319_FILL_STAMP
            )
        with self.assertRaises(AssertionError):
            wrap_assert.assert_fill_enumeration_word(
                wordless_text, T319_PROBED_COUNT, T319_FILL_STAMP
            )
        # M1 — the word-compare deleted (every known word redeems): the
        # 2955-shape fire goes red (no raise under the mutant, where the
        # shipped leg raises).
        m1 = _mutant_wrap_assert(
            pristine,
            "            elif mapped != probed_count:",
            "            elif False:",
        )
        m1["assert_fill_enumeration_word"](
            falsified_text, T319_PROBED_COUNT, T319_FILL_STAMP
        )
        # M2 — the zero-word-bearing fire silent-passed: the named-missing
        # fire goes red (no raise under the mutant, where the shipped leg
        # raises).
        m2 = _mutant_wrap_assert(
            pristine,
            "        _fire_word_named_missing(probed_stamp)",
            "        return",
        )
        m2["assert_fill_enumeration_word"](
            wordless_text, T319_PROBED_COUNT, T319_FILL_STAMP
        )
        # The module restored byte-identical after each run (nothing was
        # ever written — re-read and compared against the pristine read).
        with open(wrap_assert.__file__, "r", encoding="utf-8") as f:
            self.assertEqual(f.read(), pristine)


# ---- T321: the fill trim-streak leg (assert_fill_trim_streak) ----

# T321_STAMP — the 3016 wrap's fill stamp (the live 3016 stream's compose
# moment in the acceptance fixture's geometry).
T321_STAMP = "2026-11-05T16:38:22Z"
# T321_3020_STAMP — the 3020 wrap's fill stamp (18:11:48.352Z committed as
# 18:11:48Z; the 3020 stream's own trim leg 18:12:53.572Z post-dates it).
T321_3020_STAMP = "2026-11-05T18:11:48Z"


def _t321_stream_body(run_start_ts, trim_stamps):
    """One stream's events lines: the run_start + the trim legs (the
    stream's own flags) + two iteration events so the file reads as a
    stream, not a stub."""
    lines = [
        json.dumps({"type": "run_start", "ts": run_start_ts}),
        json.dumps({"type": "iteration", "ts": run_start_ts}),
    ]
    for ts in trim_stamps:
        lines.append(json.dumps({"type": "trim", "ts": ts}))
    lines.append(
        json.dumps({"type": "iteration_end", "ts": trim_stamps[-1]
                    if trim_stamps else run_start_ts})
    )
    return "\n".join(lines) + "\n"


def _t321_build_events_dir(tmp, streams, live=None):
    """Write the fixture events corpus: *streams* is a list of
    (rotated_name, run_start_ts, [trim stamps]) written in stream order
    (each file's mtime pinned ahead of the next's so the ls/mtime order
    is the stream order regardless of write speed); *live* is the
    (run_start_ts, [trim stamps]) of the live `events.jsonl` when given.
    Returns the events dir path (a subdir of tmp)."""
    events_dir = os.path.join(tmp, "chug-events")
    os.mkdir(events_dir)
    for i, (name, run_start_ts, trim_stamps) in enumerate(streams):
        path = os.path.join(events_dir, name)
        with open(path, "w", encoding="utf-8") as f:
            f.write(_t321_stream_body(run_start_ts, trim_stamps))
        os.utime(path, (1_700_000_000 + i, 1_700_000_000 + i))
    if live is not None:
        path = os.path.join(events_dir, "events.jsonl")
        with open(path, "w", encoding="utf-8") as f:
            f.write(_t321_stream_body(live[0], live[1]))
        os.utime(path, (1_700_000_000 + len(streams),) * 2)
    return events_dir


# The acceptance fixture's geometry — the ACTUAL cycle-3024 finding, the
# streams named by their real rotated files (the 3004 stream's whole-final
# trim count 2 with its own two legs, per the 722nd eval's committed record
# f8cf76b; the 3008/3012 trim-0 whole-final chain; the live 3016 stream
# trim-0 at the 3016 wrap's fill stamp): the resetting stream sits three
# streams back from the live current stream, chain = 2, TRUE figure 3.
T321_3024_STREAMS = [
    ("events-20261105-124928.jsonl", "2026-11-05T11:32:08.954Z",
     ["2026-11-05T11:35:28.663Z", "2026-11-05T12:16:19.639Z"]),
    ("events-20261105-140840.jsonl", "2026-11-05T12:49:28.417Z", []),
    ("events-20261105-152533.jsonl", "2026-11-05T14:08:40.772Z", []),
]
T321_3024_LIVE = ("2026-11-05T15:25:33.643Z", [])

# The 3020-fill geometry — the rotated corpus INCLUDING the 3016 stream
# (171528, trim-0 whole-final) and the 3020 stream (184419, whose single
# trim leg 18:12:53.572Z post-dates the 3020 fill stamp), NO live file
# (the newest rotated file is the current stream): chain = 3, TRUE 4 at
# the stamp — the actual 3020 wrap fill's falsified `reads 5` shape.
T321_3020_STREAMS = T321_3024_STREAMS + [
    ("events-20261105-171528.jsonl", "2026-11-05T15:25:33.643Z", []),
    ("events-20261105-184419.jsonl", "2026-11-05T17:15:28.564Z",
     ["2026-11-05T18:12:53.572Z"]),
]

# ---- T322 fixture geometries ----

# The T322 finding-2 boundary geometry — a resetting stream whose
# whole-final trim count is EXACTLY 1 (the modal real-corpus resetter
# shape: a one-post-stamp-trim stream), followed by two trim-0 streams
# and a trim-0-at-the-stamp live stream: chain = 2, TRUE 3, the walk
# stops AT the trim-1 stream and names it.
T322_TRIM1_STREAMS = [
    ("events-20261105-093000.jsonl", "2026-11-05T08:30:00.000Z",
     ["2026-11-05T09:00:00.000Z"]),
    ("events-20261105-104500.jsonl", "2026-11-05T09:30:00.000Z", []),
    ("events-20261105-120000.jsonl", "2026-11-05T10:45:00.000Z", []),
]
T322_TRIM1_LIVE = ("2026-11-05T11:00:00.000Z", [])
T322_TRIM1_STAMP = "2026-11-05T11:30:00Z"

# The T322 finding-3 boundary geometry — the live current stream carries
# ONE trim leg whose ts string EXACTLY equals the fill stamp (identical
# strings — the 3020 leg's `.` < `Z` subsecond shape pinned the
# comparison, never the equality arm): shipped, the at-stamp count
# INCLUDES it (at-stamp 1), the extension precondition fails, TRUE reads
# the bare chain 2; the `<=` -> `<` mutant excludes it (at-stamp 0, TRUE
# 3). The resetter echoes the 3004 stream's whole-final trim 2.
T322_EQUALITY_STAMP = "2026-11-05T11:30:00Z"
T322_EQUALITY_STREAMS = [
    ("events-20261105-093000.jsonl", "2026-11-05T08:30:00.000Z",
     ["2026-11-05T09:00:00.000Z", "2026-11-05T09:05:00.000Z"]),
    ("events-20261105-104500.jsonl", "2026-11-05T09:30:00.000Z", []),
    ("events-20261105-120000.jsonl", "2026-11-05T10:45:00.000Z", []),
]
T322_EQUALITY_LIVE = ("2026-11-05T11:00:00.000Z", [T322_EQUALITY_STAMP])

# The T322 finding-1 scoping geometry — a legacy two-run_start file sits
# OLDEST, BEYOND the resetting stream: the walk stops at the trim-1
# stream three files newer, so the corrupt file is never read and never
# identity-checked (the live main corpus's Sept-2026 shape — the T321
# sweep refused every fill on it).
T322_SCOPING_STREAMS = [
    ("events-20261105-080000.jsonl", "2026-11-05T07:00:00.000Z", []),
    ("events-20261105-093000.jsonl", "2026-11-05T08:30:00.000Z",
     ["2026-11-05T09:00:00.000Z"]),
    ("events-20261105-104500.jsonl", "2026-11-05T09:30:00.000Z", []),
]
T322_SCOPING_LIVE = ("2026-11-05T10:00:00.000Z", [])
T322_SCOPING_STAMP = "2026-11-05T10:30:00Z"
# The `events-tNNN-*` task-harvest name shape — NEVER a main-loop
# predecessor stream (the T321 glob swept 610+ of them into the identity
# check); the fixture's harvest file carries a two-run_start corruption
# the scoped leg must never read.
T322_HARVEST_NAME = "events-t322-harvest.jsonl"

def _t321_strip_run_start_lines(path):
    """Rewrite the events file without its run_start lines (the
    zero-run_start identity-corruption shape — a tail-spliced or
    hand-truncated file)."""
    with open(path, "r", encoding="utf-8") as f:
        kept = [
            line for line in f.read().splitlines()
            if '"run_start"' not in line
        ]
    with open(path, "w", encoding="utf-8") as f:
        f.write("\n".join(kept) + "\n")


class AssertFillTrimStreakTest(unittest.TestCase):
    def test_red_the_falsified_cycle_3024_shape_fires(self):
        # The acceptance RED: a fixture chain whose resetting stream
        # (whole-final trim count 2, its OWN two legs) sits three streams
        # back, followed by three trim-0 streams, against a text claiming
        # `reads 4` — the 725th establishment's committed falsification
        # shape. The fire names the composed claim AND the probed chain
        # AND the resetting stream's identity (the rotated filename and
        # its run_start).
        with tempfile.TemporaryDirectory() as tmp:
            events_dir = _t321_build_events_dir(
                tmp, T321_3024_STREAMS, live=T321_3024_LIVE
            )
            falsified = (
                "trim 0 == 0 [the T317 trim-enumeration leg probed 0 at "
                "the stamp, asserted same-leg: the strict post-reset "
                "streak reads 4 TRUE on THIS stream's trim-0 at the "
                "stamp — running after the 3004/3008/3012/3016 trim-0 "
                "chain per the TRUE chain [the TRUE reading at the 695th "
                "establishment: reads 4, +THIS 5]]"
            )
            with self.assertRaises(AssertionError) as ctx:
                wrap_assert.assert_fill_trim_streak(
                    falsified, events_dir, T321_STAMP
                )
            msg = str(ctx.exception)
            self.assertIn("reads 4", msg)  # the composed claim, named
            self.assertIn("the probed 3", msg)  # the probed TRUE figure
            self.assertIn("prior chain 2", msg)  # the probed chain
            # the resetting stream's identity: the rotated filename AND
            # its run_start (the stream whose whole-final trim count is 2
            # is NOT a chain member — the 725th's falsified bracket
            # included it).
            self.assertIn("events-20261105-124928.jsonl", msg)
            self.assertIn("2026-11-05T11:32:08.954Z", msg)
            self.assertIn("resetting stream", msg)

    def test_pass_the_true_cycle_3024_shape_reads_3(self):
        # The acceptance PASS: the same fixture with a `reads 3` claim —
        # the TRUE stamp-scoped figure (chain 2 + the live stream's
        # trim-0 at the stamp) — rides green.
        with tempfile.TemporaryDirectory() as tmp:
            events_dir = _t321_build_events_dir(
                tmp, T321_3024_STREAMS, live=T321_3024_LIVE
            )
            true_text = (
                "trim 0 == 0 [the T317 trim-enumeration leg probed 0 at "
                "the stamp, asserted same-leg: the strict post-reset "
                "streak reads 3 TRUE on THIS stream's trim-0 at the "
                "stamp — running after the 3008/3012 trim-0 chain per "
                "the TRUE chain]"
            )
            wrap_assert.assert_fill_trim_streak(
                true_text, events_dir, T321_STAMP
            )

    def test_pass_the_moves_increment_shapes_both_arrow_spellings(self):
        # The `moves A -> B` shape (the corpus's fill template since
        # c7dacd7) — the asserted figure is the POST-move value. The
        # actual 3016 wrap fill's falsified `moves 3 -> 4` (TRUE 2 -> 3)
        # fires; the TRUE `moves 2 -> 3` rides green in both arrow
        # spellings (`->` and the unicode `→` the e69701c rendering
        # carried).
        with tempfile.TemporaryDirectory() as tmp:
            events_dir = _t321_build_events_dir(
                tmp, T321_3024_STREAMS, live=T321_3024_LIVE
            )
            falsified_3016_fill = (
                "trim 0 == 0 [the T317 trim-enumeration leg probed 0 at "
                "the stamp, asserted same-leg: the strict post-reset "
                "streak moves 3 -> 4 TRUE on THIS stream's trim-0 "
                "whole-final — running after the 2992 stream's trim-0 "
                "whole-final per the TRUE chain [the TRUE reading at the "
                "694th establishment: 3 -> 4]]"
            )
            with self.assertRaises(AssertionError) as ctx:
                wrap_assert.assert_fill_trim_streak(
                    falsified_3016_fill, events_dir, T321_STAMP
                )
            msg = str(ctx.exception)
            self.assertIn("moves 3 -> 4", msg)
            self.assertIn("the probed 3", msg)
            true_text_ascii = (
                "the strict post-reset streak moves 2 -> 3 TRUE on THIS "
                "stream's trim-0 whole-final — running after the 3012 "
                "stream's trim-0 whole-final per the TRUE chain"
            )
            true_text_unicode = true_text_ascii.replace("->", "\u2192")
            for text in (true_text_ascii, true_text_unicode):
                wrap_assert.assert_fill_trim_streak(
                    text, events_dir, T321_STAMP
                )

    def test_red_the_3020_fill_propagated_reads_5_shape(self):
        # The 3020 wrap fill's propagated falsification: the newest
        # rotated file is the current stream (no live file — the
        # fallback), its single trim leg 18:12:53.572Z post-dates the
        # 18:11:48Z fill stamp, so the stamp-scoped figure extends the
        # chain 3 by one = 4; the fill's composed `reads 5` fires naming
        # composed 5 vs probed 4, and a TRUE `reads 4` rides green.
        with tempfile.TemporaryDirectory() as tmp:
            events_dir = _t321_build_events_dir(tmp, T321_3020_STREAMS)
            falsified = (
                "trim 0 == 0 [the T317 trim-enumeration leg probed 0 at "
                "the stamp, asserted same-leg: the strict post-reset "
                "streak reads 5 TRUE on THIS stream's trim-0 at the "
                "stamp — running after the 3004/3008/3012/3016 trim-0 "
                "chain per the TRUE chain [the TRUE reading at the 695th "
                "establishment: reads 4, +THIS 5]]"
            )
            with self.assertRaises(AssertionError) as ctx:
                wrap_assert.assert_fill_trim_streak(
                    falsified, events_dir, T321_3020_STAMP
                )
            msg = str(ctx.exception)
            self.assertIn("reads 5", msg)
            self.assertIn("the probed 4", msg)
            self.assertIn("prior chain 3", msg)
            self.assertIn("events-20261105-124928.jsonl", msg)
            true_text = falsified.replace(
                "reads 5 TRUE", "reads 4 TRUE"
            )
            wrap_assert.assert_fill_trim_streak(
                true_text, events_dir, T321_3020_STAMP
            )

    def test_stamp_scoped_extension_requires_trim_0_at_the_stamp(self):
        # The stamp-scoped +1's negative branch: with the fill stamp
        # moved ONTO the 3020 stream's own trim leg (18:12:53.572Z <=
        # 18:12:53Z lexically — the stream anchors' own format), the
        # current stream's at-stamp count is 1, the extension's
        # precondition fails, and the TRUE figure reads the bare chain 3:
        # a stale `reads 4` fires; `reads 3` passes.
        with tempfile.TemporaryDirectory() as tmp:
            events_dir = _t321_build_events_dir(tmp, T321_3020_STREAMS)
            with self.assertRaises(AssertionError) as ctx:
                wrap_assert.assert_fill_trim_streak(
                    "the strict post-reset streak reads 4 TRUE on THIS "
                    "stream's trim-0 at the stamp",
                    events_dir,
                    "2026-11-05T18:12:53Z",
                )
            msg = str(ctx.exception)
            self.assertIn("the probed 3", msg)
            self.assertIn("prior chain 3", msg)
            wrap_assert.assert_fill_trim_streak(
                "the strict post-reset streak reads 3 TRUE on THIS "
                "stream's trim-0 at the stamp",
                events_dir,
                "2026-11-05T18:12:53Z",
            )

    def test_trim_at_stamp_leaves_figure_less_claims_inert(self):
        # The 2955-fill shape: the current stream carries a trim leg at/
        # before the stamp, so the stamp-scoped claim form is legitimately
        # ABSENT — the corpus's figure-less TRUE renderings (`HOLDS TRUE`,
        # `closes TRUE`, the establishment-quote `… : HOLDS 0`) and a
        # text with NO streak mention at all ride green (no assertion);
        # the leg closes the wrong-FIGURE shape, not the form-variation
        # one (a named-missing fire would RED historically-TRUE texts —
        # the brittle-awk class the T320 boundary rejects).
        with tempfile.TemporaryDirectory() as tmp:
            events_dir = _t321_build_events_dir(tmp, T321_3020_STREAMS)
            figure_less = [
                "trim 1 == 1 [the 18:12:53.572Z event — the harness's "
                "automatic context trim; the TRUE trim-streak reading at "
                "the 679th establishment: HOLDS 0 — THIS stream's trim "
                "leg keeps it at 0]",
                "trim 0 == 0 [the strict post-reset streak HOLDS TRUE on "
                "THIS stream's trim-0 at the stamp]",
                "trim 0 == 0 [the strict post-reset streak closes TRUE "
                "on THIS stream's trim-0 whole-final]",
                "eval: cycle-3020 wrap fill — no streak mention at all",
            ]
            for text in figure_less:
                wrap_assert.assert_fill_trim_streak(
                    text, events_dir, T321_3020_STAMP
                )

    def test_quoted_establishment_reading_is_not_swept(self):
        # The extraction boundary: the 3020 fill's payload quotes BOTH
        # forms — the own claim (keyword-anchored `reads N`) and the
        # quoted establishment reading (`the TRUE reading at the 695th
        # establishment: reads 4`). Only the KEYWORD-ANCHORED claim is
        # the asserted surface: a fill whose own claim is TRUE (4) but
        # whose payload QUOTES a divergent figure (reads 9) rides green —
        # the quoted reading is not swept (the eval-side establishment
        # texts are the NEXT eval's re-derivation watch, not this leg) —
        # while the same payload with the own claim composed wrong (5)
        # fires naming the OWN claim, never the quoted one.
        with tempfile.TemporaryDirectory() as tmp:
            events_dir = _t321_build_events_dir(tmp, T321_3020_STREAMS)
            true_own_quoted_divergent = (
                "the strict post-reset streak reads 4 TRUE on THIS "
                "stream's trim-0 at the stamp — running after the "
                "3008/3012/3016 trim-0 chain per the TRUE chain [the "
                "TRUE reading at the 695th establishment: reads 9, "
                "+THIS 5]"
            )
            wrap_assert.assert_fill_trim_streak(
                true_own_quoted_divergent, events_dir, T321_3020_STAMP
            )
            with self.assertRaises(AssertionError) as ctx:
                wrap_assert.assert_fill_trim_streak(
                    true_own_quoted_divergent.replace(
                        "streak reads 4 TRUE", "streak reads 5 TRUE"
                    ),
                    events_dir,
                    T321_3020_STAMP,
                )
            msg = str(ctx.exception)
            self.assertIn("reads 5", msg)  # the OWN claim, named
            self.assertNotIn("reads 9", msg)  # the quote, never swept
            # and a figure-less text quoting the divergent reading under
            # the establishment anchor stays inert entirely.
            wrap_assert.assert_fill_trim_streak(
                "trim 0 == 0 [the reading at the 695th establishment: "
                "reads 9, +THIS 5 — the strict post-reset streak carries "
                "the TRUE reading]",
                events_dir,
                T321_3020_STAMP,
            )

    def test_doctrine_shape_with_trim_word_inserted_rides(self):
        # The amended doctrine's own shape (`the strict post-reset trim
        # streak reads N …` — the optional word `trim` between the anchor
        # and `streak`) is a keyed claim: TRUE rides green, falsified
        # fires.
        with tempfile.TemporaryDirectory() as tmp:
            events_dir = _t321_build_events_dir(
                tmp, T321_3024_STREAMS, live=T321_3024_LIVE
            )
            wrap_assert.assert_fill_trim_streak(
                "the fill compose leg's pre-commit assert ALSO "
                "re-derives the strict post-reset trim streak reads 3 "
                "at the fill stamp",
                events_dir,
                T321_STAMP,
            )
            with self.assertRaises(AssertionError) as ctx:
                wrap_assert.assert_fill_trim_streak(
                    "the strict post-reset trim streak reads 9 at the "
                    "fill stamp",
                    events_dir,
                    T321_STAMP,
                )
            self.assertIn("the probed 3", str(ctx.exception))

    def test_t320_identity_precondition_rejects_by_name(self):
        # The T320 identity leg: a rotated file with ZERO run_start
        # events (a tail-splice or a hand-truncated file) and one with
        # TWO (a double-began stream) are each rejected BY NAME — the
        # probed run_start count and ts values named, the file never
        # silently miscounted into the chain.
        with tempfile.TemporaryDirectory() as tmp:
            events_dir = _t321_build_events_dir(
                tmp, T321_3024_STREAMS, live=T321_3024_LIVE
            )
            zero_rs = os.path.join(
                events_dir, "events-20261105-140840.jsonl"
            )
            _t321_strip_run_start_lines(zero_rs)
            with self.assertRaises(AssertionError) as ctx:
                wrap_assert.assert_fill_trim_streak(
                    "the strict post-reset streak reads 3 TRUE",
                    events_dir,
                    T321_STAMP,
                )
            msg = str(ctx.exception)
            self.assertIn("events-20261105-140840.jsonl", msg)
            self.assertIn("probed 0 run_start event(s)", msg)
            self.assertIn("[none]", msg)
            self.assertIn("T320 identity precondition", msg)
        with tempfile.TemporaryDirectory() as tmp:
            events_dir = _t321_build_events_dir(
                tmp, T321_3024_STREAMS, live=T321_3024_LIVE
            )
            double_rs = os.path.join(
                events_dir, "events-20261105-152533.jsonl"
            )
            with open(double_rs, "a", encoding="utf-8") as f:
                f.write(json.dumps(
                    {"type": "run_start",
                     "ts": "2026-11-05T14:10:00.000Z"}) + "\n")
            with self.assertRaises(AssertionError) as ctx:
                wrap_assert.assert_fill_trim_streak(
                    "the strict post-reset streak reads 3 TRUE",
                    events_dir,
                    T321_STAMP,
                )
            msg = str(ctx.exception)
            self.assertIn("events-20261105-152533.jsonl", msg)
            self.assertIn("probed 2 run_start event(s)", msg)
            self.assertIn("2026-11-05T14:08:40.772Z", msg)
            self.assertIn("2026-11-05T14:10:00.000Z", msg)

    def test_expectation_is_extracted_never_a_parameter(self):
        # The T270/T288 probed-expectation clause binds the expectation
        # side: the API carries NO expected-figure argument — the claimed
        # figure is read off the text under assertion, and the TRUE
        # figure is derived from the events files alone. The signature is
        # pinned (text, events_dir, fill_stamp) and a caller attempting
        # to pass a remembered constant positionally or by keyword hits
        # the signature, not a silent pass.
        params = list(
            inspect.signature(
                wrap_assert.assert_fill_trim_streak
            ).parameters
        )
        self.assertEqual(params, ["text", "events_dir", "fill_stamp"])
        with tempfile.TemporaryDirectory() as tmp:
            events_dir = _t321_build_events_dir(
                tmp, T321_3024_STREAMS, live=T321_3024_LIVE
            )
            falsified = (
                "the strict post-reset streak reads 4 TRUE on THIS "
                "stream's trim-0 at the stamp"
            )
            with self.assertRaises(TypeError):
                wrap_assert.assert_fill_trim_streak(
                    falsified, events_dir, T321_STAMP, 3
                )
            with self.assertRaises(TypeError):
                wrap_assert.assert_fill_trim_streak(
                    falsified, events_dir, T321_STAMP, true_figure=3
                )
            # and with the signature honored, the remembered-constant
            # caller has nowhere to put the 3: the same call fires on the
            # text's own composed claim (never waved green by a constant).
            with self.assertRaises(AssertionError) as ctx:
                wrap_assert.assert_fill_trim_streak(
                    falsified, events_dir, T321_STAMP
                )
            self.assertIn("the probed 3", str(ctx.exception))

    def test_non_str_arguments_type_error_naming_the_shape(self):
        # The call-signature slip shapes: a pre-split line LIST for the
        # text, a non-str events_dir, a non-str fill_stamp — each
        # TypeError names the accepted shape.
        with self.assertRaises(TypeError) as ctx:
            wrap_assert.assert_fill_trim_streak(
                ["line one", "line two"], ".chug", T321_STAMP
            )
        self.assertIn("text must be the composed fill/wrap TEXT", str(
            ctx.exception))
        self.assertIn("probed text 2 probed line(s)", str(ctx.exception))
        with self.assertRaises(TypeError) as ctx:
            wrap_assert.assert_fill_trim_streak(
                "the strict post-reset streak reads 3", None, T321_STAMP
            )
        self.assertIn("events_dir must be the events directory's PATH",
                      str(ctx.exception))
        with self.assertRaises(TypeError) as ctx:
            wrap_assert.assert_fill_trim_streak(
                "the strict post-reset streak reads 3", ".chug", None
            )
        self.assertIn("fill_stamp must be the stamp", str(ctx.exception))

    def test_unreadable_or_empty_events_dir_fails_closed(self):
        # The fail-closed half: a missing events_dir names the probed
        # path; an empty corpus (no live file, no rotated files) refuses
        # to derive — a probe that read nothing can never wave the
        # fill's streak claim GREEN.
        with tempfile.TemporaryDirectory() as tmp:
            absent = os.path.join(tmp, "no-such-chug")
            with self.assertRaises(AssertionError) as ctx:
                wrap_assert.assert_fill_trim_streak(
                    "the strict post-reset streak reads 3 TRUE",
                    absent,
                    T321_STAMP,
                )
            self.assertIn(absent, str(ctx.exception))
            self.assertIn("fails closed", str(ctx.exception))
        with tempfile.TemporaryDirectory() as tmp:
            empty = _t321_build_events_dir(tmp, [])
            with self.assertRaises(AssertionError) as ctx:
                wrap_assert.assert_fill_trim_streak(
                    "the strict post-reset streak reads 3 TRUE",
                    empty,
                    T321_STAMP,
                )
            self.assertIn("events corpus", str(ctx.exception))

    def test_fire_is_zero_mutation(self):
        # The events files are only ever READ: the fixture's byte content
        # is snapshot before a firing call and byte-identical after.
        with tempfile.TemporaryDirectory() as tmp:
            events_dir = _t321_build_events_dir(
                tmp, T321_3024_STREAMS, live=T321_3024_LIVE
            )

            def snapshot():
                state = {}
                for name in sorted(os.listdir(events_dir)):
                    with open(
                        os.path.join(events_dir, name), "rb"
                    ) as f:
                        state[name] = f.read()
                return state

            before = snapshot()
            with self.assertRaises(AssertionError):
                wrap_assert.assert_fill_trim_streak(
                    "the strict post-reset streak reads 9 TRUE",
                    events_dir,
                    T321_STAMP,
                )
            self.assertEqual(snapshot(), before)

    def test_export_and_docstring_coverage_sentence_pinned(self):
        # The leg rides the module's __all__ (19 -> 20; T323 -> 21) and
        # the docstring coverage sentence names the T321 legs (the
        # pin-surface extended, not re-shaped).
        self.assertIn("assert_fill_trim_streak", wrap_assert.__all__)
        self.assertEqual(len(wrap_assert.__all__), 21)
        self.assertIn("T321 fill trim-streak legs", wrap_assert.__doc__)
        # The T323 extension rides the docstring too (the newest leg's
        # sentence, additive).
        self.assertIn(
            "T323 fill eval-state-untracked legs", wrap_assert.__doc__
        )
        # The extension, not a re-shape: the T317/T319/T310 sentence
        # tails still ride.
        self.assertIn("T317 fill trim-enumeration legs",
                      wrap_assert.__doc__)
        self.assertIn("T319 fill enumeration count-word legs",
                      wrap_assert.__doc__)
        self.assertIn("T310 fill-enumeration legs", wrap_assert.__doc__)

    def test_red_provability_mutants_run_on_a_copy(self):
        # RED-provability, hand-proved by mutants run on a COPY: the
        # module itself is never written (the mutation rides an in-memory
        # exec of a mutated source string), and the on-disk module is
        # re-read byte-identical after each run.
        with open(wrap_assert.__file__, "r", encoding="utf-8") as f:
            pristine = f.read()
        falsified = (
            "the strict post-reset streak reads 4 TRUE on THIS stream's "
            "trim-0 at the stamp"
        )
        with tempfile.TemporaryDirectory() as tmp:
            events_dir = _t321_build_events_dir(
                tmp, T321_3024_STREAMS, live=T321_3024_LIVE
            )
            # The shipped module fires on both inputs (the legs above pin
            # it; re-asserted here so the mutant contrast is
            # self-contained).
            with self.assertRaises(AssertionError):
                wrap_assert.assert_fill_trim_streak(
                    falsified, events_dir, T321_STAMP
                )
            # M1 — the figure compare deleted (every composed claim
            # redeems): the 3024-shape fire goes red (no raise under the
            # mutant, where the shipped leg raises).
            m1 = _mutant_wrap_assert(
                pristine,
                "        if figure != true_figure:",
                "        if False:",
            )
            m1["assert_fill_trim_streak"](
                falsified, events_dir, T321_STAMP
            )
            # M2 — the T320 identity check deleted (a zero-run_start
            # rotated file silently miscounts into the chain): the
            # identity fire goes red (no raise under the mutant, where
            # the shipped leg raises on the same input — the corrupted
            # file counted into the chain yields TRUE 3, which the
            # claim matches, so ONLY the identity tooth separates the
            # shipped leg's raise from the mutant's pass).
            m2 = _mutant_wrap_assert(
                pristine,
                "        if len(run_starts) != 1:",
                "        if False:",
            )
            zero_rs = os.path.join(
                events_dir, "events-20261105-140840.jsonl"
            )
            _t321_strip_run_start_lines(zero_rs)
            m2["assert_fill_trim_streak"](
                "the strict post-reset streak reads 3 TRUE",
                events_dir,
                T321_STAMP,
            )
        # The module restored byte-identical after each run (nothing was
        # ever written — re-read and compared against the pristine read).
        with open(wrap_assert.__file__, "r", encoding="utf-8") as f:
            self.assertEqual(f.read(), pristine)

    def test_boundary_trim_1_resetter_stops_the_walk_and_names_it(self):
        # The T322 finding-2 boundary (the M7 weak-test survivor): the
        # resetting stream's whole-final trim count EXACTLY 1 — the modal
        # real-corpus resetter shape (a one-post-stamp-trim stream), the
        # boundary value every T321 fixture overshot with count 2 —
        # STOPS the walk and is NAMED: the chain counts only the trim-0
        # streams newer than it (2), TRUE 3 at the stamp. RED-proof: the
        # `>= 1` threshold flipped to `>= 2` walks PAST the trim-1
        # stream — the TRUE `reads 3` fixture fires under the mutant
        # (probed 4, no resetting stream named) and rides green shipped.
        true_text = (
            "the strict post-reset streak reads 3 TRUE on THIS "
            "stream's trim-0 at the stamp"
        )
        with tempfile.TemporaryDirectory() as tmp:
            events_dir = _t321_build_events_dir(
                tmp, T322_TRIM1_STREAMS, live=T322_TRIM1_LIVE
            )
            wrap_assert.assert_fill_trim_streak(
                true_text, events_dir, T322_TRIM1_STAMP
            )
            with self.assertRaises(AssertionError) as ctx:
                wrap_assert.assert_fill_trim_streak(
                    "the strict post-reset streak reads 4 TRUE on THIS "
                    "stream's trim-0 at the stamp",
                    events_dir,
                    T322_TRIM1_STAMP,
                )
            msg = str(ctx.exception)
            self.assertIn("reads 4", msg)
            self.assertIn("the probed 3", msg)
            self.assertIn("prior chain 2", msg)
            # the walk STOPPED at the trim-1 stream: it is the named
            # resetting stream (its ONE trim leg resets the streak).
            self.assertIn("events-20261105-093000.jsonl", msg)
            self.assertIn("2026-11-05T08:30:00.000Z", msg)
            self.assertIn("resetting stream", msg)
        # RED-proof on a copy (the module never written): the `>= 1` ->
        # `>= 2` threshold flip walks past the trim-1 resetter.
        with open(wrap_assert.__file__, "r", encoding="utf-8") as f:
            pristine = f.read()
        m3 = _mutant_wrap_assert(
            pristine,
            "        if _streak_trim_count(events) >= 1:",
            "        if _streak_trim_count(events) >= 2:",
        )
        with tempfile.TemporaryDirectory() as tmp:
            events_dir = _t321_build_events_dir(
                tmp, T322_TRIM1_STREAMS, live=T322_TRIM1_LIVE
            )
            with self.assertRaises(AssertionError) as ctx:
                m3["assert_fill_trim_streak"](
                    true_text, events_dir, T322_TRIM1_STAMP
                )
            m_msg = str(ctx.exception)
            self.assertIn("reads 3", m_msg)
            self.assertIn("the probed 4", m_msg)
            self.assertIn(
                "no resetting stream in the enumerated window", m_msg
            )
        with open(wrap_assert.__file__, "r", encoding="utf-8") as f:
            self.assertEqual(f.read(), pristine)

    def test_boundary_trim_ts_equal_to_stamp_counts_in_at_stamp(self):
        # The T322 finding-3 boundary (the M4 weak-test survivor): the
        # current stream's trim leg whose ts string EXACTLY equals the
        # fill stamp (identical strings — the T321 fixture's `.` < `Z`
        # subsecond shape pinned the comparison without ever pinning the
        # equality arm) is INCLUDED in the at-stamp count: at-stamp 1
        # fails the extension's precondition, TRUE reads the bare chain
        # 2, and a claim composed as if the equality leg were excluded
        # (3) fires. RED-proof: `<=` flipped to `<` EXCLUDES the
        # equality leg — the TRUE `reads 2` fixture fires under the
        # mutant (probed 3) and rides green shipped.
        true_text = (
            "trim 1 == 1 [the trim leg at the stamp — the harness's "
            "automatic context trim; the strict post-reset streak "
            "reads 2 TRUE on THIS stream's trim-1 at the stamp]"
        )
        with tempfile.TemporaryDirectory() as tmp:
            events_dir = _t321_build_events_dir(
                tmp, T322_EQUALITY_STREAMS, live=T322_EQUALITY_LIVE
            )
            wrap_assert.assert_fill_trim_streak(
                true_text, events_dir, T322_EQUALITY_STAMP
            )
            with self.assertRaises(AssertionError) as ctx:
                wrap_assert.assert_fill_trim_streak(
                    "the strict post-reset streak reads 3 TRUE on THIS "
                    "stream's trim-0 at the stamp",
                    events_dir,
                    T322_EQUALITY_STAMP,
                )
            msg = str(ctx.exception)
            self.assertIn("reads 3", msg)
            self.assertIn("the probed 2", msg)
            self.assertIn("prior chain 2", msg)
        # RED-proof on a copy: the `<=` -> `<` flip drops the equality
        # leg from the at-stamp count.
        with open(wrap_assert.__file__, "r", encoding="utf-8") as f:
            pristine = f.read()
        m4 = _mutant_wrap_assert(
            pristine,
            "        if ts <= stamp:",
            "        if ts < stamp:",
        )
        with tempfile.TemporaryDirectory() as tmp:
            events_dir = _t321_build_events_dir(
                tmp, T322_EQUALITY_STREAMS, live=T322_EQUALITY_LIVE
            )
            with self.assertRaises(AssertionError) as ctx:
                m4["assert_fill_trim_streak"](
                    true_text, events_dir, T322_EQUALITY_STAMP
                )
            m_msg = str(ctx.exception)
            self.assertIn("reads 2", m_msg)
            self.assertIn("the probed 3", m_msg)
        with open(wrap_assert.__file__, "r", encoding="utf-8") as f:
            self.assertEqual(f.read(), pristine)

    def test_scoping_identity_check_reads_only_the_walked_window(self):
        # The T322 finding-1 scoping: the one-run_start identity check
        # binds ONLY the files the chain walk inspects. (a) A legacy
        # two-run_start file OLDER than the resetting stream — outside
        # the walked window, the live main corpus's Sept-2026 shape that
        # refused every T321-sweep fill — is never read and never
        # identity-checked: the leg derives past it. (b) The same
        # corruption INSIDE the window still fails closed BY NAME (the
        # T320 precondition binds what the walk trusts). (c) An
        # `events-tNNN-*` task-harvest file is excluded from the
        # enumeration by the cycle-stream name shape — a two-run_start
        # harvest file in the dir is never read either. RED-proofs: the
        # walk-stop deleted (break -> pass) reaches the outside-window
        # file and fires; the shape check deleted lets the harvest file
        # into the enumeration (newest mtime, walked FIRST) and fires.
        with open(wrap_assert.__file__, "r", encoding="utf-8") as f:
            pristine = f.read()
        scoping_text = (
            "the strict post-reset streak reads 2 TRUE on THIS "
            "stream's trim-0 at the stamp"
        )
        # (a) outside the walked window: no fire — the leg derives
        # chain 1 (104500 trim-0), stops at the trim-1 stream 093000,
        # TRUE 2 at the stamp.
        with tempfile.TemporaryDirectory() as tmp:
            events_dir = _t321_build_events_dir(
                tmp, T322_SCOPING_STREAMS, live=T322_SCOPING_LIVE
            )
            corrupt = os.path.join(
                events_dir, "events-20261105-080000.jsonl"
            )
            with open(corrupt, "a", encoding="utf-8") as f:
                f.write(json.dumps(
                    {"type": "run_start",
                     "ts": "2026-11-05T07:10:00.000Z"}) + "\n")
            # the append bumped the mtime to NOW — re-pin the fixture's
            # stream-order mtime (the ls/mtime enumeration is the stream
            # order; the corrupt file stays OLDEST, outside the window).
            os.utime(corrupt, (1_700_000_000, 1_700_000_000))
            wrap_assert.assert_fill_trim_streak(
                scoping_text, events_dir, T322_SCOPING_STAMP
            )
            # RED-proof (a): the walk-stop deleted, the walk reads PAST
            # the resetting stream — the outside-window corrupt file is
            # reached and the identity fire names it.
            m_break = _mutant_wrap_assert(
                pristine,
                "            resetter = (name, run_starts[0])\n"
                "            break",
                "            resetter = (name, run_starts[0])\n"
                "            pass",
            )
            with self.assertRaises(AssertionError) as ctx:
                m_break["assert_fill_trim_streak"](
                    scoping_text, events_dir, T322_SCOPING_STAMP
                )
            self.assertIn(
                "events-20261105-080000.jsonl", str(ctx.exception)
            )
            # (b) the same corruption INSIDE the window (the walk
            # reaches the file before stopping) fails closed BY NAME.
            with open(os.path.join(
                events_dir, "events-20261105-104500.jsonl"
            ), "a", encoding="utf-8") as f:
                f.write(json.dumps(
                    {"type": "run_start",
                     "ts": "2026-11-05T09:40:00.000Z"}) + "\n")
            os.utime(
                os.path.join(events_dir, "events-20261105-104500.jsonl"),
                (1_700_000_002, 1_700_000_002),
            )
            with self.assertRaises(AssertionError) as ctx:
                wrap_assert.assert_fill_trim_streak(
                    scoping_text, events_dir, T322_SCOPING_STAMP
                )
            msg = str(ctx.exception)
            self.assertIn("events-20261105-104500.jsonl", msg)
            self.assertIn("probed 2 run_start event(s)", msg)
        # (c) the task-harvest name shape excluded from the enumeration.
        with tempfile.TemporaryDirectory() as tmp:
            events_dir = _t321_build_events_dir(
                tmp, T322_SCOPING_STREAMS, live=T322_SCOPING_LIVE
            )
            harvest = os.path.join(events_dir, T322_HARVEST_NAME)
            with open(harvest, "w", encoding="utf-8") as f:
                f.write("\n".join([
                    json.dumps({"type": "run_start",
                                "ts": "2026-11-05T12:00:00.000Z"}),
                    json.dumps({"type": "run_start",
                                "ts": "2026-11-05T12:01:00.000Z"}),
                ]) + "\n")
            os.utime(harvest, (1_700_000_100, 1_700_000_100))
            wrap_assert.assert_fill_trim_streak(
                scoping_text, events_dir, T322_SCOPING_STAMP
            )
            # RED-proof (c): the shape check deleted, the harvest file
            # enters the enumeration and the walk reads it FIRST.
            m_shape = _mutant_wrap_assert(
                pristine,
                "        if _streak_is_cycle_stream_name(name):",
                "        if True:",
            )
            with self.assertRaises(AssertionError) as ctx:
                m_shape["assert_fill_trim_streak"](
                    scoping_text, events_dir, T322_SCOPING_STAMP
                )
            m_msg = str(ctx.exception)
            self.assertIn(T322_HARVEST_NAME, m_msg)
            self.assertIn("probed 2 run_start event(s)", m_msg)
        with open(wrap_assert.__file__, "r", encoding="utf-8") as f:
            self.assertEqual(f.read(), pristine)


# ---- T323: the fill eval-state-untracked leg (assert_fill_eval_state_untracked) ----

# The cycle-3024 shape: the wrap's notes commit (11f26ea) force-added
# `.chug/eval-state.md` into git tracking (self-caught at the notes
# commit's own stat read-back; repaired in ac4994f per the bb339d9
# precedent — `git rm --cached`, the file byte-untouched on disk, the
# `.gitignore:12` ignore re-covering it, `git ls-files` back to ZERO).
# The class's SECOND fire: a tracked disk-only state file breaks
# `delta_bookkeeping_only`'s file leg (a tracked path outside the
# bookkeeping set moves every cycle — the wrap's state rewrite touches
# the file), so book=no on every subsequent cycle, the T258 cheap-exit
# predicate can never hold, and the chain silently breaks into sustained
# forced launches. The leg asks GIT, never the filesystem and never the
# model's say-so; every fixture below is a THROWAWAY git repo in a temp
# dir — the live repo's git state is never read, never mutated.
#
# _t323_git / _t323_init_repo — the fixture builders: a committed seed
# tree, a `.gitignore` covering `.chug/` (the live repo's shape — the
# repair story's re-covering line), and `.chug/` present on disk. The
# force-add rides `git add -f` (the gitignore-covered path's only route
# into the index — the 11f26ea shape).


def _t323_git(repo, *args):
    proc = subprocess.run(
        ["git", "-C", repo, *args], capture_output=True, text=True
    )
    assert proc.returncode == 0, (
        f"fixture git {' '.join(args)} failed: {proc.stderr}"
    )
    return proc


def _t323_init_repo(tmp):
    """A fixture git repo at *tmp*/repo: one committed seed tree
    (`README.md` + the `.chug/`-covering `.gitignore`), `.chug/` present
    on disk. Returns the repo path (the leg's repo_root argument)."""
    repo = os.path.join(tmp, "repo")
    os.makedirs(os.path.join(repo, ".chug"))
    _t323_git(repo, "init", "-q")
    _t323_git(repo, "config", "user.email", "t323@example.invalid")
    _t323_git(repo, "config", "user.name", "T323 fixture")
    _t323_git(repo, "config", "commit.gpgsign", "false")
    _write(repo, "README.md", "the fixture's tracked seed\n")
    _write(repo, ".gitignore", ".chug/\n")
    _t323_git(repo, "add", "README.md", ".gitignore")
    _t323_git(repo, "commit", "-qm", "seed")
    return repo


class AssertFillEvalStateUntrackedTest(unittest.TestCase):
    """T323: the disk-only pair's GIT-STATE leg — the fill compose leg's
    pre-commit assert re-verifies the T260 disk-only pair
    (`.chug/eval-state.md` + `.chug/eval-delta.md`) stays untracked at
    the fill stamp (the cycle-1427 class's SECOND fire closed: the
    cycle-3024 wrap notes 11f26ea force-added the state file and broke
    the cheap-exit file leg until the ac4994f repair)."""

    def test_export_and_docstring_coverage_sentence_pinned(self):
        # The leg rides the module's __all__ (20 -> 21) and the docstring
        # coverage sentence names the T323 legs (the pin-surface
        # extended, not re-shaped).
        self.assertIn(
            "assert_fill_eval_state_untracked", wrap_assert.__all__
        )
        self.assertEqual(len(wrap_assert.__all__), 21)
        self.assertEqual(
            wrap_assert.__all__[-1], "assert_fill_eval_state_untracked"
        )
        self.assertIn(
            "T323 fill eval-state-untracked legs", wrap_assert.__doc__
        )
        # The extension, not a re-shape: the T321/T317 sentence tails
        # still ride.
        self.assertIn("T321 fill trim-streak legs", wrap_assert.__doc__)
        self.assertIn("T317 fill trim-enumeration legs", wrap_assert.__doc__)

    def test_fail_force_added_state_file_raises_naming_path_repair_consequence(
        self,
    ):
        # FAIL path, the cycle-3024 shape exactly: the notes-commit
        # force-add (`git add -f` through the gitignore) puts the state
        # file in the index — the leg RAISES naming (i) the tracked path
        # git reported, (ii) the consequence class
        # (`delta_bookkeeping_only`'s file leg — book=no forever, the
        # T258 cheap-exit predicate dead, the sustained forced-launch
        # burn), and (iii) the repair recipe (`git rm --cached` per the
        # bb339d9/ac4994f precedent, `.gitignore:12` re-covering).
        with tempfile.TemporaryDirectory() as tmp:
            repo = _t323_init_repo(tmp)
            _write(repo, ".chug/eval-state.md", "schema: 1\n")
            _t323_git(repo, "add", "-f", ".chug/eval-state.md")
            with self.assertRaises(AssertionError) as ctx:
                wrap_assert.assert_fill_eval_state_untracked(repo)
        msg = str(ctx.exception)
        self.assertIn(".chug/eval-state.md", msg)  # (i) the tracked path
        self.assertIn("delta_bookkeeping_only", msg)  # (ii) the file leg
        self.assertIn("book=no", msg)  # (ii) the consequence class
        self.assertIn("cheap-exit", msg)  # (ii) the predicate it kills
        self.assertIn(
            "git rm --cached .chug/eval-state.md", msg
        )  # (iii) the repair recipe
        self.assertIn(".gitignore:12", msg)  # (iii) the re-covering line
        self.assertIn("bb339d9", msg)  # the precedent named

    def test_pass_present_on_disk_but_untracked(self):
        # PASS path, the T260 disk-only convention: both files PRESENT ON
        # DISK and absent from the index (gitignored) — the DESIGNED
        # shape. The leg asks git, not the filesystem: on-disk presence
        # alone never fires.
        with tempfile.TemporaryDirectory() as tmp:
            repo = _t323_init_repo(tmp)
            _write(repo, ".chug/eval-state.md", "schema: 1\n")
            _write(repo, ".chug/eval-delta.md", "delta\n")
            self.assertTrue(os.path.exists(os.path.join(
                repo, ".chug/eval-state.md"
            )))
            wrap_assert.assert_fill_eval_state_untracked(repo)

    def test_fail_both_tracked_raises_naming_both_paths(self):
        # FAIL path, the pair fully tracked: ONE assert covers the pair —
        # the fire names BOTH tracked paths (the probed actuals, never a
        # bare count).
        with tempfile.TemporaryDirectory() as tmp:
            repo = _t323_init_repo(tmp)
            _write(repo, ".chug/eval-state.md", "schema: 1\n")
            _write(repo, ".chug/eval-delta.md", "delta\n")
            _t323_git(
                repo, "add", "-f",
                ".chug/eval-state.md", ".chug/eval-delta.md",
            )
            with self.assertRaises(AssertionError) as ctx:
                wrap_assert.assert_fill_eval_state_untracked(repo)
        msg = str(ctx.exception)
        self.assertIn(".chug/eval-state.md", msg)
        self.assertIn(".chug/eval-delta.md", msg)
        self.assertIn("2 path(s)", msg)  # both probed actuals counted
        self.assertIn(
            "git rm --cached .chug/eval-delta.md", msg
        )  # the repair names BOTH too

    def test_fail_delta_alone_tracked_raises(self):
        # FAIL path, the pair's OTHER half: `.chug/eval-delta.md` alone
        # tracked raises the same fire (rebuilt by loopd before every
        # cycle, so a tracked delta breaks the file leg identically) —
        # and names ONLY the delta (the state file's absence from the
        # fire is the probe's own output, never a hardcode).
        with tempfile.TemporaryDirectory() as tmp:
            repo = _t323_init_repo(tmp)
            _write(repo, ".chug/eval-delta.md", "delta\n")
            _t323_git(repo, "add", "-f", ".chug/eval-delta.md")
            with self.assertRaises(AssertionError) as ctx:
                wrap_assert.assert_fill_eval_state_untracked(repo)
        msg = str(ctx.exception)
        self.assertIn(".chug/eval-delta.md", msg)
        self.assertNotIn(
            ".chug/eval-state.md", msg
        )  # only the tracked half named

    def test_fail_non_repo_root_fails_closed(self):
        # FAIL path, the invocation failure: a non-repo root sends git
        # rc-nonzero — the leg FAILS CLOSED (AssertionError naming the
        # probe and git's stderr), never a silently-green pass: a
        # probed-nothing run cannot wave the fill's untracked claim
        # GREEN.
        with tempfile.TemporaryDirectory() as tmp:
            with self.assertRaises(AssertionError) as ctx:
                wrap_assert.assert_fill_eval_state_untracked(tmp)
        msg = str(ctx.exception)
        self.assertIn("git", msg)  # the probe named
        self.assertIn("ls-files", msg)  # the probe's command named
        self.assertIn("not a git repository", msg)  # git's stderr named
        self.assertIn("fails closed", msg)  # the fail-closed posture

    def test_pass_harvest_exception_shape_stays_green(self):
        # PASS path, the harvest exception: a tracked
        # `.chug/events-t*` file (the force-added harvest convention —
        # the DESIGNED exception) with the state/delta pair untracked
        # rides GREEN — the leg's scope is EXACTLY the disk-only pair,
        # never the whole `.chug/` tree.
        with tempfile.TemporaryDirectory() as tmp:
            repo = _t323_init_repo(tmp)
            _write(repo, ".chug/eval-state.md", "schema: 1\n")
            _write(repo, ".chug/eval-delta.md", "delta\n")
            harvest = ".chug/events-t999-impl-20000101.jsonl"
            _write(repo, harvest, '{"type":"run_start"}\n')
            _t323_git(repo, "add", "-f", harvest)
            _t323_git(repo, "commit", "-qm", "harvest")
            wrap_assert.assert_fill_eval_state_untracked(repo)

    def test_fail_non_str_repo_root_raises_type_error_naming_the_shape(self):
        # The interface defense: *repo_root* is the REPO TOPLEVEL's PATH
        # (a str — the probe runs `-C <repo_root>`); anything else is the
        # call-signature slip shape and TypeErrors naming the accepted
        # shape (the module's convention).
        with self.assertRaises(TypeError) as ctx:
            wrap_assert.assert_fill_eval_state_untracked(7)
        msg = str(ctx.exception)
        self.assertIn("REPO TOPLEVEL", msg)
        self.assertIn("a str", msg)
        self.assertIn("type int", msg)

    def test_red_provability_mutant_runs_on_a_copy(self):
        # RED-provability, hand-proved by a mutant run on a COPY: the
        # module itself is never written (the mutation rides an
        # in-memory exec of a mutated source string), and the on-disk
        # module is re-read byte-identical after the run. M1 — the
        # tracked-hit fire silent-passed: the cycle-3024 force-add shape
        # goes red (no raise under the mutant, where the shipped leg
        # raises).
        with open(wrap_assert.__file__, "r", encoding="utf-8") as f:
            pristine = f.read()
        with tempfile.TemporaryDirectory() as tmp:
            repo = _t323_init_repo(tmp)
            _write(repo, ".chug/eval-state.md", "schema: 1\n")
            _t323_git(repo, "add", "-f", ".chug/eval-state.md")
            # The shipped module fires on this input (the leg above pins
            # it; re-asserted here so the mutant contrast is
            # self-contained).
            with self.assertRaises(AssertionError):
                wrap_assert.assert_fill_eval_state_untracked(repo)
            m1 = _mutant_wrap_assert(
                pristine,
                "        _fire_eval_state_tracked(tracked)",
                "        pass",
            )
            m1["assert_fill_eval_state_untracked"](repo)
        # The module restored byte-identical after the run (nothing was
        # ever written — re-read and compared against the pristine read).
        with open(wrap_assert.__file__, "r", encoding="utf-8") as f:
            self.assertEqual(f.read(), pristine)


# ---- T324: the one-liner anchor skip-rule (the T316 first-occurrence
# coverage gap) — the 19th/20th/21st production exercises' guard-fires
# (the re-arm `re-arm(ed)? at the` clause-mention latches at the
# cycle-3024/3025 wraps, the establishment bare-prose-mention latch at
# the cycle-3029 wrap), each adjudicated EQUAL BY HAND at zero casualty;
# the skip-rule makes the pair extraction mechanical for the FUTURE
# compactions. No committed text is edited. ----

# The three production fires: (cycle, the EVALUATION.md commit carrying
# the full entry the guard exercised, the exercise number). The full
# entry side lives in git history (the compaction shed it); the
# one-liner side is the committed line in the live tree.
_T324_FIRES = (
    (3000, "d26e221", 19),  # the cycle-3024 wrap: the re-arm clause latch
    (3004, "deea924", 20),  # the cycle-3025 wrap: the re-arm clause latch
    (3008, "9ae8c1a", 21),  # the cycle-3029 wrap: the establishment prose latch
)


def _t324_repo_root():
    """The repo root — this test file lives at <root>/scripts/."""
    return os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def _t324_git_eval_text(ref):
    """EVALUATION.md's text at *ref* — the committed corpus, probed at
    test time (the T312 probe-then-assert discipline). Fails CLOSED with
    a named detail when git refuses (the ref absent, not a repo)."""
    proc = subprocess.run(
        ["git", "-C", _t324_repo_root(), "show", f"{ref}:EVALUATION.md"],
        capture_output=True, text=True,
    )
    if proc.returncode != 0 or not proc.stdout:
        raise AssertionError(
            f"the T324 committed-corpus probe failed — `git show "
            f"{ref}:EVALUATION.md` rc {proc.returncode} "
            f"({proc.stderr.strip()})"
        )
    return proc.stdout


def _t324_live_eval_text():
    """The LIVE tree's EVALUATION.md — the corpus the guard exercises at
    every wrap prep (the T324 spec's leg (d) brings it into the battery;
    the corpus region asserted is the template-era surface only)."""
    path = os.path.join(_t324_repo_root(), "EVALUATION.md")
    with open(path, "r", encoding="utf-8") as f:
        return f.read()


def _t324_entry_lines(text):
    """The `### Cycle ` Outcomes entry lines of an EVALUATION.md text."""
    return [l for l in text.splitlines() if l.startswith("### Cycle ")]


def _t324_one_liner(entries, cycle):
    """The committed one-liner entry for *cycle* — exactly one line
    carrying the last-6-compaction marker (the corpus convention)."""
    hits = [
        l for l in entries
        if l.startswith(f"### Cycle {cycle} ")
        and "one line per the last-6 compaction" in l
    ]
    if len(hits) != 1:
        raise AssertionError(
            f"the T324 one-liner probe for cycle {cycle} expected exactly "
            f"one committed one-liner line, probed {len(hits)}"
        )
    return hits[0]


def _t324_current_full_set(entries, count=6):
    """The current kept-full set — the FIRST *count* entry lines that do
    NOT carry the compaction marker (the last-6 convention: the six
    newest entries stay full, everything older is one-lined)."""
    fulls = [l for l in entries if "one line per the last-6 compaction" not in l]
    if len(fulls) < count:
        raise AssertionError(
            f"the T324 live-corpus smoke expected >= {count} full entries "
            f"in the live tree, probed {len(fulls)}"
        )
    return fulls[:count]


def _t324_discharged_ordinal(text):
    """The own discharge's ordinal derived INDEPENDENTLY of the extractor:
    the first uppercase `DISCHARGED` keyword (the corpus's own-discharge
    rendering `re-arm <N>th DISCHARGED EXACT`), the `<N>th` token walked
    back over whitespace. Fails closed with a named detail."""
    i = text.find("DISCHARGED")
    if i < 0:
        raise AssertionError(
            "the T324 DISCHARGED-derived cross-check found no `DISCHARGED` "
            "keyword in the entry"
        )
    j = i
    while j > 0 and text[j - 1] in " \t":
        j -= 1
    k = j
    while k > 0 and text[k - 1].isalpha():
        k -= 1
    if text[k:j] not in ("st", "nd", "rd", "th"):
        raise AssertionError(
            "the T324 DISCHARGED-derived cross-check found no `<N>th` "
            "ordinal token before the first `DISCHARGED` keyword"
        )
    m = k
    while m > 0 and text[m - 1].isdigit():
        m -= 1
    if m == k:
        raise AssertionError(
            "the T324 DISCHARGED-derived cross-check found no digits in "
            "the ordinal token before the first `DISCHARGED` keyword"
        )
    return int(text[m:k])


def _t324_first_probed_rearm(text):
    """The first `re-arm` occurrence the PRE-CHANGE (all-T324-skips-
    disabled) scan probes — the boundary refusal (an alnum/hyphen join)
    is pre-T324 (the mutant keeps it), so the walk mirrors it exactly;
    -1 when no occurrence survives. The T324 skip classes (the
    `re-arm(ed)? at the` clause-mentions and the backtick-quoted term
    mentions) are deliberately NOT refused here: the pre-change scan
    probes them, and a prior!=shipped delta riding such an occurrence is
    the skips' DESIGNED extra-probe recovery, not a behavior change (the
    T333 live-corpus-drift carve-out in the no-behavior-change pin)."""
    start = 0
    while True:
        idx = text.find("re-arm", start)
        if idx < 0:
            return -1
        if idx > 0 and (text[idx - 1].isalnum() or text[idx - 1] == "-"):
            start = idx + len("re-arm")
            continue
        return idx


# The composed full-entry shapes (legs a/b/c) — minimal, mirroring the
# committed renderings, each carrying exactly one non-anchor occurrence
# where the shape demands it.

# (a) the establishment bare prose mention BEFORE the real group anchor
# (the 21st fire's shape — the cycle-3008 full entry's carried-figure
# triage prose).
_T324_FULL_ESTABLISHMENT_MENTION = (
    "### Cycle 3999 (2099-01-01) — the fire replay: ONE eval-triage record "
    "d1799999999-2 (the carried-figure class's sighting, the FIRST on the "
    "fill-datum-citation surface: the committed citation FALSIFIED vs the "
    "fill message's TRUE datum; ZERO consequence [the establishment figure "
    "125/3,411 unaffected and TRUE; the falsified figure contained to the "
    "corridor text]; weighed -> DECLINED a doctrine row). The eval: the "
    "692nd establishment (3004 stream 108/2,707 re-derived CONCURS off "
    "events-20990101; ok:false 2 == 2; goal 1; the 102-bound FIRES [108 >= "
    "the bound — the census 47 -> 48, the non-fire streak RESETS 3 -> 0]). "
    "Verdict: eval + 0 rows, queue EMPTY."
)

# (b) the retrospective re-arm clause-mention BEFORE the discharge anchor
# (the 19th/20th fires' shape — the class-accounting prose, then the own
# discharge; a stray digit rides between them so the pre-change
# first-occurrence probe provably latches the WRONG figure).
_T324_FULL_REARM_CLAUSE = (
    "### Cycle 3998 (2099-01-01) — the fire replay: ONE eval-triage record "
    "(the carried-figure class's sighting — DECLINED, the class re-armed "
    "at the FIFTH per the 21st production exercise); the second net -> "
    "551. The eval: re-arm 619th DISCHARGED EXACT (2935 slot 90/1,509 per "
    "the lattice's 619th slot committed; (b) NEGATIVE — 90 < the 102 "
    "bound [the watch HOLDS 63]). Verdict: eval + 0 rows, queue EMPTY."
)

# (c) the non-anchor-only texts — the vacuous-pass class stays closed:
# each carries ONLY non-anchor occurrences, so the extractor must fail
# closed with the T324-named detail.
_T324_ONLY_ESTABLISHMENT_MENTION = (
    "### Cycle 3997 (2099-01-01) — the mention-only replay: ZERO "
    "consequence [the establishment figure 125/3,411 unaffected and TRUE]."
)
_T324_ONLY_ESTABLISHMENT_COMPOUND = (
    "### Cycle 3996 (2099-01-01) — the compound-only replay: the cycle "
    "carried the 695th-establishment anchor-label slip observation."
)
_T324_ONLY_REARM_CLAUSE = (
    "### Cycle 3995 (2099-01-01) — the clause-only replay: DECLINED, the "
    "class re-armed at the FIFTH."
)
_T324_ONLY_REARM_QUOTED = (
    "### Cycle 3994 (2099-01-01) — the quoted-only replay: the triage "
    "named the first-`re-arm` coverage gap and filed the skip-rule."
)


class AssertOutcomesOneLinerAnchorSkipRuleTest(unittest.TestCase):
    """T324: the anchor skip-rule — the extractor probes the group's OWN
    anchor past non-anchor mentions, and still fails closed when only
    non-anchor occurrences exist."""

    def test_pass_composed_establishment_mention_probes_group_anchor(self):
        # Leg (a): the bare `the establishment figure 125/3,411` prose
        # mention rides BEFORE the real group anchor — the extractor must
        # probe the group anchor's ordinal/stream/figures/census, not the
        # mention's figures and not a failure.
        value, detail = wrap_assert._ol_establishment_figures(
            _T324_FULL_ESTABLISHMENT_MENTION
        )
        self.assertIsNone(detail)
        self.assertEqual(value, (692, 3004, 108, 2707, 47, 48))

    def test_pass_composed_rearm_clause_probes_discharge_anchor(self):
        # Leg (b): the retrospective `the class re-armed at the FIFTH`
        # clause rides BEFORE the own discharge anchor (with a stray
        # `21st` digit between) — the extractor must probe the discharge
        # ordinal 619, not the clause region's stray figure.
        value, detail = wrap_assert._ol_rearm_figure(_T324_FULL_REARM_CLAUSE)
        self.assertIsNone(detail)
        self.assertEqual(value, (619,))

    def test_fail_closed_only_non_anchor_occurrences(self):
        # Leg (c): texts whose ONLY occurrences are non-anchors — the
        # vacuous-pass class stays closed: fail-closed with the T324-named
        # detail (the detail NAMES the skip classes, so the assertion is
        # RED against the pre-change detail strings too).
        value, detail = wrap_assert._ol_establishment_figures(
            _T324_ONLY_ESTABLISHMENT_MENTION
        )
        self.assertIsNone(value)
        self.assertIn("no own-establishment anchor probed", detail)
        self.assertIn("T324", detail)
        self.assertIn("no adjacent `<N>th` ordinal", detail)
        value, detail = wrap_assert._ol_establishment_figures(
            _T324_ONLY_ESTABLISHMENT_COMPOUND
        )
        self.assertIsNone(value)
        self.assertIn("no own-establishment anchor probed", detail)
        self.assertIn("T324", detail)
        for only_text in (_T324_ONLY_REARM_CLAUSE, _T324_ONLY_REARM_QUOTED):
            value, detail = wrap_assert._ol_rearm_figure(only_text)
            self.assertIsNone(value)
            self.assertIn("no own-`re-arm` discharge anchor probed", detail)
            self.assertIn("T324", detail)
            self.assertIn("clause-mentions", detail)

    def test_pass_3008_compaction_pair_mechanical_equal(self):
        # The acceptance's named reproduction: the EXACT text pair that
        # guard-fired at the cycle-3029 wrap — the 3008 full entry (the
        # committed corpus at 9ae8c1a, probed from git) against the
        # committed 3008 one-liner (the live tree) — passes MECHANICALLY,
        # no by-hand adjudication.
        full = _t324_entry_lines(_t324_git_eval_text("9ae8c1a"))
        one_liner = _t324_one_liner(_t324_entry_lines(_t324_live_eval_text()), 3008)
        wrap_assert.assert_outcomes_one_liner(
            next(l for l in full if l.startswith("### Cycle 3008 ")), one_liner
        )

    def test_pass_3000_3004_compaction_pairs_mechanical_equal(self):
        # The 19th/20th exercises' pairs (the re-arm clause-mention
        # latches at the cycle-3024/3025 wraps) — mechanically equal the
        # same way.
        entries = _t324_entry_lines(_t324_live_eval_text())
        for cycle, ref, _fire in _T324_FIRES[:2]:
            full = _t324_entry_lines(_t324_git_eval_text(ref))
            one_liner = _t324_one_liner(entries, cycle)
            wrap_assert.assert_outcomes_one_liner(
                next(l for l in full if l.startswith(f"### Cycle {cycle} ")),
                one_liner,
            )

    def test_red_proof_establishment_skip_disabled_3008_pair_fires(self):
        # RED provability (in-battery, the mutant idiom): with the
        # establishment ordinal-adjacency skip disabled, the 3008 pair
        # FIRES naming the establishment group and the extraction
        # failure; the shipped module passes the same pair. The module is
        # byte-identical after (the mutation rides an in-memory exec).
        with open(wrap_assert.__file__, "r", encoding="utf-8") as f:
            pristine = f.read()
        full = next(
            l for l in _t324_entry_lines(_t324_git_eval_text("9ae8c1a"))
            if l.startswith("### Cycle 3008 ")
        )
        one_liner = _t324_one_liner(
            _t324_entry_lines(_t324_live_eval_text()), 3008
        )
        wrap_assert.assert_outcomes_one_liner(full, one_liner)
        skip_line = "        if _ol_ordinal_before(text, idx) is None:"
        self.assertEqual(pristine.count(skip_line), 1)
        mutant = _mutant_wrap_assert(pristine, skip_line, "        if False:")
        with self.assertRaises(AssertionError) as caught:
            mutant["assert_outcomes_one_liner"](full, one_liner)
        message = str(caught.exception)
        self.assertIn("'establishment'", message)
        self.assertIn("no `<N>th` ordinal adjacent", message)
        with open(wrap_assert.__file__, "r", encoding="utf-8") as f:
            self.assertEqual(f.read(), pristine)

    def test_red_proof_rearm_clause_skip_disabled_pairs_fire(self):
        # RED provability: with the clause-mention skip disabled, the
        # 3004/3000 pairs FIRE naming the re-arm group with the clause
        # region's wrong latched figures vs the one-liner's own discharge
        # ordinals (the exact adjudicated divergence); the shipped module
        # passes the same pairs.
        with open(wrap_assert.__file__, "r", encoding="utf-8") as f:
            pristine = f.read()
        entries = _t324_entry_lines(_t324_live_eval_text())
        clause_line = "        if _ol_rearm_clause_at(text, idx):"
        self.assertEqual(pristine.count(clause_line), 1)
        mutant = _mutant_wrap_assert(pristine, clause_line, "        if False:")
        for cycle, ref, _fire in _T324_FIRES[:2]:
            full = next(
                l for l in _t324_entry_lines(_t324_git_eval_text(ref))
                if l.startswith(f"### Cycle {cycle} ")
            )
            one_liner = _t324_one_liner(entries, cycle)
            wrap_assert.assert_outcomes_one_liner(full, one_liner)
            with self.assertRaises(AssertionError) as caught:
                mutant["assert_outcomes_one_liner"](full, one_liner)
            message = str(caught.exception)
            self.assertIn("'re-arm ordinal'", message)
        with open(wrap_assert.__file__, "r", encoding="utf-8") as f:
            self.assertEqual(f.read(), pristine)

    def test_red_proof_quoted_mention_skip_disabled_probes_wrong_figure(self):
        # RED provability (the live-corpus smoke's 4th-instance shape —
        # the committed 3025 full entry's backtick-quoted
        # `first-re-arm` TRIAGE prose): with the quoted-mention skip
        # disabled, the own-discharge cross-check diverges (the quoted
        # mention latches the record id's digits); the shipped module
        # probes the own discharge 625. The full entry reads from the
        # COMMITTED corpus at the T324-era ref — the `_t324_git_eval_text`
        # shape the 3008 fixture legs already use, corpus-position-
        # independent: the live kept-full window has rotated past 3025
        # (T333 — only the LIVE window lost the entry, the committed
        # text is immutable).
        with open(wrap_assert.__file__, "r", encoding="utf-8") as f:
            pristine = f.read()
        full_3025 = next(
            l for l in _t324_entry_lines(_t324_git_eval_text("9ae8c1a"))
            if l.startswith("### Cycle 3025 ")
        )
        value, detail = wrap_assert._ol_rearm_figure(full_3025)
        self.assertIsNone(detail)
        self.assertEqual(value, (625,))
        quoted_line = "        if _ol_rearm_quoted_at(text, idx):"
        self.assertEqual(pristine.count(quoted_line), 1)
        mutant = _mutant_wrap_assert(pristine, quoted_line, "        if False:")
        value, detail = mutant["_ol_rearm_figure"](full_3025)
        self.assertIsNotNone(value)
        self.assertNotEqual(value, (625,))
        with open(wrap_assert.__file__, "r", encoding="utf-8") as f:
            self.assertEqual(f.read(), pristine)

    def test_live_corpus_smoke_six_full_set_extracts_and_cross_checks(self):
        # Leg (d) — the live-corpus smoke: the CURRENT kept-full set's
        # committed entries (the six newest full entries, re-derived from
        # the live tree at test time). The window ADVANCES and is
        # routine-skip-dominated (T333): an entry carrying no
        # own-establishment anchor (the extractor's DESIGNED no-anchor
        # fail-closed), no own re-arm discharge anchor, or no `DISCHARGED`
        # keyword is a SKIP-LEGITIMATE shape, not a failure — the smoke
        # probes only the entries carrying BOTH own anchors plus the
        # keyword through the FULL path: the establishment group extracts
        # and the re-arm group extracts EQUAL to the independently
        # DISCHARGED-derived own-discharge ordinal (the wrong-latch class
        # fires here, not at a future prep). The probed >= 1 floor keeps
        # the smoke from going vacuous when the window is all-skips.
        # RED-proven via the two skip-disabled mutants above: the mention
        # classes they un-skip are exactly the non-anchor shapes this
        # smoke must not read as failures.
        fulls = _t324_current_full_set(_t324_entry_lines(_t324_live_eval_text()))
        probed = 0
        skipped = 0
        for full in fulls:
            cycle = full[len("### Cycle "):].split(" ")[0]
            value, detail = wrap_assert._ol_establishment_figures(full)
            if (
                detail is not None
                and "no own-establishment anchor probed" not in detail
            ):
                raise AssertionError(
                    f"the T324 live-corpus smoke: cycle {cycle}'s "
                    f"establishment group failed to extract ({detail})"
                )
            rearm, rearm_detail = wrap_assert._ol_rearm_figure(full)
            rearm_skip = "no own-`re-arm` discharge anchor probed"
            if rearm_detail is not None and rearm_skip not in rearm_detail:
                raise AssertionError(
                    f"the T324 live-corpus smoke: cycle {cycle}'s re-arm "
                    f"group failed to extract ({rearm_detail})"
                )
            if value is None or rearm is None or "DISCHARGED" not in full:
                # the no-anchor/no-discharged skip shape (the extractor's
                # designed semantics — T333): not a failure, not probed.
                skipped += 1
                continue
            self.assertIsInstance(value, tuple)
            self.assertIsInstance(rearm, tuple)
            expected = _t324_discharged_ordinal(full)
            self.assertEqual(
                rearm, (expected,),
                f"the T324 live-corpus smoke: cycle {cycle}'s re-arm "
                f"figure {rearm} != the DISCHARGED-derived own discharge "
                f"({expected},)",
            )
            probed += 1
        self.assertGreaterEqual(
            probed, 1,
            f"the T324 live-corpus smoke: zero of the {len(fulls)} "
            f"kept-full entries probed the full path ({skipped} "
            f"skip-legitimate no-anchor/no-discharged shapes) — the "
            f"smoke must not go vacuous (T333)",
        )

    def test_live_corpus_smoke_exercised_one_liners_still_extract(self):
        # Leg (d), the one-liner side: every one-liner the guard is
        # exercised against (the three production fires' committed
        # one-liners) still extracts ALL five groups — the extraction
        # result the guard compares EQUAL against its full entry (the
        # pair legs above pin the equality).
        entries = _t324_entry_lines(_t324_live_eval_text())
        for cycle, _ref, _fire in _T324_FIRES:
            one_liner = _t324_one_liner(entries, cycle)
            for group, extract in wrap_assert._ONE_LINER_GROUPS:
                value, detail = extract(one_liner)
                if detail is not None:
                    raise AssertionError(
                        f"the T324 live-corpus smoke: the cycle {cycle} "
                        f"one-liner's {group!r} group failed to extract "
                        f"({detail})"
                    )
                self.assertIsInstance(value, tuple)

    def test_no_behavior_change_when_first_occurrence_is_the_anchor(self):
        # The acceptance's no-behavior-change pin, dynamic over the live
        # corpus: with ALL THREE skips disabled (the pre-change
        # first-occurrence scan), every one-liner in the tree extracts
        # the SAME values as the shipped module — wherever the pre-change
        # scan succeeded, the shipped scan succeeds with the identical
        # figure (fail-detail rewordings and designed extra-probe
        # recoveries are the only permitted deltas). T333: the designed
        # extra-probe recoveries are ADJUDICATED at test time — a
        # prior!=shipped delta on the re-arm group whose first probed
        # occurrence is a T324 non-anchor mention (the clause/quoted
        # classes the skips were landed for) is the skips' designed
        # recovery, carved out; the anchor-first surface stays pinned.
        with open(wrap_assert.__file__, "r", encoding="utf-8") as f:
            pristine = f.read()
        skip_line = "        if _ol_ordinal_before(text, idx) is None:"
        clause_line = "        if _ol_rearm_clause_at(text, idx):"
        quoted_line = "        if _ol_rearm_quoted_at(text, idx):"
        for needle in (skip_line, clause_line, quoted_line):
            self.assertEqual(pristine.count(needle), 1)
        disabled = "        if False:  # the T324 skip disabled"
        mutated = (
            pristine.replace(skip_line, disabled)
            .replace(clause_line, disabled)
            .replace(quoted_line, disabled)
        )
        mutant = {"__name__": "wrap_assert_mutant"}
        exec(compile(mutated, "<wrap_assert-mutant>", "exec"), mutant)
        one_liners = [
            l for l in _t324_entry_lines(_t324_live_eval_text())
            if "one line per the last-6 compaction" in l
        ]
        self.assertGreaterEqual(len(one_liners), 200)
        checked = 0
        adjudicated = 0
        for one_liner in one_liners:
            for idx, (group, extract) in enumerate(
                wrap_assert._ONE_LINER_GROUPS
            ):
                shipped_value, _ = extract(one_liner)
                prior_value, _ = mutant["_ONE_LINER_GROUPS"][idx][1](one_liner)
                if prior_value is None:
                    continue
                if prior_value != shipped_value and group == "re-arm ordinal":
                    # The T333 live-corpus-drift carve-out: the mutant
                    # probes exactly what the shipped module skips, so a
                    # prior!=shipped delta is the DESIGNED extra-probe
                    # recovery exactly when the pre-change scan's first
                    # probed occurrence is a T324 non-anchor mention;
                    # when it IS the own anchor the scans read the same
                    # occurrence and any delta is a real behavior change
                    # (this assert fires). The committed 3029/3025/3024/
                    # 3020/3016/3012 one-liners ride the clause-mention
                    # shape today.
                    first = _t324_first_probed_rearm(one_liner)
                    if first >= 0 and (
                        wrap_assert._ol_rearm_clause_at(one_liner, first)
                        or wrap_assert._ol_rearm_quoted_at(one_liner, first)
                    ):
                        adjudicated += 1
                        continue
                self.assertEqual(
                    shipped_value, prior_value,
                    f"the T324 no-behavior-change pin: a one-liner's "
                    f"extraction changed for group {group!r} (pre-change "
                    f"{prior_value} vs shipped {shipped_value})",
                )
                checked += 1
        self.assertGreater(checked, 500)
        self.assertGreaterEqual(
            adjudicated, 1,
            "the T324 no-behavior-change pin: the T333 drift carve-out "
            "absorbed zero prior!=shipped deltas — the committed "
            "clause-mention one-liners must keep exercising it",
        )


if __name__ == "__main__":
    unittest.main()
