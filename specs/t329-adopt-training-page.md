# T329 — chug.sh "Adopt chug" training page

estimate: ~160 changed lines (new adopt.html + site-sync region + nav links on index/docs + content fixtures)

## Why

Operator's Q4 goal: expand autonomous coding tools (chug) to the company.
The public site is the front door — today it has features, docs, and (with
T327) cost proof, but nothing that takes a curious engineer from "what is
this" to "my first loop is running." Training content is the adoption
bottleneck the operator named explicitly.

## What

1. **adopt.html** (new top-level page, same static idiom as docs.html, added
   to the nav on index + docs): four sections —
   - **Quickstart**: the one-command install (`install.sh`), what it puts
     where, first `loopd.sh run`, what healthy looks like in the first 10
     minutes (first cycle, first eval, where the logs live).
   - **Mental model**: driver-in-code (the loop is a program, not a prompt),
     LEDGER as external memory, TODO/specs as the work queue, gates +
     validators as the trust boundary, cheap-exit/dormancy as the cost
     boundary. One diagram (ASCII or inline SVG, no JS).
   - **Your first feature**: filing a spec + TODO row, watching the loop
     pick it up, reading the harvest, what a good first item looks like
     (small, checkable, outward).
   - **FAQ**: cost (link the T327 cost region), safety (link the risk-gate
     docs), model routing (kimi/glm, no Anthropic required), uninstall.
2. **Content source of truth**: markdown fragments under `docs/adopt/` in
   the repo, rendered by site-sync like the docs page — engineers edit
   markdown, the site follows. Fail-closed region semantics per T217.
3. **Internal/external hygiene**: the page must not name internal endpoints
   or company infrastructure (the repo is public; the 2026-10 history scrub
   is the precedent). Internal wiring belongs in the internal onboarding kit
   (P005), not here.

## Tests

- site-sync fixture: adopt.md fragments render to adopt.html; missing
  fragments → placeholder region, deploy still succeeds.
- Link check in the sync: every in-page anchor target exists.
- Hygiene grep in CI gates: adopt.html must not match the redaction
  patterns already used by the T249 internal-info lint.

## Out of scope

- Video/screencasts (static first; media is a follow-up).
- Interactive sandbox (public repo has no hosted backend).
- Internal/company onboarding content (P005's lane).
