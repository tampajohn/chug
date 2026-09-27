# T84 — driver.rs trim-machinery extraction to src/trim.rs

check: cargo test

## Repo context

- The cycle-36 eval PRE-DECLARED the mandate: "driver.rs 3,607 lines — the
  ~4,500 trip line is PRE-DECLARED now (a future crossing carries a
  T71-class extraction mandate)". driver.rs is now **5,488 lines** (+52%
  in ten cycles: T73 plan loops, T76 tgrep wiring, T77 trim machinery +
  its test module). The mandate fires.
- The cohesive block is the T77 transcript-trim machinery: production
  fns cluster at driver.rs:1271–1437 (`is_trim_marker`,
  `trim_marker_text`, `plan_trim_segments`, `transcript_trim` + their
  constants), and the trim tests are the bulk of the file's `mod tests`
  (driver.rs has 167 `fn` tokens; the trim test family spans roughly
  1566–2400: `trimming_*`, `trim_*`, `marker_texts`, `last_marker_index`,
  `prefix_bytes`, `frozen_or_marker_end`, ...).
- T71 precedent: delegate.rs (3,498 lines) was extracted from driver.rs
  the same way — a binary-crate submodule (`mod delegate;`), private
  items moved, `pub(crate)` seams where the driver still calls in.
- Verified by the cycle-47 eval before filing (`grep -n 'fn ' src/driver.rs`).

## Requirements

1. New `src/trim.rs` as a driver-visible submodule (`mod trim;` declared
   in driver.rs or main.rs per the existing module wiring); main.rs's
   module list and the README Development layout line gain `trim`
   (alphabetical position in both).
2. Move the trim machinery — the production fns above, their constants
   (segment sizes, marker format), and EVERY trim-specific test — into
   `src/trim.rs` (tests ride in its own `mod tests`). The exact cut is
   the impl's judgment under ONE rule: everything trim-specific moves;
   anything a non-trim caller uses stays reachable via a `pub(crate)` or
   `pub(super)` seam. Byte-identical behavior: no logic edits, no
   renames beyond the module path, no signature changes.
3. driver.rs drops to ≤ ~3,600 lines; `use` sites updated; the driver's
   remaining trim touch-points (the assembly call site, the estimate
   helper if it stays) call `trim::...`.
4. Non-goals: no behavior change, no config change, no doc rewrites
   beyond the two module-list lines (SPEC.md's trim paragraph names no
   file; README's Transcript-trimming bullet names no file — verify and
   leave both alone if so).
5. Commit message: `T84 — driver.rs trim-machinery extraction to
   src/trim.rs (pre-declared ~4500 trip line crossed: 5488 lines)`.

## Tests

- The moved trim suite must pass from its new home (same count of trim
  tests before/after — the impl records both counts in the commit message
  or LEDGER).
- Full `cargo build` + `cargo clippy --all-targets -- -D warnings` +
  `cargo test` green in the worktree.
- Non-vacuousness: the extraction is a MOVE — the validator's angle is
  byte-identity (the trim tests still pin the same behavior from
  `trim::`; a mutant that drops the `mod trim;` wiring fails to compile;
  a mutant that changes moved logic dies against the moved tests).
- RED-proof note: deletion legs (remove the module declaration; revert
  one moved fn to a stub) must fail LOUDLY (compile error / test red) —
  the impl demonstrates one such leg in its wrap.

## Acceptance

- `cargo test` green; driver.rs ≤ ~3,600 lines; trim machinery reachable
  only via the new module; zero behavior drift (the T77 trim pins —
  byte-stable marker format, pairing safety, frozen-segment semantics —
  all still green from `trim::`).

## Out of scope

- Splitting the driver.rs test module per se (only the trim family
  moves); extracting any other block (plan loops, prompts); any
  behavior change.
