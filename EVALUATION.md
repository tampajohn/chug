# EVALUATION — chug, assessed by chug-loop (2026-09-25, cycle 4)

Corpus: `.chug/events-20260925-165721.jsonl` (**cycle-3's full LOOP-SPEC
run**: 69 iterations, 90 tool results, 1 goal accepted, 328,009 in /
39,348 out tokens, 06:04:58→06:40:04Z — the first cycle to work a queue
end-to-end with backgrounded+polled children),
`.chug/events-t15-impl-20260922-061957.jsonl` (T15 glm child: 40/40
iterations, abort, 246,614 cumulative in),
`.chug/events-t15-validate-20260922-063206.jsonl` (T15 kimi validator: 38
iters, VERDICT PASS),
`.chug/events-t16-impl-20260922-063855.jsonl` (T16 glm child: 32 iters,
goal accepted, 132,128 cumulative in) — the three K2-harvested child
streams, mined with jq per the events-first upgrade;
`.chug/transcript-20260925-165721.jsonl` + `.chug/LEDGER-20260925-165721.md`
(cycle-3 orchestrator); `TODO.md` (T1–T16 all done with refs); git log
through `74aa7bc`; `src/` (17,195 lines; driver.rs 2,763); `README.md`.
Prior evaluations: cycle 3 at `df4039a` (Outcomes at `74aa7bc`), cycle 2
at `0d71d00`, cycle 1 at `e962f1f`.
Verification performed: full `cargo test` = **350 green (347+3)**; `cargo
clippy --all-targets -- -D warnings` clean; code inspection of the
budget-low injection site (`src/driver.rs:495-516`), `budget_low_notice`
(`:790-827`), the whole of `src/eventlog.rs` (362 lines), both `run_start`
call sites (`src/driver.rs:297`, `src/chat.rs:175`), the `Event` enum and
all `EventSink` impls (`src/events.rs`, `src/eventlog.rs`, `src/tui.rs`).

## 1. What chug does well

- **The full LOOP-SPEC cycle works, end to end, in one command.** Cycle
  3's resumed run evaluated, filed T15+T16, worked both with
  backgrounded+polled glm children, adversarially validated T15 (kimi,
  PASS, core mutants died), merged, flipped rows with refs, pushed after
  each item, and wrapped with Outcomes — 35 wall minutes, 69/80
  iterations, one `goal` verdict. Against the references: cycle 2 (healthy
  fan-out) took 75 iterations; the cycle-3 halt burned 712k tokens on a
  no-op. The amended doctrine (`988741d` fast-exit, polled children,
  auto-push) did exactly what it was written to do.
- **The backgrounded+polled child pattern is now measured, not just
  asserted.** Of the orchestrator's 74 bash calls, ~30 were sleep-paced
  polls at 105–110s each — all safely under the hard 120s cap, each ~2–3k
  input tokens, zero CONFLICT trips, zero process-group kills.
- **The K2 harvest convention paid off immediately.** All three child
  event streams survived worktree removal, so child costs are knowable
  for the first time (T15 246k / T16 132k / validator 56k cumulative
  input) — and J5 is settled with them (below).
- **Every shipped fix keeps holding.** Across 4 sessions and 270 tool
  calls this corpus: zero PATH preambles (T4), zero non-unique-edit
  thrash (T5), zero stub-test hangs (T6), zero stale-ledger inherits
  (T3/T7), and the T12/T13/T14/T15 budget surface all fired or stayed
  correctly silent. The 4 tool errors observed are one-turn,
  self-corrected, and each of a different non-recurring class (§2 L3).
- **J5 — RESOLVED with data.** Per-call context (delta of cumulative
  `input_tokens`, jq-computable since T10): T15 glm child peak **93,568**
  real tokens, T16 70,199, validator 22,287, orchestrator 76,901, cycle-2
  orchestrator 76,532. Five glm child sessions across cycles 2–3, peak
  93.5k, **zero API errors** against the 120k-estimate trim
  (`src/driver.rs:32-33`). The concern is dead unless evidence revives it.

## 2. Incidents worth fixing

