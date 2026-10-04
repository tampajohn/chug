# chug

Autonomous coding harness. Given a spec and a goal, it keeps on chugging.

Design rule #1: **the loop is code, not conversation.** The model never decides
whether to continue — the driver does. Goal and progress live in files on disk,
re-read every iteration, so context trimming can never kill the run.

## Install

Releases are cut on `v*` tags and published to GitHub Releases (the repo's
loop tags at wrap — see LOOP-SPEC.md); each release publishes
`chug-<platform>.tar.gz` + `.tar.gz.sha256` for **macos-arm64**,
**linux-x86_64** and **linux-aarch64**, built `--release --locked` from the
tagged commit (the binary is self-describing: `build.rs` bakes the git hash
into the startup banner). The one-liner below installs the latest release,
and the tarball links use GitHub's `releases/latest` redirect, so they track
the newest tag automatically.

The one-liner — detects your platform, downloads the latest release tarball,
**verifies the sha256**, installs to `~/.local/bin` (PATH hint if it is not on
your `PATH`; never uses sudo):

```bash
curl -fsSL https://chug.sh/install.sh | sh
```

Or install straight from the latest tarball for your platform:

```bash
# macOS (Apple silicon)
curl -L https://github.com/tampajohn/chug/releases/latest/download/chug-macos-arm64.tar.gz | tar xz
# Linux (x86_64)
curl -L https://github.com/tampajohn/chug/releases/latest/download/chug-linux-x86_64.tar.gz | tar xz
# Linux (arm64)
curl -L https://github.com/tampajohn/chug/releases/latest/download/chug-linux-aarch64.tar.gz | tar xz
mkdir -p ~/.local/bin && mv chug ~/.local/bin/   # then ensure ~/.local/bin is on PATH
```

macOS Gatekeeper: the binaries are unsigned (no signing/notarization); if
macOS blocks the first run, right-click → Open once, or System Settings →
Privacy & Security → Allow Anyway, or
`xattr -d com.apple.quarantine ~/.local/bin/chug`.

From source instead (any platform with a Rust toolchain) — see Quickstart
and Development below:

