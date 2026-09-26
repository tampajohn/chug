# T78 — Release builds for the loop (binary + gates)

check: cargo test

## Concern

Everything in the loop runs DEBUG Rust: loopd builds `target/debug/chug`,
child/validator templates execute it, gates run `cargo test` unoptimized.
Measured breakdown says ~85% of item wall-clock is tool execution (test
suites at 6–10s debug, builds, grep/transcript CPU) — debug codegen is
2–5× slower on exactly that class. Operator knob request 2026-09-26
("using a compiled binary"). This is the cheapest structural speedup left.

## Repo context

- `loopd.sh`: `cargo build` (debug) then executes `./target/debug/chug`.
- `LOOP-SPEC.md` / `META-SPEC.md` templates: `target/debug/chug` paths;
  bounded gates `cargo test -- --test-threads=4`.
- T47 shared target dirs: `target-shared*` already exist — release
  artifacts live in the SAME dirs (`--release` subdir), no new dirs needed.
- Tests must still build under debug for dev workflows: README's
  Development section documents both.

## Requirements

1. loopd.sh: `cargo build --release`; execute `./target/release/chug`.
2. LOOP-SPEC/META-SPEC templates: `target/release/chug` for ALL child and
   validator launches (the worktree build step becomes
   `cargo build --release`).
3. Gates: `cargo test --release -- --test-threads=4` for bounded review/
   merge/validation gates (orchestrator + validator templates). Tradeoff to
   name in templates: first release build is slower (compile time), all
   subsequent runs faster — shared target dirs (T47/T52) amortize it.
4. The spec `check:` convention stays `cargo test` (debug) — no spec churn.
5. One-time migration note in the commit: first cycle after merge pays two
   release builds (main + shared), then steady-state.

## Tests

- Doctrine item: validation REQUIRED — review checks every
  `target/debug/chug` reference in launch paths is gone (templates, loopd,
  README quickstart if referenced) and the build-time tradeoff is named.
- Acceptance: next cycle's events show `cargo test --release` gate runs;
  suite wall time recorded in Outcomes vs the ~6–10s debug baseline.

## Out of scope

- nextest/sccache/linker tuning (separate knob); changing dev default
  (README quickstart keeps debug for iteration speed); stripping/LTO flags.
