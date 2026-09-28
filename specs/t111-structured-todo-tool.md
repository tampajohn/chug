# T111 — F8 phase 1: structured todo tool (todo_add / todo_update / todo_list + .chug/todos.json + prompt injection)

check: cargo test --bin chug

## Repo context

FEATURES.md **F8 (Structured todo tool)** — the top unworked roadmap item
(Tier 2; F6-p1 and F7-p1 landed in cycles 58/59): "`todo_add/update/list`
driver-visible tools (statuses enforced) as an alternative to freeform
LEDGER edits — the orchestration ledger becomes queryable." Benchmark:
Claude Code's task tools. SPLIT per the working rules: this is phase 1;
phase 2 (remove/deps/chat-command/fork integration) is deferred — written
reason in EVALUATION.md cycle-60 §4.

Loop consumers (the mission twist — every feature must serve the loop, not
just chat):

- **T63-resume re-orientation**: a budget-death resume (21 in the era)
  re-derives "what was I doing" by replaying the transcript tail; a
  persisted `.chug/todos.json` gives the resumed child its step list with
  statuses in one read. (Worktree children start with no `.chug/` → empty
  list by construction; resumes inherit the file — the correct semantics
  fall out of the cwd-confined store.)
- **jq-mineable child state**: orchestrators and validators can read
  `.chug/todos.json` to see a child's self-declared plan and progress
  without transcript archaeology (same value class as
  `.chug/events.jsonl` over transcripts — T46/T60 doctrine).
- Chat parity with the benchmark (Claude Code tasks).

Existing patterns to follow (read them first):

- `update_ledger` is the bookkeeping-tool precedent: schema in
  `src/tools.rs` (~line 129), dispatch arm (~line 210), handler
  (`update_ledger(ctx, input)`, ~line 723) overwriting `LEDGER.md` in cwd.
- System-prompt assembly in `src/driver.rs`: run mode
  `format!("{PREAMBLE}\n\n## Spec...## Ledger\n\n{ledger_text}")` (~line
  1419); chat mode appends `## Ledger` (~line 1453); plan mode renders
  Ledger read-only and excludes write tools (`src/plan.rs`).
- `.chug/` is gitignored runtime state; `src/decisions.rs` is the
  `.chug/*.jsonl` append precedent (compact writes, no fsync ceremony).
- Tool errors name the remedy (T41/T85/T88 doctrine): an invalid status
  names the valid set; an unknown id names the existing ids.

## Requirements

1. **Store**: `.chug/todos.json` in the process cwd — a JSON array of
   `{"id": "t<N>", "title": string, "status": "pending"|"in_progress"|"done"}`.
   Ids allocate as `t1`, `t2`, … (next id = array length + 1; v1 has no
   removal, so ids are stable). Missing file = empty list. Corrupt file =
   every todo tool returns a tool error naming the file (never panic,
   never silently discard).
2. **Tools** (run + chat modes; EXCLUDED from plan mode's read-only set
   exactly like `update_ledger`):
   - `todo_add` `{title}` → returns the allocated id; appends with status
     `pending`.
   - `todo_update` `{id, status?, title?}` — id must exist (error names
     the existing ids); `status` must be one of the three (error names
     the valid set); at least one of `status`/`title` required (error
     otherwise).
   - `todo_list` `{}` → the compact rendered list
     (`t3 [in_progress] title`), or `no todos` when empty.
   Writes go through one `save()` that rewrites the file compactly;
   follow the repo's existing `.chug/` write pattern (no new deps).
3. **Prompt injection**: when the list is non-empty, the system prompt
   gains a `## Todos` section AFTER `## Ledger` in run mode AND chat mode
   (same `t3 [in_progress] title` rendering); when empty, NO empty heading
   is emitted. Plan mode renders nothing about todos (no write tools → no
   surface).
4. **No new events**: the `tool_result` records are the audit trail
   (update_ledger precedent); nothing is appended to `events.jsonl`.
5. **README.md**: the Tools section gains the three tools — INTEGRATED
   into the existing tool list (bookkeeping tools group beside
   `update_ledger`), not a bolted-on bullet at the end.
6. **Out of scope (phase 2, do not build)**: `todo_remove`, blocked-by /
   dependencies, a chat `/todo` command, fork-slot integration
   (save/restore of todos), per-run reset semantics, MCP exposure,
   sub-task hierarchies.

## Tests

- Unit (in the new module): round-trip load/save; id allocation sequence;
   status enum acceptance of all three + rejection of a bad value with the
   valid set named; unknown-id error naming existing ids; corrupt-file
   error path (no panic); empty-file bootstrapping.
- Dispatch (tools.rs): `todo_add` → id returned; `todo_update` flips
   status; `todo_list` rendering incl. the `no todos` empty case;
   `todo_update` with neither status nor title errors.
- Injection (driver.rs): `## Todos` section present with the right
   rendering when todos exist, absent when the list is empty (both modes'
   prompt-assembly paths covered per the existing Ledger-section tests'
   pattern).
- Plan mode: todo tools are NOT in the plan tool set (assert alongside
   the existing update_ledger exclusion assertion if one exists, else in
   the module tests via the plan tool-list builder).
- RED-prove at least the status-enforcement and unknown-id legs (mutate
   the validation away, watch the test die, revert) — record the RED legs
   in the commit message.

## Acceptance

- `check:` green in the worktree (whole suite — the tool-registration and
  prompt-assembly surface is broad).
- `.chug/todos.json` created lazily on first `todo_add`; a fresh worktree
  (no `.chug/`) behaves as empty-list throughout.
- README Tools section documents all three tools, integrated.

estimate: ~400 changed lines (new `src/todos.rs` ~180 with tests,
tools.rs schema+dispatch ~60, driver.rs injection ~20, driver tests ~60,
README ~10) — under the T110 ceiling
