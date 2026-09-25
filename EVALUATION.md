# EVALUATION — chug, assessed by chug-loop (2026-09-25, cycle 9)

Corpus: `.chug/events-20260925-204432.jsonl` (**cycle-8's full LOOP-SPEC
run**: 41 iterations, 47 tool results, goal accepted, freshness-skip →
queue drained 1/1), `.chug/events-20260925-202932.jsonl` (**cycle-7's full
run**: 76 iterations, 97 tool results, evaluated fresh + worked 2/3 rows),
`.chug/events-t21-{impl,validate}-*.jsonl`,
`.chug/events-t22-{impl,validate}-*.jsonl`,
`.chug/events-t23-{impl,validate}-*.jsonl` (all K2-harvested),
`.chug/loopd/` (supervisor logs — cycles 6→9 all unattended),
`LEDGER-t2{1,2,3}-*.md`, `TODO.md` (T1–T23 done with refs), git log
through `79b4c19`; `src/` (18,611 lines, +908 since cycle 7 — T23's
delegate; driver.rs 2,894 unchanged); `README.md`.
Prior evaluations: cycle 7 (with cycle-5/6/7/8 Outcomes) below at
§Outcomes; cycle 5 at same section; cycle 4 at `c8222cd`; cycle 3 at
`df4039a`; cycle 2 at `0d71d00`.
Verification performed this eval: full `cargo test` ×5 consecutive =
**381+3 green every run** (flaky-test repro hunt, ~60s wall);
`cargo clippy` clean at cycle-8 wrap (`d7147cf` notes); delegate tool
schema read live from `src/tools.rs:146`; eventlog preview seam read at
`src/eventlog.rs:179`; read_file cap read at `src/tools.rs:18,223`;
zero-delegate-call claim jq-verified against both cycle streams;
web-fetch evidence grep (docs.rs/stackoverflow/google/fetch) across both
cycle streams = 0 hits; driver spec-delivery read at
`src/driver.rs:279,545` + `src/main.rs:42`.

## 1. What chug does well

- **Eight consecutive clean glm implementation rounds.** Cycle 8's T22
  impl: green first try, goal accepted at 23/50 — comfortable headroom
  under T21's widened ceiling, on the item whose very sentence warns
  against the mirage that killed T20 at 40/40 two cycles earlier.
  Validators keep finding zero implementation defects (T22: 6/6 mutants
  died; T23: 2/2 died; T21: 4/4 legs).
- **The freshness rule fired as designed** (cycle 8): same-day eval +
  queued row → Phase 1 skipped, straight to work, 41 iterations total —
  the cheapest full cycle yet. And its complement fires now: queue
  drained → this cycle re-evaluated fresh. Both directions of the
  designed flow have been observed.
- **loopd is now boring** (the compliment): cycles 6→9 supervised
  end-to-end, 60s relaunch cadence, single-driver guard holding, no HALT
  ever needed. `loopd.log` reads like a cron that happens to run a
  self-improving agent.
- **T13/T17/T18's budget telemetry chain paid for itself again**: the
  T23 impl child's `budget_low` event (remaining 8, 20:17:49) preceded
  the first-ever complete wrap in the ceiling zone (goal accepted 50/50)
  — the J6 arc from pre-commit death → commit-no-wrap → full wrap is
  closed and documented in cycle-7 Outcomes.

## 2. Incidents worth fixing

- **N1 → T25 — a flaky test fired once and its identity was destroyed
  by the event stream's own preview cap.** The T23 impl child's suite
  run at `2026-09-25T20:17:49` (`events-t23-impl-20260925-202220.jsonl`)
  reports `FAILED. 379 passed; 1 failed` — but the recorded preview is
  the flat 200-char head (`src/eventlog.rs:179`,
  `preview.chars().take(200)`), which captured `Finished … test result:
  FAILED` noise and **not the failing test's name** (cargo prints the
  `failures:` list at the *end* of output, just past the window). The
  child burned iterations 43–47 re-running the suite 3× (all green) to
  rule out its own diff, then wrapped; the worktree transcript — the
  only other place the name existed — was not harvested (operator's
  choice per K2) and is gone. This eval re-ran the full suite 5×
  consecutively in main: 381+3 green every time, no repro. Root cause
  is not the flake (unidentifiable now) but the observability hole:
  **error previews get the same 200c budget as success previews, even
  though failure bytes cluster at the tail.** Fix filed: **T25**
  (`specs/t25-failure-aware-previews.md`) — ok-leg stays a byte-identical
  200c head; error-leg gains a tail-anchored window (≤2000c). The next
  flake will name itself in the events stream alone.
