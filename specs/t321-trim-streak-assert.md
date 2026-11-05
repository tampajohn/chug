# T321 — wrap_assert trim-streak leg (assert_fill_trim_streak) + LOOP-SPEC fill-assert list

estimate: ~150 changed lines (one assert leg + test legs + one doctrine sentence)

## Repo context

The carried-figure class's TWELFTH sighting fired at the cycle-3024 eval (the
726th, triaged d1793904365-2): the trim-streak accounting parenthetical in
corridor texts is COMPOSED from the prior eval's committed reading and never
mechanically derived, and it compounded across three consecutive establishments:

- the 723rd eval (cycle 3012) committed `TRUE streak HOLDS 0 [the 3008 stream's
  whole-final trim-0 keeps it 0]` — TRUE: moves 0 -> 1 under the increment
  semantics pinned at the 2960/2964 wrap fills (c7dacd7: "moves 0 -> 1 TRUE on
  THIS stream's trim-0 whole-final"; e69701c: "moves 1 -> 2");
- the 724th eval (cycle 3016) committed the same `HOLDS 0` for the 3012
  stream's whole-final trim-0 — TRUE: moves 1 -> 2;
- the 725th eval (cycle 3020) committed `the strict post-reset streak reads 4
  [the 3004/3008/3012/3016 trim-0 chain]` — TRUE: moves 2 -> 3. The 3004
  stream's whole-final trim count is 2 (jq re-derivation off
  events-20261105-124928: legs 2026-11-05T11:35:28.663Z +
  2026-11-05T12:16:19.639Z), and the 722nd eval's OWN committed record CONCURS
  ("the whole-final 2 — ONE post-stamp trim fired", f8cf76b). The 3004 stream
  is therefore NOT part of any trim-0 chain, and the committed "reads 4" is
  falsified against the mechanical record AND internally incoherent against
  its own two committed predecessors (0, 0 -> 4 in one step).

ZERO consequence: the streak is descriptive health accounting — no machine
consumer reads it; every trim FIGURE (the `trim 0 == 0` counts) is byte-TRUE
and T317-leg-asserted at its fill; the falsification is contained to three
committed eval texts' parentheticals plus the 3020 wrap fill's propagated
"reads 5". The designed watch (the next eval's mechanical re-derivation)
caught it one chain later — the class's detection path working.

The class's landing pattern: each NEW surface's first firing lands the assert
leg for that surface — `assert_fill_datum` (T309), `assert_fill_enumeration`
(T310), `assert_fill_enumeration_word` (T319), `assert_fill_trim` (T317),
`assert_state_on_disk` (T311/T312), `assert_outcomes_one_liner` (T316). The
streak parenthetical is the last un-asserted recurring accounting figure in
the fill/eval compose templates. This row is the weighed disposition the
class's TWELFTH-sighting trigger was armed for (the 11th triage,
d1793900863-2, re-armed the trigger at the twelfth).

Existing pieces:

- `scripts/wrap_assert.py` — the assert family. `assert_fill_trim` (:1679) is
  the sibling leg; `assert_fill_datum` (:1282) shows the stamp-scoped
  re-derivation pattern (probed values ride argv/env into the assert, never
  literal-composed per the T305/T282 discipline).
- `scripts/stream_datum.py` — the mechanical whole-stream reader (trim legs
  with timestamps, run_start, anchors) — the re-derivation's flag source.
- `scripts/test_wrap_assert.py` — the unittest surface (160 tests, ~84s wall,
  `python3 scripts/test_wrap_assert.py`).
- The TRUE chain semantics (pinned at c7dacd7/e69701c): the strict post-reset
  streak counts consecutive whole-final trim-0 streams since the last stream
  whose whole-final trim count is >= 1 (a trim>=1 whole-final RESETS/HOLDS-0
  the streak). A fill's stamp-scoped claim ("reads N TRUE on THIS stream's
  trim-0 at the stamp") extends the prior chain by exactly one when the
  current stream's trim count at its own fill stamp is 0.

