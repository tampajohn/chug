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


if __name__ == "__main__":
    unittest.main()
