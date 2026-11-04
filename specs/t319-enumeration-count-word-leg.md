# t319 — wrap_assert fill enumeration count-word leg (the enumeration surface's last unpinned element)

One concern: the wrap fill's FINAL datum block enumerates the stream's
`ok:false` legs with BOTH a numeric figure (`ok:false K == K`) and a
count-WORD rendering (the head `TWELVE legs — …` and the tail
`ALL TWELVE designed … legs`) — and the count-word surface has NO
mechanical leg. The T310 enumeration leg (`assert_fill_enumeration`)
pins the FIGURE only; the word rides prose discipline. The
corridor-figure falsification class's FIFTH committed sighting landed on
exactly this surface: the cycle-2955 wrap fill (652fd7c) committed
`ok:false 12 == 12 [the enumeration re-run off the stream's own flags
per T272/T310 at the fill stamp: ELEVEN legs — … — ALL ELEVEN designed
read-only zero-mutation 0-REAL legs […]]` at its named
2026-11-04T17:51:05.800Z fill stamp while the mechanical enumeration at
that same stamp reads TWELVE legs (jq
`select(.type=="tool_result" and .ok==false)` count-12 — all twelve
timestamps the fill itself enumerates). The figure `12 == 12` is TRUE
(the T310 leg's own designed guard-fire caught the stale figure-10
pre-commit; the final recompose corrected the figure to 12 after the
T310 fire leg itself landed as the 12th), but the count-WORD stayed
behind at ELEVEN — head and tail both. Caught one chain later by the
digest backstop's designed net at the 709th eval (the 679th
establishment's whole-final re-derivation reading ok-false 12 against
the fill's committed word) — zero casualty, corrected forward per T70 at
the cycle-2956 wrap. This spec lands the mechanical compose-time check
for the enumeration count-word surface, mirroring T310's enumeration
leg exactly.

## Repo context

- The class evidence (the falsification-watch bullet in
  `.chug/eval-state.md`): T305's origin window (three committed corridor
  texts falsified at their named stamps — the cycle-1986 eval's find);
  the cycle-2947 wrap's SECOND and THIRD committed sightings (the 2919
  one-liner's 5 falsified groups + the T184 mis-quote, both corrected
  forward per T70); the cycle-2951 wrap fill's FOURTH committed sighting
  (the trim count — T317 landed the trim leg); the cycle-2955 wrap
  fill's FIFTH committed sighting (this spec's firing evidence — the
  FIRST on the enumeration count-word surface). The enumeration
  surface's elements: the FIGURE (pinned by T310's
  `assert_fill_enumeration`), the enumerated legs' timestamps (prose,
  the enumeration's payload, stays unpinned by design), and the
  count-WORD (this spec's surface — the last unpinned element).
- The mirror: `assert_fill_enumeration` in `scripts/wrap_assert.py`
  (line ~1479) is the shape to mirror — the fill text's enumeration
  figure is asserted against a FRESH mechanical count probed by the CALL
  SITE at the fill stamp and passed IN as the probed actual (the T273
  letter: arguments are the probed actuals; the function never
  re-derives the expectation nor runs the probe itself). Its docstring
  records the T272 false-at-write class's cycle-2244 fire; the new leg's
  docstring records THIS class's cycle-2955 count-word fire.
- The probed count the call site passes is the SAME probe the T310 leg
  already runs (`jq -c 'select(.type=="tool_result" and .ok==false)'
  .chug/events.jsonl | wc -l`, the count captured into a shell variable,
  probed values riding argv/env into the script per T282's channel half,
  never literal-composed) — the word leg takes the same
  `(text, probed_count, probed_stamp)` triple; no new probe is added.
- The asserted surface: the corpus's FINAL-enumeration count-word
  renderings — the head form `… at the fill stamp: <WORD> legs` (and
  the singular `ONE leg`) and the tail form `ALL <WORD> designed …
  legs`. The word table is the corpus's observed vocabulary: ZERO
  through TWENTY (fail-closed on an unknown word, naming it). Every
  count-word occurrence found inside an enumeration block must map to
  the probed count for that block to pass; the leg keeps the datum
  leg's any-block semantics (a text legitimately quoting an earlier
  stream's enumeration stays green because the FINAL datum's own
  enumeration rides beside it). Zero word-bearing enumeration blocks is
  the named-missing fire — a datum-less fill is never a silent pass.
