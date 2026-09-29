#!/usr/bin/env bash
# orphan-reaper.sh — T152: pre-cycle orphan-process reaper (fail-closed
# precision).
#
# Cycle-70 (T145-arc) found a 9.8-hour-old spinning test binary at 99% CPU
# (~590 CPU-minutes burned) left over from the T134-validation era — a T79
# parallel-mutant leg whose validator died mid-leg (budget abort), the shape
# of a child killed at the bash 120s cap whose cargo/test process group
# outlived it, or a removed /tmp/chug-loop-t*/ /tmp/chug-mut-* worktree's
# leftover process. Nothing reaped it, and the load it caused is the
# triggering condition of the T151 flake family. This sweep runs at the ONE
# safe instant — loopd.sh calls it immediately after the single-driver probe
# passes and BEFORE the build gate — and must NEVER run mid-cycle (children
# legitimately hold processes there; the invariant below does not hold).
#
# Sweep-point invariant: the probe just passed, so no driver, no
# orchestrator, and no delegate children can legitimately exist — every chug
# process of a cycle descends from the driver argv that probe would have
# matched — so any leftover loop-artifact process is definitionally orphaned.
#
# Needle (three legs, any qualifies):
#   • artifact leg: the process's executable path — resolved via
#     `ps -o comm=`, NEVER the argv[0] text (argv[0] is spoofable; comm is
#     the path recorded at exec) — is ABSOLUTE and contains both
#     `/target-shared` and `/deps/` (the loop's shared-cache artifact shape:
#     target-shared/, target-shared-mut-<k>/, target-shared-validate/, ...).
#   • removed-worktree leg: the argv contains `/tmp/chug-loop-t` or
#     `/tmp/chug-mut-` AND the named worktree directory no longer exists.
#   • cwd leg (the cycle-72 spec amendment): the process's CURRENT WORKING
#     DIRECTORY — resolved via `lsof -a -p <pid> -d cwd -Fn` (the portable
#     probe; macOS ships lsof at /usr/sbin, frequently off PATH, so the
#     helper resolves the binary itself) — is inside a `/tmp/chug-loop-t*`
#     or `/tmp/chug-mut*` path, EXISTING OR NOT. The kernel resolves the
#     cwd, and on macOS /tmp is a symlink to /private/tmp, so the needle
#     matches the resolved `/private/tmp/...` forms too — a /tmp-form-only
#     glob would be dead code on this host. This leg exists for the
#     cycle-72 live evidence: the T151 fix-up child's
#     `sh -c 'export … & echo launched'` hammer loop survived its budget
#     death by 1h49m — innocent argv, innocent comm, identified ONLY by its
#     cwd, spawning fresh cargo/test processes the whole time. The
#     sweep-point invariant covers the cwd leg's safety: at the pre-cycle
#     point no legitimate process can have a loop worktree cwd (no
#     orchestrator and no children exist), so cwd-in-worktree implies
#     orphaned by construction. (An operator merely inspecting a worktree
#     at cycle launch is the documented cost of that invariant;
#     LOOP_REAPER=0 is the opt-out.)
# All legs additionally require the process to be OUTSIDE the reaper's own
# process group — the supervisor and everything it could legitimately have
# spawned share that group, so a candidate inside it is never signalled —
# and NOT a current-cycle process: none can exist at the sweep point by
# construction (the sweep-point invariant above), so the group check is the
# defensive backstop, not the primary guard.
#
# Fail-closed: any candidate whose identity cannot be fully resolved (ps
# failure, non-absolute or unreadable comm, argv needle without a resolvable
# worktree directory, cwd that cannot be resolved — lsof failure, no cwd
# record, non-absolute path) is SKIPPED and logged `skip pid=<n> (unresolved)`
# — an ambiguous process is never killed. If the lsof binary itself cannot
# be found, the cwd leg is BLIND for that sweep: one logged line, and rows
# are never cwd-judged (a blind leg cannot kill; per-row cwd failures, where
# the tool ran and still could not answer, still skip + log). The kill leg
# is SIGTERM only, never
# SIGKILL: a TERM-resistant survivor is still identity-matched and the next
# cycle's sweep re-judges it, while a SIGKILL would be un-undoable and this
# sweep's whole premise is precision.
#
# Bound: at most LOOP_REAPER_MAX (default 64) process-table rows are
# EXAMINED per sweep — selected deterministically (rows sorted by pid
# ascending, windowed by a persisted page offset) — so a pathological table
# costs a bounded couple of ps calls instead of stalling cycle launch, while
# every row still gets its page on a later sweep. The cwd leg adds at most
# one lsof call per examined row (only rows not already identified by the
# first two legs are probed), so the sweep stays inside the same bound.
# Pagination state is
# best-effort: a lost page file merely resets to page 1 (coverage, never
# safety, depends on it).
#
# Opt-out: LOOP_REAPER=0 disables the sweep (logged, one line).
# Dry-run: LOOP_REAPER_DRY_RUN=1 identifies candidates and logs `would-term`
# judgments without signalling anything (the test seam).
#
# Output (one line per judgment + a summary; loopd.sh appends stdout to
# .chug/loopd/loopd.log, so every judgment is in the supervisor's log):
#   orphan-reaper: term pid=<n> (<leg>: <evidence>)
#   orphan-reaper: would-term pid=<n> (<leg>: <evidence>)          [dry-run]
#   orphan-reaper: skip pid=<n> (unresolved | own process group |
#                              term failed rc=<rc>)
#   orphan-reaper: done (examined=<n> killed=<k> skipped=<s>)
#                   [would-term=<n> in dry-run]
set -u
export LC_ALL=C

