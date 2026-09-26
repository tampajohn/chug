# EVALUATION — chug, assessed by chug-loop (2026-09-26, cycle 29)

Corpus: `.chug/eval-digest.md` FIRST (regenerated 11:09:35Z at eval start —
the running supervisor predates T46's refresh line, so this is the THIRD
manual regeneration after cycles 22 and 24; 120 event streams / 5,129
iterations), then the three orchestrator streams since the cycle-26 eval:
`.chug/events-20260926-101503.jsonl` (**cycle 26**: 120/120 iters, 23m20s,
mandatory fresh eval + T57 landed + wrap committed — then ABORTED at the
iteration budget with `goal_complete` never called; loopd logged "cycle
ended WITHOUT goal complete (consecutive failures: 1)", reset by cycle 27),
`.chug/events-20260926-103911.jsonl` (**cycle 27**: 117/120, 23m7s,
freshness-skip, T59 landed, T58 + T62 in-flight at the wrap, goal
accepted), `.chug/events-20260926-110913.jsonl` (**cycle 28**: 103/120,
28m59s, freshness-skip, both in-flight recoveries executed + T61 + T60
fresh — QUEUE DRAINED, goal accepted), the 9 child streams t57–t62 (8 of 9
goal-accepted first try; t58-impl the one death — I2), `.chug/loopd/loopd.log`
(cycles 26–28 OK unattended except cycle 26's budget abort; 0 re-execs —
T50 still dormant, I1), `TODO.md` (T1–T62 all done with refs — queue EMPTY
→ this eval mandatory per the cycle-28 wrap), git log `b9371ae` (cycle-28
wrap), `src/` (23,373 lines incl. tests; **tools.rs 3,962 now the largest
file**, driver.rs 3,607), `README.md` (305 lines — §6 audit below), and the
cycle-28 wrap's three written carries (dispositions in §2: T58 weak-test
survivor → **T64 filed**; impl-child 50/50 demand signal → **T63 filed**;
loopd restart → **I1, still pending, restated top of handoff**).

## 1. What chug does well

- **The recovery-recipe discipline is load-bearing routine now.** Cycle 27
  wrapped at budget with TWO items in flight (T58 impl-complete awaiting
  the verdict, T62 self-committed 2 min before wrap), each row carrying its
  own recipe; cycle 28 executed both zero-rework and still landed two fresh
  rows — 4 rows in 103 iterations.
- **T44 overlap #5 is unremarkable** (T60 impl ∥ T61 validator, disjoint
  files, strictly serial merges) — the machinery just works.
- **Validators keep earning their budget**: t57 13/14 mutants, t58 5/6,
  t61 5/5 — and the two survivors feed the standing survivor-class→pin
  pipeline (T54, T62 precedents) as this eval's T64.
- **The digest carried Phase 1 a fourth time** (T46's acceptance holds):
  this eval reached the writing phase at ~iteration 22 against the 45–55
  pre-T46 baseline — one corpus file read up front, raw jq only for
  targeted drills (odd error classes, the FAILED sighting).
- **glm first-try rate: 8 of 9 children** goal-accepted first try (t57
  32/50, t59 27/50, t60 17/50, t61 16/50, t62 32/50; validators 18/50,
  40/50, 26/50). The one death is I2.
- **T58 dogfood poetry**: the feature's own impl died the exact death the
  feature addresses; the row notes record it as the fifth occurrence.

## 2. Incidents worth fixing

**I1 — The live supervisor is STILL five loopd.sh revisions stale; only a
human restart activates them (human-decision carry — RESTATED, top of
handoff).** `ps` shows loopd pid 90114 started 2026-09-25 20:43:53 EDT;
`grep -c re-exec .chug/loopd/loopd.log` is still 0 (T50 cannot self-heal
the chicken-and-egg case); cycles still launch at 120 iters, not T36's
160. New evidence since the cycle-26 eval, third and fourth confirmations:
(a) the digest was STALE at this eval's start (regenerated manually —
cycles 22, 24, now 29); (b) **cycle 26 ended WITHOUT goal_complete at
120/120** (loopd.log 10:09:55Z, consecutive failures: 1) — the eval + the
T57 arc fit, the wrap's `goal_complete` did not; the dormant +40 iters
(T36) is very plausibly the difference between a counted failure and a
clean cycle. `launchctl` `KeepAlive => false` stands — **chug must NOT
restart it**; the operator one-liner is in the handoff. Post-restart
safety is unchanged: the new script's ps-check (T53) skips an in-flight
cycle cleanly.

**I2 — t58-impl died 50/50 with the work complete-but-uncommitted — the
FIFTH occurrence of the class, and the cycle-28 wrap's "demand signal"
carry (NEW ROW T63, pri 2).** Occurrences: t15/t17/t20 at 40/40 (→ T21
widened the template to 50), then t28, t47, t55, and now t58 at 50/50 —
the T58 TODO row itself records "fifth occurrence of that pattern and the
exact failure this feature addresses". Cost this time: a cross-cycle
deferral (T58 + T62 wrapped in-flight at cycle 27, landed cycle 28) —
zero work lost (T19 harvest + on-row recipes), but a full cycle boundary
of latency, and the recovery consumed orchestrator budget in TWO cycles.
The recovery primitive now EXISTS: T58 shipped `delegate` `resume: true`
+ latest-segment `status` THIS cycle-triple — but LOOP-SPEC §2 never
mentions resume; its only named recoveries are orchestrator-finish (T55
precedent) and next-cycle recovery (T28 precedent), both costlier than a
same-worktree resume relaunch, and cycle 18's actual recovery was a
hand-rolled nohup `chug run --resume` — the pattern T24 replaced. Fix
(specced): step 2 gains a budget-death recovery leg — one `delegate`
relaunch in the same worktree with `resume: true` (one-attempt cap), then
the standing recipes.

