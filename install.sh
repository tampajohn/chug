#!/bin/sh
# install.sh — T100 req 5: the `curl -fsSL https://chug.sh/install.sh | sh`
# one-liner (served by the chug.sh site repo). Detects the platform
# (uname -s/-m -> macos-arm64 / linux-x86_64 / linux-aarch64), downloads the
# LATEST GitHub-release tarball + its sha256, VERIFIES the checksum, installs
# the binary into ~/.local/bin (no sudo), and prints the chug banner + a
# quickstart pointer. POSIX sh — no bashisms, no sudo; every failure message
# names the fix.
#
# Env overrides (CI/mirrors/tests):
#   CHUG_INSTALL_REPO      GitHub slug        (default tampajohn/chug)
#   CHUG_INSTALL_OS        force the OS leg   (default: `uname -s`)
#   CHUG_INSTALL_MACH      force the arch leg (default: `uname -m`)
#   CHUG_INSTALL_PLATFORM  force the platform token (skips the uname mapping)
#   CHUG_INSTALL_DIR       install dir        (default $HOME/.local/bin)
#   CHUG_RELEASE_URL_BASE  asset base URL     (default …/releases/latest/download)
set -eu
export LC_ALL=C

REPO="${CHUG_INSTALL_REPO:-tampajohn/chug}"
install_dir="${CHUG_INSTALL_DIR:-$HOME/.local/bin}"

fail() { printf 'install.sh: %s\n' "$1" >&2; exit 1; }

banner() {
  printf '\n'
  printf '   ____ _   _  ___  _   _\n'
  printf '  / ___| | | |/ _ \\| | | |\n'
  printf ' | |   | |_| | | | | | | |\n'
  printf ' | |___|  _  | |_| | |_| |\n'
  printf '  \\____|_| |_|\\___/ \\___/\n'
  printf '\n'
}

# --- platform detect ----------------------------------------------------------
# Both legs are overridable (tests/CI) and the arch mapping is OS-aware: the
# same `uname -m` means different assets on different kernels — Linux reports
# BOTH `aarch64` and `arm64` for 64-bit ARM (and the workflow publishes only
# chug-linux-aarch64.tar.gz), while macOS reports `arm64`. Mapping arch before
# OS turned Linux+aarch64 hosts into the nonexistent `linux-arm64` asset and
# turned every one of them away at the gate (validator FINDING 1).
os="${CHUG_INSTALL_OS:-$(uname -s)}"
mach="${CHUG_INSTALL_MACH:-$(uname -m)}"
case "$os" in
  Darwin) osname=macos ;;
  Linux)  osname=linux ;;
  *) fail "unsupported OS '$os' (uname -s) — chug publishes macos-arm64, linux-x86_64 and linux-aarch64; on other platforms build from source: git clone \"https://github.com/$REPO\" && cd chug && cargo install --path ." ;;
esac
case "$osname/$mach" in
  macos/arm64)               arch=arm64 ;;
  linux/x86_64|linux/amd64)  arch=x86_64 ;;
  linux/aarch64|linux/arm64) arch=aarch64 ;; # `arm64`: the alias some Linux kernels report
  *) fail "unsupported architecture '$mach' (uname -m) for $osname — chug publishes macos-arm64, linux-x86_64 and linux-aarch64; build from source: git clone \"https://github.com/$REPO\" && cd chug && cargo install --path ." ;;
esac
platform="${CHUG_INSTALL_PLATFORM:-$osname-$arch}"
case "$platform" in
  macos-arm64|linux-x86_64|linux-aarch64) ;;
  *) fail "no prebuilt binary for platform '$platform' (os=$os arch=$mach) — supported: macos-arm64, linux-x86_64, linux-aarch64; build from source: git clone \"https://github.com/$REPO\" && cd chug && cargo install --path ." ;;
esac

# --- fetch helper: curl first, wget fallback ----------------------------------
fetch_to() { # $1 url, $2 dest
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL -o "$2" "$1" || return 1
  elif command -v wget >/dev/null 2>&1; then
    wget -qO "$2" "$1" || return 1
  else
    fail "neither curl nor wget found on PATH — install one (e.g. 'apt-get install curl' or 'brew install curl') and retry"
  fi
}

