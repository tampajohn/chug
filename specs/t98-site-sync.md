# T98 — site-sync: chug.sh stats update at wrap (deterministic)

check: cargo test

## Concern

chug.sh (tampajohn/chug-site, cloned at ~/workspace/chug-site on the loopd
host) is the public face, but its numbers rot: every item the loop lands
makes the site's stats stale. Operator directive 2026-09-27: "add the
requirement for it to update its website." The digest pattern (T46) applies:
a DETERMINISTIC script regenerates the stats block — zero LLM per cycle.

## Repo context

- `scripts/eval-digest.sh`: the deterministic pre-compute pattern (T46).
- `loopd.sh` wrap path: cycles end with goal complete → 60s sleep → next.
- chug-site/index.html: hand-written static page (no build step); the
  site child marked its stats section with ids (hero numbers, proof
  section). chug tools are cwd-confined — the sync is a SCRIPT run by
  loopd/orchestrator via bash, not a chug tool.
- Site repo pushes: gh auth on the host (K7 verified working).

## Requirements

1. `scripts/site-sync.sh`: reads TODO.md + `git log` + EVALUATION.md and
   regenerates ONLY the marked region of ~/workspace/chug-site/index.html
   (`<!-- STATS:BEGIN -->` … `<!-- STATS:END -->` — add the markers to the
   site if absent): items landed (done-row count), current test count
   (from latest todo_consistency/commit message), last-5 landed items with
   refs + dates, cycle count from .chug/loopd logs. Idempotent; byte-
   identical output when inputs unchanged.
2. If the region changed: commit (`site: stats sync <date>`) and push.
   Never force-push; a rejected push warns and exits 0 (not a cycle
   failure — site sync is best-effort, like observability). T217: the
   sync FAILS CLOSED on unreadable repo inputs — TODO.md unreadable,
   .chug/loopd absent, or git log empty means every number in the block
   would be a fallback, so the sync writes nothing, commits nothing,
   prints one named error and exits nonzero (fallback zeros are never
   published over real stats; the values can't tell them apart from a
   genuinely empty repo's first run, so the guard keys on readability).
3. loopd.sh invokes it after each `cycle OK` (one line, failure-tolerant).
4. The stats block text must be FACTS from the named sources — the script
   cites no number it didn't compute from TODO.md/git log/test output.
5. Missing site clone (path absent) → warn once, exit 0.

## Tests

- Shell/Rust integration: fixture TODO.md + git log → expected block
  contents (counts match grep); second run byte-identical (idempotence);
  absent markers → script reports and exits nonzero WITHOUT editing.
- Acceptance: a later cycle's Outcomes records a site-sync commit landing
  on chug-site after an item merge.

## Out of scope

- Timeline curation (prose stays human/spec-driven); design changes;
  updating EVALUATION/FEATURES pages (site has none yet); LLM rewriting.
