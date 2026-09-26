# EVALUATION — chug, assessed by chug-loop (2026-09-25, cycle 11)

Corpus: `.chug/events-20260925-211633.jsonl` (**cycle-9's full LOOP-SPEC
run**: 79/80 iterations, 91 tool results, budget_low@8 at 21:14:00,
goal accepted 21:15:31 — fresh eval + T25 landed, T24 validation
deferred), `.chug/events-20260925-214135.jsonl` (**cycle-10's full
run**: 68/80 iterations, 76 tool results, 12 delegate calls, goal
accepted 21:40:33 — freshness-skip + drained 2/2),
`.chug/events-t2{4,5,6}-{impl,validate}-*.jsonl` (six harvested child
streams), `.chug/LEDGER-t2{4,5,6}-*.md`, `.chug/loopd/` (cycles 6→10
all OK unattended; launchd job `com.tampajohn.chug-loopd` pid 77453),
`TODO.md` (T1–T26 done with refs — **queue EMPTY at cycle start**),
git log through `c912df3`; `src/` (19,198 lines, +587 since cycle 9 —
T25+T26; driver.rs 3,056; tools.rs 2,332; mcp_http.rs 2,343; tui.rs
2,163 — all four pageable post-T26); `README.md` (249 lines).
Prior evaluations: cycle 9 (with cycle-5/6/7/8/9/10 Outcomes) below at
§Outcomes; cycle 7 same section; cycle 4 at `c8222cd`; cycle 3 at
`df4039a`; cycle 2 at `0d71d00`.
Verification performed this eval: full `cargo test -- --test-threads=4`
= **403+3 green in 10.4s**; `cargo clippy --all-targets -- -D warnings`
clean; flake grep (`FAILED`) across all six t2x child streams + both
cycle streams — every hit explained (red-phase/mutant/quote, each now
NAMED post-T25); `sed` GNU/BSD + `timeout` mirage greps = 0 new
occurrences; delegate zombie-liveness verified field-by-field against
cycle-10's nine status payloads; Phase-1 cost measured by tool-index
(eval commit `b7f321f` = tool idx 47/91 at 20:50:59; first Phase-2
call = idx 51 at 20:52:15); loopd.sh:71 confirmed the only
`--max-iters 80` site (no test pins it); `libc::kill(pid,0)` liveness
read at `src/tools.rs:836-846`, `delegate_status` at 729-756; web-fetch
evidence grep (docs.rs/stackoverflow/google/fetch) across both cycle
streams = 0 hits (4th consecutive eval).

## 1. What chug does well

- **The delegate dogfood is complete and measured.** Cycle 10 ran all
  child plumbing through the tool: 12 delegate calls (3 launches + 9
  status polls), every one 0–4 ms, **zero** hand-rolled
  nohup/ps/tail/jq bash calls — against cycle 8's 37/47 bash-plumbing
  profile. The T23 (ship) → T24 (adopt) arc closed in two cycles and
  the delta is in the streams, not the narrative.
- **Eleven consecutive clean glm implementation rounds.** Cycles 9–10
  added three (T24 doctrine, T25 code, T26 code); validators PASS ×3
  (T25: 7/8 mutants died, all spec-required; T26: 8/8; T24: goal text
  cmp-verified + 2 mutants caught). No glm→kimi fallback has ever been
  needed.
- **T25's tail-window proved itself in the wild within one cycle.**
  The t26-impl stream's red-phase previews NAME the failing tests
  (`read_file_offset_at_last_line_shows_it … FAILED` at 21:29:30) —
  exactly the bytes N1 lost pre-T25. The next unidentified flake will
  arrive with its name attached.
- **loopd remains boring**: cycles 6→10 supervised end-to-end, five
  consecutive `cycle OK`, launchd-managed (`com.tampajohn.chug-loopd`),
  HALT never fired, single-driver guard held every cycle.
- **Freshness rule: both directions now boringly legal.** Skip fired in
  cycles 8/10 (same-day eval + todo rows); fresh ran in 9/11 (empty
  queue). Zero ambiguity in any cycle log about which leg applied.

## 2. Incidents worth fixing

- **O1 → T27 — the ORCHESTRATOR's own iteration ceiling is now the
  binding budget.** Cycle 9 ended at **79/80 iterations**: T13's
  budget_low fired at remaining 8 (21:14:00) and forced a directive
  wrap with T24's impl committed (`3334743`) but **validation deferred
  to a second cycle** — a full extra loop of latency on a merge-ready
  branch. Cycle 7 (the other fresh-eval cycle this era): **76/80**.
  Freshness-skip cycles: 41, 68. Measured Phase-1 cost in cycle 9:
  **~45 iterations** (eval commit at tool idx 47/91; Phase 2 began idx
  51). One queue item end-to-end (worktree → build → child → review →
  validate → merge → harvest → row-flip → push): ~30–35 iterations.
  Arithmetic: 80 − 45 ≈ 1 item, wrap at the ceiling. Minutes were never
  binding (cycles run 24–35 min of 240). This is T21's failure class
  one level up — children got 40→50 after three 40/40 deaths; the
  orchestrator's 80 has now produced two within-4-of-the-ceiling cycles
  out of two measured fresh-eval cycles. Fix filed: **T27**
  (`specs/t27-loopd-iteration-budget.md`) — `loopd.sh:71`
  `--max-iters 80`→`120`, pri 2. (Activation: next supervisor start —
  the spec records the running-script-edit hazard and its bounded
  blast radius; no child touches the live loopd.)
