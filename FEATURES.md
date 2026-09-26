# FEATURES.md — chug capability roadmap

Benchmarks: Claude Code (tool depth, subagents, hooks, permissions, plan
mode), Codex (sandboxed parallel execution, approval modes), unreal-agent
(durable async operations, session forking), OmniGent-class multi-agent
orchestration. chug's mission twist: everything must ALSO serve the
autonomous loop, not just interactive use.

**Doctrine (META-META-SPEC §4): every fresh evaluation pulls the top
unworked roadmap item into TODO.md with a spec.** Incident rows coexist
with — never replace — the roadmap pull. An item may only be skipped with
a written reason in EVALUATION.md. Orchestrators check items off at merge
(in the row-flip commit).

Already landed: delegate (T23), wait_secs long-poll (T29), web_fetch (T37),
delegate resume (T61-63), driver.lock (T55), delegate collect (T69). ~~Parallel tool calls~~ —
retired: driver already batches (cycle-11 eval).

## Tier 1 — agentic core (pull first)

| # | Feature | What | Benchmark |
|---|---------|------|-----------|
| F1 | ~~**delegate collect**~~ — LANDED T69 (cycle 33, merge c6ce238) | Structured child result: goal_complete summary + commit refs + gates output returned to the caller (today: git/files archaeology). Makes delegate a real Task-tool equivalent for chat AND loops. | Claude Code Task |
| F13 | **Decision logs → Laya distillation** — SPLIT (cycle-34 eval): phase 1 → T70 (`decision_log` tool + `.chug/decisions.jsonl` + LOOP-SPEC adoption) **LANDED 572ec5a** (cycle 35); phases 2–3 deferred pending corpus + layad endpoint | Every loop judgment emits a structured decision record (`.chug/decisions.jsonl`): decision class, compact inputs, options, choice, model, confidence — with OUTCOME labels backfilled from git (landed-clean / fixed-up / reverted). Seed classes: validation verdicts (diff+spec→PASS/FAIL), does-this-item-need-kimi-validation (§2 step 4 routing), risk-gate (already logged), fallback/retry routing, eval accept/reject triage. Then: Laya fine-tune pipeline + confidence-gated first-pass (Laya decides ≥τ, else escalate to kimi) — classification ONLY, never completion/stuck/verdict-final judgments (measured failure class, SPEC-3 doctrine). Speeds the loop by shrinking kimi's share to the hard cases. | laya risk-gate precedent; risk_verdicts.jsonl corpus |
| F2 | **Plan mode** — SPLIT (cycle-36 eval): phase 1 → T73 (`chug plan` read-only planning mode: five read-only tools + `submit_plan` sole write-exit) **LANDED 87fe53f** (cycle 37); phase 2 deferred (`/plan` chat + `--approve` gate + web_fetch-in-plan — written reason EVALUATION.md cycle-36 §4) | `chug plan` / `/plan` in chat: read-only tool subset, model produces an implementation plan to stdout/file, exits. Optional `--approve plan.md` gate for `chug run`. | Claude Code plan mode |
| F3 | **Hooks** | `.chug/hooks.json` (or settings): shell commands on PreToolUse / PostToolUse / Stop / GoalComplete events. Policy-as-config, no doctrine forks. Laya stop-hook is the reference consumer. | Claude Code hooks |
| F4 | **Permissions policy** | Per-tool allow/deny/ask rules (glob-scoped paths, command patterns) evaluated before execution; `--risk-gate` becomes one policy source among several. Autonomous runs need deny-lists; chat needs ask-mode. | Claude Code / Codex approval modes |
| F5 | **Image input** | `read_file` on png/jpg returns image blocks (vision); chat accepts pasted/dragged screenshots. UI work needs eyes. | Claude Code |

## Tier 2 — session & steering depth

| # | Feature | What | Benchmark |
|---|---------|------|-----------|
| F6 | **Session fork** | Clone transcript+ledger at iteration N into a new session id; explore two approaches from one state. | unreal-agent forking |
| F7 | **Streaming UX** | Text deltas to sinks as they arrive (TUI live typing, headless progress); watchdog gets byte-level liveness for free. | all benchmarks |
| F8 | **Structured todo tool** | `todo_add/update/list` driver-visible tools (statuses enforced) as an alternative to freeform LEDGER edits — the orchestration ledger becomes queryable. | Claude Code tasks |
| F9 | **Slash-command packs** | `.chug/commands/*.md` repo-local commands invocable from chat (`/review`, `/triage`) and as run goals. Community-extensible without code. | Claude Code skills |

## Tier 3 — ecosystem reach

| # | Feature | What | Benchmark |
|---|---------|------|-----------|
| F10 | **chug as MCP server** | `chug mcp-serve`: expose run/delegate/status as MCP tools so Claude Code, the bridge fleet, or another chug can drive it. The fleet primitive. | unreal-agent remote ops; bridge fleet |
| F11 | **MCP resources+prompts** | Consume MCP resource/prompt capabilities (today: tools only). | MCP spec |
| F12 | **Web search** | Provider-pluggable search tool complementing web_fetch. | Claude Code |

## Working rules

- One item per evaluation pull, top-down within a tier; a tier may be
  reordered only with a written reason (dependency or measured incident).
- Every roadmap row is a FEATURE-class spec: repo context, requirements,
  tests, acceptance, out-of-scope — same bar as incident specs.
- Items can shrink: the evaluator may split a roadmap item into 2-3 rows
  when the full scope blows a 50-iter child budget.
