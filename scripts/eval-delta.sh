#!/usr/bin/env bash
# eval-delta.sh — T260: the mechanical half of the Phase-1 state+delta read path.
#
# loopd.sh runs this beside eval-digest.sh before every cycle. It validates
# the evaluator's own state file (.chug/eval-state.md, written by the eval at
# wrap) and computes the delta since that state's marker (rows, deaths,
# changed files by class, cycle summaries — all mechanical, no LLM calls),
# writing .chug/eval-delta.md. The delta's `read-path:` verdict decides how
# the next evaluation reads:
#
#   read-path: STATE-HIT                    — read state + delta instead of
#                                             the full META-META-SPEC corpus
#   read-path: FULL-READ REQUIRED (<why>)   — the full corpus list, unchanged
#
# Fail-closed: every stale/invalid/absent input yields FULL-READ (reasons:
# state-missing, schema-mismatch, git-unavailable, marker-unreachable,
# marker-behind, state-drift-flagged, state-history-missing, splice-violated),
# and a missing or failed delta file is a full read too (loopd logs the
# nonzero exit and the cycle degrades to the old corpus re-read). TRIP evals
# (the T247 valve, a stale/due evaluation) are ALWAYS full reads regardless
# of this verdict — the state is never a trip eval's only input.
#
# The verbatim-splice pin (T192 discipline): the state's `## decisions` ring
# is history, not prose. The validator compares this run's state against the
# snapshot the previous run took (.chug/eval-state.prev.md) and accepts the
# rewrite only as a FIFO splice — carried decision lines byte-identical,
# rotation only from the OLDEST end, new entries appended. A paraphrased or
# middle-removed entry (summarization in disguise) fails the check and forces
# the next eval full. The snapshot is refreshed AFTER validation so each run
# compares against exactly the pre-rewrite bytes the previous run saw.
#
# Staleness rules (T260 req 2): the full corpus read happens on state-stale —
# marker behind > 3 evals (EVALUATION.md commits since the marker), schema
# change, the eval itself flagging state drift — measured here per cycle.
#
# Usage:
#   scripts/eval-delta.sh [ROOT]      # default ROOT: the repo (script/..)
# Env:
#   CHUG_DELTA_NOW=<iso>   pin the generated-at clock (reproducible output;
#                          the eval-digest.sh CHUG_DIGEST_NOW pattern)
set -u
export LC_ALL=C

ROOT="${1:-}"
if [ -z "$ROOT" ]; then
  ROOT="$(cd "$(dirname "$0")/.." && pwd)"
fi
if ! ROOT="$(cd "$ROOT" 2>/dev/null && pwd)"; then
  echo "eval-delta: not a directory: ${1:-<empty>}" >&2
  exit 2
fi
CHUG="$ROOT/.chug"
OUT="$CHUG/eval-delta.md"
STATE="$CHUG/eval-state.md"
PREV="$CHUG/eval-state.prev.md"
mkdir -p "$CHUG"

# --- constants (the pinned schema + thresholds) ------------------------------
SCHEMA_VERSION=1
MAX_DECISIONS=12          # the state's decisions ring: last 12, oldest first
MAX_EVALS_BEHIND=3        # marker behind more than this many evals -> stale

# --- clock (pinnable for reproducible output in tests) ----------------------
if [ -n "${CHUG_DELTA_NOW:-}" ]; then
  GEN_ISO="$CHUG_DELTA_NOW"
else
  GEN_ISO=$(date -u +%Y-%m-%dT%H:%M:%SZ)
fi

# file -> mtime epoch secs; 0 when unreadable (BSD stat first, GNU fallback).
mtime_of() {
  local m
  m=$(stat -f %m "$1" 2>/dev/null) || m=$(stat -c %Y "$1" 2>/dev/null) || m=0
  printf '%s' "${m:-0}"
}

# --- state field/section readers --------------------------------------------
# `key: value` lines at column 0; `#` comment lines ignored; "" when absent.
state_field() {
  awk -v key="$2" '
    /^#/ { next }
    {
      pre = key ": "
      if (index($0, pre) == 1) { print substr($0, length(pre) + 1); exit }
    }
  ' "$1" 2>/dev/null
}

