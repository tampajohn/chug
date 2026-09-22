# EVALUATION — chug, assessed by chug-loop (2026-09-22, cycle 3)

Corpus: `.chug/events-20260922-060458.jsonl` (cycle-3 halted run: 80/80
iterations, 93 tool results, 1 abort, jq-mined per the events-first
upgrade), `.chug/events-20260922-035728.jsonl` (cycle-2 orchestrator: 75
iters, 97 tool results, 0 errors, goal accepted — the healthy reference
run), `.chug/transcript-20260922-060458.jsonl` (cycle-3, incl. the T13
budget-low warning in the wild), `LEDGER-20260922-060458.md` (cycle-3's
no-op halt ledger — the doctrine payload), `TODO.md` (T1–T14 done), git
log through `988741d`, `src/` (16,637 lines), `README.md`. Prior
evaluations: cycle 2 at `0d71d00`, cycle 1 at `e962f1f`.
Verification performed: full `cargo test` = **335+3 green in 9.05s**;
`cargo clippy --all-targets -- -D warnings` clean; code inspection of the
budget path (`src/driver.rs:380-408`, `:464-476`, `:748+`), usage
accumulation (`:515-525`), `BudgetExceeded` (`src/events.rs:70-101`), and
the mcp_http constants/headers (`src/mcp_http.rs:31-52`, `:572`, `:797`).

## 1. What chug does well

- **The doctrine loop works end-to-end.** Cycle 3 produced zero code and
  still paid: its halt ledger identified the 120s bash cap and the
  watch-and-wait burn, and the operator codified both into LOOP-SPEC the
  same night (`988741d` — invariant trips on `chug run` only, fast BLOCKED
  exit, backgrounded+polled children; `f554871` — auto-push). Failure →
  ledger → spec amendment in one turn.