```bash
git clone https://github.com/tampajohn/chug && cd chug
cargo install --path .   # puts the chug binary on PATH (~/.cargo/bin)
```

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
chug run --auto-spec --validate     # force FULL adversarial validation on this run (T189)
chug run --auto-spec --no-validate  # force the gates-only lane (operator override, recorded)
chug chat            # interactive mode (TUI)
chug ledger          # print current LEDGER.md
```

Put a `check: <shell command>` line in your spec — `goal_complete` is only
accepted when the check exits 0. The check (like every shell chug spawns)
runs with `CARGO_TARGET_DIR`/`CARGO_BUILD_TARGET_DIR` scrubbed from its
environment, so a bare `cargo test` builds the run cwd's own `target/` —
set the variable inside the `check:` line itself if you want a shared build
cache (an explicit choice, never an inheritance accident).

No spec yet? `chug run --goal "Fix the flaky login test" --auto-spec` (or
`chug quick`) drafts one for you: one read-only model call (plan mode's
loop) writes `.chug/auto-spec.md` — concern, requirements, tests,
acceptance, plus the `check:` and `estimate:` lines — and the draft's check
is dry-run executed before the run starts. A check that can't exit 0 on the
current tree is refused and redrafted once; a second refusal aborts with
the draft and the failing output, never a loosened check. In chat mode the
same flow is `/auto-spec <request>` then `/auto-spec-approve` (the approve
gate re-runs the dry-run; editing the check to something vacuous is
refused). Auto-spec is for task-class work — chores, small features;
adversarial and loop work keeps hand-written specs. Auto-spec'd runs
default to the low-stakes validation lane (T189): per item, a mechanical
predicate computed from the diff (no core-list file touched, ≤ ~150
changed lines, no new tool/command surface, no CI/`check:`-line change)
routes gates-only — the kimi validation child is skipped, never the gates
(build + clippy + the full suite in the worktree, byte-clean review, and
the scope check stay required). `--validate` forces full adversarial
validation; `--no-validate` forces the gates-only lane; either operator
override is recorded.

## Runbooks

One-page recipes for the recurring uses — a quick task, a spec'd feature,
an external-repo feasibility eval, loopd operations, adversarial review —
each with copy-pasteable commands and a wall-clock arc. Start at
[runbooks/README.md](runbooks/README.md).

## Interactive mode (`chug chat`)

A conversational session in the TUI: type a request, chug works it with tools,
returns to idle, repeat. Natural stops end the turn; budgets are per turn.

- **Always-on input dock** — submit while idle = new objective, while working =
  steering note (`[operator] …`, consumed at the next iteration boundary)
- **`@file` attachments** — `@src/main.rs` expands the file into your message
  (dirs → listings, missing → inline note); Tab-completes paths
- **Tab autocomplete** — `/commands` (built-ins + pack names) and `@paths`
  (git ls-files-backed, substring-ranked, candidate strip above the dock)
- **`/spec` `/goal` `/check` `/model` `/budget` `/ledger` `/quit` `/help`**
- **Esc** interrupts the current turn, `q` quits from idle

### Slash-command packs (`.chug/commands/*.md`)

Repo-local commands, invocable from chat: drop a markdown file into the run
cwd's `.chug/commands/` and its file stem becomes a `/`-command (`review.md`
→ `/review`). Community-extensible without code — no registration, no
rebuild; `/help` and unknown-name errors list what was discovered.

```console
$ cat .chug/commands/review.md
Review the diff below for correctness and report findings.
Focus: $ARGUMENTS
```

- **`$ARGUMENTS`** — everything after the command word replaces the token
  (`/review the login bug` → "Focus: the login bug"); no args → empty
  string. A body without the token gets the arguments appended after a
  blank line.
- **Frontmatter `description:`** — a pack whose FIRST line is exactly `---`
  carries a metadata block, stripped from the body before storage and
  expansion; the one supported key is `description:` (single line, the
  value trimmed — colons inside it are kept), and `/help` lists the pack
  as `/name — description` (bare `/name` without one). Unknown keys
  (`allowed-tools` et al.) are accepted and ignored silently — forward
  compat for a later phase. Lenient by design: no closing `---` means the
  whole file is body-as-written, no metadata; a malformed block is never
  a hard error.
- **Built-ins win** — a pack named `goal.md` is shadowed by the `/goal`
  built-in; packs only fill names the built-ins don't use. Tab completion
  follows the same rule: `/`-Tab offers the built-in commands first, then
  discovered pack names (sorted) that don't collide with one.
- **Per-checkout, like hooks/permissions** — `.chug/commands/` lives in the
  gitignored `.chug/` (no search chain, no CLI flag); a worktree child has
  its own (or none). Missing dir = zero packs, zero cost; a corrupt file is
  skipped with one stderr note (fail-open) and the rest still load.
- **A pack invocation is a normal turn** — the expanded body is submitted
  as your objective (queued if a turn is already running); `@file` mentions
  inside a pack body expand like any submitted line.

- **Run goals (phase 2a)** — `chug run --goal "/review the login bug"` (and
  the same on `chug plan`) resolves the goal through the packs at the CLI
  boundary, before any `.chug/` write: the run starts with the EXPANDED body
  as its goal, announced by one stderr line (`chug: goal expanded from pack
  'review'`). A `/name` no pack provides is a hard error naming the available
  packs (or "no packs discovered" plus the pack dir) — never a silent literal
  run of a typo'd name; a pack that expands to empty is a hard error naming
  the pack; a goal that is not `/name args` (plain text, `/` alone, leading
  whitespace) passes through byte-identical.
- **`goal_pack` + the hash-difference honesty line** — the run's opening
  `run_start` line records `goal_pack` (the pack name when expansion fired,
  `null` otherwise — the field is always present) while `goal_sha256` hashes
  the EXPANDED text, what the model actually received. So when a pack fired,
  the child's `goal_sha256` will NOT match a parent's delegate-launch echo
  (the parent hashes the literal `"/name args"` argv, the child the expanded
  body) — that difference is the expansion, not a transmission garble; the
  parent's `goal_tail` composition check remains its garble surface.

Phase 2b (T118, landed): frontmatter descriptions (`/help` surfacing) and
Tab completion of pack names — that closes F9 phase 2 (run-side goals in
phase 2a, chat UX in phase 2b). Still open, later phase: semantics for the
other frontmatter keys (`allowed-tools` et al. are accepted and ignored
today).

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
- **Live model text (streaming)** — requests default to `"stream": true`: the
  model's text appears on stderr as it arrives under a one-time
  `[chug] model: ` prefix (`chug run` / `delegate` logs gain liveness during
  minutes-long generations; the completing preview line is suppressed so text
  never double-prints). `CHUG_STREAM=0` restores the byte-identical
  non-streaming request and parse path. If an endpoint or proxy answers a
  streamed request with a plain JSON body, behavior is unchanged and ONE
  `stream_fallback` line is recorded in `.chug/events.jsonl` (first
  occurrence only)
- **LEDGER.md** — external memory the model updates each iteration; injected
  into every turn, so transcript trimming never loses progress. A fresh
  `chug run` never inherits a previous session's ledger: a non-seed
  LEDGER.md is archived to `.chug/LEDGER-<timestamp>.md` and the run starts
  from the seed (`--resume` keeps it, chat sessions share it)
- **Verification** — `goal_complete` re-runs the spec's `check:` command;
  failure rejects the claim and the loop continues
- **Approved plan (`--approve plan.md`)** — plan first, execute only against
  an operator-approved plan (Claude Code plan-mode parity). The plan becomes
  the run's execution contract: one user-side preamble block is prepended to
  the first message — "The operator approved this implementation plan;
  implement it, then satisfy your goal's check." plus the plan text verbatim —
  while `--goal` still names the objective and the spec's `check:` line still
  governs acceptance (the plan constrains HOW, the goal names WHAT, the check
  decides DONE). The run refuses to start unless the file exists, is a
  readable regular file, and is non-empty — the refusal exits nonzero with a
  stderr message naming the path and the failed leg (missing / not a readable
  regular file / empty / outside the working directory) BEFORE any `.chug/`
  write, so a rejected launch leaves no session state behind; relative paths
  resolve against the run cwd and must stay inside it (an approved plan is
  repo-local input, not a path into $HOME). The run's `run_start` events line
  records the path as typed (`approve`) and the SHA-256 of the file bytes
  (`plan_sha256`) — always-present fields, `null` when the flag is absent.
  Accepted on `chug run` only: `chug plan` produces plans, `chug chat` is
  interactive (both reject the flag)
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
  injection records one `output_truncated` line in the events log. When the
  per-request cap is below 32768 the advisory gains a remedy line naming
  `CHUG_MAX_TOKENS` — thinking models (GLM) spend the same per-request
  budget on thinking blocks and the response, so a low cap truncates large
  tool calls that a raised cap fits
- **Token budget** — `--max-tokens N` (run and chat) caps the run's
  cumulative input+output tokens — the axis iteration/wall-clock budgets can
  miss (a cheap watch-and-wait loop burns neither while racking up tokens).
  Crossing the ceiling aborts at the loop top naming the exhausted budget
  (`budget: N tokens`). Distinct from it: `--max-tokens-per-request`
  (run/plan/chat, or `$CHUG_MAX_TOKENS`) sets the per-request output cap
  sent as `max_tokens` on every API call — default 32768 (the old hardcoded
  8192 let GLM thinking blocks plus a ~7KB write truncate mid-JSON); the
  run's `run_start` events line records the configured cap as
  `max_tokens_per_request`
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
- **Transcript trimming** — past a token estimate, whole oldest-complete
  16k-token segments of old history collapse to one frozen marker
  (`[trimmed: ~16k tokens, N tool results]`) each, so the cached request
  prefix stays byte-stable; the ledger carries durable state. A fresh
  `chug run`
  rotates a non-empty `.chug/transcript.jsonl` to
  `.chug/transcript-<timestamp>.jsonl` before its first append, so
  `--resume` never splices foreign sessions into context
- **Live-context editing** — each turn the driver mirrors the current
  message list to `.chug/LIVE_CTX.md` as `[[CTX_TURN i role=...]]` blocks
  (post-system-prompt messages only), and the model may edit that file with
  its ordinary file tools. After each turn an accepted parse-back — whole
  turn blocks only, pinned content untouched, strictly smaller in tokens —
  replaces the affected transcript segment and records a `[ctx-edit:]`
  marker (so `--resume` re-derives the identical context, same discipline
  as `[trimmed:]`); a rejected edit leaves the transcript untouched and
  surfaces a one-line reason. A turn whose only effect is an accepted edit
  is FREE: it does not count against `--max-iters` (at most 3 consecutive
  free edit turns, the 4th counts normally; `--max-tokens` always binds).
  `--ctx-warn-at-tokens <N>` (default 0 = off) fires a one-shot steering
  note when pre-call context crosses the threshold, naming LIVE_CTX
  editing as the remedy. An accepted edit re-prefills the surviving suffix
  from the edit point — one cache cost against a sustained smaller
  context; `ctx_edit` event lines carry accepted + before/after tokens
- **Events log** — the driver appends its structured event stream to
  `.chug/events.jsonl`, one JSON object per line (`jq`-mineable): run start
  (the banner fields: version/commit/model/spec/cwd/mode — plus the
  checkout's `head_branch`/`head_commit` when the cwd's HEAD resolves at
  runtime, `null` when it doesn't — the configured budget ceilings —
  `max_iters`, `max_minutes`, `max_tokens` as `null` when unset — and the
  goal's `goal_sha256`, SHA-256 hex when the mode has a goal, `null` when it
  doesn't: the child-side half of the delegate integrity comparison — plus
  `goal_pack`, the pack name when the goal was expanded from a
  `.chug/commands/` pack, `null` otherwise, field always present; when
  `goal_pack` is non-null the hash is over the EXPANDED body, so it will not
  match a parent's delegate-launch echo of the literal `/name args` argv —
  the expansion, not a transmission garble — plus `approve` +
  `plan_sha256`, the operator-approved plan's path as typed and its
  file-bytes SHA-256 when the run launched with `--approve`, both `null`
  (fields present) otherwise), one line
  per iteration with cumulative tokens (plus the cumulative
  `cache_read_input_tokens`/`cache_creation_input_tokens` the endpoint
  reports, `0` when it doesn't — the per-call context size the non-cached
  `input_tokens` alone hides), one `trim` line per fired transcript trim
  (estimated tokens before/after, segments collapsed this pass, total
  `[trimmed: …]` markers — events-log telemetry only), tool results (ok/is_error/duration_ms, ≤200-char
  previews — error results keep a tail-anchored ≤2000-char window, so the
  failing test's name or error block at the end of the output is on record),
  verification commands, goal verdicts, budget-low warning
  injections (with the remaining counts at fire time), output-truncated
  advisories (one `output_truncated` line per injected advisory), and aborts
  (with the dying model and, on budget deaths, the exhausted budget). Best-effort
  telemetry: a write failure warns once on stderr and never affects the run.
  Fresh runs rotate a previous log to `.chug/events-<timestamp>.jsonl`
  alongside the transcript

## Session forks (`chug fork`)

Named save/restore slots over the two session files (`.chug/transcript.jsonl`
+ `LEDGER.md`) — the serial explore-two-approaches shape (benchmark:
unreal-agent forking): run approach A, `fork save`, keep going or branch
differently from the same state, `fork restore`, run approach B.

```bash
chug fork save approach-a     # snapshot the live session into .chug/sessions/approach-a/
chug fork list                # one line per slot: name, transcript bytes, mtime, first-message preview
chug fork restore approach-a  # put a slot back over the live state
```

- **Save** — snapshots the live transcript (required; absent or empty →
  `fork: nothing to save`) and LEDGER.md (when present) into
  `.chug/sessions/<name>/`. Names are `[A-Za-z0-9._-]+`; an existing slot is
  never overwritten without `--force`.
- **Restore is safe by construction** — it refuses while a live run holds
  `.chug/driver.lock` (naming the pid; a stale lock proceeds, same reclaim
  rule as the driver), then rotates the live transcript + non-seed ledger
  aside with the same timestamped archive machinery a fresh run uses —
  nothing is lost, the live session becomes a
  `.chug/transcript-<ts>.jsonl` / `LEDGER-<ts>.md` archive — and only then
  copies the slot into place. Copy semantics only: the slot never mutates,
  so restoring the same slot twice is byte-identical. A slot saved without a
  LEDGER.md restores to no ledger at all (the next fresh run seeds one).
- **Serial, not concurrent** — one slot set per cwd; fork is a CLI op
  outside the run loop (no events, no banner — the resumed run's own
  `run_start` records the continuation). Concurrent same-cwd runs are
  already barred by the driver lock; parallel exploration wants separate
  worktrees (`delegate`).

## Plan mode (`chug plan`)

A read-only planning session: the model explores the repo and drafts an
implementation plan, with zero mutation surface. Benchmark: Claude Code's plan
mode; chug's twist is that it also serves the autonomous loop — dispatch a
plan child into a worktree (e.g. via `delegate`) to draft an approach before
burning an implementation child.

- **Read-only contract** — the tool list advertised to the API is EXACTLY six
  tools: `read_file`, `grep`, `glob`, `list_dir`, `web_fetch` (read-only by
  design — GET-only, http/https, size-capped — so planning can research docs
  mid-plan), and `submit_plan`. No
  `write_file`, `edit_file`, `bash`, `delegate`, `update_ledger`,
  `todo_add`/`todo_update`/`todo_list`,
  `goal_complete`, `decision_log`, and no MCP tools
- **Defense in depth** — the schemas are filtered AND dispatch rejects every
  other registered tool name with a tool error naming the allowed set; the
  call never executes and the loop continues
- **`submit_plan`** — the single deliberate write/exit path. Input is the full
  plan as markdown; with `--out <path>` it is written verbatim to that path
  (parent dirs created; the same cwd-sandbox rule as `write_file` — a path
  escaping the working directory is a tool error), without `--out` it prints
  to stdout. Either way the session ends with exit 0 and the events log
  records the outcome like a run's goal acceptance (the plan rides the
  `goal`/`accepted` line as its summary)
- **No bookkeeping** — a plan run never archives or seeds LEDGER.md and never
  writes TODO.md; the ledger appears in the prompt as read-only context only.
  Budgets are `--max-iters 30` / `--max-minutes 20` (defaults) /
  `--max-tokens` (optional); budget exhaustion uses the run-mode abort path
  unchanged (nonzero exit, Aborted event naming model + budget). Events
  parity: `.chug/events.jsonl` opens with a `run_start` whose mode is "plan"

```
chug plan --goal "add a --version flag" --spec SPEC.md --out plan.md --model claude-sonnet-4-6
```

`--spec` is optional (same resolution as `run`); `--out` is optional and
cwd-sandboxed. Not yet in plan mode: a `/plan` chat slash command (deferred
phase 2b).

## TUI (`--tui`)

Activity stream (model text + tool calls), live LEDGER.md panel, status bar
(model, iteration/budget, elapsed, cumulative tokens). `i` opens the steering
line (chat mode has the dock instead), `q` = graceful operator abort,
panic-safe terminal restore.

## Tools

`read_file` (`offset`/`limit` page past the 2000-line cap; image extensions —
`png`, `jpg`/`jpeg`, `gif`, `webp` — come back as base64 image blocks with a
short text note instead of mojibake, capped at 5 MiB per image, and
downgraded to a placeholder when the endpoint rejects image content),
`write_file`,
`edit_file` (+`replace_all`), `bash`, `grep`,
`tgrep`, `glob`, `list_dir`, `update_ledger`, `todo_add`, `todo_update`,
`todo_list`, `goal_complete`, `delegate`, `web_fetch`, `web_search`,
`decision_log`.
All file-tool paths are sandboxed to `--cwd` — `..` traversal, absolute
paths outside it, AND symlinks resolving outside it are refused (T134:
`resolve_safe` resolves the real filesystem, so an in-tree link to
`/etc` cannot smuggle a read or write through). Glob patterns are
confined too (T134 F1: the glob crate follows symlinked directories
during expansion, so `glob`/`tgrep` drop every match whose real
resolution lands outside `--cwd`, and a metacharacter path passed to a
literal-path tool is refused, never expanded). `delegate`, `web_fetch`, and
`web_search` remain the three documented exceptions — `delegate`'s absolute
`cwd`/`spec` target child worktrees by design; `web_fetch` and `web_search`
are network, not filesystem). `bash` is NOT filesystem-confined: it starts in `--cwd`
but can touch absolute paths and inherits chug's environment (API
credentials included) — treat model-issued bash as running with chug's
own privileges. It runs in its own process group —
timeouts SIGKILL the whole group, so orphaned grandchildren can't wedge the
driver (120s default; `--bash-timeout` / `CHUG_BASH_TIMEOUT` overrides).
`chug plan` runs the same `read_file`/`grep`/`glob`/`list_dir`/`web_fetch` tools plus `submit_plan` (its only write and exit path) — plan mode advertises no other tool (the bookkeeping tools `update_ledger`, `decision_log`, and the todo tools included), so the registry surfaces below are run/chat surfaces.

`decision_log` is the loop's bookkeeping surface next to `update_ledger`:
structured decision records to `.chug/decisions.jsonl` (append-only,
best-effort) feeding the F13 distillation corpus — outcome records'
`choice` is a closed set (`landed-clean`/`fixed-up`/`reverted`) enforced
at write time, `scripts/decisions-audit.sh` prints the corpus-health
summary, `scripts/decisions-export.sh` joins each non-outcome record
to its outcome label into the training JSONL — one row per decision in
file order, `{id, ts, class, subject, inputs, options, choice,
confidence, outcome}`, with `outcome` null until an outcome record names
the id; like the file tools, the decision-record surface is cwd-sandboxed,
so the three documented sandbox exceptions stay exactly three. The
phase-2b consumer is `scripts/distill_experiment.py`, which is NOT a
sandboxed in-loop tool: an operator-host script (venv only, offline, corpus
read never written) that takes the corpus path as an argument and reads the
main checkout by absolute path — the measure-first training + evaluation
run (time-ordered split, mechanical baselines, τ-curve) whose go/no-go
report lives at `docs/distill-f13-2b.md`.

`todo_add` / `todo_update` / `todo_list` are the other bookkeeping surface
beside `update_ledger`: a structured todo list stored as a JSON array at
`.chug/todos.json` (ids `t1`, `t2`, …; statuses `pending`/`in_progress`/`done`
enforced — an invalid status is an error naming the set, an unknown id names
the existing ids, a corrupt store is an error naming the file). `todo_list`
renders `t3 [in_progress] title` lines (or `no todos`), and whenever the list
is non-empty the same rendering rides the system prompt as a `## Todos`
section after `## Ledger` in run and chat mode. The store is cwd-confined
like every `.chug/` surface: worktree children start with an empty list by
construction, resumes inherit the file, and jq-able orchestrators can read a
child's self-declared plan without transcript archaeology. Plan mode excludes
all three (no write tools there).

`delegate` — launch, observe, or collect a bounded child `chug run` (e.g. in
a git worktree). Three actions:
- **`launch`** — spawns a detached child (`--spec`, `--goal`, `--model`
  required; `--max-iters`/`--max-minutes` optional, defaults 40/35;
  `--max-tokens` optional — the child's cumulative input+output token
  ceiling, omitted = no token ceiling; `resume: true` optional to continue
  the child's aborted run instead of starting fresh; `env` optional — an
  allowlisted string→string map passed to the child's environment: keys must
  match `^(CARGO_|CHUG_|RUST)[A-Z0-9_]*$`, ≤16 entries, values ≤4 KiB with
  no NUL bytes, anything else a tool error naming the key; applied AFTER the
  inherited-env target-dir scrub, so an explicit `CARGO_TARGET_DIR` in `env`
  WINS over the scrub — the scrub guards the absent case, `env` is the
  explicit case; when `env` is absent the goal-carried `export` remains the
  fallback and the spawn is byte-identical, and the launch payload names the
  applied keys, never the values; the payload also gains a
  `WARN target-dir drift:` block when ≥2 of the three CARGO_TARGET_DIR
  carriers — the spec's `check:`-line export, the goal's
  `export CARGO_TARGET_DIR=…`, and the `env` map's `CARGO_TARGET_DIR` — are
  present and any pair disagrees, naming every present surface and its dir
  (advisory only — the launch proceeds and nothing is rewritten; absent
  surfaces never warn, a spec with no check export is legitimate) against an absolute
  `cwd` you prepared, appends its stdout+stderr to
  `<cwd>/.chug/delegate.log`, and returns immediately with the child `pid`,
  the log/events paths, and the goal-integrity echoes — `goal_bytes` (the
  goal's UTF-8 byte length), `goal_sha256` (SHA-256 hex over the exact goal
  string passed to the child argv), and `goal_tail` (the tail-anchored
  ≤120-char preview, the same rule as error previews — read them on your
  next iteration: the echo turns the send-time eyeball catch into a designed
  glance). Two garble
  classes, honestly separated: a composition garble (you garble your own
  tool-call text, e.g. a duplicated tail) matches in BOTH hashes — the
  child hashes the same garbled bytes — and only the `goal_tail` preview
  exposes it; a transmission garble (the argv/pipe corrupts) is what
  comparing `goal_sha256` against the child's `run_start` `goal_sha256`
  detects. One designed mismatch class is neither: a child whose `run_start`
  shows a non-null `goal_pack` hashed the EXPANDED pack body, not the literal
  `"/name args"` argv this echo carries — that is the goal expansion at work,
  not a transmission garble. chug reports, it does not adjudicate — it never waits on the child
- **`status`** — reports the child's liveness (when you pass the `pid`), a
  summary of its `.chug/events.jsonl` — the child's latest run segment
  (state, `last_iteration` + `max_iters`, budget-low / goal / abort flags
  with the abort reason) — and the tail of its console log — instant
  polling never blocks. Optionally pass `wait_secs` (status-only;
  0/absent = instant, max 600) to collapse each idle wait window into one
  blocking status call: it returns early when the child's iteration
  advances, a verdict or budget-low flag appears, or its liveness flips to
  dead (per-tool-call `last_event` churn renders at the deadline but never
  wakes it), else at the deadline, and names the actual elapsed seconds on
  a `waited:` line; adding `terminal: true` (status-only, default false,
  requires `wait_secs > 0`) narrows the wake set to the terminal facts —
  the goal or abort verdict, liveness alive→dead, or events-file creation
  — so a working child never wakes the wait on iteration advances (one
  orchestrator iteration per child run, not per child iteration), while
  telemetry still renders at the deadline
- **`collect`** — returns the child's structured result in one bounded,
  non-blocking read — the latest run segment's verdict (`goal-accepted` /
  `goal-rejected` / `aborted` with reason / `running` / `starting`), the
  accepted goal's summary (the child's own account of what it did), the
  segment's latest check cmd (the gate that decided the verdict), and
  best-effort `git log --oneline` commit refs of the child's cwd (optional
  `base` scopes the range to `<base>..HEAD`; every git failure degrades to
  a note, never an error) — pass the launch `pid` to add the same liveness
  line `status` renders; it never blocks or waits, so long-poll with
  `status` first
- **Sandbox/cwd** — `cwd` and `spec` must be absolute and must EXIST (launch
  refuses a missing/unreadable `spec` or a missing `cwd` with an error
  naming the received path verbatim — a corrupted field fails fast at the
  call site instead of spawning a doomed child); they may target child
  worktrees outside your own `--cwd`; worktree creation, building,
  harvest/merge, and killing the child stay with your `bash`

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

`web_search` — web search beside `web_fetch`, over the zero-config DuckDuckGo
HTML provider (no API key; keyed providers — Brave/Tavily — are the seam's
named phase 2). `query` required, `max_results` optional (default 5, clamped
to 1..=10 — a larger request clamps, never errors); numbered results, each
with a title, url, and one-line snippet; provider override via the
`CHUG_WEB_SEARCH_PROVIDER` env var (valid values: `duckduckgo`). Live HTML
scraping can break or be rate-limited at any time; those surface as tool
errors — never a silent empty list (a page with no parsed result blocks says
whether the page itself reported no results or the markup may have changed).
Like `web_fetch`, it is network, not filesystem.

`tgrep` — token-budgeted ranked context search: locate the 20 relevant lines
without reading a whole file. `query` is one or more terms (ranked AND-ish;
`"quoted phrases"` must appear exactly); results are ranked match CLUSTERS
(file:line + ±3-line window, best-first, each with a score; overlapping
windows merge into one cluster) and the output stops at `budget` (default
2000 tokens, above-8000 clamped down) with a `[more: N clusters omitted]`
marker — the tool result can never balloon the way a 200-hit grep does.
Optional `path` narrows to a glob (`src/**/*.rs`), a directory, or one file;
`symbols: true` with a Rust file in `path` returns its fn/struct/impl
signature skeleton (no bodies) for orientation. Ranking is deterministic
hand-rolled scoring — exact-phrase > all-terms-in-window > term density,
with a path-basename boost; no embeddings, no LLM. The workflow it teaches:
tgrep to locate, then a targeted `read_file` with `offset`/`limit` around
the best cluster. Like the file tools it is cwd-sandboxed (`path escapes
cwd` refused; `bash` is the cross-tree escape).

## Risk gate (`--risk-gate`)

Every `bash` command is classified by a [Laya](https://github.com/convaiinnovations/laya)
judge server (`LAYA_URL`, default `http://127.0.0.1:8420`) as
destructive/risky/safe before executing. `destructive` p≥0.5 is blocked with
an error the model can see and route around; `allow destructive` in a steering
note disables the gate for the run. Fail-open if the judge is down. Verdicts
logged to `.chug/risk_verdicts.jsonl`.

