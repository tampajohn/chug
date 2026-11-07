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
# — site sync must never fail a cycle. Only the marker error (exit 3) and the
# T217 unreadable-inputs refusal (exit 4) exit nonzero. Plain `git push` only
# — never a force-push.
#
# T217: the sync FAILS CLOSED on unreadable repo inputs. The regions'
# fallback text is only honest for a genuinely readable-but-empty repo; when
# TODO.md, .chug/loopd, or git log cannot be read AT ALL, every number in the
# block is a fallback — byte-identical to a legitimately empty repo's first
# run, and indistinguishable from it by the values alone. Publishing those
# zeros with the same authority as real data is how 58c3a0b gutted chug.sh
# (a rogue runner whose repo path pointed at an empty or harvested directory
# — the T186 orphan class — still had commit+push reach). So BEFORE any
# region write, bootstrap, or commit: TODO.md unreadable OR .chug/loopd
# absent OR git log empty => write NOTHING, commit NOTHING, print ONE named
# error line and exit 4. The guard is global (req 2): all three regions read
# the same repo, so one check covers them all. It keys on INPUT READABILITY,
# never on the computed values. A bootstrap path for a genuinely empty repo
# (impossible in loop context — TODO.md always exists there) must pass the
# explicit --bootstrap flag.
#
# T240: the page also carries a RELEASE band — <!-- RELEASE:BEGIN/END --> inside
# a <section id="releases"> immediately before the stats section — regenerated
# from the LOCAL checkout's tags at sync time (req 2: wrap pushes tags before
# syncing, so a release wrap reflects the just-cut release). One card (version,
# tag date, GitHub release link) plus the top 3-5 release-note bullets from the
# tag's message, each under the T99 text rules (emphasis stripped, 160-byte
# word-truncated, HTML-escaped, backticks -> <code>). Fail-closed (T217 req 3):
# no stable v* tag -> the region is regenerated EMPTY and the sync continues —
# a pre-v0.1 repo is legal, and a missing tag must never render a fallback
# version with real data's authority. Markers + section + nav anchor are
# BOOTSTRAPPED here (T99 req 4) — no hand-built releases block exists to wrap —
# and malformed markers are warn + skip, best-effort, never a cycle failure.
#
# T241: the site gains a SECOND page — docs.html — fully machine-rendered (no
# marker regions at all). Sources, and the only home of the curated docs
# content (never the site repo): the README's `## Quickstart` section, which
# LEADS the page, and every runbooks/*.md file — one section each, byte-sorted
# (LC_ALL=C), runbooks/README.md (the corpus's own index) included like any
# other file. The head/style base is index.html's own <style> block, verbatim,
# so the two pages read as one site; a small ADDITIVE block covers the shapes
# the corpus renders that the base does not style (prose sub-headings, list
# bodies, the on-this-page index) — the base block itself is never modified.
# The render is deterministic and byte-stable (req 3): the page carries no
# clock, byte-identical inputs render byte-identical output, and an unchanged
# page never commits. docs.html is ADD/OVERWRITE (git add docs.html; the T26
# plain-push rule is unchanged) and is written in the SAME commit as
# index.html when either changed (req 5). Fail-closed (T217 req 3 applied to
# the docs corpus): no runbooks/ dir, no .md files in it, or no <style> base
# -> docs.html is NOT written or touched and the sync continues; the nav
# link + get-started pointer bootstrap (reqs 2+4: docs ⇄ home, one-line
# pointer on the main page's Quickstart section) is likewise skipped — never
# a link to a page that was not rendered. Each bootstrap is idempotent by its
# own needle and commits once.
#
# T327: the page also carries a COST region — <!-- COST:BEGIN/END --> around a
# live-stats section between RELEASE and STATS — the daily Langfuse spend of
# the chug project, baked into the static page at sync time (no API keys, no
# public Langfuse; the numbers are static once written). The collector runs
# the hand-verified ClickHouse recipe (docker exec langfuse-clickhouse-1 ...
# — Langfuse is local where the sync runs) through the INJECTABLE
# CHUG_COST_CH_CMD override (fixtures/tests never shell out to docker), and
# the sync renders the tile (yesterday's total, 7-day average, a pre-rendered
# 14-character sparkline — no JS — and the TRUE streak read from the newest
# cheap-exit disposition in .chug/loopd/loopd.log) plus the per-day table on
# docs.html. Fail-closed (the T217 letter): a collector that cannot reach
# ClickHouse, returns nothing, or returns rows that do not parse PRODUCES NO
# NUMBERS — a region with previously published content keeps it byte-identical
# (stale beats blank), a fresh region gets the explicit n/a placeholder, and
# a fabricated $0.00 is never rendered. The failure never fails the sync
# (req 5): every other region still lands. Markers are BOOTSTRAPPED here with
# the T240 mechanics (the script NEVER injects markers into a malformed page
# — malformed markers are warn + skip). The tile's "as of" stamp is the one
# clock the COST tile carries (spec req 3): the region regenerates on every
# sync run by design, unlike the clock-free STATS/TIMELINE/FEATURES regions.
#
# Usage:
#   scripts/site-sync.sh [--bootstrap] [SITE_DIR] [CHUG_ROOT]
#     --bootstrap  skip the T217 input guard (first-ever run on a genuinely
#                  empty repo; never needed in loop context, where TODO.md,
#                  .chug/loopd, and git history always exist)
#     SITE_DIR   site checkout (default $CHUG_SITE_DIR, else ~/workspace/chug-site)
#     CHUG_ROOT  chug repo the facts are read from (default: this repo)
# Env:
#   CHUG_SYNC_NOW=<date|iso>  pin the sync date for the commit message/subject
#                             (CHUG_DIGEST_NOW pattern; tests pin this)
#   CHUG_SITE_SYNC_NO_PUSH=1  skip the push (fixture tests, offline runs)
#   CHUG_COST_CH_CMD=<cmd>    T327: the collector command whose stdout is the
#                             ClickHouse recipe's TSV rows (date \t model \t
#                             count \t cost). Unset = the docker exec recipe
#                             verbatim (Langfuse-local hosts only).
set -u
export LC_ALL=C

# --bootstrap (T217): the guard's only escape, explicit on the command line.
# Recognized in ANY argument position (leading `--bootstrap SITE CHUG` or the
# test harness's `SITE CHUG --bootstrap`); never shifts the positionals.
BOOTSTRAP=0
ARGS=()
for a in "$@"; do
  case "$a" in
    --bootstrap) BOOTSTRAP=1 ;;
    *) ARGS+=("$a") ;;
  esac
done
set -- ${ARGS[@]+"${ARGS[@]}"}

SITE="${1:-${CHUG_SITE_DIR:-$HOME/workspace/chug-site}}"
CHUG="${2:-$(cd "$(dirname "$0")/.." && pwd)}"
SYNC_DATE="$(printf '%s' "${CHUG_SYNC_NOW:-$(date -u +%Y-%m-%d)}" | cut -c1-10)"
SYNC_TS="${CHUG_SYNC_NOW:-$(date -u +%Y-%m-%dT%H:%M:%SZ)}"   # T327: the COST tile's as-of stamp

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

# --- T217 req 1: input guard — refuse to publish before ANY region write ----
# Checked after the site-presence checks (a missing clone still has nothing
# to sync — that leg stays exit 0) and before the marker checks: an unreadable
# repo is the deeper fault, and the T99 marker bootstrap below must never run
# off unreadable inputs either (it commits to the site). Keyed on readability,
# never on the computed values — the fallback zeros are identical to a
# genuinely empty repo's first run. First unreadable input wins; ONE line,
# nonzero exit, the page untouched.
guard_refuse() { # <path> — one named-error line, exit 4, nothing written
  warn "refusing to publish: repo inputs unreadable at $1"
  exit 4
}
if [ "$BOOTSTRAP" != "1" ]; then
  [ -r "$CHUG/TODO.md" ] || guard_refuse "$CHUG/TODO.md"
  [ -d "$CHUG/.chug/loopd" ] || guard_refuse "$CHUG/.chug/loopd"
  [ -n "$(git --no-pager -C "$CHUG" log -1 --format=%H 2>/dev/null)" ] \
    || guard_refuse "$CHUG (git log empty)"
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

