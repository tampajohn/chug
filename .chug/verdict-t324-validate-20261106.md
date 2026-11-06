# T324 validation verdict — wrap_assert one-liner anchor skip-rule

**VERDICT: PASS**

Validated in /tmp/chug-loop-t324 (branch loop-t324 @ 5c64ebd, diff vs main: exactly
`scripts/wrap_assert.py` +78/-10 and `scripts/test_wrap_assert.py` +428 — the two spec-named
target files only; doctrine files untouched, no committed corpus text edited).

## Findings

1. **Gates re-run independently — all GREEN.**
   - `python3 scripts/test_wrap_assert.py`: rc=0, 198 tests OK (87.3s), including the 11 new
     `AssertOutcomesOneLinerAnchorSkipRuleTest` legs.
   - `cargo nextest run --release` (touch src/*.rs tests/*.rs; CARGO_TARGET_DIR=…/target-shared-validate-a):
     rc=0, 1839/1839 passed in 80.1s.
   - `cargo clippy --all-targets --release -- -D warnings`: rc=0 AND zero warnings (grep -ci warning
     over the full clippy log = 0; not merely exit-0 clippy).
   - `cargo test --release --test todo_consistency`: rc=0, 21/21.

2. **RED-proof audit re-derived (child's "10/11" claim CONFIRMED).** A detached worktree at `main`
   carrying the branch's test file, with the module verified byte-identical to
   `git show main:scripts/wrap_assert.py`: 10 of 11 new legs FAIL, 198-test battery otherwise
   green (only the 10 new-leg failures). Failure reasons are the designed ones:
   - leg (a) composed establishment mention: pre-change detail non-None ("no `<N>th` ordinal
     adjacent before the establishment anchor");
   - leg (b) composed re-arm clause: pre-change latches the clause region's stray figure,
     (21,) != (619,) — the exact adjudicated wrong-latch class;
   - leg (c) only-non-anchors: pre-change details lack the T324-named skip-class strings;
   - 3000/3004 pairs: fire naming the 're-arm ordinal' group divergence;
   - 3008 pair: fires naming the 'establishment' group extraction failure;
   - six-full live smoke: fails at cycle 3029's establishment extraction;
   - the quoted red-proof leg: pre-change latches the backtick mention's record-id digits,
     (1793919173,) != (625,);
   - two red-proof legs fail needle-first (the skip lines absent pre-change, 0 != 1) — valid red,
     their behavioral red-provability rides the in-battery in-memory mutants on the shipped module.
   - The single non-red leg (`test_live_corpus_smoke_exercised_one_liners_still_extract`) is
     EXPECTED non-red: committed one-liners carry no non-anchor mentions, so the pre-change
     extractor already succeeds on them; the leg pins one-liner-side no-regression, not a fire
     reproduction. 10/11 is the correct count, not an inflated claim.

3. **Acceptance's named reproduction verified BY HAND (not just via the battery).**
   `assert_outcomes_one_liner(the 3008 full entry @ 9ae8c1a, the committed 3008 one-liner)` passes
   MECHANICALLY; corpus shape independently confirmed (bare mention at byte 1151 precedes the 692nd
   anchor at byte 4308). The 3000/3004 pairs likewise pass mechanically (clause mentions at
   1251/1294 precede the discharge anchors at 3951/4009). No by-hand adjudication needed anymore.

4. **Mutation testing (6 mutants, throwaway worktrees /tmp/chug-mut-t324-<k>, ≤3 in flight,
   targeted tests) — 5 killed, 1 analyzed survivor.**
   - mut-1 (establishment ordinal skip off): KILLED — 6 legs fail (composed leg a, leg c, 3008 pair,
     establishment red-proof, six-full smoke, + the degenerate needle-count leg).
   - mut-2 (re-arm clause skip off): KILLED — 7 legs fail (leg b, leg c, 3000/3004 pairs, clause
     red-proof, six-full smoke, quoted red-proof via 3025's co-riding clause mention, + needle leg).
   - mut-3 (re-arm quoted skip off): KILLED — 4 legs fail (leg c quoted case, quoted red-proof,
     six-full smoke, + needle leg).
   - mut-4 (clause helper `ed`-advance removed): KILLED — 6 legs fail, all substantive (the
     `re-armed at the …` shapes latch instead of skipping).
   - mut-6 (establishment skip always-on): KILLED — 18 failures across the full 198-test battery
     (pre-existing establishment fixtures pin the not-over-firing direction).
   - mut-5 (quoted helper one-sided, backtick-before only): SURVIVED — new class OK and full
     198-test battery OK. The after-side backtick check is defensive depth with no pinning fixture:
     every committed shape (the 3025 live entry, the composed quoted-only fixture) carries the
     LEADING backtick (`first-`re-arm``), and the skip direction is fail-closed (over-firing fails
     the guard closed, never extracts a wrong figure from committed corpus shapes — the
     no-behavior-change pin over 200+ one-liners and the six-full smoke cover the corpus).
     NON-BLOCKING observation; a one-line fixture with a trailing-only backtick would pin it.

5. **Code review — no defects found.** Skip ordering is correct in both extractors (`'s`/`hands`
   follows precede the ordinal-adjacency skip; boundary-block precedes clause precedes quoted);
   `_ol_ordinal_before`'s whitespace-only walk-back correctly breaks adjacency on the
   `695th-establishment` hyphen compound; `_ol_rearm_clause_at` is case-insensitive with the
   `ed`-advance and cannot false-positive on discharge anchors (a digit, never `at the`, follows
   the anchor); all-non-anchor texts still fail closed with the T324-named details; the
   newly-unreachable-in-shipped ordinal check in `_ol_establishment_figures` stays live-exercised
   by the in-battery red-proof mutant; the in-memory-exec red-proof idiom self-asserts the module
   byte-identical after each run; the no-behavior-change acceptance is pinned dynamically over
   200+ committed one-liners (>500 group comparisons) rather than by static fixtures alone.

6. **Housekeeping verified.** All six mutant worktrees and the RED-proof worktree removed
   (`git worktree list` shows only main + loop-t324); the main worktree /tmp/chug-loop-t324 is
   byte-clean (`git status` empty) before this verdict; nothing committed, merged, or pushed;
   TODO.md and LEDGER.md untouched.

— adversarial validator (T324), 2026-11-06
