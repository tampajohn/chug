#!/usr/bin/env bash
# decisions-export.sh — T200: F13 distillation export (phase 2a-ii).
#
# Joins each NON-outcome decision record to its outcome label and emits the
# training JSONL the F13 fine-tune (phase 2b, endpoint-side) will ingest and
# an eval can eyeball to see what the corpus actually teaches — until now the
# join was hand-jq, re-derived per use (the pre-T69 archaeology class, one
# level up). One JSON object per non-outcome record, in file order:
#
#   {"id":...,"ts":...,"class":...,"subject":...,"inputs":...,"options":...,
#    "choice":...,"confidence":...,"outcome":{"choice":...,"ts":...}|null}
#
# `outcome` is null when no outcome record names the id, else the label
# inlined from the FIRST outcome record in file order naming it (the corpus
# is append-only, so file order is chronology; when two outcomes name one id
# the earliest recorded verdict labels the row — later corrections stay in
# the corpus and, if prose, still count in K below). Outcome-class records
# appear ONLY as labels, never as rows. Grandfathered history is never
# silently dropped: an outcome whose `choice` fell OUTSIDE the closed set
# {landed-clean, fixed-up, reverted} (the write-time enum landed with T199 —
# everything it counts is grandfathered, immutable by the T70 append-only
# invariant) is inlined verbatim AND counted on the stderr summary line
#
#   export: N rows, M labeled, K grandfathered choice-violations
#
# where N = rows emitted, M = rows carrying a label, K = ALL outcome records
# with an out-of-set choice — including ones whose subject resolves to no id
# (those never reach a row, so the count is the only place they surface;
# same count as decisions-audit.sh's violation section, so the two agree).
#
# Deterministic (LC_ALL=C, no wall-clock fields, no sorting — rows keep file
# order) and a PURE FILTER: it never writes back to the corpus. jq only, BSD
# GNU clean, two single-pass reads (rows, then the summary counts) —
# sub-second at 10x corpus size. Malformed lines (a torn tail from a killed
# writer) drop with the same fromjson? tolerance as decisions-audit.sh and
# eval-digest.sh — they degrade the counts by that line, never kill the run.
#
# Usage:
#   scripts/decisions-export.sh [CORPUS]
#     CORPUS is a decisions.jsonl path, OR a directory holding
#     .chug/decisions.jsonl; empty = this checkout's .chug/decisions.jsonl.
#     A missing or unreadable corpus exports the empty shape (path still
#     named on stderr) — a fresh worktree is empty, not broken.
set -u
export LC_ALL=C

CORPUS="${1:-}"
if [ -z "$CORPUS" ]; then
  CORPUS="$(cd "$(dirname "$0")/.." && pwd)/.chug/decisions.jsonl"
elif [ -d "$CORPUS" ]; then
  CORPUS="$CORPUS/.chug/decisions.jsonl"
fi
if [ ! -f "$CORPUS" ] || [ ! -r "$CORPUS" ]; then
  echo "decisions-export: no readable corpus at $CORPUS — exporting the empty shape" >&2
  INPUT="/dev/null"
else
  INPUT="$CORPUS"
fi

# Pass 1 — the rows. Same corpus-parse tolerance as the audit; the label map
# is object-indexed (O(1) lookups, `//=` keeps the first outcome in file
# order) so the join stays linear; the row literal pins the documented field
# order (T70 shape + the joined outcome last).
JQ_ROWS=$(cat <<'JQEOF'
(split("\n")
 | map(select(length > 0) | fromjson? | select(type == "object"))) as $r
| ([$r[] | select(.class == "outcome")]) as $out
| (reduce $out[] as $o
    ({}; .[($o.subject // "")] //= {choice: $o.choice, ts: $o.ts})) as $labels
| $r[]
| select(.class != "outcome")
| {id: .id,
   ts: .ts,
   class: .class,
   subject: .subject,
   inputs: .inputs,
   options: .options,
   choice: .choice,
   confidence: .confidence,
   outcome: ($labels[.id // ""] // null)}
JQEOF
)
jq -Rsc "$JQ_ROWS" "$INPUT"

# Pass 2 — the summary counts, over the same corpus read: N non-outcome
# records (rows emitted), M of them carrying a label (the audit's own
# object-indexed backfill-set join, so export and audit agree by
# construction), K outcome records with an out-of-set choice (the audit's
# violation definition, verbatim).
JQ_COUNTS=$(cat <<'JQEOF'
(split("\n")
 | map(select(length > 0) | fromjson? | select(type == "object"))) as $r
| ([$r[] | select(.class == "outcome")]) as $out
| ([$out[]
    | select((.choice // "") != "landed-clean"
             and (.choice // "") != "fixed-up"
             and (.choice // "") != "reverted")]
   | length) as $k
| (reduce $out[] as $o ({}; .[($o.subject // "")] = true)) as $bfset
| ([$r[] | select(.class != "outcome")]) as $nr
| "\($nr | length) \([$nr[] | select(($bfset[.id // ""] // false))] | length) \($k)"
JQEOF
)
COUNTS=$(jq -Rsr "$JQ_COUNTS" "$INPUT")
# shellcheck disable=SC2086 # the counts line is exactly three integers
read -r N_ROWS N_LABELED N_BAD <<EOF
$COUNTS
EOF
printf 'export: %s rows, %s labeled, %s grandfathered choice-violations\n' \
  "$N_ROWS" "$N_LABELED" "$N_BAD" >&2
