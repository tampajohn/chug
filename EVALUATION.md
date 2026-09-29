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

### Cycle 72 (2026-09-29) — kimi fresh-eval (queue was EMPTY) — 6 rows filed + T158 mid-cycle (filed as T157; renumbered at the cycle-73 origin reconcile — the operator's MCP-write-verbs filing took the T157 id)

**T151 done c60ed3a (e9d6d72 + 5a58e4f).** One shared test-timing serialization domain (src/testsupport.rs, ONE domain by construction via fixed export symbol) + full serialization sweep of the six sighted families + dead_port ROOT fix (drain_pending_accepts — the test's own probe left a nonempty accept backlog; macOS completes connects to just-closed listeners). Recovery: impl 80/80 uncommitted → T63 resume → minutes-bound COMMITTED → orchestrator-finish; review caught the theft_or_regression single-shot misclassification (2/7 reds) → fix-up child (minutes-bound, committed) — tri-state confirm_connect + Invalidation{Theft,TeardownArtifact} + scripted-seam pins. Gates: nextest release 1136/1136; vacuousness mutant (fresh mutex per call) RED on the lock-domain pin. The acceptance matrix (8 runs) exposed the residual third family — spawn-failure-under-resource-pressure (26 red legs: loopd fixtures ×16, driver scripts ×3, mcp_serve stub ×3; ZERO in T151's closed classes) — filed mid-cycle as T158 with reproduction. Minutes-death census this era: 4 (t150, t151-resume, t151-fixup, t152-impl) — impl-minutes raise measure DECISIVELY tripped for next eval. Live catch: the t151 fix-up's bg hammer loop survived its death 1h49m (reaped manually) → T152 spec gained the cwd needle leg.
**T152 in flight:** base 81f43be (+1163: orphan-reaper.sh 248 + tests 891) committed; goal-gate red at its minutes-death (T158 flakes under double load, orchestrator-finish); spec amended post-hoc with the cwd leg (evidence above); fix-up child (glm) in flight at wrap — next cycle: check fix-up → gates → kimi REQUIRED → merge.
**T153/T154/T155/T156/T158 unworked** — specs ready; queue order per the cycle-72 handoff (T154 before T153).
**Wrap-truncated:** budget-low at 54 iters/4 min — post-merge target-shared-main gates + T151 ledger-harvest deferred to cycle 73 (T151 events already harvested into .chug/; worktrees /tmp/chug-loop-t151 + -t152 NOT removed).

### Cycle 73 (2026-09-29) — resumed mid-arc (prior segment truncated at wrap); origin reconcile + T152 landed

**Origin reconcile (b4e1d1a, pushed).** The cycle-72 wrap's rejected push resolved: origin had gained the operator's T157 filing (MCP write verbs — chug_abort + chug_steer, F10 phase 3) while the loop had filed its own T157 mid-cycle (spawn-pressure test family). Operator's filing keeps the id (shared remote, authoritative); the loop's row renumbered **T158** (row + spec mv + EVALUATION refs + T151 notes ref), todo_consistency 11/11.

**T152 done a59cf97 (81f43be + 39c8a26).** loopd pre-cycle orphan-process reaper — identity-based needle, fail-closed, SIGTERM-only, sweep at the one safe instant (post-single-driver, pre-build), LOOP_REAPER=0 opt-out. The fix-up child landed the spec-amended **cwd third leg** (`/tmp/chug-loop-t*`//tmp/chug-mut*` cwd via a PATH-independent probe, /private/tmp forms included, fail-closed on unresolvable cwd) driven by the cycle-72 live evidence (the T151 fix-up's hammer loop survived its child's death by 1h49m identified only by cwd). **kimi validator: PASS** — gates build+clippy+bash -n clean, worktree nextest 1152/1152, live verification on the real process table, **6/6 collected mutants killed** (lsof-fail-posture, non-abs-cwd, argv-needle, opt-out, SIGKILL, own-group-gate); 3 extra legs (m7-wiring, m8-commgate, m9-direxists) launched but **uncollected at a 60/60 budget death** — verdict survived in the child's ledger (T137 class: written-but-unannounced); m7-wiring killed by orchestrator read-back (loopd.sh:246 invokes the reaper behind the best-effort guard), m8/m9 noted as residual watch. **Post-merge gate finding:** `loopd_orphan_reaper` red ONLY under nextest parallelism — the three through-loopd tests (real loopd spawn → real sweep over the real process table) bust their 30s wait deadline at 17-way load; solo 3.8s green; T82 fallback serial run green **1155/1155, 0 failed** — family named in the landing commit. Load-flake filed as **T159** (bring the reaper family into T151's shared timing-lock domain). Harvests: events-t152-{impl-fixup,validate}-*.jsonl + LEDGER-t152-{children,validate}-*.md into .chug/.

**T154 done efa287a (400642f).** The mcp_serve.rs split — the era's fastest grower settled BEFORE T153 lands in it (the spec's dependency ordering). 1,168-line `#[cfg(test)] mod tests` body + 49 fns → `src/mcp_serve/tests.rs`; production keeps the T104/T109 shape; novel lines named in the commit (6-line provenance tail + the forced `include_str!("../mcp_serve.rs")` path fix — include paths are relative to the containing file). **Validation routed to the ORCHESTRATOR** (kimi skipped: mechanical move outside the REQUIRED file list; the spec's own acceptance names validator-or-orchestrator): worktree nextest **1138/1138**; **list-identity diff EMPTY** (1138 pre = 1138 post, identical sha256); **move-mutant RED** (dropping the `mod tests;` declaration → exactly the 49 moved tests vanish, `mcp_serve::` count 0). The impl child died **minutes-bound 48/80 AFTER committing with a clean tree** — the T156 committed variant (5th minutes-death this era; the raise measure is already tripped for the next eval). Post-merge serial fallback gates **1155/1155** (the reaper family's documented runner, per the T152 finding). Harvests: events-t154-impl-*.jsonl into .chug/.
