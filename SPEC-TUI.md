# chug TUI — live dashboard for the autonomous loop

Add a terminal UI to `chug run` via a `--tui` flag. Headless mode (current
behavior) stays the default and must remain byte-for-byte compatible.

## Architecture

Refactor: the driver must not `println!` directly. Introduce

```rust
pub enum Event {
    Iteration { n: u32, max: u32 },
    ModelText(String),                 // a text block from the assistant
    ToolStart { name: String },
    ToolResult { name: String, ok: bool, preview: String }, // preview <= 500 chars
    LedgerChanged(String),             // full new LEDGER.md contents
    Verifying { cmd: String },
    GoalRejected { reason: String },
    GoalAccepted { summary: String },
    Aborted { reason: String },
    Usage { input: u64, output: u64 }, // cumulative tokens from API usage fields
    SteeringQueued(String),            // echo of an operator note consumed by the driver
}

pub trait EventSink { fn emit(&mut self, e: Event); }
```

- `ConsoleSink` — reproduces today's log lines exactly (headless default).
- `TuiSink` — sends `Event` over `std::sync::mpsc::Sender` to the UI thread.
- API usage: accumulate `usage.input_tokens`/`usage.output_tokens` across
  calls; emit `Usage` after each API response.

With `--tui`: main thread runs the UI loop; a worker thread runs the existing
driver with a `TuiSink`. The driver must also check two shared controls at
each iteration boundary:
- `Arc<AtomicBool>` abort flag (set by `q`) → graceful abort ("operator
  abort"), same abort path as budgets (ledger + resume hint).
- `mpsc::Receiver<String>` steering queue → drain ALL pending notes; append
  each as a user message `[operator] <note>` to the transcript before the
  next LLM call, and emit `SteeringQueued`.

## Layout (ratatui + crossterm)

```
┌ chug ─ <goal, 60 chars> ─ model: <id> ─ iter 7/40 ─ 04:31 ─ in 12.3k / out 4.1k tok ┐
│ ACTIVITY (scrolling, newest at bottom, auto-follow)    │ LEDGER                     │
│  ▸ model: Now I'll write the parser...                 │ (live LEDGER.md,           │
│  ▸ write_file src/parser.rs -> ok                      │  wrapped, scrolls)         │
│  ▸ bash cargo test -> FAIL preview...                  │                            │
├ status: running | verifying | done | aborted ─ i: steer  PgUp/PgDn: scroll  q: quit ┤
└ [input line, hidden until i pressed]                                                  ┘
```

- Activity lines: model text (wrapped), one line per tool call with ok/FAIL.
- Colors: model text default, tool ok green, FAIL red, ledger border cyan when
  it changed in the last 5s, status yellow while `verifying`, green `done`,
  red `aborted`.
- Steering: `i` opens the input line (Esc cancels, Enter submits → send to
  steering channel, close input). While input is open, keys go to the input.
- Scroll: PgUp/PgDn scrolls activity, auto-follow resumes at bottom.
- Terminal guard: raw mode + alternate screen entered on start, restored on
  ANY exit path including panic (install a panic hook that restores).

## Files

- `src/tui.rs` — new: App state (activity Vec<Line>, ledger String, status,
  scroll, input buffer), event reducer (`fn apply(&mut self, e: Event)`),
  draw + input loop.
- `src/events.rs` — new: Event enum + EventSink trait + ConsoleSink.
- `src/driver.rs` — emit events instead of println; abort flag + steering
  drain at iteration boundary.
- `src/main.rs` — `--tui` flag wiring: spawn driver thread, run UI on main.
- Deps to add: `ratatui`, `crossterm` (latest stable, default features).

## Tests (no terminal required)

- Reducer: every Event variant updates App state correctly (activity grows,
  ledger replaces, status transitions, usage accumulates).
- Steering drain: driver consumes two queued notes → two user messages in
  transcript in FIFO order.
- Abort flag: set before iteration N → driver aborts at that boundary with
  "operator abort" and exit-path semantics identical to budget abort.
- ConsoleSink golden: headless output lines unchanged vs pre-TUI format.

## Acceptance

- `cargo build`, `cargo clippy --all-targets -- -D warnings`, `cargo test` all
  clean.
- Headless smoke (existing fizzbuzz flow) produces identical log lines.
- TUI runs to goal completion under a pty without corrupting the terminal on
  exit.