# The `## decisions` ring: `- `-prefixed lines under that header, in order,
# until the next `## ` header or EOF.
state_decisions() {
  awk '
    /^## decisions/ { insec = 1; next }
    /^## /          { insec = 0 }
    insec && /^- /  { print }
  ' "$1" 2>/dev/null
}

# --- the splice verdict (verbatim-splice pin) --------------------------------
# Compares the current decisions ring against the previous run's snapshot.
# The ring is append-only history (T192 discipline, never summarization):
#   * carried lines must be byte-identical, in order, as the new list's prefix
#     (a suffix of the old list carried forward);
#   * while the ring is below the cap NOTHING may be dropped (there is room —
#     dropping is rewriting);
#   * at a full ring, rotation drops from the OLDEST end only, and at least
#     one line must survive verbatim (a rewrite that carries nothing reads as
#     summarization and forces the next eval full).
# Anything else prints `violated: ...`.
#
# (The file split uses FILENAME == ARGV[1], never the NR==FNR idiom: that
# idiom breaks when the FIRST file is empty — NR==FNR stays true through the
# second file, every current line lands in the previous set, and a legal
# append onto an empty previous ring reads as a spurious violation.)
splice_verdict() {
  awk -v maxd="$MAX_DECISIONS" '
    FILENAME == ARGV[1] { p[++m] = $0; next }
    { c[++k] = $0 }
    END {
      if (k > maxd) { printf "violated: decisions %d > %d (ring overfull)\n", k, maxd; exit }
      if (m == 0) { printf "ok (carried 0, rotated 0, appended %d)\n", k; exit }
      # largest j with c[1..j] == p[m-j+1..m] (the carried prefix)
      jmax = (m < k ? m : k)
      for (j = jmax; j >= 0; j--) {
        good = 1
        for (x = 1; x <= j; x++) {
          if (c[x] != p[m - j + x]) { good = 0; break }
        }
        if (good) break
      }
      if (k < maxd && j != m) {
        for (x = 1; x <= m && x <= k; x++) {
          if (c[x] != p[x]) {
            printf "violated: carried decision %d rewritten (summarization?)\n", x
            exit
          }
        }
        if (k < m) printf "violated: %d carried decision(s) dropped (the ring has room; dropping is rewriting)\n", m - k
        else printf "violated: carried history reordered or not carried verbatim\n"
        exit
      }
      if (k == maxd && j == 0) {
        printf "violated: nothing carried verbatim from the previous ring (summarization?)\n"
        exit
      }
      printf "ok (carried %d, rotated %d, appended %d)\n", j, m - j, k - j
    }
  ' <(state_decisions "$PREV") <(state_decisions "$STATE")
}

# --- git plumbing (all best-effort; a missing repo is a FULL reason) ---------
git_ok=no
if git -C "$ROOT" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  git_ok=yes
fi
g() { git -C "$ROOT" "$@" 2>/dev/null; }

VERDICT="STATE-HIT"
REASON=""
evals_behind=""
drift=""
state_line="state: (absent) | evals-behind n/a"
MARKER=""
splice_out=""

if [ ! -f "$STATE" ]; then
  VERDICT="FULL-READ REQUIRED"
  REASON="state-missing"
