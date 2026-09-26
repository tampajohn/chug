# EVALUATION — chug, assessed by chug-loop (2026-09-26, cycle 18)

Corpus: `.chug/events-20260926-043119.jsonl` (**cycle-16's own full run**:
120/120 iterations, budget_low@8 at 04:24:42, goal REJECTED 04:25:54
("check command failed" — the T8 TODO guard), abort 04:26:07 — fresh eval +
T36 landed + T37 impl recovered to green, died at the goal gate on a
self-inflicted TODO pipe; 72 bash / 36 delegate calls, 5 tool errors),
`.chug/events-20260926-044143.jsonl` (**cycle-17's full run**: 40/120
iterations, ~9.5 min — freshness-skip + T37 recovery, goal accepted; 25 bash
/ 10 delegate, 1 bash error, `wait_secs` polling throughout, zero long
sleeps), the 4 t36/t37 child streams (glm impls: 15/50 first-try goal-accept
(T36) and 50/50 budget-abort (T37, truncated-write saga); kimi validators:
11/50 PASS (T36), 48/50 PASS (T37, budget_low@8 at 42 — wrapped inside T18's
margin)), `.chug/loopd/loopd.log` (cycles 13–18 OK unattended except cycle
16's goal-gate death; single-driver skip fired correctly at 00:44:00),
`TODO.md` (T1–T37 all done with refs — queue EMPTY at cycle start, freshness
rule cannot fire → this eval is mandatory), git log `7cb20cc` (cycle-17
wrap), `src/` (21,006 lines, +1,000 since the cycle-16 eval — webfetch.rs
+987 T37, tools.rs +44), `README.md` (265 lines — §6 audit below), and the
cycle-17 wrap's five written carries (connect-timeout const-pin, glm
truncated-write watch, T31-residual flake watch, web_fetch adoption grep,
README §6 Tools audit — disposition for each below).

## 1. What chug does well

- **The loop recovers from its own deaths.** Cycle 16 died at the goal gate
  with 0 iterations left; loopd restarted it, the single-driver skip fired
  correctly (00:44:00, avoiding a collision with the lingering process), and
  cycle 17 recovered T37 per the T29 precedent in 9.5 minutes flat. Three
  consecutive-failure halts: zero ever needed.
- **Adversarial validation keeps earning its budget.** T37's kimi validator
  killed 10/10 mutants, each by exactly the expected test, and wrapped at
  48/50 — the T32 widening (40→50) absorbed a T37-class item that would have
  died at the old cap, exactly as designed.
- **wait_secs polling is fully adopted.** Cycle 17: 10 delegate calls, zero
  bash sleeps, ~9.5 min launch-to-push for a validation+merge cycle. The
  delegate surface (launch/status/wait_secs) is now the entire child
  interface; the hand-rolled nohup path has not been needed since T24.
