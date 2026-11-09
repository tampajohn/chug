# T357 — pin the site-sync cycles leg-2 subject anchor

estimate: ~5 changed lines (one fixture in tests/site_sync.rs)

## Why

The T350 validator's mutation m8 (`--format=%s` -> `%B` on the leg-2
git-log anchor) SURVIVED the 66-test battery: no fixture carries an
`eval:`-prefixed line in a commit BODY, so counting bodies would still
pass everything. The shipped code is correct (subjects only); the
anchor is just unpinned. Real-repo impact if it regressed: 5,679 vs
5,652 (+0.5%) — same order, but the contract "disposition COMMITS"
would silently widen.

## What

One fixture in the site_sync battery: build a tiny git repo (or reuse
the existing fixture harness) with one `eval:` disposition commit whose
MESSAGE BODY also carries a line starting `eval:`; assert the cycles
figure counts ONE, not two.

## Tests

- The new fixture passes on the shipped code (count = 1).
- RED-proven: flipping `--format=%s` to `%B` makes the fixture fail.

check: CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared cargo test --release --test site_sync
