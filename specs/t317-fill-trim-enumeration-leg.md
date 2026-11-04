# t317 — wrap_assert fill trim-enumeration leg (the third flag surface)

One concern: the wrap fill's FINAL datum block enumerates the stream's
trim-fire count (`trim N == N`), and that surface has NO mechanical leg —
the fill compose's pre-commit assert battery covers the datum figures
(T309's `assert_fill_datum`: iters / wall-seconds / both stream anchors)
and the `ok:false` enumeration (T310's `assert_fill_enumeration`), but the
trim enumeration rides prose discipline only. The corridor-figure
falsification class's FOURTH committed sighting landed on exactly this
surface: the cycle-2951 wrap fill (51f2f7f) committed
`trim 0 == 0 [… on THIS stream's trim-0 at the stamp]` at its named fill
stamp 2026-11-04T13:47:56.795Z while the mechanical re-run at that same
stamp reads `trim: 1` — the trim leg 2026-11-04T13:46:55.188Z fired ~62s
BEFORE the stamp (the notes' `trim 0 == 0` at ITS stamp 13:43:57Z was TRUE;
the leg fired between the two stamps). Caught one chain later by the
digest backstop's designed net at the trip-699 eval — zero casualty,
corrected forward per T70 at the cycle-2955 wrap. The SECOND trim-surface
sighting (the first was the cycle-1982 fill's `trim 0 == 0` inside the
T305 window). This spec lands the mechanical compose-time check for the
trim surface, mirroring T310's enumeration leg exactly.

## Repo context

- The class evidence (the falsification-watch bullet in
  `.chug/eval-state.md`): T305's origin window (three committed corridor
  texts falsified at their named stamps — the cycle-1986 eval's find,
  including the cycle-1982 fill's `trim 0 == 0`, the FIRST trim-surface
  sighting); the cycle-2947 wrap's SECOND and THIRD committed sightings
  (the 2919 one-liner's 5 falsified groups + the T184 mis-quote, both
  corrected forward per T70); the cycle-2951 wrap fill's FOURTH committed
  sighting (this spec's firing evidence — the SECOND on the trim
  surface). The trim surface is the only one of the datum block's three
  stream-flag enumerations (ok:false / trim / the ctx pair) without a
  mechanical leg; the ctx pair stays prose-guarded (zero committed
  sightings to date) and is explicitly OUT of scope.
- The mirror: `assert_fill_enumeration` in `scripts/wrap_assert.py`
  (line ~1477) is the shape to mirror — the fill text's enumeration
  figure is asserted against a FRESH mechanical count probed by the CALL
  SITE at the fill stamp and passed IN as the probed actual (the T273
  letter: arguments are the probed actuals; the function never re-derives
  the expectation nor runs the probe itself). Its docstring records the
  T272 false-at-write class's cycle-2244 fire; the new leg's docstring
  records THIS class's cycle-2951 trim fire.
- The mechanical probe the call site runs (mirroring the T310 paragraph's
  jq shape): `jq -c 'select(.type=="trim")' .chug/events.jsonl | wc -l`
  (the trim events carry `{"type":"trim","ts",...}` — verified against
  the live stream; the count captured into a shell variable, probed
  values riding argv/env into the script per T282's channel half, never
  literal-composed). The asserted surface is the fill's FINAL-enumeration
  rendering `trim K == K` (and the bare `trim K` head-figure), the same
  surface shape T310 asserts for `ok:false`.
- The module shape: `scripts/wrap_assert.py` is dependency-free /
  stdlib-only. Exports ride the module's `__all__` (17 entries today).
  The module docstring's coverage sentence ("Coverage (run by the check
  line): python3 -m unittest scripts/test_wrap_assert.py -v — the
  pre-T295 legs plus the validate_state facade legs, the T309 fill-datum
  legs, and the T310 fill-enumeration legs, the T311 on-disk
  state-schema legs, and the T312 always-on marker legs, and the T316
  Outcomes one-liner figure-audit legs.") is the pin-surface this spec
  reconciles — EXTENDED, not re-shaped (append ", and the T317 fill
  trim-enumeration legs" in the same sentence).
- The T285 pin-surface probe: `grep -rn 'matches("' tests/*.rs` — no Rust
  pin names wrap_assert's `__all__` or the docstring coverage sentence
  (the only wrap_assert mentions in tests/ are comments). The python
  battery is unittest (`python3 -m unittest discover -s scripts
  -p 'test_*.py'`). No exact-count census pins the test total.
- The LOOP-SPEC wiring (the doctrine half): LOOP-SPEC's "real-eval
  (trip) wrap" section carries the fill-compose assert sentences — the
  T309 datum leg sentence and the T310 enumeration leg sentence ("The
  fill compose leg's pre-commit assert ALSO re-enumerates the stream's
  `ok:false` legs at the fill stamp in the SAME leg …"). This spec
  appends ONE parallel sentence for the trim leg in the same paragraph —
  the same compose-time catch for the trim surface. A LOOP-SPEC edit
  makes this row DOCTRINE (SOLO, no overlap, kimi REQUIRED per the
  loop/spec doctrine validation list).

## Requirements

1. New leg `assert_fill_trim(text, probed_count, probed_stamp)` in
   `scripts/wrap_assert.py`, mirroring `assert_fill_enumeration`'s
   contract: *text* is the composed fill/wrap TEXT (a str — non-str is
   the call-signature slip shape, TypeError naming the accepted shape);
   *probed_count* is the trim-leg count the CALL SITE probed from the
   LIVE events stream at the fill stamp (the jq probe above, passed IN,
   never re-derived inside); *probed_stamp* is the fill stamp, named in
   every fire. The assertion passes iff at least ONE enumeration block
   in *text* carries the probed count in the `trim K == K` (or bare
   `trim K`) shape; otherwise raise AssertionError naming the probed
   count, the stamp, and the carried figure(s) — the module's
   established named-refusal style, never a bare traceback, never a
   silent skip.
2. Export the leg in the module's `__all__` (17 → 18) and extend the
   docstring coverage sentence per repo-context.
3. The leg's docstring records the firing evidence (the cycle-2951 fill
   51f2f7f's `trim 0 == 0` vs the mechanical `trim: 1` at its own named
   stamp 2026-11-04T13:47:56.795Z — the class's FOURTH committed
   sighting, the SECOND on the trim surface after the cycle-1982 fill
   inside the T305 window), mirroring `assert_fill_enumeration`'s
   cycle-2244 docstring.
4. Tests in `scripts/test_wrap_assert.py`: (a) GREEN — a fill-text
   fixture whose `trim K == K` enumeration carries the probed count
   passes; (b) RED-proofs — a fixture whose trim figure disagrees with
   the probed count FIRES (AssertionError naming the probed count and
   the stamp); a stale-figure fixture shaped on the cycle-2951 fire
   (`trim 0 == 0` with probed 1) FIRES naming both figures; (c) the
   call-signature slip leg (non-str text) raises TypeError naming the
   accepted shape.
5. The LOOP-SPEC wiring sentence (the doctrine half): append ONE
   parallel sentence to the fill-compose assert paragraph, after the
   T310 enumeration sentence — the fill compose leg's pre-commit assert
   ALSO re-enumerates the stream's trim-fire legs at the fill stamp in
   the SAME leg (the jq probe above, count captured into a shell
   variable, probed values riding argv/env) and verifies the fill text's
   trim enumeration figure carries the probed count via
   `scripts/wrap_assert.py`'s trim-enumeration leg
   (`assert_fill_trim`, T317: the cycle-2951 fill 51f2f7f wrote
   `trim 0 == 0` at its named stamp against the mechanical 1 — the
   class's fourth committed sighting, the second on the trim surface) —
   the same compose-time catch for the trim surface.

## Tests

`python3 -m unittest discover -s scripts -p 'test_*.py'` from the worktree
root — all green (the current battery + the new tests).

## Acceptance

- The leg exists, is exported (`__all__` 17 → 18), and the docstring
  coverage sentence names the T317 legs.
- The python battery is green with the new tests; each RED-proof fixture
  kills (AssertionError naming the probed count and the stamp).
- The LOOP-SPEC fill-compose assert paragraph carries the new T317
  sentence, verbatim-shaped on the requirement above.
- No other repo file changes.

check: python3 -m unittest discover -s scripts -p 'test_*.py'

estimate: ~80 changed lines (helper ~30 + tests ~40 + LOOP-SPEC sentence ~10)
