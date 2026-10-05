# T240 — RELEASE region on chug.sh: latest release always current

check: cargo test --test site_sync

estimate: ~180 lines (region generator + nav + pins)

## Concern

Operator 2026-10-05: chug.sh should show the latest release on the
page, kept up to date with releases. Today the site says nothing about
versions — an operator checking "what's current" must hit GitHub. The
wrap-time site-sync already regenerates every marked region; a RELEASE
region rides the same machinery and is correct by construction after
every tag.

## Repo context

- Regions: STATS/TIMELINE/FEATURES (T98/T99) regenerated at wrap by
  scripts/site-sync.sh; marker-pair contract (T99) with fail-closed
  input guard (T217).
- Release facts are local: `git tag --sort=-creatordate` (immutable
  v* tags), the tag's message (release notes are generated from
  semantic commits per the wrap doctrine), and the tag date.
- Nav: the page's section nav (05 · the loop doctrine etc.) — releases
  gains an anchor.

## Requirements

1. `<!-- RELEASE:BEGIN/END -->` region between the hero and STATS:
   version (latest v* tag), tag date, and the top 3-5 release-note
   bullets (from the tag message, emphasis-stripped, HTML-escaped,
   160-byte cap per the T99 text rules).
2. Source is the LOCAL checkout's tags at sync time (wrap pushes tags
   before syncing, so the region reflects the just-cut release on
   release wraps).
3. Fail-closed (T217): no tags -> region writes nothing and the sync
   continues (pre-v0.1 repos are legal).
4. Nav gains a releases anchor next to the existing sections.
5. tests/site_sync.rs pins: a fixture with two tags renders the newer
   with its date + capped bullets; a fixture with zero tags writes an
   empty region and does not error.

## Tests

- The two pins above; existing region pins stay green.

## Out of scope

- Full release-notes pages per version (the bullets link to GitHub);
  pre-release channels (v* stable tags only).
