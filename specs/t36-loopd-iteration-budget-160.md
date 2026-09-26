# T36 — loopd cycle budget: `--max-iters 120` → `160`

check: grep -c -- '--max-iters 160' loopd.sh | grep -q '^1$' && ! grep -q -- '--max-iters 120' loopd.sh && grep -q -- '--max-minutes 240' loopd.sh && grep -q 'anthropic-system.ai.kimi-k3' loopd.sh

## One concern

The LOOP-SPEC cycle budget in `loopd.sh` is undersized for fresh-eval
cycles — again, one level up from T27. Raise the orchestrator's
iteration ceiling from 120 to 160. Nothing else changes.

## Repo context

- `loopd.sh` (repo root, 90 lines) launches each cycle with
  `--max-iters 120 --max-minutes 240` (line 72, the ONLY
  `--max-iters` site — verified by grep; line 69 carries T27's
  arithmetic comment). No test pins loopd.sh's contents.
- Evidence (cycle-16 eval Q1): BOTH 120-cap fresh-eval cycles ended
  within 4 iterations of the ceiling — cycle 13 at **116/120**
  (`.chug/events-20260926-025049.jsonl`: budget_low
  remaining_iters=8 at 02:48:11Z, goal accepted 02:49Z) and cycle 14
  at **119/120** (`.chug/events-20260926-034737.jsonl`: budget_low
  remaining_iters=8, goal accepted at iteration 119). T27 filed on
  the same pattern at the 80 cap (76/80, 79/80); the class recurred
  at the 120 cap in two data points.
- Cost model (updated for T29's wait_secs, which cut per-item poll
  overhead): fresh Phase 1 ≈45–55 iters (the §6 README audit added
  since T27's 45), one queue item end-to-end ≈28–35 (impl + polls +
  gates + validator + merge), wrap ≈8–10. Fresh eval + 3 items
  ≈ 150–165 > 120. Deferral absorbs the overflow cleanly (T35
  deferred from cycle 14 and landed first thing in cycle 15 — by
  design), but a budget_low@8 wrap squeezes Phase 3, and Phase 3 IS
  the next cycle's input (a squeezed wrap is a handoff-quality risk;
  the cycle-12 mid-arc budget death is the lesson T34 mitigated for
  narrative but not for wrap headroom).
- Minutes are never binding: cycle 14 used 56 of 240, cycle 13 ~117
  of 240; 160 iters projects to ≈80–100 min — `--max-minutes 240`
  stays.
- Precedents: T21 (impl children 40→50), T32 (validators 40→50), T27
  (orchestrator 80→120) — same failure class, each filed at two
  near-ceiling data points. This is the fourth and the arithmetic is
  identical.
- T13's budget-low warning (WARN_REMAINING_ITERS=8) and LOOP-SPEC
  §2.6's stop-dispatching rule are ceiling-relative and keep working
  unchanged at 160.

## Requirements

1. `loopd.sh` line 72: `--max-iters 120` → `--max-iters 160`. Exactly
   one `--max-iters` occurrence in the file afterwards.
2. `--max-minutes 240`, the model, the goal text, and every other
   line of loopd.sh are byte-identical.
3. Replace T27's comment line (line 69) with the updated arithmetic,
   e.g. `# cycle budget: fresh Phase 1 ≈45–55 iters + ~28–35/item + ~10 wrap (cycle-16 eval Q1); 160 fits eval + 3 items + wrap; minutes never binding (56–117 of 240)`.
4. **Do NOT signal, stop, kill, or restart the running supervisor**
   (launchd job `com.tampajohn.chug-loopd`). The edit lands in git
   only. Rationale to record in the commit message (T27's, verbatim
   logic): bash reads script files incrementally (loop bodies are
   re-sought per iteration), so a running loopd may misread an edited
   script; the blast radius is bounded (the 3-consecutive-failure
   HALT guard + single-driver guard + launchd relaunch), and the next
   supervisor start picks up 160 — the intended activation path.
   Activation timing stays a human-decision carry.

## Tests

- No cargo tests (shell-script content change; the T21/T27 precedent
  pins doctrine text via the spec check, not the suite).
- The `check:` line above pins: exactly one `--max-iters 160`, zero
  `--max-iters 120`, minutes/model intact. It is worktree-relative
  (never `cd`s to the main repo — T21/T26 lesson, T30 doctrine).
- Manual verification leg for the implementer: `bash -n loopd.sh`
  (syntax) and `git diff` showing one changed line plus one replaced
  comment line.

## Acceptance

- check passes in the worktree AND in main post-merge.
- `bash -n loopd.sh` exits 0.
- Diff vs merge-base = loopd.sh only (+2/−2 lines: one code line, one
  comment line).
- cargo build + clippy + test in the worktree stay green (nothing in
  src/ may change).
