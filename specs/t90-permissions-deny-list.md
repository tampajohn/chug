# T90 — F4 phase 1: `.chug/permissions.json` deny-list

## Repo context

**ROADMAP PULL (mandatory, META-META-SPEC §4):** F4 (Permissions policy) is
the top unworked Tier-1 item in `FEATURES.md` — F1 landed T69, F13 phase 1
landed T70 (phases 2–3 deferred, layad absent), F2 phase 1 landed T73
(phase 2 deferred), F3 phase 1 landed T83 (cycle 49; phase 2 deferred). F4's
row: per-tool allow/deny/ask rules evaluated before execution;
`--risk-gate` becomes one policy source among several.

Phase 1 (this spec) is deliberately the smallest load-bearing slice: a
**deny-only, in-process** policy layer. Why this slice first:

- **It is the fail-closed layer T83 hooks deliberately are not.** Hooks
  evaluate policy in a spawned `sh -c` and FAIL OPEN on any spawn/config
  problem (src/hooks.rs; README Hooks). An in-process deny rule has no
  process to spawn and nothing to fail open through — a match denies,
  period. The two layers compose: declarative deny for the rules that must
  not depend on a shell, hooks for everything programmable.
- **Policy order is established here and phases 2–3 build on it:**
  permissions (in-process, this spec) → PreToolUse hooks (shell) → plan gate
  / MCP / risk gate (existing dispatch chain at src/driver.rs:843+).
- Benchmark: Claude Code `permissions.deny` (settings.json). chug's twist:
  the same config also governs autonomous-loop children **when the operator
  seeds it into the child's `.chug/`** (per-checkout semantics, hooks.json
  parity — see req 1 note).

Existing seams this spec reuses: `hooks::glob_matches` (src/hooks.rs:450 —
currently private; make it `pub(crate)` or move it to a shared home, same
semantics byte-identical); the T83 driver block/dispatch seam
(src/driver.rs:840–860 — `blocked` flag + veto-shaped ToolResult); the
T83 events telemetry pattern (src/events.rs HookFired/HookError variants +
src/eventlog.rs `hook`/`hook_error` lines); the T37/T70 new-module pattern
(all logic in one new `src/permissions.rs`; registration lines only
elsewhere).

## Requirements

1. **Config file**: `<cwd>/.chug/permissions.json` — per-checkout, absent or
   empty = zero rules and zero cost (a worktree child has its own `.chug/`;
   the main repo's rules do NOT auto-propagate into children — hooks.json
   parity, noted in README). No search chain, no CLI flag (phase 2 may add
   one).
2. **Shape** (deny-only in phase 1):
   ```json
   {"permissions": {"deny": [
     {"tool": "bash", "command": "*rm -rf*"},
     {"tool": "write_file", "path": "*.pem"},
     {"tool": "web_fetch"}
   ]}}
   ```
   A rule is `{"tool": "<glob>"}` plus AT MOST one arg matcher: `command`
   (glob against the bash command string), `path` (glob against the tool's
   `path` argument — file tools), or `url` (glob against web_fetch's url).
   A rule with no arg matcher denies the whole named tool. A rule whose arg
   matcher does not fit the matched tool (e.g. `command` on `read_file`),
   an unknown matcher key, or a non-object rule is a **malformed-rule leg**:
   that one rule is skipped (valid siblings still deny) and surfaces through
   the warn-once channel (req 5).
3. **Evaluation point**: in the driver's tool-call path BEFORE the T83
   PreToolUse hook check — a denied call fires no hooks, never reaches the
   plan gate / MCP / risk gate, and never executes. The deny result is a
   `ToolResult { is_error: true, content: "[permission denied] <rule summary>" }`
   (the T83 veto shape — the model routes around it and the loop continues),
   riding the same `blocked` flag so PostToolUse never fires on a deny
   (T83's reqs 2+3 semantics cover both block classes).
4. **Arg matching** uses the hooks glob matcher (`*`/`?`; `mcp__*` tool
   names match like any other). Missing/non-string arg (e.g. a bash call
   without a string `command`) under a rule WITH that matcher = the rule
   does not match (fail toward execution — the matcher had nothing to judge;
   whole-tool rules still deny).
5. **Fail-open on config problems, fail-closed only on rule match**:
   malformed JSON / wrong top-level type → ONE stderr warning per run + ONE
   `permission_error` events line + zero rules (T83 parity). A rule match
   always denies. New events variants (PermissionDenied / PermissionError)
   follow the T83 events.rs/eventlog.rs pattern: one `permission_denied`
   line per deny (tool + rule), one `permission_error` line per config
   problem; telemetry is best-effort and never changes run behavior.
6. **Surfaces**: run + chat + **plan mode** (in-process policy — it can only
   restrict further, e.g. denying `read_file` on `*.key` inside a plan
   session; the five-tool plan contract is unchanged). Load once per run at
   the T83 load seam (src/driver.rs:591-style, mode-keyed).
7. **Phase 2 deferred with written reasons** (named in README): allow-rules
   that short-circuit the risk gate (the F4 "one policy source among
   several" leg — needs risk-gate plumbing of its own), ask-mode (chat
   interactive prompt — a TUI/chat surface item), settings.json unification,
   and a `--permissions` CLI flag.
8. **README**: new `## Permissions (.chug/permissions.json)` section
   immediately after `## Hooks` (policy surfaces together): config shape,
   deny semantics, policy order (permissions → hooks → risk gate),
   fail-open/fail-closed rule, per-checkout note, phase-2 deferrals.
9. **Size guard (the T83 grinder lesson — T83 phase 1 took 136 child
   iterations across three runs)**: production code should land ≤ ~350
   lines; if the impl finds itself past that, split at the natural seam
   (config+matcher+tests first, dispatch/events wiring second) and land the
   first half only, noting the remainder on the TODO row.

## Tests

RED-proven killing tests for the deny path, plus:

- matcher reuse legs (`*`, `?`, `mcp__*` tool names; command/path/url globs);
- whole-tool deny (web_fetch example above);
- bash command-glob deny AND a non-matching command executes (non-vacuousness);
- path-glob deny on write_file AND read_file; missing/non-string `path`
  under a path-rule does not match;
- deny text shape: `[permission denied] …` naming the rule; the call never
  executes (a marker-file leg: denied bash never creates the file);
- ordering: with a hooks.json PreToolUse entry matching the same call, a
  permission deny fires NO hook (hook counter/marker stays zero) — policy
  order pinned;
- PostToolUse never fires on a deny (T83 blocked-flag parity);
- malformed legs: bad JSON, top-level array, rule with unknown matcher key,
  rule with tool-incompatible matcher — each: rule(s) skipped, warn-once,
  valid siblings still deny, run continues;
- absent/empty config = zero cost (no file read per call);
- plan mode: a deny rule restricts a plan-allowed tool (read_file `*.key`),
  the four read-only tools otherwise work;
- events legs: `permission_denied` line per deny, `permission_error` line on
  malformed config, none on the clean path.

## Acceptance

- `cargo build && cargo clippy --all-targets -- -D warnings && cargo test`
  green; README section integrated (not appended to the nearest unrelated
  section); FEATURES.md's F4 row gains its phase-1 annotation in the
  row-flip commit (orchestrator's check-off duty per META-META-SPEC §4).
- **Validation: REQUIRED (kimi)** — this item touches the dispatch chain
  (src/driver.rs) and src/events.rs, both on LOOP-SPEC §2 step 4's REQUIRED
  list. Mutation testing on the deny path (deny removed → calls execute;
  ordering flipped → hook fires first; matcher broken → over/under-match).

check: cargo test --bin chug permissions
