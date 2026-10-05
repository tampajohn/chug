# T239 — site-sync gate-count guard: pin the three surviving-mutant legs (validator findings)

check: cargo test --test site_sync

estimate: ~60 lines (tests-only fixture pins)

## Concern

T238's kimi validator (cycle 114, verdict PASS) mutation-tested the new
gate-count guard and produced three SURVIVORS — green tests that do not
kill real mutants on load-bearing legs. All three degrade honestly (an
older full count, never a subset), which is why they were non-blocking;
but pins are cheap and each survivor names its own killing fixture:

1. **mut-runner** (the one-runner-word grammar leg removed): the
   `nextest release 1664/1664` shape — the shape EVERY wrap gate count
   since the T82 switch carries — has no fixture pin. A regression here
   silently degrades the card to an older full count.
2. **mut-loop1** (within-message walk stops after the first pair): the
   accept-after-reject continuation (`pos += oend - 1`) is load-bearing
   in production — at a HEAD whose own commit message quotes fixture
   censuses (90900bc's "nextest 113/1402" / "nextest 1400/1496" quoted
   alongside the real count), the walk must skip the rejected pairs and
   still accept the later qualifying pair IN THE SAME MESSAGE.
3. **mut-floor100** (GATE_FLOOR lowered): the floor's exact value is
   unpinned — known package subsets (5/5, 22/22) are pinned but a
   mid-size equal-operand subset (e.g. 113/113) is admitted by a
   lowered floor. Pin that a below-floor equal-operand mid-size count
   loses to an older full count.

## Requirements

1. Fixture pin: a commit message carrying `nextest release 1664/1664`
   (one runner word) is accepted and cited (kills mut-runner).
2. Fixture pin: ONE commit message carries a rejected pair followed by
   a qualifying pair ("nextest 113/1402 … nextest 1664/1664") — the
   walk accepts the qualifying pair from the same message, no walk-back
   to an older commit (kills mut-loop1).
3. Fixture pin: a mid-size equal-operand subset below the floor
   ("nextest 113/113") loses to an older full count; the card never
   shows 113 (kills mut-floor100 at any floor ≤ 500 > 113).
4. RED-prove each pin against its named mutant (drop the runner-word
   group / stop-after-first-pair / lower the floor) — the mutant must
   flip the new pin RED and no other pin may be weakened.

## Tests

- The three pins above in tests/site_sync.rs fixtures (same shape as
  the T238 pins: fixture commits, run_sync, card assertions + citation
  ref assertions).

## Out of scope

- Bare `(N/N)` prose shapes without the nextest token (validator
  finding 3 — grammar narrowness, honest degradation, commit-sourced by
  design); the self-quote transient (finding 5); any scraper behavior
  change — this row is pins for EXISTING behavior only.
