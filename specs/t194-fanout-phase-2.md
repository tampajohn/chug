# T194 — fan-out phase 2: 3rd impl slot + parallel validators

check: cargo test

estimate: ~300 lines (LOOP-SPEC amendment + target-validate split + pins)

## Concern

Operator 2026-10-01: "feels arbitrarily slow still — is there the ability
to fan out more work?" Measured: the orchestrator is NOT the bottleneck
(~1.5 iters/min, idle on long-poll); the per-item serial chain is
impl (20-45 min) → gates (5-10 min) → kimi validator (15-30+ min, capped
at 1) → serial merge. T161's caps (≤2 children, ≤1 validator, "never 3+,
never 2 validators" — LOOP-SPEC Phase 2) bind every cycle with 3+ rows.

## Repo context

- T161 overlap rule (LOOP-SPEC Phase 2 tail): ≤2 children in flight, ≤1
  validator, 2 impls only when spec-named target files are disjoint,
  merges strictly serial, FAIL pauses merge not impl.
- T47/T52 role-keyed target dirs: target-shared (impls, cargo lock
  serializes builds safely), target-validate, target-gates, target-main.
- Parallel mutants LANDED (cycle-85 wrap: m1-m3 caught in parallel
  legs) — within-validation parallelism exists; BETWEEN-validation
  parallelism does not.
- T29 wait_secs long-poll: orchestrator wakes on first state flip among
  N children — a 3rd child is ~free in orchestrator iterations.
- T189 (queued, low-stakes lane) shrinks validator DEMAND for chores;
  T194 raises validator SUPPLY for items that still need it. T193
  already landed gates-only in 52 min via the T189 predicate.
- Hardware ceiling: K7 compile storms already hit ~10 concurrent rustc
  at 2 impls + 1 validator + wrap gates; 3 impls + 2 validators ≈ 5
  concurrent suites is the watch item.

## Requirements

1. T161 amendment (LOOP-SPEC Phase 2): at most 3 children in flight, of
   which at most 2 validators; 3 impl children only when spec-named
   target files are PAIRWISE disjoint across all three; merges strictly
   serial in dispatch order (unchanged); FAIL pauses merge not impl
   (unchanged).
2. Validator target-dir split: target-validate-a / target-validate-b
   keyed by validator slot (two validators sharing one dir would
   serialize on the cargo lock, defeating the change). loopd.sh exports
   the slot's dir; wrap gates keep target-gates. T47 invariant holds: no
   two cargo processes share a target dir.
3. A 2nd validator launches ONLY when two items are simultaneously past
   gates — never two validators on the SAME item (correlated verdicts
   add nothing; family-independence rule unchanged: validators always
   kimi).
4. Resource governor: at most 4 cargo-heavy children total (impls +
   validators); if memory pressure forces a choice, validators win
   (merges unblock the queue) and the orchestrator records the
   degradation via decision_log.
5. Doctrine: LOOP-SPEC Phase 2 caps restated; T161 row annotated
   (amended by T194); README loop diagram if it names the caps.

## Tests

- Doctrine pins (loop_spec_* test crates): new caps present, "never 3+,
  never 2 validators" text gone, pairwise-disjointness gate stated.
- Disjointness gate: a 3rd impl whose spec-named files overlap any
  in-flight impl's files is blocked (pin the predicate).
- Acceptance: a cycle lands 3 items with 2 validators overlapped, or
  the wrap records why the caps didn't bind (queue shape), in Outcomes.

## Out of scope

- >3 children (merge serialization makes it queue at the gate anyway);
  validator on non-kimi family; per-item multi-validator; relaxing the
  single-driver guard (one main, one LEDGER — multi-loop is the
  repo-level lever and already exists as the dashboard/internal-monorepo pattern).
