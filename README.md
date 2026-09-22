# chug

Autonomous coding harness. Given a spec and a goal, it keeps on chugging.

Design rule #1: **the loop is code, not conversation.** The model never decides
whether to continue — the driver does. Goal and progress live in files on disk,
re-read every iteration, so context trimming can never kill the run.

## Quickstart

```bash
cargo build
# Zero setup if you have Claude Code configured: chug reads
# ANTHROPIC_BASE_URL / ANTHROPIC_AUTH_TOKEN / ANTHROPIC_API_KEY from
# ~/.claude/settings.json when the process env doesn't have them.
# (Process env always wins; default base URL is api.anthropic.com.)

chug run --spec SPEC.md --goal "Build X and make the check pass" \
  --model anthropic-system.ai.kimi-k3 --max-iters 40 --max-minutes 120

chug run --tui ...   # same, with the live dashboard
chug run --resume    # continue an aborted run from .chug/transcript.jsonl
chug chat            # interactive mode (TUI)
chug ledger          # print current LEDGER.md
```

Put a `check: <shell command>` line in your spec — `goal_complete` is only
accepted when the check exits 0.

## Interactive mode (`chug chat`)

A conversational session in the TUI: type a request, chug works it with tools,
returns to idle, repeat. Natural stops end the turn; budgets are per turn.

- **Always-on input dock** — submit while idle = new objective, while working =
  steering note (`[operator] …`, consumed at the next iteration boundary)
- **`@file` attachments** — `@src/main.rs` expands the file into your message
  (dirs → listings, missing → inline note); Tab-completes paths
- **Tab autocomplete** — `/commands` and `@paths` (git ls-files-backed,
  substring-ranked, candidate strip above the dock)
- **`/spec` `/goal` `/check` `/model` `/budget` `/ledger` `/quit` `/help`**
- **Esc** interrupts the current turn, `q` quits from idle

## Autonomous mode (`chug run`)

- **Startup banner** — `chug run` / `chug chat` print one stderr line at
  start: `chug <version> (<commit>) cwd=… spec=… model=…`, so a stale
  binary is visible at a glance. The commit is baked in at build time by
  `build.rs` (`unknown` outside a git checkout; `CHUG_GIT_HASH` overrides)
- **Anti-stall kick** — if the model stops without `goal_complete`, the driver
  injects "consult the ledger, continue" and keeps going
- **LEDGER.md** — external memory the model updates each iteration; injected
  into every turn, so transcript trimming never loses progress. A fresh
  `chug run` never inherits a previous session's ledger: a non-seed
  LEDGER.md is archived to `.chug/LEDGER-<timestamp>.md` and the run starts
  from the seed (`--resume` keeps it, chat sessions share it)
- **Verification** — `goal_complete` re-runs the spec's `check:` command;
  failure rejects the claim and the loop continues
- **Stuck tripwire** — 3 identical consecutive tool errors → abort, ledger
  intact, resumable
- **Transcript trimming** — old tool outputs collapse to `[trimmed]` past a
  token estimate; the ledger carries durable state. A fresh `chug run`
  rotates a non-empty `.chug/transcript.jsonl` to
  `.chug/transcript-<timestamp>.jsonl` before its first append, so
  `--resume` never splices foreign sessions into context
- **Events log** — the driver appends its structured event stream to
  `.chug/events.jsonl`, one JSON object per line (`jq`-mineable): run start
  (the banner fields: version/commit/model/spec/cwd/mode, once per run or
  chat session), one line per iteration with cumulative tokens,
  tool results (ok/is_error/duration_ms, ≤200-char previews), verification
  commands, goal verdicts, aborts. Best-effort telemetry: a write failure
  warns once on stderr and never affects the run. Fresh runs rotate a
  previous log to `.chug/events-<timestamp>.jsonl` alongside the transcript

## TUI (`--tui`)

Activity stream (model text + tool calls), live LEDGER.md panel, status bar
(model, iteration/budget, elapsed, cumulative tokens). `i` opens the steering
line (chat mode has the dock instead), `q` = graceful operator abort,
panic-safe terminal restore.

## Tools

