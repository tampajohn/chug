# Mental model

chug is a driver-in-code loop, not a chat prompt. Five pieces carry
everything; every feature lands inside one of them.

- **The loop is a program.** Continue/stop is the driver's call, never the
  model's: the anti-stall kick, the stuck tripwire, budget-low warnings,
  and the same-cwd driver lock live in the binary. The model proposes;
  the loop disposes.
- **LEDGER.md is external memory.** Updated every iteration, injected into
  every turn — transcript trimming can never lose progress. A fresh run
  archives and reseeds it; `--resume` keeps it.
- **TODO.md + specs/ are the work queue.** One row per item, one spec file
  per row, a mechanical `check:` line per spec. The supervisor reads the
  same files every cycle — the queue is the interface, not a chat channel.
- **Gates + validators are the trust boundary.** Build, clippy, the full
  test suite, and an adversarial validator from a *different* model family
  stand between "the model says done" and "the work lands"; mutant runs
  prove the tests can fail.
- **Cheap-exit / dormancy is the cost boundary.** An empty day takes a $0
  mechanical cheap exit instead of launching a model; an empty queue goes
  dormant — a weekly read-only audit instead of manufactured work.

```
        ┌─────────────────────────────────────────────┐
        │  TODO.md + specs/        the work queue     │
        └──────────────────────┬──────────────────────┘
                               ▼
   ┌───────────────────────────────────────────────────┐
   │          the driver loop  (a program)             │
   │                                                   │
   │   re-read SPEC + GOAL + LEDGER.md each iteration  │
   │            │                                      │
   │            ▼                                      │
   │     one model turn (read / edit / bash / grep)    │
   │            │                                      │
   │            ▼                                      │
   │   claim: goal_complete → the check: re-runs       │
   │        │                  │                       │
   │     exit 0            nonzero ──► keep chugging   │
   │        ▼                                          │
   │   gates: build + clippy + suite + validator       │
   └──────────────────────┬────────────────────────────┘
                           ▼
        ┌─────────────────────────────────────────────┐
        │  wrap: merge + push + harvest → next cycle  │
        └─────────────────────────────────────────────┘
```

LEDGER.md rides beside the loop the whole way down — rewritten every
iteration, so a trimmed or crashed run resumes from the page, not from
the model's memory.
