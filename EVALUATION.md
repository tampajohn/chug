# EVALUATION — chug, assessed by chug-loop (2026-10-04, cycle 103)

MANDATORY fresh eval — the freshness predicate failed at launch on the
todo-rows half (queue DRAINED at the cycle-102 wrap: 0 `todo` rows;
EVALUATION.md's mtime was same-day from the cycle-102 wrap-notes append,
but the conjunction needs both), so loopd routed to kimi per T81 — the
routing decision has been correct every cycle of the delta and for this
launch. Delta corpus since the cycle-98 eval: cycles 98 (work phase), 99,
100, 101, 102 — five orchestrator streams (kimi ×1: 184i/5h22m accepted,
trim fires 2; glm ×4: 157i/4h31m, 133i/3h25m, 158i/5h26m, 63i/2h3m — all
accepted) plus 20 child streams (the t212–t224 arcs: 13 impl runs, 1
fix-up, 8 validator runs) — read via the fresh digest (537 files, 33,106
iterations, FRESH per the mechanical check) with drills into the t219-impl
stream (the launch-test flake), the t212 spec's CORRECTED PREMISE section,
`.chug/decisions.jsonl` (967 records, +64 in the delta — zero zero-record
cycles), and the full git record (`f4c96b5..a38ff48`, ~35 commits incl.
the v0.16.2 and v0.17.0 releases). Headline: **all twelve queued rows
landed** (T212–T224, including two operator filings worked mid-cycle and
the T220 dispatch-time split), the loop self-tagged twice, and the queue
is EMPTY for the second time in three evals — while the cost center
shifted decisively to two places: the recovery tax (9 of 15 impl-side
child runs died at a budget, every one absorbed by T63/T55 with zero work
lost) and contention false-reds (the delegate launch-test flake family +
the T214 fence's blind variable, filed as T225). The loop's own
measurement machinery also delivered its first real verdict: the T223
judge-parity bake-off ran end-to-end and returned LAYA STAYS with numbers
(goldens parity max-abs-dp 0.00e+00, holdout n=42 AUROC 0.374) — the F13
roadmap question now has TWO independent measurements saying not-yet.

## 1. What chug does well

- **Recovery doctrine absorbed a 60% child-death rate with zero work
  lost.** 9 of 15 impl-side runs in the delta died at a budget (t215 ×2,
  t219-r1, t222 ×2, t223-r1, t223-fixup, t213, t214 — iteration-bound on
  novel-infrastructure rows, minutes-bound under host load); every one
  routed correctly: T63 resume when uncommitted (t214, t219, t223),
  T55/T150 orchestrator-finish when complete-committed (t215, t213,
  t222-r2, t223-fixup). One resume (t215) died at the SECOND budget with
  the goal gate VERIFYING and the doctrine's cap (one resume per child)
  held.
- **The validator net keeps catching what review misses — including a
  20 GB process leak.** T223 round-1 FAIL (the serve-leg daemons leaked
  via subshell PID bookkeeping: 6/6 orphans at 2–3.4 GB RSS, ~20 GB
  reclaimed; the fix-up swept the CLASS — parent-shell booking, traps,
  fail-closed routing, early-death bail) is the delta's best catch. T212
  round-1 FAIL caught a false-premise fix before merge (I6). T219's
  7-mutant sweep found the day-0 underflow + two pin-strength gaps. Zero
  defective merges in the delta.
- **The dispatch-time spec-size gate (T209) caught its first live
  oversized row.** Cycle 100 re-read T220 at dispatch (~450L filing over
  a T204-class true surface), split it by acceptance surface into
  T222+T223 with the premise defect written into both specs — the gate
  working exactly as designed, filing-time honesty not required for
  enforcement.
- **The loop's measurement machinery produced its first honest
  verdict.** T223's bake-off: fail-closed idempotent runner, real /judge
  wire, both option orderings, three named gaps, no fabricated
  comparison — and the classified kev refusal (T222) proved the SPEC-3
  honesty surface end-to-end (no architecture substituted when the
  premise broke mid-flight).
- **Decision-record discipline is background noise now** (the goal):
  967 records, +64 across 5 cycles, including the negative class
  (eval-triage rejects, model/recovery routing, outcome backfills).
- **Verdict-file doctrine absorbed the one validator death** (t215-val
  died 60/60 with VERDICT written; the T209 validator measure clause's
  UNANNOUNCED letter not tripped), and cycle-101's narrowed re-validation
  ran 24/25 on a trimmed budget — budget-tailoring to scope working.

## 2. Incidents worth fixing

### I1 — T223 serve-leg daemon leak: ~20 GB of orphaned judges (caught pre-merge, fixed)
Evidence: cycle-101 Outcomes (merge 1231a89) — `serve_leg` ran inside
command substitution, so its PID_LIST side effect died with the subshell
and EVERY run leaked its model-loaded daemons (6/6 orphans, 2.0–3.4 GB
RSS each; the orchestrator reaped ~20 GB accumulated before the fix-up).
The kimi round-1 validator proved it black-box; the fix-up (86a6b06)
swept the class: both start sites book pids in the parent shell,
EXIT/INT/TERM traps reap, unknown-checkpoint routing fails closed,
wait_healthy bails on early death. Root cause class: shell pid
bookkeeping that dies in a subshell — the runner is a committed artifact
(scripts/judge-parity.sh) that future eval-heavy rows will copy.
Candidate fix: LANDED (86a6b06 + kimi round-2 PASS). No row. Watch-item:
the T152 orphan reaper's three legs cover test/build processes and
removed-worktree cwds — a leaked daemon from a LIVE worktree rides until
worktree removal; acceptable (the reaper is a cycle-start net, not a
mid-arc one).

### I2 — Goal-gate timeout on the loop's own check line (fix LANDED in-cycle, 7f94da2)
Evidence: cycle-100's wrap goal gate rejected on a TIMEOUT, not a red
test: the check line's plain `cargo test` (debug) against
target-shared-main recompiled ~40 stale debug test binaries in 19m04s
and blew the 1200s cap mid-daemon_lifecycle (everything that ran was
green: 1216+2+1+4/5). The cycle-98 eval's I2 fixed the cold-TARGET leg
(67e11fc); this was the cold-PROFILE leg on the same surface — the
check line now carries the T82 runner verbatim (warm release nextest
~36s) and nextest_gate_runner pins it as the 4th carrier. Two bites of
the same class in four cycles; both fixed forward. No row. Lesson for
spec authors (already doctrine): a check line without the shared-cache
export + release-runner discipline is a landmine on any version bump.

### I3 — Child budget deaths at 60% of impl runs; the mix is now estimate-honesty, not size-cap
Evidence (digest): t215 ×2 (iterations 80/80 → resume died minutes with
the gate VERIFYING), t219-r1 (80/80 mid-flake-debug), t222 ×2 (80/80
mid-premise-discovery, 80/80 post-commit), t223-r1 (80/80, doc
unwritten), t223-fixup (50-min wall, 58/80), t213 (50-min, 65/80
post-commit), t214 (50-min, 61/80 uncommitted). The T209 dispatch gate
holds (it caught T220), but T219 (filed ~450 → landed 1,216) and T222
(filed ~400 → landed 1,803) show the gate reads the ESTIMATE, and
novel-infrastructure estimates undershoot 2.7–4.5× (§5's calibration).
The deaths are absorbed (5–15 orchestrator iterations per finish) — the
tax is real but bounded, and the recoveries are now routine. Candidate
fix: NONE FILED — the calibration text is the standing remedy and the
gate enforces the ceiling. MEASURE: if the next eval's census shows ≥2
iteration-bound deaths on rows filed ≥300 all-in, the eval after
considers a dispatch-time RE-ESTIMATE duty (the orchestrator re-derives
the estimate from the spec's requirements list, not just re-reads the
estimate line).

### I4 — Contention false-reds: the delegate launch-test flake + T214's blind variable (→ T225)
Evidence: (a) t219-impl run-1 died 80/80 mid-debug after **10 delegate
launch bin-tests flaked** under concurrent loopd load — the child
A/B-proved its diff not-causal (stash/re-run; the family passes at
`--test-threads=1`; goal summary in
`.chug/events-t219-impl-20261004-064230.jsonl`). A full 80-iteration
child burned on a test-infrastructure false-red. (b) Cycle-101:
`loopd_orphan_reaper::a_failing_driver_probe_means_no_sweep` expired its
T214 load-scaled fence IDENTICALLY on main under ambient load 9.68,
then passed at post-merge. Root cause: T214 scales by
`clamp(loadavg_1m/cores, 1, 4)` and K7 has **18 cores** — 9.68 reads
0.54/core → factor 1.0 → no scaling, while suite fan-out (T152 measured
30.8s at 17-way vs 3.36s solo) stretches spawn walls ~9×. The per-core
load variable is blind to the suite's own fan-out on many-core hosts,
and the delegate bin-test fences (src/delegate/tests/, ~30 bare
elapsed<15s/30s asserts over real stub children) never got any scaling.
The principled fix: a fence whose child is making progress should trip
on NO-progress, not on wall-clock — progress-reset liveness fences with
the load-scaled wall as the outer backstop. Filed as **T225** (pri 2,
robustness).

### I5 — Piped gate chain swallowed a red todo_consistency; red main pushed (→ T227)
Evidence: c06a555 went out with T220/T223 notes cells carrying literal
`|` characters (rows split into 8 cells — the T37/T8 class); the guard
was red and the orchestrator's chain reported the filter's exit 0
(fixed forward 6365b50, root cause named in the commit; red live on
origin ~3 min). The spec-`check:` pipe lint (T67→T164) covers spec
check lines; the orchestrator's ad-hoc gate chains have no carrier.
Blast radius tiny, recurrence count two (cycle-16's goal-gate death was
the same notes-cell class through a different gate). Filed as **T227**
(pri 4, doctrine, SOLO).

### I6 — The cycle-98 eval filed T212 on a false indictment (→ T228)
Evidence: specs/t212-delegate-drift-worktree-read.md's "CORRECTED
PREMISE" section — the eval asserted the T197 drift WARNs were FALSE
positives from secondhand Outcomes text; they were TRUE (the goal gate
reads the spec ARG path, driver.rs:1171; branch re-keys never reached
it). Round 1 (874c620) built the filing's premise, kimi FAILed it
three ways, full sha256-identical revert, row re-scoped. Cost: one impl
round (70/80) + one validation round (48/60) + the revert. The filing
bar's "verify T1/T2/T4/T5 actually worked" clause covers fix-adjacency,
not fresh indictments. Filed as **T228** (pri 3, doctrine, SOLO).

### I7 — t215's validator died 60/60 with the verdict written (absorbed; watch-item)
Evidence: `.chug/events-t215-validate-20261004-000130.jsonl` (60/60,
abort "iteration budget exceeded", verdict file present — the
verdict-file doctrine absorbed it; PASS d1791072243-18). The T209
validator measure clause trips on UNANNOUNCED verdict deaths — this was
announced-to-file, so not tripped; 1 death in the delta's 8 validator
runs. Watch-item only: the validator's 9/9 mutant sweep on a 549-line
test family is a big serial surface; if the next census shows ≥2
validator deaths (any announcement state), the eval trims default
mutation-leg counts per the clause.

## 3. Friction hot spots

- **The recovery tax is the delta's dominant steady-state cost** — 9
  child deaths in 15 impl-side runs, each costing 5–15 orchestrator
  iterations plus re-review and (twice) a second validation round. All
  absorbed, zero work lost, but it is why cycles run 3.5–5.5 h. The
  mitigations are the T209 gate (working — T220) and §5's calibration
  refinement; the residual is host load (I3/I4).
- **Host contention profile changed interpretation**: K7 is 18 cores /
  128 GB, and ambient load ~9.7 (fseventsd + EXO pegged) reads
  0.54/core — QUIET to per-core scaling while the suite's own fan-out
  does the damage. The T82 family-isolation tax (~2–5 min per gate) was
  paid repeatedly in cycles 99/101 (T213's model_routing fixture break,
  the cycle-101 fence flake). T225 is the filed fix.
- **Trim fires are now routine telemetry** (cycle-98 kimi orchestrator
  ×2, t219-impl ×1, t222-impl ×2, t223-impl ×1) — T77/T184 working as
  designed. Counter-observation: **ctx-edit has ZERO fires across all
  537 events files** — T192's LIVE_CTX/shrink-gate surface (landed
  cycle 89) has never fired in production; trim engages first, every
  time. Dormant ≠ broken (it costs nothing unfired), but if the next
  eval still reads zero, file a trigger-calibration measurement row —
  the surface's value claim is measured-by-telemetry per its own
  doctrine.
- **glm edit_file/schema fumbles flat** (~1–2 per fumble-prone child:
  t215 "old found N times" ×1, t222 "old not found" ×1, t223-fixup
  /tmp-confinement ×2) — T88/T94 correctives keep converting fumbles to
  same-iteration self-repairs. The alias-acceptance rejection STANDS
  (the cycle-98 re-weigh condition — rising counts — is not met;
  negative triage re-recorded this eval).
- **Orchestrator goal-gate rejections**: 3 in the delta (cycle-98
  seg-eval ×2 on the cold-target class, cycle-100 ×1 → I2's fix) plus
  cycle-101's mid-wrap "check command failed" (the c06a555 red the
  piped chain later swallowed — I5). Post-7f94da2 the gate is warm
  (~36 s nextest release); a rejection now costs a suite run, not a
  compile.

## 4. Capability gaps — ROADMAP PULL

**Pull SKIPPED with written reason — the top unworked item's GO
precondition is measured, quantified, and now corroborated by a second
independent measurement.**

Top unworked item by tier order: **F13 phase 3** (confidence-gated
first-pass routing: Laya decides ≥τ, else escalate to kimi). Two
measurements say not-yet: (1) T208 (cycle 95): frozen-encoder probe
TIES majority (79.3%), fine-tune wobbles 65.5–82.8%, no ≥95%-accuracy
operating point at ≥50% coverage in any wobble state; GO precondition
~3× the 858-record corpus plus held-out n≥60 per task. (2) T223 (cycle
101, NEW this delta): the live holdout — the served laya-judge on the
F13 time-ordered 42-record outcome panel — returned choice top-1 0.5714
and noul AUROC **0.374, below coin-flip** (deep-OOD; docs/judge-parity.md
records it honestly with SPEC-3 standing: no gating signal change).
Corpus now: **967 records** (+64 in 5 cycles ≈ 13/cycle) vs the ~2,570
precondition — ~120 cycles out at the observed accrual. Pulling phase 3
now would wire a router TWO of its own measurements reject. The F13
fine-tune (T223's named follow-up) inherits the same precondition — a
42-record panel is not a fine-tune corpus. F15 phases-2+ and all
chat-side phase-2 deferrals stand on their written reasons.

**New find — APPENDED to FEATURES.md as F16, with a written NOT-YET.**
The T222/T223 arc left exactly one resolvable contestant unserved:
kev-0.8b sits on Qwen3.5-0.8B-Base (qwen3_5, 18 Gated DeltaNet + 6 full
attention) and candle-transformers 0.11 has no qwen3_5. Serving it (a
candle DeltaNet port or a second runtime leg behind the existing /judge
wire — the landing pad is committed: kev-layout detection, provenance
pins, the fail-closed runner) is a real capability gap, but the
consumer is measured-absent: LAYA STAYS, the margin rule was vacuous
with one contestant short, and the F13 precondition binds first. F16
records the find, the blocker, and the pull trigger (precondition
within ~20 cycles, or an operator ask). No other capability find: the
delta's feature surface (delegate fleet, daemon judge + /sessions, MCP
both directions, web tools, judge-parity runner) had zero
capability-blocked moments in the corpus; the t222-impl's live HF API
probing (web_fetch ×8) worked as designed.

## 5. Top 3 priorities

1. **T225** (pri 2, robustness) — progress-reset liveness fences: the
   delta's most expensive flake class (an 80-iteration child death +
   a main-tree fence expiry + recurring gate-isolation tax), one
   principled mechanism across both surfaces, RED-provable by
   construction (the slow-progress fixture IS the flake reproduction).
