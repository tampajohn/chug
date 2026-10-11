# T290 — the python read-shape inspect-first clause joins the T270/T273 paragraph (the class's 6th fire executes the armed letter)

**Pri:** 4 (doctrine — ONE sentence; the loop's probe-authoring clause
generalizes to the reader-LAYOUT shape the 6th fire demonstrated)

**SOLO doctrine surface** (ONE sentence in META-META-SPEC.md's T270/T273
probed-assertion paragraph — the loop/spec doctrine itself): never overlaps,
kimi REQUIRED validation.

## Repo context

The python read-shape census stood at 5 with an armed letter: the trip-99
eval's WEIGHED-AND-HELD disposition (EVALUATION.md cycle-524 §2.2, record
d1791700550-4) wrote "**a 6th fire files the section-scoping clause sentence
(naming counts AND extractions) into the T270/T273 paragraph**." The trip-100
eval (cycle 528) produced the 6th fire (record d1791704581-5): a python probe
composed from an ASSUMED file layout —

```
python3 -c "…for l in open('.chug/todos.json'): ids.append(json.loads(l)['id'])…"
```

— read `.chug/todos.json` as JSONL when the file is a single-doc JSON list;
the per-line `except: pass` swallowed the TypeError and the probe printed a
false `todos 0 dups 0 tail None`. Caught same-leg by the known-count
cross-check (~1,014 ids), corrected via `head -c 300` shape-inspection + a
direct single-doc parse, zero casualty — the guard-correct shape of the
class's fires 4/5 (trip-99: "Both fires were PROBED expectations mis-SCOPED
… both guard-correct … zero casualty"). The letter executes: this row lands
the clause sentence.

The paragraph under edit is the T270/T273 probed-assertion paragraph in
META-META-SPEC.md (opens "**The probed-assertion helper (T273,
import-don't-re-author).**", ends "…fires 3 (d1791684611-3 …) and 4 (the
cycle-512 wrap's 03:02:56Z ring-lines leg …) are the citation."). The T288
sentence already covers the section-scoped COUNT variant; the 6th fire is
the READER-LAYOUT variant — neither a count nor an extraction — so the
sentence names reader shapes explicitly.

**Filing-time pin-surface probe (T285 dogfood, run before the sentence was
finalized):** the paragraph is UNPINNED — `probe-first`, `probed actuals`,
`re-authoring it in a fresh heredoc`, `remembered constant INTO the helper`
have ZERO hits across tests/*.rs; `T273` has 3 comment-only mentions
(loopd_daemon_ensure.rs:204, loopd_spoof_guard.rs:131,
loopd_stale_binary.rs:146); the generic `heredoc` needle in
loop_spec_recovery.rs (HEREDOC_NEEDLE) is scoped to META-SPEC.md by its
`meta_spec()` loader — a DIFFERENT file. The `verbatim splice` whole-file
count in META-META-SPEC.md stands 1 and the new sentence does not carry the
needle. Zero collisions to reconcile.

## Requirements

1. Append ONE sentence at the end of the T270/T273 paragraph (immediately
   after "…are the citation."), carrying: (a) the 6th fire's record id
   d1791704581-5 and its mechanism (the assumed-JSONL reader over the
   single-doc todos.json, the swallowed per-line exception printing a false
   empty answer, the same-leg known-count catch, zero casualty); (b) the
   generalization — the inspect-first clause binds READER SHAPES the same as
   sections: READ the file's actual layout (a head/probe of the shape)
   BEFORE composing the reader; (c) the masking note — a bare `except: pass`
   in a probe hides the layout mismatch instead of surfacing it.
2. No other file touched; the paragraph's existing text byte-identical
   outside the appended sentence; the `verbatim splice` whole-file count
   unmoved at 1; zero new pin collisions (re-run the filing-time probe
   pre-commit and compare byte-for-byte).
3. No new pin added (the paragraph's surface stays as probed — the T285
   declaration is this spec's).

## Tests

- The doctrine battery in the check line (the nine META-META-SPEC-reading
  suites, enumerated mechanically via `grep -l META-META-SPEC tests/*.rs`)
  green — the sentence must not trip any existing pin.
- RED-proof (orchestrator, at review): deleting the appended sentence
  returns the file byte-identical to pre-change (git diff empty); the
  sentence's record-id needle `d1791704581-5` greps exactly once in
  META-META-SPEC.md post-landing.

## Acceptance

- ONE sentence appended, paragraph otherwise byte-identical; `git diff
  --stat` shows META-META-SPEC.md only, a few `+` lines, zero `-` lines.
- The filing-time pin probe re-run byte-identical (zero pin hits, 3
  comment-only T273 mentions, HEREDOC_NEEDLE scoped to META-SPEC.md).
- Full check line green in the impl worktree.

estimate: ~10 changed lines (ONE doctrine sentence) — far under the T209
~500 ceiling; eligible for Trivial-row bundling shape but runs SOLO per the
doctrine-surface rule.

check: cd "$(git rev-parse --show-toplevel)" && export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared && touch src/*.rs tests/*.rs; set -o pipefail; cargo test --release --test eval_outcomes_carry --test eval_state_delta --test loop_spec_cheap_exit --test loop_spec_check_wall --test loop_spec_doctrine_prune --test loop_spec_eval_read_path_pointer --test loop_spec_recovery --test loop_spec_validation_lane --test todo_consistency 2>&1 | tail -20
