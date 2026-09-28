# T127 — README Install section: first release IS published (staleness fix)

check: cargo test --test todo_consistency --test readme_layout

## Repo context

README.md:13-19 (Install section) still carries the pre-release
honesty paragraph: "Until the first tag is cut there is **no release
published yet** — the one-liner and tarball links below 404 until
then; use the from-source install at the bottom of this section."
That was true when written and stayed true through the cycle-62
README audit (`git tag -l 'v*'` empty at that filing). It is FALSE
now: the operator bootstrapped the tag line on 2026-09-28
(v0.1.0 → v0.2.0 → v0.2.1; the v0.1.0/v0.2.0 workflow legs failed and
were fixed operator-side — 3cd7b7e CI cross libc, 1e84357 lock-sync),
and **v0.2.1 is published and green**: release run 36438268858
completed success, `gh release view v0.2.1` lists all six assets
(macos-arm64 / linux-x86_64 / linux-aarch64 tarballs + sha256s), and
`curl -fsSL https://chug.sh/install.sh` serves the installer
(verified at filing, cycle-64 eval). A new user reading the front
door today is told the working one-liner 404s — the worst seat in the
README for a stale claim (META-META-SPEC §6(c): superseded behavior
presented as current).

Docs-only round (README.md only, one paragraph rewrite): LOOP-SPEC's
T80 guard floor applies at review/merge; no doctrine carrier is
touched (the Install paragraph is unpinned — verified by grep at
filing: the "no release published yet" text appears ONLY in
README.md); `tests/readme_layout.rs` pins the Development layout
line, which this edit does not touch, and rides the check line as
cheap insurance.

estimate: ~8 changed lines

## Requirements

1. README.md Install section: delete the "Until the first tag is cut
   there is **no release published yet** — the one-liner and tarball
   links below 404 until then; use the from-source install at the
   bottom of this section." sentence pair and adjust the surrounding
   paragraph so it reads truthfully WITHOUT hardcoding a version
   number that goes stale again — the durable claims: releases are
   cut on `v*` tags and published to GitHub Releases (the loop tags
   at wrap per LOOP-SPEC); each release publishes the three platform
   tarballs + sha256s, built `--release --locked` from the tagged
   commit; the one-liner installs the latest release. The
   Gatekeeper/unsigned paragraph and the from-source block stay
   byte-identical (both still true and useful).
2. No version literal (`v0.2.1`, `0.2.x`) in the new text — the claim
   must age correctly (the tag itself is the moving part; GitHub's
   `releases/latest` already redirects).
3. No other README change; no spec/doctrine/TODO edits (the row flip
   is the orchestrator's, as always).

## Tests

Docs-only: `cargo test --test todo_consistency --test readme_layout`
is the gate (the T80 floor plus the README pin carrier as insurance).
No new legs — the risk surface (a broken markdown table or layout
line) is what those two files already cover; the paragraph content
itself is deliberately unpinned (prose that pins itself to "a release
exists" would go stale the same way).

## Acceptance

- The Install section no longer claims releases are unpublished, and
  contains no version literal that can stale.
- `cargo test --test todo_consistency --test readme_layout` green.
- Read top-to-bottom as a newcomer: Install → one-liner works as
  written (it does — verified at filing).

## Out of scope

Reworking the Install section structure; site content (chug.sh
mirrors ride scripts/site-sync.sh on merge — no manual site edit);
the Gatekeeper wording; pinning release prose.
