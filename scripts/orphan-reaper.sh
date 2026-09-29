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
# Needle (two legs, either qualifies):
#   • artifact leg: the process's executable path — resolved via
#     `ps -o comm=`, NEVER the argv[0] text (argv[0] is spoofable; comm is
#     the path recorded at exec) — is ABSOLUTE and contains both
#     `/target-shared` and `/deps/` (the loop's shared-cache artifact shape:
#     target-shared/, target-shared-mut-<k>/, target-shared-validate/, ...).
#   • removed-worktree leg: the argv contains `/tmp/chug-loop-t` or
#     `/tmp/chug-mut-` AND the named worktree directory no longer exists.
# Both legs additionally require the process to be OUTSIDE the reaper's own
# process group — the supervisor and everything it could legitimately have
# spawned share that group, so a candidate inside it is never signalled.
#
# Fail-closed: any candidate whose identity cannot be fully resolved (ps
# failure, non-absolute or unreadable comm, argv needle without a resolvable
# worktree directory) is SKIPPED and logged `skip pid=<n> (unresolved)` — an
# ambiguous process is never killed. The kill leg is SIGTERM only, never
# SIGKILL: a TERM-resistant survivor is still identity-matched and the next
# cycle's sweep re-judges it, while a SIGKILL would be un-undoable and this
# sweep's whole premise is precision.
#
# Bound: at most LOOP_REAPER_MAX (default 64) process-table rows are
# EXAMINED per sweep — selected deterministically (rows sorted by pid
# ascending, windowed by a persisted page offset) — so a pathological table
# costs a bounded couple of ps calls instead of stalling cycle launch, while
# every row still gets its page on a later sweep. Pagination state is
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
        ;; # worktree still exists → a live worktree's process, not a candidate
    esac
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