ts() { date -u +%Y-%m-%dT%H:%M:%SZ; }

if [ "${LOOP_REAPER:-1}" = "0" ]; then
  echo "$(ts) orphan-reaper: disabled (LOOP_REAPER=0)"
  exit 0
fi

DRY_RUN="${LOOP_REAPER_DRY_RUN:-0}"
MAX="${LOOP_REAPER_MAX:-64}"
PAGE_FILE="${LOOP_REAPER_PAGE_FILE:-}"
case "$MAX" in ''|*[!0-9]*|0) MAX=64 ;; esac

# --- enumerate ---------------------------------------------------------------
# Captured, then processed — never a ps-to-X pipeline (the T137 house rule:
# an early-exiting reader must never SIGPIPE the enumeration into a flipped
# verdict). A failing enumeration is UNKNOWN, and unknown fails CLOSED: skip
# the sweep, say why, and let the next cycle retry.
ps_rc=0
ps_raw="$(ps -ax -o pid= -o command= 2>/dev/null)" || ps_rc=$?
if [ "$ps_rc" -ne 0 ]; then
  echo "$(ts) orphan-reaper: sweep skipped (ps enumeration failed rc=$ps_rc — fail-closed)"
  exit 0
fi

# Normalize to "pid<TAB>argv" rows (empty ps lines dropped), then sort by pid
# ascending — the deterministic page order.
norm="$(awk 'NF { pid=$1; sub(/^[ \t]*[^ \t]+[ \t]*/, ""); printf "%d\t%s\n", pid, $0 }' <<<"$ps_raw")"
rows_rc=0
rows="$(sort -t "$(printf '\t')" -k1,1n <<<"$norm")" || rows_rc=$?
if [ "$rows_rc" -ne 0 ]; then
  echo "$(ts) orphan-reaper: sweep skipped (row sort failed rc=$rows_rc — fail-closed)"
  exit 0
fi
total=$(awk 'NF { c++ } END { print c + 0 }' <<<"$rows")
if [ "$total" -eq 0 ]; then
  echo "$(ts) orphan-reaper: done (examined=0 killed=0 skipped=0 — empty process table)"
  exit 0
fi

# --- deterministic pagination ------------------------------------------------
offset=0
if [ -n "$PAGE_FILE" ] && [ -f "$PAGE_FILE" ]; then
  off_raw="$(head -n 1 "$PAGE_FILE" 2>/dev/null)"
  case "$off_raw" in
    ''|*[!0-9]*) offset=0 ;;
    *) offset=$off_raw ;;
  esac
fi
if [ "$offset" -ge "$total" ]; then offset=0; fi
page="$(awk -v off="$offset" -v max="$MAX" 'NF && NR > off && NR <= off + max' <<<"$rows")"
page_rows=$(awk 'NF { c++ } END { print c + 0 }' <<<"$page")
if [ "$total" -gt "$MAX" ]; then
  echo "$(ts) orphan-reaper: bound: examining rows $((offset + 1))-$((offset + page_rows)) of $total (max $MAX per sweep; the remainder pages on later sweeps)"
fi
if [ -n "$PAGE_FILE" ]; then
  if [ $((offset + MAX)) -ge "$total" ]; then next_offset=0; else next_offset=$((offset + MAX)); fi
  printf '%s\n' "$next_offset" > "$PAGE_FILE" 2>/dev/null || true
fi

