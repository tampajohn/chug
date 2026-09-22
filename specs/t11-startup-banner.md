# T11 — Startup banner: version/commit/cwd/model

check: cargo test

## Concern

Stale-binary confusion has burned two sessions: SPEC-5's pty smoke ran an old
`target/debug/chug` because `cargo test` doesn't rebuild the bin
(`.chug/LEDGER-spec5-archive.md`), and the SPEC-9 orchestrator ran a pre-T4
binary so the check-env fix never applied to its own session
(`.chug/transcript.jsonl:501`; EVALUATION.md I11). Orchestrators also launch
children from worktree binaries that drift from main. There is no cheap way to
know which build a running chug is.

## Repo context

- `src/main.rs`: CLI entry (`run`/`chat`/`ledger` subcommands), clap derive;
  version comes from `CARGO_PKG_VERSION`.
- No `build.rs` currently; adding one is in scope (no new dependencies —
  capture `git rev-parse --short HEAD` at build time, fall back to
  `"unknown"` when git or `.git` is unavailable).
- Console output flows through `src/events.rs` sinks; a plain `eprintln!`
  before the loop starts is acceptable and simplest.

## Requirements

1. At `chug run` and `chug chat` start, print one stderr line:
   `chug <version> (<git-short-hash|unknown>) cwd=<cwd> spec=<path> model=<model>`.
2. The same line is appended to `.chug/events.jsonl` as the run-start event
   when T10 lands (if T10 is not yet merged, this spec does not require it).
3. No network, no new crates; `build.rs` must not fail the build when git is
   missing (fallback `unknown`).

## Tests

- Unit test the banner formatting function: contains the package version,
  cwd, spec, and model fields; hash placeholder tolerated.
- `build.rs` fallback: simulate missing git (env override) → banner shows
  `unknown`, build still succeeds.

## Acceptance

- `cargo build`, `cargo clippy --all-targets -- -D warnings`, `cargo test` all
  green.
- `chug run --spec …` shows the banner before iteration 1 on stderr.
