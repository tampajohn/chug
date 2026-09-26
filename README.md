# chug

Autonomous coding harness. Given a spec and a goal, it keeps on chugging.

Design rule #1: **the loop is code, not conversation.** The model never decides
whether to continue — the driver does. Goal and progress live in files on disk,
re-read every iteration, so context trimming can never kill the run.

## Quickstart

```bash
cargo build
cargo install --path .   # puts the chug binary on PATH (~/.cargo/bin)
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
  `build.rs` (`unknown` outside a git checkout; `CHUG_GIT_HASH` overrides).
  When the cwd's own worktree HEAD resolves at runtime, the line also names
  the checkout: `… head=<branch>@<short>` — worktree children run the
  main-tree binary, so `head=` (the cwd checkout) and the baked commit (the
  binary) can legitimately differ, and that difference is the signal. Not a
  repo, or git missing: the `head=` field is omitted and the line is
  unchanged (the banner never fails the run)
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
- **Budget-low warning** — when ≤8 iterations, ≤5 minutes, or ≤50,000 tokens
  remain, the loop injects a one-shot `chug: budget low — N iteration(s),
  M minute(s), K token(s) remain` user message (one shot per budget kind) so
  the model reprioritizes toward committing, gates, and bookkeeping before
  the abort at the loop top. The token count appears only when a token
  budget is set
- **Truncated-output advisory** — when a response comes back truncated at
  the API output-token ceiling (`stop_reason=max_tokens`), the loop injects
  a `chug: output truncated …` user message naming the chunking remedy
  (write_file the first chunk, then append with `edit_file` or a bash
  heredoc). Fires on every truncation — no one-shot latch — and each
  injection records one `output_truncated` line in the events log
- **Token budget** — `--max-tokens N` (run and chat) caps the run's
  cumulative input+output tokens — the axis iteration/wall-clock budgets can
  miss (a cheap watch-and-wait loop burns neither while racking up tokens).
  Crossing the ceiling aborts at the loop top naming the exhausted budget
  (`budget: N tokens`)
- **Driver lock** — a `chug run` refuses to start in a cwd a live run
  already drives: it names the holding pid on stderr and exits non-zero; if
  you know that run is gone, remove `.chug/driver.lock` and start again. A
  stale lock (dead holder, or its pid recycled by a non-chug process) is
  reclaimed transparently — `--resume` never blocks on its own predecessor —
  and the lock is released on every normal exit. `chug chat` never takes the
  lock: chat never blocks a run
- **Abort output** — every abort prints the freshest LEDGER.md and the model
  in use, plus a resume line naming the current model:
  `resume: chug run --spec <spec> --goal "<goal>" --cwd <cwd> --resume [--model <other>]  (current model: <model>)`.
  Budget deaths (iterations, wall-clock, or tokens) also name the exhausted
  budget (`budget: 40 iterations`), so a model that keeps dying on budget can be
  swapped manually: `chug run --resume --model <other>`. The run's cumulative
  token cost is printed too (`tokens: <input> in / <output> out (cumulative)`)
  — on goal-complete output as well, right after the summary line — so a
  wrapped run's spend is visible without mining `.chug/events.jsonl`
- **Transcript trimming** — old tool outputs collapse to `[trimmed]` past a
  token estimate; the ledger carries durable state. A fresh `chug run`
  rotates a non-empty `.chug/transcript.jsonl` to
  `.chug/transcript-<timestamp>.jsonl` before its first append, so
  `--resume` never splices foreign sessions into context
- **Events log** — the driver appends its structured event stream to
  `.chug/events.jsonl`, one JSON object per line (`jq`-mineable): run start
  (the banner fields: version/commit/model/spec/cwd/mode — plus the
  checkout's `head_branch`/`head_commit` when the cwd's HEAD resolves at
  runtime, `null` when it doesn't — and the configured budget ceilings —
  `max_iters`, `max_minutes`, `max_tokens` as `null` when unset), one line
  per iteration with cumulative tokens, tool results (ok/is_error/duration_ms, ≤200-char
  previews — error results keep a tail-anchored ≤2000-char window, so the
  failing test's name or error block at the end of the output is on record),
  verification commands, goal verdicts, budget-low warning
  injections (with the remaining counts at fire time), output-truncated
  advisories (one `output_truncated` line per injected advisory), and aborts
  (with the dying model and, on budget deaths, the exhausted budget). Best-effort
  telemetry: a write failure warns once on stderr and never affects the run.
  Fresh runs rotate a previous log to `.chug/events-<timestamp>.jsonl`
  alongside the transcript

## TUI (`--tui`)

Activity stream (model text + tool calls), live LEDGER.md panel, status bar
(model, iteration/budget, elapsed, cumulative tokens). `i` opens the steering
line (chat mode has the dock instead), `q` = graceful operator abort,
panic-safe terminal restore.

## Tools

`read_file` (`offset`/`limit` page past the 2000-line cap), `write_file`,
`edit_file` (+`replace_all`), `bash`, `grep`,
`glob`, `list_dir`, `update_ledger`, `goal_complete`, `delegate`, `web_fetch`.
All paths sandboxed to `--cwd` (`delegate` and `web_fetch` are
the two documented exceptions — `delegate`'s absolute `cwd`/`spec` target child
worktrees by design; `web_fetch` is network, not filesystem). `bash` runs in its own process group —
timeouts SIGKILL the whole group, so orphaned grandchildren can't wedge the
driver (120s default; `--bash-timeout` / `CHUG_BASH_TIMEOUT` overrides).

`delegate` — launch or observe a bounded child `chug run` (e.g. in a git
worktree). Two actions: **`launch`** spawns a detached child (`--spec`,
`--goal`, `--model` required; `--max-iters`/`--max-minutes` optional,
defaults 40/35; `--max-tokens` optional — the child's cumulative
input+output token ceiling, omitted = no token ceiling; `resume: true`
optional to continue the child's aborted run instead of starting fresh)
against an absolute `cwd` you prepared, appends its stdout+stderr to
`<cwd>/.chug/delegate.log`, and returns immediately with the child `pid`
and the log/events paths — it never waits on the child. **`status`**
reports the child's liveness (when you pass the `pid`), a summary of its
`.chug/events.jsonl` — the child's latest run segment (state,
`last_iteration` + `max_iters`, budget-low / goal / abort flags with the
abort reason) — and the tail of its console log — instant polling never
blocks. Optionally pass `wait_secs` (status-only;
0/absent = instant, max 600) to collapse each idle wait window into one
blocking status call: it returns early when the child's iteration
advances, a verdict or budget-low flag appears, or its liveness flips to
dead (per-tool-call `last_event` churn renders at the deadline but never
wakes it), else at the deadline, and names the actual
elapsed seconds on a `waited:` line. `cwd` and `spec` must be absolute and
may target child worktrees outside your own `--cwd`; worktree creation,
building, harvest/merge, and killing the child stay with your `bash`.

`web_fetch` — read-only HTTP(S) GET with hard bounds: `http://`/`https://` only,
≤5 redirects, connect 10s / total 30s, output capped at `max_chars` (default
20,000; a request above the 100,000 ceiling is clamped, not rejected), HTML
tag-stripped to visible text, binary content types refused by name, and every
non-2xx status or transport failure returned as a tool error — one attempt, no
retry. Strictly less powerful than the `curl` already available through
`bash`; the value is the bounded, audited, token-safe surface (caps, text
extraction, `events.jsonl` previews) that a raw shell fetch doesn't give the
loop. Like `delegate`, it reaches outside the cwd sandbox by design — it is
network, not filesystem.

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

