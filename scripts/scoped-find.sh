#!/bin/sh
# scoped-find.sh — T314: the canned scoped-find helper, the T293 6th-fire
# re-weigh's LAND verdict: the canned scoped shape is the DEFAULT reach for
# a locating probe (META-META-SPEC's scoped-walk rule).
#
# The repo-root recursive-walk 300s kill census's six fires share one
# failure mode — composing a repo-root walk at all. The sixth (the
# cycle-2729 eval boot) composed a prune over target/target-shared* but
# left `.git` unpruned and carried no bounded window, so the walk
# descended `.git/` and died at the cap after it had already printed the
# answer. This helper bakes the safe shape so it is REACHED instead of a
# hand-composed walk: prunes for target/, target-shared*, and .git, plus a
# -maxdepth window (default 4) — baked, never remembered. A positive tool,
# NOT a detector/gate: nothing greps probe text for walk shapes (the
# brittle-awk class stays rejected); the right probe is simply the easy
# one.
#
# Usage:
#   scripts/scoped-find.sh <root> [--maxdepth N] -- <find-args...>
#
# Composes (N defaults to 4):
#   find <root> -maxdepth <N> -path <root>/target -prune \
#     -o -path '<root>/target-shared*' -prune \
#     -o -path <root>/.git -prune -o <find-args...> -print
#
# The exact composed command is echoed to stderr before it runs (the probe
# stays self-describing) and the exit status is find's own. -maxdepth is a
# global option placed right after <root>, accepted by GNU and macOS BSD
# find alike (the T299 compose-time rule: no GNU-only flags).
set -u
export LC_ALL=C

usage() {
  printf 'usage: %s <root> [--maxdepth N] -- <find-args...>\n' "$0" >&2
  printf '  composes find <root> -maxdepth <N> -path <root>/target -prune \\\n' >&2
  printf '    -o -path "<root>/target-shared*" -prune -o -path <root>/.git -prune \\\n' >&2
  printf '    -o <find-args...> -print   (N defaults to 4; prunes + window are baked)\n' >&2
  exit 2
}

die() { printf 'scoped-find: %s\n' "$1" >&2; usage; }

# Render one argv element for the self-describing echo: bare when it is
# shell-safe, otherwise single-quoted with embedded quotes escaped the
# standard '\'' way — a copy-paste of the echoed line recomposes the exact
# same find invocation.
shq() {
  case $1 in
    *[!A-Za-z0-9_.,/:@%+=-]*)
      printf "'%s'" "$(printf '%s' "$1" | sed "s/'/'\\\\''/g")"
      ;;
    *) printf '%s' "$1" ;;
  esac
}

[ $# -ge 1 ] || usage
ROOT=$1
shift
case $ROOT in
  -*) usage ;;   # the root is a path, never a flag
esac
while :; do
  case $ROOT in
    */) ROOT=${ROOT%/} ;;   # `./` must compose `./target`, not `.//target`
    *) break ;;
  esac
done
[ -n "$ROOT" ] || ROOT=/

MAXDEPTH=4
saw_dd=0
while [ $# -gt 0 ]; do
  case $1 in
    --) saw_dd=1; shift; break ;;
    --maxdepth)
      [ $# -ge 2 ] || die '--maxdepth needs a depth argument'
      MAXDEPTH=$2; shift 2 ;;
    --maxdepth=*) MAXDEPTH=${1#--maxdepth=}; shift ;;
    -h|--help) usage ;;
    *) usage ;;   # the -- boundary is what keeps MY flags apart from find's
  esac
done
[ "$saw_dd" -eq 1 ] || usage
case $MAXDEPTH in
  ''|*[!0-9]*) die "maxdepth must be a non-negative integer, got '$MAXDEPTH'" ;;
esac
[ $# -gt 0 ] || die 'no find-args after -- (a bare -print walk is not a scoped probe)'

# The self-describing echo: the exact composed command, to stderr.
cmd="find $(shq "$ROOT") -maxdepth $MAXDEPTH -path $(shq "$ROOT/target") -prune"
cmd="$cmd -o -path $(shq "$ROOT/target-shared*") -prune -o -path $(shq "$ROOT/.git") -prune -o"
for arg in "$@"; do
  cmd="$cmd $(shq "$arg")"
done
cmd="$cmd -print"
printf '%s\n' "$cmd" >&2

# Run it: the prunes bound the traversal, -maxdepth bounds the window, and
# find's own exit status is ours.
set -- find "$ROOT" -maxdepth "$MAXDEPTH" \
  -path "$ROOT/target" -prune -o \
  -path "$ROOT/target-shared*" -prune -o \
  -path "$ROOT/.git" -prune -o \
  "$@" -print
exec "$@"
