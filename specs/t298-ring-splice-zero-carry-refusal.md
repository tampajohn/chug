# T298 — wrap_assert ring-splice gate: refuse the zero-carry full-ring write

## Repo context

`.chug/eval-state.md`'s `## decisions` ring is append-only history (T192,
META-META-SPEC's state-rewrite paragraph): carried lines byte-identical,
new batches PREPENDED at the head, rotation drops from the TAIL once the
ring is full (capacity 12) — and **"if this eval made more decisions than
the ring has room for, rotate at most 11 so one prior line always
survives verbatim."** Two mechanical surfaces enforce the discipline:

- `scripts/eval-delta.sh`'s `splice_verdict` (awk) — the POST-HOC check,
  run by loopd before every cycle against the previous snapshot. Its
  `k == maxd && j == 0` leg prints `violated: nothing carried verbatim
  from the previous ring (summarization?)` and forces the next eval
  FULL-READ.
- `scripts/wrap_assert.py`'s `assert_ring_splice` (the T289
  position-aware gate, reached by every wrap state write through the
  `validate_state` facade per T295) — the WRITE-TIME check. It computes
  `keep = max(0, min(capacity - prepended_count, len(prev_ring_lines)))`
  and asserts the composed ring's carried window equals the previous
  ring's `keep`-line prefix byte-identical and position-aligned.

**The gap (the cycle-1042 fire, first sighting).** When
`prepended_count >= capacity` (a wrap generation producing 12+ new
decision records), `keep` computes 0 and the carried-window assertion
passes **vacuously** — the facade waves a prepend-12/carry-0 write GREEN
while the zero-carry ring violates T192's one-prior-line-survives rule.
Cycle 1042's wrap did exactly this: its first state write prepended 12
(the cycle's 12 post-eval records), recorded `ring prepend-12/carry-0`
GREEN in its own Outcomes text, and the ring landed 12/12 same-generation
ids with ZERO prior-generation lines surviving. The post-hoc eval-delta
check caught it one generation later at the cycle-1043 boot (forced
FULL-READ — zero casualty, the designed backstop; the escape's cost is
one full-read evaluation). `grep -n 'splice-violated\|nothing carried
verbatim' EVALUATION.md` probes zero prior sightings.

estimate: ~80 changed lines (helper leg + unittest legs; all-in src +
tests + spec).

## Requirements

1. **`assert_ring_splice` refuses the zero-carry full-ring write.** When
   `prev_ring_lines` is non-empty AND the post-write ring is at capacity
   (`len(new_ring_lines) >= capacity`) AND the computed `keep == 0`,
   raise `AssertionError` naming: T192's rule ("rotate at most 11 so one
   prior line always survives verbatim"), the probed arithmetic
   (prepended count, capacity, both lengths), and the remedy (drop the
   batch's oldest new line and carry the previous ring's newest line as
   the tail — prepend-(capacity-1)/carry-1). The check rides the
   EXISTING gate, so `validate_state` callers (every wrap state write,
   T295's facade) are covered with no call-site change.
2. **No new constraint below capacity or on an empty previous ring** —
   the under-capacity append path and the `m == 0` path keep their
   current GREEN semantics (mirror eval-delta.sh's `k < maxd` and
   `m == 0` legs).
3. **Unittest legs in `scripts/test_wrap_assert.py`** (the file's
   PASS-path/FAIL-path convention, T270's probed-actuals reporting):
   (a) zero-carry full ring (12 prev, 12 new disjoint, prepended 12) →
   REFUSED, and the message names T192 + the probed arithmetic;
   (b) prepend-11/carry-1 at capacity → GREEN (the remedy shape);
   (c) under-capacity prepend (6 prev, 3 new) → GREEN;
   (d) empty previous ring with a full new ring → GREEN (the `m == 0`
   leg — first-generation writes stay legal);
   (e) the cycle-1042 regression fixture: 12 prev + the 12-record batch
   → refused at write time (the write the facade passed GREEN).
4. **RED-proof each FAIL leg**: compose the fixture, assert the
   `AssertionError` fires AND names the probed actuals (the file's
   standing convention), never a bare `assertRaises` with no message
   check.

## Tests

`python3 -m unittest scripts/test_wrap_assert.py -v` — the new legs plus
the standing pre-T295/T295/T288/T289 legs all green.

## Acceptance

- `assert_ring_splice` refuses the zero-carry full-ring write with the
  T192 naming + probed arithmetic + remedy text.
- All four PASS legs (b/c/d and the standing set) green; both FAIL legs
  (a/e) fire with message assertions.
- The wrap's next >=12-record generation is refused AT WRITE TIME
  instead of detected one generation later by eval-delta's
  `splice_verdict` (the parity contract: the write-time gate's refusal
  set now covers the post-hoc gate's `k == maxd && j == 0` leg).

check: python3 -m unittest scripts/test_wrap_assert.py -v
