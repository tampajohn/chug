VERDICT: PASS

T314 validation — scripts/scoped-find.sh (the T293 6th-fire re-weigh LAND)

Findings:

1. REQUIREMENT 1 (script) MET and verified functionally. scripts/scoped-find.sh
   (new, mode 100755) composes exactly the spec's command:
   `find <root> -maxdepth <N> -path <root>/target -prune -o -path '<root>/target-shared*'
   -prune -o -path <root>/.git -prune -o <find-args> -print` with N defaulting to 4;
   echoes the exact composed command to stderr before running (copy-paste-recomposable
   quoting via shq); exit status is find's own via exec. Sandbox-verified: prunes
   target/, target-shared-*, .git (hits in each suppressed, src hit printed); default
   maxdepth 4 bounds the window (depth-5 file hidden, appears with --maxdepth 8);
   bad predicate propagates find's exit 1; usage errors exit 2; root-with-space quoted
   correctly; trailing-slash root normalized (`./` -> `.`). All shell constructs POSIX /
   BSD-clean (ran green on macOS BSD find; sed BRE only). Spec smoke:
   `scripts/scoped-find.sh . -- -name 'com.tampajohn.chug-loopd.plist'` prints exactly
   `./scripts/com.tampajohn.chug-loopd.plist`, exit 0; acceptance smoke `-name '*.plist'`
   returned the launcher plist in 0.76s.

2. REQUIREMENT 2 (doctrine amendment) MET with mechanically verified counts. Census
   moved five -> six fires; the sixth fire's evidence is named (cycle-2729 eval boot's
   pruned-over-target/target-shared* but .git-unpruned, no -maxdepth window, killed at
   the cap AFTER printing count-1, zero casualty, scoped re-probe < 1s); the closing
   re-arm clause became the LANDED verdict (the 6th fire re-weighed and LANDED the
   helper (T314), reach for the canned scoped find FIRST). Python count of the
   probe-discipline window (T284 lead -> ## Write `EVALUATION.md` heading):
   `scripts/scoped-find.sh` x1, lowercase prune-first x1, PRUNE-FIRST x1, -not -path x2,
   record d1791821960-7 x1 (also x1 file-wide), zero-casualty note x1, mechanism x1,
   GUARD x1, WALK_DETECTOR_REJECT x1; no 'five fires' remnant; paragraph placement
   unchanged (after the T284 lead, before the Write EVALUATION.md heading).

3. REQUIREMENT 3 (pin) MET. Leg (e) resolves scripts/scoped-find.sh from the runtime
   checkout (current_dir, the T48 idiom), asserts executable (mode & 0o111), the three
   prune clauses (`-path "$ROOT/target" -prune`, `-path "$ROOT/target-shared*" -prune`,
   `-path "$ROOT/.git" -prune`) exactly once each, `-maxdepth` present and the
   `MAXDEPTH=4` default exactly once. Battery count moved 4 -> 5 (5/5 green). Header
   comment keeps the fifth-fire narrative and gains the sixth-fire + T314-landing
   paragraph. No wrap-battery enumeration needed updating (quoted per-wrap, not pinned).

4. GATES all green: cargo build --release clean; cargo clippy --release --all-targets
   -- -D warnings clean; cargo test --release -- --test-threads=4: 1837 passed / 0
   failed across 51 targets (EXIT=0); cargo nextest run --release (on PATH, nextest-first
   per the T82 rule): 1837 passed (1 slow), EXIT=0. Exact spec check line re-run:
   both legs PASS.

5. MUTATION-TESTED (serial — M1..M7 all touch the same two files
   [scripts/scoped-find.sh, META-META-SPEC.md], the overlapping-files default; run
   against the already-built pin binary in the main worktree with restore after each,
   so no throwaway worktrees/target dirs were needed; the mutated artifact is a
   runtime-read text file, not compiled code):
   M1 delete the script -> leg (e) RED;
   M2 strip the `.git` prune clause -> leg (e) RED;
   M3 strip the `-maxdepth` default (MAXDEPTH=4 -> 9) -> leg (e) RED;
   M4 (bonus) strip the target-shared* prune clause -> leg (e) RED;
   M5 (bonus) chmod -x -> leg (e) RED;
   M6 (bonus) add a 2nd lowercase prune-first in the window -> leg (b) RED;
   M7 (bonus) add a 3rd `-not -path` in the window -> leg (c) RED.
   All mutants restored; git status byte-clean before this verdict.

6. Notes (non-blocking): leg (e) is a static-text pin — exactly the pin surface the
   spec's requirement 3 specifies; the runtime behavior (stderr echo, prune coverage,
   exit status, sub-second smoke) is covered by the spec check line and was verified
   manually in finding 1. `.chug/decisions.jsonl` in this worktree does not exist
   (stays empty as required); no decision_log calls were made.
