# T248 — LOOP-SPEC Phase 3: negations never quote the empty-delta token verbatim + post-commit sleep-ok probe (2nd fire)

check: export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared; touch src/*.rs tests/*.rs; cargo test --test loop_spec_empty_chain --test todo_consistency

estimate: ~60 lines all-in (two clause sentences ~15 + two pin legs ~30 + row)

## Concern

LOOP-SPEC Phase 3's T237 bullet makes the literal token `empty-delta
disposition` load-bearing: loopd.sh's `empty_wrap_streak` awk walk
substring-matches it in `eval:` wrap-notes subjects (loopd.sh:147) to
scale the cycle-OK backoff. The walk counts NEGATED mentions too — a
wrap-notes subject that writes "NOT an empty-delta disposition … token
does not bind" still contains the substring and reads as a disposition.

This has now fired TWICE:

1. **Cycle 118** (00c26f3): "NOT an empty-delta disposition → the token
   does not bind" — machine read streak 1..4 across cycles 119–122
   against TRUE 0..3 (reproduced live at cycle 119). Adjudicated
   consequence-free (bounded over-sleep, self-correcting; the eval-trip
   threshold is human-counted) and REJECTED as a filing candidate at
   d1791277274-2 — with a named re-file trigger: **"a second
   negation-quote fire"**.
2. **Cycle 123** (b4b935d): "T237 watch: NOT an empty-delta disposition
   (real eval ran on a worked delta) — token does not bind, streak STAYS
   0" — the machine reads streak 1 at TRUE 0 RIGHT NOW (live-probed
   `./loopd.sh sleep-ok` → `120 1` at HEAD b4b935d, cycle 124). Worse,
   the cycle-123 wrap claimed "machine and true alike, sleep-ok 60 0
   probed" — but that probe ran PRE-commit (at 11fc85a); post-commit
   the claim never held. The probe-timing gap is the second defect in
   this row.

The re-file trigger has TRIPPED (cycle 124, verified live). The fix is
the authoring surface, not the walk: awk negation-detection was weighed
brittle at cycle 119 and STAYS rejected — loopd.sh is not touched. Two
sentences in Phase 3's T237 token bullet kill both classes: (a) a
negation never quotes the token verbatim, (b) the watch probe runs
after the wrap-notes commit lands.

## Repo context

- LOOP-SPEC Phase 3, the bullet "**Empty-delta wrap subjects carry the
  token (T237).**" — the new clauses append here, at the point where
  the token's load-bearing nature is stated.
- loopd.sh:138-155 `empty_wrap_streak` — awk substring walk, dumb BY
  DESIGN. UNTOUCHED by this row (the cycle-119 adjudication:
  negation-detection in awk = brittle; the divergence is
  consequence-free for the backoff and the trip is human-counted).
- tests/loop_spec_empty_chain.rs (T247) — existing pin file for the
  empty-chain doctrine (exactly-once, whitespace-collapsed needles,
  T78/T48 idioms). The two new legs go here.
- tests/loopd_empty_backoff.rs — pins the walk itself; UNTOUCHED.
- Doctrine row → SOLO (touches LOOP-SPEC.md; no other child in flight),
  kimi REQUIRED per LOOP-SPEC §2 step 4 (the loop/spec doctrine
  itself).

## Requirements

1. LOOP-SPEC Phase 3's T237 token bullet gains the **negation-phrasing
   clause**, containing the exact needle phrase "never quote the token
   verbatim in a negation": a wrap-notes subject that must say the
   disposition did NOT happen writes around the token (drops the noun —
   e.g. "NOT a disposition — the token does not bind" — or rephrases so
   the literal two-word-plus string never appears); the walk is a dumb
   substring match by design (both false positives named: cycle-118
   00c26f3 and cycle-123 b4b935d); awk negation-detection stays
   rejected per d1791277274-2 — the authoring surface is the only
   guard.
2. The same bullet gains the **probe-timing clause**, containing the
   exact needle phrase "probe runs AFTER the wrap-notes commit lands":
   the wrap's T237-watch `./loopd.sh sleep-ok` probe reads the streak
   AFTER the token commit (or the notes explicitly name the probe as
   pre-commit) — the cycle-123 miss class, where a pre-commit `60 0`
   read was reported as the post-commit machine streak.
3. `tests/loop_spec_empty_chain.rs` gains one exactly-once
   whitespace-collapsed needle leg per clause (the two needle phrases
   above), each RED-provable by reverting its phrase from LOOP-SPEC.md.
4. loopd.sh, tests/loopd_empty_backoff.rs, META-META-SPEC.md, and the
   T247 Phase-1 clause are UNTOUCHED.

## Tests

- Each new pin leg RED-proven: revert its needle phrase from
  LOOP-SPEC.md → the leg dies; restore byte-identical → green. The impl
  names the RED-proof per leg in its commit message.
- The check line above runs green (loop_spec_empty_chain +
  todo_consistency).

## Out of scope

- Any loopd.sh change (the walk stays a dumb substring match by design;
  the divergence is adjudicated consequence-free — bounded over-sleep,
  self-correcting — and the eval-trip threshold is human-counted per
  the T247 clause).
- Retro-editing the cycle-118/cycle-123 wrap subjects (history is
  immutable; the streak self-corrects at the next real wrap).
- Mechanizing negation detection, probe timing, or the trip threshold
  anywhere in loopd.sh.
