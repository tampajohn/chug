# scripts/loopd_profile_loader.sh — loopd.sh's T330 profile loader.
#
# The T330 profile contract (docs/install-profiles.md): an operator profile
# is a KEY=VALUE file — install.sh --profile copies it to
# $HOME/.chug/profile.env — exporting EXACTLY the profile-owned keys, which
# the loader translates to the loop's native env BEFORE the model defaults
# in loopd.sh apply:
#   CHUG_LLM_BASE_URL      -> ANTHROPIC_BASE_URL   (src/auth.rs reads it)
#   CHUG_LLM_API_KEY       -> ANTHROPIC_AUTH_TOKEN (the literal form)
#   CHUG_LLM_API_KEY_CMD   -> ANTHROPIC_AUTH_TOKEN (a command whose stdout is
#                             the key — evaluated ONCE here, at supervisor
#                             startup; never per cycle, never logged)
#   CHUG_MODEL_REASONING   -> LOOP_ORCH_MODEL      (fresh-eval/judgment cycles)
#   CHUG_MODEL_CODING      -> LOOP_ROUTINE_MODEL   (routine/coding cycles)
#   CHUG_LANGFUSE_*        -> LANGFUSE_*           (optional observability)
# Nothing else is profile-owned: any other key is counted and skipped.
#
# Hygiene (the T205 gate): the file's CONTENTS never reach the log — only
# the path, a skipped-line COUNT, and the NAMES of missing required keys.
#
# Backward compatible (no flag day): an absent profile file is a silent
# no-op, and ALREADY-SHAPED ENV WINS — a native variable already set in the
# environment is never overwritten by its profile counterpart, so
# `ANTHROPIC_AUTH_TOKEN=x` plus a profile carrying CHUG_LLM_API_KEY=y keeps x.
#
# Sourcing contract — all provided by the caller (loopd.sh sets them right
# before the call):
#   $PROFILE_ENV_FILE  the profile to load (loopd.sh: CHUG_PROFILE_ENV,
#                      else $HOME/.chug/profile.env)
#   $LOG               the supervisor log — only the path, a skipped-line
#                      COUNT and missing-key NAMES may reach it
#   ts()               the RFC3339 timestamp helper
# Written for loopd.sh's `set -euo pipefail` regime; defines no other state
# and executes nothing at source time (a pure function definition).
loopd_profile_apply() { # <native-var-name> <profile-value> — explicit env wins
  if [ -z "${!1:-}" ]; then
    export "$1=$2"
  fi
}
loopd_load_profile() {
if [ -f "$PROFILE_ENV_FILE" ]; then
  _prof_skipped=0
  _prof_key=
  _prof_key_cmd=
  _prof_seen_base=
  _prof_seen_reasoning=
  _prof_seen_coding=
  while IFS= read -r _line || [ -n "$_line" ]; do
    _line="${_line%$'\r'}"
    case "$_line" in ''|\#*) continue ;; esac
    _line="${_line#export }"
    case "$_line" in
      CHUG_LLM_BASE_URL=*|CHUG_LLM_API_KEY=*|CHUG_LLM_API_KEY_CMD=*|CHUG_MODEL_REASONING=*|CHUG_MODEL_CODING=*|CHUG_LANGFUSE_*)
        _key="${_line%%=*}"
        _val="${_line#*=}"
        case "$_val" in
          \"*\") _val="${_val#\"}"; _val="${_val%\"}" ;;
          \'*\') _val="${_val#\'}"; _val="${_val%\'}" ;;
        esac
        case "$_key" in
          CHUG_LLM_API_KEY_CMD) _prof_key_cmd=$_val ;;
          CHUG_LLM_API_KEY)     _prof_key=$_val ;;
          CHUG_LLM_BASE_URL)    _prof_seen_base=1;    loopd_profile_apply ANTHROPIC_BASE_URL "$_val" ;;
          CHUG_MODEL_REASONING) _prof_seen_reasoning=1; loopd_profile_apply LOOP_ORCH_MODEL "$_val" ;;
          CHUG_MODEL_CODING)    _prof_seen_coding=1;  loopd_profile_apply LOOP_ROUTINE_MODEL "$_val" ;;
          CHUG_LANGFUSE_*)      loopd_profile_apply "${_key#CHUG_}" "$_val" ;;
        esac
        ;;
      *) _prof_skipped=$((_prof_skipped + 1)) ;;
    esac
  done < "$PROFILE_ENV_FILE"
  # The credential pair: precedence is an already-set ANTHROPIC_AUTH_TOKEN
  # (explicit env wins), then the literal CHUG_LLM_API_KEY, then the *_CMD
  # escape hatch — evaluated ONCE here at supervisor startup (the T47
  # lesson — no env mutation inside the cycle loop; a rotation is edit +
  # restart). A failing fetch aborts startup LOUDLY — a profile that cannot
  # produce a key would otherwise fail every cycle with a bare
  # no-credentials error.
  if [ -n "$_prof_key" ]; then
    loopd_profile_apply ANTHROPIC_AUTH_TOKEN "$_prof_key"
  elif [ -n "$_prof_key_cmd" ] && [ -z "${ANTHROPIC_AUTH_TOKEN:-}" ]; then
    if _prof_fetched="$(sh -c "$_prof_key_cmd")"; then
      loopd_profile_apply ANTHROPIC_AUTH_TOKEN "$_prof_fetched"
    else
      _prof_rc=$?
      echo "[loopd $(ts)] profile $PROFILE_ENV_FILE: CHUG_LLM_API_KEY_CMD failed (rc=$_prof_rc) — fix the fetch command or set CHUG_LLM_API_KEY; contents never logged" >> "$LOG"
      unset _line _key _val _prof_skipped _prof_key _prof_key_cmd _prof_seen_base _prof_seen_reasoning _prof_seen_coding _prof_fetched _prof_rc
      return 1
    fi
  fi
  # The count-only skipped line — the path and the COUNT, never a value byte.
  if [ "$_prof_skipped" -gt 0 ]; then
    echo "[loopd $(ts)] profile $PROFILE_ENV_FILE: $_prof_skipped non-allowlisted or malformed line(s) ignored (contents never logged)" >> "$LOG"
  fi
  # A hand-edited or truncated profile that lost a required key still
  # applies the rest, but the supervisor names what is missing — the
  # install-time validator (install.sh --profile) is the strict gate, this
  # is the fail-loud runtime echo of the same contract.
  _prof_missing=
  if [ -z "$_prof_key" ] && [ -z "$_prof_key_cmd" ]; then
    _prof_missing=" CHUG_LLM_API_KEY (or CHUG_LLM_API_KEY_CMD)"
  fi
  [ -n "$_prof_seen_base" ]      || _prof_missing="$_prof_missing CHUG_LLM_BASE_URL"
  [ -n "$_prof_seen_reasoning" ] || _prof_missing="$_prof_missing CHUG_MODEL_REASONING"
  [ -n "$_prof_seen_coding" ]    || _prof_missing="$_prof_missing CHUG_MODEL_CODING"
  if [ -n "$_prof_missing" ]; then
    echo "[loopd $(ts)] profile $PROFILE_ENV_FILE: missing required key(s):$_prof_missing — see docs/install-profiles.md (values never logged)" >> "$LOG"
  fi
  unset _line _key _val _prof_skipped _prof_key _prof_key_cmd _prof_seen_base _prof_seen_reasoning _prof_seen_coding _prof_missing
fi
}
