# T23 — `delegate` tool: launch + status for bounded child chug runs

check: cd /Users/jadams/workspace/chug && cargo test

## Why (evidence)

Cycle-7 evaluation finding **K3**: the self-improvement loop's core
mechanic — running a bounded child `chug run` in a worktree and
watching it — exists only as hand-rolled orchestrator bash. In cycle 6,
62 of the orchestrator's 74 tool calls were bash, the great majority of
them child plumbing re-typed from LOOP-SPEC every cycle: the 6-line
`nohup … &` launch incantation, `ps -p <pid>` polls, `tail` log reads,
and jq over the child's `.chug/events.jsonl` to learn its state. It
works, but it burns orchestrator iterations on mechanics, is
error-prone (each cycle re-derives it), and is the gap LOOP-SPEC's own
mandate names verbatim ("a missing `delegate`/`web_fetch` tool").

`web_fetch` was assessed and deliberately NOT filed (zero moments in six
cycles where web access would have changed an outcome).

## Scope (one concern)

A new built-in tool **`delegate`** with two actions:

- `launch` — spawn a detached `chug run` child against a caller-provided
  cwd (a worktree the CALLER created), spec, goal, model, and budgets;
  return immediately with `{pid, log, events}`.
- `status` — observe a previously launched child: liveness + a summary
  of its `.chug/events.jsonl` (last iteration, budget flags, goal/abort)
  + the tail of its console log.

**Explicitly out of scope** (keep the tool repo-agnostic — chug is not
married to this loop): git worktree creation/branching, `cargo build` in
the worktree, harvest/merge, killing children. Those stay with the
caller's bash (LOOP-SPEC §2 steps 1/3/5 unchanged; the tool replaces the
nohup line and the ps/tail/jq polling).

## Repo context

- `src/tools.rs`:
  - `tool_schemas()` (`:38`) — the JSON tool list; add the `delegate`
    schema here.
  - `dispatch()` (`:149`) → `inner()` (`:159`) — match arm
    `"delegate" => delegate(ctx, input)`.
  - `run_shell()` (`:549`) — existing Command-setup patterns (PATH
    prepending lives in `child_path`, `:656`); `kill_process_group`
    (`:673`) is the precedent for unix process-control calls (tests
    will need it to clean up spawned stubs).
  - Test module at `:731`; `ToolCtx` carries `{cwd, bash_timeout}`
    (delegate needs neither beyond what every handler gets).
  - `ToolResult` shape + `is_error` conventions: mirror `bash()`.
- `src/main.rs:50` — `chug run` accepts `--cwd`; children may equally be
  placed via the spawned process's `current_dir` (preferred: exactly
  mirrors today's `cd <worktree> && nohup chug run …`).
- The risk gate judges **bash commands only** and is default-off
  (`src/driver.rs:612`, `risk_gate: false` at `:1333`) — `delegate`
  needs no gate integration; note this in the module comment.
- Event stream being summarized: `src/events.rs` / `src/eventlog.rs`
  define the schema — `run_start` (carries `max_iters`, `max_minutes`,
  `model`), `iteration` (`n`, cumulative tokens), `tool_result`
  (`name`, `ok`), `budget_low` (`remaining_iters` …), `goal`, `abort`
  (`reason`, `budget_kind`). Real fixtures to model test lines on:
  `.chug/events-t20-impl-20260925-182346.jsonl` (has budget_low + abort)
  and `.chug/events-t19-validate-20260925-141410.jsonl` (has goal).

## Requirements

