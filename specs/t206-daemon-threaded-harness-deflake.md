# T206 — deflake the daemon tests under the libtest threaded harness (fork-inheritance race)

check: export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared; touch src/*.rs tests/*.rs; cargo test --bin chug daemon::tests:: && cargo test --test daemon_lifecycle

## Repo context

Two daemon tests are flaky-red under the default libtest THREADED harness
(the spec `check:` line's config — `cargo test`, debug, threaded) while
green under nextest (process-per-test) and in isolation:

- `daemon::tests::daemon_lock_is_exclusive` (src/daemon.rs:1215): the third
  `acquire_lock_at` (after `drop(first)`) gets EAGAIN "held by <own pid>".
- `daemon::tests::stale_socket_connects_refused` (src/daemon.rs:1246): the
  connect classification is not ECONNREFUSED under the full 1194-test
  threaded bin run (the validator's addendum; its interfering co-tenant sits
  outside `daemon::tests`).

Evidence (cycle-95 eval reproduction on main @ dbce0e2): FAILS 3/3 under
`cargo test --bin chug daemon::tests:: -- --test-threads=4` with the
`judge_path` spawn family in the run set; PASSES with
`--skip judge_path --skip judge_contract --skip risk_gate` (7/7); PASSES in
isolation. One full-suite threaded run observed green — the failure is a
scheduling-dependent race, not a hard red.

Mechanism (diagnosed, cycle 95): `flock` locks ride the open file
description; a `Command::spawn` (fork+exec — `spawn_daemon`'s `pre_exec`
forces the fork path) forked while the lock test holds `first` inherits the
fd, and the description stays open until the child's exec closes it
(CLOEXEC). Under load (12 test threads + the documented ~48s
syspolicyd/dyld stalls on freshly built binaries, commit 492cc05) the
fork→exec lag exceeds the test's microseconds drop→reacquire window, so the
third acquisition races an inherited fd that is about to close. The lock
test over-asserts INSTANT reacquisition in a process that forks.

The third leg (kimi round-2 verdict finding 3, test hygiene):
`tests/daemon_lifecycle.rs:61` `Home::new()` uses `tempdir().keep()` —
every run litters `$TMPDIR` permanently — plus the pid-overwrite leak in
`second_daemon_exits_on_the_lock`.

Constraint: NO lock-semantics change. `acquire_lock_at`'s shipping behavior
(single-instance flock, LOCK_NB, pid line) is correct and pinned; the
second-acquisition-refused leg of `daemon_lock_is_exclusive` must stay
exact (one immediate attempt, must refuse). The fix is test-hardening.

estimate: ~90 changed lines (src/daemon.rs `mod tests` +
tests/daemon_lifecycle.rs; no shipping-code change expected — if diagnosis
proves a shipping fix is required, STOP and re-spec).

## Requirements

1. `daemon_lock_is_exclusive`: the post-drop reacquisition becomes a
   bounded retry — poll `acquire_lock_at` on a ~10s deadline with ~25ms
   backoff, asserting acquisition EVENTUALLY succeeds (what the test means:
   once every holder — including transient fork children — is gone, the
   lock is free). The live-holder refusal leg (second acquisition) is
   untouched and immediate. A comment names the fork-inheritance mechanism.
2. `stale_socket_connects_refused`: diagnose the actual transient connect
   error under the full-suite load (print/capture it during a failing run —
   e.g. run the full bin suite in a loop until red), then harden the leg
   the same bounded-retry way against that named transient. The comment
   names the observed transient error(s).
3. Sweep finding 3: `tests/daemon_lifecycle.rs` drops `.keep()` (tempdirs
   clean up on drop) and the `second_daemon_exits_on_the_lock` pid
   overwrite is guarded (the stale pid file from daemon one cannot be read
   as daemon two's pid).
4. RED-proof: for each changed leg, name the mutant that must still fail —
   e.g. a mutant `acquire_lock_at` whose holder never releases must make
   the bounded retry time out RED (not silently pass); a mutant that
   always classifies connect errors as refused must stay RED. Run each
   mutant and record the kill in the commit message or PR notes.

## Tests

The changed tests themselves; mutant kills per req 4. No new test files.

## Acceptance

- 20 consecutive `cargo test --bin chug daemon::tests:: -- --test-threads=4`
  runs green (the repro config), interleaved with normal load.
- 3 consecutive full threaded `cargo test` runs green.
- `cargo nextest run --release` green (no regression under the gate runner).
- clippy `--all-targets -- -D warnings` clean (default features; the daemon
  tests compile default — the feature-gated legs untouched).
