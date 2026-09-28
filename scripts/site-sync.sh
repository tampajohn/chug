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

# --- T99: TIMELINE + FEATURES regions ---------------------------------------
# Same deterministic doctrine as the stats block above, extended to the page's
# timeline (<!-- TIMELINE:BEGIN/END --> around the <div class="tl"> block) and
# feature grid (<!-- FEATURES:BEGIN/END --> around the <div class="grid">).
# Unlike the stats markers (one-time site setup), T99's markers are BOOTSTRAPPED
# here (req 4): when absent, the script wraps the existing block verbatim in a
# separate bootstrap commit — the hand-built entries/cards become the initial
# region content, deduped against TODO-derived entries by commit ref. A missing
# or malformed block only warns: best-effort, never fails a cycle (req 3).
#
# find_block FILE CLASS-ATTR — line numbers of a top-level
#   `<div class="ATTR">...</div>` block (depth-counted: nested divs tracked).
#   Prints "START END" or nothing (exit 4) when absent or never closed.
find_block() { # file class-attr -> "start end" | "" (exit 4)
  awk -v pat="$2" '
    found == 0 && $0 ~ pat {
      start = NR
      depth = gsub(/<div/, "&") - gsub(/<\/div/, "&")
      if (depth <= 0) { print start, NR; found = 1; exit }
      found = 2; next
    }
    found == 2 {
      depth += gsub(/<div/, "&") - gsub(/<\/div/, "&")
      if (depth <= 0) { print start, NR; found = 1; exit }
    }
    END { if (found == 2) exit 4 }
  ' "$1"
}

# insert_markers SRC OUT START END BEGIN-TXT END-TXT — copy SRC to OUT with the
# two marker lines wrapped around the START/END line numbers (block untouched).
insert_markers() { # src out start end begin-txt end-txt
  awk -v s="$3" -v e="$4" -v mb="$5" -v me="$6" '
    NR == s { print mb }
    { print }
    NR == e { print me }
  ' "$1" > "$2"
}

# splice_region SRC OUT BEGIN-RE END-RE BLOCKFILE — replace the region between
# the marker lines with BLOCKFILE's content (T98's splice, parameterized).
splice_region() { # src out begin-re end-re blockfile
  awk -v br="$3" -v er="$4" -v blkfile="$5" '
    $0 ~ br {
      print
      while ((getline line < blkfile) > 0) print line
      close(blkfile)
      inblk = 1
      next
    }
    $0 ~ er { inblk = 0; print; next }
    inblk != 1 { print }
  ' "$1" > "$2"
}

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
    # T142: count the supervisor's own rc-based verdict stamps, never raw
    # child bytes — model text reaches a cycle log verbatim, so a spoofed
    # `chug: goal complete` line in a failed cycle must not inflate the
    # public cycle count (loopd.sh stamps `verdict: ...` from the child's
    # exit status; logs older than that stamp carry none and count 0).
    n=$(grep -l 'verdict: goal complete (rc=0)' "$CHUG"/.chug/loopd/cycle-*.log 2>/dev/null | wc -l | tr -d ' ')
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

# --- T99 req 2: feature grid from FEATURES.md --------------------------------
# Each F-row -> name + one-line what + status badge. landed = the row is
# checked off in FEATURES.md (strikethrough / LANDED annotation); in-flight =
# a TODO.md row references the F-id; else queued. Names/what text are
# FEATURES.md facts (markdown emphasis stripped, <=160 bytes, HTML-escaped).
FT_WHAT_CAP=160
features_rows() { # -> lines "<F-id>\t<status>\t<name>\t<what>"
  [ -f "$CHUG/FEATURES.md" ] || return 0
  awk -F'|' -v cap="$FT_WHAT_CAP" '
    function esc(s) { gsub(/&/, "\\&amp;", s); gsub(/</, "\\&lt;", s); gsub(/>/, "\\&gt;", s); return s }
    function strip_md(s) { gsub(/~~/, "", s); gsub(/\*\*/, "", s); return s }
    function prep(s, cap,   n, w, out, kept, i, cand, m, seg, j, rebuilt) {
      gsub(/^[ \t]+|[ \t]+$/, "", s)
      n = split(s, w, " "); out = ""; kept = 0
      for (i = 1; i <= n; i++) {
        cand = (out == "") ? w[i] : out " " w[i]
        if (length(cand) > cap) break
        out = cand; kept = i
      }
      if (kept < n) out = out "…"
      out = esc(out)
      m = split(out, seg, "`")          # backtick pairs -> <code>…</code>
      if (m >= 3) {
        rebuilt = seg[1]
        for (j = 2; j + 1 <= m; j += 2) rebuilt = rebuilt "<code>" seg[j] "</code>" seg[j + 1]
        if (m % 2 == 0) rebuilt = rebuilt "`" seg[m]
        out = rebuilt
      }
      return out
    }
    /^[|]/ {
      id = $2; gsub(/[ \t]/, "", id)
      if (id !~ /^F[0-9]+$/) next
      namecell = $3
      status = "queued"
      if (namecell ~ /~~/ || namecell ~ /LANDED/) status = "landed"
      name = namecell
      sub(/—.*/, "", name)              # name cell = text before the status annotation
      name = strip_md(name); gsub(/^[ \t]+|[ \t]+$/, "", name)
      if (name == "") next
      print id "\t" status "\t" esc(name) "\t" prep($4, cap)
    }' "$CHUG/FEATURES.md"
}

