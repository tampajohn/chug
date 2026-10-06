# DEPENDENCIES.md — audited external-dependency inventory

Audited 2026-10-02 (operator session, checkout `a2f0df3`): "sans the LLM
proxy, I want to audit the dependencies of chug." This doc is the record of
that audit — the eval corpus and future dependency reviews read it, and new
dependencies get checked against it (reuse > extend > create applies to
dependencies too). Derivation rule: this inventory is derived from `src/` and
`Cargo.toml`, not from memory; when code and doc disagree, fix one of them in
the same commit.

## The one hard dependency: the LLM proxy

chug cannot loop without a Messages-API endpoint and credentials — the model
is the loop's fuel, and without `ANTHROPIC_AUTH_TOKEN`/`ANTHROPIC_API_KEY`
(process env or `~/.claude/settings.json` `env` block) the run refuses to
start (fail-closed, by design — there is nothing to degrade to). **Everything
else in this inventory degrades**: every service below is env-optional and
fire-and-forget, every spawned process failure surfaces as a tool error or a
degraded note, and the loop continues.

## Inventory

| dependency | kind | required-for | env knobs | failure mode | first-added-by |
|---|---|---|---|---|---|
| anyhow 1 | crate | error handling on every fallible path | — | in-process (compile-time) | birth `f911488` |
| clap 4 (derive) | crate | CLI parsing (`run`/`chat`/`plan`/`mcp-serve`/…) | — | in-process | birth `f911488` |
| crossterm 0.28 | crate | TUI terminal I/O (`--tui`, `chat`) | — | in-process | TUI era `76ac019` |
| glob 0.3 | crate | `glob` tool, `tgrep` path narrowing, hook + permission matchers | — | in-process | TUI era `76ac019` |
| ratatui 0.29 | crate | TUI dashboard (`--tui`) | — | in-process | TUI era `76ac019` |
| reqwest 0.12 — `blocking` + `json` + `rustls-tls`, default-features **off** | crate | ALL HTTP: LLM API, Langfuse, layad judge + notify sink, `web_fetch`, `web_search`, MCP streamable HTTP | — | rustls-only (no openssl) — what keeps the linux-aarch64 cross build feasible | birth `f911488` |
| serde 1 + serde_json 1 | crate | config + wire formats (events.jsonl, transcript, mcp.json, hooks/permissions config) | — | in-process | birth `f911488` |
| sha2 0.10 | crate | SHA-256 integrity echoes (`goal_sha256`, `plan_sha256`) — ecosystem standard, comparable with external `shasum -a 256` | — | in-process | T115 |
| libc 0.2 (cfg(unix) only) | crate | process-group spawn + group-kill (bash-tool timeout) | — | in-process | bash-group-kill fix `602c992` |
| tempfile 3 | dev-dep | test scaffolding | — | test-only | birth `f911488` |
| serde_yaml 0.9 | dev-dep | `tests/release_workflow.rs` parses the GHA workflow | — | test-only | T100 |
| LLM endpoint + credentials | service | every model call — the loop's fuel | `ANTHROPIC_BASE_URL` / `ANTHROPIC_API_KEY` / `ANTHROPIC_AUTH_TOKEN` | **fail-closed**: no credentials → run refuses; unreachable → per-call error → abort path | birth `f911488` |
| Langfuse v3 (self-hosted) | service | traces/generations/spans, outcome + iteration scores | `LANGFUSE_HOST` / `LANGFUSE_PUBLIC_KEY` / `LANGFUSE_SECRET_KEY` (fallbacks `~/.langfuse-keys-chug`, `~/.langfuse-keys`) | unset → silently off; delivery failure counted + ignored — telemetry never changes run behavior | SPEC-8 |
| `chug daemon` (baked-in judge, `daemon` feature) | crate-in-binary | `--risk-gate` bash classification provider (T204/F15 phase 1): `chug daemon` hosts the Laya model in-process (candle 0.11 + hf-hub 0.4 + tokenizers 0.22, optional feature — OFF in the default build, pinned zero-candle by `tests/daemon_feature_off.rs`) over a 0600 unix socket (`CHUG_HOME`/`CHUG_DAEMON_SOCK`, default `~/.chug/daemon.sock`) | `CHUG_JUDGE` (default `daemon`; `http`/`off` escape hatches), `CHUG_LAYA_CHECKPOINT` (local dir or HF repo; first load downloads ~650MB into the HF cache) | fail-open unchanged: daemon absent/unreachable → command allowed, degradation logged (verdicts → `.chug/risk_verdicts.jsonl`); auto-spawn + stale-socket recovery; single-instance flock | T204/F15 (this slice) |
| layad judge (external, deprecated as judge) | service | layad notify sink (`/hook/notification` push-vs-silent) — and the `CHUG_JUDGE=http` escape hatch for the risk gate | `LAYA_URL` (default `http://127.0.0.1:8420`) | fail-open: judge down → command allowed, degradation logged (verdicts → `.chug/risk_verdicts.jsonl`); the notify sink degrades to a `notify_error` event | SPEC-3 `db6fea6`; judge role replaced by the daemon (T204) |
| Hugging Face Hub (huggingface.co or any HF-compatible host) | service | the baked-in daemon's judge-checkpoint downloads (`daemon` feature): the five-file RLAgent layout, cache-first into the standard HF cache; fine-tunes live in private org repos (the publish contract below) | `CHUG_LAYA_CHECKPOINT` (local dir, `org/model`, or `org/model@REV` — pinned revision), `HF_TOKEN` (private/gated repos; a read-scoped machine token — hf-hub 0.4.3 does NOT read it from env, chug passes it through), `CHUG_HF_ENDPOINT` (else `HF_ENDPOINT`) → Artifactory or any HF-compatible host; default stays public huggingface.co | fail-open (T190 shape): daemon absent/unauthed → command allowed, degradation logged; an auth failure (401/403) is ONE stderr line naming the fix + ONE `judge_checkpoint_auth_error` events note, never a retry storm | T205 |
| DuckDuckGo HTML | service | `web_search` (keyless default provider) | `CHUG_WEB_SEARCH_PROVIDER` / `CHUG_WEB_SEARCH_BASE_URL` | scrape breakage/rate-limit → tool error to the model, never a silent empty result; one attempt, no retry | T180 |
| arbitrary URLs | capability | `web_fetch` (bounded read-only GET — not a service, the same reqwest seam) | — | non-2xx/transport → tool error; one attempt, no retry | T37 |
| GitHub via plain `git push`/tag | service | delivery of work + releases | — | in-binary git calls degrade to notes; delivery (push/tag) blocks — git is the transport of record | T100 (releases), loop protocol (work) |
| chug-site repo (chug.sh) | service | the public stats/timeline/features publish (`scripts/site-sync.sh`, T98/T99) | `CHUG_SITE_DIR` (site clone path), `CHUG_SYNC_NOW` (date pin), `CHUG_SITE_SYNC_NO_PUSH` | **fail-closed (T217)**: repo inputs unreadable (TODO.md / .chug/loopd / git log) → the sync writes NOTHING, commits NOTHING, one named error, exit 4 — fallback zeros are never published over real stats (the 58c3a0b gutting); missing clone / rejected push stay best-effort (warn + exit 0) | T98, guarded T217 |
| `sh` | process | `bash` tool (`sh -c`), hooks | — | spawn failure → tool error, loop continues; hook failures fail open | birth `f911488` |
| `git` | process | banner checkout info, `collect` refs, `@path` completion, delivery (harvest/merge/push) | — | in-binary calls degrade (`head=` omitted, note instead of refs); delivery blocks | birth `f911488` |
| `ps` | process | stale driver-lock reclaim (T55), mcp-serve ownership re-derivation (T153) | — | mcp-serve legs fail closed: unresolvable leg → nothing signalled | T55 |
| `rg` → `grep -rn` | process | `grep` tool | — | `rg` missing → `grep` fallback; both missing → tool error naming the spawn failure | birth `f911488` |
| `osascript` | process | macOS notify sink (opt-in via `.chug/notify.json`) | — | delivery failure noted exactly once per run; run never affected | T190 |
| `.chug/mcp.json` servers | process | user-configured MCP children (stdio + streamable HTTP) | per-entry `env` map; `${VAR}` header expansion from the process env | per-server fail-soft: bad entry skipped with a note, dead server errors its calls — never aborts the run | SPEC-7 (stdio), SPEC-9 (HTTP) |
| python venv `~/models/laya/venv` (torch + transformers) + HF snapshot cache (`convaiinnovations/laya` @ 55cf4c4e) | operator-host tooling — **NOT a chug build dep** (nothing in `src/` or `Cargo.toml` touches it; the cargo build never needs it) | F13 phase-2b distillation experiment ONLY — `scripts/distill_experiment.py` (T208) reads the decision corpus and trains the evaluation heads | `TRANSFORMERS_OFFLINE` / `HF_HUB_OFFLINE` (forced on by the script itself — no network fetch) | venv or snapshot absent → the script exits naming the venv path; the loop never depends on it (report + committed metrics are the product) | T208 (this slice) |

