# T346 — RED-proven guard: every loopd-spawning test harness pins the env-file loaders

estimate: ~60 changed lines (one new test target, tests-only)

## Problem

The 2026-11-09 host bite (cycle-3172 post-wrap fix-up 7a26c410): the
operator-staged `~/.chug/profile.env` re-exported itself into every spawned
`loopd.sh` fixture AFTER each harness's `env_remove` calls — the T330 profile
loader runs inside the spawned script. A `[1m]`-suffixed `CHUG_MODEL_REASONING`
landed in the gate line's model field, and the profile's proxy config starved
the daemon-socket judge fixtures onto the proxy (`laya=down conf=-`): two
`loopd_laya_triage` tests red under a full-suite run while individually green
in other orders. The repair pinned `CHUG_LOOPD_ENV` / `CHUG_PROFILE_ENV` to
ABSENT fixture paths at 11 spawn sites across 7 harnesses
(`common::pin_loopd_env_files`), but nothing stops the NEXT harness from
forgetting the pin — the class is mechanical and must be guarded, not
remembered.

## The contract being guarded

Every test that execs `loopd.sh` through `Command::new("bash")` MUST call
`common::pin_loopd_env_files(&mut cmd, <fixture root>)` (or set both loader
overrides to fixture paths that DO NOT EXIST — the loaders' absent-file
contract is a silent no-op; an existing-but-incomplete file warns). Harnesses
that test the loaders themselves (`loopd_env_loader.rs`,
`loopd_profile_loader.rs`) are the sanctioned exception — they set their own
files.

## Acceptance

A scan test (tests/loader_pin_guard.rs, tests-only): parse every
`tests/*.rs`, find `Command::new("bash")` sites whose argv chain mentions
`loopd.sh`, and assert each construction site either (a) calls
`common::pin_loopd_env_files`, or (b) sets both `CHUG_LOOPD_ENV` and
`CHUG_PROFILE_ENV` itself, or (c) sits in a sanctioned-exception file. The
test ships RED-proofed: with one pin removed the scan names the file and
line. Scan walks TEXT, not AST — the mechanical shape that survives refactor
noise is preferred (the T293 brittle-awk lesson applies to the detector's
own false-positive surface: assert the scan's exception list stays short and
named).

check: CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared cargo test --release --test loader_pin_guard
