# T152 — loopd pre-cycle orphan-process reaper (fail-closed precision)

check: cargo test

## Repo context

Cycle-70 T145-arc incident: the orchestrator found a **9.8-hour-old
spinning test binary at 99% CPU** (~590 CPU-minutes burned) left over
from the t134-validation era, and killed it mid-arc (cycle-70 Outcomes,
T145 entry). The orphan aggravated machine load across at least two
cycles — and load is the triggering condition of the T151 flake family
(six default-parallelism fires in cycles 70–71). Orphan shapes: (a) T79
parallel-mutant legs whose validator died mid-leg (budget abort) leaving
a test binary spinning in `target-shared-mut-<k>/` or
`target-shared-validate/`; (b) a child killed at the bash 120s cap whose
cargo/test process group outlived it; (c) a removed `/tmp/chug-loop-t*`/
`/tmp/chug-mut-*` worktree's leftover process. Today nothing reaps them:
loopd's pre-cycle steps are single-driver probe → build gate →
eval-digest → launch (loopd.sh:200-280).

The reaper runs at the ONE safe instant: **pre-cycle, after the
single-driver check passes, before the build** — no orchestrator and no
children can legitimately exist at that point, so any leftover chug
test/build process is definitionally orphaned. Legitimate processes that
must NEVER be killed: the operator's own builds in `./target/`
(non-shared dir), anything not matching the loop-artifact shape, and any
process whose identity is ambiguous (fail-closed: ambiguity → skip + log
line, never kill).

estimate: ~160 changed lines (loopd.sh ~50, tests/loopd_*.rs legs ~110)

## Requirements

1. **Sweep point.** loopd.sh gains a reaper step immediately AFTER the
   single-driver probe passes and BEFORE `cargo build --release` (the
   T137 gate). It logs every judgment (killed / skipped-with-reason) one
   line each to the cycle log via the existing `ts` logger.
2. **Needle (precise, two legs, either qualifies).** A process is a
   reaper candidate ONLY if (a) its executable path (not argv[0] text —
   resolve via `ps -o comm=` and require an absolute path containing
   `/target-shared` + `/deps/`) matches the loop's test/binary artifact
   shape, OR (b) its argv contains `/tmp/chug-loop-t` or `/tmp/chug-mut-`
   AND the named worktree directory no longer exists. Both legs require
   the process to NOT be the reaper's own process group and NOT a
   current cycle process (none can exist at the sweep point by
   construction — state this invariant in a comment).
3. **Fail-closed judgment.** For every candidate: if ANY identity leg
   cannot be resolved (ps failure, non-absolute comm, unreadable),
   SKIP and log `orphan-reaper: skip pid=<n> (unresolved)` — never kill
   an unresolved process. Kill leg: SIGTERM only (no SIGKILL needed —
   the next cycle's sweep catches a survivor; state why), log
   `orphan-reaper: term pid=<n> (<matched leg>)`.
4. **Opt-out + bound.** `LOOP_REAPER=0` disables the sweep (logged).
   The sweep is bounded: at most ~64 candidate examinations per cycle
   (paginate deterministically), so a pathological process table cannot
   delay cycle launch by more than a couple of seconds.
5. **Tests** (tests/loopd_*.rs harness family — follow the existing
   loopd test patterns, e.g. tests/loopd_stale_binary.rs): fixture
   processes spawned by the test itself —
   (a) a `sleep` fixture whose argv carries a `/tmp/chug-mut-t999-1`
   needle with that dir absent → the sweep function/script identifies it
   as a candidate (test uses the DRY-RUN/identify leg or a probe env
   knob — no real SIGTERM to a fixture the harness doesn't fully own;
   cleanup is the test's duty);
   (b) a fixture whose comm is an absolute path under a
   `target-shared*/deps/` shape → candidate;
   (c) a fixture with neither needle → NOT a candidate;
   (d) unresolved-identity leg → skip, no kill;
   (e) `LOOP_REAPER=0` → zero judgments logged.
   Prefer testing the reaper as a SOURCED shell function or a small
   `scripts/` helper loopd calls (the implementer picks the seam that
   the existing loopd tests already exercise) — the sweep logic must be
   exercisable without running the whole supervisor.

## Acceptance

- `cargo build`, `cargo clippy --all-targets -- -D warnings`, `cargo test`
  all green; every existing loopd test file still passes unchanged.
- `bash -n loopd.sh` clean; `shellcheck loopd.sh` no NEW warnings vs the
  parent commit (if shellcheck is on PATH; otherwise note its absence).
- A live demonstration in the commit message: spawn a needle-matching
  fixture, run the sweep leg, show the log line, show the fixture reaped
  (or identified, per the test seam), show a non-matching fixture
  untouched.
- README loopd section gains one sentence (the reaper exists, where it
  runs, `LOOP_REAPER=0`).

## Out of scope

- Reaping DURING a cycle (children legitimately hold processes; the
  sweep point invariant does not hold).
- Orphaned `chug run` driver processes (the driver.lock + single-driver
  probe own that class).
- CPU-time-based heuristics (identity-based only — a long-lived but
  correctly-parented build is not this row's business).
