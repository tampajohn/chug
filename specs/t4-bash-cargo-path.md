# T4 — Bash tool prepends ~/.cargo/bin to PATH

check: cargo test

## Status

DONE-RECORD. Shipped in `9840aba` (closed by TODO row T4). Backfilled
under T8 (EVALUATION.md I10).

## Concern

2026-09-21 transcript mining: nearly every command in every session opened
with `export PATH="$HOME/.cargo/bin:$PATH"` — children and metas each had
to discover `cargo: command not found` individually (several first
attempts died outright). chug's bash tool should prepend ~/.cargo/bin
itself — it's our own toolchain.

## What shipped

- `src/tools.rs:538-564` — `run_shell` prepends `~/.cargo/bin` to the
  child's PATH when that directory exists; a PATH that already contains it
  is left untouched.
- `run_shell` is shared by BOTH the bash tool and the goal-check path
  (`driver.rs` `verify()` runs the spec's `check:` command through it with
  `CHECK_TIMEOUT_SECS = 600`, `src/tools.rs:17`), so the model's shell and
  the verifier's shell see the same environment.

## Regression tests

`src/tools.rs` tests: `bash_tool_finds_cargo_without_path_prefix` (and the
`:897` fixture creating a fake `~/.cargo/bin`). Run `cargo test tools::tests`.