2. **T226** (pri 3, tests-only sweep) — the T217+T215 validators'
   left-behind survivors; the T224 pattern (file forward, sweep the
   family in one row) executed while both verdicts are fresh.
3. **T228** (pri 3, doctrine, SOLO) — verify-the-indictment: the I6
   defect class cost a full reverted round; the clause is one sentence
   where the eval's filing bar already lives.

**Estimate re-calibration (standing doctrine: text, never the
threshold).** Delta actuals vs filing estimates: T224 ~120 → 209
(1.7×); T221 ~122 → 122 (1.0×); T218 ~114 → 114 (1.0×); T216 ~96 → 93
(1.0×); T217 ~195 → 195 (1.0×); T212 ~130 → 297 (2.3×); T214 ~200 →
486 (2.4×); T213 ~200 → 614 (3.1×); T215 ~450 → 694 (1.5×);
T219 ~450 → 1,216 (2.7×); T222 ~400 → 1,803 (4.5×); T223 ~120 → 666
(5.6×). The distribution splits cleanly by ROW KIND: **pin/test-closure
rows on existing surfaces land 1.0–1.7×** (T216/T217/T218/T221/T224)
and **novel-infrastructure rows — a NEW registry, loader, or runner
file carrying a mechanism — land 2.7–5.6×** (T219/T222/T223), with
multi-surface doctrine+pin rows in between (2.3–3.1×). Filing guidance
from this delta: a novel-mechanism row narrative-~150 should be filed
at ~500 (the split ceiling) or split pre-emptively; pin-closure rows
can trust ~1.5×. The ~500 ceiling and the T209 gate stand; this eval's
four rows are filed at ≤~350 all-in.

## 6. README audit (usability)

Cold-read pass (1,085 lines): reading order holds — Install →
Quickstart → Runbooks signpost → chat → run → fork → plan → TUI →
Tools → risk gate → hooks → permissions → MCP → Langfuse →
self-hosting → loopd → Development. The delta's three features landed
INTEGRATED: the /sessions registry (T219) is inside the risk-gate
judge-daemon paragraph (lines 589–625, CHUG_JUDGE selector → registry
→ weightless host → kev-layout classified refusal → judge-parity
runner + verdict-doc link), and the Development layout brace carries
kev_config,kev_model (line 1083, T95 guard green). (a) Reading order:
sound. (b) Redundancy: the daemon's one cross-reference in the loopd
section (line 1034, "serves the risk gate's /judge only") is a pointer,
not a doubled claim; no drift found on the delegate/budget/sandbox
surfaces. (c) Staleness: none found — T204/T219/T222/T223 surfaces are
current; the loopd section's T81/T137/T152/T82/T142/T178 doctrine is
all live. (d) Balance: the loopd section is the heaviest (~112 lines)
and dense — every clause is load-bearing doctrine with no other home
today; WATCH-ITEM: if it grows another ~30 lines next delta, the next
eval files a runbook-extraction docs row (not now). (e) Quickstart
truth: holds (unchanged surface). **No docs row filed.**

## Handoff — recommended execution order

Queue for the next cycles (priority order per LOOP-SPEC §2; all four
specs ready, estimates ≤~350 all-in, T209-gate-clean at filing):

1. **T225** (pri 2, robustness) — glm impl; testsupport.rs is NOT on
   the step-4 core list but the ~350 estimate flips T189 lane input (b)
   → kimi REQUIRED expected (the lane inputs computed from the diff
   decide mechanically).
2. **T226** (pri 3, tests-only sweep) — glm impl; lane-eligible only if
   the landed diff stays ≤~150 AND req 2's needle stays tests-only; a
   loopd.sh needle edit flips it to kimi REQUIRED. NOT disjoint with
   T225 (tests/loopd_daemon_ensure.rs appears on both surfaces — T225
   converts its fence, T226 adds a probe pin): run serially.
3. **T228** (pri 3, doctrine, SOLO) — glm impl, kimi REQUIRED, runs
   alone (META-META-SPEC carrier).
4. **T227** (pri 4, doctrine, SOLO) — glm impl, kimi REQUIRED, runs
   alone (LOOP-SPEC carrier). NOT T45-bundlable with T228 (pri 4 fails
   bundling rule (d); different doctrine files anyway) — serial solos.