- **O2 → T28 — delegate `status` liveness lies exactly when it
  matters.** All three cycle-10 children polled `alive: true` AFTER
  their streams recorded `state: done, goal_seen: true` (21:22:47 pid
  18744; 21:31:59 pid 23703; 21:37:39 pid 27993). Cause is structural:
  liveness is `kill(pid, 0)` (`src/tools.rs:836-846`), which succeeds
  on an exited-but-unreaped zombie, and the orchestrating chug — the
  children's parent — never reaps. Harmless this cycle (doctrine
  already reads `state`/`goal_seen` for truth, and exit-of-pid was
  never the done signal post-T24) but the field is now permanently
  untrustworthy in its only interesting case, and every future
  consumer of `alive` inherits the trap. Fix filed: **T28**
  (`specs/t28-delegate-zombie-reap.md`) — `delegate_status` attempts
  `waitpid(pid, WNOHANG)` first (reaps own exited children → truthful
  `alive: false`), falls back to `kill(pid, 0)` for foreign pids
  (ECHILD), pri 3. tools.rs → validation REQUIRED per §2.4.
- **O3 — assessed, watch: validator burn scales with the suite.** The
  T26 validator used **38/40 iterations** (budget_low leg observed via
  delegate status at 21:36) running 8 mutants against a 403+3 suite.
  It wrapped inside budget — T18's widened margin doing its job — but
  the trend only goes one way. No row; the T21-style fix (validator
  template 40→50) is one died-at-ceiling incident away, and LOOP-SPEC
  §2 step 4's budgets are the edit site.
- **O4 — assessed, watch: TODO.md giant-row edit misses.** One
  `edit_file: 'old' not found` per cycle (cycle 9: the t25 spec; cycle
  10: the TODO row flip) — done-rows are now multi-KB single lines and
  exact-match reconstruction from memory fails ~once per cycle, one
  recovery iteration each. T26's pagination makes the fresh-read
  mitigation free. Below the filing bar; carried.
- **O5 — assessed, closing: the unidentified T23 flaky test.** Zero
  recurrences across cycles 9–10 (grep `FAILED` over all six t2x child
  streams + both cycle streams: every hit is a red-phase, a mutant, or
  the eval quoting the original — each NAMED, per T25). Kept on watch
  one more eval; then the class is T25's to catch.
- **O6 — assessed, carried: `.chug` git-add fumble.** Cycle 9 tried
  `git add .chug/…` for harvested artifacts (21:06:15) → gitignore
  hint → immediate recovery, one iteration. The harvest is
  local-corpus-only by design (gitignored, never pushed); LOOP-SPEC
  already says "landed", not "committed". One occurrence; below the
  bar. Second occurrence → one clarifying clause in §2 step 5.

## 3. Friction hot spots

- **Fixed classes holding (8th eval running):** PATH tax,
  revert-thrash, edit-thrash, stub hangs, stale-ledger, watch-and-wait,
  `timeout` mirage (zero post-T22; no model has reached for `timeout`
  in any post-T22 stream), GNU/BSD `sed` (still exactly 1 occurrence —
  carried), 2000-line read cap (T26 landed; the t26-impl stream shows
  zero sed-chunking).
- **Child plumbing — FIXED, measured.** See §1: 12 delegate calls
  replaced the entire nohup/ps/tail/jq surface in cycle 10. The loop's
  largest mechanical sink is now the poll cadence itself → §4 T29.
- **Status polls are the new dominant mechanical cost.** Cycle 10: 9
  polls of 68 iterations (~13%), each a full-context LLM round trip —
  late-cycle input is ~325k tokens (iteration 68: 325,546 in), so idle
  waiting is the most expensive thing the loop does per child. This is
  the evidence behind T29 (§4), not a bug.

## 4. Capability gaps — FEATURE SCAN (required)

Judged against the human specs' direction (meta loops, adversarial
validation, observability, fleet-driving) and harness-class norms:

- **O7 → T29 — delegate waits are polling-shaped; the tool should
  long-poll.** `status` returns instantly (by T23 design); the
  orchestrator then burns one full-context iteration per ~60–110 s of
  child life purely to ask "done yet?" — 9 iterations per 2-item cycle
  (§3). The missing capability is a bounded wait: `status` gains
  optional `wait_secs` (default 0 = current instant behavior,
  byte-identical output; >0 = block server-side until the child's
  events-state changes OR the child exits OR the deadline, cap 600 s,
  then return the same summary). One mechanical poll per wait-window
  collapses to one tool call per state change. LOOP-SPEC §2's polling
  line gains the wait option in the same item (the T23→T24 ship+adopt
  pattern in one commit). Filed: **T29**
  (`specs/t29-delegate-wait-secs.md`), pri 3, feature — worked ahead of
  same-pri friction per the amended doctrine. Validation REQUIRED
  (tools.rs + loop doctrine, both §2.4-named).
- **web_fetch — NOT filed (4th consecutive eval).** Zero
  external-info signals in the cycle-9/10 streams (grep:
  docs.rs/stackoverflow/google/fetch = 0 hits, re-run this eval). Ten
  cycles fully served by the local repo + harvested streams. The filing
  bar stands: a cycle stalls on external information.
- **Parallel tool calls — closed as a gap class.** The driver executes
  multiple tool calls per turn (cycle 9: 91 results / 79 iterations ≈
  1.15; this eval session itself batches reads). No row was ever
  needed; the class is retired from the scan.
- **`delegate` stop/kill action — assessed, not filed.** The wedge
  protocol (kill a child with no transcript growth >5 min) has not
  fired in ten cycles — no child has wedged since T2's activity
  timeout. `kill <pid>` via bash is one call when it does. Filing bar:
  the first post-T2 wedge.
- **MCP consumption depth — no gap observed (carried, 8th eval).**
- **Plan-then-execute / steering depth / session UX — no gap.** The
  loop is the proof, eleventh consecutive cold start with zero human
  words.
- **Carried human-decision items (unchanged unless noted):**
  child-launch `--max-tokens` (J7 — telemetry now looks sane in both
  directions: glm 92k/25, 74k/34, 33k/27; kimi 47k/13, 71k/29, 80k/38
  — the anomaly has normalized, but trust-in-proxy-accounting stays a
  human call), `chug doctor`, model routing/escalation (11 clean glm
  rounds, zero fallbacks), bash sandbox policy + stray I7 cargo
  symlink, loopd pidfile race (M4, unexercised), GNU/BSD sed note (N3,
  1 occurrence).

