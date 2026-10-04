# T215 — daemon ensure: spawn the judge from a daemon-capable binary

check: export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared; touch src/*.rs tests/*.rs; cargo test

estimate: ~150 lines (ensure resolution + pins + runbook note)

## Concern

Operator-directed diagnosis 2026-10-03 (K7): `daemon ensure` has exited
nonzero at EVERY cycle start since T204 landed (~16h, fail-open so no
operational impact). Root cause, two layers:
1. loopd's ensure spawns the REPO dev binary (target-shared/release/
   chug) — built default-OFF without the `daemon` feature (T204's lean-
   client design), so the subcommand is the stub that exits with "built
   without the judge daemon".
2. The release workflow DOES build `--features daemon` (release.yml
   line 72), but K7's installed ~/.local/bin/chug is a pre-T204 release
   ("unrecognized subcommand 'daemon'") — the dogfood upgrade path
   didn't pull v0.15/v0.16.

The daemon is HOST-SCOPED (one per box, serves any run) — the right
carrier is the installed release binary, not the repo dev build.

## Repo context

- T204 spec req 1: `daemon` cargo feature default OFF; req 6: release
  builds enable it per-target (metal mac, cpu linux) — DONE (workflow
  line 72 verified).
- T204 req 4: loopd ensures the daemon at cycle start — the ensure step
  exists and works (spawn/socket-wait/clean-fail observed live); only
  its BINARY CHOICE is wrong.
- daemon.log error (6 fires): "built without the judge daemon (the
  default): rebuild with `cargo build --features daemon`".
- After the binary resolves, the NEXT hurdle is the first-weight-load
  (~650MB HF download of the public base checkpoint — no token needed;
  CHUG_LAYA_CHECKPOINT local-dir override exists per T204 req 2/T205
  interim posture). The ensure's socket-wait budget must tolerate a
  first-download (or pre-warm via a documented one-liner).

## Requirements

1. Ensure binary resolution order: (a) CHUG_DAEMON_BIN explicit
   override; (b) ~/.local/bin/chug IF it reports daemon support
   (probe: `chug daemon --help` exit 0); (c) the repo release build IF
   compiled with the feature; else skip ensure with one log line
   (fail-open, as today). The probe result is cached per loopd run.
2. loopd's repo builds stay feature-lean (the T204 design stands) —
   do NOT add --features daemon to the shared/validate/gates builds.
3. First-run weight tolerance: the ensure's wait budget covers a cold
   HF download OR the runbook documents the pre-warm one-liner
   (`chug daemon` foreground once). Pick one; pin the choice in the
   runbooks/ daemon entry.
4. DEPENDENCIES.md (T203) + runbooks/loop-ops.md note: the daemon
   comes from the INSTALLED release binary on loop hosts; repo dev
   builds are clients only.

## Tests

- Resolution order pins: override wins; stale installed binary
  (no daemon subcommand) falls through; feature-less repo build falls
  through with the one-line log.
- Probe caching: one probe per loopd run, not per cycle.
- Acceptance: K7's next ensure after the installed-binary upgrade
  serves /healthz on the socket (recorded in the cycle Outcomes).

## Out of scope

- Building the daemon feature into repo dev builds; auto-upgrading
  ~/.local/bin/chug from loopd (dashd owns dogfood upgrades); weight
  pre-seeding into the repo.
