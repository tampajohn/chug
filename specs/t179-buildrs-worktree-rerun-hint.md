# T179 — build.rs: worktree `.git`-file case makes every cargo invocation a full rebuild

check: export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared && cargo test --test todo_consistency

## Repo context

`build.rs::emit_git_rerun_hints` emits `cargo:rerun-if-changed=.git/HEAD`
BEFORE attempting to read it; its own doc-comment says the worktree case
(`.git` is a FILE pointing at the main repo's worktree gitdir) should
"skip silently" — but in a worktree `.git/HEAD` never exists as a path,
and cargo treats a missing rerun-if-changed path as ALWAYS STALE:
`StaleItem(MissingFile { path: ".../.git/HEAD" })` (confirmed cycle 81
via `CARGO_LOG=cargo::core::compiler::fingerprint=trace`). The build
script re-runs on EVERY cargo invocation, and its dependents (the whole
chug crate, ~40-95s release) rebuild with it — the hidden tax behind
slow worktree gates, eaten gate windows (cycle 81's bulk-suite kills at
110-115s), and child budget deaths (impl children pay it on every gate
call). Cycle 81's orchestrator carried a 2-line uncommitted
gate-enablement patch in /tmp/chug-loop-t175 (move the println after
the successful read; reverted byte-identical pre-merge, disclosed to
the kimi validator in its goal) — this row is the REAL fix.

estimate: ~40 changed lines (2-line hint move + expanded comment
naming the worktree gitdir case + a source-shape pin leg in the
nearest tests file).

## Requirements

1. build.rs emits `cargo:rerun-if-changed=.git/HEAD` ONLY when the read
   succeeded (move the println inside the read-OK path, exactly the
   cycle-81 gate-enablement shape); expand the doc-comment: in a
   worktree `.git` is a file, so `.git/HEAD` is a missing path, and
   cargo treats missing rerun-if-changed paths as always-stale →
   per-invocation full rebuilds; the comment names the cycle-81
   discovery and the CARGO_LOG evidence command.
2. A source-shape pin leg (tests file of the family's choice —
   `tests/todo_consistency.rs` or a new `tests/build_script_shape.rs`
   following the loop_spec_* conventions): assert build.rs's
   `emit_git_rerun_hints` body orders the read BEFORE the
   rerun-if-changed emission (e.g. the read's `else { return; }` arm
   appears before the println in the function source) — non-vacuous
   (revert the hint move → red).
3. Main-checkout behavior unchanged: in the main tree `.git/HEAD`
   exists and is still watched (commit moves rebuild the binary — the
   banner hash stays honest where it worked before).
4. No other build.rs change; no src/ code; the cycle-81 finding that
   exporting CHUG_GIT_HASH in a gate env breaks the delegate-launch
   stub tests is DOCUMENTED in the comment (one clause) but NOT
   behaviorally changed.

## Tests

`cargo test --test todo_consistency` green (plus the new pin's file);
RED-prove the pin by reverting the hint move (the read-back-to-front
shape) — state the proof in the commit message. Verify the FIX's
effect manually in a worktree: two consecutive `cargo build --release
--tests` calls, the second printing zero `   Compiling` lines (state
the observed numbers in the commit message).

## Acceptance

- Spec check green; full `cargo test` green; clippy `-D` clean.
- Two consecutive cargo invocations in a fresh worktree: the second
  recompiles nothing (the cycle-81 measured tax gone).

## Out of scope

- Reworking the banner-hash mechanism (a gitdir-resolving rerun target
  that also works in worktrees — a follow-up if wanted), CI/release
  workflows, the CHUG_GIT_HASH stub-test interaction beyond the
  documenting clause.