1. **Schema**: one tool `delegate`, `action: "launch" | "status"`
   (required). `launch` params: `cwd` (absolute dir, required), `spec`
   (absolute path, required), `goal` (string, required), `model`
   (string, required — routing stays the caller's explicit choice),
   `max_iters` / `max_minutes` (u64, optional; defaults 40/35 to match
   today's template). `status` params: `cwd` (required), `pid`
   (optional — liveness via `kill(pid, 0)` on unix; omit → liveness
   `unknown`).
2. **Launch semantics**: spawn `<binary> run --spec <spec> --goal <goal>
   --model <model> --max-iters <n> --max-minutes <m>` with the child's
   process cwd = `cwd`. Binary resolution: `CHUG_DELEGATE_BIN` env
   override when set (the test seam), else `std::env::current_exe()`
   (the running chug — children run the same binary, as today). Stdout
   AND stderr append to `<cwd>/.chug/delegate.log` (create parent dirs;
   one fixed location so `status` and the T19 harvest find it without a
   knob). **Detached**: the call returns as soon as `spawn()` succeeds —
   never waits on the child; on unix the child is immunized against
   SIGHUP / given its own process group (parity with today's `nohup …
   &`; `libc` calls behind `#[cfg(unix)]`, non-unix fallback is a plain
   detached spawn, documented). Spawn failure (missing binary, bad cwd)
   → a `ToolResult` error naming what failed — never a panic, never a
   driver abort.
3. **Return value (launch)**: text body naming `pid`, `log`, `events`
   (the events path = `<cwd>/.chug/events.jsonl`, whether or not it
   exists yet).
4. **Status semantics**: read the TAIL of `<cwd>/.chug/events.jsonl`
   only (bounded — last ≤64 KiB; the file grows unboundedly; missing
   file → report `state: starting`, not an error). Parse the last
   complete JSON lines and report: `last_iteration` (+ `max_iters` from
   `run_start` when seen), `last_event` (type + ts), `budget_low_seen`,
   `goal_seen`, `abort_seen` (+ abort `reason` when present), `alive`
   (from `pid` when provided), and `log_tail` (last ≤3 non-empty lines
   of `delegate.log`, each ≤200 chars, when readable). Pure seam:
   `summarize_events(lines: &[&str]) -> DelegateSummary` does the
   parsing/flag logic with no I/O — all edge cases are unit-tested
   through it.
5. **Path policy (deliberate exception, documented in code)**: delegate
   paths (`cwd`, `spec`) must be absolute and are NOT confined to the
   orchestrator's cwd — children live in `/tmp` worktrees by design.
   This is the only tool exempt from `resolve_safe`; say so in a
   comment at the handler.
6. **Never blocks the driver**: no sleeps, no `wait()`, no retries, no
   tail-following anywhere in either action.
7. **README**: gains a `delegate` bullet under the tools/autonomous
   surface (user-visible tool) — name the two actions and the detached,
   never-blocking contract.

## Tests

- `summarize_events` unit pins: empty input; `run_start` only (state
  running, max_iters known, last_iteration none); mid-run (iteration n
  + tool_result) → n and last_event correct; stream with `budget_low` →
  flag set; stream ending in `goal` → goal_seen; stream ending in
  `abort` → abort_seen + reason; malformed last line → skipped, not
  fatal.
- `status` on a tempdir with no `.chug/` → `state: starting`, no error.
- End-to-end with a stub binary: `CHUG_DELEGATE_BIN` points at a tiny
  script that writes a synthetic `run_start`+`iteration` into
  `$PWD/.chug/events.jsonl` then sleeps; `launch` into a tempdir →
  returns pid; poll `status` (bounded retries inside the test, ≤5s
  total) until the synthetic summary appears with `alive: true`; test
  cleanup kills the pid (reuse `kill_process_group`). Gate the test on
  `#[cfg(unix)]` if the stub needs sh.
- Launch failure leg: `CHUG_DELEGATE_BIN=/nonexistent/chug` → tool
  error naming the binary path, `is_error == true`, no panic.
- Schema pin: `tool_schemas()` gains exactly one `delegate` entry with
  `action` required and both actions enumerated.
- Non-vacuousness: (a) reverting `goal_seen`/`abort_seen` flag logic
  must fail the summary pins; (b) removing the tail-bound (reading the
  whole file) must fail a dedicated bound pin (feed
  `summarize_events` input larger than the bound indirectly, or pin the
  constant); the implementer demonstrates (a) once during the round.

## Acceptance

- `cargo build`, `cargo clippy --all-targets -- -D warnings`, `cargo
  test` all green; suite stays fast (stub end-to-end ≤5s).
- Adversarial validation (REQUIRED — touches `src/tools.rs`, LOOP-SPEC
  §2.4): mutation legs named above (flag-logic revert, bound removal,
  detached-spawn revert → e.g. adding a `wait()` must be caught by
  review since tests can't cheaply prove non-blocking — validator
  verifies by reading); confirm no existing tool schema/dispatch arm
  changed, the `resolve_safe` exemption is confined to this handler,
  and README documents the tool.
- Dogfood (orchestrator, post-merge): the NEXT child launch this cycle
  uses `delegate` instead of the nohup incantation, if one remains.
