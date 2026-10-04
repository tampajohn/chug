# T99 — site-sync v2: timeline + feature-grid regions (deterministic)

check: cargo test

## Concern

Operator 2026-09-27: "keep the site up to date / reflective of the work
we're doing / timeline etc." T98 landed the STATS:BEGIN/END block but its
out-of-scope deferred timeline curation — and the site's timeline (16
entries, hand-built 2026-09-27) and feature grid are already aging. Every
landed item should appear on chug.sh without a human noticing. Same
doctrine as T98: DETERMINISTIC derivation from sources of truth, zero LLM,
facts only — prose titles come from TODO row titles (already curated by
the loop); the script never writes narrative.

## Repo context

- `scripts/site-sync.sh` (T98): regenerates the STATS region at wrap.
- chug-site/index.html: timeline section (16 entries, class structure from
  the timeline child), feature grid (14 cards, FEATURES.md-derived).
- FEATURES.md: roadmap with check-off annotations on landed items (the
  orchestrator checks items off at merge per doctrine).
- TODO.md done rows: `| T<n> | <title> | <spec> | <pri> | done | done
  <commit> — <note>` — title + commit + date are mechanical.

## Requirements

1. `<!-- TIMELINE:BEGIN/END -->` region: script appends one entry per
   done row not already present (keyed by commit ref): `<date> — <title>
   (<short-commit>)`, newest first, matching the existing timeline CSS
   classes. Cap: latest 20 entries full; older entries collapse to a
   single "…and N earlier milestones (T1–T<n>)" line. Idempotent —
   re-running with no new done rows changes nothing.
2. `<!-- FEATURES:BEGIN/END -->` region: grid regenerated from
   FEATURES.md — each F-item gets its name + one-line what + status badge
   (landed = checked off in FEATURES.md / in-flight = a TODO row references
   it / queued). Card order and CSS classes preserved from the current
   grid's structure.
3. T98's rules hold for both regions: byte-identical when inputs
   unchanged; commit+push only on change; best-effort never fails a cycle.
   T217: the fail-closed input guard is GLOBAL across regions (all three
   read the same repo) — TODO.md unreadable, .chug/loopd absent, or git
   log empty refuses the whole publish (write nothing, commit nothing,
   one named error, nonzero exit); an explicit --bootstrap flag is the
   first-ever-run-on-a-genuinely-empty-repo escape.
4. Markers added to index.html in a bootstrap commit on the site repo if
   absent (the timeline child's existing entries become the initial region
   content, deduped against TODO-derived entries by commit ref).
5. Facts-only audit: every rendered entry must cite a commit ref that
   exists in the chug repo (script verifies with cat-file before write).

## Tests

- Fixture TODO.md (done rows) + FEATURES.md → expected timeline entries
  (dates/refs correct, dedupe against existing region, cap behavior at
  20+N collapse).
- FEATURES badge logic: checked-off → landed; row-referenced → in-flight;
  else queued.
- Idempotence: second run byte-identical.
- Acceptance: next landed item appears on chug.sh timeline within one
  cycle wrap (recorded in Outcomes).

## Out of scope

- Narrative/prose edits to other sections (hero, proof, doctrine, get
  started); timeline entry titles beyond TODO row titles; the site build
  child pattern (this is script work, no chug sessions on the site repo).
