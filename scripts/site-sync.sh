#!/usr/bin/env bash
# site-sync.sh — T98: regenerate the chug.sh stats block at wrap (deterministic).
#
# loopd runs this after every `cycle OK`. It computes the public stats from
# the chug repo's own facts (TODO.md, git log, .chug/loopd logs,
# EVALUATION.md) and rewrites ONLY the marked region of the site's
# index.html:
#
#   <!-- STATS:BEGIN -->   ... machine-written region ...   <!-- STATS:END -->
#
# Zero LLM, sub-second — the T46 eval-digest pattern: a deterministic script,
# not a model, keeps the public numbers honest. Every number in the block is
# counted here from the named sources; the block cites nothing the script did
# not itself compute (req 4). The block carries NO clock, so unchanged inputs
# produce byte-identical output and no commit (req 1, idempotence) — the sync
# date rides the commit message only.
#
# Markers are one-time site setup (site child/maintainer wraps the proof
# section's stats region with them); this script NEVER injects them. A page
# without — or with malformed — markers is reported and left untouched with
# exit 3, so a site-side mistake cannot be papered over by machine HTML.
#
# Best-effort, like observability (reqs 2+5): a missing site clone, a
# non-git site dir, a failed commit, and a REJECTED PUSH all warn and exit 0
# — site sync must never fail a cycle. Only the marker error exits nonzero.
# Plain `git push` only — never a force-push.
#
# Usage:
#   scripts/site-sync.sh [SITE_DIR] [CHUG_ROOT]
#     SITE_DIR   site checkout (default $CHUG_SITE_DIR, else ~/workspace/chug-site)
#     CHUG_ROOT  chug repo the facts are read from (default: this repo)
# Env:
#   CHUG_SYNC_NOW=<date|iso>  pin the sync date for the commit message/subject
#                             (CHUG_DIGEST_NOW pattern; tests pin this)
#   CHUG_SITE_SYNC_NO_PUSH=1  skip the push (fixture tests, offline runs)
set -u
export LC_ALL=C

SITE="${1:-${CHUG_SITE_DIR:-$HOME/workspace/chug-site}}"
CHUG="${2:-$(cd "$(dirname "$0")/.." && pwd)}"
SYNC_DATE="$(printf '%s' "${CHUG_SYNC_NOW:-$(date -u +%Y-%m-%d)}" | cut -c1-10)"

warn() { printf 'site-sync: %s\n' "$1" >&2; }

# --- req 5: missing site clone -> warn once, exit 0 -------------------------
if [ ! -d "$SITE" ]; then
  warn "site clone not found at $SITE (set CHUG_SITE_DIR or pass it as arg 1) — nothing to sync"
  exit 0
fi
INDEX="$SITE/index.html"
if [ ! -f "$INDEX" ]; then
  warn "$INDEX not found — nothing to sync"
  exit 0
fi

# --- markers present and well-formed, else exit 3 WITHOUT editing ------------
MARK_B='^[[:space:]]*<!-- STATS:BEGIN -->[[:space:]]*$'
MARK_E='^[[:space:]]*<!-- STATS:END -->[[:space:]]*$'
begins=$(grep -cE "$MARK_B" "$INDEX") || begins=0
ends=$(grep -cE "$MARK_E" "$INDEX") || ends=0
b_line=$(grep -nE "$MARK_B" "$INDEX" | head -1 | cut -d: -f1)
e_line=$(grep -nE "$MARK_E" "$INDEX" | head -1 | cut -d: -f1)
if [ "${begins:-0}" -ne 1 ] || [ "${ends:-0}" -ne 1 ] || [ -z "$b_line" ] || [ -z "$e_line" ] \
  || [ "$b_line" -ge "$e_line" ]; then
  warn "$INDEX must contain exactly one '<!-- STATS:BEGIN -->' line followed by one '<!-- STATS:END -->' line (found $begins/$ends) — wrap the stats region with the markers, then re-run; NOT editing the page"
  exit 3
fi

# --- fact 1 (req 1): items landed — TODO.md done rows vs total item rows -----
# Same row grammar as loopd's todo_rows: col2 id matches ^T[0-9]+$, col6 status.
todo_facts() { # -> "<done> <total>" ("0 0" when TODO.md is missing)
  [ -f "$CHUG/TODO.md" ] || { printf '0 0'; return; }
  awk -F'|' '/^[|]/ {
    id = $2; gsub(/[ \t]/, "", id)
    if (id ~ /^T[0-9]+$/) {
      total++
      s = $6; gsub(/[ \t]/, "", s)
      if (s == "done") done++
    }
  } END { printf "%d %d", done + 0, total + 0 }' "$CHUG/TODO.md"
}