**I3 — Both cycle-triple validators left exactly one non-blocking survivor
(NEW ROW T64, pri 4, tests-only).** (a) t57-validate M5: LOOP-SPEC step-5's
ALWAYS-form survives rewording — the window assertion is satisfied by the
mechanism sentence's "ALWAYS main checkouts"
(`.chug/LEDGER-t57-validate-20260926-100640.md`). (b) t58-validate: an
over-reset of `max_iters`/`last_iteration` at `run_start` survives — the
two-segment latch-reset test (src/tools.rs:2110) covers goal/abort/budget
latches but not the keep-last-seen fields
(`.chug/LEDGER-t58-validate-20260926-064150.md`). The survivor-class→pin
pipeline (T54, T62) exists for exactly this.

**Carries disposition (cycle-28 wrap):** T58 weak-test survivor → I3(b)/T64;
impl-child 50/50 demand signal → I2/T63; loopd restart → I1, RESTATED.
Cycle-26 eval's watch items resolved or continuing below.

**Watch items (continue unless noted):** `dead_port_probe` — 0 sightings in
the 3 cycles since T59 landed (was promoted at its 3rd sighting); T61's
fire-time error string — shipped mid-cycle-28, and cycle 28's single
`path escapes cwd` fire PREDATES the merge, so the class's rate is
measurable from cycle 29 on; glm `output_truncated` advisories — 4 fires
across t57/t59/t62 impls (1+2+1), all self-recovered goal-accepted — T38's
advisory is being taken as designed; glm edit_file `old` slips — t56 ×2
(ASCII-vs-Unicode arrow), t60 ×1, each self-recovered in 1 iteration;
delegate no-verdict natural stop — 0 new; `write_ledger` hallucination —
0 new; `web_fetch` organic adoption — 0 calls across all 12 new streams
(demand-honest per T37's filing; the loop's work remains repo-local);
**tools.rs monolith** — 3,608 → 3,962 lines across the cycle-triple
(T23 +887, T58 ~+350, T61 ~+30 all land there) and the cycle-26 evaluator
burned 2 tool calls grepping a hallucinated `src/delegate.rs` — a model
expected the cohesive ~700-line delegate cluster (tools.rs:605+ and its
tests from ~:3036) to be its own module; one wayfinding miss is below the
row threshold — file if it recurs or the file crosses ~4,500.

## 3. Friction hot spots

`path escapes cwd` — 1 fire (cycle 28, pre-T61-merge); see the T61 watch
item. `todo_consistency` red MID-eval (the cycle-26 stream's "FAILED"
sighting) is the T8 guard doing its job during row authoring (rows exist
before their specs do) — the intended workflow is targeted
`cargo test --test todo_consistency` runs, and the guard goes green before
the eval commit; cost ≈ one focused run per eval, not a bug. The digest's
"===" and "my pid: N" error buckets in cycles 26/27 are benign exit-1
classifications (a grep on the missing `src/delegate.rs` — the wayfinding
miss above — and the single-driver check printing its own ps line); the
digest's first-line bucketing can't distinguish exit-1-means-differences
from errors — below the row threshold. `timed out after 120s` — 0 new (3
cumulative). No other class crosses the row threshold.

## 4. Capability gaps — FEATURE SCAN (required)

Audited against the META-META-SPEC candidate classes, post-T58 surface:
**parallel tool calls — PRESENT** (driver iterates every `tool_use` block
per turn). **`delegate` — PRESENT and now COMPLETE for the loop's needs**:
launch (spec/goal/model/budgets/`max_tokens`/`resume`), status (liveness,
latest-segment summary, `wait_secs` long-poll, zombie reap). The remaining
gap is not the tool but the DOCTRINE's use of it → **F1/T63** below.
**`web_fetch` — PRESENT**, 0 organic calls across 12 streams;
demand-honest. **MCP — PRESENT** (stdio + streamable HTTP);
resources/prompts zero demand (rejected, 3rd cycle running).
**Plan-then-execute — the LEDGER is the surface** (glm children sometimes
skip `update_ledger` entirely — t58-impl made 0 calls; harmless, the
events stream is the record). **Session/handoff — `--resume` + rotation +
`delegate resume` — complete as of T58.** **Steering depth — PRESENT**
(TUI steering line, chat dock, T13's steering-note injections).
**Fleet-driving — T44 caps at 2** (1 validator + 1 impl); the queue drains
in 1–2 cycles; no demand for wider fan-out (rejected, 2nd cycle running).
**Context management — PRESENT** (20-message tool-output trim; no
context-limit failure in any stream — cycle 26's 461k cumulative input is
a SUM over 120 full-context calls, not a window).

**F1 — the loop doesn't use its own resume primitive (NEW ROW T63, pri 2 —
the one filed row).** Every candidate class above is PRESENT; the credible
gap is capability-UTILIZATION: T58 closed the delegation surface's last
missing leg this cycle-triple, and the doctrine that drives every child
launch never names it (I2 has the five-occurrence demand record). Features
are first-class and this is the adoption half of T58's feature — the same
shape as T24 (LOOP-SPEC adopts `delegate` after T23 shipped it).

