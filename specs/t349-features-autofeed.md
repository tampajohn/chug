# T349 — features list auto-feed: site renders shipped work from the TODO record + F-row backfill

estimate: ~150 changed lines (scripts/site-sync.sh SHIPPED region + FEATURES.md backfill + fixtures)

## Why

Operator 2026-11-09: "features list doesn't seem to be kept up to date on
the ui." Confirmed: FEATURES.md stopped at F15 (F16 parked) — dormant
mode, ideation, cost dashboard, adopt page, install profiles, Jev
provider, the loop subcommand, the risk-gate story, and the whole
meta-harness series (T325-T345) shipped with no F-rows, so chug.sh shows
a feature grid weeks out of date. The mechanism is the bug: the grid is
hand-maintained, so it drifts whenever the loop forgets (which is always).

## What

1. **SHIPPED region** (new, site-sync.sh): render a "Recently shipped"
   feed from the AUTHORITATIVE shipped record — TODO.md done rows (id,
   title, done date derivable from the row/git). Last ~20 items, newest
   first, each linking its spec. This region is mechanical: it cannot
   drift because the loop's own flip process maintains the source. Render
   above the curated FEATURES grid on index.html; docs.html gains the
   same feed (full done list, paginated-ish).
2. **F-row backfill** (this item, format-faithful): add rows for the
   shipped flagships — F17 dormant-mode cost program; F18 ideation lane;
   F19 loop subcommand; F20 install profiles; F21 decision providers
   (local Laya + proxy Jev); F22 meta-harness pack (capabilities, typed
   status, broker, status mod, usage events); F23 adopt + risk-gate
   docs surface. Match the existing row shape EXACTLY (the F10 splice
   clobber is the cautionary tale — verify against the site-sync parser,
   cells escaped per T216).
3. **Grid annotation rule** (docs only, one line in the FEATURES region
   header): the curated grid is the flagship list; the SHIPPED feed is
   the complete record. No behavioral change to the grid render.

## Tests

- site-sync fixture: canned TODO.md with done rows → feed renders newest-
  first with spec links; empty done set → T216 placeholder; malformed
  row skipped with a named count, never fatal (T217).
- FEATURES.md after backfill: the site-sync grid parser produces the
  same-or-more cards and the F-row cells parse per the existing
  extractor legs (run them: they are the parser's own pins).
- Feed freshness: an item flipped done appears in the next sync without
  any hand edit (the actual acceptance test).

## Out of scope

- Removing the curated FEATURES grid (it stays; the feed complements).
- Editing the F1-F15 rows' content (backfill is append-only).
- Retroactive release-notes generation (feed links specs, not releases).