- **T13's budget-low warning fired correctly in the wild on its first
  trial.** Cycle-3's transcript carries `chug: budget low — 5 iteration(s)
  and 115 minute(s) remain. Stop starting new work…` at iteration 75
  (transcript-20260922-060458.jsonl:152); the session then wrapped
  cleanly (gates + ledger) instead of dying mid-bookkeeping — the exact
  J1 death mode it was built for.
- **Cycle 2 is the healthy-run reference:** 97 tool calls, **zero**
  `is_error`, zero PATH preambles, zero revert-thrash — the T4/T5 fixes
  continue to hold across model families (glm children, kimi orchestrator).
- **Events-first evaluation keeps paying.** Cycle 3's failure shape (63
  identical `CONFLICT: PID 40170` polls, one 120s process-group kill, one
  `sh` process-substitution syntax error) is fully reconstructible from
  `jq` alone; no transcript archaeology required.

## 2. Incidents worth fixing

### Cycle 3, dissected (the no-op halt)

Cycle 3 tripped the single-driver invariant on an interactive `chug chat`
(PID 40170), correctly refused to work — then held a **paced watch for
~2h**: 63+ near-identical bash polls, burning **80/80 iterations and
712,215 input + 64,091 output tokens** (events-20260922-060458.jsonl,
last iteration 06:03:34Z; abort: `budget_kind: iterations, max: 80`) on a
run that could never produce anything. Three sub-findings:

- **K1 → T15 — there is no token-denominated budget.** Iterations and
  wall-clock are the only ceilings (`src/driver.rs:386-408`); cycle 3 maxed
  its iteration budget while using **29 of 240 wall-clock minutes** — the
  two existing budgets were both the wrong shape for a cheap-poll watch,
  and only iterations (barely) caught it. The cycle-2 eval (§4) deferred a
  token budget "if J2-class sessions keep recurring" — they recurred within
  a day (712k tokens on a *no-op*; the T11/T12 session's 8.68M input tokens
  was the prior exhibit). All plumbing exists to make this cheap:
  `BudgetExceeded` enum + T12 abort output + T13 warning legs.
- **K2 — child telemetry dies with the worktree (doctrine, no code row).**
  Cycle 2's glm children ran clean, but their `.chug/events.jsonl` was
  destroyed with `/tmp/chug-loop-t13/t14`, so their token costs and peak
  contexts are unknowable (cycle-2 Outcomes: "max context unseen") — which
  is exactly why J5 (below) stays unquantified. Fix is a convention, not
  code: harvest `/tmp/chug-loop-t<N>/.chug/events*.jsonl` into the main
  repo's (gitignored) `.chug/` before `git worktree remove`. Practiced
  this cycle; amending LOOP-SPEC §2.5 to require it is the human's call
  (LOOP-SPEC is a human spec — META-META hard rule).
- **K3 — assessed, no row: two minor bash-tool bumps.** One `sh: -c:
  syntax error near unexpected token '<'` (process substitution — the
  bash tool is `sh -c`, and chug's own tool docs say so; the model adapted
  immediately) and one `timed out after 120s (process group killed)` —
  which *was* the valuable empirical discovery of the hard bash cap, now
  codified (`988741d`). Neither warrants a code row.

### The T13 warning's one cosmetic wrinkle

- **K4 — assessed, no row.** The budget-low message names *both* kinds
  ("5 iteration(s) **and 115 minute(s)** remain") even when only one is
  binding — in a watch-mode burn the minutes figure is noise. Cosmetic;
  the warning did its job (clean wrap), and cycle-3's model confusion
  about remaining iterations was self-inflicted reasoning, not a harness
  defect. Not worth a row.

### Carried debt finally filed

- **K5 → T16 — SPEC-9's R4 validator gaps were carried, never filed.**
  `LEDGER-spec9-archive.md` (2026-09-21): "Carried, low-severity test-only
  gaps from R4 validator, could seed TODO: Accept-header pin test,
  tools/list-over-SSE framing test, timeout-constant pins." Five cycles of
  ledgers later they exist nowhere in TODO.md — carried-debt items only
  survive if the evaluator re-files them. All three are cheap stub-test
  pins in `src/mcp_http.rs` (Accept headers at `:572`/`:797`; constants at
  `:31-52`).

### Watch items

- **J5 (continues, no row).** glm-5-3-flash's context window vs the 120k
  *estimated* trim (chars/4, `src/driver.rs:797+`). Cycle-2's glm children
  (28 + 18 iters) ran clean, but K2 destroyed the evidence. With the K2
  harvest convention this cycle, the next evaluation gets real numbers.
- **Single-driver detection as a feature.** Three cycles now have each
  hand-rolled `lsof`/`pgrep` invariant checks (cycle 3 ran 63 of them). A
  `chug doctor` subcommand could own this — human-decision item, listed in
  §4, not filed.

## 3. Friction hot spots

- **PATH tax / revert-thrash — fixed, holding (3rd eval running):** zero
  recurrences across cycle-2's 97 and cycle-3's 93 tool calls.
- **Watch-and-wait token burn — process-fixed, harness hole open:** the
  fast-BLOCKED-exit doctrine (`988741d`) removes the *reason* to watch;
  T15 closes the *ability* to burn unbounded tokens in any run shape.
- **Wrap-phase budget deaths — fixed, verified in the wild (K-corpus):**
  T13 fired on first trial; cycle 3 wrapped with books complete.
- **Cost observability — console fixed, ceiling missing:** T14 surfaces
  spend at abort/goal; T15 is the natural completion (the *limit* leg).

## 4. Capability gaps (against the human specs' direction)

- **Token budgets — open, filed (T15).** The third budget leg.
- **Orchestration:** LOOP-SPEC now runs the full cycle with
  backgrounded+polled children; supervision/kill/harvest primitives stay a
  human decision (carried). K2's events-harvest convention recommended for
  LOOP-SPEC §2.5.
- **Model routing/escalation:** manual fallback only (unchanged, human
  decision, carried). Cycle 2's glm children needed no fallback.
- **Single-driver tooling:** `chug doctor`-style invariant/process
  inspection — human decision (new this cycle).
- **Sandbox policy:** unrestricted-by-design (carried); the stray I7
  cargo symlink remains a remove-or-keep human decision (carried).

## 5. Top 3 priorities

1. **T15 — token-denominated budget (robustness, pri 2).** The only new
   failure mode in the cycle-3 corpus with no mitigation: 712k tokens on a
   no-op, caught only by the iteration ceiling. Reuses T12/T13 plumbing;
   single-child sized.
2. **T16 — SPEC-9 R4 test pins (robustness/tests, pri 3).** Clears the
   oldest carried debt; three small stub-test pins in mcp_http.rs.
3. **(No third row filed.)** Queue stays lean: everything else is
   verified-fixed, doctrine for the human, or a watch item awaiting K2
   data.

## 6. Handoff — recommended execution order

**LOOP-SPEC Phase 2 (this cycle), in order:**
1. **T15** — touches `src/driver.rs` + `src/events.rs` → adversarial
   validation REQUIRED.
2. **T16** — tests-only (`src/mcp_http.rs` test module) → validation
   optional per LOOP-SPEC §2.4; orchestrator gates suffice.

**Human-decision items (no rows filed, carried unless noted):**
1. LOOP-SPEC §2.5 amendment: require harvesting child
   `.chug/events*.jsonl` into the main tree before worktree removal (K2,
   new this cycle — recommended).
2. `chug doctor` subcommand for single-driver/process inspection (new).
3. Automatic model routing/escalation vs the manual fallback (carried).
4. Fleet supervision primitives vs bash conventions (carried).
5. Bash sandbox policy (carried); stray I7 cargo symlink (carried).

## Outcomes (filled at cycle wrap — LOOP-SPEC Phase 3)

_(pending)_
