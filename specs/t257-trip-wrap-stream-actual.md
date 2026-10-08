# T257 — LOOP-SPEC doctrine: a real-eval (TRIP) wrap's notes carry the trip stream's whole-stream iteration actual (wrap-notes color gap; adjudicated 2nd-omission trigger fired)

check: touch src/*.rs tests/*.rs; export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared; cargo test --release

## repo-context

The T247 empty-chain machinery paces itself off wrap-notes commit
subjects, and the cadence re-arm verification's (b) leg reads the
trip's cost from the newest trip wrap notes. That color — the trip
stream's whole-stream iteration actual (iterations + wall) — has
been ABSENT from every real-eval wrap subject for seven consecutive
trips.

- **The adjudicated trigger, fired.** The cycle-223 trip eval
  (decision d1791416867-11) named the cycle-219 wrap's (56cbbaf)
  dropped stream-actual color a single omission and hung a trigger
  on it: "a second omission files the wrap-template row." The
  second omission is cycle-223's own wrap (40717de) — it carries no
  trip stream actual — so the trigger fired at the cycle-227 trip
  evaluation.
- **Census.** The color last rides trip wraps of the c135–c152 era
  (d21f21b / a5aa255 / 44fe71b / c27bc79 each carry an iters
  actual); trips 18–24 (d1d7508 / 659ae88 / 0934c6b / 3843fe2 /
  28a4b03 / 56cbbaf / 40717de) carry NONE — seven consecutive
  omissions.
- **Consequence to date: zero.** The eval digest backstops every
  (b) read (trips 21–24 discharged their cadence re-arm checks on
  digest numbers 53/40/54/44), which is exactly how the absence
  metastasized from a lapse into a standing convention. The gap is
  handoff-surface robustness: the wrap subject is the chain's
  self-describing record, and the (b) trip-cost input is exactly
  the class of fact a cold trip cycle should read from the newest
  wrap notes without digest archaeology.

estimate: ~50 changed lines (one additive doctrine paragraph ~15
lines + one non-vacuous pin leg ~30 lines + doctrine-comment
density; the 1.5–3× band on the ~10-line text core).

## requirements

1. **Doctrine paragraph**, a new short bullet in `LOOP-SPEC.md`
   Phase 3, sibling to the "Empty-delta wrap subjects carry the
   token (T237)" bullet: a REAL-EVAL (trip) wrap's notes commit
   MUST carry the trip stream's whole-stream iteration actual
   (iterations + wall, read from the digest's own-stream entry or
   the stream itself) — the cadence re-arm check's (b) trip-cost
   input — so a cold trip cycle reads it from the newest wrap
   notes without digest archaeology. Disposition wraps are exempt
   (their streams are short and the digest backstops; the color
   binds TRIP wraps, whose stream actual the (b) leg quotes). The
   paragraph names the firing evidence: the d1791416867-11 trigger,
   the 40717de second omission, the trips 18–24 census. Quote
   discipline per T248: the paragraph MUST NOT carry the T237
   pacing token verbatim (it describes a wrap-notes color rule;
   write around the token — "the disposition token" — never the
   literal phrase).
2. **Pin leg**, one new test in `tests/loop_spec_empty_chain.rs`
   (the empty-chain doctrine's pin home — the file's existing legs
   pin the T237/T247/T248 needles): assert the new paragraph's
   load-bearing needle occurs EXACTLY ONCE in `LOOP-SPEC.md`,
   following the file's established conventions (runtime
   `current_dir()` resolution per T48, whitespace-collapsed
   matching per the T78 flat-readme idiom for any multi-word
   needle). Non-vacuous: RED on the parent text, GREEN after the
   edit; the deletion hand-check (GREEN → delete the paragraph →
   RED → restore → GREEN) is run before committing and stated in
   the commit message, per the `loop_spec_*` deletion-proof
   convention.
3. **Scope discipline:** no other files touched; no reformatting,
   no import/ordering churn. The diff touches exactly
   `LOOP-SPEC.md` and `tests/loop_spec_empty_chain.rs`.

## tests

- The new pin leg RED on the parent tree, GREEN post-edit
  (hand-check stated in the commit message).
- `cargo test --release` green (the check line), including
  `tests/loop_spec_empty_chain.rs` and the doctrine pin suites
  (`tests/loop_spec_*.rs`, `tests/todo_consistency.rs`).

## acceptance

- The check line is green.
- The paragraph and pin are present with the named evidence
  anchors; the pin's RED→GREEN hand-check is stated in the commit
  message.
- README: not user-visible (internal orchestration doctrine) —
  the merge-time README gate makes the final call.
