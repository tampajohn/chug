# T153 fix-up: chug_cancel group_gone misreads EPERM as "group empty" (zombie race)

Row: T153 (F10 phase 3a — chug_cancel). This is the row recipe's post-forensics
fix-up: the orchestrator reproduced the wire test's dead-poll failure SOLO
(12/30 runs, 40%) and instrumented the cancel path (diagnostics since reverted).
Work to date is COMMITTED on branch `loop-t153` in this worktree
(3ae17a1 feature + ae0a684 spec check-line env fix). Implement ONLY this fix-up
on top; do not re-do the feature.

estimate: ~60 changed lines (one predicate + tests + sweep annotations)

## Forensics evidence (quoted; full file: .chug/forensics-t153-cancel-eperm-20260929.md in the MAIN repo)

Failing run, first tick of the cancel grace loop (pid/pgid = the stub leader):

```
tick pid=37308 pgid=37308 waitpid=0 (Resource temporarily unavailable (os error 35))
     killgrp=-1 (Operation not permitted (os error 1)) killleader=0 (...)
  PID    PPID  PGID   UID  STAT COMMAND
  37308  37307 37308  502  Z    <defunct>
```

- The group's SOLE member is the leader, already a zombie (Z/defunct).
- `waitpid(pid, WNOHANG)` returned 0: the leader was STILL RUNNING at the reap
  moment; it died in the microsecond window between the reap and the probe.
- On macOS, `kill(-pgid, 0)` on a group whose only member is our unreaped
  zombie child returns **-1 EPERM** — while per-pid `kill(zombie_pid, 0)`
  returns 0 (success). (The errno text beside `killleader=0` is stale from the
  prior call — the rc is the truth.)
- `group_gone`'s current shape `unsafe { libc::kill(-pgid, 0) != 0 }` treats
  ANY non-zero rc as "the group emptied" → EPERM is misread as EMPTY → the
  cancel returns `signaled: term` with the leader an unreaped zombie (parent =
  the mcp-serve server; nothing reaps it later). The wire test's bounded
  dead-poll (`poll_pid_state(pid, false)` — `kill(pid,0)==0` spins while the
  zombie exists) then times out at 5s. Solo-flaky ~40%; also the load-flake
  seen at the 17-way nextest landing.

## The fix (product, src/mcp_serve.rs)

`group_gone`: ONLY `rc == -1 && errno == ESRCH` means the group emptied.

- `rc == 0` → a live, signallable member → not gone.
- `rc == -1 EPERM` (macOS: the sole member is our own unreaped zombie) → the
  group still exists → NOT gone. The NEXT tick's `waitpid` reaps the leader
  (a zombie child is immediately reapable) and the probe turns ESRCH — the
  loop converges deterministically. Escalation to SIGKILL remains the backstop
  when the grace elapses (unchanged semantics for a TERM-ignoring fixture).
- Any other rc/errno (EINVAL etc.; pgid > 0 is guaranteed by leg (b)) →
  not gone (fail-safe: keep waiting inside the bounded grace).

Update the fn's doc-comment: the current text claims "a zombie group leader
keeps a bare `kill(-pgid, 0)` green forever" — that is FALSE on macOS (EPERM,
not green). Rewrite the comment around the ESRCH-only semantics and this
empirical host behavior, and keep the reap-before-probe rationale.

## Required RED-proven tests (tests-only, src-adjacent test module or tests/mcp_serve.rs)

1. **Probe-predicate table (the killing test).** Expose the tick's probe
   decision as a small pure seam (e.g. `fn group_gone_rc(rc: i32, errno:
   Option<i32>) -> bool`, private + `#[cfg(test)]`-visible is fine) and pin:
   `rc=0 → false`; `rc=-1, ESRCH → true`; `rc=-1, EPERM → false` (dies on the
   old `krc != 0` shape); `rc=-1, EINVAL → false`. RED-prove the EPERM leg
   against the pre-fix shape (temporarily revert → test fails → restore).
