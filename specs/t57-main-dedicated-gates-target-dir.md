# T57 — Post-merge gates use a main-dedicated target dir (`target-shared-main`)

check: grep -q "target-shared-main" LOOP-SPEC.md && grep -q "^target-shared-main/$" .gitignore && cargo test --test shared_target_dir

## Repo context

T47 (`2f4cefc`) gave every worktree build one shared `CARGO_TARGET_DIR`
(`target-shared/`); T52 (`4d4c383`) role-keyed the CONCURRENT case:
validators build into `target-shared-validate/` ALWAYS, overlap-window
gates into `target-shared-gates/`, impls keep `target-shared/`. The
mechanism both rows cite: cargo's artifact filename does not encode the
checkout path, so the same package+profile+features produce the SAME
artifact filename in every checkout — one shared dir is
last-builder-wins, and mtime-based freshness can then execute a binary
compiled from a DIFFERENT checkout's source.

The SEQUENTIAL residue of that class is still open (cycle-26 eval I2):
at T55's post-merge gates the step-3 worktree gates had compiled the
worktree's PRE-T53 `tests/loopd_reexec.rs` into `target-shared`; the
merge did not touch that file, so its main-checkout mtime stayed OLDER
than the artifact; cargo considered the artifact fresh and ran the
stale 4-test binary against main's ps-based `loopd.sh` → 3/4 FALSE RED
(touch+rebuild recovered; receipt in the cycle-25 Outcomes entry). The
symmetric false-GREEN leg — a stale PASSING binary masking a real main
failure — is silent. First sighting of the class was T43's
`env!(CARGO_MANIFEST_DIR)` bake (T48 fixed the compile-time leg; this is
the mtime leg).

Why a fourth dir and not a narrower rule: during a T44 overlap,
post-merge gates already use `target-shared-gates`, but worktree review
gates during ANY earlier overlap window also wrote worktree-source
artifacts into that same dir, so the sequential staleness can land in
`target-shared-gates` too. Only a dir whose builders are ALWAYS main
checkouts makes artifact identity correct by construction. First use is
one cold build (~40s), warm thereafter — the T52 precedent's accepted
one-time cost per role.

## Requirements

1. **LOOP-SPEC.md** — the post-merge gate run (§2 step 5, "merge to
   main, re-run gates in main") and the Phase-3 final gates in main use
   `CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-main`,
   ALWAYS (never conditionally on overlap). Step 3's worktree-review
   gates keep their T52 rule (`target-shared` when no child is in
   flight, `target-shared-gates` during an overlap window). One
   mechanism sentence, woven into the existing T52 paragraph style:
   cargo's artifact filename excludes the checkout path, so a shared
   dir's artifact slots are last-builder-wins and only a
   main-builders-only dir keeps post-merge artifacts identical to main
   content (the T55 false-red receipt named in a parenthetical).
2. **.gitignore** — gains `target-shared-main/` as a line contiguous
   with the existing `target-shared*/` entries.
3. **README.md** — the continuous-mode paragraph's target-cache clause
   (which already names `target-shared-validate/` and
   `target-shared-gates/`) gains `target-shared-main/` for post-merge
   and final main gates — integrated into the existing sentence
   structure, not a new bullet.
4. **tests/shared_target_dir.rs** — gains pins per the T47/T52 carrier
   doctrine (exact-count, strictly stronger than existence):
   (a) `.gitignore` contains `target-shared-main/` exactly once;
   (b) LOOP-SPEC.md names `target-shared-main` at least once AND the
   post-merge gate instruction (`re-run gates in main`) names it;
   (c) the pre-existing T52 pins keep passing unmodified.
5. Nothing else changes: `target-shared`, `target-shared-validate`,
   `target-shared-gates` keep their current roles; META-SPEC.md and
   loopd.sh are untouched.

## Tests

- The new `tests/shared_target_dir.rs` pins (4a/4b) fail before the
  LOOP-SPEC/.gitignore edits and pass after (non-vacuousness hand-check
  by the implementer, then revert).
- `cargo test --test shared_target_dir` green; full `cargo test` green;
  clippy clean.

## Acceptance

- `check:` above passes in the impl worktree.
- Orchestrator re-runs the spec's grep legs and the full suite in main
  post-merge (with the NEW doctrine's own dir: first post-merge gate
  run under `target-shared-main` — its cold build is the acceptance
  evidence the dir works).
- No `|` in the TODO row's notes cell (T40).
