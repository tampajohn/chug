# T249 — internal-info lint: the repo can never re-accumulate org details

check: export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared; touch src/*.rs tests/*.rs; cargo test --test internal_info_lint --test todo_consistency

estimate: ~150 lines (lint walk + pattern list + pins)

## Concern

Operator 2026-10-06: "safety concerns (internal info shared about the
org) in the repo, let's clean that up." The cleanup landed (operator
pass 47cd0d1, amended from an earlier local pass whose commit message
named the literals — that earlier object is local-only dangling
history; scrubbing it is an operator decision, out of scope): an
internal proxy hostname in src comments + a spec, org HF-org names,
org-private/fleet framings, and internal-codename references across
specs/runbooks/TODO/DEPENDENCIES/test fixtures. The repo is public —
the class must not regrow. Today only secrets are linted
(no_secret_spill); org-identifying content has no guard.

## Pattern list (AMENDED at dispatch, cycle 128 — derived from the 47cd0d1 diff, not the lossy first inventory)

The first filing's inventory listed the redaction's REPLACEMENT
vocabulary (`org-private`, `internal-monorepo`, `org-gated`,
`internal machine fleet`) as patterns — wrong: those are the
generalized forms the operator introduced, they are legal and appear
everywhere. The patterns are the PRE-redaction forms. This spec names
them by CLASS and match rule only (the spec itself must stay
redacted — it is a tracked file the lint scans); the LINT SOURCE is
the single source of the exact literals (req 1), each entry carrying
a reason comment and a pointer to this spec + to `git show 47cd0d1`
(the diff's before/after pairs are the derivation record):

1. the org NAME (case-insensitive word) — the operator's company name;
   allow-listed in LICENSE only (MIT attribution).
2. the org's two-letter fleet abbreviation (case-sensitive,
   word-bounded) — word-boundary so hex shas and prose never trip.
3. the internal monorepo codename (case-insensitive) — one word.
4. the monorepo's intermediate name (case-insensitive).
5. an internal machine codename (letter+digits, case-sensitive,
   word-bounded).
6. the internal proxy hostname (exact literal).
7. the internal monorepo repo name (caps form; the lowercase
   after-form is legal and must pass).
8. internal DNS hostnames: `[a-z0-9][a-z0-9.-]*\.internal\.com\b`
   (trailing word-boundary so `x.internal.company` does not match).

EXCLUDED with reason (must NOT be patterns): the `dashd` session-role
string — a public code contract (SESSION_ROLES in src/daemon.rs:730,
the external watch consumer registers with it; the operator's own
sweep kept it and only generalized prose to "dashboard"); the
replacement vocabulary `org-private`/`org-gated`/`internal-monorepo`/
`internal machine fleet`/`org/…` forms; `tampajohn` (public GitHub
username); Langfuse / Artifactory / Hugging Face (public products).

The child derives the exact literals from `git show 47cd0d1` (before
lines) + the pre-amend message inventory BEFORE writing the lint, then
validates zero-hit-at-HEAD: every pattern must have zero hits in
tracked files once this arc lands (LICENSE exempt). A pattern that
hits legal text is over-broad — refine its rule; a pattern that hits
real residue means the residue gets redacted in this same arc (below).

## Repo context

- no_secret_spill (T205 req 5) is the adjacent precedent: a suite-side
  walk over the tree for token shapes. This is the org-info sibling
  walk — read its shape first (collect files, per-line match, report
  file:line NEVER the content, self-file excluded by exact filename).
- Walk TRACKED files (`git ls-files -z` from the test cwd = package
  root; never CARGO_MANIFEST_DIR per the T48 pin), skip `Cargo.lock`
  and any `*.lock`, skip the lint's own source file (its pattern
  literals are the point, the no_secret_spill.rs self-exclusion is the
  precedent). UTF-8-unreadable files are skipped.
- Loop-authored content is the main ingress (evals quote operator
  context; specs get written with internal names).

## Requirements

1. A suite-side lint (cargo test target `internal_info_lint`): every
   tracked file (not .lock) scanned for the pattern list; a hit fails
   the suite with the file:line named (never the line content). The
   pattern list lives in the lint with a comment pointing at this spec
   (single source).
2. Allow-list mechanism: explicit per-file or per-line exceptions
   (LICENSE only today) — no substring-broad exemptions.
3. todo_consistency and the wrap gates run it automatically (it is a
   normal test target); loopd dispatch docs note that filings must
   pass it.
4. Pins (in-test, synthetic/tempdir fixtures — NEVER tracked fixture
   files, which would trip the lint they test): a planted org-name
   literal fails; LICENSE passes BECAUSE of the allow-list (and the
   pin proves the allow-list is load-bearing: without it LICENSE would
   hit); `org/laya-judge` passes (already-redacted form is legal);
   matcher unit pins for the word boundaries (a hex sha containing the
   fleet-abbreviation letters passes; a mid-sha occurrence of the
   machine-codename letters passes; standalone forms fail) and the
   internal-host trailing boundary (a host-shaped string under the
   internal TLD fails; the same name under a longer public TLD
   passes).
5. In-arc redactions (so the lint is green at merge): the runbook
   `runbooks/laya-hf-hosting.md` carries four fleet-abbreviation
   residuals and `specs/t205-hf-org-laya-hosting.md` one — redact them
   to the operator's generalized vocabulary ("org-private org",
   "internal machine fleet"). The orchestrator handles the TODO row
   literal, the loopd.sh prose mention, and this spec's own literals
   as pre-dispatch bookkeeping — the child does NOT touch TODO.md,
   loopd.sh, or README.md.

## Out of scope

- History scrubbing (a git filter-repo + force-push is an operator
  decision, explicitly out — the lint guards forward-only; the
  dangling pre-amend object ages out on gc);
  scanning the site repo or other repos (this covers this repo's
  tracked content only);
- renaming the `dashd` session-role string (public consumer contract).
