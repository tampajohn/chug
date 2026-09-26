#!/usr/bin/env bash
# eval-digest.sh — T46: mechanical pre-digest of the Phase-1 evaluation corpus.
#
# Scans <root>/.chug/events*.jsonl and writes <root>/.chug/eval-digest.md:
# per events file — iterations, wall time, tool distribution, error classes
# w/ counts, token totals + late-cycle input-token curve, aborts w/ reasons,
# budget_low fires; plus TODO status counts and days since the last
# EVALUATION.md write. Deterministic (LC_ALL=C, jq + awk only), no LLM calls,
# sub-second. loopd.sh runs it before every cycle so the evaluator reads ONE
# digest instead of doing ad-hoc jq ETL over raw archives.
#
# Staleness (T46 req 4): the digest records generated-at and the newest
# events mtime, flags events that moved DURING generation (write race), and
# carries the reader-side check command (find -newer) that detects a digest
# that has gone stale since it was written.
#
# Usage:
#   scripts/eval-digest.sh [ROOT]     # default ROOT: the repo (script/..)
# Env:
#   CHUG_DIGEST_NOW=<iso>  pin the generated-at clock (reproducible output;
#                          used by tests/eval_digest.rs)
#
# Output shape (one section per events file, then corpus + staleness):
#   ### events-t44-impl-20260925-170831.jsonl
#   - runs: 1 | model: ... | spec: ... | iters-ceil: 50
#   - iterations: 48 (last n=48) | verifying calls: 6
#   - wall: 2123s (35m23s) | 2026-09-25T17:08:31Z -> 17:43:54Z
#   - tokens (cumulative at last iteration): 169.2k in / 28.6k out
#   - input context curve (cumulative, iter quartiles): 2.7k@1 -> ... -> 169.2k@48
#   - tools: bash 41, read_file 22, ...
#   - failed tool results: 7 (+ normalized first-line error classes)
#   - goal: accepted 1 / rejected 0
#   - aborts: ... (reason x count, model, budget label)
#   - budget_low fires: 1 | output_truncated: 0
set -u
export LC_ALL=C

ROOT="${1:-}"
if [ -z "$ROOT" ]; then
  ROOT="$(cd "$(dirname "$0")/.." && pwd)"
fi
if ! ROOT="$(cd "$ROOT" 2>/dev/null && pwd)"; then
  echo "eval-digest: not a directory: ${1:-<empty>}" >&2
  exit 2
fi
CHUG="$ROOT/.chug"
OUT="$CHUG/eval-digest.md"
mkdir -p "$CHUG"

# --- clock (pinnable for reproducible output in tests) ---------------------
now_iso() {
  if [ -n "${CHUG_DIGEST_NOW:-}" ]; then printf '%s' "$CHUG_DIGEST_NOW"; return; fi
  date -u +%Y-%m-%dT%H:%M:%SZ
}
# RFC3339 (fractional secs optional) -> epoch secs; "" when unparsable.
iso_to_epoch() {
  [ -n "${1:-}" ] || return 0
  jq -rn --arg s "$1" '($s | sub("\\.[0-9]+Z$";"Z") | (try fromdateiso8601 catch empty)) // empty'
}
# file -> mtime epoch secs; 0 when unreadable (BSD stat first, GNU fallback).
mtime_of() {
  local m
  m=$(stat -f %m "$1" 2>/dev/null) || m=$(stat -c %Y "$1" 2>/dev/null) || m=0
  printf '%s' "${m:-0}"
}
# epoch secs -> UTC iso (BSD date -r first, GNU date -d @ fallback).
epoch_to_iso() {
  local e="$1"
  if date -u -r 0 +%s >/dev/null 2>&1; then
    date -u -r "$e" +%Y-%m-%dT%H:%M:%SZ 2>/dev/null || printf 'unknown'
  else
    date -u -d "@$e" +%Y-%m-%dT%H:%M:%SZ 2>/dev/null || printf 'unknown'
  fi
}
# seconds -> "35m23s" / "1h12m" / "45s"
human_dur() {
  local s="$1"
  if [ "$s" -ge 3600 ]; then printf '%dh%dm' $((s / 3600)) $(((s % 3600) / 60))
  elif [ "$s" -ge 60 ]; then printf '%dm%ds' $((s / 60)) $((s % 60))
  else printf '%ss' "$s"; fi
}