- `LOOP-SPEC.md` — **the one-command self-improvement loop**: evaluate
  (meta-meta) → generate the TODO queue → work it with child runs →
  adversarial validation → merge → wrap
- `META-SPEC.md` — chug orchestrating child chug runs (git-worktree-per-round
  protocol; how SPEC-5 was implemented)
- `META-META-SPEC.md` — the evaluator: reads the corpus (events.jsonl,
  ledgers, specs, code), writes EVALUATION.md + the next TODO rows/specs
- `FEATURES.md` — the standing capability roadmap (benchmarked vs Claude
  Code / Codex / unreal-agent); every evaluation pulls its top unworked
  item into the queue, so capability work is the default, not the exception
- `SELF-SPEC.md` — continuous self-improvement: chug writes improvement specs,
  maintains a TODO ledger, works items within budget
- `specs/` — work-item specs: `t<N>-*.md` (the machine queue, one per TODO
  row, guard-enforced) and `spec-*.md` (era-1 features: TUI, tools/risk-gate,
  interactive, auth, MCP, Langfuse — each written for, and mostly implemented
  by, chug itself). `specs/archive/` holds superseded one-offs
- Layout rule: **root is doctrine** (the files above — protocols you launch
  with), **`specs/` is work** (anything a TODO row or feature round points
  at)