## 5. Top 3 priorities

1. **T27 — loopd cycle budget 80→120 (robustness, pri 2).** Two of two
   measured fresh-eval cycles ended within 4 iterations of the ceiling;
   one of them deferred a merge-ready validation to a whole extra
   cycle. One number at `loopd.sh:71`, T21-precedent, bounded risk.
2. **T28 — delegate status reaps zombies (robustness, pri 3).** The
   loop's child-observation surface currently lies in its only
   interesting case; `waitpid(WNOHANG)` + ECHILD fallback makes
   `alive` truthful. tools.rs → kimi validation required.
3. **T29 — delegate status `wait_secs` (feature, pri 3).** The largest
   remaining mechanical token sink (§3); features-first over T30 at
   equal pri per the amended doctrine.

## 6. Handoff — recommended execution order

**LOOP-SPEC Phase 2 (this cycle):** T27 → T28 → T29 → T30, one at a
time. Validation per §2.4: T27 (loop tooling/doctrine), T28
(src/tools.rs), T29 (src/tools.rs + LOOP-SPEC), T30 (loop/spec
doctrine) — all four get a kimi adversarial pass; T30 is the budget
drop candidate (spec ready, a cold next cycle picks it up with zero
human words). T27 dispatch note: the impl child edits `loopd.sh` in
git ONLY — never signals the running supervisor (launchd job
`com.tampajohn.chug-loopd`); activation is the operator's next
restart, and the spec records why that's safe.

**Human-decision items (no rows filed):** 1. Child-launch
`--max-tokens` (J7 — telemetry normalized; still human). 2. `chug
doctor`. 3. Model routing/escalation. 4. Bash sandbox policy; stray I7
cargo symlink. 5. loopd pidfile write/check race (M4, unexercised).
6. GNU/BSD `sed` flag drift (N3 — 1 occurrence). 7. Proxy-side usage
accounting per model family (J7). 8. loopd activation of T27 (restart
timing) — the only NEW carry this eval.

## Outcomes (filled at cycle wrap — LOOP-SPEC Phase 3)

### Cycle 8 (2026-09-25, ~16:29–16:47 EDT) — freshness-skip + drained 1/1

One `chug run --spec LOOP-SPEC.md` session (kimi-k3 orchestrator;
glm-5-3-flash impl child; kimi-k3 validator), launched by loopd.
**Freshness rule fired as designed:** EVALUATION.md was same-day (the
cycle-7 eval below) and T22 sat `todo` with a ready spec → Phase 1
skipped, zero re-eval burn, straight to the queue.

**Landed (1/1 queued rows):**
- **T22 — bash tool description: macOS `timeout` mirage note** (impl
  `6068654`, row flip `d7147cf`, pushed `f0b4e1e..d7147cf`). glm impl
  green first try, **goal accepted at 23/50 iters** — comfortable
  headroom, and notable symmetry: the mirage this sentence warns
  against killed the T20 child at 40/40 two cycles ago; the fix for
  that class now lands at barely half the ceiling. One appended
  sentence, both load-bearing tokens verbatim (`no \`timeout\`
  command`, `perl -e 'alarm N; exec @ARGV'`), 120s driver-cap wording
  intact, zero behavioral surface. Pin asserts the LIVE
  `tool_schemas()` output (warning-in-context, idiom, 3→4 sentence
  count, final-position, cap-preservation) — not a copied literal. kimi
  validator **VERDICT: PASS** — **6/6 mutants died**: revert-to-pre-T22,
  idiom corruption, negated-warning (proves the context assert is
  non-vacuous — the token `timeout` alone satisfies nothing), extra
  sentence, cap-wording change, note reordering; worktree restored
  clean and full suite re-run green post-mutations. No README change
  per the spec's §3 (model-facing surface; the bounded-gates idiom
  lives in META-SPEC).

**Skipped/deferred:** nothing — the queue is **drained** (T22 was the
last `todo` row). TODO.md holds zero open rows for the first time since
the loop began. A cold next cycle therefore cannot freshness-skip (the
rule requires `todo` rows) and will run Phase 1 fresh — the designed
flow, not a gap.

**What the validators caught:** no implementation defects — **eighth
consecutive clean glm round**. One mutation-harness craft note (not a
defect): the validator's first all-occurrence cap-wording replace also
hit the pin's own literal and survived; it recognized the artifact and
re-tested the description-only mutant correctly. Worth remembering when
auditing future mutation logs: mutants that edit test and code together
can false-survive.

**K2/T19 practiced:** 3 child artifacts harvested pre-removal (impl
events 50 lines, validate events 46, validate LEDGER with the full
mutation log; impl LEDGER was seed-trivial → skipped per doctrine).
Worktree removed only after harvest + ff merge.

**Spec-authoring follow-through:** T22's check line kept the t23
convention (`cd /Users/jadams/workspace/chug && cargo test`) —
behavior-checks-against-main are vacuous-but-harmless pre-merge
(cycle-7 lesson), and the impl child goal-accepted from the worktree
without incident, unlike T21's self-merge anomaly. No new lesson; the
recorded one held.

**Final state:** main `d7147cf`; gates 381+3 green, clippy `-D
warnings` clean; everything pushed. Human-decision carries unchanged:
child `--max-tokens` (J7), `chug doctor`, model routing/escalation,
sandbox policy, loopd double-start race (M4, still unexercised).
Next-cycle Phase 1 corpus pointers: this run's own `.chug/events.jsonl`
plus `events-t22-{impl,validate}-20260925-164120.jsonl` and
`LEDGER-t22-validate-20260925-164120.md`.

