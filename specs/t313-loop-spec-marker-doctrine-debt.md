# t313-loop-spec-marker-doctrine-debt.md

# T313 — LOOP-SPEC T311 paragraph narrates the pre-T312 opt-in marker shape (doctrine-debt)

## Why

The T312 kimi validator's observation (c), cycle 2708: LOOP-SPEC.md's T311
paragraph (the `assert_state_on_disk` leg, ~lines 1128–1131) still narrates
the OLD opt-in marker shape — "the eval-commit probed via `git log -1
--format=%H -- EVALUATION.md` **when gated**". T312 (merged 4b49b2e, flipped
198dcd6) made the marker leg ALWAYS-ON and SELF-PROBING: the leg owns the
probe in-module, runs by default with no keyword, and the
`expected_eval_commit` keyword is demoted to an override. Doctrine that
narrates the opt-in shape tells a cold orchestrator the leg can be left off —
exactly the T306 composed-marker class the always-on leg closed (the
cycle-2704 fabricated-marker fire: 7-char prefix true, 33-hex suffix
memory-composed, the leg inert without the opt-in arg).

## Targets

- `LOOP-SPEC.md` — the T311 paragraph's marker parenthetical only (~lines
  1128–1131). No other paragraph, no other doctrine file.

## Acceptance

1. The parenthetical narrates the always-on self-probing shape: the leg
   self-probes the expected marker via the in-module owned probe (`git log -1
   --format=%H -- EVALUATION.md` in the repo toplevel) on EVERY run by
   default; the `expected_eval_commit` keyword is an OVERRIDE (asserted
   against instead of the self-probe when supplied, never the leg's
   enabler); a probe failure fails closed (AssertionError naming the probe
   and the OS error).
2. The edit names T312 as the shape-change carrier and keeps the existing
   T311 fire narrative intact (the 2515 wrap free-form state, fill a66f604).
3. Diff touches ONLY the T311 paragraph's marker parenthetical region —
   no re-flow of surrounding sentences beyond the edited clause.

estimate: ~15 changed lines (doctrine prose).

check: cargo test --release --test todo_consistency
