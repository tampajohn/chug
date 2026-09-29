# EVALUATION — chug, assessed by chug-loop (2026-09-29, cycle 70)

**MANDATORY fresh eval** — the queue is EMPTY again (cycle 69 landed both
cycle-68 deferrals, T129 `2b4490b` + T131 `b52e62c`, and its wrap commit
`caf592c` declared "queue now EMPTY -> next cycle fresh-evals on kimi"),
so the freshness predicate's queue half fails and loopd routed kimi per
T81. The delta corpus since the cycle-65 eval text is cycles 66–69: four
orchestrator streams and ~25 child streams (t128–t143 impl/validate/
fix-up/resume segments), read via `.chug/eval-digest.md` (346 events
files, 17,964 iterations — fresh at generation, T131's new
self-exclusion check rendering FRESH) with targeted drills into
`decisions.jsonl` for the carried findings. The headline: **the loop
absorbed its heaviest adversarial era yet** — 15 items landed in 4
cycles (9 codex-review HIGHs, 4 carried queue rows, 2 feature phases),
two self-cut release tags (v0.4.0, v0.4.1) shipped green, every budget
death was absorbed by a T63 resume (7-for-7), and adversarial validation
caught three real FAIL classes pre-merge (T134 symlink exfil, T137
pipefail fail-open, T139 gate-reorder weak tests) — while the intake
process itself exposed this eval's filings: bulk-filed specs skipped the
estimate bar (→ T150), and a goal-gate executed a foreign worktree's
tests twice (→ T144). The mandatory roadmap pull is **F2 phase 2,
SPLIT → T146 (`--approve` gate + web_fetch-in-plan)**. Seven rows filed:
T144–T150.

## 1. What chug does well — be brief

- **T63 resume is the era's quiet hero**: 7-for-7 budget-death absorptions
  across cycles 66–69 (t134-fixup, t135, t136, t138, t138-validate,
  t139, t143) — zero lost work, zero orchestrator-finishes, zero
  next-cycle recoveries. The 80/80 death is now a ~15-minute tax, not a
  failure mode.
- **Adversarial validation has teeth in fact**: three FAIL rounds this
  era, each catching a class the gates alone would have shipped — T134's
  glob-metachar `/etc/passwd` exfiltration through an in-tree symlink,
  T137's pipefail fail-open single-driver guard (proven byte-exact both
  directions), T139's gate-reorder mutants surviving weak tests. All
  three fix-ups re-PASSED with the class swept.
- **The truncation class is CLOSED**: T141 (retryable classification of
  truncated SSE) + T143 (max_tokens 8192→32768 default) landed cycles
  66/68; zero `stuck: repeated error` or malformed-tool-JSON aborts in
  any post-`00adb88` stream (t139/t129/t131 children + two orchestrator
  streams verified in the digest).
- **F10 phase 2 CLOSED**: `chug_collect` (T128) + flag-gated
  `chug_launch` (T129) — the MCP fleet shape is proven end to end with
  a default-deny policy boundary; two self-cut tags (v0.4.0/v0.4.1)
  shipped with green release workflows.
- **T129's descope clause fired exactly as designed**: the estimate line
  named the e2e drop condition at filing time; the impl measured
  (989 → 814 lines) and dropped ONLY the e2e, trimming zero validation
  legs. Estimate discipline working at work-time.

## 2. Incidents worth fixing

- **I1 (→ T144, pri 1, bug): the goal-gate check harness executed a
  FOREIGN worktree's test binary — twice.** `d1790632587-1` (T135's
  validator watched its `goal_complete` check run T142's
  `cycle_count_fallback` test, absent from its own tree) and the T135
  round-1 goal-reject (`d1790640546-3`). Mechanism fully traced:
  `loopd.sh:319` launches the orchestrator with
  `CARGO_TARGET_DIR=target-shared` as an env prefix → children inherit
  it → `verify()`'s `run_shell` (src/tools.rs:1031) spawns the check
  with the driver's full env → cargo's path-independent artifact names
  make the shared dir last-builder-wins (the T52 mechanism, on a surface
  T52 never covered: in-driver spawns). Observed legs were false-RED;
  the symmetric false-GREEN (gate runs another checkout's green tests
  and ACCEPTS) is silent. Root cause: env inheritance across a spawn
  boundary that was designed for explicit per-command prefixes only.
- **I2 (→ T149, pri 3, doctrine): validator budget deaths 4 in 4
  cycles.** t134-validate 50/50, t137-validate died at **30m08s** (the
  MINUTES ceiling — verdict written, unannounced; a full re-launch
  burned ~11 min wall), t138-validate 50/50, t142-validate 50/50. T79
  parallel mutants + cold `target-shared-validate` builds eat both
  budgets; 50/30 predates that shape. The 60/40 raise is the
  validator-side T102.
- **I3 (→ T150, pri 3, tooling guard): the codex intake filed 10/10
  specs with NO `estimate:` line** (T134–T143, verified by grep), and 4
  of those 10 rows died 80/80 mid-work (T136/T138/T139/T143). The T110
  filing-time ceiling can't bite on an unestimated row. 121/141 total
  specs predate the rule, so the pin binds `todo`-status rows only
  (filing time), not history.
- **I4 (→ T145, pri 2, robustness): T136's crash-safety class left
  `update_ledger` unswept** (validator carry `d1790643172-7`).
  LEDGER.md — the external memory every child reads first — is still
  written non-atomically; a kill mid-write tears it. `fsatomic` exists;
  the sweep missed one writer.
- **I5 (CLOSED, no row): truncated-SSE malformed-tool-JSON deaths**
  (cycle-67 ×2, incl. t135-impl dying at iter 7). T141 + T143 landed;
  zero recurrences post-merge across 8 later streams. Watch closed.
- **I6 (→ T147, pri 3, tests): four carried weak-pin survivors** —
  T128 M4 (collect liveness render), T135 M2/F2 (driver_lock drop
  sweep), T138 plan-guard gap (mcp start exclusion), T141 M1 (api
  open-block check, masked). Each non-blocking alone; together an
  accumulation of unpinned core behavior. One tests-only row kills all
  four.
- **I7 (→ T148, pri 3, tests): T129's descoped wire e2e is now due.**
  The estimate-line clause dropped it cleanly, and the validator's
  M7/M8 survivors (CLI plumbing, isError arm) map onto exactly the
  missing wire coverage. The follow-up the filing named, filed.
- **I8 (noted, no row): cycle-69 decision-log mislabel**
  (`d1790664534-4` carried T131's inputs on T129's subject; corrected
  by superseding records -5/-6 in the same wrap). The append-only
  corpus keeps the stray — a supersession-tolerance requirement for
  F13's eventual consumer, noted for that design. Process worked
  (caught + corrected same-cycle).

## 3. Friction hot spots — fix assessment

- **decision_log schema fumbles** (`options`/`class`/`choice` missing):
  still ~1–3 per child stream across the era (t125/t127/t134/t135/t136/
  t139/t142-fixup/t143 + orchestrators). Every one a ≤1-iteration
  self-correct; zero records lost (the mislabel I8 was a content slip,
  not a schema fumble). The cycle-64 rejection STANDS — fresh reject
  record filed to keep the negative class current.
- **edit_file `old` not found on TODO.md/EVALUATION.md**: ~1–2 per
  orchestrator stream; anchor drift on 130 KB+ files; self-correcting
  re-reads. Rejection stands (cycle-64). EVALUATION compaction keeps the
  growth bounded; no row.
- **`path escapes cwd` in validators** (/tmp helper writes): T126's §6
  heredoc sentence landed cycle 64 with measure "target zero". Post-fix
  count: t112-validate ×6, t125-validate ×3, t134/t142 ×1–2 — NOT zero,
  but every fire is an instant self-correct to a bash heredoc (~seconds).
  Verdict: the boundary stays, the cost is noise; rejection stands, the
  T126 measure is reported as missed-but-cheap.
- **120s bash-cap timeouts**: omnipresent background rate (children pace
  long legs with `perl -e 'alarm …'`); by design; baseline.
- **Harvest naming chaos** (cycle-66's `events-t134-events-…-….jsonl`
  double-timestamp names): cosmetic; the cycle-68 mislabel was CAUGHT
  pre-removal by the read-back step — the verify-then-remove doctrine
  working. No row.
- **`.gitignore` harvest git-add fumbles**: ~1 per cycle; harvests are
  intentionally untracked now; self-correcting. No row.
- **T125 recalibration counter**: the era's estimated rows — T129
  ~400 → +814 (2.0x, inside band WITH the descope firing); T131 ~30 →
  +56 (1.9x); the 10 intake rows unestimated (I3). Density rule holds:
  pin/test density is the multiplier; the unestimated intake rows are
  the calibration blind spot T150 closes mechanically.

## 4. Capability gaps — ROADMAP PULL: F2 phase 2a → T146 (pri 2)

Tier walk (top-down, Tier 1 first):

- **F13 phases 2–3 — DEFERRED (standing written reason)**: the layad
  endpoint is still absent. Note the asymmetry shift: the corpus half is
  now rich (346 streams, ~18k iterations, hundreds of decision records
  incl. negative classes); only the endpoint blocks. No change.
- **PULL: F2 phase 2 → T146, SPLIT.** Cycle-36's deferral reason
  ("multiplies the diff past a 50-iter child budget") is obsolete —
  children run 80 iterations and T110's estimate discipline governs
  sizing. Phase 2a (this row): `chug run --approve <plan.md>`
  refuse-without-approved-plan gate (contract injection + run_start
  honesty fields) + web_fetch admitted to plan mode's read-only tool
  set — the two headless/loop-serving surfaces. Phase 2b (`/plan` chat
  toggle) DEFERRED: chat-only UX surface with no loop consumer — the
  F5-p2/F7-p2 deferral class, written reason hereby refreshed.
- **F10 phase 3 — newly eligible, not pulled**: its deferral reason
  ("no consumer until phase 2 proves the fleet shape") was SATISFIED at
  cycle 69 (phase 2 CLOSED). It sits in Tier 3 behind the Tier-1 pull;
  it is the likely next pull once T146 lands (notifications/resources/
  cancellation/server log for `chug mcp-serve`).
- **F11 / F12**: below F10-p3 in Tier 3; unworked, unremarked.
- **New finds beyond the roadmap**: none this eval — the era's finds
  were all incident-class, not capability-class.

## 5. Top 3 priorities

1. **T144** (pri 1, bug) — a goal gate that can execute another
   checkout's tests is a silent false-accept path on the loop's only
   acceptance gate; reproduced twice live. kimi REQUIRED.
2. **T146** (pri 2, feature) — the mandatory roadmap pull; retires the
   oldest Tier-1 deferral (34 cycles) whose blocking reason went stale.
