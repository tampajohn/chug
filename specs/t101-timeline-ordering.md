# T101 — site-sync timeline: commit-time order, curated merge, cap enforcement

check: cargo test

## Concern

User 2026-09-28: "timeline looks out of order." T99's site-sync generates
TIMELINE entries from TODO done-rows and (a) sorts by row order / date
string, not commit time — a 2026-09-26 entry renders AFTER a wall of
09-27s and T99 (d13a253) appears BEFORE T98 (8721c83); (b) the latest-20
cap + collapse never fired (20 raw entries present); (c) raw TODO titles
("get_str error names received keys (alias self-correction)") were dumped
AFTER the 16 curated hand-written entries instead of merging by commit
ref, so the curated "chug.sh — this site" crescendo is buried mid-list
and the section reads as prose-then-log-dump.

## Repo context

- `scripts/site-sync.sh` (T98/T99): regenerates TIMELINE:BEGIN/END.
- The 16 curated entries (day-one → chug.sh launch) carry commit refs
  (`<span class="hash">f911488</span>`) — the merge key exists in-region.
- TODO done rows: `done <commit>` — commit time via
  `git show -s --format=%ct <ref>` (author date, not row order).

## Requirements

1. Sort ALL entries (curated + generated) by commit time ascending —
   resolve each entry's ref (hash span or row's done-commit) with
   `git show -s --format=%ct`; undatable entries keep stable position
   after their dated neighbors (never before).
2. Merge by ref: when a generated row's commit matches a curated entry's
   hash, KEEP the curated text (prose wins) — one entry, not two.
3. Cap enforced: newest 20 entries in full, older collapse to the
   single "…and N earlier milestones" line (T99 spec's rule — it never
   fired; pin it with a test).
4. Same-day entries order by commit time (not row id); the spec's
   newest-first reading was wrong for the page's chronological design —
   ascending day-one → latest, matching the curated flow.
5. Facts-only audit unchanged: every rendered ref exists (cat-file).

## Tests

- Fixture TODO rows with out-of-order row ids vs commit times → output
  ordered by %ct (T99-before-T98 case named in a test).
- Curated-ref merge: a region containing curated f911488 + a generated
  row for f911488 → ONE entry, curated text preserved.
- Cap: 25 done-rows → 20 full + collapse line naming N=5.
- Acceptance: chug.sh timeline reads chronologically after the next wrap
  sync (recorded in Outcomes).

## Out of scope

- Rewriting curated prose; timeline CSS; the sync's STATS/FEATURES regions.