- **N2 — assessed, watch item: the flaky test itself.** One occurrence,
  no identity, no repro in 5 tries. No row is fileable without a name —
  which is precisely T25's point. If it recurs post-T25, the events
  stream will carry the name and a bug row (pri 1) follows.
- **N3 — assessed, watch item: GNU-vs-BSD userland, second species.**
  The cycle-8 orchestrator hit `sed: 1: ",+60p\n": invalid command code
  ,` (GNU sed range syntax on BSD sed;
  `events-20260925-204432.jsonl`). Same class as T22's `timeout` mirage,
  different species, ONE occurrence (T22 was filed on two). Below the
  filing bar T22 set; if it recurs, the fix generalizes the bash
  description's platform note to name BSD-userland flag drift. Carried.
- **N4 — assessed, no row: validator `read_file` on the spec path
  escapes the worktree.** The T23 validator tried `read_file
  /Users/jadams/workspace/chug/specs/t23-delegate-tool.md` from its
  worktree → `path escapes cwd`, recovered immediately. The driver
  delivers the spec directly (`src/driver.rs:279`, re-read every
  iteration, `src/main.rs:42`), so this is cosmetic; same carried class
  as the edit_file confinement note from cycle 7.

## 3. Friction hot spots

- **PATH tax / revert-thrash / edit-thrash / stub hangs / stale-ledger
  / watch-and-wait / `timeout` mirage — fixed, holding (7th eval
  running).** Zero recurrences; the T22-note corpus itself stayed clean
  (no model reached for `timeout` in any post-T22 stream).
- **Orchestration plumbing is STILL the orchestrator's dominant cost —
  and the fix now exists but is unused.** Cycle 8: 37 of 47 tool calls
  were bash, the plurality child plumbing by hand (`git worktree add`,
  the 6-line nohup incantation, `ps -p` polls, log tails, jq on the
  child's events.jsonl). The `delegate` tool (T23, landed `1012dca`)
  internalizes exactly launch+status — and recorded **zero calls** in
  the cycle-7 and cycle-8 loop streams (jq-verified): LOOP-SPEC §2
  still teaches the hand-rolled template, so the loop's newest
  capability is invisible to its own core mechanic. This is the
  evidence behind T24 (§4).
- **2000-line read cap: now four files over it.** tools.rs crossed the
  cap this cycle (2,048, +879 from T23) joining driver.rs (2,894),
  mcp_http.rs (2,343), tui.rs (2,163). The cap note names the window
  but offers no paging primitive, so children fall back to `sed -n
  X,Yp` chunking (T20 impl burned iterations 1–5 on this, cycle-7 eval).
  Fix filed: **T26** (`specs/t26-read-file-pagination.md`), pri 3.
- **Spec-authoring lessons (cycle-5 L4, cycle-7 T21-anomaly): holding.**
  No derivation slips in cycle-8's spec; the T22 check line kept the
  vacuous-but-harmless behavior-check convention and the impl child
  goal-accepted without incident. T24's spec (this eval) applies the
  content-check lesson: its `check:` greps the *worktree-relative*
  LOOP-SPEC.md, never `cd`s to main.

## 4. Capability gaps — FEATURE SCAN (required)

Judged against the human specs' direction (meta loops, adversarial
validation, observability, fleet-driving) and harness-class norms:

- **N5 → T24 — the loop's orchestration doctrine has not adopted the
  loop's orchestration tool.** `delegate` exists in every session's
  schema since `1012dca`; LOOP-SPEC §2's launch template is still the
  6-line nohup incantation and its polling guidance is still
  `ps`+`tail`+jq by hand (§3's 37/47 evidence). A doctrine edit —
  impl-child launch via `delegate{action:launch}` with explicit
  `--max-iters 50` (delegate defaults are 40/35, T21 needs 50), polling
  via `delegate{action:status}`, validator launch likewise as a
  LOOP-SPEC override of META-SPEC §6's mechanics (META-SPEC.md itself
  untouched — human-owned doctrine) — converts T23 from shipped code
  into practiced capability and deletes the orchestrator's largest
  remaining token sink. Filed: **T24**
  (`specs/t24-loop-adopts-delegate.md`), pri 2 per the features-first
  doctrine. Validation REQUIRED (loop doctrine, §2.4).
- **web_fetch — assessed, not filed (3rd consecutive eval).** Zero
  external-info signals in the cycle-7/8 streams (grep:
  docs.rs/stackoverflow/google/fetch = 0 hits); every cycle was fully
  served by the local repo + harvested streams. The filing bar remains
  "a cycle stalls on external information"; it has not happened.
- **Parallel tool calls — assessed, not filed (carried).** Cycle 8 ran
  47 tool results over 41 iterations (≈1.15/call) — no batching
  pressure; shell `&&` covers what exists. Thin evidence, real
  complexity. Noted.
- **MCP consumption depth — no gap observed (carried).** spec-7/9
  capability remains ahead of its use case; nothing in eight cycles
  needed it.
- **Session/handoff UX — working, no gap.** This cycle is again the
  proof: cold start from cycle-8's wrap, zero human words, every fact
  on disk.
- **Carried human-decision items (unchanged):** child-launch
  `--max-tokens` (J7 — data less anomalous this cycle: glm 116k/50 and
  31k/23, kimi 112k/25 and 30k/21, but trust not established), `chug
  doctor`, model routing/escalation (zero glm fallbacks through 8 clean
  rounds), bash sandbox policy + stray I7 cargo symlink, loopd pidfile
  race (M4, unexercised), GNU/BSD sed note (N3, one occurrence).

## 5. Top 3 priorities

1. **T25 — failure-aware event previews (robustness, pri 2).** The
   loop's postmortem surface provably lost a flaky test's identity this
   cycle; the fix is one seam (`src/eventlog.rs:179`) with the ok-leg
   byte-identical. Every future failure — flaky test, child compile
   error, validator mutation — names itself. Worked first per doctrine
   order (robustness > features).
2. **T24 — LOOP-SPEC adopts `delegate` (feature, pri 2).** The largest
   live capability gap is adoption-shaped: the tool exists, is tested,
   is documented — and the doctrine that should use it predates it.
   Converts the orchestrator's dominant token sink (hand-rolled
   plumbing) into the tested primitive T23 built.
3. **T26 — read_file offset/limit pagination (DX friction, pri 3).**
   Four files now exceed the read cap; every child that must read them
   re-derives sed paging. One schema addition, default path
   byte-identical.

## 6. Handoff — recommended execution order

**LOOP-SPEC Phase 2 (this cycle):** T25 → T24 → T26, one at a time, each
with kimi adversarial validation (T25 touches `src/eventlog.rs` —
explicitly named in §2.4; T24 is loop doctrine; T26 touches
`src/tools.rs`). Budget realism: cycle 7 worked 2 rows in ~2h/76 iters;
this cycle's Phase 1 cost ~16 iterations, so T26 is the drop candidate
per §2.6 (its spec is ready; a cold next cycle picks it up with zero
human words). If T24 lands, dispatch later children **via the new
delegate-based template** — an in-cycle dogfood to cite in the wrap.
If any child dies mid-work: harvest, validate what landed, merge only
if green+complete, else leave the row `todo` with findings.

**Human-decision items (no rows filed):** 1. Child-launch `--max-tokens`
(carried; J7). 2. `chug doctor` (carried). 3. Model routing/escalation
(carried). 4. Bash sandbox policy; stray I7 cargo symlink (carried).
5. loopd pidfile write/check race (M4, unexercised). 6. GNU/BSD `sed`
flag drift (N3 — one occurrence; second occurrence generalizes the bash
description's platform note). 7. J7: proxy-side usage accounting per
model family.

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