The judge client is selected by `CHUG_JUDGE` (T204/F15): `daemon` (the
default) talks to chug's **baked-in judge daemon** — `chug daemon` hosts the
same Laya model in-process over a `0600` unix domain socket at
`~/.chug/daemon.sock` (`CHUG_HOME`/`CHUG_DAEMON_SOCK` relocate it; no TCP
port, no network listener), loads the ~650MB checkpoint once per host, and is
auto-spawned on the first judge call with stale-socket recovery; `http` sends
the request to the external layad at `LAYA_URL` byte-for-byte as before (the
escape hatch); `off` disables classification (every command fails open,
logged). The inference stack (candle + hf-hub + tokenizers) lives behind the
`daemon` cargo feature — off by default, so the hot `chug run` build never
compiles it. See [DEPENDENCIES.md](DEPENDENCIES.md).

The same socket also hosts the **session registry** (T219): every chug run
registers a TTL heartbeat — `POST /sessions` upserts one entry
(`{id, role, started, last_event_ts, status}`; roles `loopd-cycle`,
`delegate-child`, `dashd`, `daemon`), `GET /sessions` lists the live ones
with `age_sec`; entries expire after 10 idle minutes. loopd heartbeats at
each cycle start and delegate children register at launch (all best-effort —
a downed daemon is a silent no-op), and the daemon lists itself while
serving. One `curl --unix-socket ~/.chug/daemon.sock localhost/sessions`
answers "what chug runs are alive on this box" locally. A weightless
registry host (`CHUG_DAEMON_SESSIONS=1`) serves `/sessions` + `/healthz`
without loading the model — `/judge` refuses there exactly as on the stub.

