# T246 — outcome-backfill audit at wrap: malformed-chain check + wrap audit step

check: export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-validate-a; touch src/*.rs tests/*.rs; cargo test --test decisions_audit --test decisions_export --test loop_spec_decision_records --test todo_consistency

estimate: ~80 lines (audit-script section + fixture leg + doctrine bullet + pin legs + row)

## Concern

The outcome-backfill defect class fired in BOTH of the last two wraps
(cycle-117 eval I1), caught only by the NEXT eval's manual jq spot-check:

- Cycle-115 wrap: T241's flip (085be34) claimed backfills that never
  landed — 3 missing (d1791235160-17, d1791236658-18, d1791239794-19);
  caught at the cycle-116 eval, hand-backfilled with provenance.
- Cycle-116 wrap: T245's verdict id d1791262814-2 has NO outcome record
  naming it, and the flip-time record d1791264594-4 subjects ANOTHER
  OUTCOME record (d1791262925-3) instead of a logged decision id — a
  malformed chain. Caught at the cycle-117 eval.

The silent cost: `scripts/decisions-export.sh` joins outcome labels to
NON-outcome records by subject id. An outcome subjecting an outcome never
joins (shape-harmless) AND the intended subject exports `outcome: null` —
it trains UNLABELED. Every mis-targeted or missing backfill is one fewer
label in the F13 corpus whose size is the GO precondition. The T199 audit
script checks subject-resolves-to-NO-id (`$badsubj`) but NOT
subject-resolves-to-an-OUTCOME-record, and no step runs the audit at wrap —
the net that caught both fires is unowned and unwritten.

## Repo context

- `scripts/decisions-audit.sh` (T199): REPORT-only corpus health summary;
  five shape-stable sections printed even when zero (records by class,
  out-of-set choices, unresolved subjects, unbackfilled routing/verdict
  counts, duplicate ids). jq one-pass, object-indexed sets, LC_ALL=C.
- `tests/decisions_audit.rs`: T199's golden-section pins over a fixture
  corpus — the new section's home.
- `tests/loop_spec_decision_records.rs`: the doctrine pin file for
  decision-record doctrine tokens.
- LOOP-SPEC Phase 3's decisions.jsonl bullet (the wrap checklist) names
  the record classes per cycle and the "incomplete wrap" clause; it has
  no verification step.
- T70 append-only invariant: malformed records are NEVER edited —
  fix-forward by appending the correctly-targeted outcome with a
  provenance note.

## Requirements

1. `scripts/decisions-audit.sh` gains a sixth shape-stable section:
   `## outcome subject resolves to another outcome record (malformed
   chain): N` listing the offending outcome record ids (an outcome whose
   `subject` resolves to a record whose `class` is `outcome`), printed
   even when zero, sorted, same object-indexed one-pass style.
2. LOOP-SPEC Phase 3's decisions bullet gains the wrap step: the wrap
   RUNS `scripts/decisions-audit.sh`; routing/verdict/recovery ids from
   THIS cycle without outcome backfills get outcome records appended with
   a provenance note before the wrap push; a nonzero malformed-chain
   count is named in the wrap notes and fixed forward per the T70
   append-only invariant (append the correctly-targeted outcome with
   provenance; never edit history).
3. Pin legs: (a) a `tests/decisions_audit.rs` fixture leg — an outcome
   subjecting an outcome appears in the new section (RED-provable by
   reverting the jq addition); (b) a doctrine pin leg in
   `tests/loop_spec_decision_records.rs` asserting the LOOP-SPEC bullet
   carries the audit-run token (exactly-once whitespace-collapsed, the
   T78 idiom).
4. The row's own arc dogfoods the doctrine: its wrap appends the T245
   verdict-id backfill (d1791262814-2, landed-clean) with a provenance
   note if the cycle-117 wrap has not already.

## Tests

- The new audit fixture leg RED-proven by reverting the jq addition and
  restored byte-identical.
- The doctrine pin leg RED-proven by token removal and restored.
- The check line above runs green (decisions_audit, decisions_export —
  the shared corpus-semantics neighbor — loop_spec_decision_records,
  todo_consistency).

## Out of scope

- Editing or deleting the malformed historical records (T70 append-only —
  forbidden; fix-forward only).
- Changing `decisions-export.sh`'s join semantics (an outcome-subjecting
  outcome is already shape-harmless there; the label loss is the AUDIT's
  surface to name).
- Backfilling the grandfathered unbackfilled cohort (pre-doctrine
  history; the wrap step scopes to THIS cycle's ids).
- Gating the wrap on the audit (REPORT-only stays; the step is run +
  backfill + name, never a merge gate).
