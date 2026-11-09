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
#
# Flag (T330 — the LLM profile contract, docs/install-profiles.md):
#   --profile <name>  resolve profiles/<name>.env (repo checkout) or
#                     ~/.chug/profiles/<name>.env (private values, win),
#                     validate it BEFORE anything is written (a bad profile
#                     installs nothing and never writes ~/.chug/profile.env),
#                     copy it there (mode 600) for the loop supervisor to
#                     load at startup, and print the loop launch line.
#
# Flags (T341+T342 — the meta hook packs, docs/meta-harness.md):
#   --with-meta-pack      install hooks/meta-broker/ and hooks/meta-status/
#                         into <claude-dir>/hooks/<pack> and wire their
#                         hooks into <claude-dir>/settings.json additively
#                         (existing hooks untouched). The packs come from
#                         the SAME verified release tarball the binary
#                         comes from — never a second fetch — or from the
#                         working tree when this script runs from a repo
#                         checkout that carries them.
#   --uninstall-meta-pack the exact reverse, offline: unwire the packs'
#                         hooks (exactly their own blocks), remove the
#                         installed pack dirs and the status mod's session
#                         files. No download, no other change. Takes no
#                         other flags: --profile is refused named (a
#                         profile belongs to an install).
#   <claude-dir> is $CLAUDE_CONFIG_DIR when set, else ~/.claude — the same
#   resolution the hook itself uses at run time.
set -eu
export LC_ALL=C

REPO="${CHUG_INSTALL_REPO:-tampajohn/chug}"
install_dir="${CHUG_INSTALL_DIR:-$HOME/.local/bin}"

fail() { printf 'install.sh: %s\n' "$1" >&2; exit 1; }

banner() {
  printf '\n'
  printf '   ____ _   _ _   _  __ _ \n'
  printf '  / ___| | | | | | |/ _\\` |\n'
  printf ' | |   | |_| | | | | (_| |\n'
  printf ' | |___|  _  | |_| |\\__, |\n'
  printf '  \\____|_| |_|\\___/ |___/ \n'
  printf '\n'
}

# --- T330/T341: flag parsing + the offline uninstall leg ----------------------
# --profile resolves before any download (a bad profile fails fast and
# offline). The meta-pack uninstall is offline by contract: it runs before
# the platform detect, needs no network, and moves nothing else.
# --with-meta-pack rides the normal install flow (binary + pack, one fetch).
profile_name=
with_meta_pack=0
uninstall_meta_pack=0
while [ $# -gt 0 ]; do
  case "$1" in
    --profile)
      [ $# -ge 2 ] || fail "--profile needs a profile name — usage: install.sh --profile <name> (docs/install-profiles.md)"
      profile_name=$2
      shift 2
      ;;
    --profile=*)
      profile_name=${1#--profile=}
      [ -n "$profile_name" ] || fail "--profile= needs a profile name — usage: install.sh --profile <name> (docs/install-profiles.md)"
      shift
      ;;
    --with-meta-pack)
      with_meta_pack=1
      shift
      ;;
    --uninstall-meta-pack)
      uninstall_meta_pack=1
      shift
      ;;
    *)
      fail "unknown argument '$1' — the flags are --profile <name>, --with-meta-pack and --uninstall-meta-pack (docs/install-profiles.md, docs/meta-harness.md)"
      ;;
  esac
done

