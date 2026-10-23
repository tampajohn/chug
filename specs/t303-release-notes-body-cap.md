# T303 — release-notes body cap: GitHub 125,000-char release-body limit

spec: LOOP-SPEC.md Phase 2 child spec (glm impl, 80/50 budgets).

## Finding (filed at the cycle-1743 eval)

The tag-triggered release workflow (`.github/workflows/release.yml`, T100) has
FAILED on FOUR consecutive tags — v0.17.19 (run 38282125419, 2026-10-12),
v0.17.20 (run 38694409185, 2026-10-15), v0.17.21 (run 39034279498,
2026-10-17), v0.17.22 (run 39884529107, 2026-10-23) — every one at the
`gh release create` leg with:

```
HTTP 422: Validation Failed (https://api.github.com/repos/tampajohn/chug/releases)
body is too long (maximum is 125000 characters)
```

Root cause: `scripts/release-notes.sh` emits ONE bullet per commit in the
range with the FULL commit subject (`- <subject> (`<sha>`)`), and the loop's
own `eval:`/wrap commit subjects run multi-KB each. The v0.17.21→v0.17.22
window covered 1,122 commits; the generated body measures 1,070,737 bytes
(`scripts/release-notes.sh v0.17.22 | wc -c`) — 8.6× over GitHub's 125,000
limit. Last good release: v0.17.18 (2026-10-11); `install.sh` users are
served a four-tags-stale release. The tags themselves are immutable history
and stand (T245); only the release OBJECTS are missing.

## Remedy (both legs, deterministic)

The script's output is pinned byte-identical on identical inputs
(tests/release_scripts.rs); the cap must keep that property.

1. **Per-subject truncation.** In the four bullet-generating loops, truncate
   each commit subject to a fixed budget (suggested 200 BYTES) appending an
   ASCII ellipsis marker (`...`) when truncated. Loop subjects are the
   multi-KB offenders; ordinary subjects pass through unchanged.
2. **Total-body hard cap.** After the four groups are assembled, if the
   emitted body exceeds a byte budget (suggested 120,000 bytes, comfortably
   under GitHub's 125,000), truncate at a LINE boundary (drop whole trailing
   bullet lines, never a partial line) and append ONE deterministic footer
   line naming the omission — suggested shape:
   `- ... (truncated: N more commits in <range>; full list: git log <range>)`
   where N and the range are computed from the same git data the bullets
   were. Same input → same N → byte-identical output.

Implementation note: the truncation helpers live in `scripts/release-notes.sh`
(POSIX sh, the file's existing dialect — `set -u`, `LC_ALL=C`, no bashisms);
a small awk or sh leg is fine. Keep `scripts/check-tag-version.sh` and the
workflow YAML UNTOUCHED — the fix is generator-side only.

## Tests (tests/release_scripts.rs, the existing pin file)

- A fixture repo (the tests already build synthetic git histories) whose
  range contains a commit with a multi-KB subject → assert the emitted line
  is truncated to the budget + carries the `...` marker.
- A fixture whose generated body would exceed the hard cap (many long-subject
  commits) → assert: output ≤ the cap, exactly ONE truncation footer line,
  the footer's N matches the omitted-bullet count, and no partial line at
  the cut.
- The determinism pin (stdout(A) == stdout(B) across two runs) keeps holding
  on both fixtures and the existing small-history fixture.
- Existing fixtures that fit under the cap keep byte-identical output (no
  footer added, no subject touched).

## Acceptance surface / estimate

- `scripts/release-notes.sh` + `tests/release_scripts.rs` ONLY (no workflow,
  no src/, no doctrine).
- estimate: ~120 changed lines all-in (script ~25, tests ~95).
- Out of scope (named for the operator, NOT this row): backfilling the four
  missing release objects (v0.17.19–v0.17.22) — a `gh release create <tag>
  --verify-tag --notes-file <capped-notes>` per tag is the operator's call;
  this row's fix makes the NEXT tag publish.

check: cd "$(git rev-parse --show-toplevel)" && export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared && touch src/*.rs tests/*.rs && cargo test --release --test release_scripts --test todo_consistency --test internal_info_lint
