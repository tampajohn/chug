VERDICT: PASS

Scope reviewed: git diff main...loop-t311 (0756d23) — scripts/wrap_assert.py (+103),
scripts/test_wrap_assert.py (+210), LOOP-SPEC.md (+21); 333 insertions, 3 files, no
rust sources touched.

Findings:
1. Requirement 1 (the leg) — MET. assert_state_on_disk(path, expected_eval_commit=None)
   reads the state file's BYTES (open(path,"rb") + utf-8 decode), non-str path raises
   TypeError naming the accepted shape (the module's call-signature-slip convention,
   including the list/tuple "probed line(s)" form mirrored from validate_state), runs the
   FULL existing validate_state gate with expected_eval_commit forwarded (splice/
   corpus-tail default None → inert, the on-disk state stands alone; ring count/unique
   still gate). Failures propagate from the facade unwrapped (the module invariant); a
   missing/unreadable file raises AssertionError naming the probed path AND the OS error
   (OSError + UnicodeDecodeError caught, `from err` — never a bare traceback).
2. Requirement 2 (docstring names the class) — MET. The docstring names the cycle-2515
   fill's "FACADE-GREEN first attempt" claim RED-proven against the committed bytes,
   states a GREEN claim is never composed without the mechanical run over the on-disk
   file, and names the wiring (the fill compose leg's pre-commit assert, the SAME leg as
   the commit, the T309/T310 pattern). The module docstring + __all__ + coverage line
   updated consistently.
3. Requirement 3 (LOOP-SPEC doctrine) — MET. Phase 3's wrap doctrine block (the one
   naming assert_fill_datum/assert_fill_enumeration) gains the third leg: after the
   wrap's state write the fill pre-commit assert runs the on-disk state-schema leg
   against the committed .chug/eval-state.md; a refusal BLOCKS the fill commit; rewrite
   as the full single write (cycle-309 precedent) and re-gate before push.
4. Requirement 4 (RED proof) — INDEPENDENTLY REPRODUCED. main's wrap_assert.py (no leg)
   + the branch's test file: exactly the 11 new T311 legs ERROR (AttributeError: module
   'wrap_assert' has no attribute 'assert_state_on_disk' — the 2515 prose shape passes
   the pre-T311 call surface because no such leg exists), the 102 pre-existing legs stay
   green; leg restored byte-identical → 113/113 OK (git status byte-clean).
5. Tests (spec's Tests section) — ALL PRESENT AND STRONG: PASS with/without
   expected_eval_commit; FAIL 2515 prose shape from the FIELDS leg asserting
   assert_fields_non_empty, 9/9, each of the nine missing keys named, and the substituted
   last-eval-commit NOT among the missing; FAIL eval-commit mismatch naming BOTH the
   probed field value (T306_TRUE_HASH) and the supplied actual (T306_FABRICATED_HASH)
   plus the surface (T270); FAIL missing file naming the probed path + "No such file or
   directory". Existing battery untouched (diff adds only). Extra hardening beyond spec:
   non-str path TypeError, composed-text slip (File name too long — the d1791921121-12
   shape re-pinned), non-str expected_eval_commit propagating the facade TypeError,
   unreadable dir (IsADirectoryError), real-disk-read GREEN→RED across a later overwrite
   (the exact 2515 class), read-only zero-mutation fire.
6. Gates — ALL GREEN, run by me in this worktree: python3 scripts/test_wrap_assert.py
   113/113 OK; cargo build --release green; cargo clippy --release --all-targets zero
   warnings; cargo nextest run --release (nextest 0.9.146 on PATH, the T82 runner)
   1836/1836 passed, 0 failed (the doctrine carriers' pins included — loop_spec_*,
   eval_state_delta, todo_consistency, shared_target_dir). CARGO_TARGET_DIR=
   /Users/jadams/workspace/chug/target-shared-validate-a with the T195 touch prefix.
7. Mutation testing — 4 mutants, ALL KILLED. Overlap judgment: all mutants touch
   scripts/wrap_assert.py (overlapping file) → serial per the goal's default, one
   throwaway worktree /tmp/chug-mut-t311-x, apply→run python battery→byte-identical
   revert per mutant, worktree removed after collection.
   - MUT-A (drop the expected_eval_commit forwarding → validate_state(text)): killed,
     2 failures (the with-probed-hash PASS leg + the mismatch leg).
   - MUT-B (weaken the unreadable-file AssertionError to a silent return): killed,
     3 failures (missing-file, unreadable-dir, composed-text-slip legs).
   - MUT-C (swap the TypeError guard for a pass): killed, 1 failure (the non-str path
     leg — open(7) walks as an fd → OSError → AssertionError, never the TypeError).
   - MUT-D (drop the validate_state call entirely — the leg reads but never gates):
     killed, 5 failures (2515 prose, mismatch, facade-TypeError propagation,
     real-disk-read, zero-mutation legs).
8. Observations (non-blocking, no spec violation): (a) diff is 333 lines vs the ~120
   estimate — the battery is 11 legs vs the 4 spec-required, all in the module's
   established style; estimate miss only, not a correctness bug. (b) A path str with an
   embedded NUL would raise ValueError (not OSError) from open, escaping the
   AssertionError wrap — outside the spec's test scope and not a corridor-realistic
   shape (the call site passes .chug/eval-state.md); noted for completeness only.

Main worktree byte-clean at verdict time (git status empty); .chug/decisions.jsonl
absent (empty) as required; no leftover mutant worktrees.
