# T97 — README `delegate` paragraph → per-action sub-bullets

check: grep -q '`delegate`' README.md && cargo test --test nextest_gate_runner --test shared_target_dir

## Repo context

Cycle-53 eval §6(b): the `delegate` paragraph in README.md's ## Tools
section is ONE ~24-line paragraph now carrying five sub-behaviors —
launch, status, collect, wait_secs (+terminal), and the sandbox
exception. The density watch carried across 7 evals is judged: split it
into per-action sub-bullets for readability. This is a RESTRUCTURE of
reference prose: zero behavior claims change, zero sentences are lost,
nothing is re-ordered beyond the bullet grouping.

## Requirements

1. The single `delegate` paragraph becomes a short lead-in sentence plus
   sub-bullets: **`launch`**, **`status`** (incl. wait_secs + terminal),
   **`collect`**, and a sandbox/cwd note (the absolute-path +
   worktree-targeting + "worktree creation, building, harvest/merge, and
   killing stay with your bash" sentences).
2. Every factual claim of the current paragraph survives verbatim or
   near-verbatim (defaults 40/35, max_tokens, resume, delegate.log path,
   latest-run-segment semantics, wait_secs 0/absent instant + 600 cap +
   waited: line, terminal wake set + the one-orchestrator-iteration-per-
   child-run rationale, collect's verdict/summary/check-cmd/commit-refs +
   base scoping, the two documented sandbox exceptions sentence that
   follows the tools list is NOT part of this paragraph — leave it).
3. ZERO behavior-text change: no numbers, defaults, caps, or semantics
   altered; no new claims; nothing deleted but the paragraph's connective
   filler.
4. The paragraph's neighbors (`decision_log` before, `web_fetch` after)
   stay byte-identical.

## Tests

- `cargo test --test nextest_gate_runner --test shared_target_dir` green —
  these pin files read README.md (their needles are elsewhere in the file;
  this is the sweep proving the restructure broke no pin). If any OTHER
  test pins delegate-paragraph text (run the full suite to find out),
  update it minimally and prove it RED against the pre-edit README.
- Content check per the `check:` line.

## Acceptance

- `check:` green verbatim in the worktree; full suite green.
- Diff is README.md ONLY → md-only classification: gates are the
  docs-only floor (`cargo test --test todo_consistency`) + the
  README-reading pin files above at review and post-merge (LOOP-SPEC §2
  step 3's pinned-carrier rule — the floor does not run those pin files,
  so the editor runs them explicitly, which the check line already does).
- Validation: kimi SKIPPED per §2 step 4 (docs-only, T16/T31 precedent);
  orchestrator diff-review confirms claim-for-claim survival.
