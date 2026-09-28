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
set -u
export LC_ALL=C

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

# --- log the range, newest-first (git log order is deterministic) --------------
# `range` was resolved above: FROM..TO, or TO alone for a tagless first release.

feats=$(git -C "$ROOT" log --format='%h%x09%s' "$range" | while IFS="$(printf '\t')" read -r sha subject; do
  [ "$(classify "$subject")" = features ] || continue
  printf -- '- %s (`%s`)\n' "$subject" "$sha"
done)
fixes=$(git -C "$ROOT" log --format='%h%x09%s' "$range" | while IFS="$(printf '\t')" read -r sha subject; do
  [ "$(classify "$subject")" = fixes ] || continue
  printf -- '- %s (`%s`)\n' "$subject" "$sha"
done)
docs=$(git -C "$ROOT" log --format='%h%x09%s' "$range" | while IFS="$(printf '\t')" read -r sha subject; do
  [ "$(classify "$subject")" = docs ] || continue
  printf -- '- %s (`%s`)\n' "$subject" "$sha"
done)
chores=$(git -C "$ROOT" log --format='%h%x09%s' "$range" | while IFS="$(printf '\t')" read -r sha subject; do
  [ "$(classify "$subject")" = chore ] || continue
  printf -- '- %s (`%s`)\n' "$subject" "$sha"
done)

emit() { # $1 heading, $2 body
  [ -n "$2" ] || return 0
  printf '## %s\n\n%s\n' "$1" "$2"
}

{
  emit Features "$feats"
  emit Fixes    "$fixes"
  emit Docs     "$docs"
  emit Chore    "$chores"
} | sed -e '${/^$/d;}'   # drop the trailing blank line the last emit adds
