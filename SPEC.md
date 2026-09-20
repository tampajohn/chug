# chug — autonomous coding harness

`chug` is a single-binary CLI that runs an LLM agent loop against a coding goal.
Given a spec file and a goal, it keeps working — tool call after tool call —
until the goal is verifiably done or a budget/tripwire stops it.

Design rule #1: **the loop is code, not conversation.** The model never decides
whether to continue; the driver does. Goal and progress live in files on disk,
re-read every iteration, so context trimming can never lose them.

## CLI

```
chug run --spec <path> --goal "<text>" [--cwd <dir>] [--model <id>]
         [--max-iters <n=40>] [--max-minutes <n=120>] [--resume]
chug ledger [--cwd <dir>]     # print current LEDGER.md
```

- `--cwd` defaults to `.`. All file/bash tools are sandboxed to this directory.
- `--resume` reloads `<cwd>/.chug/transcript.jsonl` and continues from it.
- Model resolution order: `--model` flag → `CHUG_MODEL` env → default
  `claude-sonnet-4-6`.

## LLM endpoint

Anthropic Messages API (`POST {base}/v1/messages`), streaming NOT required.

- Base URL: `ANTHROPIC_BASE_URL` env, default `https://api.anthropic.com`.
- Auth: send BOTH `x-api-key: $ANTHROPIC_API_KEY` (if set) and
  `Authorization: Bearer $ANTHROPIC_AUTH_TOKEN` (if set); at least one must
  exist. Also send `anthropic-version: 2023-06-01`.
- Request: `{model, max_tokens: 8192, system, messages, tools, tool_choice: {type:"auto"}}`.
- Non-200 → read body, retry with exponential backoff (1s,2s,4s,8s) max 4
  attempts; on 429/529 honor `retry-after` if present.
- One shared `reqwest::Client` (rustls), 600s read timeout.

## Tools (7)

Register these in the `tools` array; dispatch on `name`. Every tool result is
returned as a `tool_result` content block; errors go in `content` with
`is_error: true` so the model can see and recover.

1. `read_file {path}` → file contents (max 2000 lines, note truncation).
2. `write_file {path, content}` → create/overwrite; mkdir -p parents.
3. `edit_file {path, old, new}` → exact string replace; error if `old` not
   found or found more than once.
4. `bash {command}` → run via `sh -c` in cwd; capture stdout+stderr+exit code;
   120s timeout; output truncated to 30_000 chars (keep head 20k + tail 10k).
5. `grep {pattern, path?}` → shell out to `rg -n --max-count 100`; fall back to
   `grep -rn` if rg missing.
6. `update_ledger {content}` → overwrite `<cwd>/LEDGER.md`.
7. `goal_complete {summary}` → model asserts the goal is met. This does NOT end
   the run by itself — see Verification.

Path safety: all paths resolved lexically against cwd; reject any resolved path
escaping cwd (`..` traversal, absolute paths outside cwd). `bash` runs with cwd
set but is otherwise unrestricted (the user runs chug on their own box).

## Files on disk (under `--cwd`)

- `SPEC.md`-like input: `--spec` path is read at startup AND re-read every
  iteration (user may edit mid-run).
- `LEDGER.md` — the model's external memory. If absent at start, the driver
  seeds it: `# Ledger\n\n## Done\n- (nothing yet)\n\n## Next\n- Read the spec\n\n## Blockers\n- none`.
- `.chug/transcript.jsonl` — one JSON object per line: every message appended
  (role + content blocks). Append-only; rewritten only by `--resume` trimming.

## System prompt (assembled per iteration)

1. Fixed harness preamble: you are chug, an autonomous coding agent; work in
   small verified steps; after each step update the ledger; verify with builds/
   tests; never declare done without running the relevant checks; when the goal
   is fully met and verified, call `goal_complete`.
2. `## Spec` — current contents of the spec file.
3. `## Goal` — the goal text.
4. `## Ledger` — current contents of LEDGER.md.

## Driver loop

```
iteration = 0
loop:
    if iteration >= max_iters: abort("iteration budget")
    if elapsed >= max_minutes: abort("time budget")
    messages = build(system_prompt(), transcript)
    resp = call_llm(messages)
    append assistant msg to transcript
    if resp has tool_use blocks:
        for each: dispatch, append tool_result to transcript
        if any was goal_complete: run Verification
    else:
        # model stopped talking without finishing — the anti-stall kick
        append user msg: "Ledger and goal are above. You have not called
         goal_complete. Continue with the next ledger item, or update the
         ledger if the plan changed."
    trim_transcript_if_needed()
    iteration += 1
```

**Verification** (on `goal_complete`): run the spec's check command if the spec
contains a line `check: <shell command>` (first match). If the check exits
non-zero, append a user message with the failing output and CONTINUE the loop
(goal not accepted). If no `check:` line exists, accept immediately. On accept:
print summary + ledger, exit 0.

**Stuck tripwire**: track the last 3 tool results; if all 3 are errors with
identical `content` (first 500 chars), abort("stuck: repeated error") exit 2.
Any abort prints the ledger and `chug run --resume` hint, exit non-zero.

**Transcript trimming**: estimate tokens as total chars / 4. Above 120_000,
walk from the 2nd message onward replacing tool_result/tool_use text older
than the last 20 messages with `"[trimmed]"` until under 80_000. Never trim
message 0 or the last 20. Ledger carries durable state — trimming is safe.

## Structure (single crate, ~6 files)

```
src/main.rs      CLI parse (clap derive), run/ledger subcommands
src/api.rs       Messages API client, retry, request/response types
src/tools.rs     tool schemas (hand-written JSON), dispatch, path safety
src/driver.rs    the loop, verification, tripwire, trimming
src/ledger.rs    seed/read LEDGER.md helpers
src/transcript.rs JSONL append + resume load
```

Deps: `clap` (derive), `serde`, `serde_json`, `reqwest` (rustls-tls, blocking
is fine — use `reqwest::blocking` to keep the code synchronous), `anyhow`,
`thiserror` optional. NO tokio.

## Acceptance

- `cargo build` clean, `cargo clippy -- -D warnings` clean.
- `chug run --spec s.md --goal "x" --max-iters 3` against a mock or real
  endpoint drives the loop, writes LEDGER.md + .chug/transcript.jsonl.
- Unit tests: path-safety rejects `../x`; edit_file double-match errors;
  tripwire fires on 3 identical errors; trimming preserves last 20 messages;
  `check:` parsing from spec text.
```
