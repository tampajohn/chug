# T144 — goal-gate check + driver-spawned shells must not inherit CARGO_TARGET_DIR (cross-worktree foreign-test execution)

check: cargo test

estimate: ~120 changed lines (src ~40, tests ~70, docs ~10)

## Repo context

Twice in cycles 66–67 a `goal_complete` check ran a FOREIGN worktree's test
binary (decision `d1790632587-1`, reproduced live in the T135 round-1
validator's goal-reject `d1790640546-3`). Mechanism:

- `loopd.sh:319` launches every orchestrator with
  `CARGO_TARGET_DIR="$ROOT/target-shared"` as a per-invocation env prefix,
  so every delegate-spawned child inherits it.
- The goal-gate check runs via `tools::run_shell(ctx.cwd, command, …)`
  (src/driver.rs:1649, after T139's gate chain), and `run_shell`
  (src/tools.rs:1031) spawns `sh -c` with the driver process's FULL
  inherited env — no scrub.
- Cargo's artifact filename excludes the checkout path (the T52 lesson):
  same package+profile+features → the SAME artifact filename in every
  worktree. A shared target dir is last-builder-wins, so a `cargo test`
  goal gate in worktree A can execute a test binary compiled from
  worktree B's source. The observed legs were false-RED; the symmetric
  false-GREEN leg (the gate runs another checkout's green tests and
  accepts) is silent.
- The bash tool has the same shape (same `run_shell`): a child that
  forgets its goal-carried `export CARGO_TARGET_DIR=<role-keyed>` builds
  into the shared dir and collides the same way (the class T52 fixed for
  orchestrator-side gates, still open inside children).

T52/T57 role-keyed dirs covered orchestrator gates, validators, and impl
builds — but NOT the in-driver spawn boundary. This row closes that gap.

Invariant: **a shell the driver spawns must never inherit a target dir
that other checkouts also write.** Explicit per-command prefixes inside
the command string (`CARGO_TARGET_DIR=x cargo test`, or the goal-carried
`export … && cargo …`) are unaffected and remain the warm-path mechanism.

## Requirements

1. Scrub `CARGO_TARGET_DIR` AND `CARGO_BUILD_TARGET_DIR` (the alias) from
   the environment of every shell `run_shell` spawns (one scrub point
   covers the bash tool AND the check harness — both go through
   `run_shell`). After the scrub a bare `cargo test` builds worktree-local
   (`<cwd>/target`), which is content-correct by construction.
2. Sweep for any OTHER `Command` spawn site in src/ that runs cargo or a
   shell with the inherited env (delegate.rs:315 spawns the child chug
   binary — its env inheritance is how the variable reaches children at
   all). The delegate launch MUST also scrub both variables from the
   child env (children get correct-by-default local builds; the
   goal-carried export still selects the role-keyed warm cache when they
   follow instructions). List every spawn site found in the commit
   message with its verdict (scrubbed / fixed-argv-no-cargo / N/A).
3. Keep the T9/T139 honesty text truthful: the goal_rejected env note
   (src/driver.rs ~1561) gains one clause naming the scrub (the check
   runs with `CARGO_TARGET_DIR` removed; set it inside the `check:` line
   itself if a shared cache is wanted — that is the operator's explicit
   choice, not an inheritance accident).
4. README gains one sentence in the Quickstart `check:` paragraph naming
   the scrub. LOOP-SPEC/META-SPEC are NOT edited (their per-invocation
   env prefixes are inside command strings and keep working).
5. Cold-build tradeoff is accepted and stated in the commit message: a
   goal-gate `cargo test` in a worktree now builds `<cwd>/target` cold on
   first use (~minutes, bounded by CHECK_TIMEOUT_SECS=600 — measured
   headroom: the chug dep tree cold debug build is well under it on this
   host; the child may note the measured cold-gate wall time in its
   commit message).

## Tests

- RED first: a test that seeds `CARGO_TARGET_DIR` in the process env (or
  passes it via a `run_shell`-level seam), runs a `run_shell` command
  that prints `${CARGO_TARGET_DIR:-UNSET}`, and asserts the output is
  `UNSET` — fails pre-fix. Same leg for `CARGO_BUILD_TARGET_DIR`.
- A leg proving an explicit in-command prefix still wins:
  `CARGO_TARGET_DIR=/tmp/xyz run_shell("echo $CARGO_TARGET_DIR")` prints
  `/tmp/xyz` even when the process env has a different value.
- Delegate-launch leg (test seam already exists: `CHUG_DELEGATE_BIN`
  lock, src/delegate.rs:305): with the variable set in the test env, the
  spawned stub records its env; assert both variables are absent. Use a
  serialization guard if the env lock requires it (the existing pattern).
- A goal-gate-shaped integration leg: a spec whose `check:` echoes the
  variable into a file; run the driver with the variable set; assert the
  file records it unset. (May share the driver's existing test harness.)

## Acceptance

- `cargo build && cargo clippy --all-targets -- -D warnings && cargo test`
  all green in the worktree.
- Every spawn site in src/ enumerated in the commit message with its
  scrub verdict.
- Validator REQUIRED (src/driver.rs + src/tools.rs + src/delegate.rs are
  all on the core list). Mutation candidates: revert the scrub (must die
  on the RED leg), scrub only one of the two variables (must die),
  over-scrub breaking the explicit-prefix leg (must die).