## Hooks (`.chug/hooks.json`)

Operator policy-as-config: shell commands fire around every tool call. The
file lives in the run cwd's `.chug/` (gitignored, per-checkout — a worktree
child has its own; no search chain, no CLI flag):

```json
{"hooks": {
  "PreToolUse":  [{"match": "bash",  "command": "./.chug/hooks/gate.sh"}],
  "PostToolUse": [{"match": "edit_*", "command": "./.chug/hooks/note.sh"}]
}}
```

`match` is a glob on the tool name (`*`/`?`; `mcp__*` matches MCP tools by
their registered `mcp__<name>__<tool>` name like any other). Runs in run and
chat mode; **plan mode never fires hooks** (its tool contract is exactly the six read-only tools, submit_plan included). Each hook runs `sh -c <command>` in its own process
group, cwd = the run cwd, with a JSON payload on stdin:
`{"event","tool","input","cwd"}` (+ `"is_error"` for PostToolUse).

- **PreToolUse** (before the tool executes): exit 0 → allow; non-zero →
  **veto** — the tool does not execute and the model receives a tool error
  `[hook veto] <stderr>` it routes around (the risk-gate shape).
- **PostToolUse** (after, ok or error): advisory only — non-empty
  stdout+stderr is appended to the tool result as `\n\n[hook] <text>`
  (` (exit <n>)` when non-zero); it never blocks or changes the result.

