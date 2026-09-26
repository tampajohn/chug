# EVALUATION — chug, assessed by chug-loop (2026-09-26, cycle 36)

**MANDATORY fresh eval** — cycle 35 drained the queue at T72's landing
(`a80510e`: "QUEUE DRAINED — next cycle opens with a mandatory fresh
eval"), so the freshness-skip rule cannot fire. The roadmap pull is **F2
(Plan mode)** — F1 landed T69, F13 phase 1 landed T70 with phases 2–3
deferred (corpus + layad endpoint absent).

Corpus: `.chug/eval-digest.md` FIRST (STALE at eval start — SEVENTH manual
regeneration, cycles 22/24/29/31/32/34/now 36, ALL downstream of I1: the
running supervisor pid 90114 predates T46's refresh line, so its frozen
loop body never regenerates the digest pre-cycle; 153 event streams /
6,860 iterations post-regen), then the streams since the cycle-34 eval:
the cycle-34 orchestrator stream (`events-20260926-144249.jsonl` —
**120/120 BUDGET ABORT** mid-T70-arc, 42 min, 28 delegate calls), the
cycle-35 orchestrator stream (`events-20260926-155156.jsonl` — 119/120
goal accepted, 68 min, 43 delegate calls, T70 recovery + T71 + T72
landed), the seven t70–t72 child streams (t70-impl 2 runs: 50/50 abort →
T63 THIRD resume accepted 18/50, 1 goal-gate rejection = the
body_watchdog flake; t70-validate 29/50 FAIL 1 finding; t70-validate2
42/50 PASS; t71-impl 2 runs: 50/50 abort → T63 FOURTH resume accepted
19/50, 25m36s the longest impl child; t71-validate 50/50 PASS at ceiling;
t72-impl 26/50 first-try; t72-validate 20/50 PASS),
`.chug/loopd/loopd.log` (cycles 34–36; 1 budget death; 0 re-execs — I1),
`TODO.md` (T1–T72 all done with refs), git log `a80510e`, `src/` (25,410
lines; post-T71 topology: driver.rs 3,607 now the largest, delegate.rs
3,485, tools.rs 1,782), `.chug/decisions.jsonl` (ABSENT — I3), `README.md`
(cold read, §6).

## 1. What chug does well

- **T63 resume is now routine infrastructure — four live exercises, four
  accepted on the resumed segment.** t70-impl 50/50 → 18/50 and t71-impl
  50/50 → 19/50 this cycle pair; t69-impl + t69-validator cycles 32–33.
  The doctrine's condition (budget abort + incomplete work in the
  worktree) is checked, not cargo-culted, each time; the cycle-34/35 arcs
  would have needed full re-dispatches without it.
- **The vacuous-pin family lesson is now doctrine and already works.**
  T72's sweep-the-family sentence landed with a 7-mutant adversarial
  proof (kimi PASS 20/50, weakened-pin escape-then-RED load-bearing leg);
  this eval applies the doctrine at FILING time (T73's rejection sweep is
  written one-RED-proof-per-leg; T74's margin audit is family-scoped).
- **Review gates + independent re-runs keep earning their cost.** Cycle
  35's orchestrator re-verified T71's byte-identity claims independently
  (schema, dispatch arm, fn-name inventory, 71/448 count) before gating;
  kimi re-ran the BEFORE count in a separate worktree. "Never trust a
  claim of green without seeing it" held at every hand-off.
- **delegate collect is dogfooded and paying rent.** T70's review used
  collect for the first live structured read (verdict + summary + check
  cmd + scoped refs in ONE call); cycle 35's recovery arc opened with
  exactly one collect call per the row recipe — the F1 gap is closed in
  practice, not just in code.
- **Cycle 35 landed a 3-item queue + a mid-arc recovery in 68 minutes,
  every arc green** — 3 impl children, 3 validators, 2 resumes, 0
  reverts, 0 force-pushes, per-item Outcomes written at each landing.

## 2. Incidents worth fixing

**I1 — the frozen supervisor (loopd pid 90114) now blocks FOUR landed
fixes; restated 9th, mechanism fully nailed, HUMAN item.** The supervisor
parsed its while-loop body at startup (2026-09-25 20:43 EDT); bash
executes a parsed compound command from memory forever — no edit to
loopd.sh takes effect without a restart. The frozen body therefore still:
(a) launches cycles at `--max-iters 120` (t36's 160 landed 00:04,
post-startup, `d5141c9`) — cycles 32 and 34 died AT 120/120 mid-arc,
cycles 31/35 wrapped at 116/119 with budget_low@8; (b) never refreshes
the eval digest (T46 line absent) — SEVEN manual regenerations to date
incl. this eval; (c) cannot re-exec (T50 landed post-startup — dormant by
construction, 0 re-execs in loopd.log); (d) runs the pgrep single-driver
guard that fails OPEN on this host (T53's ps-based guard landed
post-startup). No chug row can fix this — the budget is baked into the
cycle's own argv. **Operator action at the next natural break:
`./loopd.sh stop`, wait for the in-flight cycle's clean exit (loopd.log),
then `nohup ./loopd.sh >/dev/null 2>&1 &`.** Not filed as a TODO row —
not chug-actionable (rejected candidate, eval-triage record logged).

**I2 — api.rs body_watchdog cold-parallel flake, 2 sightings one day →
T74 (pri 2).** Sighting 1: t70-impl's first goal-gate REJECTION
(`check command failed`, cold-build full-suite, 4x green re-runs;
`.chug/events-t70-impl-20260926-140700.jsonl` goal rejected 1; commit
`b6b22c7` row notes). Sighting 2: T71 impl's 518/1 parallel run (3x
green re-runs; commit `e1e940c` notes). Prime suspect:
`body_watchdog_aborts_silent_stream`'s elapsed<1900ms upper bound under
saturated-scheduler load — the bound discriminates the 1s activity
timeout from the 600s total and survives widening; the steady-stream
sibling carries the mirror hazard (250ms sleeps stretching past the 1s
timeout). Gate-blocker class per T66: every goal gate and review gate is
a default-parallel `cargo test`, so the flake false-reds any future arc.

**I3 — decision_log zero organic adoption across 2 cycles → T75 (pri
3).** T70 landed the tool + 4 named LOOP-SPEC adoption points (merge
`572ec5a`); cycles 34+35 then made ≥9 routing/verdict decisions (3
eval-triage filings + rejects at the cycle-34 eval, 2 T63 resume
recovery-routings, 3 per-item validation-routings, 3+ validation
verdicts incl. a FAIL-fix-revalidate arc) with ZERO decision_log calls
in either orchestrator stream (digest tool distributions: cycle 34 = bash
77/delegate 28/read_file 13/write_file 6/update_ledger 4/edit_file 3;
cycle 35 = bash 80/delegate 43/update_ledger 5/edit_file 4/read_file 2/
goal_complete 1) — `.chug/decisions.jsonl` does not exist on disk. The
T23→T24 zero-calls lesson: named-point sentences don't fire at
accounting time. The wrap checklist (the accounting surface) gains the
records sentence + pin. F13 phase 2's corpus precondition is otherwise
unmeasurable. THIS eval logs the first organic records (the T70
acceptance datapoint).

**Minor — cycle-34 eval-phase tool errors (rejected, no row).** Three
self-corrected one-offs in the cycle-34 stream: edit_file against the
literal template path `specs/tN-delegate-extraction.md` (placeholder not
substituted), a write to `/tmp/eval-front.md` refused by the sandbox
(cross-tree writes go through bash — adapted), reading src/decisions.rs
before the impl existed. No doctrine gap; the tool errors are corrective
by design.

**Minor — t72-impl harvested twice (rejected, no row).**
`events-t72-impl-20260926-114913.jsonl` and `-114917.jsonl` are
byte-identical (same stream, two harvest timestamps 4s apart). Cosmetic;
zero operational impact.

**Rejected — raise impl-child --max-iters past 50.** T70/T71 both needed
T63 resumes; all four resume exercises accepted cleanly at ~2
orchestrator iterations each. The 50-iter budget bounds rounds and the
resume doctrine absorbs the overflow — the system working as designed,
not a defect.

## 3. Friction hot spots