# card_chunks REGIONFILE — emit the region with card blocks delimited by
# ^\036CARD / ^\036ENDCARD sentinel lines (depth-counted like tl_filter).
# Every card chunk is emitted in full (T99 fix-up finding 1 sweep): this is
# the ONLY region-walk leg that buffers whole items, and after the fix it
# drops nothing — the same-shaped gate in tl_filter is deliberately KEPT
# there (see its comment), because timeline machine entries are rebuilt.
card_chunks() { # regionfile
  awk '
    function flush_card() {
      initem = 0
      # print UNCONDITIONALLY (T99 fix-up finding 1): the old `if (buf ~ /<p/)`
      # gate silently dropped any card with no <p> — h3-only cards, <ul>-body
      # cards — contradicting "no card is ever dropped" below. Every card
      # reaches the badge walk; cards without a matchable <span> name pass
      # through it untouched.
      print buf
      printf "\036ENDCARD\n"
      buf = ""
    }
    /^[[:space:]]*<div class="card">/ {
      printf "\036CARD\n"
      buf = $0
      depth = gsub(/<div/, "&") - gsub(/<\/div/, "&")
      if (depth <= 0) { flush_card(); next }
      initem = 1; next
    }
    initem == 1 {
      buf = buf "\n" $0
      depth += gsub(/<div/, "&") - gsub(/<\/div/, "&")
      if (depth <= 0) flush_card()
      next
    }
    { print }
  ' "$1"
}

card_name() { # cardfile -> first <span>…</span> text inside its <h3>
  awk '
    /<h3/ && !got {
      if (match($0, /<span[^>]*>/)) {
        s = substr($0, RSTART + RLENGTH); p = index(s, "</span>")
        if (p > 0) { print substr(s, 1, p - 1); got = 1; exit }
      }
    }' "$1"
}

norm_id() { # -> lowercase alphanumerics only (deterministic name matching)
  printf '%s' "$1" | tr -cd '[:alnum:]' | tr '[:upper:]' '[:lower:]'
}

# badge_ensure CARDFILE STATUS -> stdout: exactly one <i class="q">STATUS</i>
# at the end of the card's h3 (existing badge replaced). CSS class is the
# current grid's own badge class — no new styles (req 2).
badge_ensure() { # cardfile status
  awk -v st="$2" '
    /<h3/ && /<\/h3>/ {
      gsub(/<i class="q">[^<]*<\/i>/, "")
      if (match($0, /<\/h3>/)) {
        $0 = substr($0, 1, RSTART - 1) "<i class=\"q\">" st "</i>" substr($0, RSTART)
      }
    }
    { print }' "$1"
}

