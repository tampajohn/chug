# chug spec 3 — file-discovery tools + laya risk gate

Two additions. Keep each small; all existing behavior unchanged unless flagged.

## 1. File-discovery tools (close the Claude Code gap)

Add two tools to the existing 7 (schemas + dispatch in tools.rs):

- `glob {pattern, path?}` — match files under cwd (or under `path` relative to
  cwd) using the `glob` crate. Return newline-separated relative paths, sorted,
  capped at 200 entries with a truncation note. Same path-safety rules as the
  other file tools.
- `list_dir {path?}` — list immediate entries of a directory (default cwd),
  one per line, directories suffixed with `/`, sorted (dirs first), capped at
  500 with a truncation note.

Also extend `edit_file` with an optional `replace_all: bool` (default false).
When true, replace every occurrence and return the replacement count. Existing
single-match behavior (error on 0 or >1 matches) unchanged when false/absent.

## 2. Laya risk gate (`--risk-gate`)

chug runs unattended; this is the semantic pre-execution safety layer.

- New flag: `chug run --risk-gate`. Env `LAYA_URL`, default
  `http://127.0.0.1:8420`.
- When enabled, BEFORE executing any `bash` tool call, POST
  `{LAYA_URL}/judge` with body:
  ```json
  {"state": {"context": "An autonomous coding agent working toward a goal in a project directory wants to execute a bash command.", "command": "<the command>"},
   "questions": {"risk": {"type": "choice",
     "instructions": "Judge whether the bash command is destructive or hard to reverse: permanently deletes files or state that cannot be regenerated, force-pushes, drops data, kills processes it does not own, modifies things outside the project directory.",
     "criteria": {"destructive": "permanently destroys work or state that cannot be regenerated, or escapes the project directory",
                  "risky": "reversible but disruptive, or destructive only to regenerable artifacts (build outputs, caches, node_modules)",
                  "safe": "read-only or fully reversible within the project"}}}}
  ```
  Timeout 2s.
- Decision: block when `answers.risk.probabilities.destructive >= 0.5`. A
  blocked command is NOT executed; return a tool_result with `is_error: true`
  and content: `BLOCKED by risk gate (destructive p=<p>): <command>. Choose a
  safer alternative, or wait for an operator steering note: 'allow
  destructive' disables the gate for the rest of this run.`
- `risky`/other: execute normally.
- Operator override: a steering note whose trimmed lowercase content equals
  `allow destructive` disables the gate for the remainder of the run (emit an
  event + note it in the transcript).
- Fail-open: layad unreachable, non-200, malformed JSON, or timeout → execute
  the command, and record the failure in the log below.
- Logging: append every judgment (or failure) as one JSON line to
  `<cwd>/.chug/risk_verdicts.jsonl`: `{ts, command_preview(200), choice,
  p_destructive, blocked, gate_failure?}`.
- Events: add `Event::RiskVerdict { blocked: bool, choice: String, p: f64,
  preview: String }` emitted after each judgment; ConsoleSink prints
  `[chug] risk gate: <choice> (p=<p>) <blocked?>`; TUI shows it in the
  activity stream (red if blocked, yellow otherwise). Update the ConsoleSink
  golden test fixtures to tolerate (not require) the new line — existing
  lines stay byte-identical.

Design constraint (learned the hard way elsewhere): laya does text
classification ONLY — no counting, negation, or completion judgments. The
gate's only input is the command string + fixed context.

## Tests (no network)

- glob: nested pattern matches, cap+truncation note, path-escape rejected.
- list_dir: dirs-first with `/` suffix, cap behavior.
- edit_file replace_all: replaces N occurrences, returns count; default path
  still errors on multi-match.
- Risk-gate decision logic behind an injectable judge (trait object or fn
  pointer): canned verdict destructive p=0.7 → blocked, command NOT executed,
  error tool_result text contains "BLOCKED"; p=0.4 → executed; judge error →
  executed (fail-open) + failure logged; `allow destructive` steering note →
  gate disabled for subsequent commands.
- Event emission for each judgment.

## Acceptance

`cargo build`, `cargo clippy --all-targets -- -D warnings`, `cargo test`
clean. Headless log format unchanged when `--risk-gate` is absent.
