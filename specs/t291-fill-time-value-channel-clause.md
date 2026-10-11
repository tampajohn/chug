# T291 — the fill-time value-channel clause joins the T282 paragraph (the name-from-memory class's 3rd fire executes the armed letter)

**Pri:** 4 (doctrine — ONE sentence + a read-only sweep; the class's armed
letter executes on its 3rd fire)

**SOLO doctrine surface** (ONE sentence in META-META-SPEC.md's T282
probed-figures paragraph — the loop/spec doctrine itself): never overlaps,
kimi REQUIRED validation.

## Repo context

The name-from-memory counted census stood at 2 with an armed letter: a 3rd
fire files the sweep row. The cycle-528 wrap's fill leg produced the 3rd
fire (record d1791706040-13): the Outcomes fill script hardcoded the notes
commit's cI as `2026-10-11T04:09:56-04:00` — a composed-from-assumption
figure — while the probe (`git show -s --format=%cI 93b87a0`) rode the SAME
command's stdout reading `04:06:42-04:00`. Caught on the result read-back
PRE-COMMIT (the fill uncommitted), corrected to the probed value one leg
later, zero casualty. The class's mechanism HERE is the fill-time value
channel: a probed value literal-composed into a script's text sits
unmarked, indistinguishable from authored prose — the T282 paragraph's
"probed at write time" clause binds the FIGURE, and this row lands the
channel half: HOW probed values enter the fill script.

**Filing-time pin-surface probe (T285 dogfood, run before the sentence was
finalized):** the T282 paragraph (META-META-SPEC.md:348, opens
"**Outcomes-entry figures (T282).**") is PINNED INDIRECTLY — the window-scoped
`Outcomes` census in tests/eval_outcomes_carry.rs counts the capitalized
token inside the Write-EVALUATION window at exactly THREE (two in the
required carry-forward sentence + the T282 bold lead's mandated occurrence;
the pin comment: "its body text stays lowercase per the T281 census dodge")
→ the sentence MUST carry ZERO capitalized `Outcomes` tokens (lowercase
"outcomes" only). Whole-file exactly-once needles the sentence must NOT
carry: `carry forward` (count 1), `verbatim` (count 1), `probed at write`
(count 1 — UNPINNED but declared), the verify-clause needle. The anchor
"the backstops caught them." greps exactly once. The `verbatim splice`
whole-file count stands 1 and the new sentence does not carry the needle.

## Requirements

1. Append ONE sentence at the end of the T282 paragraph (immediately after
   "…the backstops caught them."), carrying: (a) the 3rd fire's record id
   d1791706040-13 and its mechanism (the fill script's hardcoded cI
   04:09:56 vs the probed 04:06:42 riding the same command's stdout,
   caught pre-commit on the result read-back, zero casualty); (b) the
   channel rule — a fill script's substituted values are PROBED INTO SHELL
   VARIABLES FIRST (the probe's stdout captured into a var) and the script
   reads them from argv or the environment; a probed value is never
   literal-composed into the script text, where an invented or stale
   figure sits indistinguishable from authored prose; (c) the sentence
   carries ZERO capitalized `Outcomes` tokens (the T281 census dodge) and
   none of the declared exactly-once needles.
2. The read-only sweep: grep the wrap/fill authored surfaces
   (LOOP-SPEC.md, META-META-SPEC.md, scripts/*.py, scripts/*.sh) for
   fill-shaped scripts carrying literal-composed timestamps/hashes (the
   candidate shape: a 20…T… timestamp or a 7-hex ref literal inside a
   heredoc in doctrine text); report the findings in the commit message
   (ZERO findings expected — the doctrine names probed channels already;
   any finding is named, not fixed, in this row).
3. No other file touched; the paragraph's existing text byte-identical
   outside the appended sentence; `verbatim splice` count unmoved at 1;
   zero new pin collisions (re-run the filing-time probe pre-commit and
   compare byte-for-byte).
4. No new pin added (the paragraph's surface stays as probed — the T285
   declaration is this spec's).

## Tests

- The doctrine battery in the check line (the nine META-META-SPEC-reading
  suites, enumerated mechanically via `grep -l META-META-SPEC tests/*.rs`)
  green — the sentence must not trip any existing pin (the `Outcomes`
  window census stays THREE).
- RED-proof (orchestrator, at review): deleting the appended sentence
  returns the file byte-identical to pre-change (git diff empty); the
  sentence's record-id needle `d1791706040-13` greps exactly once in
  META-META-SPEC.md post-landing; the windowed `Outcomes` census re-probed
  THREE post-landing.

## Acceptance

- ONE sentence appended, paragraph otherwise byte-identical; `git diff
  --stat` shows META-META-SPEC.md only, a few `+` lines, zero `-` lines.
- The filing-time pin probe re-run byte-identical (window census THREE, 3
  comment-only T282-era mentions unmoved, exactly-once needles unmoved).
- The sweep's findings named in the commit message.
- Full check line green in the impl worktree.

estimate: ~15 changed lines (ONE doctrine sentence + the read-only sweep)
— far under the T209 ~500 ceiling; SOLO per the doctrine-surface rule.

check: cd "$(git rev-parse --show-toplevel)" && export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared && touch src/*.rs tests/*.rs; set -o pipefail; cargo test --release --test eval_outcomes_carry --test eval_state_delta --test loop_spec_cheap_exit --test loop_spec_check_wall --test loop_spec_doctrine_prune --test loop_spec_eval_read_path_pointer --test loop_spec_recovery --test loop_spec_validation_lane --test todo_consistency 2>&1 | tail -20
