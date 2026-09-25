# T27 — loopd cycle budget: `--max-iters 80` → `120`

check: grep -c -- '--max-iters 120' loopd.sh | grep -q '^1$' && ! grep -q -- '--max-iters 80' loopd.sh && grep -q -- '--max-minutes 240' loopd.sh && grep -q 'anthropic-system.ai.kimi-k3' loopd.sh

## One concern

The LOOP-SPEC cycle budget in `loopd.sh` is undersized for fresh-eval
cycles. Raise the orchestrator's iteration ceiling from 80 to 120.
Nothing else changes.

## Repo context

- `loopd.sh` (repo root, 86 lines) is the unattended supervisor: it
  builds chug, then launches each LOOP-SPEC cycle with
  `./target/debug/chug run --spec LOOP-SPEC.md --goal … --model
  anthropic-system.ai.kimi-k3 --max-iters 80 --max-minutes 240`
  (line 71, the ONLY `--max-iters` site in the file — verified by
  grep). No test pins loopd.sh's contents.
- Evidence (cycle-11 eval O1): the two measured fresh-eval cycles both
  ended within 4 iterations of the 80 ceiling — cycle 7 at 76/80,
  cycle 9 at **79/80** (`.chug/events-20260925-211633.jsonl`:
  budget_low remaining_iters=8 at 21:14:00, goal 21:15:31). Cycle 9's
  budget-low forced a directive wrap with T24's validation deferred to
  a whole second cycle. Freshness-skip cycles used 41 and 68. Phase 1
  alone costs ~45 iterations (eval commit at tool idx 47/91); one
  queue item end-to-end costs ~30–35. Minutes are never binding
  (cycles run 24–35 of 240).
- Precedent: T21 raised the impl-child template 40→50 for the same
  failure class (iteration ceiling binding while minutes are not).
- T13's budget-low warning (WARN_REMAINING_ITERS=8) and LOOP-SPEC
  §2.6's stop-dispatching rule are ceiling-relative and keep working
  unchanged at 120.

## Requirements

1. `loopd.sh` line 71: `--max-iters 80` → `--max-iters 120`. Exactly
   one `--max-iters` occurrence in the file afterwards.
2. `--max-minutes 240`, the model, the goal text, and every other line
   of loopd.sh are byte-identical.
3. Add one comment line directly above the launch block recording the
   arithmetic, e.g. `# cycle budget: fresh Phase 1 ≈45 iters + ~30–35/item (cycle-11 eval O1); 120 fits eval + 2 items + wrap`.
4. **Do NOT signal, stop, kill, or restart the running supervisor**
   (launchd job `com.tampajohn.chug-loopd`). The edit lands in git
   only. Rationale to record in the commit message: bash reads script
   files incrementally (loop bodies are re-sought per iteration), so a
   running loopd may misread an edited script; the blast radius is
   bounded (the 3-consecutive-failure HALT guard + single-driver guard
   + launchd relaunch), and the next supervisor start picks up 120 —
   which is the intended activation path. Activation timing is a
   human-decision carry (EVALUATION.md §6 item 8).

## Tests

- No cargo tests (shell-script content change; the T21 precedent pins
  doctrine text via the spec check, not the suite).
- The `check:` line above pins: exactly one `--max-iters 120`, zero
  `--max-iters 80`, minutes/model intact. It is worktree-relative
  (never `cd`s to the main repo — T21/T26 lesson).
- Manual verification leg for the implementer: `bash -n loopd.sh`
  (syntax) and `git diff` showing exactly one changed line plus one
  added comment line.

## Acceptance

- check passes in the worktree AND in main post-merge.
- `bash -n loopd.sh` exits 0.
- Diff vs merge-base = loopd.sh only (+2/−1 lines).
- cargo build + clippy + test in the worktree stay green (nothing in
  src/ may change).
