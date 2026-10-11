# T292 — the T257-color exemption narrowed to stamped mechanical whole-stream reads

## Repo context

Doctrine row (META-META-SPEC.md only), pri 3, SOLO (doctrine items never
overlap), kimi validation REQUIRED (the loop/spec doctrine itself is on
LOOP-SPEC step 4's REQUIRED list).

The T271 (deferred-equality) and T272 (enumeration-by-flag) paragraphs each
close with a class boundary exempting the at-commit T257 color from the
clause — "the color's probe stamp IS the scoping" (META-META-SPEC.md, the
T271 paragraph's boundary sentence and the T272 paragraph's "The class
boundary carries from T271 unchanged: …" sentence). The exemption's
rationale scopes TAIL legs — events firing between the color's named probe
stamp and the whole-stream final, which the digest backstop catches one
chain later by design. It never reaches PRE-STAMP legs: legs already
present in the stream at the named stamp that the color's enumeration
omits. That gap has now fired TWICE:

1. First sighting (recorded d1791709463-4, the 32nd digest backstop): the
   cycle-528 wrap notes' probe-stamped "ok:false legs ZERO" was false at
   its own stamp — the digest's ONE failed leg (07:43:47Z
   `AssertionError: cell count: 7`) sat INSIDE the stamp window.
2. Second sighting (recorded d1791717921-2, the 36th digest backstop):
   the cycle-541 wrap notes' T257 color (commit 13e2e3e), probe-stamped
   10:49:49Z (stream iteration 61 = 10:49:49.629Z), claimed "ok:false
   legs ZERO by the stream's own flag + TWO interpreter-caught pre-write
   AssertionError legs" while FOUR ok:false legs were present at the
   stamp: the two named compaction-arc legs (10:43:51.372Z,
   10:44:22.809Z) PLUS two unnamed — 10:36:23.188Z the write_file
   path-escapes-cwd refusal on /tmp/eval-body-541.md (the designed
   cwd-confinement guard, re-issued via bash; the same unclassed-minor
   class the cycle-520 color named explicitly as its leg (3)) and
   10:49:30.345Z the /tmp/prep541.py AssertionError (the T270/T273
   remembered-constant 7th fire, counted post-notes d1791715907-6).
   ctx-edit 1==1 and trim 0==0 closed EXACT; the claim's failure is the
   ok:false enumeration, false at its own named stamp.

Both sightings share one mechanism: the color's enumeration was composed
around the salient event (the eval's own counted fire; the compaction
arc) from recall or a narrowed read — NOT a fresh mechanical whole-stream
flag read at composition time, the exact shape T272's re-run mandate
("a read taken earlier MUST be re-run before the write if any tool call
intervened") already bans for every other wrap-text surface. The
boundary sentence's blanket exemption lets the T257 color — the highest-
visibility whole-stream enumeration the loop writes — skate the same
mandate. Zero casualty both times (the digest backstop caught each one
chain later); the drip is the honesty surface drifting, not injury.

estimate: ~25 changed lines (two boundary sentences amended in place +
the two-fire citation; META-META-SPEC.md only).

**Pin surface (T285 probe, run at filing):** `grep -rn 'matches("'
tests/*.rs` over the boundary text returns ZERO needles; a windowed grep
for `probe stamp IS the scoping` / `class boundary` across tests/*.rs
returns ZERO — no literal-needle census or const/positional pin quotes
either boundary sentence. The amendment therefore perturbs no pin; the
doctrine-carrier suites below gate the edit as doctrine text. The
META-META-SPEC-reading test files (mechanical `grep -l META-META
tests/*.rs` at filing): eval_outcomes_carry, eval_state_delta,
loop_spec_cheap_exit, loop_spec_check_wall, loop_spec_doctrine_prune,
loop_spec_eval_read_path_pointer, loop_spec_recovery,
loop_spec_validation_lane, todo_consistency.

## Requirements

1. Amend the T271 paragraph's class-boundary sentence (the one ending
   "the color's probe stamp IS the scoping.") so the exemption carries
   its precondition: the at-commit T257 color is outside the class ONLY
   when its enumeration is itself a mechanical whole-stream read of the
   stream's own flags taken at the named stamp — every ok:false /
   ctx-edit leg PRESENT at the stamp named (classified or
   unclassed-minor); the stamp scopes only TAIL legs (events after the
   stamp), and a color whose enumeration omits a leg present at its
   named stamp is INSIDE the class (the false-at-write shape).
2. Amend the T272 paragraph's "The class boundary carries from T271
   unchanged:" sentence the same way (the two sentences must not drift —
   same precondition, same consequence, each readable standalone).
3. Cite both fires in the amended text by their backstop records
   (d1791709463-4 the first sighting, d1791717921-2 the second) — the
   amendment is evidence-linked per the doctrine's own convention.
4. No other repo file changes; every pre-existing line of
   META-META-SPEC.md outside the two amended sentences stays
   byte-identical; no tokens removed that any doctrine-carrier suite
   pins (the suites in repo-context gate this).

## Tests

- Needle greps: the amended T271 boundary sentence greps exactly once
  with the precondition's key phrase (e.g. `PRESENT at the stamp` or the
  author-chosen equivalent), the T272 one likewise; `probe stamp IS the
  scoping` may remain (the scoping claim survives, narrowed) but the
  exemption's blanket form must not.
- Byte-carry: `git diff` on META-META-SPEC.md touches only the two
  boundary sentences' lines.
- The doctrine-carrier suites named in repo-context all pass.

## Acceptance

Both boundary sentences carry the precondition + consequence + the
two-fire citation; the byte-carry holds; the doctrine-carrier suites
green; RED-proof: strip the amendment and the needle greps fail.

check: cd "$(git rev-parse --show-toplevel)" && export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-validate-a && touch src/*.rs tests/*.rs; set -o pipefail; grep -c 'PRESENT at the stamp' META-META-SPEC.md && cargo test --release --test eval_outcomes_carry --test eval_state_delta --test loop_spec_cheap_exit --test loop_spec_check_wall --test loop_spec_doctrine_prune --test loop_spec_eval_read_path_pointer --test loop_spec_recovery --test loop_spec_validation_lane --test todo_consistency 2>&1 | tail -20
