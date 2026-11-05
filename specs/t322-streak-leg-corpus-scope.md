# T322 — wrap_assert trim-streak leg: identity-check corpus scoping + boundary fixtures (the T321 validator's findings 1-3)

estimate: ~90 changed lines (one scoping change + two fixture legs + docstring)

## Repo context

The T321 kimi validator's verdict (PASS d1793910418-5) carries three non-blocking
findings that together gate the streak leg's production use:

**Finding 1 (production-blocking).** `assert_fill_trim_streak`'s T320 identity
check (one `run_start` per rotated file) sweeps EVERY rotated
`.chug/events-*.jsonl` in the directory. The REAL main-repo corpus contains
104/1581 rotated files violating the one-run_start identity (Sept-2026 cycle
files with 2 run_starts each), and the glob also sweeps the 610 `events-tNNN-*`
task-harvest files (never main-loop predecessor streams). Exercised against the
live corpus the leg fails closed EVERY time — naming
`events-20260922-032901.jsonl` (probed 2 run_starts) — before reaching the
streak derivation, so the LOOP-SPEC fill-assert doctrine T321 amended in
cannot verify in production: the catch fires by design on every fill. The
walked chain window itself is clean (the newest rotated files are the true
cycle streams, verified by mtime). The validator's recommendation: scope the
identity check to the walked chain window or narrow the glob.

**Finding 2 (weak-test survivor M7).** The resetter threshold `>= 1` flipped to
`>= 2` survives the full suite — every fixture's resetting stream has
whole-final trim count 2, so the boundary value 1 (the modal real-corpus
resetter shape: a one-post-stamp-trim stream) is never exercised. The reset
MECHANISM is red-proven; a trim-1-resetter fixture closes the boundary.

**Finding 3 (weak-test survivor M4).** The at-stamp scope `ts <= stamp` flipped
to `ts < stamp` survives — no fixture carries a trim leg whose ts string
EXACTLY equals the fill stamp, so the equality arm of the spec-pinned `<=` is
unpinned.

Existing pieces: `scripts/wrap_assert.py`'s `assert_fill_trim_streak` (T321,
merged 4b8b73a) — the enumeration (mtime-then-name order), the T320 identity
check, the whole-final trim counts, the back-walk chain, the stamp-scoped +1,
the extraction + assertion. `scripts/test_wrap_assert.py` (175 tests, ~87s).

## Requirements

1. **Scope the identity check to what the walk reads.** The rotated-file
   enumeration narrows to main-loop cycle-stream files — the
   `events-<YYYYmmdd>-<HHMMSS>.jsonl` shape (14-digit date + 6-digit time) —
   excluding `events-tNNN-*` task-harvest files by name shape; and the
   one-run_start identity assertion applies ONLY to files the chain walk
   actually inspects (the newest-first candidates down to and including the
   resetting stream, or the full window when no resetter is reached) — files
   older than the walk's stop point are never read and never identity-checked.
   A legacy multi-run_start file INSIDE the walked window still fails closed
   by name (the T320 precondition binds what the walk trusts); a file outside
   the window is out of scope by construction.
2. **Derivation semantics unchanged** — the chain rule, the stamp-scoped +1,
   the extraction shapes, and every error message shape for the in-window legs
   are byte-preserved (the existing 15 T321 legs must pass unmodified except
   where a leg pins the OLD unscoped sweep; such a pin is updated with its
   RED-proof re-shown).
3. **Finding-2 fixture.** A fixture chain whose resetting stream's whole-final
   trim count is EXACTLY 1 asserts the walk stops there (the prior chain
   counts only trim-0 streams newer than it) and names the resetter — RED-proof
   by flipping `>= 1` to `>= 2` and watching the fixture fail.
4. **Finding-3 fixture.** A current-stream fixture carrying a trim leg whose
   ts string EXACTLY equals the fill stamp asserts the at-stamp count INCLUDES
   it (the `<=` equality arm) — RED-proof by flipping `<=` to `<` and watching
   the fixture fail.
5. **Live-corpus smoke leg (manual, recorded in the commit message).** After
   the change, the leg exercised against the LIVE main corpus
   (`python3 -c` calling `assert_fill_trim_streak` over `.chug/` with a
   composed fill text carrying the eval-derived TRUE figure) returns
   verdict-TRUE (derives and matches) instead of refusing — the production
   readiness the LOOP-SPEC doctrine needs. This leg is orchestrator-run at
   review, not a committed test (the live corpus is not a fixture).
6. No changes outside `scripts/wrap_assert.py`, `scripts/test_wrap_assert.py`.

## Acceptance

- The scoping change keeps all 15 T321 legs green (or re-RED-proves any pin it
  must update); the two new boundary fixtures RED-catch their named mutants
  (`>=1`->`>=2`, `<=`->`<`) and PASS the TRUE shapes;
- the full `python3 scripts/test_wrap_assert.py` suite green (175 + the new
  legs);
- the live-corpus smoke leg (req 5) returns verdict-TRUE — recorded in the
  commit message with the derived figure;
- `cargo test --release --test todo_consistency` green (bookkeeping floor).

check: python3 scripts/test_wrap_assert.py
