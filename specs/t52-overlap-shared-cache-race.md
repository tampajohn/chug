# SPEC — t52: T44 overlap × target-shared — role-keyed target dirs (cross-checkout artifact race)

One concern: two cargo processes sharing `target-shared` from DIFFERENT
checkouts race on identical artifact filenames — a test binary compiled from
one checkout's source can be executed by another checkout's `cargo test`.
Make concurrent cargo consumers use role-keyed target dirs so a binary is
always executed by the checkout it was compiled from.

## Repo context

- T47's shared cache: every worktree builds into ONE
  `CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared` (impl-goal
  export, orchestrator gate prefix, loopd.sh:67 env prefix).
  tests/shared_target_dir.rs pins the carriers (bare-export ban,
  count-based per-carrier pins).
- T44 pipeline overlap allows 2 children in flight: 1 validator + 1 impl.
- T48 (d000b72) made path-reading tests cwd-relative, closing the
  compile-time env! path leg. The CONTENT leg this spec closes is separate.
- OBSERVED (cycle 22, T48 validation window): while the T48 kimi validator
  mutation-tested in /tmp/chug-loop-t48 (building into target-shared), the
  orchestrator's T49 review gates in /tmp/chug-loop-t49 executed the
  validator's leftover MUTANT test binary → one integration suite red 6/6;
  re-run green after the artifact was rebuilt from t49's own source. The
  validator's own PASS was uncontaminated (it finished before the
  orchestrator's builds). Mechanism, verified against the cache:
  (a) cargo's target metadata hash excludes the checkout path — the same
      package+profile+features produces the SAME artifact filename
      (e.g. shared_target_dir-e79273a164543ff6) for every worktree;
  (b) dep-info paths are RELATIVE (`tests/shared_target_dir.rs`), so
      mtime-freshness compares the artifact against the CALLING checkout's
      source files;
  (c) a freshly built artifact is newer than a just-created worktree's
      checkout mtimes → cargo calls it fresh and RUNS the other checkout's
      binary. Cargo's target-dir lock serializes compilation; it does NOT
      prevent interleaved build→run sequences across processes.
- False legs: RED (this cycle — mutant or stale content; loud, self-heals
  on rebuild, burns a review pass) and GREEN (a reused binary's assertions
  test another checkout's code while the current tree is broken — silent;
  the pin-count tell (452 vs 453) is easy to miss).
- Three concurrent-consumer cases exist under T44 overlap:
  (1) validator mutation builds vs the other worktree's gates (observed);
  (2) validator mutate→build→run vs the impl child's builds (the validator
      can eat the impl's artifact mid-mutation);
  (3) orchestrator review/merge gates for item N vs impl N+1's builds.
  Non-overlapping (serial) use of one dir is safe: T44's cap means at most
  ONE builder per role at a time.

## Requirements

1. **Role-keyed persistent target dirs.** LOOP-SPEC gains the rule:
   - impl children: `target-shared` (unchanged);
   - validators: `CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-validate`
     — ALWAYS, not conditionally (a conditional rule is a future
     mis-application; the T44 cap makes one validator dir sufficient
     because validators are serial). Step 4's launch paragraph and the
     §6-goal export sentence it carries change to this path.
   - orchestrator review/post-merge gates: `target-shared` when no child
     is in flight; `CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-gates`
     whenever an impl child may be concurrently building (the T44 overlap
     window, including N+1's impl during N's post-merge gates). Step 3's
     gate prefix sentence gains the conditional + one sentence naming the
     mechanism (metadata hash excludes checkout path → same artifact name;
     last-builder-wins).
   Each dir persists across cycles (warm after first use); first use is a
   cold build, which is the accepted one-time cost per role.
2. **`.gitignore`** gains `target-shared-validate/` and
   `target-shared-gates/` next to `target-shared/`.
3. **tests/shared_target_dir.rs** extended: pins for the two new
   .gitignore entries and for the new LOOP-SPEC carriers (validator goal
   export path; step-3 conditional sentence), following the existing
   count-based pattern. Existing T47 pins must stay green.
4. **README.md** loopd/T47 paragraph: one clause noting validators and
   overlap-window gates use sibling dirs so concurrent cargo consumers
   never share an artifact slot (integrated, not appended).
5. No change to loopd.sh's own env prefix (line 67): it is the
   orchestrator's DEFAULT; the gate commands override per the step-3
   conditional.
6. Doctrine item: runs ALONE (no overlap) per the T44 doctrine rule.

## Tests

- Extend tests/shared_target_dir.rs per req 3.
- `grep -c "target-shared-validate" LOOP-SPEC.md` ≥ 2 (step 4 paragraph +
  goal export sentence) — encode via the existing pin pattern.

## Acceptance

- `cargo test` green (existing T47 pins + new pins).
- `grep -n "target-shared-validate\|target-shared-gates" LOOP-SPEC.md
  .gitignore README.md tests/shared_target_dir.rs` shows all four
  surfaces.
- Validator launches after this lands carry the isolated export (visible
  in their delegate.log banner commands).

## check: cargo test --test shared_target_dir
