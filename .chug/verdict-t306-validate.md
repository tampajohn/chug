VERDICT: PASS

T306 validation — commit 8eee1e3 on loop-t306 (worktree /tmp/chug-loop-t306), reviewed against the spec's 3 requirements, Tests, and Acceptance.

## Spec compliance
1. Req 1 (probed-equality leg): PASS. `validate_state` gains `expected_eval_commit=None`; when supplied, the parsed `eval-commit` field must EQUAL it, else AssertionError naming BOTH the probed field value and the supplied probed actual (plus the probe discipline — never a bare constant mismatch); when unsupplied the leg is inert (INERT leg green; mutant M4 proves the guard binds).
2. Req 2 (docstring call shape): PASS. The docstring names the intended shape — the wrap's state-write helper PROBES `git rev-parse <commit>` and passes the probed value in, never composing from memory; the helper lives in the wrap corridor's shell outside tracked code, the contract lands here.
3. Req 3 (RED-proven): PASS, independently reproduced. Parent-commit wrap_assert.py + new tests → FAIL-path red (TypeError: unexpected keyword argument; 1 failure + 2 errors, 77 carried legs green), and the INERT leg passing on the fabricated text pins the pre-leg hole (the falsified hash sailed the old gate). Restored: 80/80 OK.

## Tests
- PASS / FAIL / INERT legs all present in ValidateStateTest; FAIL leg asserts both hashes + "eval-commit" + "git rev-parse" in the message, so the AssertionError is pinned to the eval-commit leg, not a stray leg.
- 4th leg (non-str expected → TypeError naming the accepted shape) exceeds the spec's three named legs — additive interface defense consistent with the facade's pre-existing type guards; Acceptance ("all pre-existing legs + the three new ones") satisfied.
- Test file purely additive (+88/-0): the 76 carried legs are byte-unmodified.
- Fixture honesty verified against the repo: TRUE hash 39304522865f99ca95b6e8b8750e1714e627e7fb resolves (`git cat-file -t` → commit, rc 0); fabricated 3930452b1f9e0e05a12e93d69a2d6ba95d7cbe47 → rc-128 "could not get object info" — the spec's evidence exactly; shared 7-char prefix 3930452.

## Acceptance
- `python3 scripts/test_wrap_assert.py` → 80/80 OK.
- eval-delta.sh untouched (diff name-only: only the two python files); the boot-time fail-closed catch stays as-is.
- The spec file (specs/t306-eval-commit-reachability.md) landed separately in the eval commit 39296b0 — the impl commit is correctly code+tests only.
- No other validate_state call sites exist in tracked code (grep across *.py/*.sh/*.rs) — the optional kwarg is trivially backwards-compatible.

## Gates (CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-validate-a)
- cargo build: EXIT 0 (2m30s)
- cargo clippy --all-targets -- -D warnings: EXIT 0, zero warning lines
- cargo nextest run --release: EXIT 0, 1834/1834 passed (1 slow), 0 skipped

## Mutation testing (serial; overlap declared: all 6 mutants touch scripts/wrap_assert.py, so the T79 parallel legs do not apply; one throwaway worktree, byte-clean after each leg, removed after collection)
- M1 flip !=→==: CAUGHT by exactly the PASS + FAIL legs (2 failures)
- M2 drop the mismatch raise (if False): CAUGHT by the FAIL leg (1 failure)
- M3 redact BOTH named values from the message: CAUGHT by the FAIL leg's message asserts (1 failure)
- M4 leg runs without the keyword (if True): CAUGHT — 11 failures across the keyword-less legs incl. INERT
- M5 drop the non-str TypeError guard: CAUGHT by the TypeError leg (1 failure)
- M6 probe the wrong field key (eval-at): CAUGHT by the PASS + FAIL legs (2 failures)
6/6 killed, each by the right tests — the round's tests are not vacuous.

## Edge probes (beyond the suite)
- Trailing-whitespace field value strips in read_key_values → equality passes (no false-fire).
- empty-str expected → AssertionError naming both values (a caller-side probe bug fails loudly).
- eval-commit key missing entirely → the FIELDS leg fires first (STATE_FIELD_KEYS covers eval-commit) — sensible ordering.

## Findings
1. MINOR (cosmetic, non-blocking): validate_state's docstring "The gate, in order:" list numbers its legs 1,2,3,4,4 — SPLICE and CORPUS-TAIL IDENTITY are both "4." (the diff renumbered SPLICE 3→4 but left CORPUS-TAIL at 4). The in-code step comments are correctly numbered 1-5, and no behavior is affected. Recommend a one-character follow-up (docstring 4.→5. on the corpus-tail entry) at the next touch of this file.

## Housekeeping
- .chug/decisions.jsonl in this worktree: EMPTY (no decision_log calls made — the verdict record is the orchestrator's write).
- Main worktree byte-clean at verdict time; throwaway mutant worktree removed; RED/mutation scratch lived in /tmp only.
