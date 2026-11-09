# T348 — install.sh: release-staleness detection + --with-daemon/--with-loopd service legs

estimate: ~120 changed lines (install.sh + fixtures + docs/install-profiles.md note)

## Why

Found 2026-11-09 during the K7 update: with GitHub releases stuck at
v0.17.27 (T347), install.sh **succeeded while installing a month-old
binary** — no warning, no version floor. The script is otherwise current
(loop subcommand guidance, profile contract, meta-pack text all present),
so this item is two targeted gaps, not a rewrite:

1. **Stale-release blindness**: the script trusts `releases/latest`
   without checking the artifact's age or version. When the release
   pipeline breaks (T347's exact failure), every pilot install is
   silently a downgrade.
2. **Supervision is tribal knowledge**: the managed launchd jobs for
   chugd and loopd exist (scripts/com.tampajohn.chug{,d}.plist, installed
   by hand on K7 2026-11-09) but install.sh offers no leg to wire them —
   a pilot's loop is one reboot away from down unless they know the
   plist dance.

## What

1. **Staleness check** (in install.sh, after the tarball resolves):
   - Query the release metadata (api.github.com/repos/$REPO/releases/
     latest → published_at) BEFORE downloading; if the release is older
     than 7 days, print a loud warning naming the date and suggesting
     `--build-from-source` (or CHUG_RELEASE_URL_BASE override), and
     require `--allow-stale` to proceed non-interactively.
   - After install, print the installed `chug --version` (it already
     verifies checksum; the version echo makes staleness visible in
     logs).
2. **`--with-daemon` leg**: installs `scripts/com.tampajohn.chugd.plist`
   to ~/Library/LaunchAgents and bootstraps it (gui domain). Idempotent:
   detects an existing job and kickstarts instead. Prints the socket
   path and the fail-open note.
3. **`--with-loopd` leg**: same shape for the supervisor plist — but
   ONLY inside a repo that already has loop files (chug loop init has
   run), else it prints the init hint and skips. Both legs are macOS-only
   (Linux prints the systemd TODO note and continues).
4. **Docs**: docs/install-profiles.md gains one paragraph on the legs;
   docs/adopt/ quickstart stays as-is (the legs are optional).

## Tests

- install.sh fixture battery: stale-release metadata triggers the
  warning + --allow-stale gate; fresh release passes silently; the
  --version echo appears in output.
- Plist legs (on macOS CI or a fixture harness): idempotent install,
  bootstrap, kickstart-on-rerun; non-macOS path prints the TODO and
  exits 0.
- --with-loopd outside an initialized repo: skips with the init hint,
  no plist installed.

## Out of scope

- The release pipeline fix itself (T347 — this item only makes the
  script loud about it).
- systemd units (print the TODO; a Linux leg is a follow-up if pilots
  appear there).
