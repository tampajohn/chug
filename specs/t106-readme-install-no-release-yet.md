# T106 — README Install section: name the pending first release (quickstart truth)

check: grep -q 'no release published yet' README.md && cargo test --test todo_consistency

## Repo context

Cycle-58 eval §6(e) (EVALUATION.md — README quickstart-truth bite): the
Install section (T100) presents the release surface as working TODAY —
"Releases are cut on `v*` tags … The one-liner … `curl -fsSL
https://chug.sh/install.sh | sh`" — but **no `v*` tag exists**
(`git tag -l 'v*'` is empty; T100's bootstrap deliberately holds the
first tag for the operator, carried in the cycle-56/57 wraps). A cold
reader running the one-liner today hits the
`releases/latest/download/...` 404 — the install.sh failure legs name
the problem, but the README's command fails AS WRITTEN, which is exactly
the §6(e) class (commands must work as written, in the order given).
The fix is one honesty sentence, not a release-process change: the
bootstrap timing stays the operator's call.

## Requirements

1. ONE sentence added to the README's `## Install` section, inside the
   first paragraph (after the "Releases are cut on `v*` tags …" sentence
   and before the one-liner code block), containing the exact needle
   `no release published yet` — e.g. "Until the first tag is cut there
   is **no release published yet** — the one-liner and tarball links
   below 404 until then; use the from-source install at the bottom of
   this section." (impl's wording, needle verbatim).
2. Every other Install line byte-identical: the one-liner, the
   per-platform tarball block, the Gatekeeper note, and the from-source
   block are all untouched (they become true the moment the first tag
   lands; the sentence stays honest after — a release page existing
   doesn't invalidate "until the first tag is cut there was none" only
   if reworded conditionally — use wording that is TRUE both before and
   after the first tag: the conditional form above).
3. No other file touched. No pin in tests/ quotes the Install section
   (verified at filing: `grep -rn "install.sh" tests/` matches only
   site-sync/install-fixture legs that never read README's Install
   paragraph) — the impl re-verifies and, if a pin casualty appears,
   updates it minimally with a RED proof.

## Tests

- Docs-only round (README.md only) → the LOOP-SPEC §2 step-3 reduced
  gate set: the guard floor `cargo test --test todo_consistency` (the
  `check:` line above) — full cargo gates skipped per the md-only
  classification.
- The grep needle in the `check:` line is the pin.

## Acceptance

- The `check:` line passes verbatim in the worktree.
- Diff is README.md ONLY (one sentence).
- Validation routing: docs-only → kimi SKIPPED per T16/T31/T97
  precedent; orchestrator gate + diff review suffices.
