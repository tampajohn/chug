# Why it's safe to let the loop write code

The question this page answers is the one every engineering-leadership
review asks first: **what stops it from doing something bad?** The answer
here is not a promise. It is an inventory of mechanisms that already run,
each cited by the file, feature id, or gate name that implements it —
follow any citation to its source; the repo is public. Where a mechanism
has a known hole (a fail-open leg, a lane that skips review), the hole is
named next to the citation, because the reviewer looking for it will find
it anyway.

This file is the source of truth; `scripts/site-sync.sh` renders it as a
section of [the docs page](https://chug.sh/docs.html#risk-gate) at every
sync (T331, the T241 docs-page pipeline), and the adopt page's safety FAQ
(`docs/adopt/faq.md`) links that section.

## The gates

What runs on loop-written code before it lands. The implementation child
is launched with "keep cargo build and cargo test green" in its goal text
(the `META-SPEC.md` §6 round-protocol template), and the gates below are
then re-run independently — "never trust a claim of green without seeing
it" (`LOOP-SPEC.md` step 3).

| Gate | Command | When |
|---|---|---|
| Full suite | `cargo nextest run --release` when `cargo nextest` is on PATH, else `cargo test --release -- --test-threads=4` — the T82 gate runner, whose fallback is unconditional (nextest is a host tool, never a hard dependency) | every item's worktree (`LOOP-SPEC.md` step 3), again post-merge (step 5) |
| Lint | `cargo clippy --all-targets --release -- -D warnings` | beside the suite, never through it (`LOOP-SPEC.md` step 3) |
| Python battery | `python3 -m unittest scripts/test_wrap_assert.py` and `python3 -m unittest scripts.test_todo_flip` — the unittest batteries behind `scripts/wrap_assert.py` (T273) and `scripts/todo_flip.py` (T279) | each script's own check line; re-run beside the cargo gates on landing (the T289 row records "gates independently re-run python 56/56") |
| TODO row guard | `cargo test --release --test todo_consistency` | after every `TODO.md` edit (`tests/todo_consistency.rs`; the row-flip path itself goes through `scripts/todo_flip.py`, which asserts the row invariants before writing — the three-fire todo-flip census, `LOOP-SPEC.md`) |
| Docs-only floor | the same `todo_consistency` run, plus the affected pin file's tests | when the diff is `*.md`-only — the T80 classification is mechanical (`git diff --name-only main...<branch> \| grep -qvE '\.md$'`), and any ambiguity defaults to full gates |

Two properties of the lint leg are load-bearing, not incidental:

- **Zero warnings, not exit-0.** `nextest` compiles `cfg(test)` code but
  never lints it, so a narrower clippy form lets test-only warnings
  through — the T176 semantics that define the bar (`LOOP-SPEC.md`
  step 3).
- **Release profile.** The gates run `--release` (T78): the first build
  into a cold cache pays compile time, every later run is faster — the
  shared target dirs (T47/T52) amortize it.

### What "green" means

- **Unpiped exits.** A gate run is unpiped, or the chain begins
  `set -o pipefail;` — a piped gate whose filter exits 0 reports green on
  a red guard (the c06a555 lesson, `LOOP-SPEC.md`'s todo_consistency
  rule).
- **Full-suite counts only.** The public test count on chug.sh is parsed
  from commit messages by `scripts/site-sync.sh`, which accepts only
  provably-full-suite shapes — an equal-operand N/N pair at or above the
  500 floor, or a "full suite N unit" label adjacent to the number. The
  T238 rule exists because a mid-run census ("nextest 113/1402") was
  once published as if it were the suite total.
- **Fail-closed publishing.** The same sync refuses to publish anything
  when its repo inputs are unreadable — `TODO.md` missing, `.chug/loopd`
  absent, or empty git log is one named error and exit 4, nothing
  written (the T217 letter, `scripts/site-sync.sh`). Fallback zeros are
  byte-identical to a genuinely empty repo's first run, so they are
  never published with real data's authority.
- **Pins, not prose.** Doctrine text is pinned by test files —
  `tests/loop_spec_*.rs`, `tests/shared_target_dir.rs`,
  `tests/nextest_gate_runner.rs` — so an edit that breaks a pinned
  clause goes red in the suite, not just in review (the T80 floor names
  them because that is the risk surface of a docs-only diff).

## The validators

The author model never validates its own work. Implementation children
run on glm; validation children run on kimi — different model families
(`META-SPEC.md` §6) — and a glm-orchestrated cycle still launches the
kimi validator (the T81 family-independence rule, `LOOP-SPEC.md` step 4:
"glm implements, so a glm-orchestrated cycle never lets glm validate
glm").

- **Adversarial validation** (`META-SPEC.md` §6 step 6): the validator
  child reviews the diff against the spec for correctness bugs, missing
  spec requirements, and weak tests, and must end with a typed verdict —
  `VERDICT: PASS` or `VERDICT: FAIL` plus a numbered findings list. It
  writes the verdict to `.chug/verdict.md` in its worktree BEFORE any
  wrap-up, so a budget death loses nothing (the T209 doctrine: a written
  verdict IS the verdict). PASS merges; FAIL feeds the findings into the
  fix-up child's goal verbatim.
- **Mutation testing** (`META-SPEC.md` §6 goal text): the validator
  deliberately breaks the new code — flip a condition, drop a check,
  corrupt a value — and confirms the tests catch it, because "green
  tests that survive mutations are vacuous (round 1 shipped dead code
  with 245/245 green until mutations exposed it)". Mutants run in
  parallel throwaway worktrees, one role-keyed target dir per leg,
  capped at 3 in flight; serial stays the default when mutants touch
  overlapping files; the main worktree must be byte-clean before the
  verdict (T79, `META-SPEC.md` §6).
- **RED-proven fixes.** A fix-up round's killing test must be shown RED
  against the broken code, not asserted after the fact — and when a
  finding names one instance of a class, the goal requires EVERY
  instance swept with its own RED-proven killing test (the cycle-33
  lesson: T69's run_start latch resets took three rounds one leg at a
  time; the sweep-the-family goal closed it in one — `LOOP-SPEC.md`
  step 4). Mutation legs for RED-proofs are applied inside the worktree
  via bash, because the confined file tools would silently produce false
  survivors (`LOOP-SPEC.md` step 4).
- **Survivor adjudication.** A surviving mutant is a finding, not a
  shrug: the verdict record logs PASS/FAIL, findings count, and survivor
  count to the decision corpus (`.chug/decisions.jsonl`, classes
  `validation-routing` / `validation-verdict` — `LOOP-SPEC.md` step 4),
  and survivors kept as weak-test findings get named individually in the
  landing record (e.g. the T316 row: "6 mutants 4 killed, 2 low-impact
  survivors ... documented as weak-test findings").
- **The lane, and what it skips.** Validation is REQUIRED for any item
  touching `src/driver.rs`, `src/api.rs`, `src/tools.rs`,
  `src/events.rs`, or the loop/spec doctrine. The low-stakes lane (T189)
  can skip the validator child — but the predicate is mechanical,
  computed from the diff and never the model's say-so: no core-list
  file, ≤ ~150 changed lines, no new tool/command surface, no CI or
  `check:` line change; any single flip forces full validation, and the
  lane skips the kimi child, never the gates (`LOOP-SPEC.md` step 4).

## The decision gate

Consequential actions pass typed verdicts before they execute — the
Laya/Jev precedent (F13/F15):

- **The runtime risk gate.** Every `bash` command is classified
  destructive/risky/safe by a Laya judge before executing;
  `destructive` p≥0.5 is blocked with an error the model can see and
  route around; verdicts are logged to `.chug/risk_verdicts.jsonl`
  (`README.md`, `## Risk gate (--risk-gate)`; `src/riskgate.rs`). The
  hole, stated: the gate fails OPEN when the judge is down, and an
  `allow destructive` steering note disables it for the run — both
  pinned in that README section, which is why the policy layers below
  it exist.
- **The policy layers under it.** `.chug/permissions.json` deny rules
  are evaluated in-process before every tool call: a rule match fails
  closed (it always denies), while a malformed rule is skipped with a
  warning (`README.md`, `## Permissions (.chug/permissions.json)`).
  `.chug/hooks.json` PreToolUse hooks veto with `[hook veto]` —
  programmable, and therefore fail-open (`README.md`, `## Hooks
  (.chug/hooks.json)`). Policy order is permissions → hooks → risk
  gate; a denied call never executes.
- **The judge itself is contested, not trusted.** The T223 bake-off
  ([docs/judge-parity.md](judge-parity.md), run by the fail-closed
  `scripts/judge-parity.sh`) measured Laya against the kev-0.8b and
  openJev-verdict-2.0 challengers over golden fixtures and a
  decision-log holdout; the recorded verdict is "Laya stays" with the
  honest gaps named — kev is candle-blocked (T222) and openJev's id
  returns 401 — rather than a fabricated comparison.
- **The baked-in judge daemon** (F15): `CHUG_JUDGE=daemon` serves the
  same model over a `0600` unix socket at `~/.chug/daemon.sock` — no
  TCP port, no network listener; filesystem permissions gate the policy
  surface (`FEATURES.md` F15, `src/daemon.rs`).
- **The decision-log corpus as evidence** (F13): every loop judgment
  emits a structured record to `.chug/decisions.jsonl` — decision
  class, compact inputs, options considered, choice, model, confidence
  — with outcome labels backfilled from git (landed-clean / fixed-up /
  reverted). The triage layer (T259) records every would-be-launch
  classification the same way (class `laya-triage`, plus the backfilled
  `outcome`), so "the loop decided correctly" is a queryable corpus,
  not an anecdote.
- **The goal gate.** A run ends when the spec's `check:` line — re-run
  by `goal_complete` verification — passes, never when a model claims
  done (`src/driver.rs`: "the spec's `check:` line still decides DONE";
  `LOOP-SPEC.md`'s check-line doctrine, including the T67 class: a spec
  `check:` line IS executable text and gets validated like code).

## The cost guardrails

The loop cannot spend unboundedly while idle. Three mechanisms, all
supervisor-computed from git and `.chug/loopd/loopd.log` — never from an
LLM's say-so (`LOOP-SPEC.md`):

- **The cheap exit (T258).** Before any launch the supervisor computes a
  mechanical predicate: no new `TODO.md` rows since the last evaluation,
  no child deaths, a bookkeeping-only delta (every commit an `eval:`
  commit touching nothing outside `EVALUATION.md` and `TODO.md`), and
  `EVALUATION.md` fresh (same UTC day). When all four hold it writes a
  one-line disposition and skips the cycle entirely — no LLM call. The
  disposition records the predicate inputs in `.chug/loopd/loopd.log`.
- **The valve (T247).** The empty-delta chain is not self-sustaining:
  after ~3 consecutive empty-delta dispositions, the 4th empty cycle
  runs the real evaluation. The trip binds on the TRUE streak —
  human-counted from the wrap-notes chain for adjudication,
  machine-read (`empty_wrap_streak`, loopd's git walk) for the
  supervisor's gate — and the cycle-118 divergence (machine read 4 vs
  TRUE 3) is the recorded reason both readings exist (`LOOP-SPEC.md`,
  "The empty-delta chain (T247)").
- **Dormancy (T325).** With an empty queue the loop is DORMANT: the
  cheap exit fires every cycle, the valve is suspended, and the weekly
  audit timer launches one READ-ONLY evaluation per week at most —
  gates green (build, clippy, test), pins verified, site-sync
  freshness, daemon liveness. It files `TODO.md` rows only on observed
  breakage, exactly one row each, zero on green, and MUST NOT file
  loop-tooling meta-work — "an empty backlog must make the loop nearly
  free; it must never make the loop invent meta-work" (`LOOP-SPEC.md`,
  "Dormant mode (T325)"; `tests/loopd_dormant_mode.rs` pins the
  battery).
- **Bounded children.** Impl children launch at `--max-iters 40
  --max-minutes 35` (`META-SPEC.md` §6 step 4); validators at 60/50 —
  the `LOOP-SPEC.md` step-4 override, raised after 4 of 4 validator
  children died at budget in cycles 66–69. A wedge dies at budget, and
  the T237 backoff paces the skipped-eval chain. The T259 triage sits
  between the mechanical layer and System Two, fail-open to the T258
  routing, with its one threshold (`TRIAGE_HIGH` 0.85) in one place in
  `loopd.sh`.

## The history

The repo is public, and it has already failed this review once.

- **The 2026-10 scrub.** On 2026-10-06 the operator reported "safety
  concerns (internal info shared about the org) in the repo, let's
  clean that up" — an internal proxy hostname in src comments and a
  spec, org HF-org names, org-private/fleet framings, and internal
  codenames across specs, runbooks, TODO, DEPENDENCIES, and test
  fixtures. The redaction pass landed as commit 47cd0d1
  (`specs/t249-internal-info-lint.md` records the derivation).
- **The standing guard.** `tests/internal_info_lint.rs` (T249) walks
  every tracked file (`git ls-files`) line by line against seven
  word-bounded pattern classes derived from that diff; any hit fails
  the suite, so the class cannot regrow through loop-authored content.
  The lint's own source composes its literals from pieces at runtime,
  so no tracked line carries a whole redaction literal. The site sync
  runs the same pattern classes over the composed adopt page before it
  may publish — a hit blocks the render and the previously published
  page rides (`t249_scan` in `scripts/site-sync.sh`, T329).
- **The secrets sibling.** `tests/no_secret_spill.rs` (T205 req 5) is
  the credential-side twin of the same doctrine.

## What this page does not claim

- The risk gate is not a sandbox: it classifies and blocks, and fails
  open when the judge is down (`README.md`, `## Risk gate
  (--risk-gate)`). The cwd confinement of the file tools (`README.md`,
  "All file-tool paths are sandboxed to `--cwd`") and the deny-rule
  layer above are the structural bounds; the gate is one policy source
  among several.
- The low-stakes lane really does skip the kimi validator for small,
  non-core diffs (T189) — the citation above names the exact mechanical
  predicate, so the reviewer can audit what "small" means rather than
  trust it.
- Fail-open and fail-closed are mixed deliberately, per mechanism, and
  the polarity is pinned per mechanism (permissions fail closed on a
  match, hooks fail open on a config problem — the two `README.md`
  sections cited above). The claim is not "nothing can fail"; it is
  that each failure mode is named, cited, and tested where this page
  says it is.
