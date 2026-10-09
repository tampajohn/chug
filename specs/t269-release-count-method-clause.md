# T269 — LOOP-SPEC release-count method clause (mechanical distinct flip-id count in `tag..HEAD`)

## One concern

LOOP-SPEC Phase 3's release trigger (T100) cuts a tag when "≥3 items
landed since the newest `v*` tag" — but the doctrine never says HOW the
count is computed, and the practiced count (in-cycle memory plus
merge-commit subject enumeration against the canonical
`Merge branch 'loop-t<N>'` shape) undercounts when a landing's commit
shape varies. Live fire, verified at filing (cycle-393 eval §2.2):
`v0.17.11..HEAD` holds FOUR flipped rows — T265/T266/T267/T268 (flip
commits 09e12bb / 961911c / f565198 / a49d27b, every merge NOT an
ancestor of the tag, the tag an ancestor of HEAD per T245) — while the
wrap record said "1 item" at cycle 309 (true 2: T265's lowercase
`merge: t265 …` subject 5020f30 fell out one wrap after landing), "1
item" at cycle 341 (true 3 — **the trigger tripped here; v0.17.12 was
due and suppressed**), "2 items" at cycle 345 (true 4), and quoted "2
items" verbatim across the thirteen zero-row wraps since. T266's
landing is a custom-subject merge commit (5979b0f, parents
a4fb519+f77fd9f — topologically a merge, subject starts `T266: …`, no
`Merge branch` token), so a `--grep='Merge branch'` enumeration returns
exactly 2 — the frozen recorded count. The fix names the mechanical
count authority in the doctrine and pins it: the T265 lesson (that
clause fixed eval-text corpus counts recorded from memory) applied one
surface over.

## Repo context

- Carrier: LOOP-SPEC.md Phase 3, the "**Release trigger (T100): the
  loop cuts tags.**" bullet — the sentence "If ≥3 items landed since
  the newest `v*` tag OR any FEATURES.md check-off landed". The T245
  ancestor-sanity sentence governs WHICH tag the count anchors on; the
  count METHOD itself is currently unnamed (verified at filing: no
  `sort -u`, no `distinct`, no count command in the bullet).
- The mechanical count, verified at filing:
  `git log v0.17.11..HEAD --format='%s' | grep -E '^todo:' | grep -oE 'T[0-9]+' | sort -u | wc -l`
  → 4. Row-flip subjects are the authority: LOOP-SPEC Phase 2 step 5
  mandates a `todo:` flip commit per landed row, and a bundled flip
  names every row in one commit — so the count is DISTINCT ROW-IDS, not
  commits (a bundle counts its rows; merge-subject greps and in-cycle
  memory are NOT the authority — the two drift shapes named above).
- No test pins the release-count method in LOOP-SPEC.md prose today
  (nothing to break; the T268 pointer pin
  tests/loop_spec_eval_read_path_pointer.rs is the shape precedent).
- estimate: ~10 changed lines of doctrine + a new pin file (~150 lines)
  → ~160 all-in (the T267 lesson: the pin file carries the bulk — ~6
  named → 159 all-in there).
- Doctrine row: SOLO (no overlap with anything), kimi validation
  REQUIRED (LOOP-SPEC.md is on the core list).

## Requirements

1. LOOP-SPEC.md Phase 3's release-trigger bullet gains ONE clause
   naming the mechanical count: the items-since-tag count is the count
   of DISTINCT flipped TODO row-ids in the range — the flip-commit
   subjects (`^todo:`) name every flipped row including bundled rows —
   with the exact command shape
   `git log <tag>..HEAD --format='%s' | grep -E '^todo:' | grep -oE 'T[0-9]+' | sort -u | wc -l`
   (or an equivalent distinct-id method), recomputed mechanically at
   EVERY wrap (never quoted forward from a previous wrap's notes), and
   the explicit ban: merge-subject enumeration (`Merge branch` greps)
   and in-cycle memory are never the count authority (the T265/T266
   drift shapes named as the evidence).
2. A NEW pin test file `tests/loop_spec_release_count_method.rs`
   asserts LOOP-SPEC.md carries the method's load-bearing tokens:
   `^todo:`, `sort -u`, a case-insensitive `distinct`, and the ban
   phrase matching `never the count authority` — the clause cannot
   silently drift out in a future doctrine edit.
3. No other LOOP-SPEC.md text changes; the release-trigger bullet's
   semantics are untouched (one clause inserted; the T245 ancestor
   sanity, the HARD RULES, and the FEATURES.md check-off half stand).

## Tests

- The new pin (req 2) GREEN against the edited LOOP-SPEC.md.
- RED-proof (orchestrator, pre-merge): revert the LOOP-SPEC.md edit in
  the worktree → the pin FAILS; restore → GREEN.
- The affected LOOP-SPEC pin family still green:
  `cargo test --release --test loop_spec_release_count_method --test loop_spec_eval_read_path_pointer --test loop_spec_loopd_log_pointer --test loop_spec_cheap_exit --test loop_spec_empty_chain --test loop_spec_doctrine_prune`
  (the doctrine carrier's existing pins must not break; the two pointer
  pins run beside the new one as the family's shape proof).

## Acceptance

- `grep -c 'sort -u' LOOP-SPEC.md` ≥ 1 AND `grep -c '\^todo:' LOOP-SPEC.md` ≥ 1
  AND `grep -ci 'distinct' LOOP-SPEC.md` ≥ 1 AND
  `grep -c 'never the count authority' LOOP-SPEC.md` ≥ 1.
- The pin test file exists and passes; the RED-proof was run and named
  in the merge/flip notes.
- Diff touches ONLY LOOP-SPEC.md + the one new test file (docs+pin only
  — the T80 docs-only classification does NOT shrink the gates: the edit
  touches a pinned doctrine carrier, so the editor ALSO runs the
  affected pin files' tests per LOOP-SPEC step 3's docs-only clause; the
  full suite is not required — zero src/ changes).

check: grep -q 'sort -u' LOOP-SPEC.md && grep -q 'never the count authority' LOOP-SPEC.md && export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared && touch src/*.rs tests/*.rs; cargo test --release --test loop_spec_release_count_method --test loop_spec_eval_read_path_pointer --test loop_spec_loopd_log_pointer --test loop_spec_cheap_exit --test loop_spec_empty_chain --test loop_spec_doctrine_prune