# features_generate REGIONFILE OUTFILE — rebuild the region: every existing
# card keeps its position and markup (CSS classes/order preserved, req 2);
# cards whose name matches an F-item get their badge ensured; F-items with no
# card are appended as synthesized cards in FEATURES.md order. No card is
# ever dropped (flush_card prints every chunk — fix-up finding 1), so nothing
# outside FEATURES.md is lost.
features_generate() { # regionfile outfile
  local region="$1" out="$2" rowstmp rows cardname cn fn short long line fid fstatus fname fwhat
  local matched="" sentinel match_id
  rowstmp="$(mktemp "${TMPDIR:-/tmp}/site-sync-frows.XXXXXX")"
  rows="$(mktemp "${TMPDIR:-/tmp}/site-sync-frows2.XXXXXX")"
  sentinel="$(printf '\036')"
  features_rows > "$rowstmp"
  if [ ! -s "$rowstmp" ]; then
    warn "no F-rows parsed from FEATURES.md — feature grid not updated"
    cat "$region" > "$out"; rm -f "$rowstmp" "$rows"; return 0
  fi
  # in-flight refinement: a TODO.md row referencing the F-id (F1 never matches F13)
  while IFS="$(printf '\t')" read -r fid fstatus fname fwhat; do
    [ -n "$fid" ] || continue
    if [ "$fstatus" != "landed" ] && [ -f "$CHUG/TODO.md" ] \
      && grep -qE "(^|[^A-Za-z0-9])F${fid#F}([^0-9]|$)" "$CHUG/TODO.md"; then
      fstatus="in-flight"
    fi
    printf '%s\t%s\t%s\t%s\n' "$fid" "$fstatus" "$fname" "$fwhat" >> "$rows"
  done < "$rowstmp"
  {
    local incard=0
    while IFS= read -r line; do
      case "$line" in
        "$sentinel"CARD)   incard=1; : > "$TMP_CARD"; continue ;;
        "$sentinel"ENDCARD)
          incard=0
          if [ -s "$TMP_CARD" ]; then
            cardname="$(card_name "$TMP_CARD")"; cn="$(norm_id "$cardname")"
            match_id=""
            if [ -n "$cn" ]; then
              while IFS="$(printf '\t')" read -r fid fstatus fname fwhat; do
                [ -n "$fid" ] || continue
                case " $matched " in *" $fid "*) continue ;; esac
                fn="$(norm_id "$fname")"
                [ -n "$fn" ] || continue
                if [ "${#cn}" -le "${#fn}" ]; then short="$cn"; long="$fn"; else short="$fn"; long="$cn"; fi
                if [ "${#short}" -ge 4 ] && case "$long" in *"$short"*) true ;; *) false ;; esac; then
                  match_id="$fid"; match_status="$fstatus"
                  break
                fi
              done < "$rows"
            fi
            if [ -n "$match_id" ]; then
              matched="$matched $match_id"
              badge_ensure "$TMP_CARD" "$match_status"
            else
              cat "$TMP_CARD"
            fi
          fi
          continue ;;
      esac
      if [ "$incard" = 1 ]; then printf '%s\n' "$line" >> "$TMP_CARD"; else printf '%s\n' "$line"; fi
    done < <(card_chunks "$region")
    # unterminated trailing card chunk (malformed region) — keep verbatim
    if [ "$incard" = 1 ]; then cat "$TMP_CARD"; fi
    # F-items with no card: synthesized, FEATURES.md order, same card classes
    while IFS="$(printf '\t')" read -r fid fstatus fname fwhat; do
      [ -n "$fid" ] || continue
      case " $matched " in *" $fid "*) continue ;; esac
      printf '    <div class="card">\n'
      printf '      <h3><span>%s</span><i class="q">%s</i></h3>\n' "$fname" "$fstatus"
      printf '      <p>%s</p>\n' "$fwhat"
      printf '    </div>\n'
    done < "$rows"
  } > "$out" 2>/dev/null
  rm -f "$rowstmp" "$rows"
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

# --- T99 req 1+5: timeline entries from TODO.md done rows --------------------
# One entry per done row whose notes cite a commit that EXISTS in the chug repo
# (git cat-file before write, req 5). The ref is the first 7-40 hex-char token
# in the notes cell (boundary-checked, so "t76"/"specs/t3" never match); if it
# does not verify, later tokens are tried, then the row is skipped with a
# warning. Date = the commit's git author date; title = the row's own title.
done_rows() { # -> lines "<id>\t<cand1 cand2 ...>\t<title>" (title escaped+truncated)
  [ -f "$CHUG/TODO.md" ] || return 0
  awk -F'|' '
    function esc_trunc(s,   n, split_s, out, w, i, kept) {
      gsub(/^[ \t]+|[ \t]+$/, "", s)
      n = split(s, w, " "); out = ""; kept = 0
      for (i = 1; i <= n; i++) {
        cand = (out == "") ? w[i] : out " " w[i]
        if (length(cand) > 100) break
        out = cand; kept = i
      }
      if (kept < n) out = out "…"
      gsub(/&/, "\\&amp;", out); gsub(/</, "\\&lt;", out); gsub(/>/, "\\&gt;", out)
      return out
    }
    /^[|]/ {
      id = $2; gsub(/[ \t]/, "", id)
      if (id !~ /^T[0-9]+$/) next
      s = $6; gsub(/[ \t]/, "", s)
      if (s != "done") next
      notes = " " $7 " "
      cands = ""; pos = 1; ncand = 0
      while (ncand < 6 && match(substr(notes, pos), /[0-9a-fA-F][0-9a-fA-F][0-9a-fA-F][0-9a-fA-F][0-9a-fA-F][0-9a-fA-F][0-9a-fA-F][0-9a-fA-F]*/)) {
        mstart = pos + RSTART - 1
        mlen = RLENGTH; if (mlen > 40) mlen = 40
        before = substr(notes, mstart - 1, 1); after = substr(notes, mstart + RLENGTH, 1)
        if (before !~ /[0-9a-fA-F]/ && after !~ /[0-9a-fA-F]/) {
          cands = cands (cands == "" ? "" : " ") tolower(substr(notes, mstart, mlen))
          ncand++
        }
        pos = mstart + RLENGTH
      }
      if (cands == "") next
      print id "\t" cands "\t" esc_trunc($3)
    }' "$CHUG/TODO.md"
}

