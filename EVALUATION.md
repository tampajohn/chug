# EVALUATION — chug, assessed by chug-loop (2026-09-25, cycle 7)

Corpus: `.chug/events.jsonl` (**this cycle-7 LOOP-SPEC run, live** — an
interactive `chug chat` (sonnet, operator) interleaves from 18:42Z —
noted per the hard rules, non-blocking),
`.chug/events-20260925-183937.jsonl` (**cycle-6's full LOOP-SPEC run**: 67
iterations, 74 tool results, goal accepted, queue drained 2/2),
`.chug/events-t19-{impl,validate}-*.jsonl` +
`.chug/events-t20-{impl,validate}-*.jsonl` (cycle-6's child streams,
K2-harvested per the just-landed T19 doctrine — 7 artifacts),
`.chug/loopd/` (supervisor logs — **loopd's first full evaluation**; it
landed in `548d494` after cycle 5's eval and cycle 6 skipped Phase 1),
`LEDGER-t19-impl-*.md`, `LEDGER-t20-validate-*.md`, `TODO.md` (T1–T20 done
with refs), git log through `7ee6bcb`; `src/` (17,703 lines; driver.rs
2,894, +8 since cycle 5); `README.md`.
Prior evaluations: cycle 5 (with cycle-5/6 Outcomes) below at §Outcomes;
cycle 4 at `c8222cd`; cycle 3 at `df4039a`; cycle 2 at `0d71d00`.
Verification performed this eval: full `cargo test` = **365 green
(362+3)**, `cargo clippy --all-targets -- -D warnings` clean; T19 doctrine
verified practiced (7 harvested cycle-6 artifacts on disk); T20 dogfooded
twice (banner prints `head=main@7ee6bcb`; this run's own `run_start`
carries `head_branch`/`head_commit`); T20 impl wrap-death timeline
reconstructed event-by-event; `timeout` mirage reproduced live by this
orchestrator; bash tool description confirmed unpinned; `--cwd` flag and
risk-gate scope (bash-only, default-off) read in `src/main.rs:50` /
`src/driver.rs:612`.

## 1. What chug does well

- **The doctrine flywheel is visibly compounding.** Cycle 6 practiced
  T19's harvest-before-removal in the same cycle that landed it (7
  artifacts), dogfooded T20's banner field from the next child launch on,
  and both children + both validators finished with goal accepted (T19
  impl ~20/40, T19 validate 10/40, T20 validate 29/40). Four of the last
  five rounds were first-try clean.
- **loopd works unattended.** Two cycles supervised with zero human
  words: build → launch → detect goal complete → relaunch after 60s.
  Cycle 6 ran 35 wall minutes end-to-end (`loopd.log`, cycle
  `180335`→`183936`). The single-driver guard (`pgrep -f "chug run
  --spec LOOP-SPEC.md"`) and the 3-failure HALT are in place and legible.
- **The wrap-as-handoff contract held.** This cycle started cold from
  TODO.md + EVALUATION.md + specs/ with no human input: the queue was
  empty, the freshness rule correctly could not fire, and every fact
  needed to re-evaluate was on disk.
- **Events-first mining is total.** Every timeline in this eval (the
  T20 wrap-death second-by-second, the 62/74 bash histogram, the J7
  token anomalies) came from jq over events.jsonl in seconds. No
  transcript archaeology anywhere.

## 2. Incidents worth fixing

- **M1 → T21 — child iteration ceiling still kills wraps (3 of the last
  5 glm impl children died at 40/40).** T18 (26/40) and T19 (~20/40)
  were one-constant / docs-only edits; every multi-file feature child
  (T15, T17, T20) hit the ceiling. The T20 timeline is the cleanest:
  `budget_low` at iteration 32 (remaining 8, 18:20:36), commit `f08ea04`
  lands ~iteration 37, then **two recovery iterations lost to the
  `timeout` mirage (M2)** and abort at 40/40 (18:21:09) — post-commit,
  pre-wrap (goal_complete never fired; ledger left a seed stub).
  `WARN_REMAINING_ITERS=8` (T18) bought the commit but not the wrap; the
  directive warning text ("Stop starting new work: commit what is done…")
  already exists and partially worked. Minutes were never binding (T20
  used 6 of 35). Fix filed: **T21** (`specs/t21-child-iteration-headroom.md`)
  — LOOP-SPEC §2 impl-child template `--max-iters 40 → 50` (doctrine
  edit, validation required per §2.4). Deaths needed +4–6; 50 covers the
  wrap plus one hiccup; no budget-inflation evidence exists (T18/T19
  finished in 20–26 with 40 available).
