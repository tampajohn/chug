# T310 — Fill-enumeration mechanical guard (wrap_assert enumeration leg)

check: python3 scripts/test_wrap_assert.py && grep -q 'assert_fill_enumeration' scripts/wrap_assert.py && grep -q 'T310' LOOP-SPEC.md

## Repo context

The T272 false-at-write class's newest fire shipped in the cycle-2244 wrap
fill `5b5e38f`: the committed FINAL datum block at its named
`2026-10-28T14:03:44Z` stamp reads "ok:false 4 == 4 adjudicated" while the
mechanical whole-final enumeration of the 2244 stream finds FIVE `ok:false`
legs present at that stamp — the fifth the `2026-10-28T14:03:24.421Z`
`assert_fill_datum` AssertionError (the T309 datum-leg guard's first
production fire, itself the designed pre-commit catch). The firing sequence
is the class's canonical shape: the fill compose leg's enumeration probe ran
BEFORE the datum-leg assert fired (4 legs at the probe), the guard fire plus
the recompose intervened, and the recomposed fill text updated the datum
figures (the T309 leg forced that) but NOT the `ok:false` enumeration — a
read taken earlier, not re-run before the write (META-META-SPEC T272: "a
read taken earlier MUST be re-run before the write if any tool call
intervened between the read and the write: the false-at-write shape"). The
digest backstop caught the miss one chain later (the cycle-2248 eval's
498th-establishment derivation — the designed net) and the correction rides
the eval prose per T70 — zero casualty, but the committed record is
permanently wrong, and the enumeration surface has prior corridor form: the
T305 sentence itself names the cycle-1982 fill's falsified `ok:false 4 == 4`
+ `trim 0 == 0` figures (each composed at an earlier stamp and carried
forward). The T309 datum leg (`assert_fill_datum`, landed `7923efc`) proved
the guard shape in production in its first use — re-run the mechanical
authority at the fill stamp in the SAME pre-commit leg, assert the composed
fill text carries its figures — and the enumeration surface is the datum
leg's sibling: the mechanical authority is a `wc -l` count over a
`jq -c 'select(.type=="tool_result" and .ok==false)'` projection of the live
events stream at the stamp, and LOOP-SPEC Phase 3's T305 sentence (the T257
bullet) is where the datum-leg mandate already lives.

The self-reference property (the assert leg's own failure adds an `ok:false`
leg) converges the same way the datum leg's iteration count does: the
recompose re-runs the probe and asserts the NEW text carries the NEW count —
the cycle-2244 recompose landed 159/4,147.136 after the guard's 158/4,126.879
fire, and the enumeration analog is 5-after-fire vs 4-at-first-probe.

estimate: ~150 changed lines (scripts/wrap_assert.py +60,
scripts/test_wrap_assert.py +80, LOOP-SPEC.md +9/-1)

## Requirements

