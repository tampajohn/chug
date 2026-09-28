# T123 — release v0.2.0 workflow failure record (filed post-hoc per the Phase-3 wire)

check: cargo test

Post-hoc row, created already-`done`: the Phase-3 wire requires a failed
release workflow to file a TODO row NAMING THE TAG. The remedy landed
operator-side before this row existed.

## The failure

`v0.2.0` (aa5ed2e, cycle-62 wrap — the loop's first self-cut tag) failed its
release workflow on TWO legs:

1. **`cargo build --release --locked` lockfile mismatch** — the version-bump
   commit edited Cargo.toml 0.1.0→0.2.0 without syncing Cargo.lock (cargo
   auto-synced it locally on the next build, uncommitted, so nothing local
   ever showed red). `--locked` in the workflow refuses a stale lockfile.
2. **aarch64 cross build missing `libc6-dev-arm64-cross`** (ring's cc sysroot
   headers) — pre-existing, had already failed v0.1.0's run 36437458891.

Neither tag is deleted or moved (published tags are immutable history).

## Remedy (landed)

- `3cd7b7e` fix(ci): release aarch64 cross build installs libc6-dev-arm64-cross.
- `1e84357` chore: release v0.2.1 — Cargo.lock synced + LOOP-SPEC Phase-3
  doctrine added: release bumps must sync Cargo.lock.
- `aaf31eb` (cycle-62 wrap tail): build_info banner pins derive the version
  from the `VERSION` const — a bump can no longer red the banner tests (the
  bump commit had also never been gated; the goal-gate's `cargo test` caught
  it post-tag).

## Carried to next eval

Add a post-bump targeted gate to the Phase-3 release procedure before tagging:
`cargo test --bin chug build_info` + a `--locked` build check (seconds each).