To SELF-SPEC (continuous improvement): none new. Big enough for
META-SPEC fan-out: none new (the loop is the fan-out). Human-decision
items: the laya HF hosting decision (operator-pending Monday 2026-10-05
per runbooks/laya-hf-hosting.md — do-not-execute honored) and the F13
phase-3 GO precondition (corpus 967 / ~2,570 — no human action, stated
for visibility; F16's pull trigger rides the same precondition).

## Outcomes (filled at cycle wrap — LOOP-SPEC Phase 3)

### Cycle 103 (2026-10-04, ~16:50 UTC–) — MANDATORY fresh-eval cycle (kimi; predicate failed on the todo-rows half: queue DRAINED at the cycle-102 wrap, 0 todo rows)

**T226 — close the T217+T215 validators' left-behind survivors (pri 3, tests-only sweep)** — LANDED (merge 0c45f95; impl glm 80/80 goal-accepted on the LAST iteration ~50 min, zero deaths). Three pin families +342/-1, zero production diff: (1) T217 m4 — the zero-commit git repo refuses exit 4 naming 'git log empty', page byte-identical, validator-verified SOLE killer (the five pre-existing t217 pins stay green under the same mutant); (2) T215 LOW-(a) — the leg-(b) probe blindness pinned as ACCEPTED against both binary shapes through the real loopd.sh sandbox; the spec's preferred needle correctly did NOT land (the Daemon subcommand is ungated, clap renders identical help — validator-verified), install.sh carrier argument recorded; (3) T215 LOW-(b) — the feature-off wrong-binary client fails open with the latch:true fast-fail flavor ('exited before serving', never the 60s 'gave up waiting'), refusal trail exactly once. Kimi PASS (d1791143226-18): 3 parallel mutant legs (disjoint files) all RED-reproduced independently, tree shasum-byte-clean. Gates: worktree + post-merge main nextest 1621/1621 (suite +4).

**T225 — progress-reset liveness fences for the child-spawn test surfaces (pri 2, robustness)** — LANDED (merge 12c0446; impl glm 66/80 goal-accepted ~44 min, ZERO budget deaths). testsupport.rs gains ProgressDeadline (trips only after `base` of NO observed advance on the watched surface; absolute backstop load_scaled_deadline×4; fail-safe unreadable=progress; pure trip_decision + synthetic-clock seams) + surface_fingerprint + 5 pin legs + the extended adoption grep pin over 6 converted surfaces; adopted at the 3 loopd verdict fences (supervisor-log growth surface) + launch.rs's 4 real-child poll fences; status.rs's no-surface reap poll re-based to load_scaled_deadline; semantic wait_secs bounds stayed absolute by design (compliance table verified by the validator). Kimi PASS (d1791138089-13): 8 mutant legs 7 RED + 1 predicted survivor (M7 mtime-arm), old-vs-new discrimination independently re-run (pre-T225 absolute fence blows at 634.6ms mid-advance; committed shape green), tree byte-clean, T79 overlap judgment declared. Gates: worktree nextest 1617/1617 (36.4s), post-merge main 1617/1617 (36.9s). Validator's 2 pin-strength findings filed forward as T229 (T224 pattern). The row closes the cycle-100/101 false-red class whose measured costs were an 80-iteration child death (t219-r1, 10 delegate launch bin-test flakes) and a main-tree fence expiry (cycle-101) — T214's per-core load variable reads QUIET on 18-core K7 while suite fan-out does the damage.

### Cycle 102 (2026-10-04, ~14:41 UTC–) — routine freshness-skip cycle (glm; predicate held: 1 todo row + EVALUATION mtime 10:33 UTC = launch day)

**T224 landed (b535218: e0bc65c + spec re-key).** The three T222-validator
pin-strength findings closed as tests-only (+209/-0 in tests/kev_loader.rs,
zero production diff), each RED-proven against its named mutant then
reverted shasum-byte-clean (T69 sweep-the-family): m9
`auroc_midrank_tie_handling_pin` (divergent tie corpus pins midrank 0.875
EXACT — plain rank gives 0.75/1.0, so the mutant cannot produce it; all-tied
corpus pins the 0.5 convention; same structure end-to-end through
load_corpus→parse_record→evaluate); m12 `parse_record_refusal_legs_pin`
(bad probs sum 1.5 binary-exact / label 2 out of range / non-permutation
flip, each refused by its named error, valid control first); m8
`daemon_routing::daemon_dir_arm_routes_kev_layout_to_the_classified_refusal`
cfg(all(unix, feature="daemon")) — every lifecycle test runs
CHUG_DAEMON_STUB and drops CHUG_LAYA_CHECKPOINT, so the real_backend Dir-arm
routing was deletable unnoticed; the pin spawns the REAL binary (no stub)
with CHUG_LAYA_CHECKPOINT at a kev-layout tempdir (adapter_config +
provenance + stub head.pt — the classified refusal fires BEFORE any big
fetch) and pins non-zero exit + 'cannot serve yet'+'candle-blocked' stderr +
no socket bound. Arc: glm impl goal-accepted 55/80 (~35 min, first try);
T189 lane call (d1791128510-1) computed from the diff — input (b) flipped
(209 changed lines > ~150), so FULL adversarial validation ran despite the
row's filed gates-only expectation (that expectation was keyed to the
~120-line estimate; the honest arc ran 209); kimi validator (pid 66267,
slot validate-a, 41/60, goal-accepted) VERDICT PASS (d1791130461-2): 5
mutants re-run RED in throwaway worktrees with the T222-predicted signatures
(m9 0.75; m8 'missing model.safetensors' fall-through; m12 sum/label/
permutation deletions), m9/m12 serial (overlapping file) + m8 parallel
(disjoint src/daemon.rs), gates re-run green both feature legs (nextest
1593/1593, daemon 9/9, clippy -D clean), production tree shasum-verified
byte-clean. Two non-blocking validator observations recorded (m8 deadline
path drops the Child without kill — matches the daemon_lifecycle.rs pattern;
209 vs ~120 estimate = the m8 fixture scaffolding). Post-merge gates in main
1593/1593. Dispatch note: the validator-launch target-dir drift WARN was
resolved by the dispatch-time spec re-key (b535218, T175/T212) — committed
after the validator exited because its live git index lock held the
worktree; the goal gate reads the FILE per iteration, so the validator's
check legs ran validate-a regardless. Queue EMPTY after flip — next cycle
is a mandatory fresh eval.

### Cycle 101 (2026-10-04, ~09:09 UTC–) — routine freshness-skip cycle (glm; predicate held: EVALUATION mtime 08:33 UTC = launch day [the cycle-100 wrap commit 2810ed9] + T220/T222/T223 todo) — queue: T222 → T223

**WRAP (T220/T222/T223 landed; T224 filed, stays `todo` — wall budget, spec
ready; v0.17.0 tagged+pushed with final gates green at HEAD: 4 items since
v0.16.2 ≥ 3, minor for the two features). Books: both worktrees harvested
(9 t222 artifacts, 9 t223 artifacts incl. 4 event segments + both verdicts)
then removed with every hosted pid verified gone-or-defunct; all routing/
verdict/outcome records landed (18 this cycle) incl. the T152-pattern
correction for the phantom-id outcome subject; one process-hygiene incident
closed (the round-1 validator's + impl's leaked judge daemons, ~20 GB RSS,
reaped by the orchestrator before the fix-up); one bookkeeping incident
closed (the orchestrator's gate chain lacked pipefail and pushed a red
todo_consistency — the T8 `max|dp|` notes-cell pipes — fixed forward in
6365b50 with the root cause named in the commit). Handoff: T224 spec ready
(tests-only, pri 3); the kev DeltaNet candle port is the named first blocker
for any real two-contestant bake-off (out of scope, needs its own eval
filing if pulled); the F13 fine-tune is the named follow-up to the holdout
AUROC 0.374 finding.

- **T222 LANDED (merge cdf2e06, landed-clean)** — the kev-0.8b loader + judge-parity
  harness (T220's code half), and the row's real product is its second premise
  defect: the CONFIRMED contestant `jaredpalmer/kev-0.8b@bf75a6a8` sits on
  `Qwen/Qwen3.5-0.8B-Base@dc7cdfe2` — model_type `qwen3_5`, 18 Gated DeltaNet
  (linear attention) + 6 full attention layers, LoRA r16/α32 over
  attention+MLP+DeltaNet projections — and candle-transformers 0.11 (max on
  crates.io) has NO qwen3_5 model, so the 0.8B contestant is in the spec's OWN
  "DeltaNet, candle-blocked" out-of-scope category. The impl child refused to
  substitute candle's dense qwen3 (a different model served under the same name
  would be a SPEC-3 violation) and delivered the honest surface instead:
  `src/kev_config.rs` (ungated: both revision pins, PEFT LoRA parse refusing
  rslora/DoRA/fan_in_fan_out, base-arch classification DeltaNet-vs-plain-qwen3-
  vs-unknown-never-guessed, provenance base-pin verification — 6 plain-gate unit
  pins), `src/kev_model.rs` (daemon-gated: kev-layout detection incl. cache-first
  offline hub fallback, small-files-first inspect, classified refusal BEFORE any
  big download), `daemon.rs` real_backend kev-routing (clients fail open, zero
  gating change), `judge_pack` parameterization (model label + `act_probability:
  Option` — kev omits the rl_agent ext, never fabricated; the laya goldens stay
  byte-green), and `tests/kev_loader.rs` (the harness: choice top-1, score MAE,
  noul AUROC via midranks, mean |p_diff|, option-order flip rate over committed
  fixtures + fixture-shape/determinism/both-orderings/wire-shape pins + a
  daemon-gated live leg that skips-not-fails offline and re-verifies both
  revision pins when live). Impl glm run-1 died 80/80 mid-premise-discovery (T63
  resume), run-2 died 80/80 POST-COMMIT (2ddeca3) → orchestrator-finish (T55/T150
  precedent): the `readme_layout` pin caught the two new modules missing from
  README's layout line (880fb8a). T189 lane failed on size (1801 lines) → kimi
  REQUIRED (d1791109733-4): VERDICT PASS (d1791112723-5) — gates re-run green
  incl. daemon-feature nextest 1602/1602; the premise deviation independently
  verified against the LIVE HF API; 2/2 executed mutants killed (m1
  classify-arm, m2 base-pin); 7 findings — 3 predicted-survivor pin gaps (m8
  daemon kev-routing Dir arm, m9 AUROC tie-handling, m12 corpus refusal legs)
  filed forward as T224; T223 (the bake-off run) inherits the DeltaNet port as
  its first blocker. Bookkeeping incident: the merge commit message first cited
  a phantom verdict id (d1791109733-5 — the record landed as d1791112723-5);
  fixed by amend pre-push + the T152-pattern correction record in
  `.chug/decisions.jsonl` (d1791112737-8).

- **T223 + T220 LANDED (merge 1231a89, landed-clean after a FAIL→fix-up→PASS
  arc)** — the bake-off RUN + verdict doc closes the operator's T220 filing
  (both halves now in): `scripts/judge-parity.sh` is the fail-closed idempotent
  runner (T200 export → 211 closed-set labels; the F13 time-ordered last-20%
  tail = 42-record holdout; the 8 INDEX-keyed stop-judge goldens; one release
  daemon per revision-pinned checkpoint over the real /judge wire, canonical +
  option-flipped orderings, the T222 metric set per class), and
  `docs/judge-parity.md` records the honest one-contestant partial verdict:
  **LAYA STAYS** (the margin rule is vacuous with one contestant short — kev-0.8b
  candle-blocked per T222's premise defect #2, verdict-2.0 id still HTTP 401
  re-probed live, LocalLLaMA anchor not local/skipped) with REAL numbers —
  goldens parity max|dp| 0.00e+00 / 0 violations / top-1 8/8 / MAE 0.0001
  (verified 3 runs — the served laya IS the recorded laya); holdout n=42 choice
  top-1 0.5714, noul AUROC 0.374 (below coin-flip: deep-OOD, no gating signal,
  SPEC-3 stands — the F13 fine-tune is the named follow-up); option-order flips
  5/9 goldens + 17/42 holdout (the kev-card caveat holds for laya too; the
  runner measures both orderings by construction). Arc: glm run-1 died 80/80
  (runner built + validated by hand, doc unwritten) → T63 resume goal-accepted
  14/80 (1ef6482); kimi round-1 VERDICT FAIL (d1791123300-14) — MAJOR: the
  serve legs leaked their model-loaded daemons EVERY run (serve_leg ran inside
  command substitution so its PID_LIST side effect died with the subshell; the
  validator proved 6/6 orphans at 2.0–3.4 GB RSS and the orchestrator reaped
  ~20 GB of accumulated orphans before the fix) + 4 nits; the fix-up (86a6b06)
  swept the CLASS — both daemon starts now book their pid in the parent shell,
  cleanup+traps reap on EXIT/INT/TERM (130/143), manifest routing refuses
  unknown checkpoints fail-closed, wait_healthy bails on early daemon death,
  the tie-rule comment corrected — verified by orchestrator e2e (exit-1
  fail-closed, metrics byte-identical, ZERO daemons post-exit) and a proper
  trap test (an initial "leak" reading was the orchestrator's own test error —
  `$!` of a `cd && cmd &` compound is the wrapper subshell, not the script);
  kimi round-2 narrowed reval (d1791122288-13) VERDICT PASS (d1791123300-15).
  Environmental note recorded: the T214 load-scaled fence test
  (`a_failing_driver_probe_means_no_sweep`) expired its fence IDENTICALLY on
  main under ambient load 9.68 (fseventsd + EXO pegged since July) mid-cycle,
  then passed at post-merge — host-load flake family (T151/T172 lineage), not
  branch-attributable (the branch diff is script+doc only).

