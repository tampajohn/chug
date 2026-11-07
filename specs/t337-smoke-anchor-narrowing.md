# t337 — wrap_assert T324 smoke: narrow the own-anchor probe to OWN-establishment claims

estimate: ~40 lines (scripts/wrap_assert.py anchor-probe filter + a RED-proven fixture leg)

## Problem
The T324 live-corpus smoke leg (test_live_corpus_smoke_six_full_set_extracts_and_cross_checks)
derives the kept-full set from the live EVALUATION.md and probes each entry's own-establishment anchor.
The committed cycle-3081 entry carries the phrase "the 710th/711th/712th establishments bracket" inside
its T316-leg prose (a QUOTE of what the one-lined 3073 compaction carried) — the anchor probe reads it
as an OWN-establishment claim, then `_ol_establishment_figures` cannot extract a `<id> stream` X/Y group
after it, and the smoke raises.

Verified pre-existing at ed858fe (the 3086 wrap ran the pre-edit corpus through the extractor and got the
identical failure); the 198-test wrap_assert suite FAILs 1 leg on it.

## Fix
Narrow `_ol_establishment_figures`'s anchor probe: a mention inside a T316-leg QUOTE/bracket clause
(`the <ord>…establishments bracket` shape, or any establishment word inside a parenthetical that also names
the one-lining) is a MENTION, not an own-anchor — the smoke must read it as the designed no-anchor skip.
RED-prove: a fixture entry with only the bracket mention asserts the skip under the fix and the failure
without it; the suite goes 198/198.

check: python3 scripts/test_wrap_assert.py
