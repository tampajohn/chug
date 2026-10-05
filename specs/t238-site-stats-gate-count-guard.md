# T238 — site-sync stats: full-suite gate count reads subset commits (113 vs ~1500)

check: export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-validate-a; touch src/*.rs tests/*.rs; cargo test --test site_sync

estimate: ~100 lines (scraper guard + pins)

## Concern

Operator 2026-10-05: "the site seems out of date" — chug.sh's stats
region showed "113 tests green at the newest full-suite gate count in a
commit message (ab93f94)" when the full suite is ~1500. The gate-count
scraper (scripts/site-sync.sh stats region) takes the newest commit
message carrying a gate count WITHOUT checking it is the FULL suite:
subset runs (single-package gates, functional-test-only counts,
per-family pins) masquerade as the suite total.

## Repo context

- The wrap gate output names the full count in a stable form
  ("post-merge gates green (1496/1496)" and release builds
  "1467/1467" — the N/N shape with matching operands).
- Subset commits use mismatched or partial forms ("113 tests",
  "site_sync 22/22", "loop_spec 5/5").
- T205-era no_secret_spill walk already filters target* dirs from one
  scraper; this is the same class for the gate count.

## Requirements

1. The scraper accepts only N/N counts with EQUAL operands above a
   floor (say 500, above every package subset) OR a count explicitly
   labeled full-suite in the commit; else it keeps walking older
   commits and, failing all, prints the last known-good count rather
   than a subset number.
2. Pin: a fixture log where the newest count is "113 tests" and an
   older one is "1496/1496" -> the region shows 1496.
3. Pin: equal-operand subsets (site_sync 22/22) are not accepted.

## Tests

- The two pins above in tests/site_sync.rs fixtures.

## Out of scope

- Changing how the wrap records gate counts (the scraper reads what
  exists); live-suite verification (the number stays commit-sourced).