Bounds: 10s wall cap per hook (expiry SIGKILLs the whole process group), hook
output tail-anchored to ≤2000 chars, and everything **fails open** — an
absent/empty config is zero hooks and zero cost; a malformed config or a
failing/spawn-erroring hook warns once on stderr + one line in
`.chug/events.jsonl` (`"type":"hook"` per fire, `"type":"hook_error"` per
problem) and the run continues. A hook never triggers hooks. Phase 2 scope
(again as config, no doctrine forks): Stop/GoalComplete events, arg-glob
matchers, `--hooks` CLI flag, and the Laya stop-hook reference consumer.

## Permissions (`.chug/permissions.json`)

Declarative per-tool deny rules, evaluated in-process before every tool
call — the fail-closed policy surface hooks deliberately are not (a hook
runs in a spawned `sh -c` and fails open on any spawn/config problem; a deny
rule has no process to spawn and nothing to fall back through — a match
denies, period). The two layers compose: permissions for the rules that must
not depend on a shell, hooks for everything programmable. The file lives in
the run cwd's `.chug/` (gitignored, per-checkout — a worktree child has its
own and does NOT inherit the parent's rules; no search chain, no CLI flag):

```json
{"permissions": {"deny": [
  {"tool": "bash", "command": "*rm -rf*"},
  {"tool": "write_file", "path": "*.pem"},
  {"tool": "web_fetch"}
]}}
```

