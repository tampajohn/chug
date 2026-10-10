# T282 — Outcomes-entry figures probed at write time against their mechanical source (META-META-SPEC clause)
estimate: ~30 changed lines (one ~10-line META-META-SPEC.md paragraph; doctrine, md-only)

check: cd "$(git rev-parse --show-toplevel)" && export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared && touch src/*.rs tests/*.rs && grep -q 'Outcomes-entry figures (T282)' META-META-SPEC.md && grep -q -- 'probed at write time against its mechanical source' META-META-SPEC.md && grep -q 'Corpus counts (T265)' META-META-SPEC.md && grep -q 'Corpus counts at wrap (T281)' META-META-SPEC.md && cargo test --release --test todo_consistency --test eval_outcomes_carry --test eval_state_delta --test loop_spec_cheap_exit --test loop_spec_check_wall --test loop_spec_doctrine_prune --test loop_spec_eval_read_path_pointer --test loop_spec_recovery --test loop_spec_validation_lane --test internal_info_lint

## Repo context

- The T272 clause (`**Enumeration by flag, never recall (T272,
  enumeration-by-flag).**`) pins whole-stream failed-leg/ctx-edit
  ENUMERATIONS: mechanical reads of the stream's own flags at the named
  read time, never recall. The T265/T281 clauses pin the same method for
  decision-corpus counts (eval-side and wrap-side). The T270/T273 clauses
  pin it for fill/wrap helper assertions.
- The gap the clauses do NOT name: every OTHER figure an Outcomes entry
  or wrap/fill commit records — release recounts and their eval-time
  previews, timestamps quoted from committed text, gate tallies, leg
  counts. The Outcomes-figure confabulation class lives in that gap and
  has FOUR fires, all zero-casualty only because a backstop caught each:
  - fire 3 (d1791662162-10, cycle-490 wrap): the trip-90 eval's Outcomes
    preview wrote ZERO flip-subjects for the release recount — it
    conflated the chain window (65b0b21..HEAD, dispositions only) with
    the recount window (v0.17.15..HEAD, which holds the T281 flip
    28ee221); the wrap's mechanical recount read TRUE 1 {T281}. Zero
    casualty (NO TAG either way), the preview figure corrected per T70.
  - fire 4 (d1791662375-12, cycle-490 fill): the Outcomes entry's
    sh-backtick bullet carried a `19:36Z` timestamp composed from
    post-compaction memory against the committed eval text's `19:16:12Z`;
    the state-write re-read against the committed text caught it and the
    fill corrected it with its T70 clause named.
- The class's re-arm letter (recorded at the cycle-490 wrap): the third
  fire ACTIVATED the weigh obligation for trip 91, both fires riding as
  evidence. This row is that discharge, filed at trip 91 on schedule.
- estimate: ~30 changed lines all-in (one META-META-SPEC.md paragraph
  ~10 lines + this spec + the TODO row; the child's own diff is the
  paragraph only). Mechanical doctrine addition — no code, no test
  changes.
- **Pin-carrier discipline (T80):** META-META-SPEC.md is a pinned
  doctrine carrier — NINE test binaries read it (enumerated mechanically
  from `grep -l META-META-SPEC tests/*.rs` at filing):
  `tests/todo_consistency.rs`, `tests/eval_outcomes_carry.rs`,
  `tests/eval_state_delta.rs`, `tests/loop_spec_cheap_exit.rs`,
  `tests/loop_spec_check_wall.rs`, `tests/loop_spec_doctrine_prune.rs`,
  `tests/loop_spec_eval_read_path_pointer.rs`,
  `tests/loop_spec_recovery.rs`, `tests/loop_spec_validation_lane.rs`,
  plus `tests/internal_info_lint.rs` walking every tracked file (some
  with exact-count needles). The new paragraph is append-only and must
  NOT quote any existing pinned needle verbatim; the `check:` line runs
  every reader binary, so a broken pin fails the goal gate, not just
  review. The T265 and T281 paragraphs' own bold-lead needles
  (`Corpus counts (T265)`, `Corpus counts at wrap (T281)`) must survive
  intact — the check's third and fourth greps pin that.

## Requirements

1. Add ONE paragraph to `META-META-SPEC.md`, placed immediately after the
   existing `**Corpus counts at wrap (T281).**` paragraph (before the
   `Sections:` list), bold-led `**Outcomes-entry figures (T282).**`,
   carrying the rule:
   - EVERY figure an Outcomes entry or a wrap/fill commit records — a
     release recount or its eval-time preview, a timestamp quoted from
     committed text, a gate tally, a leg count — is probed at write time
     against its mechanical source: `git log` for recounts (each window
     NAMED — the recount window `v<tag>..HEAD` is never the chain
     window), the committed eval text for carried quotes, `wc -l`/`jq`
     for counts, the stream's own flags per T272. A figure is never
     composed from memory, an earlier draft, or a conflated window.
   - The designed backstops are named: the wrap's mechanical recount and
     the fill's re-read against the committed text; a figure a backstop
     catches is corrected in the fill with the T70 clause named (the
     correction rides the fill, never history edits).
   - The evidence sentence: the class's third and fourth fires at the
     cycle-490 wrap — the eval preview's ZERO-flip figure conflated the
     chain window with the recount window (the mechanical recount read
     TRUE 1), and the entry's `19:36Z` timestamp against the committed
     eval text's `19:16:12Z` — both zero-casualty because the backstops
     caught them.
2. No other repo file changes (this spec and the TODO row are the
   orchestrator's bookkeeping, not the child's).
3. The paragraph must not duplicate any existing META-META-SPEC sentence
   verbatim (the exact-count pins), must not alter the T265/T272/T281
   paragraphs (their needles survive — the check's survival greps), and
   must not name operator-identifying paths (the internal-info lint):
   `.chug/decisions.jsonl`-class repo-relative names are the only paths
   the rule needs, and it needs none beyond what T272/T281 already name.

## Tests

- The `check:` line's four greps (the new bold lead, the
  `probed at write time against its mechanical source` needle, the T265
  and T281 paragraphs' survival).
- Every pin binary that reads META-META-SPEC.md green (the `check:`
  line's nine `--test` legs + the lint) — the T80 pinned-carrier
  discipline for an md-only edit.

## Acceptance

- META-META-SPEC.md carries the Outcomes-entry figures paragraph;
  `check:` green.
- Review/merge gates per the docs-only classification (the diff touches
  ONLY `*.md`): the T80 guard floor
  (`cargo test --release --test todo_consistency`) PLUS the pin binaries
  named above; the full suite skipped per the T80 letter.
- Validation: kimi REQUIRED (doctrine edit — META-META-SPEC.md is on the
  step-4 core list by construction).
