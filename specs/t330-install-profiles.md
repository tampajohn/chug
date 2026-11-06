# T330 — install.sh profiles: `--profile <name>` with a documented LLM-proxy contract

estimate: ~100 changed lines (install.sh profile leg + profiles/ dir + docs + fixtures)

## Why

Q4 goal deliverable #1: "one-command install against tools-proxy." Today
install.sh gets the binary onto a box; getting the loop *talking to a
model provider* is hand-configured folklore. Every new adopter (company
pilot teams first, public users same shape) hits the same wall: which env
vars, which URL, which models. A profile contract turns adoption into one
flag — and keeps company-specific values OUT of the public repo (they ship
as operator-supplied profile files, not committed content).

## What

1. **Profile contract** (documented in `docs/install-profiles.md`): a
   profile is a file `profiles/<name>.env` (or `~/.chug/profiles/<name>.env`
   for private values) exporting exactly: `CHUG_LLM_BASE_URL`,
   `CHUG_LLM_API_KEY` (or `CHUG_LLM_API_KEY_CMD` for a fetch command,
   e.g. vault read), `CHUG_MODEL_REASONING`, `CHUG_MODEL_CODING`,
   optional `CHUG_LANGFUSE_*`. Nothing else is profile-owned.
2. **install.sh --profile <name>**: copies the profile into
   `~/.chug/profile.env`, prints the loopd launch line with the profile
   sourced. Missing keys → named error listing the absent ones, never a
   half-written profile.
3. **Ship one committed example**: `profiles/local-openai-compat.env`
   (localhost placeholder values + comments), so the contract is executable
   documentation. Company profiles live in `~/.chug/profiles/` on operator
   machines — the repo never carries endpoints or keys (T249 lint extends
   to `profiles/`).
4. **loopd integration**: loopd sources `~/.chug/profile.env` if present
   (already-shaped env wins — backward compatible, no flag day).

## Tests

- install.sh fixture battery: good profile installs + sources; missing-key
  profile fails with the named-key error; `--profile` without a file fails
  closed.
- loopd fixture: profile env overrides defaults; absent profile preserves
  today's behavior exactly.
- T249 lint: a profile containing a token-shaped string is caught.

## Out of scope

- Company/VA profile contents (operator-distributed, P005's kit).
- Secret storage backends beyond the `*_CMD` escape hatch.
- Model health probing at install time (loopd already degrades gracefully).
