# T83 — F3 hooks phase 1: `.chug/hooks.json` (PreToolUse veto + PostToolUse advisory)

check: cargo test

## Repo context

- FEATURES.md F3 (Tier 1, top unworked): "`.chug/hooks.json` (or settings):
  shell commands on PreToolUse / PostToolUse / Stop / GoalComplete events.
  Policy-as-config, no doctrine forks. Laya stop-hook is the reference
  consumer." Benchmark: Claude Code hooks. This spec is PHASE 1 of the
  cycle-47 eval's split: the two tool-cycle events only.
- Tool calls dispatch in `src/driver.rs` (`drive_loop`); tool schemas live
  in `src/tools.rs`; the event stream types live `src/events.rs` /
  `src/eventlog.rs` (`.chug/events.jsonl`, one JSON object per line).
- Veto precedent: the risk gate (`src/riskgate.rs`, `--risk-gate`) blocks a
  `bash` call by returning a tool error the model sees and routes around;
  the loop continues. PostToolUse advisory precedent: steering notes and
  the truncation/budget-low advisories appear as injected text the model
  reads.
- `.chug/` is gitignored: hooks.json is per-checkout operator config (a
  worktree child has its own `.chug/`; phase-1 semantics are cwd-local —
  an orchestrator that wants hooks in a child writes the child's
  `.chug/hooks.json` itself via bash).
- Config-load precedent: `mcp.json` loading (first-found-wins, per-entry
  fail-soft with a `.chug/mcp-<name>.log` note, never aborts).

## Requirements

1. **Config**: `.chug/hooks.json` in the run's cwd (no search chain, no
   CLI flag this phase). Shape:
   ```json
   {"hooks": {
     "PreToolUse":  [{"match": "bash",  "command": "./.chug/hooks/gate.sh"}],
     "PostToolUse": [{"match": "edit_*", "command": "..."}]
   }}
   ```
   `match` is a glob on the tool name (`*`/`?` semantics; `bash`,
   `edit_file`, `mcp__*` — MCP tools match on their registered
   `mcp__<name>__<tool>` name like any other, no special-casing). Absent
   file, empty file, or an empty/absent event list = zero hooks, zero
   cost. Malformed JSON or an unreadable file = warn ONCE per run on
   stderr + one events.jsonl error line, then run with zero hooks
   (fail-open — hooks are policy, never a run-killer).
2. **PreToolUse**: before a tool executes, fire every matching hook:
   spawn `command` via `sh -c` in its OWN PROCESS GROUP (the T6/bash-tool
   pattern — a hook timeout SIGKILLs the whole group), cwd = the run cwd,
   JSON on stdin: `{"event":"PreToolUse","tool":<name>,"input":<the tool
   call input object>,"cwd":<run cwd>}`. Bounded: 10s wall cap.
   - exit 0 → allow (stdout/stderr ignored for the allow leg).
   - non-zero exit → VETO: the tool does NOT execute; the model receives
     a tool-error result whose text is `[hook veto] ` + the hook's stderr
     (tail-anchored, ≤2000 chars, the T25 shape); the loop continues
     (same shape as a risk-gate block).
   - spawn failure / timeout / any hook-side error → fail-open: allow the
     call, warn once per run on stderr + one events.jsonl error line.
3. **PostToolUse**: after a tool executes (ok or error), fire every
   matching hook with the same stdin shape (`"event":"PostToolUse"`, plus
   `"is_error":<bool>`). Advisory only: non-empty hook stdout+stderr
   (tail-anchored ≤2000 chars) is APPENDED to the tool result content as
   `\n\n[hook] <text>` (with ` (exit <n>)` when non-zero). Never blocks,
   never changes the result's ok/is_error, never re-fires (a hook does
   not trigger hooks). Same 10s cap, process group, fail-open.
4. **events.jsonl**: one line per hook fire —
   `{"type":"hook","event":"PreToolUse"|"PostToolUse","tool":...,
   "command":...,"exit":<n|null>,"veto":<bool>,"duration_ms":...}` —
   plus the one-line config/spawn error records above. Best-effort like
   every event write (a logging failure never aborts).
5. **No new surfaces**: run and chat get hooks (same drive path); plan
   mode does NOT fire hooks (its tool contract is exactly five tools —
   hooks would be a sixth behavior; note the exclusion in the README
   clause); TUI unchanged; no Stop/GoalComplete events (phase 2); no
   arg-glob matchers (phase 2); no `--hooks` CLI flag (phase 2 candidate).
6. **README**: new `## Hooks (.chug/hooks.json)` section between
   `## Risk gate` and `## MCP servers`: config shape, the two events,
   veto/advisory semantics, the bounds (10s, process group, ≤2000c,
   fail-open), the cwd-local/gitignored note, and one line naming phase-2
   scope (Stop/GoalComplete, arg globs, the Laya stop-hook consumer).

## Tests

- Config: absent/empty/malformed file legs; glob matcher (`bash`,
  `edit_*`, `mcp__*`, `*`; non-match); multiple entries same event in
  order.
- PreToolUse: exit-0 allow (tool executes); exit-2 veto (tool does NOT
  execute — prove with a bash call that would have created a file; the
  tool error carries `[hook veto]` + the hook's stderr); a vetoed call
  leaves the loop alive (next turn proceeds — drive_loop-level test).
- PostToolUse: output appended (`[hook] ...` in the tool result the model
  receives); non-zero exit noted; empty output appends nothing; ok AND
  error tool results both fire.
- Bounds/fail-open: hook that sleeps past the cap is killed (test-only
  seam injecting a short timeout — the api.rs retry_delays seam pattern,
  not a slow wall-clock test), call allowed, one error event; hook that
  does not exist → fail-open + one warn; hook killing its own group child
  cannot orphan grandchildren (process-group spawn asserted by shape).
- events.jsonl: fire line fields (event/tool/exit/veto/duration);
  malformed-config error line exactly once despite two tool calls.
- Plan mode: a plan session with a hooks.json present fires NOTHING.
- RED-prove the load-bearing legs per the T72 doctrine: each new
  behavior test names the mutant it kills (veto leg kills allow-by-default;
  timeout leg kills unbounded wait; process-group leg kills plain spawn).

## Acceptance

- `cargo build` + `cargo clippy --all-targets -- -D warnings` +
  `cargo test` green in the worktree.
- Live smoke by the impl child: a run-mode hooks.json that vetoes
  `bash` commands containing a marker string works end-to-end (model sees
  the veto error and routes around), and a PostToolUse echo hook's note
  lands in the tool result.
- Validation: REQUIRED (kimi) — driver.rs + the tools surface are on the
  LOOP-SPEC step-4 REQUIRED list.

## Out of scope

- Stop/GoalComplete events; arg-glob matchers; `--hooks` CLI flag;
  settings.json chain; TUI surfacing; the Laya stop-hook reference
  consumer (layad endpoint absent — F13-p2's blocker).