# --- own process group: the ONE absolute exclusion ---------------------------
own_raw="$(ps -o pgid= -p $$ 2>/dev/null)"
own_rc=$?
own_pgid="${own_raw//[[:space:]]/}"
if [ "$own_rc" -ne 0 ] || [ -z "$own_pgid" ]; then
  echo "$(ts) orphan-reaper: sweep skipped (cannot resolve own process group — fail-closed)"
  exit 0
fi

# --- the cwd-leg tool: resolve `lsof` ONCE, PATH-independently ----------------
# `lsof -a -p <pid> -d cwd -Fn` is the portable cwd probe (macOS + Linux),
# but /usr/sbin — where macOS ships it — is frequently NOT on PATH (observed
# on this host), so the standard absolute locations are fallbacks. Absent
# entirely → the cwd leg is BLIND for this sweep: one logged line, and rows
# are never cwd-judged (a blind leg cannot kill; legs 1-2 still judge).
LSOF_BIN="$(command -v lsof 2>/dev/null || true)"
if [ -z "$LSOF_BIN" ]; then
  for lsof_cand in /usr/sbin/lsof /usr/bin/lsof /bin/lsof; do
    if [ -x "$lsof_cand" ]; then
      LSOF_BIN="$lsof_cand"
      break
    fi
  done
fi
if [ -z "$LSOF_BIN" ]; then
  echo "$(ts) orphan-reaper: cwd leg unavailable (lsof not found — the cwd leg is blind this sweep)"
fi