# --- fact 2 (req 1): current test count — newest commit message quoting a
# FULL-suite gate (T238: subset counts never masquerade as the suite total —
# the operator's "113 tests green" was ab93f94's mid-run census "nextest
# 113/1402", not a suite count). A candidate count inside a message is
# accepted only when it is provably the full suite:
#   (a) an N/N pair with EQUAL operands at or above GATE_FLOOR (500 — above
#       every package subset; the wrap gate's stable shapes: "nextest
#       1496/1496", release builds "1664/1664"), or
#   (b) a count the message labels full-suite ADJACENT to the number
#       ("full suite 40 unit" — the early-era prose shape).
# Everything else is skipped and the walk CONTINUES to older commits:
# mismatched mid-run censuses ("nextest 113/1402"), equal-operand package
# subsets ("site_sync 22/22" via a package nextest run — equal but far below
# the floor), and bare partial forms ("113 tests"). Failing all 200 messages
# the card prints the honest n/a — never a subset number. The label leg must
# sit next to the count, never anywhere in the message: ab93f94's own subject
# says "full-suite load" around a subset census, so a whole-message label
# check would re-publish exactly the bug this guard kills.
# Priority inside a message: `nextest N/M` (N = passed; one runner word such
# as "release" may sit between) > the labeled `full suite N unit` prose. A
# filtered run ("cargo test --bin chug image 16/16") matches none of these,
# deliberately: the card cites a full-suite gate count or nothing.
GATE_FLOOR=500
test_count() { # -> "<n>\t<short-ref>" ("" when no message quotes a full-suite gate)
  git --no-pager -C "$CHUG" log -200 --format='%x1e%h%x1f%B' 2>/dev/null \
    | awk -v RS="$(printf '\036')" -F"$(printf '\037')" -v floor="$GATE_FLOOR" '
      NF < 2 { next }   # text before the first record separator
      {
        # leg (a): every nextest N/M in the message, leftmost first — accept
        # the first pair the guard passes, keep walking within/older when none
        msg = $2; pos = 1
        while (match(substr(msg, pos), /nextest( [a-z][a-z-]*)? [0-9]+\/[0-9]+/)) {
          oend = RSTART + RLENGTH   # captured before the inner match clobbers it
          pair = substr(msg, pos + RSTART - 1, RLENGTH)
          if (match(pair, /[0-9]+\/[0-9]+$/)) {
            split(substr(pair, RSTART, RLENGTH), nm, "/")
            if (nm[1] + 0 == nm[2] + 0 && nm[1] + 0 >= floor) {
              print nm[1] "\t" $1; exit
            }
          }
          pos += oend - 1
        }
        # leg (b): the count itself labeled full-suite, adjacent
        if (match($2, /full[ -]suite[ :]*[0-9]+ unit/)) {
          lbl = substr($2, RSTART, RLENGTH)
          match(lbl, /[0-9]+/)
          print substr(lbl, RSTART, RLENGTH) "\t" $1; exit
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
    # T142 fix-up F1: count a cycle only on its LAST verdict line — never on
    # any matching line. Model text reaches a cycle log verbatim (stderr
    # deltas; the abort path puts model-written ledger text on stdout), so a
    # failed cycle whose bytes contain a forged `verdict: goal complete
    # (rc=0)` line made the old `grep -l` rename-count it: the marker was
    # renamed, not made unforgeable (validator probe published "cycles 2"
    # for one real OK cycle). The supervisor appends its stamp AFTER the
    # child is fully dead — both pipes closed — and writes nothing to the
    # log after it, so the last `verdict:` line of a stamp-supervised cycle
    # log is always the supervisor's and a forged one is necessarily
    # followed by the real stamp. Logs older than the stamp carry none and
    # count 0.
    n=0
    for f in "$CHUG"/.chug/loopd/cycle-*.log; do
      [ -f "$f" ] || continue
      case "$(grep 'verdict:' "$f" 2>/dev/null | tail -n 1)" in
        *'verdict: goal complete (rc=0)') n=$((n + 1)) ;;
      esac
    done
    printf '%s\tcycle logs whose final verdict stamp reached goal complete (.chug/loopd/cycle-*.log)' "$n"
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
# T216: a markdown-escaped pipe (`\|`) is cell CONTENT, not a delimiter —
# neutralized on a line copy BEFORE the split (T216 rule below).
FT_WHAT_CAP=160
features_rows() { # -> lines "<F-id>\t<status>\t<name>\t<what>"
  [ -f "$CHUG/FEATURES.md" ] || return 0
  # T216: a -Fpipe split cannot tell a markdown-escaped pipe from a real one,
  # so `\|` in a cell phantom-splits it: the F15 name cell
  # `CHUG_JUDGE=daemon\|http\|off` truncated at daemon-backslash — the LANDED
  # annotation beyond the split was never seen (card rendered QUEUED) and the
  # what cell was the fragment http-backslash. Park each escape on a line
  # COPY before the split (a \001 placeholder no markdown row carries), then
  # restore the literal pipe in the surviving cells — `\|` is markdown for a
  # literal pipe, so the HTML text renders the pipe.
  awk -v cap="$FT_WHAT_CAP" '
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
    BEGIN { ph = "\001" }
    /^[|]/ {
      line = $0
      gsub(/\\[|]/, ph, line)
      split(line, f, "|")
      id = f[2]; gsub(/[ \t]/, "", id)
      if (id !~ /^F[0-9]+$/) next
      namecell = f[3]; gsub(ph, "|", namecell)
      status = "queued"
      if (namecell ~ /~~/ || namecell ~ /LANDED/) status = "landed"
      name = namecell
      sub(/—.*/, "", name)              # name cell = text before the status annotation
      name = strip_md(name); gsub(/^[ \t]+|[ \t]+$/, "", name)
      if (name == "") next
      what = f[4]; gsub(ph, "|", what)
      print id "\t" status "\t" esc(name) "\t" prep(what, cap)
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

# body_ensure WHAT (card on stdin -> stdout) — T218: the card's <p> body is a
# FEATURES.md FACT, machine-owned like the badge, not curation. The text
# between the first <p…> open tag and its </p> is replaced with WHAT (the
# row's already-prep()d what-text: capped, emphasis-stripped, escaped, code
# spans), so a body written by any historically-buggy generator — the live
# F15 card fossilized the T216 `http\` fragment for HOURS — heals on the
# next sync, badge and body in one pass, no hand-deletion needed. The open
# tag is kept VERBATIM (its own class/attributes) plus the line's indent and
# any prefix, and everything after </p> is untouched. Single-line only, the
# same convention as badge_ensure's h3: a <p> whose </p> is not on its line
# is left alone (the generator never writes one, and rewriting across lines
# could eat a <ul> body — the T99 no-drop shape). WHAT rides ENVIRON rather
# than -v: -v processes escape sequences, so a what-text carrying a literal
# backslash (exactly the fossilized class) would be mangled.
body_ensure() { # what
  WHAT="$1" awk '
    !done && match($0, /<p[ \t][^>]*>|<p>/) {
      pre = substr($0, 1, RSTART - 1)
      open = substr($0, RSTART, RLENGTH)
      rest = substr($0, RSTART + RLENGTH)
      if (match(rest, /<\/p>/)) {
        $0 = pre open ENVIRON["WHAT"] substr(rest, RSTART)
        done = 1
      }
    }
    { print }'
}

# features_generate REGIONFILE OUTFILE — rebuild the region: every existing
# card keeps its position and markup (CSS classes/order preserved, req 2);
# cards whose name matches an F-item get their badge ensured AND their <p>
# body regenerated from the row's what-text (T218: name, badge, and body are
# all FEATURES.md facts — a stale body written by any historically-buggy
# generator heals on the next sync, so the operator never hand-deletes a
# card again); F-items with no card are appended as synthesized cards in
# FEATURES.md order. No card is
# ever dropped (flush_card prints every chunk — fix-up finding 1), so nothing
# outside FEATURES.md is lost.
features_generate() { # regionfile outfile
  local region="$1" out="$2" rowstmp rows openrows cardname cn fn short long line fid fstatus fname fwhat
  local matched="" sentinel match_id
  rowstmp="$(mktemp "${TMPDIR:-/tmp}/site-sync-frows.XXXXXX")"
  rows="$(mktemp "${TMPDIR:-/tmp}/site-sync-frows2.XXXXXX")"
  sentinel="$(printf '\036')"
  features_rows > "$rowstmp"
  if [ ! -s "$rowstmp" ]; then
    warn "no F-rows parsed from FEATURES.md — feature grid not updated"
    cat "$region" > "$out"; rm -f "$rowstmp" "$rows"; return 0
  fi
  # in-flight refinement: an OPEN TODO.md row referencing the F-id (F1 never
  # matches F13). T193: OPEN rows only — status cell `todo` or `in-progress`,
  # same -F'|' field conventions as todo_facts. Done rows keep their
  # F-references forever (T105's done row names F6; T180's names F12), so the
  # old whole-file grep rendered every feature a completed item ever touched
  # in-flight permanently. Landed wins (req 3): a row already classified
  # landed skips this refinement entirely — an open row referencing a LANDED
  # feature cannot demote it.
  openrows="$(mktemp "${TMPDIR:-/tmp}/site-sync-openrows.XXXXXX")"
  if [ -f "$CHUG/TODO.md" ]; then
    awk -F'|' '
      /^[|]/ {
        id = $2; gsub(/[ \t]/, "", id)
        if (id !~ /^T[0-9]+$/) next
        s = $6; gsub(/[ \t]/, "", s)
        if (s == "todo" || s == "in-progress") print
      }' "$CHUG/TODO.md" > "$openrows"
  fi
  while IFS="$(printf '\t')" read -r fid fstatus fname fwhat; do
    [ -n "$fid" ] || continue
    if [ "$fstatus" != "landed" ] && [ -s "$openrows" ] \
      && grep -qE "(^|[^A-Za-z0-9])F${fid#F}([^0-9]|$)" "$openrows"; then
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
                  match_id="$fid"; match_status="$fstatus"; match_what="$fwhat"
                  break
                fi
              done < "$rows"
            fi
            if [ -n "$match_id" ]; then
              matched="$matched $match_id"
              # T218: badge AND body in one pass — the h3 and the <p> are
              # disjoint lines, so the filter order does not matter
              badge_ensure "$TMP_CARD" "$match_status" | body_ensure "$match_what"
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
  rm -f "$rowstmp" "$rows" "$openrows"
}

# --- T240: RELEASE region — the latest release, always current ----------------
# Facts are the LOCAL checkout's tags at sync time (req 2): the newest STABLE
# v* tag by tag creation date (`git tag --sort=-creatordate`; pre-release
# channels — v0.18.0-rc1 — are out of scope), its tag date, and the top 3-5
# release-note bullets from the tag's message (%(contents) is the annotated
# tag's message — release notes are generated from semantic commits per the
# wrap doctrine — or the tagged commit's for a lightweight tag). Each bullet
# under the T99 text rules: markdown emphasis stripped, word-truncated at 160
# bytes (never splits a multibyte char), HTML-escaped, backtick pairs ->
# <code>. NO cat-file audit is needed (T99 req 5 analog): the tag comes from
# `git tag -l` of this same repo, so it exists by construction.
REL_BULLET_CAP=160                          # T99 what-text cap, per bullet
REL_BULLET_MAX=5                            # "top 3-5" — render at most five
REL_RELEASES_URL="https://github.com/tampajohn/chug/releases/tag"
RELEASE_TAG=""; RELEASE_DATE=""; RELEASE_BULLETS=""

release_facts() { # -> RELEASE_TAG / RELEASE_DATE / RELEASE_BULLETS (empty = no stable v* tag)
  local tag
  tag=$(git --no-pager -C "$CHUG" tag --sort=-creatordate --list 'v*' 2>/dev/null \
    | awk '/^v[0-9]+\.[0-9]+\.[0-9]+$/ { print; exit }')
  [ -n "$tag" ] || return 0                 # fail-closed (req 3): no tag -> nothing
  RELEASE_TAG="$tag"
  RELEASE_DATE=$(git --no-pager -C "$CHUG" for-each-ref "refs/tags/$tag" \
    --format='%(creatordate:short)' 2>/dev/null)
  RELEASE_BULLETS=$(git --no-pager -C "$CHUG" for-each-ref "refs/tags/$tag" \
    --format='%(contents)' 2>/dev/null | awk -v cap="$REL_BULLET_CAP" -v max="$REL_BULLET_MAX" '
    /^[[:space:]]*-[[:space:]]/ {
      if (n >= max) exit
      s = $0
      sub(/^[[:space:]]*-[[:space:]]+/, "", s)
      gsub(/~~/, "", s); gsub(/\*\*/, "", s)  # T99 strip_md: emphasis is markup, not text
      gsub(/^[ \t]+|[ \t]+$/, "", s)
      m = split(s, w, " "); out = ""; kept = 0
      for (i = 1; i <= m; i++) {
        cand = (out == "") ? w[i] : out " " w[i]
        if (length(cand) > cap) break          # byte cap, word-truncated — never splits a multibyte char
        out = cand; kept = i
      }
      if (kept < m) out = out "…"
      gsub(/&/, "\\&amp;", out); gsub(/</, "\\&lt;", out); gsub(/>/, "\\&gt;", out)
      k = split(out, seg, "`")                 # backtick pairs -> <code>…</code> (T99)
      if (k >= 3) {
        rebuilt = seg[1]
        for (j = 2; j + 1 <= k; j += 2) rebuilt = rebuilt "<code>" seg[j] "</code>" seg[j + 1]
        if (k % 2 == 0) rebuilt = rebuilt "`" seg[k]
        out = rebuilt
      }
      if (out == "") next
      n++
      print out
    }')
}

release_generate() { # outfile — empty for a tag-less repo (req 3: writes nothing)
  local out="$1" b
  {
    if [ -n "$RELEASE_TAG" ]; then
      # the site's own panel class — a full-width .stat card, no new CSS
      printf '<div class="stat"><b>%s</b><span>latest release — tagged %s · <a href="%s/%s">notes on GitHub</a></span></div>\n' \
        "$RELEASE_TAG" "$RELEASE_DATE" "$REL_RELEASES_URL" "$RELEASE_TAG"
      if [ -n "$RELEASE_BULLETS" ]; then
        printf '<ul>\n'
        printf '%s\n' "$RELEASE_BULLETS" | while IFS= read -r b; do
          [ -n "$b" ] || continue
          printf '  <li>%s</li>\n' "$b"
        done
        printf '</ul>\n'
      fi
    fi
  } > "$out"
}

bootstrap_release() { # T240 req 4: markers + section skeleton + nav anchor
  local nb ne nl sl indent kick
  nb=$(grep -cE "^[[:space:]]*<!-- RELEASE:BEGIN -->[[:space:]]*\$" "$INDEX" 2>/dev/null) || nb=0
  ne=$(grep -cE "^[[:space:]]*<!-- RELEASE:END -->[[:space:]]*\$" "$INDEX" 2>/dev/null) || ne=0
  if [ "$nb" -eq 1 ] && [ "$ne" -eq 1 ]; then return 0; fi
  if [ "$nb" -ne 0 ] || [ "$ne" -ne 0 ]; then
    warn "index.html has malformed RELEASE markers (found $nb/$ne, expected 0 or 1/1) — skipping the RELEASE region"
    return 1
  fi
  # no hand-built releases block exists to wrap (T99's wrap-the-existing-block
  # leg has no analog here): the section skeleton is machine-owned from birth
  if grep -q '<section id="releases"' "$INDEX"; then
    warn "a releases section exists in index.html without RELEASE markers — wrap its region with the markers by hand, skipping the RELEASE region"
    return 1
  fi
  nl=$(grep -n 'href="#stats">stats</a>' "$INDEX" 2>/dev/null | head -1 | cut -d: -f1)
  if [ -z "$nl" ]; then
    warn "no <a href=\"#stats\">stats</a> nav link in index.html — cannot place the releases anchor, skipping the RELEASE region"
    return 1
  fi
  sl=$(grep -nE '^<section id="stats"' "$INDEX" 2>/dev/null | head -1 | cut -d: -f1)
  if [ -z "$sl" ]; then
    warn "no <section id=\"stats\"> in index.html — cannot bootstrap the RELEASE region"
    return 1
  fi
  indent=$(sed -n "${nl}s/[^ ].*//p" "$INDEX")
  kick="latest release · synced from the repo's v* tags by scripts/site-sync.sh"
  # both insertions in ONE pass over the ORIGINAL line numbers: the nav anchor
  # rides directly before the stats link (the nav mirrors the section order,
  # req 4) and the section skeleton immediately before the stats section, so
  # the region sits between the hero and STATS (req 1)
  awk -v nl="$nl" -v sl="$sl" -v indent="$indent" -v kick="$kick" '
    NR == nl { print indent "<a href=\"#releases\">releases</a>" }
    NR == sl {
      print "<section id=\"releases\" class=\"live-stats\"><div class=\"wrap\">"
      print "  <p class=\"kicker\"><span class=\"hash\">##</span> " kick "</p>"
      print "<!-- RELEASE:BEGIN -->"
      print "<!-- RELEASE:END -->"
      print "</div></section>"
    }
    { print }' "$INDEX" > "$TMPD/boot-rel"
  cat "$TMPD/boot-rel" > "$INDEX" \
    || { warn "RELEASE bootstrap page write failed — index.html may be truncated, NOT committing"; return 1; }
  if ! git -C "$SITE" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    warn "$SITE is not a git work tree — RELEASE markers written but not committed"
    return 0
  fi
  git -C "$SITE" add index.html
  if git -C "$SITE" commit -q -m "site: RELEASE region markers bootstrap (T240)" \
    -m "scripts/site-sync.sh (T240) req 1+4: <section id=\"releases\"> inserted immediately before the stats section (the region sits between the hero and STATS) with its markers, plus the releases nav anchor directly before the stats link — the nav mirrors the section order. The region is empty until the next sync fills it."; then
    echo "site-sync: bootstrapped RELEASE markers + nav anchor ($(git -C "$SITE" rev-parse --short HEAD 2>/dev/null))"
  else
    warn "RELEASE bootstrap commit failed (git identity?) — markers written but uncommitted"
  fi
  return 0
}

# --- T327: COST region — the daily Langfuse spend, public --------------------
# The collector runs the hand-verified recipe verbatim (spec, 2026-11-06);
# CHUG_COST_CH_CMD replaces the invocation for fixtures/tests (never docker
# in tests) and anywhere else the sync runs off the Langfuse-local host.
#
# NULL tolerance is part of the recipe's own arithmetic: the live
# `GROUP BY 1,2` output carries `\N` (ClickHouse TSV NULL) model names for
# uncosted, non-generation observations, and a `\N` cost when a group has no
# recorded cost at all. The hand-verified per-day total sums every group's
# cost with NULL contributing nothing — so the parser reproduces exactly
# that: a `\N` cost adds 0 (never a fabricated figure, the recipe's own sum
# semantics), a `\N` model name lands outside the kimi/glm split (its calls
# and cost still count). Any OTHER unparseable cell is fail-closed: no
# numbers at all (stale/placeholder), never a partial render.
COST_CH_QUERY="SELECT toDate(start_time), provided_model_name, count(), sum(total_cost) FROM observations WHERE project_id='chug' AND start_time >= now() - INTERVAL 14 DAY GROUP BY 1,2"
COST_STALE_NOTE="keeping any previously published COST region content (stale beats blank, never a fabricated zero)"

# true_streak — the newest cheap-exit disposition's streak (goal note 6):
# the last .chug/loopd/loopd.log line carrying both the disposition tag and
# `streak=<n>`; "" when the log is unreadable or no disposition carries one
# — the tile omits the field, never fabricates.
true_streak() { # -> "<n>" | ""
  local log="$CHUG/.chug/loopd/loopd.log" line
  [ -r "$log" ] || return 0
  line=$(grep 'cheap-exit disposition' "$log" 2>/dev/null | grep -E 'streak=[0-9]+' | tail -n 1)
  [ -n "$line" ] || return 0
  printf '%s' "$line" | sed -n 's/.*streak=\([0-9][0-9]*\).*/\1/p'
}

# cost_collect OUTFILE — run the collector and canonicalize its rows into
# per-day lines `DATE \t CALLS \t KIMI \t GLM \t TOTAL` (ascending by date,
# raw sums at fixed precision). Exit 0 = usable data; nonzero = fail-closed
# (unreachable, empty, malformed — never a partial render).
cost_collect() { # outfile
  local out="$1" rc
  if [ -n "${CHUG_COST_CH_CMD:-}" ]; then
    bash -c "$CHUG_COST_CH_CMD" > "$TMPD/cost-raw" 2>/dev/null
  else
    docker exec langfuse-clickhouse-1 clickhouse-client -q "$COST_CH_QUERY" > "$TMPD/cost-raw" 2>/dev/null
  fi
  rc=$?
  if [ "$rc" -ne 0 ]; then
    warn "cost collector failed (Langfuse/ClickHouse unreachable, rc=$rc) — $COST_STALE_NOTE"
    return 1
  fi
  awk '
    BEGIN { FS = "\t" }
    NF < 4 { bad = 1; next }
    {
      d = $1
      if (d !~ /^[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]$/) { bad = 1; next }
      if ($3 !~ /^[0-9]+$/) { bad = 1; next }                    # count(): never NULL, never negative
      c = $4
      if (c == "\\N") { x = 0 }                                  # NULL cost: no recorded cost in the group
      else if (c ~ /^[-+]?[0-9]+([.][0-9]+)?([eE][-+]?[0-9]+)?$/) x = c + 0
      else { bad = 1; next }
      m = tolower($2)
      calls[d] += ($3 + 0)
      tot[d] += x
      if (index(m, "kimi")) kimi[d] += x
      else if (index(m, "glm")) glm[d] += x
      n++
    }
    END {
      if (bad || n == 0) exit 3
      nd = 0
      for (d in calls) ds[++nd] = d
      for (i = 2; i <= nd; i++) {          # insertion sort ascending — YYYY-MM-DD sorts chronologically
        v = ds[i]; j = i - 1
        while (j >= 1 && ds[j] > v) { ds[j + 1] = ds[j]; j-- }
        ds[j + 1] = v
      }
      for (i = 1; i <= nd; i++) {
        d = ds[i]
        printf "%s\t%d\t%.10f\t%.10f\t%.10f\n", d, calls[d] + 0, kimi[d] + 0, glm[d] + 0, tot[d] + 0
      }
    }
  ' "$TMPD/cost-raw" > "$out"
  rc=$?
  if [ "$rc" -ne 0 ]; then
    warn "cost collector output unusable (empty or malformed rows) — $COST_STALE_NOTE"
    return 1
  fi
  return 0
}

# cost_generate OUTFILE YJ STREAK TS — the index.html tile from the canonical
# per-day lines. YJ = yesterday's Julian day number (the tile window ends on
# the last COMPLETE day — a partial today never renders in the tile); the
# 14-slot sparkline covers [YJ-13 .. YJ] with observed absence as the lowest
# glyph (the collector reached ClickHouse: no rows that day is a fact, not a
# fallback). STREAK = the true_streak() value ("" omits the field); TS = the
# as-of stamp (spec req 3 — the one clock the tile carries).
cost_generate() { # outfile yj streak ts
  awk -v yj="$2" -v streak="$3" -v ts="$4" '
    function jdn(d,   y, m, dd, a, yy) {   # Fliegel–Van Flandern, the day_key idiom (no date(1) dialects)
      y = substr(d, 1, 4) + 0; m = substr(d, 6, 2) + 0; dd = substr(d, 9, 2) + 0
      a = int((14 - m) / 12); yy = y + 4800 - a
      return dd + int((153 * (m + 12 * a - 3) + 2) / 5) + yy * 365 \
        + int(yy / 4) - int(yy / 100) + int(yy / 400) - 32045
    }
    function jdn_date(j,   l, n, i, jj, dd, m, y) {   # the inverse — JDN back to YYYY-MM-DD
      l = j + 68569
      n = int((4 * l) / 146097)
      l = l - int((146097 * n + 3) / 4)
      i = int((4000 * (l + 1)) / 1461001)
      l = l - int((1461 * i) / 4) + 31
      jj = int((80 * l) / 2447)
      dd = l - int((2447 * jj) / 80)
      l = int(jj / 11)
      m = jj + 2 - 12 * l
      y = 100 * (n - 49) + i + l
      return sprintf("%04d-%02d-%02d", y, m, dd)
    }
    function money(x) {
      if (x > 0 && x < 0.005) return sprintf("$%.6f", x)   # near-zero stays legible
      return sprintf("$%.2f", x)
    }
    function glyph(v, mx,   lv) {              # the pre-rendered-character sparkline idiom — no JS
      lv = 1
      if (mx > 0) {
        lv = 1 + int((v / mx) * 7 + 0.5)
        if (lv > 8) lv = 8
        if (lv < 1) lv = 1
      }
      if (lv >= 8) return "█"
      if (lv == 7) return "▇"
      if (lv == 6) return "▆"
      if (lv == 5) return "▅"
      if (lv == 4) return "▄"
      if (lv == 3) return "▃"
      if (lv == 2) return "▂"
      return "▁"
    }
    BEGIN { FS = "\t" }
    {
      j = jdn($1)
      calls[j] = $2 + 0; tot[j] = $5 + 0
    }
    END {
      yt = tot[yj] + 0
      s7 = 0
      for (k = 0; k < 7; k++) s7 += tot[yj - k] + 0
      mx = 0
      for (k = 0; k < 14; k++) { v = tot[yj - k] + 0; if (v > mx) mx = v }
      spark = ""
      for (k = 13; k >= 0; k--) spark = spark glyph(tot[yj - k] + 0, mx)
      streaktxt = (streak == "") ? "" : sprintf(" · TRUE streak %d", streak + 0)
      printf "<div class=\"stats\">\n"
      printf "  <div class=\"stat\"><b>%s</b><span>Langfuse spend yesterday (%s UTC) · 7-day avg %s/day%s · as of %s</span></div>\n", \
        money(yt), jdn_date(yj), money(s7 / 7), streaktxt, ts
      printf "  <div class=\"stat\"><b>%s</b><span>14-day spend shape, oldest → newest — daily total from Langfuse (chug project); per-day table on the <a href=\"https://chug.sh/docs.html#cost\">docs page</a></span></div>\n", spark
      printf "</div>\n"
    }
  ' "$COST_DAYS" > "$1"
}

# cost_placeholder — the fail-closed shape (T217 letter): no numbers, ever.
# Timestamp-free on purpose — a persistently-down collector must not re-commit
# an unchanged placeholder every sync; the region regenerates with data (and
# its as-of stamp) when the collector recovers.
cost_placeholder() { # outfile
  cat <<'PH'
<div class="stats">
  <div class="stat"><b>n/a</b><span>Langfuse spend — no cost data at this sync (collector unreachable, empty, or malformed); previously published numbers are kept rather than replaced with zeros</span></div>
</div>
PH
}

# cost_docs_section OUTFILE — docs.html's per-day table (date, calls, kimi $,
# glm $, total $ — the spec's exact columns), newest first, capped at the 14
# days the collector covers. Rendered only from usable data (fail-closed):
# no collector output, no table — never a table of zeros.
cost_docs_section() { # outfile
  {
    printf '<section id="cost"><div class="wrap">\n'
    printf '  <p class="kicker"><span class="hash">##</span> cost · Langfuse daily spend, chug project — baked at sync time by scripts/site-sync.sh</p>\n'
    printf '  <div class="doctext">\n'
    printf '    <table>\n'
    printf '      <thead><tr><th>date</th><th>calls</th><th>kimi</th><th>glm</th><th>total</th></tr></thead>\n'
    printf '      <tbody>\n'
    awk -v cap=14 '
      function money(x) {
        if (x > 0 && x < 0.005) return sprintf("$%.6f", x)
        return sprintf("$%.2f", x)
      }
      { n++; d[n] = $1; c[n] = $2; k[n] = $3; g[n] = $4; t[n] = $5 }
      END {
        for (i = n; i >= 1 && i > n - cap; i--)
          printf "        <tr><td>%s</td><td>%d</td><td>%s</td><td>%s</td><td>%s</td></tr>\n", \
            d[i], c[i] + 0, money(k[i] + 0), money(g[i] + 0), money(t[i] + 0)
      }
    ' "$COST_DAYS"
    printf '      </tbody>\n'
    printf '    </table>\n'
    printf '  </div>\n</div></section>\n'
  } > "$1"
}

# bootstrap_cost — T327 req 2, the T240 mechanics verbatim: markers + section
# skeleton + nav anchor, one bootstrap commit; malformed markers (or a
# hand-built cost section without markers) are warn + skip, never a cycle
# failure. The section sits between RELEASE and STATS (the live-stats band
# order on the page) and the nav anchor rides directly before the stats link
# (the nav mirrors the section order, req 4).
bootstrap_cost() { # T327 req 2
  local nb ne nl sl indent
  nb=$(grep -cE "^[[:space:]]*<!-- COST:BEGIN -->[[:space:]]*\$" "$INDEX" 2>/dev/null) || nb=0
  ne=$(grep -cE "^[[:space:]]*<!-- COST:END -->[[:space:]]*\$" "$INDEX" 2>/dev/null) || ne=0
  if [ "$nb" -eq 1 ] && [ "$ne" -eq 1 ]; then return 0; fi
  if [ "$nb" -ne 0 ] || [ "$ne" -ne 0 ]; then
    warn "index.html has malformed COST markers (found $nb/$ne, expected 0 or 1/1) — skipping the COST region"
    return 1
  fi
  if grep -q '<section id="cost"' "$INDEX"; then
    warn "a cost section exists in index.html without COST markers — wrap its region with the markers by hand, skipping the COST region"
    return 1
  fi
  nl=$(grep -n 'href="#stats">stats</a>' "$INDEX" 2>/dev/null | head -1 | cut -d: -f1)
  if [ -z "$nl" ]; then
    warn "no <a href=\"#stats\">stats</a> nav link in index.html — cannot place the cost anchor, skipping the COST region"
    return 1
  fi
  sl=$(grep -nE '^<section id="stats"' "$INDEX" 2>/dev/null | head -1 | cut -d: -f1)
  if [ -z "$sl" ]; then
    warn "no <section id=\"stats\"> in index.html — cannot bootstrap the COST region"
    return 1
  fi
  indent=$(sed -n "${nl}s/[^ ].*//p" "$INDEX")
  awk -v nl="$nl" -v sl="$sl" -v indent="$indent" '
    NR == nl { print indent "<a href=\"#cost\">cost</a>" }
    NR == sl {
      print "<section id=\"cost\" class=\"live-stats\"><div class=\"wrap\">"
      print "  <p class=\"kicker\"><span class=\"hash\">##</span> cost · synced from Langfuse by scripts/site-sync.sh</p>"
      print "<!-- COST:BEGIN -->"
      print "<!-- COST:END -->"
      print "</div></section>"
    }
    { print }' "$INDEX" > "$TMPD/boot-cost"
  cat "$TMPD/boot-cost" > "$INDEX" \
    || { warn "COST bootstrap page write failed — index.html may be truncated, NOT committing"; return 1; }
  if ! git -C "$SITE" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    warn "$SITE is not a git work tree — COST markers written but not committed"
    return 0
  fi
  git -C "$SITE" add index.html
  if git -C "$SITE" commit -q -m "site: COST region markers bootstrap (T327)" \
    -m "scripts/site-sync.sh (T327) req 2: <section id=\"cost\"> inserted between the releases and stats sections with its markers, plus the cost nav anchor directly before the stats link (the nav mirrors the section order). The region is empty until the next sync fills it from Langfuse."; then
    echo "site-sync: bootstrapped COST markers + nav anchor ($(git -C "$SITE" rev-parse --short HEAD 2>/dev/null))"
  else
    warn "COST bootstrap commit failed (git identity?) — markers written but uncommitted"
  fi
  return 0
}

# --- T241: docs.html — the second page, fully machine-rendered ----------------
# Sources, and the only home of the curated docs content (never the site
# repo): the README's `## Quickstart` section (LEADS the page) and every
# runbooks/*.md file — one section each, byte-sorted, runbooks/README.md (the
# corpus's own index) included like any other file. The head/style base is
# index.html's own <style> block, verbatim, so the two pages read as one
# site. Deterministic and byte-stable (req 3): the page carries no clock, so
# byte-identical inputs render byte-identical output and an unchanged page
# never commits. docs.html is ADD/OVERWRITE (git add docs.html).
DOCS_GH_BLOB="https://github.com/tampajohn/chug/blob/main/"
DOCS_GH_TREE="https://github.com/tampajohn/chug/tree/main/runbooks"
DOCS_HOME="https://chug.sh/"
DOCS_STATE="skip"; DOCS_CHANGED=0; DOCS_NOTE="skipped"; DOCS_N=0; DOCS_QS=0
DOCS_POINTER="  <p class=\"lede flush\">The full runbook corpus lives on the <a href=\"https://chug.sh/docs.html\">docs page</a>, regenerated from the repo's runbooks/ at every sync.</p>"  # finding 3: no hardcoded runbook enumeration — the pointer cannot drift from the corpus

# readme_quickstart OUTFILE — the README's `## Quickstart` section body
# (heading dropped; ends at the next heading OUTSIDE any fence — the
# quickstart's bash block itself carries `#` comment lines). Empty output =
# no section (the caller warns; the page ships without it, best-effort).
readme_quickstart() { # outfile
  [ -f "$CHUG/README.md" ] || return 0
  awk '
    !started && fence != 1 && /^## Quickstart[[:space:]]*$/ { started = 1; next }
    started {
      if ($0 ~ /^[[:space:]]*```/) fence = !fence
      else if (fence != 1 && $0 ~ /^#{1,6}[[:space:]]/) exit
      print
    }
  ' "$CHUG/README.md"
}

# docs_md FILE MODE HEADING-OFFSET REL-BASE — the markdown renderer, one
# program, two modes. title: the first H1's inline-rendered text (for the
# on-this-page index). body: the corpus subset, line-oriented and
# deterministic — fenced code -> pre.code (HTML-escaped verbatim), ATX
# headings -> h(level+OFFSET) clamped to 2..6 (a runbook's H1 becomes the
# section's h2), flat lists -> ul/ol (indented lines continue the item, a
# blank line closes the list), GFM pipe tables -> <table> (T244: a |-led
# run whose SECOND line is a |-only delimiter row renders as thead+tbody,
# th header cells and td body cells; a run with no delimiter keeps the
# pre.code fallback — zero information loss), paragraphs soft-joined like
# markdown. Table cells split on UNESCAPED pipes only: a markdown-escaped
# \| parks on a sentinel before the split and restores as a literal pipe
# INSIDE its cell (the T216 rule — a cell is never split on its own
# escapes). Inline, per the T99 text rules: HTML-escape FIRST, ~~
# strikethrough~~ markers stripped, backtick pairs parked OPAQUELY (T244:
# content moves to an indexed sentinel, so no later pass matches inside a
# code span — `*` globs never italicize, `a**b` never bolds), **bold** ->
# <strong>, *italic* -> <em>, [text](url) links — relative URLs rewrite
# onto the GitHub blob base (REL-BASE, e.g. runbooks/) so nothing on the
# rendered page dangles, and only https (same-page #anchors and
# site-absolute paths included) becomes an href: any other absolute URL
# degrades to its link text (the canvas serves https only).
docs_md() { # file mode hoff relbase
  awk -v mode="$2" -v hoff="$3" -v relbase="$4" -v ghref="$DOCS_GH_BLOB" '
    function esc(s) { gsub(/&/, "\\&amp;", s); gsub(/</, "\\&lt;", s); gsub(/>/, "\\&gt;", s); return s }
    function inline(s,   k, seg, j, out, b, m, p, txt, url, href, nc, idx, cspan) {
      s = esc(s)
      gsub(/~~/, "", s)
      # code spans park OPAQUELY (T244): content -> cspan[n], text carries
      # \001n\002 — no emphasis/link pass can match across the sentinels,
      # so span content is never re-scanned and never corrupts output.
      k = split(s, seg, "`")
      nc = 0
      if (k >= 3) {
        out = seg[1]
        for (j = 2; j + 1 <= k; j += 2) {
          nc++
          cspan[nc] = seg[j]
          out = out "\001" nc "\002" seg[j + 1]
        }
        if (k % 2 == 0) out = out "`" seg[k]
        s = out
      }
      while (match(s, /\*\*[^*]+\*\*/)) {
        b = substr(s, RSTART + 2, RLENGTH - 4)
        s = substr(s, 1, RSTART - 1) "<strong>" b "</strong>" substr(s, RSTART + RLENGTH)
      }
      while (match(s, /\*[^*[:space:]][^*]*\*/)) {
        b = substr(s, RSTART + 1, RLENGTH - 2)
        s = substr(s, 1, RSTART - 1) "<em>" b "</em>" substr(s, RSTART + RLENGTH)
      }
      while (match(s, /\[[^]]+\]\([^)]+\)/)) {
        m = substr(s, RSTART, RLENGTH)
        p = index(m, "](")
        txt = substr(m, 2, p - 2)
        url = substr(m, p + 2); sub(/\)$/, "", url)
        href = ""
        if (url ~ /^https:\/\// || url ~ /^\// || url ~ /^#/) href = url
        else if (url !~ /^[a-zA-Z][a-zA-Z0-9+.-]*:/) href = ghref relbase url
        if (href != "") {
          s = substr(s, 1, RSTART - 1) "<a href=\"" href "\">" txt "</a>" substr(s, RSTART + RLENGTH)
        } else {
          # not an https-grade href: the text stays, the markup goes
          s = substr(s, 1, RSTART - 1) txt substr(s, RSTART + RLENGTH)
        }
      }
      while (match(s, /\001[0-9]+\002/)) {
        idx = substr(s, RSTART + 1, RLENGTH - 2) + 0
        s = substr(s, 1, RSTART - 1) "<code>" cspan[idx] "</code>" substr(s, RSTART + RLENGTH)
      }
      return s
    }
    function is_delim(s) { # a GFM delimiter row: only | - : and blanks, at least one dash
      if (s !~ /^[[:space:]]*\|[-:|[:space:]]*$/) return 0
      return s ~ /-/
    }
    function trow(s, tag,   line, ph, m, cells, j, out, c, lo, hi) {
      ph = "\003"                          # \001/\002 belong to the inline() code parking
      line = s
      gsub(/\\[|]/, ph, line)              # T216: a markdown-escaped pipe is CELL CONTENT
      m = split(line, cells, "|")
      lo = 1; hi = m
      if (cells[1] == "") lo = 2           # drop the empty field of an optional leading pipe
      if (cells[m] == "") hi = m - 1       # drop the empty field of an optional trailing pipe
      out = ""
      for (j = lo; j <= hi; j++) {
        c = cells[j]
        gsub(ph, "|", c)                   # the escape restores INSIDE its cell
        gsub(/^[[:space:]]+/, "", c); gsub(/[[:space:]]+$/, "", c)
        out = out "<" tag ">" inline(c) "</" tag ">"
      }
      return out
    }
    function flushrun(   n, rl, i) {
      if (run == "") return
      n = split(run, rl, "\n")             # rl[n] is "" — run carries one \n per line
      if (n >= 3 && is_delim(rl[2])) {     # header, then a delimiter row: a real table
        print "<table>"
        print "<thead><tr>" trow(rl[1], "th") "</tr></thead>"
        print "<tbody>"
        for (i = 3; i <= n - 1; i++) print "<tr>" trow(rl[i], "td") "</tr>"
        print "</tbody>"
        print "</table>"
      } else {                             # no separator -> the verbatim pre.code fallback
        print "<pre class=\"code\">"
        printf "%s", esc(run)
        print "</pre>"
      }
      run = ""
    }
    function flushpara() { if (para) { print "<p>" inline(ptxt) "</p>"; para = 0; ptxt = "" } }
    function flushli()   { if (inli) { print "<li>" inline(li) "</li>"; inli = 0; li = "" } }
    function closelist() { flushli(); if (inlist != "") { print (inlist == "ul" ? "</ul>" : "</ol>"); inlist = "" } }
    function closeall()  { flushpara(); closelist() }
    BEGIN { infence = 0; inlist = ""; para = 0; inli = 0; done = 0; run = "" }
    mode == "title" {
      if (!done && infence == 0 && $0 ~ /^#[[:space:]]/) {
        t = $0; sub(/^#[[:space:]]+/, "", t); sub(/[[:space:]]+$/, "", t)
        print inline(t); done = 1; exit
      }
      if ($0 ~ /^[[:space:]]*```/) infence = !infence
      next
    }
    {
      if ($0 ~ /^[[:space:]]*```/) {
        flushrun()
        closeall()
        if (infence) { infence = 0; print "</pre>" } else { infence = 1; print "<pre class=\"code\">" }
        next
      }
      if (infence) { print esc($0); next }
      if ($0 ~ /^[[:space:]]*\|/) {
        closeall()
        run = run $0 "\n"                  # buffer the |-led run; the table decision
        next                               # happens once the run ends (flushrun)
      }
      flushrun()                           # any non-| line ends the run
      if ($0 ~ /^#{1,6}[[:space:]]/) {
        closeall()
        h = $0; n = 0
        while (substr(h, 1, 1) == "#") { n++; h = substr(h, 2) }
        sub(/^[[:space:]]+/, "", h); sub(/[[:space:]]+$/, "", h)
        lvl = n + hoff; if (lvl > 6) lvl = 6; if (lvl < 2) lvl = 2
        print "<h" lvl ">" inline(h) "</h" lvl ">"
        next
      }
      if ($0 ~ /^[[:space:]]*[-*][[:space:]]+/) {
        flushpara()
        if (inlist != "ul") { closelist(); print "<ul>"; inlist = "ul" }
        flushli()
        li = $0; sub(/^[[:space:]]*[-*][[:space:]]+/, "", li); inli = 1
        next
      }
      if ($0 ~ /^[[:space:]]*[0-9]+[.][[:space:]]+/) {
        flushpara()
        if (inlist != "ol") { closelist(); print "<ol>"; inlist = "ol" }
        flushli()
        li = $0; sub(/^[[:space:]]*[0-9]+[.][[:space:]]+/, "", li); inli = 1
        next
      }
      if ($0 ~ /^[[:space:]]*$/) { closeall(); next }
      t = $0
      if (inli) { sub(/^[[:space:]]+/, "", t); li = li " " t; next }
      sub(/^[[:space:]]+/, "", t)
      if (para) { ptxt = ptxt " " t } else { para = 1; ptxt = t }
      next
    }
    END {
      if (mode != "title") {
        flushrun()
        if (infence) print "</pre>"
        closeall()
      }
    }
  ' "$1"
}

# docs_generate — compose docs.html into $TMPD/docs-out. Sets DOCS_STATE
# (ok|skip), DOCS_CHANGED (vs the site's existing page) and DOCS_NOTE (the
# sync commit body + stdout). Fail-closed (T217 req 3 applied to the docs
# corpus): no runbooks/ dir, no .md files in it, or no <style> block in
# index.html -> nothing is written, the sync continues (req 5's
# missing-runbooks leg). T331: the page also renders docs/risk-gate.md as
# one section between the runbooks and the cost table — missing source
# renders the explicit placeholder in the slot (the T217 letter, the
# adopt_placeholder shape) and the sync continues.
docs_generate() {
  DOCS_STATE="skip"; DOCS_CHANGED=0; DOCS_NOTE="skipped"; DOCS_N=0; DOCS_QS=0
  if [ ! -d "$CHUG/runbooks" ]; then
    warn "no runbooks/ dir in the chug repo — docs.html not written, sync continues (fail-closed: the docs corpus is missing)"
    DOCS_NOTE="skipped (no runbooks/)"
    return 0
  fi
  awk '/<style>/{s=1} s{print} /<\/style>/{exit}' "$INDEX" > "$TMPD/docs-style" 2>/dev/null
  if [ ! -s "$TMPD/docs-style" ]; then
    warn "no <style> block in index.html — no shared style base, docs.html not written, sync continues"
    DOCS_NOTE="skipped (no style base)"
    return 0
  fi
  DOCS_FILES=$(find "$CHUG/runbooks" -maxdepth 1 -type f -name '*.md' 2>/dev/null | sort)
  if [ -z "$DOCS_FILES" ]; then
    warn "runbooks/ holds no .md files — docs.html not written, sync continues"
    DOCS_NOTE="skipped (no runbooks)"
    return 0
  fi
  : > "$TMPD/docs-qs"
  if [ -f "$CHUG/README.md" ]; then readme_quickstart > "$TMPD/docs-qs"; fi
  if [ -s "$TMPD/docs-qs" ]; then
    DOCS_QS=1
  else
    warn "no '## Quickstart' section in README.md — docs.html ships without its quickstart section"
  fi
  : > "$TMPD/docs-index-li"; : > "$TMPD/docs-sections"
  # T327: the per-day cost table — fresh from the collector when it produced
  # data; otherwise the PREVIOUSLY PUBLISHED table rides again byte-identical
  # (stale beats blank: a collector blip must never gut the published table —
  # the T217 class). A page with no published table stays without one.
  : > "$TMPD/docs-cost"
  if [ "$COST_HAVE" = 1 ]; then
    cost_docs_section "$TMPD/docs-cost"
  elif [ -f "$SITE/docs.html" ]; then
    sed -n '/^<section id="cost">/,/<\/section>$/p' "$SITE/docs.html" > "$TMPD/docs-cost" 2>/dev/null || :
    [ -s "$TMPD/docs-cost" ] && warn "no usable collector data this sync — docs.html keeps its previously published cost table (stale beats blank)"
  fi
  for f in $DOCS_FILES; do
    if [ ! -r "$f" ]; then warn "runbook not readable: $f — skipped (fail-closed)"; continue; fi
    b=$(basename "$f" .md)
    slug=$(printf '%s' "$b" | tr '[:upper:]' '[:lower:]' | tr -c '[:alnum:]' '-')
    title=$(docs_md "$f" title 1 "")
    [ -n "$title" ] || title="$b"
    printf '      <li><a href="#runbook-%s">%s</a></li>\n' "$slug" "$title" >> "$TMPD/docs-index-li"
    {
      printf '<section id="runbook-%s"><div class="wrap">\n' "$slug"
      printf '  <p class="kicker"><span class="hash">##</span> runbook · %s.md</p>\n' "$b"
      printf '  <div class="doctext">\n'
      docs_md "$f" body 1 "runbooks/"
      printf '  </div>\n</div></section>\n'
    } >> "$TMPD/docs-sections"
    DOCS_N=$((DOCS_N + 1))
  done
  if [ "$DOCS_N" -eq 0 ]; then
    warn "no readable runbooks/*.md — docs.html not written, sync continues"
    DOCS_NOTE="skipped (no runbooks)"
    return 0
  fi
  # T331: the risk-gate story section — docs/risk-gate.md rendered by the
  # SAME pipeline as the runbooks (docs_md: H1 -> section h2, relative links
  # rewritten onto the GitHub blob base under docs/). Fail-closed per the
  # T217 letter, in the adopt_placeholder shape: a missing or unreadable
  # source renders the explicit placeholder region in its slot — honest
  # absence, never fabricated content, never filler — and the sync
  # continues. BOTH shapes carry the section id, so the on-this-page index
  # is stable and the adopt page's safety-FAQ cross-link
  # (docs/adopt/faq.md -> https://chug.sh/docs.html#risk-gate) resolves on
  # every render.
  : > "$TMPD/docs-riskgate"
  RG_SRC="$CHUG/docs/risk-gate.md"
  RG_NOTE="risk-gate section rendered"
  if [ -r "$RG_SRC" ]; then
    RG_TITLE=$(docs_md "$RG_SRC" title 1 "")
    [ -n "$RG_TITLE" ] || RG_TITLE="Risk gate"
    {
      printf '<section id="risk-gate"><div class="wrap">\n'
      printf '  <p class="kicker"><span class="hash">##</span> risk gate · docs/risk-gate.md</p>\n'
      printf '  <div class="doctext">\n'
      docs_md "$RG_SRC" body 1 "docs/"
      printf '  </div>\n</div></section>\n'
    } >> "$TMPD/docs-riskgate"
  else
    warn "docs/risk-gate.md missing or unreadable — the risk-gate section renders its placeholder (the slot and anchor exist either way)"
    RG_TITLE="Risk gate"
    RG_NOTE="risk-gate placeholder (source missing)"
    {
      printf '<section id="risk-gate"><div class="wrap">\n'
      printf '  <p class="kicker"><span class="hash">##</span> risk gate · docs/risk-gate.md</p>\n'
      printf '  <h2>%s</h2>\n' "$RG_TITLE"
      printf '  <div class="doctext">\n'
      printf '    <p class="lede">No content yet — this section'"'"'s source (docs/risk-gate.md) is missing or unreadable in the repo, so nothing is rendered here. Add the file and the next sync fills this slot.</p>\n'
      printf '  </div>\n</div></section>\n'
    } >> "$TMPD/docs-riskgate"
  fi
  printf '      <li><a href="#risk-gate">%s</a></li>\n' "$RG_TITLE" >> "$TMPD/docs-index-li"
  {
    printf '<!doctype html>\n<html lang="en">\n<head>\n'
    printf '<meta charset="utf-8">\n'
    printf '<meta name="viewport" content="width=device-width, initial-scale=1">\n'
    printf '<title>chug — docs</title>\n'
    printf '<meta name="description" content="How to use chug effectively — the quickstart and every runbook, machine-rendered from the repo README and runbooks/ at every sync.">\n'
    printf '<link rel="canonical" href="https://chug.sh/docs.html">\n'
    printf '<meta name="theme-color" content="#0d1117">\n'
    printf '<link rel="icon" type="image/svg+xml" href="/assets/favicon.svg">\n'
    printf '<link rel="apple-touch-icon" href="/assets/apple-touch-icon.png">\n'
    cat "$TMPD/docs-style"
    cat <<'STYLE'
<style>
/* docs.html additions (T241) — the block above is index.html's own <style>,
   verbatim, and is never modified here; these few rules cover only the shapes
   the runbook corpus renders that the base does not style (prose
   sub-headings, list bodies, tables + strong/em — T244 — the on-this-page
   index). */
.doctext h3,.doctext h4{font-family:var(--mono);color:var(--text);line-height:1.4}
.doctext h3{font-size:15px;margin:26px 0 8px}
.doctext h4{font-size:13px;margin:20px 0 6px;color:var(--muted)}
.doctext ul,.doctext ol{color:var(--muted);max-width:var(--measure);margin:0 0 14px;padding-left:22px}
.doctext li{line-height:1.7;margin:0 0 6px}
.doctext li b,.doctext li strong{color:var(--text)}
.doctext p code,.doctext li code,.doctext td code{font-size:.88em;color:var(--text);background:var(--bg2);border:1px solid var(--border);border-radius:4px;padding:0 4px}
.doctext pre.code{margin:0 0 14px;max-width:var(--content)}
.doctext table{border-collapse:collapse;margin:0 0 14px;max-width:var(--measure);font-size:13.5px;line-height:1.6}
.doctext th,.doctext td{border:1px solid var(--border);padding:5px 9px;text-align:left;vertical-align:top}
.doctext th{color:var(--text);font-family:var(--mono);font-size:12px}
.docs-index{margin:26px 0 0;max-width:var(--content)}
.docs-index p{font-family:var(--mono);font-size:12px;letter-spacing:.16em;text-transform:uppercase;color:var(--accent);margin:0 0 10px}
.docs-index ul{list-style:none;margin:0;padding:0;max-width:none}
.docs-index li{margin:0 0 8px;line-height:1.6}
.docs-index a{font-family:var(--mono);font-size:13.5px}
</style>
STYLE
    printf '</head>\n<body>\n'
    printf '<nav class="nav"><div class="wrap nav-in">\n'
    printf '  <a class="brand" href="%s">chug<b>_</b></a>\n' "$DOCS_HOME"
    printf '  <div class="nav-links">\n'
    printf '    <a href="#quickstart">quickstart</a>\n'
    printf '    <a href="%s">runbooks/</a>\n' "$DOCS_GH_TREE"
    if [ "$ADOPT_STATE" = ok ] || [ "$ADOPT_LIVE" = 1 ]; then
      # the link follows the PAGE's existence on the site — rendered this
      # sync, or riding stale from an earlier one. It is never the reverse:
      # a blocked render (dangling anchor, hygiene hit) must not flap the
      # docs page — stale beats blank covers the link to the page too.
      printf '    <a href="https://chug.sh/adopt.html">adopt</a>\n'
    fi
    printf '  </div>\n'
    printf '  <a class="gh" href="%s">home</a>\n' "$DOCS_HOME"
    printf '</div></nav>\n'
    printf '<header class="hero" id="docs"><div class="wrap">\n'
    printf '  <p class="kicker"><span class="hash">##</span> docs · machine-rendered from the repo at every sync</p>\n'
    printf '  <h1>chug docs</h1>\n'
    printf '  <p class="promise">How to use chug effectively — the quickstart and every runbook on one page, never stale: rendered from README.md and runbooks/ by scripts/site-sync.sh at every wrap.</p>\n'
    printf '  <div class="docs-index">\n'
    printf '    <p>on this page</p>\n'
    printf '    <ul>\n'
    if [ "$DOCS_QS" -eq 1 ]; then
      printf '      <li><a href="#quickstart">Quickstart — install, build, first run</a></li>\n'
    fi
    cat "$TMPD/docs-index-li"
    if [ -s "$TMPD/docs-cost" ]; then
      printf '      <li><a href="#cost">Cost — daily Langfuse spend</a></li>\n'
    fi
    printf '    </ul>\n  </div>\n</div></header>\n'
    if [ "$DOCS_QS" -eq 1 ]; then
      printf '<section id="quickstart"><div class="wrap">\n'
      printf '  <p class="kicker"><span class="hash">##</span> 01 · quickstart</p>\n'
      printf '  <h2>Quickstart</h2>\n'
      printf "  <p class=\"lede\">The repo's own quickstart, verbatim from README.md — re-rendered at every sync.</p>\n"
      printf '  <div class="doctext">\n'
      docs_md "$TMPD/docs-qs" body 1 ""
      printf '  </div>\n</div></section>\n'
    fi
    cat "$TMPD/docs-sections"
    # T331: the risk-gate story — always a section (rendered body or the
    # explicit placeholder), after the runbooks and before the cost table
    # (T327 keeps the cost table at the page's end).
    cat "$TMPD/docs-riskgate"
    # T327: the per-day cost table rides at the page's end — the quickstart
    # still LEADS (the T241 doctrine) and the runbooks stay in corpus order.
    # Rendered from usable collector data, or kept from the published page
    # when the collector blipped (stale beats blank); never a table of zeros.
    if [ -s "$TMPD/docs-cost" ]; then
      cat "$TMPD/docs-cost"
    fi
    printf '<footer><div class="wrap">\n'
    printf '  chug docs — regenerated from the repo README quickstart and runbooks/ at every sync; never hand-edited.<br>\n'
    printf '  <a href="%s">home</a> · <a href="https://github.com/tampajohn/chug">source</a> · <a href="%s">README</a> · <a href="%s">runbooks/</a>\n' "$DOCS_HOME" "${DOCS_GH_BLOB}README.md" "$DOCS_GH_TREE"
    printf '</div>\n</footer>\n'
    printf '</body>\n</html>\n'
  } > "$TMPD/docs-out" || { warn "docs.html compose failed — skipping"; return 0; }
  DOCS_STATE="ok"
  if [ ! -f "$SITE/docs.html" ] || ! cmp -s "$TMPD/docs-out" "$SITE/docs.html"; then DOCS_CHANGED=1; fi
  if [ "$DOCS_QS" -eq 1 ]; then
    DOCS_NOTE="rendered $DOCS_N runbooks + quickstart"
  else
    DOCS_NOTE="rendered $DOCS_N runbooks (no README quickstart)"
  fi
  DOCS_NOTE="$DOCS_NOTE; $RG_NOTE"
  return 0
}

# ensure_docs_nav — T241 reqs 2+4: index.html gains the docs.html link as the
# LAST nav item (before the nav-links close) and the get-started section (the
# main page's Quickstart) gains the ONE-LINE pointer to docs.html, inserted at
# the section's end. Each idempotent by its OWN needle (the nav needle is
# page-wide; the pointer needle is scoped to the get-started section, so the
# nav link landing first never masks the pointer check). Both insertions in
# one pass over the ORIGINAL line numbers; one bootstrap commit, the T240
# pattern. Only called after docs_generate rendered a page — never a link to
# a page that is not rendered (the fail-closed leg, req 5).
ensure_docs_nav() {
  local navhave navopen navclose indent gsopen gsclose gshave do_nav do_gs
  navhave=$(grep -cF 'href="https://chug.sh/docs.html">docs</a>' "$INDEX" 2>/dev/null) || navhave=0
  navopen=$(grep -nF '<div class="nav-links">' "$INDEX" 2>/dev/null | head -1 | cut -d: -f1)
  navclose=""
  if [ -n "$navopen" ]; then
    navclose=$(tail -n +"$((navopen + 1))" "$INDEX" 2>/dev/null \
      | grep -nE '^[[:space:]]*</div>[[:space:]]*$' | head -1 | cut -d: -f1)
    [ -n "$navclose" ] && navclose=$((navopen + navclose))
    indent=$(sed -n "$((navopen + 1))s/[^ ].*//p" "$INDEX" 2>/dev/null)
  fi
  do_nav=0
  if [ "${navhave:-0}" -eq 0 ] && [ -n "$navclose" ]; then do_nav=1; fi
  gsopen=$(grep -n '<section id="get-started"' "$INDEX" 2>/dev/null | head -1 | cut -d: -f1)
  gsclose=""; gshave=0
  if [ -n "$gsopen" ]; then
    gsclose=$(tail -n +"$gsopen" "$INDEX" 2>/dev/null \
      | grep -nE '^</div></section>[[:space:]]*$' | head -1 | cut -d: -f1)
    [ -n "$gsclose" ] && gsclose=$((gsopen + gsclose - 1))
    if [ -n "$gsclose" ]; then
      gshave=$(sed -n "${gsopen},${gsclose}p" "$INDEX" 2>/dev/null | grep -cF 'chug.sh/docs.html') || gshave=0
    fi
  fi
  do_gs=0
  if [ "${gshave:-0}" -eq 0 ] && [ -n "$gsclose" ]; then do_gs=1; fi
  if [ "$do_nav" -eq 0 ] && [ "$do_gs" -eq 0 ]; then return 0; fi
  awk -v nc="$navclose" -v dn="$do_nav" -v gc="$gsclose" -v dg="$do_gs" \
      -v indent="${indent:-    }" -v pointer="$DOCS_POINTER" '
    function navlink(ind) { print ind "<a href=\"https://chug.sh/docs.html\">docs</a>" }
    { if (dn == 1 && NR == nc) navlink(indent)
      if (dg == 1 && NR == gc) print pointer
      print }' "$INDEX" > "$TMPD/docs-nav" \
    || { warn "docs nav bootstrap splice failed — skipping"; return 1; }
  cat "$TMPD/docs-nav" > "$INDEX" \
    || { warn "docs nav bootstrap page write failed — index.html may be truncated, NOT committing"; return 1; }
  if ! git -C "$SITE" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    warn "$SITE is not a git work tree — docs nav edits written but not committed"
    return 0
  fi
  git -C "$SITE" add index.html
  if git -C "$SITE" commit -q -m "site: docs page nav bootstrap (T241)" \
    -m "scripts/site-sync.sh (T241) reqs 2+4: index.html gains the docs.html link as the last nav item and the get-started section gains a one-line pointer to it — docs ⇄ home. docs.html itself is machine-rendered from runbooks/ + the README quickstart by this same sync; docs.html is never hand-edited."; then
    echo "site-sync: bootstrapped docs nav link + get-started pointer ($(git -C "$SITE" rev-parse --short HEAD 2>/dev/null))"
  else
    warn "docs nav bootstrap commit failed (git identity?) — edits written but uncommitted"
  fi
  return 0
}

# --- T329: adopt.html — the adopt page, machine-rendered from docs/adopt/ -----
# Training content was the adoption bottleneck (operator Q4: expand chug to
# the company), so the site gains a THIRD page — the front door that takes a
# curious engineer from "what is this" to "my first loop is running". The
# doctrine is T241's verbatim, with the adopt corpus's own shape:
#
# Content source of truth: the four curated markdown fragments under
# docs/adopt/ in the chug repo (quickstart, mental-model, first-feature, faq
# — the page's fixed section order), rendered by the docs_md renderer. The
# corpus's own README.md is the index and never renders; any OTHER unknown
# fragment is warned and skipped — the page renders exactly the four curated
# sections, never whatever lands in the directory (fail-closed by design: a
# stray note cannot leak onto the public site).
#
# Fail-closed per T217: a missing fragment renders an explicit placeholder
# region in its slot (honest absence, never fabricated content) and the sync
# continues; NO canonical fragments at all -> adopt.html is NOT written and
# the nav bootstrap is skipped (never a link to a page that was not
# rendered — the T241 leg). Deterministic and byte-stable (req 3): no clock,
# byte-identical inputs render byte-identical output, an unchanged page never
# commits. adopt.html is ADD/OVERWRITE (T241 req 3) written in the SAME
# commit as index.html/docs.html when either changed (req 5).
#
# Two publish gates run over the composed page before it may land, both
# fail-closed with the previously published page riding (stale beats blank —
# the T327 doctrine):
#   link check — every in-page anchor (href="#x") must resolve to an id="x"
#     on the page; the render emits no heading ids, so fragment links target
#     the four section anchors or nothing. A dangling anchor is broken
#     training content: not written this sync.
#   hygiene — the T249 internal-info redaction patterns, scanned line by
#     line over the render. The repo's own lint covers the tracked fragments;
#     this scan covers the rendered page in the SITE repo, which no chug-repo
#     lint reaches. A hit is reported as line:pattern (never the content).
ADOPT_DIR="$CHUG/docs/adopt"
ADOPT_STATE="skip"; ADOPT_CHANGED=0; ADOPT_NOTE="skipped"; ADOPT_N=0
ADOPT_LINK='href="https://chug.sh/adopt.html">adopt</a>'

# t249_scan FILE — the T249 redaction pattern classes over each line; prints
# "<line>:<pattern>" per hit, never the line content. Mirrors
# tests/internal_info_lint.rs exactly: word-bounded needles (BOTH ends
# non-word — ASCII alnum + underscore), the per-class case flags, and the
# class-8 host shape `[a-z0-9][a-z0-9.-]*\.internal\.com` (a [a-z0-9]
# somewhere in the consumed left run, non-word char after the TLD).
t249_scan() { # file
  awk '
    function isw(c) { return c ~ /[A-Za-z0-9_]/ }
    function wb(s, needle, ci,   hay, from, pos, start, before, after, n) {
      hay = ci ? tolower(s) : s
      if (ci) needle = tolower(needle)
      n = length(needle); from = 1
      while (from <= length(hay)) {
        pos = index(substr(hay, from), needle)
        if (pos == 0) return 0
        start = from + pos - 1
        before = start > 1 ? substr(hay, start - 1, 1) : ""
        after = start + n <= length(hay) ? substr(hay, start + n, 1) : ""
        if (!isw(before) && !isw(after)) return 1
        from = start + 1
      }
      return 0
    }
    function hostpat(s,   from, pos, dot, end, after, left, c, haslead) {
      from = 1
      while (from <= length(s)) {
        pos = index(substr(s, from), ".internal.com")
        if (pos == 0) return 0
        dot = from + pos - 1
        end = dot + 13
        after = end <= length(s) ? substr(s, end, 1) : ""
        left = dot; haslead = 0
        while (left > 1) {
          c = substr(s, left - 1, 1)
          if (c ~ /[a-z0-9.-]/) { if (c ~ /[a-z0-9]/) haslead = 1; left-- }
          else break
        }
        if ((after == "" || !isw(after)) && haslead) return 1
        from = dot + 1
      }
      return 0
    }
    {
      # the needles are composed from pieces at runtime — this script is a
      # tracked file and tests/internal_info_lint.rs scans every tracked
      # line, so no source line here may carry a whole redaction literal
      if (wb($0, "video" "amp", 1)) print NR ":org-name"
      if (wb($0, "V" "A", 0)) print NR ":fleet-abbrev"
      if (wb($0, "vamp" "flake", 1)) print NR ":monorepo-codename"
      if (wb($0, "cen" "tral", 1)) print NR ":monorepo-intermediate"
      if (wb($0, "internal-" "llm" "-proxy", 1)) print NR ":proxy-hostname"
      if (wb($0, "INTERNAL-" "MONO" "REPO", 0)) print NR ":monorepo-repo-caps"
      if (hostpat($0)) print NR ":internal-host"
    }
  ' "$1"
}

# adopt_placeholder OUTFILE SLUG NUM TITLE — the explicit missing-fragment
# region: the section slot exists, the anchor resolves, and the page says
# exactly what is absent and where to put it. Honest absence, never filler.
# APPENDS (the sections buffer is shared with the rendered sections).
adopt_placeholder() { # outfile slug num title
  {
    printf '<section id="%s"><div class="wrap">\n' "$2"
    printf '  <p class="kicker"><span class="hash">##</span> %s · %s</p>\n' "$3" "$2"
    printf '  <h2>%s</h2>\n' "$4"
    printf '  <div class="doctext">\n'
    printf '    <p class="lede">No content yet — this section'"'"'s fragment (docs/adopt/%s.md) is missing from the repo, so nothing is rendered here. Add the fragment and the next sync fills this slot.</p>\n' "$2"
    printf '  </div>\n</div></section>\n'
  } >> "$1"
}

# adopt_section STEM NUM TITLE — render one canonical section into the page
# buffers: a readable fragment renders through docs_md (its H1 is the
# section h2, its relative links rewrite onto the GitHub blob base), a
# missing one renders the placeholder. Both shapes carry the section id, so
# the on-this-page index is stable and every anchor resolves.
adopt_section() { # stem num title
  local stem="$1" num="$2" title="$3" f="$ADOPT_DIR/$1.md" label
  if [ -r "$f" ]; then
    label=$(docs_md "$f" title 1 "")
    [ -n "$label" ] || label="$title"
    printf '      <li><a href="#%s">%s</a></li>\n' "$stem" "$label" >> "$TMPD/adopt-index-li"
    {
      printf '<section id="%s"><div class="wrap">\n' "$stem"
      printf '  <p class="kicker"><span class="hash">##</span> %s · %s</p>\n' "$num" "$stem"
      printf '  <div class="doctext">\n'
      docs_md "$f" body 1 "docs/adopt/"
      printf '  </div>\n</div></section>\n'
    } >> "$TMPD/adopt-sections"
    ADOPT_N=$((ADOPT_N + 1)); ADOPT_HAVE=$((ADOPT_HAVE + 1))
  else
    printf '%s\n' "$stem" >> "$TMPD/adopt-missing"
    adopt_placeholder "$TMPD/adopt-sections" "$stem" "$num" "$title"
    printf '      <li><a href="#%s">%s</a></li>\n' "$stem" "$title" >> "$TMPD/adopt-index-li"
  fi
}

# adopt_generate — compose adopt.html into $TMPD/adopt-out. Sets ADOPT_STATE
# (ok|skip), ADOPT_CHANGED (vs the site's existing page), ADOPT_NOTE (the
# sync commit body + stdout) and ADOPT_LIVE (the page exists on the site —
# rendered this sync, or riding stale). Rendered FIRST in the sync flow, so
# the docs page can link it only when it actually rendered.
adopt_generate() {
  ADOPT_LIVE=0
  adopt_generate_core
  if [ "$ADOPT_STATE" = ok ]; then
    ADOPT_LIVE=1
  elif grep -qF '<link rel="canonical" href="https://chug.sh/adopt.html">' "$SITE/adopt.html" 2>/dev/null; then
    # a previously published page riding stale — the generator's own
    # canonical meta is the fingerprint; a stray operator-written file
    # without it is NOT a rendered page and earns no links anywhere
    ADOPT_LIVE=1
  fi
  return 0
}

adopt_generate_core() {
  ADOPT_STATE="skip"; ADOPT_CHANGED=0; ADOPT_NOTE="skipped"; ADOPT_N=0
  if [ ! -d "$ADOPT_DIR" ]; then
    warn "no docs/adopt/ dir in the chug repo — adopt.html not written, sync continues (fail-closed: the adopt corpus is missing)"
    ADOPT_NOTE="skipped (no docs/adopt/)"
    return 0
  fi
  awk '/<style>/{s=1} s{print} /<\/style>/{exit}' "$INDEX" > "$TMPD/adopt-style" 2>/dev/null
  if [ ! -s "$TMPD/adopt-style" ]; then
    warn "no <style> block in index.html — no shared style base, adopt.html not written, sync continues"
    ADOPT_NOTE="skipped (no style base)"
    return 0
  fi
  # the corpus walk: warn-and-skip anything that is not a canonical section
  # (the corpus's own README.md is the index and rides silently — it exists
  # for the repo, not the page)
  for f in $(find "$ADOPT_DIR" -maxdepth 1 -type f -name '*.md' 2>/dev/null | sort); do
    b=$(basename "$f" .md)
    [ "$b" = "README" ] && continue
    case " quickstart mental-model first-feature faq " in
      *" $b "*) ;;
      *) warn "unknown adopt fragment docs/adopt/$b.md — the page renders the four canonical sections only; skipped" ;;
    esac
  done
  : > "$TMPD/adopt-index-li"; : > "$TMPD/adopt-sections"; : > "$TMPD/adopt-missing"
  ADOPT_HAVE=0
  adopt_section quickstart 01 "Quickstart"
  adopt_section mental-model 02 "Mental model"
  adopt_section first-feature 03 "Your first feature"
  adopt_section faq 04 "FAQ"
  if [ "$ADOPT_HAVE" -eq 0 ]; then
    warn "no adopt fragments in docs/adopt/ (expected quickstart.md, mental-model.md, first-feature.md, faq.md) — adopt.html not written, sync continues (fail-closed: the adopt corpus is empty)"
    ADOPT_NOTE="skipped (no fragments)"
    return 0
  fi
  {
    printf '<!doctype html>\n<html lang="en">\n<head>\n'
    printf '<meta charset="utf-8">\n'
    printf '<meta name="viewport" content="width=device-width, initial-scale=1">\n'
    printf '<title>chug — adopt</title>\n'
    printf '<meta name="description" content="Adopt chug: install it, understand the loop, file your first feature, get answers — machine-rendered from the repo docs/adopt/ fragments at every sync.">\n'
    printf '<link rel="canonical" href="https://chug.sh/adopt.html">\n'
    printf '<meta name="theme-color" content="#0d1117">\n'
    printf '<link rel="icon" type="image/svg+xml" href="/assets/favicon.svg">\n'
    printf '<link rel="apple-touch-icon" href="/assets/apple-touch-icon.png">\n'
    cat "$TMPD/adopt-style"
    cat <<'STYLE'
<style>
/* adopt.html additions (T329) — the block above is index.html's own <style>,
   verbatim, and is never modified here; these few rules cover only the shapes
   the adopt corpus renders that the base does not style (prose
   sub-headings, list bodies, tables + strong/em, the on-this-page index). */
.doctext h3,.doctext h4{font-family:var(--mono);color:var(--text);line-height:1.4}
.doctext h3{font-size:15px;margin:26px 0 8px}
.doctext h4{font-size:13px;margin:20px 0 6px;color:var(--muted)}
.doctext ul,.doctext ol{color:var(--muted);max-width:var(--measure);margin:0 0 14px;padding-left:22px}
.doctext li{line-height:1.7;margin:0 0 6px}
.doctext li b,.doctext li strong{color:var(--text)}
.doctext p code,.doctext li code,.doctext td code{font-size:.88em;color:var(--text);background:var(--bg2);border:1px solid var(--border);border-radius:4px;padding:0 4px}
.doctext pre.code{margin:0 0 14px;max-width:var(--content)}
.doctext table{border-collapse:collapse;margin:0 0 14px;max-width:var(--measure);font-size:13.5px;line-height:1.6}
.doctext th,.doctext td{border:1px solid var(--border);padding:5px 9px;text-align:left;vertical-align:top}
.doctext th{color:var(--text);font-family:var(--mono);font-size:12px}
.docs-index{margin:26px 0 0;max-width:var(--content)}
.docs-index p{font-family:var(--mono);font-size:12px;letter-spacing:.16em;text-transform:uppercase;color:var(--accent);margin:0 0 10px}
.docs-index ul{list-style:none;margin:0;padding:0;max-width:none}
.docs-index li{margin:0 0 8px;line-height:1.6}
.docs-index a{font-family:var(--mono);font-size:13.5px}
</style>
STYLE
    printf '</head>\n<body>\n'
    printf '<nav class="nav"><div class="wrap nav-in">\n'
    printf '  <a class="brand" href="%s">chug<b>_</b></a>\n' "$DOCS_HOME"
    printf '  <div class="nav-links">\n'
    printf '    <a href="#quickstart">quickstart</a>\n'
    printf '    <a href="#mental-model">mental model</a>\n'
    printf '    <a href="#first-feature">first feature</a>\n'
    printf '    <a href="#faq">faq</a>\n'
    printf '  </div>\n'
    printf '  <a class="gh" href="%s">home</a>\n' "$DOCS_HOME"
    printf '</div></nav>\n'
    printf '<header class="hero" id="adopt"><div class="wrap">\n'
    printf '  <p class="kicker"><span class="hash">##</span> adopt · machine-rendered from the repo docs/adopt/ at every sync</p>\n'
    printf '  <h1>adopt chug</h1>\n'
    printf '  <p class="promise">From zero to your first loop — install it, understand it, hand it one small feature, get answers. The training page is never stale: rendered from docs/adopt/ by scripts/site-sync.sh at every wrap.</p>\n'
    printf '  <div class="docs-index">\n'
    printf '    <p>on this page</p>\n'
    printf '    <ul>\n'
    cat "$TMPD/adopt-index-li"
    printf '    </ul>\n  </div>\n</div></header>\n'
    cat "$TMPD/adopt-sections"
    printf '<footer><div class="wrap">\n'
    printf '  chug adopt — regenerated from the repo docs/adopt/ fragments at every sync; never hand-edited.<br>\n'
    printf '  <a href="%s">home</a> · <a href="https://github.com/tampajohn/chug">source</a> · <a href="%s">README</a> · <a href="%s">edit this page (docs/adopt/)</a>\n' "$DOCS_HOME" "${DOCS_GH_BLOB}README.md" "${DOCS_GH_TREE%runbooks}docs/adopt"
    printf '</div>\n</footer>\n'
    printf '</body>\n</html>\n'
  } > "$TMPD/adopt-out" || { warn "adopt.html compose failed — skipping"; return 0; }
  # publish gate 1 — the link check: every in-page anchor must resolve; a
  # dangling #href is broken training content, so the page does not land and
  # the previously published one rides (stale beats blank)
  dangling=$(awk '
    { s = $0
      while (match(s, /href="#[^"]+"/)) {
        hrefs[substr(s, RSTART + 7, RLENGTH - 8)] = 1
        s = substr(s, RSTART + RLENGTH)
      }
      s = $0
      while (match(s, /id="[^"]+"/)) {
        ids[substr(s, RSTART + 4, RLENGTH - 5)] = 1
        s = substr(s, RSTART + RLENGTH)
      }
    }
    END { for (h in hrefs) if (!(h in ids)) print h }
  ' "$TMPD/adopt-out" | sort)
  if [ -n "$dangling" ]; then
    warn "adopt.html would carry dangling in-page anchor(s): $(printf '%s' "$dangling" | tr '\n' ' ')— not written this sync (fail-closed); point fragment links at the section anchors"
    ADOPT_NOTE="skipped (dangling anchor)"
    return 0
  fi
  # publish gate 2 — the T249 hygiene scan over the composed page
  hits=$(t249_scan "$TMPD/adopt-out")
  if [ -n "$hits" ]; then
    warn "adopt.html would carry internal-info pattern hit(s) (the T249 redaction classes) — adopt.html not written this sync (fail-closed); the previously published page rides:"
    printf '%s\n' "$hits" | while IFS= read -r h; do
      warn "  adopt.html line ${h%%:*}: pattern ${h#*:} (line content not printed)"
    done
    ADOPT_NOTE="skipped (hygiene)"
    return 0
  fi
  ADOPT_STATE="ok"
  if [ ! -f "$SITE/adopt.html" ] || ! cmp -s "$TMPD/adopt-out" "$SITE/adopt.html"; then ADOPT_CHANGED=1; fi
  if [ -s "$TMPD/adopt-missing" ]; then
    ADOPT_NOTE="rendered $ADOPT_N sections ($(tr '\n' ' ' < "$TMPD/adopt-missing" | sed 's/ $//') missing)"
  else
    ADOPT_NOTE="rendered $ADOPT_N sections"
  fi
  return 0
}

# ensure_adopt_nav — T329: index.html gains the adopt.html link, directly
# BEFORE the docs link when that exists (adopt is the front door, docs the
# reference shelf), else as the last nav item (the T241 mechanics). Idempotent
# by its own needle; one bootstrap commit; only called after adopt_generate
# rendered a page — never a link to a page that is not rendered.
ensure_adopt_nav() {
  local navhave navopen navclose indent docsline do_nav
  navhave=$(grep -cF "$ADOPT_LINK" "$INDEX" 2>/dev/null) || navhave=0
  navopen=$(grep -nF '<div class="nav-links">' "$INDEX" 2>/dev/null | head -1 | cut -d: -f1)
  navclose=""; docsline=""
  if [ -n "$navopen" ]; then
    navclose=$(tail -n +"$((navopen + 1))" "$INDEX" 2>/dev/null \
      | grep -nE '^[[:space:]]*</div>[[:space:]]*$' | head -1 | cut -d: -f1)
    if [ -n "$navclose" ]; then
      navclose=$((navopen + navclose))
      docsline=$(sed -n "${navopen},${navclose}p" "$INDEX" 2>/dev/null \
        | grep -nF 'href="https://chug.sh/docs.html">docs</a>' | head -1 | cut -d: -f1)
      [ -n "$docsline" ] && docsline=$((navopen + docsline - 1))
    fi
    indent=$(sed -n "$((navopen + 1))s/[^ ].*//p" "$INDEX" 2>/dev/null)
  fi
  do_nav=0
  if [ "${navhave:-0}" -eq 0 ] && [ -n "$navclose" ]; then do_nav=1; fi
  if [ "$do_nav" -eq 0 ]; then return 0; fi
  awk -v at="${docsline:-$navclose}" -v indent="${indent:-    }" -v link="$ADOPT_LINK" '
    NR == at { print indent "<a " link }
    { print }' "$INDEX" > "$TMPD/adopt-nav" \
    || { warn "adopt nav bootstrap splice failed — skipping"; return 1; }
  cat "$TMPD/adopt-nav" > "$INDEX" \
    || { warn "adopt nav bootstrap page write failed — index.html may be truncated, NOT committing"; return 1; }
  if ! git -C "$SITE" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    warn "$SITE is not a git work tree — adopt nav edits written but not committed"
    return 0
  fi
  git -C "$SITE" add index.html
  if git -C "$SITE" commit -q -m "site: adopt page nav bootstrap (T329)" \
    -m "scripts/site-sync.sh (T329): index.html gains the adopt.html link, directly before the docs link (adopt is the front door, docs the reference shelf). adopt.html itself is machine-rendered from the repo docs/adopt/ fragments by this same sync; adopt.html is never hand-edited."; then
    echo "site-sync: bootstrapped adopt nav link ($(git -C "$SITE" rev-parse --short HEAD 2>/dev/null))"
  else
    warn "adopt nav bootstrap commit failed (git identity?) — edits written but uncommitted"
  fi
  return 0
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
release_facts                            # T240: RELEASE_TAG/DATE/BULLETS ("" = no stable v* tag)

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
TMP_NEW4="$TMPD/splice-release"
TMP_NEW5="$TMPD/splice-cost"               # T327: the COST region splices last
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
  cat "$TMPD/boot" > "$INDEX" \
    || { warn "${nm} bootstrap page write failed — index.html may be truncated, NOT committing"; return 1; }
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
bootstrap_release || true
bootstrap_cost || true

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

# --- T327: collect the cost facts once, before the docs render (both consume).
# The collector runs only when something can render it (the COST region's
# markers or the docs corpus) — never a Langfuse probe on a page that cannot
# carry the numbers. YJ, yesterday's Julian day number, comes from the sync
# clock (the same CHUG_SYNC_NOW the tests pin); an unparseable clock leaves
# the tile window unknown and the region fails closed with it.
COST_DAYS="$TMPD/cost-days"; : > "$COST_DAYS"
COST_HAVE=0; COST_STATE=skip
if region_ok COST || [ -d "$CHUG/runbooks" ]; then
  if cost_collect "$COST_DAYS"; then COST_HAVE=1; fi
fi
YJ="$(awk -v d="$SYNC_DATE" 'BEGIN {
  if (d !~ /^[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]$/) exit 1
  y = substr(d, 1, 4) + 0; m = substr(d, 6, 2) + 0; dd = substr(d, 9, 2) + 0
  a = int((14 - m) / 12); yy = y + 4800 - a
  print dd + int((153 * (m + 12 * a - 3) + 2) / 5) + yy * 365 \
    + int(yy / 4) - int(yy / 100) + int(yy / 400) - 32045 - 1
}')"

# --- T329: adopt.html — the adopt page -----------------------------------------
# Rendered FIRST: the docs page's nav links it only when it actually rendered
# (never a link to a page that is not rendered — the fail-closed leg), and its
# own nav bootstrap runs with the docs one below.
adopt_generate
# --- T241: docs.html — the second page ----------------------------------------
# The page is rendered FIRST; only a rendered page earns its nav link + the
# get-started pointer (never a link to a page that is not rendered — the
# fail-closed missing-runbooks leg, req 5).
docs_generate
if [ "$DOCS_STATE" = ok ]; then ensure_docs_nav || true; fi
if [ "$ADOPT_STATE" = ok ] || [ "$ADOPT_LIVE" = 1 ]; then ensure_adopt_nav || true; fi

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
REL_STATE=skip
if region_ok RELEASE; then
  release_generate "$TMPD/rel-out" && REL_STATE=ok
else
  warn "no well-formed RELEASE markers in index.html — release not updated"
fi
# --- T327: regenerate the COST region — data, placeholder, or stale -----------
# ok          usable collector data -> the tile regenerates (with its as-of stamp)
# placeholder usable-collector-miss on an EMPTY region -> the explicit n/a shape
# stale       usable-collector-miss over PUBLISHED content -> untouched bytes
#             (stale beats blank — the T327 test letter); never a fabricated zero
# skip        no well-formed COST markers -> nothing touched
COST_HAS_CONTENT=0
if region_ok COST; then
  region_extract "$INDEX" "$TMPD/cost-region" COST
  [ -n "$(tr -d '[:space:]' < "$TMPD/cost-region")" ] && COST_HAS_CONTENT=1
  if [ "$COST_HAVE" = 1 ] && [ -n "$YJ" ]; then
    cost_generate "$TMPD/cost-out" "$YJ" "$(true_streak)" "$SYNC_TS" && COST_STATE=ok
  fi
  if [ "$COST_STATE" != ok ]; then
    if [ "$COST_HAS_CONTENT" = 1 ]; then
      COST_STATE=stale
      warn "cost region keeps its previously published content (no usable collector data this sync) — stale beats blank"
    else
      cost_placeholder > "$TMPD/cost-out" && COST_STATE=placeholder
    fi
  fi
else
  warn "no well-formed COST markers in index.html — cost region not updated"
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
[ "$REL_STATE" = ok ] && splice_region "$TMP_NEW3" "$TMP_NEW4" \
  "^[[:space:]]*<!-- RELEASE:BEGIN -->[[:space:]]*\$" \
  "^[[:space:]]*<!-- RELEASE:END -->[[:space:]]*\$" "$TMPD/rel-out" \
  || cp "$TMP_NEW3" "$TMP_NEW4"
# T327: the COST region splices only when the sync has something to write —
# data (ok) or the fresh-region placeholder. skip/stale pass the bytes through
# untouched.
if [ "$COST_STATE" = ok ] || [ "$COST_STATE" = placeholder ]; then
  splice_region "$TMP_NEW4" "$TMP_NEW5" \
    "^[[:space:]]*<!-- COST:BEGIN -->[[:space:]]*\$" \
    "^[[:space:]]*<!-- COST:END -->[[:space:]]*\$" "$TMPD/cost-out" \
    || cp "$TMP_NEW4" "$TMP_NEW5"
else
  cp "$TMP_NEW4" "$TMP_NEW5"
fi

if cmp -s "$TMP_NEW5" "$INDEX" && [ "$DOCS_CHANGED" != 1 ] && [ "$ADOPT_CHANGED" != 1 ]; then
  echo "site-sync: page unchanged — no commit"
  exit 0
fi
if ! cmp -s "$TMP_NEW5" "$INDEX"; then
  cat "$TMP_NEW5" > "$INDEX"   # in place: keeps the inode and the page's permissions
  echo "site-sync: rewrote the marked regions of $INDEX (items $DONE/$TOTAL, tests $TEST_N, cycles $CYC_N, last eval $EVAL_D, cost $COST_STATE)"
fi
# T241 req 3: docs.html is ADD/OVERWRITE — written in the same sync, committed
# in the same commit as index.html when either changed (req 5)
if [ "$DOCS_CHANGED" = 1 ]; then
  if cat "$TMPD/docs-out" > "$SITE/docs.html"; then
    echo "site-sync: wrote $SITE/docs.html ($DOCS_NOTE)"
  else
    warn "docs.html write failed — not committing a stale render"
    DOCS_CHANGED=0
  fi
fi
# T329: adopt.html is ADD/OVERWRITE the same way — same commit when changed;
# a failed write never commits a partial render
if [ "$ADOPT_CHANGED" = 1 ]; then
  if cat "$TMPD/adopt-out" > "$SITE/adopt.html"; then
    echo "site-sync: wrote $SITE/adopt.html ($ADOPT_NOTE)"
  else
    warn "adopt.html write failed — not committing a stale render"
    ADOPT_CHANGED=0
  fi
fi

# --- commit + push (req 2): best-effort, never force, rejection warns + exit 0
if ! git -C "$SITE" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  warn "$SITE is not a git work tree — pages updated but not committed"
  exit 0
fi
if [ -z "$(git -C "$SITE" status --porcelain -- index.html docs.html adopt.html 2>/dev/null)" ]; then
  echo "site-sync: page change not visible to git — nothing to commit"
  exit 0
fi
git -C "$SITE" add index.html
if [ "$DOCS_CHANGED" = 1 ]; then git -C "$SITE" add docs.html; fi  # T241 finding 1: stage docs.html only when THIS sync rendered it — a fail-closed leg never sweeps a stray file, a failed write never commits a partial
if [ "$ADOPT_CHANGED" = 1 ]; then git -C "$SITE" add adopt.html; fi  # T329, same rule: stage adopt.html only when THIS sync rendered it
if ! git -C "$SITE" commit -q -m "site: stats sync $SYNC_DATE" \
  -m "scripts/site-sync.sh (T98+T99), deterministic — items $DONE/$TOTAL (TODO.md), tests $TEST_N ($TEST_SRC${TEST_REF:+, $TEST_REF}), cycles $CYC_N ($CYC_SRC), last eval $EVAL_D; last-5 landed: $LANDED_REFS; timeline: $TL_STATE (cap $TL_CAP), features: $FT_STATE; release: ${RELEASE_TAG:-none}; cost: $COST_STATE; docs: $DOCS_NOTE; adopt: $ADOPT_NOTE"; then
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
