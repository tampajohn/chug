# EVALUATION — chug, assessed by chug-loop (2026-09-26, cycle 31)

Corpus: `.chug/eval-digest.md` FIRST (STALE at eval start — regenerated
manually 12:06:55Z, the FOURTH manual regeneration after cycles 22, 24 and
29; the running supervisor still predates T46's refresh line, I1; 127 event
streams / 5,501 iterations), then the two orchestrator streams since the
cycle-29 eval: `.chug/events-20260926-112826.jsonl` (**cycle 29**: 115/120
iters, 18m11s, MANDATORY fresh eval + T63 landed, goal accepted, budget_low
first at remaining_iters=8), `.chug/events-20260926-120615.jsonl` (**cycle
30**: 114/120, 36m45s, freshness-skip, T64 + T65 landed, then the wrap's
own goal gate false-red on the port-theft flake → T66 filed LIVE and landed
in-cycle, goal accepted after 1 rejection, budget_low at 8), the 5 child
streams t63–t66 (4 of 5 goal-accepted first try; t64-impl the one
rejection — I2), `.chug/loopd/loopd.log` (cycles 29–30 OK unattended; 0
re-execs — T50 dormant, I1), `TODO.md` (T1–T66 all done with refs — queue
EMPTY → this eval mandatory per the cycle-30 wrap), git log `836ac42`
(cycle-30 wrap), `src/` (23,593 lines incl. test modules; **tools.rs 4,050
still the largest** — 3,962 → 4,050 via T64's +88, watch item continues),
`README.md` (cold read, §6). Per the LOOP-SPEC upgrade, first look was
`jq -r '.type'` counts over the two orchestrator streams; raw jq drills
only for the wait_secs code read (I3) and the write_file fires (§3).

## 1. What chug does well

- **The goal gate caught a real flake the review gates missed — and the
  loop turned it around in ~20 min.** Cycle 30's first `goal_complete` was
  REJECTED by `dead_port_probe_distinguishes_live_from_dead` at
  default-parallel `cargo test` (the goal-gate invocation, port 50956) —
  the first post-T59 organic sighting of the port-theft class, which four
  `--test-threads=4` gate runs that cycle had not caught. The wrap
  diagnosed (passes isolated, class signature, nothing in the cycle
  touched mcp_http), filed T66 pri 2 with spec, ran the standard child arc
  (glm 27/50 first-try), landed it, and re-attempted — 525/525 at default
  parallelism. Verification-as-designed, not ceremony.
- **Queue drained 3/3 in cycle 30, all glm one-arcs** (T64 48/50 committed,
  T65 19/50, T66 27/50) on top of cycle 29's T63 (glm 29/50 first-try +
  kimi PASS 16/50, 5/5 mutants). The glm first-try rate across the last 14
  impl children is 13/14.
- **T61's fire-time remedy string was observed in the wild — twice,
  including at this evaluator.** Cycle 29's single `path escapes cwd`
  fire carried the new suffix ("cross-tree paths go through bash") and
  self-recovered next iteration; and during THIS eval, the evaluator's own
  `write_file` to /tmp fired the error with the suffix naming the exact
  remedy (rerouted through bash, zero iterations lost).
- **The T63 resume doctrine distinguished itself correctly at its first
  opportunity**: t64-impl's budget death was complete-AND-committed, so
  the T55/T62 orchestrator-finish recipe applied and resume stayed armed —
  the doctrine's condition (incomplete work in the worktree) was checked,
  not cargo-culted (cycle-30 wrap note b).
