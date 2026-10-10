#!/usr/bin/env python3
"""todo_flip.py — T279: the checked TODO.md row flip (rebuild-from-cells).

The todo-flip structural-slip census tripped at 3, and its armed trigger's
letter binds verbatim: "trip 87 WEIGHS the checked flip-helper row (rebuild
the row from cells, never string-splice around the trailing pipe; assert
single-physical-line post-edit)". The three fires, every one a hand-rolled
splice edit around the trailing pipe:

  1. the census's origin fire — a flip's glued note split the row's cell
     count; the T8 guard caught it;
  2. d1791632010-15 (the T275 flip) — python glued the LANDED note AFTER
     the row's trailing pipe -> a 7-cell row -> the T8 guard FAILED
     pre-commit (`expected exactly 6 cells, got 7`) — the designed catch,
     zero casualty, repaired one leg later;
  3. d1791648496-11 (the T278 flip) — the edit appended the newline INSIDE
     the notes cell while the join re-added the trailing pipe -> the row
     split across two physical lines; exit-0-masked, self-caught by the
     same-command verification grep, rejoined byte-exact.

This is the T273 import-starvation shape one surface over: the recurring
edit re-authors its structural checks inline each flip (or skips them).
So the flip gets ONE invoke target that rebuilds the target row FROM CELLS
and proves the structural invariants PRE-WRITE — the file is written only
after every assert passes, and any AssertionError leaves it byte-identical
(write-then-verify is the banned order). Every failure message names the
PROBED actuals (the T270 reporting requirement), never a bare constant.

The byte-carry assert is IMPORTED from the carried helper library, never
re-authored (the T273 move): `assert_byte_carry(prev_lines, new_lines)`
from scripts/wrap_assert.py.

Usage:
    python3 scripts/todo_flip.py TODO.md T279 --status done --notes "..."

`--status` defaults to `done` (the flip target); `--notes` is optional —
a status-only flip keeps the notes cell. Exit 0 on success (stdout prints
the rebuilt row, ready for the orchestrator's verification grep); exit 1
with the probed actuals on stderr on any failure.
"""

import argparse
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from wrap_assert import assert_byte_carry  # noqa: E402

# A data row parses into exactly this many cells (the T8 guard's own
# count — `expected exactly 6 cells, got N`).
EXPECTED_CELLS = 6

# The legal status vocabulary, copied from the T8 guard's own STATUSES
# (tests/todo_consistency.rs) — a status outside it fails the guard
# post-write, so it is rejected pre-write here.
LEGAL_STATUSES = ("todo", "in-progress", "blocked", "done")


def _row_cells(line):
    """Parse one physical line into table cells, mirroring the T8 guard's
    `table_cells` so the two parsers cannot drift apart.

    Non-row lines (no leading `|` after trim) return None. Header
    (first cell `id`) and all-dash separator rows are NOT data rows and
    return None, exactly as the guard skips them — the header can never
    be matched, let alone rewritten, by an id flip.
    """
    stripped = line.strip()
    if not stripped.startswith("|"):
        return None
    cells = [cell.strip() for cell in stripped.strip("|").split("|")]
    if cells and cells[0] == "id":
        return None
    if cells and all(cell != "" and all(ch == "-" for ch in cell) for cell in cells):
        return None
    return cells


def _splice(lines, idx, rebuilt):
    """Replace lines[idx] with *rebuilt*, keeping every other line at its
    position. Factored as its own function so the byte-carry guard's FAIL
    path (the (f) teeth) can be proven by injecting a defect here in the
    unittest — the production path is the only caller."""
    return lines[:idx] + [rebuilt] + lines[idx + 1 :]


