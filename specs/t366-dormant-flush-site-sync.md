# T366 — dormant release flush never syncs the site: chug.sh's latest-release panel lags the flush's tag until the next launched cycle

estimate: ~45 changed lines (loopd.sh flush leg + loopd_dormant_mode fixture legs)

check: sh -n loopd.sh && CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared cargo test --release --test loopd_dormant_mode --test loopd_laya_triage

## Why

Observed at the cycle-3212 dormant weekly audit (T325, 2026-11-18): the
published site shows **v0.26.0** as "latest release" while **v0.26.1**
was tagged and pushed 7 days earlier. Timeline, all 2026-11-11:

- 12:35:07-05:00 — last site sync commit (`site: stats sync`, chug-site
  305e1e0);
- 12:41:55-05:00 — the T361 dormant release flush cut `chore: release
  v0.26.1` (5b8e0125) and pushed the tag (98e3cb3f on origin).

Root cause: the flush cycle (`dormant_release_flush` in loopd.sh) bumps,
tags, pushes, sleeps, and `continue`s — it never runs
`scripts/site-sync.sh`. The sync rides only the launched-cycle-OK path
(loopd.sh ~line 2756), which dormancy suppresses: dormant cheap-exits
never launch, the flush is model-free, so the ONLY sync dormancy ever
runs is the weekly audit's own cycle-OK leg. A flush can therefore leave
the public release panel stale for up to the audit cadence (7d). This is
T361's own premise one leg downstream: the flush publishes the release
to GitHub but not to chug.sh — the release channel thaws, the
publication channel stays frozen.

## What

1. **Site-sync leg in `dormant_release_flush`**, after the tag push
   verifies (the T363 ls-remote check): run `scripts/site-sync.sh`
   exactly like the cycle-OK leg — best-effort, failure-tolerant: a
   nonzero exit logs a named warn and never fails the flush (the tag is
   already published; site sync is observability, not a gate). Log line:
   `dormant release flush (T361): site-sync ran (rc=N)` (or the script's
   own skip warn when the clone is absent — the script already degrades
   to a warn + exit 0 there).
2. **Ordering**: the sync runs ONLY after a successful, verified push —
   a declined flush or a failed push verification never syncs (nothing
   new to publish; the panel stays honestly on the pushed tag).
3. The weekly audit's cycle-OK sync stays as the backstop (unchanged) —
   with the flush leg, the panel lags a release by minutes, never by
   the audit cadence.

## Tests

- loopd fixture: dormant + flush succeeds (push verified) → the sync
  stub is invoked exactly once AFTER the push-verify leg (ordering
  asserted), flush log carries the sync line.
- Flush + sync stub exits nonzero → flush still succeeds, the named
  warn is logged, the pacing sleep still runs.
- Flush declined (anchor-safety refusal) → sync stub NEVER invoked.
- Flush + push verification empty (T363 retry path) → sync stub never
  invoked on the unverified leg; invoked on the retry that verifies.

## Compat

Only the dormant flush path changes; the launched-cycle-OK sync, the
audit contract, and the flush's mechanical (zero-LLM) shape are
untouched. The sync is already failure-tolerant everywhere it runs.

## Out of scope

- Backfilling the v0.26.1 panel by hand (the audit's own cycle-OK sync
  heals it at this cycle's completion — before this item dispatches).
- Syncing on dormant cheap-exit cycles (nothing to publish; keeps the
  cheap exit $0).
- Changing what site-sync renders (T98/T240/T327/T349 own that).
