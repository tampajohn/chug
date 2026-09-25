# T20 — Startup banner + run_start gain the cwd's worktree HEAD (branch@commit)

check: cd /Users/jadams/workspace/chug && cargo test

## Why (evidence)

The wrong-HEAD confusion class has now fired twice across two cycles:

1. Cycle 4: kimi validator 2 for T17 was killed early after forming a
   wrong-HEAD belief about its worktree (EVALUATION.md cycle-4 Outcomes).
2. Cycle 5: the T18 kimi validator opened with "I'm on `main` but the
   commit exists" (`/tmp/chug-loop-t18-validate.log`, iteration 3) and
   only self-corrected because its goal spelled out the ground truth
   (`git diff main...loop-t18`, commit `9056c78`).

Root cause: the T11 banner prints the **binary's build-time** commit
(`build.rs`-baked `CHUG_GIT_HASH`) but children run the **main-tree
binary** with a **worktree cwd** (the standard LOOP-SPEC §2 launch) — so
the one identity line a child sees at startup names a commit that is
*not* its checkout's HEAD, and says nothing about the branch. The banner
is actively misleading in exactly the sessions (worktree children) that
most need orientation.

## Repo context

- `src/build_info.rs` — `banner()` (`:25`), `print_startup_banner()`
  (`:35`), `GIT_COMMIT` baked const; module tests pin the exact banner
  string (`:44`, `:59`, `:67`). The T11 hard rule: **the banner never
  fails the run** — build-time git failure falls back to `"unknown"`.
- Call sites: `src/main.rs:260` (run), `src/main.rs:380` (chat).
- `run_start` event: `src/eventlog.rs:50` (`run_start()` writes
  version/commit/cwd/spec/mode/model + T17's budget ceilings), call
  sites `src/driver.rs:1357-1371` and `src/chat.rs:173`/:544-568; the
  T11/T17 pins live in `src/eventlog.rs` tests (`:243` area) and the
  driver/chat test modules.
- Runtime git elsewhere: build.rs uses `git rev-parse` at build time;
  there is no runtime git call in the binary today — this row adds the
  first one, best-effort.

## Requirements

1. At startup, resolve the **cwd's** worktree identity at runtime:
   `git -C <cwd> rev-parse --abbrev-ref HEAD` (branch; may be `HEAD` when
   detached) and `git -C <cwd> rev-parse --short HEAD`. Best-effort fast
   path (plain `Command` spawn, output captured; any failure → `None`).
   Never block, never fail the run.
2. The banner gains `head=<branch>@<short>` **only when resolvable**;
   when it isn't (not a repo, git missing), the banner line is
   byte-identical to today (T11's fallback pins keep passing untouched).
   The baked `(commit)` field stays — it identifies the binary; `head=`
   identifies the checkout; both are meaningful and their difference is
   precisely the signal.
3. `run_start` gains `head_branch` + `head_commit` (strings, or
   absent/null when unresolved — pick one representation and pin it) so
   harvested child streams carry the orientation too.
4. Every existing pin that asserts the banner/run_start shape is
   audited: exact-string banner tests get the new field injected via the
   same seams the tests already use (pure `banner()` takes the resolved
   values as parameters — no git in unit tests); run_start serialization
   pins gain the fields. No assertion weakened; the git-less fallback
   pins must NOT change.
5. README bullet under the banner feature (T11's section): the banner
   also names the checkout's `branch@commit` when it can.

## Tests

- `banner()` pure-function pins: with head `Some(("loop-t18","9056c78"))`
  the line contains `head=loop-t18@9056c78`; with `None` the line is the
  pre-T20 bytes exactly.
- A real-repo integration pin: resolution in the repo's own cwd yields
  the current branch and a 7+-char hex short hash; a non-repo tempdir
  yields `None` (no panic, no banner change).
- `run_start` line carries the two new fields (absent/null per the
  chosen representation when unresolved).
- Non-vacuousness: reverting to the pre-T20 banner must fail the new
  `head=` pin.

## Acceptance

- `cargo build`, `cargo clippy --all-targets -- -D warnings`, `cargo test`
  all green.
- Adversarial validation (REQUIRED — touches the events surface via
  `run_start`): confirm the never-fail invariant (simulate git failure),
  confirm no existing fallback pin changed, mutation-test the
  `Some`/`None` render legs.