# --- the sweep ---------------------------------------------------------------
examined=0
killed=0
skipped=0
would=0
while IFS= read -r row; do
  [ -n "$row" ] || continue
  pid="${row%%$'\t'*}"
  cmd="${row#*$'\t'}"
  # pid 0 means "my process group" to kill(2) — never let a normalized-garbage
  # row near it.
  case "$pid" in ''|0|*[!0-9]*) continue ;; esac
  examined=$((examined + 1))

  # Identity leg 1 (artifact): resolve the executable via `ps -o comm=` —
  # the kernel's exec path — and NEVER judge from argv[0] text (spoofable).
  # Must be absolute and carry the shared-cache artifact shape.
  comm_rc=0
  comm_raw="$(ps -o comm= -p "$pid" 2>/dev/null)" || comm_rc=$?
  comm=""
  read -r comm <<<"$comm_raw"
  comm_ok=0
  if [ "$comm_rc" -eq 0 ] && [ -n "$comm" ]; then comm_ok=1; fi

  leg=""
  evidence=""
  unresolved=""
  if [ "$comm_ok" -eq 1 ] \
     && [[ "$comm" == /* && "$comm" == *"/target-shared"* && "$comm" == *"/deps/"* ]]; then
    leg="target-shared-deps"
    evidence="$comm"
  fi

  # Identity leg 2 (removed worktree): argv carries the /tmp/chug-loop-t* or
  # /tmp/chug-mut-* needle. The named worktree directory is the needle's path
  # component — the loop's real naming is /tmp/chug-loop-t<N> (delegate
  # worktrees) and /tmp/chug-mut-<item>-<k> (T79 mutant legs), so the
  # component continues with [0-9t-] after the needle — EXTRACTED (leftmost
  # match) rather than trusted from the raw argv; a needle without a
  # well-formed directory is unresolvable.
  if [ -z "$leg" ]; then
    case "$cmd" in
      *"/tmp/chug-loop-t"*|*"/tmp/chug-mut-"*)
        wt="$(awk 'match($0, /\/tmp\/chug-(loop-t|mut-)[0-9t][0-9t-]*/) { print substr($0, RSTART, RLENGTH); exit }' <<<"$cmd")"
        if [ -z "$wt" ]; then
          unresolved=1
        elif [ ! -d "$wt" ]; then
          leg="removed-worktree-argv"
          evidence="$wt"
        fi
        ;; # worktree still exists → not a candidate on THIS leg (leg 3
           # below may still catch a process whose cwd sits inside it)
    esac
  fi

  # Identity leg 3 (cwd): the process's CURRENT WORKING DIRECTORY, resolved
  # via `lsof -a -p <pid> -d cwd -Fn` — the kernel-resolved cwd, `-Fn` the
  # machine-readable form whose `n` record carries the path. SWEEP-POINT
  # INVARIANT: at the pre-cycle point no legitimate process can have a loop
  # worktree cwd — no orchestrator and no children exist (the probe just
  # passed) — so cwd-in-worktree implies orphaned by construction. The
  # needle is the worktree family `/tmp/chug-loop-t*` / `/tmp/chug-mut*`,
  # EXISTING OR NOT (a removed worktree still shows as the process's cwd),
  # plus the macOS-resolved `/private/tmp/...` forms (lsof reports the
  # resolved path; /tmp is a symlink to /private/tmp on macOS, so a
  # /tmp-form-only needle would be dead code on this host). Fail-closed: a
  # cwd that cannot be resolved (lsof failure, no cwd record — zombie,
  # other-user, exited-between-enumeration-and-query — or a non-absolute
  # path) marks the row UNRESOLVED: skip + log, never kill. Only rows not
  # already identified by legs 1-2 are probed: candidacy is established
  # there, and the probe costs one lsof per examined row (bounded by MAX).
  if [ -z "$leg" ] && [ -z "$unresolved" ] && [ -n "$LSOF_BIN" ]; then
    cwd_rc=0
    cwd_raw="$("$LSOF_BIN" -a -p "$pid" -d cwd -Fn 2>/dev/null)" || cwd_rc=$?
    cwd=""
    while IFS= read -r cwd_line; do
      case "$cwd_line" in n?*) cwd="${cwd_line#n}" ;; esac
    done <<<"$cwd_raw"
    if [ "$cwd_rc" -ne 0 ] || [ -z "$cwd" ] || [ "${cwd#/}" = "$cwd" ]; then
      unresolved=1
    else
      case "$cwd" in
        /tmp/chug-loop-t*|/private/tmp/chug-loop-t*|/tmp/chug-mut*|/private/tmp/chug-mut*)
          leg="cwd-in-worktree"
          evidence="$cwd"
          ;;
      esac
    fi
  fi

  # Neither needle → never a candidate, never a judgment (the operator's own
  # ./target builds, editors, and everything else not matching the shape).
  if [ -z "$leg" ] && [ -z "$unresolved" ]; then
    continue
  fi

  # Fail-closed gate 1: unresolvable identity — never kill, always log.
  if [ -n "$unresolved" ]; then
    skipped=$((skipped + 1))
    echo "$(ts) orphan-reaper: skip pid=$pid (unresolved)"
    continue
  fi
  # Fail-closed gate 2: a kill candidate's identity must include a resolved,
  # ABSOLUTE executable — an argv needle alone is not identity enough to
  # signal a process, and a relative/bare comm (unreadable ps, Linux-style
  # basename) is exactly the ambiguity this sweep refuses to act on.
  if [ "$comm_ok" -eq 0 ] || [ "${comm#/}" = "$comm" ]; then
    skipped=$((skipped + 1))
    echo "$(ts) orphan-reaper: skip pid=$pid (unresolved)"
    continue
  fi
  # Fail-closed gate 3: not the reaper's own process group (the supervisor
  # and everything it could legitimately have spawned). At the sweep point
  # no current-cycle process exists by construction (see the invariant in
  # the header) — this gate is the defensive backstop for that invariant.
  grp_rc=0
  grp_raw="$(ps -o pgid= -p "$pid" 2>/dev/null)" || grp_rc=$?
  grp="${grp_raw//[[:space:]]/}"
  if [ "$grp_rc" -ne 0 ] || [ -z "$grp" ]; then
    skipped=$((skipped + 1))
    echo "$(ts) orphan-reaper: skip pid=$pid (unresolved)"
    continue
  fi
  if [ "$grp" = "$own_pgid" ]; then
    skipped=$((skipped + 1))
    echo "$(ts) orphan-reaper: skip pid=$pid (own process group)"
    continue
  fi

  # Judgment: SIGTERM only — no SIGKILL. A TERM-surviving process is still
  # identity-matched and the next cycle's sweep re-judges it (see header).
  if [ "$DRY_RUN" = "1" ]; then
    would=$((would + 1))
    echo "$(ts) orphan-reaper: would-term pid=$pid ($leg: $evidence)"
    continue
  fi
  kill_rc=0
  kill -TERM "$pid" 2>/dev/null || kill_rc=$?
  if [ "$kill_rc" -eq 0 ]; then
    killed=$((killed + 1))
    echo "$(ts) orphan-reaper: term pid=$pid ($leg: $evidence)"
  else
    skipped=$((skipped + 1))
    echo "$(ts) orphan-reaper: skip pid=$pid (term failed rc=$kill_rc)"
  fi
done <<<"$page"

if [ "$DRY_RUN" = "1" ]; then
  echo "$(ts) orphan-reaper: done (examined=$examined would-term=$would skipped=$skipped)"
else
  echo "$(ts) orphan-reaper: done (examined=$examined killed=$killed skipped=$skipped)"
fi
exit 0
