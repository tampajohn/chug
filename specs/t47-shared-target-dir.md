# T47 — Shared CARGO_TARGET_DIR for worktree builds

check: cargo test

## Concern

Every round's worktree builds chug from scratch in its own target dir
(measured 43s–6 min per worktree, and the validator's mutation-test runs
rebuild after each revert). With overlap (T44) this doubles. A shared
target dir turns per-round builds into incremental ones.

## Repo context

- `LOOP-SPEC.md` §2 step 1: `cargo build` in each `/tmp/chug-loop-t<N>`.
- Validators mutate → test → revert → re-test: 2–4 builds per round.
- Cargo locks the target dir during builds; concurrent builds queue
  safely (relevant once T44 overlap runs two children).

## Requirements

1. LOOP-SPEC/META-SPEC child-launch templates export
   `CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared` (a NEW dir,
   NOT the repo's own `target/` — keep the operator's build cache separate
   so a wedge can't poison daily builds).
2. loopd.sh creation: `mkdir -p target-shared` at supervisor start;
   `git clean`/`cargo clean` policy: none automatic — note in the spec that
   reclaiming is operator's call (`du -sh target-shared`).
3. `.gitignore` gains `target-shared/`.
4. Templates note the tradeoff: a poisoned shared cache affects all
   children; recovery is `rm -rf target-shared` (cheap, rebuild once).

## Tests

- Doctrine item: validation REQUIRED — review checks the separation from
  `target/`, the gitignore line, and that no template references the repo's
  own target dir.
- Acceptance: a subsequent cycle's events show worktree `cargo build`
  completing in seconds (warm cache) — record in that cycle's Outcomes.

## Out of scope

- sccache/mold/linker tuning; sharing `target/` itself.
