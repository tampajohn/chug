# adopt corpus

These fragments render to `https://chug.sh/adopt.html` at every site sync
(`scripts/site-sync.sh`, T329) — edit markdown here, the site follows at
the next wrap. This file is the corpus's own index; it never renders.

The page's four sections are fixed, in this order:

| file | section |
|------|---------|
| `quickstart.md` | Quickstart — install, first run, the continuous loop, what healthy looks like |
| `mental-model.md` | Mental model — the loop, the memory, the queue, the two boundaries |
| `first-feature.md` | Your first feature — filing a row and reading the harvest |
| `faq.md` | FAQ — cost, safety, model routing, uninstall |

The contract, all fail-closed (the T217 letter):

- A missing fragment renders an explicit placeholder region in its slot —
  nothing is ever fabricated into the public page.
- No canonical fragments at all: `adopt.html` is not written and the nav
  links are not bootstrapped — never a link to a page that was not
  rendered.
- Every in-page link must target one of the four section anchors
  (`#quickstart`, `#mental-model`, `#first-feature`, `#faq`) — the render
  emits no heading ids, and the sync refuses to publish a page with a
  dangling anchor (the previously published page rides until the link is
  fixed).
- The sync re-scans the composed page for the T249 redaction patterns and
  refuses to publish on a hit — the repo's internal-info lint covers these
  fragments; that scan covers the rendered page.
- Public-safe only: no internal endpoints, no company infrastructure.
  Internal wiring belongs to the internal onboarding kit, not here.
- Other `.md` files dropped in this directory are warned about and
  skipped: the page renders exactly the four curated sections.