# --- fact 2 (req 1): current test count — newest commit message quoting a gate
# Priority inside a message: `nextest N/M` (N = passed) > `suite N unit` >
# `N unit`. A filtered run ("cargo test --bin chug image 16/16") matches none
# of these, deliberately: the card cites a full-suite gate count or nothing.
# Walks newest-first and stops at the newest message that carries one; the
# card names that commit, so the citation stays checkable (req 4).
test_count() { # -> "<n>\t<short-ref>" ("" when no message quotes a gate)
  git --no-pager -C "$CHUG" log -200 --format='%x1e%h%x1f%B' 2>/dev/null \
    | awk -v RS="$(printf '\036')" -F"$(printf '\037')" '
      NF < 2 { next }   # text before the first record separator
      {
        if (match($2, /nextest [0-9]+\/[0-9]+/)) {
          n = substr($2, RSTART + 8, RLENGTH - 8); sub(/\/[0-9]+$/, "", n)
          print n "\t" $1; exit
        }
        if (match($2, /suite [0-9]+ unit/)) {
          n = substr($2, RSTART + 6, RLENGTH - 11)
          print n "\t" $1; exit
        }
        if (match($2, /[0-9]+ unit/)) {
          n = substr($2, RSTART, RLENGTH - 5)
          print n "\t" $1; exit
        }
      }'
}

# --- fact 3 (req 1): cycle count from the loopd logs -------------------------
cycle_count() { # -> "<n>\t<source text>"
  local log="$CHUG/.chug/loopd/loopd.log" n
  if [ -f "$log" ]; then
    n=$(grep -c ' cycle OK:' "$log" 2>/dev/null) || n=0
    printf '%s\t"cycle OK" lines in .chug/loopd/loopd.log' "$n"
    return
  fi
  if [ -d "$CHUG/.chug/loopd" ]; then
    n=$(grep -l 'chug: goal complete' "$CHUG"/.chug/loopd/cycle-*.log 2>/dev/null | wc -l | tr -d ' ')
    printf '%s\tcycle logs that reached goal complete (.chug/loopd/cycle-*.log)' "${n:-0}"
    return
  fi
  printf '0\tno .chug/loopd logs found in the chug repo'
}

# --- fact 4 (req 1 + EVALUATION.md source): last evaluation write ------------
eval_date() { # -> date ("" when the file is missing or git never saw it)
  [ -f "$CHUG/EVALUATION.md" ] || return 0
  git --no-pager -C "$CHUG" log -1 --format=%as -- EVALUATION.md 2>/dev/null
}

# --- fact 5 (req 1): last-5 landed items — refs + dates from git log ---------
# Landed items are the impl commits whose subject opens "tNN: " (the loop's
# commit convention); merge/"todo:" commits never match. Deduped by item, so
# fix-up rounds collapse into their item. Titles are word-truncated to <=100
# bytes (never splits a multibyte char) and HTML-escaped (& < >).
last_landed() { # -> lines "<ID>\t<short-ref>\t<date>\t<escaped title>"
  git --no-pager -C "$CHUG" log -120 --date=short --format='%h%x09%ad%x09%s' 2>/dev/null \
    | awk -F'\t' '
      $3 ~ /^[tT][0-9]+: / {
        id = $3; sub(/:.*/, "", id); id = toupper(id)
        if (seen[id]++) next
        t = $3; sub(/^[tT][0-9]+: */, "", t)
        n = split(t, w, " "); out = ""; kept = 0
        for (i = 1; i <= n; i++) {
          cand = (out == "") ? w[i] : out " " w[i]
          if (length(cand) > 100) break
          out = cand; kept = i
        }
        if (kept < n) out = out "…"
        gsub(/&/, "\\&amp;", out); gsub(/</, "\\&lt;", out); gsub(/>/, "\\&gt;", out)
        print id "\t" $1 "\t" $2 "\t" out
        if (++k >= 5) exit
      }'
}

# --- compute the facts --------------------------------------------------------
FACTS=$(todo_facts)
DONE="${FACTS%% *}"; TOTAL="${FACTS#* }"
TESTS=$(test_count)                      # "<n>\t<ref>" or ""
TEST_N="n/a"; TEST_REF=""; TEST_SRC="no full-suite gate count found in recent commit messages"
if [ -n "$TESTS" ]; then
  TEST_N="${TESTS%%$'\t'*}"; TEST_REF="${TESTS#*$'\t'}"
  TEST_SRC="newest full-suite gate count in a commit message"
fi
CYCLES=$(cycle_count)                    # "<n>\t<source text>"
CYC_N="${CYCLES%%$'\t'*}"; CYC_SRC="${CYCLES#*$'\t'}"
EVAL_D=$(eval_date); [ -n "$EVAL_D" ] || EVAL_D="n/a"
LANDED=$(last_landed)                    # ID \t ref \t date \t title lines
LANDED_REFS=$(printf '%s\n' "$LANDED" | awk -F'\t' 'NF >= 3 { printf "%s ", $2 }')