- **The digest carried Phase 1 a fifth time**: this eval read one file,
  then drilled raw only twice (the wait_secs wake condition in
  src/tools.rs, the write_file fires in cycle 29's stream).

## 2. Incidents worth fixing

**I1 — The live supervisor is STILL loopd.sh's pre-T36 revision — now SIX
merges stale; only a human restart activates them (human-decision carry —
RESTATED, top of handoff).** `ps` shows loopd pid 90114 started
2026-09-25 20:43:53 EDT; `git log` since: `d5141c9` (T36 160-iters),
`00469ed` (T46 digest), `c3e8942` + `a65904f` (T47), `ca11c6b` (T50
re-exec), `5c842a6` (T53 ps-check). `grep -c re-exec loopd.log` still 0.
New evidence this eval, fifth and sixth confirmations: (a) the digest was
STALE at this eval's start — fourth manual regeneration (cycles 22, 24,
29, now 31); (b) BOTH cycles 29 and 30 wrapped at the budget margin —
115/120 and 114/120, `budget_low` first at remaining_iters=8 each time —
the dormant +40 iterations (T36) is the difference between a comfortable
wrap and a coin flip on every eval cycle. `launchctl` `KeepAlive =>
false` stands — chug must NOT restart it; the operator one-liner is in
the handoff.

**I2 — The spec check-line defect: `cargo test --lib` exits 101 on this
binary-only crate — NINE child-stream sightings, and t64 burned its goal
gate on it this cycle (NEW ROW T67, pri 2).** Every `check:` line is
evaluator-authored (META-META-SPEC "Extend TODO.md"), and evals keep
writing `cargo test --lib`: sightings in t22/t25/t26/t29/t39/t42/t58/t59
child streams, then t64-impl — work completed AND self-committed at 48/50,
then `goal_complete` REJECTED because leg 2 of its own spec's check was
unsatisfiable ("no library targets found"; leg 1 had passed 14/14). The
orchestrator substituted review + gates + independent mutant reproduction
(T55/T62) and the row landed, but the class has now burned budget in nine
children and could mask real red behind a wrong-reason rejection.
Reproduced at eval time on main: `cargo test --lib` → "error: no library
targets found in package `chug`", exit 101. Fix (specced, T67): the
convention sentence goes into META-META-SPEC (never `--lib`; plain
`cargo test` or `cargo test --bin chug`), the T8 guard gains a lint leg
over `specs/t*.md` check lines + a doctrine pin, and t64's historical
check line (the only live spec still carrying it) is corrected.

**I3 — `delegate status` `wait_secs` wakes on per-tool-call churn, not
significant change — the T29 long-poll's premise is unrealized (NEW ROW
T68, pri 3; promoted from the cycle-29/30 watch item after two cycles of
every-poll fires).** T29 (cycle 13) shipped the long-poll because cycle 10
spent 9/68 iterations (~13%) on instant status polls — "idle waiting is
the loop's largest remaining mechanical token sink". The wake condition is
`now_summary != entry_summary` over ALL `DelegateSummary` fields
(src/tools.rs ~:986), and the struct carries `last_event_type`/
`last_event_ts` (~:1065); an active child appends `tool_result` events
every 2–10 s, so the wait returns at the first 2.5 s poll tick almost
every time. Observed: EVERY long-poll with `wait_secs: 90`–`110` woke at
2–7 s, in cycle 29 AND cycle 30 (cycle-30 wrap note d — pacing fell back
to bash sleeps + instant status, 2 iterations per window instead of 1).
With cycles wrapping at 114–115/120 and `budget_low` at 8, ~10–25
orchestrator iterations per child wait is a first-class budget leak. The
rendered payload is fine (churn is worth showing) — the WAKE set should
be: `max_iters`, `last_iteration`, `budget_low_seen`, `goal_seen`,
`abort_seen`, `abort_reason` (+ the unchanged creation/liveness/deadline
legs).

**Carries disposition (cycle-30 wrap):** check-line defect candidate →
I2/T67 (filed, pri 2 as suggested); `wait_secs` early-wake → I3/T68
(promoted watch → row); loopd restart → I1, RESTATED.

