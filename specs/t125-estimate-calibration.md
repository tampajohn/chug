# T125 — META-META-SPEC estimate calibration: test/doc density ~2x, ~400 should-split band

check: cargo test --test loop_spec_recovery

## Repo context

META-META-SPEC's spec quality bar carries the T110 filing-time
ceiling: every spec states `estimate: ~N changed lines`, and a row
estimated above ~500 lines MUST split at filing time. The ceiling is
only as good as the estimate, and the era's landed actuals show
estimates undershooting SYSTEMATICALLY on feature rows, where test +
doc density multiplies the src diff (cycle-61/62 Outcomes records):

- T113 estimated ~455, landed +583 (1.3x)
- T115 estimated ~130, landed +398 (3.0x)
- T116 estimated ~30, landed +116 (3.9x)
- T117 estimated ~280, landed +603 (2.2x) — and its glm impl child
  DIED mid-impl at 80/80 iterations (uncommitted, t1 of 6 done),
  costing a T63 resume (events-t117-impl-20260928-132236.jsonl).
  The TRUE size was over the ceiling the estimate claimed to be under:
  an accurate filing-time estimate would have split the row.

The failure mode is estimate ERROR, not ceiling value — a 500 ceiling
cannot catch a row filed at "~280" that lands at 603. The fix is a
calibration rule at the same filing-time surface, one paragraph below
the ceiling sentence it qualifies (META-META-SPEC's "Extend TODO.md"
spec-quality-bar paragraph, the T110 text around `estimate: ~N
changed lines`).

Doctrine item: touches META-META-SPEC.md + its pin carrier
tests/loop_spec_recovery.rs ONLY. Runs ALONE (no other child in
flight); kimi REQUIRED (loop/spec doctrine).

estimate: ~45 changed lines (≈15 doctrine + ≈30 pin legs)

## Requirements

1. META-META-SPEC's estimate paragraph gains the calibration rule,
   in one or two sentences placed immediately after the T110 ceiling
   sentence (before any T104/T109 exemption clause, or woven in where
   it reads naturally — the exact placement is the child's, the
   exactly-once content is not):
   - Estimates count ALL changed lines — src + tests + docs — and
     feature-row test/doc density has empirically run ~1.5–3x the src
     diff (the four named data points T113/T115/T116/T117 with their
     estimate→actual pairs, and T117's mid-impl 80/80 death named as
     the cost of undershoot).
   - A novel-logic row whose all-in estimate exceeds ~400 SHOULD be
     split at filing time even though the hard ceiling stays ~500 —
     the band absorbs the observed undershoot (the mechanical
     byte-identical move-row exemption is unchanged and applies to
     both numbers).
   - Each evaluation re-checks landed actuals against filing
     estimates (the Outcomes records) and re-calibrates in the eval
     text — it does not edit the threshold number in passing.
2. `tests/loop_spec_recovery.rs` gains pin leg(s) (next letter in
   the existing leg series beside the T114/T120 legs) asserting the
   calibration rule's load-bearing tokens appear EXACTLY ONCE each in
   META-META-SPEC.md (the existing `meta_meta_spec()` loader +
   exactly-once helper pattern): the `~400` should-split threshold,
   the `T117` evidence token, and the density claim (a stable needle
   like `1.5`–`3x` as written). RED-prove each new leg: it fails
   against the pre-edit META-META-SPEC.md.
3. No other META-META-SPEC content changes: the ~500 hard ceiling,
   the T104/T109 exemption, and the check:-line rules byte-identical.
   No LOOP-SPEC/META-SPEC/README edits.

## Tests

The new pin leg(s) are the test change; the full
`cargo test --test loop_spec_recovery` suite is the regression set
(the T114 legs i-j, T120 legs k-l, and the step-2 window pins must
stay green — they share the file).

## Acceptance

- `cargo test --test loop_spec_recovery` green; each new leg
  RED-proven against the old doctrine text.
- The inserted sentences sit inside the same spec-quality-bar
  paragraph region as the estimate ceiling (not a new section —
  evaluators read the bar top-to-bottom at filing time).
- `cargo clippy --all-targets -- -D warnings` clean.

## Out of scope

Changing the ~500 ceiling value; LOOP-SPEC edits; any attempt to
auto-enforce estimates mechanically; re-estimating the open queue
(there is none open at filing — T124/T126/T127 estimates apply the
rule at their own filings, done this same eval).
