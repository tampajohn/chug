# T55 — `.chug/driver.lock`: same-cwd mutual exclusion for concurrent `chug run`s

One concern: nothing in the harness stops two `chug run` processes from
sharing one cwd, where they interleave-corrupt `.chug/transcript.jsonl`,
`.chug/events.jsonl`, and `LEDGER.md`. Today the only guards are doctrine
(META-SPEC: "Children MUST NOT share your cwd") and loopd's single-driver
skip — which T53 just found dead on the production host. The harness
should enforce its own state-file exclusivity.

## Repo context

- The driver appends to `.chug/transcript.jsonl` (every message),
  `.chug/events.jsonl` (every event), and `LEDGER.md` (on
  `update_ledger`) under `--cwd`. Two drivers in one cwd splice each
  other's streams: T7's rotation protects `--resume` from FOREIGN
  sessions at startup, but nothing protects a LIVE session from a
  concurrent one.
- Cycle-24 eval I1: loopd's `pgrep`-based skip is dead on this host —
  after T53 it is `ps`-based, but a supervisor-level guard still can't
  stop a manual second `chug run` launched by hand in the repo. This row
  is the in-harness layer (defense-in-depth: doctrine → loopd skip →
  this lock).
- Pattern precedents in-tree: T50's pidfile guard in `loopd.sh`
  (refuse only a LIVE holder, stale-pid fall-through); T28's
  `reap_and_alive` seam (liveness behind a testable function); T20's
  `build_info::resolve_head` (best-effort runtime probe, every failure
  leg → a defined fallback, never aborts the run); T3/T7's run-startup
  path in `src/driver.rs` (fresh-run archive/rotate decisions) where the
  lock acquisition will live.
- LOOP-SPEC hard rule: an interactive `chug chat` does NOT block a cycle
  ("shared `.chug/` appends interleave harmlessly") — so chat must never
  acquire, refuse, or remove the lock.

## Requirements

1. **Acquisition (run mode only).** `chug run` (including `--resume`)
   acquires `.chug/driver.lock` at startup — after the `.chug/` directory
   exists and BEFORE the first transcript rotation/append (T3/T7
   startup path), so rotations are serialized too. The lock file contains
   the acquirer's pid (first line) and may carry a start epoch or argv
   hint on later lines — keep the format trivially parseable (`pid` alone
   on line 1 is the contract; anything after is informational).
2. **Holder status.** On finding an existing lock, read its pid. The
   holder counts as ALIVE only when BOTH: (a) `kill(pid, 0)` succeeds
   (process exists — same libc call delegate uses), AND (b) the pid's
   current argv still names a chug binary — checked via
   `ps -o command= -p <pid>` output containing `chug`, NEVER via `pgrep`
   (cycle-24 eval I1: pgrep persistently fails to enumerate some process
   trees on macOS; ps sees them). Leg (b) is the PID-reuse guard: a stale
   lock whose pid now belongs to an unrelated process must NOT refuse.
3. **Refusal.** Holder alive → print to stderr a message naming the
   holding pid and the manual remedy
   (`if you know that run is gone, remove .chug/driver.lock`) and exit
   non-zero (use the existing startup-error exit-code convention in
   `src/main.rs`). No override flag — the remedy is deleting the file.
4. **Reclaim.** Holder dead (kill ESRCH), argv mismatch (pid reused),
   unreadable/malformed/empty lock file, or lock pid unparseable → the
   acquirer overwrites the lock and continues. Reclaim NEVER refuses;
   every failure leg of the status probe degrades to reclaim, not to
   abort (T20 never-fail discipline) — EXCEPT the explicit alive+chug
   double-positive of req 2/3.
5. **Release.** The lock is removed best-effort on every normal exit
   path (goal accepted, abort, natural stop, goal-gate rejection loop
   end). SIGKILL leaves a stale lock BY DESIGN — the next acquirer
   reclaims it via req 4; document this tradeoff in the module comment.
6. **Chat exemption.** `chug chat` never acquires, never refuses, and
   never removes `.chug/driver.lock` (LOOP-SPEC: chat does not block).
7. **`--resume`.** Identical acquisition path: the aborted predecessor is
   dead, so its stale lock is reclaimed transparently (req 4) — resume
   never blocks on its own predecessor.
8. **Seams.** The two impure probes — `pid_alive(pid) -> bool`
   (kill(pid,0)) and `argv_names_chug(pid) -> bool` (ps read) — live
   behind function seams so unit tests inject fakes (T28/T20 style);
   `holder_status(lock_contents, alive_fn, argv_fn) -> HolderStatus` is a
   pure function covering the full decision matrix. Probes that error
   (ps missing, spawn failure) map to "not alive" (reclaim) — never
   abort.
9. **README.** Autonomous-mode section gains ONE bullet (user-visible
   behavior): a run refuses to start in a cwd already driven by a live
   run, names the holding pid, and the remedy is removing the stale lock;
   chat never blocks.

## Tests

- Pure-matrix unit tests over `holder_status`: no lock → acquire; dead
  pid → reclaim; alive + non-chug argv → reclaim (PID-reuse leg); alive +
  chug argv → refuse; malformed pid → reclaim; empty file → reclaim;
  probe errors → reclaim.
- Real-process legs (no injection): spawn a `sleep` (alive, argv lacks
  chug) → reclaimed; kill it → dead-pid reclaimed. (A real chug-argv
  holder needs no live LLM: the pure matrix already covers the refuse
  leg; the real-process tests pin the impure probes' wiring.)
- Release legs: a scripted-LLM driver test (existing `ScriptedLlm`
  pattern in driver.rs tests) that runs to goal-acceptance asserts the
  lock is gone afterward; a second scripted run in the same cwd acquires
  cleanly (resume-class reclaim of a missing lock).
- Run-path integration pin: a scripted run that finds a lock held by a
  live `sleep` starts successfully (reclaim), and its events/transcript
  writes proceed — proving the check sits before the appends without
  blocking them.
- Chat exemption: a structural pin (grep-style, the
  `tests/no_compile_time_manifest_dir.rs` pattern) that the lock
  acquisition call site lives in the `run` path, not the chat path —
  or, if the code shape makes that cleaner, a direct unit test that the
  chat startup never creates the file.

## Acceptance

- Two concurrent `chug run`s in one cwd: the second exits non-zero with
  the pid + remedy on stderr; the first is unaffected (verify once by
  hand against scripted or real runs; the unit matrix is the durable
  proof).
- A stale lock (holder killed with SIGKILL) never blocks the next run.
- `cargo build`, `cargo clippy --all-targets -- -D warnings`, and
  `cargo test` all green; README bullet present and accurate.

check: cargo test
