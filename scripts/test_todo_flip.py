#!/usr/bin/env python3
"""test_todo_flip.py — T279: unittest coverage for scripts/todo_flip.py.

Per the carried conventions (scripts/test_wrap_assert.py): every assert in
the helper gets a PASS path and a FAIL path; each fail path asserts the
AssertionError fires AND names the probed actuals (the T270 reporting
requirement). The TWO regression pins of THIS row's firing shapes:

  - d1791632010-15 (the T275 flip): notes containing `|` — the T8 split
    hazard — rejected PRE-WRITE with the file left byte-identical;
  - d1791648496-11 (the T278 flip): notes containing a newline — the
    single-physical-line hazard — rejected PRE-WRITE with the file left
    byte-identical.

Plus the byte-carry PASS case (one row of a 5-row fixture flips, the other
four rows provably byte-identical) and the id-count failures (0 and 2)
naming the probed counts. The (f) FAIL path injects a splice defect via
the factored `_splice` (the production path is the only caller), proving
the IMPORTED assert_byte_carry has teeth and that the failure leaves the
file byte-identical.

Run (the check line): python3 -m unittest scripts.test_todo_flip -v
"""

import ast
import os
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

# The check line imports this file as `scripts.test_todo_flip` (a
# namespace-package path with the repo root on sys.path), so THIS module's
# own directory is not importable from there — insert it explicitly or
# `import todo_flip` fails. Also makes a direct
# `python3 scripts/test_todo_flip.py` run work.
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import todo_flip  # noqa: E402


def _write(tmpdir, name, text):
    path = os.path.join(tmpdir, name)
    with open(path, "w", encoding="utf-8") as f:
        f.write(text)
    return path


def _read(path):
    with open(path, "r", encoding="utf-8") as f:
        return f.read()


# The 5-row fixture: 5 data rows + header + separator, the real table's
# shape (6 cells per row; R2's notes cell empty).
FIVE_ROWS = (
    "# TODO\n"
    "\n"
    "| id | title | spec | pri | status | notes |\n"
    "|----|-------|------|-----|--------|-------|\n"
    "| R1 | first row | specs/r1.md | 1 | todo | seeded one |\n"
    "| R2 | second row | specs/r2.md | 2 | todo | |\n"
    "| R3 | third row | specs/r3.md | 3 | done | landed 9840aba |\n"
    "| R4 | fourth row | specs/r4.md | 4 | in-progress | working |\n"
    "| R5 | fifth row | specs/r5.md | 5 | todo | last |\n"
)
R2_OLD = "| R2 | second row | specs/r2.md | 2 | todo | |"
R2_FLIPPED = (
    "| R2 | second row | specs/r2.md | 2 | done | landed 4242abc — "
    "wrapped clean |"
)


class ImportShapeTest(unittest.TestCase):
    """stdlib-only plus the ONE sanctioned import (the T273 move): the
    `from wrap_assert import assert_byte_carry` line appears exactly once,
    and no third-party module is imported."""

    def test_wrap_assert_import_line_appears_exactly_once(self):
        with open(todo_flip.__file__, "r", encoding="utf-8") as f:
            source = f.read()
        self.assertEqual(
            source.count("from wrap_assert import assert_byte_carry"),
            1,
            "the wrap_assert import must appear exactly once — the byte-carry "
            "assert is imported, never re-authored (T273)",
        )

    def test_imports_are_stdlib_plus_wrap_assert_only(self):
        with open(todo_flip.__file__, "r", encoding="utf-8") as f:
            tree = ast.parse(f.read())
        modules = []
        for node in ast.walk(tree):
            if isinstance(node, ast.Import):
                modules.extend(alias.name.split(".")[0] for alias in node.names)
            elif isinstance(node, ast.ImportFrom) and node.level == 0 and node.module:
                modules.append(node.module.split(".")[0])
        self.assertEqual(
            sorted(set(modules)),
            ["argparse", "os", "sys", "wrap_assert"],
            "todo_flip.py must stay stdlib-only plus the one wrap_assert import",
        )


