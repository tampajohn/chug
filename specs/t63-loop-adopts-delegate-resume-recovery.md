# T63 — LOOP-SPEC adopts `delegate resume:true` as the standard child budget-death recovery

One concern: the delegation surface has an in-tool recovery primitive for the
loop's most common child failure — budget death with incomplete work — and the
doctrine never names it.

## Repo context

- T58 (merge `5145536`, this cycle-triple) shipped `delegate` launch's optional
  `resume: true` (appends `--resume` to the child argv; absent/false
  byte-identical, pinned) AND fixed `summarize_events` to reset the verdict
  latches at each new `run_start`, so `delegate status` describes the child's
  LATEST run segment after a resume (the cycle-18 bite: a healthy resumed
  validator reported `state: aborted` from the pre-resume segment).
- The demand class is the loop's most common child failure: FIVE children have
  died at the iteration ceiling with work complete-but-uncommitted or
  unwrapped — t15/t17/t20 at 40/40 (which drove T21 widening the template to
  50), t28, t47, t55, and now t58 at 50/50 (the T58 TODO row itself records
  "fifth occurrence of that pattern and the exact failure this feature
  addresses"). t58's death cost a cross-cycle deferral (T58+T62 wrapped
  in-flight at cycle 27, landed cycle 28) — zero work lost (T19 harvest
  discipline + recovery recipes), but a full cycle boundary of latency.
- LOOP-SPEC §2 step 2 today names only implicit recoveries: orchestrator-finish
  (T55 precedent: the orchestrator completes the child's remaining fixes and
  commits) and next-cycle recovery (T28 precedent: preserve the worktree, write
  a recipe on the row). Cycle 18's actual recovery was a hand-rolled bash
  `chug run --resume` under nohup — the exact launch pattern T24 replaced —
  because `delegate` had no resume leg. Now it does.
- `delegate`'s tool description already documents resume (src/tools.rs:156);
  the README delegate paragraph documents it. The missing surface is the
  DOCTRINE: an orchestrator mid-arc reads LOOP-SPEC, not the tool schema
  history.

## Requirements

1. **LOOP-SPEC §2 step 2 gains a budget-death recovery leg**, inserted in the
   polling paragraph directly after "Exit of the pid = child done; then
   review." (before the hand-rolled-nohup fallback sentences), saying in
   substance: when the child exited on a BUDGET abort (iteration / token /
   minutes ceiling — the events summary's abort flag names the budget) with
   the goal not accepted and the worktree holding incomplete work, the FIRST
   recovery is one `delegate` relaunch in the SAME worktree with
   `resume: true` — same spec, same goal (the goal re-carries the T47
   `CARGO_TARGET_DIR` export), same model, same budgets (50/35 impl,
   50/30 validate) — which continues the child's prior transcript in that
   worktree instead of starting cold.
2. **The leg names its mechanics and its cap**: resume works because the
   worktree is never removed pre-harvest (T19) so the child's untracked
   `.chug/` transcript persists; `delegate status` reads the LATEST run
   segment (T58), so the pre-resume abort no longer latches the summary; cap
   ONE resume attempt per child — a resumed child that dies at budget again
   without goal acceptance falls back to the standing recipes
   (orchestrator-finish for complete-but-uncommitted work, T55 precedent;
   next-cycle recovery with a recipe written on the row, T28 precedent).
3. **Scope guards**: fix-up children (step 4's FAIL arc) are unaffected —
   they start fresh by design. The launch template, budgets, model routing,
   polling cadence, and the hand-rolled-nohup tool-failure fallback stay
   byte-identical. META-SPEC.md untouched. Step numbering unchanged (the leg
   folds into step 2, preserving t13's §2.5 reference resolution).
4. **Pins**: a new `tests/loop_spec_recovery.rs` (precedent: T48's
   `no_compile_time_manifest_dir.rs` — a new small doctrine-pin file) asserts
   the recovery leg exists: (a) a needle from the leg ("resume" + the
   one-attempt cap language) occurs in LOOP-SPEC.md exactly once; (b) the leg
   sits inside step 2 (after "Exit of the pid = child done" and before step
   3's heading); (c) the pre-existing nohup-fallback sentence is still
   present (byte-identical) — the resume leg supplements, not replaces, the
   tool-failure fallback.
5. Non-vacuousness hand-check (record in the commit message): delete the leg
   then pins go red; restore then green.

## Tests

The new `tests/loop_spec_recovery.rs` (3–4 pins, all reading the worktree's
LOOP-SPEC.md at runtime via `current_dir`, never
`env!("CARGO_MANIFEST_DIR")` — T48 doctrine). No production code changes.
Full suite + clippy green.

## Acceptance

- LOOP-SPEC.md step 2 carries the recovery leg per reqs 1–3; diff is
  doctrine-only (+ the new pin file).
- Pins green; non-vacuousness shown; `cargo test` + clippy green.
- Adversarial validation REQUIRED (loop doctrine — LOOP-SPEC §2 step 4).
- This item is DOCTRINE: it runs alone, no T44 overlap (LOOP-SPEC hard
  rules).

check: cargo test --test loop_spec_recovery && grep -c "resume" LOOP-SPEC.md