- **L1 → T17 — budget-low warning injections are invisible in
  events.jsonl (the only new row this cycle).** The T15 glm child aborted
  at 40/40 iterations with the implementation complete but **uncommitted**
  (abort event 2026-09-22T06:17:07Z, `budget_kind: iterations`); the
  orchestrator harvested the diff and committed it. Whether the child's
  T13 warning ever fired — and at what remaining counts — is unknowable:
  the K2 harvest preserves events, not transcripts, and the injection
  site (`src/driver.rs:501-516`) emits no event. Consequence: T13's
  one-shot latches and T15's token leg cannot be verified per-session for
  exactly the sessions (children) whose transcripts don't survive, and J6
  (below) can't be decided with data. Adjacent hole in the same theme:
  `run_start` (`src/eventlog.rs:47-66`) records no budgets, so for
  *successful* runs (no abort event to carry `budget_max`) even the
  configured ceilings are absent from the jq record. Root cause: T10's
  event set predates T13/T15; telemetry wasn't extended when the budget
  legs landed. Fix filed: **T17** (`specs/t17-budget-events.md`).
- **L2 — assessed, watch item J6 (child wrap margin), no row yet.** The
  T15 child is the second J1 (code complete, died pre-commit) but the
  first *post-T13*: the warning text already says "commit what is done,
  run the gates, and finish bookkeeping now" (`src/driver.rs:818-822`),
  so wording isn't the gap — margin is. A 40-iteration child doing
  implement+test+commit gets ≤5 iterations of wrap runway, and glm's
  edit-heavy style (35 `edit_file` calls in 40 iterations) burns deep.
  The harvest protocol lost nothing (one test-fixture slip, fixed by the
  orchestrator). One post-T13 occurrence → no harness change; if a second
  occurs, candidates are WARN_REMAINING_ITERS 5→8 or a stronger ≤2-
  remaining second-stage notice. T17's telemetry makes this decidable
  (warned-at vs abort iteration per child).
- **L3 — assessed, no row: three one-turn bumps, each self-corrected.**
  (a) Orchestrator `edit_file` on `/tmp/chug-loop-t15/src/driver.rs`
  rejected (path escapes cwd — by design; bash is the documented escape
  hatch, used immediately). (b) The kimi validator tried `git worktree
  add -b loop-t15` from inside that very worktree (branch in use; adapted)
  — the durable fix is validation-goal template wording in META-SPEC §6,
  a human spec. (c) The T15 glm child emitted one malformed
  `edit_file` call (`missing or non-string field: old`; adapted). No
  class repeats; each cost one turn.

## 3. Friction hot spots

- **PATH tax / revert-thrash / edit-thrash / stub hangs — fixed, holding
  (4th eval running):** zero recurrences in 270 tool calls across four
  sessions and two model families.
- **Watch-and-wait burn — closed twice over:** the fast-BLOCKED doctrine
  removes the reason (`988741d`), T15's `--max-tokens` removes the
  ability; neither was needed this cycle (no invariant trip, no runaway).
- **Wrap-phase deaths — orchestrator leg verified, child leg = J6:** the
  orchestrator wrapped with books complete; the child case is the watch
  item above.
- **Poll cadence vs the 120s cap:** 105–110s sleep-paced polls are the
  doctrine working as designed, not friction; token cost ~2–3k per poll
  is the price of the cap and it's fine.
- **driver.rs 2,763 lines (+558 total src since cycle 3):** cohesive
  (loop/budget/trim/notices + in-module tests); no incident traces to
  file size. No row.

## 4. Capability gaps (against the human specs' direction)

- **Budget observability in events — open, filed (T17).** The one gap
  this corpus actually demonstrates.
- **K2 codification (human decision, carried):** the events-harvest
  convention has now been practiced two cycles running; amending LOOP-SPEC
  §2.5 to *require* it — and whether to also harvest child
  transcripts/ledgers (the T15 child's final-turn reasoning died with its
  worktree) — is the operator's call; LOOP-SPEC is a human spec.
- **Child launch budgets (human decision, new):** LOOP-SPEC §2's child
  template carries `--max-iters 40 --max-minutes 35` but no `--max-tokens`
  — T15's knob exists but the doctrine doesn't use it yet.
- **Single-driver tooling (`chug doctor`) — human decision (carried).**
  This cycle's invariant check was two `ps` greps; not needed, but the
  hand-rolling continues.
- **Model routing/escalation (carried):** four glm children across cycles
  2–3, zero fallbacks to kimi needed; manual fallback remains sufficient.
- **Sandbox policy / stray I7 cargo symlink (carried, unchanged).**

## 5. Top 3 priorities

1. **T17 — budget telemetry in events.jsonl (observability, pri 3).** The
   only new gap in an otherwise clean corpus; small (one Event variant +
   three run_start fields), and it makes T13/T15's legs and J6 empirically
   verifiable from harvested child streams alone.
