# T333 — the T324 battery's stale live-corpus legs (the kept-full window advanced past the fixtures' assumptions)

estimate: ~60 lines (tests-only; scripts/test_wrap_assert.py, no production file)

## The defect

`scripts/test_wrap_assert.py` runs RED on main HEAD: `python3 scripts/test_wrap_assert.py`
reports 195/198 with THREE failures, all in `AssertOutcomesOneLinerAnchorSkipRuleTest`
(the T324 acceptance legs), all live-corpus staleness — the tests assume a corpus
shape the advancing Outcomes window no longer has:

1. `test_red_proof_quoted_mention_skip_disabled_probes_wrong_figure` — reads
   `### Cycle 3025 ` from the LIVE kept-full set via
   `_t324_current_full_set(...)` + `next(...)`; cycle 3025 was one-lined by the
   kept-full-6 rotation cycles ago → `StopIteration` (ERROR). The fixture
   pattern two tests over already uses — read the FULL entry from a COMMITTED
   git ref (`_t324_git_eval_text(ref)`, the 9ae8c1a shape) — is the fix: the
   3025 full text exists at the T324-era refs, only the LIVE window lost it.
2. `test_live_corpus_smoke_six_full_set_extracts_and_cross_checks` — asserts
   EVERY kept-full entry extracts an establishment tuple AND a re-arm figure
   AND re-arm == the DISCHARGED-derived ordinal. The current window is
   routine-skip-dominated: 3075/3074/3072 carry NO own-establishment anchor
   (value None, detail None — the extractor's DESIGNED no-anchor shape) and no
   DISCHARGED keyword, so the smoke's `assertIsInstance(value, tuple)` and the
   `_t324_discharged_ordinal` call fail on LEGITIMATE entries. The smoke must
   treat the no-anchor/no-discharged shape as a skip (the extractor's designed
   semantics), not a failure — while still probing >=1 entry through the FULL
   path (assert probed >= 1, so the smoke cannot go vacuous when the window is
   all-skips; today 3073/3070 exercise it).
3. `test_no_behavior_change_when_first_occurrence_is_the_anchor` — the
   pre-change-vs-shipped sweep over 500+ live groups trips on a corpus entry
   the extractor now reads differently: a one-liner's re-arm group extracts
   `(626,)` shipped vs `(0,)` pre-change (the mutant harness's baseline figure
   vs the corpus's own discharge digits). Adjudicate the entry shape at test
   time the same way the smoke must (skip-legitimate shapes), or scope the
   sweep to the surfaces the no-behavior-change pin actually guards (the
   anchor-first occurrence), with the live-corpus drift carved out explicitly.

## The shape of the fix (tests-only)

- Keep every RED-proven mutant leg's semantics: the mutant
  still fires; the shipped module still passes the pairs it was landed to
  pass. Only the LIVE-corpus selection/expectation logic adapts.
- The two git-ref fixture reads (`_t324_git_eval_text("9ae8c1a")`-style) stay
  byte-exact — they are corpus-position-independent already.
- No production change to `scripts/wrap_assert.py`; no doc/doctrine text
  changes; the battery's own count pins (the `__all__` pins) are untouched.

## check:

check: python3 scripts/test_wrap_assert.py
