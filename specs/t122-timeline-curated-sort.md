# T122 — site-sync timeline: curated entries sort by display date

check: cargo test

## Concern

User 2026-09-28: "timeline still out of order — some things pinned to
bottom." T101 (bcd0b66) sorted GENERATED entries by commit %ct, but
curated entries without machine-resolvable commit refs keep their
original stable position — which is the BOTTOM of the region, below
newer generated entries. Live evidence: "The move to K7 — always-on"
(no hash span) and "chug.sh — this site e998d45" both render AFTER
2026-09-28 entries despite being 2026-09-27 milestones.

## Repo context

- `scripts/site-sync.sh` T101 logic: %ct-ascending sort for ref-resolved
  entries; "undatable entries keep stable position after their dated
  neighbors (never before)" — the bottom-pin bug is that clause.
- Every curated entry carries `<div class="tl-date">2026-09-27</div>` —
  a parseable sort key even when the h3 has no hash span.

## Requirements

1. Sort key precedence per entry: commit %ct (ref resolves) → tl-date
   text parsed as date (day precision) → stable position ONLY among
   entries sharing that date. "Never before dated neighbors" dies.
2. Day-precision ties break by the entry's original region position
   (curated flow within a day is preserved).
3. The two named entries (K7 move, chug.sh launch) sort into their
   2026-09-27 positions in the output — pinned with a fixture test.
4. T101's guarantees unchanged: %ct primary ordering, curated-text-wins
   merge, 20+collapse cap, cat-file audit for refs that DO resolve.

## Tests

- Fixture: curated undated-ref entries at 09-27 + generated entries at
  09-28 → output orders curated-09-27 BEFORE generated-09-28.
- Same-day tie: two undatable entries at one date keep original order.
- Ref-resolved entries still sort by %ct within the same day (T101
  regression leg).

## Out of scope

- Responsive/CSS issues (separate site-repo session); cap behavior.
