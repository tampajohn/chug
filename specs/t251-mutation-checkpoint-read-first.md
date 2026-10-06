# T251 — LOOP-SPEC mutation-checkpoint ordering + read-first recovery rule (the ctx-edit casualty clauses)

check: cd "$(git rev-parse --show-toplevel)" && export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-validate-a && touch src/*.rs tests/*.rs && cargo test --release --test loop_spec_recovery

## Repo context

The cycle-128 glm orchestrator (events-20261006-182121.jsonl)
fast-forwarded main `bd66e31`→`c982efa` (the T249 landing), wrote three
bookkeeping files dirty (TODO flip, EVALUATION.md Outcomes,
DEPENDENCIES.md), and then — under the T230 occupancy nudge at ~102–108k
live tokens — had an ACCEPTED LIVE_CTX edit collapse its context
108,645 → 1,004 tokens in ONE edit (iter 103, 17:55:52Z; turns 0–175
deleted wholesale). The collapse amputated the turns covering the merge
and the dirty files' provenance. The model spent ~45 iterations (~22
min, iters 103–148) rediscovering its own state: it read the reflog
(FF proven), read the three dirty files, judged them load-bearing, and
landed them VERBATIM as `cfef97f` (recovery narrative:
.chug/loopd/cycle-20261006-152144.log:354–366; goal summary: "a lost
segment — ctx-edit casualty ~13:36"). Zero work was lost — this time.

Fleet ctx-edit census (cycle-130 eval, I1): 10 accepted LIVE_CTX edits
since T192, 8 of them >75% single-edit collapses (4 of them 89–99%).
Deep collapses are the feature's NORMAL operation and previously
harmless — the hazard is collapse TIMING (mid-mutation, uncommitted
bookkeeping), not depth. A driver-side ratio/shrink-floor gate was
WEIGHED AND REJECTED at filing (it fights the primary use pattern; the
T192 pinned/pair/shrink guards already reject malformed edits — 15
rejections fleet-wide, 9 in this stream alone).

The near-miss path: a recovering orchestrator that reflexively
`git checkout --` / `git clean` / `git reset --hard` on "foreign" dirty
files destroys the uncommitted bookkeeping. This model read first;
nothing in doctrine REQUIRED it to. The cycle-61 kill rule
(verify-then-kill is SEQUENTIAL — read first, kill after) is the
existing sibling for suspected-corrupt PAYLOADS; this row is the
self-STATE sibling for suspected-foreign REPO state.

estimate: ~70 lines (two LOOP-SPEC clauses + needle-leg pins on the
existing tests/loop_spec_recovery.rs — the doctrine+needle-leg kind,
calibrated 1.3–1.5x all-in per the cycle-126/130 eval guidance).

Doctrine row: runs SOLO (no other child in flight), kimi validation
REQUIRED (LOOP-SPEC doctrine is on the step-4 REQUIRED list).

## Requirements

1. **Mutation-checkpoint ordering clause** in LOOP-SPEC §2 step 5 (the
   harvest/merge/bookkeeping step): after any IRREVERSIBLE mutation
   (a merge or fast-forward into main, a push), the row-flip/Outcomes
   bookkeeping edits MUST be written to disk BEFORE the mutation where
   the content is knowable pre-mutation, and committed IMMEDIATELY
   after — and NO further child dispatch may happen between an
   irreversible mutation and its bookkeeping commit. The clause names
   the ctx-edit casualty as its evidence (the accepted 99% collapse
   fired between the FF and the bookkeeping commit; on-disk dirty
   files were the only reason recovery was verbatim).
2. **Read-first recovery rule** in LOOP-SPEC's Hard rules, beside the
   cycle-61 kill rule's spirit: an orchestrator that encounters repo
   state it does not remember producing (dirty files, a moved HEAD,
   commits on main it did not watch land) MUST treat that state as
   load-bearing: read the files, read the reflog, reconstruct the
   story BEFORE any mutating command — and NEVER run
   `git checkout --`, `git clean`, `git reset --hard`, or
   `git worktree remove` against unremembered state until
   reconstruction proves it disposable. The rule names the ctx-edit
   casualty (context amputation makes the orchestrator a stranger to
   its own work) and the cycle-61 kill rule (the same
   verify-then-act discipline, payloads vs repo state).
3. **Pins**: needle legs in tests/loop_spec_recovery.rs (the existing
   recovery-doctrine pin carrier — 34 tests today) asserting BOTH
   clauses' load-bearing phrases appear in LOOP-SPEC.md
   (mutation-checkpoint ordering; read-first recovery against
   unremembered state), per that file's existing needle-leg pattern.
   Each leg RED-proven: deleting the clause from LOOP-SPEC.md fails
   the leg (prove by execution, restore byte-identical).
4. **Scope discipline**: LOOP-SPEC.md and tests/loop_spec_recovery.rs
   ONLY. No driver code changes (the T192 shrink/pair/pinned guards
   are untouched — collapse depth stays the model's choice);
   loopd.sh untouched; META-SPEC.md/META-META-SPEC.md untouched;
   no raw-token discipline issues (this row is unrelated to the
   empty-delta token).

## Acceptance

- Both clauses present in LOOP-SPEC.md at the named locations.
- `cargo test --release --test loop_spec_recovery` green, including
  the new needle legs; each new leg RED-proven by execution with
  byte-identical restore.
- Full gates green in the worktree (build + clippy --all-targets
  -- -D warnings + the T82 release runner).
- `git diff --name-only` shows exactly the two named files.
