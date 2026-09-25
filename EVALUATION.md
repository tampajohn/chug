# EVALUATION — chug, assessed by chug-loop (2026-09-25, cycle 5)

Corpus: `.chug/events.jsonl` (**this cycle-5 LOOP-SPEC run, live** —
~86k in / 27k out cumulative at mining time; an interactive `chug chat`
(sonnet, operator) opened at 17:44Z and interleaves into the same
stream — noted per the hard rules, non-blocking),
`.chug/events-20260925-172902.jsonl` (**cycle-4's full LOOP-SPEC run**:
80 iterations, 95 tool results, goal accepted, 260,967 in / 56,479 out,
16:57→17:25Z — the cycle that landed T17 and filed T18),
`.chug/events-t17-{impl,validate,validate2,validate3}-*.jsonl` (cycle-4's
K2-harvested child streams, mined by the cycle-4 eval),
`/tmp/chug-loop-t18.log` + `/tmp/chug-loop-t18-validate.log` (**cycle-5's
two child console logs** — the only surviving record: both children's
`.chug/` streams were lost at worktree removal, see L1),
`TODO.md` (T1–T18 done with refs, T19–T20 filed this eval); git log
through `019cfa8`; `src/` (17,467 lines; driver.rs 2,886, +123 since
cycle 4); `README.md`.
Prior evaluations: cycle 4 at `c8222cd` (Outcomes at `31ae7f1`), cycle 3
at `df4039a`, cycle 2 at `0d71d00`, cycle 1 at `e962f1f`.
Verification performed: full `cargo test` = **356 green (353+3)**; `cargo
clippy --all-targets -- -D warnings` clean; line-by-line review of the
T18 diff; independent re-derivation of its fire-time arithmetic against
`src/driver.rs` (`iteration` init `:401`, ceiling abort `:410`,
`remaining = max_iters − iteration` at `:505` **before** the increment at
`:766`, latch mirror `:535`); `budget_low_notice` (`:800+`) and the
untouched interpolation pins (`:1923`/:1928/:2143`) re-read; the
validator's two mutation kills re-checked against the test names.

## 1. What chug does well

- **The loop is now cheap.** Cycle 5 worked T18 end-to-end in ~25 wall
  minutes with two children that both finished **first try**: glm impl
  26/40 iters, goal accepted, self-committed (`9056c78`), non-vacuousness
  self-demonstrated; kimi validator 22/40 iters, VERDICT PASS with 2/2
  mutants killed. Contrast cycle 4: 3 of 4 children died at the ceiling,
  validation took three children. The deltas were (a) the spec named
  every pin to flip, (b) the validation goal carried ground-truth
  pointers (`git diff main...loop-t18`, the merge sha) plus a hard
  schedule — the exact lessons cycle 4's Outcomes wrote down. Doctrine
  learned from failure and the failure stopped.
- **The adversarial net caught a spec bug, not just code bugs.** T18's
  spec asserted fire-time "remaining == 7 / 7 iteration(s)"; the true
  value is 8 (0-based `iteration`, check pre-increment). Three
  independent readings — implementer, validator, orchestrator — derived
  the same correction, and the OLD pin ("4th call, 5 remaining" under
  WARN=5) is consistent only with that model. The loop's quality bar now
  extends to the specs themselves.
- **Every shipped fix keeps holding (5th eval running).** Zero PATH
  preambles, zero edit-thrash, zero stub hangs, zero stale-ledger
  inherits across the corpus; T18's own pins prove WARN=8 is live; T17's
  `run_start` ceilings are on disk in this run's stream (verified by
  jq — no `budget_low` event yet: the orchestrator never neared its
  ceiling this cycle, correctly silent).
- **Events-first mining is the default operating mode.** Both cycle
  streams were characterized with jq in seconds (type histograms,
  cumulative tokens, goal/abort presence); nothing required transcript
  archaeology this cycle.

## 2. Incidents worth fixing

- **L1 → T19 — K2 harvest skipped at cleanup; both T18 child streams
  lost (the only process failure this cycle).** After merging T18 the
  orchestrator ran `git worktree remove /tmp/chug-loop-t18` **without**
  first copying the children's `.chug/events.jsonl` (+ transcripts, +
  impl LEDGER.md) into the main repo — one cycle after cycle 4 harvested
  4/4 correctly by hand. `git worktree remove` deletes untracked files
  silently, and `.chug/` is untracked in the worktree. Evidence: this
  eval's corpus names console logs where cycle 4 named event streams.
  Root cause: the convention lives in ledgers and memory, not in
  LOOP-SPEC. Fix filed: **T19** (`specs/t19-events-harvest-step.md`) —
  codify harvest-before-removal in LOOP-SPEC §2 step 5, which cycle 4
  had already flagged as a human-decision item; it now has a failure
  case attached.
- **L2 → T20 — wrong-HEAD confusion, second occurrence of the class.**
  Cycle 4: T17's kimi validator 2 was killed early on a wrong-HEAD
  belief (cycle-4 Outcomes). Cycle 5: the T18 validator opened with "I'm
  on `main` but the commit exists" (`/tmp/chug-loop-t18-validate.log`,
  iteration 3) and self-corrected only because the goal spelled out the
  diff ground truth. Root cause: the T11 banner prints the **binary's
  build-time** commit, but children run the main-tree binary with a
  worktree cwd — the one identity line a child sees names a commit that
  is not its checkout's HEAD and no branch. Fix filed: **T20**
  (`specs/t20-banner-worktree-head.md`) — runtime, best-effort
  `head=<branch>@<short>` in the banner + `head_branch`/`head_commit` in
  `run_start`; never fails the run, git-less fallback bytes unchanged.
- **L3 — assessed, watch item J7 (usage-telemetry trust per model
  family), no row.** The T18 kimi validator's console printed **17,384
  cumulative input tokens for 22 iterations** (spec + diff + two cargo
  gates + two mutation runs — each API call alone re-sends the growing
  conversation); cycle 4's kimi validator 3 printed 35.5k for a full
  review while glm children print 132–246k for comparable work. Either
  the proxy under-reports kimi usage or kimi's per-call accounting
  differs. Two sessions, same direction → watch item; no row because no
  decision is blocked (child budgets are iteration/minute-denominated;
  `--max-tokens` users should know the number may undercount on kimi).
- **L4 — assessed, no row: the T18 spec's off-by-one (see §1).** A
  spec-authoring lesson, absorbed: computed expectations must carry
  their derivation (`remaining = max_iters − iteration`, 0-based,
  pre-increment), not just the number. No harness change; the
  three-reader catch worked.

## 3. Friction hot spots

- **PATH tax / revert-thrash / edit-thrash / stub hangs — fixed, holding
  (5th eval):** zero recurrences in this corpus.
- **Watch-and-wait burn — stayed closed:** no invariant trip, no
  runaway; single-driver check was two `ps` greps again.
- **Child wrap deaths (J6) — the fix shipped this cycle, effect
  unmeasured:** T18 landed WARN=8, but neither cycle-5 child came near
  the ceiling (26/40, 22/40), so the widened margin's value is
  unverified in anger. Next multi-child cycle is the test; T17's
  telemetry now records warned-at counts per child.
- **2000-line read cap vs growing driver.rs (2,886 lines):** the impl
  child spent iterations 1–5 chunking driver.rs via sed — no failure,
  ~3 turns of tax. Not filing: the cap's paging behavior worked as
  designed; noted because the file grows ~100 lines/cycle and the tax
  scales with it.
- **Spec-authoring precision:** see L4 — the only friction the
  implementer hit was correcting the spec's arithmetic (cost: a few
  turns of empirical re-derivation).

## 4. Capability gaps (against the human specs' direction)

- **K2 codification — filed (T19)** with the failure evidence attached.
- **Worktree identity at startup — filed (T20).**
- **Child launch budgets (human decision, carried, reinforced):**
  LOOP-SPEC's child template still lacks `--max-tokens`; the T18 glm
  impl burned 179,400 cumulative input on a one-constant change
  (edit-heavy style + big-file re-reads). Iteration economics are fine
  (26/40); token economics argue for the ceiling.
- **`chug doctor` single-driver tooling (carried).** Two `ps` greps
  again; fine, still hand-rolled.
- **Model routing/escalation (carried):** zero glm fallbacks needed
  across cycles 2–5; manual remains sufficient.
- **Sandbox policy / stray I7 cargo symlink (carried, unchanged).**

## 5. Top 3 priorities

1. **T19 — codify the events harvest (robustness of the loop's own
   evidence trail, pri 3).** Fresh failure case; one-paragraph doctrine
   edit; the whole evaluation pipeline depends on these streams.
2. **T20 — banner/run_start worktree HEAD (DX, pri 3).** Twice-observed
   confusion class with a cheap, well-seamed fix; pays off every child
   launch from now on.
3. **(No third row filed.)** Queue honesty: J7 is a watch item, J6 is
   fix-shipped-awaiting-evidence, everything else is verified-fixed or
   a human-decision carry.

## 6. Handoff — recommended execution order

**LOOP-SPEC Phase 2 (next cycle):**
1. **T19** — doctrine edit to LOOP-SPEC.md → adversarial validation
   **REQUIRED** (loop/spec doctrine itself, §2.4). No code; the
   validator checks internal consistency + harvest-before-removal
   ordering.
2. **T20** — touches `src/build_info.rs`, `src/main.rs`, `src/eventlog.rs`
   (the events surface) → adversarial validation **REQUIRED**; mutation
   legs named in the spec (Some/None render, never-fail invariant).

**Human-decision items (no rows filed):**
1. LOOP-SPEC §2 child template: add `--max-tokens` (reinforced: 179k
   input for a one-constant glm impl).
2. `chug doctor` (carried). 3. Model routing/escalation (carried).
4. Bash sandbox policy; stray I7 cargo symlink (carried).
5. J7: whether kimi usage under-reporting matters for `--max-tokens`
   semantics on kimi runs (needs proxy-side evidence).

## Outcomes (filled at cycle wrap — LOOP-SPEC Phase 3)

Cycle 5 executed 2026-09-25 ~13:29–14:05 EDT, one `chug run --spec
LOOP-SPEC.md` session (kimi-k3 orchestrator; glm-5-3-flash impl child;
kimi-k3 validator).

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
