VERDICT: PASS

T303 validation — release-notes body cap (impl commit 335b2fb, reviewed against specs/t303-release-notes-body-cap.md).

Gates (clean tree, CARGO_TARGET_DIR=target-shared-validate-a):
- spec check: release_scripts 15/15, todo_consistency 21/21, internal_info_lint 8/8 — green
- cargo build --release — green
- cargo clippy --release --all-targets -- -D warnings — green
- cargo nextest run --release — 1815/1815 green

Correctness review findings:
1. Leg 1 (trunc_subj): POSIX `%.*s` byte-precision verified empirically on macOS sh AND dash; `${#1}` is byte length under the script's exported LC_ALL=C; boundary `-gt` is correct (exactly-200-byte subjects pass untouched, no false marker). Real-repo smoke test: a 322-byte eval subject emitted exactly 200 bytes + "..."; marker/passthrough byte-exact.
2. Leg 2 (cap_body): cap math sound — reserved budget uses N=total (digits(N) ≤ digits(total) since N ≤ total), so output ≤ 120,000 always; cut is whole-line (awk record buffering, never partial); dangling heading/blank strip is correct; footer N = rev-list total − kept bullets, and classify()'s catch-all `*) chore` guarantees every commit lands exactly one bullet, so N == omitted-bullet count exactly. Under-cap path passes bytes through unchanged (golden pin confirms).
3. Determinism: no clock, no randomness; rev-list/git-log over the same $range; awk order deterministic. Pinned by reruns in both new fixtures plus the pre-existing reruns_are_byte_identical test.
4. Acceptance surface respected: 335b2fb touches ONLY scripts/release-notes.sh + tests/release_scripts.rs (workflow YAML and check-tag-version.sh untouched). The two later commits (72cc37a, eb60001) are orchestrator-side spec-filing check-line fixes in specs/, not impl.

Mutation battery (6 mutants, isolated worktrees + role-keyed target dirs, 3 in flight max; mutants touch one shared file but run in disjoint worktrees, so parallel-by-worktree was sound):
5. M1 trunc-off (leg 1 disabled) — KILLED by overlong_subjects_are_truncated_with_an_ellipsis_marker.
6. M2 marker-off ("..." dropped) — KILLED by the same pin (byte-exact expected line).
7. M3 cap-off (awk always passthrough) — KILLED by over_cap_bodies_are_cut_at_a_line_boundary_with_one_footer (len > 120,000, no footer).
8. M4 N-wrong (footer N = total, not total − kept) — KILLED by kept+n==TOTAL and the exact-footer pin.
9. M5 strip-off (dangling-heading cleanup removed) — SURVIVED. Weak-test gap (non-blocking): the 600-commit fixture is single-group, so the budget cut always lands mid-bullets and the strip loop is a no-op under the whole suite. The loop is a cosmetic robustness leg beyond the spec's floor — a dangling "## Heading" would still satisfy all four spec assertions (≤ cap, one footer, N correct, no partial line), and the size math accounts for it either way. Optional hardening: a multi-group over-cap fixture whose cut lands right after a heading.
10. M6 boundary-flip (-gt → -ge: exactly-200-byte subjects get a false "..." marker) — SURVIVED. Weak-test gap (non-blocking): no test pins the exactly-at-budget passthrough; test 2 uses 200-byte subjects but asserts only cap/footer/counts, not bullet content. Shipped behavior (-gt) is correct; a one-line addition to the leg-1 test (a 200-byte subject stays untouched) would close it.

Findings list: no correctness bugs, no missing spec requirements. Two non-blocking weak-test observations (items 9, 10) for optional future hardening; both legs' spec-required behavior is pinned and mutation-proven (4/6 killed, survivors are cosmetic/boundary only).