## Requirements

1. `scripts/wrap_assert.py` gains `assert_fill_trim_streak(text, events_dir,
   fill_stamp)` (the name follows the family). It re-derives the TRUE streak
   MECHANICALLY from the events files alone — never from committed corridor
   texts:
   a. enumerate the rotated `.chug/events-*.jsonl` files in stream order
      (ls/mtime order — the rotation mechanism: each rotated file holds
      exactly one predecessor stream);
   b. verify one `run_start` per rotated file (the T320 identity precondition
      — a file with zero or multiple run_start events is rejected BY NAME,
      not silently miscounted);
   c. per file, read the whole-final trim count via the stream's own flags
      (events of `type == "trim"`);
   d. the TRUE prior chain = the count of consecutive whole-final trim-0
      streams walking BACK from the current stream's predecessor, stopping
      before the first stream whose whole-final trim count is >= 1;
   e. the stamp-scoped TRUE figure = chain + 1 when the current stream's trim
      count at `fill_stamp` == 0 (trim legs carry ISO-Z timestamps; the stamp
      compares lexically — the stream anchors' own format);
   f. assert the text's claimed streak figure equals the probed TRUE figure;
      on mismatch raise AssertionError naming BOTH the composed claim and the
      probed chain, including the resetting stream's identity (the rotated
      filename and its run_start).
2. The claimed figure is EXTRACTED from the text under assertion (the
   parenthetical's `reads N` / `moves A -> B` shapes), never passed in as a
   remembered constant — the T270/T288 probed-expectation clause binds the
   expectation side too.
3. LOOP-SPEC Phase 3's fill pre-commit assert paragraph (the T317 trim leg's
   paragraph, ~line 1141) gains ONE sentence in the family's own shape: the
   fill compose leg's pre-commit assert ALSO re-derives the strict post-reset
   trim streak at the fill stamp in the SAME leg (the chain re-read off the
   rotated events files' whole-final trim counts, the current stream's count
   probed at the stamp) and verifies the fill text's streak figure via
   `scripts/wrap_assert.py`'s streak leg (`assert_fill_trim_streak`, T321:
   the cycle-3024 twelfth-sighting finding — the composed streak misread
   across three consecutive establishments, the class's first firing on the
   streak surface) — the same compose-time catch for the streak surface.
4. `scripts/test_wrap_assert.py` gains RED-proven legs (fixture events files
   written to a temp dir, never the live corpus):
   - the cycle-3024 shape: a fixture chain whose resetting stream (whole-final
     trim count 2) sits three streams back, followed by three trim-0 streams —
     a text claiming "reads 4" RAISES and names the resetting stream's
     identity; the same fixture with a "reads 3" claim PASSES;
   - the stamp-scoped +1: a current stream with zero trim legs at/before the
     stamp extends the derived chain by one; a current stream with a trim leg
     at/before the stamp makes the stamp-scoped claim form absent (no
     assertion) or mismatch per the composed text;
   - the T320 identity leg: a fixture rotated file with zero or two
     `run_start` events is rejected by name;
   - the expectation-side clause: a caller passing the expected figure as a
     remembered constant (bypassing extraction) is rejected or never the API —
     the expectation is extracted from the text under assertion.
5. No changes outside `scripts/wrap_assert.py`, `scripts/test_wrap_assert.py`,
   LOOP-SPEC.md.

## Acceptance

- The new legs RED-catch the falsified cycle-3024 shape (AssertionError names
  composed 4 vs probed 3 and the resetting stream) and PASS the TRUE shape;
- the full `python3 scripts/test_wrap_assert.py` suite green (160 prior + the
  new legs);
- LOOP-SPEC.md carries the amended fill-assert sentence (the pin-surface
  probe per T285 was run at filing: no `matches!("…")` census or window pin
  covers the trim/streak doctrine text — the amendment is free);
- `cargo test --release --test todo_consistency` green (the bookkeeping
  floor).

check: python3 scripts/test_wrap_assert.py