3. **T145** (pri 2, robustness) — completes T136's crash-safety class on
   the loop's own memory file; smallest diff of the pri-2s.

## 6. README audit (usability)

Cold read, top to bottom (793 lines). **(a) Reading order**: sound —
what-it-is → Install → Quickstart → chat → run → forks → plan → TUI →
Tools → policy chain (risk gate → hooks → permissions) → MCP (client +
server) → observability → self-hosting specs → loopd → Development. A
newcomer is never asked to know something not yet introduced.
**(b) Redundancy**: the loopd section re-states the role-keyed-dir
rationale the T47/T52/T57 specs own — deliberate (it is the operator
runbook), but it is now the densest section in the file; third
consecutive balance note, still below the row-filing bar since each
clause answers a distinct operator question. **(c) Staleness**: none
found — Install (T127), the T137 build gate, the T142 rc-based verdict
rule, T82 nextest runner, and the T129 `--allow-launch` mcp-serve
contract are all present and current. **(d) Balance**: covered in (b).
**(e) Quickstart truth**: `cargo build && cargo install --path .`, the
`~/.claude/settings.json` auth chain, and the first `chug run` line all
work as written (the SPEC-6 chain as documented matches this era's
observed auth behavior). **No docs row filed — third consecutive clean
audit.**

## Handoff

- **Work order** (with reasons): **T144** (pri 1 bug; kimi REQUIRED;
  touches src/driver.rs + src/tools.rs + src/delegate.rs) → **T145**
  (pri 2 robustness; src/tools.rs — SERIAL with T144, shared file) →
  **T146** (pri 2 feature; src/main.rs + src/driver.rs + src/plan.rs —
  serial behind T144, shared driver.rs) → **T147** (pri 3, tests-only:
  delegate/driver_lock/mcp/api test surfaces) → **T148** (pri 3,
  tests/mcp_serve.rs wire e2e) → **T149** (pri 3, DOCTRINE — runs
  ALONE, no overlap ever, kimi REQUIRED) → **T150** (pri 3,
  tests/todo_consistency.rs guard).
- **Bundle check (T45 conjunctive)**: T147 (~150) and T148 (~220) both
  fail (a) ≤30 → NO bundles this cycle. Every row gets its own arc.
- **Overlap check (T44)**: T144/T145 share src/tools.rs → serial.
  T147 (tests in delegate/driver_lock/mcp/api areas) is disjoint from
  T148 (tests/mcp_serve.rs + README) → the pair MAY overlap one
  validator window if both are reached; T149 never overlaps (doctrine);
  T150 touches only tests/ but follows the queue serially behind the
  doctrine row. Realistic ceiling this cycle: 3–5 arcs.
- **Expected kimi routing at work time** (logged as they happen):
  T144 REQUIRED (driver/tools/delegate core surfaces); T145 REQUIRED
  (tools.rs); T146 REQUIRED (driver.rs); T147 skip (tests-only, T130
  precedent — orchestrator re-runs the four mutants at review); T148
  optional, leaning skip (tests-only on the MCP wire surface; the
  T128/T129 exercise precedent applied to PRODUCTION diffs — this row
  adds no production code); T149 REQUIRED (doctrine); T150 skip
  (tests-only tooling guard).