else
  # The state exists: render its own header line before any git-dependent
  # part (a git failure must not blank the state's own fields).
  state_line="state: eval-at $(state_field "$STATE" eval-at) | drift $(state_field "$STATE" state-drift) | evals-behind ?"
  schema=$(state_field "$STATE" schema)
  if [ "$schema" != "$SCHEMA_VERSION" ]; then
    VERDICT="FULL-READ REQUIRED"
    REASON="schema-mismatch (found: '${schema:-absent}', want: $SCHEMA_VERSION)"
  else
    missing=""
    for f in eval-commit eval-at state-drift fresh-input-last-eval cache-read-last-eval health open-threads pacing-streak; do
      [ -n "$(state_field "$STATE" "$f")" ] || missing="$missing $f"
    done
    if [ -n "$missing" ]; then
      VERDICT="FULL-READ REQUIRED"
      REASON="schema-mismatch (missing:${missing})"
    elif [ "$git_ok" != yes ]; then
      VERDICT="FULL-READ REQUIRED"
      REASON="git-unavailable (marker ancestry unverifiable)"
    else
      MARKER=$(state_field "$STATE" eval-commit)
      if ! g rev-parse --verify --quiet "${MARKER}^{commit}" >/dev/null; then
        VERDICT="FULL-READ REQUIRED"
        REASON="marker-unreachable (${MARKER} is not a commit in this repo)"
      elif ! g merge-base --is-ancestor "$MARKER" HEAD >/dev/null; then
        VERDICT="FULL-READ REQUIRED"
        REASON="marker-unreachable (${MARKER} is not an ancestor of HEAD)"
      else
        evals_behind=$(g rev-list --count "${MARKER}..HEAD" -- EVALUATION.md)
        [ -n "$evals_behind" ] || evals_behind=0
        drift=$(state_field "$STATE" state-drift)
        state_line="state: marker $(g log -1 --format=%h "$MARKER") | eval-at $(state_field "$STATE" eval-at) | evals-behind ${evals_behind} | drift ${drift}"
        if [ "$evals_behind" -gt "$MAX_EVALS_BEHIND" ]; then
          VERDICT="FULL-READ REQUIRED"
          REASON="marker-behind (${evals_behind} evals > ${MAX_EVALS_BEHIND})"
        elif [ "$drift" != "none" ]; then
          VERDICT="FULL-READ REQUIRED"
          REASON="state-drift-flagged (state-drift: ${drift})"
        elif [ ! -f "$PREV" ]; then
          VERDICT="FULL-READ REQUIRED"
          REASON="state-history-missing (no previous snapshot to splice-check against)"
        else
          sp="$(splice_verdict)"
          splice_out="$sp"
          case "$sp" in
            ok*) : ;;
            *) VERDICT="FULL-READ REQUIRED"; REASON="splice-violated (${sp})" ;;
          esac
        fi
      fi
    fi
  fi
  # Refresh the snapshot AFTER validation: the next run splice-checks the
  # rewrite against exactly these bytes. Best-effort (a read-only fs must
  # not kill the delta; the next run just reports state-history-missing).
  cp "$STATE" "$PREV" 2>/dev/null || true
fi