def flip(path, todo_id, status="done", notes=None):
    """Flip ONE TODO.md row in place; returns the rebuilt row line.

    Assert order (every assert PRE-WRITE — any AssertionError leaves the
    file byte-identical):

      (a) the file exists, and exactly ONE data row's FIRST cell
          (strip-compared) equals *todo_id* — probed count != 1 names the
          id and the probed count;
      (b) that row parses into exactly 6 cells — probed count != 6 names
          the probed count (the d1791632010-15 7-cell fire shape);
      (b2) *status* is one of the T8 guard's legal statuses;
      (c) *notes*, when given, contains no `|` (the T8 split hazard) and
          no newline (the single-physical-line hazard) — each rejection
          names the offending character and its probed position;
      (d) the row is rebuilt FROM CELLS — `| ` + ` | `.join(cells) +
          ` |` — never string-spliced around the trailing pipe;
      (e) the rebuilt row is ONE physical line — by construction after
          (c), asserted anyway;
      (f) every OTHER line is byte-identical pre/post via the IMPORTED
          assert_byte_carry;
      (g) only then is the file written.
    """
    if not os.path.isfile(path):
        raise AssertionError(
            f"todo_flip({path!r}, {todo_id!r}): probed path is NOT an "
            f"existing file — nothing read, nothing written"
        )
    with open(path, "r", encoding="utf-8") as f:
        text = f.read()
    # split("\n") — NOT splitlines(): every non-target line is carried
    # byte-exact (incl. any stray \r), and a trailing newline survives the
    # join as the carried empty last element.
    lines = text.split("\n")

    # (a) locate the row by FIRST-cell id match (strip-compared).
    matches = []
    for idx, line in enumerate(lines):
        cells = _row_cells(line)
        if cells is not None and cells[0] == todo_id:
            matches.append((idx, cells))
    if len(matches) != 1:
        raise AssertionError(
            f"todo_flip({path!r}, {todo_id!r}): probed {len(matches)} "
            f"matching data row(s) for id {todo_id!r} (need exactly 1) — "
            f"file untouched"
        )
    idx, cells = matches[0]

    # (b) the row parses into exactly 6 cells.
    if len(cells) != EXPECTED_CELLS:
        raise AssertionError(
            f"todo_flip({path!r}, {todo_id!r}): probed row {todo_id!r} "
            f"parses into {len(cells)} cells (need exactly "
            f"{EXPECTED_CELLS}) — the T8 guard would fail this row "
            f"post-edit (`expected exactly 6 cells, got {len(cells)}`, "
            f"the d1791632010-15 fire shape); file untouched"
        )

    # (b2) the status is one of the T8 guard's legal statuses.
    if status not in LEGAL_STATUSES:
        raise AssertionError(
            f"todo_flip({path!r}, {todo_id!r}): probed status {status!r} "
            f"is not one of the T8 guard's legal statuses {LEGAL_STATUSES} "
            f"— the guard would fail the row post-edit; file untouched"
        )

    # (c) the notes carry no `|` and no newline, PRE-WRITE.
    if notes is not None:
        for hazard, why in (
            (
                "|",
                "the T8 split hazard — the guard splits every row on `|`, "
                "so a stray pipe splits one cell into two (the "
                "d1791632010-15 fire shape)",
            ),
            (
                "\n",
                "the single-physical-line hazard — a newline inside the "
                "notes cell splits the row across two physical lines (the "
                "d1791648496-11 fire shape)",
            ),
        ):
            pos = notes.find(hazard)
            if pos != -1:
                raise AssertionError(
                    f"todo_flip({path!r}, {todo_id!r}): probed --notes "
                    f"contains {hazard!r} at probed position {pos} (0-based "
                    f"in the probed notes {notes!r}) — {why}; file untouched"
                )

    # (d) rebuild FROM CELLS — never string-splice around the trailing pipe.
    new_cells = list(cells)
    new_cells[4] = status
    if notes is not None:
        new_cells[5] = notes.strip()
    rebuilt = "| " + " | ".join(new_cells) + " |"

    # (e) one physical line — by construction after (c), asserted anyway.
    physical = rebuilt.splitlines()
    if len(physical) != 1:
        raise AssertionError(
            f"todo_flip({path!r}, {todo_id!r}): the rebuilt row is not ONE "
            f"physical line (probed {len(physical)} line(s) under python's "
            f"line splitting) — by construction after the (c) rejects, "
            f"asserted anyway; file untouched"
        )

    # (f) every OTHER line byte-identical pre/post (the IMPORTED carry
    # assert — never re-authored).
    new_lines = _splice(lines, idx, rebuilt)
    others_before = lines[:idx] + lines[idx + 1 :]
    assert_byte_carry(others_before, new_lines)

    # (g) write only after every assert passed.
    with open(path, "w", encoding="utf-8") as f:
        f.write("\n".join(new_lines))
    return rebuilt


def main(argv=None):
    """The CLI: exit 0 on success (the rebuilt row on stdout), exit 1 with
    the probed actuals on stderr on any AssertionError."""
    parser = argparse.ArgumentParser(
        description=(
            "Flip one TODO.md table row (rebuild-from-cells, every "
            "structural assert pre-write — T279)."
        )
    )
    parser.add_argument("path", help="the TODO.md table file (usually TODO.md)")
    parser.add_argument("id", help="the row id in the FIRST cell (e.g. T279)")
    parser.add_argument(
        "--status",
        default="done",
        help="the new status cell (default: done)",
    )
    parser.add_argument(
        "--notes",
        default=None,
        help=(
            "replacement notes cell text (a status-only flip keeps the "
            "notes cell); must contain no `|` and no newline"
        ),
    )
    args = parser.parse_args(argv)
    try:
        rebuilt = flip(args.path, args.id, status=args.status, notes=args.notes)
    except AssertionError as exc:
        print(f"todo_flip: FAILED — {exc}", file=sys.stderr)
        return 1
    print(rebuilt)
    return 0


if __name__ == "__main__":
    sys.exit(main())
