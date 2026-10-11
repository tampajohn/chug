# T289 — wrap_assert learns ring-id uniqueness + position-aware ring-splice assert

**Pri:** 4 (robustness — the state file's append-only history surface has an
unguarded tooth: a ring-splice mis-assembly can duplicate carried lines and
pass every existing write gate; the cycle-520 wrap's rewrite did exactly
that and the corruption is on disk until rotation heals it)

**SOLO doctrine surface** (scripts/wrap_assert.py + scripts/test_wrap_assert.py
+ ONE sentence in META-META-SPEC.md's ring paragraph — the loop/spec doctrine
itself): never overlaps, kimi REQUIRED validation.

## Repo context

The trip-99 eval (cycle 524, EVALUATION.md §2.2, eval-triage d1791700535-2)
verified mechanically that the cycle-520 wrap's `.chug/eval-state.md` rewrite
landed a corrupted decisions ring: **12 ring lines, 8 unique decision ids** —
the tail 4 lines are byte-duplicates of the head 4 (`sort | uniq -c`: 2× each
of d1791696920-1 / d1791697568-2 / d1791697568-3 / d1791697575-4). The ring's
splice contract (META-META-SPEC.md's T192 paragraph) is prepend-at-head +
carry-byte-identical + rotate-from-tail; a duplicated tail is none of those —
it is corruption of the append-only history.

Every existing guard passed it:

- `assert_ring_count` (T288) covers COUNT only — 12 either way.
- The T266 structural self-check covers HEADERS only (one `purpose:`, one
  `## health`, one `## open-threads (`, one `## decisions`).
- `assert_byte_carry` is position-free BY DESIGN (its docstring: "this
  position-free subsequence check is correct under it unchanged", T287) —
  correct for a well-formed prepend, VACUOUS for a duplicated tail: the
  wrap's landing call (the wrap_assert call-signature 8th-fire arc,
  d1791698630-11) verified the new batch's lines appear in the composed
  ring, which duplicates trivially satisfy.

The detection half WORKED: the next delta run (06:11:42Z) read FULL-READ
REQUIRED — "nothing carried verbatim from the previous ring (summarization?)"
— true, the corrupted ring carries zero prior-era lines; the splice-verdict
misfire census stays FROZEN (this was no misfire). Then two acceptance gaps
closed over the flag: the `.chug/eval-state.prev.md` snapshot refreshes AFTER
validation, so the following run (06:15:08Z) compared corrupted-vs-corrupted
→ carried 12 → STATE-HIT (the ratchet self-healed the verdict), and the
FULL-READ verdict had no consumer on a cheap-exit cycle (the T258 predicate
never reads the delta verdict).

This row closes the GUARD gap (the acceptance gaps are named in the eval for
the record; the eval-delta gate's position-aware shape is the model, and
eval-delta.sh is NOT in scope — it fired correctly).

estimate: ~120 changed lines (helper ~40 + unittest ~60 + ONE doctrine
sentence + this spec's own surfaces) — well under the T209 ~500 ceiling.

## Requirements

1. **Ring uniqueness assertion.** `scripts/wrap_assert.py` learns
   `assert_ring_unique(path_or_lines)`: extract the ring section (reuse
   T288's `ring_section_lines` / `_ring_section_bounds` — LINE-ANCHORED on
   the ring's own header per T266, never a bare substring), then assert
   (a) no duplicated LINE byte-for-byte and (b) no duplicated decision id
   (the `- d<N>-<seq>:` prefix of each ring bullet). The failure names the
   duplicated id (or line repr) and BOTH offsets, probed from the artifact.
2. **Position-aware ring-splice assertion.** Learn
   `assert_ring_splice(prev_ring_lines, new_ring_lines, prepended_count,
   capacity=12)`: the composed ring must be PREPENDED + CARRIED structurally —
   `new_ring_lines[prepended_count:]` equals
   `prev_ring_lines[0 : max(0, min(capacity - prepended_count, len(prev_ring_lines)))]`
   BYTE-IDENTICAL and POSITION-ALIGNED (element-wise, never a subsequence) —
   mirroring the eval-delta splice gate's shape (carried = the previous
   list's prefix below the new head; rotation drops from the previous
   ring's TAIL only). `prepended_count` is PROBED by the caller from the
   batch it composed (the T270/T273 letter: probed actuals, never a
   remembered constant). The existing position-free `assert_byte_carry`
   STAYS for non-ring surfaces (its T287 docstring note stands) — the ring
   surface upgrades to the position-aware form.
3. **Unittest RED legs** in `scripts/test_wrap_assert.py` (stdlib unittest,
   temp files, the existing suite's style):
   (a) a 12-line ring fixture whose tail 4 lines duplicate its head 4 (the
   cycle-520 shape) FAILS `assert_ring_unique` with the duplicated ids
   named — RED pre-change, green after;
   (b) a well-formed 12-line ring (8 prepended + 4 carried from a previous
   ring) PASSES uniqueness AND `assert_ring_splice` with the probed prepend
   count;
   (c) the vacuous-pass shape — a splice whose "carried" window is a copy of
   the new batch's own head — PASSES the old position-free
   `assert_byte_carry` (documenting the gap) but FAILS `assert_ring_splice`;
   (d) a legal prepend + tail-rotation splice at full capacity PASSES
   `assert_ring_splice` (rotation drops from the previous ring's tail only).
4. **ONE sentence in META-META-SPEC.md's ring paragraph**, inserted
   immediately after the "...rotate at most 11 so one prior line always
   survives verbatim;" sentence, naming the uniqueness contract: the ring
   holds no duplicated line — a duplicated carried line is corruption of
   the append-only history, never a rotation — citing the cycle-520 wrap's
   rewrite (12 lines / 8 unique decision ids, the new batch's own head
   written twice into the tail, every count/structural guard passing it)
   and the helper's new uniqueness + position-aware ring assertions as the
   teeth. The sentence MUST NOT quote the pinned needle "verbatim splice,
   never summarization" (eval_state_delta.rs:751's presence pin — the
   insertion point is AFTER that sentence, the pin's needle text
   untouched) and MUST NOT add any occurrence of the exact token
   "verbatim splice" (the eval_outcomes_carry.rs window's comment names
   the whole-file count rationale — keep the token's file count unmoved).
5. **Pin-surface probe (T285) re-run at impl time**: the filing-time probe
   ran `grep -rn 'verbatim splice\|newest batch at the head\|rotate at most
   11\|drops from the' tests/*.rs` — exactly two adjacent pins found
   (eval_state_delta.rs:751's presence pin on the splice-discipline needle;
   eval_outcomes_carry.rs's window-scoped VERBATIM pin, comment-documented
   whole-file count >1 with no whole-file exact-count pin) and three
   comment-only wrap_assert mentions (loopd_daemon_ensure.rs:204,
   loopd_spoof_guard.rs:131, loopd_stale_binary.rs:146 — the T273
   __pycache__ side-effect comments, untouched). Re-run before finalizing;
   declare any collision with its reconciliation.
6. Do NOT change the ring's prepend convention (T287), `assert_byte_carry`'s
   contract, `assert_ring_count`'s API, or scripts/eval-delta.sh (the gate
   fired correctly — its shape is the model, not a target). No other repo
   file changes beyond the three named targets.

## Tests

- `python3 scripts/test_wrap_assert.py` — the full unittest suite, extended
  per requirement 3 (the RED legs named there must FAIL against the
  pre-change helper where the assertion shape is new, PASS after).
- The doctrine-carrier battery stays green (the META-META-SPEC.md sentence
  touches pinned doctrine carriers): the cargo suites that read
  META-META-SPEC text — eval_outcomes_carry, eval_state_delta,
  loop_spec_cheap_exit, loop_spec_check_wall, loop_spec_doctrine_prune,
  loop_spec_eval_read_path_pointer, loop_spec_recovery,
  loop_spec_validation_lane, todo_consistency.

## Acceptance

- `assert_ring_unique` FAILS on the 12-line/8-unique-id fixture naming the
  duplicated ids, PASSES on a well-formed ring.
- `assert_ring_splice` FAILS on the vacuous-pass shape (the old position-free
  form passes it — the documented gap) and PASSES on legal prepend +
  tail-rotation splices.
- The ring paragraph carries the new sentence citing the cycle-520 fire;
  zero pin collisions (requirement 5's probe re-run EMPTY of new collisions
  before AND after the edit; the "verbatim splice" token's whole-file count
  unmoved).
- `python3 scripts/test_wrap_assert.py` green; the named cargo suites green.

check: cd "$(git rev-parse --show-toplevel)" && export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-validate-a && touch src/*.rs tests/*.rs; python3 scripts/test_wrap_assert.py && set -o pipefail; cargo test --release --test eval_outcomes_carry --test eval_state_delta --test loop_spec_cheap_exit --test loop_spec_check_wall --test loop_spec_doctrine_prune --test loop_spec_eval_read_path_pointer --test loop_spec_recovery --test loop_spec_validation_lane --test todo_consistency 2>&1 | tail -20