2. **Real-zombie probe leg.** Spawn a child with `process_group(0)` whose body
   exits immediately; do NOT reap it → a real zombie group leader. Probe the
   REAL `kill(-pgid, 0)` and assert the fixed decision is `false` (on this
   macOS host the rc is -1/EPERM; the assertion must also hold on a host where
   the rc is 0 — both are "not gone"). Then reap it (waitpid) and assert the
   probe turns ESRCH → decision `true`. This kills the "EPERM→gone" and
   "zombie stays a member forever" mutants and documents the host quirk.
3. **Convergence invariant.** With the same real zombie, loop `group_gone`
   (bounded, e.g. 20 iterations) and after it first returns true assert
   `kill(pid, 0) != 0` — the leader is REAPED, not left a zombie. This kills
   the "return true but leave the zombie" mutant (the original bug's
   observable shape).
4. **The wire happy-path leg goes deterministic.**
   `mcp_serve_chug_cancel_happy_path_over_the_real_wire` was failing 12/30
   solo pre-fix. Post-fix, run it 15× solo (`--test-threads=1`) and record
   15/15 green in the commit message. Do NOT touch its 5s deadlines
   (mechanism, not timeouts — T151 doctrine).

## Class sweep (REQUIRED — the finding names a CLASS: any non-zero kill(2) rc
read as the "gone/dead" answer, conflating EPERM with ESRCH)

Enumerate EVERY kill(2)-based liveness/existence probe in the repo (grep
`kill(` across src/ and loopd.sh). For each site: fix it under the class rule
or record a one-line justification in the commit message. Known instances to
adjudicate:

- `src/mcp_serve.rs` `group_gone` — THE bug (fix above).
- `src/delegate.rs` `process_alive` (via `reap_and_alive`): today
  `kill(pid,0) != 0` → "not alive", so a LIVE process we lack permission to
  signal (EPERM) reads as DEAD. Consumers: cancel leg (a) (a refusal is
  fail-safe but the text would say "no such process" for a live process —
  dishonest), delegate status/collect liveness renders (a live root-owned
  child would render dead). Fix direction: EPERM → the probe could not be
  completed — `reap_and_alive` returns `None` so cancel renders the existing
  fail-closed "liveness could not be probed" refusal, and status/collect
  render their not-probed/unknown posture. RED-prove with a live EPERM probe
  (a root-owned pid verified live first — probe `kill(1, 0)` from this uid and
  PIN whatever it actually returns after verifying empirically; if pid 1
  returns 0 on this host, find a live root pid via `ps -o pid,uid` and pin
  that). If every root pid probe returns 0 (macOS may allow signal-0
  existence checks), then EPERM is unreachable for per-pid probes on this
  host — record that finding and keep `process_alive` unchanged WITH the
  justification, but still make the zombie-after-reap semantics explicit.
  Either way the sweep verdict must state which it proved.
- `tests/mcp_serve.rs` `poll_pid_state` — test-side `kill(pid,0)==0` as alive;
  same conflation (EPERM reads dead). Document in a comment; no behavior
  change required (the test's targets are same-uid children).
- `loopd.sh` / T152 reaper — if it probes liveness with kill(1)-style rc
  checks, note the site in the sweep verdict; change only if it has the same
  EPERM-conflation with a live-consequence path.

## Constraints

- ONLY this fix-up: `src/mcp_serve.rs` (+ its test module or tests/mcp_serve.rs)
  and the `src/delegate.rs` sweep leg if adjudicated. NO timing-lock changes,
  NO record-wait changes (that is T151/T159 territory), NO spec/doctrine
  edits, NO TODO.md/LEDGER.md (orchestrator-owned), NO README/FEATURES edits
  (the feature docs landed with 3ae17a1).
- Keep cargo build + clippy + test green. Commit your work here (worktree
  cwd, branch loop-t153). Always commit ONLY from your worktree cwd; never
  run git add/commit with the main repo as cwd.
- export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared before
  every cargo command (shared warm cache — delegate cannot pass env).

check: cd /private/tmp/chug-loop-t153 && export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared && cargo clippy --release --all-targets 2>&1 | tail -5 && cargo test --release --test mcp_serve 2>&1 | tail -5