2. **(No second row filed.)** The T15 validator's three weak-test
   survivors stay carried per cycle-3 doctrine ("if a third one
   accumulates" — none did); T17's validation sweep re-kills those
   mutants as adjacency smoke instead.
3. **(No third row filed.)** Queue honesty over queue mass: everything
   else is verified-fixed, resolved-with-data (J5), doctrine for the
   human, or a single-occurrence watch item (J6).

## 6. Handoff — recommended execution order

**LOOP-SPEC Phase 2 (this cycle):**
1. **T17** — touches `src/driver.rs` + `src/events.rs` (+ `eventlog.rs`,
   `chat.rs` call sites) → adversarial validation **REQUIRED** (kimi);
   ask the validator to also re-kill the carried T15 budget mutants as
   adjacency smoke.

**Human-decision items (no rows filed):**
1. LOOP-SPEC §2.5: codify the events-harvest convention (K2, practiced
   2×); decide whether child transcripts/ledgers harvest too (L2's "why"
   currently dies with the worktree).
2. LOOP-SPEC §2 child template: add `--max-tokens` (T15's knob, unused by
   doctrine so far).
3. `chug doctor` single-driver/process inspection (carried).
4. Automatic model routing/escalation vs manual fallback (carried).
5. Bash sandbox policy; stray I7 cargo symlink (carried).

## Outcomes (filled at cycle wrap — LOOP-SPEC Phase 3)

Cycle 4 executed 2026-09-25 ~13:00–13:40 EDT, one `chug run --spec
LOOP-SPEC.md` session (kimi-k3 orchestrator; glm-5-3-flash implementation
children; kimi-k3 validators), wrapped on the T13 warning with 5
iterations to spare — the warning working as designed on the orchestrator
itself.

**Landed (1/1 queued rows):**
- **T17 — budget telemetry in events.jsonl** (impl `0abc5de`, merge
  `96cc8ef`, row flip `a74e979`). The glm child ran 40/40 iterations and
  died pre-commit (J1, 2nd post-T13); the orchestrator harvested the
  complete on-spec diff (6 files, +283/−15, gates 353+3 + clippy green).
  Validation took three children: validator 1 (kimi) died 40/40 with the
  review done and mutants 1/2/4 killed but the verdict unwritten (J1,
  3rd post-T13); validator 2 was killed early after forming a wrong-HEAD
  belief; the orchestrator ran the remaining 3 mutants itself (all died,
  including the T15 adjacency smoke — the `max_tokens>0` guard and the
  iteration-abort boundary); validator 3 signed off **VERDICT: PASS**
  (re-killed 2/2 samples, full correctness pass against spec reqs 1–5,
  bounded gate 353+3, goal accepted at 35.5k/11k tokens).

**Filed but unworked (budget, per Phase-2 budget check):**
- **T18 — WARN_REMAINING_ITERS 5→8** (row + spec `edd636b`, pushed). J6
  fired mid-cycle: 3 of the last 4 children died at the iteration ceiling
  with work complete but wrap unfinished (T15 impl, T17 impl, T17
  validator 1 — which acknowledged the warning at 36 and still fell ~3
  turns short). Next cycle's first row; spec names every pin to flip.

**What the validators caught:** no implementation defects — third
consecutive clean glm implementation round. The catch this cycle was
process-shaped: validator 1's death proved J6 better than any transcript
archaeology could, and validator 2's confusion showed that "confirm vs
commit <sha>" instructions invite git-probe misreads (validator 3's goal
used `git diff main` ground truth instead — worked first try).

**Process notes:** glm-5-3-flash ran ~3s/iteration — child economics are
now iteration-bound, not wall-clock-bound (a 40-iteration child finished
in ~4 min). K2 harvest practiced: all four T17 child event streams are in
the main `.chug/` (`events-t17-impl/validate/validate2/validate3-*`).
The orchestrator garbled two launch commands (shell-quoting); both were
caught by polling within minutes, one cost validator 2's early kill.
T17's own feature shipped mid-cycle too late to help this cycle's
diagnosis — next cycle's children will have their warning injections on
the jq record.

**Final state:** TODO.md T1–T17 `done` with commit refs, T18 `todo`;
`tests/todo_consistency.rs` green; main-tree gates 353+3 green, clippy
clean; README documents T17 (rode the impl commit `0abc5de`); eval +
merge + row flips + T18 filing all pushed to origin.
