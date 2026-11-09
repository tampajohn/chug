# T350 — STATS cycles counter: count the durable cycle record, not the rotating loopd.log

estimate: ~60 changed lines (scripts/site-sync.sh stats leg + fixtures)

## Why

Found in the 2026-11-09 staleness audit: the site's STATS region claims
"1,042 cycles completed" while the loop is on cycle ~3,172. The scraper
counts `cycle OK` lines in the CURRENT `.chug/loopd/loopd.log`, which
rotates — so the counter resets invisibly and understates forever. Same
bug class as the fossilized FEATURES grid (T349): a source that looks
mechanical but is not durable. Audit also confirmed TIMELINE (TODO done
rows), COST (Langfuse), and the RELEASE band (git tags) are on durable
sources — this is the last counter on a volatile one.

## What

In the STATS leg of `scripts/site-sync.sh`, change the cycles figure's
source to the durable record, first match wins:
1. count of `cycle-*.log` filenames under `.chug/loopd/` (every cycle
   writes one; archives included), else
2. count of `eval:` disposition commits in git log (the other durable
   per-cycle record), else
3. the legacy log-line count, rendered as `≥N` so a degraded figure is
   honest about being a floor.
Label the figure from its source (`cycles on record` vs the current
"cycles completed by the loopd supervisor") so a future reader can tell
what the number means. Fail-closed per T217: no durable source →
placeholder, never a fabricated zero.

## Tests

- STATS fixture battery: a dir with N cycle-*.log files → exact count;
  files absent + eval commits present → commit count; both absent →
  log-line count rendered with the ≥ prefix; nothing → placeholder.
- The real repo renders a figure ≥ 3,100 at build time (acceptance).

## Out of scope

- The other STATS figures (queue items from TODO.md ✓ durable; gate
  count from commit messages — brittle but guarded by T238, separate
  item if it ever lies).
- Backfilling historical cycle counts into the page (the counter goes
  truthful from the next sync; history is in git).

check: CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-impl-b cargo test --release --test site_sync
