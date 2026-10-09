# T266 — eval-state rewrite: structural self-check + line-anchored splices

## One concern

The wrap-time `.chug/eval-state.md` rewrite (META-META-SPEC's "Rewriting the
state at wrap" paragraph) is a hand splice with a FIELD-ONLY self-check: it
verifies the nine bare field extractions are non-empty but never the file's
STRUCTURE. The cycle-305 wrap's three-part splice anchored its health-section
replacement on the bare substring `## open-threads`, which matched INSIDE the
header's own open-threads one-liner cross-reference (`(## open-threads)`)
instead of at the section header — the rewrite left a duplicated header-tail
fragment plus the stale previous-cycle `## health` block in the file, and the
field-only self-check PASSED (first-match reads are fresh). The parser
contract (scripts/eval-delta.sh) also reads first-matches plus the decisions
ring, so the corruption is invisible to STATE-HIT verdicts and COMPOUNDS one
generation per repeated splice.

## Repo context

- Carrier: META-META-SPEC.md, the "Rewriting the state at wrap (every
  evaluation does this, hit or full)" paragraph (the T262 two-phase rewrite
  procedure: bare field block + one-liners, `## health`, `## open-threads`,
  `## pacing-streak`, `## decisions` ring).
- Evidence (the 2nd fire of the T262 state-rewrite format-drift census,
  1→2): `.chug/eval-state.md` as the cycle-305 wrap left it — the duplicated
  fragment + stale block (lines 24–36 of that file); the trip-44 stream's
  three splice tool-results (04:10:04 "field block + ring + one-liners
  rewritten", 04:10:31 "health section rewritten", 04:10:47 "open-threads +
  pacing-streak rewritten" + the field-only SELF-CHECK passing); the first
  fire's record d1791498040-2 (census 1, trigger named verbatim: "a 2nd fire
  files the wrap-checklist doctrine-clause row"); the trip-45 launch's
  STATE-HIT read (8th consecutive — zero casualty, first-match + ring reads).
- estimate: ~15 changed lines (META-META-SPEC.md only).
- The T262 two-phase rewrite STAYS (the marker + wrap-hash fill); this clause
  governs HOW the bytes are written, never what they carry.

## Requirements

1. The rewrite is EITHER a single full-file write (always correct — the
   cycle-309 wrap's healing act) OR per-section splices whose anchors are
   LINE-ANCHORED (`^## ` plus the section's own heading text, matched at
   line start) — a bare-substring anchor that can match inside another
   field's prose is BANNED (the cycle-305 `## open-threads` fire named as
   the evidence).
2. After ANY rewrite, a structural self-check runs alongside the existing
   field check: exactly one `purpose:` line, exactly one `## health` header,
   exactly one `## open-threads (` section header, exactly one `## decisions`
   ring header (`grep -c` pins), all nine `state_field` extractions
   non-empty (the existing field check, unchanged).
3. A failed structural check → the rewrite is redone as a single full-file
   write BEFORE the wrap push (a malformed state file is never pushed).
4. A wrap that FINDS the file malformed on read heals it with the full
   single write in the same act (the cycle-309 precedent) and names the
   healing in the wrap notes.

## Tests

The check below (content greps on the amended paragraph) plus the existing
META-META-SPEC pin suites, which must keep passing: tests/eval_state_delta.rs,
tests/eval_outcomes_carry.rs, tests/loop_spec_doctrine_prune.rs.

## Acceptance

META-META-SPEC.md carries the clause (all four requirements, the cycle-305
fire named); the check line passes; the three pin files plus
tests/todo_consistency.rs stay green.

check: export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-validate-a; touch src/*.rs tests/*.rs; grep -q 'structural self-check' META-META-SPEC.md && grep -q 'LINE-ANCHORED' META-META-SPEC.md && grep -q 'single full-file write' META-META-SPEC.md && cargo test --test eval_state_delta --test eval_outcomes_carry --test loop_spec_doctrine_prune --test todo_consistency
