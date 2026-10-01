# T193 — site-sync: in-flight classifier counts done TODO rows (stale chips)

check: export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared && cargo test --test site_sync

estimate: ~120 lines (grep fix + FEATURES annotations + pins)

## Concern

Operator 2026-10-01: chug.sh shows "Session fork in-flight" and "Web
search in-flight" — both shipped phase 1 (F6 T105, cycle 58; F12 T180,
released v0.11.0). Two compounding causes:
1. The in-flight refinement (scripts/site-sync.sh:354-361) greps ALL of
   TODO.md for the F-id — done rows keep their F-references forever
   (T105's done row mentions F6; T180's mentions F12), so any feature
   ever touched by a completed item renders in-flight permanently.
2. The landed awk test (site-sync.sh:237-262) requires literal uppercase
   `LANDED` or strikethrough in the name cell; F6's row says "phase 1
   landed" (lowercase — off-convention; F3-F5/F7-F9/F11 all use LANDED)
   and F12's row never received its phase-1 landed annotation when T180
   shipped.

## Repo context

- scripts/site-sync.sh: landed awk at ~237-262; in-flight refinement at
  ~354-361 (already skips rows classified landed).
- tests/site_sync.rs — existing region pins; classifier pins belong here.
- FEATURES.md annotation convention: "phase N LANDED (T###, <sha>, cycle
  NN)" uppercase (F3-F5, F7-F9, F11 are the exemplars).
- F14 is the control case: OPEN rows T184 + T192 reference F14 —
  in-flight is CORRECT there and must survive this fix.

## Requirements

1. The in-flight grep considers only OPEN TODO rows (status cell `todo`
   or `in-progress`) — never done rows. F14 must still render in-flight
   via open T184/T192.
2. FEATURES.md content fixes in the same item: F6 row — bring the
   phase-1 annotation to convention (`phase 1 LANDED (T105, dc29137,
   cycle 58)`, uppercase); F12 row — add "phase 1 LANDED (T180, e438685
   / v0.11.0, cycle 83)" ahead of the phase-2 deferred clause.
3. Landed-wins ordering preserved: a row classified landed skips the
   in-flight refinement (current behavior — keep, pin it).
4. After merge, the next wrap's site-sync run regenerates the region —
   F6 + F12 landed, F14 in-flight. Do NOT hand-edit the site repo; the
   region is generated.

## Tests

- tests/site_sync.rs pins: (a) done-row-only F-reference does NOT yield
  in-flight; (b) open-row F-reference yields in-flight; (c) a SPLIT row
  with uppercase LANDED + an open referencing TODO row still renders
  landed (landed wins).
- Acceptance: chug.sh feature grid shows F6 and F12 landed, F14
  in-flight after the next wrap sync (verify in cycle Outcomes).

## Out of scope

- No new badge taxonomy (no "partial" chip); STATS/TIMELINE regions
  untouched; no direct edits to the site repository.
