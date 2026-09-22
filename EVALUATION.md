# EVALUATION — chug, assessed by chug-loop (2026-09-21, cycle 2)

Corpus: `.chug/events-20260922-032901.jsonl` (70 iterations, 112 tool results,
2 aborts — the T11/T12 self session, jq-mined per LOOP-SPEC's events-first
upgrade), `.chug/transcript-20260922-030812.jsonl` (322 lines: the T3/T7/T8/T9
session + the T6/T10 session), `.chug/transcript-20260922-032901.jsonl` (432KB,
T11/T12), four LEDGER archives, `TODO.md`, git log, `src/` (16,381 lines),
`README.md`. Prior evaluation (I1–I13) lives at commit `e962f1f`.
Verification performed: full `cargo test` = **327+3 green in 9s**;
`tests/todo_consistency.rs` green (every `done` row's spec on disk); false-
positive checks on suspicious transcript entries (J4); code inspection of the
budget-check path (`src/driver.rs:374-398`), trim path (`:797+`), and
ConsoleSink (`src/events.rs:145+`).

## 1. What chug does well

- **The loop closed its own backlog.** T1–T12 all landed, each with a spec
  file and a commit ref in TODO.md, guarded by T8's consistency test so the
  books can't silently drift again.
- **T10's events.jsonl paid off in one day.** This evaluation ran primarily on
  `jq` over events (token sums, abort timestamps, tool-error counts) instead
  of mining trimmed, spliced transcripts — exactly the use case T10 was filed
  for. Zero `[trimmed]` lines fought.
- **T11's banner self-verified.** The second `run_start` in
  `events-20260922-032901.jsonl` carries `"commit":"a115a71","version":"0.1.0"`
  — the fix visible in its own telemetry stream.
- **The T4/T5 friction fixes hold.** The T11/T12 session: 112 tool calls,
  **zero** `is_error`, zero `export PATH` preambles, zero `git checkout --`
  reverts (events.jsonl + transcript grep).

## 2. Incidents worth fixing

### Existing TODO rows — assessment (all done, not re-filed)

T1–T12 are all `done` with commit refs; spot-verified in code: T10 rotation
fired correctly for *this* session (events + transcript archived at 23:29),
T12's abort output names model/budget (`src/events.rs` ConsoleSink), T8's
guard rejects malformed rows (tests/todo_consistency.rs, 3 tests green).

### New incidents

- **J1 → T13 — wrap-phase budget deaths, three in one evening.** The loop
  checks budgets only at the top of each iteration (`src/driver.rs:374-398`)
  and aborts with **no advance signal**, so wrap work (commit, gates, TODO
  flip) — which always comes last — is what dies:
  1. T10: budget-aborted with the implementation complete and green but
     **uncommitted**; harvested post-mortem by the operator
     (`a115a71` todo note).
  2. T11/T12 session: abort at `03:15:40Z` (40 iters), resumed, abort again
     at `03:19:09Z` (30 iters) **right after the T12 code commit**
     (`.chug/events-20260922-032901.jsonl` abort events;
     `.chug/LEDGER-20260922-032901.md`: "operator ran the gates, flipped the
     TODO row, wrote this wrap").
  3. Same pattern forced LOOP-SPEC §2.5's "orchestrator owns the books" rule
     (process mitigation). The in-harness fix is a one-shot budget-low
     warning so the model itself reprioritizes to commit+wrap (T13).
- **J2 → T14 — token-cost blindness in headless output.** ConsoleSink drops
  `Event::Usage` (`src/events.rs`: `Event::Usage { .. } => {}`), so
  `chug run` prints no token totals at abort or goal-complete. The T11/T12
  session burned **8,683,323 input + 1,243,749 output tokens** over 70
  iterations (jq sum; last-iteration context 101,576) — invisible without
  jq-mining events.jsonl, which is what this evaluation had to do. TUI mode
  shows live tokens in the title; only the console sink is blind.
- **J3 — assessed, no row (covered by T13 + routing doctrine).** The T10
  session's acceptance smoke test hit a muse-endpoint proxy 400
  (`/tmp/t10-smoke`, `03:01:08Z`, tail of transcript-...-030812) and burned
  the session's final iterations on an environmental failure — the model
  correctly diagnosed it, too late to matter. `3c795b3` already re-routed the
  default implementation child to glm-5-3-flash via tools-proxy (muse now
  opt-in). Lesson for spec authors: acceptance criteria that require *live
  external endpoints* are budget hazards; prefer hermetic verification.
- **J4 — false positive, verified, no row.** `Goal: {}` / `Goal: OLD SESSION`
  / `Goal: x` in the old transcript pile are `src/driver.rs:248` source and
  `:1410` test fixtures read into context, not real sessions; the tests use
  tempdirs (`driver.rs:1407+`). Checked before filing.
- **J5 — watch item, no row.** `transcript_trim` triggers at 120k *estimated*
  tokens via chars/4 (`src/driver.rs:797+`), which underestimates code by
  ~20-30%. kimi-k3 tolerates it; **glm-5-3-flash (the new default
  implementation child) has an unverified context window** — if it's 128k,
  real tokens could cross the window before trimming, and T1's 4xx fail-fast
  turns that into a hard child death. First glm child sessions will provide
  the evidence; file a row only if it fires.

## 3. Friction hot spots

- **PATH tax / edit_file revert-thrash — fixed, verified (J-corpus):** zero
  recurrences in 112 post-fix tool calls.
- **Wrap-phase budget deaths — open, ×3.** → T13 (the only repeated failure
  mode in the new corpus).
- **Cost observability — half-fixed.** T10 records tokens; the console
  doesn't surface them. → T14.
- **Live-endpoint acceptance criteria — open as doctrine.** J3; spec authors
  should write "any working endpoint" and prefer hermetic checks.

## 4. Capability gaps (against the human specs' direction)

- **Orchestration:** LOOP-SPEC (`aac3629`) now defines the one-command cycle
  — this session is its first execution. Children are still raw bash +
  worktrees with mtime polling; no supervise/kill/harvest primitives (human
  decision, carried).
- **Model routing/escalation:** manual fallback only (T12 hint + LOOP-SPEC
  fallback rule). Automatic `--fallback-model` stays a human decision.
- **Token budgets:** iterations + wall-clock only; no token-denominated
  budget. T14 surfaces the data; a token budget is a future row *if* J2-class
  sessions keep recurring.
- **Context-window safety for smaller models:** J5 watch item.
- **Sandbox policy:** unrestricted-by-design (SPEC.md). The I7 symlink
  artifact (`~/.claude/plugins/cache/typesafe-ai/typesafe/0.5.7/bin/cargo`)
  still exists; T4 made it non-load-bearing for chug — remove-or-keep stays a
  human decision (carried).

## 5. Top 3 priorities

1. **T13 — budget-low warning (robustness).** Three deaths, one evening, same
   phase. The only repeated failure mode in the new corpus; cheap (one-shot
   message injection + two flags).
2. **T14 — cumulative tokens in console output (DX).** Completes the T10
   observability story; a few lines in ConsoleSink plus tests.
3. **(No third row filed.)** The queue is deliberately lean: every other
   finding is either verified-fixed, covered by existing doctrine, or a watch
   item awaiting evidence (J5). Filing speculative rows burns child budget.

## 6. Handoff — recommended execution order

**LOOP-SPEC Phase 2 (this cycle), in order:**
1. **T13** — touches `src/driver.rs` → adversarial validation REQUIRED.
2. **T14** — touches `src/events.rs` → adversarial validation REQUIRED.

Both are single-child-round sized (narrow diffs, scripted-harness tests).

**Human-decision items (no rows filed, carried from cycle 1):**
1. Automatic model routing/escalation vs the manual LOOP-SPEC fallback.
2. Fleet supervision primitives (`chug supervise`) vs bash conventions.
3. Bash sandbox policy (unrestricted-by-design vs guardrails).
4. The stray I7 cargo symlink — now non-load-bearing; remove or keep.

## Outcomes (filled at cycle wrap — LOOP-SPEC Phase 3)

Cycle 2 executed 2026-09-21 23:29–23:59 EDT, one `chug run --spec LOOP-SPEC.md`
session (kimi-k3 orchestrator; glm-5-3-flash implementation children; kimi-k3
validation children).

**Landed (2/2 rows, both mutation-validated):**
- **T13 — budget-low warning** (`96dbe99`, merge `3c1c5b8`, row flip `8a2a084`).
  glm child in 28 iterations; kimi validation VERDICT: PASS with **9/9
  mutations caught** (threshold flips, `||`→`&&`, dropped latches/push/append,
  off-by-one, corrupted interpolation). J1's wrap-phase death mode now has an
  in-harness mitigation: one-shot notice at ≤5 iterations / ≤5 minutes.
- **T14 — cumulative tokens in console output** (`82a38d8`, merge `c46136a`,
  row flip `d972aac`). glm child in 18 iterations; kimi validation VERDICT:
  PASS with **10/10 mutations caught**. Self-verified in the wild like T11's
  banner: the validator's own goal-complete output printed
  `tokens: 80148 in / 8687 out (cumulative)`.

**Skipped/deferred:** nothing — the queue was two rows and both landed with
~30 orchestrator iterations to spare. J3/J4 were assessed in-eval (no rows);
J5 (glm-5-3-flash context window vs the 120k-estimate trim) stays a watch
item — both glm children ran clean (max context unseen but no API errors),
so the concern is weaker than feared but still unquantified.

**What the validators caught:** no defects — both rounds passed first time.
The mutations (19 total) instead proved the new tests are non-vacuous, which
is the point of the exercise (round-1's 245/245-over-dead-code lesson).

**Cost note for the next evaluator:** this cycle's two glm implementation
children were dramatically cheaper than the kimi-k3 self sessions in the
corpus (28 + 18 iterations, ~6 min and ~2 min wall-clock) — early evidence
the `3c795b3` routing decision was right.

**Final state:** TODO.md T1–T14 all `done` with commit refs;
`tests/todo_consistency.rs` green; main-tree gates 335+3 green, clippy clean;
README documents both additions (T13 bullet, T14 abort-output update).
Not pushed — the human pushes.