A rule is `{"tool": "<glob>"}` plus at most one arg matcher: `command`
(glob against the bash command string), `path` (glob against the tool's
`path` argument — the file tools), or `url` (glob against web_fetch's url).
A rule with no matcher denies the whole named tool. Tool globs are plain
`*`/`?` globs (`mcp__*` matches MCP tools by their registered
`mcp__<name>__<tool>` name like any other). Matching is fail-toward-
execution on a missing/non-string arg: a bash call without a string
`command` under a `command` rule does not match (the matcher had nothing to
judge); whole-tool rules always deny.

**Policy order**: permissions → PreToolUse hooks → plan gate / MCP / risk
gate. A denied call fires no hooks, reaches none of the later gates, and
never executes; the model receives a `[permission denied] <rule>` tool error
it routes around and the loop continues. Rules run in config order, first
match wins.

**Failure semantics**: the opposite polarity of hooks, both pinned — config
problems (unreadable/malformed config, a rule with an unknown key, two
matchers, or a matcher that cannot fit the named tool, e.g. `command` on
`read_file`) **fail open**: each is skipped with one stderr warning per run +
one `"type":"permission_error"` line in `.chug/events.jsonl` (a malformed
RULE is skipped in place; valid siblings still deny), and the run continues.
A rule match **fails closed**: it always denies, recording one
`"type":"permission_denied"` line per deny.

Surfaces: run, chat, and plan mode (an in-process policy can only restrict
further — denying `read_file *.key` inside a plan session is exactly the
point; the six-tool plan contract is unchanged). Phase 2, deferred with
reasons: allow-rules that short-circuit the risk gate (needs risk-gate
plumbing of its own — that is the leg that makes `--risk-gate` one policy
source among several), ask-mode (a chat/TUI interactive prompt),
settings.json unification (one config shape across policy layers), and a
`--permissions` CLI flag.

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

Resources (F11 phase 1a + 1b-i): chug consumes tools AND reads resources
from servers that advertise them — `resources/list` + `resources/read`
over BOTH stdio and streamable HTTP, on servers whose initialize
handshake advertised the `resources` capability (capped at 200 per
server, like tools); a server without the capability is never asked. The
model-facing surface is the builtin **`mcp_resource`**
tool: `{"action": "list", "server"?}` returns the resource catalog, one
resource per line (`server uri — description (mimeType)`; all servers, or
one named server), and `{"action": "read", "server", "uri"}` returns one
resource's contents (text inline; binary as base64 with the mimeType
named; output char-capped with a truncation note, like `web_fetch`).
Capability-gated and named-error shaped: a non-capable, unknown, or dead
server is a tool error naming the server — never a hang — and with no
servers connected, `list` says so plainly. The tool is a builtin (not an
`mcp__`-prefixed server tool), so T90 deny rules can match the plain
`mcp_resource` name, and it is read-only: it bypasses the laya risk gate
exactly like `mcp__` tool calls (the gate judges bash commands only).
Prompts (F11 phase 1b-ii + 1b-iii): chug also consumes prompts from servers
that advertise the `prompts` capability — `prompts/list` (capped at 200 per
server, like tools and resources) and `prompts/get` with the prompt's
optional arguments map, over BOTH stdio and streamable HTTP; a server
without the capability is never asked. The legs are registry-internal for
now — a model-facing prompts surface (slash-pack surfacing) is a later
F11/F9 phase.

Per-server fail-soft at runtime too: a stdio server that dies mid-run errors
its calls without killing the run; a remote server that refuses connection
retries 3× (1s, 2s, 4s) then returns a tool error. Timeouts mirror stdio:
connect 10s, first-byte 30s, per-call total 60s.

Spawn timing and environment (T138): servers are NOT spawned at startup —
each starts only after `.chug/permissions.json` has loaded, and a whole-tool
`bash` deny, an `mcp__*` deny, or a per-server `mcp__<name>__*` deny in that
config prevents the spawn entirely (a repository-controlled `mcp.json` must
not execute its command before permission enforcement). A spawned stdio
server does not inherit the process environment: it gets a minimal baseline
(`PATH`, `HOME`, `TMPDIR`, `LANG`) plus exactly the entry's `env` map —
pass secrets explicitly per entry, never through ambient inheritance.

Remote specifics (SPEC-9): JSON-RPC over POST; `Mcp-Session-Id` captured and
replayed; notifications expect `202`; SSE response streams are read until the
matching-id response; a server-pushed request gets a JSON-RPC
`method not found` reply and notifications are dropped; the GET listen stream
reconnects with jittered backoff (1s→30s cap) and `Last-Event-ID` replay for
the life of the run. Stdio servers spawn in their own process groups and are
group-killed on every exit path. MCP tools bypass the laya risk gate (which
judges bash only). No config anywhere = byte-identical behavior.

### chug as MCP server (`chug mcp-serve`)

chug also EXPOSES itself to other agents (F10): `chug mcp-serve` runs a
newline-delimited JSON-RPC 2.0 stdio MCP server — the same protocol shape
the client speaks (`2025-06-18`, `initialize` →
`notifications/initialized` → `tools/list`, one JSON object per line) —
until stdin EOF, then exits 0. Claude Code-compatible client config:

```json
{"mcpServers": {"chug": {"command": "chug", "args": ["mcp-serve"]}}}
```

Two read-only tools ship (the write legs below are separately
flag-gated). `chug_status`: input
`{"cwd": "<absolute path>"}`, output a compact self-describing summary of
that chug cwd's latest `.chug/events.jsonl` run segment (state,
last_iteration vs max_iters, goal/abort/budget-low flags, abort reason).
`chug_collect`: input `{"cwd": "<absolute path>", "pid": <optional int>,
"base": "<optional git ref>"}`, output the latest segment's structured
result — the verdict (goal-accepted/goal-rejected/aborted/running/
starting), the accepted goal's summary, the check cmd, a liveness line
when `pid` is given, and best-effort commit refs (`base` scopes the
range as `<base>..HEAD`). No process spawning, no writes anywhere.