# newest events archive: "<mtime-epoch>\t<path>" ("" when none exist)
newest_events() {
  local best=0 bestf="" f m
  for f in "$CHUG"/events*.jsonl; do
    [ -e "$f" ] || continue
    m=$(mtime_of "$f")
    if [ "$m" -gt "$best" ]; then best=$m; bestf=$f; fi
  done
  if [ -n "$bestf" ]; then printf '%s\t%s' "$best" "$bestf"; fi
  return 0
}

# --- staleness pre-scan: newest mtime BEFORE the digests are computed ------
read -r PRE_NEWEST_MTIME PRE_NEWEST_PATH < <(newest_events | tr '\t' ' ' | awk '{print $1, $2}')
: "${PRE_NEWEST_MTIME:=0}"
PRE_NEWEST_ISO=$(epoch_to_iso "$PRE_NEWEST_MTIME")
GEN_ISO=$(now_iso)
GEN_EPOCH=$(iso_to_epoch "$GEN_ISO")
[ -n "$GEN_EPOCH" ] || GEN_EPOCH=0

# --- per-file corpus digest -------------------------------------------------
FILES=()
for f in "$CHUG"/events*.jsonl; do
  [ -e "$f" ] || continue
  FILES+=("$f")
done

# --- global corpus inputs: TODO status counts + EVALUATION.md age -----------
TODO_FILE="$ROOT/TODO.md"
EVAL_FILE="$ROOT/EVALUATION.md"
TODO_COUNTS="n/a (TODO.md missing)"
if [ -f "$TODO_FILE" ]; then
  TODO_COUNTS=$(awk -F'|' '
    /^\|/ {
      s = $6
      gsub(/^[ \t]+|[ \t]+$/, "", s)
      if (s == "todo" || s == "in-progress" || s == "blocked" || s == "done") c[s]++
    }
    END {
      printf "todo=%d in-progress=%d blocked=%d done=%d",
        c["todo"] + 0, c["in-progress"] + 0, c["blocked"] + 0, c["done"] + 0
    }' "$TODO_FILE")
fi
EVAL_LINE="EVALUATION.md missing (never evaluated)"
if [ -f "$EVAL_FILE" ]; then
  EVAL_MTIME=$(mtime_of "$EVAL_FILE")
  EVAL_AGE=$((GEN_EPOCH - EVAL_MTIME))
  [ "$EVAL_AGE" -lt 0 ] && EVAL_AGE=0
  EVAL_DAYS=$(awk -v a="$EVAL_AGE" 'BEGIN { printf "%.1f", a / 86400 }')
  EVAL_LINE="days since last EVALUATION.md write: ${EVAL_DAYS} (${EVAL_AGE}s; mtime $(epoch_to_iso "$EVAL_MTIME"))"
fi

# jq program: one pass over a raw-line events file -> one markdown block.
# fromjson? silently drops malformed lines; every count below is derived from
# the same parsed stream, so it always agrees with
#   jq -Rr 'fromjson? | select(.type=="iteration") | 1' FILE | wc -l
JQ_PROG=$(cat <<'JQEOF'
def num($n):
  if $n >= 1000000 then (((($n / 1000000) * 10 | round) / 10) | tostring) + "M"
  elif $n >= 1000 then (((($n / 1000) * 10 | round) / 10) | tostring) + "k"
  else ($n | tostring) end;
def dur($s):
  if $s >= 3600 then "\(($s / 3600) | floor)h\((($s % 3600) / 60) | floor)m"
  elif $s >= 60 then "\(($s / 60) | floor)m\($s % 60)s"
  else "\($s)s" end;
def ep($s):
  (if ($s | type) == "string"
   then ($s | sub("\\.[0-9]+Z$"; "Z") | (try fromdateiso8601 catch null))
   else null end);
def cls($s):
  ($s // "" | split("\n")[0] | gsub("[0-9]+"; "N")
   | if length > 110 then .[0:110] + "..." else . end);
[inputs | fromjson?] as $ev
| ($ev | map(select(.type == "iteration")) | sort_by(.n)) as $it
| ($ev | map(select(.type == "tool_result"))) as $tr
| ($ev | map(select(.type == "abort"))) as $ab
| ($ev | map(select(.type == "budget_low"))) as $bl
| ($ev | map(select(.type == "goal"))) as $gl
| ($ev | map(select(.type == "run_start"))) as $rs
| ($ev | map(select(.type == "verifying"))) as $vf
| ($ev | map(select(.type == "output_truncated"))) as $ot
| ($ev | map(select(.type == "iteration" or .type == "usage"))) as $tok
| ($tr | map(select(.is_error == true))) as $errs
| ($ev | map(ep(.ts)) | map(select(. != null))) as $eps
| (if ($eps | length) > 0 then ($eps | min) else null end) as $t0
| (if ($eps | length) > 0 then ($eps | max) else null end) as $t1
| (if $t0 != null and $t1 != null then ($t1 - $t0) else null end) as $wall
| (if ($it | length) > 0 then ($it | last | .n) else 0 end) as $lastn
| (if ($tok | length) > 0 then ($tok | last) else null end) as $lasttok
| (if ($rs | length) > 0 then ($rs | map(.model // "unknown") | unique | join(",")) else "-" end) as $models
| (if ($rs | length) > 0 then ($rs | map((.spec // "-") | split("/") | last) | unique | join(",")) else "-" end) as $specs
| (if ($rs | length) > 0 then (($rs | last | .max_iters) // "-") else "-" end) as $ceil
| ([$tr[] | {k: .name}] | group_by(.k) | map({k: .[0].k, n: length}) | sort_by(-.n) | .[0:6] | map("\(.k) \(.n)") | join(", ")) as $tools
| ([$errs[] | {k: cls(.preview)}] | group_by(.k) | map({k: .[0].k, n: length}) | sort_by(-.n) | .[0:6] | map("  - \(.k) x\(.n)") | join("\n")) as $errcls
| ($ab | group_by([.reason, (.budget_kind // "-"), (.budget_max // "-")])
   | map({reason: .[0].reason, bk: (.[0].budget_kind // "-"), bm: (.[0].budget_max // "-"), n: length,
          models: (map(.model // "?") | unique | join(","))})
   | sort_by(-.n)
   | map("  - \"\(.reason)\" x\(.n) — model: \(.models)" + (if .bk == "-" then "" else " — budget: \(.bm) \(.bk)" end))
   | join("\n")) as $aborts
| ([$gl[] | {k: .outcome}] | group_by(.k) | map({k: .[0].k, n: length}) | map("\(.k) \(.n)") | join(" / ")) as $goals
| ([$gl[] | select(.outcome == "rejected") | {k: (.reason // "?")}]
   | group_by(.k) | map({k: .[0].k, n: length}) | sort_by(-.n) | .[0:3]
   | map("  - rejected: \"\(.k)\" x\(.n)") | join("\n")) as $rej
| (if ($it | length) == 0 then "n/a (no iteration events)"
   else
     ($it | length) as $L
     | [($it[0].input_tokens // 0), ($it[(($L * 1 / 4) | floor)].input_tokens // 0),
        ($it[(($L * 2 / 4) | floor)].input_tokens // 0), ($it[(($L * 3 / 4) | floor)].input_tokens // 0),
        ($it[-1].input_tokens // 0)] as $c
     | ($it[0].n) as $n0 | ($it[(($L * 1 / 4) | floor)].n) as $n1 | ($it[(($L * 2 / 4) | floor)].n) as $n2
     | ($it[(($L * 3 / 4) | floor)].n) as $n3 | ($it[-1].n) as $n4
     | ([[$c[0], $n0], [$c[1], $n1], [$c[2], $n2], [$c[3], $n3], [$c[4], $n4]]
        | map("\(num(.[0]))@\(.[1])") | join(" -> "))
   end) as $curve
| (if ($ev | length) == 0 then "  - (no parseable event lines)\n"
   else
     "- runs: \($rs | length) | model: \($models) | spec: \($specs) | iters-ceil: \($ceil)\n"
     + "- iterations: \($it | length) (last n=\($lastn)) | verifying calls: \($vf | length)\n"
     + (if $wall == null
        then "- wall: unknown (no parsable timestamps)\n"
        else "- wall: \($wall)s (\(dur($wall))) | \($t0 | todateiso8601) -> \($t1 | todateiso8601)\n" end)
     + (if $lasttok == null
        then "- tokens: none recorded\n"
        else "- tokens (cumulative at last iteration): \(num($lasttok.input_tokens // 0)) in / \(num($lasttok.output_tokens // 0)) out\n" end)
     + "- input context curve (cumulative, iter quartiles): \($curve)\n"
     + (if $tools == "" then "- tools: (none)\n" else "- tools: \($tools)\n" end)
     + "- failed tool results: \($errs | length)"
     + (if $errcls == "" then "\n" else " — classes (first line, digits->N):\n\($errcls)\n" end)
     + (if $goals == "" then "- goal: none\n" else "- goal: \($goals)\n" end)
     + (if $rej == "" then "" else "\($rej)\n" end)
     + "- aborts: \($ab | length)" + (if ($ab | length) == 0 then "\n" else ":\n\($aborts)\n" end)
     + (if ($bl | length) == 0
        then "- budget_low fires: 0"
        else "- budget_low fires: \($bl | length) (first at remaining_iters=\($bl[0].remaining_iters))" end)
     + " | output_truncated: \($ot | length)\n"
   end)
JQEOF
)

DIGEST_BODY=""
TOTAL_ITERS=0
for f in "${FILES[@]+${FILES[@]}}"; do
  base=$(basename "$f")
  fsize=$(wc -c < "$f" | tr -d ' ')
  fmtime=$(mtime_of "$f")
  block=$(jq -Rrn "$JQ_PROG" "$f" 2>/dev/null) || block="- (jq parse failure — digest skipped this file)"
  iters=$(jq -Rr 'fromjson? | select(.type == "iteration") | 1' "$f" 2>/dev/null | wc -l | tr -d ' ')
  TOTAL_ITERS=$((TOTAL_ITERS + iters))
  DIGEST_BODY="${DIGEST_BODY}### ${base}

(${fsize} bytes | mtime $(epoch_to_iso "$fmtime"))

${block}

"
done
if [ "${#FILES[@]}" -eq 0 ]; then
  DIGEST_BODY="(no events files in .chug/ — nothing has run in this checkout yet)

"
fi

# --- staleness post-scan: did events move while we were digesting? ----------
POST=$(newest_events)
POST_NEWEST_MTIME="${POST%%$'\t'*}"
POST_NEWEST_PATH="${POST#*$'\t'}"
[ -n "$POST_NEWEST_PATH" ] || { POST_NEWEST_MTIME=0; POST_NEWEST_PATH="-"; }
MOVED="no"
if [ "$POST_NEWEST_MTIME" -gt "$PRE_NEWEST_MTIME" ]; then
  MOVED="YES (events were written while the digest was being generated — re-run scripts/eval-digest.sh)"
fi
CORPUS_AGE=$((GEN_EPOCH - POST_NEWEST_MTIME))
AGE_TXT="n/a"
if [ "$POST_NEWEST_MTIME" -gt 0 ]; then
  if [ "$CORPUS_AGE" -lt 0 ]; then
    AGE_TXT="negative (${CORPUS_AGE}s) — events are newer than the generation clock (write race)"
  else
    AGE_TXT="${CORPUS_AGE}s ($(human_dur "$CORPUS_AGE")) before generation"
  fi
fi
REL_NEWEST="${POST_NEWEST_PATH#"$ROOT"/}"

# --- write the digest --------------------------------------------------------
{
  printf '# Eval digest — T46 pre-computed Phase-1 corpus summary\n\n'
  printf 'generated-at: %s | repo: %s | events files: %d | total iterations: %d\n\n' \
    "$GEN_ISO" "$ROOT" "${#FILES[@]}" "$TOTAL_ITERS"
  printf 'Read this FIRST (META-META-SPEC corpus item 1). Raw .chug/events*.jsonl is for\n'
  printf 'drilling into a specific incident this digest raised — not for ad-hoc jq ETL.\n\n'
  printf '## Events files\n\n'
  printf '%s' "$DIGEST_BODY"
  printf '## Corpus inputs\n\n'
  printf -- '- TODO.md status counts: %s\n' "$TODO_COUNTS"
  printf -- '- %s\n' "$EVAL_LINE"
  printf '\n## Staleness\n\n'
  printf -- '- generated-at: %s\n' "$GEN_ISO"
  if [ "$POST_NEWEST_MTIME" -gt 0 ]; then
    printf -- '- newest-events-mtime: %s (%s)\n' "$PRE_NEWEST_ISO" "$REL_NEWEST"
  else
    printf -- '- newest-events-mtime: none (no events files exist)\n'
  fi
  printf -- '- corpus age at generation: %s\n' "$AGE_TXT"
  printf -- '- events-moved-during-generation: %s\n' "$MOVED"
  printf -- '- reader staleness check: if any .chug/events*.jsonl mtime > generated-at, this\n'
  printf -- '  digest is stale — regenerate with scripts/eval-digest.sh. Mechanical check:\n'
  printf -- '  find .chug -maxdepth 1 -name '"'"'events*.jsonl'"'"' -newer .chug/eval-digest.md | grep -q . && echo STALE\n'
} > "$OUT"

# --- one-line stdout summary (loopd logs it) ---------------------------------
echo "eval-digest: wrote .chug/eval-digest.md (files=${#FILES[@]}, iterations=$TOTAL_ITERS, generated $GEN_ISO)"
