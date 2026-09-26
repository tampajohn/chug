# T75 — LOOP-SPEC wrap checklist gains the decision-records sentence (zero-call adoption gap)

check: cargo test --test loop_spec_decision_records && cargo test

## Repo context

T70 (merge 572ec5a, cycle 35) landed the `decision_log` tool +
`.chug/decisions.jsonl` AND wired LOOP-SPEC adoption at four named points:
Phase-1 `eval-triage` (filed rows AND rejected candidates), step-2
`recovery-routing` / `model-fallback`, step-4 `validation-routing` /
`validation-verdict`, step-5 `outcome` backfills. F13 phase 2's entire
premise (confidence-gated routing distilled from the corpus) depends on
those records EXISTING.

**The gap:** two cycles since landing, zero organic calls. The cycle-36
eval's digest read shows cycle 34's orchestrator stream (eval + dispatch:
bash 77, delegate 28, read_file 13, write_file 6, update_ledger 4,
edit_file 3) and cycle 35's (3 items landed incl. 2 resume recoveries +
3 validation verdicts: bash 80, delegate 43, update_ledger 5, edit_file 4,
read_file 2, goal_complete 1) — NO `decision_log` in either tool
distribution, and `.chug/decisions.jsonl` does not exist on disk. The
adoption sentences are present in LOOP-SPEC but nothing in the arc ASKS
for the records at the moment of accounting. This is the T23→T24
zero-calls lesson repeating: delegate landed T23 and sat uncalled until
T24 wired it into LOOP-SPEC's workflow; decision_log has the named-point
sentences but lacks the wrap-time check that makes a skipped record
VISIBLE.

The remedy is one sentence at the accounting surface: Phase 3's wrap
checklist. The pin-file precedent is tests/loop_spec_sweep_family.rs
(T72) / tests/loop_spec_recovery.rs (T63): a small integration file
grepping LOOP-SPEC.md for the needles so a future edit cannot silently
drop the rule.

## Requirements

1. **ONE insertion in LOOP-SPEC.md's Phase 3 (Wrap) section**, at the
   checklist — in place, no renumbering (T19/T30 rule). The new checklist
   line requires: `.chug/decisions.jsonl` carries the cycle's records —
   `eval-triage` at eval time (filed rows AND rejected candidates),
   `recovery-routing` / `model-fallback` at dispatch, `validation-routing`
   + `validation-verdict` per item, `outcome` backfills at row flips — and
   states that a cycle which worked items with zero `decision_log`
   records is an incomplete wrap (the T23→T24 zero-calls lesson; cycle
   34+35 shipped nine routing/verdict decisions with none recorded).
   Keep every other Phase-3 byte unchanged.
2. **Pin file `tests/loop_spec_decision_records.rs`** (T63/T64/T72
   precedent): greps the repo-root LOOP-SPEC.md for the new line's
   load-bearing needles — exactly-once counts for (a) a
   decisions.jsonl-carries-records token, (b) an incomplete-wrap token,
   (c) the zero-calls/T23→T24 citation token — plus a leg proving the
   needles live INSIDE the Phase-3 wrap window (scope between the
   Phase-3 heading and the Hard-rules heading, heading text matched
   loosely enough to survive wording tweaks — the T64 heading-scope
   pattern).
3. Nothing else changes: no other LOOP-SPEC.md byte, no other file. Do
   not add grep legs over specs/t75-*.md (the check greps LOOP-SPEC.md
   only — T67 self-match lesson).

## Tests

- The new pin file's legs green against the edited LOOP-SPEC.md.
- Deletion hand-check recorded in the commit message: removing the
  inserted line turns the exactly-once legs RED; restoring returns green.
- `cargo test` whole-suite green; clippy clean.

## Acceptance

- `cargo build && cargo clippy --all-targets -- -D warnings && cargo test`
  green in the worktree (export
  `CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared` first).
- `check:` line passes verbatim from the worktree root.
- `git diff` = LOOP-SPEC.md (one hunk) +
  tests/loop_spec_decision_records.rs (new). Nothing else.

## Out of scope

- A mechanical guard that VERIFIES records exist per cycle (a test cannot
  observe cycle behavior; the wrap checklist is the accounting surface).
  If cycles continue shipping zero-record arcs after this sentence lands,
  the NEXT eval weighs a stronger mechanism.
- Editing META-SPEC.md or the decision_log tool itself.
- Validation routing: doctrine row — LOOP-SPEC §2 step 4 REQUIRED kimi,
  and doctrine items run alone (no T44 overlap).
