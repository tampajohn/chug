#!/bin/sh
# release-notes.sh — T100 req 2: mechanical release notes from semantic commits.
#
# The repo's commit subjects are semantic and grep-able (`t99: …` feature
# impls, `fix:`/`feat:` conventional commits, `docs:`/`spec:`/`meta:`
# doctrine+docs, `todo:`/`eval:`/`merge:` bookkeeping) — so the notes are a
# deterministic script, not a model: zero LLM, byte-identical on identical
# inputs (the T46 digest pattern). Both the GitHub Release body and the
# annotated tag message are generated with this script (LOOP-SPEC Phase 3
# tag doctrine: tag message = the generated notes since the previous tag).
#
# Groups (heading order fixed; empty groups are omitted):
#   ## Features — feat:*, t<N>:*/T<N>:* (item impls)
#   ## Fixes    — fix:*
#   ## Docs     — docs:*, spec:*, specs:*, meta:*, self:*, readme:*
#   ## Chore    — chore:* and everything else (todo:/eval:/merge:/…)
#
# Usage:
#   scripts/release-notes.sh [TO_REF] [FROM_REF] [REPO_ROOT]
#     TO_REF     end of the range (default: HEAD; the workflow passes the tag)
#     FROM_REF   start (exclusive). Default: the newest v* tag reachable from
#                TO_REF's parent (`git describe --tags --abbrev=0 "TO^"`),
#                falling back to the root commit when no tag exists — the
#                first release then covers the whole history.
#     REPO_ROOT  git checkout (default: cwd)
# Env: CHUG_NOTES_NOW pins nothing — output has NO clock (unchanged inputs
# are byte-identical, pinned by tests/release_scripts.rs).
#
# T303 — GitHub caps release bodies at 125,000 chars; four consecutive tags
# failed the `gh release create` leg on over-long bodies (loop eval:/wrap
# subjects run multi-KB each; one window measured 1,070,737 bytes). Two
# deterministic guards keep the body publishable while preserving the
# byte-identical-on-identical-inputs pin:
#   1. each commit subject is truncated to SUBJECT_MAX bytes (+ "..." when
#      truncated) — ordinary subjects pass through untouched;
#   2. if the assembled body still exceeds BODY_CAP bytes, whole trailing
#      bullet lines are dropped (never a partial line) and ONE footer names
#      the omission count and the range — same git data in, same footer out.
set -u
export LC_ALL=C

SUBJECT_MAX=200    # per-subject byte budget (LC_ALL=C: chars == bytes)
BODY_CAP=120000    # hard body budget, under GitHub's 125,000-char limit

TO="${1:-HEAD}"
FROM="${2:-}"
ROOT="${3:-$PWD}"

die() { printf 'release-notes: %s\n' "$1" >&2; exit 2; }
command -v git >/dev/null 2>&1 || die "git not found on PATH"
git -C "$ROOT" rev-parse --git-dir >/dev/null 2>&1 || die "$ROOT is not a git checkout"
git -C "$ROOT" rev-parse --verify --quiet "$TO" >/dev/null || die "TO_REF '$TO' does not resolve in $ROOT"

# --- FROM: previous tag, else root commit ------------------------------------
range=""
if [ -z "$FROM" ]; then
  FROM=$(git -C "$ROOT" describe --tags --abbrev=0 "$TO^" 2>/dev/null) || FROM=""
fi
if [ -z "$FROM" ]; then
  # No previous tag: the FIRST release covers the whole history — git log on
  # TO alone (a FROM..TO range would exclude the root commit itself).
  range="$TO"
  FROM=""
fi
if [ -n "$FROM" ]; then
  git -C "$ROOT" rev-parse --verify --quiet "$FROM" >/dev/null || die "FROM_REF '$FROM' does not resolve in $ROOT"
  [ "$FROM" = "$TO" ] && exit 0   # empty range: no notes, nothing to lie about
  range="$FROM..$TO"
fi

