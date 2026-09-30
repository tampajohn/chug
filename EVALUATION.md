# EVALUATION — chug, assessed by chug-loop (2026-09-29, cycle 72)

**MANDATORY fresh eval** — the queue is EMPTY again (cycle 71 landed all
four remaining cycle-70 rows — T147 `6b217c3`, T148 `8cd59ac`, T149
`8699a70`, T150 `92f987a` — and its wrap commit `9c1d6bd` declared
"queue EMPTY"), so the freshness predicate's queue half fails and loopd
routed kimi per T81. The delta corpus since the cycle-70 eval text is
cycles 70–71: two orchestrator streams (kimi 169 iters/1.6M-in fresh-eval
+ glm 161 iters/395k-in routine) and 9 child streams (t144–t150
impl/validate/resume segments), read via `.chug/eval-digest.md` (358
events files, 18,914 iterations, FRESH per T131's self-exclusion check)
with targeted drills into `decisions.jsonl` (507 records) and the
harvested child ledgers. The headline: **the loop just ran its cleanest
two-cycle stretch of the adversarial era** — seven items filed and ALL
SEVEN landed within two cycles, two self-cut release tags (v0.5.0,
v0.5.1) shipped with green release workflows (verified via `gh run
list`), every budget death absorbed by the T63 family (one resume, one
committed-work orchestrator-finish), and kimi ran exactly ONE REQUIRED
round in cycle 71 (doctrine row T149) while the orchestrator's own
mutant re-runs kept the tests-only rows honest (T147 5/5, T148 3/3,
T150 vacuous-guard — ALL RED). The era's new friction is infrastructural,
not cognitive: **default-parallelism full-suite flakes under machine
load** (six fires, → T151) fed by **orphaned spinning test processes**
(a 9.8-hour 99%-CPU survivor, → T152). The mandatory roadmap pull is
**F10 phase 3a, SPLIT → T153 (`chug_cancel` MCP write leg)**. Six rows
filed: T151–T156.

## 1. What chug does well — be brief

- **The T63 recovery family absorbed 100% of the era's budget deaths**:
  t146-impl 80/80 → ONE resume → landed; t147-impl 80/80 (cycle-70
  carry) → ONE resume → 26/80 landed; t150-impl MINUTES-bound at 48/80
  with the work complete+committed → orchestrator-finish without
  burning a resume (routing d1790691515-11). Zero lost work, zero
  next-cycle recoveries. The committed-work variant is now doctrine
  (T156).
- **Validation stayed adversarially honest at LOW kimi volume**: cycle
  71 ran kimi once (T149 doctrine, PASS with 2 non-blocking
  unpinned-text observations → T155), and the orchestrator
  independently re-ran mutants on every tests-only row (T147 5/5 RED,
  T148 3/3 RED, T150 vacuous-guard RED) — the no-kimi rounds lost no
  teeth, and the verdicts were logged (d1790681871-3, d1790684835-6,
  d1790692009-12).
- **The estimate discipline held**: T150's pin now enforces
  `estimate: ~N` at filing time (mechanical, fail-closed on unreadable
  specs); the era's actuals ran 1.6–3.1x estimates (calibration §3) —
  inside the density band, and T146's ~741-line landing is exactly the
  shape the ~400 should-split band warns about (one 80/80, absorbed).
- **Release machinery is routine**: v0.5.0 + v0.5.1 self-cut at wraps
  with generated notes, lockfile regenerated, pairing verified, both
  release workflows green (`gh run list` — no failure rows needed).
- **T144's env-scrub closed the false-accept class**: zero foreign-
  worktree goal-gate executions post-merge; the predicted cost (first
  goal gates build worktree-local, ~85s vs the 600s cap) landed as
  measured and was absorbed by every post-T144 child.

## 2. Incidents worth fixing

- **I1 (→ T151, pri 2, robustness): default-parallelism `cargo test`
  full-suite flakes under machine load — SIX fires in two cycles.**
  (a) t145-impl ×2 goal-gate rejections
  (`mcp_http::tests::dead_port_probe_retry_recovers_after_scripted_theft`,
  src/mcp_http.rs:1920 — each 8/8 green isolated, a different test each
  run; T31/T59/T66 residual class); (b) t144-validate's first gate
  attempt hit the 600s check timeout under its own mutation-build load
  with ZERO failures — pure wall-clock stretch; (c) t148-impl reproduced
  `driver::tests::events::drive_loop_writes_events_jsonl`
  (src/driver/tests/events.rs:165),
  `driver::tests::hooks_policy::hook_allow_executes_tool_and_post_note_lands_in_result`
  (src/driver/tests/hooks_policy.rs:182),
  `driver::tests::permissions_policy::malformed_permissions_config_fails_open_once_and_run_continues`
  (src/driver/tests/permissions_policy.rs:425) failing on the CLEAN
  base with the tree stashed; (d) t150-impl hit 3/958 red
  (`delegate::tests::launch::delegate_launch_boundary_max_tokens_one_reaches_child`,
  src/delegate/tests/launch.rs:475 + 2 siblings) and died MINUTES-bound
  re-running gates; (e) cycle-70's T144 arc sighted
  `delegate::tests::launch::delegate_launch_stub_then_status_reports_summary_and_liveness`
  (src/delegate/tests/launch.rs:17). Root cause: T31's
  `RUN_SHELL_TIMING_LOCK` (src/tools.rs:1903) serializes only
  src/tools.rs's own 3 wall-clock tests; the driver/delegate/mcp_http
  spawn-timing families have NO shared serialization domain, and
  several legs assert absolute wall-clock bounds that machine load
  (parallel children + nextest gates + mutation builds + orphans, I2)
  stretches. Nextest-based orchestrator gates are unaffected
  (per-process isolation) — the victims are children's own
  default-parallelism `cargo test` goal gates. Fix is mechanism, not
  timeouts (T31 doctrine): ONE shared lock + convert-to-polling.
- **I2 (→ T152, pri 3, robustness): orphaned spinning test processes
  survive their worktrees for HOURS.** The cycle-70 T145 arc found a
  9.8-hour-old 99%-CPU spinning test binary from the t134-validation
  era (~590 CPU-minutes burned) and killed it mid-arc (cycle-70
  Outcomes, T145 entry). Orphan shapes: T79 mutation legs whose
  validator died mid-leg; bash-120s-cap kills whose process group
  outlived the child; removed /tmp/chug-mut-* worktrees' leftovers.
  Nothing reaps them; they ARE part of the load behind I1. The one
  safe sweep point is loopd's pre-cycle window (post-single-driver,
  pre-build — no legitimate loop process can exist there). Fail-closed
  identity-based needle; ambiguity skips, never kills.
