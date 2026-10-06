# T253 — gate/guard doctrine names the profile: T242 known-warm profile-exact + `--release` on the guard-floor and clippy surfaces

check: cd "$(git rev-parse --show-toplevel)" && export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared && touch src/*.rs tests/*.rs && cargo test --release --test loop_spec_cold_gates --test loop_spec_docs_only_gates --test loop_spec_recovery --test todo_consistency

## Repo context

T78/T82 made every orchestrator GATE leg release-profile
(`cargo nextest run --release`, release warm builds, release post-merge
and final gates), so every shared target dir the loop keeps warm —
`target-shared-main` above all — is warm in the RELEASE profile. But
three families of doctrine text still name the DEV profile or no
profile at all:

1. **T242's known-warm clause is profile-blind.** LOOP-SPEC step 5's
   cold-scale gate-leg rule ends: "Inline is permitted ONLY known-warm:
   a same-HEAD cargo leg already completed THIS cycle in
   `target-shared-main` (e.g. this step's todo_consistency guard run)
   makes a later same-HEAD leg incremental." Phase 3's restatement
   carries the same profile-blind phrasing. Warmth is per-PROFILE: a
   same-HEAD release leg does not warm a debug leg.
2. **The guard-floor invocations run bare `cargo test` (debug).**
   Step 5's main-dedicated invocation
   (`CARGO_TARGET_DIR=... target-shared-main cargo test --test
   todo_consistency`, with the "(seconds)" claim justified by "the
   main-dedicated dir is warm across cycles") and step 3's docs-only
   guard floor both omit `--release`. In `target-shared-main` the
   "(seconds)" claim is falsified for the debug profile.
3. **The clippy templates name no profile.** The impl-goal clippy bar
   (step 2's T176 prose) and the three orchestrator gate surfaces
   (step 3 review, step 5 post-merge, Phase 3 final — pinned verbatim
   by T235's `CLIPPY_ALL_TARGETS_FORM` in tests/loop_spec_recovery.rs)
   all read `cargo clippy --all-targets -- -D warnings`, which runs
   the dev profile.

**Cycle-130 evidence (two fires, one delta — the third/fourth of the
standing inline-cold-scale census, which the cycle-130 eval armed at
re-file-on-3):**

- **Fire 1 (orchestrator, main-dedicated):** the cycle-130 wrap's
  post-merge guard leg
  `CARGO_TARGET_DIR=.../target-shared-main cargo test --test
  todo_consistency --test eval_outcomes_carry --test
  internal_info_lint` (NO `--release`) was killed at the 300s bash cap
  (duration 300,136ms, 20:26:22Z of
  .chug/events-20261006-211733.jsonl) — the first DEV-profile leg
  after the f24ba45 merge moved HEAD (build.rs watches `.git/HEAD`).
  The T242 post-merge window had already exited (PROC-GONE — no lock
  contention); the release profile was warm from the gates that had
  just finished. The identical legs re-run WITH `--release` passed
  instantly (4+8+21 tests, sub-second). T242's letter PERMITTED the
  inline run ("a same-HEAD cargo leg already completed THIS cycle") —
  the clause's example even names the guard run, which is itself a
  cross-profile counterexample.
- **Fire 2 (impl child, worktree shared dir):** t251-impl's
  `cargo clippy --all-targets -- -D warnings` (dev) was killed at the
  300s bash cap inline (19:39:08Z of
  .chug/events-t251-impl-20261006-195843.jsonl); re-run in a T242
  background window the dev clippy took 7m44s cold vs 51s for the
  release clippy (the child's harvested LEDGER:
  .chug/LEDGER-t251-impl-20261006-195843.md). The step-1 warm build
  and the spec check line (`cargo test --release ...`) had warmed only
  release.

Root cause is doctrinal, not cargo: the templates and the known-warm
clause never name a profile, so a leg's warmth is left to inference —
and the inference is wrong whenever the leg's profile differs from the
dir's warm one.

estimate: ~300 lines all-in (±30 doctrine lines across six
LOOP-SPEC surfaces + T235's `CLIPPY_ALL_TARGETS_FORM` needle and
loop_spec_docs_only_gates' `FLOOR_CMD`/`FLOOR_CMD_T8` needle updates +
1–2 new needle legs on tests/loop_spec_cold_gates.rs — the
doctrine+needle-leg kind calibrated ~400–500 all-in on this carrier
per the cycle-130 validator's calibration note; this row's smaller
doctrine surface prices under that band's middle).

Doctrine row: runs SOLO (no other child in flight), kimi validation
REQUIRED (LOOP-SPEC doctrine is on the step-4 REQUIRED list).

## Requirements

1. **T242's known-warm clause becomes profile-exact at BOTH surfaces**
   (step 5's statement and Phase 3's restatement): a same-HEAD,
   SAME-PROFILE cargo leg already completed this cycle in
   `target-shared-main` makes a later same-HEAD, same-profile leg
   incremental — with one sentence stating the per-profile warmth
   principle and naming the cycle-130 fire (a same-HEAD debug guard
   leg died at the bash cap minutes after the release gates ran
   green). The guard-run example stays correct by construction once
   req 2 lands (guard runs become release, matching the gates).
2. **The step-5 main-dedicated guard invocation gains `--release`:**
   `CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-main
   cargo test --release --test todo_consistency` — and the "(seconds)"
   claim in the preceding paragraph gains the profile qualifier (the
   dir is warm across cycles IN THE PROFILE THE GATES RUN — release
   per T78; a bare `cargo test` debug leg is cold-scale there, the
   cycle-130 fire named with its 300,136ms duration and the instant
   `--release` recovery).
3. **Step 3's docs-only guard floor gains `--release`:** the floor
   reads `cargo test --release --test todo_consistency`, keeping every
   orchestrator gate surface on the one profile the loop warms.
4. **All four clippy surfaces name the release form
   `cargo clippy --all-targets --release -- -D warnings`** — step 2's
   impl-goal prose (with one clause of t251-impl evidence: dev clippy
   7m44s cold vs 51s release in the same worktree), step 3's review
   leg, step 5's post-merge leg, Phase 3's final-gates leg. The T235
   pin needle (`CLIPPY_ALL_TARGETS_FORM` in
   tests/loop_spec_recovery.rs) updates to the new exact form (the
   file-wide count stays 4 — all four surfaces change uniformly).
5. **Pin updates + new needle legs:**
   - tests/loop_spec_docs_only_gates.rs: `FLOOR_CMD` and
     `FLOOR_CMD_T8` needles update to the `--release` forms.
   - tests/loop_spec_cold_gates.rs: one new needle leg (T48/T64
     idiom, exactly-once file-wide, windowed to step 5 / Phase 3)
     pinning the profile-exact known-warm phrasing at BOTH T242
     surfaces, the `--release` main-dedicated guard invocation, and
     the cycle-130 evidence tokens (the 300,136ms duration figure or
     an equally load-bearing unique token of the fire).
   - Any pre-existing needle elsewhere that asserts a bare-form
     surface this row edits gets updated in lockstep (the suite is the
     oracle: every pin red at the bare form must end green at the
     release form with its needle updated — never deleted).
6. **RED-proofs:** with the amendments reverted one at a time
   (profile-exact clause, guard `--release`, clippy `--release`), the
   corresponding pin leg goes red; restored byte-identical after each.

## Tests

- Updated needles (T235's clippy form, the floor commands) green at
  the release forms.
- New loop_spec_cold_gates leg(s) green; non-vacuous by req 6's
  RED-proofs.
- Full suite + `cargo clippy --all-targets --release -- -D warnings`
  zero warnings in the worktree.

## Acceptance

- All six LOOP-SPEC surfaces amended as specified; no other
  LOOP-SPEC.md text touched.
- The check line above green in the worktree; full release gates green
  at review.
- META-SPEC.md deliberately UNCHANGED (the LOOP-SPEC override governs
  the loop's surfaces; the divergence is accepted practice per the
  T78/T82 precedent).