- The module shape: `scripts/wrap_assert.py` is dependency-free /
  stdlib-only. Exports ride the module's `__all__` (18 entries today,
  post-T317). The module docstring's coverage sentence ("Coverage (run
  by the check line): python3 -m unittest scripts/test_wrap_assert.py -v
  — the pre-T295 legs plus the validate_state facade legs, the T309
  fill-datum legs, and the T310 fill-enumeration legs, the T311 on-disk
  state-schema legs, and the T312 always-on marker legs, and the T316
  Outcomes one-liner figure-audit legs, and the T317 fill
  trim-enumeration legs.") is the pin-surface this spec reconciles —
  EXTENDED, not re-shaped (append ", and the T319 fill enumeration
  count-word legs" in the same sentence).
- The T285 pin-surface probe: `grep -rn 'matches("' tests/*.rs` — no
  Rust pin names wrap_assert's `__all__` or the docstring coverage
  sentence (the only wrap_assert mentions in tests/ are comments). The
  python battery is unittest (`python3 -m unittest discover -s scripts
  -p 'test_*.py'`). No exact-count census pins the test total.
- The LOOP-SPEC wiring (the doctrine half): LOOP-SPEC's "real-eval
  (trip) wrap" section carries the fill-compose assert sentences — the
  T309 datum leg sentence, the T310 enumeration leg sentence ("The fill
  compose leg's pre-commit assert ALSO re-enumerates the stream's
  `ok:false` legs at the fill stamp in the SAME leg …"), and the T317
  trim sentence. This spec inserts ONE parallel sentence for the
  count-word leg immediately AFTER the T310 enumeration sentence (the
  enumeration cluster stays together, before the T317 trim sentence) —
  the same compose-time catch for the enumeration count-word surface. A
  LOOP-SPEC edit makes this row DOCTRINE (SOLO, no overlap, kimi
  REQUIRED per the loop/spec doctrine validation list).

## Requirements

1. New leg `assert_fill_enumeration_word(text, probed_count,
   probed_stamp)` in `scripts/wrap_assert.py`, mirroring
   `assert_fill_enumeration`'s contract: *text* is the composed
   fill/wrap TEXT (a str — non-str is the call-signature slip shape,
   TypeError naming the accepted shape); *probed_count* is the
   `ok:false`-leg count the CALL SITE probed from the LIVE events
   stream at the fill stamp (the T310 probe above, passed IN, never
   re-derived inside); *probed_stamp* is the fill stamp, named in every
   fire. The leg scans the enumeration blocks (the same
   `_enumeration_blocks` surface the T310 leg scans — import-free, no
   re, the module's hand-scanned idiom) for the count-word renderings
   (the head `<WORD> legs` / `ONE leg` after the stamp colon, and the
   tail `ALL <WORD> designed` when present), maps each word through the
   ZERO..TWENTY table (an unknown word fails closed naming the word),
   and passes iff at least ONE enumeration block's every count-word
   maps to the probed count; otherwise raise AssertionError naming the
   offending word(s) with the mapped value(s), the probed count, the
   stamp, and the remedy — the module's established named-refusal
   style, never a bare traceback, never a silent skip.
2. Export the leg in the module's `__all__` (18 → 19) and extend the
   docstring coverage sentence per repo-context.
3. The leg's docstring records the firing evidence (the cycle-2955 fill
   652fd7c's `ELEVEN legs` head + `ALL ELEVEN designed` tail vs the
   mechanical TWELVE at its own named stamp 2026-11-04T17:51:05.800Z —
   the class's FIFTH committed sighting, the FIRST on the enumeration
   count-word surface; the T310 guard-fire's designed catch corrected
   the figure while the word rode stale), mirroring
   `assert_fill_enumeration`'s cycle-2244 docstring.
4. Tests in `scripts/test_wrap_assert.py`: (a) GREEN — fill-text
   fixtures whose enumeration count-words carry the probed count pass
   (the `TWELVE legs` head + `ALL TWELVE designed` tail at probed 12;
   the singular `ONE leg` at probed 1; `ZERO legs` at probed 0); (b)
   RED-proofs — a fixture shaped on the cycle-2955 fire (head `ELEVEN
   legs` + tail `ALL ELEVEN designed` with probed_count 12) FIRES
   naming the word, the mapped 11, the probed 12, and the stamp; a
   word-mismatch-inside-one-block fixture (head right, tail wrong)
   FIRES; (c) the named-missing fire (an enumeration block with no
   count-word) names the missing surface; (d) the degenerate
   probed_count fail-closed; (e) the call-signature slip legs (non-str
   text) raise TypeError naming the accepted shape; (f) pre-write
   zero-mutation (the module re-read byte-identical after); (g)
   RED-provability by mutants run on a COPY (the word-compare deleted →
   the RED fixtures' AssertionErrors disappear under the mutant; the
   module byte-identical after).
5. The LOOP-SPEC wiring sentence (the doctrine half): insert ONE
   parallel sentence into the fill-compose assert paragraph,
   immediately after the T310 enumeration sentence and before the T317
   trim sentence — the fill compose leg's pre-commit assert ALSO
   re-verifies the enumeration block's count-WORD at the fill stamp in
   the SAME leg (the same probed count the T310 leg takes, riding
   argv/env) and verifies the fill text's enumeration count-word
   renderings carry the probed count via `scripts/wrap_assert.py`'s
   enumeration count-word leg (`assert_fill_enumeration_word`, T319:
   the cycle-2955 fill 652fd7c wrote `ELEVEN legs` at its named stamp
   against the mechanical TWELVE — the class's fifth committed
   sighting, the first on the enumeration count-word surface) — the
   same compose-time catch for the enumeration count-word surface.

## Tests

`python3 -m unittest discover -s scripts -p 'test_*.py'` from the worktree
root — all green (the current battery + the new tests).

## Acceptance

- The leg exists, is exported (`__all__` 18 → 19), and the docstring
  coverage sentence names the T319 legs.
- The python battery is green with the new tests; each RED-proof fixture
  kills (AssertionError naming the word, the mapped value, the probed
  count, and the stamp).
- The LOOP-SPEC fill-compose assert paragraph carries the new T319
  sentence, verbatim-shaped on the requirement above, positioned after
  the T310 sentence and before the T317 sentence.
- No other repo file changes.

check: python3 -m unittest discover -s scripts -p 'test_*.py'

estimate: ~85 changed lines (helper ~35 + tests ~45 + LOOP-SPEC sentence ~10)
