# T302 — name the `eval:`-prefixed wrap-corridor subject convention in LOOP-SPEC (the wrap-fill subject-shape census's 2nd-fire doctrine row)

## Repo context

loopd's cheap-exit gate (T258) reads the delta window's commits to decide
whether the delta is bookkeeping-only: the subject walk anchors on `^eval:` —
commits whose subjects start with `eval:` count as bookkeeping; anything else
reads as work. The wrap corridor writes four commits per eval cycle — the
eval body, the wrap prep, the wrap notes, the wrap fill — and every one MUST
carry the `eval:` prefix or the corridor reads as NON-bookkeeping and the
gate launches a real evaluation over an otherwise cheap-exitable predicate
(the fail-safe direction, a bounded burn: one full eval cycle, never a
silent skip).

The convention was unwritten until now. First fire: the cycle-1737 wrap fill
fd5d33f's subject `fill: cycle-1737 — …` — the ONLY `^fill:` commit in the
wrap-fill census (345 `eval:`-prefixed wrap-fill subjects vs 1, the git-walk
re-probed at the cycle-1738 eval: `git log --format=%s | grep -cE '^fill:'`
count-1 == fd5d33f alone vs `grep -cE '^eval:.*wrap fill'` count-345) —
flipped the leg to book=no and forced the cycle-1738 launch (the T258 else
arm, kimi, a bounded burn). The cycle-1738 eval REJECTED the first sighting
below the filing bar and ARMED the watch as a state health bullet (the
wrap-fill subject-shape census 1; "the 2nd fire files the doctrine row
naming the `eval:`-prefixed fill-subject convention in LOOP-SPEC"). Second
fire: the cycle-1738 wrap's OWN fill first message carried
`Fill - cycle 1738 wrap tail:` (no `eval:` prefix), caught pre-push at the
commit's own read-back and amended message-only to f6bc6f6 (unpushed HEAD,
EMPTY commit, zero consequence — the bad subject never reached origin, so
loopd's delta leg never reads it). The armed letter's trigger fired: this
row. Doctrine is the fix — the mechanical consumer (loopd's `^eval:` walk)
made legible to the composer (the wrap's fill-message template).

estimate: ~30 changed lines all-in (LOOP-SPEC.md +~10 — one Phase-3 bullet;
tests/loop_spec_empty_chain.rs +~20 — one needle const + one test fn +
comment; this spec).

check: cargo test --test loop_spec_empty_chain --test loop_spec_cheap_exit --test todo_consistency --test internal_info_lint

## Pin-surface declaration (T285 — probed before filing)

- `tests/loop_spec_empty_chain.rs` pins the T237/T248 wrap-notes token area:
  QUOTE_NEEDLE `verbatim ONLY in a true disposition wrap subject`,
  PROBE_NEEDLE `probe runs AFTER the wrap-notes commit lands`, and the three
  clause needles (DISPOSITION `zero children launched, zero items landed`,
  VALVE `next (4th) empty cycle RUNS the real evaluation`, HUMAN
  `HUMAN-counted from the wrap-notes chain`), all exactly-once over
  `flat(LOOP-SPEC.md)`. The mandated bullet is INSERT-ONLY and reproduces
  none of these needles verbatim (probed at filing: no clause of the bullet
  text matches any needle), so every existing count stays 1. The edit ADDS
  one needle leg to that file — the bullet's lead token
  `Wrap-corridor commit subjects stay` (count 1) — guarding the new
  doctrine from silent rot.
- The RAW T237 count pin (`count_eq(&read("LOOP-SPEC.md"),
  "empty-delta disposition", 1, …)` in tests/loop_spec_cheap_exit.rs): the
  mandated bullet writes around the contiguous token — it names "the
  T237-token rules" only, never the literal (quote discipline T248).
- Collision probes run at filing: no test file pins `"Wrap-corridor"` (0
  hits) or `"eval:-prefixed"` (0 hits) as a LOOP-SPEC needle; the remaining
  count_eq needles in tests/loop_spec_cheap_exit.rs are loopd.sh
  fixture/script-text asserts, not LOOP-SPEC prose pins.
- Positional/structural pins are unaffected: the insertion is a NEW bullet
  between two existing Phase-3 bullets — no heading edits
  (tests/loop_spec_cold_gates.rs's step-5/step-6 heading walk), no
  numbered-list edits (tests/loop_spec_validation_lane.rs's `\n7. **`
  count), no edits to any region tests/loop_spec_recovery.rs names.

## Requirements

1. In `LOOP-SPEC.md`'s `## Phase 3 — Wrap` section, insert ONE new bullet
   IMMEDIATELY AFTER the `- **Empty-delta wrap subjects carry the token
   (T237).**` bullet block and BEFORE the `- **A real-eval (trip) wrap's
   notes carry the trip stream's whole-stream iteration actual (T257).**`
   bullet, carrying every load-bearing token of:

   ```
   - **Wrap-corridor commit subjects stay `eval:`-prefixed (T302).** Every
     commit an eval/wrap corridor writes — the eval body, the wrap prep, the
     wrap notes, the wrap fill — carries an `eval:`-prefixed subject, because
     loopd's delta_bookkeeping_only subject walk reads the corridor as
     bookkeeping by the `^eval:` anchor: a wrap commit whose subject starts
     with anything else flips the leg to book=no and forces a launched
     evaluation over an otherwise cheap-exitable predicate (the fail-safe
     direction — a bounded burn, never a silent skip over real work). Both
     fires are the wrap fill's subject: fd5d33f's `fill:`-prefixed subject
     (the ONLY `^fill:` in the wrap-fill census) forcing the cycle-1738
     launch, and the cycle-1738 wrap's own first fill message caught
     pre-push and amended message-only (zero consequence — the bad subject
     never reached origin). The T237-token rules above govern the notes
     subject's token; this rule governs the prefix every wrap commit
     shares.
   ```

   Minor wording variance is acceptable; the bullet MUST keep: the lead
   token `Wrap-corridor commit subjects stay`, the `eval:`-prefixed
   requirement over all four corridor commits (eval body / prep / notes /
   fill), the `^eval:` anchor mechanism, the two fires cited (fd5d33f and
   the cycle-1738 self-caught amend), and the closing scoping sentence
   (T237-token rules govern the notes token, this rule the prefix). The
   bullet MUST NOT contain the literal contiguous token
   `empty-delta disposition` (the RAW count pin in
   tests/loop_spec_cheap_exit.rs stays at 1) nor either
   tests/loop_spec_empty_chain.rs exactly-once needle
   (`verbatim ONLY in a true disposition wrap subject`,
   `probe runs AFTER the wrap-notes commit lands`).
2. In `tests/loop_spec_empty_chain.rs`, add ONE needle leg: a const
   `WRAP_PREFIX_NEEDLE: &str = "Wrap-corridor commit subjects stay";` and a
   test fn asserting it occurs EXACTLY once in `flat(LOOP-SPEC.md)`, with
   the T48 needle self-check idiom (a mangled-needle assert guarding the
   needle itself, matching the file's existing legs' shape).
3. No other repo file changes. Never rewrap or move existing pinned text.

## Tests

- The new needle leg is RED-provable: deleting the bullet drops the count
  to 0 (the deletion hand-check — GREEN → delete bullet → RED → restore →
  GREEN — stated in the commit message per the loop_spec_* deletion-proof
  convention).
- `cargo test --test loop_spec_empty_chain --test loop_spec_cheap_exit
  --test todo_consistency --test internal_info_lint` GREEN.

## Acceptance

- LOOP-SPEC carries the `eval:`-prefixed wrap-corridor subject convention
  with both fires cited; the new needle leg holds it at exactly-once; the
  RAW T237 count pin and every pre-existing exactly-once needle stay at
  their counts; the suite legs above green in the impl child's worktree.
