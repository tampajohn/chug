# T243 — mcp_serve stub-spawn test: poll events.jsonl instead of point-asserting after argv.txt

## Repo context

`src/mcp_serve/tests.rs:1077`
(`chug_launch_stub_spawn_pins_exact_argv_cwd_and_return_paths`) spawns a
shell stub that writes, IN ORDER: `cwd.txt`, then atomically `argv.txt`,
then `: > .chug/events.jsonl`, then sleeps. The test polls `argv.txt` via
`wait_for_stub_dump` (10s deadline) and then POINT-ASSERTS
`events_path.is_file()` (~line 1164). Under full-suite load the inter-line
gap between the stub's `argv.txt` publish and its `events.jsonl` touch
opens, and the assert fires early: the T237 validator's round-1 nextest
run fail-fast'd at 1265/1664 on exactly this (cycle-113 wrap notes +
`.chug/verdict-t237-validate.md` finding; 5/5 green in isolation; 1664/1664
on the `--no-fail-fast` rerun — a load race, pre-existing, NOT a product
bug: the stub's write order models the real child's ordering, and the
test's wait discipline is what's incomplete). A flake that reds a
VALIDATOR's gate mid-round costs a full suite rerun and risks a
mis-adjudication; filed forward by the validator, verified against the
code by the cycle-115 eval.

estimate: ~25 changed lines all-in (tests-only: one test file, maybe two
if the family sweep finds siblings)

## Requirements

1. In `chug_launch_stub_spawn_pins_exact_argv_cwd_and_return_paths`,
   replace the point-assert on `events_path.is_file()` with a
   deadline-bounded poll for THAT file before asserting — the same 10s
   discipline as `wait_for_stub_dump` (reuse it or add a small
   `wait_for_file` sibling; on a miss the assertion names the launch
   outcome, the T158 req-3 pattern).
2. **The stub is unchanged** — its write order is the behavior under
   test. Only the test's waiting changes.
3. **Sweep the family** (the cycle-33 doctrine): every OTHER test in
   `src/mcp_serve/tests.rs` and `tests/mcp_serve.rs` that polls one
   stub-written file and then point-asserts a DIFFERENT stub-written file
   gets the same per-file poll. The impl child lists every stub-written
   file assert in both files in its commit message and either converts or
   justifies each (an assert on a file written BEFORE the polled file —
   like `cwd.txt` here — is already safe and says why).
4. No production-code change; no new dependencies; clippy
   `--all-targets -D warnings` stays zero-warning.

## Tests

- The named test + the whole `mcp_serve` module stem green (bin unit
  tests), plus `tests/mcp_serve.rs` if the sweep touched it.
- The child demonstrates the fix's premise by code inspection in its
  goal summary: every assert on a stub-written file is now preceded by a
  poll on THAT SAME file (a flake's absence cannot be proven by a green
  run — the ordering gap is closed by construction).
- 5 consecutive full runs of the named test binary under nextest, green
  (the isolation shape that previously masked the race), plus one full
  `cargo test --bin chug` run green.

## Acceptance

- The ordering gap is closed at every instance found in the sweep; the
  named test's assert order matches the stub's write order with polls;
  gates green. T189 gates-only lane expected (src/mcp_serve/tests.rs is
  NOT on the step-4 REQUIRED core list; ≤150 lines; no new surface; no
  CI/check-line change beyond this spec).

check: export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared; touch src/*.rs tests/*.rs; cargo test --bin chug mcp_serve && cargo test --test mcp_serve
