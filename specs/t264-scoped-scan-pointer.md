# T264 — Scoped-scan pointer: unscoped recursive walks die at the bash cap (META-META-SPEC clause)

check: cd "$(git rev-parse --show-toplevel)" && export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-validate-a && touch src/*.rs tests/*.rs && grep -q 'Scoped drills (T264)' META-META-SPEC.md && grep -q -- '--exclude-dir' META-META-SPEC.md && cargo test --release --test todo_consistency --test eval_state_delta --test loop_spec_cheap_exit --test loop_spec_recovery --test eval_outcomes_carry --test loop_spec_validation_lane

## Repo context

- Two fires of the same class, both in trip-eval streams, both killed at the
  300s bash cap (`CHUG_BASH_TIMEOUT=300` across the loop fleet):
  1. cycle-241 stream (13:31:49Z, duration 300,289ms): a `find` over the
     operator's home tree (`-maxdepth 4`, looking for loopd logs) — a
     home-tree walk (`~/Library/Caches` and all), killed. Absorbed at census
     1 with the trigger hung: "2nd fire files a scoped-scan pointer row"
     (eval-triage d1791469301-2).
  2. cycle-245 stream (14:32:10Z, duration 300,160ms): `grep -rln '<needle>'
     --include='*.html' --include='*.md' --include='*.js' .` from the repo
     root during the site tests-fact hunt — the `--include` filters CONTENT
     but the `-r` walk still descends `target/` and `.git/`; the trailing
     `| grep -v target | head` pipe filters AFTER the walk, so it bounds
     nothing. Killed.
- Root cause (knowable, verified): an unscoped recursive walk's input shape
  is unbounded — the walk VISITS every directory entry under the root;
  content flags (`--include`, `-name`) only decide what gets read or
  printed, never what gets visited. Under the loop's 300s bash cap the
  command dies mid-eval at ~5 min of orchestrator wall plus one failed tool
  result per fire. Both fires self-corrected the next command (`git grep`,
  exact paths).
- The drills are otherwise correct instincts: META-META-SPEC corpus item 1
  sends the evaluator to the raw archives "ONLY to drill into a specific
  incident the digest raised" — the missing half is that a drill's
  filesystem commands must be scoped the same way the corpus list is.
- estimate: ~20 changed lines (one META-META-SPEC.md paragraph + this spec +
  the TODO row). Mechanical doctrine addition — no code, no test changes.
- **Pin-carrier discipline (T80):** META-META-SPEC.md is a pinned doctrine
  carrier — `tests/eval_state_delta.rs`, `tests/loop_spec_cheap_exit.rs`,
  `tests/loop_spec_recovery.rs`, `tests/eval_outcomes_carry.rs`,
  `tests/todo_consistency.rs`, and `tests/loop_spec_validation_lane.rs` all
  read its content (some with exact-count needles). The new paragraph is
  append-only and must NOT quote any existing pinned needle verbatim; the
  `check:` line runs every pin binary that reads the file, so a broken pin
  fails the goal gate, not just review.

## Requirements

1. Add ONE paragraph to `META-META-SPEC.md`, placed immediately after the
   corpus list (after item 6's README bullet, before the
   `## Write EVALUATION.md` header), bold-led `**Scoped drills (T264).**`,
   carrying the rule:
   - Every ad-hoc filesystem walk an evaluation drill runs is SCOPED —
     never `find` from `$HOME` and never `grep -r` from the repo root: the
     walk itself is unbounded (`target/`, `.git/`, `~/Library`), the 300s
     bash cap kills it mid-eval, and a trailing `| grep -v … | head` pipe
     filters AFTER the walk so it bounds nothing (two fires: the cycle-241
     home-tree `find` 300s kill, the cycle-245 repo-root `grep -rln … .`
     300s kill).
   - The scoped alternatives a drill reaches for FIRST: `git grep`
     (index-bounded), `grep -r --exclude-dir=target --exclude-dir=.git`,
     `find` under a bounded root with `-prune` for the big dirs, or the
     digest/delta's mechanical surfaces before any raw walk.
2. No other repo file changes (this spec and the TODO row are the
   orchestrator's bookkeeping, not the child's).
3. The paragraph must not duplicate any existing META-META-SPEC sentence
   verbatim (the exact-count pins), and must not name operator-identifying
   paths (the internal-info lint): `$HOME`, `target/`, `.git/`,
   `~/Library` are the load-bearing examples.

## Tests

- The `check:` line's two greps (the bold lead + the `--exclude-dir`
  alternative).
- Every pin binary that reads META-META-SPEC.md green (the `check:` line's
  six `--test` legs) — the T80 pinned-carrier discipline for an md-only
  edit.

## Acceptance

- META-META-SPEC.md carries the scoped-drills paragraph; `check:` green.
- Review/merge gates per the docs-only classification (the diff touches
  ONLY `*.md`): the T80 guard floor
  (`cargo test --release --test todo_consistency`) PLUS the six pin
  binaries named above; the full suite skipped per the T80 letter.
- Validation: kimi REQUIRED (doctrine edit — META-META-SPEC.md is on the
  step-4 core list by construction).
