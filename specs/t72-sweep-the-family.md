# T72 — LOOP-SPEC fix-up arc gains the sweep-the-family directive

check: cargo test --test loop_spec_sweep_family && cargo test

## Repo context

Cycle 33's T69 validation arc took THREE kimi rounds where one should have
sufficed. Round 1 (FAIL) found two vacuous-pin survivors of the run_start
latch-reset class (abort_reason render dead per suite; verdict_latch reset
vacuous — a reset whose deletion left every test green because each
post-resume fixture overwrote the latch). The fix-up pinned exactly the two
named legs. Round 2 (FAIL) found ONE more survivor of the SAME class — the
check_cmd latch reset, masked by the identical mechanism. The fix-up pinned
it. Round 3 (PASS, .chug/LEDGER-t69-validate3-20260926-134811.md) only
closed the class because its goal carried explicit sweep-the-family
guidance: all four run_start latch resets were swept, each with its own
killing test.

The structural pattern: when a validator finding names ONE instance of a
class ("this reset leg is vacuous"), a fix-up that pins only the named leg
guarantees the NEXT round finds the next instance — one round per leg, ~15
min per round. The remedy is evaluator/orchestrator-side, and it is one
sentence of doctrine at the exact dispatch point.

The doctrine surface is LOOP-SPEC.md §2 step 4's FAIL arc, which today reads
"FAIL → fix-up child with the findings pasted into its goal, then
re-validate." The pin-file precedent is tests/loop_spec_recovery.rs (T63):
a small integration file grepping LOOP-SPEC.md for the needle so a future
edit cannot silently drop the rule.

## Requirements

1. **ONE insertion in LOOP-SPEC.md §2 step 4**, at the FAIL-arc sentence
   (the final sentence of step 4: "FAIL → fix-up child with the findings
   pasted into its goal, then re-validate."). Extend it IN PLACE (no step
   renumbering — T19/T30 rule) so the fix-up directive becomes: the
   findings are pasted into the fix-up goal AND, when a finding names one
   instance of a class (a vacuous pin, a missing reset, an unchecked error
   leg), the goal ALSO names the class and requires EVERY instance swept
   with its own RED-proven killing test — the cycle-33 lesson (T69's
   run_start latch resets took three rounds one leg at a time; the
   sweep-the-family goal closed it in one). Keep the existing bytes of the
   sentence intact where possible; the T63 recovery paragraph and every
   other step-4 byte stay unchanged.
2. **Pin file `tests/loop_spec_sweep_family.rs`** (T63 precedent —
   tests/loop_spec_recovery.rs pattern): greps the repo-root LOOP-SPEC.md
   for the new directive's load-bearing needles — exactly-once counts for
   (a) a sweep-the-family phrase token, (b) the every-instance/RED-proven
   requirement token, (c) the cycle-33/T69 citation token; plus a leg
   proving the needles live INSIDE §2 step 4 (scope the search between the
   step-4 heading and the step-5 heading, heading-text matched loosely
   enough to survive wording tweaks — the T64 heading-scope pattern).
3. Nothing else changes: no other LOOP-SPEC.md byte, no other file. The
   spec itself must NOT carry its own needles unbracketed outside this
   Requirements prose (T67 self-match lesson — the check greps
   LOOP-SPEC.md, never this spec, so ordinary prose here is safe; do not
   add grep legs over specs/t72-*.md).

## Tests

- The new pin file's legs are green against the edited LOOP-SPEC.md.
- Deletion hand-check recorded in the commit message: removing the inserted
  clause turns the exactly-once legs RED (3 of 3); restoring returns green.
- `cargo test` whole-suite green; clippy clean.

## Acceptance

- `cargo build && cargo clippy --all-targets -- -D warnings && cargo test`
  green in the worktree (export
  `CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared` first).
- `check:` line passes verbatim from the worktree root.
- `git diff` = LOOP-SPEC.md (one hunk) + tests/loop_spec_sweep_family.rs
  (new). Nothing else.

## Out of scope

- Editing META-SPEC.md §6 (human file; the validator goal template is
  byte-frozen — the sweep guidance lives at the orchestrator's fix-up
  dispatch point, which is LOOP-SPEC's surface).
- Any code change.