**Watch items (continue unless noted):** `write_file` missing-path — 2
fires in cycle 29 (same timestamp — one parallel double-call with
malformed args, kimi fumble, self-recovered; the error message is clear),
0 in cycle 30; glm `output_truncated` advisories — 1 fire each in t62 and
t66 impls, both chunked-and-recovered goal-accepted (T38's advisory taken
as designed); `dead_port_probe` — T66 closed the class at all three legs
in-cycle; a future sighting elsewhere in mcp_http is a NEW row, not a
retry (cycle-30 Outcomes); **tools.rs monolith** — 3,962 → 4,050 (T64's
+88; the ~4,500 trip line stands, no new wayfinding miss this cycle);
`path escapes cwd` — 1 fire in cycle 29 WITH the T61 suffix visible, 0 in
cycle 30, plus the evaluator's own /tmp fire this eval — the remedy
reaches the model at the moment of need; `web_fetch` organic adoption —
0 calls across all 13 new streams (demand remains honestly absent);
delegate no-verdict natural stop — 0 new; **T63 resume armed, 0
exercises** — its first live exercise (a budget death with incomplete
work in the worktree) is the acceptance datapoint.

## 3. Friction hot spots

The check-line rejection (I2) is the cycle's top mechanical friction —
nine sightings, one goal gate burned this cycle. Cycle-29's remaining
failed-tool classes were model fumbles, all self-recovered in 1 iteration:
`write_file` without `path` ×2 (one parallel double-call), `edit_file`
`old` not found on scratch files (`/tmp/eval-outcomes-preserve.md`,
`EVALUATION-head.md`) and once on TODO.md in cycle 30, the one cross-tree
`path escapes cwd` (T61 suffix attached, watch item). The digest's
blank/N error buckets in both orchestrator streams are benign exit-1
classifications (bash greps printing counts). `timed out after 120s` — 0
new. Nothing else crosses the row threshold.

## 4. Capability gaps — FEATURE SCAN (required)

Audited against the META-META-SPEC candidate classes on the post-T66
surface: **parallel tool calls — PRESENT. `delegate` — PRESENT and
doctrine-complete** (launch/status/`wait_secs`/`resume` — T58 shipped,
T63 adopted; the one rough edge is I3, a wake-semantics robustness row,
not a gap). **`web_fetch` — PRESENT**, 0 organic calls across 13 streams
(demand-honest). **MCP — PRESENT** (stdio + streamable HTTP);
resources/prompts zero demand (rejected, 4th cycle). **Plan-then-execute —
the LEDGER surface** (no failure attributed to its absence). **Session/
handoff — `--resume` + rotation + `delegate resume` — complete. Steering
— PRESENT** (TUI line, chat dock, T13/T38 injections). **Fleet-driving —
T44 caps at 2**; the queue drains in 1–2 cycles; no wider fan-out demand
(rejected, 3rd cycle). **Context management — PRESENT** (no context-limit
failure in any stream; the 510–516k cumulative-input figures are sums
over ~115 full-context calls, not window pressure).

