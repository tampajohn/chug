# T9 — goal_rejected message states the check environment

check: cargo test

## Concern

When `goal_complete`'s check fails, the model is told only "goal_complete
rejected: the spec check command failed. Output: … Fix the failure and try
again" (`src/driver.rs` `verify()` call site). With no environment context, the
SPEC-9 meta concluded cargo had to be made globally resolvable and created a
symlink under `~/.claude/plugins/cache/…` — outside the sandbox, still on disk
(transcript 479–489; EVALUATION.md I7). T4 already unified the environments
(the check runs via the same `tools::run_shell` as the bash tool, with
`~/.cargo/bin` prepended), but the model has no way to know that.

## Repo context

- `src/driver.rs`: `verify()` runs `tools::run_shell(cwd, command,
  CHECK_TIMEOUT_SECS)`; the rejection text is built at the `VerifyOutcome::Failed`
  call site in `drive_loop`.
- `src/tools.rs:15,17`: `BASH_TIMEOUT_SECS=120`, `CHECK_TIMEOUT_SECS=600`;
  `:538-564` — `run_shell` PATH prepend (T4) shared by bash and the check.
- This is a message-only change: verification semantics do not change.

## Requirements

1. The rejection message gains an environment note, e.g.:
   "Environment note: the check ran via the same shell wrapper as your bash
   tool (`sh -c` in the run cwd, `~/.cargo/bin` prepended to PATH when that
   directory exists, 600s timeout). If the check fails on a missing tool that
   works in your bash tool, suspect the check command itself — do NOT create
   or modify files outside the run cwd to make the check pass."
2. Wording must stay accurate if the T4 prepend logic changes: derive any
   literal paths/values from the same constants where feasible, or assert the
   key phrases in a test so drift breaks the build.

## Tests

- Unit test on the constructed rejection message: contains the `~/.cargo/bin`
  mention, the shared-run_shell fact, and the out-of-cwd prohibition.
- Existing `goal_complete` rejection tests (driver/chat) updated only if they
  assert exact message text.

## Acceptance

- `cargo build`, `cargo clippy --all-targets -- -D warnings`, `cargo test` all
  green.
- A failing-check run shows the new note in the tool result the model sees.
