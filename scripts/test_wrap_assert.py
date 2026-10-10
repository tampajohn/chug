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


if __name__ == "__main__":
    unittest.main()
