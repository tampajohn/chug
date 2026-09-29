# T159 — bring the loopd_orphan_reaper family into the shared timing-lock domain

check: cargo test

estimate: ~40 lines (lock-domain join + a serialization assertion, tests-only)

## Concern

T152's landing exposed a new member of T151's load-flake class: the three
**through-loopd** tests in `tests/loopd_orphan_reaper.rs`
(`the_cwd_leg_identifies_an_orphan_through_loopd`,
`a_failing_driver_probe_means_no_sweep`,
`the_reaper_terms_an_orphan_through_loopd_before_the_build`) each spawn a REAL
`loopd.sh`, whose sweep walks the real process table spawning per-pid probes.
Under nextest's default parallelism (17-way on this host, cycle 73's post-merge
gate) the three ran concurrently with the rest of the suite and each busted its
30s `wait_for_any` deadline — solo each finishes in <4s, serial
(`cargo test --release -- --test-threads=4`) the whole family is green
(1155/1155). The T82 fallback was used at landing with the family named in the
commit; without this row, EVERY next cycle's nextest gate re-rolls the same
flake and burns a fallback round.

## Requirements

1. Join the three through-loopd tests (and any sibling in the file that spawns
   a real `loopd`) to T151's shared serialization lock domain in
   `src/testsupport.rs` — same mechanism, same discipline: ZERO timeout bumps.
   The tests' own 30s `wait_for_any` deadline is a quiescence cap, not a load
   assumption; with the lock, contention disappears and the cap is untouched.
2. Prove the mechanism the T151 way: a RED-proven killing check — e.g. a
   mutation/perturbation that lets two through-loopd tests run concurrently
   must demonstrably stretch them (record the observed stretch, do not pin a
   wall-clock number), while the locked run is green. If the suite's parallel
   profile cannot reliably reproduce the stretch on this host, say so and pin
   the LOCK-SCOPE invariant instead (a test asserting the three tests contend
   on the one shared domain — the T151 lock-domain pin shape).
3. No product-code changes: `src/` untouched except `src/testsupport.rs` if
   the domain needs a new accessor; every other diff line lives in
   `tests/loopd_orphan_reaper.rs`.
4. Gates: build + clippy `--all-targets -- -D warnings` green; nextest
   `--release` green WITH the lock in place (this is the acceptance that the
   fallback round is no longer needed).

## Out of scope

- Re-litigating T151's domain design (it landed; this row is a join, not a
  redesign).
- The `install_sh real_uname_path_drives_the_mapping` 221s slow test — a
  watch item, not a flake (it passed, slowly); note it in the row notes if it
  fires for real.
- Any reaper behavioral change (T152 is done and validated).
