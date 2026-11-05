# T323 — wrap_assert fill eval-state-untracked leg (assert_fill_eval_state_untracked) + LOOP-SPEC fill-assert list

estimate: ~130 changed lines (one assert leg + test legs + one doctrine sentence)

## Repo context

The cycle-1427 class's SECOND fire landed at the cycle-3024 wrap: the wrap
notes commit (11f26ea) force-added `.chug/eval-state.md` into git tracking
(self-caught at the notes commit's own stat read-back; repaired in ac4994f
per the bb339d9 precedent — `git rm --cached`, the file byte-untouched on
disk, the `.gitignore:12` ignore re-covering it, `git ls-files
.chug/eval-state.md` back to ZERO).

Why tracking the state file is a chain-break: loopd.sh's
`delta_bookkeeping_only` file leg reads the baseline..HEAD tree diff and
returns book=no when ANY tracked path outside the bookkeeping set moves. A
tracked `.chug/eval-state.md` is exactly such a path — and the wrap's state
rewrite touches it every cycle, so EVERY subsequent cycle's delta reads
book=no: the T258 cheap-exit predicate can never hold, every cycle becomes
a forced launch, and the chain stays broken until someone repairs the
tracking by hand. That is a SILENT sustained budget burn (the whole point
of the cheap-exit cascade is the $0 skip). The 3024 wrap caught the fire
same-cycle; the class's detection path is an orchestrator self-catch, not
a mechanical guard.

The class's landing pattern (the same shape as the carried-figure
assert-family rows): each NEW surface's firing lands the compose-time
mechanical leg for that surface — `assert_fill_datum` (T309),
`assert_fill_enumeration` (T310), `assert_fill_enumeration_word` (T319),
`assert_fill_trim` (T317), `assert_fill_trim_streak` (T321),
`assert_state_on_disk` (T311/T312), `assert_outcomes_one_liner` (T316).
The disk-only-tracking surface is the last un-asserted piece of the wrap
corridor's git-state hygiene. This row is the weighed disposition of the
class's second fire (triaged d1793919173-3 at the cycle-3025 eval).

Existing pieces:

- `scripts/wrap_assert.py` — the assert family. `assert_state_on_disk`
  (:~2100s, T311/T312) is the sibling on-disk leg (reads the on-disk state
  bytes through `validate_state`); this row's leg is a GIT-STATE check, not
  a file-content check — it asks git, not the filesystem.
- The T260 disk-only pair: `.chug/eval-state.md` (the scoped-walk state,
  rewritten at each wrap) and `.chug/eval-delta.md` (rebuilt by loopd
  before every cycle). Both are `.gitignore:12`-covered and MUST stay
  untracked; the harvest convention (force-added `events-t*`/`LEDGER-t*`/
  `verdict-t*` files, 49 tracked at filing) is the designed exception.
- `scripts/test_wrap_assert.py` — the unittest surface (`python3
  scripts/test_wrap_assert.py`).
- The repair recipe (for the AssertionError text): `git rm --cached
  .chug/eval-state.md` (byte-untouched on disk, the gitignore re-covers
  it) — the bb339d9 / ac4994f precedent.

## Requirements

1. `scripts/wrap_assert.py` gains
   `assert_fill_eval_state_untracked(repo_root)` (the name follows the
   family). It probes git, never the model's say-so:
   a. run `git -C <repo_root> ls-files -- .chug/eval-state.md
      .chug/eval-delta.md` (argv list, never a shell string);
   b. the output MUST be empty (both files untracked);
   c. on any hit raise AssertionError naming (i) each tracked path git
      reported, (ii) the consequence class (a tracked disk-only state file
      breaks `delta_bookkeeping_only`'s file leg — book=no on every
      subsequent cycle, the T258 cheap-exit predicate can never hold, the
      chain silently breaks into sustained forced launches), and (iii) the
      repair recipe (`git rm --cached <path>` per bb339d9/ac4994f — the
      file byte-untouched on disk, `.gitignore:12` re-covering it);
   d. a git INVOCATION failure (not a repo, git missing) raises the same
      AssertionError with the invocation error named — fail closed, never
      silently green.
2. The leg takes the repo root as its only input (the caller passes the
   repo toplevel) — no remembered expected value; the expectation IS the
   mechanical probe (the T270/T288 probed-expectation clause is satisfied
   by construction: the empty-set expectation is the probe's own output).
3. LOOP-SPEC Phase 3's fill pre-commit assert paragraph (the T317/T321
   trim/streak legs' paragraph) gains ONE sentence in the family's own
   shape: the fill compose leg's pre-commit assert ALSO re-verifies the
   T260 disk-only pair stays untracked at the fill stamp in the SAME leg
   (`git ls-files -- .chug/eval-state.md .chug/eval-delta.md` empty) via
   `scripts/wrap_assert.py`'s eval-state-untracked leg
   (`assert_fill_eval_state_untracked`, T323: the cycle-3024 wrap's
   force-add fire — the notes commit tracked the state file and broke the
   cheap-exit file leg until the ac4994f repair, the class's second fire)
   — the same compose-time catch for the disk-only-tracking surface.
4. `scripts/test_wrap_assert.py` gains RED-proven legs (fixture git repos
   in temp dirs, never the live repo's git state):
   - a fixture repo with `.chug/eval-state.md` force-added to the index
     RAISES and names the tracked path + the repair recipe;
   - the same fixture with the file present on disk but untracked PASSES
     (the disk-only convention: present-on-disk + absent-from-index);
   - both files tracked raises naming BOTH paths (one assert covers the
     pair);
   - `.chug/eval-delta.md` alone tracked raises (the pair's other half);
   - a non-repo temp dir (git invocation failure) raises — fail closed;
   - the harvest-exception shape stays green: a fixture with a tracked
     `.chug/events-t999-impl-20000101.jsonl` and untracked state/delta
     files PASSES (the leg's scope is exactly the disk-only pair, never
     the whole `.chug/` tree).
5. No changes outside `scripts/wrap_assert.py`,
   `scripts/test_wrap_assert.py`, LOOP-SPEC.md.

## Acceptance

- The new legs RED-catch the cycle-3024 shape (a fixture with the state
  file in the index raises naming the path, the consequence class, and
  the `git rm --cached` repair) and PASS the TRUE shape (disk-only pair
  untracked, harvest files tracked);
- the full `python3 scripts/test_wrap_assert.py` suite green (the prior
  tests + the new legs);
- LOOP-SPEC.md carries the amended fill-assert sentence;
- `cargo test --release --test todo_consistency` green (the bookkeeping
  floor).

check: python3 scripts/test_wrap_assert.py
