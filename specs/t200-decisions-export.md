# T200 — Distillation export: decisions.jsonl → joined training JSONL (F13 phase 2a-ii)

check: touch src/*.rs tests/*.rs; export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared && cargo test --test decisions_export

## Repo context

Sequences after T199 (F13 phase 2a-i: write-time outcome-choice enum
+ the audit script). This row is the export half of the
distillation-readiness pull: with the corpus hygienic (T199) and
791 records on disk, the missing piece is a deterministic,
operator-consumable export that joins each decision to its outcome
label — the artifact a future Laya fine-tune ingests, and the thing
an eval can eyeball to see what the corpus actually teaches. Today
the join is hand-jq, re-derived per use (the pre-T69 archaeology
class, one level up).

estimate: ~200 changed lines (script ~130 + golden pins ~70).

## Requirements

1. **`scripts/decisions-export.sh`** (jq, LC_ALL=C, BSD/GNU clean,
   sub-second at 10x corpus size): reads `.chug/decisions.jsonl`
   (path overridable via `$1`, default `.chug/decisions.jsonl`) and
   emits one JSON object per NON-outcome record, in file order:
   `{id, ts, class, subject, inputs, options, choice, confidence,
   outcome}` where `outcome` is `null` when no outcome record names
   the id, else `{"choice": <closed-set choice>, "ts": <ts>}`.
   Outcome-class records appear ONLY as labels (never as training
   rows). Records whose `choice` (outcome side) is outside the
   closed set are passed through verbatim AND counted on a stderr
   summary line (`export: N rows, M labeled, K grandfathered
   choice-violations`) — the export never silently drops history.
2. Deterministic: byte-identical output on repeated runs over the
   same input (no wall-clock fields, no locale-dependent ordering).
3. The script is a pure filter: it never writes back to the corpus.
4. README's `decision_log` paragraph (or Self-hosting specs section —
   child's call, named in the commit) gains one sentence naming the
   export and its output shape.

## Tests

- `tests/decisions_export.rs` golden pins over fixture corpora (the
  T62 run-digest harness pattern): (a) join correctness — a routing
  record + its outcome → one row with the label inlined; (b)
  unlabeled records → `outcome: null`; (c) outcome records never
  appear as rows; (d) grandfathered free-text choice passes through
  AND increments the stderr count; (e) determinism — two runs
  byte-identical; (f) field order/shape of the output object (golden
  section, T62-style shape pinning).
- Non-vacuousness: dropping the join (all outcomes null) turns ≥2
  legs RED, stated in the commit message.

## Acceptance

- Check line green; clippy green (trivially — scripts + tests).
- A live run at review: `scripts/decisions-export.sh | jq -c
  'select(.outcome != null)' | wc -l` lands within ±5 of the audit's
  labeled count (T199's script) — the two agree, evidence in the
  commit message.

## Out of scope

- Any training-side format opinion beyond the self-describing row
  shape (no prompt/completion templating — that is F13-2b's call,
  endpoint-side).
- Chunking/privacy passes (the corpus is local-only).
