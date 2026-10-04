# scripts/loopd_env_loader.sh — loopd.sh's K7 env-file loader (T213).
#
# Extracted BYTE-IDENTICAL from the block that was inline in loopd.sh
# (formerly loopd.sh:55-81) so tests/loopd_env_loader.rs can drive the REAL
# lines via `bash -c '. scripts/loopd_env_loader.sh; loopd_load_env_file'`
# without spawning the whole supervisor (the T205 validator's M4 loader
# pin). The function body below is the moved block, byte for byte — do not
# reformat: a re-indent would break the byte-identity pin on the move.
#
# Sourcing contract — all three are provided by the caller (loopd.sh sets
# them right before the call):
#   $LOOPD_ENV_FILE  the env file to load (loopd.sh: CHUG_LOOPD_ENV, else
#                    $HOME/.chug/loopd.env)
#   $LOG             the supervisor log — only the path and a skipped-line
#                    COUNT may reach it, never the file's contents (the
#                    T205 hygiene gate)
#   ts()             the RFC3339 timestamp helper
# Written for loopd.sh's `set -euo pipefail` regime; defines no other state
# and executes nothing at source time (a pure function definition).
loopd_load_env_file() {
if [ -f "$LOOPD_ENV_FILE" ]; then
  _loopd_env_skipped=0
  while IFS= read -r _line || [ -n "$_line" ]; do
    _line="${_line%$'\r'}"
    case "$_line" in ''|\#*) continue ;; esac
    _line="${_line#export }"
    case "$_line" in
      CHUG_LAYA_CHECKPOINT=*|HF_TOKEN=*|CHUG_HF_ENDPOINT=*)
        _key="${_line%%=*}"
        _val="${_line#*=}"
        case "$_val" in
          \"*\") _val="${_val#\"}"; _val="${_val%\"}" ;;
          \'*\') _val="${_val#\'}"; _val="${_val%\'}" ;;
        esac
        # explicit process env wins over the file
        if [ -z "${!_key:-}" ]; then
          export "$_key=$_val"
        fi
        ;;
      *) _loopd_env_skipped=$((_loopd_env_skipped + 1)) ;;
    esac
  done < "$LOOPD_ENV_FILE"
  if [ "$_loopd_env_skipped" -gt 0 ]; then
    echo "[loopd $(ts)] env file $LOOPD_ENV_FILE: $_loopd_env_skipped non-allowlisted or malformed line(s) ignored (contents never logged)" >> "$LOG"
  fi
  unset _line _key _val _loopd_env_skipped
fi
}