- **SELF-SPEC**: none. **Human items** (carried): (1) `gh auth refresh
  -s workflow` remains the proper fix for the workflow-scope class (SSH
  pushurl workaround holding); (2) `com.tampajohn.chug-loopd.plist`
  stays untracked (operator's launchd unit, carried since cycle 61).
- **Watch items carried**: loopd orchestrator cap (era max 162/200 —
  healthy; re-open at ≥180 or an iteration death); module sizes
  (api.rs 3,208 — +287 since the cycle-64 census; mcp_http.rs 2,800;
  tgrep.rs 2,638; tui.rs 2,527; **mcp_serve.rs 1,949 — +1,166 since
  new-at-783, the era's fast grower, split candidate on the NEXT
  feature landing in it**; driver.rs 1,822 steady post-T104-split);
  stream_fallback (zero organic post-T141/T143); decision-log
  supersession tolerance (I8 — a design note for F13's consumer);
  digest staleness self-exclusion (T131 — first live run this digest:
  FRESH verdict, self-stream correctly excluded).
- **Weighed and REJECTED this eval** (each with an eval-triage record in
  `.chug/decisions.jsonl`): decision_log schema-fumble row (§3 — ≤1-iter
  self-corrects, zero losses); edit_file anchor-drift row (§3 —
  self-correcting); /tmp-sandbox boundary move (§3 — fires continue but
  cost seconds; the T126 zero-target measure missed-but-cheap);
  harvest naming cosmetics + .gitignore git-add fumbles (§3 —
  self-correcting; doctrine caught the one real mislabel pre-removal);
  T136 carry (b) legacy chat-shape repair gap + (c) write_atomic
  dir-fsync/tmp-reap nits (edge case + power-loss-depth durability
  beyond the loop's tolerance — rename atomicity suffices); T135 F1
  non-unix cfg break (support matrix is unix-only — install.sh ships
  macOS+Linux, libc flock is cfg-gated; a Windows port surfaces it at
  first build); T137's 3 non-blockings (leg-harness rc-echo inversion
  nit, bash≥4 SHELLOPTS note, hardcoded `sleep 120` — loopd poll
  cadence is doctrine-tuned, no consumer pulling); T143 m9 observ-sink
  survivor + 3 coverage observations (outside the spec surface,
  observability cosmetic); T129 stub-spawn sub-ms ordering nit (T148's
  real-process design sidesteps it — noted in that spec); F13 phases
  2–3 (layad endpoint absent — standing dependency); F2 phase 2b
  `/plan` chat toggle (deferred in the SPLIT — chat-only UX, no loop
  consumer); F10 phase 3 (eligible but Tier 3 — behind the Tier-1
  pull); orchestrator goal-reject in cycle-68's glm stream (the goal
  gate working as designed, self-corrected same run).

## Outcomes (filled at cycle wrap — LOOP-SPEC Phase 3)

### Cycle 71 (2026-09-29) — routine glm freshness-skip (queue non-empty, eval fresh) — ALL FOUR landed (T147+T148+T149+T150), queue EMPTY

**T150 — todo-row spec estimate pin (92f987a, merge of loop-t150:
3f5a99a).** The T110 filing-time estimate ceiling now has a mechanical
guard: every `todo`-status TODO row's named spec must carry an
`estimate: ~<number>` line (filing time only — 121/141 legacy specs
predate the rule and are exempt history). Tests-only +195/-25 in
tests/todo_consistency.rs with the T8 row parsing lifted into shared
helpers (behavior-identical). Pure function over (table text,
spec-reader closure) — repo-live leg plus 4 synthetic legs + a
fail-closed unreadable leg. The child RED-proved 4 mutants (flag-arm
suppressed, always-fires, status-filter dropped, ~N requirement dropped)
with sha-verified byte restores; the orchestrator independently re-ran
the vacuous-guard mutant — both must-flag legs RED. kimi SKIPPED per the
spec's own routing (tests-only tooling guard). Process note: the child
ABORTED on the 35-min TIME budget at 48/80 AFTER committing the work —
the final full-suite gates chased the documented default-parallelism
load-flake class; the committed-work variant of the budget-death recipe
applied (orchestrator review + gates + merge, routing
d1790691515-11, no resume burned). Gates 1135/1135 nextest --release
worktree + main. QUEUE NOW EMPTY.

**T149 — validator budgets 60/40, doctrine (8699a70, merge of loop-t149:
10b6f6a).** The validator-side T102: after 4 validator budget deaths in 4
cycles (T134/T138/T142 at 50/50, T137 MINUTES-bound at 30m08s with the
verdict written but unannounced), LOOP-SPEC step 4 now launches validators
at max_iters 60 / max_minutes 40 with the census in the rationale and the
measure clause (>1 of the next 8 dying at 60/40 unannounced → next eval
considers trimming default mutation-leg counts, not further raises). Step
2's T63-resume echo aligned; META-SPEC §6's template (which the grep found
DOES name validator budgets) aligned in the same commit — supersedes T32's
do-not-touch stance for this row only; src/tools.rs's delegate description
correctly untouched (its 40/35 is delegate's own default, census-listed).
Exactly one pin fixed the old numbers (tests/loop_spec_recovery.rs) —
updated and RED-proven against pre-edit text. kimi REQUIRED (routing
d1790687293-8, doctrine row, ran ALONE), launched under the row's own new
60/40 budgets, verdict PASS (d1790688874-9): 3 parallel mutants —
echo-revert sent the pin RED (RED-proof independently reproduced),
weakened-needle still RED (pin non-vacuous); the step-4-sentence and
META-SPEC text-revert mutants went full-suite GREEN — pre-existing
unpinned-text gaps, non-blocking observations (candidate pin-breadth rows
for a future eval). Gates 1129/1129 nextest --release worktree + main.

**T148 — chug_launch wire e2e (8cd59ac, merge of loop-t148: 14d1bcd).**
T129's descoped follow-up closed the M7+M8 survivor gap with a real-wire
e2e: the REAL `chug mcp-serve` binary over REAL stdio (T124 harness,
+425 tests-only + one README line), `CHUG_DELEGATE_BIN` stub child
recording argv+env — real server, real spawn, no model endpoint. Three
legs: happy path (advertised ⇔ callable, pid/log/events payload, stub
record byte-equals the wire's spec/goal/model + budgets 7/9 — distinct
from the 40/35 delegate defaults), above-ceiling refusal (isError:true
naming "got 201" + the 200 ceiling, 1s bounded absence probe, loop alive
after refusal), default-deny (unadvertised, unknown-tool -32602). Carried
nit honored — zero sub-ms ordering assertions. kimi SKIPPED (routing
d1790684833-5, tests-only precedent); the orchestrator independently
re-ran the 3 canonical mutants — ALL RED (verdict d1790684835-6):
flag-drop → happy-path FAIL, isError:false → ceiling-leg FAIL,
advertise-unconditional → default-deny FAIL. Gates 1129/1129 nextest
--release worktree + main. Child's gate note: default-parallelism
`cargo test` bin-test flakes reproduce on the CLEAN base (dc6c11d,
tree stashed) under current machine load — pre-existing, nextest-based
gates unaffected; watch item for the next eval (threading or
isolation candidate).

**T147 — 4 carried survivor pins, tests-only (6b217c3, merge of loop-t147:
b08e909).** The cycle-70 80/80 budget death recovered by ONE T63 resume in
the standing worktree: child finished at 26/80 with the goal accepted,
gates green (build, clippy -D warnings, 1126/1126). All four carried
survivors now RED-proven pinned: T128 M4 (both collect-liveness arms + None
leg), T135 M2 (release must spend its 50×10ms budget on a held .chug flock)
+ F2 (lock absence + observe-seam first-attempt acquire), T138 plan-guard
(pending flag-writing mcp.json registry through the REAL plan loop — no
flag, no server; the prod empty-registry construction was the mask), T141
M1 (message_stop-masked open-block rejector — the mutant synthesized an
executable goal_complete). kimi SKIPPED per routing d1790681505-2
(tests-only, T130 precedent); the orchestrator independently re-ran all 5
mutants at review (T79 throwaway worktrees + role-keyed target dirs) — ALL
5 RED, 0 survivors (verdict d1790681871-3); post-merge nextest 1126/1126 in
target-shared-main. One launch slip caught by read-back: the first mutant
legs ran with cwd=main (no per-leg cd) — killed, relaunched with subshell
cds, main tree untouched. Carried weak-pin debt (cycles 65-70) CLOSED.

**Cycle-level notes.** Four for four, zero goal-rejections, one budget
death per arc (T147 resumed via T63; T150 was the committed-work variant
— orchestrator-finish, no resume burned). kimi ran ONE round (T149
REQUIRED, doctrine) and its PASS carried two non-blocking observations:
the step-4-sentence and META-SPEC text-revert mutants stay full-suite
GREEN — pre-existing unpinned-text gaps, candidate pin-breadth rows for
the next eval. The orchestrator independently re-ran mutants on every
tests-only row (T147: 5/5 RED; T148: 3/3 RED; T150: vacuous-guard RED) —
the no-kimi rounds lost no teeth. Recurring machine-load watch: TWO
children (T148 impl, T150 impl) reproduced default-parallelism
`cargo test` bin-test flakes on the CLEAN base (spawn-timing tests,
T144/T148 class) and fell back to `--test-threads=4` per the doctrine's
gate posture; nextest-based gates were unaffected both times — next eval
should consider an isolation/threading row. Iteration census: impl
children used 26 (T147-resume), 52 (T148), 43 (T149), 48-of-80 (T150,
minutes-bound) of 80 — no 80/80 deaths; the T110 estimate ceiling held
(T150's guard now enforces it at filing time). Release v0.5.1 cut at wrap
(4 items since v0.5.0, tests/doctrine/guards only → patch). Next cycle:
queue EMPTY → fresh eval routes kimi (loopd).

### Cycle 70 (2026-09-29) — kimi fresh-eval cycle (queue was empty) — 7 rows filed, T144 landed

Eval commit c802934: T144-T150 filed (bug T144 pri 1; T145 crash-safety;
T146 F2-2a roadmap pull; T147 survivor pins; T148 wire e2e; T149
validator budgets doctrine; T150 estimate pin), 13 rejected candidates,
20 eval-triage records, README audit third consecutive clean.

**T144 — goal-gate check + driver-spawned shells must not inherit
CARGO_TARGET_DIR (pri 1 bug, landed 8cabc79 ff-merge).** The cycle-66/67
infra finding promoted to a row: `goal_complete` checks executed a
FOREIGN worktree's test binary twice (d1790632587-1, d1790640546-3) —
and the fix's own validator reproduced it a THIRD time when its first
gate attempt (running the pre-T144 main binary, which still inherited
`target-shared`) hit the 600s check timeout under its own mutation-build
load with zero test failures. Fix: `tools::scrub_target_dir_vars`
removes both spellings at every driver-side spawn passing the inherited
env — `run_shell` (bash tool + goal-gate check), `hooks::run_hook`,
`delegate_launch`; explicit in-command prefixes keep the warm role-keyed
path (pinned green). Spawn-site sweep in the commit message (mcp N/A via
T138 env_clear; git/ps/rg fixed-argv). glm impl 73/80 one segment. kimi
REQUIRED PASS (d1790668684-21/d1790670698-22): gates 1110/1110
independent; mutants M1 (run_shell-scrub revert), M2 (alias dropped),
M3 (delegate-scrub revert), M6b (over-scrub breaking in-command prefix)
ALL DIED on named legs; non-blocking survivors M4 (general env
passthrough unpinned — broader isolation class, out of scope) + M5
(the beyond-spec hooks.rs scrub has no test of its own) carried to the
next eval. The `delegate_launch_stub_then_status_reports_summary_and_
liveness` full-suite load flake (one sighting) assessed by the
validator: 2 O(1) env_remove calls pre-spawn, timing window not widened.
Post-merge nextest 1110/1110. Watch for next cycle: children now run
the scrubbed binary — first goal gates build worktree-local
(impl-measured 85s vs the 600s cap).

**T145 — update_ledger writes through fsatomic::write_atomic (pri 2
robustness, landed 52a0abf ff-merge).** T136's crash-safety sweep had
left the ledger — the file every child reads first — on a truncating
`fs::write` (validator carry d1790643172-7). Now routed through the
shared same-dir-temp+fsync+rename primitive; happy path byte-identical,
a failed write errors the tool call with the previous ledger intact.
RED leg obstructs the pid-suffixed temp path (chosen over a second
RLIMIT_FSIZE leg to dodge process-wide rlimit flake against
transcript.rs's T136 test in the same binary — the validator assessed
the choice SOUND). Sweep verdicts in the commit message: update_ledger
CONVERTED; ensure_seeded / fork save+restore / archive::rotate /
generic write tools EXCLUDED with reasons. glm impl 62/80; its two
goal-gate rejections were the pre-existing load-sensitive mcp_http
dead-port race (T31/T59/T66 class, both legs 8/8 isolated, different
test each run) — aggravated by a 9.8-HOUR orphaned spinning test binary
from the t134 validation era (99% CPU, 590 min burned), which the
orchestrator found and killed mid-arc; see the cycle notes for the
orphaned-process finding. kimi REQUIRED PASS (d1790673125-26 /
d1790673939-27): independent gates 1113/1113; mutants M1
(revert-to-direct-write) DIED on the RED leg, M3 (wrong-dir temp) no
silent pass, M4 (error-swallow) DIED on both failure legs; fsync-drop
indistinguishable, skipped per spec; two non-blocking nits (124 vs ~60
estimate — informational; rename-replaces-inode symlink nit). Post-merge
nextest 1113/1113.

**T146 — F2 phase 2a: `chug run --approve plan.md` + web_fetch in plan
mode (pri 2 FEATURE, the mandatory roadmap pull; landed 55d59c3 merge of
8ea88e8 + d7d7aa0).** Retires the oldest Tier-1 deferral (34 cycles —
cycle-36's 50-iteration budget reason obsolete). Surface 1: the clap
flag exists on `run` only (plan/chat are clap errors);
`driver::load_approved_plan` refuses missing/unreadable/empty/escaping
legs with named-leg messages BEFORE any `.chug/` write (the T117
ordering); on success the approval sentence + plan text prepend the
first message with the goal undisplaced; `run_start` gains
always-present null-able `approve` + `plan_sha256` (new
`eventlog::sha256_hex`, the T117 honesty shape). Surface 2: web_fetch
joins plan mode's read-only set across all five enumeration sites
(six-tool contract, cardinality pinned). glm impl died 80/80 at a NEW
pace class (~5s/iteration, 6m41s for the whole budget — flash models
make iteration counts, not minutes, the binding constraint) and finished
on the T63 resume (impl recovery d1790674063-31; validator recovery
d1790677504-33). kimi REQUIRED PASS across two segments (validator died
50/50 mid mutation wave 2 — the 5th validator budget death in 5 cycles,
more T149 census — resumed and goal-accepted at iteration 4; verdict
d1790677901-34): independent gates 1118/1118; all 4 spec-named mutants
(before-.chug ordering, null-fields shape, approve-on-plan clap,
web_fetch enumeration) killed + 2 extras (injection-order, empty-check);
2 stale five-tool comments fixed on-branch pre-merge (d7d7aa0), 4
cosmetic nits carried. Post-merge nextest 1121/1121. FEATURES.md F2
annotated phase-2a landed, 2b deferred with refreshed reason.

**Cycle-70 wrap notes (budget-low at 240-min orchestrator ceiling).**
Landed 3/7 rows: T144 (bug), T145 (robustness), T146 (feature). DEFERRED
to next cycle: T147 (impl died 80/80 at the glm-flash ~5s/iter pace
class — 613k input tokens in ~10 min; worktree /tmp/chug-loop-t147 left
STANDING with uncommitted work + a recovery pointer on the row),
T148/T150 (not dispatched), T149 (doctrine — never dispatched; its
census GREW this cycle: t146-validate was the 5th validator budget death
in 5 cycles). Cycle-level findings: (1) a 9.8-hour orphaned spinning
t134-era test binary (99% CPU, 590 min burned) was found and killed
mid-T145 — a mutation-leg test process outlived its validator by ~11h;
the port-test flakes that cost T145 two gate rejections trace to its
load; watch for recurrence, candidate row next eval if the T6 bounded
harness needs a reaper. (2) The mcp_http dead-port race (T31/T59/T66
class) fired twice under load this cycle — third era sighting; next
eval should weigh a deflake row. (3) glm-flash pace class: iteration
budgets, not minutes, bind (T146 impl 80 iters in 6m41s; T147 impl same
death) — next eval weighs whether glm children need higher iteration
budgets or tighter specs. (4) Tool-result RENDER garbles (7 sightings)
proved to be artifacts on every disk read-back — verify-then-act held
every time; no action needed beyond the standing doctrine. Validators
this cycle: 3 REQUIRED PASSes (T144/T145/T146), 4+6+3 mutants killed,
survivors carried (M4 env-passthrough, M5 hooks-scrub pin, T146
cosmetics). Release: v0.5.0 (minor — T146 feature) cut at this wrap per
the T100 trigger (3 items + FEATURES phase annotation since v0.4.1).

### Cycle 69 (2026-09-29) — routine glm freshness-skip (queue non-empty: T129/T131 deferred from cycle 68, eval fresh) — the deferrals worked first

**T129 — F10 phase 2b: chug_launch MCP write leg (pri 2 feature, landed
2b4490b ff-merge).** The fleet primitive's actual verb, held one cycle by
the cycle-68 wall-clock defer, landed first-try: `chug mcp-serve
--allow-launch` (default OFF — the flag is the policy boundary; the
flagless server is byte-identical read-only and a `chug_launch` call gets
the unknown-tool `-32602`, so a read-only deployment cannot probe the
flag into revealing the tool exists). One boolean feeds both `tools/list`
and `tools/call` (advertised ⇔ callable by construction); the full
validation chain (shared `validate_chug_cwd` with error-text parity,
absolute readable-file spec, non-empty-trim goal, pass-through model,
budgets 1..=200/240 reject-above naming the received value + ceiling);
spawn hands off to the ONE `delegate_launch` path (`pub(crate)`
visibility-only — no second spawner, `CHUG_DELEGATE_BIN` seam reused,
`DELEGATE_ENV_LOCK` shared for the stub legs); launch failures are
`isError` results and no error kills the server loop; 13 bin-internal
legs; README params + safety paragraph + phase 2 CLOSED. The estimate
line's descope clause FIRED (all-in diff 814 > ~500): the optional
flag-ON wire e2e dropped to a follow-up, zero validation legs trimmed
(child descope record d1790662565-1). glm impl 77/80 first-try
(budget-low@8 fired, accepted before the ceiling — T21 headroom held).
kimi REQUIRED (routing d1790663037-1) VERDICT PASS (verdict
d1790664201-2): 8 mutants, M1–M6 killed (flag guard, advertise gate,
ceiling boundary, goal trim, spec-absolute, budget argv pass-through);
M7 (`--allow-launch` CLI plumbing) + M8 (launch-failure isError arm)
SURVIVED as non-blocking "correct code, test gap" findings — M7 is
exactly the descoped e2e's pin, M8 wants a spawn-failure leg; carried to
the next eval. Gates 1105/1105 + clippy clean independently; post-merge
nextest 1105/1105 (target-shared-main). Pipeline overlap ran once clean
(T129-validator ‖ T131-impl — disjoint file sets, strictly serial
merges). FEATURES.md F10 updated at the row flip (phase 2b LANDED, phase
2 CLOSED).

**T131 — eval-digest reader staleness check excludes the reader's own
live stream (pri 4, landed b52e62c rebased ff from 26b3ec6).** The
cycle-65 eval's live-observed annoyance (digest 17:47:52Z vs own-stream
rotation 17:48:12Z → STALE within the first minute, "regenerate if
stale" unsatisfiable mid-cycle) fixed at the render: the newest events
file (`ls -t | head -1`) is dropped from the `-newer` candidate set via
`grep -vx`, both verdicts render (`&& echo STALE || echo FRESH`), and
the rendered rationale sentence says why the newest file is excluded and
what STALE now means (foreign corpus). Pre/post-scan machinery,
corpus-age field, and the four pinned staleness labels byte-identical;
one pin leg RED-proven on the parent tree (died at the check-line pin,
old render lacked exclusion + FRESH) with a live 3-leg tempdir demo of
the extracted check (digest-newest→FRESH, own-stream→FRESH,
foreign-second→STALE) recorded in the commit message. glm impl 27/80
first-try; kimi SKIPPED per routing d1790664201-3 (T16/T116
tests+tooling precedent — the child's RED proof + orchestrator gates
carried it; a kimi round on a ~30-line script render is the optional
tier the routing call declined). Review + post-merge nextest 1106/1106
(target-shared-main). Est ~30 → actual +73/−4 (test density again beats
the line estimate; within the no-action band).

**CYCLE-69 CYCLE-LEVEL NOTES (wrap).** BOTH deferred rows landed — the
cycle-68 deferrals were honored in queue order (feature T129 first, then
small T131) — and the queue is now EMPTY. Zero budget deaths, zero T63
resumes (77/27 and 41/50 of their ceilings — the healthiest census in
the post-T110 era); minutes never binding. Pipeline overlap ran once
clean (T129-validator ‖ T131-impl, disjoint file sets, strictly serial
merges, clean rebase-ff for T131). Two decision-log hygiene notes for
the record: (a) one outcome record (d1790664534-4) was appended with
T131's inputs under T129's routing subject — corrected by the two
following records (d1790664540-5/-6); the corpus is append-only so the
stray stays readable; (b) child decision records (T129's descope call,
T131's RED-first verdict) were merged into the main decisions.jsonl at
harvest. Harvest: 3 impl/validate streams + 1 verdict LEDGER into
`.chug/` (untracked by design — .gitignore carries .chug/; commit
messages name the harvest). NO release tag this wrap: 2 items since
v0.4.1 (< 3) and no FEATURES.md check-off (F10 phase 2 CLOSED is a
progress annotation on a still-open row — phase 3 deferred — matching
the T128 precedent). Carried to the next eval (queue empty → fresh
eval routes kimi): T129's M7 (--allow-launch CLI plumbing pin = the
descoped wire e2e) + M8 (launch-failure isError arm pin) survivors, the
stub-spawn events-ordering nit, and the optional follow-up row for the
flag-ON wire e2e. Final gates at HEAD: build + clippy clean, nextest
1106/1106 (target-shared-main).

### Cycle 67 (2026-09-28/29) — routine glm freshness-skip (queue non-empty, eval fresh) — codex-review pri-2 rows

### Cycle 67 (2026-09-28/29) — routine (freshness-skip) — codex-intake pri-2 queue

**CYCLE-67 CYCLE-LEVEL NOTES (wrap).** TWO items landed (T135, T136 —
both pri-2 codex-review §1 crash/concurrency rows, both through full
kimi REQUIRED rounds); T137 reached impl+fix-up complete but its merge
is BLOCKED on the round-2 re-validation (detached, verdict lands after
this wrap); T138 impl #1 budget-died and is parked with a
resume-ready worktree. The T63 resume recipe went 3-for-3 this cycle
(T135 impl error-death, T136 impl 80/80, T137 impl error-death — all
resumed children finished their arcs). Server-side truncated-SSE
malformed-tool-JSON error-deaths: 2 sightings (T135#1, T137#1, both
glm at iteration 7, both instant with no abort event) — reliability
finding for the next eval: T141 made clean-EOF truncation retryable as
Connection class, but a stream truncated mid tool-input still reaches
the tool-input parser as a FATAL "LLM request failed: malformed tool_use
input JSON" and kills the run; remedy candidate: classify that error
shape retryable too, or survive one LLM failure with a bounded retry.
The cycle-66 infra finding d1790632587-1 (goal_complete check harness
runs cargo test WITHOUT a role-keyed target dir → cross-loop artifact
collision) REPRODUCED live: T135's validator was goal-rejected by it
while T136's impl built concurrently; the verdict was taken from the
validator's LEDGER read-back. Remedy candidate stands: role-key the
check env (validate dir for validators) or serialize checks across
loops. Pipeline overlap ran twice clean (T135-val‖T136-impl,
T137-val‖T138-impl — disjoint file sets, serial merges, T137's merge
still respects queue order over T138's). Routing ids: T135
d1790639671-2/d1790640546-3; T136 d1790642217-6/d1790643172-7; T137
d1790645926-10/d1790647780-11; recoveries d1790637275-1, d1790641323-5,
d1790643743-9, d1790647876-12. Carried survivors/findings for the next
eval: T135 M2 (drop-sweep mutant, no release-side regression test) + F1
(latent non-unix const-gating break, unix-only ship); T136's 3
non-blocking (update_ledger wholesale write, legacy chat-shape repair
gap, write_atomic dir-fsync/tmp-reap nits); T128 M4 + T141 M1 (older).
Release: 2 items since v0.4.0, no FEATURES check-off — NO tag this
wrap. Next cycle's cold input: T137 verdict collect → merge; T138
resume; then T139, T129 (feature), T131.

**T136 — crash mid-tool-batch leaves an unresumable transcript (pri 2,
landed bf672b6, d67d434 rebased ff).** The review's second §1 HIGH
crash-safety item: tool results were persisted only after a whole tool
batch finished, so a kill mid-batch left the transcript's last assistant
message holding unanswered `tool_use` blocks the endpoint rejects on
resume — and earlier tools in the batch may already have changed files
with no persisted record. Fix (impl d67d434, glm, 98/80 iters across
two segments): `resume_messages` — the single choke point shared by run
and chat resume — calls new `repair_interrupted_tools`, which appends
one user message with an is_error "[interrupted] … effects unknown"
`tool_result` per unanswered id, in memory AND on disk, idempotently
(the exact shape the loop writes after an intact batch, so the endpoint
pairing rule is satisfied and the model is routed at re-verifying real
state instead of trusting a result that never landed). Swept legs:
`transcript::load_with_torn` drops a torn trailing JSONL line (crash
mid-append) and physically truncates it so later appends can't merge
into the torn bytes (mid-file corruption still errors loudly);
`transcript::rewrite` (the transcript.rs:77 truncating-rewrite destroy
leg) and the same-shape `todos::save` now go through new
`fsatomic::write_atomic` (same-dir pid-suffixed temp + fsync + atomic
rename, temp cleaned on failure). Validation: 6 RED-proven tests
pre-fix incl. a deterministic RLIMIT_FSIZE destroy-leg test and a
recorded-transport test simulating the endpoint's tool_use/tool_result
pairing rule over resumed request bodies. kimi REQUIRED PASS round 1
(d1790643172-7): 6/6 mutants killed with zero survivors, the 6 RED legs
independently reproduced at base, gates re-run 1056/1056 + clippy -D
warnings clean, and the load-bearing invariant verified (unanswered
tool_use only ever sits at the transcript tail, so append-at-end repair
IS the immediately-following user message; `transcript::load` demoted
to cfg(test) so no production bypass compiles). Non-blocking findings
carried for the next eval: `update_ledger` (tools.rs:749) still does a
wholesale truncating write of LEDGER.md (same shape, bounded impact);
append-at-end repair cannot rescue pre-fix-damaged chat transcripts
where user messages piled up after the unanswered assistant;
`write_atomic` doesn't fsync the parent directory after rename and
never reaps crash-orphaned `*.tmp` siblings. Recovery note: the impl
child died 80/80 mid-work (iteration budget) and the ONE T63 resume
(d1790641323-5) finished in 18 more iterations — the resume recipe's
third consecutive success (T134 fix-up, T135 impl error-death, T136
impl budget-death). Post-merge nextest 1057/1057.

**T135 — driver.lock acquisition race (pri 2, landed 611916e ff-merge).**
The cycle's first pri-2 codex-review item: acquire's read-absent →
write → verify sequence was not atomic, so two simultaneous starters
could both decide from the same absent lock and both proceed into
transcript housekeeping. Fix: the whole critical section (loop
iterations included) now runs under an exclusive advisory `flock` on
the `.chug` directory fd — a stable inode, unlike `driver.lock` itself,
which Guard::drop unlinks (a file-mutex would swap inodes under
unlink+recreate and stop excluding). LOCK_NB + bounded retries
(300×10ms acquire / 50×10ms release); unobtainable mutex degrades to
the pre-T135 best-effort path with a stderr warning — T20 never-fail,
no new abort/hang legs. Guard::drop's compare-then-delete swept under
the same mutex (same read-then-act shape). The review's missing test
leg pinned: `simultaneous_starters_only_one_decides_from_an_absent_lock`
— two racers, injectable pids beyond every pid_max, synchronized AFTER
both read the absent lock via an observe seam on the new
`acquire_with`; RED pre-fix (both decided absent), GREEN post-fix,
20/20 stable. glm impl 66/80 — first try died at iter 7 on a
SERVER-side failure (truncated SSE → malformed tool_use input JSON,
T141-adjacent; transcript clean) → T63 resume d1790637275-1 (mechanism
extended to error-deaths, not just budget) finished in 59 more iters.
kimi REQUIRED PASS: routing d1790639671-2, verdict d1790640546-3 —
M1/M3/M8 killed (M1 mutex-removal = the exact RED proof), M2
release-side sweep mutant SURVIVED (no drop-side regression test —
carried), F1 latent non-unix compile break carried (cfg(unix) consts
used ungated; project ships unix-only), F3 informational
(threads-vs-processes leg shape). The validator's chug-level
goal_complete was REJECTED — its `cargo test` check ran without a
role-keyed target dir and collided with the in-flight T136 impl
build's artifacts: the cycle-66 infra finding d1790632587-1
REPRODUCED live; verdict taken from the validator LEDGER read-back.
Post-merge nextest 1044/1044 (target-shared-main).

### Cycle 68 (2026-09-29) — routine glm freshness-skip (queue non-empty, eval fresh) — codex-intake pri-2 queue (T137 merge pickup + T138 resume + T139)

**T137 — loopd launches stale binary after a failed build (pri 2, landed
08f0a39+46ec1a9 rebased ff; orig 3c0e403+f827659, cbfa952+f827659 pre-cycle-68-rebase).** The codex review's
§1 HIGH supervisor item: loopd.sh never checked `cargo build`'s status,
so a broken merge relaunched the previous release binary all cycle. The
round-1 validator FAILED it on the real class the fix created: with
`set -euo pipefail`, the pipelined `ps | grep -q` single-driver guard
flipped FAIL-OPEN under a SIGPIPE'd ps leg (the 83KB-ps
EARLY-MATCH-FLIPPED-FALSE proof) — a live driver would no longer block a
second launch. Fix-up 8a823ae de-pipelines that guard (rc-latched ps
capture + herestring grep; probe failure fails CLOSED) and sweeps the
class (all remaining pipelines enumerated). Round-2 kimi PASS
d1790651023-2: F1 re-probed byte-exact BOTH directions (fixed shape
skips a live driver under the 2.6MB early-needle attack; reverting the
shape re-opens it — attack teeth confirmed), class sweep independently
re-enumerated complete (:127/:344 masked, probe+verdict de-pipelined),
full pipefail leg audit clean incl. a SHELLOPTS non-export probe, 4/4
prescribed mutants died (M-A pipeline-revert killed twice rc 101, M-B
|| true-swallow 0.87s, M-C bare-capture set -e death signature, M-D
HALT-threshold — covering round-1's unexecuted M6 path). 3 non-blocking
findings carried (validator leg-harness rc-echo inversion; bash≥4
SHELLOPTS inheritance note; hardcoded sleep 120 observation).
Post-merge nextest 1064/1064 (target-shared-main). Cross-cycle lesson
confirmed: a fix that hardens one guard can silently break another —
adversarial round 2 on the FIX-UP (not just the feature) is what caught
F1.

**T138 — mcp.json executes before permission enforcement (pri 2, landed
37f8edc rebased ff from 0712c61).** The codex review's §2 HIGH
supply-chain item: a repo-controlled `mcp.json` spawned its stdio
command during startup — BEFORE permissions loaded — so denying
`bash`/`mcp__*` never prevented the checkout's own code from executing
(mcp.rs:366/469, driver.rs:295/658). Fix (impl 0712c61, glm, 96/80
across two segments after a T63 resume): `McpRegistry::new` is now
parse-only; a new `start(&Permissions)` does spawn+handshake and is
called by drive_loop immediately after `Permissions::load` (run AND
chat; plan mode structurally never starts MCP). The spawn gate
`Permissions::mcp_spawn_block_reason` judges deny rules alone — no tool
list exists pre-spawn — with three blocking shapes: a whole-namespace
deny (probed with TWO distinct synthetic servers so a server-specific
glob never reads as deny-everything), a whole-server deny, and (stdio
only) a `bash` deny — arg-matcher rules exempt. Credentials leg: stdio
spawns env_clear + fixed baseline (PATH/HOME/TMPDIR/LANG/LC_ALL +
windows extras) + the entry's env map only. RED proven with flag-file
driver tests (the command ran despite denies pre-fix; reproduced at
parent ae257da by the validator). kimi REQUIRED PASS d1790652558-6:
gate legs verified by mutation (6 mutants, M2–M6 killed; M1 drop-2nd-
probe SURVIVED — non-blocking, availability-only delta — plus
plan-guard test gap and doc nits, all carried); validator's 1st segment
budget-died 50/50 mid-mutant-batch, resumed per T63 and finished at
iter 4 (5th consecutive resume success). Post-merge nextest 1077/1077
(target-shared-main).

**T143 — per-request max_tokens too small for thinking models (pri 1,
operator intake, landed 00adb88 rebased ff from 67c4913).** A live
operator session died at iteration 9: the request-side `max_tokens` was
hardcoded 8192 in src/api.rs, and GLM thinking blocks consume the SAME
budget as the response content — a ~7KB `write_file` JSON overflowed
the cap and truncated mid-stream (the REQUEST-side sibling of T141's
stream-side detection). Fix (impl 67c4913, glm, 140/80 across two
segments after a T63 resume — the first segment died 80/80 in 4m45s of
fast small steps): `DEFAULT_MAX_TOKENS = 32768` (the operator-proven
value, not their stashed diff verbatim), `$CHUG_MAX_TOKENS` env +
`--max-tokens-per-request` CLI on run/plan/chat, resolution flag > env
> default via a pure main.rs helper; the cap flows through
RunConfig/PlanConfig/ChatConfig into `Client::new(model, cap)` and out
the single request-body site (api.rs:1350); T38's budget-abort advisory
gains a `raise CHUG_MAX_TOKENS` remedy line ONLY when
stop_reason=max_tokens AND the cap is below 32768 (byte-identical
otherwise); eventlog run_start records the cap. T15's cumulative
`--max-tokens` independence pinned (clap introspection + both-set
test). kimi REQUIRED PASS d1790655550-10: RED re-proven at parent
(body==8192, assert vs 32768 fails), remedy boundary + run_start
field-drop mutants killed, 8/9 mutants (m9 observ-sink survivor
non-blocking, outside spec surface, carried) + 3 minor coverage
observations. Post-merge nextest 1084/1084 (target-shared-main).

**T139 — editable spec check bypassed bash deny + risk gate (pri 2,
landed b5f1fa0+bb02823 ff-merge).** The codex review's §2 HIGH
self-modification item: goal_complete's spec `check:` command executed
via run_shell DIRECTLY (driver.rs:822/1390), outside permissions, hooks,
and the risk gate — deny bash + permit file editing let the model edit
its own spec to an arbitrary `check:` shell command and execute it the
next iteration. Fix (impl b5f1fa0, glm, 101/80 across two segments after
a T63 resume): verify() now routes the check through the SAME gate chain
as an ordinary bash call, in dispatch order — permissions.check deny →
PreToolUse hook veto → risk gate (fail-open judge + allow-destructive
override unchanged) → run_shell — and fails CLOSED: a blocked check
never executes AND never verifies (new VerifyOutcome::Blocked, the goal
rejected with "check command blocked by policy", blocked≠failed in the
model message); PostToolUse intentionally not fired (T83's advisory
follows dispatched tools only); the legit path is preserved (default
permissions still gate `check: cargo test`). Class swept: exactly two
production run_shell callers exist (tools.rs:582 the gated bash tool,
driver.rs verify()), and every sibling candidate (hooks = operator
config, mcp argv = T138-gated, delegate/git/ps = fixed argv) carries no
model-controlled shell string. RED proven END-TO-END (deny bash + edited
spec + goal_complete executed the attacker command; the trigger test
failed at the right assert on parent e56979d). kimi round-1 FAIL
d1790658499-14 — NOT a code bug: the production chain verified correct
in every leg, but the REORDER mutants M4 (risk-gate-before-permissions)
and M11 (hooks-before-permissions) survived the family AND the full
suite, violating the spec's own "a mutant that reorders or drops a leg
must be caught" bar. Fix-up bb02823 (test-only, +246, driver.rs
byte-identical) pins ALL THREE adjacent gate-chain pairs with
RED-proven killing tests — the sweep-the-class remedy from the cycle-33
lesson closed in ONE round what one-leg-at-a-time would have spread over
three. kimi round-2 PASS d1790660325-15: M4/M11/M12 all die on their
order-signal pins (RecordingAllowJudge consultation flag), gates
1092/1092 zero flakes. Post-merge nextest 1092/1092
(target-shared-main).

**CYCLE-68 CYCLE-LEVEL NOTES (wrap).** FOUR items landed (T137,
T138, T143, T139 — three codex-review pri-2 HIGHs + one operator pri-1
report), three through full kimi REQUIRED rounds and T139 through the
era's cleanest FAIL→fix-up→PASS arc (round-1 FAIL was a TEST gap, the
code verified correct; the sweep-the-class fix-up closed all three
gate-order adjacencies in ONE round). Release: v0.4.1 tagged at wrap
(4 items since v0.4.0, no FEATURES check-off → PATCH). T63 resumes went
4-for-4 this cycle (T138 impl, T138 validator, T143 impl, T139 impl —
all resumed children finished their arcs; era total 8-for-8). Iteration
economics: 2 of the last 4 impl children died at 80/80 with the work
done (T143 seg-1 in 4m45s of fast small steps; T139 seg-1 pre-commit
with green tests) — the T110 measure clause trips again; the remedy
stands (filing-time ~500-line estimate ceiling, META-META spec-quality
bar), no further iteration raises. Harvest hygiene: one mislabel caught
BEFORE worktree removal by the T19 read-back practice (T137's "024850
validate2" copy was the fix-up stream dup; the real round-2 stream was
the worktree's live events.jsonl — re-harvested correctly; both old
files removed). Origin divergence reconciled at cycle start (operator
intake 1257b05 rebased cleanly; T137's flip re-hashed
3c0e403+8a823ae→08f0a39+46ec1a9 — row refs corrected in c3f2568).
T129 (feature pri 2) + T131 (pri 4) deferred on wall-clock — bugs
outrank features, queue order held; T129's spec is ready, next cycle
works it FIRST. Carried to the next eval: T138 M1 (drop-2nd-probe
mutant, availability-only delta), T143 m9 (observ-sink generation
maxTokens unpinned at non-default caps) + 3 coverage observations,
T137's 3 non-blocking (leg-harness rc-echo inversion, bash≥4 SHELLOPTS
note, hardcoded sleep 120), T135 M2 + F1, T136's 3 non-blocking
(update_ledger wholesale write, legacy chat-shape repair gap,
write_atomic dir-fsync/tmp-reap nits), T128 M4, T141 M1.

### Cycle 66 (2026-09-28) — routine (reconciled cycle-65 divergence first) — codex-intake queue T134–T142

**CYCLE-66 CYCLE-LEVEL NOTES (wrap).** FOUR items landed (T140,
T141, T142, T134 — every pri-1 codex-review row), two of them through
full FAIL→fix-up→re-validate arcs. kimi ran FOUR validator rounds (T140
PASS 4/4 mutants; T141 PASS 5/6, M1 weak-pin carried; T142 round-1
FAIL 2 findings → round-2 PASS 10/10 after fix-up; T134 round-1 FAIL
2 findings → round-2 PASS M1–M5 all killed after fix-up) plus one
round-1 re-run after a T63 resume (T134 validator died 50/50
mid-battery, resume d1790631787-10 finished in 17 more iterations).
Budget deaths: T141 impl #1 (13/80, stuck:repeated-error on its OWN
malformed grep regex — recovery d1790626727-4 fresh relaunch), T134
fix-up #1 (80/80 mid-gates with +456/-18 UNCOMMITTED — T63 resume
d1790634951-13 finished the gates + committed in 13 iters), T134
validator round-1 (50/50 mid-battery — resume). The T63 resume
recipe (ONE relaunch, same worktree/spec/model/budgets) went 2-for-2
this cycle — both resumed children finished their arcs. Routing ids:
T140 d1790625539-1/d1790626612-2; T141 d1790627696-5/d1790628970-6;
T142 d1790629235-7/d1790630340-8/d1790634233-12; T134
d1790630798-9/d1790632841-11/d1790636105-14; recoveries
d1790626727-4, d1790631787-10, d1790634951-13. INFRA finding for the
next eval (d1790632587-1, logged by the T134 validator and
independently re-hit by the T142 re-validator): the goal_complete
check harness runs cargo test WITHOUT the T52 role-keyed target dir,
so two loops sharing `target-shared` collide on same-named
integration-test binaries — a foreign suite's failure gets reported
against the wrong tree. Remedy candidate: role-key the check env
(validate dir for validators) or serialize checks across loops.
Carried survivors: T128 M4 (alive-render weak pin, cycle-65), T141 M1
(open-block-check mutant masked by the message_stop guard —
behavior-preserving). Release v0.4.0 cut at this wrap: 4 items + the
T128 FEATURES check-off since v0.3.0, minor bump, tags immutable.
T135–T139 (pri 2) + T129/T131 remain `todo` with specs ready — the
next cycle's cold input needs zero human words.

