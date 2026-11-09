# T345 — chugd: wedged-listener degrade leg (the validator's t340 finding 2)

check: set -o pipefail; CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-impl-a cargo test --release --test daemon_status_contract

## Problem

The t340 validator's mutation leg m3 (read/write timeouts removed from `uds_request`)
SURVIVED its stated premise — the socket-absent degrade test kills nothing there
because connect fails at ENOENT before any read. The validator then proved the 2s
bound load-bearing by hand: a wedged listener (accepts, never answers) hung the
mutant >8s unbounded vs 2.49s bounded in the real code. The battery has no
wedged-listener fixture leg, so that bound is unpinned.

## Deliverable

Add one wire test to `tests/daemon_status_contract.rs` (or a small sibling fixture):

1. Bind a unix listener that `accept()`s and then never answers (drop the
   connection after accept, or just sleep-loop), point `CHUG_DAEMON_SOCK` at it,
   and assert `chug capabilities` completes with `daemon_status: false`, never an
   error, and bounded wall time (< 5s — the real code bounds at ~2.5s via the
   inherited JUDGE_TIMEOUT read timeout).
2. The test must KILL the m3 mutant (timeouts removed → the leg hangs/red — if
   the test framework needs a timeout wrapper to turn a hang into a red, wrap the
   exec in a bounded wait and assert the real code finishes inside the bound).
3. Optionally assert the same bound on the daemon-side probe path (the T340
   /status handlers read loopd state files, not sockets — this leg is
   capabilities-side only; do not invent a daemon-side socket read that does not
   exist).

## Constraints

- Named targets: `tests/daemon_status_contract.rs` (or the new fixture file) only.
  No src/ changes — the bound exists; this pins it.
- Keep the T48 rule: runtime cwd reads (`std::env::current_dir()`), never a
  compile-time `env!("CARGO_MANIFEST_DIR")` — the no_compile lint walks every
  .rs file.
- Gates: `cargo test --release --test daemon_status_contract` (or the new test
  target) + clippy `-D warnings` zero + the full suite green.

## Acceptance

- The new leg is green on main and RED under the m3 mutant (timeouts removed)
  — RED-proven, not asserted.
- estimate: ~40 lines.
