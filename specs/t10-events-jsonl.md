# T10 — Append-only .chug/events.jsonl

check: cargo test

## Concern

Postmortems and meta-evaluations debug runs by mining
`.chug/transcript.jsonl`, which is lossy (231 of 510 lines in the current file
are `[trimmed]`) and, until T7 lands, splices sessions. The only durable local
log is the risk gate's `.chug/risk_verdicts.jsonl` (`src/riskgate.rs:132`);
Langfuse (SPEC-8) is optional and remote. The driver already has a structured
event stream (`src/events.rs` `Event`) — persist the useful slice locally so
`chug-meta`/`chug-meta-meta` sessions have a cheap, untrimmed source of truth
(EVALUATION.md §3/§4).

## Repo context

- `src/events.rs`: `Event` enum + `EventSink` (Console/TUI sinks exist).
- `src/driver.rs`: emits `Iteration`, `Usage`, `ToolStart`, `ToolResult`,
  `GoalAccepted`, `GoalRejected`, `Aborted`, `Verifying`, `SteeringQueued`.
- `src/transcript.rs`: the append/rotate pattern to mirror (T7 rotation should
  also rotate `events.jsonl` if both land — coordinate; either order works as
  long as the final state rotates both files together).
- `.chug/` is gitignored — no repo-pollution concern.

## Requirements

1. Append one JSON object per line to `<cwd>/.chug/events.jsonl` for: run
   start (model, spec path, cwd, mode), iteration (n, cumulative
   input/output tokens), tool result (name, ok, is_error, duration_ms,
   content preview ≤200 chars), verifying (command), goal accepted/rejected
   (reason), abort (reason).
2. Always on, zero config. Best-effort: any write/open failure is ignored
   (count or once-warn on stderr) — telemetry must never change run behavior.
3. No secrets: previews only of tool results, never of request bodies, auth
   headers, or ledger contents.
4. File lives next to the transcript and follows its lifecycle: fresh runs
   start a new file (after T7-style rotation when that lands).
5. Overhead must be negligible (one small write per event, no buffering
   threads).

## Tests

- Scripted `drive_loop` run (existing `ScriptedLlm` harness) →
  `events.jsonl` exists, every line parses as JSON, contains at least one
  tool-result and the terminal goal/abort event, and previews are ≤200 chars.
- Unwritable `.chug` (or injected write failure) → run still completes.
- MCP/tool error events recorded with `is_error: true`.

## Acceptance

- `cargo build`, `cargo clippy --all-targets -- -D warnings`, `cargo test` all
  green.
- A short real run leaves a `jq`-mineable events.jsonl.