**T140 — goal_complete denial bypass (pri 1, landed 9b36a2e).** The
cycle's first codex-review item: a permission-denied or hook-vetoed
`goal_complete` still completed the run because `goal_summary` was
populated from the tool name alone, without the `result.is_error`
check `submit_plan` has always applied. Fix is the one-line latch
guard at driver.rs:1084 (`!result.is_error`) — complete by
construction for all three block classes (permission deny, PreToolUse
veto, risk-gate block), each of which yields an is_error result
without executing the tool. glm impl 61/80 first-try; 4 new tests RED
pre-fix (permissions_policy + hooks_policy mods, +267/-1). kimi
REQUIRED PASS (routing d1790625539-1, verdict d1790626612-2): 4/4
mutants killed by named tests (revert, negate, is_error-only,
assertion-drop), positive control proves legitimate completions still
accept, class sweep confirmed goal_summary + plan_submitted are the
only tool-name-keyed run exits and both guarded, tree byte-clean.
Worktree gates nextest 1010/1010 + clippy clean; post-merge
target-shared-main nextest 1010/1010. Outcome landed-clean
d1790626687-3. T44 overlap: T141 impl (api.rs, disjoint) flew during
T140 validation.

**T141 — SSE truncation acceptance (pri 1, landed 70ec4a7, rebased
ff from impl 20fd96a).** The second codex-review item: a proxy
returning HTTP 200 SSE that ends the body before
`content_block_stop`/`message_stop` was accepted as success —
`finish()` closed the open block, missing input became `{}`, and an
executable `goal_complete` could be synthesized from a truncated
stream. Fix in api.rs `finish()`: open block at EOF now errors
"truncated SSE stream: body ended before content_block_stop" and
`message_stop` is tracked; the error rides the retryable Connection
class (a fresh accumulator per attempt — a real transport mid-body
EOF surfaces identically). 5 new tests RED pre-fix, including the
client-level test that reproduces the review's exact symptom
(executable goal_complete synthesized from a truncated stream).
First impl child died 13/80 stuck:repeated-error on its OWN malformed
grep regex (no orchestrator fault); recovery routing d1790626727-4
chose fresh relaunch over resume (13/80 with no committed work —
resume's continuation value nil); glm impl 51/80 second-try. kimi
REQUIRED PASS (routing d1790627696-5, verdict d1790628970-6): 6
mutants, 5 killed on named tests, M1 open-block-check mutant SURVIVES
masked by the message_stop guard — behavior-preserving under the
T141 threat model, carried to the next eval alongside T128's M4;
positive control proves legitimate closed zero-arg tools still
synthesize `{}`; class-sweep claims (mcp_http SSE POST EOF→Failed,
listen EOF→reconnect, non-streaming JSON bail) verified by code read.
Post-merge target-shared-main nextest 1015/1015. T44 overlap: T142
impl (loopd.sh+events.rs, disjoint from api.rs) flew during T141
validation.

**T142 — loopd grep spoofing (pri 1, landed 3bc3169 + fix-up dd184d4,
rebased ff).** The third codex-review item: the supervisor decided
cycle OK/fail by grepping the mixed cycle log for `chug: goal
complete` — but raw model text reaches that log verbatim (events.rs
stderr deltas, the F7 raw-bytes doctrine), so a run that died on
verification or budget while SAYING the marker recorded OK, reset the
failure counter, and ran site sync. Fix (3341658, glm impl 56/80):
the verdict is the child's EXIT STATUS (driver.rs run-mode exit 0 ⟺
accepted goal; budget/abort 1, stuck 2) and the child's stdout is
captured apart from the stderr log — forged lines can neither satisfy
the marker grep nor shadow the recorded summary; the supervisor
stamps an rc-based `verdict:` line for downstream consumers. kimi
round 1 VERDICT FAIL (d1790630340-8): the primary fix verified
correct + RED-proven, but F1 the site-sync cycle_count fallback just
RENAMED the forgeable marker (`verdict: goal complete (rc=0)` still
greppable in child bytes — real probe published "cycles 2" for a
failed cycle) and F2 M5 stamp-branch inversion passed the entire
suite. Fix-up (0b643be, glm 53/80, findings + class sweep in goal per
the cycle-33 lesson): fallback counts a log only when its LAST
verdict line stamps goal-complete (the supervisor writes nothing
after child death → unforgeable), behavioral stamp↔rc ties both ways
(inversion mutant now dead), observation 3 kept behavioral
(rc-gate-drop killed only by the abort-path stdout-ledger test).
kimi re-validation PASS (d1790634233-12): 10 mutants each killed on
named tests, the original forged-marker attack re-probed end-to-end
(3 forged lines in a FAILED cycle's bytes moved neither primary nor
fallback count). Post-merge target-shared-main nextest 1027/1027.
Two infra lessons logged: (1) T134-validator's note d1790632587-1 —
the goal_complete check harness runs cargo test without the T52
role-keyed dir, so concurrent loops sharing target-shared collide on
same-named test binaries (first re-validation check rejected by
exactly this while the T134 fix-up flew; retried clean); (2) T142
re-validator hit the same class. Both are check-harness findings for
the next eval, not row blockers.

