# T70 — `decision_log` tool: structured loop decision records (.chug/decisions.jsonl)

FEATURE-class (FEATURES.md F13, phase 1 of 3 — operator directive dc18a7a).

check: cargo test --bin chug decision && grep -c "decision_log" LOOP-SPEC.md && cargo test

## Repo context

The loop's long-term speed lever is shrinking kimi's share of loop judgments
to the hard cases (F13: Laya fine-tune on decision records, then
confidence-gated first-pass routing). That needs a corpus, and today every
loop-level judgment — the orchestrator's kimi-validation routing call per
item, the verdict it accepts, its recovery routing on a child budget death,
its model-fallback call, the evaluator's file/reject triage — lives only as
prose scattered across ledgers, commit messages, and EVALUATION.md. Not one
is machine-readable; a fine-tune pipeline has nothing to eat.

The precedent is the risk gate's `.chug/risk_verdicts.jsonl`
(src/riskgate.rs:132, `log_verdict`): append-only JSONL, one decision per
line, best-effort (a write failure never aborts the run). F13 names
risk_verdicts.jsonl as the seed corpus; this item generalizes the pattern to
loop judgments with a model-facing tool.

Module precedent: `src/webfetch.rs` (T37) — a standalone module exposing
`pub fn schema()` + the tool fn, registered with TWO one-line edits in
src/tools.rs (`tool_schemas()` push + `inner()` dispatch arm) plus
`mod webfetch;` in src/main.rs. decision_log follows it exactly so the
tools.rs monolith (5,233 lines, past the T71 trip line) does not grow.

## Requirements

1. **New module `src/decisions.rs`** (webfetch.rs pattern): all logic lives
   here; src/tools.rs gains ONLY the two registration lines (with the
   T37-style comment) and src/main.rs gains `mod decisions;`.

2. **New builtin tool `decision_log`.** Input schema (all required unless
   noted):
   - `class` (string) — the decision class. The schema description names the
     seed classes verbatim so the model does not invent synonyms:
     `validation-routing`, `validation-verdict`, `recovery-routing`,
     `model-fallback`, `eval-triage`, `outcome`. Free string beyond that
     (new classes must not need a code change — the description says so).
   - `subject` (string) — what was decided, compact (e.g. `T70`, or
     `cycle-34-eval candidate: wait_secs pacing`).
   - `inputs` (string) — the compact evidence (e.g. `files:
     src/tools.rs+LOOP-SPEC.md; diff +863/-16; 3 mutants 1 survivor`).
   - `options` (string) — the options considered (e.g. `kimi-required |
     kimi-optional | skip`).
   - `choice` (string) — the chosen option.
   - `confidence` (number, 0..=1 inclusive) — the deciding model's stated
     confidence; the field F13's confidence-gated routing will train against.
   Outcome backfills are ordinary records with `class: "outcome"`, `subject`
   naming the earlier decision id, and `choice` one of `landed-clean`,
   `fixed-up`, `reverted` (the description says exactly this).

3. **Append-only writer**: each call appends ONE JSON object line to
   `<cwd>/.chug/decisions.jsonl` (cwd = ToolCtx cwd; children in worktrees
   write their own, harvested like events streams — no special casing).
   Record shape: `{"id","ts","class","subject","inputs","options","choice",
   "confidence"}` — `ts` = unix seconds; `id` = `d<ts>-<n>` where `n` is a
   per-process monotonically increasing counter (std-only; a process-shared
   AtomicU64 is fine). Create `.chug/` if missing. No rotation in v1
   (volume is a handful of lines per cycle — say so in the module comment).

4. **Tool result**: ok returns `recorded <id>` (the model needs the id to
   backfill outcomes later). Validation failures (missing/non-string field,
   confidence not a number in 0..=1) are tool errors naming the field — the
   dispatch wrapper already converts errors to `is_error` results, so a
   failure can never abort the run; a write I/O failure likewise returns a
   tool error (never panic).