# --- the delta since the marker (computed for EITHER verdict) ----------------
DELTA_BODY=""
if [ "$git_ok" = yes ]; then
  if [ -z "$MARKER" ]; then
    # No usable state marker: the newest EVALUATION.md commit is the fallback
    # base (the same default loopd's routing walk uses); none -> repo start.
    MARKER=$(g log -1 --format=%H -- EVALUATION.md)
  fi
  if [ -n "$MARKER" ] && g rev-parse --verify --quiet "${MARKER}^{commit}" >/dev/null; then
    marker_short=$(g log -1 --format=%h "$MARKER")
    marker_subj=$(g log -1 --format=%s "$MARKER")
    marker_epoch=$(g log -1 --format=%ct "$MARKER")
    [ -n "$marker_epoch" ] || marker_epoch=0
    commits_since=$(g rev-list --count "${MARKER}..HEAD")
    [ -n "$commits_since" ] || commits_since=0

    # changed files by class (T258's bookkeeping set + everything else)
    work_n=0; book_n=0
    work_files=()
    while IFS= read -r f; do
      [ -n "$f" ] || continue
      case "$f" in
        TODO.md|EVALUATION.md|specs/*) book_n=$((book_n + 1)) ;;
        *) work_n=$((work_n + 1)); work_files+=("$f") ;;
      esac
    done < <(g diff --name-only "$MARKER" HEAD)
    work_list=$(printf '%s\n' "${work_files[@]+"${work_files[@]}"}" | head -12 | tr '\n' ' ' | sed 's/ $//')
    [ -n "$work_list" ] || work_list="none"
    work_more_txt=""
    if [ "$work_n" -gt 12 ]; then work_more_txt=" (+$((work_n - 12)) more)"; fi

    # FEATURES.md movement (META-META-SPEC's read-path block keys a
    # conditional Tier-1 re-read on it — so the fact must be explicit):
    # changed|unchanged since the marker.
    features_md="unchanged"
    if [ -n "$(g diff --name-only "$MARKER" HEAD -- FEATURES.md 2>/dev/null)" ]; then
      features_md="changed"
    fi

    # TODO rows since the marker: id+status set diff (added/closed/removed).
    # Two-file awk (never -v) so row text with backslashes survives verbatim;
    # a missing old TODO.md yields an empty old set (everything "added").
    OLD_TODO=$(g show "$MARKER:TODO.md" 2>/dev/null || true)
    rows_line=$(awk -F'|' '
      function collect(file, set,      line, c, id, st) {
        while ((getline line < file) > 0) {
          if (line !~ /^[|]/) continue
          if (split(line, c, "|") < 6) continue
          id = c[2]; gsub(/[ \t]/, "", id)
          if (id !~ /^T[0-9]+$/) continue
          st = c[6]; gsub(/[ \t]/, "", st)
          set[id] = st
        }
        close(file)
      }
      BEGIN {
        collect(ARGV[1], old); collect(ARGV[2], new)
        for (id in new) {
          if (!(id in old)) added = added (added == "" ? "" : ",") id
          else if (old[id] != "done" && new[id] == "done") closed = closed (closed == "" ? "" : ",") id
        }
        for (id in old) if (!(id in new)) removed = removed (removed == "" ? "" : ",") id
        printf "added %s | closed %s | removed %s", \
          (added == "" ? "none" : added), (closed == "" ? "none" : closed), \
          (removed == "" ? "none" : removed)
      }
    ' <(printf '%s' "$OLD_TODO") <(cat "$ROOT/TODO.md" 2>/dev/null || true) </dev/null 2>/dev/null) || rows_line="added ? | closed ? | removed ?"

    # cycle summaries: the wrap commits since the marker, NEWEST LAST —
    # git log emits newest-first, so the newest 8 are reversed for output.
    cycles=$(g log --format='  - %h %s' --no-decorate "${MARKER}..HEAD" -- EVALUATION.md | head -8 \
      | awk '{ l[NR] = $0 } END { for (i = NR; i >= 1; i--) print l[i] }')
    [ -n "$cycles" ] || cycles="  - (none)"

    # deaths + goal rejects in events files newer than the marker
    deaths_total=0; rejects_total=0
    death_classes=""; death_n=0
    for f in "$CHUG"/events*.jsonl; do
      [ -e "$f" ] || continue
      [ "$(mtime_of "$f")" -gt "$marker_epoch" ] || continue
      n=$(jq -Rrn '[inputs | fromjson? | select(.type=="abort")] | length' "$f" 2>/dev/null) || n=0
      deaths_total=$((deaths_total + ${n:-0}))
      cls=$(jq -Rrn '[inputs | fromjson? | select(.type=="abort") | ((.reason // "?") | split("\n")[0] | gsub("[0-9]+"; "N"))]
                     | group_by(.) | map("\(.[0]) x\(length)") | join("; ")' "$f" 2>/dev/null) || cls=""
      if [ -n "$cls" ] && [ "$death_n" -lt 3 ]; then
        death_classes="${death_classes}${death_classes:+; }$cls"
        death_n=$((death_n + 1))
      fi
      r=$(jq -Rrn '[inputs | fromjson? | select(.type=="goal" and .outcome=="rejected")] | length' "$f" 2>/dev/null) || r=0
      rejects_total=$((rejects_total + ${r:-0}))
    done

    # T184 telemetry: the last cumulative token line of each LOOP-SPEC
    # stream (the orchestrator runs LOOP-SPEC every cycle; children carry
    # specs/t*.md), newest 3, so the fresh-input before/after the wrap
    # records in Outcomes is mechanically at hand. Collected in `ls -t`
    # order (newest-first), printed NEWEST LAST below.
    telem_nf=""
    loops_n=0
    for f in $(ls -t "$CHUG"/events*.jsonl 2>/dev/null | head -40); do
      is_loop=$(jq -Rr 'fromjson? | select(.type=="run_start") | (.spec // "") | select(endswith("LOOP-SPEC.md"))' "$f" 2>/dev/null | head -1)
      [ -n "$is_loop" ] || continue
      toks=$(jq -Rrn '[inputs | fromjson? | select(.type=="iteration" or .type=="usage")] | (last // empty)
                      | [(.input_tokens // 0), (.output_tokens // 0), (.cache_read_input_tokens // 0), (.cache_creation_input_tokens // 0)] | @tsv' "$f" 2>/dev/null) || toks=""
      [ -n "$toks" ] || toks=$(printf '0\t0\t0\t0')
      since=no
      [ "$(mtime_of "$f")" -gt "$marker_epoch" ] && since=yes
      name=$(basename "$f")
      telem_nf="${telem_nf}  - ${name}: fresh-input $(printf '%s' "$toks" | cut -f1) | cache-read $(printf '%s' "$toks" | cut -f3) | out $(printf '%s' "$toks" | cut -f2) | since-marker ${since}
"
      loops_n=$((loops_n + 1))
      [ "$loops_n" -ge 3 ] && break
    done
    # `ls -t` is newest-first; the delta's label (and reading order) is
    # newest LAST — reverse the collected lines verbatim.
    if [ -n "$telem_nf" ]; then
      telem=$(printf '%s' "$telem_nf" | awk '{ l[NR] = $0 } END { for (i = NR; i >= 1; i--) print l[i] }')
    else
      telem=""
    fi

    DELTA_BODY="## Delta since ${marker_short} (${marker_subj})

- commits since marker: ${commits_since}
- changed files: work ${work_n} / bookkeeping ${book_n}
- work files: ${work_list}${work_more_txt}
- features-md: ${features_md}
- TODO rows since marker: ${rows_line}
- child deaths since marker: ${deaths_total}${death_classes:+ — ${death_classes}}
- goal rejects since marker: ${rejects_total}
- cycle summaries (commits touching EVALUATION.md since marker, newest last):
${cycles}
- T184 fresh-input telemetry (last cumulative line per LOOP-SPEC stream, newest last):
${telem:-  - (no LOOP-SPEC streams found)}
- state-recorded last-eval: fresh-input $(state_field "$STATE" fresh-input-last-eval 2>/dev/null || echo n/a) | cache-read $(state_field "$STATE" cache-read-last-eval 2>/dev/null || echo n/a)
"
  else
    total_commits=$(g rev-list --count HEAD 2>/dev/null || echo "?")
    DELTA_BODY="## Delta (no marker — nothing evaluated yet)

- commits: ${total_commits} (no EVALUATION.md commit and no state marker to diff against)
"
  fi
fi

# --- write the delta ---------------------------------------------------------
{
  printf '# Eval delta — T260 state+delta read-path input (mechanical, loopd-built)\n\n'
  printf 'generated-at: %s | repo: %s | state schema: %s\n\n' "$GEN_ISO" "$ROOT" "$SCHEMA_VERSION"
  printf 'read-path: %s\n' "$VERDICT"
  if [ -n "$REASON" ]; then printf 'reason: %s\n' "$REASON"; fi
  printf '%s\n' "$state_line"
  if [ -n "$splice_out" ]; then printf 'splice-check: %s\n' "$splice_out"; fi
  printf '\n%s\n' "$DELTA_BODY"
  printf '## Read-path rules\n\n'
  printf -- '- STATE-HIT (non-trip eval): read .chug/eval-state.md + this delta INSTEAD of the\n'
  printf -- '  full META-META-SPEC corpus; rewrite the state at wrap (verbatim splice: carried\n'
  printf -- '  decision lines byte-identical — rotate from the oldest end, append new; never\n'
  printf -- '  summarize) and record the fresh-input before/after in Outcomes (T184 lines above).\n'
  printf -- '- FULL-READ REQUIRED: the full META-META-SPEC corpus list applies, unchanged.\n'
  printf -- '- TRIP evals (the T247 valve, a stale/due evaluation) are ALWAYS full reads\n'
  printf -- '  regardless of the verdict above — the state is never a trip eval only input.\n'
  printf -- '- A missing or failed delta is a full read (fail-closed).\n'
} > "$OUT"

# --- one-line stdout summary (loopd logs it) ---------------------------------
echo "eval-delta: wrote .chug/eval-delta.md (verdict=${VERDICT}${REASON:+: ${REASON}}, evals-behind=${evals_behind:-n/a}, generated $GEN_ISO)"
