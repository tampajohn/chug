# T35 — README quickstart: make the commands work as written for a cold reader

check: grep -q 'cargo install --path .' README.md

## Repo context

The README Quickstart (`README.md`, first code block) reads:

```
cargo build
# Zero setup if you have Claude Code configured: …

chug run --spec SPEC.md --goal "Build X and make the check pass" …
chug run --tui …
chug run --resume
chug chat
chug ledger
```

`cargo build` produces `target/debug/chug` and puts NOTHING on PATH, so
every bare `chug …` invocation that follows fails with
`command not found` for a cold reader following the block top to
bottom. Verified live in the cycle-14 eval (EVALUATION.md §6(e)):
`which chug` is empty and `~/.cargo/bin/chug` absent even for the
operator — the supervisor launches `./target/debug/chug`. This is
META-META-SPEC §6(e) quickstart-truth debt: the commands do not work
as written, in the order given.

## Requirements

1. Add a `cargo install --path .` step to the Quickstart block,
   immediately after `cargo build`, with a one-line comment noting it
   puts the `chug` binary on PATH (e.g. `# install the binary onto
   PATH (~/.cargo/bin)`). This is the pinned mechanism — chosen over
   `./target/debug/chug` prefixes because it keeps every later
   invocation (and the `chug chat` / `chug ledger` lines, and the
   `loopd.sh` section's examples) noise-free and matches the
   block's "zero setup" framing.
2. Keep the rest of the Quickstart block byte-identical (the
   `~/.claude/settings.json` auth comment, the example invocations).
3. No other README section is touched. (The Development section's
   `cargo build && cargo clippy … && cargo test` line is correct
   as-is — those are cargo commands, not chug invocations.)
4. Commit message cites the §6(e) finding and the live verification.

## Tests

- Docs-only change: no code tests apply. The `check:` line pins the
  install step's presence.
- Run `cargo test -- --test-threads=4` once to confirm the tree is
  green (you changed no code).

## Acceptance

- `check:` passes verbatim from the worktree.
- A cold reader following the Quickstart block top-to-bottom gets a
  working `chug` on PATH before the first `chug …` invocation
  (reviewer reads the block in order).
- `git diff` shows hunks in README.md only.

## Boundaries

- Implement TODO item t35 ONLY. DO NOT touch TODO.md or LEDGER.md in
  the main tree — bookkeeping is the orchestrator's. Do not edit any
  file other than README.md.
