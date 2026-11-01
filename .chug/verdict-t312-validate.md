VERDICT: PASS

T312 validation — 632ad7f (scripts/wrap_assert.py + scripts/test_wrap_assert.py) vs specs/t312-state-marker-gate.md.

Findings:

1. Req 1 (always-on self-probing leg) — MET. After the facade run, assert_state_on_disk self-probes via the owned `_probe_eval_commit()` (`git rev-parse --show-toplevel` then `git log -1 --format=%H -- EVALUATION.md` with cwd=toplevel — the T311 probe convention owned in-module) and asserts the parsed `eval-commit` field EQUALS the probed hash, naming BOTH values in `_fire_marker_mismatch`. The keyword is a true OVERRIDE: under `expected_eval_commit` the probe is not called (`if expected_eval_commit is None:` guard), pinned by the raising-probe test. The leg runs by default — verified empirically: pre-T312 module → the fabricated-suffix and drift tests go RED ("AssertionError not raised").

2. Req 2 (probe_marker injectable + fail-closed probe failure) — MET. Injectable callable, None defaults to the owned subprocess probe; non-callable TypeErrors naming the shape. OSError from the probe → AssertionError naming the probe command + the OS error, never a bare traceback. `_probe_eval_commit` additionally fails closed on rc!=0 and on non-40-hex output. Parse consistency verified: the leg and the facade both read via `read_key_values(text)`.

3. Req 3 (module docstring names the second fire) — MET. The module docstring names the 2704 fire with both hashes (2f905d7e8c… vs TRUE 2f905d7f8cfcafbeff2894be019b2488b4cefc02), the inert opt-in leg, and the zero-casualty fail-closed catch. AST import-free shape preserved (`__import__("subprocess")`), pinned by the pre-existing import-free test.

4. Req 4 (RED-proven) — MET and independently re-verified: with 632ad7f~1's module swapped in, the T312 class showed 4 failures + 2 errors (the FAIL-path tests red with "AssertionError not raised" — exactly the spec's fabricated-fixture-passes-the-old-surface shape); tree restored byte-identical (git status clean), 120/120 green again.

5. Test coverage — all 5 spec-named legs present plus 2 extras (non-callable probe_marker; the in-suite RED mutant via the T310 exec-copy idiom with byte-identical re-read). PASS no-keyword rides the test's OWN dynamic probe (never hardcoded); FAIL tests assert BOTH values, the surface, the fire, and the reachability doctrine; the drift test uses the repo root commit (reachable-but-different) — equality, not reachability, is the contract.

6. Mutation testing — 5/5 mutants KILLED, run in parallel worktrees (cap 3; all mutants touched scripts/wrap_assert.py only, but each rode its own throwaway checkout with python-only targeted runs, so no interference — the declared overlap judgment): M1 drop-fire (caught by the 2704-shape + drift + RED-proof tests), M2 flip != to == (8 failures, PASS paths fire on match), M3 probe-under-override (the override raiser test + the T311 keyword-forward test), M4 except OSError->ValueError (probe-failure test ERRORS on the escaped OSError), M5 %H->%h short-hash probe (7 failures, the owned probe's 40-hex shape check fails closed).

7. Gates — python3 scripts/test_wrap_assert.py 120/120 OK (113 pre-existing + 7 new, counts verified against 632ad7f~1). cargo build rc 0, cargo clippy --all-targets -- -D warnings rc 0, cargo nextest run --release 1836/1836 PASS rc 0 (CARGO_TARGET_DIR=target-shared-validate-a, T195 touch prefix on every cargo command).

8. Observations (non-blocking): (a) the header comment `# assert_state_on_disk(..., probe_marker=_probe_eval_commit)` names a default the signature expresses as None (-> owned probe); functionally identical, docstring + TypeError message accurate — cosmetic. (b) Two T311 fixtures were modified to carry the probed marker — FORCED by the always-on leg (the old 7-char placeholder marker cannot pass an always-on equality leg); assertions unchanged, documented in the commit; the spec's "battery stays green unmodified" reads as "stays green" and is satisfied. (c) LOOP-SPEC.md ~1126 still narrates the old opt-in shape ("probed ... when gated") — out of this spec's declared scope, doctrine-debt follow-up. (d) Diff is 472 insertions vs the ~85-line estimate — estimate undershoot, no correctness impact.

9. Hygiene — throwaway worktrees /tmp/chug-mut-t312-{1..5} removed (git worktree list shows only main + this worktree); this worktree byte-clean (git status empty); .chug/decisions.jsonl absent (empty per instructions); no decision_log calls made.