Other candidates weighed and rejected: token-budgeted impl children via
T39's knob (the deaths are iteration-ceiling, not token; recovery is
cheap); impl max_iters 50→60 (T21's lever again — HOLD while T63's cheaper
recovery lands; revisit if deaths continue post-T63); `src/delegate.rs`
extraction (watch item, §2 — one wayfinding miss below threshold); MCP
resources/prompts, web_fetch POST/HEAD, delegate fleet view, plan-file
mode — zero demand, recorded so next cycle doesn't re-litigate.

## 5. Top 3 priorities

1. **HUMAN: restart the supervisor (I1)** — caps every cycle at 120/160
   (cycle 26 died 120/120 with the wrap committed but `goal_complete`
   uncalled) and leaves the digest stale at every eval start; 30 seconds,
   zero risk — the new ps-check skips an in-flight cycle cleanly.
2. **T63 (pri 2, doctrine/robustness)** — adopt `resume: true` as the
   standard budget-death recovery; the fifth occurrence landed this
   cycle-triple and the primitive is already shipped and documented.
3. **T64 (pri 4, tests-only)** — pin the two validator survivors before
   the classes accumulate (T54/T62 pipeline); cheap.

## 6. README audit (usability, not just accuracy)

Cold read top-to-bottom (305 lines). **(a) Reading order:** what-it-is →
quickstart → interactive → autonomous → TUI → tools → risk gate → MCP →
Langfuse → self-hosting specs → continuous mode → development — the
newcomer arc holds; no append-only accretion in section order. **(b)
Redundancy:** none new — `output-token ceiling` still occurs exactly once
(T56's dedupe holds); the delegate paragraph is the single source for
launch/status semantics (resume + wait_secs + latest-segment all current
post-T58). **(c) Staleness:** none found — the Development layout brace
list matches the tree (21 modules + main.rs, verified against main.rs's
mod list post-T60); the Tools intro's two-exceptions phrasing is current
(post-T41/T51); the loopd section names the re-exec behavior (T56).
**(d) Balance:** ONE new accretion spot — the Continuous-mode
target-cache paragraph (README.md:266–280) is a single ~12-line sentence
chain with three nested em-dash parentheticals (T47's export rationale +
T52's role-keying + T57's main-dedicated dir + reclaim instructions). The
audience for "never a supervisor-wide export … leaving
`./target/debug/chug` stale" is a loopd.sh EDITOR, not a README reader;
the mechanism lives in specs t47/t52/t57. **→ T65 (pri 4, docs-only).**
The MCP remote-specifics paragraph stays acceptable (it IS the user-facing
configuration contract; unchanged since the cycle-26 note). **(e)
Quickstart truth:** commands verified unchanged against main (`cargo
build`, `cargo install --path .`, flags current; `check:` behavior
accurately described; auth chain matches the code).

## Handoff — recommended execution order

**Human-decision item FIRST (blocks nothing chug-side but caps every
cycle): restart the supervisor.** `launchctl kickstart -k
gui/$(id -u)/com.tampajohn.chug-loopd` (or `./loopd.sh stop` + the
operator's usual start). Activates T36 (160-iter cycles — cycle 26 died
120/120 with the wrap committed but `goal_complete` uncalled), T46 (digest
refresh — stale at 3 of the last 4 eval starts, regenerated by hand each
time), T47 (warm shared cache), T50 (self-re-exec — after this restart the
pileup can never recur), T53 (ps-based skip). KeepAlive=false means chug
must never kill it (I1). Post-restart, an in-flight cycle is skipped
safely by the new ps-check.

Queue order (bugs > robustness > features > DX):

1. **T63 — pri 2, DOCTRINE item: runs ALONE** (LOOP-SPEC edit — T44
   forbids overlap for doctrine), REQUIRED kimi validation (loop/spec
   doctrine). Files: LOOP-SPEC.md + new tests/loop_spec_recovery.rs.
2. **T64 — pri 4, tests-only** (tests/shared_target_dir.rs + src/tools.rs
   test module): kimi round optional per T16/T31/T59/T62 precedent;
   orchestrator gates + the two named-mutant non-vacuousness runs suffice.
   Runs strictly after T63 (doctrine never overlaps).
3. **T65 — pri 4, docs-only** (README.md Continuous-mode paragraph;
   tests/shared_target_dir.rs only if its pinned tokens must move): kimi
   skipped per T16/T31/T35/T51/T56/T60. Spec-named files are DISJOINT from
   T64's (README.md vs tests/shared_target_dir.rs + src/tools.rs) UNLESS
   the pin file must move — check the impl's diff before overlapping; if
   disjoint, T65's impl MAY overlap T64's gates window under T44. Merges
   stay serial in queue order regardless.

All three specs are written and ready; a cold next cycle needs zero human
words. If the queue outlives this cycle's budget, unworked rows stay
`todo` with specs — that is a fine outcome.

## Outcomes (filled at cycle wrap — LOOP-SPEC Phase 3)

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

### Cycle 27 (2026-09-26, ~06:15–06:40 EDT) — freshness-skip; T59 landed; T58 + T62 IN-FLIGHT at budget wrap (recovery recipes on their rows)

**T59 — mcp_http dead_port tests: bounded retry on port-theft (pri 3,
robustness, tests-only) → done `062c175` (fast-forward merge).** glm impl
27/50 goal-accepted ~8 min first-try: `dead_port_retry_with` driver
(acquire/attempt seams, bounded 3 attempts, exhaustion panic names the
attempt count + the T31 theft mechanism), `check_dead_port` non-panicking
re-verify seam, and a `theft_or_regression` discriminator — a connect
against a handout that STILL refuses is a real regression and panics
immediately, un-retried (never retried into a flake-shaped message); only
a now-live port is retryable theft. Both mcp_http connect-phase consumers
converted (`dead_server_retries_then_tool_error_without_sleeping` whole
body; the probe test's final acquire-verify leg = the sighted t48/t55
flake). 2 unit pins: scripted real-thief theft succeeds at exactly
attempt 2 (non-vacuous: the pre-T59 single-attempt shape cannot pass);
all-attempts theft exhausts at exactly 3 with the mechanism named
(catch_unwind). `assert_dead_port` behavior untouched (webfetch's T31 leg
keeps it). No sleeps — retry-on-theft, T31 doctrine. All hunks inside
`#[cfg(test)] mod tests` (diff --stat: src/mcp_http.rs only, +250/−47).
kimi round skipped per the row's T16/T31 tests-only precedent;
orchestrator gates independently re-run 507/507 (475+6+9+1+13+3) +
clippy clean by the child. 2 artifacts harvested pre-removal (impl
events + LEDGER). The last known organic flake in the suite is closed.

**Wrap (budget_low at 8 iters; main pushed through `f73cd70`, final
gates green there — 507/507 post-T59-merge under `target-shared-main`,
main unchanged since).** **In-flight, worktrees PRESERVED with recovery
recipes on their TODO rows:** **T58** — glm impl died 50/50 (iteration
budget) with the work COMPLETE but UNCOMMITTED (fifth occurrence of the
pattern: t15/t17/t20/t55/t58 — the exact failure T58's resume feature
addresses; the demand signal is now overwhelming — next eval should
weigh a larger impl-child iteration default or auto-resume-on-budget).
Orchestrator finish per the T55 precedent: full diff review (all 5 reqs
+ all spec'd test legs present, e2e via the stub harness), committed
`b189517` on `loop-t58`; orchestrator gates independently re-run 516/516
+ clippy clean. The REQUIRED kimi validator (pid 20915,
`target-shared-validate` per T52) was mid-mutation-phase at wrap — its
verdict lands in the preserved worktree's `.chug/`; doctrine forbids
merging src/tools.rs without it, so the merge defers to cycle 28.
**T62** — glm impl (pid 21055) at iter 27/50 in gates phase at wrap; its
commit (or orchestrator finish) + review + merge also defers. **Deferred
`todo` with ready specs:** T60 (pri 4 docs), T61 (pri 4, strictly after
T58 merges). **Cycle notes:** (a) T44 planning catch — T58 and T60 BOTH
name README.md (T58 req 5's delegate paragraph, T60's layout line), so
the originally-planned T58-validator ∥ T60-impl overlap was FORBIDDEN;
T62 (tests/eval_digest.rs only) was the disjoint partner instead. The
disjointness check must enumerate both specs' FULL file lists, not just
headline files. (b) The freshness rule fired cleanly; T59's full arc
cost ~25 min wall (worktree → impl 8 min → review/gates → merge → flip
→ push). (c) 5 artifacts harvested at wrap (t58 impl events+LEDGER from
the rotated archive — the validator's fresh run rotated them, a clean
per-role split; t58-validate + t62-impl live-stream snapshots named
`-inflight-` so cycle 28 knows the FINAL streams are in the worktrees).
(d) Freshness fires again for cycle 28 (this eval is same-day + todo
rows exist) — it resumes T58's verdict + T62's landing, then T61
(after T58 merges), then T60. (e) Human-decision carry RESTATED:
restart the stale loopd supervisor (pid 90114, 5 script revisions
stale; launchd KeepAlive=false so chug must never kill it) —
`launchctl kickstart -k gui/$(id -u)/com.tampajohn.chug-loopd`.

### Cycle 26 (2026-09-26, ~05:46–06:40 EDT) — MANDATORY fresh eval (queue empty) + T57 landed (main-dedicated gates dir); wrap at budget margin

**Fresh evaluation (committed `67374a9`).** Queue was drained at cycle
start → Phase 1 mandatory per the freshness rule. Digest-first
(regenerated 09:46:52Z; T46's forward-looking acceptance HELD: writing
reached at ~iteration 18 vs the 45–55 pre-T46 baseline). Filed 6 rows
T57–T62 with specs. Dispositions: T55 validator finding (ii) ACCEPTED
(`acquire`'s write-then-verify loop already mitigates the non-atomic
window; documented in eval §2); the cycle-19 T39 LEDGER mystery CLOSED —
the impl made zero `update_ledger` calls, so the worktree ledger was
still SEED and T3's archive_stale correctly skipped it (no bug).
PROMOTED human carry: live loopd pid 90114 is five script revisions
stale (T36/T46/T47/T50/T53 dormant — cycles still launch 120 iters, 0
re-execs, pgrep skip dead → T55's lock now turns a manual-run collision
into FAILED cycles, 3 = HALT); launchd KeepAlive=false so chug must
never kill it; the operator restart one-liner is in the eval handoff.

**T57 — post-merge + final main gates use a main-dedicated target dir
(`target-shared-main`) (pri 1, doctrine) → done `2f8bbe4`.** glm impl
32/50 first-try (~7 min, `1c2beee`): LOOP-SPEC step-5 post-merge gates
and Phase-3 final gates now ALWAYS run under
`CARGO_TARGET_DIR=.../target-shared-main` (step-3 worktree-review gates
keep their T52 `target-shared`/`target-shared-gates` split) with the
mtime-freshness mechanism + the T55 false-red receipt in-line;
`.gitignore` gains the dir contiguously; README's target-cache clause
gains it integrated; 4 new pins in `tests/shared_target_dir.rs` (13
total; the T52 pins byte-untouched — validator-verified). kimi VERDICT:
**PASS** 18/50 — all 5 reqs verified, gates independently re-run green
(505) under `target-shared-validate`, 13/14 mutants killed; 1 weak-pin
survivor (M5: the step-5 ALWAYS-form assertion is also satisfied by an
adjacent mechanism sentence — pin-window imprecision, the doctrine text
itself correct) + 2 non-blocking coherence notes (validator transcript;
events + LEDGERs harvested). Carried to the next eval as a watch item,
non-blocking. Post-merge gates in main ran under the NEW dir — cold
build 25.7s (the spec's acceptance evidence) + clippy + full suite green
(505). 4 artifacts harvested pre-removal (impl/validate events + both
LEDGERs). Acceptance is forward-looking: no future post-merge or final
gate run can execute a non-main binary by construction.

**Wrap.** Final gates green in main under `target-shared-main` (first
T57 dogfood: build + clippy + 505 tests). Cycle-18 double-heading merged
(carry (e), done below; the other Cycle-18 grep matches are backtick
references in the cycle-25/26 notes, intentional). Bulk compaction of
cycles ≤19 DEFERRED to cycle-27 (budget margin at wrap; hygiene, same
disposition as cycles 19/20/25 — size bound not yet critical at ~640
lines). Deferred rows, all `todo` with ready specs: T58 (pri 3 feature —
delegate resume), T59 (pri 3 — dead_port retry), T60 (pri 4 docs), T61
(pri 4 — error string names bash, strictly after T58), T62 (pri 4 —
digest golden pin); recommended order + overlap plan in the eval
handoff. Human-decision carry RESTATED at top of that handoff: restart
the supervisor (`launchctl kickstart -k
gui/$(id -u)/com.tampajohn.chug-loopd`).

### Cycle 25 (2026-09-26, ~05:21–05:45 EDT) — freshness-skip; T55 recovered + landed (driver.lock feature) + T56 landed (README) — queue DRAINED

**Landed:**
- **T56 — README: loopd self-re-exec documented + output_truncated gloss deduped** (pri 4, docs-only; impl `34554e4` glm 17/50 goal-accepted ~2.5 min first-try — recovered instantly from 2 edit_file errors caused by an ASCII-vs-Unicode-arrow mismatch; merge `611af46`; flip + this entry per T34). Continuous-mode section gains one integrated sentence in the state/pidfile/HALTED paragraph: between cycles the supervisor fingerprints its own script and re-execs itself when the file changed, so merged loopd edits activate without an operator restart — a pending stop still wins (no cksum/same-pid mechanism detail, per spec). Events-log bullet drops the repeated `output-token ceiling` mechanism gloss — the Truncated-output advisory bullet stays the single definition (the string now occurs EXACTLY once); everything else byte-identical (2 hunks, README only). Docs-only → kimi round skipped per T16/T31/T35/T51 + the cycle-24 handoff plan; orchestrator gates independently re-run 501 + clippy, spec check verbatim PASS (ceiling-count==1, re-exec present, 'one exception' absent). 2 artifacts harvested pre-removal (impl events + delegate log; LEDGER trivial).
- **T55 — .chug/driver.lock: same-cwd mutual exclusion for concurrent chug runs** (pri 2, FEATURE — in-harness layer of the defense-in-depth: doctrine → loopd skip → this lock; recovered per its own row recipe from the cycle-24 preserved worktree: glm impl pid 55328 had died 50/50 with the work complete but UNCOMMITTED and 3 compile/clippy fixes short — `drive_turn`→`drive_loop` + the `initial_spec: None` arg in the chat-exemption test, clippy `single_match`→`if let` in acquire, one needless borrow; orchestrator finished, impl `cb5906f`; merge `2e7945b`; flip + this entry per T34). New `src/driver_lock.rs` (495 lines): `holder_status` pure decision matrix behind injectable `pid_alive`/`argv_names_chug` probe seams (T28/T20 style); refusal ONLY on the alive+chug double-positive (kill(pid,0) AND `ps -o command= -p <pid>` names chug — NEVER pgrep, cycle-24 eval I1); every other leg (dead pid, argv mismatch = PID-reuse, malformed/empty/unreadable, probe error) degrades to reclaim, never aborts (T20 never-fail); RAII Guard compare-then-delete release on every normal exit, SIGKILL-stale by design; acquire at run_loop top BEFORE the T3/T7/T10 rotations; chat structurally exempt (never reaches run_loop). 20 new tests (16 module + 4 run-path incl. real-sleep reclaim, SIGKILL-stale reclaim, release-on-goal-acceptance + same-cwd successor, chat never-creates-never-removes). kimi VERDICT: **PASS** 45/50 (target-shared-validate per T52 ALWAYS; budget_low@8 fired at 42, verdict delivered inside the T18 margin) — all 9 reqs hand-verified, gates independently re-run green, 6 mutants: 4 KILLED (drop-argv-leg incl. the run-path pin, drop-compare-then-delete, remove-acquire-call → 2 run-path tests, acquire-in-drive_loop → chat pin has teeth), 2 SURVIVORS in the refuse-wiring/refusal-message cluster that the spec explicitly delegates to hand-verification — which the validator then performed with the REAL binary: live chug-argv holder → second run exit 1 + pid/remedy on stderr + zero transcript/events writes + holder unaffected; stale dead-pid lock → reclaimed <1s. 3 non-blocking findings carried to the next eval: (i) module-comment says a static pin in `tests/` guards chat, the pin is actually a unit test (doc drift); (ii) acquire's read-then-write window is non-atomic (latent, spec-silent — bounded 3-attempt re-verification mitigates); (iii) PRE-EXISTING mcp_http dead_port_probe flake under parallel load (passes isolated 3/3 — the T31 watch item). **Post-merge gates incident:** first main gate run showed loopd_reexec 3/4 FAILED — a T52-class FALSE RED: the step-3 worktree gates (target-shared, no child in flight) had compiled the worktree's PRE-T53/T54 4-test loopd_reexec.rs into the shared artifact slot; the merge didn't touch that file (old mtime) so cargo reused the stale binary against main's ps-based loopd.sh (needle mismatch). touch+rebuild recovered (T43's incident class, second sighting — now SEQUENTIAL not just concurrent): main 501 (473+6+9+1+9+3) green + clippy clean. 3 validate artifacts harvested pre-removal (impl's 3 harvested cycle-24).

**Cycle notes:** (a) both queued rows landed, queue DRAINED — the freshness rule CANNOT fire next cycle (no todo rows): next cycle runs a MANDATORY fresh Phase-1 eval. (b) T55's post-merge false red is a NEW VARIANT of the T52/T43 stale-artifact class — SEQUENTIAL, not concurrent: step-3 worktree gates compiled the worktree's pre-T53/T54 4-test loopd_reexec.rs into the shared slot; the merge didn't touch that file (old mtime) so cargo's mtime freshness reused the stale binary against main's ps-based loopd.sh. Role-keyed dirs govern CONCURRENT builds only. Carry to next eval: post-merge main gates need a staleness flush (touch tests/ or a rebuild pin) whenever the worktree gates ran first in the same shared dir. (c) T55's kimi validator found the refuse-wiring/message mutant cluster untestable from the suite — spec-sanctioned hand-verification, which it performed with the real binary (exit 1 + pid/remedy, zero appends, holder unaffected; stale reclaim <1s); its 3 non-blocking findings (module-comment tests/-pin drift, non-atomic acquire window spec-silent, PRE-EXISTING mcp_http dead_port_probe flake — T31 watch item, passes isolated 3/3) are carried to the next eval. (d) T56's full arc cost ~7 min wall (worktree → impl 2.5 min → review/gates → merge → flip) — the T45-class fixed cost on a 2-hunk docs row remains the queue's dominant per-row overhead. (e) T43 cycle-18 double-heading compaction DEFERRED again (budget wrap): two `### Cycle 18` headings in Outcomes need one careful manual merge — hygiene, not loss, full narrative in git. (f) Final main gates 501 (473+6+9+1+9+3) + clippy green at the T56 flip; todo_consistency 3/3 green pre-commit.

### Cycle 24 (2026-09-26, ~04:57–05:19 EDT) — MANDATORY fresh eval (queue was EMPTY); T53+T54 bundle landed; T55 mid-arc at budget (recovery recipe on its row); T56 deferred unworked

**Deferred:** **T55** (pri 2 feature) — glm impl died 50/50 with the work UNCOMMITTED in the preserved worktree /tmp/chug-loop-t55 (README.md, api.rs, driver.rs, main.rs modified + new src/driver_lock.rs); 3 artifacts harvested (events/LEDGER/delegate.log 20260926-091931); full recovery recipe on the TODO row. **T56** (pri 4 docs) — never dispatched (budget); row stays `todo` with its ready spec. **Cycle notes:** (a) the eval's live I1 reproduction is the cycle's payload — pgrep's blindness is persistent (9/9), process-specific (in-tree nohup'd sleep matched), and its enumeration flaps — the ps fix landed in T53 within the hour; (b) T45's bundle rule executed for the first time: one child, two commits in queue order, one kimi round, one flip commit — arc cost ~7 min wall vs two full serial arcs; (c) T44 overlap executed for the fourth time (T55 impl during the bundle validator, disjoint files); (d) the bundle validator's live smoke doubled as independent confirmation that the new ps pipeline sees the exact process class pgrep missed (the orchestrator itself, pid 37073, ONLY match); (e) wrap fired at budget_low@8 with the T55 impl's abort arriving mid-merge — the per-item Outcomes discipline (T34) meant T53+T54's narrative was already committed before the wrap; (f) T56's deferral leaves the README loopd section silent on re-exec for one more cycle — harmless, the spec is ready. **Final state:** main pushed through the wrap commit; gates 481 (453+6+9+1+9+3) green at the T53+T54 merge, `bash -n` clean; todo_consistency 3/3 green pre-commit; QUEUE: T55 in-progress (recipe on row), T56 todo — next cycle skips Phase 1 (this eval is same-day) and resumes T55 from the worktree. The standing restart-loopd carry is now self-terminating (T50 merged; the next restart is the last manual one — and T53's ps-based guard rides with it).

**Landed:**
- **T53 — loopd single-driver check goes ps-based (pgrep dead on this host)** (pri 1, BUG; T45 bundle with T54 — conjunctive eligibility verified at eval: ≤30 lines each, same 2 files, no core loop, pri ≤3; impl `5c842a6` glm 24/50 first-try goal-accepted; merge `8624078`; flip + this entry per T34). The guard (`pgrep -f "chug run --spec LOOP-SPEC.md"`) was dead code on the production host — the cycle-24 eval reproduced the blindness LIVE 9/9 (pgrep -f/-l/-P all miss the launchd-spawned loopd tree: the orchestrator pid 37073 and supervisor pid 90114 absent from pgrep's full list while `ps -ax` showed both, argv intact; enumeration also flapped for other processes; the skip line fired exactly once in supervisor history, against an out-of-tree driver) — so the single-driver guard failed OPEN. Fix: `ps -ax -o command= | grep -q "[c]hug run --spec LOOP-SPEC.md"` (bracket idiom excludes the grep's own argv; supervisor argv holds no needle; chat doesn't match by design), skip body (log line/sleep 120/continue) byte-identical, why-comment citing eval I1; tests/loopd_reexec.rs: driver_check anchor re-pinned (T50 positional assertion preserved verbatim) + 2 pins (pgrep form GONE — a revert fails the suite; bracket idiom present + needle exactly-once). kimi VERDICT: **PASS** 24/50 (target-shared-validate per T52 ALWAYS) — gates independently re-run 481 (453+6+9+1+9+3) + clippy + `bash -n`, 5/5 T53 mutants killed (restore-pgrep, drop-bracket, corrupt-needle, delete-pipeline, duplicate-block), live smoke independently re-demonstrated (real driver pid 37073 matched as the ONLY match; negative leg exit 1), worktree restored byte-clean, both spec checks PASS verbatim. Post-merge main gates 481 + `bash -n` green. Bundle artifacts harvested under `t53t54` names pre-removal.
- **T54 — loopd_reexec.rs exact-count SELF_CKSUM pins (additive-mutant survivor closed)** (pri 3, robustness; T45 bundle with T53; impl `6587b22`; merge `8624078`; flip + this entry per T34). The T50 validator's non-blocking finding (cycle-23 carry ii): an additive second `SELF_CKSUM=` assignment inside the while body silently defeats the re-exec (fingerprint refreshes every cycle → while-top comparison never fires) with all 4 positional pins green — they asserted existence + order, never counts. Three exact-count pins per the T47-carrier doctrine: `SELF_CKSUM=` assignments ==1, `!= "$SELF_CKSUM"` comparisons ==1, literal occurrences ==2, each failure message naming the count + the defeat mechanism; tests-only, loopd.sh untouched. kimi VERDICT: **PASS** 24/50 covering the bundle — the ORIGINAL SURVIVOR now dies (additive 2nd assignment → pins 1+3), additive 2nd comparison → pins 2+3, delete-assignment → pin 1 + the pre-existing fingerprint pin; 8/8 bundle mutants killed total; the four pre-existing T50 pins stayed green throughout. Post-merge main gates 481 green.

### Cycle 23 (2026-09-26, ~04:08–04:56 EDT) — freshness-skip; T49+T52+T50+T51 landed (4/4 queued rows); QUEUE EMPTY → next cycle MANDATORY eval

**Landed:**
- **T49 — api.rs LLM client gains a 10s connect timeout const + value pins** (pri 2; cycle-22 impl `56ecf72` glm 34/50 + orchestrator review; this cycle: kimi validation, merge `9a73b12`, flip + this entry per T34). `CONNECT_TIMEOUT_SECS = 10` const with the class doc + `.connect_timeout` builder leg + 3-value pin test; `classify_reqwest` unmodified (is_timeout/is_connect → retryable, verified incl. reqwest 0.12.28 from Cargo.lock). kimi VERDICT: **PASS** 17/50 — all 5 reqs verified, gates independently re-run (468 passed / 0 failed), 4/4 value mutants KILLED with the panic naming the correct const; the `.connect_timeout`-leg deletion mutant SURVIVES as the spec-sanctioned evidence level (T16/T42 precedent: unobservable without a SYN-dropping endpoint; pins pin values, not wiring); tree restored byte-clean. Post-merge main gates 453+6+1+6+3 + clippy green. 2 validator artifacts harvested pre-removal.
- **T50 — loopd re-execs itself between cycles when its own script changed** (pri 2, FEATURE — closes the activation-lag gap: T36/T46/T47 sat dormant 7 cycles in the running supervisor; impl `ca11c6b` glm 37/50 goal-accepted; merge `078cb77`; flip + this entry per T34). `SELF_CKSUM` cksum fingerprint before the cycle loop (content-based — `touch`/content-preserving checkout never re-execs); compare→log→`exec "$ROOT/loopd.sh" run` as the FIRST while-body statement (re-exec only BETWEEN cycles, zero incremental-read exposure; STOP wins at the while condition, never cleared by this path); pidfile guard refuses only a LIVE pid ≠ `$$` (same-pid pass for the exec'd self; stale-pid fall-through and foreign-supervisor refusal kept); tradeoffs documented in-script; tests/loopd_reexec.rs +144 (4 positional static pins — before-the-loop, first-statement ordering, log-before-exec, guard shape + liveness). Impl ran the spec's FULL optional live smoke: re-exec line observed at the next while-top ~58s after a comment append, same pid, no refusal, STOP leg clean, scratch removed; 2 non-vacuousness mutants killed. kimi VERDICT: **PASS** 41/50 (first validator on `target-shared-validate` per T52's ALWAYS rule) — gates independently re-run 476 + `bash -n`, 8/8 spec-shaped mutants killed (exec deletion, guard revert, `!=`→`=`, fingerprint deletion, log deletion, `kill -0` drop, block relocation, log/exec swap), live smoke independently re-demonstrated (re-exec 103s after append, same pid 29636, STOP landed clean), reqs 1–6 verified, tree clean. Non-blocking: (a) hardening suggestion — an additive duplicate-`SELF_CKSUM` mutant at the while-body end silently defeats the mechanism with all pins green (exact-count pin candidate, T47-carrier doctrine); (b) observation carried to the next eval — the host's `pgrep` could not see the long-running real chug during validation while `ps` showed it (pre-existing single-driver-check exposure; loopd's skip line uses `pgrep -f`), and the live supervisor still launches `--max-iters 120` (T50's premise confirmed in the wild). Post-merge main gates 453+6+4+1+9+3=476 + clippy green, `bash -n` clean. 4 artifacts harvested pre-removal.
- **T51 — README delegate paragraph: stale "one exception" killed + spec-grade trim** (pri 4, docs-only; impl `dd024de` glm 26/50 first-try; merge `372f920`; flip + this entry per T34). The stale "Delegate paths are the one exception to cwd sandboxing" sentence (drift that moved rather than died at T41) is replaced by the short consistent clause (`cwd`/`spec` absolute, may target child worktrees outside your own); process-group/SIGHUP/nohup-parity spawn semantics and the bounded-tail-read mechanism trimmed (canonical home: specs/t23-delegate-tool.md); every user-facing semantic in req 2's keep-list verified present (two actions, args + 40/35 defaults, `--max-tokens` passthrough, returns-at-spawn, status liveness/events/log-tail, wait_secs early-return + 600s max + `waited:` line, bash owns the worktree lifecycle); README only, net −3 lines. Spec check PASS in main (`! grep "one exception" && grep "two documented exceptions"`). Orchestrator gates 453+6+1+9+3 + clippy + full diff review; kimi round skipped per T16/T31/T35 (docs-only). Landed under T44 overlap with T50's validator (disjoint files: README.md vs loopd.sh/tests); merge held strictly serial after T50. 2 artifacts harvested pre-removal.
- **T52 — role-keyed target dirs (T44-overlap × target-shared race fix)** (pri 2, doctrine, ran alone; impl `bdba301` glm 28/50 first-try; merge `4d4c383`; flip + this entry per T34). LOOP-SPEC: step-3 gates dir conditional (`target-shared` no-child / `target-shared-gates` in the overlap window incl. N+1's impl during N's post-merge gates) + the mechanism sentence (metadata hash excludes checkout path → same artifact name → last-builder-wins); step-4 validator export `target-shared-validate` ALWAYS; step-5 pointer. `.gitignore` gains both siblings contiguous (position-pinned); README T47 paragraph gains the integrated sibling-cache clause; loopd.sh + META-SPEC untouched; tests/shared_target_dir.rs +3 pins (6→9 — exact-count carriers are strictly stronger than the spec's `≥2`: a duplicated-carrier mutant dies). kimi VERDICT: **PASS** 35/50 — all 6 reqs verified, gates independently re-run 453+6+1+9+3, 10/12 mutants killed (2 survivors spec-sanctioned doc carriers: step-5 reminder sentence + README clause, same class as T44's prose survivors), tree byte-clean; the validator dogfooded the rule under test by building into `target-shared-validate` itself (auto-created, cold 27s, now warm). Post-merge main gates 453+6+1+9+3 + clippy green. 5 artifacts harvested pre-removal (impl stream was T7-rotated at the validator's reseed — both recovered).

**Cycle-level notes:** (a) **T52's role-keyed dirs dogfooded immediately** — the T50 validator was the first launched under the new ALWAYS rule (`target-shared-validate` — auto-created cold at T52's own validation, warm since); T44 overlap practiced for the third time (T51 impl during T50's validator; disjoint files README.md vs loopd.sh/tests; merges strictly serial — T50 merged before T51 despite T51 finishing first). (b) **What the validators caught:** nothing blocking — three first-round PASSes (T49 17/50; T52 35/50; T50 41/50). Mutation tallies: T49 4/4 killed (+1 spec-sanctioned wiring survivor), T52 10/12 (2 doc-carrier survivors), T50 8/8 (+1 additive duplicate-`SELF_CKSUM` survivor → hardening candidate: exact-count pin per the T47-carrier doctrine). (c) **T50 live leg, honestly recorded:** ZERO re-exec lines in `.chug/loopd/loopd.log` at wrap — expected: the running supervisor (started 00:43Z, predates T50) executes its while-loop from an already-parsed AST, so the merged re-exec code is inert in the current process; it activates at the operator's next loopd restart, and every supervisor started after this merge carries it. The cycle-22 premise stayed confirmed in the wild: this cycle still ran 120 iters, not T36's dormant 160. (d) **Carries for the next (mandatory) eval:** (i) T50-validator observation — the host's `pgrep` could not see the long-running real chug while `ps` showed it; loopd's single-driver skip uses `pgrep -f "chug run --spec LOOP-SPEC.md"`, so if the blindness is real and persistent the skip fails open (duplicate drivers) — investigate with `pgrep -fl` vs `ps` in a live window; (ii) duplicate-`SELF_CKSUM` exact-count pin hardening; (iii) T51's intro-line reflow liberty (req 4's "no other section" read strictly) — noted, no action. (e) Budget: wrap fired at budget_low@8 again — 4 items + one T44 overlap in ~48 min inside the 120 ceiling; T36's 160 stays dormant (see (c)).

**Final state:** main = `9301652` (merges: T49 `9a73b12`, T52 `4d4c383`, T50 `078cb77`, T51 `372f920`; flips `8079516`/`e50d1dd`/`abcc174`/`9301652`); gates 476 (453+6+4+1+9+3) + clippy --all-targets + `bash -n loopd.sh` green in main; TODO truthful — **QUEUE EMPTY** (T1–T52 all done with refs) → the freshness rule cannot fire → the next cycle MUST run a fresh Phase-1 eval; 13 child artifacts harvested into `.chug/` (t49 validate ×2, t52 ×5, t50 ×4, t51 ×2); all five loop worktrees removed and branches deleted (all merged, `git branch --merged` verified); README gate satisfied (T50's behavior change is documented in-script + its spec; T51's trim kept the Tools section truthful); pushed through `9301652`. **Handoff: mandatory fresh eval; carries in (d); T50 activates at the operator's next loopd restart.**


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
