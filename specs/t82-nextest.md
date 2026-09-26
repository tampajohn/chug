# T82 — cargo-nextest for gates

check: cargo test

## Concern

`cargo test` runs the 560+-test suite with conservative scheduling;
cargo-nextest is the standard drop-in faster runner (process-per-test
isolation, better parallelism, typically 2–3× wall-time on suites this
size). Gates run 3–6× per item (worktree, validator, post-merge, mutation
legs), so suite wall time is a top tool-execution cost even after T78
release builds. Operator-approved knob 2026-09-26.

## Repo context

- Gates: `cargo test --release -- --test-threads=4` post-T78 (bounded,
  META-SPEC gates rule).
- nextest invocation: `cargo nextest run --release` (install:
  `cargo install cargo-nextest` — host tool, NOT a crate dependency; loop
  templates must handle absence gracefully).
- Some test families are parallel-sensitive by design (T31's dead_port
  family, RUN_SHELL_TIMING_LOCK) — nextest's process isolation is
  compatible, but the suite must be run as-is first to confirm green.

## Requirements

1. Gate templates (LOOP-SPEC/META-SPEC bounded-gates rule) become:
   `cargo nextest run --release` when `cargo nextest` is on PATH,
   falling back to `cargo test --release -- --test-threads=4` (never a
   hard dependency).
2. loopd.sh installs nothing; it checks `command -v cargo-nextest` and
   logs which runner cycles use (one line in loopd.log at startup).
3. First cycle after merge: run BOTH runners once in main and record both
   wall times in that cycle's Outcomes (the measurement that justifies
   keeping it).
4. If any test family goes red ONLY under nextest (isolation
   assumptions), the fallback is per-template, and the family is named in
   the commit message.

## Tests

- Doctrine item: validation REQUIRED — review checks the fallback is
  unconditional, no template hard-requires nextest, and the both-runners
  measurement is in the acceptance path.
- Acceptance: Outcomes of the first nextest cycle records both wall times;
  subsequent cycles use nextest when present.

## Out of scope

- sccache/linker tuning; splitting the suite into shards; changing test
  code for isolation (fallback instead).