# The checkout's pack wins when this script runs from a repo checkout that
# carries hooks/<pack>/ (repo-checkout users install from the working
# tree); otherwise the packs are extracted from the SAME verified tarball
# the binary comes from. A curl|sh run has no checkout unless the cwd
# happens to be one — the file-existence gate decides, exactly like
# ./profiles/ does.
case "$0" in
  */*) pack_checkout_dir=${0%/*} ;;
  *)   pack_checkout_dir=. ;;
esac
pack_checkout_src() { # $1 = pack name; prints the checkout-tree dir, or nothing
  if [ -f "$pack_checkout_dir/hooks/$1/hook.sh" ]; then
    printf '%s\n' "$pack_checkout_dir/hooks/$1"
  fi
}

# --- T341/T342: --uninstall-meta-pack (offline — no download, nothing else moves) ---
if [ "$uninstall_meta_pack" = 1 ]; then
  if [ "$with_meta_pack" = 1 ]; then
    fail "--uninstall-meta-pack takes no other flags — it is the exact reverse of --with-meta-pack and runs offline"
  fi
  # A --profile belongs to an INSTALL (the loop supervisor loads it at
  # startup); the uninstall is offline and moves nothing but the packs, so
  # the combination is refused NAMED rather than silently ignored (the
  # uninstall leg runs before profile resolution and would otherwise drop
  # the flag on the floor).
  if [ -n "$profile_name" ]; then
    fail "--uninstall-meta-pack does not take --profile — the uninstall is offline and moves nothing but the packs; a profile belongs to an install: run 'install.sh --profile <name>' on its own (docs/install-profiles.md)"
  fi
  claude_dir=${CLAUDE_CONFIG_DIR:-$HOME/.claude}
  settings_file=$claude_dir/settings.json
  for pack in meta-broker meta-status; do
    pack_dir=$claude_dir/hooks/$pack
    hook_command=$pack_dir/hook.sh
    checkout_src=$(pack_checkout_src "$pack")
    wire=
    if [ -f "$pack_dir/wire.py" ]; then
      wire=$pack_dir/wire.py
    elif [ -n "$checkout_src" ] && [ -f "$checkout_src/wire.py" ]; then
      wire=$checkout_src/wire.py
    elif [ -f "$settings_file" ] && grep -q "$pack" "$settings_file" 2>/dev/null; then
      fail "the $pack pack is wired in $settings_file but its uninstaller is gone (no wire.py in $pack_dir or $checkout_src) — remove the $pack hook entries from $settings_file by hand, or reinstall the pack first: install.sh --with-meta-pack"
    fi
    if [ -n "$wire" ]; then
      python3 "$wire" remove "$settings_file" "$hook_command" || fail "unwiring the $pack hook failed — $settings_file was left as the uninstaller reported; fix the named cause and retry"
    fi
    if [ -d "$pack_dir" ]; then
      rm -rf "$pack_dir" || fail "cannot remove $pack_dir — check permissions (this script never elevates)"
      printf 'install.sh: %s pack removed (%s)\n' "$pack" "$pack_dir"
    else
      printf 'install.sh: %s pack not installed (%s absent)\n' "$pack" "$pack_dir"
    fi
  done
  # The status mod's session files are its on-disk footprint; the
  # uninstall is the exact reverse, so they go too (readers treat a
  # missing file as not-a-meta-session — the contract's absent posture).
  if [ -d "$claude_dir/meta-status" ]; then
    rm -rf "$claude_dir/meta-status" || fail "cannot remove $claude_dir/meta-status — check permissions (this script never elevates)"
    printf 'install.sh: session-status files removed (%s)\n' "$claude_dir/meta-status"
  fi
  exit 0
fi



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

profile_file=
if [ -n "$profile_name" ]; then
  case "$profile_name" in
    ''|.|..|*/*|*.env) fail "invalid profile name '$profile_name' — a profile is the file's NAME without .env (e.g. --profile local-openai-compat); profiles live at ~/.chug/profiles/<name>.env or ./profiles/<name>.env (docs/install-profiles.md)" ;;
  esac
  # Private values win: ~/.chug/profiles/ before a repo checkout's profiles/.
  for profile_cand in "$HOME/.chug/profiles/$profile_name.env" "./profiles/$profile_name.env"; do
    if [ -f "$profile_cand" ]; then
      profile_file=$profile_cand
      break
    fi
  done
  [ -n "$profile_file" ] || fail "profile '$profile_name' not found — looked in ~/.chug/profiles/$profile_name.env (private values) and ./profiles/$profile_name.env (repo checkout); start from profiles/local-openai-compat.env in the repo — docs/install-profiles.md"

  # Validate BEFORE anything is written (never a half-written profile). The
  # parse mirrors the loopd loader's tolerance (optional `export ` prefix,
  # one pair of surrounding quotes, CRLF, `#` comments, blank lines); empty
  # values count as absent, matching src/auth.rs's resolution rule.
  p_cr=$(printf '\r')
  p_base= p_key= p_cmd= p_reason= p_coding= p_unknown=
  while IFS= read -r p_line || [ -n "$p_line" ]; do
    p_line=${p_line%"$p_cr"}
    case "$p_line" in ''|\#*) continue ;; esac
    p_line=${p_line#export }
    case "$p_line" in
      *=*)
        p_k=${p_line%%=*}
        p_v=${p_line#*=}
        case "$p_v" in
          \"*\") p_v=${p_v#\"}; p_v=${p_v%\"} ;;
          \'*\') p_v=${p_v#\'}; p_v=${p_v%\'} ;;
        esac
        case "$p_k" in
          CHUG_LLM_BASE_URL)    p_base=$p_v ;;
          CHUG_LLM_API_KEY)     p_key=$p_v ;;
          CHUG_LLM_API_KEY_CMD) p_cmd=$p_v ;;
          CHUG_MODEL_REASONING) p_reason=$p_v ;;
          CHUG_MODEL_CODING)    p_coding=$p_v ;;
          # T332 — the optional decision-provider pass-throughs (the values
          # are the native env names; the loopd loaders export them verbatim).
          CHUG_DECISION_PROVIDER|CHUG_DECISION_PROXY_URL|CHUG_DECISION_PROXY_TOKEN) ;;
          CHUG_LANGFUSE_*)      ;; # the optional observability class
          *) p_unknown="$p_unknown $p_k" ;;
        esac
        ;;
      *)
        fail "profile '$profile_name': every non-comment line must be KEY=VALUE — fix or remove the malformed line (docs/install-profiles.md)"
        ;;
    esac
  done < "$profile_file"

  profile_missing=
  [ -n "$p_base" ]   || profile_missing="$profile_missing CHUG_LLM_BASE_URL"
  [ -n "$p_reason" ] || profile_missing="$profile_missing CHUG_MODEL_REASONING"
  [ -n "$p_coding" ] || profile_missing="$profile_missing CHUG_MODEL_CODING"
  if [ -z "$p_key" ] && [ -z "$p_cmd" ]; then
    profile_missing="$profile_missing CHUG_LLM_API_KEY (or CHUG_LLM_API_KEY_CMD)"
  fi
  [ -z "$profile_missing" ] || fail "profile '$profile_name' is missing required key(s):$profile_missing — nothing installed and ~/.chug/profile.env NOT written; add them and retry (docs/install-profiles.md)"
  if [ -n "$p_key" ] && [ -n "$p_cmd" ]; then
    fail "profile '$profile_name' sets BOTH CHUG_LLM_API_KEY and CHUG_LLM_API_KEY_CMD — the contract takes exactly one; comment the other out and retry (docs/install-profiles.md)"
  fi
  [ -z "$p_unknown" ] || fail "profile '$profile_name' sets non-profile-owned key(s):$p_unknown — a profile owns exactly CHUG_LLM_BASE_URL, CHUG_LLM_API_KEY (or CHUG_LLM_API_KEY_CMD), CHUG_MODEL_REASONING, CHUG_MODEL_CODING, CHUG_LANGFUSE_*, CHUG_DECISION_PROVIDER, CHUG_DECISION_PROXY_URL, CHUG_DECISION_PROXY_TOKEN; nothing installed (docs/install-profiles.md)"

  mkdir -p "$HOME/.chug" || fail "cannot create $HOME/.chug — check \$HOME permissions (this script never elevates)"
  cp "$profile_file" "$HOME/.chug/profile.env" || fail "cannot write $HOME/.chug/profile.env — check write permission (this script never elevates)"
  # A profile may carry a literal CHUG_LLM_API_KEY — never group/world-readable.
  chmod 600 "$HOME/.chug/profile.env" || fail "cannot restrict $HOME/.chug/profile.env to mode 600 — a profile may carry a literal key; check permissions (this script never elevates)"