### Cycle 100 (2026-10-04, ~05:37 UTC–) — routine freshness-skip cycle (glm; predicate held: EVALUATION mtime 01:28 UTC = launch day + T219/T220 todo) — queue: T219 → T221 → T220

- **T221 LANDED (merge c85f6ba, landed-clean)** — the T219 validator's three
  actionable findings closed in one pass: the day-0 `parse_rfc3339` guard (rejects
  `day < 1` like day 32 → registration fails open to server-now; the debug
  conn-thread panic / release wrap-and-evict pair the validator verified black-box
  is gone), the `age_sec` value pin (kills mutant m5), and the 599/601 TTL boundary
  table (kills mutant m6 — the symbolic const legs could never see it), plus the
  ±2s band on the roundtrip `now` comparison (finding-4 flake). glm 49/80 with
  three serial self-mutations killed + reverted before commit (1792a51). T189
  gates-only lane (d1791101242-6, all four inputs pass: no core file, 122 lines,
  no new surface, no check change) — full gates still run: nextest release
  1579/1579 + clippy -D + spec check, worktree AND post-merge in main.

- **T219 LANDED (merge 0516e05, landed-clean after a T63 resume)** — the daemon's
  0600 socket grew the `/sessions` registry: `SessionRegistry` (upsert-by-id, lazy
  TTL eviction at SESSION_TTL_SECS=600, four SESSION_ROLES incl. the external
  dashd), `POST /sessions` fail-open (optional fields default to server-now, client
  strings echo verbatim, `started` sticks to first registration so `age_sec` tracks
  the process not the heartbeat), `GET /sessions` live-only/id-sorted with
  `age_sec`+`now`, daemon self-registration + heartbeat on each registry serve,
  no-chrono RFC3339 parse/format (Hinnant `days_from_civil` mirroring archive.rs),
  a weightless `CHUG_DAEMON_SESSIONS=1` registry host whose /judge refuses exactly
  like the stub's (SPEC-3: no fabricated classifications), stub refusal of
  /sessions on both verbs, and the shared best-effort `register_session()` helper
  wired into delegate launch+alive-status and loopd.sh cycle start (bounded curl,
  always-true guard). glm run-1 died 80/80 mid-debug of a launch-test flake it
  A/B-proved not-causal (T63 resume d1791094859-2); run-2 goal-accepted 16 iters,
  commit 2128e75. T189 lane (b)+(c) failed → kimi REQUIRED (d1791096146-3):
  **VERDICT PASS** (d1791098231-4) — 7-mutant serial sweep, 5 caught (TTL boundary
  flip, started-overwrite, stub-serves-registry, role-gate removal, self-heartbeat
  drop); m5 (age derivation inversion) + m6 (TTL 600→300) SURVIVED as pin-strength
  gaps against a correct implementation; gates build + clippy -D + spec check 6/6 +
  nextest release 1576/1576 + the fallback runner; tree byte-clean post-revert.
  6 findings → 1 MINOR (parse_rfc3339 day-0 underflow: debug conn-thread panic /
  release wraps-and-evicts, off-spec-path, verified black-box both profiles) + 2
  weak pins + 1 flake nit filed forward as **T221** (pri 2); findings 5-6 (dashd
  external-supervisor wiring, self-heartbeat on POST-only) adjudicated no-action.
  Note for the next eval: the filing-time estimate said ~450L, the arc landed
  ~1,205L (+803/-11 daemon.rs incl. ~314 inline tests + 362 pin file) — the second
  consecutive large estimate gap (T204 class); the dispatch gate held because the
  estimate was under the ceiling, so the honesty gap is in filing, not gating.

- **T221 LANDED (merge c85f6ba, landed-clean)** — the T219 validator's three
  actionable findings closed in one pass: the day-0 `parse_rfc3339` guard (rejects
  `day < 1` like day 32 → registration fails open to server-now; the debug
  conn-thread panic / release wrap-and-evict pair the validator verified black-box
  is gone), the `age_sec` value pin (kills mutant m5), and the 599/601 TTL boundary
  table (kills mutant m6 — the symbolic const legs could never see it), plus the
  ±2s band on the roundtrip `now` comparison (finding-4 flake). glm 49/80 with
  three serial self-mutations killed + reverted before commit (1792a51). T189
  gates-only lane (d1791101242-6, all four inputs pass: no core file, 122 lines,
  no new surface, no check change) — full gates still run: nextest release
  1579/1579 + clippy -D + spec check, worktree AND post-merge in main.

- **T220 SPLIT + DEFERRED (wrap disposition, not an arc)** — the dispatch re-read
  found the T204-class true surface (new candle architecture + HF weight downloads
  + live inference + a committed verdict doc behind a ~450L filing estimate — the
  third consecutive estimate-honesty gap) AND a premise defect found by a 30-second
  HF API probe: the researched contestant id `Heman10x-NGU/openJev-verdict-2.0`
  does NOT resolve ("Invalid username or password" = absent/private as written)
  while `jaredpalmer/kev-0.8b` resolves clean (sha `bf75a6a8…`, LoRA + head.pt +
  provenance.json). Split by acceptance surface (T209) into **T222** (kev loader +
  harness, ~400L est) and **T223** (bake-off run + docs/judge-parity.md verdict,
  ~120L est; T220 closes with T223); the premise defect is written into both specs
  so the next cycle's dispatch is honest without re-research. Deferred with reason:
  wrap-margin discipline (T207) — two ~50-min children with T204-class death
  profiles would have spent the entire remaining wall inside one row.

- **Cycle-level notes.** Routine glm freshness-skip (T81 routing 9-for-9 correct —
  the predicate held and the cycle was glm). Two full arcs landed (T219 feature +
  T221 validator-findings bug, both in the daemon area, serial) plus one prepared
  split; one T63 resume burned (t219 run-1, 80/80 mid-debug of a launch-test flake
  the child itself A/B-proved not-causal — the delegate launch-test family flakes
  under concurrent loopd load, passes at --test-threads=1; noted for the next eval
  as a possible deflake row). The T197/T212 dispatch-time re-key discipline caught
  its own omission mid-flight: the validator launch WARNed on check-vs-goal
  target-dir drift and the validate-a re-key landed on-branch (ef19737) before the
  gate could run against the wrong slot. Queue for the next cycle: T222 → T223
  (the split bake-off), then whatever the fresh eval files. Release: 2 items landed
  since v0.16.2 (< 3, no FEATURES check-off) → no tag this wrap.

### Cycle 99 (2026-10-04, ~01:00 UTC–) — routine freshness-skip cycle (glm; predicate held: EVALUATION mtime UTC day = launch day) — working T214/T213 from the cycle-98 eval

- **T214 LANDED (merge 654993d, landed-clean after a T63 resume)** — the isolation
  tax closed at its root: `load_scaled_deadline(base) = base × clamp(loadavg_1m/cores,
  1, 4)` with a fail-safe seam (any read failure → factor 1.0, byte-identical base),
  pure `scale_factor` legs, and all THREE spawn-heavy families' verdict fences
  (reaper 30s, spoof 30s, daemon_ensure 90s) re-based on measured load — bases
  unchanged (zero-timeout-bump doctrine), T158 markers kept, adoption grep pin
  (T48-runtime needles) asserts zero bare `from_secs(30|90)` constructions. glm impl
  died the 50-min wall at 61/80 UNCOMMITTED → ONE T63 resume committed dc63eba +
  goal-accepted (1550/1550 full nextest release, NO family isolation, quiet-host note
  honest per spec). 484-line diff flipped T189 lane criterion (b) → kimi REQUIRED
  despite the tests-only filing: verdict PASS, 5/5 mutants killed (M2's 8.0-clamp
  caught only by the child's above-spec legs), 4 non-blocking observations (double
  seam read race ~0.1%, single-line pin needle, mul_f64 MAX unreachable, sysctl
  /usr/sbin fallback exercised by the gate shell itself).
- **T213 LANDED (merge d8e24f3, landed-clean after orchestrator-finish)** — the T205
  validator's three LOW survivors closed in one SOLO arc: the canary VALUE pin
  (emit_fix_line recording sink + three-surface absence assert), the BYTE-IDENTICAL
  loopd.sh env-loader extraction into scripts/loopd_env_loader.sh with a 7-leg
  driver (allowlist, count-only log, explicit-env-wins, malformed tolerance,
  absent-file no-op), and the one-attempt-fetch comment. glm committed all three
  parts then died the 50-min wall at 65/80 mid-verification, goal unaccepted →
  T55 orchestrator-finish; MY step-3 gates caught a real break the child never saw:
  loopd_model_routing's minimal fixture_dir copies loopd.sh text WITHOUT the new
  scripts/ companion, so the sourcing line (line 59, before any mode dispatch) died
  under set -euo pipefail in 5 sandbox tests — fragment-beside-copy fixture repair
  committed on-branch before validation. kimi REQUIRED (loopd.sh doctrine carrier):
  PASS, 3/3 mutants RED (m1 canary leak = the spec's M1 RED-proof, m2 allowlist
  widening caught twice, m3 explicit-env-loses), byte-identity independently
  re-verified via git show, 1559/1559 nextest release + clippy -D. T217/T218
  operator filings merged mid-cycle (4d578e1) after a push rejection.
