# T65 — README Continuous-mode target-cache paragraph de-accretion

One concern: the README's Continuous-mode target-cache paragraph accreted
T47 + T52 + T57 into a single ~12-line sentence chain with three nested
em-dash parentheticals; the mechanism rationale belongs to a loopd.sh
editor (and lives in the specs), not to a README reader.

## Repo context

- `README.md` "Continuous mode (`loopd.sh`)", the paragraph beginning "The
  supervisor also creates `target-shared/`" (around :266–280): one sentence
  chain carrying (i) T47's per-invocation-env-prefix rationale ("never a
  supervisor-wide export, which would persist across loop iterations and
  redirect the supervisor's own `cargo build` … leaving `./target/debug/chug`
  stale"), (ii) T52's role-keying of validate/gates sibling caches, (iii)
  T57's main-dedicated dir + the last-builder-wins mechanism, and (iv)
  operator-reclaim instructions — three nested em-dash parentheticals deep.
  Cycle-29 eval §6(d) filed it as the README's current accretion spot
  (T51/T56/T60 fixed the previous ones; accretion is a bug class, not a
  style choice — META-META-SPEC §6).
- **Pinned surface**: `tests/shared_target_dir.rs` (:505–528) pins parts of
  this paragraph — the README must keep the T52 sibling-cache clause, must
  name `target-shared-main/`, and must say the dir serves the post-merge
  (and final) gates. The rewrite MUST keep those pins green: preserve the
  pinned tokens; if a pinned phrase must move, update the pin in the same
  diff with non-vacuousness evidence (pin red on the mutant, green after).
- The mechanism + rationale already lives in `specs/t47-shared-target-dir.md`,
  `specs/t52-overlap-shared-cache-race.md`, and
  `specs/t57-main-dedicated-gates-target-dir.md` — the README points, the
  specs carry.

## Requirements

1. Split the paragraph into (a) one short paragraph of USER semantics: the
   four gitignored role-keyed caches exist — `target-shared/` (impl children
   + worktree-review gates), `target-shared-validate/` (validators),
   `target-shared-gates/` (overlap-window gates), `target-shared-main/`
   (post-merge + final main gates) — warm after first use; one sentence of
   why (cargo's artifact filename excludes the checkout path, so one shared
   dir is last-builder-wins — role-keyed dirs keep each consumer's artifacts
   its own); operator reclaims (`du -sh` / `rm -rf`; nothing cleans them
   automatically). And (b) one pointer sentence naming specs t47, t52, t57
   for the mechanism and rationale.
2. The per-invocation-env-prefix fact compresses to a clause ("the
   supervisor hands `CARGO_TARGET_DIR` to each cycle as a per-invocation
   env prefix — loopd.sh carries the rationale") — the full
   export-in-while-loop hazard story leaves the README (it is
   loopd.sh-editor-facing and lives in spec t47).
3. **Keep `tests/shared_target_dir.rs` green**: preserve its pinned README
   tokens (verify by running the test before and after); if a pinned phrase
   must move, adjust the pin in the same diff and show non-vacuousness.
4. Docs-only: README.md (+ the pin file only if req 3 forces it). Every
   other README section byte-identical — integrated into the existing
   paragraph's position, not appended (LOOP-SPEC README gate).
5. Net line change should be negative or near-neutral (the paragraph
   shrinks).

## Tests

No new tests (docs-only). Existing pins must stay green:
`cargo test --test shared_target_dir`. Full suite + clippy green.

## Acceptance

- The paragraph reads as two short paragraphs per req 1; no nested em-dash
  chains remain; all four dir names still present; the three specs named.
- `tests/shared_target_dir.rs` green (and byte-identical unless req 3
  forced a same-diff pin update with evidence).
- `cargo test` + `cargo clippy --all-targets -- -D warnings` green.
- Adversarial validation SKIPPED (docs-only — T16/T31/T35/T51/T56/T60
  precedent); orchestrator gates + full diff review suffice.

check: cargo test --test shared_target_dir && grep -c "target-shared-main" README.md