fi

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

# --- T341/T342: the meta hook packs (--with-meta-pack) -------------------------
# Both packs come from the SAME verified tarball the binary came from (or
# the working tree when this script runs from a repo checkout that carries
# them — the checkout wins), land in <claude-dir>/hooks/<pack>, and their
# hooks are wired into <claude-dir>/settings.json additively: existing
# hooks untouched, a re-run a no-op, the write atomic. Both sources are
# resolved BEFORE anything is created or wired — a release that predates
# either pack fails named with nothing touched.
if [ "$with_meta_pack" = 1 ]; then
  command -v python3 >/dev/null 2>&1 || fail "python3 is required for --with-meta-pack (the packs' handlers and settings wiring run on it) — install python3 and retry"
  claude_dir=${CLAUDE_CONFIG_DIR:-$HOME/.claude}
  settings_file=$claude_dir/settings.json
  for pack in meta-broker meta-status; do
    if [ -z "$(pack_checkout_src "$pack")" ] && [ ! -f "$tmp/hooks/$pack/hook.sh" ]; then
      fail "the downloaded release tarball does not carry the meta pack (no hooks/$pack/) — this release predates the hook packs; use a newer release or build from source: git clone \"https://github.com/$REPO\" && cd chug && cargo install --path ."
    fi
  done
  # The status mod's session files live in their own directory, created at
  # install: the hook itself stays fork-free (a missing dir means skip, so
  # the hot path never pays a mkdir).
  mkdir -p "$claude_dir/hooks" "$claude_dir/meta-status" || fail "cannot create $claude_dir/hooks or $claude_dir/meta-status — check permissions (this script never elevates)"
  for pack in meta-broker meta-status; do
    pack_src=$(pack_checkout_src "$pack")
    pack_src_desc="the working tree ($pack_src)"
    if [ -z "$pack_src" ]; then
      pack_src=$tmp/hooks/$pack
      pack_src_desc="the verified release tarball"
    fi
    pack_dir=$claude_dir/hooks/$pack
    rm -rf "$pack_dir" || fail "cannot refresh $pack_dir — check permissions (this script never elevates)"
    cp -R "$pack_src" "$pack_dir" || fail "cannot copy the $pack pack into $pack_dir — check permissions (this script never elevates)"
    chmod 755 "$pack_dir/hook.sh" "$pack_dir/wire.py" 2>/dev/null || true
    chmod 755 "$pack_dir/hook.py" 2>/dev/null || true
    printf 'install.sh: %s pack installed from %s -> %s\n' "$pack" "$pack_src_desc" "$pack_dir"
    python3 "$pack_dir/wire.py" add "$settings_file" "$pack_dir/hook.sh" || fail "wiring the $pack hook into $settings_file failed — the pack is installed but NOT wired; fix the named cause and retry (uninstall: install.sh --uninstall-meta-pack)"
  done
