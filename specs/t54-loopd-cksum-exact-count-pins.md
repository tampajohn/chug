# T54 — loopd_reexec.rs: exact-count pins for SELF_CKSUM (close the additive-mutant survivor)

One concern: the four T50 positional pins in `tests/loopd_reexec.rs`
assert existence + relative order of the re-exec machinery but never
COUNTS — an additive duplicate `SELF_CKSUM="$(cksum …)"` assignment inside
the while body silently defeats the re-exec (the fingerprint refreshes
every cycle, so the while-top comparison never fires) with all pins green.

## Repo context

- T50 (merged `078cb77`) gave loopd a self-re-exec: `loopd.sh:75`
  `SELF_CKSUM="$(cksum "$ROOT/loopd.sh")"` (fingerprint BEFORE the cycle
  loop) and `loopd.sh:93` `if [ "$(cksum "$ROOT/loopd.sh")" !=
  "$SELF_CKSUM" ]` (comparison as the first while-body statement).
- The T50 kimi validator's one non-blocking finding (VERDICT: PASS 41/50):
  "an additive duplicate-`SELF_CKSUM` mutant at the while-body end
  silently defeats the mechanism with all pins green (exact-count pin
  candidate, T47-carrier doctrine)". The cycle-23 wrap carried it to this
  eval; the eval filed it (I3).
- `tests/loopd_reexec.rs` (144 lines, 4 tests) is the static-pin home for
  supervisor shape; T47's `tests/shared_target_dir.rs` established the
  count-carrier doctrine (exact `count_eq` pins strictly stronger than
  `>=` bounds).
- Baseline verified at spec authorship (cycle-24 eval): loopd.sh contains
  exactly ONE `SELF_CKSUM=` assignment (line 75) and exactly ONE
  `!= "$SELF_CKSUM"` comparison (line 93); the string `SELF_CKSUM` occurs
  exactly 2 times in the file.

## Requirements

Add exact-count pins to `tests/loopd_reexec.rs` (no production changes —
loopd.sh is untouched by this row):

1. **Assignment count:** lines matching `SELF_CKSUM=` occur EXACTLY once
   in loopd.sh (a second assignment anywhere — while-body end included —
   refreshes the fingerprint and defeats the re-exec).
2. **Comparison count:** occurrences of `!= "$SELF_CKSUM"` total EXACTLY
   one (a second comparison is the other half of a duplicated block).
3. **Total occurrences:** the literal string `SELF_CKSUM` occurs EXACTLY
   twice in loopd.sh (catches any other additive reference, e.g. an
   exported copy or a second conditional shape the first two pins miss).
4. Each pin's failure message names what the count should be and why
   (the additive-mutant defeat mechanism), matching the file's existing
   message style.

## Tests

The pins ARE the tests. Non-vacuousness, verified once by hand during
implementation and then reverted: (a) appending a second
`SELF_CKSUM="$(cksum "$ROOT/loopd.sh")"` line inside the while body turns
pin 1 (and 3) red; (b) appending a second `!= "$SELF_CKSUM"` conditional
turns pin 2 (and 3) red; (c) the pre-existing four pins stay green
throughout.

## Acceptance

- `cargo test --test loopd_reexec` green in the worktree.
- The three counts hold against main's loopd.sh at merge time (2 total
  occurrences, 1 assignment, 1 comparison).
- No changes to loopd.sh or any other file outside
  `tests/loopd_reexec.rs`.

check: cargo test --test loopd_reexec && test "$(grep -c 'SELF_CKSUM=' loopd.sh)" -eq 1 && test "$(grep -o 'SELF_CKSUM' loopd.sh | wc -l | tr -d ' ')" -eq 2