`read_file`, `write_file`, `edit_file` (+`replace_all`), `bash`, `grep`,
`glob`, `list_dir`, `update_ledger`, `goal_complete`.
All paths sandboxed to `--cwd`. `bash` runs in its own process group —
timeouts SIGKILL the whole group, so orphaned grandchildren can't wedge the
driver (120s default; `--bash-timeout` / `CHUG_BASH_TIMEOUT` overrides).

## Risk gate (`--risk-gate`)

Every `bash` command is classified by a [Laya](https://github.com/convaiinnovations/laya)
judge server (`LAYA_URL`, default `http://127.0.0.1:8420`) as
destructive/risky/safe before executing. `destructive` p≥0.5 is blocked with
an error the model can see and route around; `allow destructive` in a steering
note disables the gate for the run. Fail-open if the judge is down. Verdicts
logged to `.chug/risk_verdicts.jsonl`.

## MCP servers (`mcp.json`)

chug consumes tools from MCP servers (tools capability) over **stdio** or
**streamable HTTP**. Claude Code-compatible config, first found wins:
`--mcp-config <path>` → `./mcp.json` → `~/.config/chug/mcp.json`
(`--mcp-off` disables).

```json
{"mcpServers": {
  "<name>":   {"command": "...", "args": ["..."], "env": {"K": "V"}},
  "<remote>": {"url": "https://mcp.example.com/mcp", "transport": "http",
               "headers": {"Authorization": "Bearer ${MCP_TOKEN}"}}
}}
```

An entry with `url` is remote (streamable HTTP); `transport` defaults to
`"http"`. `${VAR}` in header values expands from the process env at load
time. Per-server fail-soft: a bad entry (missing `${VAR}`, unknown
transport, unreachable server) is skipped with a note in
`.chug/mcp-<name>.log` — it never aborts the run.

Server tools appear as `mcp__<name>__<tool>` alongside the builtins.
Per-server fail-soft at runtime too: a stdio server that dies mid-run errors
its calls without killing the run; a remote server that refuses connection
retries 3× (1s, 2s, 4s) then returns a tool error. Timeouts mirror stdio:
connect 10s, first-byte 30s, per-call total 60s.

Remote specifics (SPEC-9): JSON-RPC over POST; `Mcp-Session-Id` captured and
replayed; notifications expect `202`; SSE response streams are read until the
matching-id response; a server-pushed request gets a JSON-RPC
`method not found` reply and notifications are dropped; the GET listen stream
reconnects with jittered backoff (1s→30s cap) and `Last-Event-ID` replay for
the life of the run. Stdio servers spawn in their own process groups and are
group-killed on every exit path. MCP tools bypass the laya risk gate (which
judges bash only). No config anywhere = byte-identical behavior.

## Langfuse observability (optional)

Traces to a self-hosted Langfuse v3 when configured — off with zero cost
otherwise. Config: `LANGFUSE_HOST` + `LANGFUSE_PUBLIC_KEY` +
`LANGFUSE_SECRET_KEY` (env wins; falls back to `~/.langfuse-keys-chug`, then
`~/.langfuse-keys`).

- **Trace** per run / chat session, **generation** per LLM call (usage incl.
  `cache_read_input_tokens` → the Langfuse model registry computes cost),
  **span** per tool call
- **Events**: goal accepted/rejected, aborts, risk-gate verdicts, steering
- **Scores**: `outcome` (completed|aborted|budget|stuck) + `iterations`
- **Fire-and-forget**: bounded queue + background flusher, exit-drain on all
  paths; any delivery failure is counted and ignored — telemetry never
  changes run behavior

## Self-hosting specs

- `META-SPEC.md` — chug orchestrating child chug runs (git-worktree-per-round
  protocol; how SPEC-5 was implemented)
- `SELF-SPEC.md` — continuous self-improvement: chug writes improvement specs,
  maintains a TODO ledger, works items within budget
- `SPEC-*.md` — feature specs, each written for (and mostly implemented by)
  chug itself

## Development

```bash
cargo build && cargo clippy --all-targets -- -D warnings && cargo test
```

All three must stay green. Layout: `src/{api,driver,eventlog,events,tools,tui,chat,
attach,complete,riskgate,mcp,mcp_http,sse,observ,auth,ledger,transcript,build_info}.rs`
(+ `main.rs`; `build.rs` only bakes the git commit into the startup banner).