## Private fine-tune hosting — the T205 publish contract

Any laya fine-tune destined for an org-private HF org (or the interim
private `tampajohn/*` repos) is, before ANY consumer points at it:

1. **PRIVATE at creation** — org-membership gating is the mechanism, NOT
   HF's "gated" feature (public + approval workflow — wrong tool).
2. **Secret-scanned over its training corpus** (gitleaks-class), with the
   scan result recorded in the model card — the operator's spill history
   makes this a hard gate, not advice. The repo-wide equivalent lives at
   `tests/no_secret_spill.rs` (HF_TOKEN appears as a NAME, never a value).
3. **Revision-pinned by consumers** — `CHUG_LAYA_CHECKPOINT=org/model@REV`
   (a policy-affecting artifact must not move under a running fleet).

Base laya (`convaiinnovations/laya`, Apache 2.0, public) needs no gating;
mirroring it to `org/laya-base` is OPTIONAL (availability pinning
only). The full runbook — consumer config, the loopd env file, and the
operator checklist (PENDING the 2026-10-05 hosting decision) — is
`runbooks/laya-hf-hosting.md`.

Deliberately absent: **no database, no docker, no `gh` CLI at runtime** (gh
exists only inside the GHA release job), **no external config service**, no
telemetry SDK (Langfuse is raw reqwest posts). `.chug/mcp.json` entries are
user-configured, not repo dependencies — a repo-controlled mcp.json is
policy-gated (spawn blocked by deny rules before it can execute, T138).

