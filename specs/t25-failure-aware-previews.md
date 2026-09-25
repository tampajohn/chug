# T25 — Failure-aware event previews: error tool results keep a tail window

check: cd /Users/jadams/workspace/chug && cargo test

## Why (evidence)

Cycle-9 evaluation finding **N1**: the T23 impl child's suite run at
`2026-09-25T20:17:49` failed once (`FAILED. 379 passed; 1 failed`) and
the recorded event preview — `.chug/events-t23-impl-20260925-202220.jsonl`
— captured only `Finished … test result: FAILED` noise, **not the
failing test's name**. The child burned iterations 43–47 re-running the
suite 3× (all green) to rule out its own diff; the worktree transcript
(the only other place the name existed) was not harvested and is gone.
Cycle 9 re-ran the full suite 5× in main: 381+3 green every time — the
flake is unidentifiable and unreproduced. Root cause is the
observability hole, not the flake: failure bytes cluster at the **end**
of command output (cargo's `failures:` list, rustc's `error[Exxxx]`
blocks), but both preview seams keep a flat **head** window for ok and
error results alike.

## Repo context

Two seams, both flat head-takes today:

- `src/driver.rs:667` — the driver builds the event's preview from the
  tool result content: `preview: result.content.chars().take(500).collect()`
  (500-char head, unconditionally).
- `src/eventlog.rs:179` — the JSONL sink truncates again:
  `"preview": preview.chars().take(200).collect::<String>()`
  (200-char head, unconditionally). The `Event::ToolResult` in scope
  carries `ok`, and the serialized line already exposes
  `is_error: !ok` — the seam has everything it needs.
- Existing pin: `src/eventlog.rs:418`
  `sink_truncates_tool_preview_at_200_chars` (fixture preview =
  `"x".repeat(500)`). Check the fixture's `ok` value before editing —
  under the new behavior an error fixture of 500 chars would NOT
  truncate; the pin must assert the **ok leg**.
- Bash tool results already keep a 10,000-byte output tail
  (`src/tools.rs` `OUTPUT_KEEP_TAIL`), so the interesting failure bytes
  are present in `result.content` — they are simply never selected for
  the event preview.
- Consumers: `delegate`'s `summarize_events` (`src/tools.rs`) reads
  event `type`/flags, not preview content — no consumer parses preview
  internals. Console/TUI sinks use their own preview paths
  (`src/events.rs:186,267`) — **out of scope, byte-identical**.

## Requirements

1. **Ok leg byte-identical at both seams.** Successful tool results keep
   exactly today's behavior: driver preview = 500-char head of content;
   eventlog line = 200-char head of that. No constant renames that touch
   the ok path's values.
2. **Error leg tail-anchored.** When `result.is_error` (driver) /
   `!ok` (eventlog):
   - driver (`src/driver.rs`): preview = the **last** ≤2000 chars of
     `result.content` (char-boundary safe — `chars()`, never byte
     slicing; introduce a named const, e.g.
     `ERROR_PREVIEW_TAIL_CHARS: usize = 2000`).
   - eventlog (`src/eventlog.rs`): the serialized `preview` for error
     results passes through up to 2000 chars (`take(2000)` on the
     already-tail-selected driver preview — introduce
     `ERROR_PREVIEW_MAX_CHARS: usize = 2000`). Error previews shorter
     than 2000 chars are kept whole.
3. No new event fields, no schema changes to the JSONL line shape
   (`is_error` already exists); console/TUI sinks untouched; rotation
   and best-effort semantics (T10) untouched.
4. README: one bullet/phrase update in the events.jsonl documentation
   noting error previews are tail-anchored with a larger window (the
   T10-era README text documents the ≤200c preview — it must stay
   truthful).

## Tests

- driver.rs test module: an `is_error: true` tool result whose content
  is >2000 chars with a unique marker at the very end → the emitted
  `Event::ToolResult` preview **ends with** the marker and is ≤2000
  chars; an `is_error: false` result keeps the 500-char head (existing
  behavior pinned).
- eventlog.rs test module:
  - ok leg: preview of 500 chars → 200 kept (the existing pin, fixture
    `ok` confirmed/adjusted to `true` — assert head-truncation exactly
    as today);
  - error leg, long: `ok: false` + 2500-char preview → exactly 2000
    chars serialized;
  - error leg, short: `ok: false` + 300-char preview → all 300 kept
    (this is the non-vacuous heart — pre-T25 code truncates it to 200);
  - multibyte leg: a >2000-char error preview containing multibyte
    chars serializes without panic and within the char budget.
- Non-vacuousness (implementer demonstrates once per seam): reverting
  either seam to the flat head-take kills the new error-leg tests.

## Acceptance

- `cargo build`, `cargo clippy --all-targets -- -D warnings`, `cargo
  test` all green.
- Adversarial validation (REQUIRED — touches `src/driver.rs`, LOOP-SPEC
  §2.4): confirm both seams' ok legs are byte-identical to pre-T25, the
  error legs are tail-anchored at the driver and window-passing at the
  sink, mutants (revert driver to `take(500)`; revert sink to
  `take(200)`; head-instead-of-tail at the driver) all die, and no
  pre-T25 pin was weakened.
