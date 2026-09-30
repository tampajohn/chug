# T164 — spec `check:` line lint: no `--lib`, no failure-masking pipes, no main-repo `cd`

check: export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared && cargo test --test todo_consistency

## Repo context

`tests/todo_consistency.rs` — the T8 guard: scans TODO.md rows and
specs/t*.md mechanically (table format, spec-exists, estimate line). It
ALREADY carries `spec_check_lines_never_invoke_cargo_test_lib` (:261) —
but it matches the literal `cargo test --lib`, so `cargo test --release
--lib` (a flag in between) slips through: that is exactly how t160's line
passed the lint. The other two banned shapes have no lint at all. The
three banned check-line shapes are doctrine in META-META-SPEC.md's spec
quality bar (`--lib` ban; worktree-relative rule) and this eval's §2.2
(pipe-masking), with zero effective mechanical enforcement. Offenders in
tree today:
`specs/t160-t153-weak-pin-survivors.md` (`--release --lib` leg piped
through `tail`),
`specs/t153-chug-cancel-mcp.md` / `specs/t156-*.md` (`| tail` pipes),
`specs/t152-loopd-orphan-reaper.md` (`cd /Users/jadams/workspace/chug`).

estimate: ~220 changed lines (lint ~140 + legacy sweep ~10 spec files +
doctrine sentence + pin tests).

## Evidence

Cycle-76 eval §2.2: t160's `--lib ... | tail -3` leg exited 101 (binary-only
crate) and the goal gate PASSED — the pipe masked it (pipeline exit status
is the last command's). The same masking shape can hide a RED suite in any
spec's check line. Doctrine bans without a lint are honor-system.

## Requirements

1. Extend `tests/todo_consistency.rs` with a `check:`-line lint over every
   `specs/t*.md` (building on the existing
   `spec_check_lines_never_invoke_cargo_test_lib` at :261 — generalize it):
   (a) a check line MUST NOT pass `--lib` to cargo in ANY flag position
   (word-boundary match — the literal `cargo test --lib` substring misses
   `cargo test --release --lib`, the t160 escape);
   (b) a check line that pipes a build/test command (`cargo ...`) through
   `tail`/`head`/`grep` MUST contain `pipefail` (set -o pipefail) —
   otherwise the pipe masks failures;
   (c) a check line MUST NOT `cd` to an absolute path (worktree-relative
   rule — the T21 anomaly class).
2. SWEEP the legacy offenders so the lint is green: rewrite the offending
   check lines in the named spec files to compliant forms that still run
   the same test sets (e.g. drop `| tail`, drop `--lib` legs, make paths
   worktree-relative). Every rewritten line must still be a working check
   for its spec (verify by reading each spec's test surface).
3. META-META-SPEC.md's spec quality bar gains the pipe-masking ban as ONE
   sentence beside the `--lib` ban (the `cd` and `--lib` rules already
   exist there).
4. Each lint leg is RED-proven: a fixture/builtin negative case per leg
   (a sample spec text containing the banned shape fails the lint) —
   follow the file's existing test style for negative cases.

## Tests

- The three lint legs, positive (compliant lines pass) and negative
  (banned shapes fail, naming the rule).
- The full todo_consistency binary green after the sweep.

## Acceptance

- `cargo test --test todo_consistency` green; full `cargo test` green.
- Commit message lists every legacy spec rewritten and the RED-proof per
  lint leg.

## Out of scope

- Linting anything else about check lines (walls are T163's doctrine);
  rewriting specs beyond the compliance minimum; touching src/.
