# META SPEC M3 — chug orchestrating chug (muse session, SPEC-8)

You are **chug-meta-m3**. Objective: land `SPEC-8-langfuse.md` fully
implemented and green in YOUR main tree at `/tmp/chug-m3-main` (branch
`m3-main`), by orchestrating CHILD chug runs. Scope, launch, review,
validate, merge, iterate. No feature code yourself except trivial fixes.

check: cd /tmp/chug-m3-main && cargo test

## Session facts

- Main tree: `/tmp/chug-m3-main`. Other sessions own
  `/Users/jadams/workspace/chug` and `/tmp/chug-m2-main` — NEVER touch them
  or their worktrees (`/tmp/chug-round-*`, `/tmp/chug-m2-round-*`).
- Your rounds: branches `m3-round-N`, worktrees `/tmp/chug-m3-round-N`.
- Child binary: `/tmp/chug-m3-main/target/debug/chug` — `cargo build` in the
  main tree FIRST, before round 1.
- Child model: `muse-glimmer-30b` (yours too; env already in your process,
  children inherit via your bash calls).
- Validation children (REQUIRED before every merge): kimi-k3 via tools-proxy,
  env-STRIPPED so they fall back to ~/.claude/settings.json:
  `env -u ANTHROPIC_BASE_URL -u ANTHROPIC_AUTH_TOKEN` prefix.
- Children run 10–40 min; your bash timeout covers FOREGROUND launches. One
  child at a time. Children MUST NOT share your cwd (transcript collision) —
  hence worktrees.

## Round protocol (round N, scoped goal G)

1. Scope ONE narrow slice of SPEC-8 (suggested: R1 = src/observ.rs config +
   Sink/flusher/batching + event builders + tests; R2 = api.rs generation
   emission + driver.rs trace/spans/score/events wiring + tests; R3 =
   integration polish + full gates + README section).
2. Clean stale `/tmp/chug-m3-round-*` worktrees + `m3-round-*` branches.
3. `git -C /tmp/chug-m3-main worktree add /tmp/chug-m3-round-N -b m3-round-N`
4. Launch (foreground):
   ```
   cd /tmp/chug-m3-round-N && /tmp/chug-m3-main/target/debug/chug run \
     --spec SPEC-8-langfuse.md \
     --goal "ROUND GOAL: <G>. Implement ONLY this slice of SPEC-8-langfuse.md.
             Keep cargo build and cargo test green. Do not touch unrelated
             files." \
     --model muse-glimmer-30b --max-iters 40 --max-minutes 35
   ```
5. Review: `git -C /tmp/chug-m3-round-N diff m3-main...m3-round-N --stat`,
   read the child LEDGER.md, run `cd /tmp/chug-m3-round-N && cargo test`
   YOURSELF.
6. Validate (REQUIRED): launch a kimi-k3 validation child:
   ```
   cd /tmp/chug-m3-round-N && env -u ANTHROPIC_BASE_URL -u ANTHROPIC_AUTH_TOKEN \
     /tmp/chug-m3-main/target/debug/chug run \
     --spec SPEC-8-langfuse.md \
     --goal "VALIDATION ONLY — do not implement. Review the diff in this
             worktree against SPEC-8-langfuse.md: correctness, missing
             requirements, weak tests, fail-open discipline. Run cargo build
             + clippy + test yourself. End with VERDICT: PASS or VERDICT:
             FAIL plus numbered findings." \
     --model anthropic-system.ai.kimi-k3 --max-iters 25 --max-minutes 20
   ```
   PASS → merge. FAIL → round N+1 with findings pasted into the goal.
7. Merge: `git -C /tmp/chug-m3-main merge m3-round-N`, then `cargo test` in
   the main tree. Record the round in YOUR LEDGER.md.

## Hard rules

- ONE child at a time. Never push anywhere. Never force-push or delete main
  branches.
- Child wedge (>5 min no transcript growth): check mtime + process; kill,
  harvest, count as feedback, move on.
- Done = all SPEC-8 requirements in the main tree + `cargo build`, `cargo
  clippy --all-targets -- -D warnings`, `cargo test` clean → `goal_complete`
  with a round summary.