## Environment surface (exhaustive, derived from `src/`)

Every variable read via `std::env::var`/`var_os` in `src/` (+ `build.rs`):

- `ANTHROPIC_BASE_URL`, `ANTHROPIC_API_KEY`, `ANTHROPIC_AUTH_TOKEN` — LLM
  proxy; process env wins over the `~/.claude/settings.json` `env` block
- `CHUG_BASH_TIMEOUT` — bash-tool cap, seconds (flag > env > 120s default)
- `CHUG_DELEGATE_BIN` — delegate child binary override (test seam)
- `CHUG_GIT_HASH` — build-time banner-hash override (reproducible builds)
- `CHUG_MAX_TOKENS` — per-request output cap (flag > env)
- `CHUG_MODEL` — model fallback when no `--model` flag
- `CHUG_STREAM` — `0` restores the non-streaming request path
- `CHUG_WEB_SEARCH_PROVIDER` — search provider override (valid: `duckduckgo`)
- `CHUG_WEB_SEARCH_BASE_URL` — DuckDuckGo endpoint override
- `CARGO_TARGET_DIR`, `CARGO_BUILD_TARGET_DIR` — never inherited by spawned
  children: scrubbed from every child env (T144); a delegate `env` map entry
  is the explicit override that wins
- `HOME` (unix) / `USERPROFILE` (windows) — settings + key-file resolution
- `PATH` — spawned-process resolution; also the MCP stdio baseline env
- `TMPDIR`, `LANG`, `LC_ALL` — the rest of the unix MCP stdio baseline env
  (T138, with `PATH` + `HOME` above; windows adds `SystemRoot`, `TEMP`,
  `TMP`, `USERPROFILE`, `APPDATA`, `LOCALAPPDATA`): a spawned stdio server
  gets the baseline plus its entry's `env` map, never the whole inherited
  environment (API keys do not leak into children)
