# T212 — delegate's T197 drift advisory must read the spec copy the goal gate reads

## Repo context

T197 (landed) added a launch-time target-dir drift advisory to `delegate`:
`src/delegate.rs` `target_dir_drift(spec_text, goal, env_dir)` (pure seam,
~line 456) compares the CARGO_TARGET_DIR carriers across three surfaces —
the spec's `check:` line, the goal text, and the `env` map — and the launch
return text gains a `WARN target-dir drift:` block when they disagree
(caller at ~lines 553-560).

The advisory's spec text comes from `fs::read_to_string(&spec)` where
`spec` is the launch argument — the MAIN-repo absolute path
(`/Users/jadams/workspace/chug/specs/t<N>-<slug>.md`). But the child's
goal gate reads the WORKTREE copy (`<cwd>/specs/t<N>-<slug>.md`), and the
orchestrator's dispatch-time check re-key (T175/T52 doctrine: the check
line's export is re-keyed to the child/validator's role slot on-branch
before launch) edits only the worktree copy. At launch, main's copy still
names the default `target-shared` slot while the goal/env name the
role-keyed slot → the advisory fires on a CORRECTLY pre-keyed dispatch.

Evidence (cycle 97, two sightings): the WARN fired at the t209 validator
launch (re-key 3d8f6ca on-branch) and the t210 validator launch (re-key
1526e5a on-branch) — both dispatches were correct; both warnings were
false positives (cycle-97 Outcomes carried observation (a)). A
false-positive advisory on a load-bearing warning is the
boy-who-cried-wolf class: it trains orchestrators to ignore TRUE drift.

`delegate.rs` is NOT on LOOP-SPEC §2 step 4's core-validation list; the
expected diff is well under 150 lines with no new tool/command surface —
T189 lane-eligible at dispatch (mechanical inputs decide there, not here).

estimate: ~90 changed lines (src/delegate.rs resolution rule + unit legs;
test-dense per the cycle-98 eval's pin-carrier calibration)

## Requirements

1. At `delegate` launch, the drift advisory's spec text is resolved as:
   if a file exists at `<cwd>/specs/<basename-of-spec-arg>`, read THAT
   copy (the worktree copy — the same file the child's goal gate will
   read); otherwise read the spec argument path as today (chat-side and
   cwd-external spec launches keep current behavior).
2. The resolution rule is named in one comment at the call site: the
   advisory must judge the copy the goal gate reads, because the
   dispatch-time re-key (T175) edits the worktree copy on-branch.
3. Failure legs byte-identical to today: unreadable/missing spec at BOTH
   candidate paths → no advisory block (the advisory never blocks a
   launch), and the return text stays byte-identical when no drift.
4. The true-drift path is unchanged: when the resolved copy's check line
   disagrees with goal/env carriers, the WARN block fires byte-identical
   to today.
5. `target_dir_drift` stays pure — the change is WHICH text is fed to
   it, never its comparison logic.

## Tests

Unit legs in `src/delegate.rs`'s test module (tempdir fixtures):

- **Worktree-copy-wins leg**: tempdir `cwd` containing
  `specs/t999-x.md` keyed to `target-shared-validate-a`; spec ARG path
  pointing at a second file (outside the tempdir, or a second tempdir)
  keyed to plain `target-shared`; goal+env carry `validate-a`. Assert NO
  WARN (the resolved copy agrees) — RED before the fix (main-path read
  warns), green after.
- **Fallback leg**: no `<cwd>/specs/<basename>` present → the spec arg
  path is read (drift in the arg copy still warns; no-drift stays
  silent).
- **Both-missing leg**: neither path readable → no advisory, launch
  return text byte-identical to the pre-T197 shape (the never-block
  pin).
- **True-drift-via-worktree leg**: worktree copy keyed to validate-a,
  goal/env carry plain `target-shared` → WARN fires (the advisory still
  judges the copy the gate reads when it IS the drifted one).
- The existing T197 unit legs keep passing byte-identical (the
  comparison logic untouched).

## Acceptance

- All new legs green; the worktree-copy-wins leg is RED-proven against
  the pre-fix code (orchestrator or child proves the mutant RED by
  reverting the resolution rule, then restores).
- Full unit suite (`cargo test --bin chug`) green; clippy
  `--all-targets -- -D warnings` clean.
- No behavior change to launch mechanics, budgets, or the env map.

## Out of scope

- Changing the WARN text, the comparison logic, or the three-surface
  contract (T197's design stands).
- Teaching the orchestrator anything (doctrine unchanged — the T175
  re-key stays a dispatch-time act; this fix makes the advisory see it).

check: export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared; touch src/*.rs tests/*.rs; cargo test --bin chug
