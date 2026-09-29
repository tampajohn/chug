# T146 — F2 phase 2a: `chug run --approve <plan.md>` gate + web_fetch in plan mode (ROADMAP PULL)

check: cargo test

estimate: ~300 changed lines (src ~150, tests ~130, README ~20) — the
~400 SHOULD-split band respected: 2a is itself the split (cycle-36's F2
phase 2 bundled three surfaces; the `/plan` chat toggle stays deferred
as phase 2b, reason in EVALUATION.md cycle-70 §4)

## Repo context

FEATURES.md F2 (Plan mode): phase 1 landed T73 (`chug plan` read-only
mode: five tools — the read-only four + `submit_plan` — dispatch-side
rejection of everything else, src/plan.rs). Phase 2 was deferred at
cycle 36 with the reason "chat/approve surfaces multiply the diff past
a 50-iter child budget" — obsolete (80-iter children + T110 estimate
discipline). This row is phase 2a: the two headless/loop-serving
surfaces.

Surface 1 — approved-plan gate for `chug run`. Claude Code parity:
plan first, execute only against an operator-approved plan. chug's
headless shape: `chug run --spec S --goal G --approve plan.md`.

- The file must exist, be a readable regular file, and be non-empty
  after trim — otherwise the run REFUSES to start (exit nonzero with a
  stderr message naming the path and the leg that failed), BEFORE any
  `.chug/` write (mirrors the goal-pack expansion ordering, T117).
- On success the plan text is injected as the run's execution contract:
  one user-side preamble block prepended to the run's first context
  ("The operator approved this implementation plan; implement it,
  then satisfy your goal's check." + the plan text verbatim) — the
  `--goal` text still names the objective and the spec's `check:` line
  still governs acceptance. The plan constrains HOW, the goal names
  WHAT, the check decides DONE.
- `run_start` in `.chug/events.jsonl` records `approve` (path) and
  `plan_sha256` (of the file bytes) next to the existing
  `goal_pack`/`goal_sha256` fields — always-present fields, `null`
  when the flag is absent (the T117 honesty shape).
- `--approve` is accepted on `chug run` ONLY (clap): `chug plan`
  PRODUCES plans, `chug chat` is interactive (phase 2b territory);
  either receiving the flag is a clap error.
- Relative paths resolve against the run cwd and must stay inside it
  (the read_file cwd-sandbox rule — an approved plan is repo-local
  input, not a path into $HOME).

Surface 2 — web_fetch admitted to plan mode's read-only tool set.
web_fetch is read-only by design (GET-only, http/https, size-capped —
the T37 contract), so it satisfies plan mode's contract; research
during planning is the benchmark shape (Claude Code plan mode fetches
docs). src/plan.rs: `READ_ONLY_TOOLS` (:39), `PLAN_TOOL_NAMES` (:43),
`tool_schemas()` (:72), the dispatch rejection list, and
`PLAN_PREAMBLE` (:46) each gain web_fetch by the same pattern the
other four follow. Everything else about plan mode is byte-unchanged
(submit_plan remains the only write exit; all write tools still
rejected by name).

## Requirements

1. Both surfaces above, minimal diffs, no new dependencies.
2. Sweep: every place that enumerates plan-mode tools (schema builder,
   dispatch rejection, preamble, README, any tests pinning the count
   or the exact list) updated in the same commit — a count pin left at
   5 is a RED escape.
3. README: Plan mode section gains the web_fetch sentence; Autonomous
   mode gains a short `--approve` paragraph (contract semantics, the
   refuse-before-.chug ordering, the run_start fields).
4. Out of scope (phase 2b+, do not build): `/plan` chat toggle,
   interactive approve flows, `--approve` for chat/plan, plan diffing
   or approval signatures.

## Tests

- clap legs: `--approve` parses on run; errors on plan and chat.
- Refusal legs (each RED pre-fix): missing file, unreadable/empty file,
  path escaping the cwd — each refuses BEFORE `.chug/` is touched
  (assert no `.chug/events.jsonl` created), exit nonzero, message names
  the leg.
- Contract leg (stub-model harness): a run with `--approve` shows the
  plan text in the first outbound request; `run_start` carries
  `approve` + `plan_sha256`; a run WITHOUT the flag has both fields
  null.
- web_fetch legs: advertised in `tool_schemas()`; dispatch accepts it
  (tool-error-free path against the existing web_fetch test seam or a
  name-level dispatch assertion); every write tool still rejected with
  the allowed-set error; `PLAN_PREAMBLE` mentions web_fetch.
- Sweep leg: the plan tool-count/list pin updated and proven RED when
  reverted (the mutant the validator will flip first).

## Acceptance

- `cargo build && cargo clippy --all-targets -- -D warnings && cargo test`
  all green in the worktree.
- FEATURES.md F2 row annotated in the row-flip commit (orchestrator
  does this, not the child): phase 2a landed, 2b deferred.
- Validator REQUIRED (src/driver.rs + src/main.rs + src/plan.rs — driver
  is core-list). Mutation candidates: drop the before-`.chug` ordering
  (must die), drop the null-fields shape (must die), approve accepted on
  plan/chat (must die), web_fetch removed from one of the four
  enumeration sites (must die on the sweep leg).
