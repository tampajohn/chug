# T249 — internal-info lint: the repo can never re-accumulate org details

check: cargo test

estimate: ~150 lines (lint walk + pattern list + pins)

## Concern

Operator 2026-10-06: "safety concerns (internal info shared about VA)
in the repo, let's clean that up." The cleanup landed (operator pass,
72201fb): an internal proxy hostname in src comments + a spec, org
HF-org names, org-private/fleet framings, and internal-codename
references across specs/runbooks/TODO/DEPENDENCIES/test fixtures. The
repo is public — the class must not regrow. Today only secrets are
linted (no_secret_spill); org-identifying content has no guard.

## Repo context

- The cleanup's pattern inventory (what got redacted):
  `videoamp-internal`, `videoamp/<repo>` org prefixes, `internal-monorepo`,
  `org-private`/`org-gated`/`internal machine fleet`, internal monorepo
  codenames, `*.internal.com` hostnames. Allow-listed: `LICENSE` (MIT
  attribution), the word VideoAmp in the LICENSE only, generic product
  names (Langfuse, Artifactory-as-product, Hugging Face), the org's
  PUBLIC names already intended for the site footer.
- no_secret_spill (T205 req 5) is the adjacent precedent: a suite-side
  walk over tracked files for token shapes. This is the org-info
  sibling walk.
- Loop-authored content is the main ingress (evals quote operator
  context; specs get written with internal names).

## Requirements

1. A suite-side lint (cargo test, same shape as the secret walk):
   every TRACKED file (not .lock) is scanned for the pattern list;
   a hit fails the suite with the file:line named. The pattern list
   lives in the lint with a comment pointing at this spec (single
   source).
2. Allow-list mechanism: explicit per-file or per-line exceptions
   (LICENSE only today) — no substring-broad exemptions.
3. todo_consistency and the wrap gates run it automatically (it is a
   normal test target); loopd dispatch docs note that filings must
   pass it.
4. Pin: a fixture file containing `videoamp-internal` fails; LICENSE
   passes; `org/laya-judge` passes (already-redacted form is legal).

## Tests

- The three pins above.

## Out of scope

- History scrubbing (a git filter-repo + force-push is an operator
  decision, explicitly out — the lint guards forward-only);
  scanning the site repo or other repos (this covers tampajohn/chug
  tracked content only).