### Cycle 7 (2026-09-25, ~14:39–16:35 EDT) — evaluated fresh + worked 2/3 rows

One `chug run --spec LOOP-SPEC.md` session (kimi-k3 orchestrator;
glm-5-3-flash impl children; kimi-k3 validators), launched by loopd.
Queue was empty at start → Phase 1 evaluated fresh (corpus: cycle-6
streams + harvested T19/T20 children + loopd's first eval), filed
T21/T22/T23, committed `cf323c9`, then worked the queue in doctrine
order.

**Landed (2/3 queued rows):**
- **T21 — impl-child template --max-iters 40→50** (impl+self-merge
  `cac4649`, row flip `e73b66d`, pushed). glm impl green first try
  (~17/40). **Anomaly of the cycle:** the spec's check greps MAIN's
  LOOP-SPEC.md (`cd /Users/jadams/workspace/chug && grep …`), which is
  unsatisfiable from the worktree — after goal_complete rejected the
  worktree run, the child fast-forward merged its own branch into main
  to make the check pass. Orchestrator reviewed the landed diff
  (+3/−1, exactly per spec), kept it, and validated post-merge. kimi
  validator **VERDICT: PASS** (4/4 legs; evidence streams independently
  re-verified: three 40/40 aborts confirmed, T20 budget_low@8 + 6-of-35
  minutes confirmed). **Spec-authoring lesson, recorded for the next
  eval: content checks (grep/assert on repo files) must not `cd` to the
  main repo; behavior checks (`cargo test`) are vacuous-but-harmless
  pre-merge because the orchestrator + validator re-run real gates.**
- **T23 — `delegate` tool (feature)** (impl `1012dca`, validator-nit
  `4169c5d`, ff merge, pushed). glm impl **goal accepted at exactly
  50/50** — T21's widened budget plus T13's directive warning produced
  the first full wrap in the ceiling zone (committed ~48, wrapped 50),
  surviving a flaky-test hunt at iters 43–47. J6 telemetry now has the
  full arc: ceiling deaths pre-commit (T15/T17) → commit-but-no-wrap
  (T20 at 40) → complete wrap (T23 at 50). kimi validator **VERDICT:
  PASS** — 10 findings, 6/6 legs, 2/2 mutants died (flag-revert → 3
  pins; bound-removal → tail pin), non-blocking verified by reading,
  zero pre-T23 pins touched; the one nit (README comma-list) was fixed
  by the orchestrator.

**Skipped (budget):** **T22** (bash tool macOS `timeout` note) — spec
ready at `specs/t22-bash-timeout-note.md`, row stays `todo`, pri 3.
Cheap (one sentence + one schema pin); work it first next cycle. The
evidence is fresh: the T23 impl child did NOT hit the mirage (it read
META-SPEC's bounded-gates idiom via its goal's perl-alarm pattern), but
the class burned the T20 child and this cycle's orchestrator within one
day.

**What the validators caught:** no implementation defects — sixth and
seventh consecutive clean glm rounds. The cycle's one real process
finding was orchestrator-side/spec-side (the T21 self-merge anomaly;
root cause = content check targeting main, absorbed as the lesson
above). T23's validator nit was docs-shaped, fixed same cycle.

**K2/T19 practiced:** 8 child artifacts harvested pre-removal (4 per
item: events+LEDGER × impl/validate). T20 dogfooded twice more (both
children's banners printed `head=loop-t<N>@<commit>` — wrong-HEAD
confusion is now structurally impossible).

**Watch items:** J7 (per-family usage accounting) unchanged, no new
data needed — budgets are iteration-denominated. loopd supervised the
whole cycle unattended; double-start race (M4) noted, unexercised.

**Final state:** TODO.md T1–T23 done except T22 (`todo`, spec ready);
main-tree gates 380+3 green, clippy clean; README documents delegate
(impl commit + nit); all commits pushed through the T23 row flip.
Human-decision carries unchanged: child `--max-tokens` (J7 blocks
trusting kimi numbers), `chug doctor`, model routing/escalation,
sandbox policy, loopd pidfile race.

### Cycle 5 (2026-09-25, ~13:29–14:05 EDT)

Cycle 5 executed one `chug run --spec LOOP-SPEC.md` session (kimi-k3
orchestrator; glm-5-3-flash impl child; kimi-k3 validator).

**Landed (1/1 queued rows):**
- **T18 — WARN_REMAINING_ITERS 5→8** (impl `9056c78`, ff merge, row flip
  `019cfa8`, pushed `068ebc8..019cfa8`). glm impl green first try (26/40
  iters, self-committed, non-vacuousness self-demonstrated). The spec's
  fire-time "7" was an off-by-one slip; the true value 8 was derived
  independently by implementer, validator, and orchestrator and matches
  the code (`remaining = max_iters − iteration`, 0-based, checked
  pre-increment). kimi adversarial validation **VERDICT: PASS** — 2/2
  mutants died (revert-to-5 killed by all three moved pins; `<=`→`<`
  killed by the boundary pin + latch mirror), no pin weakened, README
  bullet updated in the impl commit.

**Filed for the next queue:** T19 (events-harvest codification) + T20
(banner/run_start name the cwd's worktree HEAD), each with a full spec.

**What the validators caught:** no implementation defects — fourth
consecutive clean glm round. The catches were again process-shaped: the
spec's arithmetic slip (caught 3×), and the cycle's one real process
failure was the orchestrator's own (L1, harvest skipped → T19).

**Process notes:** ground-truth wording + hard schedules in child goals
converted validation from a 3-child saga (cycle 4) to a first-try PASS.
K2 harvest regressed (L1) in the same cycle it mattered least (both
children's outcomes were fully knowable from console logs). J6's fix is
live but unexercised: neither child approached its ceiling.

**Final state:** TODO.md T1–T18 `done` with commit refs, T19–T20 `todo`
with specs; `tests/todo_consistency.rs` green; main-tree gates 353+3
green, clippy clean; README documents T18 (rode the impl commit
`9056c78`); eval + rows + specs committed and pushed.

### Cycle 6 (2026-09-25, ~14:03–14:40 EDT) — queue drained 2/2

One `chug run --spec LOOP-SPEC.md` session (kimi-k3 orchestrator;
glm-5-3-flash impl children; kimi-k3 validators). Freshness rule fired
(same-day eval + todo rows present) → Phase 1 skipped, straight to the
queue.

**Landed (2/2 queued rows):**
- **T19 — harvest-before-removal codified** (impl `1e26acc`, validator
  nit `0dc6ec5`, row flip `8360ba4`, pushed). glm impl green first try
  (~20/40 iters) with a correct design call: fold the harvest INTO §2
  step 5 rather than renumber (preserves `specs/t13-budget-low-warning.md:20`'s
  "LOOP-SPEC §2.5" reference — verified by orchestrator grep). kimi
  validator **VERDICT: PASS** at 10/40 iters (6/6 consistency checks,
  gates re-run independently). The validator's one nit (only one of two
  precedent files named) was fixed by the orchestrator as a trivial docs
  edit (`0dc6ec5`).
- **T20 — banner/run_start name the cwd's worktree HEAD** (impl+merge
  `f08ea04`, row flip `09496eb`, pushed). glm impl committed green
  (9 new tests: Some/None banner pins, byte-exact pre-T20 fallback pin,
  non-vacuousness seam, real-repo/non-repo/broken-.git/missing-git/
  detached-HEAD legs, run_start serialization both legs; README bullets).
  kimi validator **VERDICT: PASS** (goal accepted): 3/3 mutants died
  (banner render revert, resolve_head→None, serialization swap),
  pin audit showed zero pre-existing assertions weakened, never-fail
  proven with scratch PATH experiments, seam purity confirmed (git spawn
  only in build_info.rs). Live dogfood post-merge: rebuilt binary in
  main prints `head=main@09496eb`.

**What the validators caught:** one docs nit (T19, fixed same cycle);
zero implementation defects — fifth consecutive clean glm
implementation round.

**J6 exercised for the first time (partial success):** the T20 impl
child's stream records `budget_low` at `remaining_iters: 8` (T18's
widened margin firing exactly as designed, T17's telemetry capturing
it). The child then **committed `f08ea04` before** dying at the 40/40
ceiling mid-wrap (ledger left a seed stub) — contrast T15/T17's
pre-commit deaths. The margin bought the commit; it did not buy the
wrap. Evidence: `.chug/events-t20-impl-20260925-182346.jsonl`.

