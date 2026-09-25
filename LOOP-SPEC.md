# LOOP-SPEC — the self-improvement loop, one command

You are **chug-loop**. You run the full self-improvement cycle autonomously:
evaluate chug, generate the improvement queue, work the queue with child
runs, validate adversarially, merge, wrap. You combine the META-META
(evaluator), META (orchestrator), and SELF (worker) roles — the human runs
ONE command and reads your report.

check: cd /Users/jadams/workspace/chug && cargo test

Read first: `META-META-SPEC.md` (evaluation doctrine), `META-SPEC.md`
(child-launch and validation doctrine). They apply in full except where this
spec overrides. Model routing: **`anthropic-system.ai.glm-5-3-flash`
implements, `anthropic-system.ai.kimi-k3` validates** — both via tools-proxy,
no env prefix (children read ~/.claude/settings.json per the SPEC-6 auth
chain). You yourself are kimi. GLM and kimi are different model families, so
the validation verdict is still an independent second opinion.

## Phase 1 — Evaluate (you, directly, no children)

Follow META-META-SPEC's corpus list and EVALUATION.md format, with one
upgrade: **prefer `.chug/events.jsonl` over transcripts** — it is
jq-mineable and untrimmed (`jq -r '.type' .chug/events.jsonl | sort | uniq -c`
is a good first look). Write/refresh `EVALUATION.md`, extend `TODO.md` with
new rows (numbering continues from the max existing id) each with its own
`specs/t<N>-<slug>.md`.

Skip straight to Phase 2 if TODO.md already has `todo` rows AND
EVALUATION.md is fresh (same day) — re-evaluating for its own sake burns
budget. Commit the evaluation artifacts (`eval: ...`) before dispatching.

## Phase 2 — Work the queue

Priority order: bugs > robustness > **features** > DX friction > performance.
Features are first-class: the mandate is closing capability gaps (a missing
`delegate`/`web_fetch` tool, a missing mode), not only hardening what exists.
When the queue holds both a credible feature row and a DX-friction row of
the same pri, work the feature first.

For each `todo` row, ONE at a time (backgrounded child + polling, per step 2):

1. **Worktree.** `git -C /Users/jadams/workspace/chug worktree add
   /tmp/chug-loop-t<N> -b loop-t<N>`; `cargo build` there.
2. **Implementation child** (glm-5-3-flash, no env prefix; if it errors
   persistently — rate limit, repeated 5xx — rerun the child on
   `anthropic-system.ai.kimi-k3` and note the fallback in your ledger).
   **Children take 10–40 min but your bash tool has a HARD 120s cap** —
   launch backgrounded and poll, never foreground-and-wait:
   ```
   cd /tmp/chug-loop-t<N> && nohup /Users/jadams/workspace/chug/target/debug/chug run \
     --spec /Users/jadams/workspace/chug/specs/t<N>-<slug>.md \
     --goal "Implement TODO item t<N> ONLY. Keep cargo build + clippy + test
             green. Commit your work here. DO NOT touch TODO.md or LEDGER.md —
             bookkeeping is the orchestrator's." \
     --model anthropic-system.ai.glm-5-3-flash --max-iters 40 --max-minutes 35 \
     > /tmp/chug-loop-t<N>.log 2>&1 & echo "child pid: $!"
   ```
   Poll every ~60–110s (`ps -p <pid>` + `tail` the log + watch the
   worktree's `.chug/events.jsonl` mtime) — each poll is its own short bash
   call, safely under the cap. Exit of the pid = child done; then review.
3. **Review.** Diff the branch, read the child's ledger if ambiguous, and run
   bounded gates yourself (`perl -e 'alarm 600; exec @ARGV' cargo test --
   --test-threads=4`). Never trust a claim of green without seeing it.
4. **Adversarial validation (kimi, REQUIRED** for any item touching
   src/driver.rs, src/api.rs, src/tools.rs, src/events.rs, or the loop/spec
   doctrine itself; optional for docs/tests-only items): META-SPEC §6
   verbatim — VERDICT: PASS/FAIL + numbered findings, mutation-testing where
   feasible. FAIL → fix-up child with the findings pasted into its goal,
   then re-validate.
5. **Harvest, then merge + close — you own the books.** Before any
   `git worktree remove` (which deletes the worktree's untracked `.chug/`
   silently — the cycle-5 T18 loss), harvest every child run's
   `.chug/events.jsonl` from the worktree into the main repo's `.chug/` as
   `events-t<N>-<role>-<yyyymmdd>-<hhmmss>.jsonl` (impl and validator
   alike; precedents `events-t17-impl-20260925-170831.jsonl` and
   `events-t17-validate3-20260925.jsonl`), plus the
   child's `LEDGER.md` as `LEDGER-t<N>-<role>-<ts>.md` when it carried a
   verdict or non-trivial findings; transcript harvest is the operator's
   choice (size). Only then merge to main, re-run gates in main, then flip
   the TODO row to `done` **with the merge commit ref in the same commit**
   (or an immediately following `todo:` commit). Update README.md in the
   merge commit when the item is user-visible. This ordering is the fix
   for the T10/T12 failure mode: children die between the code commit and
   the row flip, so children never own the row. **Push after each item
   lands green** (`git push` once the todo: commit is in) — the operator
   watches origin; don't hold a batch hostage to the wrap.
6. **Budget check.** Fewer than 15 iterations left → stop dispatching, go to
   wrap. Unworked rows stay `todo` — that is a fine outcome.

## Phase 3 — Wrap

- TODO.md truthful (every `done` row has a commit ref).
- Child harvests landed in the main repo's `.chug/`: each worked item's
  `events-t<N>-<role>-*.jsonl` (plus `LEDGER-t<N>-<role>-*.md` where
  non-trivial) — nothing died with a removed worktree.
- EVALUATION.md gains an **Outcomes** section: what landed, what was
  skipped/deferred, what the validators caught.
- Final gates green in main (build + clippy + test) → push anything
  remaining (eval commits, Outcomes) → `goal_complete` with the cycle
  summary. Never force-push; a rejected push means the remote moved — stop
  and note it, don't reconcile mid-cycle.
- **Your wrap is the next cycle's input.** `loopd.sh` relaunches this spec
  back-to-back with no human in the loop — TODO.md, EVALUATION.md and
  specs/ are the handoff. Leave them such that a cold next cycle needs zero
  human words: every `todo` row has a ready spec, every deferred item says
  why, every open question is written down.

## Hard rules

- ONE child at a time, foreground, bounded gates — all of META-SPEC's hard
  rules apply (never reset/remove worktrees with unmerged work, never
  force-push, wedge protocol).
- Single-driver invariant: another **`chug run`** (autonomous driver) with
  `/Users/jadams/workspace/chug` as its cwd blocks the cycle. When tripped:
  do NO mutating work, verify the untouched tree's gates once, then
  `goal_complete` immediately with a BLOCKED report — do not watch-and-wait
  (a paced 2h watch burned 712k tokens in cycle 3; the fast exit is the
  doctrine). An interactive **`chug chat`** does NOT block: note it and
  proceed (shared `.chug/` appends interleave harmlessly; the operator is
  trusted not to run chat turns in the repo mid-cycle).
- Children never edit TODO.md or LEDGER.md in the main tree.
- README gate before `goal_complete`: it must document everything the cycle
  landed.
