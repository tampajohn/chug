# T157 — harden the spawn-failure-under-resource-pressure test family (invalidation seams, deterministic premises)

check: cargo test

## Repo context

T151 (cycle 72) closed the timing-interference flake classes (one shared
serialization domain + dead_port root fix via `drain_pending_accepts`).
Its acceptance matrix then exposed the REMAINING full-suite flake
family — the one the T151 fix-up child's watch note named
"spawn-failure-under-resource-pressure" and the spec scoped OUT
("do NOT chase — report"). Cycle-72 matrix evidence (default
parallelism, ordinary loop machine load):

- run 1 (3 red): `driver::tests::events::drive_loop_writes_events_jsonl`
  at events.rs:216 — `tools[1]["is_error"]` was TRUE for a scripted
  `printf 'x…x500'` bash that cannot exit nonzero: the run_shell SPAWN
  (or scheduling under pressure) failed / stretched past its timeout and
  the events line correctly recorded is_error (the events WRITER is
  fine — the test's premise "printf always succeeds" broke);
  `driver::tests::hooks_policy::vetoed_premature_goal_complete_loops_until_honest_claim`
  at hooks_policy.rs:475 — `honest-work.txt` never written: the work
  tool's spawn failed under pressure, the loop correctly continued to
  the honest claim, the file-premise broke;
  `mcp_serve::tests::chug_launch_stub_spawn_pins_exact_argv_cwd_and_return_paths`
  at mcp_serve.rs:1751 — `stub never wrote argv.txt` inside the 10s
  `wait_for_stub_dump` poll: the stub spawn failed or starved >10s and
  the helper has NO visibility into the spawn result (it only polls the
  file).
- run 2 (1 red): `drive_loop_writes_events_jsonl` at :216 again, same
  shape.
- run 3 (4 red, under atypical DOUBLE cargo load — a concurrent child
  was building): all four `tests/loopd_spoof_guard.rs` legs panicked at
  :129 — `wait_for_verdict`'s 30s deadline blew: the fixture supervisor
  never reached its verdict line in 30s (the fixture's cycle startup —
  including its build gate — stretches under load; the panic already
  dumps the supervisor log, the deadline premise is the fragile leg).
  These four passed under nextest (per-process scheduling) ~1h prior in
  the same tree.
- run 4 (5 red, induced 8x `yes`-spinner load + a concurrent child
  build): all five `tests/loopd_stale_binary.rs` legs panicked at :165 —
  the same verdict-deadline shape (T135/T137's fixture waits on a real
  supervisor cycle). The loopd fixture family (spoof_guard +
  stale_binary + siblings) is the DOMINANT red face under load — zero
  reds in runs 3-4 were driver/mcp_serve spawn legs, and zero reds in
  runs 1-4 touched any T151-closed class (dead_port, serialized six).
- The T148-impl (cycle 71) "bash is_error flips" and the T144-era
  `delegate_launch_stub_then_status_reports_summary_and_liveness`
  sighting are the same family's earlier faces.

The family mechanism: at default parallelism the full suite runs ~960
unit tests with dozens of concurrent REAL process spawns (bash stubs,
real chug binaries in mcp_serve/delegate tests); under that inherent
pressure a `Command::spawn` can fail outright (EAGAIN-class) or a
trivial child can stretch past a fixed test deadline. Production
behavior in every observed leg is CORRECT (tool errors surface as
is_error results; the loop continues) — the red legs are TEST PREMISES
that assume spawn/schedule success, or single-shot assertions on
outcomes an environment-invalidated attempt can flip. T151's lock
cannot help: the pressure is system-wide, not inter-test interference.

This is the T31-doctrine family of fixes: mechanism, not timeouts —
per T59/T66's `Invalidation` pattern (T151 fix-up): an attempt
invalidated by the ENVIRONMENT is retried (bounded, distinctly named);
an attempt invalidated by the CODE UNDER TEST still panics immediately,
un-retried, byte-distinct.

estimate: ~240 changed lines (tests/test-support only — invalidation
seam + conversions across the named legs ~200, pins ~40)

## Requirements

1. **A spawn-invalidation seam for drive-loop script tests.** The
   driver test harness gains a bounded retry wrapper (name it in the
   Invalidation family's style, e.g. `drive_attempt_with_spawn_retry`)
   that runs the scripted drive_loop attempt and, ONLY when the
   attempt's red leg carries the exact spawn-invalidated marker
   (the `spawning sh -c` context error text, the run_shell timeout
   marker, or the tool error naming the missing spawn — enumerate the
   markers in code with comments), invalidates and retries the WHOLE
   attempt (fresh tempdir), bounded (3 attempts, exhaustion panics
   naming the marker and count). A red leg WITHOUT a spawn marker
   panics immediately, un-retried, byte-distinct — a real regression is
   never retried into a flake-shaped message (T59's guarantee,
   verbatim).
2. **Convert the named legs.** `drive_loop_writes_events_jsonl`,
   `vetoed_premature_goal_complete_loops_until_honest_claim`, and every
   sibling scripted-drive test asserting a tool succeeded (sweep the
   class — the cycle-33 lesson) run through the seam. Where a cheaper
   DETERMINISTIC premise exists, prefer it (e.g. a scripted bash that
   cannot be spawn-invalidated — `command: "true"`/`"false"` builtins
   still spawn; a `write_file`-only script avoids the class entirely —
   judgment per test, named in the commit message). The
   `tests/loopd_spoof_guard.rs` (and sibling loopd fixture) 30s
   verdict-deadline legs: readiness-based waiting where the fixture can
   expose readiness, else the invalidation seam — the 30s constant
   itself does NOT increase (req 4).
3. **`wait_for_stub_dump` visibility** (src/mcp_serve.rs test module +
   the delegate tests' twin helper if present — check
   src/delegate/tests/*): the helper's deadline assert names the
   observed spawn/launch outcome (the server's isError payload or the
   spawn error) when the dump never lands — "stub never wrote X AND the
   launch returned isError: <text>" vs "launch reported pid N but no
   dump in 10s". No deadline increase; the 10s stays (a live spawn
   writes in milliseconds — a >10s miss is a failed spawn, and the
   assertion must SAY which).
4. **Zero timeout/grace/slack constant increases** (same evidence rule
   as T151: `git diff -U0` grep in the commit message; NEW liveness
   fences only, >=10x multiples stated).
5. **Non-vacuousness pins**: (a) the seam retries ONLY on the
   enumerated markers — a scripted real-regression attempt panics
   un-retried on attempt 1 (RED-proof: flip a marker string, the pin
   dies); (b) exhaustion wording names the marker class and attempt
   count exactly.

## Acceptance

- `cargo build`, `cargo clippy --all-targets -- -D warnings`, `cargo test`
  green.
- Full-suite repetition matrix, completed by the implementer OR handed
  back explicitly: >=6 consecutive default-parallelism runs green
  (one under induced `yes`-spinner load) + >=2 with `--test-threads=4`.
  Under CONTINUED pressure the invalidation retries must be OBSERVED
  (progress lines) and bounded — a matrix run with retries visible and
  green counts as green; a run red on a NON-spawn leg fails the matrix.
- The commit message reports the observed invalidation counts per run.

## Out of scope

- Production run_shell behavior (is_error on spawn failure is CORRECT —
  a real run must see the error).
- run_shell timeout values, driver bash-timeout knobs.
- A test-threads default change (masked, not fixed — T31 doctrine).
- Anything still green from T151 (dead_port, serialized timing domain).