fi

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
if [ -n "$profile_name" ]; then
  printf '  profile:   %s -> %s (profile %s)\n' "$profile_file" "$HOME/.chug/profile.env" "$profile_name"
  case "$p_key" in
    ''|'$'*|'<'*|'('*) ;; # no literal key, or a fetch/indirection value
    *) printf '  note:      the profile carries a literal CHUG_LLM_API_KEY — prefer CHUG_LLM_API_KEY_CMD (a vault fetch) so the key never rests in a file\n' ;;
  esac
  printf '\n'
  printf '  launch the loop in a repo of yours (no clone of chug needed):\n'
  printf '    cd <your-repo> && chug loop init && chug loop run\n'
  printf '    # the loop sources %s (profile %s)\n' "$HOME/.chug/profile.env" "$profile_name"
  printf '\n'
else
  printf '\n'
  printf '  next: point the loop at a model provider —\n'
  printf '    install.sh --profile <name>   # the LLM profile contract; docs/install-profiles.md\n'
  printf '    install.sh --with-meta-pack   # the meta hook packs (broker + status mod); docs/meta-harness.md\n'
  printf '\n'
fi
if [ "$with_meta_pack" = 1 ]; then
  printf '\n'
  printf '  meta packs: %s/hooks/{meta-broker,meta-status} (hooks wired into %s)\n' "$claude_dir" "$settings_file"
  printf '    broker socket: %s/meta-broker.sock  (the harness listens here)\n' "$claude_dir"
  printf '    audit trail:   %s/meta-broker-audit.jsonl  (one JSONL line per brokered decision)\n' "$claude_dir"
  printf '    status files:  %s/meta-status/<session-id>.json  (one typed status doc per session)\n' "$claude_dir"
  printf '    behavior: stock permission prompts until a broker answers on the socket; the status mod only ever writes its own files\n'
  printf '    uninstall: install.sh --uninstall-meta-pack\n'
  printf '    contract:  docs/meta-harness.md (the T341 and T342 sections)\n'
fi
printf '  quickstart:\n'
printf '    chug run --spec SPEC.md --goal "Build X and make the check pass" \\\n'
printf '      --model <model> --max-iters 40 --max-minutes 120\n'
printf '  docs: https://github.com/%s#readme\n' "$REPO"
printf '\n'
