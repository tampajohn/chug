# T288 — wrap_assert learns probe-deriving, section-scoped count assertions

**Pri:** 3 (robustness — the wrap's own write gate keeps aborting correct
artifacts; each fire costs a re-run in the wrap tail and, two wraps running,
landed in the notes→goal-gate window as digest-backstop tail legs)

**SOLO doctrine surface** (scripts/wrap_assert.py + scripts/test_wrap_assert.py
+ ONE sentence in META-META-SPEC.md's T273 paragraph — the loop/spec doctrine
itself): never overlaps, kimi REQUIRED validation.

## Repo context

The T270/T273 remembered-constant class's standing rule (recorded at the
trip-96 eval, EVALUATION.md §2.2): **the FOURTH post-adoption fire FILES this
row.** The class: fill/wrap-phase verification-helper assertions whose count
expectation mis-calibrates against STRUCTURALLY CORRECT content and fires a
wrong-expectation abort on a correct artifact (META-META-SPEC.md's T270
clause; `scripts/wrap_assert.py` is the T273 helper whose import was supposed
to kill the shape).

The four post-adoption-relevant fires, all zero-casualty (the write gate
caught every one pre-write):

1. d1791605730-7 (trip 75) — hardcoded `(6, 6)` count expectation vs an
   artifact holding that plus pre-existing legitimate textual mentions.
2. d1791618821-7 (cycle-446 wrap) — hardcoded `blocks == 76` vs a live-grown
   transcript (78+ blocks).
3. d1791684611-3 (the cycle-508 wrap, fire 3 — the NEW remembered-ARG
   sub-shape) — two `assert_count_exact` failures (expected 6 vs probed 8):
   the remembered constant rode the helper's imported expected-count ARG,
   not a hardcoded literal (8 = 6 + 2 legitimate pre-existing mentions).
4. **The cycle-512 wrap's 03:02:56Z leg (fire 4 — this row's trigger)** —
   `assert_count_exact('ring lines in the temp file')`: expected **12** (the
   ring's schema capacity, a remembered constant passed as the helper's ARG)
   vs probed actual **17** — 12 ring lines + 5 open-threads bullets that
   also start `- d` (`- decision_log-arg-slip …`, `- describe-count-slip …`,
   `- duplicate-subject …`), all counted because the probe split the WHOLE
   temp file on `startswith('- d')` instead of scoping to the `## decisions`
   ring section. The artifact was structurally correct (the state landed
   ring 12: 4 prepended/8 carried/4 rotated, post-write ok read passed,
   splice-check `ok (carried 8, rotated 4, appended 4)`); the assertion was
   wrong twice over — a remembered ARG *and* an under-scoped count probe.

The two sub-shapes share one anatomy: **the expectation is a remembered
constant riding the helper's ARG, and/or the count probe's surface is wider
than the section it names** — so legitimate content mis-fires the guard.

estimate: ~150 changed lines (helper ~45 + unittest ~70 + ONE doctrine
sentence + this spec's own surfaces) — well under the T209 ~500 ceiling.

## Requirements

1. **Probe-deriving count assertions.** `scripts/wrap_assert.py` learns a
   variant whose `expected` is PROBED from the live artifact at call time,
   never passed as a remembered constant. Concretely: a primitive that
   extracts the `## decisions` ring section from a state-file text (the
   lines between the ring header — `## decisions ring (last 12, newest
   first — newest batch at the head)` — and the next `## ` header or EOF)
   and an `assert_ring_count(path_or_lines, expected_capacity=12)`-shaped
   assertion that counts `- d` lines WITHIN that section only. The existing
   `assert_count_exact(actual, expected, label)` stays for genuinely
   two-probed-count call sites (its contract already says "2 probed counts").
2. **Section scoping is the fix's teeth.** The open-threads `- d` bullets
   must be UNABLE to enter a ring-line count: the extraction is anchored on
   the ring's own header line, LINE-ANCHORED (`^## ` per T266), never a
   bare substring.
3. **Unittest RED legs** in `scripts/test_wrap_assert.py`: a fixture state
   file whose ring holds 12 `- d` lines AND whose open-threads section
   holds ≥3 `- d` bullets — the OLD whole-file count shape reads 15+ and
   mis-fires; the new section-scoped assertion passes; a ring with a REAL
   violation (11 or 13 ring lines) still fails. Mirror the existing suite's
   style (stdlib unittest, temp files).
4. **ONE sentence in META-META-SPEC.md's T273 paragraph** (after the
   "The helper's arguments are the probed actuals — passing a remembered
   constant INTO the helper is the same banned shape as inlining it."
   sentence) naming the probe-deriving variant: count expectations over a
   NAMED SECTION derive from the section-scoped live probe the helper
   performs (the ring-lines count scopes to the `## decisions` ring
   section), never a remembered ARG — fires 3 (d1791684611-3) and 4 (the
   cycle-512 wrap's 03:02:56Z ring-lines leg) are the citation.
5. **Pin-surface probe (T285) re-run at impl time**: the filing-time probe
   read ZERO rust-test needles quoting the T273 paragraph
   (`git grep -n 'probed-assertion helper\|import-don\|fresh heredoc\|remembered constant\|re-authoring' -- tests/*.rs`
   EMPTY) and only comment mentions of wrap_assert in tests/*.rs
   (loopd_daemon_ensure.rs:204, loopd_spoof_guard.rs:131,
   loopd_stale_binary.rs:146 — the T273 __pycache__ side-effect comments,
   untouched by this change). Re-run before finalizing; declare any
   collision with its reconciliation.
6. Do NOT change `assert_byte_carry`, `assert_window_count`,
   `assert_window_clean`, or the ring's prepend convention (T287, landed).
   No other repo file changes beyond the three named targets.

## Tests

- `python3 scripts/test_wrap_assert.py` — the full unittest suite,
  extended per requirement 3 (RED legs named above must FAIL against the
  pre-change helper where the assertion shape is new, PASS after).
- The doctrine-carrier battery stays green (the META-META-SPEC.md sentence
  touches pinned doctrine carriers):
  `cargo test --release` for the suites that read META-META-SPEC text
  (eval_outcomes_carry, eval_state_delta, loop_spec_cheap_exit,
  loop_spec_check_wall, loop_spec_doctrine_prune,
  loop_spec_eval_read_path_pointer, loop_spec_recovery,
  loop_spec_validation_lane, todo_consistency).

## Acceptance

- The new section-scoped ring assertion passes on a structurally-correct
  state file holding 12 ring `- d` lines + ≥3 open-threads `- d` bullets
  (the fire-4 fixture) and FAILS on a real ring violation.
- The T273 paragraph carries the new sentence citing fires 3+4; zero pin
  collisions (requirement 5's probe re-run EMPTY before AND after the edit).
- `python3 scripts/test_wrap_assert.py` green; the named cargo suites green.

check: cd "$(git rev-parse --show-toplevel)" && export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared && touch src/*.rs tests/*.rs; python3 scripts/test_wrap_assert.py && set -o pipefail; cargo test --release --test eval_outcomes_carry --test eval_state_delta --test loop_spec_cheap_exit --test loop_spec_check_wall --test loop_spec_doctrine_prune --test loop_spec_eval_read_path_pointer --test loop_spec_recovery --test loop_spec_validation_lane --test todo_consistency 2>&1 | tail -20