# --- classify one subject ------------------------------------------------------
classify() {
  s=$(printf '%s' "$1" | tr '[:upper:]' '[:lower:]')
  case "$s" in
    fix:*|fix\(*|fix\))     echo fixes ;;
    feat:*|feat\(*|feat\))  echo features ;;
    t[0-9]*:*|t[0-9]*\(*|t[0-9]*\)) echo features ;;
    docs:*|readme:*|spec:*|specs:*|meta:*|self:*) echo docs ;;
    *)                      echo chore ;;
  esac
}

# --- T303 leg 1: cap one subject at SUBJECT_MAX bytes --------------------------
# POSIX printf's %.*s precision caps the bytes written (LC_ALL=C: chars ==
# bytes) — no external cut, no stray newline. The "..." marker names the cut.
trunc_subj() {
  if [ "${#1}" -gt "$SUBJECT_MAX" ]; then
    printf '%.*s...' "$SUBJECT_MAX" "$1"
  else
    printf '%s' "$1"
  fi
}

# --- log the range, newest-first (git log order is deterministic) --------------
# `range` was resolved above: FROM..TO, or TO alone for a tagless first release.

feats=$(git -C "$ROOT" log --format='%h%x09%s' "$range" | while IFS="$(printf '\t')" read -r sha subject; do
  [ "$(classify "$subject")" = features ] || continue
  printf -- '- %s (`%s`)\n' "$(trunc_subj "$subject")" "$sha"
done)
fixes=$(git -C "$ROOT" log --format='%h%x09%s' "$range" | while IFS="$(printf '\t')" read -r sha subject; do
  [ "$(classify "$subject")" = fixes ] || continue
  printf -- '- %s (`%s`)\n' "$(trunc_subj "$subject")" "$sha"
done)
docs=$(git -C "$ROOT" log --format='%h%x09%s' "$range" | while IFS="$(printf '\t')" read -r sha subject; do
  [ "$(classify "$subject")" = docs ] || continue
  printf -- '- %s (`%s`)\n' "$(trunc_subj "$subject")" "$sha"
done)
chores=$(git -C "$ROOT" log --format='%h%x09%s' "$range" | while IFS="$(printf '\t')" read -r sha subject; do
  [ "$(classify "$subject")" = chore ] || continue
  printf -- '- %s (`%s`)\n' "$(trunc_subj "$subject")" "$sha"
done)

emit() { # $1 heading, $2 body
  [ -n "$2" ] || return 0
  printf '## %s\n\n%s\n' "$1" "$2"
}

# --- T303 leg 2: hard cap the assembled body ------------------------------------
# Every commit in the range lands exactly one bullet, so the rev-list count is
# the bullet total the footer's N is computed from. awk buffers the body; when
# it fits the cap it is passed through byte-identical (the under-cap pin), else
# whole trailing lines are dropped down to a budget that reserves room for the
# longest possible footer (N ≤ total ⇒ N=total's footer is the longest), then
# any heading/blank lines the cut left dangling, then the footer itself.
total_commits=$(git -C "$ROOT" rev-list --count "$range")

cap_body() { # $1 total commits in range, $2 range string
  awk -v cap="$BODY_CAP" -v total="$1" -v range="$2" '
    { line[NR] = $0; size += length($0) + 1 }
    END {
      if (NR == 0 || size <= cap) {
        for (i = 1; i <= NR; i++) print line[i]
        exit
      }
      footer = "- ... (truncated: " total " more commits in " range "; full list: git log " range ")"
      budget = cap - length(footer) - 1
      kept = 0; used = 0
      for (i = 1; i <= NR; i++) {
        if (used + length(line[i]) + 1 > budget) break
        used += length(line[i]) + 1; kept = i
      }
      while (kept > 0 && (line[kept] == "" || line[kept] ~ /^## /)) kept--
      bullets = 0
      for (i = 1; i <= kept; i++) {
        print line[i]
        if (line[i] ~ /^- /) bullets++
      }
      printf "- ... (truncated: %d more commits in %s; full list: git log %s)\n", total - bullets, range, range
    }'
}

{
  emit Features "$feats"
  emit Fixes    "$fixes"
  emit Docs     "$docs"
  emit Chore    "$chores"
} | sed -e '${/^$/d;}' | cap_body "$total_commits" "$range"  # drop the trailing blank line the last emit adds
