# T163 — goal-gate check timeout vs real check walls (the check-wall)

check: export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared && cargo test --bin chug goal && cargo test --test loop_spec_check_wall

## Repo context

`src/tools.rs:17` — `pub const CHECK_TIMEOUT_SECS: u64 = 600;` caps every
spec `check:` line run by the goal gate (`goal_complete` verification in
src/driver.rs; the rejection message at src/driver.rs:1699–1720 interpolates
the const). `src/driver/tests/goal.rs:23` pins the timeout in the rejection
text via the const. Doctrine carrier: META-META-SPEC.md's spec quality bar
(authors spec `check:` lines). Pin files live in tests/loop_spec_*.rs
(convention: one concern per file, e.g. tests/loop_spec_recovery.rs).

estimate: ~90 changed lines (const + test sweep + doctrine sentence + new
pin file ~60 + README verify).

## Evidence

Cycle-76 eval §2.1: t153-fixup's spec check embedded `cargo clippy
--release --all-targets` + release tests (warm wall >600s) — the child was
rejected TWICE with `check command failed` on green work
(.chug/events-t153-fixup-20260929-185044.jsonl: `goal: rejected 2`);
t152-fixup took one more. The cycle-73 and cycle-75 wraps both name this
the top next-eval candidate (warm full debug suite ~800–1000s vs the 600s
cap). Each false rejection burns child budget and can cost a wrap.

## Requirements

1. Raise `CHECK_TIMEOUT_SECS` from 600 to **1200** (20 min: covers the
   observed 800–1000s warm full-suite wall with headroom; still bounded
   enough that a truly hung check dies inside an 80-iter/35-min child).
2. Sweep every reference to the old value: rejection-message tests
   (src/driver/tests/goal.rs — they interpolate the const, so verify they
   need no edit), any other test or doc that hardcodes `600` as the check
   timeout (`grep -rn "600" src/driver tests/ README.md LOOP-SPEC.md
   META-SPEC.md` — record findings in the commit message; README's
   delegate `wait_secs` max-600 is UNRELATED and must not change).
3. META-META-SPEC.md's spec quality bar gains ONE sentence (near the
   `check:` line rules): spec authors MUST budget the check line's warm
   wall — prefer targeted test binaries that finish in ~300s warm; a
   check line whose warm wall exceeds ~600s (half the gate) must state
   its measured warm wall in the spec's repo-context section.
4. New pin file `tests/loop_spec_check_wall.rs` (loop_spec_* convention):
   (a) `CHECK_TIMEOUT_SECS == 1200` const pin; (b) the META-META-SPEC
   sentence's load-bearing tokens exactly-once (`warm wall`, `~600s`,
   `measured warm wall`); (c) the rejection message still interpolates the
   const (not a hardcoded number) — asserted against the driver source or
   the rendered message, non-vacuous (removing the const from the message
   kills the pin).

## Tests

- The new pin file (a)/(b)/(c), each RED-proven (state the mutant that
  dies in the commit message).
- Existing goal-gate tests stay green unchanged (they use the const).

## Acceptance

- `cargo test --bin chug goal` and `cargo test --test loop_spec_check_wall`
  green; full `cargo test` green; clippy `-D` clean.
- Commit message records: the 600→1200 sweep findings (every reference
  found, changed or justified-unchanged) and the RED-proof mutants.

## Out of scope

- Per-spec timeout overrides, check-line wall measurement tooling, any
  change to the bash tool's own 120s cap or delegate wait_secs' 600 max.
