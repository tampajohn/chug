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


if __name__ == "__main__":
    unittest.main()
