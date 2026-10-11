# T287 — eval-delta splice_verdict learns the ring's prepend convention (close the two-gate contradiction)

## Repo context

`.chug/eval-state.md`'s `## decisions` ring is rewritten by every eval wrap
(T260) and gated at write time by `scripts/wrap_assert.py`'s
`assert_byte_carry` (T273) — whose docstring contract explicitly blesses
PREPENDING ("at any offset, since the new head prepends above the carry",
scripts/wrap_assert.py:136). The on-disk ring is newest-batch-first (the
cycle-512 trip-96 eval's read: the two newest records head the file, batches
descending, batch-internal chronological). But
`scripts/eval-delta.sh`'s `splice_verdict` (the awk block, lines 115–164)
pins the OPPOSITE convention — append-at-tail: the current ring's PREFIX
must equal a SUFFIX of the previous snapshot's ring — and so do
`tests/eval_state_delta.rs`'s splice legs (the "newest-last chronology"
comment at line 38; `splice_plain_append_is_legal_below_the_cap` line 299;
`splice_full_ring_rotation_from_the_oldest_end_is_legal` line 318; the
fixture header `## decisions (last 12, oldest first)` at line 105) and
META-META-SPEC.md's ring discipline (lines 60–66: "last 12, oldest first …
new decisions appended"). Result: every wrap's legal rewrite misfires
`violated: nothing carried verbatim from the previous ring
(summarization?)` on the FIRST post-rewrite delta run — 28 logged in the
retained `.chug/loopd/loopd.log` (plus 4 `decisions 13 > 12 (ring
overfull)` of the older ring-13 class), the verdict histogram oscillating
1 FULL-READ + 3 STATE-HIT per era — then self-heals via the snapshot
refresh (the next run compares the state against its own bytes →
`ok (carried 12, rotated 0, appended 0)`). The T260 verbatim-splice pin is
thereby VOID at the only moment it exists for (validating the rewrite); a
REAL ring summarization would read identically and heal silently, so the
alarm's 28-fire history is indistinguishable from 28 real fires. The
state's ring header ("last 12, oldest first") contradicts its newest-first
content. Verified against the code at filing (the indictment above is read
from scripts/eval-delta.sh:99–164, scripts/wrap_assert.py:128–162,
tests/eval_state_delta.rs:260–360 + 916–935, `.chug/eval-state.md`'s live
ring, and the loopd.log violation census), not from memory.

Chosen shape (option B — align the checker + pins + doctrine with the
convention the writer and the write gate already practice): the wrap
writer and `assert_byte_carry` stay untouched; the de-facto on-disk shape
becomes legal instantly and no live-state heal is needed. (The rejected
option A — flip the writer + write gate to append-at-tail — would fight
the entrenched read-first preservation habit, require a one-time heal of
the gitignored live state file outside the repo diff, and special-case the
write gate's first post-flip write.)

## Requirements

1. `scripts/eval-delta.sh` `splice_verdict`: mirror the legality rule for
   PREPEND storage — the carried segment is the current ring's SUFFIX
   equal to a PREFIX of the previous snapshot's ring (byte-identical, in
   order); below the cap NOTHING may be dropped (the current suffix must
   equal the whole previous ring); at a full ring, rotation drops from the
   previous ring's TAIL (the oldest end under newest-first storage) and at
   least one line must survive verbatim; the ok message keeps its shape
   `ok (carried J, rotated R, appended A)` with `appended` counting the
   new head lines and `rotated` the dropped previous-tail lines; the
   violation message classes keep their names (rewritten / dropped /
   reordered / nothing carried verbatim / overfull). The comment block at
   lines 60 and 99–114 mirrors the new rule (including the
   FILENAME == ARGV[1] rationale, unchanged). The read-path rules print
   block's "rotate from the oldest end, append new" (line 384) becomes
   "rotate from the tail (the oldest end), prepend new" — unpinned text,
   no test names it (probed).
2. `tests/eval_state_delta.rs`: flip the splice legs to prepend —
   `splice_plain_append_is_legal_below_the_cap` becomes
   `splice_plain_prepend_is_legal_below_the_cap` (the new line HEADS the
   ring; ok tallies unchanged: carried 3, rotated 0, appended 1);
   `splice_full_ring_rotation_from_the_oldest_end_is_legal` drops from the
   previous ring's TAIL (prev ring(12) → current `[new, prev[0..11]]`,
   ok carried 11, rotated 1, appended 1) and is retitled
   `..._rotation_from_the_tail_is_legal` with a comment naming the
   tail-as-oldest-end under newest-first storage;
   `splice_violations_force_full_read` keeps every existing negative case
   and GAINS an append-at-tail case (legal under the old rule, reads
   violated as a reorder under the new);
   `splice_empty_previous_ring_with_nonempty_current_is_a_legal_append`
   stands unchanged (m==0 → ok appended k under both conventions); the
   line-38 doc comment and the line-105 fixture header flip to
   newest-first. Every flipped/added leg is RED-proven (fails against the
   pre-flip script, passes after).
3. META-META-SPEC.md lines 60–66: the ring discipline rewords to the
   practiced convention — "the `## decisions` ring is append-only history
   (last 12, NEWEST first): carried lines byte-identical, new decision
   batches PREPENDED at the head, rotation drops from the TAIL (the
   oldest end) once the ring is full; if this eval made more decisions
   than the ring has room for, rotate at most 11 so one prior line always
   survives verbatim" — and the state ring header the wrap writes becomes
   `## decisions ring (last 12, newest first — ...)`. Every other needle
   in the pinned block is preserved byte-identical (the T285 probe:
   `doctrine_carries_the_state_delta_read_path` pins "verbatim splice,
   never summarization" and neighbors over this file — all untouched).
4. `scripts/wrap_assert.py`: NO behavior change (its position-free
   subsequence carry is correct under both conventions); ONE docstring
   sentence naming the alignment (the helper's prepend blessing is now
   the doctrine's letter).

## Tests

- The flipped + new legs of requirement 2, all RED-proven against the
  pre-flip `splice_verdict` (run the new legs against the old script:
  prepend-legal reads violated; append-as-reorder reads ok — capture both
  in the impl's commit message).
- The mirrored awk covered by the fixture matrix: prepend below cap,
  full-ring tail rotation, paraphrase/drop/reorder violations, the
  append-at-tail violation, empty-previous append, overfull.

## Acceptance

- `splice_verdict` validates a prepend rewrite as `ok (carried ≥ 1)` on
  the FIRST post-rewrite delta run — the 28-fire misfire class closes.
- A paraphrased, dropped, reordered, or append-at-tail carry still forces
  FULL-READ with the named violation class; overfull still reads
  `decisions N > 12 (ring overfull)`.
- `grep -c 'nothing carried verbatim'` on the live loopd.log stays at its
  pre-merge count through the next wrap's rewrite (the first post-flip
  wrap validates clean).
- Full suite + clippy green; the doctrine pins
  (`doctrine_carries_the_state_delta_read_path`) pass with the needles
  preserved.

estimate: ~140 changed lines (script ±40, tests ±65, META-META-SPEC ±10,
wrap_assert ±3, spec bookkeeping)

check: touch src/*.rs tests/*.rs; CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-validate-a cargo test --release --test eval_state_delta --test eval_outcomes_carry --test todo_consistency