# hex_spans — hex tokens cited inside <span class="hash">…</span> of a file
# (boundary-checked, like done_rows; these are the region's existing facts).
hex_spans() { # file -> one ref per line
  grep -oE '<span class="hash"[^>]*>[^<]*</span>' "$1" 2>/dev/null | awk '
    {
      rest = " " $0 " "; pos = 1
      while (match(substr(rest, pos), /[0-9a-fA-F][0-9a-fA-F][0-9a-fA-F][0-9a-fA-F][0-9a-fA-F][0-9a-fA-F][0-9a-fA-F][0-9a-fA-F]*/)) {
        mstart = pos + RSTART - 1
        before = substr(rest, mstart - 1, 1); after = substr(rest, mstart + RLENGTH, 1)
        if (before !~ /[0-9a-fA-F]/ && after !~ /[0-9a-fA-F]/) print tolower(substr(rest, mstart, RLENGTH))
        pos = mstart + RLENGTH
      }
    }'
}

# tl_date_of FILE — the first <div class="tl-date">…</div> date (YYYY-MM-DD)
# inside a timeline entry, "" when absent. Every curated entry carries one
# (site convention): it is the entry's own claimed day, parseable even when
# the h3 has no hash span (T122's whole point).
tl_date_of() { # file -> YYYY-MM-DD | ""
  sed -n 's/^[[:space:]]*<div class="tl-date">[[:space:]]*\([0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]\)[[:space:]]*<\/div>.*/\1/p' "$1" 2>/dev/null | head -1
}

# day_key DATE — T122 day-precision sort key: the LAST second (23:59:59Z) of
# that UTC day, via Fliegel–Van Flandern civil->Julian-day arithmetic (pure
# awk: no date(1) dialects, no TZ dependence). An entry known only to its day
# — curated prose whose hash spans resolve nowhere — sorts INSIDE its day:
# after same-day %ct entries (whose exact second a day-precision tl-date
# cannot claim) and before the next day's. The old T101 clause keyed these
# +inf ("after dated neighbors, never before"), which pinned 09-27 curated
# milestones below 09-28 generated ones — that bottom-pin dies. Unparseable
# input prints nothing; only an entry with NEITHER a resolvable ref NOR a
# parseable tl-date keeps the +inf sentinel (9999999999, retained for the
# truly undatable).
day_key() { # YYYY-MM-DD -> epoch seconds of that day's 23:59:59Z | ""
  awk -v d="$1" 'BEGIN {
    if (d !~ /^[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]$/) exit 1
    y = substr(d, 1, 4) + 0; m = substr(d, 6, 2) + 0; day = substr(d, 9, 2) + 0
    a = int((14 - m) / 12); yy = y + 4800 - a
    j = day + int((153 * (m + 12 * a - 3) + 2) / 5) \
        + yy * 365 + int(yy / 4) - int(yy / 100) + int(yy / 400) - 32045
    print (j - 2440588) * 86400 + 86399
  }'
}