The write leg `chug_launch` (T129) exists only when the operator starts
the server with `chug mcp-serve --allow-launch` — the flag is the policy
boundary, and the default is OFF. Without it the server is byte-for-byte
the read-only one: `chug_launch` is not advertised in `tools/list`, and a
`tools/call` for it returns the unknown-tool error (`-32602`). With it,
`tools/list` advertises `chug_launch` (advertised ⇔ callable) and a call
launches a bounded detached `chug run` in a chug working directory,
returning the spawned pid, the events path, and the log path. Params:
`cwd` (required — absolute path to an existing chug working directory
with `.chug/`), `spec` (required — absolute path to an existing spec
file), `goal` (required — non-empty after trim), `model` (required —
passed through; the spawned child's own auth/settings chain validates
it), plus optional `max_iters` (1..=200) and `max_minutes` (1..=240) — a
value above a ceiling is rejected, not clamped; budgets absent fall back
to the delegate defaults (40 iterations / 35 minutes).

The second write leg `chug_cancel` (T153) lives behind the SAME
`--allow-launch` boundary — advertised ⇔ callable beside `chug_launch`,
and invisible without the flag (a call gets the unknown-tool `-32602`,
nothing probed). Input: `{"cwd": "<absolute path>", "pid": <positive
int>}` — the pid a `chug_launch` result carried. Before ANY signal the
ownership of the pid is re-derived fail-closed: the server is stateless
across requests and keeps no launch registry, so every call re-proves,
in order, (a) the pid is alive (`kill(pid, 0)` — an exited child of the
server is reaped first, so a zombie never reads as live), (b) the pid is
its OWN process-group leader (`pgid == pid`, the delegate
detached-spawn fingerprint), and (c) the pid's command line
(`ps -o command=`) names a `chug run` invocation (the adjacent
`run --spec` pair — `--spec` is a required argument of `run`). The FIRST
failed leg is an `isError` result naming the pid and the leg — "no such
process", "not its own process-group leader", "not a `chug run`
invocation" — and NOTHING is signalled; an unresolvable leg (ps failure,
no group record) also fails closed. On pass the whole process GROUP is
SIGTERM'd (negative-pid kill — the detached tree dies, not just the
driver), the call waits up to ~5 s (50 ms poll) for the group to empty,
and a still-alive group is SIGKILLed once. Success payload: `pid` +
`signaled: term|kill` (`term` = the group emptied within the grace,
`kill` = the escalation fired) + `waited_ms`. Failures are `isError`
tool results — never JSON-RPC errors, never fatal.

The control verbs (T157, F10 phase 3) live behind a SECOND policy flag,
`chug mcp-serve --allow-control` — the same default-deny family,
advertised ⇔ callable, and INDEPENDENT of `--allow-launch` (the operator
who grants launching need not grant control of running children). With
it, `tools/list` advertises two more tools:

- `chug_abort` — the RUN-level counterpart of `chug_cancel`. Input:
  `{"cwd": "<absolute path>", "pid": <positive int>}`. Same fail-closed
  ownership re-derivation and the same TERM→~5 s grace→KILL group
  discipline, three contract differences: the result names the terminal
  state (`aborted` — signalled and recorded; `already-done` — the child's
  latest run segment already has a verdict, idempotent and unsignalled;
  `not-found` — dead pid, no verdict, `isError`, nothing signalled); and
  a successful abort is RECORDED in the child's `.chug/events.jsonl`
  (reason `operator abort via chug_abort`) so `chug_status`/`chug_collect`
  report the run as aborted — the gap a bare signal leaves. A verdict
  wins over liveness: a finished run is `already-done` even if its pid
  still answers. The record write is best-effort (`recorded: false`
  degrades, never errors).
- `chug_steer` — inject an operator steering note into a running child
  through the driver's EXISTING `[operator]` mechanism, no new mechanism.
  Input: `{"cwd": "<absolute path>", "pid": <positive int>, "note":
  "<1..=4000 chars>"}` (rejected above the ceiling naming the received
  length, never clamped; empty-after-trim rejected). The note is appended
  to the child's cross-process queue `.chug/steer.jsonl` (a detached
  child's in-process channel is dead) and the child's
  `drive_loop` drains that queue at its NEXT iteration boundary into the
  same path the TUI chat dock feeds — it lands as an `[operator] …` user
  message in the child's transcript (steering stays out of
  events.jsonl by design; the queue is consumed by the rename-away
  drain, and a note queued for a child that died without a verdict
  lingers for the cwd's next driver). The result is `queued: true` or
  `undeliverable` (`isError` — the latest segment already has a verdict,
  or the pid is not alive — with NOTHING written, so a stale note never
  poisons the next driver).

Both verbs are ordinary MCP tools on the client side —
`mcp__<server>__chug_abort` etc. — so T90's `.chug/permissions.json`
glob rules (`mcp__*`, `mcp__<server>__*`, per-tool) gate them like every
other MCP tool name, no new permission surface.

**Launch safety**: the spawned child is an ordinary `chug run` in the
target cwd — it runs that cwd's OWN policy chain (`.chug/permissions.json`
deny rules, `.chug/hooks.json` vetoes, risk gate) exactly as if a human
typed the command, and single-driver safety is the child's own
`.chug/driver.lock` (a conflicting launch fails fast child-side and
surfaces via `chug_status`/`chug_collect`). Launch failures are `isError`
results; nothing else about the server changes, and no error kills the
loop. Wire-level e2e coverage (T148): `tests/mcp_serve.rs` spawns the REAL
`chug mcp-serve` over real stdio with the `CHUG_DELEGATE_BIN` stub-child
seam and pins the full launch conversation deadline-bounded — the
`--allow-launch` happy path (advertised ⇔ callable, the pid/log/events
payload, the exact child argv carrying the wire's spec/goal/model/budgets),
the above-ceiling refusal's `isError` arm (received value named, nothing
spawned, loop alive), and the default-deny boundary (`chug_launch`
unadvertised, its call answered by the unknown-tool error).

Phase 2 (the read tools plus `chug_launch`) is CLOSED, phase 3a —
cancellation, `chug_cancel` — landed (T153), and phase 3's control verbs
— `chug_abort` + `chug_steer` behind `--allow-control` — have landed
(T157). Phase 3b (resources, notifications, a server log) is still
deferred: no consumer pulls MCP-spec-completeness surfaces (the
cycle-72 EVALUATION §4 reason). `chug_delegate` (spawning into an
EXISTING run instead of a fresh one) was EVALUATED for phase 3 and
DEFERRED with a written reason — see the T157 commit message.