class RowLocationTest(unittest.TestCase):
    """Assert (a): FIRST-cell id match, strip-compared; probed count != 1
    names the id and the probed count, file untouched."""

    def test_pass_flips_the_matched_row(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = _write(tmp, "TODO.md", FIVE_ROWS)
            rebuilt = todo_flip.flip(path, "R2", notes="landed 4242abc — wrapped clean")
            self.assertEqual(rebuilt, R2_FLIPPED)
            self.assertIn(R2_FLIPPED, _read(path))
            self.assertIn(R2_FLIPPED, _read(path).split("\n")[5])  # R2's line

    def test_pass_match_is_strip_compared_on_the_first_cell(self):
        # The real TODO.md carries rows with a leading blank before the
        # pipe; the strip-compared first cell still matches.
        with tempfile.TemporaryDirectory() as tmp:
            padded = FIVE_ROWS.replace(
                "| R2 | second row", " | R2 | second row"
            )
            path = _write(tmp, "TODO.md", padded)
            todo_flip.flip(path, "R2", status="done")
            self.assertIn(
                "| R2 | second row | specs/r2.md | 2 | done |  |", _read(path)
            )

    def test_fail_zero_matches_names_the_id_and_probed_count(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = _write(tmp, "TODO.md", FIVE_ROWS)
            before = _read(path)
            with self.assertRaises(AssertionError) as ctx:
                todo_flip.flip(path, "NOPE", status="done")
            msg = str(ctx.exception)
            self.assertIn("'NOPE'", msg)  # the id named
            self.assertIn("probed 0 matching data row(s)", msg)  # the probed count
            self.assertEqual(_read(path), before)  # file untouched

    def test_fail_two_matches_name_the_probed_count(self):
        duplicated = FIVE_ROWS + (
            "| R2 | duplicate id row | specs/r2b.md | 6 | todo | dup |\n"
        )
        with tempfile.TemporaryDirectory() as tmp:
            path = _write(tmp, "TODO.md", duplicated)
            before = _read(path)
            with self.assertRaises(AssertionError) as ctx:
                todo_flip.flip(path, "R2", status="done")
            msg = str(ctx.exception)
            self.assertIn("'R2'", msg)
            self.assertIn("probed 2 matching data row(s)", msg)
            self.assertEqual(_read(path), before)

    def test_fail_header_and_separator_rows_are_never_matched(self):
        # The guard skips the header (first cell `id`) and the all-dash
        # separator; a flip keyed on either must probe 0 matches, never
        # rewrite the header row.
        with tempfile.TemporaryDirectory() as tmp:
            path = _write(tmp, "TODO.md", FIVE_ROWS)
            before = _read(path)
            for needle in ("id", "----"):
                with self.assertRaises(AssertionError) as ctx:
                    todo_flip.flip(path, needle, status="done")
                self.assertIn("probed 0 matching data row(s)", str(ctx.exception))
            self.assertEqual(_read(path), before)


class CellCountTest(unittest.TestCase):
    """Assert (b): the row parses into exactly 6 cells; probed count != 6
    names the probed count, file untouched."""

    def test_fail_seven_cells_names_the_probed_count(self):
        # The d1791632010-15 fire shape: a note glued after the trailing
        # pipe splits the row into 7 cells.
        seven = FIVE_ROWS.replace(
            R2_OLD, "| R2 | second row | specs/r2.md | 2 | todo | note | glued |"
        )
        with tempfile.TemporaryDirectory() as tmp:
            path = _write(tmp, "TODO.md", seven)
            before = _read(path)
            with self.assertRaises(AssertionError) as ctx:
                todo_flip.flip(path, "R2", status="done")
            msg = str(ctx.exception)
            self.assertIn("parses into 7 cells", msg)  # the probed count
            self.assertIn("need exactly 6", msg)
            self.assertIn("d1791632010-15", msg)  # the fire shape named
            self.assertEqual(_read(path), before)

    def test_fail_four_cells_names_the_probed_count(self):
        four = FIVE_ROWS.replace(R2_OLD, "| R2 | second row | specs/r2.md | 2 |")
        with tempfile.TemporaryDirectory() as tmp:
            path = _write(tmp, "TODO.md", four)
            before = _read(path)
            with self.assertRaises(AssertionError) as ctx:
                todo_flip.flip(path, "R2", status="done")
            msg = str(ctx.exception)
            self.assertIn("parses into 4 cells", msg)
            self.assertEqual(_read(path), before)

    def test_pass_six_cell_row_flips(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = _write(tmp, "TODO.md", FIVE_ROWS)
            todo_flip.flip(path, "R2", notes="landed 4242abc — wrapped clean")
            self.assertIn(R2_FLIPPED, _read(path))


class StatusCellTest(unittest.TestCase):
    """Assert (b2): the status must be one of the T8 guard's legal
    statuses; the default is `done`; a status-only flip keeps the notes."""

    def test_default_status_is_done_and_notes_cell_is_kept(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = _write(tmp, "TODO.md", FIVE_ROWS)
            rebuilt = todo_flip.flip(path, "R3", status="done")
            # R3 is already done — the status-only flip keeps its notes.
            self.assertEqual(
                rebuilt, "| R3 | third row | specs/r3.md | 3 | done | landed 9840aba |"
            )
            self.assertIn(rebuilt, _read(path))

    def test_pass_each_legal_status_accepted(self):
        for status in ("todo", "in-progress", "blocked", "done"):
            with tempfile.TemporaryDirectory() as tmp:
                path = _write(tmp, "TODO.md", FIVE_ROWS)
                todo_flip.flip(path, "R2", status=status)
                self.assertIn(
                    f"| R2 | second row | specs/r2.md | 2 | {status} |  |",
                    _read(path),
                )

    def test_fail_nonlegal_status_names_probed_status_and_legal_list(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = _write(tmp, "TODO.md", FIVE_ROWS)
            before = _read(path)
            with self.assertRaises(AssertionError) as ctx:
                todo_flip.flip(path, "R2", status="finished")
            msg = str(ctx.exception)
            self.assertIn("'finished'", msg)  # the probed status
            self.assertIn("legal statuses", msg)  # the legal list named
            self.assertIn("'in-progress'", msg)  # ...with a member shown
            self.assertEqual(_read(path), before)


class NotesRejectTest(unittest.TestCase):
    """Assert (c): `|` and newline notes rejected PRE-WRITE, the
    AssertionError naming the offending character and its probed
    position; clean notes still pass."""

    def test_regression_t275_pipe_notes_rejected_pre_write(self):
        # d1791632010-15 (the T275 flip): the `|` in the notes is the T8
        # split hazard — the guard splits every row on it.
        notes = "landed 9840aba — recipe `x | y` survived"
        self.assertIn("|", notes)  # the pin's premise
        with tempfile.TemporaryDirectory() as tmp:
            path = _write(tmp, "TODO.md", FIVE_ROWS)
            before = _read(path)
            with self.assertRaises(AssertionError) as ctx:
                todo_flip.flip(path, "R2", status="done", notes=notes)
            msg = str(ctx.exception)
            self.assertIn("'|'", msg)  # the offending character
            self.assertIn(f"probed position {notes.index('|')}", msg)  # its position
            self.assertIn("T8 split hazard", msg)  # the named why
            self.assertIn(repr(notes), msg)  # the probed notes
            self.assertEqual(_read(path), before)  # file byte-identical

    def test_regression_t278_newline_notes_rejected_pre_write(self):
        # d1791648496-11 (the T278 flip): the newline inside the notes
        # splits the row across two physical lines.
        notes = "wrapped clean\nsecond physical line"
        self.assertIn("\n", notes)  # the pin's premise
        with tempfile.TemporaryDirectory() as tmp:
            path = _write(tmp, "TODO.md", FIVE_ROWS)
            before = _read(path)
            with self.assertRaises(AssertionError) as ctx:
                todo_flip.flip(path, "R2", status="done", notes=notes)
            msg = str(ctx.exception)
            self.assertIn("'\\n'", msg)  # the offending character
            self.assertIn(f"probed position {notes.index(chr(10))}", msg)
            self.assertIn("single-physical-line hazard", msg)
            self.assertEqual(_read(path), before)

    def test_pass_clean_notes_with_punctuation_accepted(self):
        # The reject is scoped to the two hazards — em dashes, backticks,
        # parens, and colons are all legal notes text.
        notes = "landed 9840aba (impl 96dbe99) — `60 0`; gates green"
        with tempfile.TemporaryDirectory() as tmp:
            path = _write(tmp, "TODO.md", FIVE_ROWS)
            todo_flip.flip(path, "R2", status="done", notes=notes)
            self.assertIn(
                f"| R2 | second row | specs/r2.md | 2 | done | {notes} |",
                _read(path),
            )

    def test_pass_status_only_flip_leaves_notes_cell_untouched(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = _write(tmp, "TODO.md", FIVE_ROWS)
            todo_flip.flip(path, "R2", status="in-progress")
            self.assertIn(
                "| R2 | second row | specs/r2.md | 2 | in-progress |  |",
                _read(path),
            )


class RebuildShapeTest(unittest.TestCase):
    """Asserts (d)+(e): the row is rebuilt FROM CELLS — `| ` +
    ` | `.join(cells) + ` |` — and is ONE physical line."""

    def test_pass_rebuilt_row_is_the_from_cells_formula(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = _write(tmp, "TODO.md", FIVE_ROWS)
            rebuilt = todo_flip.flip(path, "R2", notes="landed 4242abc — wrapped clean")
            cells = ["R2", "second row", "specs/r2.md", "2", "done",
                     "landed 4242abc — wrapped clean"]
            self.assertEqual(rebuilt, "| " + " | ".join(cells) + " |")
            self.assertNotIn("\n", rebuilt)  # (e): ONE physical line
            # The write landed the exact rebuilt line.
            self.assertIn(rebuilt + "\n", _read(path))

    def test_pass_rebuilt_row_replaces_the_target_line_in_place(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = _write(tmp, "TODO.md", FIVE_ROWS)
            todo_flip.flip(path, "R2", status="done")
            new_lines = _read(path).split("\n")
            old_lines = FIVE_ROWS.split("\n")
            self.assertEqual(len(new_lines), len(old_lines))
            self.assertEqual(
                new_lines[5], "| R2 | second row | specs/r2.md | 2 | done |  |"
            )  # same position (R2 = line 5), notes cell kept
            self.assertIn(R2_OLD, old_lines)  # the old row was line 5


class ByteCarryTest(unittest.TestCase):
    """Assert (f): every OTHER line byte-identical pre/post via the
    IMPORTED assert_byte_carry — PASS on the real splice, FAIL on an
    injected splice defect (the teeth, file left byte-identical)."""

    def test_pass_one_row_flips_the_other_four_rows_byte_identical(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = _write(tmp, "TODO.md", FIVE_ROWS)
            todo_flip.flip(path, "R2", notes="landed 4242abc — wrapped clean")
            old_lines = FIVE_ROWS.split("\n")
            new_lines = _read(path).split("\n")
            self.assertEqual(len(old_lines), len(new_lines))
            for i, (old, new) in enumerate(zip(old_lines, new_lines)):
                if i == 5:  # R2's line — the one flipped row
                    self.assertEqual(new, R2_FLIPPED)
                else:
                    self.assertEqual(
                        old, new, f"line {i} must be byte-identical"
                    )

    def test_fail_dropped_neighbor_line_breaks_the_carry(self):
        def defective(lines, idx, rebuilt):
            return lines[:idx] + [rebuilt] + lines[idx + 2 :]  # drops a line

        with tempfile.TemporaryDirectory() as tmp:
            path = _write(tmp, "TODO.md", FIVE_ROWS)
            before = _read(path)
            with mock.patch.object(todo_flip, "_splice", defective):
                with self.assertRaises(AssertionError) as ctx:
                    todo_flip.flip(path, "R2", status="done")
            msg = str(ctx.exception)
            self.assertIn("assert_byte_carry", msg)  # the IMPORTED assert fired
            self.assertIn("carry line", msg)  # which carried line broke
            self.assertEqual(_read(path), before)  # file byte-identical

    def test_fail_mutated_neighbor_line_breaks_the_carry(self):
        def defective(lines, idx, rebuilt):
            mutated = lines[idx + 1] + " (mutated)"
            return lines[:idx] + [rebuilt, mutated] + lines[idx + 2 :]

        with tempfile.TemporaryDirectory() as tmp:
            path = _write(tmp, "TODO.md", FIVE_ROWS)
            before = _read(path)
            with mock.patch.object(todo_flip, "_splice", defective):
                with self.assertRaises(AssertionError) as ctx:
                    todo_flip.flip(path, "R2", status="done")
            msg = str(ctx.exception)
            self.assertIn("assert_byte_carry", msg)
            self.assertIn("carry line", msg)
            self.assertEqual(_read(path), before)


class WriteOnlyAfterAssertsTest(unittest.TestCase):
    """Assert (g): every AssertionError leaves the file byte-identical —
    the write happens only after every assert passes."""

    def test_every_failure_mode_leaves_the_file_byte_identical(self):
        # The (c)-independent failure modes swept in one loop; the two
        # notes-hazard modes are pinned by NotesRejectTest's regression
        # tests (each also asserts the file byte-identical pre-write).
        failures = (
            ("zero-match id", dict(todo_id="NOPE")),
            ("seven-cell row", dict(todo_id="R2", _rows_seven=True)),
            ("nonlegal status", dict(todo_id="R2", status="finished")),
        )
        with tempfile.TemporaryDirectory() as tmp:
            for label, kwargs in failures:
                rows = (
                    FIVE_ROWS.replace(
                        R2_OLD,
                        "| R2 | second row | specs/r2.md | 2 | todo | n | glued |",
                    )
                    if kwargs.pop("_rows_seven", False)
                    else FIVE_ROWS
                )
                path = _write(tmp, "TODO.md", rows)
                before = _read(path)
                with self.assertRaises(AssertionError, msg=label):
                    todo_flip.flip(path, **kwargs)
                self.assertEqual(_read(path), before, label)

    def test_success_actually_writes_the_file(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = _write(tmp, "TODO.md", FIVE_ROWS)
            todo_flip.flip(path, "R2", notes="landed 4242abc — wrapped clean")
            self.assertIn(R2_FLIPPED, _read(path))
            self.assertNotIn(R2_OLD, _read(path))


class CliTest(unittest.TestCase):
    """The CLI contract: exit 0 with the rebuilt row on stdout; non-zero
    with the probed actuals on stderr on any failure."""

    def _run(self, *argv):
        return subprocess.run(
            [sys.executable, todo_flip.__file__, *argv],
            capture_output=True,
            text=True,
            check=False,
        )

    def test_exit_zero_prints_the_rebuilt_row(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = _write(tmp, "TODO.md", FIVE_ROWS)
            proc = self._run(
                path, "R2", "--status", "done",
                "--notes", "landed 4242abc — wrapped clean",
            )
            self.assertEqual(proc.returncode, 0, proc.stderr)
            self.assertIn(R2_FLIPPED, proc.stdout)
            self.assertIn(R2_FLIPPED, _read(path))

    def test_exit_nonzero_names_the_probed_actuals_on_stderr(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = _write(tmp, "TODO.md", FIVE_ROWS)
            before = _read(path)
            proc = self._run(path, "NOPE", "--status", "done")
            self.assertNotEqual(proc.returncode, 0)
            self.assertIn("'NOPE'", proc.stderr)
            self.assertIn("probed 0 matching data row(s)", proc.stderr)
            self.assertEqual(_read(path), before)  # file untouched


if __name__ == "__main__":
    unittest.main()
