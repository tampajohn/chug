# T156 — LOOP-SPEC T63 doctrine: name the complete+committed budget-death variant

check: cd /private/tmp/chug-loop-t156 && export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared && cargo test --test loop_spec_recovery 2>&1 | tail -3

## Repo context

LOOP-SPEC §2 step 2's **Budget-death recovery (T63)** paragraph routes a
budget-aborted child to ONE `resume: true` relaunch "with the goal not
accepted and the worktree holding incomplete work", and names the
standing recipes only as fallbacks for "complete-but-uncommitted work
(T55 precedent)" or "next-cycle recovery (T28 precedent)". Cycle 71
practiced a THIRD variant the paragraph does not name: T150-impl died on
the 35-minute MINUTES budget at 48/80 with the work **complete AND
committed** (3f5a99a) but the goal unaccepted — the orchestrator applied
orchestrator-finish (review + gates + merge, NO resume burned; routing
d1790691515-11) and the cycle-71 wrap ledger carried: "T150's
T63-variant (committed-then-timeout) may deserve a doctrine sentence
alongside the resume recipe." A resume for a fully-committed child is
waste (the resumed child re-verifies work already committed); a
doctrine that only names "uncommitted" for the finish recipe invites the
wrong first recovery.

This row adds ONE clarifying sentence (plus pin alignment) naming the
three-way routing. It is a DOCTRINE row: it runs ALONE (no overlap
ever), kimi validation REQUIRED (LOOP-SPEC §2 step 4's REQUIRED list:
"the loop/spec doctrine itself").

estimate: ~40 changed lines (doctrine ~10, pin alignment ~30)

## Requirements

1. **One sentence in the T63 paragraph** (LOOP-SPEC §2 step 2), placed
   immediately after the resume-first rule, naming the three-way
   routing with its discriminating test: (i) work INCOMPLETE (uncommitted
   or partial) → ONE resume relaunch (the standing rule, unchanged);
   (ii) work COMPLETE + COMMITTED but goal unaccepted →
   orchestrator-finish directly (review the branch, run the gates,
   merge if green — NO resume burned), with T150-impl (cycle 71,
   d1790691515-11) named as the precedent alongside T55; (iii)
   resume-exhausted or unrecoverable → next-cycle recovery with a
   recipe on the row (T28 precedent, unchanged). The sentence must not
   loosen the ONE-resume cap or the never-remove-unmerged-work rule.
2. **Pin alignment.** The T63 paragraph is pinned in
   tests/loop_spec_recovery.rs (verify at implementation time; it pins
   the resume-first rule's tokens). Add/adjust the pin so the new
   routing sentence's stable tokens (e.g. "complete and committed",
   "orchestrator-finish", the T150 precedent token) are asserted
   verbatim, and RED-prove each new pin leg against the pre-edit text
   (text-revert mutant dies; byte-restore verified).
3. **No other doctrine drift.** Nothing else in LOOP-SPEC.md changes;
   META-SPEC.md is untouched (the T63 paragraph lives only in
   LOOP-SPEC).

## Acceptance

- `cargo build`, `cargo clippy --all-targets -- -D warnings`, `cargo test`
  all green (the doctrine pins are tests/ integration binaries — plain
  `cargo test` per the META-META check-breadth rule).
- RED proofs per pin leg in the commit message.
- The paragraph's existing pins (resume-first, ONE-cap, T55/T28
  precedent tokens) still green unchanged.

## Out of scope

- Changing the routing itself (the sentence documents practiced,
  validated behavior — it does not invent a new recipe).
- minutes-budget raises for impl children (rejected at the cycle-72
  eval: ONE minutes-death in the era; T151 attacks the wall-clock sink).
- Loopd-side recovery automation (the routing is orchestrator judgment
  + decision_log records, by design).
