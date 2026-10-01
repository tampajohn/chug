# T187 — META-SPEC bounded-gates: alarm value must sit BELOW the bash cap (600 → the rule + 280/110 examples)

check: export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared && cargo test --test nextest_gate_runner --test todo_consistency && grep -q "alarm 280" META-SPEC.md && ! grep -q "alarm 600" META-SPEC.md

## Repo context

T178 (cycle 82) moved LOOP-SPEC's gate templates to
`perl -e 'alarm 280; exec @ARGV' …` with the rule stated explicitly:
with loopd exporting `CHUG_BASH_TIMEOUT=300` fleet-wide, the inner
alarm bound must sit BELOW the bash cap so the alarm fires first —
"the cargo-culted 600s alarm could never fire under the old 120s
default (EVALUATION.md §2.4), so the templates carry `alarm 280`".

META-SPEC.md's bounded-gates hard rule still illustrates the dead
form: `perl -e 'alarm 600; exec @ARGV' cargo nextest run --release`
on macOS and `timeout 600 cargo nextest run --release` on Linux.
Under any bash cap below 600s (the 120s default; the loop fleet's
300s) a 600s inner alarm can never fire — the cap's process-group
SIGKILL lands first, so the illustration teaches a guard that does
not guard. Carried twice already (cycle-82 wrap handoff: "fold into
the next META-SPEC touch or leave pinned as-is"; cycle-83 eval
§2.6(g): "fold into the next META-SPEC touch, not a standalone row")
— no META-SPEC touch has been scheduled, so the drift stands; this
row is the touch. The class risk: an impl or validator child
cargo-cults `alarm 600` into a new template where it silently never
fires.

This row is DOCTRINE-ONLY (META-SPEC.md only) → SOLO: no other child
in flight while it runs.

estimate: ~25 changed lines (a few sentences + the check-line greps).

## Requirements

1. META-SPEC.md's bounded-gates paragraph replaces the illustrative
   `alarm 600` / `timeout 600` with `alarm 280` / `timeout 280` AND
   gains the rule in one sentence: the inner alarm value must sit
   BELOW the driver's bash cap so the inner bound fires first (280
   under the loop fleet's `CHUG_BASH_TIMEOUT=300`; ≤ ~110 under the
   120s default) — a 600s alarm under either cap is dead weight that
   lets the cap's SIGKILL land first.
2. No other META-SPEC text changes; LOOP-SPEC.md untouched (it is
   already correct).
3. Any test pinning the old `alarm 600` text is amended in-commit
   (sweep: `grep -rn "alarm 600" tests/ META-SPEC.md`); if a pin
   currently ASSERTS the 600 form, its amendment is justified in the
   commit message.

## Tests

- The spec's check line greps: `alarm 280` present, `alarm 600`
  absent file-wide in META-SPEC.md.
- `cargo test --test nextest_gate_runner --test todo_consistency`
  green (the gate-runner doctrine carriers).

## Acceptance

- Check line green; `cargo clippy --all-targets -- -D warnings` green
  (trivially — no code).
- The diff is confined to META-SPEC.md (+ any pin amendment named in
  the commit message).

## Out of scope

- Re-numbering or restructuring META-SPEC's hard rules.
- Changing the T178 LOOP-SPEC templates (already carry 280).
