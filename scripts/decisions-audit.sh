#!/usr/bin/env bash
# decisions-audit.sh — T199: F13 decision-corpus health summary.
#
# Prints the corpus-health summary the F13 distillation work (FEATURES.md
# phase 2a) reads before any training or export (T200): record counts by
# class; outcome records whose `choice` fell OUTSIDE the closed label set
# {landed-clean, fixed-up, reverted} — the write-time enum landed with T199,
# so everything it counts there is grandfathered history, immutable by the
# T70 append-only invariant; outcome records whose `subject` resolves to no
# record id (the exact-equality definition the write-time subject lint in
# src/decisions.rs applies, so lint and audit agree); routing/verdict
# records with NO outcome backfill naming their id (count by class); and
# duplicate ids. All five sections print even when zero, so digests and
# eyeball diffs stay shape-stable. REPORT-only by doctrine: missing
# backfills are surfaced, never gated (T199 out-of-scope).
#
# jq + bash builtins only, LC_ALL=C, one pass over the corpus (fromjson?
# drops malformed lines — a torn tail line from a killed writer degrades
# the counts by that line, it never kills the audit; same tolerance as
# eval-digest.sh). Set lookups are object-indexed, so the pass is linear:
# sub-second at 10x the live corpus size (measured: ~0.2s at 8.3k records).
#
# Usage:
#   scripts/decisions-audit.sh [CORPUS]
#     CORPUS is a decisions.jsonl path, OR a directory holding
#     .chug/decisions.jsonl; empty = this checkout's .chug/decisions.jsonl.
#     A missing or unreadable corpus reports the all-zero shape (path still
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
  echo "decisions-audit: no readable corpus at $CORPUS — reporting the empty shape" >&2
  INPUT="/dev/null"
else
  INPUT="$CORPUS"
fi

# One pass over the raw corpus -> the whole rendered report. Line-oriented
# fromjson? tolerance up front; object-indexed sets for the two joins (ids,
# backfilled subjects) so lookups stay O(1) at corpus scale.
JQ_PROG=$(cat <<'JQEOF'
(split("\n")
 | map(select(length > 0) | fromjson? | select(type == "object"))) as $r
| (reduce ($r | map(.id // ""))[] as $i ({}; .[$i] = true)) as $idset
| ([$r[] | .class // "?"] | group_by(.)
   | map({k: .[0], n: length}) | sort_by(.k)) as $byclass
| ([$r[] | .id // ""] | group_by(.)
   | map(select(length > 1) | .[0]) | sort_by(.)) as $dups
| ([$r[] | select(.class == "outcome")]) as $out
| ([$out[] | select((.choice // "") != "landed-clean"
                and (.choice // "") != "fixed-up"
                and (.choice // "") != "reverted")]) as $badchoice
| ([$out[] | select(($idset[.subject // ""] // false) | not)]) as $badsubj
| (reduce ([$out[] | .subject // ""] | unique)[] as $s ({}; .[$s] = true)) as $bfset
| (["validation-routing", "validation-verdict", "recovery-routing"] | map(
    . as $c | {k: $c, n: ([$r[] | select(.class == $c
                  and (($bfset[.id // ""] // false) | not))] | length)})) as $nobf
| "# decisions corpus audit — \($corpus)\n"
+ "records: \($r | length)\n"
+ "\n## records by class\n"
+ (if ($byclass | length) == 0 then ""
   else ($byclass | map("  - \(.k): \(.n)") | join("\n")) + "\n" end)
+ "\n## outcome choice outside closed set (landed-clean | fixed-up | reverted): \($badchoice | length)\n"
+ (if ($badchoice | length) == 0 then ""
   else ($badchoice | map(.id // "?") | sort | map("  - \(.)") | join("\n")) + "\n" end)
+ "\n## outcome subject resolves to no existing id: \($badsubj | length)\n"
+ (if ($badsubj | length) == 0 then ""
   else ($badsubj | map(.subject // "?") | sort | map("  - \(.)") | join("\n")) + "\n" end)
+ "\n## routing/verdict records without an outcome backfill naming their id\n"
+ ($nobf | map("  - \(.k): \(.n)") | join("\n")) + "\n"
+ "\n## duplicate ids: \($dups | length)"
JQEOF
)

jq -Rsr --arg corpus "$CORPUS" "$JQ_PROG" "$INPUT"