# timeline_generate REGIONFILE OUTFILE — T101: ONE merged timeline. Every
# entry (curated + machine) is sorted by COMMIT TIME ascending (%ct via
# `git show -s --format=%ct`, reqs 1+4): T99's date-string/id-desc sort read
# newest-first against the page's day-one→latest flow and put T99 (d13a253)
# before T98 (8721c83). Curated entries keep their verbatim text wherever
# their anchor commit's %ct places them — prose wins (req 2): a done row
# whose verified commit matches ANY curated hash span merges away (one
# entry, not two). A curated entry with no resolvable commit (site-side or
# foreign hash, e.g. the live page's launchd plist) is dated by its OWN
# tl-date at day precision — the last second of that UTC day — so it sorts
# INTO its day, after same-day %ct entries, before the next day's (T122:
# the old +inf bottom-pin, "after dated neighbors, never before", buried
# 09-27 curated milestones under 09-28 generated ones; only an entry with
# neither a resolvable ref nor a parseable tl-date stays +inf). Machine
# entries
# are rebuilt from verified done rows (cat-file audit unchanged, req 5) and
# capped: the newest 20 render in full, older rows collapse into ONE
# "…and N earlier milestones (T…)" line placed where the oldest collapsed
# row sat (T99's rule, req 3 — pinned N=5 with 25 rows). Curated entries
# never collapse. Idempotent: machine + collapse output carries no <p>, so
# the next run re-classifies it as machine and rebuilds it byte-identically.
TL_CAP=20
timeline_generate() { # regionfile outfile
  local region="$1" out="$2"
  local items stray meta keys machs span ct file key
  local ncur=0 nmach=0
  items="$(mktemp "${TMPDIR:-/tmp}/site-sync-tli.XXXXXX")"
  stray="$(mktemp "${TMPDIR:-/tmp}/site-sync-stray.XXXXXX")"
  meta="$(mktemp "${TMPDIR:-/tmp}/site-sync-meta.XXXXXX")"
  keys="$(mktemp "${TMPDIR:-/tmp}/site-sync-keys.XXXXXX")"
  machs="$(mktemp "${TMPDIR:-/tmp}/site-sync-machs.XXXXXX")"
  tl_split "$region" "$items" "$stray"
  # --- curated entries: keep verbatim; anchor = first hash span that resolves
  local initem=0
  while IFS= read -r line; do
    case "$line" in
      "$(printf '\036')TL") initem=1; : > "$TMP_TLITEM"; continue ;;
      "$(printf '\036')ENDTL")
        initem=0
        if [ -s "$TMP_TLITEM" ] && grep -q '<p' "$TMP_TLITEM"; then
          file="$TMPD/tl-cur-$ncur"
          cp "$TMP_TLITEM" "$file"
          ct=""
          for span in $(hex_spans "$TMP_TLITEM"); do
            if git --no-pager -C "$CHUG" cat-file -e "$span^{commit}" 2>/dev/null; then
              ct="$(commit_time "$span")"
              if [ -n "$ct" ]; then break; fi
            fi
          done
          # T122: undatable by ref? date by the entry's own tl-date, day
          # precision (day_key = that day's 23:59:59Z): it sorts INTO its
          # day, after same-day %ct neighbors, before the next day's. The
          # +inf bottom-pin survives ONLY for an entry with no resolvable
          # ref AND no parseable tl-date (the truly undatable).
          key="$ct"
          if [ -z "$key" ]; then key="$(day_key "$(tl_date_of "$TMP_TLITEM")")"; fi
          printf '%s\t0\t%s\t%s\n' "${key:-9999999999}" "$ncur" "$file" >> "$meta"
          hex_spans "$TMP_TLITEM" >> "$keys"
          ncur=$((ncur + 1))
        fi
        continue ;;
    esac
    if [ "$initem" = 1 ]; then printf '%s\n' "$line" >> "$TMP_TLITEM"; fi
  done < "$items"
  # --- machine entries: verify each row's ref (req 5, unchanged) ...
  while IFS="$(printf '\t')" read -r id cands title; do
    [ -n "$id" ] || continue
    ref=""; ct=""; date=""
    for c in $cands; do
      if git --no-pager -C "$CHUG" cat-file -e "$c^{commit}" 2>/dev/null; then
        ct="$(commit_time "$c")"
        if [ -n "$ct" ]; then
          ref="$c"
          date="$(git --no-pager -C "$CHUG" show -s --format=%as "$c" 2>/dev/null)"
          break
        fi
      fi
    done
    if [ -z "$ref" ]; then
      warn "T${id#T}: done-row notes cite no commit found in the chug repo (git cat-file) — no timeline entry"
      continue
    fi
    # ... then merge by ref (req 2): a curated entry already citing this
    # commit keeps ITS prose — the raw row renders nowhere.
    if grep -qxF "$ref" "$keys"; then continue; fi
    nmach=$((nmach + 1))
    printf '%s\t2\t%s\t%s\t%s\t%s\t%s\n' "$ct" "$nmach" "${id#T}" "$ref" "$date" "$title" >> "$machs"
  done < <(done_rows)
  if [ "$ncur" -eq 0 ] && [ "$nmach" -eq 0 ]; then
    warn "no timeline entries (no curated <p> entries, no done rows with verifiable commit refs) — timeline not updated"
    cat "$region" > "$out"
    rm -f "$items" "$stray" "$meta" "$keys" "$machs"
    return 0
  fi
  cat "$machs" >> "$meta"
  {
    # non-item region lines (hand-authored structure outside any tl-item)
    # stay, verbatim, ahead of the merged list
    cat "$stray"
    # meta lines: KEY(%ct)\tRANK\tSEQ\tREST — rank 0 curated (REST = chunk
    # file), rank 2 machine (REST = id\tref\tdate\ttitle). Stable insertion
    # sort ascending by (key, rank, seq): same-second entries keep their
    # source order (row order for machine), prose before raw on a tie.
    awk -F'\t' -v cap="$TL_CAP" '
      function collapse(c, mn, mx,   range) {
        range = (mn == mx) ? sprintf("T%d", mn) : sprintf("T%d–T%d", mn, mx)
        printf "    <div class=\"tl-item\">\n"
        printf "      <div class=\"tl-date\"></div>\n"
        printf "      <div class=\"tl-rail\"><span class=\"tl-dot\"></span></div>\n"
        printf "      <div class=\"tl-body\">\n"
        printf "        <h3>…and %d earlier milestones (%s)</h3>\n", c, range
        printf "      </div>\n"
        printf "    </div>\n"
      }
      {
        n++
        K[n] = $1 + 0; R[n] = $2 + 0; S[n] = $3 + 0
        F4[n] = $4; F5[n] = $5; F6[n] = $6; F7[n] = $7
        i = n
        while (i > 1 && (K[i - 1] > K[i] || (K[i - 1] == K[i] && (R[i - 1] > R[i] || (R[i - 1] == R[i] && S[i - 1] > S[i]))))) {
          t = K[i]; K[i] = K[i - 1]; K[i - 1] = t
          t = R[i]; R[i] = R[i - 1]; R[i - 1] = t
          t = S[i]; S[i] = S[i - 1]; S[i - 1] = t
          t = F4[i]; F4[i] = F4[i - 1]; F4[i - 1] = t
          t = F5[i]; F5[i] = F5[i - 1]; F5[i - 1] = t
          t = F6[i]; F6[i] = F6[i - 1]; F6[i - 1] = t
          t = F7[i]; F7[i] = F7[i - 1]; F7[i - 1] = t
          i--
        }
      }
      END {
        # cap pass (machine only, req 3): the newest `cap` machine entries in
        # this ascending order render in full; the older ones collapse
        m = 0
        for (k = 1; k <= n; k++) if (R[k] == 2) m++
        coln = 0; seenc = 0; ak = 0; mn = 0; mx = 0
        for (k = 1; k <= n; k++) {
          if (R[k] != 2) continue
          seenc++
          if (seenc + cap > m) continue
          coln++
          if (coln == 1) { ak = k; mn = F4[k] + 0; mx = F4[k] + 0 }
          else { id = F4[k] + 0; if (id < mn) mn = id; if (id > mx) mx = id }
          collapsed[k] = 1
        }
        # emit ascending; the collapse line sits where the oldest collapsed
        # row sat (k >= ak precedes it)
        done_col = 0
        for (k = 1; k <= n; k++) {
          if (coln > 0 && !done_col && k >= ak) { collapse(coln, mn, mx); done_col = 1 }
          if (collapsed[k]) continue
          if (R[k] == 0) {
            while ((getline line < F4[k]) > 0) print line
            close(F4[k])
          } else {
            printf "    <div class=\"tl-item\">\n"
            printf "      <div class=\"tl-date\">%s</div>\n", F6[k]
            printf "      <div class=\"tl-rail\"><span class=\"tl-dot\"></span></div>\n"
            printf "      <div class=\"tl-body\">\n"
            printf "        <h3>%s <span class=\"hash\">(%s)</span></h3>\n", F7[k], F5[k]
            printf "      </div>\n"
            printf "    </div>\n"
          }
        }
        if (coln > 0 && !done_col) collapse(coln, mn, mx)
      }' "$meta"
  } > "$out"
  rm -f "$items" "$stray" "$meta" "$keys" "$machs" "$TMPD"/tl-cur-*
}