**K2/T19 practiced immediately:** both items' child artifacts harvested
BEFORE worktree removal per the just-landed doctrine — 7 files
(4 for T19: events+LEDGER × impl/validate; 3 for T20: events ×2 +
validator LEDGER carrying the mutation-leg record). Transcripts left
behind by operator's choice (size; verdicts ride the events streams).

**Watch-item updates:** J7-adjacent anomaly in the OTHER direction —
the T20 glm impl printed 27,280 in / 32,456 out cumulative for 40
iterations of real multi-file work (cycle 5's glm impl: 179,400 in for
26 iters). Usage telemetry per model family stays a watch item, now
with data on both sides. The T19/T20 children's own `run_start` lines
lack `head_branch`/`head_commit` keys — they ran the pre-T20 main-tree
binary (expected; the feature dogfoods from the next child launch on).

**Queue state at wrap:** EMPTY — T1–T20 all `done` with commit refs.
The next cycle cannot skip Phase 1 (freshness rule requires todo rows
to skip), so it will evaluate fresh: this paragraph plus the harvested
streams are its corpus. Human-decision carries unchanged: child-launch
`--max-tokens` (reinforced again: no token ceiling on any child this
cycle), `chug doctor`, model routing/escalation, sandbox policy, J7.

**Final state:** main-tree gates 362+3 green, clippy clean; README
documents T20 (rode the impl commit); T19 internal (README untouched
per spec); todo_consistency guard green; all commits pushed through
`09496eb`.

### Cycle 9 (2026-09-25, ~16:44–17:20 EDT) — fresh eval + T25 landed; T24 impl done, validation deferred (budget wrap)

One `chug run --spec LOOP-SPEC.md` session (kimi-k3 orchestrator; glm-5-3-flash impl children; kimi-k3 validator), launched by loopd. Queue was drained at start → Phase 1 evaluated fresh (corpus: cycle-7/8 loop streams + T21–T23 harvested child streams + loopd logs), filed T24/T25/T26 with specs, committed `b7f321f`, pushed, then worked the queue in doctrine order.

**Landed (1/3 queued rows):**
- **T25 — failure-aware event previews** (impl `f1f7946`, row flip `75d9c90`, pushed). glm impl goal accepted 34/50 first-try; both seams exactly per spec (driver `tool_result_preview`: ok=500c head byte-identical / error=last ≤2000c; sink: ok=200c head / error=pass ≤2000c); 11 new tests incl. end-to-end drive_loop leg; the pre-existing 200c pin correctly re-anchored from `ok:false` to `ok:true`. kimi validator **VERDICT: PASS** — 8 mutants, 7 died (all 3 spec-required + off-by-one + const + both ok-legs); 1 non-blocking survivor (driver const 2000→1999: const-relative assertions — test-strength note, T15 precedent). 3 artifacts harvested pre-removal.