- **I3 (CLOSED, no row): the foreign-worktree goal-gate class.** T144's
  scrub landed cycle 70; zero recurrences in any post-`8cabc79` stream
  (7 child streams verified in the digest). Watch closed.
- **I4 (→ T156, pri 4, doctrine): the complete+COMMITTED budget-death
  variant is practiced but unwritten.** T150-impl died minutes-bound at
  48/80 with 3f5a99a committed and the goal unaccepted; the
  orchestrator finished (review + gates + merge, no resume) per
  d1790691515-11, and the cycle-71 wrap ledger carried the doctrine
  sentence request. The T63 paragraph names resume (incomplete) and
  orchestrator-finish only for "complete-but-UNCOMMITTED" (T55) — a
  reader could resume a fully-committed child and burn a child budget
  re-verifying committed work.
- **I5 (→ T155, pri 4, tests): T149's two unpinned-text gaps.**
  Validator verdict d1790688874-9 (PASS, non-blocking): text-revert
  mutants on (a) LOOP-SPEC step-4's 60/40 RATIONALE text (the census +
  measure clause) and (b) META-SPEC §6's validator-template budget text
  stayed full-suite GREEN — only the bare numbers are pinned
  (tests/loop_spec_recovery.rs). Named "candidate pin-breadth rows for
  a future eval" — this is that eval.
- **I6 (noted, no row): stale `.git/index.lock` after a bash-120s
  SIGKILL mid-git (cycle 71).** One sighting; the orchestrator verified
  no live git process and removed it. Unavoidable at SIGKILL; the
  verify-then-remove recovery worked. Watch.
- **I7 (noted, no row): worktree `.git`-is-a-file commit-message write
  (t148-impl: `sh: .git/tN-msg.txt: Not a directory`).** One sighting,
  instant self-correct to `-m`. Watch; a second sighting files a
  goal-template sentence.

## 3. Friction hot spots — fix assessment

- **decision_log schema fumbles** (`options` ×4, `choice` ×2 missing):
  7 fires across 5 streams this era (orchestrators 055146/065941,
  t148-impl ×2, t150-impl, +1), every one a ≤1-iteration self-correct
  with the field named in the error, zero records lost. The
  cycle-64/70 rejection STANDS — fresh reject record filed (negative
  class kept current).
