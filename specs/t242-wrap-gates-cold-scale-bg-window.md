# T242 — wrap gates: the cold-scale first cargo leg runs as a bounded background window (doctrine, SOLO)

## Repo context

`build.rs` watches `.git/HEAD` and the loose ref it points at (the T11
banner-hash feature — the binary reports its checkout's commit). Therefore
EVERY main commit — including md-only wrap-notes and eval commits —
invalidates the chug-crate fingerprint in
`CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-main`, and the
first cargo leg after any wrap commit re-lints/rebuilds the whole crate:
measured 4.5–15+ min depending on host load (8m19s cycle 112, 15m27s cycle
113, 2m01s warm-rerun cycle 114). loopd exports `CHUG_BASH_TIMEOUT=300`
(T178), so a cold-scale leg CANNOT fit any inline bash call — the T178
`alarm 280` inner bound is right for per-command wedges and wrong here.

Measured cost of the doctrine gap (the filing evidence, cycle-115 eval §2
I1): FIVE 300s kills across cycles 113–114 (~25 min orchestrator wall) —
cycle 113: `Compiling chug v0.17.3` killed at 15:04:01Z and a second kill at
15:25:23Z before the ad-hoc bg window; cycle 114: a nextest+bg-poll chain
killed at 17:50:09Z (the suite itself went 1670/1670 in 43s — the inline
poll outlived the cap), `Compiling` killed 18:07:02Z, `Checking` killed
18:12:23Z. Cycles 110–113 absorbed the same leg with an ad-hoc background
window; the pattern lives in wrap notes, not in LOOP-SPEC, so each
orchestrator re-discovers it (or doesn't).

estimate: ~60 changed lines all-in (LOOP-SPEC clause + new pin file
tests/loop_spec_cold_gates.rs; doctrine+pin density band 0.4–3.3x applies)

## Requirements

1. **LOOP-SPEC gains the cold-scale gate-leg rule**, carried at BOTH
   orchestrator gate surfaces that run in `target-shared-main` after a main
   commit — the Phase-3 final-gates clause AND the step-5 post-merge gates
   clause (one shared statement, referenced from both): the FIRST cargo leg
   (build / clippy / nextest) in `target-shared-main` after ANY main commit
   is cold-scale by construction and runs as a bounded BACKGROUND window —
   never inline under the bash-tool cap, and never inline-POLLED in the same
   bash call beyond the cap (the cycle-114 17:50 kill shape: the suite
   finished in 43s and the poll died at 300s).
2. **The clause names the mechanism** — `build.rs` watches `.git/HEAD` +
   the loose ref, so every wrap/eval/merge commit invalidates the
   chug-crate fingerprint — so a future editor does not delete the rule as
   superstition, and names the measured band (4.5–15+ min cold vs the 300s
   cap).
3. **The background-window pattern is specified exactly**, not left to
   per-cycle reinvention: `nohup sh -c '<cargo legs>' > /tmp/wrap-gates-<ts>.log 2>&1 & echo $!`,
   then continue other wrap duties, then poll the log in LATER bash calls
   (each poll a quick tail; process liveness via `kill -0 <pid>` with the
   defunct-zombie exclusion); the window itself is unbounded (a nohup'd
   process outlives any single bash call) and the orchestrator's own
   iteration budget is the bound.
4. **Inline is permitted only when known-warm**: a same-HEAD cargo leg
   already completed THIS cycle in `target-shared-main` (e.g. the
   todo_consistency guard run), making a later same-HEAD leg incremental.
   The clause says so explicitly so the rule is not read as banning all
   inline gates.
5. **Pins**: a new `tests/loop_spec_cold_gates.rs` carrier pin — (a) the
   clause tokens appear in LOOP-SPEC.md at both gate surfaces
   (`background window` or the exact phrase the clause lands, the
   cold-scale mechanism token `.git/HEAD`, and the known-warm exemption
   token); (b) a RED-proof-ready shape: each token asserted with
   `assert!(text.contains(...))` counts so removing the clause flips the
   pin RED; (c) the pin file joins the existing `loop_spec_*` family
   naming convention.
6. **No README change** — loop-internal doctrine, nothing user-visible
   (the README gate confirms untouched).

## Tests

- The new pin file's own legs (5a–5c) — run green against the landed
  clause, and the child RED-proves at least one leg by removing one token
  from LOOP-SPEC.md in the worktree (pin flips RED), then restores
  byte-identical.
- The full `loop_spec_*` family stays green (the clause text must not
  break a sibling pin's counts).

## Acceptance

- LOOP-SPEC.md carries the rule at both named surfaces with the mechanism
  and the exact bg-window pattern; `tests/loop_spec_cold_gates.rs` pins it;
  the family is green; no other file touched (doctrine rows stay in their
  named targets).

check: export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared; touch src/*.rs tests/*.rs; cargo test --test loop_spec_cold_gates --test loop_spec_check_wall --test loop_spec_docs_only_gates --test loop_spec_tag_doctrine --test loop_spec_validation_lane --test todo_consistency