- `LANGFUSE_HOST`, `LANGFUSE_PUBLIC_KEY`, `LANGFUSE_SECRET_KEY` — telemetry
- `LAYA_URL` — layad notify sink + the `CHUG_JUDGE=http` escape hatch
- `CHUG_JUDGE` — risk-gate judge client: `daemon` (default) | `http` | `off`
- `CHUG_DAEMON_SOCK` / `CHUG_HOME` — the baked-in judge daemon's 0600 unix socket location
- `CHUG_DAEMON_BIN` — loopd's judge-daemon binary override (T215): used when executable, else loopd falls through to the installed `~/.local/bin/chug` (probed via `daemon --help`) / a feature-on repo build; the daemon comes from the INSTALLED release binary on loop hosts — repo dev builds are clients only (T204 keeps them feature-lean), and with nothing daemon-capable the cycle-start ensure is skipped behind one log line (fail-open)
- `CHUG_LAYA_CHECKPOINT` — the daemon's model checkpoint (local dir, `org/model`, or `org/model@REV` — a pinned revision sha/tag; T205)
- `HF_TOKEN` — private/gated HF repo access for the daemon's checkpoint fetch: read-scoped, passed through to hf-hub (which reads only its cached token file on its own); the VALUE never enters a log, event, or error (tests/no_secret_spill.rs pins the repo to the name only)
- `CHUG_HF_ENDPOINT` (chug-specific, wins) / `HF_ENDPOINT` (standard, fallback) — point the checkpoint fetch at Artifactory or any HF-compatible host; hf-hub 0.4.3 ignores `HF_ENDPOINT` via `Api::new()`, so the override is applied by `hf_hosting` (T205)
- `CHUG_DAEMON_STUB` — the daemon lifecycle test seam (`chug daemon` serve mode, SHIPPING code path read at startup): `1` serves the real transport with NO model — /judge refuses outright (a stub must never fabricate classifications)
- `CHUG_LAYA_LIVE_PARITY` — test-only: `=1` enables the weights-loaded golden-parity tests (judge_model + daemon socket paths)

Adjacent, not chug-owned knobs: mcp.json header values expand `${VAR}` from
the process env at load time (`REMOTE_TOKEN` in tests is an example of a
user-chosen secret var, not a chug knob); delegate `env` keys must match
`^(CARGO_|CHUG_|RUST)[A-Z0-9_]*$` (T183 — a goal cannot rewrite
PATH/HOME/DYLD_* on a child); `loopd.sh` owns `LOOP_ORCH_MODEL`,
`LOOP_ROUTINE_MODEL`, `LOOP_REAPER`.

## Failure semantics — the spine

Three polarities, per dependency, pinned by tests: **fail-open** (hooks,
risk-gate judge, notify sinks, MCP config — a missing/broken optional thing
is zero behavior change plus at most one logged note); **fail-closed**
(policy surfaces — a permissions deny has no process to fall back through;
credentials; mcp-serve ownership legs; the chug-site publish's input guard,
T217 — unreadable repo inputs mean every computed stat is a fallback, so the
sync refuses to publish rather than push zeros over real stats); **blocking**
(git delivery and the LLM proxy call itself — the two steps whose failure is
the loop's failure). Telemetry drops, the judge degrades logged, git blocks.

## Adding a dependency

1. **Check this doc first.** Reuse > extend > create applies to dependencies:
   an existing crate row, std, or a hand-rolled fixed pattern beats a new
   dependency (T183 hand-rolled the one allowlist regex rather than add
   regex; T115 chose sha2 over inventing a hash).
2. **Fail-open unless it's a policy surface.** Optional niceties degrade and
   log; only things that must not depend on a spawned process (permissions
   deny rules) fail closed.
3. **Default-features off where feasible** (the reqwest rustls-only shape —
   no openssl in the build graph).
4. **Record the row.** The commit that adds the dependency adds its row here,
   with the TODO row that added it (`first-added-by`), and touches
   `Cargo.toml` + `Cargo.lock` together. A dependency without a row, or a row
   without a dependency, is drift — fix it in the same commit.

Out of scope here: SBOM/cargo-deny CI enforcement (a later row if the
operator wants it); changing any dependency is normal feature work, not this
doc's job.
