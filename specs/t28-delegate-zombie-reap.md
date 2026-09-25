# T28 — delegate `status` reaps zombie children (waitpid WNOHANG)

check: cargo test

## One concern

`delegate{action:"status"}` reports `alive: true` for children that
have already exited, because liveness is `kill(pid, 0)` and the
orchestrating chug (the child's parent) never reaps. Make `status`
reap its own exited children so `alive` tells the truth.

## Repo context

- `src/tools.rs`: `delegate_status` at ~729–756; the liveness probe
  `process_alive` at ~836–846 is `libc::kill(pid as i32, 0)` with an
  EPERM-means-alive leg. `render_status` (~901–909) prints
  `alive: true|false|unknown (no pid given)`.
- `delegate_launch` (~650) spawns the child DETACHED from the
  orchestrator's process group (`process_group(0)` + SIGHUP-IGN, T23)
  but the orchestrator remains the child's PARENT — so an exited child
  is a zombie until reaped, and `kill(pid, 0)` succeeds on zombies.
- Evidence (cycle-11 eval O2): all three cycle-10 children polled
  `alive: true` AFTER their events streams recorded `state: done,
  goal_seen: true` (status payloads at 21:22:47 pid 18744, 21:31:59
  pid 23703, 21:37:39 pid 27993 in
  `.chug/events-20260925-214135.jsonl`).
- `libc` is already a unix dependency (`Cargo.toml`
  `[target."cfg(unix)".dependencies]`) and `src/tools.rs` already calls
  `libc::kill`/`libc::signal` — `libc::waitpid` needs no new dep.
- Non-unix: the existing code is unix-flavored already; keep any new
  waitpid leg behind the same cfg conventions the file already uses.

## Requirements

1. In the `status` action's liveness path: BEFORE (or fused with) the
   `kill(pid, 0)` probe, attempt
   `libc::waitpid(pid as i32, &mut status, libc::WNOHANG)`.
   - Returns `pid` (>0): the child was ours and had exited — it is now
     REAPED; report `alive: false`.
   - Returns `0`: our child, still running → fall through to the
     existing probe (or report alive directly).
   - Returns -1 with `ECHILD`: not our child (foreign pid, or already
     reaped) → fall back to the existing `kill(pid, 0)` semantics
     (alive / not-alive / EPERM-alive) UNCHANGED.
   - Any other errno: fall back to the existing probe.
2. `launch` is untouched; `status` output shape is untouched (same
   fields, same `alive:` rendering); the only behavioral delta is that
   an exited OWN-child now reports `alive: false` instead of
   `alive: true` — and is reaped (a second `status` call reports
   `alive: false` via the ECHILD→kill leg, not an error).
3. No panic paths: every waitpid failure leg degrades to the current
   behavior.
4. Liveness-without-pid (`alive: unknown (no pid given)`) unchanged.

## Tests

New tests in `src/tools.rs`'s test module (the file already has
delegate tests incl. a real-`sleep` fixture and the test-process-self
liveness pin at ~1754):

1. **Own exited child reaped and reported dead**: spawn a real
   short-lived child via the same Command path launch uses (or a bare
   `Command::new("true")`/equivalent), wait for exit, call the
   status-liveness seam → `Some(false)`; a SECOND call still
   `Some(false)` (ECHILD→kill leg, no error).
2. **Own running child reported alive**: spawn `sleep 30`, seam →
   `Some(true)`; kill+reap it in test cleanup (no zombie leaks from
   the suite itself).
3. **Foreign/not-our-child pid keeps kill(pid,0) semantics**: a pid
   that exists but isn't our child (e.g. pid 1, or the test harness's
   parent) → same answer as pre-T28 (alive via EPERM/kill leg); a
   provably-dead never-was-our-child pid (spawn+reap fully via
   `child.wait()`, then probe) → `Some(false)`.
4. **No-pid leg unchanged**: `None` → `alive: unknown`.
5. Non-vacuousness: test 1 fails pre-T28 (kill(pid,0) on the zombie
   returns alive) — state this in a comment.

## Acceptance

- `cargo build`, `cargo clippy --all-targets -- -D warnings`, and
  `cargo test` green in the worktree.
- The five new tests present and green; no pre-existing test touched
  except where a pin asserted the OLD (lying) behavior — if one does,
  the diff must name it and the validator must agree the old pin was
  the bug.
- README: no change (internal truthfulness fix; the delegate bullets
  stay accurate).
