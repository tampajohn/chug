# T199 — Decision-corpus integrity: outcome-choice enum at write time + corpus audit script (F13 phase 2a-i)

check: export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared && cargo test

## Repo context

FEATURES.md F13 (decision logs → Laya distillation) split at the
cycle-34 eval: phase 1 (the `decision_log` tool + `.chug/decisions.jsonl`
+ LOOP-SPEC adoption) LANDED as T70; phases 2–3 deferred "pending
corpus + layad endpoint". The corpus half of that deferral is now
SATISFIED — `.chug/decisions.jsonl` holds 791 records (237
eval-triage, 132 validation-routing, 123 validation-verdict, 92
recovery-routing, 200 outcome backfills: 153 landed-clean, 40
fixed-up). The layad FINE-TUNE endpoint remains absent, so phase 2
splits again: this row is the in-repo corpus-integrity half (2a-i);
the export half (2a-ii) is T200; the fine-tune + confidence-gated
routing half (2b) stays endpoint-blocked on FEATURES.md.

The corpus is NOT distillation-clean today — measured this eval:
**7 outcome records carry free-text prose in `choice`** (e.g.
"landed-clean — merge 7b9b2bf (keep-both conflict resolution) +
flip…", "landed-clean — correction record: the T152 outcome belongs
to…"), polluting the closed label set {landed-clean, fixed-up,
reverted} the F13 classifier trains against, and one correction
record exists precisely because an outcome's `subject` misattributed
its decision id. `decision_log` validates required fields but accepts
any string for `choice` on every class.

estimate: ~260 changed lines (enum enforcement ~80 src + tests,
audit script ~120 + golden pins ~60).

## Requirements

1. **Write-time enum (src/decisions.rs):** when `class == "outcome"`,
   `choice` MUST be one of `landed-clean` / `fixed-up` / `reverted` —
   anything else is a tool error naming the closed set (the existing
   invalid-call error shape), never a silent clamp. Provenance text
   belongs in `inputs` (the error message says so). Other classes'
   `choice` stays free-string (the seed classes' options are
   per-decision).
2. **Subject lint (best-effort, advisory):** when `class ==
   "outcome"` and the corpus file is readable, a `subject` that
   matches NO existing record id in the file appends a
   `note: subject <id> not found in <path>` line to the tool's return
   text — never an error (append-only best-effort is the T70
   invariant; an unreadable file skips the check silently). The scan
   is bounded (the file is line-oriented; a first-match substring
   search per line is fine at corpus scale, documented in the code).
3. **Audit script `scripts/decisions-audit.sh`** (jq, LC_ALL=C,
   BSD/GNU clean, sub-second at 10x corpus size): prints the F13
   corpus-health summary — record counts by class; outcome records
   with choice OUTSIDE the closed set (count + ids); outcome records
   whose subject resolves to no id (count + ids); routing/verdict
   records with NO outcome backfill naming their id (count by class);
   duplicate ids (count). All five sections print even when zero
   (shape-stable for the digest and for eyeball diffing).
4. **Corpus history is immutable** — the 7 polluted records are NOT
   rewritten (append-only provenance; the audit's grandfather count
   is expected to be 7 at landing, asserted in the golden test's
   fixture leg, not against the live corpus).
5. README's `decision_log` paragraph gains one clause: outcome
   records' `choice` is a closed set enforced at write time, and
   `scripts/decisions-audit.sh` prints the corpus-health summary.

## Tests

- Enum legs: outcome+bad-choice → error naming the set; the three
  valid choices pass; non-outcome classes keep free choice; the error
  names `inputs` as the provenance home.
- Subject-lint legs: known id → no note; unknown id → note in return
  text; unreadable corpus → launch/write proceeds, no note.
- `tests/decisions_audit.rs` golden-section pins over a FIXTURE
  corpus (T62 harness pattern): section heading order, field labels,
  a 7-violation fixture rendering its count, a clean fixture
  rendering zeros; volatile values pinned by shape.
- Non-vacuousness: removing the enum check turns ≥2 legs RED, stated
  in the commit message.

## Acceptance

- Check line green; `cargo clippy --all-targets -- -D warnings`
  green.
- `scripts/decisions-audit.sh` run against the LIVE corpus at review:
  choice-violation count = 7 (the known grandfathered set), zero
  duplicate ids.

## Out of scope

- The export/join script (T200) and everything endpoint-side
  (F13-2b).
- Backfill-completion enforcement (the audit REPORTS missing
  backfills; it does not gate).