- **edit_file `old` not found / found-N-times**: ~4 fires (orchestrator
  on EVALUATION/specs ×3, t146-impl on driver.rs/main.rs ×2);
  self-correcting re-reads on 100KB+ files. Rejection stands.
- **`path escapes cwd` in validators**: zero NEW evidence this era
  (the T126 heredoc sentence is holding at noise level); the
  missed-but-cheap measure report stands. Rejection stands.
- **`.gitignore` harvest git-add fumbles**: 4 fires (one per glm
  orchestrator stream — 025718/055146/065941/cycle-71); harvests are
  intentionally untracked; self-correcting. Rejection stands (cycle-70).
- **Minutes pressure on impl children**: ONE minutes-death this era
  (t150, work committed, recipe absorbed) + t148-impl accepting at
  ~35.7m wall. Post-T144 cold worktree-local goal-gate builds (~85s) +
  I1 flake re-runs are the wall-clock sink; T151 attacks the root
  cause. An impl-minutes raise (35→40) is REJECTED at one data point
  (T21-class wants 2+ binding deaths): measure — revisit if 2 of the
  next 8 impl children die minutes-bound.
- **Estimate calibration (META-META mandatory re-check)**: T144 ~120→344
  (2.9x), T145 ~60→125 (2.1x), T146 ~300→741 (2.5x, the one 80/80),
  T147 ~150→235 (1.6x), T148 ~220→434 (2.0x), T149 ~40→17 (0.4x,
  doctrine undershoot), T150 ~70→220 (3.1x). Feature/test rows hold the
  1.5–3.1x density band; doctrine rows undershoot. The ~400/~500
  thresholds STAND (T146's ~741 actual is the band's warned shape and
  was absorbed); T150's pin now guarantees the input exists.
- **Cycle-70 orchestrator token burn**: 1.6M cumulative input tokens
  for eval + 3 items + wrap (169/200 iters, budget_low fired). The T89
  terminal-wait already collapsed child-wait churn; the residual is
  Phase-1 corpus reading + 200-iter context growth — structural, the
  T46 digest is the mitigation in place. No row.

## 4. Capability gaps — ROADMAP PULL: F10 phase 3a → T153 (pri 2)

Tier walk (top-down, Tier 1 first):

