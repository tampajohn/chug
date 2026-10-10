# T286 — probe hygiene: name the error-visible jq variant

estimate: ~15 changed lines (META-META-SPEC.md one-sentence extension of the T284 paragraph + this spec)

check: cd "$(git rev-parse --show-toplevel)" && export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared && touch src/*.rs tests/*.rs && grep -q 'Probe hygiene (T284' META-META-SPEC.md && grep -q 'Cannot index array with string' META-META-SPEC.md && grep -q 'string and number cannot be added' META-META-SPEC.md && cargo test --release --test todo_consistency --test eval_outcomes_carry --test eval_state_delta --test loop_spec_cheap_exit --test loop_spec_check_wall --test loop_spec_doctrine_prune --test loop_spec_eval_read_path_pointer --test loop_spec_recovery --test loop_spec_validation_lane --test internal_info_lint

## Context

META-META-SPEC.md carries the probe-hygiene paragraph (bold lead
`**Probe hygiene (T284, exit codes and accessor shapes).**`, immediately after
the T272 clause), landed at 0ae979d. Its jq half describes only the SILENT
failure variant — a guessed accessor on the wrong event shape exits 0 printing
`null` for every leg, so the slip never trips the ok:false flag — and never
names the ERROR-VISIBLE variant: jq exits nonzero naming the mis-shape
(`Cannot index array with string "todos"` when a guessed object accessor meets
an array-shaped file; `string and number cannot be added` when arithmetic
rides a string). The jq-probe-slip family census reached 10 at the trip-92
evaluation with BOTH post-T284-landing fires being the unnamed noisy variant
(the cycle-494 stream's 21:44:04Z string+number leg on a decision id; the
trip-92 stream's `.todos[].id` composed from schema memory on the
ARRAY-shaped .chug/todos.json, exit 5) — the 10th-family-sighting trigger for
the jq-idioms doctrine row, discharged as this narrow extension of the landed
clause rather than a new paragraph: the root discipline (compose the
projection from the INSPECTED shape, never schema memory) is shared and
already landed in T284; the gap is the clause's coverage of the noisy
variant, and both post-landing fires are the evidence a reader does not
pattern-match the silent-variant text to the noisy slip.

## Requirements

- Extend the T284 probe-hygiene paragraph IN PLACE (ONE sentence, no new
  paragraph, no new bold lead): name the error-visible variant — jq also
  fails a guessed shape noisily, exiting nonzero and naming the mis-shape
  (`Cannot index array with string`, `string and number cannot be added`) —
  and state that the same inspect-first habit (a `type` / `keys:` shape probe
  before the projection) covers both variants.
- The pin-surface probe ran at filing per the T285 clause:
  `grep -rn "Probe hygiene\|T284\|accessor" tests/*.rs` printed ZERO hits —
  no pin window references the T284 paragraph, so the in-place extension
  disturbs no census needle. RE-RUN the probe before committing; a hit means
  the edit moves the sentence into a fresh paragraph instead (never edit
  inside a pinned window blind).
- docs-only edit — format nothing else; the paragraph stays ONE paragraph.

## Tests

- `grep -c 'Cannot index array with string' META-META-SPEC.md` >= 1 (the
  variant's literal token present).
- `grep -c 'string and number cannot be added' META-META-SPEC.md` >= 1.
- `cargo test --release --test eval_outcomes_carry --test loop_spec_doctrine_prune`
  green (the META-META-SPEC-reading pins — the extension adds no `Outcomes`
  token to the Write-EVALUATION.md window and disturbs no doctrine needle).
- docs-only floor: `cargo test --release --test todo_consistency` green.

## Acceptance

- The T284 paragraph names both jq failure variants with their literal error
  tokens and the inspect-first habit covering both; the paragraph stays ONE
  paragraph; the pin-surface probe re-run is empty; the pins above green;
  kimi validation PASS (doctrine item — SOLO, kimi REQUIRED per LOOP-SPEC
  step 4's doctrine list).