**No feature row filed this cycle** — the honest scan finds every class
PRESENT or previously rejected with zero new demand evidence. Weighed and
rejected again (recorded so next cycle doesn't re-litigate): token-
budgeted impl children via T39's knob (deaths are iteration-ceiling, and
T63's resume is the cheaper recovery — armed, 0 exercises); impl
max_iters 50→60 (HOLD — the only 50/50 death since T63 was
complete-and-committed, which resume correctly didn't cover);
`src/delegate.rs` extraction (watch — 4,050 < 4,500 trip, no new
wayfinding miss); a `scripts/gates.sh` shorthand for the bounded-gates
incantation (shell alias territory, below the feature bar); MCP
resources/prompts, web_fetch POST/HEAD, delegate fleet view — zero
demand, third/fourth cycle rejected.

## 5. Top 3 priorities

1. **HUMAN: restart the supervisor (I1)** — six stale revisions; every
   cycle launches at 120 not 160 (T36 dormant) and both cycles wrapped at
   the margin (115/120, 114/120, budget_low@8); the digest is stale at
   every eval start (4 manual regenerations). 30 seconds, zero risk — the
   new ps-check skips an in-flight cycle cleanly.
2. **T67 (pri 2, doctrine/robustness)** — the nine-sighting check-line
   defect; a two-line convention + a guard leg kills the class forever
   and repairs t64's spec. Cheap, high certainty.
3. **T68 (pri 3, src/tools.rs robustness)** — significant-change wake for
   the long-poll; returns ~10–25 orchestrator iterations per child wait
   to cycles that wrap at 114–115/120.

## 6. README audit (usability, not just accuracy)

Cold read top-to-bottom (post-T65). **(a) Reading order:** what-it-is →
quickstart → interactive → autonomous → TUI → tools → risk gate → MCP →
Langfuse → self-hosting specs → continuous mode → development — the
newcomer arc holds; no append-only accretion. **(b) Redundancy:** none
found — the delegate paragraph is the single source for launch/status/
wait semantics; the target-cache mechanics appear once, in the
Continuous-mode section, with the spec-pointer sentence doing the
delegation. **(c) Staleness:** none found — the delegate paragraph matches
the shipped surface (launch args + 40/35 defaults, max-tokens passthrough,
resume, status latest-segment, `wait_secs` with the `waited:` line); the
Development layout brace list matches the tree (T60 verified, 20 modules +
main.rs); the loopd section names the re-exec behavior (T56) and the
digest refresh (T46). **(d) Balance:** the T65 rewrite reads at README
altitude — user semantics in two short paragraphs, mechanism deferred to
specs t47/t52/t57; the MCP remote-specifics paragraph stays acceptable
(it IS the configuration contract). One NB: the README's `wait_secs`
clause ("returns early when the child's state changes") will need the
same rewording T68 gives the tool description — T68's spec already
carries that leg, so no separate docs row. **(e) Quickstart truth:**
commands verified unchanged against main (`cargo build`, `cargo install
--path .`, flags current; `check:` behavior accurately described; auth
chain matches the code). **No docs row filed** — the first zero-finding
audit since the §6 audits began; the de-accretion rows (T51/T56/T60/T65)
are compounding.

## Handoff — recommended execution order

**Human-decision item FIRST (blocks nothing chug-side but caps every
cycle): restart the supervisor.** `launchctl kickstart -k
gui/$(id -u)/com.tampajohn.chug-loopd` (or `./loopd.sh stop` + the
operator's usual start). Activates T36 (160-iter cycles — both cycles
wrapped at the 120 margin with budget_low@8), T46 (digest refresh — stale
at 4 of the last 5 eval starts), T47 (warm shared cache), T50
(self-re-exec — after this restart the pileup can never recur), T53
(ps-based skip). KeepAlive=false means chug must never kill it (I1).

Queue order (bugs > robustness > features > DX):

1. **T67 — pri 2, DOCTRINE item: runs ALONE** (META-META-SPEC.md edit —
   T44 forbids overlap for doctrine), REQUIRED kimi validation (loop/spec
   doctrine). Files: META-META-SPEC.md + tests/todo_consistency.rs +
   specs/t64-validator-survivor-pins.md (one-word check-line repair).
2. **T68 — pri 3, src/tools.rs: REQUIRED kimi validation** (core tool
   surface). Files: src/tools.rs (wake condition + doc comment + schema
   description) + README.md (one clause). Runs strictly after T67 merges
   (doctrine never overlaps; serial queue of 2).

Both specs are written and ready; a cold next cycle needs zero human
words. If the queue outlives this cycle's budget, unworked rows stay
`todo` with specs — that is a fine outcome.

## Outcomes (filled at cycle wrap — LOOP-SPEC Phase 3)

### Cycle 31 (2026-09-26, ~08:06– EDT) — MANDATORY fresh eval (queue EMPTY); T67 landed (check-line doctrine + guard)

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