**T134 — symlink sandbox escape + doctrine drift (pri 1, landed
d5d9c28 + fix-up bfd316a, rebased ff).** The fourth codex-review item:
resolve_safe was lexical-only, so an in-tree symlink to an external
path made every file tool follow it on disk — no race needed. Fix
(8785a2a, glm impl 48/80): two-stage resolve_safe — stage 1 the
unchanged lexical pass, stage 2 walks every component against the real
filesystem (lstat sees dangling links), expands symlinks under a hop
budget (loops fail closed), refuses when resolution escapes the
canonical sandbox root (macOS /tmp→/private/tmp handled), nonexistent
tails stay legal, returned path stays lexical; one fix point sweeps
all 9 call sites (get_path read/write/edit/image, grep, glob, list_dir,
tgrep, @attach, submit_plan --out); the false README/SPEC claims
replaced with the accurate contract. kimi round 1 VERDICT FAIL
(d1790632841-11): F1 MEDIUM — glob-metachar paths bypass stage 2
(resolve_safe treated `*`/`**` as inert-missing, the glob crate follows
symlinked dirs in expansion, tgrep's corpus arm read external
CONTENTS; /etc/passwd exfiltrated end-to-end via `path="**/passwd"`
through an in-tree etcdir→/etc link; external names leaked via the
glob tool); F2 LOW — SPEC's corrected claim had no pin test. Fix-up
(f28f38c→bfd316a after T63 resume d1790634951-13 — the first fix-up
child died 80/80 mid-gates with +456/-18 uncommitted): metachar
components refused fail-closed in the model-supplied relative part +
resolve_glob_pattern confines the literal prefix of both glob-expanding
surfaces + confine_glob_match re-passes EVERY concrete match through
the full two-stage check before its name is reported or bytes read;
SPEC.md pin added, stale "resolved lexically" line corrected. kimi
re-validation PASS (d1790636105-14): all three layers verified wired
at every call site, M1–M5 all killed (round 2 finished the fix-up
child's expired M4/M5 legs), the round-1 attack re-proven dead
byte-exact with glob 0.3.4. 8 RED proofs; post-merge
target-shared-main nextest 1043/1043.

### Cycle 65 (2026-09-28) — fresh eval (kimi, loopd-routed: queue empty) — T128–T131 filed (F10 phase 2 PULLED + SPLIT); T130 + T128 LANDED (976e4ae, 85ca4c1); T129/T131 DEFERRED by mid-cycle operator intake (codex adversarial review, 9 HIGH rows T134–T142); cycle ended on a push divergence (doctrine: no mid-cycle reconcile). Cycle notes below the per-item entries.
### Cycle 64 (2026-09-28) — fresh eval (kimi, loopd-routed: queue empty) — T124–T127 filed; ALL FOUR landed clean (T127 5dbab0d README install truth, T125 532c403 estimate calibration, T124 d6264be F10-p1 mcp-serve+chug_status, T126 eef7a29 validator /tmp-heredoc doctrine) — the era's FOURTH all-PASS cycle after 58/61/62. Cycle notes below the per-item entries.
### Cycle 63 (2026-09-28) — routine glm freshness-skip — T122 LANDED (93dae89 fast-forward)
### Cycle 62 (2026-09-28) — ALL FIVE landed clean (T121 cbc21e0 loopd 160→200, T117 3579d9d F9-p2a run-side expansion, T119 43f427d weak-test pins, T120 c173c2f kill/sed doctrine, T118 4c96414 F9-p2b frontmatter+completion — **F9 phase 2 CLOSED**) — the era's THIRD all-PASS cycle after 58/61. Cycle notes below.
### Cycle 61 (2026-09-28) — ALL FIVE landed clean (T112 0325a36 streaming-usage fix live-verified, T113 55fe207 F9-p1 packs, T114 63119e6 check:-breadth doctrine, T115 ed6c96f goal-integrity surface, T116 eb55003 todo symmetry) — the era\'s second all-PASS cycle after 58. Cycle notes: kimi ran 4 rounds (3 REQUIRED + 1 optional-exercised T113, T116 skipped per precedent), 31 mutants total, zero blocking findings era-wide second cycle running; T44 overlap ran twice clean (T112-val||T113-impl, T115-val||T116-impl — disjoint file sets, strictly serial merges, clean rebase-ffs). ZERO impl-child budget deaths and zero T63 resumes (26/62/44/63/26 of 80 — the T102+T110 era\'s healthiest census). Display-artifact watch: ~11 render-only sightings in the ORCHESTRATOR\'s own stream (incl. one big duplicated-block render), every payload verified intact by transcript/disk read-back — plus ONE real self-inflicted incident: SIGKILLed a healthy T112 validator at 35s misreading a render garble (verify-then-kill must be SEQUENTIAL — read first, kill after; the kill was issued in the same breath as the check) and one anchor-typo silent-replace no-op caught by read-back (assert replacements). Validator zone watch 21/26/19/44 of 50 — inside T18 margin. Estimate calibration: T115 ~130→+398 (3.0x), T113 ~455→+583, T116 ~30→+116 — test+doc density beats estimates ~2-3x, ceiling\'s failure measure still unmet. T115\'s integrity surface activates NEXT cycle (loopd rebuilds before launch). Carried to next eval: T113\'s 7 + T114\'s 1 + T115\'s 2 non-blocking (multibyte-goal leg, run_start call-site wiring pin, append-leg newline-collapse deviation). Tag bootstrap holds (no v* — operator\'s first, now triply feature-worthy F7+F8+F9-p1). Untracked com.tampajohn.chug-loopd.plist appeared mid-cycle — operator\'s launchd plist, left untracked. Queue EMPTY -> next cycle eval-routes kimi. Final gates 924/924 + clippy + build at HEAD under target-shared-main.
### Cycle 60 (2026-09-28) — fresh eval (kimi, loopd-routed: queue empty) — T110 + T111 filed (F8 SPLIT phase 1 → T111); BOTH landed (T110 5a16ce5, T111 d8fdea0+fc1d691)
### Cycle 59 (2026-09-28) — fresh eval (kimi, loopd-routed: queue empty) — T107–T109 filed (F7 SPLIT phase 1 → T108); ALL THREE landed (T107 793a0fc, T108 53e4aed+2a51cc5, T109 75025c9)
### Cycle 58 (2026-09-28) — fresh eval (kimi, loopd-routed: queue empty) — T102–T106 filed (F6 SPLIT phase 1 → T105); ALL FIVE landed clean (T102 ffebdaf, T103 80d4a14, T105 dc29137, T104 ee3943e, T106 e5cdebd)
### Cycle 57 (2026-09-28) — eval-routed kimi turned routine after reconciling cycle-56's origin divergence (T101 row appeared post-rebase, predicate held — cycle-56 precedent repeated); T101 LANDED (bcd0b66, fast-forward)
### Cycle 56 (2026-09-28) — T100 LANDED (9d1182a fast-forward: GitHub-releases surface — tag-triggered release.yml + install.sh + notes generator, +2008/-12 across 12 files) via FAIL→fix-up→PASS arc (kimi r1 caught the aarch64→linux-arm64 collapse); cycle-55 divergence reconciled; live chug-site edit 8eee10b; wrap 419afbc
### Cycle 55 (2026-09-28) — eval-routed kimi cycle turned routine (operator's T99 directive filled the empty queue at launch); T99 LANDED (d13a253, fast-forward)
### Cycle 54 (2026-09-28) — routine glm freshness-skip: T96 META-META-SPEC check-filter-breadth (merge 457720d, mid-arc recovery per row recipe, kimi REQUIRED PASS zero findings) + T97 README delegate sub-bullets (b0c6041 fast-forward, flip eca240a, kimi SKIPPED per T16/T31) landed; T97 push-divergence incident (operator moved origin mid-cycle, reconciled next cycle per no-force-push rule) — full narrative in git: `git log --grep "T9[67]"`.
### Cycle 53 (2026-09-27) — six landed: T92 9db86bc (impl 50→65), T91 ae7ff5f (F5-p1 images), T93 aeb12ea (mcp__ canary), T94 06f3b9e (get_str diag), T95 e80b3c5 (readme_layout), T98 8721c83 (site-sync); T96 mid-arc→c54, T97→c54 — one-line compaction; full narrative in git (row-flip commits + TODO done rows)
### Cycle 52 (2026-09-27) — routine glm freshness-skip; T90 LANDED (e9afed9, fast-forward merge) — F4 permissions phase 1: the deny-only fail-closed policy layer
### Cycle 51 (2026-09-27) — fresh eval (kimi: queue empty) filed T88+T89+T90; T89 delegate terminal-wait + LOOP-SPEC adoption LANDED (merge d2b402a, impl 6e95df9 via T63 resume #18; kimi PASS) + T88 decision_log corrective validation errors LANDED (c5a4f9e). Verdict: 2/2 landed
### Cycle 50 (2026-09-27) — T85+T86+T87 docs bundle LANDED (merge 2440520: cross-tree bash escape-hatch doctrine on both review surfaces, README layout tgrep/plan/hooks, docs-only floor honest risk model + pin leg); cycle-49 mid-arc recovery executed via T63 resume #17; kimi PASS 0 blocking 18/50, 2/2 mutants RED. Verdict: 3/3 landed
### Cycle 49 (2026-09-27) — T83 hooks phase 1 LANDED (merge ccb828a, F3: .chug/hooks.json PreToolUse veto + PostToolUse advisory; 2 budget-aborts -> resumes #15/#16; kimi R1 FAIL PostToolUse-on-vetoed-calls -> class sweep incl. the risk-gate sibling leg -> R2 PASS 0 blocking); T85-87 bundle MID-ARC (landed cycle 50). Verdict: 1/1 landed after 2 resumes
### Cycle 48 (2026-09-27) — cycle-47 dead eval recovered (artifacts landed verbatim 89d1b5a + 11 triage records reconstructed); T84 trim.rs extraction LANDED (745f8ff, 981 moved lines byte-identical, kimi PASS 4/4 mutants); T83 MID-ARC (landed cycle 49). Verdict: recovery + 1/1 landed
### Cycle 46 (2026-09-27) — FIRST loopd-routed cycle (routine glm, predicate honored end-to-end); T82 nextest gates LANDED (6edc5ee — 2.5x faster measured post-merge) + T75 decision-records wrap-checklist LANDED (b4c159f), both first-round kimi PASS, 12/12 mutants RED; queue EMPTY. Verdict: 2/2 landed
### Cycle 45 (2026-09-27) — freshness-skip; T81 landed (merge c1daaca)
### Cycle 44 (2026-09-27) — T80 docs-only gate slimming (9c4221d) + T79 parallel mutants (660f652) landed; T81 MID-ARC (impl in flight, recovered cycle 45); morning tools-proxy outage halted cycles 42/43 fast (halt-is-the-pager shape); full narrative: git log 9c4221d..660f652 + TODO done rows
### Cycle 40 (2026-09-26) — T78 release builds landed (merge 23fd276)
### Cycle 39 (2026-09-26, ~20:1x-21:2x EDT) — freshness-skip; T76 + T77 both recovered and LANDED (merges 355f253, fedb9ef; pushed 26b547d, 09d9f03); T78 MID-ARC (impl in flight)
### Cycle 38 (2026-09-26) — freshness-skip; T76 + T77 MID-ARC at budget wrap, nothing merged
### Cycle 37 (2026-09-26, ~13:22–18:5x EDT) — freshness-skip; T73 recovered mid-arc + landed (merge 87fe53f); T76 MID-ARC (fix-up-2 in flight); I1 RESOLVED
### Cycle 36 (2026-09-26) — MANDATORY fresh eval (F2 SPLIT → T73/T74/T75 filed); T74 landed (merge ef0c3eb, impl 0f1475e, kimi PASS 24/50 3 mutants RED — body_watchdog flake margins re-classified mechanism-not-timeouts); T73 landed cycle 37 (87fe53f), T75 landed cycle 46 (b4c159f). Verdict: eval + 1/1 landed
### Cycle 35 (2026-09-26, ~10:42 EDT) — T70 recovered mid-arc + landed (merge 572ec5a: decision_log tool + LOOP-SPEC adoption at 4 named points, F13 phase 1); T71 (e1e940c) + T72 (db74a99) landed cycle 36. Verdict: 1/1 landed
### Cycle 34 (2026-09-26, ~09:55 EDT) — MANDATORY fresh eval (F13 SPLIT → T70/T71/T72 filed); T70 MID-ARC at budget wrap (validator-2 in flight; landed cycle 35 merge 572ec5a). Verdict: eval + staging
### Cycle 33 (2026-09-26, ~09:20 EDT) — T69 (F1 delegate collect) recovered + landed (merge c6ce238; impl ccd9923 + 2 fix-up rounds 50dcbe7/88227ab, kimi R3 PASS 0 findings, latch-reset class swept); QUEUE DRAINED. Verdict: 1/1 landed after 3 validation rounds
### Cycle 32 (2026-09-26, ~08:51 EDT) — delta eval + ROADMAP PULL (T69 = F1 delegate collect); T69 MID-ARC at budget wrap (validated-in-flight; landed cycle 33 merge c6ce238). Verdict: eval + staging
### Cycle 31 (2026-09-26, ~08:06 EDT) — MANDATORY fresh eval (queue EMPTY); T67 landed (spec check lines never cargo test --lib + todo_consistency lint; 0d3c88b) + T68 landed (delegate wait_secs wakes on significant change only; 362e9b9) — QUEUE DRAINED. Verdict: 2/2 landed
### Cycle 30 (2026-09-26, ~07:28 EDT) — freshness-skip; T64 (validator survivor pins; 1859b21) + T65 (README target-cache de-accretion; 458751c) + T66 (dead_port_probe drop-leg theft-retry; ac2ea36) landed — QUEUE DRAINED. Verdict: 3/3 landed, live goal-gate flake filed + landed same cycle
### Cycle 29 (2026-09-26, ~07:10 EDT) — MANDATORY fresh eval (queue EMPTY); T63 landed (LOOP-SPEC adopts delegate resume:true as standard budget-death recovery; merge db539c9, kimi PASS 5/5 mutants). Verdict: eval + 1/1 landed
### Cycle 28 (2026-09-26, ~06:39 EDT) — freshness-skip; ALL 4 queued rows landed green: T58 (delegate resume + latest-segment status; 5145536), T62 (eval-digest golden-section pins; f4e3bc3) resumed from cycle-27 wrap, T61 (resolve_safe error names bash; c176321), T60 (docs drift pass; 2ce4f8c) — QUEUE DRAINED. Verdict: 4/4 landed
### Cycle 27 (2026-09-26) — freshness-skip; T59 (mcp_http dead_port bounded theft-retry, tests-only; glm impl 27/50 first-try, ff-merge 062c175, scripted-theft pins exactly-attempt-2 + exhaustion-at-3 naming) landed — last known organic flake closed; T58 impl died 50/50 complete-but-UNCOMMITTED (5th occurrence: t15/t17/t20/t55/t58 — the demand signal for T58's own resume feature) → T55-precedent orchestrator finish b189517, T58 validator + T62 impl IN-FLIGHT at budget wrap with recovery recipes (both landed cycle 28); T60/T61 deferred with ready specs; T44 planning catch (disjointness must enumerate BOTH specs' FULL file lists — T58+T60 share README.md). Verdict: 1/3 landed, two clean recoveries staged
### Cycle 26 (2026-09-26) — MANDATORY fresh eval (queue empty; T46 digest-first acceptance HELD, filed T57–T62) + T57 landed (main-dedicated gates dir target-shared-main, doctrine; impl 1c2beee glm 32/50 first-try, merge 2f8bbe4, kimi PASS 18/50 13/14 mutants, 1 weak-pin survivor watch-item) — no future post-merge/final gate can execute a non-main binary by construction; cycle-19 T39 LEDGER mystery CLOSED (zero update_ledger calls, archive_stale correctly skipped seed); stale-loopd human carry PROMOTED. Verdict: eval + 1/1 landed
### Cycle 25 (2026-09-26) — freshness-skip; T55 (driver.lock same-cwd mutual exclusion, FEATURE; impl cb5906f, merge 2e7945b, kimi PASS 45/50 with real-binary hand-verification of the refusal cluster) + T56 (README loopd re-exec + ceiling dedupe; 611af46) landed — queue drained; SEQUENTIAL variant of the T52/T43 stale-artifact false-red observed at post-merge gates (touch+rebuild recovered, led to T57). Verdict: 2/2 landed, staleness class fully mapped
### Cycle 24 (2026-09-26) — MANDATORY fresh eval (queue EMPTY); T53 (loopd single-driver check ps-based, pgrep dead on this host — live 9/9 reproduction; impl 5c842a6) + T54 (loopd_reexec.rs exact-count SELF_CKSUM pins, additive-mutant survivor closed; impl 6587b22) landed as the FIRST T45 bundle (merge 8624078, kimi PASS 24/50, 8/8 bundle mutants); T55 mid-arc at budget (recovery recipe on its row, landed cycle 25); T56 deferred. Verdict: bundle arc ~7 min wall, pgrep blindness fixed within the hour
### Cycle 23 (2026-09-26) — freshness-skip; T49 (api.rs connect-timeout const + pins; 9a73b12), T52 (role-keyed target dirs; 4d4c383), T50 (loopd self-re-exec between cycles; 078cb77), T51 (README delegate paragraph trim; 372f920) landed 4/4 queued rows (kimi PASS x3: 17/50, 35/50, 41/50); T44 overlap #3 (T51 impl during T50 validator); QUEUE EMPTY → next cycle mandatory eval. Verdict: 4/4 landed, loopd-restart carry first stated
### Cycle 22 (2026-09-26) — MANDATORY fresh eval (queue EMPTY); T48 landed (test code resolves repo root at runtime, not env!(CARGO_MANIFEST_DIR); d000b72); T49 impl done, kimi validation deferred at budget (landed cycle 23); T44-overlap × target-shared race observed live (gates executed a validator's leftover mutant) → T52 filed pri-1. Verdict: 1/2 rows landed, race lesson became next cycle's doctrine
### Cycle 21 (2026-09-26) — freshness-skip; T43 landed (EVALUATION.md Outcomes pruning rule — last 6 cycles full — + first compaction of cycles 5-13; 8694003); CARGO_MANIFEST_DIR × target-shared staleness bit the post-merge gates → T48 filed. Verdict: 1/1 landed, staleness class identified
### Cycle 20 (2026-09-26) — freshness-skip; T40 recovered + landed (TODO notes-cell pipe doctrine + todo_consistency-after-edits; 75c21c8), T47 (shared CARGO_TARGET_DIR for worktree builds; 2f4cefc), T41 (sandbox-candor descriptions + README Tools intro; 7170fb4), T42 (webfetch timeout const pins; 7ff8bfa) landed; T43 deferred. Verdict: 4/5 landed
### Cycle 19 (2026-09-26) — freshness-skip; T46 (eval digest: pre-computed Phase-1 corpus summary; 6a3fa98), T45 (trivial-row bundling doctrine; 3efb98d), T39 (delegate launch optional max_tokens passthrough; 52ec8ab) landed. Verdict: 3/3 landed
### Cycle 18 (2026-09-26) — fresh eval (queue EMPTY at start); T38 landed (truncated-response advisory names the chunking remedy; a686522) + T44 landed (pipeline overlap, operator pri-1 speed initiative; 41f61a1); TWO 50/50 child deaths, both recovered. Verdict: 2/2 landed, child-death recovery proven twice
### Cycle 17 (2026-09-26) — freshness-skip; T37 (web_fetch tool) recovered from the cycle-16 goal-gate pipe death + landed (refs: 1432a2e, 1e82ffb) — queue drained
### Cycle 16 (2026-09-26) — fresh eval + T36 landed (loopd --max-iters 120→160: impl d5141c9, merge 0586e1e, kimi PASS 5/5 check-mutants); T37 web_fetch impl-complete on branch loop-t37 @ 1432a2e (430+3) with kimi validation deferred at budget; first organic T31-residual dead_port_probe sighting (~1/8); glm truncated-write death class first seen
### Cycle 15 (2026-09-25, ~23:47–23:56 EDT) — T35 landed (README quickstart cargo install --path .; impl 8ebbe59, merge 09730de, flip eb885a8) — queue drained; docs-only validation-optional per T16/T31; first forward practice of T34 per-item doctrine
### Cycle 14 (2026-09-25, ~22:50–23:59 EDT) — fresh eval filed T31-T35 (dd857c2); T31 landed (deflake parallel-load family; impl f8012c2, merge e765de2, flip 65b99fb) + T32 (validation-child template 40→50; a173ff2, merge a3d60ad, flip 723912e) + T33 (META-META priority line; 4b409c0, merge 0ee166b, flip e9853ce) + T34 (Outcomes per-item at landing; a965053, merge dcbff00); T35 deferred to cycle 15; all three kimi validators PASS first-try
### Cycle 8 (2026-09-25, ~16:29–16:47 EDT) — T22 landed (bash tool macOS timeout-mirage note; impl 6068654, flip d7147cf) — queue drained, zero open rows; eighth consecutive clean glm round
### Cycle 7 (2026-09-25, ~14:39–16:35 EDT) — T21 landed (impl-child template --max-iters 40→50; cac4649, flip e73b66d) + T23 landed (delegate tool; 1012dca); T21 self-merge anomaly (check cd'd to main) → worktree-relative check lesson; T22 deferred to cycle 8
### Cycle 5 (2026-09-25, ~13:29–14:05 EDT) — T18 landed (WARN_REMAINING_ITERS 5→8; impl 9056c78, flip 019cfa8): first-try PASS, fire-time 7→8 derived by impl/validator/orchestrator; T19/T20 filed for the next queue
### Cycle 6 (2026-09-25, ~14:03–14:40 EDT) — T19 landed (harvest-before-removal; impl 1e26acc, flip 8360ba4) + T20 landed (banner/run_start name worktree HEAD; f08ea04, flip 09496eb) — queue drained; J6 budget_low fired first time (T20 impl committed before ceiling)
### Cycle 9 (2026-09-25, ~16:44–17:20 EDT) — T25 landed (failure-aware event previews; impl f1f7946, flip 75d9c90); T24 impl committed 3334743, left unmerged in preserved worktree (recovered cycle 10); T26 spec ready
### Cycle 10 (2026-09-25, ~17:16–17:45 EDT) — T24 recovered + landed (LOOP-SPEC adopts delegate; merge e913cf9, flip ad80910) + T26 landed (read_file offset/limit pagination; merge 179e1aa, flip 4b77d6b) — queue drained; first full delegate-dogfood cycle
### Cycle 11 (2026-09-25, ~17:41–18:55 EDT) — T27 landed (loopd cycle budget --max-iters 80→120; merge 759c45e, flip 414acf3); hibernate-mid-child incident survived by design (awake-time budgets); T28 impl left running in preserved worktree
### Cycle 12 (2026-09-25, ~18:55–20:57 EDT) — T28 landed (delegate status reaps zombies; impl a0a7c97, merge fcaa7c2, flip 07dd3af); T29 mid-arc at exit — validator #1 FAIL, wrap deferred (RECONSTRUCTED at cycle-13 wrap; deferred-wrap lesson)
### Cycle 13 (2026-09-25, ~20:58–22:55 EDT) — T29 recovered + landed (delegate wait_secs long-poll; impl 549d250 + fix-up e36b30c, merge 47dfcaf, flip cd7d43c) + T30 landed (worktree-relative check: rule; merge 00ce617, flip a1dde80) — queue drained; operator spec 9b7e904 absorbed mid-cycle; flake-family sightings handed to next eval
