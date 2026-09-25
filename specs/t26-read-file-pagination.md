# T26 — `read_file` gains `offset`/`limit` pagination

check: cd /Users/jadams/workspace/chug && cargo test

## Why (evidence)

Cycle-9 evaluation §3: **four source files now exceed the 2000-line
read cap** — `driver.rs` 2,894, `mcp_http.rs` 2,343, `tui.rs` 2,163,
and (new this cycle, +879 from T23) `tools.rs` 2,048. The cap note
(`[truncated: showing lines 1-2000 of N]`) tells the model the file
continues but gives it **no primitive to page with**, so children fall
back to `sed -n 'X,Yp'` chunking via bash — the T20 impl child burned
its iterations 1–5 doing exactly that to driver.rs (cycle-7 eval §3,
carried). Every future child that must read past line 2000 of any of
these files re-derives the same workaround. This is DX friction with a
growing tax (~100 lines/cycle on driver.rs alone), not a behavior bug:
the tool does what it says, it just says too little.

## Repo context

- `src/tools.rs:18` — `const READ_MAX_LINES: usize = 2000;`
- `src/tools.rs:53-54` — the schema entry; description: `"Read a text
  file. Paths are relative to the working directory. Output is capped
  at 2000 lines and truncation is noted."` (T22 pinned a different
  tool's description; this one is currently unpinned — verify by grep
  before writing the new pin).
- `src/tools.rs:223` — `fn read_file`: reads the whole file, flat
  `take(READ_MAX_LINES)` head when over the cap, truncation note naming
  `1-{READ_MAX_LINES} of {line_count}`. Pure, synchronous, trivially
  testable; the existing tools.rs test module has read_file fixtures to
  extend.
- Callers: the tool layer only — `inner()` dispatches by name
  (`src/tools.rs:191`); nothing else in the binary parses read_file
  output.

## Requirements

1. **Schema**: `read_file`'s `input_schema` gains two optional integer
   properties — `offset` (1-based first line to show; default 1) and
   `limit` (max lines to show; default `READ_MAX_LINES`). Description
   gains ONE short clause documenting them (e.g. "Use `offset`/`limit`
   to page beyond the cap."). Required list unchanged (`path` only).
2. **Default path byte-identical**: with neither param, output (content
   AND truncation note wording) is exactly today's — full file when
   ≤2000 lines, else head-2000 + the existing note.
3. **Window semantics**: with `offset`/`limit`, show lines
   `offset ..= min(offset+limit-1, line_count)`; whenever the shown
   window is not the whole file, append a note naming the ACTUAL
   window, e.g. `[showing lines 2001-2894 of 2894]`. `limit` may exceed
   `READ_MAX_LINES` (explicit paging is the point); `offset < 1` is a
   tool error naming the 1-based convention; `offset > line_count` is
   NOT an error — return a short content string naming the file length
   (paging loops must not crash).
4. No other tools touched; `write_file`/`edit_file` untouched; the
   driver's 2000-char event preview semantics untouched (that is T25's
   seam, a different item).
5. README: the Tools section's read_file mention stays truthful — add
   the paging params to its one-liner.

## Tests

All in the tools.rs test module:

- default-leg pin: a >2000-line fixture returns head-2000 with the
  byte-identical pre-T26 truncation note (guard against note drift);
- window legs: `offset: 2001` on a 2894-line fixture starts at the
  file's line 2001 and the note names the true window;
  `offset: 2001, limit: 50` shows exactly lines 2001–2050;
- edge legs: `offset: 0` → `is_error: true` + 1-based message;
  `offset` beyond EOF → non-error content naming the file length;
  `limit: 1` → exactly one line;
- schema pin: `tool_schemas()` read_file entry contains `offset` and
  `limit` properties and they are NOT in `required` (assert on the live
  schema JSON, per the T22 pinning convention);
- non-vacuousness (demonstrated once): reverting `read_file` to the
  flat head-take kills the window-leg tests.

## Acceptance

- `cargo build`, `cargo clippy --all-targets -- -D warnings`, `cargo
  test` all green.
- Adversarial validation (REQUIRED — touches `src/tools.rs`, LOOP-SPEC
  §2.4): confirm the default leg is byte-identical (revert-mutant on
  the note wording dies), window arithmetic has no off-by-one (mutants:
  `offset` used 0-based; `+limit` vs `+limit-1`; `>` vs `>=` on the EOF
  comparison), the schema pin asserts live JSON, and no other tool's
  schema or behavior changed.