# tl_split REGIONFILE ITEMSFILE STRAYFILE — split the region into tl-item
# chunks (ALL of them: curated and machine, sentinel-delimited) plus the
# non-item lines between them. Classification is per item downstream: a
# tl-item CONTAINING <p> is hand-authored prose and is kept verbatim; a
# tl-item with NO <p> is machine-generated (timeline_generate emits
# facts-only entries, never a <p>) and is rebuilt from scratch each run —
# re-collapsing under the cap without dupes. The convention this costs,
# stated plainly: every hand-authored timeline entry MUST carry a <p>; an
# h3-only hand entry is treated as machine and rebuilt away. (T101 supersedes
# tl_filter's in-place walk: entries are no longer kept "in order then
# appended after" — they are MERGED into one commit-time-ordered list.)
tl_split() { # regionfile itemsfile strayfile
  awk -v stray="$3" '
    /^[[:space:]]*<div class="tl-item/ {
      printf "\036TL\n"
      buf = $0
      depth = gsub(/<div/, "&") - gsub(/<\/div/, "&")
      if (depth <= 0) { print buf; printf "\036ENDTL\n"; next }
      initem = 1; next
    }
    initem == 1 {
      buf = buf "\n" $0
      depth += gsub(/<div/, "&") - gsub(/<\/div/, "&")
      if (depth <= 0) { print buf; printf "\036ENDTL\n"; initem = 0; next }
      next
    }
    { if ($0 !~ /^[[:space:]]*$/) print > stray }
  ' "$1" > "$2"
}