- **M2 → T22 — the macOS `timeout` mirage: two independent exit-127
  occurrences in one day, two model families.** (a) The T20 glm impl
  child, at iteration ~39 of 40, invented `timeout 5 …` for a banner
  dogfood (its spec never mentioned `timeout` — verified,
  `specs/t20-banner-worktree-head.md` has zero occurrences) and got
  `sh: timeout: command not found` — two iterations of recovery at the
  worst possible moment, contributing to M1's death. (b) **This
  orchestrator (kimi), this very eval**, reached for the identical
  `timeout 5` dogfood and got the identical exit 127. META-SPEC:126
  names the `perl -e 'alarm N; exec @ARGV'` idiom, but impl children
  never read META-SPEC (they get only their t-spec + goal), and nothing
  in the runtime surface tells any model the platform lacks GNU
  coreutils. Fix filed: **T22** (`specs/t22-bash-timeout-note.md`) —
  the bash tool's description (the one surface every session sees
  regardless of spec) gains the platform note + idiom; pin the string.
  Cheap, well-seamed, touches `src/tools.rs` → validation required.
- **M3 — assessed, watch item J7 stays (usage telemetry trust per model
  family), no row.** Now anomalous in both directions with three data
  points: kimi validator 17,384 in / 22 iters (cycle 5), glm impl
  179,400 in / 26 iters (cycle 5), glm impl 27,280 in / **40** iters
  (cycle 6 — fewer cumulative input tokens than a 22-iter kimi session
  with a fraction of the work). The proxy's per-family accounting is not
  trustworthy; child budgets are iteration/minute-denominated so no
  decision is blocked. `--max-tokens` users on kimi remain the exposed
  case. Needs proxy-side evidence; carried.
- **M4 — assessed, no row: loopd double-start.** `loopd.log` shows two
  `loopd start` lines 37s apart (pids 76155 → 77453); the first died
  silently (launchd `*.err.log`/`*.out.log` exist, empty — likely an
  operator relaunch under launchd). The pidfile guard held (one
  survivor, no overlapping cycles). No failure observed; the pidfile
  write/check race is real but unexercised. Noted for the operator.

## 3. Friction hot spots

- **PATH tax / revert-thrash / edit-thrash / stub hangs / stale-ledger
  / watch-and-wait — fixed, holding (6th eval running).** Zero
  recurrences in this corpus.
- **Orchestration plumbing is the orchestrator's dominant cost.** Cycle
  6: 62 of 74 tool calls were bash, and the great majority of those were
  child plumbing by hand — `git worktree add`, the nohup launch
  incantation, `ps -p` polls, `tail` log reads, jq on child
  events.jsonl, harvest `cp`s. It works (doctrine is precise) but it is
  re-derived keystroke-by-keystroke every cycle and burns orchestrator
  iterations on mechanics instead of judgment. This is the evidence
  behind T23's feature row (§4).
- **`edit_file` cwd confinement vs worktrees:** 1 occurrence (cycle-6
  orchestrator tried `edit_file` on `/tmp/chug-loop-t19/LOOP-SPEC.md`
  for the validator's nit fix → `path escapes cwd`, fell back to a bash
  heredoc, ~1 turn lost). The confinement is deliberate (SPEC-3/9 risk
  doctrine); the bash fallback works. Not filing — carried friction.
- **2000-line read cap vs driver.rs (2,894 lines, +8 since cycle 5):**
  carried from cycle 5 (noted, not filed); the T20 impl spent iterations
  1–5 chunking driver.rs via sed. Paging works as designed; the tax
  grows ~100 lines/cycle.
- **Spec-authoring precision (cycle-5 L4): absorbed, holding** — no
  arithmetic/derivation slips in cycle-6 specs; the T20 validator's only
  catch was a docs nit (one precedent unnamed).

## 4. Capability gaps — FEATURE SCAN (required)

Judged against the human specs' direction (meta loops, adversarial
validation, observability, fleet-driving) and harness-class norms:

