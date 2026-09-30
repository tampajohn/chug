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
| F2 | **Plan mode** — SPLIT (cycle-36 eval): phase 1 → T73 (`chug plan` read-only planning mode: five read-only tools + `submit_plan` sole write-exit) **LANDED 87fe53f** (cycle 37); phase 2 SPLIT (cycle-70 eval): 2a **LANDED 55d59c3** (cycle 70 — `chug run --approve plan.md` refuse-before-`.chug` gate + contract injection + `run_start` approve/plan_sha256 fields; web_fetch admitted to plan mode's read-only set, six-tool contract); 2b (`/plan` chat toggle) deferred — chat-only UX, no loop consumer (EVALUATION.md cycle-70 §4) | `chug plan` / `/plan` in chat: read-only tool subset, model produces an implementation plan to stdout/file, exits. Optional `--approve plan.md` gate for `chug run`. | Claude Code plan mode |
| F3 | **Hooks** — SPLIT (cycle-47 eval): phase 1 → T83 (`.chug/hooks.json` PreToolUse veto + PostToolUse advisory) **LANDED ccb828a** (cycle 49); phase 2 deferred (Stop/GoalComplete events, arg-glob matchers, Laya stop-hook consumer — layad endpoint absent, EVALUATION.md cycle-47 §4) | `.chug/hooks.json` (or settings): shell commands on PreToolUse / PostToolUse / Stop / GoalComplete events. Policy-as-config, no doctrine forks. Laya stop-hook is the reference consumer. | Claude Code hooks |
| F4 | **Permissions policy** — SPLIT (cycle-51 eval): phase 1 → T90 (`.chug/permissions.json` deny-list — in-process, fail-closed on match, first in the policy chain before hooks + risk gate) **LANDED e9afed9** (cycle 52); phases 2+ deferred with written reasons (allow-rules short-circuiting the risk gate, ask-mode, settings.json unification, `--permissions` CLI flag — see README Permissions section) | Per-tool allow/deny/ask rules (glob-scoped paths, command patterns) evaluated before execution; `--risk-gate` becomes one policy source among several. Autonomous runs need deny-lists; chat needs ask-mode. | Claude Code / Codex approval modes |
| F5 | **Image input** — SPLIT (cycle-53 eval): **phase 1 LANDED** (T91, merge ae7ff5f — `read_file` on png/jpg/jpeg/gif/webp returns base64 image content blocks + 5 MiB guard + endpoint-reject degrade latch; chat paste/drag NOT landed); phase 2 deferred (chat paste/drag of screenshots — TUI + terminal-capability work with no loop consumer; rides the F7 surface more than the tool surface — written reason EVALUATION.md cycle-53 §4) | `read_file` on png/jpg returns image blocks (vision); chat accepts pasted/dragged screenshots. UI work needs eyes. | Claude Code |

## Tier 2 — session & steering depth

| # | Feature | What | Benchmark |
|---|---------|------|-----------|
| F6 | **Session fork** — SPLIT: **phase 1 landed (T105, dc29137, cycle 58)** — `chug fork save/list/restore` named slots over transcript+ledger (serial explore-two-approaches; restore archives the live session first). Phase 2 DEFERRED: `--session <name>` concurrent path plumbing + fork-at-iteration-N surgery — the serial slot covers the benchmark shape; same-cwd concurrency is structurally barred by the driver lock and parallel exploration already has worktrees+delegate | Clone transcript+ledger at iteration N into a new session id; explore two approaches from one state. | unreal-agent forking |
| F7 | **Streaming UX** — phase 1 LANDED (2a51cc5, cycle 59: stream:true transport + SSE accumulation + console text deltas + `CHUG_STREAM=0` + first-per-run latched fallback telemetry); phase 2 DEFERRED (TUI live typing / progressive tool-input rendering — rides the tui.rs redraw architecture, chat-only surface with no loop consumer; phase 1's console deltas cover the loop's headless-liveness need — written reason EVALUATION.md cycle-59 §4) | Text deltas to sinks as they arrive (TUI live typing, headless progress); watchdog gets byte-level liveness for free. | all benchmarks |
| F8 | **Structured todo tool** — SPLIT (cycle-60 eval): **phase 1 LANDED** (T111, d8fdea0+fc1d691, cycle 60 — `todo_add`/`todo_update`/`todo_list` + `.chug/todos.json` + `## Todos` prompt injection); phase 2 deferred (remove/deps/chat-command/fork integration — written reason EVALUATION.md cycle-60 §4) | `todo_add/update/list` driver-visible tools (statuses enforced) as an alternative to freeform LEDGER edits — the orchestration ledger becomes queryable. | Claude Code tasks |
| F9 | **Slash-command packs** — PHASE 1 LANDED (T113, 55fe207 cycle 61: `.chug/commands/*.md` discovery + `$ARGUMENTS` expansion + chat invocation + `/help` line); PHASE 2a LANDED (T117, 3579d9d cycle 62: run-side `chug run/plan --goal "/name args"` CLI-boundary expansion + `goal_pack` in run_start + hash-difference honesty line); PHASE 2b LANDED (T118, cycle 62: frontmatter `description:` parse+strip with lenient unclosed-fence fallback + `/help` `/name — description` lines + pack-name Tab completion under the built-ins-win shadow rule — F9 phase 2 closed; `allowed-tools` et al. semantics remain a later phase) | `.chug/commands/*.md` repo-local commands invocable from chat (`/review`, `/triage`) and as run goals. Community-extensible without code. | Claude Code skills |

## Tier 3 — ecosystem reach

| # | Feature | What | Benchmark |
|---|---------|------|-----------|
| F10 | **chug as MCP server** — SPLIT (cycle-64 eval): phase 1 → T124 **LANDED d6264be** (cycle 64 — `chug mcp-serve` stdio JSON-RPC server skeleton + read-only `chug_status` over any cwd); phase 2 SPLIT (cycle-65 eval): phase 2a → T128 **LANDED 85ca4c1** (cycle 65 — read-only `chug_collect`: latest-segment verdict/goal-summary/check-cmd/liveness/commit-refs over delegate's collect seams); phase 2b → T129 **LANDED 2b4490b** (cycle 69 — `chug_launch` write leg flag-gated by `--allow-launch`: default-deny policy boundary, advertised ⇔ callable, spawn through the one delegate launch path, ceilings 200/240 reject-above; phase 2 CLOSED — the fleet shape is proven end to end); phase 3 SPLIT (cycle-72 eval): phase 3a → T153 **LANDED 675076d** (cycle 73 — `chug_cancel`, the second write leg behind the SAME `--allow-launch` boundary — ownership re-derived fail-closed per call from process identity (alive → pgid==pid detached fingerprint → `run --spec` argv), SIGTERM-group → one bounded SIGKILL escalation; EPERM/zombie fix-up 6e92f84 made the dead-poll deterministic); phase 3 control verbs → T157 **LANDED** (this cycle — `chug_abort` (run-level: same ownership + TERM→grace→KILL discipline, terminal states `aborted|already-done|not-found`, the abort RECORDED in the child's `.chug/events.jsonl` with the distinctive `operator abort via chug_abort` reason so the read tools report the run as aborted) + `chug_steer` (operator note into a running child via the driver's EXISTING `[operator]` mechanism over the new cross-process queue `.chug/steer.jsonl`, drained by drive_loop at the iteration boundary and landed as an `[operator]` transcript message; queued vs undeliverable, nothing written when undeliverable) behind a SECOND independent default-deny flag `--allow-control` (abort+steer together, advertised ⇔ callable; mcp__ prefix permission rules apply per T90); `chug_delegate` EVALUATED and DEFERRED with a written reason — see the T157 commit message); phase 3b (notifications/resources/server log) still deferred — no consumer pulls MCP-spec-completeness surfaces | `chug mcp-serve`: expose run/delegate/status as MCP tools so Claude Code, the bridge fleet, or another chug can drive it. The fleet primitive. | unreal-agent remote ops; bridge fleet |
| F11 | **MCP resources+prompts** — SPLIT (cycle-76 eval); **PHASE 1B COMPLETE** (cycle 80): phase 1a LANDED (T162, 8120b00, cycle 77 — resources/list + resources/read consume legs on the stdio registry path + capability catalog); phase 1b-i LANDED (T169, af4ff7a, cycle 79 — the model-facing `mcp_resource` builtin: list/read actions, capability-gated named errors, T90-matchable); phase 1b-ii LANDED (T170, 88119dc, cycle 80 — prompts/list+get on stdio, shared mappers, MAX_MCP_PROMPTS warn-and-cap); phase 1b-iii LANDED (T171, 7b9b2bf, cycle 80 — all four legs over the HTTP transport reusing the shared mappers, capability gating single-sourced, named-error parity). Remaining (later phases): the model-facing prompts surface (F9), resources/templates, subscriptions | Consume MCP resource/prompt capabilities (today: tools only). | MCP spec |
| F12 | **Web search** | Provider-pluggable search tool complementing web_fetch. | Claude Code |

## Working rules

- One item per evaluation pull, top-down within a tier; a tier may be
  reordered only with a written reason (dependency or measured incident).
- Every roadmap row is a FEATURE-class spec: repo context, requirements,
  tests, acceptance, out-of-scope — same bar as incident specs.
- Items can shrink: the evaluator may split a roadmap item into 2-3 rows
  when the full scope blows the filing-time ~500-line estimate ceiling
  (META-META-SPEC's spec quality bar; mechanical byte-identical move rows
  are exempt — the old 50-iter child budget predated the T21/T92/T102
  raises to 80).
