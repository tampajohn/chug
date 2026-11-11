# T363 — T361 flush cut v0.26.0 but never pushed it (tag stayed local; no release workflow, no GitHub release)

estimate: ~40 changed lines (the flush leg's push step + verification + fixtures)

check: sh -n loopd.sh && CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared cargo test --release --test loopd_dormant_mode --test loop_spec_tag_doctrine

## Why

First live exercise of the T361 dormant release flush (2026-11-11):
the flush cut v0.26.0 locally at 06:16Z (TRUE recount 3: T360+T361+
T362) — and pushed NOTHING. `git ls-remote origin` had no v0.26 ref for
~9 hours: no release workflow, no GitHub release, and Fleet Primary's
acceptance suite (armed on the carrying tag) could not fetch it. The
spec's "cut + push" became cut-only, and it FAILED SILENTLY — the wrap
note even claimed "release v0.26.0 tagged" as if done. Operator pushed
the tag by hand (workflow fired immediately). A release flush that
doesn't publish is a release that didn't happen.

## What

1. **The flush leg MUST push and VERIFY**: after cutting the tag,
   `git push origin <tag>` and then `git ls-remote origin <tag>` —
   non-empty output or the flush FAILS LOUD (log line
   `dormant release flush FAILED: push verification empty for <tag>`,
   and the next cycle retries instead of recording success).
2. **The wrap note must not claim a release that isn't pushed**: the
   recount line says `tagged+pushed` only after the ls-remote check;
   otherwise `tagged-LOCAL-ONLY (push pending)`.
3. **Retry**: a failed push (network, auth) does not lose the tag — the
   local tag persists and the next dormant cycle retries the push only
   (no duplicate cut).

## Tests

- Fixture: push succeeds → ls-remote verified → wrap claims
  `tagged+pushed`.
- Fixture: push fails (stubbed remote error) → FAIL LOUD, no success
  claim, retry scheduled; second run pushes only (no duplicate tag).
- The 2026-11-11 incident is the acceptance case: v0.26.0's local-only
  window must be impossible after this lands.

## Out of scope

- The release workflow itself (T347 fixed it; it fired immediately on
  the manual push).
- Backfilling v0.26.0's GitHub release (the manual push already
  triggered it).