## Continuous mode (`loopd.sh`)

The self-improvement loop runs *continuously* — cycles back-to-back, no
human per-phase prompting. Each cycle's wrap (TODO.md, EVALUATION.md,
specs/) is the next cycle's input; phases chain through files.

```bash
nohup ./loopd.sh > /dev/null 2>&1 &   # start (detached)
./loopd.sh status                     # liveness + recent cycle activity
./loopd.sh stop                       # exits after the current cycle
```

Per cycle: kimi-k3 orchestrates LOOP-SPEC (evaluate or skip per freshness,
work the queue — bugs > robustness > **features** > DX > perf — glm-5-3-flash
children implement, kimi validates adversarially, auto-push per item).
Before each cycle the supervisor refreshes `.chug/eval-digest.md` via
`scripts/eval-digest.sh` — a deterministic (jq/awk-only, sub-second) digest of
the `.chug/events*.jsonl` corpus (per-file iterations, wall time, tool
distribution, error classes, token curve, aborts, budget-low fires, TODO
status counts, staleness flag) so the evaluation phase reads one file instead
of re-mining raw archives.
The supervisor creates four gitignored build caches, one per cargo-consumer
role, warm after first use: `target-shared/` (implementation children and
worktree-review gates), `target-shared-validate/` (validators),
`target-shared-gates/` (overlap-window gates), and `target-shared-main/`
(post-merge and final main gates). The supervisor hands `CARGO_TARGET_DIR`
to each cycle as a per-invocation env prefix; loopd.sh carries the
rationale. Why role-keyed: cargo's artifact filename excludes the checkout
path, so one shared dir is last-builder-wins — role-keyed dirs keep each
consumer's artifacts its own. Nothing cleans them automatically; reclaiming
is the operator's call (`du -sh target-shared` to size it,
`rm -rf target-shared` to reset it; rebuilt once, warm for every worktree
again).

The mechanism and rationale live in the work specs:
`specs/t47-shared-target-dir.md`, `specs/t52-overlap-shared-cache-race.md`,
and `specs/t57-main-dedicated-gates-target-dir.md`.
State in `.chug/loopd/` (cycle logs, pidfile). Three consecutive cycles
without `goal_complete` → HALTED marker + exit. A second supervisor refuses
to start (pidfile); a manual `LOOP-SPEC` run makes it skip a cycle rather
than collide. Between cycles the supervisor fingerprints its own script and
re-execs itself when the file has changed, so loopd edits merged to main
activate without an operator restart — a pending `stop` still wins.

## Development

```bash
cargo build && cargo clippy --all-targets -- -D warnings && cargo test
```

All three must stay green. Layout: `src/{api,archive,driver,driver_lock,eventlog,events,tools,tui,webfetch,chat,
attach,complete,riskgate,mcp,mcp_http,sse,observ,auth,ledger,transcript,build_info}.rs`
(+ `main.rs`; `build.rs` only bakes the git commit into the startup banner).