# commit_time REF — the commit's committer time (%ct), "" if unresolvable.
# This is the ORDER key for every timeline entry (T101 req 1+4): a row's
# position is its commit time, never its TODO.md row order or row id.
commit_time() { # ref
  git --no-pager -C "$CHUG" show -s --format=%ct "$1" 2>/dev/null
}

TMPD="$(mktemp -d "${TMPDIR:-/tmp}/site-sync.XXXXXXXX")" || exit 0
trap 'rm -rf "$TMPD"' EXIT
TMP_BLOCK="$TMPD/stats-block"             # T98 stats region, regenerated
TMP_NEW="$TMPD/splice-stats"              # spliced after each region, in turn
TMP_NEW2="$TMPD/splice-timeline"
TMP_NEW3="$TMPD/splice-features"
TMP_CARD="$TMPD/card"                     # current card chunk (features walk)
TMP_TLITEM="$TMPD/tlitem"                 # current tl-item chunk (timeline walk)
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

# --- T99 req 4: bootstrap TIMELINE/FEATURES markers when absent ---------------
# Wrap the existing <div class="tl"> / <div class="grid"> blocks verbatim in
# their markers and commit that alone — the hand-built content becomes the
# initial region content (deduped against TODO-derived entries by commit ref
# during generation). Absent block or malformed markers: warn + skip, never
# fail the cycle. Each region may bootstrap at most once per run.
bootstrap_region() { # NAME CLASS-ATTR
  local nm="$1" cls="$2" nb ne blk
  nb=$(grep -cE "^[[:space:]]*<!-- ${nm}:BEGIN -->[[:space:]]*\$" "$INDEX" 2>/dev/null) || nb=0
  ne=$(grep -cE "^[[:space:]]*<!-- ${nm}:END -->[[:space:]]*\$" "$INDEX" 2>/dev/null) || ne=0
  if [ "$nb" -eq 1 ] && [ "$ne" -eq 1 ]; then return 0; fi
  if [ "$nb" -ne 0 ] || [ "$ne" -ne 0 ]; then
    warn "index.html has malformed ${nm} markers (found $nb/$ne, expected 0 or 1/1) — skipping the ${nm} region"
    return 1
  fi
  blk=$(find_block "$INDEX" "<div class=\"${cls}\">")
  if [ -z "$blk" ]; then
    warn "no <div class=\"${cls}\"> block in index.html — cannot bootstrap ${nm} markers, skipping the region"
    return 1
  fi
  # markers wrap the block's INNER content, so generated entries/cards stay
  # inside the styled container (the rail line, the grid layout)
  set -- $blk
  if [ "$(($2 - $1))" -lt 2 ]; then
    warn "<div class=\"${cls}\"> block has no inner content — cannot bootstrap ${nm} markers, skipping the region"
    return 1
  fi
  insert_markers "$INDEX" "$TMPD/boot" $(($1 + 1)) $(($2 - 1)) "<!-- ${nm}:BEGIN -->" "<!-- ${nm}:END -->" \
    || { warn "${nm} bootstrap failed — skipping the region"; return 1; }
  cat "$TMPD/boot" > "$INDEX"
  if ! git -C "$SITE" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    warn "$SITE is not a git work tree — ${nm} markers written but not committed"
    return 0
  fi
  git -C "$SITE" add index.html
  if git -C "$SITE" commit -q -m "site: ${nm} region markers bootstrap (T99)" \
    -m "scripts/site-sync.sh (T99) req 4: markers wrapped around the existing <div class=\"${cls}\"> block verbatim — the block is the initial region content, deduped against TODO-derived entries by commit ref during generation."; then
    echo "site-sync: bootstrapped ${nm} markers ($(git -C "$SITE" rev-parse --short HEAD 2>/dev/null))"
  else
    warn "${nm} bootstrap commit failed (git identity?) — markers written but uncommitted"
  fi
  return 0
}

