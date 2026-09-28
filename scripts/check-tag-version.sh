#!/bin/sh
# check-tag-version.sh — T100 req 3: a release tag must match Cargo.toml.
#
# A `v*` tag cut on a commit whose Cargo.toml version disagrees produces a
# release whose name lies about its contents (the binary's --version and
# the tag diverge forever — a published tag is immutable history, never
# moved). This check runs BEFORE anything expensive:
#
#   - in the release workflow (.github/workflows/release.yml): the tag that
#     triggered the run (GITHUB_REF) is compared against the checked-out
#     Cargo.toml; a divergence fails the release before the build.
#   - at wrap (LOOP-SPEC Phase 3 tag doctrine): the loop verifies the tag
#     string it is ABOUT to cut, after bumping Cargo.toml and before
#     `git tag` — catching the divergence while it is still free to fix.
#
# Exit codes: 0 = tag matches; 1 = divergence or bad tag shape (the fix is
# named in the message: bump Cargo.toml or cut a NEW tag — never move a
# published one); 2 = usage/environment (no tag resolvable, Cargo.toml
# missing or unreadable, no version field found).
#
# Usage:
#   scripts/check-tag-version.sh [TAG] [REPO_ROOT]
#     TAG        explicit tag string (default: $GITHUB_REF, else
#                `git describe --tags --exact-match HEAD` in REPO_ROOT)
#     REPO_ROOT  checkout holding Cargo.toml (default: cwd)
# Env: GITHUB_REF=refs/tags/vX.Y.Z is honored when TAG is not given.
set -u
export LC_ALL=C

fail() { printf 'check-tag-version: %s\n' "$1" >&2; exit "${2:-1}"; }

ROOT="${2:-${CHUG_TAG_VERSION_ROOT:-$PWD}}"
CARGO_TOML="$ROOT/Cargo.toml"

# --- resolve the tag to check ------------------------------------------------
TAG="${1:-}"
if [ -z "$TAG" ] && [ -n "${GITHUB_REF:-}" ]; then
  TAG=${GITHUB_REF#refs/tags/}
  [ "$TAG" = "$GITHUB_REF" ] && TAG=""   # GITHUB_REF was not a tag ref
fi
if [ -z "$TAG" ] && command -v git >/dev/null 2>&1; then
  TAG=$(git -C "$ROOT" describe --tags --exact-match HEAD 2>/dev/null) || TAG=""
fi
[ -n "$TAG" ] || fail "no tag to check (pass one: $0 vX.Y.Z [root])" 2

# --- tag shape: v<major>.<minor>.<patch>, exactly ----------------------------
printf '%s' "$TAG" | grep -Eq '^v[0-9]+\.[0-9]+\.[0-9]+$' || \
  fail "tag '$TAG' is not v<major>.<minor>.<patch> (e.g. v0.2.0) — fix the tag shape before cutting it"

# --- Cargo.toml's [package] version ------------------------------------------
[ -f "$CARGO_TOML" ] || fail "Cargo.toml not found at $CARGO_TOML — run me from the repo root (or pass [REPO_ROOT])" 2
version=$(awk '
  /^\[package\]/  { inpkg = 1; next }
  /^\[/           { inpkg = 0 }
  inpkg && /^version[[:space:]]*=/ { sub(/^[^=]*=/, ""); gsub(/[[:space:]]*"/, ""); print; exit }
' "$CARGO_TOML")
[ -n "$version" ] || fail "no [package] version found in $CARGO_TOML — the manifest must carry version = \"X.Y.Z\"" 2

# --- the comparison -----------------------------------------------------------
if [ "v$version" = "$TAG" ]; then
  printf 'check-tag-version: ok — tag %s matches Cargo.toml version %s\n' "$TAG" "$version"
  exit 0
fi
fail "tag '$TAG' does not match Cargo.toml version $version (expected tag v$version) — \
bump Cargo.toml to the tag's version, or cut a NEW tag at the right commit; \
a published tag is never moved or re-tagged"