- **delegate status liveness told the truth all cycle** (T28's zombie reap):
  zero alive-after-done sightings in the corpus.
- **Gates are fast and trustworthy:** 430+3 in ~9s, clippy -D warnings
  clean, T8 guard caught the cycle-16 pipe bug within seconds of the edit
  (the failure was budget exhaustion, not detection).

## 2. Incidents worth fixing

**R1 — Truncated responses are invisible to the model (NEW ROW T38, pri
2).** `MAX_TOKENS = 8192` (`src/api.rs:10`) caps every response's output.
The T37 glm impl child tried to `write_file` a 987-line webfetch.rs (~30KB ≈
8–10k tokens) in one call — physically impossible under the cap. The API
truncated it (`stop_reason: max_tokens`), chug executed the partial write
without a word, and the child spent its remaining budget discovering chunked
heredocs on its own; it died 50/50 mid-gates (events-t37-impl, budget_low
04:20:41, abort 04:21:06) and the orchestrator harvest-completed 4 fixes
(E0382, an every-fetch off-by-one panic, DOCTYPE leak, trim + 2 test
adjudications). Root cause: `Response::stop_reason()` exists
(`src/api.rs:417`) and is shipped to observability, but **the driver never
acts on it**. T13's injection seam (`src/driver.rs:509-538`) is the
ready-made fix surface. This is the top child-robustness gap in the corpus.

**R2 — Cycle 16 died at the goal gate on its own TODO edit (NEW ROW T40,
pri 3).** The cycle-16 orchestrator annotated the T37 row with a recovery
recipe containing a `|`; the T8 guard (splits on every pipe) read 7 cells,
`cargo test` failed, `goal_complete` was REJECTED at ~iteration 118
(04:25:54). The fix (`75da192`, one character) plus push consumed the last
iterations; abort at 120/120 (04:26:07). Detection was perfect — the guard
named row 41 and the cell count — but the doctrine never told the
orchestrator the hazard existed. LOOP-SPEC §2 step 5 (where rows get
flipped/annotated) says nothing about pipes or running the guard after
edits. One sentence closes it.

**Watch items (disposition of the cycle-17 wrap's carries):**
- **glm truncated-write watch** → PROMOTED to row T38 (root cause found:
  8192-token ceiling + no stop_reason handling).
- **T31-residual flake watch** → 0 new sightings in cycle 17's streams (the
  one organic sighting, T36-impl's dead_port_probe pin under parallel load,
  was already recorded in `a2127f0`). Watch continues; not a row.
- **web_fetch adoption grep** → 0 organic calls across all streams
  (`grep -c web_fetch .chug/*.jsonl` — the tool landed <30 min before this
  eval; the loop's work is repo-local). Watch continues; demand honesty
  stands (filed on capability-gap doctrine, and the T37 row says so).
- **connect-timeout const-pin candidate** → PROMOTED to row T42 (pri 4,
  tests-only).
- **delegate-status preview truncation** (cycle-16 watch) → CLOSED by T25's
  failure-aware tail windows; no recurrence in the corpus.

## 3. Friction hot spots

**R4 — `path escapes cwd` discoveries (NEW ROW T41, pri 4).** Two sightings
in cycle 16 alone: `read_file /tmp/chug-loop-t37/src/webfetch.rs` (the
review step reaching into a child worktree) and `write_file
/tmp/eval-head.md` (eval scratch) — each cost an iteration plus a bash
workaround, and the cycle-16 eval logged the same class firing on *its* run.
The descriptions undersell the restriction: `read_file` says only "File path
relative to cwd" (`src/tools.rs:65`); the refusal is learned by error. T22
is the template: the tool description is the only universal surface. (The
confinement itself stays — it's load-bearing sandbox posture; this is candor
plus naming the `bash` escape hatch.) While auditing, found the README
Tools intro calling delegate "the one documented exception" — stale since
T37 made web_fetch the second; folded into T41.

**R3 — EVALUATION.md Outcomes accretion (NEW ROW T43, pri 4).** Outcomes is
50,957 of 65,802 bytes (371 of 625 lines, 77%), grows every cycle (T34's
per-item doctrine — correct at write time), and is carried verbatim through
every fresh-eval rewrite (~13k tokens of output just to re-emit old
narrative). The narrative is redundant with git: every row-flip commit
carries the same saga (`7cb20cc`, `1e82ffb`). Filed a pruning rule (last 6
cycles full, older one-lined) + first compaction as one row.

**Verified already-fixed (not re-filed):** the PATH tax (T4 —
`bash_tool_finds_cargo_without_path_prefix` green, zero `export PATH` lines
in the corpus); edit disambiguation (T5 — cycle 16's 2 edit_file errors were
plain `old not found`, no revert-thrash); delegate polling profile (T24/T29
— see §1); events-first evaluation (this eval's corpus mining was 100%
events.jsonl + git; zero transcript reads needed until the cycle-16 goal
rejection required the transcript tail for the failure text — exactly the
failure-cluster-at-end pattern T25 built for).

## 4. Capability gaps — FEATURE SCAN (required)

LOOP-SPEC §2's two named gaps are **closed**: delegate (T23) and web_fetch
(T37). Interrogating the standard classes against this corpus:

- **Parallel tool calls** — ALREADY SUPPORTED: the driver iterates every
  tool_use block in a response (`src/driver.rs:614`), serial in-turn
  execution, standard for this harness class. Not a gap.
- **Token budgets for delegated children** — REAL GAP, **NEW ROW T39** (pri
  3, the cycle's feature row): T15 shipped `--max-tokens` for run/chat, but
  delegate's launch argv (`src/tools.rs:698-708`) passes only
  iters/minutes. A watch-and-wait child (wait_secs long-polls) is T15's
  exact gap class one level down; the t37-validate child burned ~169k tokens
  in 48 iters with no ceiling available. Small, coherent, closes the knob
  matrix.
- **Plan-then-execute modes** — no corpus evidence: children's failures are
  budget/truncation-class, not plan-absence class. The spec+goal+ledger
  discipline already forces externalized planning. Not filed.
- **Richer MCP consumption** — SPEC-9 shipped stdio+HTTP, fail-soft, listen
  streams; zero MCP-related incidents in the corpus. Not filed.
- **Session/handoff UX** — `--resume` + LEDGER + events + the T3/T7/T10
  rotation trilogy; cycle 17's recovery was 9.5 min end-to-end. Not filed.
- **Steering depth** — chat steering + run steering notes + T13 budget-low
  injection; T38 extends the same seam to truncation. Sufficient.

## 5. Top 3 priorities

1. **T38 (pri 2, robustness)** — the truncated-response advisory. Kills the
   newest child-death class at its root; T13's seam makes it small;
   driver.rs + events.rs → adversarial validation REQUIRED.
2. **T39 (pri 3, feature)** — delegate `max_tokens` passthrough. The
   cycle's feature row per the amended doctrine (features first-class);
   tools.rs → validation REQUIRED.
3. **T40 (pri 3, doctrine)** — the TODO-pipe sentence. One hunk, and it
   protects every future cycle's goal gate; loop-doctrine → validation
   REQUIRED per §2 step 4.

(T41/T42/T43, all pri 4, follow if budget allows: sandbox candor, webfetch
const pins, Outcomes pruning. T42 is tests-only → validation optional per
T16/T31 precedent; T41 touches src/tools.rs descriptions → validation
REQUIRED per the T22 precedent; T43 touches LOOP-SPEC → validation
REQUIRED.)

## 6. README audit (usability, not just accuracy)

Cold-read of all 265 lines:

(a) **Reading order** — sound: what-it-is → quickstart → interactive →
autonomous → TUI → tools → risk gate → MCP → observability → self-hosting
specs → loopd → development. The delegate/web_fetch additions are integrated
paragraphs in Tools, not glued bullets. No accretion debt found.

(b) **Redundancy/drift — ONE finding:** Tools intro says paths are sandboxed
"(`delegate` is the one documented exception…)" while the web_fetch
paragraph (5 lines later) says "Like `delegate`, it reaches outside the cwd
sandbox by design". The intro's "one" went stale the day T37 landed. Folded
into T41. (The delegate paragraph also restates the exception a third time —
consistent today, worth one watchful eye, not a finding.)

(c) **Staleness** — none beyond (b). Budget-low says ≤8 iterations (T18
current), token budget documented (T15), events-log bullet covers T17/T20/
T25, banner covers T11/T20. Quickstart has the T35 install step.

(d) **Balance** — MCP's paragraph is long but it's a user-facing config
surface; delegate/web_fetch paragraphs match the established tool-paragraph
pattern. Nothing demands a spec-file move.

(e) **Quickstart truth** — `cargo build` → `cargo install --path .` → `chug
run …`: true as written (cycle-16 verified cargo is on PATH for a login
shell; T35 added the install step the cold reader needs).

No standalone docs row filed; the one finding rides T41.

## Handoff — recommended execution order

Queue (all rows have ready specs; priority doctrine bugs > robustness >
features > DX > perf, features first-class at equal pri):

1. **T38** (pri 2) — glm impl + kimi validation REQUIRED (driver.rs,
   events.rs). Est. 30–35 orchestrator iterations.
2. **T39** (pri 3) — glm impl + kimi validation REQUIRED (tools.rs). Est.
   25–30.
3. **T40** (pri 3) — glm impl + kimi validation REQUIRED (loop doctrine;
   T30/T33/T34 validators ran ~11–19 iters on doctrine items). Est. 20–25.
4. **T41** (pri 4) — glm impl + kimi validation REQUIRED (tools.rs
   descriptions; T22 precedent). Est. 25–30.
5. **T42** (pri 4) — glm impl; validation OPTIONAL (tests-only, T16/T31
   precedent). Est. 15–20.
6. **T43** (pri 4) — glm impl + kimi validation REQUIRED (LOOP-SPEC +
   EVALUATION.md). Est. 20–25.

Budget arithmetic at the 120-iteration cap (T36's 160 is merged but the
running loopd predates it — activation remains the operator's restart
carry): Phase 1 cost ~45–55, one item ≈ 25–35, wrap ≈ 10 → this cycle
realistically lands **2–3 rows** (T38 + T39, + T40 if lean); the rest stay
`todo` with ready specs — a fine outcome per doctrine, and the freshness
rule lets the next cycle skip Phase 1 and start straight on the queue.

Human-decision items (unchanged, for the operator): restart loopd to
activate T36's 160-iteration budget (this cycle still runs 120); web_fetch
adoption remains watch-only.

## Outcomes (filled at cycle wrap — LOOP-SPEC Phase 3)

### Cycle 21 (2026-09-26, ~03:13–03:40 EDT) — freshness-skip; T43 landed; CARGO_MANIFEST_DIR × target-shared staleness bit the post-merge gates

One `chug run --spec LOOP-SPEC.md` session (kimi-k3 orchestrator; glm-5-3-flash impl child; kimi-k3 validator), launched by loopd. **Freshness rule fired** (cycle-18 eval same-day + T43 `todo` with a ready spec) → straight to the queue. Single todo row → no bundling, no overlap candidates; single-driver verified (only this `chug run`).

**Landed (1/1 queued rows):**
- **T43 — EVALUATION.md Outcomes pruning rule + first compaction** (pri 4, doctrine; spec amended pre-dispatch `67fa2b8` — the check's 200-line bound was unreachable: authored at cycle 18 on a "cycles 14–17 full ≈150" estimate, cycles 18–20 then added ~256 full-entry lines; bound raised to 400 with the arithmetic recorded in req 3, 1d6780d precedent; impl `4ed7bdd` glm goal-accepted 20/50 ~4 min first-try, merge `8694003`, flip + this entry in the same `todo:` commit per T34 doctrine). LOOP-SPEC Phase 3 gains the wrap-time rule (+5 lines, all else byte-identical): Outcomes keeps the last 6 cycles in full; older entries compact to one-liners; the narrative lives in git. First compaction: cycles 5–13 (311 lines) → 9 one-liners, all 30 cited refs cat-file-verified; EVALUATION lines 1–536 cmp-verified byte-identical; Outcomes 627→333 lines; file 80,908→51,888 bytes. kimi VERDICT: **PASS** 17/50 — all 4 reqs hand-verified (byte-identity cmp, ref existence + subject/date matching against parent entries), 5/5 mutants killed (rule-token corruption, ≥400 padding, Cycle 5/13/14 heading corruption/deletion), gates re-run 452+6+6+3 + clippy, tree restored clean; one nit (commit-message "35 refs" vs actual 30) fixed by orchestrator amend pre-merge. 6 artifacts harvested pre-removal (impl events/LEDGER/delegate.log copied BEFORE the validator launch per the T39 reseed lesson, then validator events/LEDGER/delegate.log).

**Skipped/deferred:** none — the single queued row landed; the queue is **EMPTY** → the freshness rule cannot fire → the next cycle MUST run a fresh Phase-1 eval.

**What the validators caught:** no implementation defects. The kimi validator's ref-by-ref subject/date matching against the parent full entries is the deepest truthfulness check a compaction has received; its 5-mutant set proved every check leg kills something.

**Cycle-level notes:** (a) **NEW INCIDENT CLASS — `env!("CARGO_MANIFEST_DIR")` × target-shared staleness.** Post-merge gates in main FAILED 2/452 (`build_info::resolve_head_in_the_real_repo…` panicked "checkout resolves"; `tools::readme_tools_intro_names_both_sandbox_exceptions`) — the shared-cache test binary had last been compiled in the worktree (validator's gate run), cargo's fingerprint accepted it as fresh in main, and the compile-time `env!` had baked `/tmp/chug-loop-t43` — deleted at worktree removal seconds earlier. Recovery: `touch src/*.rs build.rs` → rebuild → 452+6+6+3 green. **Five test sites use the compile-time env var** (src/tools.rs README pin, src/build_info.rs real-repo resolve, tests/todo_consistency.rs, tests/shared_target_dir.rs, tests/eval_digest.rs) — each reads the WRONG checkout when the shared cache serves a binary compiled elsewhere: false RED if that checkout is deleted (this cycle), false GREEN if it still exists (the scarier leg — gates could pass on stale file contents). Fix candidates for the next eval to weigh: runtime path resolution in the tests (env::var/current_exe/dir-diff rebuild discipline), or a LOOP-SPEC gate rule (touch a src file before main-tree gates after any worktree removal). Filed as a written carry, not a row — Phase 1 owns row-filing and the queue-empty eval is mandatory next cycle. (b) **T43's own rule practiced at this wrap:** adding cycle 21 pushed cycles 14–15 outside the last-6-in-full window → both compacted to one-liners in this same entry edit (refs cat-file-verified) — the rule's first wrap-time application; Outcomes holds at 319 lines (627 at this cycle's start). (c) The pre-dispatch spec-amendment discipline (1d6780d → 67fa2b8) held again: measure the spec's stale assumptions before dispatch, amend with arithmetic, cite in the flip. (d) The glm child caught the 65,802→80,908 byte drift unprompted and cited both figures in its commit — spec-quality × model discipline compounding.

**Final state:** main = this `todo:` commit (merge `8694003` + flip); gates 452+6+6+3 green in main post-touch-rebuild, clippy `--all-targets -D warnings` clean; TODO.md truthful (T1–T47 all done with refs — zero `todo` rows); 6 t43 child artifacts in `.chug/` (impl+validate events, impl+validate LEDGERs, 2 delegate logs); README gate satisfied (no user-visible surface this cycle — doctrine + handoff-doc compaction only); everything pushed. **Handoff:** next cycle MUST fresh-eval (queue empty); written carries: the CARGO_MANIFEST_DIR staleness finding + fix candidates in note (a); standing human-decision carries unchanged.

### Cycle 20 (2026-09-26, ~02:17– EDT) — freshness-skip; T40 recovered, T47+T41+T42 landed; T43 deferred

**T41 (sandbox-candor tool descriptions — DX friction) → done 7170fb4.**
glm impl 37/50 first-try: all five filesystem tools (read_file, write_file,
edit_file, glob, list_dir) now name the `path escapes cwd` refusal and the
`bash` escape hatch in both the tool description and the path property;
README's stale "one documented exception" line now names delegate AND
web_fetch. Five live-schema pin tests (T22 pattern — assert tool_schemas()
output, not copied literals). kimi VERDICT: PASS 36/50 — 10/10 mutants
killed including a resolve_safe error-string corruption and a surgical
refusal-naming drop; the T22/T26/delegate/path_safety pins all pass
unmodified. Landed under the FIRST live T44 overlap: T42's impl ran
concurrently (disjoint files — tools.rs+README vs webfetch.rs), both
children finished within 7 minutes, merges stayed serial.

**T42 (webfetch timeout const pins — robustness nit) — done 7ff8bfa.**
glm impl 9ebd55d 28/50 first-try under the first live T44 overlap (ran
concurrent with T41's kimi validator — disjoint files, tools.rs+README vs
webfetch.rs; both children finished ~7 min; merges stayed serial). Pins
WEB_FETCH_CONNECT_TIMEOUT == 10s and WEB_FETCH_TOTAL_TIMEOUT == 30s in
webfetch.rs's test module so a const-only edit now fails the suite (T37
validator's non-blocking observation, closed). Tests-only item —
adversarial validation optional per T16/T31 precedent; orchestrator gates
(452+6+6+3 in main) + diff review sufficed. 1 artifact harvested
pre-removal.

**T47 (shared CARGO_TARGET_DIR for worktree builds — operator speed
initiative) → done 2f4cefc.** The cycle's adversarial-validation showcase:
glm impl c3e8942 died 50/50 post-commit (T15-class recovery, warm-cache
first dogfood 28.6s), kimi round-1 returned VERDICT: FAIL with a PROVEN
cross-cycle loopd.sh bug — the bare `export CARGO_TARGET_DIR` inside the
while loop persists into cycle 2+, redirecting the supervisor's own build
into target-shared and leaving ./target/debug/chug permanently stale
(faithful 2-cycle simulation) — plus the M6 mutant (both META-SPEC nohup
prefixes dropped) surviving the weak >=3 carrier pin. glm fix-up a65904f
(env-prefix on the chug invocation, bare-export ban pin, per-carrier
count_eq pins, self-mutation-tested) then kimi re-validation VERDICT: PASS
42/50 — both findings verified resolved with the required mutants, 8/8
mutants killed, non-vacuousness proven on the parent, tree sha-restored,
acceptance evidence recorded (3.0s warm build vs the 43s–6min cold range).
The loopd restart is pending operator action, so the export reaches cycles
only after the next supervisor start.

**T40 (LOOP-SPEC step-5 pipe doctrine) → done 75c21c8.** The cycle-19
mid-arc handoff, recovered per the T29/T37 precedent: diff reviewed (+7
lines LOOP-SPEC.md, one sentence woven into step 5's row-flip directive,
exactly per spec), gates re-run in the preserved worktree (444+6+3 +
clippy), kimi validation (REQUIRED — doctrine) VERDICT: PASS at 15/50 —
non-vacuousness proven (all 3 check tokens absent on the parent commit),
M1/M2/M3 token mutants killed, M4 prose-weakening survivor adjudicated
by-design per the T44 precedent, tree restored sha256-verified, commit
citations grounded (75da192 + cycle-18 eval). Impl events harvested BEFORE
the validator launch (the T39 reseed lesson), validator events + verdict
LEDGER harvested pre-removal; gates re-run in main post-merge. The new
doctrine was practiced at its own row flip: no pipe in the notes cell,
`cargo test --test todo_consistency` run before committing.

**Cycle-level notes.** Four items landed (T40 recovery, T47, T41, T42);
T43 (Outcomes pruning, pri 4, doctrine) deferred on the step-6 budget rule
— its spec is ready and its check is self-contained; work it FIRST next
cycle (it bounds the very carry-forward cost this cycle's four Outcomes
entries just added to). Validators earned their budget all cycle: T47's
round-1 FAIL caught a proven cross-cycle loopd bug simulation-verified;
T41's killed 10/10 mutants. First live T44 overlap ran clean (T41 validator
+ T42 impl, ~7 min wall for both). T47's warm cache dogfooded immediately:
every child build this cycle was seconds, not minutes. Watch items carried:
T46 golden-section pin test (cycle-19), T36/T47 loopd restart still pending
operator (the env-prefix reaches cycles only after the next supervisor
start), README gate: all four items integrated (T40/T47 doctrine is in-spec,
T41 README two-exceptions line + T47 loopd paragraph landed in their merge
commits; T42 is tests-only, no user surface).

### Cycle 19 (2026-09-26, ~01:23–02:20 EDT) — freshness-skip; T46 + T45 + T39 landed

**T39 (delegate launch max_tokens passthrough — FEATURE) → done 52ec8ab.**
The queue's feature row, worked third after the two pri-2 speed rows.
glm impl 30/50 first-try. `delegate` launch gains optional integer
`max_tokens` (schema `minimum: 1`): present → `--max-tokens N` appended at
the child argv tail (T15 parity); absent → argv byte-identical to pre-T39
(children keep no-ceiling behavior unless the orchestrator opts in).
Implementation shape: `delegate_child_argv` pure seam (whole-list pinned
in tests, plus end-to-end argv dumps through the `CHUG_DELEGATE_BIN`
stub), `delegate_max_tokens` parse (`< 1` → tool error naming the
constraint — 0 would read as "unlimited" on the child CLI, the
silent-unbounded-launch failure the guard exists to prevent), return-text
echo `max_tokens: N` only when configured (absent → byte-identical),
status ignores it exactly as it ignores max_iters today (pinned), README
launch sentence gains one hyphenated clause matching how
`--max-iters`/`--max-minutes` are presented. 8 new tests (444+6+3 total).
kimi validation VERDICT: PASS 38/50 — all 5 reqs hand-verified (incl. the
child CLI's u64/0=unlimited semantics confirming the reject-0 guard is
correct, not a behavior change), gates re-run on the pristine tree, both
spec-named acceptance mutants KILLED (drop-argv-append, accept-0) plus
drop-return-echo and drop-schema-minimum; M3 (as_u64 for as_i64) survives
adjudicated spec-neutral (both error legs name the constraint; 0/neg
still rejected pre-spawn); 3 non-blocking findings (negative-leg message
not uniquely pinned, >i64::MAX rejects as "non-integer" fail-closed, stub
`sleep 60` linger matches pre-existing delegate test pattern). 3
artifacts harvested pre-removal (impl events, validate events, verdict
LEDGER — the impl's own LEDGER was overwritten by the validator's
fresh-run reseed before archiving; its substance survives in the impl
events stream + commit message). Gates re-run in main green.
LOOP-SPEC's child-launch templates deliberately do NOT pass max_tokens
yet — spec's out-of-scope: adoption is a doctrine decision for a later
evaluation (the T37-validate 169k-token datum is the motivating case).

**T45 (trivial-row bundling doctrine) → done 3efb98d.** Second speed row,
LOOP-SPEC-only (+39/−5). glm impl 28/50 first-try — the doctrine-item
landing class holds (T32/T33/T34/T44/T45 all ≤28 iters). §2 gains
"Trivial-row bundling (T45)": ONE impl child may take up to 3 rows under a
CONJUNCTIVE four-condition eligibility gate ((a) ≤ ~30 changed lines per
row, (b) same 1–2 files or docs/doctrine-only, (c) no driver.rs/api.rs
core loop, (d) every pri ≤ 3 — "when any one is in doubt, run the rows
separately"); never bundled: features, cross-area, >3 rows. Child goal =
step-2 template + explicit row list + spec paths, ONE commit PER ROW in
queue order; ONE kimi round covers a bundle (mutation-test per row where
feasible; any core-adjacent row → REQUIRED validation covers the set;
docs/pins-only bundles may rely on orchestrator gates per T16/T31/T35);
bundled rows may flip in ONE `todo:` commit naming every row + ref; T44
interplay written in (bundle = ONE impl child under the 2-in-flight cap;
a doctrine-containing bundle never overlaps). kimi validation VERDICT:
PASS 20/50 — all 4 reqs verified against the on-disk spec, gates
independently re-run (436+6+3 + clippy), M3 mutant (TODO spec-cell
corruption) KILLED by todo_consistency (the T8 guard the doctrine leans
on is live and toothy), M1 (ALL→ANY conjunctivity flip) / M2
(squash-commit inversion) prose mutants by-design survivors per the T44
precedent (the spec's Tests leg designates review as the check), tree
restored byte-identical hash-verified. One non-blocking finding, recorded
for the first bundle dispatch: delegate's mandatory `spec:` field takes
exactly one path — use the first row's spec, the child reads the rest
from its worktree checkout. 4 artifacts harvested pre-removal; gates
re-run in main green. Acceptance is forward-looking: a later cycle lands
a ≥2-row bundle with per-row commits under one verdict.

**T46 (eval digest) → done 6a3fa98.** Freshness rule fired (EVALUATION.md
same-day + 8 todo rows) → Phase 1 skipped, straight to the queue. Queue
order T46 → T45 → T39: the cycle-18 handoff recommended T45 first, but
T46 is the same pri and its script is the more mechanical land; both are
doctrine items (no overlap either way). glm impl 49/50 goal-accepted
(budget_low@8 fired — wrapped inside T18's margin):
`scripts/eval-digest.sh` (293 lines, jq/awk-only, `LC_ALL=C`, pinned-clock
`CHUG_DIGEST_NOW`, BSD/GNU stat+date fallbacks, ~0.2s on the real corpus)
writes `.chug/eval-digest.md`: per-archive iterations/wall/tool
distribution/error classes/cumulative token totals (T14
last-value-not-summed semantics, hand-verified by the validator against
raw jq)/quartile input-context curve/aborts/budget_low/output_truncated,
plus corpus-wide TODO counts, EVALUATION.md age, and a self-flagging
staleness block (generated-at vs newest-events-mtime + write-race flag +
reader-side `find -newer` check). loopd.sh refreshes it before every
cycle (set -u-only → digest failure non-fatal). META-META-SPEC corpus
item 1 = digest-FIRST, raw events only for incident drill-down
(renumbered 1–6; the §6 cross-ref is to the EVALUATION section list,
unaffected). tests/eval_digest.rs: 6 legs — spec-pinned jq baseline on 3
sampled archives, empty + missing `.chug/`, malformed-line tolerance
(digest=4 where the naive pipeline dies), pinned-clock byte-determinism,
TODO/EVAL parsing. kimi validation VERDICT: PASS 31/50 — 5/5 reqs
hand-verified, gates re-run twice (436+6+3 + clippy), 6/6 kill-mutants
died; 7/7 survivors (unpinned spec-required output fields: tools top-N,
quartile divisor, iters-ceil source, abort budget label, `dur()` hours
branch, reader-check text, budget_low detail) adjudicated non-blocking
per the T44 precedent (spec Tests scoped iteration-counts + empty-digest
only) — **watch item for a future eval: one golden-section pin test per
archive would close the survivor class**; 4 cosmetic observations (one —
the corpus-item-3 backticks — orchestrator nit-fixed pre-merge, 6a3fa98).
4 artifacts harvested pre-removal; gates re-run in main green.
Acceptance is forward-looking per the spec: the next FRESH evaluation's
events should show the digest read early and Phase-1 iterations visibly
below the ~45–55 baseline — to be recorded in that cycle's Outcomes.

**Cycle-19 wrap.** Three rows landed (T46, T45, T39 — every impl glm
first-try, every validation a first-arc PASS, zero fix-up rounds; each
full arc cost ~13–15 orchestrator iterations with sleep-paced status
polling). **T40 mid-arc handoff:** impl child goal-accepted 13/50,
committed 311d305 on branch loop-t40 in the PRESERVED worktree
/tmp/chug-loop-t40 (LOOP-SPEC.md +7, one file) — the orchestrator's
budget-low (8 iterations left) hit before review/validation/merge. The
child is dead (state: done), its cwd was the worktree, so no
single-driver conflict. Recovery recipe is written on the T40 TODO row
(T29/T37 precedent); the worktree must NOT be removed before its
.chug/events.jsonl is harvested. **Deferred, all todo with ready specs:**
T47 (pri 3, shared CARGO_TARGET_DIR), T41 (pri 4, sandbox-candor
descriptions — NOT bundle-compatible with T42 under T45's same-1–2-files
clause: T41 is tools.rs + README, T42 is webfetch tests), T42 (pri 4,
webfetch timeout pins), T43 (pri 4, Outcomes pruning — increasingly
load-bearing: this section grows every landing). **Doctrine landed this
cycle, available but not yet exercised:** T44 pipeline overlap and T45
trivial-row bundling (this cycle ran under the pre-T44/T45 launched spec;
their acceptance markers are forward-looking). **Watch items for the next
evaluation:** (a) T46 validator's golden-section pin recommendation (7/7
survivor class: unpinned digest output fields); (b) T39 validator nits
(negative-leg error message not uniquely pinned; delegate stub sleep-60
linger, pre-existing pattern); (c) T46 acceptance — next FRESH eval must
show the digest read early with Phase-1 iterations visibly below the
~45–55 baseline; (d) T36's 160-iteration loopd budget is STILL pending an
operator loopd restart — this cycle ran at 120 and fit only because
Phase 1 was skipped; a fresh-eval cycle at 120 lands ~2 items; (e) small
mystery for the corpus: T39's impl LEDGER.md was overwritten by the
validator's fresh-run reseed without a .chug/LEDGER-\* archive (T3 should
have archived a non-seed ledger — worth one jq query next eval).
Freshness rule fires again next cycle (EVALUATION.md same-day, todo rows
remain): skip Phase 1, recover T40 first.

### Cycle 18 (2026-09-26, ~00:41–01:30 EDT, continued) — T44 (operator pri-1 speed initiative) landed

**T44 (pipeline overlap doctrine) → done 41f61a1.** Operator filed
T44–T47 mid-cycle (58be922, "feels awfully slow"); priority doctrine
re-planned the queue to T44 first (pri 1). glm impl 23/50 FIRST-TRY (the
doctrine-item class: T32/T33/T34/T44 all land 11–23 iters) — §2 opening +
new "Pipeline overlap (T44)" paragraph + Hard-rules rewrite (ONE WRITER
per file set, serial merges; META-SPEC override scoped to launch
concurrency only; stale "foreground" dropped). kimi validation VERDICT:
PASS 19/50 — all 5 reqs verified, gates re-run independently (436+3 +
clippy), guard-liveness mutant (removed t44 spec) killed by
todo_consistency, prose-mutant survival adjudicated by-design (the suite
cannot police prose; detection = review + next-cycle events per spec
Tests), tree restored byte-identical (hash-object verified). 4
non-blocking observations (empty-target-list edge, fix-up-child cap
classification implicit, 2-validator sequencing implicit, prose
unpoliceable). 4 artifacts harvested. Acceptance is forward-looking:
observable when a later multi-item cycle's events.jsonl shows impl N+1's
run_start before validator N's verdict. **Deferred this cycle (budget
step-6 rule, ~10 iters left at T44 landing): T39, T40, T41, T42, T43,
T45, T46, T47 — all todo with ready specs; the freshness rule lets the
next cycle skip Phase 1 and start on the queue. Recommended order for the
next cycle per priority doctrine: T45 (pri 2 bundling — compounds with
T44; T40/T41/T42/T43 are its first bundle candidates), T46 (pri 2), T39
(pri 3 feature), T47 (pri 3), then the pri-4 set.**

### Cycle 18 (2026-09-26, ~00:41–01:10 EDT) — fresh eval (queue EMPTY at start) + T38 landed; TWO 50/50 child deaths, both recovered

**T38 (truncated-response advisory) → done a686522.** Fresh evaluation filed
6 rows (T38–T43); T38 worked first (pri 2). glm impl died 50/50 mid-wrap
(T18's budget_low@42 fired and was acknowledged — 5th of the 50/50 class;
227k/28.6k tokens) with the work complete-but-uncommitted (396 insertions,
exactly the right 6 files, README done). Orchestrator harvest-completed one
fix the dying child never got to run: its run_t38 scripted harness used
max_iters=10, so the T13 budget-low notice fired on turn 3 and landed AFTER
the second advisory, breaking the no-latch test's last-message assertion
(1/436 red as harvested; 10→20, 436+3 green, clippy clean → 364b95c). kimi
adversarial validation then died 50/50 ITSELF — post-mutations, pre-verdict:
10/10 mutants killed on record (condition delete/flip, token corrupt,
event/transcript/chat-injection drop, type-string corrupt, latch, ordering,
sink-silence), tree restored clean. Orchestrator re-verified (clean tree,
436+3) and RESUMED the validator via `chug run --resume` over bash nohup —
**delegate cannot express `--resume` (capability gap, watch item for the
next eval)** — which emitted VERDICT: PASS, goal accepted (all 7 reqs
verified, gates green twice consecutive post-restore; 3 non-blocking
observations: chat goal-accepted edge gets no advisory by design, TODO row
flip pending = orchestrator's job, one transient unnamed flake during
mutation runs). 4 artifacts harvested pre-removal (impl+validate events,
impl+validate LEDGERs). Also noted live: delegate status misreports a
RESUMED child — its events stream holds the prior run's abort, so status
said `state: aborted` while the resumed validator ran healthy (watch item;
ps fallback used). T41 sighting #3: orchestrator's own edit_file to the /tmp
worktree refused (bash python3 workaround).

### Cycle 17 (2026-09-26, ~00:31–00:45 EDT) — freshness-skip; T37 recovered + landed, QUEUE DRAINED

One `chug run --spec LOOP-SPEC.md` session (kimi-k3 orchestrator, loopd-launched; kimi-k3 validator child via delegate). **Freshness rule fired as designed** (cycle-16 eval same-day + T37 `todo` with ready spec + preserved worktree) → zero re-eval burn, straight to the queue's single row. T37's per-item entry is written here at landing per T34 doctrine (this commit IS the row-flip commit's companion).

**Landed (1/1 queued rows):**
- **T37 — `web_fetch` tool: read-only HTTP(S) GET, bounded** (pri 3, **feature** — the second half of LOOP-SPEC §2's named capability gap; impl `1432a2e` harvest-completed by the cycle-16 orchestrator from a 50/50-budget-aborted glm child, merge `1e82ffb`, row flip + this entry in the companion commits). Recovery per the T29 precedent: worktree `/tmp/chug-loop-t37` preserved at `1432a2e` → gates re-run (430+3 in 14.35s, clippy `-D warnings` clean) → kimi adversarial validation (REQUIRED — src/tools.rs) launched via delegate at the T32-widened 50/30 budget → **VERDICT: PASS 48/50** (budget_low@8 fired at 42, wrapped inside T18's margin — third consecutive child to finish in the ceiling zone this week, all surviving on the margin). Validator: all 7 spec reqs verified (schema, transport bounds, content handling, errors-as-tool-errors, sandbox candor, README integration, no new dep); risk gate judges bash only (driver.rs:621-623); events.jsonl ToolResult preview on every fetch (driver.rs:670); **10/10 mutants killed** — cap-20k→50k, ceiling-clamp removal, redirect `>=`→`>`, scheme-gate drop, binary-refusal→accept, script/style-strip off, truncation-branch removal, non-2xx-arm removal, marker-text corrupt, whitespace-collapse break — each died to exactly the expected test(s); gates green pre- AND post-mutations, tree restored clean. 3 non-blocking observations: (1) req-7 out-of-scope list not enumerated in the impl commit message (the merge commit records it); (2) connect-timeout constant not value-pinned by a test; (3) `skip_raw_block` prefix-matches a closing `</scriptx>` tag (harmless — swallows to the real close or EOF, no content leak). Merged `1e82ffb`; gates re-run independently in main post-merge (**430+3 green in 15.12s**, clippy clean); todo_consistency guard green post-flip. 2 artifacts harvested pre-removal (`events-t37-validate-20260926-043809.jsonl` + `LEDGER-t37-validate-20260926-043809.md`; impl events harvested in cycle 16). **The capability gap named in LOOP-SPEC §2 is now CLOSED — both halves shipped (delegate T23, web_fetch T37).**

**Skipped/deferred:** none — the queue is **drained** (T1–T37 all done with refs). A cold next cycle therefore cannot freshness-skip (the rule requires `todo` rows) and MUST run a fresh Phase-1 eval — the designed flow.

**What the validators caught:** no implementation defects — the kimi validator's 10/10 mutant kill confirmed the suite has teeth; its 3 observations are documentation/pinning nits, recorded above for the next eval's consideration (a const-value pin for the connect timeout is a candidate micro-row if the eval judges it worth filing).

**Cycle-level notes:** (a) **The recovery recipe worked exactly as written** — cycle-16's deferred-with-ready-state handoff (T29 precedent) consumed cold: worktree intact, gates re-run, validation dispatched, merged, flipped, pushed in ~15 min wall-clock and ~20 orchestrator iterations. Deferred-with-ready-state is now 2-for-2 (T29, T37). (b) **The two-eyes doctrine held across a child death**: the dying glm child shipped 2 real production bugs (every-fetch `read_body_capped` panic; `<!DOCTYPE` leak) that the cycle-16 orchestrator caught in harvest-completion review and the cycle-17 kimi validator then confirmed fixed — orchestrator-review + adversarial-validation is the load-bearing pair for the harvest-completion path. (c) **delegate+wait_secs polling profile**: validator lifecycle (48 iters, ~5 min) observed with 4 instant/wait_secs status calls + 3 bounded bash sleeps; the 2s early-wake pattern on a churning stream noted again — instant polls after sleeps remain the cheap cadence for active children. (d) Budget: wrap begins at ~25/120 iterations used — the freshness-skip + single-recovery-item cycle is the cheapest profile the loop has; T36's 160-cap (awaiting the operator's loopd restart) is sized for the fresh-eval + 3-item cycles, not this one.

**Final state:** main at the `todo:`/`eval:` companion commits atop merge `1e82ffb`; gates 430+3 green in main, clippy `-D warnings` clean; TODO.md truthful (T1–T37 ALL done with refs — zero `todo` rows); 3 t37 artifacts in `.chug/` (impl events from cycle 16 + validate events + validate LEDGER); README gate satisfied — T37's Tools bullet + prose sentence landed integrated in the Tools section with its merge (verified in the diff, req 6); everything pushed. **Handoff to the next cycle: queue EMPTY → freshness rule CANNOT fire → the next cycle MUST run a fresh Phase-1 eval** (META-META-SPEC §6 README-audit duty included — the Tools section gained a tool this cycle, so the audit re-verifies that section's integration). Carries for that eval: (1) the validator's 3 T37 observations (connect-timeout const pin candidate); (2) glm truncated-write death class watch (cycle-16 note — if it recurs, a write_file chunking hint row); (3) T31-residual flake watch (1 sighting / ~8 runs at cycle 16; 0 new sightings this cycle — 2 suite runs, both green); (4) the web_fetch organic-demand watch (the 6-quiet-eval grep now has a tool to measure adoption of — the eval's grep should now look for `web_fetch` tool calls in child streams); (5) human-decision carries unchanged (loopd restart activates T36's 160-cap).

### Cycle 16 (2026-09-26, ~23:55–00:35 EDT) — fresh eval (queue was EMPTY → rule couldn't fire) + T36 landed; T37 impl-complete, validation deferred (budget wrap)

**Landed:**
- **T36 — loopd `--max-iters` 120→160** (pri 2, robustness/throughput; impl `d5141c9` glm goal-accepted 15/50 ~2.5 min first-try, merge `0586e1e`, flip + this entry in the same commit per T34 doctrine). +2/−2 loopd.sh only (line 72 code + line 69 comment, cycle-16 arithmetic: Phase 1 ≈45–55, item ≈28–35, wrap ≈10; minutes never binding 56–117 of 240). Spec check verbatim green in worktree AND main post-merge; `bash -n` clean; gates re-run independently — worktree 416+3 (14.6s), main 416+3 (14.8s), clippy `-D warnings` clean both trees. kimi adversarial validation **VERDICT: PASS** 11/50 ~2 min — 5/5 spec-check mutants killed (160→120, 160→999, duplicate-line→2 occurrences, 240→300, model→kimi-k9), baseline+restored PASS, all 4 requirements itemized, tree byte-identical post-mutations, no-supervisor-touch verified. 3 artifacts harvested pre-removal (impl + validate events, validate LEDGER; impl LEDGER seed-trivial, skipped per T32-cycle precedent). **Flake-watch note**: the impl child's full-suite run caught ONE transient `dead_port_probe_distinguishes_live_from_dead` failure under parallel load ("dropped port did not refuse connections") — 5/5 green isolated, child's full-suite re-run green, orchestrator's worktree + main gate runs both green; a shell-script number+comment cannot affect a TCP probe. First organic T31-residual sighting; carried to §3's watch line at wrap. Activation: operator's next loopd restart (human-decision carry, T27 path).

**Deferred (with ready state — recover per T29 precedent):**
- **T37 — `web_fetch` tool** (pri 3, feature). glm impl child (pid 80693) **died 50/50 budget-abort** — its large `write_file` calls kept truncating mid-file (3×; a glm output-limit behavior on >30KB writes); it recovered via impl-first + chunked bash-heredoc test appends, wired everything, and died mid-gates **before ever running its tests** (`cargo build` doesn't compile test code — its final green build hid a red suite). Orchestrator harvest-completed (T15/T17/T28 precedent): **(1)** leg-1 test E0382 clone; **(2)** `read_body_capped` off-by-one — the UTF-8 step-back indexed `bytes[len]` at `len == bytes.len()`, so EVERY sub-cap body panicked in the worker (7 of 8 failures); guarded to real cuts; **(3)** `html_to_text` skipped `<!…>`/`<?…?>` declarations (tag_name_at requires `[A-Za-z]` after `<`); **(4)** `html_to_text` trims output + two test adjudications toward the spec legs (naive `!contains('<')` contradicted the test's OWN expected visible text → targeted leaked-tag asserts; the "unterminated tag" leg used a TERMINATED input → split into the true unterminated (→"") and terminated (→"truncated") cases). Result committed in the preserved worktree: branch `loop-t37`, commit `1432a2e`, suite **430+3 green** (+14 tests), clippy clean, README integrated. **Kimi adversarial validation (REQUIRED — src/tools.rs) + merge deferred at budget_low@8** — doctrine forbids merging unvalidated; T37 stays `todo` with the recovery recipe annotated on its row; the impl events stream is harvested (`events-t37-impl-20260926-042452.jsonl`) so a worktree loss costs no narrative.

**Cycle-level notes:** (a) **What the validators caught**: T36's kimi PASS found zero issues (5/5 check mutants killed) — but the T37 arc showed the ORCHESTRATOR-as-reviewer catching what a dying child couldn't: 2 real production bugs (every-fetch panic; declaration leak) that would have made web_fetch fail on its first real call, plus 2 test-quality bugs. The harvest-completion path (orchestrator fixes + validator still to come) preserves the two-eyes doctrine. (b) **glm truncated-write death** — a new child-death class: not budget-mismanagement but a model-side output limit on very large single writes; the child's own recovery (chunked heredocs) worked but cost the iterations that killed it. Watch item for the next eval: if it recurs, the row is a tool-side "large write" chunking hint in the write_file description. (c) **Flake watch — first organic T31-residual sighting**: one transient `dead_port_probe_distinguishes_live_from_dead` failure under parallel load in T36's impl suite run ("dropped port did not refuse connections" — a parallel test re-grabbed the just-released port); 5/5 green isolated + 4 subsequent full-suite runs green across two worktrees and main. Sample: 1 sighting / ~8 suite runs post-T31. Not filed; the §3 grep line stays. (d) **Budget**: wrap fired at 8 iterations / 210 minutes remaining — iterations remain the binding axis even at 120; T36 (120→160, landed this cycle) is the fix, active on the operator's next loopd restart. (e) **Final state**: main `a2127f0`+wrap commit; gates 416+3 green in main post-T36 (worktree suite is 430+3 — the +14 lands with T37); TODO.md truthful (T36 done with ref, T37 todo with recovery recipe); 4 artifacts harvested this cycle (t36 impl+validate events, t36 validate LEDGER, t37 impl events); README gate satisfied (T36 no user-visible surface; T37's README paragraph lands with its merge). **Handoff: next cycle's first act is T37's recovery (worktree preserved at /tmp/chug-loop-t37 — if /tmp was wiped, branch `loop-t37` in the main repo holds commit `1432a2e`; re-add a worktree from it): re-run gates, kimi-validate (REQUIRED), merge, flip, push. Then the queue is empty → freshness rule cannot fire → fresh eval.**

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
