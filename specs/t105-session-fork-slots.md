# T105 — F6 phase 1: session fork slots (`chug fork` save/list/restore)

check: cargo test --bin chug fork

## Repo context

Cycle-58 eval §4 (EVALUATION.md): the mandatory roadmap pull. Tier 1 is
exhausted (F1, F13-p1, F2-p1, F3-p1, F4-p1, F5-p1 all landed); Tier 2's
top unworked item is **F6 (Session fork)** — "Clone transcript+ledger at
iteration N into a new session id; explore two approaches from one
state" (benchmark: unreal-agent forking). SPLIT per the FEATURES.md
working rules (full scope blows one child budget): **phase 1 = named
fork slots** (serial exploration), phase 2 deferred with written reason
(`--session <name>` concurrent path plumbing through
transcript/driver/chat + fork-at-iteration-N surgery riding the T77 trim
machinery — the serial slot covers the benchmark's explore-two-approaches
shape, and concurrent same-cwd forks are already barred by the T55
driver lock; concurrency wants separate worktrees, which the loop
already has via delegate).

State of the art this builds on: sessions are per-cwd —
`.chug/transcript.jsonl` (src/transcript.rs: `transcript_path(cwd)`,
`append`/`load`/`rewrite`, `rotate_fresh` archives a non-empty
transcript to `.chug/transcript-<ts>.jsonl`, T7) plus `LEDGER.md` (a
fresh run archives a non-seed ledger, T3). `--resume` continues both.
There is no session-id concept; a fork is a named SAVE/RESTORE slot over
the same two files.

## Requirements

1. New subcommand `chug fork` (src/main.rs CLI + a new `src/fork.rs`;
   README layout line + the T95 guard absorb the new module — the guard
   walks `src/*.rs` so `fork` JOINS the layout list) with three actions:
   - `chug fork save <name>` — snapshot `.chug/transcript.jsonl` and
     `LEDGER.md` (whichever exists; transcript ABSENT or empty → error
     `fork: nothing to save — no transcript in <cwd>`) into
     `.chug/sessions/<name>/{transcript.jsonl,LEDGER.md}`. `<name>`
     validates as `[A-Za-z0-9._-]+` (no separators, no `..`), error
     names the rule. Refuses to overwrite an existing slot without
     `--force`.
   - `chug fork list` — one line per slot: name, transcript bytes,
     mtime, and the first user/message preview (best-effort, ≤80 chars);
     empty slots dir → one `no forks` line, exit 0.
   - `chug fork restore <name>` — refuses while a live run holds
     `.chug/driver.lock` (T55 interlock: reuse driver_lock's liveness
     probe; error names the holding pid), then ROTATES the current live
     transcript + non-seed LEDGER.md aside with the existing T7/T3
     archive machinery (nothing is lost — the live session becomes a
     timestamped archive), then COPIES the slot's files into place
     (`.chug/transcript.jsonl`, `LEDGER.md`). Prints what was archived
     and what was restored. Missing slot → error naming it.
2. Copy semantics only — a slot never mutates on restore (restore again
   → byte-identical outcome). Restore of a slot whose LEDGER.md is
   absent leaves any current LEDGER archived and no ledger seeded (the
   next fresh run seeds per T3).
3. Events/banners: none (fork is a CLI op outside a run; the resumed
   run's own `run_start` records the continuation). No driver.rs,
   chat.rs, transcript.rs, or ledger.rs behavior changes — the T3/T7
   archive fns are REUSED, not edited (if a seam is genuinely missing,
   the smallest `pub(crate)` exposure with a comment, nothing more).
4. Errors are honest and name the fix: every failure leg (bad name,
   missing slot, existing slot without --force, lock held, IO error)
   prints a one-line stderr message and exits non-zero; success prints a
   one-line summary per action.

## Tests

New `src/fork.rs` test module (tempdir fixtures, the T3/T7 harness
shape):
- save → slot files byte-identical to sources; save existing name →
  refusal; `--force` overwrites; name-validation legs (`a/b`, `..`,
  empty, `a b`) all refused naming the rule; save with no transcript →
  the req-1 error.
- list: empty dir → `no forks` exit 0; populated → names + sizes.
- restore: happy path restores byte-identical files AND the pre-restore
  live transcript/ledger land in timestamped archives (T7/T3 reuse
  proven, nothing lost); restore is idempotent (slot unchanged, second
  restore re-archives and re-copies); missing slot → error; a LIVE
  driver.lock (real pid of the test process) → refusal naming the pid;
  a STALE lock → restore proceeds (the T55 stale-reclaim semantics).
- RED proof: at least two legs demonstrated failing against a reverted
  leg (e.g. overwrite-refusal removed; lock check removed) in the commit
  message or ledger.
- Full `cargo build` + `cargo clippy --all-targets -- -D warnings` +
  `cargo test` green in the worktree; `cargo test --test readme_layout`
  green AFTER the layout line gains `fork` (the guard REQUIRES the edit
  — prove it RED by adding the module without the line).

## Acceptance

- The `check:` line passes in the worktree (the `fork` stem runs every
  new fork test — T96 breadth rule; verify the filter catches them).
- Diff is src/main.rs + src/fork.rs + README.md (+ any minimal seam
  comment) ONLY. README gains a compact `## Session forks` section
  between Autonomous mode and Plan mode: three commands, the serial
  semantics (explore A, restore, explore B), the nothing-lost guarantee
  (restore archives the live session first), and the lock interlock.
- FEATURES.md F6 row gains the phase-1 LANDED annotation + the phase-2
  deferral reason in the row-flip commit (orchestrator's edit at merge).
- Validation routing: no REQUIRED-listed file (driver/api/tools/events/
  doctrine untouched) → kimi OPTIONAL per T16/T31; the destructive leg
  is safe-by-construction (restore rotates before copying).
