VERDICT: PASS

T323 (assert_fill_eval_state_untracked + RED-proven fixture legs + ONE LOOP-SPEC sentence), validated adversarially per META-SPEC §6 on branch loop-t323, worktree /tmp/chug-loop-t323, HEAD 8522e19. CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-validate-a on every cargo command.

Findings:

1. Gates — all GREEN, all re-run by this validator (never trusted from the commit message):
   a. T82 gate with the T195 touch prefix (touch src/*.rs tests/*.rs): cargo nextest run --release — 1839/1839 passed in 77.895s, exit 0.
   b. The spec's check: python3 scripts/test_wrap_assert.py — 187/187 OK (89s), covering the 9 new T323 legs + the four 20->21 pin bumps + all prior legs.
   c. cargo clippy --all-targets --release -- -D warnings — exit 0 AND zero "warning" lines in the log (case-insensitive grep count 0), i.e. the zero-warnings bar, not mere exit-0.
   d. cargo test --release --test todo_consistency (the bookkeeping floor) — 21/21 ok, exit 0.

2. Requirements conformance — all five spec requirements verified against the landed diff:
   a. (req 1) The leg runs the pinned argv list ["git","-C",repo_root,"ls-files","--",<pair>] via subprocess.run with no shell; asserts the output EMPTY; the fire names (i) each tracked path, (ii) the consequence class (delta_bookkeeping_only's file leg -> book=no every cycle -> the T258 cheap-exit predicate dead -> silent sustained forced-launch burn), (iii) the repair recipe (git rm --cached <path> per path, bb339d9/ac4994f precedent, .gitignore:12 re-covering — line 12 of the repo .gitignore verified to literally be `.chug/`); the invocation-failure surface fails closed on BOTH branches (rc!=0 naming rc+stderr; OSError naming the OS error).
   b. (req 2) Single repo_root input, no remembered expected value — the empty-set expectation IS the probe's output (T270/T288 probed-expectation satisfied by construction). TypeError interface defense on non-str input.
   c. (req 3) LOOP-SPEC.md gains exactly ONE sentence in the family's own shape inside the T317/T321 fill pre-commit assert paragraph (verified in place at :1155-1163, between the T321 streak sentence and the T311 sentence).
   d. (req 4) All six required fixture legs present (3024 force-add raise; disk-only-untracked pass; both-tracked naming BOTH; delta-alone naming ONLY delta; non-repo fail-closed; harvest-exception green) plus the TypeError leg and the in-memory red-provability leg; every fixture is a temp-dir git repo, never the live repo's git state.
   e. (req 5) Scope exact: git show --stat names only scripts/wrap_assert.py, scripts/test_wrap_assert.py, LOOP-SPEC.md (405 insertions, 16 deletions).

3. Mutation testing (validator-run independently, /tmp/t323-mutants/mutate.py, mutants via in-memory exec of mutated source strings — the worktree only ever READ; 19/19 checks as expected):
   a. Baselines re-derived by hand: RED on the 3024 force-add shape (message content verified: path+repair+consequence+precedent all present), both-tracked, delta-alone (names ONLY delta), non-repo (fail-closed, names probe + git's "not a git repository" stderr); GREEN on the disk-only TRUE shape, the harvest-exception shape, and the LIVE worktree (ground-truth git ls-files probe also empty — the ac4994f repair holds).
   b. M1 (fire stashed: _fire_eval_state_tracked call -> pass): RED-killed on the 3024 shape AND the delta-alone shape — silent green where shipped raises; stays green on the TRUE shape (no false fire).
   c. M2a (argv probe scope: pair shrunk to delta-only): RED-killed on the 3024 shape (state file escapes) while still firing on delta-tracked — proves the kill is the dropped path, not a broken probe. M2b (argv probe command: ls-files -> ls-file): RED-killed — fail-closed fires on a green fixture, proving the probe command is pinned and the rc!=0 leg works.
   d. M3 (rc fail-closed guard -> if False): RED-killed on a non-repo root — the mutant waves a probed-nothing run GREEN, exactly the fail-open fire the leg exists to kill. M4 (OSError except-block -> pass, git removed from PATH): shipped raises AssertionError naming the OS error; mutant dies UnboundLocalError — never the fail-closed AssertionError — RED-killed.
   e. M5 (empty-expectation detection stashed: tracked = []): RED-killed on both the 3024 shape and both-tracked.
   f. Overlap judgment (declared per §6): all mutants touch the same single leg region of wrap_assert.py — overlapping files — so serial execution (one process, in-memory exec) was the correct mode; no parallel mutant worktrees used.

4. Test-leg RED-provability proven beyond the shipped M1 self-test: the new AssertFillEvalStateUntrackedTest class run against a body-stubbed copy of the leg — 6/9 go RED (all four fail-path fixture legs + TypeError + red-provability leg). The 3 survivors (the export pin, the two PASS-shape legs) structurally cannot fail under a silent stub; their sensitivity is covered by M2a/M2b (probe misbehavior flips the PASS shapes red).

5. Class sweep: no defect was found naming an instance, so no sweep obligations triggered; the fail-closed class was swept to BOTH branches (rc and OSError) as M3/M4, and the argv-probe class to BOTH surfaces (path scope M2a, command M2b).

6. Non-blocking observations (not defects, no row): (a) the "~130 changed lines" estimate vs 405 insertions actual (~3x) — the family's verbose docstring/prose convention, informational only; (b) the lazy `subprocess = __import__("subprocess")` idiom and the no-timeout subprocess call both match the module's existing convention exactly (the T312 _probe_eval_commit probe at :2211-2225; zero timeout sites anywhere in the module) — consistent, not findings.

Byte-cleanliness: git status --porcelain empty at verdict time; sha256 of the three changed files identical before/after the mutation harness (wrap_assert.py 1fe942b0…, test_wrap_assert.py 0d707109…, LOOP-SPEC.md 50f9050d…) — all mutation was in-memory on copies; the worktree was never written. Nothing committed anywhere by the validator; TODO.md/LEDGER.md untouched.