- **Orchestrator iteration budget is THE binding constraint.** Deaths at
  the 120 ceiling: cycles 32, 34 (both mid-arc, both recovered next
  cycle); wraps with ≤4 spare: 31 (116/120), 35 (119/120). The primary
  fix is I1's restart (loopd.sh already says 160 = eval + 3 items + wrap
  per its line-128 budget comment). The landed secondary mitigation
  works: T68's wake fix means cycle 35's 43 delegate calls spent waits
  inside wait_secs long-polls, not 2–7s churn-wakes.
- **Digest staleness is I1-downstream** (§2 I1b) — seventh manual regen.
- **Child goal-gate cold-build flakiness** — T74's row; one rejection
  cost a resumed child ~4 iterations + 4 check re-runs.
- **Prior fixes assessed, not re-filed:** T68 (wake churn) — cycle 35's
  long-polls behave, no churn-wake pattern in the stream; T67
  (never---lib) — no --lib goal-gate rejection since; T66 (dead_port
  drop-leg) — zero sightings since landing; T57 (main-dedicated gates
  dir) — no stale-artifact false-red since.

## 4. Capability gaps — ROADMAP PULL (required)

**PULL: F2 → T73 (pri 2), SPLIT per the FEATURES.md working rules**
("items can shrink: the evaluator may split a roadmap item into 2-3 rows
when the full scope blows a 50-iter child budget"). F2 is the top unworked
Tier-1 item (F1 landed T69 cycle 33; F13 phase 1 landed T70 cycle 35,
phases 2–3 deferred). Phase 1 (this cycle's row, specs/t73-plan-mode.md):
`chug plan` subcommand — read-only tool contract (the API tool list is
EXACTLY read_file/grep/glob/list_dir + the new submit_plan), dispatch-side
rejection of the 8 excluded tools naming the allowed set (one RED-proven
leg each — the T72 sweep doctrine applied at filing time), `--out`
cwd-sandboxed or stdout, run/chat surfaces byte-unchanged, no
TODO/LEDGER writes, README integrated. Phase 2 DEFERRED with written
reason: `/plan` chat command + `--approve plan.md` run gate +
web-fetch-in-plan — the read-only contract and the submit_plan exit are
the load-bearing semantics; the chat/approve surfaces multiply the diff
past a 50-iter child budget for no doctrinal need. F13 phases 2–3 REMAIN
deferred: the corpus literally starts with this eval's records
(decisions.jsonl absent until now — I3) and the layad endpoint is still
absent.

**New finds beyond the roadmap:** none this eval. The two capability-class
observations (plan-mode web_fetch, the plan-approve gate) are F2 phase-2
scope, already named in the T73 spec's Out-of-scope. FEATURES.md gains
F2's SPLIT annotation at the T73 row flip (orchestrator, per the working
rules).

## 5. Top 3 priorities

1. **I1 operator restart of loopd** (human, ~30 seconds) — unblocks
   160-iteration cycles (the budget-death class: cycles 32, 34), the
   pre-cycle digest refresh (7 manual regens), T50 re-exec, and the T53
   ps-based single-driver guard in ONE action.
2. **T74 body_watchdog flake** (robustness, gate-blocker class) — two
   sightings in one day; every goal gate and review gate is exposed.
3. **T73 plan mode** (roadmap pull, feature) — closes the top Tier-1 gap;
   the read-only contract also gives the loop a cheap explore-before-impl
   dispatch shape for future arcs.

## 6. README audit (usability, not just accuracy)

(a) **Reading order** — quickstart → interactive → autonomous → TUI →
tools → risk gate → MCP → observability → self-hosting specs → continuous
→ development: still guides a newcomer; no accretion since T65's
de-accretion. (b) **Redundancy** — decision_log appears in the Tools list
plus one descriptive sentence; no double-claim drift found. (c)
**Staleness** — the delegate paragraph documents launch/status/collect
incl. T68's `waited:` line and T69's collect contract; accurate at
a80510e. (d) **Balance** — the delegate paragraph remains the densest
block (watch item, second consecutive eval); still reference-grade, not
yet a split candidate (the pinned-token tests make churn costly; revisit
if a fourth action ever lands). (e) **Quickstart truth** — install/run
commands unchanged; no new user-visible surface since T70's decision_log
(documented at the Tools section). **Third consecutive zero-finding
audit.** T73 adds the Plan mode section — the first structural addition
since T69's collect paragraph.

## Handoff — recommended execution order

Strictly serial merges in queue order (robustness > feature > doctrine
per the priority doctrine; features outrank DX friction at equal pri):

1. **T74** (robustness, pri 2) — tests-only inside src/api.rs; kimi
   REQUIRED (api.rs is on the step-4 REQUIRED list even for tests-only).
   One child round expected.
2. **T73** (feature, pri 2) — the roadmap pull; kimi REQUIRED (main.rs,
   driver.rs, the tools registry). The largest of the three; budget for a
   fix-up round. T44 overlap with T74's validator is eligible ONLY while
   both specs' file lists stay disjoint (T74: src/api.rs only; T73:
   src/main.rs, src/driver.rs, tools/registry, README.md) — verify at
   dispatch; when in doubt, run serially.
3. **T75** (doctrine, pri 3) — runs ALONE (doctrine items never overlap);
   REQUIRED kimi; the T72 in-place-hunk + pin-file pattern, one round.

**Human item:** I1 loopd restart (§2). **Watch items carried:** driver.rs
3,607 lines — the ~4,500 trip line is PRE-DECLARED now (a future crossing
carries a T71-class extraction mandate); delegate-paragraph density
(second eval); t72-impl duplicate harvest (cosmetic); decision_log's
first organic records land THIS eval (T70 acceptance datapoint); F13
phases 2–3 deferral stands.

## Outcomes (filled at cycle wrap — LOOP-SPEC Phase 3)

### Cycle 37 (2026-09-26, ~13:22– EDT) — freshness-skip; T73 recovered mid-arc + landed (merge 87fe53f); I1 RESOLVED

- **T73 landed** (`chug plan` read-only planning mode, FEATURES.md F2
  phase 1; merge `87fe53f`, row flip this cycle). Recovery arc per the
  cycle-36 row recipe: fix-up child (pid 94084) budget-died 50/50 with
  the seam half-written → T63 SEVENTH resume (pid 2638, decision
  d1790443812-1) goal accepted 12/50 → `3b90103` (run_turn seam:
  `run_plan_loop` takes `&mut dyn Llm` + `McpRegistry`, ZERO impl
  semantic changes; 5 loop-level killing tests each RED-proven — M7
  run_start mode, M7b ensure_seeded, M7c archive_stale, M11 dropped
  is_error guard = the real exit-semantics hole, M-ext live-MCP-registry;
  hand-written run_start mirror deleted). Orchestrator gates
  independently re-run 585/585 + clippy under target-shared. Kimi round
  2 VERDICT PASS 0 blocking findings (46/50, routing d1790444316-2,
  verdict d1790444813-3): 9 mutants ALL caught RED, gates re-run under
  target-shared-validate; 3 non-blocking observations (sandbox-escape
  pins assert against the shared $TMPDIR root — escaped writes land for
  real during mutation runs, non-hermetic/sticky-RED, follow-up
  candidate; submit_plan schema minLength:1 nit, runtime-enforced +
  RED-pinned; MCP no-extension leg premise-asserted not mutated).
  **Merge incident absorbed:** the fix-up child had cherry-picked
  dbda51a onto main at cycle 36's tail (5464878) and reset main twice;
  cycle 37 opened by verifying main at the true tip (7de118a =
  origin/main), confirmed cherry-pick↔dbda51a driver.rs byte-identical,
  and resolved the merge's 7 add/add driver.rs hunks (cherry-pick broke
  shared ancestry) `--theirs` byte-exact to loop-t73. Post-merge gates
  585/585 + clippy in main under target-shared-main. FEATURES.md F2
  annotated SPLIT/phase-1-landed. 5 artifacts harvested (fixup +
  validate2 events/ledgers, child decisions.jsonl — 2 child-side verdict
  records); validate1 ledger verified identical to the cycle-36 harvest.
  8 outcome backfills (all fixed-up — the arc ran a fix-up).
