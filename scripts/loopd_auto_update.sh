# scripts/loopd_auto_update.sh — loopd.sh's opt-in self-update check (T352).
#
# CHUG_AUTO_UPDATE=1 in the profile (the flag loopd_profile_loader.sh
# allowlists) runs `chug update --check` ONCE PER DAY (the cheap-exit-era
# cadence; the Rust side pins CHECK_INTERVAL_SECS = 86400) and, when an
# update is available, applies it BETWEEN CYCLES — never mid-cycle: the
# supervisor is resident, and the swap must not touch a binary a running
# cycle is executing. The disposition line says so.
#
# Sourcing contract — all provided by the caller (loopd.sh sets them):
#   $STATE  the supervisor state dir (the cadence latch lives here)
#   $LOG    the supervisor log
#   ts()    the RFC3339 timestamp helper
# Written for loopd.sh's `set -euo pipefail` regime; defines no other state
# and executes nothing at source time (a pure function definition).
# Best-effort end to end: every leg returns 0 — an update check must never
# gate a cycle.
loopd_auto_update_check() {
  # Opt-in by construction: unset or 0 is a no-op (no check, no log line).
  [ "${CHUG_AUTO_UPDATE:-0}" = "1" ] || return 0
  # The installed binary, resolved at call time — PATH stubs (the fixtures)
  # shadow it, and the loop integration updates the INSTALLED chug, never
  # the repo's own target/release build.
  command -v chug >/dev/null 2>&1 || {
    echo "$(ts) auto-update: no chug on PATH — skipping (set CHUG_AUTO_UPDATE=0 to silence)" >> "$LOG"
    return 0
  }
  # The daily cadence latch: one check per CHECK_INTERVAL_SECS (86400s).
  # A completed check stamps the latch even when the check FAILED (a
  # wedged network retries tomorrow, not every cycle — the T325 shape
  # stamps only on child success; this is cadence control, not a work
  # product gate).
  local latch="$STATE/update-check.json" now last=0
  now=$(date -u +%s)
  if [ -f "$latch" ]; then
    last=$(awk '{ if (match($0, /"last_check_ts"[ \t]*:[ \t]*[0-9]+/)) { v = substr($0, RSTART, RLENGTH); sub(/.*:[ \t]*/, "", v); printf "%d", v; exit } }' "$latch" 2>/dev/null)
    case "$last" in ''|*[!0-9]*) last=0 ;; esac
  fi
  [ "$((now - last))" -ge 86400 ] || return 0
  printf '{"last_check_ts": %s}\n' "$now" > "$latch" 2>/dev/null || true
  # The check: --check is the machine surface — exit 0 current, 10 update
  # available, 2 error (T352's contract). One line per event in the log.
  local check_rc=0 check_out
  check_out="$(chug update --check 2>&1)" || check_rc=$?
  check_out="${check_out%%$'\n'*}"
  case "$check_rc" in
    0)
      echo "$(ts) auto-update: already current ($check_out)" >> "$LOG"
      ;;
    10)
      # The swap happens HERE, between cycles: no cycle is running inside
      # this while-body pass, so the update can never interleave with one
      # (the cycle-id capture in tests/update_cmd.rs asserts it). The
      # resident supervisor and any in-flight process keep their images;
      # new launches pick the new binary up at their next natural restart.
      echo "$(ts) auto-update: update available — applying between cycles (the resident supervisor never swaps a binary a running cycle is executing; the swap lands here and running processes keep their images until their next natural restart)" >> "$LOG"
      local apply_rc=0 apply_out
      apply_out="$(chug update 2>&1)" || apply_rc=$?
      apply_out="${apply_out%%$'\n'*}"
      if [ "$apply_rc" -eq 0 ]; then
        echo "$(ts) auto-update: applied — $apply_out" >> "$LOG"
      else
        echo "$(ts) auto-update: apply FAILED (rc=$apply_rc): $apply_out — best-effort, the next daily check retries" >> "$LOG"
      fi
      ;;
    *)
      echo "$(ts) auto-update: check failed (rc=$check_rc): $check_out — best-effort, the next daily check retries" >> "$LOG"
      ;;
  esac
  return 0
}
