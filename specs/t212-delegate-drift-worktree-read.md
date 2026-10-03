# T212 — the goal gate must read the spec copy the branch re-keys (launch-template spec arg → worktree copy)

## Repo context — CORRECTED PREMISE (round-1 validator finding)

Round 1 (874c620) fixed the WRONG bug. Its premise — "the child's goal gate
reads the WORKTREE copy of the spec" — is FALSE, proven by the round-1 kimi
validator and independently re-verified by the orchestrator:

- `src/main.rs:87`: the `--spec` arg is documented "re-read every iteration".
- `src/driver.rs:462` reads `cfg.spec_path` (the ARG path) at startup;
  `src/driver.rs:1171-1175` re-reads `knobs.spec_path` (the same ARG path)
  every iteration; the goal gate's check line is parsed from THAT text
  (`driver.rs:1556` `parse_check_command(spec_text)`).
- The launch template's spec arg is the MAIN-repo absolute path
  (`/Users/jadams/workspace/chug/specs/t<N>-<slug>.md`), so the gate reads
  MAIN's copy — NEVER the branch-re-keyed worktree copy. Evidence: the
  t209/t210 validators' verifying events ran the un-keyed `target-shared`
  check while their worktree copies were keyed validate-a; and T175's only
  WORKING instance (607e877, cycle 77) is a first-parent MAIN commit — it
  worked precisely because it edited MAIN's copy mid-flight.
- Therefore the cycle-97 drift WARNs (and the WARN at this row's own
  validator launch) were TRUE positives: the gate genuinely ran a different
  dir than the child built into. T197's advisory was CORRECT all along —
  it reads the same arg-path copy the gate reads.

The real defect class: the T175 doctrine (re-key the spec's check line
on-branch at dispatch) never reaches the gate — and leaves slot-keyed
residue behind when the branch merges (main's copies of specs/t209 and
specs/t210 still carry `target-shared-validate-a` today). Round 1's
advisory change would have silenced the ONE component that was telling the
truth, so this row REVERTS it and instead makes the gate, the advisory, and
the child's prompt read ONE copy by pointing the launch's spec arg at the
worktree copy — the copy the branch re-key actually edits.

Why the template fix over a driver-side resolution rule (the validator's
other remedy): a driver rule (`<cwd>/specs/<basename>` shadows the arg)
silently changes spec resolution for EVERY chug run (cwd-shadowing
surprise for ad-hoc users); the template change is explicit, local to the
loop's launches, and makes gate/advisory/prompt consistent in all cases
(chat-side delegate launches included — arg path = the one copy).

This is a DOCTRINE row (LOOP-SPEC edit) → SOLO, kimi REQUIRED.

estimate: ~130 changed lines (LOOP-SPEC template + rule sentence + pin legs
+ the delegate revert + 2 spec-residue restores + this spec's rewrite;
pin-dense per the cycle-98 eval calibration)

## Requirements

1. **Revert 874c620's code half in full**: `src/delegate.rs` returns to its
   pre-T212 state (no `advisory_spec_text`; the T197 call site reads the
   spec arg path directly — that read was correct), and the 4 round-1 legs
   in `src/delegate/tests/launch.rs` + the `mod.rs` count-pin bumps revert
   with it (counts back to 28/104). `git diff main -- src/` must be EMPTY
   at merge except this row's intended src diffs (none — the row is
   doctrine+specs+tests-of-doctrine only after the revert).
2. **LOOP-SPEC step 2's launch template**: the `spec:` argument becomes
   `/tmp/chug-loop-t<N>/specs/t<N>-<slug>.md` (the WORKTREE copy), with ONE
   new rule sentence immediately at that line: the goal gate re-reads the
   spec arg every iteration (driver.rs:1171), the T175 dispatch-time re-key
   edits the worktree copy on-branch, so the spec arg MUST name the
   worktree copy — a main-path arg makes the gate read main's un-keyed
   copy (the cycle-97 true-positive WARNs; the round-1 validator finding).
   The T63 resume sentence and step 4's validator launch inherit the same
   template path (name both in the sentence or its immediate continuation).
3. **Restore the slot residue**: main's copies of
   `specs/t209-dispatch-size-gate.md` and
   `specs/t210-placeholder-guard.md` check lines return to
   `export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared;`
   (the default slot; slot-keying is a dispatch-time act on the branch,
   never main-resident state).
4. **Pin** (todo_consistency.rs, the T210 pin's home): the template's
   `spec:` line carries `/tmp/chug-loop-t<N>/specs/t<N>-<slug>.md` exactly
   once, the rule sentence's needles (re-read every iteration / worktree
   copy / cycle-97 true-positive naming) present, and the OLD main-path
   template form `spec:        "/Users/jadams/workspace/chug/specs/t<N>-`
   occurs ZERO times in LOOP-SPEC.md. RED-proven by a deletion/revert
   mutant.
5. The T210 placeholder-marker pin, the T57 three-carrier pin, and every
   existing doctrine pin stay green — if the template edit collides with a
   needle, the needle is updated IN THE SAME COMMIT with its reason named.
6. No change to `src/driver.rs`, the T197 advisory's comparison logic, or
   the WARN text.

## Tests

- The new todo_consistency pin legs (req 4) green + RED-proven.
- `git diff main -- src/` empty (the revert is complete — assert in the
  commit message).
- Full doctrine guard suite: `cargo test --test todo_consistency` green;
  `cargo test --test loop_spec_recovery --test shared_target_dir` green
  (the template window is pinned by several families).
- Full unit suite + clippy `--all-targets -- -D warnings` clean (the
  revert touches src/delegate.rs mechanically).

## Acceptance

- The round-1 validator's finding is answered point-by-point in the commit
  message: gate reads arg path → the arg now names the worktree copy;
  advisory was correct → its round-1 change reverted; residue → restored.
- A launch replay leg (manual, shown in the commit message): a dispatch
  whose worktree copy is re-keyed to validate-a launches with the
  worktree-copy spec arg and the advisory prints NO WARN (goal/env/check
  agree), while a launch whose goal/env name a DIFFERENT dir than the
  worktree check line still WARNS (true drift stays loud). This leg runs
  against the built binary in the worktree (delegate launch into a
  throwaway worktree or a dry harness — the implementer picks the cheapest
  honest demonstration).
- Gates: full nextest release suite green + clippy -D.

## Out of scope

- Any driver-side spec-resolution rule (rejected: cwd-shadowing semantics
  for every run).
- Changing T175's re-key doctrine text (it describes the intended
  behavior; this row makes the mechanism match it).
- src/ changes beyond the revert (none intended).

check: export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared; touch src/*.rs tests/*.rs; cargo test --bin chug && cargo test --test todo_consistency --test loop_spec_recovery --test shared_target_dir