**stdout purity**: a stdio MCP server's stdout IS the wire — `chug
mcp-serve` prints nothing but protocol messages (no banner, no log
lines). Never run it in a terminal expecting chatty output; the wire is
the stdout, diagnostics go to stderr. Errors never kill the server:
unparseable lines get `-32700`, unknown methods `-32601`, unknown tools
`-32602`, invalid requests `-32600`, and notifications never get a
reply.

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

Per cycle: the orchestrator's model is per-phase (T81) — kimi-k3
orchestrates fresh-eval cycles, glm-5-3-flash routine ones, decided by
`loopd.sh` before launch from LOOP-SPEC's freshness rule (a non-empty queue
with a same-UTC-day EVALUATION.md means routine; `./loopd.sh routing` prints
the decision the next cycle would get): work the queue — bugs > robustness >
**features** > DX > perf — glm-5-3-flash children implement, kimi validates
adversarially regardless of who orchestrates (family independence: glm never
validates glm), auto-push per item. Two env knobs: `LOOP_ORCH_MODEL` (default
kimi-k3) and `LOOP_ROUTINE_MODEL` (default glm-5-3-flash) — setting
`LOOP_ROUTINE_MODEL=anthropic-system.ai.kimi-k3` restores single-model
operation.
loopd also exports `CHUG_BASH_TIMEOUT=300` for the whole loop fleet (T178):
the orchestrator and every delegate child (children inherit the
orchestrator's env) get a 300s bash-tool cap instead of the 120s default, so
cold-cache gates and builds stop dying mid-run (`timed out after Ns`) and
buying a retry; an explicit `--bash-timeout` flag overrides the env, and
non-loop (interactive/chat) use keeps the 120s default.
The supervisor builds the release binary (`cargo build --release`) before
each cycle and the loop runs on it — and since the T137 build gate, a
FAILED build aborts the cycle instead of relaunching the previous
release binary (a broken merge can no longer silently keep driving on
stale code), with `set -euo pipefail` guarding the whole supervisor
loop and the single-driver probe failing closed. Immediately after that
probe passes — and before the build — the supervisor also runs the T152
orphan-process reaper (`scripts/orphan-reaper.sh`), which SIGTERMs leftover
chug test/build processes (an absolute `…/target-shared*/deps/` artifact
binary, a process whose argv points into a removed
`/tmp/chug-loop-t*/`/`/tmp/chug-mut-*` worktree, or — the cycle-72 shape — a
plain process whose working directory sits inside one of those worktrees,
innocent argv and all) at the one instant they are
definitionally orphaned; the sweep is identity-based and fail-closed
(ambiguous processes are skipped and logged, never killed, own-process-group
never signalled) and `LOOP_REAPER=0` disables it. Delegate children
re-launch that same executable, and the bounded review/validation gates
run the nextest-first
gate runner (T82): `cargo nextest run --release` when `cargo nextest` is on
PATH — a host tool, not a crate dependency — falling back unconditionally
to `cargo test --release` when it is absent (never a hard dependency; the
first cycle after the switch runs both runners once and records both wall
times in Outcomes — the measurement that justifies keeping it;
`loopd.sh` checks at startup and logs which runner cycles use to
loopd.log). The first release build into a cold cache is slower to compile;
the shared caches below amortize it.
A cycle's success verdict is the child's EXIT STATUS (0 = accepted goal;
budget/abort 1, stuck 2), never a log grep (T142: raw model text reaches the
cycle log verbatim, so the supervisor captures the child's stdout apart from
the stderr log, greps only that, and stamps its own rc-based `verdict:` line
— `scripts/site-sync.sh` counts cycles by supervisor stamps only, so forged
`chug: goal complete` text can neither record a failed cycle as OK nor
inflate the published cycle count).
Before each cycle the supervisor refreshes `.chug/eval-digest.md` via
`scripts/eval-digest.sh` — a deterministic (jq/awk-only, sub-second) digest of
the `.chug/events*.jsonl` corpus (per-file iterations, wall time, tool
distribution, error classes, token curve, aborts, budget-low fires, TODO
status counts, staleness flag) so the evaluation phase reads one file instead
of re-mining raw archives.
The autonomous run's notification leg (`.chug/notify.json`, off unless the
file exists) fires a macOS banner per configured event — natively via
`osascript`, or through layad's push-vs-silent judgment by POSTing
`{LAYA_URL}/hook/notification` when the sink is set to `layad`. Phase 1 of
the baked-in judge daemon (T204/F15) serves the risk gate's `/judge` only;
the `/hook/*` surface stays with external layad until a phase-2 migration,
and a `layad` sink with no layad on the network degrades to a `notify_error`
event (never a run failure).
The supervisor creates five gitignored build caches, one per cargo-consumer
role, warm after first use: `target-shared/` (implementation children and
worktree-review gates), `target-shared-validate-a/` and
`target-shared-validate-b/` (validators — one dir per validator slot, T194:
two validators sharing one dir would serialize on cargo's build lock),
`target-shared-gates/` (overlap-window gates), and `target-shared-main/`
(post-merge and final main gates). Validation rounds that mutation-test in
parallel (T79) add per-leg caches `target-shared-mut-<k>/` — one per mutant
leg, cap 3, gitignored by glob, created on demand in the repo root and
never shared across legs (the same role-keying, one level down).
Two-impl overlap (T161) adds two impl-child slots, `target-shared-impl-a/`
and `target-shared-impl-b/` — the impl launched into an overlap takes the
slot no flying impl holds, so two concurrent implementation children never
share one artifact dir (the T52 role-keying, widened); T194's 3-child
fleet adds a third impl slot, `target-shared-impl-c/`, and the second
validator slot `target-shared-validate-b/` (a second validator flies only
when two items are simultaneously past gates — never two on one item).
The supervisor hands `CARGO_TARGET_DIR`
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

All three must stay green. External dependencies — crates, runtime services,
spawned processes, the env surface — are inventoried with their failure modes
in [DEPENDENCIES.md](DEPENDENCIES.md); check it before adding one.
Layout: `src/{api,autospec,archive,driver,driver_lock,eventlog,events,fork,fsatomic,hf_hosting,judge_model,judge_pack,live_ctx,testsupport,tools,todos,tgrep,tui,valroute,webfetch,websearch,chat,
attach,complete,commands,decisions,delegate,notify,permissions,plan,riskgate,hooks,mcp,mcp_http,mcp_serve,sse,observ,auth,daemon,ledger,transcript,trim,build_info}.rs`
(+ `main.rs`; `build.rs` only bakes the git commit into the startup banner).
