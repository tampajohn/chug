# SPEC 6 — default endpoint/auth from Claude settings

Today chug requires `ANTHROPIC_BASE_URL` + a credential env var. Add a
fallback: read them from the `env` block of `~/.claude/settings.json` so
chug works with zero environment setup for Claude Code users.

## Resolution order

For each of base URL and credentials, first non-empty wins:

1. Process env: `ANTHROPIC_BASE_URL` / `ANTHROPIC_AUTH_TOKEN` /
   `ANTHROPIC_API_KEY` (existing behavior).
2. `~/.claude/settings.json` → top-level `env` object → same keys.
3. Built-in default for base URL only: `https://api.anthropic.com`.

- Read the settings file LAZILY on first API-client construction, parse with
  serde_json (existing dep), cache the result (OnceLock or OnceCell — no new
  deps if avoidable). Missing file, unparseable JSON, or missing keys →
  treat as absent and continue down the chain — never error on the file
  itself.
- Values from any source are trimmed; empty-after-trim counts as absent.
- Values from the file are trimmed; empty-after-trim counts as absent.
- Home resolution: `std::env::var("HOME")` (unix) / `USERPROFILE` (windows);
  don't add a dep for this alone.
- Error message when NO credential is found after the full chain must mention
  both sources: `no credentials: set ANTHROPIC_AUTH_TOKEN or
  ANTHROPIC_API_KEY (env or ~/.claude/settings.json env block)`.
- The existing rule stands: send `x-api-key` when the API-key var is set,
  `Authorization: Bearer` when the token var is set, both if both.
- Never log credential values; if a debug line names the source, say
  "claude settings" not the value.

## Files

- `src/auth.rs` — new: `resolve_endpoint() -> (base_url, Credentials)` with a
  `resolve_endpoint_with(env: &dyn Fn(&str)->Option<String>, home: &Path)`
  pure core for tests, settings-file loader behind it.
- `src/api.rs` — `Client::new()` (and chat/run config paths) consume it
  instead of reading env directly.

## Tests (temp files, no network, no real $HOME)

- env-only, file-only, env-wins-over-file, file-wins-over-builtin-default,
  missing file, malformed JSON file, empty-string values treated absent,
  no-credential error message names both sources, caching (second resolve
  doesn't re-read the file — inject a counting loader).

## Acceptance

- Gates clean: build / clippy -D warnings / test.
- With env unset and a populated `~/.claude/settings.json`, `chug run`
  against the proxy works with NO exported variables.
- With env set, behavior identical to today.