base="${CHUG_RELEASE_URL_BASE:-https://github.com/$REPO/releases/latest/download}"
tarball_url="$base/chug-$platform.tar.gz"
sha_url="$base/chug-$platform.tar.gz.sha256"

tmp=$(mktemp -d "${TMPDIR:-/tmp}/chug-install.XXXXXX") || fail "mktemp failed — is /tmp (or \$TMPDIR) writable?"
trap 'rm -rf "$tmp"' EXIT
trap 'rm -rf "$tmp"; exit 1' INT TERM

printf 'install.sh: downloading %s\n' "$tarball_url"
fetch_to "$tarball_url" "$tmp/chug.tar.gz" || fail "download failed: $tarball_url — if no release has been published yet, build from source: git clone \"https://github.com/$REPO\" && cd chug && cargo install --path . (releases appear as each vX.Y.Z tag lands)"
fetch_to "$sha_url" "$tmp/chug.sha256" || fail "download failed: $sha_url — the release tarball exists but its .sha256 sidecar is missing; re-check the release assets (or install manually: curl -L \"$tarball_url\" | tar xz)"

# --- verify the sha256 BEFORE anything runs ------------------------------------
want=$(awk '{print $1}' "$tmp/chug.sha256" | tr -d '[:space:]')
printf '%s' "$want" | grep -Eq '^[0-9a-fA-F]{64}$' || \
  fail "checksum file did not contain a sha256 (got '$(head -c 80 "$tmp/chug.sha256")') — the .sha256 sidecar is malformed; install manually: curl -L \"$tarball_url\" | tar xz"
if command -v sha256sum >/dev/null 2>&1; then
  got=$(sha256sum "$tmp/chug.tar.gz" | awk '{print $1}')
elif command -v shasum >/dev/null 2>&1; then
  got=$(shasum -a 256 "$tmp/chug.tar.gz" | awk '{print $1}')
elif command -v openssl >/dev/null 2>&1; then
  got=$(openssl dgst -sha256 -r "$tmp/chug.tar.gz" | awk '{print $1}')
else
  fail "no sha256 tool found (need sha256sum, shasum, or openssl) — install one and retry, or verify by hand: curl -L \"$sha_url\""
fi
[ "$want" = "$got" ] || fail "CHECKSUM MISMATCH for $tarball_url (want $want, got $got) — do not install a tampered or corrupt download; re-run later or build from source: git clone \"https://github.com/$REPO\" && cd chug && cargo install --path ."

# --- extract + install (no sudo — ~/.local/bin) ---------------------------------
tar -xzf "$tmp/chug.tar.gz" -C "$tmp" || fail "tar extraction failed for the downloaded archive — the tarball may be corrupt despite the matching checksum; retry or install manually: curl -L \"$tarball_url\" | tar xz"
[ -f "$tmp/chug" ] || fail "archive did not contain a 'chug' binary — the release asset layout changed; install manually from $tarball_url"
mkdir -p "$install_dir" || fail "cannot create $install_dir — check \$HOME permissions (this script never elevates)"
mv "$tmp/chug" "$install_dir/chug" || fail "cannot move the binary into $install_dir — check write permission (this script never elevates)"
chmod +x "$install_dir/chug"

# --- PATH hint + banner ----------------------------------------------------------
case ":$PATH:" in
  *":$install_dir:"*) ;;
  *)
    hint='export PATH="$HOME/.local/bin:$PATH"'
    printf 'install.sh: %s is NOT on your PATH — add it (pick your shell rc):\n' "$install_dir"
    printf '  %s >> ~/.profile   # sh/bash\n' "$hint"
    printf '  %s >> ~/.zshrc     # zsh\n' "$hint"
    ;;
esac

banner
printf '  installed: %s/chug\n' "$install_dir"
printf '  repo:      https://github.com/%s\n' "$REPO"
printf '\n'
printf '  quickstart:\n'
printf '    chug run --spec SPEC.md --goal "Build X and make the check pass" \\\n'
printf '      --model <model> --max-iters 40 --max-minutes 120\n'
printf '  docs: https://github.com/%s#readme\n' "$REPO"
printf '\n'