5. **Doctrine adoption in LOOP-SPEC.md** (ship+adopt in one item — the
   T23→T24 zero-calls lesson), FOUR one-sentence insertions, NO step
   renumbering (T19 rule: t13's `§2.5` ref must keep resolving):
   - §2 step 2 (after the T63 budget-death recovery paragraph): a child
     budget-death recovery routing (resume / orchestrator-finish /
     next-cycle) AND any glm→kimi model fallback are each logged via
     `decision_log` (classes `recovery-routing` / `model-fallback`).
   - §2 step 4: the per-item validation routing call (REQUIRED / optional /
     skipped + why) and the validator verdict received (PASS/FAIL + findings
     count + survivor count) are logged (`validation-routing` /
     `validation-verdict`).
   - §2 step 5 (the row-flip paragraph): the row-flip commit appends an
     `outcome` record per decision id logged for the item — `landed-clean`,
     or `fixed-up` when a fix-up arc ran; a later revert appends `reverted`.
   - Phase 1: every filed row AND every weighed-and-rejected candidate gets
     an `eval-triage` record (the reject half is what teaches a future
     classifier the negative class).
   Each sentence names `decision_log` and its class verbatim. Do NOT edit
   META-SPEC.md (human file — the §6 goal stays byte-identical; verdict
   logging is orchestrator-side by design).

6. **README.md Tools section**: the tool list gains `decision_log` and one
   sentence in the Tools intro area — the loop's bookkeeping surface next to
   `update_ledger`: structured decision records to `.chug/decisions.jsonl`
   (append-only, best-effort) feeding the F13 distillation corpus. It is
   cwd-sandboxed; the two documented sandbox exceptions stay exactly two.

7. **Events stream**: out of scope — events.jsonl already logs the
   tool_result preview; no new Event variant (state this in the module
   comment so nobody "fixes" it later).

## Tests (src/decisions.rs `#[cfg(test)]` module, tempfile pattern per riskgate.rs)

- **Schema pin** (T22 convention): LIVE `tool_schemas()` contains
  `decision_log` with all six properties in `required`, and the description
  carries the seed-class tokens verbatim (`validation-routing`,
  `recovery-routing`, `eval-triage`, `outcome`) — reverting any one fails
  the pin.
- **Dispatch round-trip**: a call against a tempfile ToolCtx writes exactly
  one parseable JSON line carrying every field; the `recorded <id>` result
  content echoes the line's `id`.
- **Append-only**: two calls → two lines; ids distinct and the counter
  component increases.
- **Outcome record**: `class: "outcome"` with a prior id as subject
  round-trips byte-faithfully.
- **Validation legs**: missing field, non-string field, confidence −0.1,
  1.1, and `"high"` each → tool error naming the field; boundaries 0.0 and
  1.0 accepted.
- **Best-effort**: a FILE sitting where `.chug/` should be created → tool
  error, no panic (the run survives).
- **Non-vacuousness hand-check** recorded in the commit message: delete the
  append → the two-lines test goes RED; restore → green.

## Acceptance

- `cargo build && cargo clippy --all-targets -- -D warnings && cargo test`
  all green in the worktree (export
  `CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared` first).
- The `check:` line at the top passes verbatim from the worktree root.
- LOOP-SPEC.md's four insertion points carry the sentences; every other
  byte of LOOP-SPEC.md unchanged (diff review).
- No edits to META-SPEC.md, META-META-SPEC.md, TODO.md, or LEDGER.md.

## Out of scope (F13 phases 2–3, deferred with reasons in EVALUATION §4)

- Laya fine-tune export pipeline (needs a corpus worth training on —
  accumulate cycles first).
- Confidence-gated first-pass routing (needs the trained judge; a layad
  decision-class endpoint does not exist yet).
- Rotation / compaction of decisions.jsonl (volume does not justify it).