- **K3 → T23 — no `delegate` tool: the loop's orchestration exists only
  as orchestrator hand-rolled bash.** LOOP-SPEC §2's launch template is
  a 6-line nohup incantation; polling is `ps`+`tail`+jq by hand; child
  state inspection is jq over the child's events.jsonl by hand (see §3's
  62/74 evidence). A bounded `delegate` tool (launch a detached `chug
  run` child against a spec+goal+budgets, return pid/log/events handle;
  a status action that summarizes the child's events.jsonl — alive, last
  iteration, budget_low/goal/abort seen) turns the loop's core mechanic
  into a first-class, tested capability. LOOP-SPEC's own mandate names
  this gap verbatim ("a missing `delegate`/`web_fetch` tool"). Scope
  deliberately excludes worktree creation (repo-specific; stays with the
  caller's bash). Filed: **T23** (`specs/t23-delegate-tool.md`), pri 2
  per the features-first doctrine — the capability gap outranks DX
  friction.
- **web_fetch — assessed, not filed.** Named alongside `delegate` in
  LOOP-SPEC's mandate, but six cycles of corpus contain zero moments
  where web access would have changed an outcome; every evaluation and
  implementation was fully served by the local repo + harvested streams.
  Filing without evidence would be doctrine-quoting, not evaluation.
  Revisit when a cycle actually stalls on external information.
- **Parallel tool calls — assessed, not filed.** The driver runs ~1 tool
  call per iteration (67/74 in cycle 6); shell `&&` composition covers
  the batching cases the loop actually hits (poll = ps+tail+jq in one
  bash). Thin evidence, moderate value, real complexity (partial
  failures, streaming interleave). Noted.
- **MCP consumption depth — assessed, no gap observed.** spec-7/9 landed
  stdio+http consumption, T16 pinned the seams; nothing in six cycles of
  loop work needed an MCP tool. The capability exists ahead of its use
  case — fine.
- **Session/handoff UX — working, no gap.** This cycle is the proof:
  cold start from files, zero human words.
- **Carried human-decision items (unchanged):** child-launch
  `--max-tokens` (J7 says the kimi number under-reports — do not trust
  it yet), `chug doctor` single-driver tooling (two `ps` greps, still
  fine), model routing/escalation (zero glm fallbacks needed, cycles
  2–6), bash sandbox policy + stray I7 cargo symlink.

## 5. Top 3 priorities

1. **T21 — impl-child iteration headroom (robustness, pri 2).** 3-of-5
   death rate on the loop's critical path with a one-number doctrine fix;
   every future multi-file item benefits, including T23 itself.
2. **T23 — `delegate` tool (feature, pri 2).** The largest capability
   gap for the loop's own operation; converts the orchestrator's
   dominant token sink (hand-rolled plumbing) into a tested primitive.
   Worked after T21 per doctrine order — and deliberately so: its impl
   child gets the 50-iteration budget it will need.
3. **T22 — bash tool macOS `timeout` note (DX friction, pri 3).** Two
   exit-127s in one day across two model families; one-line fix with a
   pin; prevents recurrence of the exact recovery-loop that helped kill
   the T20 child.

## 6. Handoff — recommended execution order

**LOOP-SPEC Phase 2 (this cycle):** T21 → T23 → T22, one at a time, each
with kimi adversarial validation (T21 = loop doctrine per §2.4; T22/T23
touch `src/tools.rs`). Budget realism: polling children costs
orchestrator iterations (~1/min); cycle 6 did two items in 67 iterations
— three items is the stretch goal, T22 is the drop candidate. If T23's
child dies mid-work: harvest, validate what landed, merge only if
green+complete, else leave the row `todo` with findings (a fine outcome
per §2.6).

**Human-decision items (no rows filed):**
1. Child-launch `--max-tokens` (carried; J7 blocks trusting the number).
2. `chug doctor` (carried). 3. Model routing/escalation (carried).
4. Bash sandbox policy; stray I7 cargo symlink (carried).
5. loopd pidfile write/check race (M4) — operator awareness only.
6. J7: proxy-side usage accounting evidence per model family.

## Outcomes (filled at cycle wrap — LOOP-SPEC Phase 3)

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
(banner/run_start worktree HEAD), each with a full spec.

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
