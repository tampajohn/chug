check: cd "$(git rev-parse --show-toplevel)" && export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-validate-a && touch src/*.rs tests/*.rs && grep -q 'enumeration-by-flag' META-META-SPEC.md && grep -q 'select(.type=="tool_result" and .ok==false)' META-META-SPEC.md && cargo test --release --test todo_consistency --test eval_outcomes_carry --test eval_state_delta --test loop_spec_doctrine_prune --test internal_info_lint

# T272 — META-META-SPEC enumeration-by-flag clause (the T271 clause's teeth)

## One concern

The T271 deferred-equality scope-or-re-probe clause governs HOW a wrap
scopes its whole-stream failed-leg claims. Four fires in, the scoping
held every time — and the miss lived one layer down each time, in the
ENUMERATION the scoping framed: wraps compose their ok:false leg lists
from recall of earlier probes, not from a mechanical read of the
stream at the named read time. The class's fifth fire (the trip-80
eval's tenth digest backstop, d1791621339-3) is the purest shape:

- The cycle-446 wrap's state write carried TWO bullets enumerating the
  same stream's ok:false legs at TWO different named scopes: the
  scope-slip bullet's "3 ok:false legs ... scoped through the 07:5xZ
  read" (TRUE at its probe) and the correct-accounting streak bullet's
  "3 ok:false legs ... through the 08:03Z read" — FALSE at its named
  read: two ok:false legs had ALREADY fired inside the named window
  and were omitted (08:00:57.960Z — the state-rewrite helper's
  ring-carry AssertionError; 08:01:20.531Z — the helper's
  /tmp/state-bullets-446.txt FileNotFoundError). The digest's
  whole-stream final reads failed=5; the two omitted legs fired BEFORE
  the named 08:03Z read. The same file's two bullets disagreed because
  the 08:03Z enumeration was written from recall of the 07:5xZ probe,
  not re-read.

The carried re-arm — named at the trip-79 eval (cycle-446 EVALUATION.md
§2.2): "a FIFTH fire of any sub-shape files the enumeration-by-flag
teeth row — the state-write helper reads the events' own `ok:false`
flags mechanically ... before writing ANY whole-stream failed-leg
claim, never enumeration-by-recall" — is this row's filing authority.
The fifth fire is the second false-at-write sub-shape (fires 1–3
tail-leg timing, fire 4 the cycle-442 all-first-try zero-claim, fire
5 this one). Every miss caught one chain later by the digest backstop,
zero casualty — the row is teeth for the enumeration surface, not a
gate.

## Repo context

- Carrier: META-META-SPEC.md, inside/adjacent to the T271
  "Deferred digest-final equalities" paragraph — the enumeration
  discipline is the layer beneath that clause's scoping discipline;
  the same wrap arcs write both.
- The firing shapes to name: tail-leg timing (true at probe, false at
  final — T271's scoping covers it) and false-at-write (the named read
  already contained omitted legs — ONLY a mechanical re-read at write
  time covers it).
- The mechanical probe the clause names (the exact jq the trip-80 eval
  used to close the tenth backstop in one read):
  `jq -c 'select(.type=="tool_result" and .ok==false) | {ts, name, preview}' .chug/events.jsonl`
  (plus the rotated segment when the claim spans it); ctx-edit claims
  read `select(.type=="ctx_edit")` the same way.
- Class boundary (carried from T271, unchanged): the at-commit T257
  color (explicitly probe-stamped) is NOT this class — the probe stamp
  IS the scoping; this clause governs whole-stream enumerations in
  wrap text (Outcomes entries, eval-state open-threads/health bullets,
  wrap-notes supplements).

## Requirements

1. ONE new paragraph in META-META-SPEC.md, adjacent to the T271
   paragraph, carrying the literal token `enumeration-by-flag` and the
   literal jq needle `select(.type=="tool_result" and .ok==false)`.
2. The paragraph states the rule: any whole-stream failed-leg/ctx-edit
   enumeration written into wrap text is produced by a mechanical read
   of the stream's own flags AT the named read time — count +
   timestamps + previews quoted or paraphrased FROM that read, never
   from recall of an earlier probe; and a read taken earlier MUST be
   re-run if any tool call intervened between the read and the write
   (the false-at-write shape). The scoped claim names the read's
   timestamp (and iteration n where the surface carries one).
3. The paragraph cites the fires (the trip-76 first sighting
   d1791610441-3; the trip-77 deferred-5 vs digest-9; the cycle-438
   zero-claim vs digest-6; the cycle-442 false-at-write zero-claim;
   the cycle-446 08:03Z-scoped omission of the 08:00:57Z/08:01:20Z
   legs — digest 5 vs claimed 3) and names the zero-casualty boundary
   (the digest backstop caught every miss one chain later by design).
4. No other file changes. No TODO.md/LEDGER.md edits by the child.

## Tests

- The check line's two grep needles against META-META-SPEC.md.
- `cargo test --release --test todo_consistency --test eval_outcomes_carry --test eval_state_delta --test loop_spec_doctrine_prune --test internal_info_lint`
  (the suites a META-META-SPEC text change can break).

## Acceptance

- Both needles present exactly once each in META-META-SPEC.md; the
  five check-line suites green; no diff outside META-META-SPEC.md.

- estimate: ~20 changed lines of doctrine (one paragraph) + this spec
