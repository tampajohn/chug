# T154 — split src/mcp_serve.rs: byte-identical test-module extraction (mechanical move)

check: cargo test

## Repo context

src/mcp_serve.rs is 1,949 lines — production ~790 (through
`to_line` at :765) plus a ~1,160-line `#[cfg(test)] mod tests` — the
era's fastest grower (+1,166 since new-at-783 in cycle 64; cycle-70
EVALUATION watch item: "split candidate on the NEXT feature landing in
it"). That trigger now fires: T153 (`chug_cancel`) lands in this file
next, and the split lands FIRST so the feature's diff stays readable
against a settled layout (dependency ordering named in the cycle-72
EVALUATION handoff).

This is the T104/T109 class: a **mechanical, byte-identical move** —
the filing-time ~500-line estimate ceiling does not apply to moved
lines (the class exemption: moved lines are not novel-logic lines;
T104's 3,100-line byte-identical test move landed first-try at 75/80).
Precedents: src/driver.rs → src/driver/tests/* (T104),
src/delegate.rs → src/delegate/tests/* (T109-class) — both kept
`src/driver.rs` / `src/delegate.rs` as the production file and
extracted ONLY the test modules into `src/<name>/tests/*.rs`
submodules. This row follows THAT SAME shape for consistency: verify
the exact precedent shape at implementation time and MATCH it — one
shape across the three splits. The test module moves as ONE file
(`src/mcp_serve/tests.rs`) unless the implementer finds a natural
≥2-way seam already present in the module (e.g. unit-vs-wire
groupings) — a single moved file is the default, further splits only
when byte-identical per file.

estimate: ~1,949 MOVED lines (mechanical byte-identical move —
exemption class; novel lines ~5: the `#[cfg(test)] mod tests;`
declaration + `use super::*;` adjustments)

## Requirements

1. **Byte-identical move.** Every moved test keeps its name, order,
   and body byte-for-byte. The ONLY novel lines: the module declaration
   in the production file, the path/`use` wiring in the new test
   file(s), and any `pub(crate)` visibility adjustments the compiler
   requires (each named in the commit message — expected ZERO: the
   tests are a child module of the same code either way).
2. **Zero production edits.** No production function body, signature,
   or ordering changes. `git diff` on the production file shows only
   the removed test module + the added `mod tests;` line.
3. **Run-set identity.** The set of executed tests is identical before
   and after: capture the full sorted test-name list
   (`cargo test -- --list` sorted) pre-move and post-move and diff —
   must be EMPTY. Both lists ride the commit message (or a bounded
   digest of them: count + sha256 of the sorted list).
4. **No behavior pins weakened.** Clippy `-D warnings` clean; the
   todo_consistency / doctrine pin files untouched (no doctrine text
   changes in this row).

## Acceptance

- `cargo build`, `cargo clippy --all-targets -- -D warnings`, `cargo test`
  all green; test-name list identical pre/post (requirement 3).
- `git diff --stat` shows the move shape only (one file's test module
  out, new file(s) in, ~5 novel lines).
- The validator (or orchestrator, per routing) independently re-runs
  the list-identity check and one move-mutant (drop one moved test
  file from the `mod` declaration → the list diff is non-empty → RED).

## Out of scope

- Splitting the ~790 lines of production code (schema fns / tool impls /
  render fns) — no consumer pulling; revisit when production crosses
  ~1,500.
- mcp_http.rs (2,800) / tgrep.rs (2,638) / tui.rs (2,527) — stable, no
  feature landings queued; watch items only.
- Any test CONTENT changes (renames, refactors, new tests).