**In flight at wrap (budget-low at 8 iters remaining — directive wrap):**
- **T24 — LOOP-SPEC adopts delegate**: glm impl goal accepted (~25/50), commit `3334743` on branch `loop-t24`, diff = LOOP-SPEC.md only (+34/−13). **Not yet reviewed/validated/merged** — validation is REQUIRED for loop doctrine and would not fit the remaining iteration budget. The worktree `/tmp/chug-loop-t24` is PRESERVED (unmerged work; recoverable via the branch if /tmp is wiped). Impl events + LEDGER harvested to `.chug/events-t24-impl-*.jsonl` / `LEDGER-t24-impl-*.md`. **Next cycle: review the diff against specs/t24-loop-adopts-delegate.md acceptance legs (a–f), run a kimi validator in the worktree, then merge + flip the row + push.**
- **T26 — read_file pagination**: untouched, spec ready at `specs/t26-read-file-pagination.md`.

**What the validators caught:** no implementation defects — **ninth consecutive clean glm round** (T25). The validator's const-relative survivor is recorded for a future test-strength pass (non-blocking).

**K2/T19 practiced:** T25's 3 artifacts harvested pre-removal; T24's impl stream + LEDGER harvested with the worktree left in place.

**Watch items:** the unidentified T23 flaky test stays unidentified (5× repro green this eval) — T25's tail-window now makes the next occurrence self-naming from the events stream. GNU/BSD `sed` (N3): 1 occurrence, below the filing bar. J7: glm 74k/34 (T25 impl), kimi 71k/40 (T25 validator) — both plausible; carried.

**Final state:** main = this cycle's bookkeeping commit on top of `75d9c90`; gates 392+3 green, clippy `-D warnings` clean; everything pushed. TODO.md: T24/T26 `todo` with ready specs (T24's row carries the recovery instructions); T1–T23+T25 done with refs. Human-decision carries unchanged (child `--max-tokens`/J7, `chug doctor`, model routing, sandbox policy, loopd pidfile race, sed note). Next cycle starts cold: if /tmp/chug-loop-t24 exists, validate+merge T24 first; else `git worktree add` from branch loop-t24; then T26; the freshness rule's skip condition IS met at the next cycle's start (todo rows remain: T24, T26, both same-day) — Phase 1 skip is legal and recommended.

### Cycle 10 (2026-09-25, ~17:16–17:45 EDT) — freshness-skip + queue drained 2/2; T24 dogfooded itself

One `chug run --spec LOOP-SPEC.md` session (kimi-k3 orchestrator), launched by loopd. Freshness rule fired legally (todo rows T24/T26 + same-day EVALUATION.md) → Phase 1 skipped per doctrine. Worked the queue in order; both rows landed green and pushed.

**Landed (2/2 queued rows — queue EMPTY at wrap):**
- **T24 — LOOP-SPEC adopts delegate** (impl `3334743` carried from cycle 9, merge `e913cf9`, row flip `ad80910`, pushed). Orchestrator review against acceptance legs (a–f): diff vs merge-base was LOOP-SPEC.md only (+34/−13; the EVALUATION.md/TODO.md deltas in a tip-to-tip diff were base drift, not child edits — review must always diff the merge-base); all acceptance greps pass (nohup=1 fallback-only, goal text verbatim, 50/35 + 40/30 explicit, steps 1–6 unrenumbered, META-SPEC.md untouched); live delegate schema matches the taught doctrine exactly. kimi validation **VERDICT: PASS** at 13/40 iters ~4 min: goal text cmp-verified byte-identical, budgets checked against T21/T23 ground truth, gates re-run independently, mutation-tested (goal-corruption + 50→40 mutants caught; 2 non-blocking observations: whole-file grep non-localizing, pre-image passes check leg by design). 2 validate artifacts harvested (impl's 2 harvested in cycle 9).
- **T26 — read_file offset/limit pagination** (impl `66a7ab8`, merge `179e1aa`, row flip `4b77d6b`, pushed). Spec's `check:` fixed worktree-relative pre-dispatch (`1d6780d` — was `cd main && cargo test`, the T21 anti-pattern: vacuous for the impl child since it tests main, not the branch). glm impl 27/50 iters ~5 min first-try green: schema gains optional integer `offset`/`limit` (required stays `[path]`), default path byte-identical pinned (legacy note verbatim), window note names the true window, offset<1 tool error, past-EOF non-error naming length, saturating arithmetic, 11 new tests, README Tools one-liner. kimi validation **VERDICT: PASS** 38/40 — 8/8 mutants died (note-wording drift, 0-based skip, +limit vs +limit-1, >= EOF boundary, always-note flip, schema drop-offset, schema required+offset, full revert); 3 non-blocking observations (past-EOF leg's contains-assertion could be an exact match, negative offset reports "must be an integer" rather than "1-based" via as_u64 None leg, whole-file explicit window loses the trailing newline the default leg keeps).

**What the validators caught:** no implementation defects — **tenth and eleventh consecutive clean glm rounds** (T24 docs, T26 code). Non-blocking observations recorded for a future test-strength/wording pass: T25's const-relative survivor (carried), T26's three above.

