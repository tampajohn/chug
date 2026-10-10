check: cd "$(git rev-parse --show-toplevel)" && export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared && touch src/*.rs tests/*.rs && python3 -m unittest scripts.test_todo_flip -v && cargo test --release --test todo_consistency --test internal_info_lint

# T280 — mut-e survivor pin: the (e) one-physical-line assert's non-\n splitlines boundary case

**One concern:** add ONE regression test to `scripts/test_todo_flip.py`
that reaches the `scripts/todo_flip.py` (e) one-physical-line assert
through a Python `splitlines()` boundary OTHER than `\n` — the exact
shape of the T279 kimi validator's documented survivor mut-e — so the
assert's teeth are pinned and the mutant dies.

## Why

The T279 kimi verdict d1791652506-6 (PASS, 12 mutants 11 killed, 1
documented survivor) finding 1: **mut-e survives** — no test reaches the
(e) one-physical-line assert (`physical = rebuilt.splitlines()`,
scripts/todo_flip.py:187) for splitlines boundaries beyond `\n`. The
validator labeled the survivor acceptable ("asserted anyway" by
construction after (c)) and handed the suggested regression pin to the
next eval — the same discharge path as T274 (the T273 m6 survivor pin,
filed on schedule at the following eval). Python's `str.splitlines()`
splits on `\r`, `\r\n`, `\v`, `\f`, `\x1c`–`\x1e`, `\x85`,
  `\u2028`, `\u2029` — all of which pass the (c) reject (which names
only `|` and `\n`, scripts/todo_flip.py:159-184), so a notes payload
carrying one reaches (e), and (e) is the LAST line of defense: the
mutant that drops (e) writes the payload into TODO.md silently.

## Repo context (read these first)

- `scripts/todo_flip.py` — the (c) notes reject (lines ~159-184: only
  `|` and `\n` named) and the (e) assert (lines ~186-194:
  `physical = rebuilt.splitlines()`; `len(physical) != 1` raises
  AssertionError naming the probed line count; every assert pre-write,
  so the file stays byte-identical on fire).
- `scripts/test_todo_flip.py` — the suite's conventions (26 cases):
  `tempfile.TemporaryDirectory`, `_write(tmp, "TODO.md", FIVE_ROWS)`,
  `_read(path)`, `todo_flip.flip(path, "R2", status="done", notes=…)`;
  the two existing regression pins
  (`test_regression_t275_pipe_notes_rejected_pre_write`,
  `test_regression_t278_newline_notes_rejected_pre_write`, lines ~251-282)
  are the shape to mirror: premise assert, assertRaises(AssertionError),
  message-content asserts, file byte-identical.
- The survivor's semantics note: (e) via `splitlines()` is STRICTER than
  the file's actual `\n`-delimited physical-line semantics — a ``
  payload is one physical file line but two `splitlines()` elements.
  Firing on it is the intended defense-in-depth (refuse unicode line
  separators in TODO.md rather than write them); the pin documents that
  behavior, it does not change it.

## Requirements

1. ONE new test method in `scripts/test_todo_flip.py`, mirroring the two
   existing regression pins: flip a fixture row with `--notes` carrying
   a `\u2028` (LINE SEPARATOR) character; assert AssertionError
   fires; assert the message names the probed line count
   (`not ONE physical line` / `probed 2 line(s)` per the (e) message);
   assert the file is byte-identical post-fire.
2. The pin's premise assert (`self.assertIn("\u2028", notes)` or
   equivalent) rides the test per the existing convention.
3. NO behavior change to `scripts/todo_flip.py` — the pin documents the
   current (e)-fires behavior; the helper file is untouched.
4. RED-proof (orchestrator-run at review): the mut-e mutant — neuter
   the (e) assert (e.g. `physical = [rebuilt]`) — MUST turn EXACTLY the
   new test red while the other 26 stay green; revert byte-clean.

## Tests

- The new case runs under `python3 -m unittest scripts.test_todo_flip`
  (27/27 green after the change).
- The cargo legs in the `check:` line cover the all-tracked-files lint
  (`internal_info_lint`) and the TODO.md table guard
  (`todo_consistency`).

## Acceptance

- `python3 -m unittest scripts.test_todo_flip -v` green (27 cases).
- The check: line green verbatim.
- The mut-e RED-proof kills exactly the new test (orchestrator leg).
- Diff touches ONLY `scripts/test_todo_flip.py`.

estimate: ~15 changed lines (one test method + its comment), tests-only