1. `assert_fill_enumeration(text, probed_count, probed_stamp)` in
   `scripts/wrap_assert.py`: the fill/wrap TEXT (str) plus the probed
   `ok:false` count (int) plus the probed enumeration stamp (str) — the
   arguments are the probed actuals (the T273 letter: arguments are the
   probed actuals, never a remembered constant; the function never
   re-derives the expectation nor runs the enumeration itself). Hand-scan
   (import-free, no `re` — the module's standing constraint) the corpus's
   FINAL-enumeration rendering `ok:false K == K` (and the bare `ok:false K`
   head-figure) inside the datum block(s); assert at least one block carries
   the probed count exactly (any-block semantics, the datum leg's rule). Zero
   blocks is the named-missing fire (a datum-less fill is never a silent
   pass); a degenerate `probed_count` (non-int, negative) fails closed naming
   the probed value; non-str `text` TypeErrors naming the accepted shapes
   (the pre-split line list is the call-signature slip shape). Zero-mutation
   by construction (all arguments values, nothing opened or written).
2. The composer's probe (doctrine text, not the helper): the fill compose
   leg re-runs the mechanical enumeration at the fill stamp in the SAME
   leg — `jq -c 'select(.type=="tool_result" and .ok==false)'
   .chug/events.jsonl | wc -l` — captures the count into a shell variable
   (the T282 channel half: probed values ride argv/env into the script,
   never literal-composed), and passes THAT count into
   `assert_fill_enumeration` alongside the fill text.
3. Tests in `scripts/test_wrap_assert.py`: ≥8 legs — the TRUE-block pass
   (the count carried exactly), the falsified-count fire (4 vs 5) naming
   both figures (the probed count and the divergent block figure), the
   zero-block named-missing fire, the degenerate-count fail-closed, the
   TypeError shapes, the any-block semantics with a stale + TRUE block pair
   (the TRUE block redeems the text), the zero-mutation proof (the
   AssertionError fires before any write with the tempdir byte-unchanged),
   and RED-provability hand-proved by mutants run on a copy (delete the
   count compare → the falsified leg goes red; silent-pass the zero-block
   fire → the named-missing leg goes red), the module restored byte-identical
   after each run.
4. LOOP-SPEC.md's T305 sentence (the T257 bullet) gains ONE clause appended
   after the T309 datum-leg clause's closing "— the compose-time catch over
   the same-wrap correction commit.": the fill compose leg's pre-commit
   assert ALSO re-enumerates the stream's `ok:false` legs at the fill stamp
   in the SAME leg (`jq -c 'select(.type=="tool_result" and .ok==false)'
   .chug/events.jsonl | wc -l`) and verifies the fill text's enumeration
   figure carries the probed count via `scripts/wrap_assert.py`'s
   enumeration leg (`assert_fill_enumeration`, T310: the cycle-2244 fill
   5b5e38f wrote "ok:false 4 == 4" at its 14:03:44Z stamp against the
   mechanical 5 — the recompose-after-guard-fire updated the datum figures,
   never re-ran the enumeration — corrected forward per T70 at the
   cycle-2248 eval's establishment) — the same compose-time catch for the
   enumeration surface.
5. PIN-SURFACE PROBE (T285, run BEFORE finalizing this text):
   `grep -rn 'matches("' tests/*.rs` — 27 hits across 13 files, NONE pins
   the T305 sentence or the T309 clause (zero `T309`/`assert_fill_datum`/
   `never carried from an earlier probe` hits under `tests/`); the closest,
   `tests/loop_spec_empty_chain.rs`, pins TRIP_ACTUAL_NEEDLE
   ("MUST carry … whole-stream iteration actual") exactly-once plus 4
   Phase-3 anchors (d1791416867-11 / 40717de / seven consecutive omissions /
   53/40/54/44) — the clause adds none of those tokens and keeps all four
   inside the Phase-3 window; `tests/stream_datum.rs` exercises the helper's
   behavior, not the LOOP-SPEC sentence text. Every pin either unperturbed
   or re-verified green post-edit (loop_spec_empty_chain 9/9 and the
   loop_spec_* sweep).

## Acceptance

`python3 scripts/test_wrap_assert.py` green (carried + new, both invocation
shapes: direct and `-m unittest scripts.test_wrap_assert`); the `check:`
line green; `cargo test --release --test todo_consistency` green
(CARGO_TARGET_DIR=target-shared per T47); the LOOP-SPEC pin batteries
post-edit green (loop_spec_empty_chain + loop_spec_doctrine_prune +
loop_spec_scoped_walk + loop_spec_validation_lane + stream_datum + the
remaining loop_spec_*/eval_state_delta/shared_target_dir sweep); diff scope
ONLY the three named targets (scripts/wrap_assert.py,
scripts/test_wrap_assert.py, LOOP-SPEC.md); TODO.md and LEDGER.md untouched
(bookkeeping is the orchestrator's).