# --- generate the block (no clock in it: unchanged inputs => byte-identical) --
TMP_BLOCK=$(mktemp "${TMPDIR:-/tmp}/site-sync-block.XXXXXX") || exit 0
TMP_NEW=$(mktemp "${TMPDIR:-/tmp}/site-sync-new.XXXXXX") || { rm -f "$TMP_BLOCK"; exit 0; }
trap 'rm -f "$TMP_BLOCK" "$TMP_NEW"' EXIT
{
  printf '<div class="stats">\n'
  printf '  <div class="stat"><b>%s/%s</b><span>queue items landed — done rows in TODO.md, %s item rows total</span></div>\n' "$DONE" "$TOTAL" "$TOTAL"
  if [ -n "$TEST_REF" ]; then
    printf '  <div class="stat"><b>%s</b><span>tests green at the %s (%s)</span></div>\n' "$TEST_N" "$TEST_SRC" "$TEST_REF"
  else
    printf '  <div class="stat"><b>%s</b><span>tests green — %s</span></div>\n' "$TEST_N" "$TEST_SRC"
  fi
  printf '  <div class="stat"><b>%s</b><span>cycles completed by the loopd supervisor — %s</span></div>\n' "$CYC_N" "$CYC_SRC"
  printf '  <div class="stat"><b>%s</b><span>last EVALUATION.md write (git log, chug repo)</span></div>\n' "$EVAL_D"
  printf '</div>\n'
  if [ -n "$LANDED" ]; then
    printf '<p class="lede">Last five items landed — item · ref · date, quoted from the chug repo'"'"'s git log:</p>\n'
    printf '<ul>\n'
    printf '%s\n' "$LANDED" | while IFS="$(printf '\t')" read -r lid lref ldate ltitle; do
      [ -n "$lid" ] || continue
      printf '  <li><code>%s</code> · <code>%s</code> · %s — %s</li>\n' "$lid" "$lref" "$ldate" "$ltitle"
    done
    printf '</ul>\n'
  else
    printf '<p class="lede">No landed-item commits ("tNN: …" subjects) in the chug repo'"'"'s git log yet.</p>\n'
  fi
} > "$TMP_BLOCK"

# --- splice: replace ONLY the region between the markers ----------------------
awk -v blkfile="$TMP_BLOCK" '
  $0 ~ /^[[:space:]]*<!-- STATS:BEGIN -->[[:space:]]*$/ {
    print
    while ((getline line < blkfile) > 0) print line
    close(blkfile)
    inblk = 1
    next
  }
  $0 ~ /^[[:space:]]*<!-- STATS:END -->[[:space:]]*$/ { inblk = 0; print; next }
  inblk != 1 { print }
' "$INDEX" > "$TMP_NEW"

if cmp -s "$TMP_NEW" "$INDEX"; then
  echo "site-sync: stats block unchanged — no commit"
  exit 0
fi
cat "$TMP_NEW" > "$INDEX"   # in place: keeps the inode and the page's permissions

echo "site-sync: rewrote the stats region of $INDEX (items $DONE/$TOTAL, tests $TEST_N, cycles $CYC_N, last eval $EVAL_D)"

# --- commit + push (req 2): best-effort, never force, rejection warns + exit 0
if ! git -C "$SITE" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  warn "$SITE is not a git work tree — page updated but not committed"
  exit 0
fi
if [ -z "$(git -C "$SITE" status --porcelain -- index.html)" ]; then
  echo "site-sync: index.html change not visible to git — nothing to commit"
  exit 0
fi
git -C "$SITE" add index.html
if ! git -C "$SITE" commit -q -m "site: stats sync $SYNC_DATE" \
  -m "scripts/site-sync.sh (T98), deterministic — items $DONE/$TOTAL (TODO.md), tests $TEST_N ($TEST_SRC${TEST_REF:+, $TEST_REF}), cycles $CYC_N ($CYC_SRC), last eval $EVAL_D; last-5 landed: $LANDED_REFS"; then
  warn "commit failed (git identity?) — page updated but uncommitted"
  exit 0
fi
echo "site-sync: committed $(git -C "$SITE" rev-parse --short HEAD 2>/dev/null)"
if [ "${CHUG_SITE_SYNC_NO_PUSH:-0}" = "1" ]; then
  echo "site-sync: push skipped (CHUG_SITE_SYNC_NO_PUSH=1)"
elif git -C "$SITE" push 2>/dev/null; then
  echo "site-sync: pushed"
else
  warn "push REJECTED (best-effort, not a failure) — push $SITE manually"
fi
exit 0