- **T217 LANDED (merge e242689, landed-clean first try)** — the operator's pri-1
  "stats are broke" closed fail-closed at the source: scripts/site-sync.sh refuses
  (one named error, exit 4, zero writes/commits) when TODO.md, .chug/loopd, or git
  log are unreadable — the 58c3a0b rogue-writer shape (orphan vs worktree-removal
  race) can no longer gut the live page; --bootstrap is the explicit escape and the
  commit message now carries items/tests/cycles counts so a gutting commit is
  distinguishable at a glance. glm goal-accepted first try 39/80; 195-line diff
  flipped T189 lane (b) → kimi REQUIRED: PASS, 3/3 priority mutants RED, gates
  site_sync 27/27 + nextest release 1564/1564 + clippy -D. One MINOR filed forward:
  the git-log guard leg has no isolated fixture pin (a bonus m4 mutant survived only
  that leg — beyond the spec's Tests section, defense-in-depth).
- **T218 LANDED (merge 104a8d8, landed-clean first try, gates-only lane)** — the
  third site-generator layer closed: card BODIES are now machine-owned like badges.
  body_ensure regenerates matched cards' <p> from the row's prep()d what-text in the
  same pass (position/classes preserved, non-F cards untouched, single-line <p>
  only, what-text via ENVIRON so backslashes survive) — the F15 "http\" fossil can
  never re-fossilize; the next sync after any bad write heals it. glm goal-accepted
  first try 34/80; T189 gates-only lane (~114 lines, re-key-only check change per
  the T216 precedent): nextest release 1565/1565 + clippy -D in worktree and main;
  orchestrator RED-proof (body_ensure no-op → the heal pin + 2 t99 idempotence pins
  RED, byte-identical restore). T219 (daemon /sessions feature) + T220 (Kev parity
  bake-off) remain queued with ready specs for the next cycle.

- **Release v0.16.2 tagged at wrap** — 4 items since v0.16.1 (T214/T213/T217/T218,
  none a FEATURES check-off) → patch bump, `cargo check` lock regen, notes generated
  by scripts/release-notes.sh (HEAD v0.16.1), check-tag-version ok, final gates
  green at HEAD (build + clippy -D + nextest release 1565/1565), commit bcc98c4 +
  tag pushed together.
- **Cycle-level notes** — a routine freshness-skip cycle (glm orchestrator) that
  landed 4 rows with 2 kimi validation rounds and 2 lane calls; both child budget
  deaths absorbed by doctrine (T63 resume when uncommitted, T55 orchestrator-finish
  when complete-committed) with zero work lost; the orchestrator's own step-3 gates
  caught the one real break children missed (T213's fixture-vs-sourcing-line break —
  the child died mid-verification of exactly that sweep). Validator deaths: none
  (all three kimi rounds finished within budget, verdicts written). Deferred: T219
  (feature, spec ready) and T220 (eval-heavy, spec ready) — 56 iterations at the
  boundary vs a ~30 wrap tail: does not fit with margin. Three mid-cycle operator
  push rejections reconciled by merge (T217/T218/T219/T220 filings; one TODO.md
  conflict, both sides kept). Next cycle opens with 2 ready specs and no blockers.

### Cycle 98 (2026-10-03, ~19:05 UTC–) — MANDATORY fresh eval (queue drained at cycle-97 wrap; kimi, T81 routing 8-for-8) — 3 rows filed (T212/T214/T213)

- **T215 LANDED (merge 04ac3f1, fixed-up)** — the operator's mid-cycle pri-2 filing
  (daemon ensure spawns the feature-off stub, ~16h of nonzero ensures on K7).
  loopd.sh resolves the daemon-capable binary ONCE per run before the cycle loop
  (CHUG_DAEMON_BIN executable probe-free → ~/.local/bin/chug `daemon --help` probe →
  feature-on repo build via the refusal-literal byte-grep), one startup log line,
  fail-open skip; the 549-line tests/loopd_daemon_ensure.rs family pins the order
  behaviorally against the real loopd.sh (11 pins incl. probe-cache and
  literal-coupling-to-src legs); the runbook pins the pre-warm one-liner (the
  validator verified spawn_daemon detaches, so a cold 650MB download survives the
  bounded wait). glm impl died at BOTH budgets (80/80 iterations → T63 resume
  d1791066541-16 → resume died at the 50-min wall with the goal gate VERIFYING and
  the work complete-committed → T55/T150 orchestrator-finish, no second resume).
  kimi validator PASS (d1791072243-18; the validator itself died 60/60 with the
  verdict WRITTEN — the verdict-file doctrine absorbed it): 9/9 mutant runs RED
  including the K7-regression leg, gates reproduced exactly (1527/1527 nextest
  full-suite, fallback 1527, family 11/11 isolated in 4.5s). Findings filed
  forward: the new family's full-suite fence trips are the T152 load-stretch class
  (T214's spec amended pre-dispatch to adopt it — 3 legs tripped at ~90.7s on one
  full-suite run, 11/11 green isolated); LOW: leg-(b)'s `daemon --help` probe does
  not discriminate feature-off from feature-on (realistic carrier path safe —
  install.sh installs release.yml's feature-on tarballs) and the client-side
  daemon_binary() wrong-binary shape (latch:true mitigates; a future sweep row
  candidate); 3 NITs + 2 bookkeeping observations. Post-merge gates 1527/1527.
  The operator reconciled two more mid-cycle pushes this cycle (f3e01e0 F15 tidy).

- **T216 LANDED (merge f5cd2e4, landed-clean)** — the operator's second mid-cycle
  filing (chug.sh F15 card rendered QUEUED + fragment what-text: markdown-escaped
  pipes in the name cell phantom-split under awk -F'|'). The fix is the spec's
  named remedy — park each escaped pipe on a \001 placeholder in a line COPY
  before the split, restore the literal pipe in the surviving cells — plus 3
  site_sync pins (+68 lines). T189 gates-only lane (d1791073767-22: 96 changed
  lines, no core-list file, no new surface, re-key-only check change) — the lane's
  first use this cycle and its cheapest arc: glm goal-accepted 38/80 in ~12 min,
  nextest 1528/1528 + clippy -D in the worktree, post-merge 1528/1528. The F15
  card renders landed with the full what-text on the next site-sync run.

- **T212 LANDED (merge 7c76356, round-2, fixed-up)** — the cycle's deep one. Round 1
  built the advisory fix on a FALSE premise and the round-1 kimi validator caught it
  (FAIL d1791061091-8, proven three ways: main.rs:87 re-read-every-iteration,
  driver.rs:462/1171/1556 arg-path reads, the t209/t210 verifying events running the
  un-keyed check): the goal gate reads the spec ARG path, branch re-keys never reached
  it, the cycle-97 WARNs were TRUE positives, and T197's advisory was correct all
  along — round 1 would have silenced the one truthful component. Round 2 (option C,
  d1791061091-9 — launch-template spec arg → the WORKTREE copy, rejecting a driver-side
  cwd-shadowing rule for every run): full revert (sha256 byte-identical), the SPEC-ARG
  RULE sentence in step 2's template (T63 resume + step-4 validator inherit; the
  advisory then judges the very text the gate enforces), t209/t210 main-spec slot
  residue restored, todo_consistency pin 21/21 with 3 T79 parallel mutants RED-proven.
  kimi round-2 PASS (d1791064272-10): all five findings answered + independently
  re-verified, gates reproduced exactly (1516/1516 nextest, clippy -D, fallback PASS),
  launch replay re-derived (no-WARN on agreement, WARN naming both carriers on drift).
  Orchestrator review caught + fixed one sed over-reach (the validate-a re-key had also
  rewritten req 3's quoted literal — restored pre-merge). The design dogfooded at its
  own launches: the round-2 fix-up and validator launches carried the worktree-copy
  spec arg and printed ZERO drift WARNs. glm r1 70/80 (one gate rejection on the
  T31-named setsid flake, recovered), fix-up 60/80, both goal-accepted.

### Cycle 97 (2026-10-03, ~16:48 UTC–) — glm routine freshness-skip cycle (predicate holds: eval same UTC day + 2 todo rows; routing correct 7-for-7) — queue: T209 → T210

- **T209 landed** (cc1d446, ff-merge 3d8f6ca, flip cycle 97) — the dispatch-time spec-size gate: LOOP-SPEC step 2 gains the gate stated BEFORE the launch template (re-read the spec's `estimate:` line at dispatch; >~500 lines or missing estimate on a non-trivial row → NOT dispatched, re-split 2–3 rows first, specs rewritten); the split rule is now **by acceptance surface, not by component** — each half carries its own independently-gateable `check:` surface and its own ≤~500-line estimate, a half still over the ceiling (T204 child B: ~2,100 lines after a by-component split) is re-split again; the T173 minutes-raise paragraph carries its Measure-clause RESOLVED marker (census tripped: t197/t203 minutes-bound + T204 iteration-bound; remedy = the dispatch gate, no further budget raises); step 4's validator-budget paragraph records the validator note (both t204 validators died 60/60 WITH verdicts written — the verdict-file doctrine absorbed both, the clause's UNANNOUNCED letter not tripped; the next trip trims default mutation-leg counts); META-META-SPEC's spec quality bar gains the estimate-is-a-DISPATCH-TIME-contract sentence (filing-time honesty is not enforcement — T204 said ~800, landed ~3,000+ all-in). Pins: legs ah–ak in tests/loop_spec_recovery.rs, RED-proven by 5 deletion mutants; the kimi validator independently re-proved 6 mutants (incl. M6 gate-moved — the BEFORE-the-template requirement is pinned by ORDERING, not mere presence) and re-ran gates 1514/1514 nextest release + clippy -D (verdict d1791051840-3). The arc itself tripped the class it fixes: the glm impl died at the 50-minute wall 64/80 with the work complete-uncommitted (a t197/t203-class minutes death) — ONE T63 resume landed it in 8 iters (recovery d1791049411-1), the doctrine's cap working as designed; the T197 drift warning fired at validator launch (spec check line bare `target-shared` vs the validate-a goal/env) and the T175 re-key landed mid-flight on-branch (3d8f6ca) before the validator's next spec read.

- **T210 landed** (7deec48, ff-merge 1526e5a, flip cycle 97) — the launch-template placeholder guard: LOOP-SPEC step 2's launch template gains an inline PLACEHOLDER MARKER at its first `<N>` occurrence (the `cwd:` line annotation): every `<N>` is the row number's slot — substitute before dispatch; a literal `<N>`, `t<N>`, or `chug-loop-tN` surviving into a launched goal, a delegate spec path, or a bash command is a dispatch defect (the cycle-94 glm four-incident evidence named inline). Plus the pre-launch checklist line after the launch block: grep the rendered goal and the `cwd`/`spec` arguments for `<N>`/`tN` placeholders BEFORE the delegate call — one cheap look, not a tool. Pin `loop_spec_step2_template_carries_the_placeholder_marker_and_checklist` in tests/todo_consistency.rs pins the marker exactly-once AND inline, the template KEEPS its `<N>` placeholders (substitution pinned, never removal — m7 kills placeholder-stripping), and both checklist needles exactly-once. glm impl first-try goal-accepted 37/80 (~14 min); kimi validator PASS 7/7 mutants killed serial (deletion RED-proof m1, relocation m4 via the inline-placement assert, partial-substitution m5, reword m6, placeholder-removal m7), gates re-run 1515/1515 nextest release + clippy -D + todo_consistency 20/20 (verdict d1791054123-6). Two infrastructure observations for the next eval: (a) the T197 drift warning fired at validator launch despite the T175 pre-key (1526e5a) — delegate's drift check reads the spec from the MAIN-repo path while the child's goal gate reads the WORKTREE copy, so a correctly pre-keyed branch still warns; advisory false positive; (b) a `git worktree add … | head -2` pipe SIGPIPE'd git mid-checkout (worktree dir cleaned, branch left dangling) and the `;`-separated cargo build then ran in MAIN unconditionally — state-changing git commands must never ride a `head` pipe (both notes filed in the cycle-97 Outcomes; no code change this cycle).

**Cycle-97 wrap (2026-10-03, ~19:1x UTC):** queue DRAINED — both filed rows (T209, T210) landed, zero open rows; next cycle's freshness predicate FAILS on the todo-rows half (queue empty) → MANDATORY fresh eval (kimi per T81, the 8th consecutive correct routing prediction would be kimi). Handoff state: EVALUATION.md dated 2026-10-03, specs/ all worked, no stale worktrees, main even with origin/main at 89f877b. Release check: 2 items landed since v0.16.0 (< 3, no FEATURES check-off) → no tag this wrap. Carried observations for the next eval: (a) the T197 drift warning's main-path spec read vs worktree-copy read (advisory false positive when pre-keyed, fired twice this cycle); (b) `git worktree add | head` SIGPIPE leaves a dangling branch + the `;`-separated cargo build then runs in main unconditionally (T195's no-block property biting in an unexpected spot — no code change filed, note-only until it recurs); (c) t209-impl's T63 resume carried the iteration budget forward as a FRESH 80 (the resumed segment ran 8 iters — headroom well spent, no census impact).

### Cycle 96 (2026-10-03) — 3/3 high-pri rows landed: T207 stop-dispatch margin 15→30 + wrap-state hard rule (7d640b5, kimi PASS 9 findings MET), T211 __pycache__ copy-bomb filter (8e44df1, gates-only lane), T205 F13 consumption path hf_hosting (cb52c53, kimi PASS) — v0.16.0 tag driver; T209/T210 deferred at the wall (specs ready). Verdict: the wrap exercised T207's own medicine (mid-wrap resume)




### Cycle 95 (2026-10-03) — 2 landed: T208 F13 distillation experiment MEASURED NO-GO with GO precondition ~3× corpus (cf6366b, kimi FAIL→fix-up→PASS — the choice-leak mutant proved the honest hygiene causal), T206 flock fork-inheritance race deflake test-side (1a41a00, gates-only lane, orchestrator RED-proofs M1/M2). Verdict: kimi's 5 honesty-defect catch on a zero-Rust diff was the cycle's best catch; v0.15.0 tagged (T203/T204/T206/T208)

### Cycle 94 (2026-10-02) — 2 landed: T203 DEPENDENCIES.md audit codified (c3c5975, docs-only guard floor), T204 baked-in Laya judge daemon F15 phase 1 (dbce0e2, 3-child split + 2 T63 resumes + kimi FAIL→fix-up→PASS, F15 checked off); T205 deferred (operator-blocked). Verdict: the ~2×-budget row validated the T110 spec-size class — the split recipe landed it next cycle

### Cycle 93 (2026-10-02) — 2/2 landed, queue drained: T199 decision-corpus integrity closed choice set + audit script (cd0e020, kimi PASS 6/6 mutants), T200 distillation export (48bd6c9, kimi PASS 6/7, M7 non-blocking); operator pushed T203–T205 mid-cycle → next cycle reconciles first. Verdict: T197's drift advisory caught its first real fumble within one cycle of landing (8bd6e15)

### Cycle 92 (2026-10-02) — 2 landed: T197 delegate target-dir drift launch-time check (0649f12), T202 T192-completion orchestrator-finish close (3127b9e); T199/T200 deferred (wall). Verdict: the deferred-handoff recipe closed by finish, not cold-restart

### Cycle 91 (2026-10-02, ~09:00–12:45 UTC) — kimi MANDATORY fresh-eval cycle (queue drained at cycle-90 wrap; T81 routing correct 4-for-4) — 8 rows filed (T195–T202) + the F13 phase-2a roadmap pull; 4 landed (T195, T196, T198, T201 — all doctrine), T202 impl-complete-but-unmerged (recipe on the row), T197/T199/T200 deferred (wall)

- **T195 landed** (merge c634a6c, impl df2c285 — glm 67/80 first-try,
  ~32 min, zero budget deaths). The gate source-touch guard: every
  bounded-gate template running in a non-main checkout against a shared
  role-keyed dir now carries the immediate `touch src/*.rs tests/*.rs;`
  prefix (LOOP-SPEC steps 1/3/4, META-SPEC §5/§7, the guard-floor clause,
  META-META-SPEC's check-line convention sentence); main-dedicated gates
  exempt by T57 construction, impl inner loop exempt — both exemptions
  stated once at the first carrier. Measured grounding (this eval):
  touch forces an 18s release bin rebuild / 29.7s all-targets — the
  guard's per-gate cost against the ~30-min false-red diagnoses it kills
  (cycle-84 precedent) and the silent false-green leg. Pins amended
  T187-style + ONE new structural pin (adjacency-exact, RED-proven).
  kimi REQUIRED PASS (routing d1790935661-17, verdict d1790936624-18):
  gates re-run 1434/1434 + clippy, 7 serial mutants ALL RED-as-expected
  (incl. the spec-mandated M1 + both structural-pin legs), 0 survivors,
  4 minor non-blocking observations. Post-merge main gates 1434/1434
  under target-shared-main. The arc dogfooded its own doctrine: the
  review gate and the validator both ran the touch prefix.

- **T196 + T198 + T201 landed (T45 doctrine bundle)** (merge 1136ade;
  per-row commits d4229bf / 96ccd9f / ca82fd9 in queue order). T196: the
  step-2 impl goal template bans tree-wide formatters (one woven
  sentence + exactly-once pin, RED-proven). T198: loopd cycle budget
  `--max-minutes 240→360` with the walls arithmetic in-comment (360 =
  p95 × 1.5; iters stay 200) + the argv pin amended in-commit. T201:
  validator verdict-first — VERDICT + findings land in
  `.chug/verdict.md` at decision time (META-SPEC §6) and step 4 reads
  it before spending a T63 resume (LOOP-SPEC) — the deviation
  ("heredoc" → "one bash command") verified justified by the T126
  exactly-once pin. ONE kimi round covered the bundle (routing
  d1790939775-22, verdict d1790940890-23): PASS first round, 5/5
  mutants killed across two T79 waves (per-row deletions + needle
  corruption), 1 non-blocking observation (loopd.sh:314's stale
  171/240 comment — comment-hygiene candidate). Arc: impl died 80/80
  with t201 written-uncommitted → ONE T63 resume (routing
  d1790939069-21) finished in 10 iters / ~9.5 min. DISCLOSED INCIDENT:
  the impl's first t196 commit heredoc mangled and the shell EXECUTED
  the prose's backticked `cargo fmt` — a tree-wide reformat (89 files)
  the child stripped T63-style before recommitting; the row's own
  incident class via a different vector (heredoc-execution, not
  intent) — noted for the next eval. The validator DOGFOODED T201:
  its verdict.md was written at decision time and harvested
  (verdict-t196-bundle-20261002.md). Post-merge main gates 1436/1436.

- **T202 deferred at the wrap wall** (recipe on the row). Full impl
  landed in `loop-t202` as two commits (1d98253 legs 1–2, 169fad3 leg
  3) via an 80/80 death + ONE T63 resume; the resume's first
  goal_complete was REJECTED — the child's 300s bash cap kills the
  T195 touch-forced all-targets rebuild mid-check (the goal GATE's
  1200s would pass; a new friction interaction for the next eval:
  children observing a touch-guarded check end-to-end need the split
  build-then-test run). At the wrap wall with the resume still in
  flight, the arc hands off per T28/T19 (routing d1790944041-27).
  SPEC-ACCURACY MISS (mine): the filed acceptance claimed "ctx-edit
  fires: 2" on the cycle-89 stream — re-measured at wrap: ZERO
  `ctx_edit` events in every delta stream (the emission site
  driver.rs:1651 is live; no accepted live-ctx edit occurred in the
  sample). My eval misread a combined grep output; the acceptance
  evidence becomes 0-on-delta-streams + fixture-pinned rendering,
  adjudicated at next cycle's review.
- **T197, T199, T200 deferred — unworked** (wall-clock; T197's
  ~70–90-min arc vs ~75 min left after the bundle was a coin flip
  that risked a mid-validation orchestrator death, and dispatching it
  would ALSO have trapped T202's serial merge behind it — the
  cycle-88 T187-deferral precedent; T199/T200 sequence behind T197 —
  all three specs are dispatch-ready: worktree + warm build + glm
  80/50 + gates + the lane call from the diff).
- **Cycle-level notes.** 4 rows landed (T195 + the T196/T198/T201
  bundle), all doctrine, 2 kimi rounds both PASS-first-round
  (12/12 mutants killed, 0 survivors, 2 justified deviations).
  Validator census: 2/2 announced verdicts — and T201 was dogfooded
  in-cycle (the bundle validator wrote verdict.md at decision time).
  Impl budget deaths: 2 of 3 impl arcs (bundle 80/80, t202 80/80) —
  both recovered via ONE T63 resume each (the T63 machinery held).
  The T195 review gate + both validators ran the touch prefix
  in-cycle (adoption immediate). The disclosed fmt-execution incident
  (bundle impl's mangled heredoc EXECUTED the prose's backticked
  `cargo fmt`, 89 files, stripped pre-commit) is the T188 class via
  the heredoc-execution vector — the ban covers intent, not shell
  mangling; flagged for the next eval (commit-message quoting
  discipline). Release trigger check at wrap: 4 items since v0.13.0
  (T195, T196, T198, T201) ≥3 → patch bump (no feature check-off:
  all doctrine) → v0.13.1 pending final gates.

### Cycle 90 (2026-10-02, 08:06–09:00 UTC) — glm routine freshness-skip cycle (freshness held: EVALUATION.md 03:41Z same UTC day + 1 todo row T187) — 1 landed (T187), QUEUE DRAINED

- **T187 landed** (merge ae9ea36, impl d3d4da5). The 3x-deferred doctrine row landed first-try clean: META-SPEC.md's bounded-gates (T6) hard rule re-keys the illustrative caps from the dead form (`alarm 600` / `timeout 600` — under any bash cap below 600s the cap's process-group SIGKILL lands before the inner alarm can fire) to `alarm 280` / `timeout 280`, matching T178's LOOP-SPEC templates verbatim, and gains the rule in one sentence: the inner bound must sit BELOW the driver's bash cap (280 under the fleet's CHUG_BASH_TIMEOUT=300, ≤ ~110 under the 120s default). Both pins that ASSERTED the 600 form were amended in-commit with the justification named (nextest_gate_runner::bounded_caps_wrap_the_nextest_form, shared_target_dir::meta_spec_release_carriers_are_pinned_per_carrier — the pin follows the re-keyed carrier, T178's LOOP-SPEC half already pinned 280). kimi REQUIRED PASS (23/60 iters, ~10 min): gates independently re-run (spec check line green incl. both greps, clippy -D warnings, nextest --release 1433/1433 under the doctrine's own alarm-280 wrap), 4 mutants serial in-tree (overlap declared — all touch the same META-SPEC paragraph): M1 alarm-600 restore KILLED (both pins + both greps), M2 timeout-600 restore KILLED (both pins; grep legs alone blind — the check line's timeout leg is covered by the cargo-test leg, observation), M3 rule-sentence drop SURVIVED (observation — the sentence is unpinned; the pins protect the cap VALUES, not the rule statement), M4 alarm-300 corrupt-value KILLED. Tree byte-clean post-mutants. Child economy: impl 21/80 in ~20 min, validator 23/60 in ~10 min — zero budget deaths, zero resumes; three deferrals (cycle-82 handoff, cycle-83 eval, cycle-89 wrap) were pure queue position, not difficulty. Post-merge gates 1433/1433 in target-shared-main + clippy clean.

### Cycle 88 (2026-10-02, 00:54 UTC–, continued as cycle-89 run starting 05:03 UTC) — glm routine freshness-skip cycle (freshness held: EVALUATION.md touched 00:37 UTC same day + 7 todo rows) — COMPLETE

- **T194 landed** (merge 0fad6c7, impl f9f5340). The 3-child fleet + parallel validators doctrine: LOOP-SPEC Phase 2 cap amended T161's ≤2-children/≤1-validator to ≤3-children/≤2-validators with Pipeline-overlap pattern (iii) — a 3rd impl child flies only when all three items' spec-named target-file lists are PAIRWISE disjoint — and pattern (iv) — a 2nd validator flies only when two items are simultaneously past gates, never two on the same item, always kimi. Validator target dirs split into slot dirs target-shared-validate-a/b (two validators sharing one dir would serialize on cargo's build lock); a third impl slot impl-c added beyond the spec's letter (the ≤2-slot rule ignored a held impl-b on relaunch — impl-c keeps 'always a slot no in-flight impl holds' true for the fleet). Resource governor as a Hard rule: ≤4 cargo-heavy children, validators win under memory pressure, degradation recorded via decision_log. Pins: tests/loop_spec_fanout.rs (10 tests) + shared_target_dir.rs re-key. kimi validator: all 5 spec reqs verified at every carrier, 10/10 T79-parallel mutants killed by targeted pins, 5 non-blocking findings (worst: governor ≤4 is spec-mirrored and vacuous under the ≤3 cap — it binds only if the child cap later rises). Impl child died 80/80 wrapping up with work committed → T63 orchestrator-finish (no resume burned). Post-merge gates 1362/1362 + clippy clean.

- **T189 landed** (merge 7074afe, impl 5ca8a43). The low-stakes validation lane: LOOP-SPEC §2 step 4 gains the mechanical gates-only predicate — gates-only when ALL four hold: (a) no core-list file touched (driver/api/tools/events.rs; any LOOP/META/META-META/SELF-SPEC edit stays FULL), (b) ≤ ~150 changed lines by `git diff --numstat`, (c) no new tool/command surface (schema enum, CLI flag, MCP tool, hook event), (d) no CI/workflow or `check:` line change; any one flipped → full adversarial. Inputs are diff-computed (src/valroute.rs: parse_numstat incl. rename rows, surface markers on added lines only, check:-header exclusion); the routing record names all four inputs; the lane skips the kimi child NEVER the gates; gates-red lane-eligible diffs still get fix-up children. `--validate`/`--no-validate` operator overrides wired into `run --auto-spec` + `quick`, recorded. kimi PASS: m1-m4 mutants killed (core-direction flip, 150 boundary, surface marker, fix-up needle), m5 survivor LOW (cmd_run override-record call site lacks an e2e test; function-level pins hold — carried, noted for a future eval), core-list scope finding (doctrine's clause (a) uses the 4-file list; the spec's repo-context named a longer list — internally consistent, fails toward FULL, blessing deferred to next eval). Impl arc: 80/80 death uncommitted → T63 resume (5ca8a43) — resume count now 2 this cycle. Post-merge 1379/1379 + clippy clean.

- **T186 landed** (merge 634c205, impl 051e447 + orchestrator clippy fixup bff6630). LOOP-SPEC step 5 sharpened per the cycle-84 losses: (1) harvest-ALL mechanics — copy every `.chug/events*.jsonl` in the worktree (live stream AND each rotated segment, T10/T7 rotation named), one harvested file per source file, each named per the segment(s) it actually contains, `impl-validate` combined-name ban for single-segment files; (2) removal precondition exact in BOTH directions — no child pid launched in the worktree may be alive (liveness from `delegate status`/`kill -0`, truncated-`ps` ban, pid 6260 named) AND a bare ps/kill-0 hit must exclude defunct-Z (`ps -p <pid> -o stat=`); (3) Phase-3 wrap bullet matches. 3 new exactly-once pin legs in loop_spec_recovery.rs, RED-proofed in-commit. Impl died at the 50-min wall 60/80 committed-complete → orchestrator-finish (d1790921090-3); the independent clippy gate found `useless_concat` on TRUNCATED_PS_BAN (the T166 class AGAIN — 3rd sighting; fixed on-branch bff6630, string byte-equivalent). kimi REQUIRED PASS (d1790921297-4 routing, d1790922047-5 verdict): gates 1405/1405 zero flakes + clippy clean re-run, all 17 needles independently grep-verified exactly-once, 5/6 mutants killed serially (M5 drop-a-test uncaught — informational, the spec mandates redness on sentence reverts which M1–M3 prove), tree byte-clean. Validator process WEDGED post-goal_complete (14 min silent, verdict fully rendered in log) → read-back-first kill (the T186 kill rule exercised); its own doctrine then governed the worktree removal (defunct-Z check exact both ways). The 50-min wall: 1 of the next-8 census so far (T173 measure clause live).

- **T192 landed** (merge da366df, impl 004b6cb — flipped at cycle-89 tail). F14 PHASE 2 CLOSED: live-context editing, the CLM port. The driver mirrors the post-system-prompt message list to `.chug/LIVE_CTX.md` as `[[CTX_TURN i role=...]]` blocks pre-call each iteration; the model edits it with its ordinary file tools; an accepted parse-back — whole blocks, pinned content untouched, strictly smaller in tokens — splices the transcript and records `[ctx-edit: ~Nk tokens, M turns]` markers (T77's resume-identity discipline); edit-only turns are FREE (≤3 consecutive, 4th counts, `--max-tokens` always binds); one-shot `--ctx-warn-at-tokens` nudge (default 0 = off); `Event::CtxEdit` lines for the T184 digest. src/live_ctx.rs 1142 lines + driver/tests/live_ctx.rs 653. ARC: impl died 80/80 twice (fast-proxy ~7.5-min run 1; ONE T63 resume died 80/80 with the feature functionally complete) → orchestrator-finish with three gate fixes (README T95 module-list guard + the spec's req-6 bullet; the nudge test's position asserts vs the T13 budget-low co-fire; the free-turn test's CtxEditorLlm script bug — the drops queue is served BEFORE the responses queue, so `.then_read_big_file` was unreachable and call 6 re-read instead of calling goal_complete). kimi REQUIRED PASS (d1790924978-9 routing, d1790926020-10 verdict): gates re-run 1433/1433 + clippy clean, ALL 7 mutants caught (shrink-gate-flip, pinned-drop, free-turn-cap-4, parse-order-drop, pairing-drop, classify-bash-readonly, nudge-unlatched), tree byte-clean; the validator died 60/60 post-verdict pre-announcement — verdict recovered from the log (4th of the unannounced-verdict class; the validator measure census now reads: 2 of last 9 at 60/60 unannounced). Findings: 1 weak-test (non-blocking — `token_budget_binds_on_free_edit_turns`'s scripted edits are actually rejected, so no genuinely-free turn exercises the usage-accumulation skip; carried for the next eval) + 3 informational (chat-mode mirror in-scope-but-out-of-spec, plan-mode never mirrors, MIRROR_WARNED process-static). F14's value claim is NOT asserted — T184's cache-token telemetry is the arbiter.

- **T191 landed** (merge 69c4b4d, impl 7b10652 — flipped at cycle-89 start after a mid-arc segment death). Runbooks/: five one-page operator runbooks (quick-task, feature, repo-eval, loop-ops, adversarial-review) + index + a README Runbooks section integrated between Validation and Interactive mode. Docs-only (+304/-1, 8 .md files) → kimi SKIPPED per the docs-only optional rule (routing d1790914438-12), but the T67 duty was done for real: every load-bearing command executable-text-verified — quick/ledger/--validate/--no-validate against a CURRENT-source build (the repo ./target binary was stale vs the T189 merge — caught and avoided), loop-ops HALT/3-strikes/goal-less claims against loopd.sh lines 124/150/279/318/369. Reduced gates 81 green in the worktree (todo_consistency 19, shared_target_dir 33, loop_spec_docs_only_gates 8, loop_spec_validation_lane 6, install_sh 15), post-merge guard floor green in target-shared-main. The 2-impl overlap (T190 solo-default + T191 impl-a, d1790911785-10) worked: disjoint files, no build-slot contention, T190's merge landed first per serial-merge order.

- **T190 landed** (merge 75158e8, impl d7843c2 — flipped at cycle-89 start after a mid-arc segment death). Completion notifications (operator ask 2026-10-01: async-first needs a nudge): opt-in `.chug/notify.json` (absent/disabled = ZERO behavior change; malformed = off + one T90-style warn), fire-and-forget machinery mirroring SPEC-8 — bounded channel, drop-on-full, one flusher thread, drain-capped Drop so the terminal notice lands without stalling exit. Sinks: osascript `display notification` + layad POST (push/silent judgment stays layad's). Fires from the CHILD chug process in run_loop (loopd forwards nothing; plan/chat unwired by design): goal-complete w/ summary, abort w/ cause+model+budget, validation verdict w/ item+PASS/FAIL; short-run goal suppression via min_duration_secs (aborts/verdicts never suppressed); delivery failure counted + noted EXACTLY ONCE per run as a `notify_error` events line, run never affected. New `Event::ValidationVerdict {item, passed}` emitted at the verify() legs (blocked check = FAIL — a blocked check verified nothing), console/TUI silent. `release` event name accepted + Kind::Release plumbed end-to-end with NO in-repo emit point (the release tag push lives in the orchestrator's release flow — documented deviation, validator-confirmed). Arc: impl died 80/80 (~8 min, unusually fast proxy) UNCOMMITTED → ONE T63 resume → died at the 50-minute wall 49/80 with work COMPLETE+COMMITTED → orchestrator-finish per T55/T63 (resume cap burned, no second resume). kimi REQUIRED PASS (d1790915606-14 routing, d1790916846-15 verdict): gates re-run 1402/1402 + clippy -D clean, 5/5 T79-parallel mutants KILLED (disabled-path-fires, unbounded-channel, abort-suppression, note-once-latch, blocked-check-PASS), test-quality leg 23 discriminating legs no vacuous greens; 2 minor findings (dual-cause notify lines latch, cross-thread events interleave theoretical in degraded path) + 1 cosmetic carried as hardening candidates. Post-merge nextest release green in target-shared-main.

**Cycle-88/89 wrap notes.** Worked in queue order: T194 → T189 → T190+T191 (the 2-impl overlap, serial merges) → T186 (doctrine SOLO) → T192 (the F14 phase-2 feature pull). **Deferred with reason**: T187 (META-SPEC alarm 280 — pri 4, doctrine SOLO, ~105-min arc vs 73 min left at the 09:03 deadline; spec cold-start-ready, carried 3x; the next cycle should run it FIRST). Queue is handoff-clean: one open row, spec ready, SOLO. **Release v0.13.0 tagged at wrap** (e66f2e6 bump + tag; pairing verified scripts/check-tag-version.sh; notes = scripts/release-notes.sh HEAD v0.12.0 — 6 items since v0.12.0: T194, T189, T190, T191, T186, T192-minor-feature + the F14 PHASE 2 check-off in FEATURES.md). Final gates at tagged HEAD under target-shared-main: nextest release 1433/1433, clippy -D clean, build OK. **Child-economy census (the T173-era measure clauses)**: impl children 6 dispatched (T194, T189, T190, T191, T186, T192-run1) + 3 resumes — deaths: T194 80/80 committed (orch-finish), T189 80/80 uncommitted (resume→landed), T190 80/80 uncommitted → resume → 50-min wall committed (orch-finish), T186 60/80 committed (orch-finish), T192 run1 80/80 uncommitted (~7.5 min, fast-proxy) → resume died 80/80 functionally-complete (orch-finish). The 50-min wall caught T190 (1 of the next-8 census: 1). **Validators**: 4 REQUIRED kimi runs (T194 10/10, T190 5/5, T186 5/6, T192 7/7 — 27/28 mutants era-wide, zero blocking findings, all first-round PASSes) + 1 docs-only skip (T191). T192's validator died 60/60 POST-verdict (recovered from the log, no resume burned) — the unannounced-verdict class census: 2 of the last 9 (t173 measure: >1 of next 8 → trim mutation legs next eval; NOT yet tripped at 2). **Incidents**: T186's validator WEDGED post-goal_complete (14 min silent, verdict fully rendered) → read-back-first kill — the T186 kill rule exercised before its own doctrine even landed; T192's fast-proxy produced 80 iterations in ~7.5 min (twice) — the iteration ceiling, not minutes, was binding. **Harvest inventory** (all in .chug/): events-t194-* (2 segments), events-t189-* (2), events-t190-* (2), events-t191-impl, events-t186-impl-2seg + events-t186-validate + LEDGER-t186-validate, events-t192-impl-2seg + events-t192-validate + LEDGER-t192-validate — the T186 harvest-all doctrine was LIVE-TESTED on its own worktree mid-cycle and held (both segments of every multi-segment file recovered).

### Cycle 87 (2026-10-02, 20:52–00:50 UTC) — glm routine freshness-skip cycle (freshness held: EVALUATION.md cycle-85 same UTC day + 7 todo rows; skip record d1790888022-1) — 1 landed (T188 after a 4-round arc), T186/T187/T189/T190/T191/T192 unworked (specs ready; budget went to the T188 validation tail)



**T188 done 61aa90c (merge of loop-t188: 4e0a66c impl + 7377425 docs + fe584fb r2-fixup + 5c81faa r3-fixup + e2779c6 r4-drain-pin; routing/recovery records d1790888022-2, d1790890361-3, d1790896112-6; validation records d1790894090-4, d1790897479-7, d1790897512-8).** The auto-spec feature is LANDED: `chug run --goal ... --auto-spec` / `chug quick` drafts spec+check from a bare goal (one read-only LLM call reusing plan-mode machinery, PlanKind::SpecDraft), gates the draft with a real dry-run of its check (vacuous checks rejected, redraft once, abort honestly on a second failure — never loosened), runs against `.chug/auto-spec.md`, plus chat `/auto-spec` + `/auto-spec-approve` and the task-class doctrine sentence in README/SPEC. Cycle-87 arcs: r3 fixup died 80/80 UNCOMMITTED → ONE T63 resume with a noise-strip addendum (the child had mass cargo-fmt'd the whole repo — 83 files +12.6k/−9.2k uncommitted; the resumed child stripped it to 775 real insertions; NEW incident class for the next eval: impl-child tree-wide formatters need a goal-template ban) closing F1 (e2e mock-Llm draft→gate→run→goal_complete + m6 killers), F2 (SpecDraft wiring pins, m7 killed), F7 (parse_slash/tui/CLI entry pins). Kimi narrowed verdict (d1790897479-7): FAIL 1 — the en-route chat drain fix (ui_gone latch + one drain pass so a queued auto-spec request survives UI quit) was UNPINNED (m8 byte-exact revert survives 21/21; session-level tests race). Round-4 fixup e2779c6 factored the idle poll into `poll_step<P: IdlePollSources>` + `PollStep` with a deterministic scripted-source test; the kimi r4 round was skipped on wall budget (routing d1790897512-8, confidence 0.72 — the deviation to watch) and the ORCHESTRATOR RED-proved m8 in lieu: Idle→Drained on the disconnect leg made the new test RED, revert green, nextest 1351/1351 branch + 1352/1352 main (target-shared-main), clippy -D zero. Harvest: events-t188-round34 + LEDGER-t188-round34 on disk; worktree /tmp/chug-loop-t188 kept at wrap (T186 rule) — cleanup is next cycle's first mechanical act. Also this cycle: the 74d3331 class bit AGAIN (branch-side check re-key invisible to the delegate goal gate — check_cmd ran target-shared-impl-a; dispatch fix 82804fb puts the re-key in MAIN's spec copy).

### Cycle 86 (2026-10-01) — glm freshness-skip; 1 landed (T193), T188 in-flight on kept branch (round-2 FAIL, wall stop), rest unworked specs-ready

### Cycle 85 (2026-10-01) — kimi fresh-eval, 4 rows (T184–T187 + F14 reframe), 2 landed (T185 068e223, T184 feature F14), T186/T187 deferred; defunct-zombie liveness amendment; no tag (2 < 3 since v0.11.0)

### Cycle 84 (2026-10-01) — glm routine freshness-skip cycle (freshness held: EVALUATION.md same UTC day, 1 todo row T181; skip record d1790856978-1) — 1 landed (T181) — queue DRAINED

### Cycle 83 (2026-10-01) — kimi mandatory fresh-eval cycle (queue drained at cycle-82 wrap 7646250; T81 routing per the cycle-82 handoff) — 4 rows filed (T180–T183 + F14), 2 landed (T180, T182), T183 MID-ARC at wrap (recovery recipe below), T181 unworked — no tag (2 items < 3 since v0.10.1)

### Cycle 82 (2026-10-01) — glm routine freshness-skip cycle (freshness held: cycle-81 wrap note c4d778c landed 03:39 UTC) — Phase 1 skipped per skip-rule record d1790826770-1 — 2 landed (T179, T178), QUEUE DRAINED, release v0.10.1 tagged

### Cycle 81 (2026-10-01) — kimi routine freshness-skip cycle — 2 landed (T175, T176); T178 deferred to the next cycle (4-minute wrap wall hit first — its spec is cold-start-ready)

### Cycle 80 (2026-09-30) — glm routine freshness-skip cycle — 4 landed (T177 af4f4b4, T170 88119dc, T174 ab421d3, T171 7b9b2bf — F11 phase 1b CLOSED), doctrine bundle T175+T176+T178 deferred with reason — full narrative in git (row-flip commits + todo: flips).

### Cycle 79 (2026-09-30) — fresh eval (kimi; 8e832d2: 10 rows + 16 triage records) — 3 landed (T173 minutes-budgets 553928a, T172 load-lock deflake bae88d5, T169 F11-1b-i pull 3796983), 7 deferred with reason (minutes binding; iteration-ceiling deaths the new census class; T63 resume textbook; kill-rule confirmed x4) — one-line backfill at the cycle-84 wrap (entry absent since an earlier wrap's compaction; full text in git 8b4db7d and the todo: commits).

### Cycle 67 (2026-09-28/29) — routine (freshness-skip) — codex-intake pri-2 queue — T135 LANDED 611916e (merge + stale-flip cleanup), T136 arc mid-flight at wrap (recovered + landed cycle 68; its crash-safety class completed by T145 in cycle 70) — compacted at the cycle-73 wrap (last-6 rule; the full text lived in git until the cycle-72 eval commit dropped it — restored verbatim for 69/70/71 below, 67/66 re-compacted from git).

### Cycle 68 (2026-09-29) — routine glm freshness-skip (queue non-empty, eval fresh) — codex-intake pri-2 queue (T137 merge pickup + T138 resume + T139)

### Cycle 71 (2026-09-29) — routine glm freshness-skip (queue non-empty, eval fresh) — ALL FOUR landed (T147+T148+T149+T150), queue EMPTY

### Cycle 70 (2026-09-29) — kimi fresh-eval cycle (queue was empty) — 7 rows filed, T144 landed

### Cycle 69 (2026-09-29) — routine glm freshness-skip (queue non-empty: T129/T131 deferred from cycle 68, eval fresh) — the deferrals worked first

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

### Cycle 73 (2026-09-29) — resumed mid-arc (prior segment truncated at wrap); origin reconcile + T152 landed

### Cycle 74 (2026-09-30) — freshness-skip routine cycle; T161 (the operator's parallelization lever) landed first

### Cycle 75 (2026-09-30) — freshness-skip routine cycle; the queue drained: T160 landed, zero rows remain

### Cycle 76 (2026-09-30) — MANDATORY fresh eval (queue empty at cycle-75 wrap; kimi, loopd-routed) — 7 rows filed (T162–T168); T163 landed; cycle in progress

### Cycle 77 (2026-09-30) — freshness-skip routine cycle (EVALUATION.md fresh, 5 todo rows); T162 landed; cycle in progress

### Cycle 78 (2026-09-30) — freshness-skip routine cycle (eval fresh 09:17); BOTH queued rows landed (T167, T168), queue drained

### Cycle 79 (2026-09-30) — MANDATORY fresh eval (queue drained at cycle-78 wrap; kimi, loopd-routed) — 10 rows filed (T169–T178); 3 landed (T173, T172, T169 + the F11 1b-i roadmap pull); 7 deferred with reason