# region_extract SRC OUT NAME — lines strictly between the region's markers
region_extract() { # src out name
  awk -v nm="$3" 'inblk { if ($0 ~ "^[[:space:]]*<!-- " nm ":END -->[[:space:]]*$") { inblk = 0; next } print; next }
    $0 ~ "^[[:space:]]*<!-- " nm ":BEGIN -->[[:space:]]*$" { inblk = 1 }' "$1" > "$2"
}

bootstrap_region TIMELINE tl || true
bootstrap_region FEATURES grid || true

# region_ok NAME — the region is generatable iff exactly one BEGIN/END pair
# exists, in order (malformed => warn + skip, best-effort, req 3)
region_ok() { # name
  local nb ne bl el
  nb=$(grep -cE "^[[:space:]]*<!-- $1:BEGIN -->[[:space:]]*\$" "$INDEX" 2>/dev/null) || nb=0
  ne=$(grep -cE "^[[:space:]]*<!-- $1:END -->[[:space:]]*\$" "$INDEX" 2>/dev/null) || ne=0
  [ "$nb" -eq 1 ] && [ "$ne" -eq 1 ] || return 1
  bl=$(grep -nE "^[[:space:]]*<!-- $1:BEGIN -->[[:space:]]*\$" "$INDEX" | head -1 | cut -d: -f1)
  el=$(grep -nE "^[[:space:]]*<!-- $1:END -->[[:space:]]*\$" "$INDEX" | head -1 | cut -d: -f1)
  [ -n "$bl" ] && [ -n "$el" ] && [ "$bl" -lt "$el" ]
}

# --- T99: regenerate the TIMELINE and FEATURES regions ------------------------
TL_STATE=skip; FT_STATE=skip
if region_ok TIMELINE; then
  region_extract "$INDEX" "$TMPD/tl-region" TIMELINE
  timeline_generate "$TMPD/tl-region" "$TMPD/tl-out" && TL_STATE=ok
else
  warn "no well-formed TIMELINE markers in index.html — timeline not updated"
fi
if region_ok FEATURES; then
  region_extract "$INDEX" "$TMPD/ft-region" FEATURES
  features_generate "$TMPD/ft-region" "$TMPD/ft-out" && FT_STATE=ok
else
  warn "no well-formed FEATURES markers in index.html — feature grid not updated"
fi

# --- splice: replace ONLY each marked region, in turn -------------------------
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
[ "$TL_STATE" = ok ] && splice_region "$TMP_NEW" "$TMP_NEW2" \
  "^[[:space:]]*<!-- TIMELINE:BEGIN -->[[:space:]]*\$" \
  "^[[:space:]]*<!-- TIMELINE:END -->[[:space:]]*\$" "$TMPD/tl-out" \
  || cp "$TMP_NEW" "$TMP_NEW2"
[ "$FT_STATE" = ok ] && splice_region "$TMP_NEW2" "$TMP_NEW3" \
  "^[[:space:]]*<!-- FEATURES:BEGIN -->[[:space:]]*\$" \
  "^[[:space:]]*<!-- FEATURES:END -->[[:space:]]*\$" "$TMPD/ft-out" \
  || cp "$TMP_NEW2" "$TMP_NEW3"

if cmp -s "$TMP_NEW3" "$INDEX"; then
  echo "site-sync: page unchanged — no commit"
  exit 0
fi
cat "$TMP_NEW3" > "$INDEX"   # in place: keeps the inode and the page's permissions

echo "site-sync: rewrote the marked regions of $INDEX (items $DONE/$TOTAL, tests $TEST_N, cycles $CYC_N, last eval $EVAL_D)"

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
  -m "scripts/site-sync.sh (T98+T99), deterministic — items $DONE/$TOTAL (TODO.md), tests $TEST_N ($TEST_SRC${TEST_REF:+, $TEST_REF}), cycles $CYC_N ($CYC_SRC), last eval $EVAL_D; last-5 landed: $LANDED_REFS; timeline: $TL_STATE (cap $TL_CAP), features: $FT_STATE"; then
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