- **I1 RESOLVED (operator):** loopd restarted between cycles 36/37 (new
  supervisor pid 94890, 17:21Z) — digest auto-regenerated pre-cycle
  (157 streams/7194 iters, the SEVENTH-manual-regen pattern ends), the
  launch line runs 160/240 not 120, T36/T46/T50/T53 all live. Nine
  restatements of the human item close here.
- **Cycle-level notes:** T63 resume exercised a SEVENTH time (7/7
  accepted on resumed segments). The T72 sweep-the-family directive
  closed the round-1 FAIL in ONE fix-up round (vs T69's three) — first
  live evidence the doctrine shortens the vacuous-class arc. Cycle-36
  wrap artifact (EVALUATION.md Outcomes) committed by cycle 37 at open
  (7de118a) — the T34 per-item rule kept the mid-arc death lossless.
- **T76 (pri 1, tgrep) next; T77 (pri 2); T75 (pri 3 doctrine, runs
  alone, REQUIRED kimi).**

### Cycle 36 (2026-09-26, ~11:52– EDT) — MANDATORY fresh eval (F2 SPLIT → T73/T74/T75 filed); T74 landed; T73/T75 next

- **Eval committed `4db039d` + pushed.** ROADMAP PULL F2 executed as a
  SPLIT (FEATURES.md working rules): phase 1 → T73 (`chug plan` read-only
  mode, pri 2), phase 2 deferred (/plan chat + --approve gate +
  web_fetch-in-plan — written reason §4). Incident rows T74 (api.rs
  body_watchdog cold-parallel flake, 2 sightings one day, pri 2) + T75
  (decision_log zero organic adoption cycles 34+35 — wrap-checklist
  records sentence + pin, pri 3 doctrine). I1 loopd-restart carry
  RESTATED 9th, mechanism fully nailed (supervisor pid 90114 predates
  t36/T46/T50/T53; bash runs parsed compound commands from memory —
  cycles run 120-not-160, digest 7th manual regen, 0 re-execs, pgrep
  guard). driver.rs 3,607: ~4,500 trip line PRE-DECLARED. README audit:
  3rd consecutive zero-finding. **First organic decision_log records: 7
  eval-triage (3 filed + 4 rejected)** — T70 acceptance datapoint.
- **T74 landed: merge `ef0c3eb`** (per-item entry written at landing,
  T34). glm first-try goal accepted 46/50 (`0f1475e`, tests-only +46/-9
  confined to src/api.rs mod tests): aborts_silent upper 1900ms → 15s
  (activity-vs-600s-total discrimination kept, 40x under; 900ms
  not-instant lower kept), allows_slow_steady 250ms×5 → 50ms×60 (total
  3x the timeout, gaps 20x under, load-monotone), family sweep
  documented in-code. RED-proofs both directions in the commit;
  acceptance 10/10 consecutive default-parallel full suites 563/563.
  Orchestrator gates independently re-run 563/563 + clippy under
  target-shared. **kimi VERDICT PASS, 0 blocking findings** (24/50):
  gates re-run under target-shared-validate + 3 more default-parallel
  suites, 3 mutants ALL RED (gutted → aborts_silent only; total-deadline
  → steady only; instant-fire → 900ms lower at 811µs, load-bearing), 2
  non-blocking observations. Impl child logged 5 organic decision_log
  records (margin designs ×2, family sweep, RED-proofs, outcome
  backfill) + validator 1 — first child-side organic adoption. Post-merge
  gates 563/563 + clippy in main under target-shared-main. 5 artifacts
  harvested (2 events, 2 ledgers, child decisions.jsonl).
- **T73 MID-ARC at budget wrap** (recovery recipe on its row, T28/T69/T70
  precedent; per-item entry written at wrap, T34). glm impl 50/50 abort
  with uncommitted work → T63 FIFTH resume, goal accepted 48/50
  (`dbda51a`, +1270/-16: src/plan.rs 421 new — five-tool schema filter +
  dispatch rejection of every excluded name incl. `mcp__*`; driver
  Mode::Plan + run_plan_loop + submit_plan→GoalAccepted-exit-0; main.rs
  clap subcommand 30/20 defaults; README integrated section). Orchestrator
  review passed: 580/580 + clippy under target-shared, production hunks
  read. **kimi round 1 VERDICT FAIL — weak tests, not correctness**
  (validator 50/50 pre-verdict → T63 SIXTH resume accepted 2/50): impl
  CORRECT reqs 1-8, gates re-run under target-shared-validate, M1-M6 all
  RED; 3 SURVIVORS one class — run_plan_loop startup/exit guarantees
  unpinned at loop level (concrete Client blocks ScriptedLlm): M7
  run_start mode, M7b ensure_seeded insertion, M11 dropped is_error
  guard (a rejected submit_plan would exit 0 — the real hole). GLM
  FIX-UP CHILD IN FLIGHT at wrap (pid 94084, same preserved worktree;
  goal carries the findings + class + 5-leg sweep-the-family directive,
  T72 doctrine's first live use). 4 completed-segment artifacts harvested;
  the in-flight stream + validator-resume segment harvest next cycle.
  Next cycle: collect fix-up verdict → kimi round 2 → merge/flip/
  FEATURES.md F2 SPLIT annotation per the row recipe.
- **T75 deferred with a ready spec** (doctrine, runs alone — never
  dispatched this cycle; the wrap-checklist sentence it adds was
  nevertheless exercised THIS cycle: 17 decision_log records incl. the
  3 filed-row triages, 4 rejects, 2 recovery-routings, 2
  validation-routings, 1 verdict, 3 outcome backfills + child-side
  records — adoption is now organic, the row pins it).
- **Cycle-level notes:** T44 overlap #4 (T73 impl during T74 validator,
  disjoint file lists — clean). T63 exercised twice more (6 total, all
  accepted on resumed segments). The kimi round-1 FAIL is the second
  consecutive cycle where mutation testing caught a coverage-class hole
  a green suite masked (T69's vacuous latches, now T73's loop-level
  startup/exit guarantees) — the validator's root-cause framing
  (concrete Client blocks ScriptedLlm) gave the fix-up its shape.

### Cycle 35 (2026-09-26, ~10:42– EDT) — T70 recovered mid-arc + landed (merge 572ec5a); T71/T72 next

- **T70 recovery arc executed from the cycle-34 row recipe (per-item
  entry written at landing, T34).** Kimi round-2 validator (pid 54532,
  in flight at cycle-34 wrap) exited goal-accepted 42/50; worktree tree
  verified BYTE-CLEAN pre-merge (diff exactly b98ca1c + f50fac6, no
  leftover mutants, no unexpected untracked files). **VERDICT: PASS, 0
  blocking findings** — round-1 fix verified tests-only (impl portion
  byte-identical b98ca1c↔f50fac6) and lethal: fix-verification mutants
  ALL RED (M1a rename — the round-1 survivor — plus rename-other-class,
  drop-class, reorder siblings); own mutation suite 13 implementation
  mutants ALL RED (validation legs, confidence bounds both directions,
  append drop, counter, id echo, best-effort context, key order, id
  prefix, required list, description tokens, create_dir_all). 3 surviving
  mutants all non-blocking observation-class outside the spec's
  enumerated test contract (ts presence-only matches the riskgate.rs
  precedent standard; schema min/max + property types advisory with
  code-side enforcement fully pinned).
- **Landed: merge 572ec5a.** Post-merge gates re-run in main under
  target-shared-main (T57 ALWAYS rule): build + clippy --all-targets
  -D warnings + 559/559 green. Validator events + ledger harvested to
  .chug (final round-2 stream alongside the cycle-34 inflight snapshot).
  FEATURES.md F13 annotated phase 1 landed (phases 2–3 remain deferred:
  corpus + layad endpoint absent). New watch item carried from the impl
  arc: the first goal-gate rejection was a cold-build flake — api.rs
  body_watchdog timing suspected, 4x green re-runs (worth a row if it
  recurs).

- **T71 landed: merge e1e940c** (per-item entry written at landing, T34).
  glm impl hit 50/50 with the move complete-but-uncommitted → **T63's
  FOURTH live resume exercise**, goal accepted 19/50 (commit 3b732c9).
  tools.rs 5237→1783, delegate.rs +3485: 26 prod fns/structs + 9 consts
  byte-identical (sole delta the required pub(crate) on delegate()),
  test partition exact 133 = 54 retained + 79 moved, delegate filter
  count 71/448 exact match to main @ b6b22c7, hand-check RED-proven.
  Orchestrator review independently verified schema byte-identity,
  dispatch-arm call path, no pub use shim, and the fn-name inventory
  before gating 559/559 + clippy green. **kimi VERDICT PASS, 0 blocking
  findings** (accepted 50/50 under budget_low): byte-identity
  re-verified, BEFORE count re-run in a separate worktree, 4 structural
  mutants ALL caught (dispatch-arm swap, deleted moved test, wait-cap
  loosen → boundary pin RED, stale duplicate → clippy dead_code);
  visibility-widen class closed by exhaustive diff inspection. 3
  non-blocking observations (README `decisions` layout-list omission
  noted pre-existing at b6b22c7 — a T70-era gap worth a one-word fix
  next docs pass). Post-merge gates 559/559 + clippy in main under
  target-shared-main. 4 artifacts harvested. **Watch item escalated:
  api.rs body_watchdog cold-parallel flake sighted TWICE today** (T70
  impl first goal-gate; T71 impl 518/1 parallel run) — both re-ran green
  3–4×; a candidate row for the next eval.

- **T72 landed: merge db74a99** (per-item entry written at landing, T34).
  glm first-try goal accepted 26/50 (8a8e18a): ONE in-place hunk extends
  the §2 step-4 FAIL-arc sentence (fix-up goals now name the CLASS and
  require every instance swept with its own RED-proven killing test,
  citing cycle-33/T69), no renumbering; tests/loop_spec_sweep_family.rs
  (T63/T64 precedent) pins the needles exactly-once inside the step-4
  window; deletion hand-check RED-proven. **kimi VERDICT PASS, 0 blocking
  findings** (20/50): 7 doc-side mutants all behaved as required —
  delete/reword/relocate RED, weakened at-least-once pin escape-then-RED
  (load-bearing proven, not vacuous), heading reword survives (loose
  match). Orchestrator gates 563/563 + clippy pre-merge. QUEUE DRAINED
  at landing — next cycle opens with a mandatory fresh eval. Watch item
  for that eval: api.rs body_watchdog cold-parallel flake, 2 sightings
  today (T70 impl goal-gate, T71 impl 518/1); both re-ran green.

### Cycle 34 (2026-09-26, ~09:55– EDT) — MANDATORY fresh eval (F13 SPLIT → T70/T71/T72 filed); T70 MID-ARC at budget wrap (validator-2 in flight)

- **Eval committed `d5ddac5` + pushed.** ROADMAP PULL F13 executed as a
  SPLIT (FEATURES.md working rules): phase 1 → T70 (pri 2), phases 2–3
  deferred with written reasons (corpus + layad endpoint absent);
  FEATURES.md F13 annotated SPLIT, not checked off. Incident rows T71
  (tools.rs 5,233 — pre-declared ~4,500 trip line CROSSED; delegate
  extraction, pri 3) + T72 (sweep-the-family fix-up doctrine — T69's
  3-round vacuous-pin family; pri 3). I1 loopd-restart carry RESTATED
  8th (cycle 32 died AT 120/120 mid-arc; digest stale at eval start,
  6th manual regen). T63 resume watch CLOSED (2 live exercises
  pre-cycle). README audit: 2nd consecutive zero-finding.
- **T70 impl arc (per-item entry at the wrap, T34).** glm impl 50/50
  BUDGET ABORT with work uncommitted (died running final gates — 6th
  occurrence of the class) → **T63 THIRD live exercise: resume accepted
  18/50** → first goal-gate REJECTION diagnosed by the child as a
  cold-build flake (api.rs body_watchdog timing suspected; check re-run
  4x green; NEW WATCH ITEM for the next eval) → goal accepted, committed
  b98ca1c (+src/decisions.rs 562 lines, 2 registration lines tools.rs,
  1 mod line, 4 LOOP-SPEC sentences, README list+sentence). **delegate
  collect dogfooded live for the first time** at the review (verdict +
  full summary + check cmd + scoped refs in one call; base param keeps
  output small). Orchestrator gates independently re-run: clippy clean,
  559/559 (7 suites), spec check verbatim — under target-shared.
- **Kimi round 1 VERDICT: FAIL, 1 finding (29/50)** — implementation
  verified correct and spec-complete, gates re-run green, 9/10 mutants
  killed; the survivor: the schema pin's seed-class leg was VACUOUS
  (looped over SEED_CLASSES, the const that generates the description;
  M1a rename survived green). glm fix-up f50fac6 (tests-only +37/-1):
  test-local PINNED_SEED_CLASSES literals + exact rendered-list leg
  (per-token contains cannot kill a rename) + class sweep clean; M1a
  proven RED post-fix. Kimi round-2 validator pid 54532 IN FLIGHT at
  wrap. Recovery recipe on the T70 row; worktree PRESERVED; 7 artifacts
  harvested (4 events incl. -inflight validator-2 snapshot, 2 ledgers,
  check-rerun log).
- **Deferred:** T71, T72 unworked — specs ready, strictly serial behind
  T70 (file overlaps + doctrine isolation). T44/T45 unused (T70 doctrine
  runs alone).
- **Verdict: eval + T70 validated-in-flight; recovery is one collect
  call at next cycle's start.**

### Cycle 33 (2026-09-26, ~09:20– EDT) — T69 (F1 delegate collect) recovered + landed (merge c6ce238); QUEUE DRAINED

- **T69 recovery arc executed from the cycle-32 row recipe (per-item entry
  written at landing, T34).** Validator seg-1 confirmed budget-aborted
  50/50 pre-verdict; worktree tree verified BYTE-CLEAN of leftover mutants
  pre-resume (git status empty, diff = exactly ccd9923 + 138d042). **T63's
  SECOND live exercise: validator resume, goal accepted 3/50** — VERDICT:
  FAIL, but implementation verified CORRECT on every reviewed leg (gates
  independently re-run 550/550 + clippy + 70/70 under
  target-shared-validate; 8 mutants, 6 caught): the 2 survivors were
  vacuous-pin findings, not behavior bugs — (1) abort_reason render block
  dead code per the suite, (2) verdict_latch leg of the run_start segment
  reset vacuous (masked by post-resume accepted overwrite).
- **Two fix-up rounds, each RED-proven.** glm fix-up 50dcbe7 (+68
  tests-only, dispatch-level aborted-fixture render pin + post-resume
  NO-verdict leg, both mutants proven RED then restored; accepted 27/50).
  Kimi round 2 (accepted 45/50) confirmed both survivors killed + fix-up
  tests-only + T68 contamination/churn mutants die — and found ONE new
  survivor of the same class: the check_cmd latch reset (leak demonstrated
  Some("check A") vs None). glm fix-up2 88227ab (+4, one assertion,
  RED-proven; accepted 15/50). Kimi round 3 (accepted 22/50): **VERDICT
  PASS, 0 findings** — all four run_start latch resets swept, each with a
  killing test; gates 551/551 + clippy + 71/71; tree byte-clean.
- **Landed: merge c6ce238**, post-merge gates 551/551 + clippy under
  target-shared-main. FEATURES.md F1 checked off (Already landed + row
  struck). 6 child-run event streams + 2 ledgers harvested to .chug
  pre-merge. The lesson the arc teaches: the run_start latch-reset class
  was vacuous as a FAMILY — each reset leg needed its own verdict-less
  post-resume pin; validators catch survivors one leg per round when the
  fix-up pins only the named leg. Sweep-the-family guidance went into the
  round-3 goal and closed it.
- **Verdict: T69 done; queue DRAINED at landing → the next cycle opens
  with a mandatory fresh eval whose mandated ROADMAP PULL is F13
  (decision logs → Laya distillation, operator directive dc18a7a).**

### Cycle 32 (2026-09-26, ~08:51–09:16 EDT) — delta eval + ROADMAP PULL (T69 = F1 delegate collect); T69 MID-ARC at budget wrap (validated-in-flight)

- **Delta eval + mandatory pull (committed 50f1caf).** Queue was EMPTY 3 min
  before launch (cycle-31 wrap 08:49) → fresh eval mandatory; a full re-read
  of a 3-minute-old eval would burn ~45 iters for zero new corpus, so the
  eval is a DELTA over the cycle-31 streams: no new incident rows (wrap
  clean — T63 armed 0 exercises, T44/T45 unused, t68 49/50 inside the T18
  margin). ROADMAP PULL executed per operator directive 259b5ed + dc18a7a:
  F1 → T69 (pri 2, spec + row); F13 (decision logs → Laya) named the NEXT
  fresh eval's mandated pull — one item per pull per FEATURES.md rules, no
  re-triage needed. I1 loopd-restart carry RESTATED 7th time (this cycle's
  own launch line: --max-iters 120, T36's 160 dormant).
- **T69 (F1 delegate collect) — impl landed, validation in flight at wrap.**
  glm impl child 50/50 BUDGET ABORT with uncommitted work on all three
  surfaces → **T63's FIRST live exercise: resume relaunch, goal ACCEPTED
  25/50 on the resumed segment** (impl ccd9923, +863/−16: collect action —
  latest-segment verdict + accepted-goal summary + latest verifying cmd +
  bounded git-log commit refs with T20-precedent degrades; separate
  CollectSummary keeps the T68 six-field wake set byte-untouched; schema,
  README clause, LOOP-SPEC §2-step-3 adoption sentence; +18 pins).
  **Orchestrator review gates caught a real defect the child's gates
  missed**: the real-checkout integration pin whitelisted only two verdict
  states (mid-run/clean) and red-fired on the child's own goal-accepted
  stream — a test whose truth depends on the live run state of the checkout
  it runs in; orchestrator review-fix 138d042 pins the verdict line's SHAPE
  (five known verdicts), never the state. Gates post-fix: 550/550 + clippy
  + spec check 70/70 under target-shared. Kimi validator (pid 75343,
  50/30) reached 46/50 mid-mutation at my budget wrap — M2/M3/M5 mutants
  caught, M1 (abort_reason render line) UNCAUGHT survivor noted, gates
  independently re-run green mid-run. Verdict unread at wrap.
- **Recovery recipe lives on the T69 row** (validator verdict → mutant-free
  tree check → merge/flip/FEATURES-check-off/push, or fix-up arc on FAIL).
  Worktree /tmp/chug-loop-t69 PRESERVED with both commits; impl events +
  validator partial stream + impl ledger harvested to .chug/ pre-wrap.
- **Verdict: roadmap pull executed, feature impl complete + review-hardened,
  merge deferred one cycle on the validation arc — the per-item Outcomes
  doctrine (T34) means this entry exists even though the row is not done.**

### Cycle 31 (2026-09-26, ~08:06– EDT) — MANDATORY fresh eval (queue EMPTY); T67 + T68 landed — QUEUE DRAINED

**T68 — `delegate status` `wait_secs` wakes on significant change only
(pri 3, src/tools.rs + README clause, +283/−16) → done `362e9b9` (impl
`6e10195`, merge commit).** glm impl goal-accepted at 49/50 (budget_low
at 8, wrapped inside the T18 margin): `DelegateSummary::significant_ne`
compares exactly the six-field wake set (max_iters, last_iteration,
budget_low_seen, goal_seen, abort_seen, abort_reason) with a
field-by-field pin so a field cannot silently migrate between the wake
set and the churn set; `last_event` churn still renders (the deadline
leg's final read) but never wakes; the creation/liveness/deadline legs
are byte-unchanged and T29's byte-identical instant-leg pin passes
untouched. The demand evidence got one more leg DURING this very arc:
the T67/T68 child polls woke at 2s on `wait_secs: 100`–`110` four times
before the orchestrator fell back to bash-sleep pacing — the leak
fixing itself mid-cycle, recorded. Doc honesty at all three surfaces
(doc comment, both schema descriptions, README clause); the child
deliberately left LOOP-SPEC §2's polling sentence untouched
(orchestrator-facing, makes no wake-condition claim — the validator
agreed, finding 4). kimi VERDICT: PASS 39/50 — reqs 1–4 + acceptance
verified point by point, gates independently re-run (532/532 + clippy +
spec check 18+13 under target-shared-validate), 6/6 mutants killed
(any-field revert RED at 2.650s vs the 2.9s lower bound — reproducing
the child's own 2.651s evidence, gutted `significant_ne`, churn field
added to the wake set, goal_seen dropped, stale schema description,
stale README clause), tree restored byte-clean with final gates re-run;
one non-blocking observation (the churn pin's 30s upper bound is loose;
the 2.9s lower bound + `waited ≥ 3` tripwire are load-bearing).
Orchestrator gates independently re-run 532/532 + clippy under
target-shared in-worktree, then 532/532 + clippy post-merge under
target-shared-main. 3 artifacts harvested (impl + validator events,
validator verdict ledger; the impl's ledger was seed-trivial — zero
`update_ledger` calls — skipped per T32). T29's long-poll premise
(cycle 10: 13% of orchestrator iterations on instant polls) is finally
realized: a healthy child now holds a `wait_secs` block until its
iteration advances, a verdict/flag appears, it dies, or the deadline —
the ~10–25-iteration-per-child leak on 120-iteration cycles is closed.

**T67 — spec check lines never invoke `cargo test --lib` (pri 2,
DOCTRINE: META-META-SPEC.md + tests/todo_consistency.rs + t64 spec
repair, +135/−15) → done `0d3c88b` (impl `1fa4238`, merge commit).**
glm impl first-try goal-accepted at 27/50 in ~6 min: the convention
sentence woven into META-META-SPEC's check-line paragraph (carries the
pinned token `no library targets`, bites list now names the nine
streams), t64's check line repaired to `--bin chug`, and the T8 guard
gained two legs — the specs-corpus lint (every `specs/t*.md`, col-0 AND
heading check-line forms, 60-file corpus floor + t64 membership pin so
it cannot pass vacuously) and the doctrine pin. The notable moment: the
pre-fix acceptance grep matched t64 AND **t67's own spec** — the row's
own prose carried the literal token its gate greps for, the exact
unsatisfiable-gate class being killed — so the impl self-scrubbed its
spec to the `--l[i]b` bracket idiom (T53's `[c]hug` trick) with an
explanatory note, and the validator independently verified the scrub
was forced, not gratuitous. kimi VERDICT: PASS 23/50 — reqs 1–4
reviewed, doctrine's factual basis re-verified (`cargo test --lib` →
exit 101), gates independently re-run 527/527 + clippy under
target-shared-validate, spec check line passed verbatim, 9 mutant legs
killed (t66 col-0 injection, t52 heading-form injection — the heading
parser genuinely exercised, doctrine-token corruption, condition flip,
weakened-needle escape probe proving the needle load-bearing, floor
corruption, t64-pin corruption, and a specs/t999 probe file with T8-leg
isolation), tree cmp-restored clean. Orchestrator gates independently
re-run 527/527 + clippy under target-shared in-worktree, then 527/527 +
clippy post-merge under target-shared-main. 4 artifacts harvested
(impl + validator events, both ledgers — the validator's carries the
verdict, the impl's the self-reference plan). The nine-sighting
check-line defect class (t22–t64) is now closed by convention + lint +
pin; the next eval that writes `--lib` into a spec check line fails
`cargo test` at commit time.

**Skipped/deferred:** none — queue drained 2/2 (T67, T68).

**Cycle notes.** (a) **The operator landed doctrine mid-cycle** —
`259b5ed` (FEATURES.md capability roadmap + META-META-SPEC §4 rewritten
to the mandatory ROADMAP PULL + README pointer) and `dc18a7a` (F13
directed to pull right after F1), committed 08:14/08:17 EDT during the
T67 arc. The T67 merge absorbed them; gates ran green on the combined
tree and BOTH META-META needles (`ROADMAP PULL` + `no library targets`)
verified co-present with the guard 5/5 — the operator's edit and T67's
survived each other. Human commits, not a second driver; the invariant
never tripped. (b) **Next cycle: queue EMPTY → MANDATORY fresh eval, and
the roadmap pull is now mandatory** (amended META-META §4): pull **F1
(delegate collect)** into TODO.md with a full FEATURE-class spec (the
working rules allow splitting into 2–3 rows when scope blows a 50-iter
child budget), then **F13 (decision logs → Laya distillation)** per the
operator's explicit ordering; check items off in FEATURES.md in the
row-flip commit. (c) T44 unused (serial 2-row queue; T67 doctrine runs
alone and nothing sat behind T68); T45 not applicable (doctrine row +
different file areas). (d) **T63 resume armed, 0 exercises** — t68-impl
hit 49/50 with budget_low@8 but goal-accepted inside the T18 margin; the
first live resume remains the acceptance datapoint. (e) **The T68 leak
produced its last raw evidence in this very cycle**: pre-merge, four
`wait_secs: 100–110` long-polls during T67/T68 child waits woke at 2s on
tool-call churn before pacing fell back to bash sleeps; post-merge the
fix IS main's behavior, and the first long-poll that holds to a real
window is the live acceptance. (f) glm one-arcs all four children
(27/50 + 49/50 impls, 23/50 + 39/50 validators — no model fallback);
validators killed 15/15 mutant legs (9 + 6) with one non-blocking
observation (churn pin's 30s upper bound loose; the 2.9s lower bound +
`waited ≥ 3` tripwire are load-bearing). (g) **Self-scrub lesson**:
t67's own spec carried the literal token its gate greps for (pre-fix
acceptance grep matched t64 AND t67) — the impl bracket-scrubbed its
spec (T53's `[c]hug` idiom) with an explanatory note and the validator
verified the scrub was forced. Watch-level: future evaluators writing
grep-gated specs must check the spec's own prose against its gate; file
a doctrine row if it recurs. (h) Outcomes compaction this wrap: cycle 25
→ one-liner (last-6-full now 31–26); the stale cycle-18 double-heading
note from cycle 25 was overtaken — only one `### Cycle 18` heading
remains after earlier compactions. (i) **loopd-restart human carry
RESTATED**: supervisor pid 90114 now SIX revisions stale (T36/T46/
T47×2/T50/T53) — this cycle again ran 120 iters not T36's 160 and the
digest was stale at eval start (4th manual regen); the operator
one-liner is in the Handoff above.
**Final state:** main pushed through `3600061`; post-merge gates 532/532
+ clippy under target-shared-main at both landings; todo_consistency
5/5 green before every TODO commit; 7 artifacts harvested to .chug
(gitignored, local) — 4 T67 + 3 T68; both worktrees removed, both
merged branches deleted; wrap landed at ~111/120 iterations.

### Cycle 30 (2026-09-26, ~07:28–08:05 EDT) — freshness-skip; T64 + T65 landed, then goal-gate flake sighting → T66 filed + landed — QUEUE DRAINED

**T66 — dead_port_probe drop→probe leg bounded theft-retry (pri 2,
robustness, tests-only: src/mcp_http.rs test module +146/−14) → done
`ac2ea36` (impl `e418b8e`, merge commit).** Filed LIVE at the cycle's
own goal gate: the wrap's first goal_complete was REJECTED by
`dead_port_probe_distinguishes_live_from_dead` failing at
default-parallel `cargo test` — "dropped port 50956 did not refuse
connections" (src/mcp_http.rs:1747), the first post-T59 organic
sighting of the port-theft class. Diagnosis before dispatch: T59
wrapped only the test's third (acquire-verify) leg; the second
(bind→drop→re-probe) leg carries the identical window unretried —
passes isolated, 4 green runs at `--test-threads=4`, nothing in the
cycle touched mcp_http. glm impl first-try goal-accepted at 27/50 in
~8 min: new sibling driver `dead_port_probe_retry_with` (the spec's
endorsed fallback — `dead_port_retry_with` not contorted: its acquire
yields a bare u16 while this attempt must own the listener across the
live probe). One attempt = bind + live-probe + drop + dead-probe; the
live leg's failure panics un-retried (no theft window → real
regression); a live reading after the drop goes through
`theft_or_regression` UNCHANGED (still live at catch → theft → fresh
stub; refuses → regression panics, never retried into green); bounded
by the reused `DEAD_PORT_RETRY_ATTEMPTS`; exhaustion names the
attempts + mechanism. Two scripted-theft pins use a try_clone-dup'd
socket to keep attempt 1's port genuinely live across the whole drop
window — mechanically identical to a thief, deterministic, race-free —
pinning exactly-once retry and exhaustion naming. Non-vacuousness run
both directions by the child (probe gutted to `false` → retried test
exhausts and panics, never masked into green; `true` → live-leg assert
catches it; revert → green). kimi skipped per the row (tests-only,
T16/T31/T59/T62). Orchestrator gates independently re-run: 525/525 at
BOTH `--test-threads=4` AND default parallelism (the exact goal-gate
invocation that false-red) + clippy under target-shared in-worktree,
then 525/525 post-merge under target-shared-main at default
parallelism. 1 artifact harvested. The class is now closed at all
three legs of the probe test; a FUTURE sighting elsewhere in mcp_http
is a new row, not a retry of this one.

**T65 — README Continuous-mode target-cache paragraph de-accretion (pri
4, docs-only: README.md single hunk 16+/16−, net 0) → done `458751c`
(impl `99b78fa`, merge commit).** glm impl first-try goal-accepted at
19/50 in ~4 min (the check line's two legs — `--test shared_target_dir`
+ `grep -c "target-shared-main" README.md` — are both satisfiable, so no
repeat of T64's check-line rejection). The 17-line T47+T52+T57 sentence
chain (three nested em-dash parentheticals) became two short paragraphs:
(a) user semantics — the four gitignored role-keyed caches with their
roles, warm-after-first-use, the per-invocation env-prefix fact
compressed to a clause pointing at loopd.sh for the rationale, one why
sentence (artifact filename excludes the checkout path → last-builder-
wins → role-keyed keeps each consumer's artifacts its own), operator
reclaim; (b) one pointer sentence naming specs t47/t52/t57 (all three
filenames verified to exist). Pinned surface preserved with the pin file
untouched: `target-shared-main/` still exactly 1× README-wide and still
co-located with `target-shared-validate/` + `post-merge` in one
paragraph — tests/shared_target_dir.rs 14/14 green unchanged. All other
README sections byte-identical (single hunk). kimi skipped per the row
(docs-only, T51/T56/T60). Orchestrator gates 523/523 + clippy under
target-shared in-worktree, 523/523 + clippy post-merge under
target-shared-main. 1 artifact harvested (events; child ledger was
boilerplate, not harvested). The README gate is satisfied by the edit
itself — the accretion spot is gone, integrated at its original
position.

**T64 — Pin the two validator survivor classes (pri 4, tests-only:
tests/shared_target_dir.rs +62, src/tools.rs test module +88) → done
`1859b21` (impl `95bc91a`, merge commit).** glm impl completed the work
and self-committed at 48/50, then hit the cycle's notable finding:
goal_complete was REJECTED not by the work but by the spec check line's
own defect — `cargo test --lib` exits 101 on this binary-only crate
("no library targets found"), a defect pre-observed in eight prior child
streams (t22/t25/t26/t29/t39/t42/t58/t59) and now a candidate row for
the next eval (check lines should say `cargo test --bin chug`). Leg 1
(`--test shared_target_dir`) had passed 14/14 first. Nothing was
incomplete, so no T63 resume — orchestrator review + gates + independent
mutant reproduction substituted per T55/T62. Pin (a)
`loop_spec_step5_window_carries_the_always_form_exactly_once` scopes
step 5 heading-to-heading with four legs (MAIN exactly 1×, "ALWAYS,
never conditionally" exactly 1×, the byte-exact combined carrier 1×, the
NOT-step-3 disambiguator) — the mechanism sentence's "ALWAYS main
checkouts" can no longer stand in for the rule form. Pin (b)
`delegate_summary_run_start_gap_keeps_last_seen_max_iters_and_iteration`
asserts the gap window after segment-2's run_start (max_iters Some(50)
from the latest run_start, last_iteration Some(40) still segment 1's —
deliberately unequal values prove provenance), then segment-2 values
after its first iteration. Child's non-vacuousness evidence reproduced
BOTH survivor stories with before/after output; the orchestrator
independently re-ran the M5 mutant → new pin RED (13 passed/1 failed,
old T57 pin staying green — the exact survivor mechanism), revert →
14/14. Gates: 523/523 + clippy under target-shared in-worktree, then
523/523 + clippy post-merge under target-shared-main. kimi skipped per
the row's optional-validation + T16/T31/T59/T62 precedent. 2 artifacts
harvested (events + non-trivial LEDGER carrying the check-line defect
report).

**Skipped/deferred:** none — queue drained 3/3 (T64, T65, then T66
filed LIVE at the wrap's goal gate and landed in-cycle).

**Cycle notes.** (a) The cycle's payload finding is the **spec
check-line defect**: T64's goal_complete was rejected by the spec's own
`check:` — `cargo test --lib` exits 101 on this binary-only crate
(src/main.rs, no lib target), AFTER leg 1 (`--test shared_target_dir`)
had passed 14/14 — the NINTH child-stream sighting (t22/t25/t26/t29/
t39/t42/t58/t59 + t64). The rejection burns the child's remaining
budget on an unsatisfiable gate and could mask real red. The fix is
evaluator-side, not historical-spec-side: META-META's spec-authoring
convention should write `cargo test --bin chug` (the crate's unit-test
target) or plain `cargo test`, never `--lib`. Candidate row for the next
eval. (b) The T63 resume doctrine was NOT exercised: T64's impl had
completed AND committed before its budget death, so the T55/T62
orchestrator-finish recipe applied (nothing incomplete to resume into);
resume remains the recipe for incomplete-in-worktree deaths. (c) T45
bundling REJECTED at dispatch — both rows pri 4, failing conjunctive
condition (d) (pri ≤ 3); T44 overlap unused (no validators in flight —
both rows validation-skipped, so the overlap window never opened). (d)
The `wait_secs` early-wake watch item fired on EVERY long-poll (woke at
2–7s on a 90–110s request); pacing fell back to bash sleeps + instant
status — still watch-level, not row-level. (e) Both children were glm
one-arcs: T64 48/50 committed (rejection + 2-iter tail), T65 19/50
first-try goal-accepted ~4 min — no model fallback needed. (f) The
README gate is satisfied intrinsically: T65's edit IS the README
integration, at the paragraph's original position. (g) **The wrap's
first goal_complete was itself the T66 trigger** — the goal gate
(default-parallel `cargo test` in the repo target/) caught what four
`--test-threads=4` gate runs had not: the last unretried port-theft
window. The arc — diagnose (passes isolated, class signature,
nothing touched mcp_http) → file pri-2 row + spec → standard child arc
→ land → re-attempt — cost ~20 min wall and closes the class at all
three legs; the goal gate is no longer a coin flip on this flake.
**Final state:** main pushed through the wrap; post-merge gates
523/523 (T64, T65) then 525/525 (T66, at DEFAULT parallelism — the
goal-gate invocation) + clippy under target-shared-main at all
landings; todo_consistency 3/3 green before every TODO commit; 3 events files + 1 non-trivial LEDGER (4 artifacts) harvested to .chug (gitignored,
local); all three worktrees removed post-harvest.
**loopd-restart human carry RESTATED:** supervisor pid 90114 still
running 5+ revisions stale (started 8:43PM, pre-T57) — the next manual
restart picks up everything since; no automated restart is safe
mid-cycle.

**Next cycle: queue EMPTY → MANDATORY fresh eval.** Carries for it:
(i) the check-line defect candidate row (note a above — pri ~2, every
impl child whose spec carries the `--lib` check burns budget on an
unsatisfiable goal gate); (ii) `wait_secs` early-wake (note d); (iii)
restart loopd (human).

### Cycle 29 (2026-09-26, ~07:10–07:30 EDT) — MANDATORY fresh eval (queue EMPTY); T63 landed (doctrine: delegate resume recovery)

**T63 — LOOP-SPEC adopts `delegate resume:true` as the standard child
budget-death recovery (pri 2, DOCTRINE, LOOP-SPEC.md +
tests/loop_spec_recovery.rs) → done `db539c9` (impl `12c3d40`, merge
commit).** glm impl first-try goal-accepted at 29/50 in ~5 min: the
recovery leg landed in step 2's polling paragraph exactly where specced
(after "Exit of the pid = child done; then review.", before the nohup
fallback — whose bytes survived byte-identical, pinned with escaped
non-ASCII so editor normalization cannot silently unpin), naming the
BUDGET-abort condition, the ONE-resume-attempt cap, the T19/T58
mechanics, and the fix-up-children exclusion; 4 new pins (needle
exactly-once, in-step-2 ordering, byte-exact fallback, mechanics window)
with a deletion hand-check (3/4 red) plus two voluntary extra mutation
checks. kimi adversarial validation VERDICT: PASS 16/50 — reqs 1–3
verified point by point, the leg's `abort_reason: iteration budget
exceeded` example fact-checked against src/tools.rs (accurate), gates
independently re-run (521 green under target-shared-validate), and 5/5
mutants killed (leg deletion, fallback rewrap, leg moved past step 3,
duplicate cap needle, corrupt budgets); tree restored clean and
re-verified. Orchestrator gates independently re-run in-worktree (521 +
clippy under target-shared); post-merge gates 521 + clippy in main under
target-shared-main. 4 artifacts harvested pre-removal (impl + validate
event streams, validate verdict LEDGER, delegate.log). The loop's most
common child failure (five 50/50-class deaths on record, latest = T58's
own impl) now has an in-doctrine first recovery: one same-worktree
`delegate resume:true` relaunch, then the standing T55/T28 recipes.


**Cycle-level notes:** (a) **T64 + T65 deferred unworked — specs ready**;
the budget wrap arrived at 112/120 with T63 landed green (fresh eval +
one doctrine arc ≈ the whole 120-iter budget); next cycle the freshness
rule FIRES (same-day eval + 2 todo rows) → skip Phase 1, work T64
(tests-only: tests/shared_target_dir.rs + src/tools.rs test module) then
T65 (docs-only: README.md; shared_target_dir.rs only if pinned tokens
must move — the conditional pin-file overlap ruled out T44 bundling by
default; serial). (b) **T44 overlap not used** — T63 ran alone
(doctrine), and step 6 stopped dispatch at <15 remaining. (c) **T61's
first measured fire:** the orchestrator's own read_file of a /tmp
worktree path hit the NEW error string ("— cross-tree paths go through
bash") — the remedy now arrives AT fire time; one-iteration recovery via
bash, exactly as designed. (d) **New watch items for the next eval:**
`write_file` "missing or non-string field: path" fired 2× on this
orchestrator (large writes; immediate retry succeeded — the cycle-26
digest class, second sighting); `delegate status` `wait_secs` early-wakes
at ~2s on most polls while a child is chatty (cosmetic — the collapse
works best on quiet children). (e) **T63 arc stats:** glm impl 29/50
first-try ~5 min; kimi 16/50 ~3 min PASS, 5/5 mutants; arc ≈ 12 min
end-to-end; the impl volunteered two extra mutation checks beyond the
spec's hand-check. (f) **loopd-restart human carry RESTATED** — pid
90114 still runs 5 revisions stale; this cycle again wrapped at
budget_low@8 inside 120 (160 dormant); the digest was stale at this
eval's start (3rd manual regen).

**Final state:** main = `cb3efe9` (T63 merge `db539c9`, flip+Outcomes
`cb3efe9`); gates 521 + clippy green in main under target-shared-main
(post-merge, `db539c9`); todo_consistency 3/3 pre-commit; TODO truthful
(T63 done with refs; T64/T65 todo with ready specs); 4 t63 artifacts
harvested into `.chug/` (impl+validate streams, validate verdict LEDGER,
delegate.log) and the worktree removed (branch merged); README gate
satisfied (T63 is loop-internal doctrine — no user-facing surface; the
README's delegate paragraph already documents resume post-T58); pushed
through `cb3efe9`. **Handoff: freshness rule FIRES next cycle — work T64
→ T65 serially; loopd restart is the standing human item.**

### Cycle 28 (2026-09-26, ~06:39–07:15 EDT) — freshness-skip; ALL 4 queued rows landed green (T58 + T62 resumed from cycle-27's wrap, T61 + T60 fresh) — QUEUE DRAINED

**T58 — delegate launch gains `resume: true` + status summarizes the
latest run segment (pri 3, FEATURE, src/tools.rs) → done `5145536`
(impl `b189517`, merge commit).** Cycle-27's in-flight recovery
executed per the row's own recipe: the kimi validator finished 2 min
before cycle-27's budget wrap (VERDICT: PASS — gates 516/516
independently re-run under `target-shared-validate`; 5/6 mutants
killed: argv-append drop, latch-reset drop, default flip, echo drop,
non-boolean coercion; 1 survivor = over-reset of
max_iters/last_iteration at run_start, a weak test on spec req 4's
stream-scope sentence, non-blocking — the code is spec-correct).
Orchestrator step-3 gates independently re-run in-worktree (516 +
clippy clean), both child streams harvested (7 artifacts incl. the
rotated impl stream + validator LEDGER with the verdict; cycle-27's
`-inflight-` snapshots superseded + removed per the T28 precedent),
then merged before T61 per the row's ordering. What landed: launch's
optional `resume` boolean appends `--resume` as the argv tail (absent/
false = byte-identical whole-list pinned; non-boolean is a tool error,
never a silent fresh start), the return text echoes `resume: true`
(T39 pattern), and `summarize_events` resets the verdict latches
(goal/abort/budget_low + abort_reason) at each new `run_start` so a
resumed child's summary describes its LATEST segment — the cycle-18
bite (hand-rolled nohup resume + a healthy resumed validator reporting
`aborted`) is now expressible and correctly observed through the tool.
8 new test legs (argv pin, parse, two-segment summaries a/b/c +
single-segment regression, schema pin, e2e stub argv, synthetic
two-segment status) + README delegate paragraph clause integrated.
Dogfood note: the impl child itself died 50/50 with the work complete
but uncommitted — the fifth occurrence of the pattern this feature
addresses (t15/t17/t20/t55/t58); orchestrator finish per the T55
precedent produced `b189517`.

**T62 — eval-digest golden-section pin (pri 4, robustness, tests-only)
→ done `f4e3bc3` (impl `92f0243`, merge commit).** Cycle-27's second
in-flight recovery per the row's recipe: the glm impl finished cleanly
(self-committed + goal-accepted at 10:37:08Z, 2 min before cycle-27's
budget wrap — no orchestrator finish needed). Diff review: exactly
`tests/eval_digest.rs` +217 (the row's constraint). What landed:
`golden_section_pins_the_digest_output_skeleton` runs the real
eval-digest.sh via the existing `run_digest` harness (pinned clock)
against an all-fields fixture and pins (a) the header line exactly +
the summary-line field labels, (b) the three section headings each
once in order (Events files → Corpus inputs → Staleness), (c) the 10
per-file field lines in emission order with per-line shape templates
(a tiny `matches_shape` matcher: `#` = digit-run, `*` = any —
volatile values wild-carded so a legitimate data change cannot go
red, spec req 2), (d) the staleness labels in order. Closes the T46
validator's 7/7 output-field survivor class. kimi round skipped per
the row's optional-validation + the T16/T31/T59 tests-only precedent;
orchestrator gates independently re-run 508/508 + clippy clean
in-worktree, and the impl's LEDGER records the hand-mutation check
(field-order swap → RED → revert → green). 3 artifacts harvested
(impl events + LEDGER + delegate.log; the cycle-27 `-inflight-`
snapshot superseded + removed).

**T61 — resolve_safe error string names the `bash` fallback at fire
time (pri 4, DX friction, src/tools.rs) → done `c176321` (impl
`9d8e613`, merge commit).** glm impl goal-accepted 16/50 ~5 min
first-try: one shared `PATH_ESCAPES_CWD_SUFFIX` const (" — cross-tree
paths go through bash") appended at BOTH refusal construction sites
(`..`-past-root pop + outside-cwd prefix check), prefix byte-identical
so the T41 `starts_with` pin holds; the pin gains `bash` + single-line
assertions and two relative-cwd legs so the pop site is actually
reached (an absolute cwd can only reach the prefix check). Impl
hand-checked non-vacuousness (suffix removed → pin RED; restored →
green). kimi VERDICT: PASS 26/50 — gates independently re-run 517/517
under `target-shared-validate` (one transient mcp_http port flake,
green on re-run, unrelated), mutation testing 5/5 killed (suffix
dropped at either site, `bash`→`shell`, newline injected, prefix
corrupted ×2 tests). Two non-blocking nits fixed by the orchestrator
pre-merge (`b092e00`, comment-only): the new const had absorbed
`resolve_safe`'s rustdoc (doc now split — const keeps the T61 lines,
the fn keeps its own), and the test comment overclaimed leg-4's site
attribution. REQUIRED-validation ordering held: merged strictly after
T58 (same file). 5 artifacts harvested. Irony noted live: the
orchestrator's own cross-tree `edit_file` to the worktree hit the OLD
bare `path escapes cwd` error mid-arc — the last sighting of the
pre-T61 string on this host.

**T60 — docs drift pass (pri 4, DX friction, docs-only) → done
`2ce4f8c` (impl `5f6d4d8`, merge commit).** glm impl goal-accepted
17/50 ~5 min first-try, exactly two hunks: (1) `src/driver_lock.rs`'s
module comment now names the real pin — the
`chat_turn_never_creates_or_removes_the_driver_lock` unit test in
`src/driver.rs`'s test module — dropping the false "`tests/`" claim
(T55 validator finding i); (2) README's Development layout brace list
gains `archive`, `driver_lock`, `webfetch` (two of them this month's
features, previously invisible in the newcomer map) — the
orchestrator diffed the list against the actual tree: identical (20
files + `main.rs` named separately). Docs-only → kimi skipped per
T16/T31/T35/T51; orchestrator gates independently re-run 517/517 +
clippy + the spec's grep legs green. Landed under the T44 overlap as
the N+1 impl alongside T61's validator (disjoint file sets verified:
src/tools.rs vs src/driver_lock.rs + README.md), merged strictly
after T61 per the serial-merge rule. 3 artifacts harvested. Queue now
DRAINED.

**Wrap (main pushed through `f90230a`; final gates green — 517/517 +
clippy + todo_consistency 3/3 under `target-shared-main`).** Cycle
notes: (a) T44 overlap #5 executed: T60's glm impl ran alongside T61's
kimi validator with disjoint spec-named file sets (src/tools.rs vs
src/driver_lock.rs + README.md — cycle-27's README-overlap planning
catch applied at dispatch), merges strictly serial (T61 `f14f048`
before T60 `f90230a`). (b) Both cycle-27 in-flight recoveries executed
per their own rows' recipes with zero rework — T58's validator had
finished 2 min before wrap (PASS), T62's impl had self-committed 2 min
before wrap; the preserved-worktree + recovery-recipe + `-inflight-`
snapshot discipline worked exactly as designed. (c) Harvest tally: 18
artifacts across 4 items (t58 ×7 incl. superseding + removing 2
`-inflight-` partials per the T28 precedent; t62 ×3; t61 ×5; t60 ×3);
all four worktrees removed post-harvest, branches deleted. (d) Bulk
compaction executed this wrap (deferred at cycles 26+27): cycles 18–22
→ one-liners; last 6 full = 28–23; full narrative one `git log` away.
(e) Both fresh glm impls first-try goal-accepted well inside budget
(16/50 and 17/50, ~5 min each) — no 50/50 death this cycle; T58's
resume feature shipped just after the fifth occurrence of the death
class it addresses. (f) README gate: T58's delegate paragraph clause
landed in its impl commit; T60 was itself a README item; T61 needed no
README surface (the T41 sandbox paragraph already names the model —
the error string now self-documents at fire time); T62 tests-only. (g)
Human-decision carry RESTATED: restart the stale loopd supervisor (pid
90114, 5 script revisions stale — T36/T46/T47/T50/T53 dormant;
launchd KeepAlive=false so chug must never kill it) —
`launchctl kickstart -k gui/$(id -u)/com.tampajohn.chug-loopd`. (h)
Next cycle: queue EMPTY → freshness rule CANNOT fire → MANDATORY fresh
Phase-1 eval. Carries for it: the T58 validator's non-blocking
weak-test survivor (over-reset of max_iters/last_iteration at
run_start — spec req 4 sentence 2); the impl-child 50/50 death-class
demand signal (5 occurrences, work complete but uncommitted — weigh a
larger impl-child iteration default or an auto-resume-on-budget now
that T58 makes resume expressible); the loopd-restart carry above.

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

