# Install profiles — one flag from installed binary to talking model

`install.sh` puts the `chug` binary on a box. A **profile** tells the loop
*which model provider to talk to* — base URL, credentials, the two models —
so adoption is one flag instead of hand-configured folklore:

```bash
install.sh --profile <name>
```

## The contract

A profile is a shell-style env file named `profiles/<name>.env`. It
exports EXACTLY these keys — nothing else is profile-owned:

| key | required | the loop uses it as |
|-----|----------|---------------------|
| `CHUG_LLM_BASE_URL` | yes | `ANTHROPIC_BASE_URL` — an endpoint speaking the Anthropic Messages API (your LLM proxy, an OpenAI-compat gateway that fronts it, or api.anthropic.com) |
| `CHUG_LLM_API_KEY` | one of the two | `ANTHROPIC_AUTH_TOKEN` — the literal key (sent as `Authorization: Bearer …`) |
| `CHUG_LLM_API_KEY_CMD` | one of the two | `ANTHROPIC_AUTH_TOKEN` — a shell command whose **stdout** is the key (a vault/op one-liner); evaluated once at supervisor startup, its value is never logged |
| `CHUG_MODEL_REASONING` | yes | `LOOP_ORCH_MODEL` — the fresh-eval / judgment orchestrator model |
| `CHUG_MODEL_CODING` | yes | `LOOP_ROUTINE_MODEL` — the routine queue-working (coding) model |
| `CHUG_LANGFUSE_HOST`<br>`CHUG_LANGFUSE_PUBLIC_KEY`<br>`CHUG_LANGFUSE_SECRET_KEY` | optional | `LANGFUSE_HOST` / `LANGFUSE_PUBLIC_KEY` / `LANGFUSE_SECRET_KEY` — Langfuse tracing (all three or none; see SPEC-8) |
| `CHUG_AUTO_UPDATE` | optional | the opt-in self-update flag (T352): `1` makes loopd run `chug update --check` once per day BETWEEN cycles and apply an available update there (never mid-cycle); unset/`0` keeps today's no-auto-update behavior |

Model values are FULL provider-routed ids exactly as `--model` takes them
(there is no alias layer — whatever you write goes verbatim into the API
request body).

## Where profiles live

- `~/.chug/profiles/<name>.env` — **private values live here.** Operator-
  distributed company profiles (the onboarding kit) land in this directory;
  the file may carry real endpoints and keys because it never leaves the box.
- `profiles/<name>.env` — **repo checkouts.** The repo carries exactly one
  committed example, `profiles/local-openai-compat.env` (localhost
  placeholders + comments): executable documentation of the shape. The repo
  NEVER carries company endpoints or keys — the secret-spill and
  internal-info lints scan `profiles/` and fail the tree on a hit.

`install.sh --profile <name>` resolves the name in that order (a private
file wins over a repo file), then copies it to `~/.chug/profile.env`
(mode 600).

## Validation — never a half-written profile

Before anything is written, install.sh parses the profile and fails closed,
naming what is wrong:

- a required key missing → the error lists every absent key by name;
- neither `CHUG_LLM_API_KEY` nor `CHUG_LLM_API_KEY_CMD` → both named (the
  contract takes exactly one);
- both of them set → named conflict (comment one out);
- any non-profile-owned key → named (the loopd loader would silently skip
  it; install refuses so the typo surfaces at install time);
- a non-`KEY=VALUE` line (comments and blank lines are fine) → named;
- `--profile` with no matching file → the error lists every place searched.

A failed validation installs nothing and never writes `~/.chug/profile.env`.

## What the loop does with it

`loopd.sh` loads `~/.chug/profile.env` ONCE at supervisor startup (override
the path with `CHUG_PROFILE_ENV`), translating the table above to the loop's
native env before the model defaults apply. Two compatibility rules, no flag
day:

- **Already-shaped env wins.** If you export `ANTHROPIC_BASE_URL`,
  `ANTHROPIC_AUTH_TOKEN`, `LOOP_ORCH_MODEL` or `LOOP_ROUTINE_MODEL`
  yourself, the profile's counterpart is ignored — the file never overrides
  an explicit environment.
- **Absent profile is a silent no-op.** No `~/.chug/profile.env` and
  everything behaves exactly as before the profile contract existed.

With a literal `CHUG_LLM_API_KEY` in the file install.sh prints a note
pointing at the `CHUG_LLM_API_KEY_CMD` form, which keeps the key out of the
file entirely (fetch-once at startup). A profile whose `_CMD` fetch fails
aborts the supervisor startup loudly, naming the fetch — a profile that
cannot produce a key would otherwise fail every cycle with a bare
no-credentials error.

## Launching

install.sh prints the launch line after a `--profile` install; it is just:

```bash
./loopd.sh    # sources ~/.chug/profile.env (profile <name>)
```

For a one-off `chug run` outside the supervisor, source it yourself:

```bash
set -a; . ~/.chug/profile.env; set +a   # then translate per the table, or use loopd
```

(One-off runs are the folklore path; the supervisor is the supported one.)

On macOS, install.sh can also wire the launchd jobs so the loop survives a
reboot instead of relying on the operator's plist dance. `install.sh
--with-daemon` installs the chugd job (`scripts/com.tampajohn.chugd.plist`
into `~/Library/LaunchAgents`, bootstrapped in the gui domain; an
already-bootstrapped job is kickstarted on re-runs, so the leg is
idempotent) and prints the socket path (`~/.chug/daemon.sock`) with the
fail-open note: the risk gate fails OPEN — an unreachable daemon never
blocks a run, it only costs the semantic judge layer. `install.sh
--with-loopd` is the same shape for the loop supervisor, but ONLY inside a
repo whose loop scaffold exists (`chug loop init` has run): it writes a
`com.tampajohn.chug-loopd.plist` whose `ProgramArguments` run the installed
binary's `chug loop run` against THAT repo (logs under its `.chug/loopd/`);
elsewhere it prints the init hint and skips. Both legs are macOS-only — on
Linux they print the systemd TODO note and continue.

## Example

`profiles/local-openai-compat.env` in the repo — copy it to
`~/.chug/profiles/`, fill in your proxy's values, then:

```bash
install.sh --profile local-openai-compat
```
