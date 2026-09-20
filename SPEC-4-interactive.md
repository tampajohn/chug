# SPEC 4 — chug interactive mode (`chug chat`)

Implement an interactive, conversational mode for chug. Today chug only runs
autonomously (`chug run --spec --goal` → loop to verified completion). This
spec adds `chug chat`: a persistent session in the existing TUI where the user
drives turn by turn — type a request, chug works it with tools, returns to
idle, repeat. Think Claude Code's interactive session, with chug's engine.

check: cargo test

## Repo context (read first)

- `src/driver.rs` — the autonomous loop + `Controls` (abort flag, steering
  channel). `src/events.rs` — `Event`/`EventSink`/`ConsoleSink`. `src/tui.rs` —
  ratatui app: `App` state, `apply` reducer, `on_key`, draw, `run_tui`.
  `src/tools.rs` — 9 tools, dispatch, path safety. `src/api.rs` — Messages API.
- Conventions: anyhow::Result, no unwrap/expect outside tests,
  `cargo clippy --all-targets -- -D warnings` must stay clean, new logic gets
  unit tests, no tokio (threads + mpsc only).

## CLI

`chug chat [--cwd <dir>] [--model <id>] [--max-iters <n=40>]
[--max-minutes <n=120>] [--resume] [--risk-gate]`

- TUI is mandatory in chat mode (it IS the interface). No headless chat.
- Budgets are PER TURN: each user request gets up to max-iters LLM iterations
  and max-minutes wall time. Exceeding either ends the turn (not the app) with
  an `Aborted{reason}` event and a return to idle.
- `--resume` loads `.chug/transcript.jsonl` as usual.

## State machine

```
Idle ──submit request──▶ Working ──turn ends──▶ Idle
Working ──Esc (interrupt)──▶ Idle
Working ──budget exceeded──▶ Idle
```

- **Idle**: the input dock is focused. Status: `idle — type a request`.
- **Working**: the driver loop runs exactly as in `run` mode (tools dispatch,
  ledger, trimming, risk gate if enabled), with TWO differences from `run`:
  1. A natural stop (assistant message with no tool_use) ENDS THE TURN — no
     anti-stall kick. The user is present; they judge completeness.
  2. `goal_complete` also ends the turn (verification applies only if a check
     command is configured — see `/check`), printing the summary into the
     activity stream, then Idle.
- **Interrupt**: Esc while Working sets the abort flag; the driver aborts at
  the next iteration boundary with reason `operator interrupt`, control
  returns to Idle (the app does NOT exit). `q` in Working = same as Esc.
  `q` or Ctrl+C in Idle = quit the app (graceful, same exit path as today).

## Input dock (replaces the hidden `i` steering line in chat mode)

- Always visible at the bottom, focused by default. Prompt glyph: `❯` when
  Idle, `…` when Working.
- Submitted while Idle → becomes the **current objective**: appended as a user
  message, banner line `─ objective: <text, truncated 80> ─` in the activity
  stream, state → Working.
- Submitted while Working → a steering note, EXACTLY the existing mechanism
  (`[operator] <note>` drained at the next boundary).
- Lines beginning with `/` are slash commands (below), never sent to the LLM.
- Turn boundary markers: when a turn ends, print `─ turn complete ─`
  (or `─ turn interrupted: <reason> ─`) in the activity stream.

## Slash commands

- `/spec <path>` — load or replace the spec file injected into the system
  prompt (re-read every iteration, same as run mode). `/spec` alone clears it.
- `/goal <text>` — set a persistent goal injected into the system prompt for
  all subsequent turns. `/goal` alone clears it.
- `/check <cmd>` — set the verification shell command used to gate
  `goal_complete` (same semantics as the `check:` line in run mode).
  `/check` alone clears it (goal_complete then ends the turn unverified).
- `/ledger` — no-op visually (ledger panel already live), but refocuses the
  ledger pane. (Trivial; keep for symmetry.)
- `/model <id>` — switch model for subsequent API calls.
- `/budget <iters> <minutes>` — change per-turn budgets.
- `/quit` — quit (same as q in Idle).
- `/help` — one activity-stream line listing these.
- Unknown `/x` → activity line `unknown command: /x (see /help)` — never an
  LLM round-trip.

## System prompt in chat mode

Same assembly as run mode: harness preamble (adjusted: "you are chug in an
interactive session; work the user's current objective; when it is done, stop
— the user will give the next objective"), then `## Spec` if loaded, `## Goal`
if set, `## Ledger` as today. The CURRENT OBJECTIVE is the final user message,
not part of the system prompt.

## Events & UI

- New `Event::TurnStart { objective: String }` and
  `Event::TurnEnd { reason: TurnEndReason }` (Completed | GoalAccepted |
  Interrupted | BudgetExceeded). ConsoleSink doesn't need them (no headless
  chat) but keep the reducer total.
- Status bar in chat mode: `idle` (cyan) / `working` (yellow) /
  `interrupting…` (red) / plus the current objective (truncated) while
  Working.
- The existing `i`-opens-input behavior is DISABLED in chat mode (input dock
  replaces it). Run-mode TUI unchanged.

## Files

- `src/chat.rs` — new: state machine, slash-command parser, turn lifecycle,
  wiring between UI input and driver.
- `src/driver.rs` — chat-mode turn runner (reuse the iteration loop; parameterize
  the two behavior differences: natural-stop ends turn, per-turn budgets).
- `src/tui.rs` — input dock, banners, status additions.
- `src/main.rs` — `chat` subcommand.

## Tests (no network, no terminal)

- State machine: Idle→Working→Idle on natural stop; goal_complete path (with
  and without `/check`); Esc interrupt → Idle; per-turn budget → turn ends,
  app continues; second request after a completed turn works (transcript
  continuity).
- Slash parser: each command parses; unknown command handled; `/spec` with a
  path updates state; lines with leading whitespace + `/` still parse.
- Objective vs steering: submitted-in-Idle becomes a user message and starts a
  turn; submitted-in-Working queues as `[operator]` steering.
- `cargo test` (the check line) must pass.

## Acceptance

- `cargo build`, `cargo clippy --all-targets -- -D warnings`, `cargo test`
  all clean.
- `chug run` headless and `chug run --tui` behavior unchanged.
- Manual pty smoke: `chug chat`, type `create hello.py printing hello`,
  watch it work to Idle, type `/quit` — terminal restored.
