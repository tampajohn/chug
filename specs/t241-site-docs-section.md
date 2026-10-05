# T241 — DOCS as a second page (chug.sh/docs.html): runbooks rendered into their own page

check: export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-validate-a; touch src/*.rs tests/*.rs; cargo test --test site_sync

estimate: ~260 lines (region generator + markdown render + nav + pins)

## Concern

Operator 2026-10-05: "build out a docs section of chug.sh, with how to
use it effectively… docs session could be a new page." The main page is
already long; the how-to corpus deserves its own page. The corpus
exists and is maintained: runbooks/ (T191 — quick-task, feature,
repo-eval, loop-ops, adversarial-review) plus README's Quickstart.
Rendering that content into a SECOND site page at wrap time means the
docs can never drift from the repo — one edit in runbooks/ ships to
chug.sh/docs.html on the next wrap.

## Repo context

- runbooks/ files are short, structured markdown (goal template,
  when-to-use, expected arc) — the region renders them as subsections
  under a "docs" anchor.
- The page has an existing section/markup style (wrap, kicker, card
  classes); the render maps markdown to it (headings -> section
  headers, code fences -> the page's code style, lists -> lists).
  Emphasis/escaping per the T99 text rules.
- Marker contract: `<!-- DOCS:BEGIN/END -->`; T217 fail-closed applies
  (missing runbooks/ dir -> skip, no error).

## Requirements

1. A NEW page `docs.html` in the site repo (chug.sh/docs.html):
   fully machine-rendered (no marker regions — the whole file is
   generated), one section per runbooks/*.md file (sorted), each
   rendered from the file's content at sync time — heading, the goal
   template in a code block, when-to-use, expected arc. An index of
   links at the top. Same head/style base as index.html so the two
   pages read as one site.
2. A "Quickstart" section leads docs.html, sourced from README's
   Quickstart section (the single install + first-run block); the main
   page's Quickstart section keeps a one-line pointer to docs.html.
3. The render is deterministic from repo content (byte-stable for
   byte-identical inputs; curated content lives ONLY in runbooks/ and
   README — never in the site repo). docs.html is add/overwrite in the
   site repo (git add docs.html; the T26 plain-push rule unchanged).
4. Nav: index.html gains a docs.html link; docs.html links back
   (docs ⇄ home).
5. tests/site_sync.rs pins: a fixture runbooks dir renders docs.html
   with all files sorted and code fences escaped; missing runbooks/ ->
   docs.html untouched, sync continues; byte-identical re-render
   produces no diff (idempotence); the sync commits BOTH files when
   either changed.

## Tests

- The three pins above; existing region pins stay green.

## Out of scope

- Versioned docs per release (latest-only); search; docs/ directory
  routing (docs.html first — split into per-runbook pages only if the
  corpus outgrows one page); marker-region curation on docs.html (it is
  fully generated).
