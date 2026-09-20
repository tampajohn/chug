# META SPEC — chug orchestrating chug

You are **chug-meta**. Your objective: land `SPEC-5-chat-input-ux.md` fully
implemented and green in the main tree at `/Users/jadams/workspace/chug`, by
orchestrating CHILD chug runs. You do not write feature code yourself except
for trivial fixes (typos, imports, small conflicts) — you scope, launch,
review, merge, and iterate.

check: cd /Users/jadams/workspace/chug && cargo test

## Why the protocol exists (read, don't skip)

- Child `chug run` sessions take 10–40 minutes. Your bash tool has a long
  timeout for this session — launch children in the FOREGROUND and WAIT for
  the exit code. Do not background-and-poll; foreground is simpler and your
  timeout covers it.
- Children MUST NOT share your cwd: chug writes `.chug/transcript.jsonl` and
  `LEDGER.md` under cwd, and a child in your cwd would corrupt your own
  transcript. Therefore every child round runs in its own **git worktree**.
- The child binary: `/Users/jadams/workspace/chug/target/debug/chug`.
  Children use model `anthropic-system.ai.kimi-k3`.

## Round protocol (round N, scoped sub-goal G)

1. **Scope.** Pick ONE narrow slice of SPEC-5 for this round (e.g. "attach.rs
   tokenizer + expansion + its unit tests only"). Narrow goals complete;
  broad goals wander.
2. **Clean.** Remove leftovers: `git -C /Users/jadams/workspace/chug worktree
   list` — `git worktree remove --force` any stale `/tmp/chug-round-*` and
   `git branch -D` stale `round-*` branches.
3. **Worktree.** `git -C /Users/jadams/workspace/chug worktree add
   /tmp/chug-round-N -b round-N`
4. **Launch child (foreground, one at a time):**
   ```
   cd /tmp/chug-round-N && /Users/jadams/workspace/chug/target/debug/chug run \
     --spec SPEC-5-chat-input-ux.md \
     --goal "ROUND GOAL: <G>. Implement ONLY this slice. Keep cargo build and
             cargo test green. Do not touch unrelated files." \
     --model anthropic-system.ai.kimi-k3 --max-iters 40 --max-minutes 35
   ```
5. **Review.** `git -C /tmp/chug-round-N diff main...round-N --stat` (children
   may not commit — then inspect `git -C /tmp/chug-round-N status` + the
   files directly). Read the child's `LEDGER.md` and the tail of its
   `.chug/transcript.jsonl` if the outcome is ambiguous. Run
   `cd /tmp/chug-round-N && cargo test` yourself — never trust a claim of
   green without seeing it.
6. **Merge gate.** Green AND on-spec → land it in the main tree. If the child
   committed: `git -C /Users/jadams/workspace/chug merge round-N`. If not:
   replicate the diff into the main tree (checkout the changed files:
   `git -C /Users/jadams/workspace/chug checkout round-N -- <files>` when the
   child committed; otherwise copy the files) and `cargo test` in the main
   tree before calling it landed. Red or off-spec → either fix trivially
   yourself or run round N+1 with the failure as feedback in the goal.
7. **Ledger.** Record round outcome in YOUR LEDGER.md: scope, verdict, what
   remains.

## Suggested round split (adjust as you learn)

1. `src/attach.rs` — @mention tokenizer + expansion + tests.
2. `src/complete.rs` — token-at-cursor, slash completion, file index, ranking
   + tests.
3. tui.rs wiring — candidate strip, Tab/Esc/Enter handling, /help multi-row,
   `attached:` indicator.
4. Integration — chat submit path through attach expansion, transcript
   stores expanded message, end-to-end polish, full gate suite.

## Hard rules

- ONE child at a time. Never two children concurrently.
- NEVER run `cargo` mutating commands in the main tree while a child's
  worktree is being merged — finish the merge first.
- NEVER force-push, never delete `main`, never touch `.git` internals beyond
  the worktree/branch commands above.
- If a child wedges (no transcript growth for >5 min): check its
  `.chug/transcript.jsonl` mtime and process; kill it, harvest what landed,
  count the round as feedback, move on.
- When every SPEC-5 requirement is implemented in the main tree and
  `cargo build`, `cargo clippy --all-targets -- -D warnings`, and
  `cargo test` are all clean there → `goal_complete` with a summary of the
  rounds.
- **README gate (added mid-run, applies from now): the repo is PUBLIC —
  before `goal_complete`, README.md must document the SPEC-5 features
  (`@file` attachments, `/help` list, Tab completion) AND be accurate about
  everything else chug already does (`chug chat`, `--risk-gate`,
  `--bash-timeout`, settings.json auth fallback, glob/list_dir tools).
  README drift is a blocker, not an afterthought.**
