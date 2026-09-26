# T48 — test code resolves the repo root at runtime, never env!(CARGO_MANIFEST_DIR)

check: cargo test

## Context

T47 introduced a shared `CARGO_TARGET_DIR` (`target-shared/`) for every
worktree build. Cargo's incremental cache can hand a *cached* test binary to
a later `cargo test` run in a different checkout (main) without recompiling —
and any `env!("CARGO_MANIFEST_DIR")` in that binary holds the COMPILE-TIME
path (the since-removed worktree), not the run-time checkout.

This fired for real in cycle 21: post-merge gates in main failed 2/452
because the cached binary had been compiled in `/tmp/chug-loop-t43` and read
files under that dead path (commit `c8a8382`'s message records the incident;
recovery was touch + rebuild). Once the operator restarts loopd, T47 is
active at supervisor level and EVERY gate runs the shared cache — every
removed worktree becomes a stale-path landmine, and the loop's own spec
`check:` (`cargo test`) is exposed: a false red can reject a truthful
`goal_complete` at the iteration margin (the cycle-16 death class).

The five sites (ALL are test code — no production use exists):

1. `src/tools.rs:3583` — unit test reads
   `concat!(env!("CARGO_MANIFEST_DIR"), "/README.md")`
2. `src/build_info.rs:194` — unit test
   `resolve_head(Path::new(env!("CARGO_MANIFEST_DIR")))`
3. `tests/todo_consistency.rs:135` — `Path::new(env!("CARGO_MANIFEST_DIR"))`
4. `tests/shared_target_dir.rs:21` — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
   (its `repo_root()` helper)
5. `tests/eval_digest.rs:19` —
   `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts/eval-digest.sh")`

The fix invariant: **cargo executes test binaries with the current
directory set to the package root** (the directory containing Cargo.toml),
for both unit tests (src/) and integration tests (tests/). So
`std::env::current_dir()` at test run time resolves the checkout the binary
is RUNNING against — correct under cached-binary reuse, correct in
worktrees, correct in main.

## Requirements

1. Replace all five `env!("CARGO_MANIFEST_DIR")` test sites with a runtime
   resolution based on `std::env::current_dir()`. Each replaced site (or a
   shared helper) carries a one-line comment naming the invariant: cargo
   runs test binaries with cwd = package root; `env!` bakes the compile-time
   checkout, which is wrong under the T47 shared cache (cycle-21 incident).
2. Integration tests (`tests/`) may each resolve inline or via a tiny local
   helper (no new shared crate module — test files can't share code without
   one); the two src/ unit tests may add a `#[cfg(test)]` helper in their
   module or inline `current_dir()`. Duplication of a two-line helper across
   test files is acceptable and preferred over new plumbing.
3. NO production (non-`#[cfg(test)]`) code changes. The build-time
   `build.rs` / `CHUG_GIT_HASH` machinery is untouched (it does not use
   `CARGO_MANIFEST_DIR` via `env!` in a cache-unsafe way and is out of
   scope).
4. New static regression pin: a small integration test
   (`tests/no_compile_time_manifest_dir.rs`, following the
   `tests/shared_target_dir.rs` static-content-pin pattern) that walks
   `src/` and `tests/` `.rs` files and asserts ZERO occurrences of
   `env!("CARGO_MANIFEST_DIR")` (byte-level substring search is fine; the
   pin fails if the pattern re-enters anywhere in test or production Rust
   code, forcing a deliberate re-review). The pin test itself must NOT
   contain the literal substring in a way that matches its own scan — e.g.
   build the needle by concatenation (`concat!("env!(", "\"CARGO_MANIFEST_DIR\"", ")")`)
   or scan with the quote characters constructed, and it must also skip
   itself when walking (either way is fine; pick one and comment it).
5. Behavior of the five existing tests is unchanged except for the root
   resolution: same assertions, same fixtures (e.g. `tests/eval_digest.rs`
   still runs `scripts/eval-digest.sh` from the resolved root;
   `tests/todo_consistency.rs` still reads TODO.md + specs/ from the
   resolved root).

## Tests

- The five existing test sites pass unmodified in behavior (requirement 5).
- New `tests/no_compile_time_manifest_dir.rs`: green now, red if any
  `env!("CARGO_MANIFEST_DIR")` reappears under `src/` or `tests/`.
- Full gates: `cargo build`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test` — all green BOTH plain AND with
  `CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared` (the T47
  export the goal carries). Green under the shared cache from the worktree
  is the acceptance-critical leg: it is the exact configuration that
  false-redded cycle 21.

## Acceptance

- `grep -rn 'env!("CARGO_MANIFEST_DIR")' src/ tests/` returns nothing.
- Gates green in the impl worktree under the shared target cache.
- The new pin test demonstrably fails when a scratch
  `env!("CARGO_MANIFEST_DIR")` is added to any test file (the impl child
  verifies this once, then reverts — mutation self-check, recorded in the
  child's ledger).
- `cargo test` (the check above) passes from the worktree.

## Notes

- Out of scope: making cargo's cache key checkout-aware, per-worktree
  target dirs (defeats T47), or touching loopd.sh. Those are rejected
  alternatives, not future work.
- If a sixth `env!("CARGO_MANIFEST_DIR")` site is discovered during
  implementation, convert it too and name it in the commit message.