- **F13 phases 2–3 — DEFERRED (standing written reason)**: layad
  endpoint still absent. The corpus keeps deepening (507 decision
  records, incl. a growing negative class + supersession-tolerance
  note from cycle-69's I8). No change.
- **F2 phase 2b / F3 phase 2 / F4 phases 2+ / F5 phase 2 / F6 phase 2 /
  F7 phase 2 / F8 phase 2 — DEFERRED (standing written reasons)**:
  chat-only UX surfaces and ask-mode/policy variants with no loop
  consumer; each deferral re-read and re-affirmed this eval.
- **PULL: F10 phase 3 → T153, SPLIT.** Phase 2 CLOSED at cycle 69 with
  the fleet shape proven end to end (T124/T128/T129 + T148's wire e2e),
  satisfying phase 3's deferral condition; cycle-70's eval named it
  "the likely next pull once T146 lands" — T146 landed. Phase 3a (this
  row): `chug_cancel`, the fleet's missing stop button — a launched
  child is unstoppable over the wire today (cancel = ssh + kill by
  hand). Same `--allow-launch` policy boundary, ownership re-derived
  fail-closed per call (pgid==pid detached fingerprint + `chug run`
  argv), SIGTERM-group → bounded SIGKILL. Phase 3b (resources /
  notifications / server log) DEFERRED with written reason: no consumer
  pulls MCP-spec-completeness surfaces; the fleet's demonstrated
  consumers (Claude Code config, the bridge-fleet shape) use tools,
  not resources; a server log is observability the events stream
  already serves.
- **F11 / F12**: below F10-p3 in Tier 3 order; unworked, unremarked —
  next eval re-walks after T153 lands.
- **New finds beyond the roadmap**: none this eval — the era's finds
  were infrastructure-class (I1/I2), not capability-class.

## 5. Top 3 priorities

1. **T151** (pri 2, robustness) — the flake family is the loop's top
   budget-eater: two goal-gate rejections, one minutes-death, and one
   600s stretch in TWO cycles; mechanism fix per T31 doctrine.
2. **T153** (pri 2, feature) — the mandatory roadmap pull; completes
   the fleet's launch/observe/stop triangle behind one policy flag.
3. **T152** (pri 3, robustness) — cheap, fail-closed, and removes a
   root-cause contributor to I1's load; one 9.8-hour orphan is one too
   many.

## 6. README audit (usability)

Cold read, top to bottom (825 lines, +32 since cycle 70). **(a) Reading
order**: sound — what-it-is → Install → Quickstart → chat → run → forks
→ plan → TUI → Tools → risk gate → hooks → permissions → MCP (client +
server) → observability → specs → loopd → Development; the T146
`--approve` bullet landed inside the run-mode list in its correct
policy position, not appended. **(b) Redundancy**: the loopd section
still re-states role-keyed-dir rationale the T47/T52/T57 specs own —
deliberate (operator runbook), FOURTH consecutive balance note, still
below the row-filing bar. **(c) Staleness**: none found — plan mode's
six-tool contract with web_fetch (T146), the T143 max-tokens knobs,
T137's build gate, T142's rc-verdict rule, T129's `--allow-launch`
contract with the T148 wire-e2e paragraph, and the phase-3 deferral
line all present and current. **(d) Balance**: covered in (b).
**(e) Quickstart truth**: install → auth chain → first run works as
written (unchanged since T127; the SPEC-6 chain matches observed auth
behavior). **No docs row filed — fourth consecutive clean audit.**

## Handoff

- **Work order** (with reasons): **T151** (pri 2 robustness; touches
  src/driver/tests/*, src/delegate/tests/*, src/mcp_http.rs +
  src/tools.rs test modules + one new test-support module — SERIAL
  with nothing else in the queue; kimi routing: skip-leaning per the
  tests-only precedent, orchestrator re-runs the vacuousness legs) →
  **T152** (pri 3 robustness; loopd.sh + tests/loopd_*.rs — DISJOINT
  from T151's file set, MAY overlap one validator window; kimi
  REQUIRED: fail-closed process-killing shell is safety-adjacent, the
  T135/T137 loopd precedent) → **T154** (pri 3 mechanical move;
  src/mcp_serve.rs only; lands BEFORE T153 by dependency — the split
  settles the layout the feature diffs against; kimi optional, leaning
  skip: run-set identity + one move-mutant orchestrator-side) →
  **T153** (pri 2 feature; src/mcp_serve.rs + tests/mcp_serve.rs +
  README — SERIAL behind T154, shared file; kimi REQUIRED: new
  write-leg policy surface + process signaling) → **T156** (pri 4
  DOCTRINE — runs ALONE, no overlap ever; kimi REQUIRED) → **T155**
  (pri 4 tests-only pins; disjoint from everything; kimi skip,
  orchestrator RED-proofs).
- **Bundle check (T45 conjunctive)**: T151 (~260), T152 (~160), T153
  (~380), T155 (~90) all fail (a) ≤30 → no bundles containing them.
  T156 (~40) alone is near the line but doctrine rows are worked with
  kimi REQUIRED regardless — a bundle buys nothing. NO bundles.
- **Overlap check (T44)**: T151 (src test modules) ∥ T152's validator
  window — disjoint, allowed. T153/T154 share src/mcp_serve.rs →
  serial. T156 never overlaps (doctrine). T155 (tests/loop_spec_*.rs)
  disjoint from T151/T152 → flexible. Realistic ceiling this cycle:
  4–6 arcs.
- **Expected kimi routing at work time** (logged as they happen):
  T151 optional/skip (tests-only; T147/T148/T150 precedent —
  orchestrator re-runs the lock-domain vacuousness mutant + a
  flake-family spot-check); T152 REQUIRED (fail-closed killing);
  T154 optional/skip (mechanical move, list-identity + move-mutant
  orchestrator-side); T153 REQUIRED (src/mcp_serve.rs production +
  new signal surface); T156 REQUIRED (doctrine); T155 skip
  (tests-only).
- **SELF-SPEC**: none. **Human items** (carried): (1) `gh auth refresh
  -s workflow` remains the proper fix for the workflow-scope class
  (SSH pushurl workaround holding — release workflows green via the
  workaround); (2) `com.tampajohn.chug-loopd.plist` stays untracked
  (operator's launchd unit, carried since cycle 61).
- **Watch items carried**: loopd orchestrator cap (era max 169/200 —
  healthy; re-open at ≥180 or an iteration death); impl-minutes
  pressure (§3 measure: 2 of next 8 minutes-bound → revisit 35→40);
  T149's validator-budget census (>1 of next 8 validators dying at
  60/40 unannounced → trim default mutation-leg counts; 0 of 1 so
  far — only T149's own validator has run at 60/40); module sizes
  (api.rs 3,242 +34, tools.rs 3,005, mcp_http.rs 2,800, tgrep.rs
  2,638, tui.rs 2,527, mcp_serve.rs 1,949 — T154/T153 queued in it,
  re-census next eval; driver.rs 1,911 +89 post-T146); stream_fallback
  (zero organic post-T141/T143 — second era running); stale
  index.lock (I6) + .git-gitfile writes (I7) — one sighting each,
  file on second; digest "remaining_iters=" label on minutes/token
  budget-low legs is terse (cosmetic, rejected).
- **Weighed and REJECTED this eval** (each with an eval-triage record
  in `.chug/decisions.jsonl`): decision_log schema-fumble row (§3 —
  7 fires, ≤1-iter self-corrects, zero losses); edit_file
  anchor-drift row (§3 — self-correcting); /tmp-sandbox boundary move
  (§3 — zero new evidence, noise-level holding); .gitignore harvest
  git-add fumbles (§3 — self-correcting, intentionally untracked);
  stale index.lock row (I6 — one sighting, recovery worked); worktree
  .git-gitfile goal-template sentence (I7 — one sighting); impl-minutes
  35→40 raise (§3 — one binding death, T151 attacks the root sink);
  mcp_http.rs/tgrep.rs/tui.rs splits (stable, no feature landings
  queued — watch); api.rs 3,242 split (stable growth, +34/era — watch);
  F10 phase 3b (deferred in the SPLIT — no consumer pulling); F13
  phases 2–3 (standing dependency: layad endpoint absent); F2-2b /
  F3-p2 / F4-p2+ / F5-p2 / F6-p2 / F7-p2 / F8-p2 (standing deferrals
  re-affirmed); F11/F12 (Tier-3 order behind the F10-p3a pull);
  digest budget-low label terseness (cosmetic).

## Outcomes (filled at cycle wrap — LOOP-SPEC Phase 3)


### Cycle 67 (2026-09-28/29) — routine (freshness-skip) — codex-intake pri-2 queue — T135 LANDED 611916e (merge + stale-flip cleanup), T136 arc mid-flight at wrap (recovered + landed cycle 68; its crash-safety class completed by T145 in cycle 70) — compacted at the cycle-73 wrap (last-6 rule; the full text lived in git until the cycle-72 eval commit dropped it — restored verbatim for 69/70/71 below, 67/66 re-compacted from git).

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

### Cycle 66 (2026-09-28) — routine (reconciled cycle-65 divergence first) — codex-intake queue T134–T142 — T140 9b36a2e (goal_complete denial bypass), T141 70ec4a7 (SSE truncation acceptance, rebased), T142 3bc3169 + fix-up dd184d4 (loopd grep spoofing), T134 d5d9c28 + fix-up bfd316a (symlink sandbox escape + doctrine drift) — compacted at the cycle-73 wrap (last-6 rule); full narrative in git.

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

### Cycle 72 (2026-09-29) — kimi fresh-eval (queue was EMPTY) — 6 rows filed + T158 mid-cycle (filed as T157; renumbered at the cycle-73 origin reconcile — the operator's MCP-write-verbs filing took the T157 id)

**T151 done c60ed3a (e9d6d72 + 5a58e4f).** One shared test-timing serialization domain (src/testsupport.rs, ONE domain by construction via fixed export symbol) + full serialization sweep of the six sighted families + dead_port ROOT fix (drain_pending_accepts — the test's own probe left a nonempty accept backlog; macOS completes connects to just-closed listeners). Recovery: impl 80/80 uncommitted → T63 resume → minutes-bound COMMITTED → orchestrator-finish; review caught the theft_or_regression single-shot misclassification (2/7 reds) → fix-up child (minutes-bound, committed) — tri-state confirm_connect + Invalidation{Theft,TeardownArtifact} + scripted-seam pins. Gates: nextest release 1136/1136; vacuousness mutant (fresh mutex per call) RED on the lock-domain pin. The acceptance matrix (8 runs) exposed the residual third family — spawn-failure-under-resource-pressure (26 red legs: loopd fixtures ×16, driver scripts ×3, mcp_serve stub ×3; ZERO in T151's closed classes) — filed mid-cycle as T158 with reproduction. Minutes-death census this era: 4 (t150, t151-resume, t151-fixup, t152-impl) — impl-minutes raise measure DECISIVELY tripped for next eval. Live catch: the t151 fix-up's bg hammer loop survived its death 1h49m (reaped manually) → T152 spec gained the cwd needle leg.
**T152 in flight:** base 81f43be (+1163: orphan-reaper.sh 248 + tests 891) committed; goal-gate red at its minutes-death (T158 flakes under double load, orchestrator-finish); spec amended post-hoc with the cwd leg (evidence above); fix-up child (glm) in flight at wrap — next cycle: check fix-up → gates → kimi REQUIRED → merge.
**T153/T154/T155/T156/T158 unworked** — specs ready; queue order per the cycle-72 handoff (T154 before T153).
**Wrap-truncated:** budget-low at 54 iters/4 min — post-merge target-shared-main gates + T151 ledger-harvest deferred to cycle 73 (T151 events already harvested into .chug/; worktrees /tmp/chug-loop-t151 + -t152 NOT removed).

### Cycle 73 (2026-09-29) — resumed mid-arc (prior segment truncated at wrap); origin reconcile + T152 landed

**Origin reconcile (b4e1d1a, pushed).** The cycle-72 wrap's rejected push resolved: origin had gained the operator's T157 filing (MCP write verbs — chug_abort + chug_steer, F10 phase 3) while the loop had filed its own T157 mid-cycle (spawn-pressure test family). Operator's filing keeps the id (shared remote, authoritative); the loop's row renumbered **T158** (row + spec mv + EVALUATION refs + T151 notes ref), todo_consistency 11/11.

**T152 done a59cf97 (81f43be + 39c8a26).** loopd pre-cycle orphan-process reaper — identity-based needle, fail-closed, SIGTERM-only, sweep at the one safe instant (post-single-driver, pre-build), LOOP_REAPER=0 opt-out. The fix-up child landed the spec-amended **cwd third leg** (`/tmp/chug-loop-t*`//tmp/chug-mut*` cwd via a PATH-independent probe, /private/tmp forms included, fail-closed on unresolvable cwd) driven by the cycle-72 live evidence (the T151 fix-up's hammer loop survived its child's death by 1h49m identified only by cwd). **kimi validator: PASS** — gates build+clippy+bash -n clean, worktree nextest 1152/1152, live verification on the real process table, **6/6 collected mutants killed** (lsof-fail-posture, non-abs-cwd, argv-needle, opt-out, SIGKILL, own-group-gate); 3 extra legs (m7-wiring, m8-commgate, m9-direxists) launched but **uncollected at a 60/60 budget death** — verdict survived in the child's ledger (T137 class: written-but-unannounced); m7-wiring killed by orchestrator read-back (loopd.sh:246 invokes the reaper behind the best-effort guard), m8/m9 noted as residual watch. **Post-merge gate finding:** `loopd_orphan_reaper` red ONLY under nextest parallelism — the three through-loopd tests (real loopd spawn → real sweep over the real process table) bust their 30s wait deadline at 17-way load; solo 3.8s green; T82 fallback serial run green **1155/1155, 0 failed** — family named in the landing commit. Load-flake filed as **T159** (bring the reaper family into T151's shared timing-lock domain). Harvests: events-t152-{impl-fixup,validate}-*.jsonl + LEDGER-t152-{children,validate}-*.md into .chug/.

**T154 done efa287a (400642f).** The mcp_serve.rs split — the era's fastest grower settled BEFORE T153 lands in it (the spec's dependency ordering). 1,168-line `#[cfg(test)] mod tests` body + 49 fns → `src/mcp_serve/tests.rs`; production keeps the T104/T109 shape; novel lines named in the commit (6-line provenance tail + the forced `include_str!("../mcp_serve.rs")` path fix — include paths are relative to the containing file). **Validation routed to the ORCHESTRATOR** (kimi skipped: mechanical move outside the REQUIRED file list; the spec's own acceptance names validator-or-orchestrator): worktree nextest **1138/1138**; **list-identity diff EMPTY** (1138 pre = 1138 post, identical sha256); **move-mutant RED** (dropping the `mod tests;` declaration → exactly the 49 moved tests vanish, `mcp_serve::` count 0). The impl child died **minutes-bound 48/80 AFTER committing with a clean tree** — the T156 committed variant (5th minutes-death this era; the raise measure is already tripped for the next eval). Post-merge serial fallback gates **1155/1155** (the reaper family's documented runner, per the T152 finding). Harvests: events-t154-impl-*.jsonl into .chug/.

**T153 MID-ARC at the earlier segment's wrap (driver wall) — since RESOLVED; the worktree resumed this same day and the row landed (see the T153 done 675076d entry below).** chug_cancel (the second flag-gated MCP write leg) is fully implemented and COMMITTED on `loop-t153` in `/tmp/chug-loop-t153` (3ae17a1, +1197/−23: schema + advertised⇔callable under --allow-launch, fail-closed ownership re-derivation per call, TERM-group → 5s grace → SIGKILL, 12 unit tests + 3 wire legs, README/FEATURES/main.rs) plus ae0a684 (the spec's `check:` line now embeds the T47 shared cache — the goal-gate check had run env-starved, cold-built into `./target`, and hit the 600s wrapper: an infrastructure kill, not a test failure). Child ledger gates: build + clippy green; per-binary green; two full `cargo test` runs green. Two load-flakes observed at the edges (both documented, neither a product red): the new `chug_cancel_happy_path_over_the_real_wire` dead-poll leg failed ONCE under nextest 17-way (passes solo 1.13s — T151 family) and the pre-existing `delegate_collect_mid_run_reports_running` 5s wall assert (passes solo 0.93s). Known risk carried in the child ledger: the full suite is ~800–1000s warm vs the 600s check wall — remedy candidate for the next eval. Impl arc burned TWO budget deaths (80/80 + the resumed leg at 35:00 mid-check); T63's one-resume cap reached → orchestrator-finish attempted, superseded by the wall. Harvests: events-t153-impl-resume-*.jsonl + transcript-t153-impl-*.jsonl in .chug/.

**T153 done 675076d (3ae17a1 + ae0a684 + 6e92f84, cycle 73 resumed).** chug_cancel — the second flag-gated MCP write leg (F10 phase 3a): advertised ⇔ callable under `--allow-launch`, stateless fail-closed ownership re-derivation (alive → pgid==pid → ps argv names `chug run`), TERM to the whole process group → 5s grace → one SIGKILL escalation, payload `{pid, signaled, waited_ms}`; 12 unit legs + 3 wire legs on the T148 harness. **The resume arc's forensics found a REAL product bug, not load:** the cancel dead-poll failure reproduced SOLO 12/30 and the instrumented tick caught `kill(-pgid,0)` answering **-1/EPERM** over a group whose sole member was the server's own unreaped zombie child (macOS killpg-on-zombie; per-pid `kill(zombie,0)` returns 0 — the group and pid probes disagree) — `group_gone`'s `rc != 0` read EPERM as "group empty" and returned `signaled: term` with the leader an unreaped zombie. Fix-up 6e92f84: `group_gone_rc` seam reads ONLY ESRCH as empty (rc==0 and every other rc/errno mean not-gone; the next tick's waitpid reaps the zombie and the probe turns ESRCH — deterministic convergence), predicate-table + real-zombie + convergence RED proofs, wire happy path 15× solo green (was 12/30), and a kill(2) class sweep in the commit message (`process_alive` adjudicated already-compliant EPERM→alive, pinned empirically via `kill(1,0)`=EPERM on live root launchd; driver_lock/loopd.sh/reaper/delivery-rc sites justified). **kimi REQUIRED PASS** (routing d1790721266-2, verdict d1790722238-3): clean gates nextest 1170/1170 run independently in the role-keyed validate cache, 7/7 spec requirements verified, T79 parallel mutation round 9 mutants — 7 RED (incl. M4 re-proving the EPERM quirk live), **2 weak-test survivors carried as T160** (M5: needle table lacks a `--special` near-miss row; M6: the escalation fixture leg asserts the payload but never proves the group emptied — FixtureGuard cleanup masks a no-SIGKILL mutant), 2 observations (needle-without-`chug`-token is by design and documented). The validator's goal-gate rejected on the spec check line vs the 600s wrapper wall — the row's documented KNOWN RISK (warm full debug suite ~800–1000s); verdict accepted per the T137 written-before-rejection class, and the check-wall remedy stays a next-eval candidate. Worktree + post-merge gates 1170/1170 (target-shared-main). Harvests: events-t153-fixup/validate + LEDGER-t153-fixup/validate into .chug/.

**T155 done 00d4f7e (6780191 rebased, cycle 73).** The two T149 kimi non-blocking unpinned-text gaps closed: `tests/loop_spec_recovery.rs` +248 — (p) step-4's budget RATIONALE tokens (the 4-of-the-last-4 census clause, the >1-of-8 measure tripwire, the trim-mutation-leg-counts remedy) each pinned EXACTLY once inside step 4's T64 window, AFTER the 60/40 numbers pair; (q) META-SPEC §6's validator argv `--max-iters 60 --max-minutes 40` pinned as a contiguous exactly-once fragment. glm impl child died minutes-bound 49/80 AFTER committing (the T150 committed-variant class, 6th minutes-death this era) — orchestrator-finish per routing d1790723435-6: no resume, review + gates + merge. kimi SKIPPED per the tests-only T16/T31/T130/T131 precedent with the orchestrator RED-proving both text-revert mutants personally (census 4→9 → pin panicked at the count leg; argv 40→30 → pin panicked at the fragment leg; pristine restore 17/17). Worktree (rebased onto the T153 merge) + post-merge nextest 1172/1172.

**T156 done (merge of loop-t156, cycle 73).** The T63 doctrine now names the complete+committed budget-death variant — the routing discriminator sentence sits in LOOP-SPEC §2 step 2 right after the resume-first rule: work INCOMPLETE takes the ONE resume; work complete and committed with the goal unaccepted goes to orchestrator-finish directly (no resume burned; T150-impl named alongside T55); resume-exhausted goes to next-cycle with a recipe (T28). Pins (r)(s)(t) in tests/loop_spec_recovery.rs (+227): 9 routing tokens exactly-once inside step 2's window, byte-exact adjacency/ordering chain, standing-recipes byte-pin. The doctrine row validated its own thesis in practice: the impl child died minutes-bound 47/80 AFTER committing (4th consecutive committed-variant — T150, T154, T155, T156) and the orchestrator-finish routing applied cleanly. kimi REQUIRED PASS (routing d1790726950-9, verdict d1790729228-10): all 3 spec requirements verified, own gates 1175/1175, serial mutants budget-truncated with RED corroborated by pre-edit zero-occurrence analysis and the commit's documented M0–M4 matrix (restore sha independently verified) — plus the orchestrator's own sentence-revert RED proof (both routing pins died; pristine 20/20). Gates wt+main 1175/1175.

**Cycle-73 wrap notes (2026-09-29/30, resumed mid-arc → full wrap).** FIVE rows landed: T152 (a59cf97, prior segment), T154 (efa287a), T153 (675076d — the feature + the forensics-driven EPERM/zombie fix-up), T155 (00d4f7e), T156 (merge of loop-t156) — plus the b4e1d1a + df042ad origin reconciles (operator filings T157/T161 kept their ids). **Release: v0.6.0 tagged 622cc79** (5 items since v0.5.1 + the F10 phase-3a check-off; minor bump; notes generated; check-tag-version ok; commit+tag pushed together). Validators: kimi REQUIRED ×2 (T153 PASS 7/9 mutants, 2 weak-test survivors → T160 filed; T156 PASS, mutants budget-truncated + RED corroborated), kimi SKIPPED ×1 tests-only (T155, orchestrator RED-proofs both pins), orchestrator-verified ×2 (T152 prior segment, T154 list-identity). **Minutes-death census now 6-8 this era — ALL committed-variant** (T150, T151-resume+fixup, T152, T154, T155, T156 + T153's arc): the T63 three-way routing (T156, landed this cycle) named the variant and the orchestrator-finish path executed cleanly four times in one cycle. T63 resume used ONCE (T153's mid-arc). Queue for cycle 74: T157 (operator MCP write verbs — chug_abort + chug_steer, F10 phase 3), T158 (spawn-pressure family), T159 (reaper timing-lock), T160 (T153 weak pins), T161 (operator two-impl overlap lever) — all with ready specs. **Bookkeeping incident found + fixed at this wrap:** the cycle-72 eval commit (1206d93) dropped the Cycle 69/70/71 Outcomes sections that existed at the cycle-71 wrap (regenerated file, lost carry-over) — restored verbatim from 9c1d6bd:EVALUATION.md; 67/66 re-compacted to one-liners (outside the last-6 window); the eval-commit template must carry forward ALL existing Outcomes sections (next-eval candidate). Final gates at HEAD: build + clippy + nextest 1175/1175 (target-shared-main).
