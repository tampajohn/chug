# T50 — loopd re-execs itself between cycles when its own script changed

check: cargo test

## Context

`loopd.sh` supervises back-to-back LOOP-SPEC cycles. Merges to loopd.sh
currently activate only when the operator manually restarts the supervisor
— a documented tradeoff (T27's commit message: signalling a running
supervisor is unsafe because bash reads script files incrementally, so
in-place edits mid-execution can corrupt the running parse).

The cost is now observable, not theoretical (cycle-22 eval S3): the running
loopd (started 2026-09-26 00:43Z) predates THREE landed loopd.sh changes —
T36 (`--max-iters 120`→`160`), T46 (eval-digest refresh line), T47
(target-shared env prefix). Consequences in the live corpus: cycles launch
with the old 120-iteration budget (three consecutive margin-wraps at
116-119/120), the T46 digest was absent at cycle-22 start (regenerated
manually), and the supervisor hands no shared-cache env. Three reviewed,
validated, pushed changes sat dormant for 7 cycles. T47's own kimi
validator proved the mechanism live (export persistence across while-loop
iterations).

The fix applies design rule #1 one level up: the loop is code, not
conversation — a supervisor that cannot pick up its own landed code is a
human-in-the-loop dependency. The safe moment to re-read the script is
BETWEEN cycles, at the top of the while loop, where the incremental-read
hazard does not exist for a *freshly exec'd* process.

## Requirements

1. **Fingerprint at start.** Before the while loop, record a content
   fingerprint of the running script: `SELF_CKSUM="$(cksum "$ROOT/loopd.sh")"`.
   POSIX `cksum` is content-based (no mtime false positives from `touch`)
   and byte-identical across BSD/GNU for the same file content.
2. **Compare + exec at while-top.** As the FIRST statement inside the
   `while [ ! -f "$STOP" ]` loop body (before the single-driver check,
   before `cargo build`): if `cksum "$ROOT/loopd.sh"` differs from
   `SELF_CKSUM`, log one line to `$LOG`
   (`loopd: script changed on disk — re-exec (pid $$)`) and
   `exec "$ROOT/loopd.sh" run`. Placement guarantees the re-exec only ever
   happens BETWEEN cycles, never mid-cycle; `exec` replaces the process
   image, so the new process reads the new script from disk with zero
   incremental-read exposure. The STOP file wins naturally: the while
   condition is evaluated before the body, so a pending stop exits without
   re-exec'ing.
3. **Pidfile guard: same-pid pass.** Today's guard
   (`if [ -f "$PIDFILE" ] && kill -0 "$(cat "$PIDFILE")" ...` → refuse)
   would refuse the re-exec'd self, because `exec` preserves the pid and
   the pidfile already contains it. Amend the guard to refuse only when the
   pidfile's pid is alive AND different from `$$`; when it equals `$$`,
   fall through (re-write the pidfile, re-arm the EXIT trap — both already
   happen on the normal path). A stale pidfile (dead pid) keeps today's
   fall-through. A foreign live supervisor keeps today's refusal.
4. **Documented semantics, in the script's own comments:** (a) the
   consecutive-failure counter resets across a re-exec — intentional:
   failures against old code don't count against new code; (b) a re-exec
   is logged per requirement 2 so `.chug/loopd/loopd.log` always explains a
   budget/behavior change; (c) the operator's `loopd.sh stop` during a
   cycle still lands at the next while-condition check, re-exec or not.
5. **Keep every existing behavior otherwise byte-identical**: the
   single-driver check, `cargo build`, the T46 digest refresh, the T47
   env-prefix invocation (never a bare export — tests/shared_target_dir.rs
   pins this), the goal-complete grep, the 3-failure HALT, the 60s
   inter-cycle sleep.
6. **tests/shared_target_dir.rs adjacency:** that suite pins loopd.sh
   content (bare-export ban, per-carrier `count_eq` pins). If the new block
   changes any pinned count, update the pin in the SAME commit with a
   comment noting the T50 addition; the bare-export ban must remain green
   (the re-exec block contains no `export`).

## Tests

- New static-pin integration test file `tests/loopd_reexec.rs` (following
  the tests/shared_target_dir.rs static-content pattern) asserting loopd.sh
  contains, each at least once: the `SELF_CKSUM` record (a `cksum`
  invocation against `"$ROOT/loopd.sh"`), the compare-and-`exec
  "$ROOT/loopd.sh" run` block, the re-exec log line, and the same-pid
  guard clause (a test that the pidfile guard compares the pidfile pid
  against `$$`). Static pins — the suite must fail if the re-exec block is
  deleted or the guard regression is reverted.
- A full live loopd integration test is OUT of scope (spawning real cycles
  from the test suite is impractical; loopd changes have been static-pin +
  review + validator territory since T27/T47). The spec's live acceptance
  leg is post-merge observation (below).
- Full gates green: `cargo build`, `cargo clippy --all-targets --
  -D warnings`, `cargo test`, with the T47
  `CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared` export the
  goal carries. `bash -n loopd.sh` must pass (syntax check — include it in
  the impl child's own verification, not the cargo suite).

## Acceptance

- `cargo test` (the check above) passes from the worktree; the new pins are
  non-vacuous (impl child deletes the exec line once, watches the pin fail,
  reverts, records in ledger).
- `bash -n loopd.sh` clean.
- A manual smoke the impl child CAN run safely: copy loopd.sh + a fake
  3-line `chug` stub + fake `cargo`/`scripts/eval-digest.sh` shims into a
  scratch dir under /tmp, start it, append a comment to the copied
  loopd.sh, and observe the re-exec log line within one cycle — this is
  encouraged (record outcome in the child ledger) but NOT a hard gate,
  because shim fidelity is imperfect; the hard gates are the static pins +
  gates above. If attempted, the scratch dir is removed afterward and
  nothing under the repo's `.chug/` is touched by the smoke.
- Post-merge live leg (orchestrator/operator observability, not the
  child's check): the running supervisor logs the re-exec line within one
  cycle of this item's merge touching loopd.sh. The orchestrator records
  whether that happened in EVALUATION.md Outcomes at wrap (or the next
  eval does — honest either way).

## Notes

- Out of scope: signal-based reload (SIGHUP), in-place re-read of the
  running script (the incremental-read hazard this design avoids),
  systemd/launchd unit changes (launchd supervision, if any, is orthogonal
  — KeepAlive relaunches are not an activation mechanism, they are a crash
  mechanism).
- Why cksum over mtime: `touch loopd.sh` or a git checkout that preserves
  content must not trigger a spurious re-exec; content comparison makes
  re-exec exactly track real changes.
- The re-exec'd process re-runs `rm -f "$STOP" "$STATE/HALTED"` at startup
  (existing line): a HALTED marker from a prior crash streak is cleared by
  the re-exec — acceptable, because a re-exec only happens after a merge
  changed the code (new code, fresh start; documented per requirement 4).
  If the operator set STOP, the while-condition prevents the re-exec
  entirely, so STOP is never cleared by this path.