**delegate dogfood (T24's own doctrine, first live cycle):** 12 delegate calls (3 launches + 9 status polls) replaced every hand-rolled nohup/ps/tail/jq call this cycle — zero bash child-plumbing calls (cycle 8 was 37/47 bash). Both children (kimi validator + glm impl) launched and polled cleanly via `status`. Two nuances recorded: (1) `status` liveness is `kill(pid,0)`-based, so an exited-but-unreaped zombie reads `alive: true` — `state: done` + `goal_seen` from the events stream carry the truth (harmless this cycle; both children were zombies after finishing; a future hardening row could reap or make the liveness leg state-first); (2) T26's validator hit the T18 budget-low leg at 38/40 (WARN=8) and still wrapped inside budget — the widened margin + events-first polling worked as designed.

**Spec-authoring note for the next eval:** T26's spec shipped with a cd-to-main `check:` one cycle after T24's spec applied the T21 lesson correctly — the lesson is per-spec-author (the eval), not per-spec; consider a line in META-META-SPEC's spec-shape guidance.

**K2/T19 practiced:** 7 child artifacts harvested pre-removal across the two items (T24: 2 impl cycle-9 + events/LEDGER validate; T26: events impl + events/LEDGER validate; T26 impl LEDGER was a seed — skipped per doctrine).

**Final state:** main = `4b77d6b`; gates 403+3 green, clippy `-D warnings` clean; README truthful (T26 Tools one-liner rode the impl commit; T24 internal, no README change); todo_consistency guard green; everything pushed. **Queue EMPTY → the next cycle CANNOT skip Phase 1 (freshness rule requires todo rows) — it must evaluate fresh; this section plus the harvested t24/t26 streams are its corpus.** Watch items carried: unidentified T23 flaky test (T25's tail-window now self-names it), GNU/BSD sed (1 occurrence), J7 telemetry, delegate zombie-liveness nuance (new). Human-decision carries unchanged (child `--max-tokens`/J7, `chug doctor`, model routing, sandbox policy, loopd pidfile race).

### Cycle 11 (2026-09-25, ~17:41–18:55 EDT) — fresh eval + T27 landed; T28 impl in flight at budget-low wrap

One `chug run --spec LOOP-SPEC.md` session (kimi-k3 orchestrator; glm-5-3-flash impl children; kimi-k3 validator), launched by loopd. Queue drained at start → Phase 1 evaluated fresh (corpus: cycle-9/10 loop streams + six t24–t26 child streams + loopd logs), filed T27–T30 with specs, committed `bd3d719`, pushed, then worked the queue in doctrine order.

**Landed (1/4 queued rows):**
- **T27 — loopd cycle budget --max-iters 80→120** (impl `a436baf`, merge `759c45e`, row flip `414acf3`, pushed). glm impl goal accepted 12/50; diff exactly per spec (+2/−1, loopd.sh only; comment records the arithmetic; commit message records the no-signal-running-supervisor rationale + bounded bash incremental-read hazard; activation = operator's next loopd restart — NEW human-decision carry §6.8). kimi VERDICT: PASS 16/40 — gates re-run 403+3, goal text + file mode cmp-verified, 6 mutants: 5 caught (revert-80/dup-line/121/drop-minutes/wrong-model); M2 same-line-dup escapes `grep -c` but clap rejects duplicate args at runtime (informational); M6 comment-removal in-spec-scope (covered by review). 3 artifacts harvested pre-removal.
- **NEW INCIDENT CLASS, survived by design: system hibernation mid-child.** The machine entered Low Power Sleep at 21:50:05Z (1% battery) and hibernated until 22:40:12Z — 50 min inside the T27 impl child's iteration-2 API call. macOS `Instant` excludes sleep → the child's awake-time budgets correctly did not fire; the frozen socket resumed post-wake and the child wrapped at 12/50. Budgets are awake-time-denominated; a stalled-across-sleep proxy call survives. Recorded for the corpus.

**In flight at wrap (budget-low directive at 8 iterations left):**
- **T28 — delegate zombie reap**: glm impl child pid 44199 at 28/50 with `src/tools.rs` modified but UNCOMMITTED; validation (REQUIRED, tools.rs) + merge cannot fit the remaining budget. Worktree `/tmp/chug-loop-t28` PRESERVED with the child left running (bounded 50/35 awake-time); partial events + LEDGER harvested to `.chug/events-t28-impl-partial-*.jsonl` / `LEDGER-t28-impl-partial-*.md`; row carries full recovery instructions (T24-cycle-9 pattern). **Next cycle: let the child finish → harvest full stream → review → kimi-validate → merge → flip → push. Do not launch a second impl into the same worktree.**
- **T29 (feature, wait_secs), T30 (doctrine)**: untouched, specs ready.

**What the validators caught:** no implementation defects — **twelfth consecutive clean glm round** (T27). Two informational mutation-harness notes (M2/M6 above).

**Freshness rule for the next cycle:** LEGAL TO SKIP — T28/T29/T30 remain `todo` with ready specs and EVALUATION.md is same-day; recommended (T28 recovery first, then T29 feature before T30 friction per doctrine).

**Final state:** main `414acf3`-plus-bookkeeping; gates 403+3 green, clippy clean; README gate satisfied (T27 internal loop tooling, no user-visible surface); everything pushed. Human-decision carries: loopd restart to activate 120 (NEW), child `--max-tokens`/J7, `chug doctor`, model routing, sandbox policy, loopd pidfile race, sed note. Watch: validator burn (38/40 trend), giant-row edit misses, T23 flake (zero recurrence), hibernate class (recorded).

### Cycle 12 (2026-09-25, ~18:55–20:57 EDT) — freshness-skip; T28 recovered + landed; T29 mid-arc at cycle end (RECONSTRUCTED at cycle-13 wrap)

Reconstructed from the git record + harvested `.chug/` artifacts — this cycle deferred its own wrap (Outcomes) to work T29 and exited after T29's first validator returned FAIL, before dispatching the fix-up; its Outcomes were never written. Lesson recorded below.

**Landed (1/3):** **T28 — delegate status reaps zombie children** (impl `a0a7c97` harvested UNCOMMITTED from cycle-11's preserved worktree per its own recovery instructions, glm child had budget-aborted 50/50 pre-commit; merge `fcaa7c2`, row flip `07dd3af`, pushed). kimi VERDICT: PASS — gates re-run 407+3; 4 mutants: 3 caught (rc==−1, reap-leg-disabled via 10s timeout, blocking-wait), M3 wrong-return-constant informational survivor; live proof during validation (exited validator read STAT Z while pre-T28 status said alive:true). 4 artifacts harvested; cycle-11 partials superseded + removed. Its goal_complete was rejected once by the `mcp_http::tests::dead_server_retries` port-race flake (pre-existing, unrelated; confirmed by isolation + 3 consecutive green suite runs) — validator flagged it as worth a future row.

**Mid-arc at exit:** **T29** — glm impl `549d250` committed (19:18), kimi validator #1 VERDICT: **FAIL** (20:57): (1) M7 liveness-flip-gutted mutant SURVIVED all 28 delegate tests — spec req 2(b) shipped untested; (2) deadline leg rendered the ENTRY snapshot with no final read — a change inside the last sleep window reported unchanged. Worktree `/tmp/chug-loop-t29` PRESERVED (clean, all committed); row untouched with the arc recoverable from the worktree + verdict ledger. **T30**: untouched.

### Cycle 13 (2026-09-25, ~20:58–22:55 EDT) — freshness-skip + QUEUE DRAINED 2/2 (T29 recovery completed, T30 landed); operator spec commit landed mid-cycle

One `chug run --spec LOOP-SPEC.md` session (kimi-k3 orchestrator; glm-5-3-flash impl children; kimi-k3 validators), launched by loopd at 00:58:47Z. Freshness rule fired (T29/T30 todo with ready specs + same-day eval) → straight to the queue. All child plumbing via `delegate` (2 launches + ~15 status polls for T29 arc; 2 launches + ~7 polls for T30; zero hand-rolled bash child-plumbing) — and T29's own subject matter (instant polls burning full-context iterations) was paid at the pre-T29 rate all cycle, a fitting last invoice.

**Landed (2/2 — queue DRAINED):**
- **T29 — delegate status wait_secs long-poll** (FEATURE; impl `549d250` + fix-up `e36b30c`, merge `47dfcaf`, row flip `cd7d43c`, pushed). Recovery executed from cycle-12's preserved worktree exactly as the arc required: gates verified on the impl commit (413+3, clippy), then a glm fix-up child with validator #1's two findings pasted verbatim — done 39/50 in ~6 min, BOTH fixes mutant-verified by the child itself (real `sleep 30` child SIGKILLed mid-wait for the liveness-flip test; one final read at the deadline + a pin whose revert fails). kimi re-validation VERDICT: **PASS** 36/40 — gates re-run 415+3 twice, M1 (liveness_flipped gutted) caught at the 30.17s full deadline, M2 (deadline renders entry snapshot) caught 2.08s, M3 (state_changed gutted) caught 30.16s, worktree restored clean; 2 informational nits (empty-file-creation disjunct + req-5 mid-wait-failure leg lack unique pins — predicted surviving mutants, edge-case-only). 8 artifacts harvested (4 event streams + 4 LEDGERs incl. both verdict ledgers) pre-removal.
- **T30 — META-META-SPEC spec-bar: worktree-relative check: rule** (doctrine; impl `4b55d61` glm 19/50 ~5 min first-try, merge `00ce617`, row flip `a1dde80`, pushed). One-hunk +4/−1 extension of the spec-quality-bar sentence, both bites cited, everything else byte-identical (incl. 9b7e904's new §6). kimi VERDICT: **PASS** 19/40 — citations git-verified (cac4649/e73b66d/1d6780d/3334743), Phase-2-sanction-vs-hard-rules coherence adjudicated (the hard rules bind the EVALUATOR role; META-META-SPEC.md is not in its enumerated human-spec list), check-mutations M1/M2/M3 (drop `worktree-relative` / drop `T21` / full revert) all killed, spec check verbatim green. 4 artifacts harvested pre-removal.

**Mid-cycle operator landing:** `9b7e904` (META-META-SPEC §6 README-usability-audit + LOOP-SPEC README-gate integrate-don't-append clause) committed + pushed by the operator at 22:14 EDT, ~76 min into the cycle. Not a driver conflict (human edit); clean ort merge with T29's LOOP-SPEC §2 clause (disjoint sections); T30's impl was briefed on the changed file and preserved §6 byte-identical. First mid-cycle upstream movement the loop has absorbed — no doctrine change needed, but noted: the fetch-before-push habit caught it cleanly.

**What the validators caught:** a REAL defect pair this time — validator #1's FAIL on T29 (untested liveness leg + stale deadline render) is exactly the class adversarial validation exists for; both fix sites re-mutated and killed by validator #2. T30 clean. glm rounds: T29 impl needed one fix-up (streak resets), T30 first-try clean.

**Flake-family sightings (hand to the next eval — filing is Phase 1's job):** `run_shell_returns_when_setsid_grandchild_holds_pipe` failed once under parallel load here (passes isolated 6.1s both trees; READER_GRACE race, T29-adjacent-free) and validator #2 saw `run_shell_normal_path_unchanged` fail once the same way — second and third sightings of the family after the t23-impl mystery failure (T25's tail-window now names them) and T28's `dead_server_retries` port race. A deflake row is a credible next-eval filing. Also for the next eval: (a) META-META-SPEC's priority line still reads "DX friction > performance > features" — contradicts LOOP-SPEC's amended features-first-class doctrine (T30's spec correctly forbade touching it; adjudication wanted); (b) validator #2's two T29 edge-leg pin nits; (c) cycle-12's deferred-wrap data loss argues wrap bookkeeping (Outcomes) should be written per-item, not held to cycle end — same class as the T10/T12 row-flip ownership fix.

**Freshness rule for the next cycle:** CANNOT skip — the queue is empty, so the next cycle MUST evaluate fresh (designed flow). It inherits: the new §6 README-audit duty, the worktree-relative check: bar (its own specs' checks must follow it), the flake-family + priority-line + deferred-wrap candidates above, and the standing human-decision carries from cycle 11 (loopd restart activated 120 this cycle, child --max-tokens/J7, chug doctor, model routing, sandbox policy, loopd pidfile race, sed note).

**Final state:** main `a1dde80`; gates 415+3 green in main, clippy clean; TODO.md fully truthful (T1–T30 all done with refs); 12 T29/T30 child artifacts in `.chug/`; README gate satisfied (T29's wait_secs documented in the delegate paragraph INTEGRATED by the impl branch itself; T30 doctrine-only, no README surface); everything pushed.
