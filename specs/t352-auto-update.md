# T352 — `chug update`: self-update with version-floor protection + source-build fallback

estimate: ~170 changed lines (src/cli update subcommand + install-flow reuse + docs + fixtures)

## Why

Operator 2026-11-09: "maybe chug needs auto-update capabilities." The
evidence is fresh: F94 was running chug 0.9.0 with a repo 5,753 commits
behind; K7's daemon was a month stale; releases can freeze silently
(T347). Every box drifts alone and nobody notices until a feature is
missing (the Fleet Primary readiness probe found all of this by hand).
The fleet needs chug to keep itself current.

## What

1. **`chug update`** (new subcommand):
   - Resolve the latest release (GitHub API, honoring
     CHUG_RELEASE_URL_BASE), compare against `chug --version`.
   - Newer → download + verify the sha256 sidecar (same verification as
     install.sh — REUSE that code path, don't fork it), atomic replace
     (temp file + mv — NEVER cp-overwrite a running Mach-O), print
     `old -> new` plus the contract_version change if any.
   - Same version → `already current`; older → REFUSE (no silent
     downgrade) unless `--force`.
   - `--check`: dry-run, exit 0 current / 10 update-available / 2 error.
2. **Source-build fallback**: when the latest release is stale (older
   than 7 days — the T348 rule) AND a checkout + cargo toolchain exist
   (`--source <path>` or CHUG_SOURCE), `cargo build --release` (with
   `--features daemon` iff the installed binary has the daemon
   subcommand live — probe it) and swap. This is the T347-era path; the
   Fleet Primary's 9.7s cached build on F94 is the proof it's cheap.
3. **Opt-in loop integration**: `CHUG_AUTO_UPDATE=1` in the profile →
   loopd runs `chug update --check` once per day (cheap-exit era
   cadence); when an update is available it applies BETWEEN cycles,
   never mid-cycle (the supervisor must not swap a binary a running
   cycle is executing — verify the exec-per-cycle shape first; if the
   supervisor is resident, the swap defers to next natural restart and
   the disposition line says so).
4. **Capability surface**: `chug capabilities` gains
   `features.auto_update: true` when this lands; update decisions log
   old/new + contract_version so harnesses can plan around feature
   changes.

## Tests

- Update fixtures against a fake release server: newer → swapped with
  checksum verified; same → no-op; older → refused; stale-release +
  stub cargo → source path taken; checksum mismatch → no swap, error.
- Atomicity fixture: a running binary is replaced via rename, never
  truncated in place.
- Loop fixture: CHUG_AUTO_UPDATE=1 schedules the check; swap never
  interleaves with a running cycle (asserted by cycle-id capture).

## Compat

Fully opt-in: the subcommand changes nothing unless invoked; the loop
integration activates only via the profile flag. No background update
behavior by default anywhere.

## Out of scope

- The release pipeline fix (T347 — this item's fallback exists BECAUSE
  of it).
- Updating loopd.sh / the repo checkout itself (git-pull territory; a
  separate `chug repo sync` item if wanted later).
- Fleet-wide fan-out (boxes self-update; orchestrated rollout is a
  fleet item, not chug's).
