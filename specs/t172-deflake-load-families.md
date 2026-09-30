# T172 — Deflake the nextest-parallel load families (mechanism, not timeouts)

check: export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared && cargo test --test loopd_stale_binary --test loopd_spoof_guard --test loopd_orphan_reaper && export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared && cargo test --bin chug mcp_serve

## Repo context

The cycle-79 eval (EVALUATION.md §2.2) carries FOUR cycles of flake
sightings in two deadline-bearing families under gate-parallel load:
(a) the loopd supervisor harness — `loopd_stale_binary` +
`loopd_spoof_guard` 90s timeouts at nextest parallel load (T167 gates,
solo-green 3/3 under the T82 fallback) and
`the_reaper_terms_an_orphan_through_loopd_before_the_build`
(tests/loopd_orphan_reaper.rs:1042) exceeding its 30s verdict deadline
(30.8s at 17-way load; solo 3.36s — the T152 signature);
(b) the mcp_serve stub-spawn family —
`chug_launch_stub_spawn_pins_exact_argv_cwd_and_return_paths`
(src/mcp_serve/tests.rs) starved 3× in cycle 77 under different load
shapes (solo 0.55s) plus one `chug_cancel_happy_path` false rejection.
All are wall-clock-deadline tests whose windows stretch when the suite
manufactures CPU starvation. Doctrine: T31 (mechanism, not timeouts —
RUN_SHELL_TIMING_LOCK serialized the run_shell wall-clock tests, zero
constants touched) and T151 (flake families, landed cycle 72).
tests/loopd_stale_binary.rs:191 shows the deadline pattern
(`Instant::now() + Duration::from_secs(30)`).

estimate: ~150 changed lines (serialization/seam mechanism ~40-80 +
per-family application + regression notes; tests-only — src/ production
code untouched; if a production seam proves necessary the estimate holds
but the validation routing upgrades to kimi REQUIRED).

## Requirements

1. Identify every deadline/timeout-sensitive test in the three loopd
   harness files (tests/loopd_stale_binary.rs, tests/loopd_spoof_guard.rs,
   tests/loopd_orphan_reaper.rs) and the two named mcp_serve tests;
   enumerate them in the commit message.
2. Apply a MECHANISM fix so suite-manufactured scheduler stretch can no
   longer co-occur with a clocked window. Acceptable shapes (child's
   choice, justified in the commit message): a cross-binary
   serialization harness that works under BOTH gate runners (a static
   Mutex is per-binary — nextest runs each test binary as its own
   process, so the T31 static-Mutex shape alone does NOT cover
   cross-binary contention; a file-lock in a shared-but-content-safe
   location, e.g. under the target dir or a tempdir keyed by the test
   binary family, does), a nextest test-group serial configuration PLUS
   an equivalent for the `cargo test --test-threads=4` fallback, or
   clock/deadline seams that remove real-time dependence. Whichever
   shape: ZERO timeout/deadline/grace constants are bumped — every
   existing second-valued bound stays byte-identical (T31 doctrine).
3. The fix must hold under BOTH runners: `cargo nextest run --release`
   at full parallelism AND `cargo test --release -- --test-threads=4`.
4. If any mechanism needs a file lock, it degrades fail-open with a
   stderr note (a locked/absent lockfile must never hang the suite —
   bounded wait, then proceed), mirroring T135's flock discipline.
5. Tests-only: no production src/ edits (a genuinely needed seam is
   allowed but must be minimal and named in the commit message, and the
   orchestrator re-routes validation to kimi REQUIRED in that case).

## Tests

- The named families green SOLO and under load: run the check, then run
  the full nextest release suite once (the orchestrator's gates re-run it
  independently). State in the commit message: which mechanism, how many
  tests serialized/seamed, and the before/after under-load runs.
- RED-proof the mechanism: temporarily re-introduce serial-unsafe
  execution (or shrink a deadline in a scratch copy — reverted before
  commit) and show the new harness fails/holds as designed; state it.

## Acceptance

- Spec check green; full `cargo test` green; clippy `-D` clean.
- `git diff main -- 'src/*.rs'` empty (unless req 5's named seam).
- `git diff main | grep -E "from_secs\([0-9]" ` shows NO changed
  second-valued constants.

## Out of scope

- Timeout/deadline value changes, new test frameworks, nextest version
  changes, flakes outside the two named families (new sightings get their
  own row), the T173/T178 budget rows.
