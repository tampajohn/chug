# T223 — judge-parity bake-off RUN + verdict doc (T220 split, measurement half)

check: test -f docs/judge-parity.md && grep -q "default-checkpoint decision" docs/judge-parity.md

estimate: ~120 lines (one runner script + the committed verdict doc)

## Concern

T220's acceptance requires the MEASUREMENT, not just the loader: both
contestants run over the stop-judge goldens + the decision-log holdout,
per-class verdicts named with numbers, and the default-checkpoint
decision recorded. Sequenced AFTER T222 (the loader half).

## Requirements

1. Runner (scripts/judge-parity.sh or a cargo-test entry): runs Laya
   and kev-0.8b (T222's loader, pinned sha) over (a) the stop-judge
   golden vectors and (b) the labeled decision-log holdout; emits the
   T222 metric set per class.
2. docs/judge-parity.md committed: per-class winner with the numbers,
   corpus sizes named, the option-order flip rates, and the
   default-checkpoint decision — Laya stays unless kev wins a class by
   >2 points top-1 / AUROC (margin rule from T220 req 3).
3. If the researched verdict-2.0 id cannot be resolved by then, the doc
   names the gap explicitly (one contestant short) rather than
   fabricating a comparison — an honest partial verdict is the
   deliverable.
4. External anchor optional: the LocalLLaMA typed-decisions suite runs
   alongside only if it is already local; never fetched mid-run.

## Tests

- The runner is idempotent and exits nonzero if a judge fails to load
  (fail-closed measurement — T217 doctrine).
- The doc carries the pinned checkpoint shas used (T205 provenance).

## Out of scope

- Loader work (T222); gating changes; fine-tuning; hosted Jev.
