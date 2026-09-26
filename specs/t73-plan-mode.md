# T73 — `chug plan`: read-only planning mode (FEATURES.md F2, phase 1)

check: cargo test --bin chug plan && cargo test

## Repo context

FEATURES.md F2 is the top unworked Tier-1 roadmap item (F1 delegate collect
landed T69; F13 phase 1 landed T70, phases 2–3 deferred). Benchmark: Claude
Code's plan mode — a read-only session in which the model explores the repo
and produces an implementation plan, with zero mutation surface. chug's
mission twist (FEATURES.md header): it must also serve the autonomous loop —
an orchestrator can dispatch a plan child into a worktree to draft an
approach before burning an impl child.

Cycle-36 eval SPLIT F2 per the FEATURES.md working rules ("items can
shrink: the evaluator may split a roadmap item into 2-3 rows when the full
scope blows a 50-iter child budget"). Phase 1 (this row) is the core:
`chug plan` subcommand + read-only tool contract + a single deliberate
write/exit path. Phase 2 (deferred, NOT this row): `/plan` chat slash
command, `--approve plan.md` gate for `chug run`, web_fetch inside plan
mode.

Existing surfaces:

- `src/main.rs` (543 lines): clap CLI, `#[command(subcommand)]` at line 38;
  subcommands today are `run` and `chat` (T15 added `--max-tokens` to
  both). Run budgets: `--max-iters`/`--max-minutes`/`--max-tokens`.
- `src/driver.rs`: `pub fn run(cfg: RunConfig, sink) -> anyhow::Result<i32>`
  (line 205), `run_turn` (line 377), `Mode` enum, `TurnKnobs`. The loop
  exits when the model calls `goal_complete` (run mode) — plan mode needs
  its own deliberate exit (below).
- Tool registry: `src/tools.rs` (post-T71 home of read_file/write_file/
  edit_file/bash/grep/glob/list_dir/update_ledger/goal_complete/web_fetch),
  `src/delegate.rs` (delegate), `src/decisions.rs` (decision_log, on the
  webfetch.rs one-tool-per-file pattern). The tool list sent to the API is
  assembled in one place; dispatch is a single match.
- Test harness: `ScriptedLlm` (src/api.rs:677) + the
  `tool_use_response`/`text_only_response`/`ctx_for` helpers in
  src/driver.rs `mod tests` — the whole driver loop is drivable without a
  live API. Chat tests (src/chat.rs:354+) show the same harness on a second
  mode.
- Path sandbox: write_file/edit_file reject absolute paths and `..`
  escapes; plan mode's `--out` must inherit exactly that sandbox.

## Requirements

1. **New clap subcommand `chug plan`** in src/main.rs: `--goal` (required),
   `--spec` (optional spec file, same resolution as run), `--out <path>`
   (optional, cwd-sandboxed relative path for the plan file),
   `--model`, `--max-iters` (default 30), `--max-minutes` (default 20),
   `--max-tokens` (optional, T15 parity). Unknown/run-only flags are not
   silently accepted (clap's default rejection is fine).
2. **Read-only tool contract.** In plan mode the tool list sent to the API
   contains EXACTLY five tools: `read_file`, `grep`, `glob`, `list_dir`,
   and the new `submit_plan`. No other tool schema is advertised.
3. **New tool `submit_plan`** (plan-mode ONLY — absent from run/chat
   surfaces): input `{ "plan": string }` (required, min length 1). On
   dispatch: write the plan string VERBATIM to the `--out` path (creating
   parent dirs; rejected with a tool error if it escapes the cwd sandbox —
   same rule write_file uses) or print it to stdout when `--out` is absent;
   then end the loop successfully (exit 0; the events stream records the
   plan-completed outcome the same way run mode records goal acceptance —
   reusing the existing goal/verdict event machinery is fine and
   preferred).
4. **Defense in depth.** Even though the schemas are filtered, dispatch
   MUST also reject: a tool_use of any other registered tool name
   (`write_file`, `edit_file`, `bash`, `delegate`, `web_fetch`,
   `update_ledger`, `goal_complete`, `decision_log`) in plan mode returns
   a tool error naming the allowed set — it never executes, and the loop
   continues (the model can route around the error).
5. **run/chat surfaces byte-unchanged.** Their tool lists, goal_complete
   semantics, and exit codes are untouched; a regression pin proves the
   run-mode tool list is exactly the pre-change set.
6. **No bookkeeping writes.** A plan run never writes TODO.md and never
   appends to LEDGER.md (update_ledger is absent AND dispatch-rejected);
   `.chug/events.jsonl` observability parity with run mode is kept
   (run_start names the plan mode).
7. **Budget exhaustion without submit_plan** uses the existing abort path
   unchanged (nonzero exit, Aborted event, model/budget named).
8. **README.md**: a new short `## Plan mode (`chug plan`)` section
   integrated after Autonomous mode (read-only contract, the five-tool
   surface, `--out` semantics, one example invocation), plus one honest
   sentence in the Tools section naming the plan-mode surface. Integrated
   into the structure, not a bullet glued to the nearest section (README
   gate).

## Tests

All driver-level tests use the existing ScriptedLlm harness — NO live API.

- **Schema-filter pin**: the tool list a plan-mode run sends to the API is
  exactly the five names (set compare with exact cardinality — a sixth
  added or one dropped turns it RED).
- **Rejection sweep (the family, one leg per excluded tool)**: scripted
  tool_use of `write_file`, `edit_file`, `bash`, `delegate`, `web_fetch`,
  `update_ledger`, `goal_complete`, `decision_log` in plan mode → tool
  error naming the allowed set; the loop continues; no file was written,
  no command ran. Eight legs; each RED-proven by the child's own
  mutation/removal hand-check (T72 sweep-the-family doctrine: one vacuous
  leg is a FAIL-class finding at validation).
- **submit_plan end-to-end**: scripted submit_plan with a multi-line
  markdown plan → the `--out` file's bytes are exactly the plan string;
  exit code 0; the events stream carries the completion outcome.
- **stdout leg**: submit_plan without `--out` prints the plan (capture
  mechanism is the child's choice; pin the no-file-created behavior too).
- **Sandbox leg**: `--out ../x.md` (or absolute) → the plan is NOT
  written outside cwd; the tool error names the sandbox rule.
- **submit_plan absence pin**: run-mode and chat-mode tool lists do NOT
  contain submit_plan; plan-mode list does (one test, three assertions).
- **clap pins**: `chug plan --goal x` parses with defaults 30/20;
  `--max-iters 5 --max-minutes 3 --max-tokens 1000 --out p.md` parse;
  missing `--goal` errors.
- **Regression pin**: the run-mode advertised tool list equals the exact
  pre-change set (twelve tools today: read_file, write_file, edit_file,
  bash, grep, glob, list_dir, update_ledger, goal_complete, delegate,
  web_fetch, decision_log — if a rebase changes that set, update the pin
  in the same diff and say so).

## Acceptance

- `cargo build && cargo clippy --all-targets -- -D warnings && cargo test`
  green in the worktree (export
  `CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared` first).
- `check:` line passes verbatim from the worktree root.
- One live smoke the reviewer can eyeball: run the built binary
  `chug plan --goal "draft a plan to add a --version flag" --out
  /tmp-tmpdir-inside-worktree/plan.md` (a scratch dir INSIDE the worktree)
  and confirm a coherent plan file lands and no repo file was modified —
  record the outcome in the commit message or LEDGER.
- Commit message names the phase-2 deferrals explicitly (they are NOT
  implemented here).

## Out of scope

- `/plan` chat slash command (phase 2).
- `--approve plan.md` gate for `chug run` (phase 2).
- web_fetch inside plan mode (phase 2 reconsideration; phase 1 keeps the
  surface minimal and purely local).
- Any change to goal_complete, decision_log, delegate, or the run/chat
  paths beyond the byte-unchanged regression guarantee.
