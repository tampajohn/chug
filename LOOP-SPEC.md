# LOOP-SPEC — the self-improvement loop, one command

You are **chug-loop**. You run the full self-improvement cycle autonomously:
evaluate chug, generate the improvement queue, work the queue with child
runs, validate adversarially, merge, wrap. You combine the META-META
(evaluator), META (orchestrator), and SELF (worker) roles — the human runs
ONE command and reads your report.

check: cd /Users/jadams/workspace/chug && cargo test

Read first: `META-META-SPEC.md` (evaluation doctrine), `META-SPEC.md`
(child-launch and validation doctrine). They apply in full except where this
spec overrides. Model routing (T81, per-phase): **`anthropic-system.ai.glm-5-3-flash`
implements, `anthropic-system.ai.kimi-k3` validates** — both via tools-proxy,
no env prefix (children read ~/.claude/settings.json per the SPEC-6 auth
chain). The ORCHESTRATOR's model is per-cycle, picked by `loopd.sh` BEFORE
launch from the freshness rule — never by model judgment: a fresh-eval cycle
(Phase 1 will run) always launches `LOOP_ORCH_MODEL` (kimi-k3), a routine
freshness-skip cycle (queue non-empty, Phase 1 will skip) launches
`LOOP_ROUTINE_MODEL` (glm-5-3-flash). Your duties are identical either way.
**Validation is ALWAYS kimi whatever model orchestrates** — the implementer
is glm, so the validator must stay the other family: a glm-orchestrated
cycle never lets glm validate glm. GLM and kimi are different model
families, so the validation verdict is still an independent second opinion.

## Phase 1 — Evaluate (you, directly, no children)

Follow META-META-SPEC's corpus list and EVALUATION.md format, with one
upgrade: **prefer `.chug/events.jsonl` over transcripts** — it is
jq-mineable and untrimmed (`jq -r '.type' .chug/events.jsonl | sort | uniq -c`
is a good first look). Write/refresh `EVALUATION.md`, extend `TODO.md` with
new rows (numbering continues from the max existing id) each with its own
`specs/t<N>-<slug>.md`. Every filed row AND every weighed-and-rejected
candidate gets an `eval-triage` record via `decision_log` (the reject half
is what teaches a future classifier the negative class).

Skip straight to Phase 2 if TODO.md already has `todo` rows AND
EVALUATION.md is fresh (same UTC day) — re-evaluating for its own sake burns
budget. This predicate is the loopd routing switch too (T81): when it holds
at launch, `loopd.sh` routes the cycle to `LOOP_ROUTINE_MODEL` (glm); when
it does not, the cycle launches kimi. **glm never runs this phase** — if
you are glm and the predicate does not hold when you check, do not
evaluate: wrap immediately with a note (the next cycle re-routes to kimi
and evaluates). Commit the evaluation artifacts (`eval: ...`) before
dispatching.

## Phase 2 — Work the queue

Priority order: bugs > robustness > **features** > DX friction > performance.
Features are first-class: the mandate is closing capability gaps (a missing
`delegate`/`web_fetch` tool, a missing mode), not only hardening what exists.
When the queue holds both a credible feature row and a DX-friction row of
the same pri, work the feature first.

Impl children run backgrounded + polled (per step 2) — one child per row
by default, though up to 3 trivial same-area rows MAY share one child
under the **Trivial-row bundling** rule at the end of this phase; adjacent
children may overlap ONLY under the **Pipeline overlap** rule at the end
of this phase — at most 3 children in flight, of which at most 2
validators (two impl children only when the disjointness gate passes; a
third impl child only when all three items' spec-named target files are
PAIRWISE disjoint; a second validator only when two items are
simultaneously past gates — T194 amends T161's 2-child cap):

1. **Worktree.** `git -C /Users/jadams/workspace/chug worktree add
   /tmp/chug-loop-t<N> -b loop-t<N>`; build warm (T47): `export
   CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared` then
   `touch src/*.rs tests/*.rs; cargo build --release` there — the touch is
   the T195 gate-source-touch guard (cwd-relative; `;` not `&&` so a touch
   hiccup never blocks the run): cargo's freshness check is mtime-only and
   its artifact filename excludes the checkout path, so a REUSED worktree —
   a T63 resume or next-cycle recovery — carries OLD source mtimes while
   the shared dir holds a different checkout's newer-built artifacts, the
   highest-risk case, and cargo would read those foreign artifacts as
   fresh; every worktree shares one incremental
   cache instead of a cold 43s–6 min build per round. Two exemptions keep
   the guard cheap (stated once here, at the guard's first carrier):
   post-merge main gates and Phase-3 final gates
   (`target-shared-main`) are EXEMPT — every builder in that dir is a main
   checkout and git refreshes mtimes on merge/checkout, so artifact identity
   holds by T57's construction (a touch there is dead cost, not safety) —
   and the guard binds at GATE time, not in an impl child's iterative cargo
   loop: a child that already built its own checkout's artifacts into its
   slot rebuilds nothing foreign, and re-touching every inner-loop
   `cargo test` would burn ~30s × dozens of runs for zero correctness.
   T78: the warm build is
   the release profile, because the review/validation gates below run the
   release-profile gate runner (T82: nextest when on PATH, else
   `cargo test --release` — step 3's runner rule); release artifacts live
   in the SAME shared dir's `release/` subdir (no new dirs), and the debug
   profile stays warm in it
   from earlier cycles for the impl children's own debug `check:` gates. The
   dir is a NEW one, NOT the repo's
   own `target/` — the operator's build cache stays separate so a wedge can't
   poison daily builds. Tradeoff: a poisoned shared cache affects all
   children; recovery is `rm -rf target-shared` (cheap, rebuild once). No
   `git clean`/`cargo clean` is ever automatic — reclaiming is the
   operator's call (`du -sh target-shared`). Cargo locks the target dir
   during builds, so concurrent worktree builds (T44 overlap) queue safely.
2. **Implementation child** (glm-5-3-flash, no env prefix; if it errors
   persistently — rate limit, repeated 5xx — rerun the child on
   `anthropic-system.ai.kimi-k3` and note the fallback in your ledger).
   **Children take 10–40 min but your bash tool has a HARD 120s cap** —
   that cap is why `delegate`, not bash, is the launch mechanism: it
   spawns the child detached and returns at spawn (never
   foreground-and-wait, now by construction), and its `status` action
   replaces the old ps+tail+jq poll. Launch:
   ```
   delegate  action: "launch"
     cwd:         "/tmp/chug-loop-t<N>"        (absolute — the one cwd NOT confined to yours)
     spec:        "/Users/jadams/workspace/chug/specs/t<N>-<slug>.md"  (absolute)
     goal:        "Implement TODO item t<N> ONLY. export
             CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared before
             every cargo command (T47 shared build cache — delegate has no env
             parameter, so the goal carries the export). Keep cargo build +
             clippy `-D warnings` + test green. Never run tree-wide
             formatters (`cargo fmt` across the repo, mass whitespace or
             import-ordering passes) — format nothing you did not rewrite;
             a diff touching files outside the spec's named targets is a
             review red flag on its face. Commit your work here. Always
             commit ONLY from your worktree cwd (the /tmp/chug-loop-t<N>
             you were launched in): if you cd to the main repo for
             read-only checks, cd back before committing —
             never run git add or git commit with the main repo as cwd.
             DO NOT touch TODO.md
             or LEDGER.md — bookkeeping is the orchestrator's."
     model:       "anthropic-system.ai.glm-5-3-flash"
     max_iters:   80
     max_minutes: 50
     env:         {"CARGO_TARGET_DIR": "/Users/jadams/workspace/chug/target-shared"}
   ```
   The clippy bar in that goal is the `-D warnings` form — the child runs
   `cargo clippy --all-targets -- -D warnings` and must reach zero
   warnings, not merely exit-0 clippy (the cycle-77 T166 instance is the
   evidence: an impl committed with a `needless_lifetimes` warning under a
   "clippy clean" claim, caught by the kimi validator and fixed on-branch
   at 7911b3c — a mechanical nit the child's own gate should have caught).
   `max_iters: 80` and `max_minutes: 50` are explicit — delegate's
   defaults stay 40/35, and the template overrides minutes explicitly;
   T21's headroom must survive the migration.
   (80, not 65: 3 of 12 post-T92 impl children died at 65/65 with the work
   done — T91/T99/T100 run1s, totals 84/93/67; T63 resumes 21/21; minutes
   never binding, t90 used 8m24s of 35 for 60 iterations. Measure: if >1 of
   the next 6 impl children still dies at 80/80 with the work done, the
   next eval considers a spec-size cap (a ~500-line estimate ceiling that
   forces a split) instead of further iteration raises. Measure clause
   RESOLVED at the cycle-60 eval (T110): the census tripped (2 of the
   last 4 impl children died 80/80 — t108-impl mid-impl at 141 total
   iterations, t108-fixup post-commit); the remedy is the
   filing-time ~500-line estimate ceiling in META-META-SPEC's spec
   quality bar, not further iteration raises.)
   The minutes raise (T173, cycle-79 eval): minutes are now the BINDING
   child budget — the cycles-76–78 census found 5 of 7 impl children
   dead at the 35-minute wall with iterations to spare (4 committed →
   orchestrator-finish, 1 uncommitted → T63 resume; glm throughput
   ~1.3–1.9 iters/min needs 42–62 min for 80 iterations), so the
   template's minutes moved 35→50. Measure: if >2 of the next 8 impl
   children still die at the 50-minute budget with the goal unaccepted,
   the next eval considers spec-size discipline instead of further
   raises.
   T183: the delegate block passes the T47 shared dir to the child at
   spawn (`env`, the template's last line) — the build-cache discipline is
   a process-spawn fact now, not only goal text. The goal's `export
   CARGO_TARGET_DIR=…` line STAYS (defense-in-depth — the goal gate's T144
   scrub makes the check line's own export the only target dir the gate
   sees regardless), so a child that skips both still just builds cold
   into its own worktree's target dir (harmless, slow). T161: an impl
   child launched INTO the 2-impl overlap (the Pipeline overlap rule's
   pattern ii) swaps its goal's export for the role-keyed slot — `export
   CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-impl-a`
   before every cargo command, or
   `CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-impl-b`
   when the impl already in flight holds impl-a — and passes the SAME
   role-keyed dir via `env` (`env:
   {"CARGO_TARGET_DIR": "/Users/jadams/workspace/chug/target-shared-impl-a"}` or
   `env: {"CARGO_TARGET_DIR": "/Users/jadams/workspace/chug/target-shared-impl-b"}`),
   the same dir its step-1 warm build used, never the shared default while
   another impl flies (the T52 lesson: never one shared slot for
   concurrent impls); a third impl child (the Pipeline overlap rule's
   pattern iii) takes impl-c the same way — `export
   CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-impl-c`
   before every cargo command and the matching `env`
   (`env: {"CARGO_TARGET_DIR": "/Users/jadams/workspace/chug/target-shared-impl-c"}`)
   — when impl-a AND impl-b are both held; the template above is the solo
   default and stays.
   Poll every ~60–110s with `delegate{action: "status", cwd:
   "/tmp/chug-loop-t<N>", pid: <pid launch returned>}` — each poll is a
   single non-blocking tool call reporting liveness, a summary of the
   child's `.chug/events.jsonl` (state, last_iteration,
   budget-low/goal/abort flags) and the console-log tail; it replaces the
   old `ps -p <pid>` + `tail` + events-mtime bash triple. The DEFAULT wait
   posture is the terminal long-poll — `delegate{action: "status", cwd,
   pid, terminal: true, wait_secs: 600}` collapses the whole child run into
   one blocking call that wakes only on the terminal facts (goal or abort
   verdict, liveness alive→dead, events-file creation) or the deadline —
   never on iteration advances or budget-low flags (the iteration
   economics: one orchestrator iteration per child run, not per child
   iteration — wake-on-advance cost ~40 orchestrator iterations per
   40-iteration child even with `wait_secs: 600`). The significant wake
   (`wait_secs` without `terminal`) and instant polls remain for active
   monitoring — e.g. watching a known-flaky child's per-iteration progress.
   If status
   reports liveness unknown (pid omitted or lost), fall back to
   `ps -p <pid>`. Exit of the pid = child done; then review.
   **Budget-death recovery (T63):** when the child exited on a BUDGET
   abort (iteration / token / minutes ceiling — the events summary's
   abort flag names the budget, e.g. `abort_reason: iteration budget
   exceeded`) with the goal not accepted and the worktree holding
   incomplete work, the FIRST recovery is ONE `delegate` relaunch in the
   SAME worktree with `resume: true` — same spec, same goal (the goal
   re-carries the T47 `CARGO_TARGET_DIR` export), same model, same
   budgets (80/50 impl, 60/50 validate) — which continues the child's
   prior transcript in that worktree instead of starting cold. The
   routing discriminator is the work's state at death: work INCOMPLETE
   (uncommitted or partial) takes the resume relaunch above; work
   complete and committed with the goal still unaccepted goes to
   orchestrator-finish directly — review the branch, run the gates,
   merge if green, NO resume burned (T150-impl precedent, cycle 71
   routing d1790691515-11, alongside T55); resume-exhausted or
   unrecoverable goes to next-cycle recovery with a recipe written on
   the row (T28 precedent). Resume
   works because the worktree is never removed pre-harvest (T19), so the
   child's untracked `.chug/` transcript persists, and `delegate status`
   reads the LATEST run segment (T58), so the pre-resume abort no longer
   latches the summary. Cap ONE resume attempt per child — a resumed
   child that dies at budget again without goal acceptance falls back to
   the standing recipes (orchestrator-finish for complete-but-uncommitted
   work, T55 precedent; next-cycle recovery with a recipe written on the
   row, T28 precedent). Fix-up children (step 4's FAIL arc) are
   unaffected — they start fresh by design. If
   `delegate` itself errors persistently (the tool, not the child),
   META-SPEC §4's hand-rolled nohup launch template remains the fallback
   launch path — note the fallback in your ledger. A child budget-death
   recovery routing (resume / orchestrator-finish / next-cycle) AND any
   glm→kimi model fallback are each logged via `decision_log` (classes
   `recovery-routing` / `model-fallback`).
   **Kill rule (cycle-61): verify-then-kill is SEQUENTIAL — read first,
   kill after.** Before killing any child over a suspected payload garble
   (a delegate goal echo, a log tail, a status render that LOOKS
   corrupt), read the payload back from the child's on-disk artifacts
   (transcript / events / files via bash) in a SEPARATE completed step,
   and kill only when the read-back proves the payload itself corrupt.
   **Never kill in the same breath** as the check: render-only garbles
   (terminal/preview artifacts) are the common case and kill nothing.
   The evidence is the cycle-61 T112-validator SIGKILL — a
   duplicated-tail rendering artifact in the orchestrator's own console
   view read as a corrupt launch goal, the kill landed 35s in, and
   transcript read-back only AFTER the kill proved the 2166-byte payload
   intact (a full validator spin-up burned on the relaunch, ~11 min
   wall).
3. **Review.** After the child exits, the review's first look is one
   `delegate{action:"collect", cwd, pid}` call — it returns the latest run
   segment's verdict, the accepted goal's summary, the check cmd, and the
   commit refs in one bounded non-blocking read (every git failure degrades
   to a note) — then diff the branch, read the child's ledger if ambiguous
   (read worktree files via `bash` — the file tools are cwd-confined and
   refuse `/tmp/chug-loop-*` paths with `path escapes cwd`), and run
   bounded gates yourself (T82 gate runner, nextest-first — the
   predicate is `command -v cargo-nextest`: when it succeeds run
   `touch src/*.rs tests/*.rs; CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared perl -e 'alarm 280; exec @ARGV' cargo nextest run --release`,
   else the same touch guard + bounded cap around the fallback
   `touch src/*.rs tests/*.rs; CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared perl -e 'alarm 280; exec @ARGV' cargo test --release -- --test-threads=4` —
   the T47 env prefix keeps the gate on the shared warm cache; bash tool
   calls don't share env, so the step-1 export doesn't persist between
   calls; the T195 `touch` prefix rebinds the shared dir's artifacts to
   THIS checkout before the gate runs them — a foreign checkout's artifacts
   in the dir are mtime-fresh against this checkout's older sources).
   The `perl -e 'alarm N'` inner bound stays as the per-command
   wedge guard (T178): with loopd exporting `CHUG_BASH_TIMEOUT=300` for the
   whole fleet, the alarm value must sit BELOW the bash cap so the inner
   bound fires first — the cargo-culted 600s alarm could never fire under
   the old 120s default (EVALUATION.md §2.4), so the templates carry
   `alarm 280`. The runner rule (T82, operator-approved 2026-09-26): gates
   prefer `cargo nextest run --release` — cargo-nextest is a HOST tool
   (installed via `cargo install cargo-nextest` or the get.nexte.st
   tarball), NOT a crate dependency — and the fallback to
   `cargo test --release -- --test-threads=4` is unconditional: nextest is
   never a hard dependency, and every template degrades gracefully when it
   is absent. First cycle after the switch: run BOTH runners once in main
   and record both wall times in that cycle's Outcomes (the measurement
   that justifies keeping nextest). If a test family is red ONLY under
   nextest, that family's gates use the fallback and the commit message
   names the family. The docs-only guard floor below is exempt — it runs
   one targeted test binary, where nextest's suite-wide scheduling buys
   nothing. The gates are the RELEASE profile (T78): the
   first release build into a cold cache is slower (compile time), every
   subsequent run is faster than debug — the shared target dirs (T47/T52)
   amortize it. Role-key the dir (T52): the `target-shared` shown
   above when NO child is in flight, but
   `CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-gates`
   whenever an impl child may be concurrently building — the T44 overlap
   window, including N+1's impl during N's post-merge gates. Why a separate
   dir: cargo's target metadata hash excludes the checkout path, so the same
   package+profile+features produce the SAME artifact filename in every
   worktree — one shared dir is last-builder-wins, and a gate run can then
   execute a binary compiled from a different checkout's source (observed
   cycle 22: the t49 gates executed a validator's leftover mutant and ran it
   as their own). The gates dir persists across cycles like the others (warm
   after a first cold build). Never trust a claim of green without seeing it.
   **Docs-only rounds skip the cargo gates (T80).** When the round diff
   touches ONLY `*.md` — file-extension-exact, not "mostly docs" — gates
   shrink to the guard floor `cargo test --test todo_consistency` (still
   under the bounded-cap rule, same T195 touch guard + env prefix as the
   full gate), and the
   full build/clippy/test is skipped at review AND post-merge (step 5
   applies the same
   classification). The classification is mechanical and stated as a
   template — `git diff --name-only main...<branch> | grep -qvE '\.md$'`:
   exit 0 (some changed file does not end `.md`) → full gates; exit 1
   (every changed file ends `.md`) → the reduced set.
   `tests/todo_consistency.rs` covers the actual docs-only risk surface
   (table format, doctrine tokens): an md-only edit to unpinned docs text
   — a README clause, a doctrine sentence — cannot go red in the full
   suite, so for those diffs the full runs are pure latency. But an
   md-only edit touching a pinned doctrine carrier — a paragraph pinned
   by `tests/loop_spec_*.rs`, `tests/shared_target_dir.rs`, or
   `tests/nextest_gate_runner.rs` — CAN go red: it can break a pin the
   floor does not run, so the editor (orchestrator or child) ALSO
   runs the affected pin file's tests (seconds) before shrinking the
   gates; any doubt is classification ambiguity and takes the
   ambiguity default below. Any ambiguity in the classification — a
   predicate that cannot be evaluated, a diff shape it does not fit —
   defaults to full gates.
   Orchestrator mutation legs for RED-proofs MUST be applied inside the
   worktree via bash (e.g. `perl -i` with cwd `/tmp/chug-loop-t<N>`) —
   `edit_file`/`write_file` are confined to the main tree and silently
   produce false survivors when the gates under proof run the worktree
   copy; after any main-tree edit during a round, verify main is
   restored byte-identical before merging.
4. **Adversarial validation (kimi, REQUIRED** for any item touching
   src/driver.rs, src/api.rs, src/tools.rs, src/events.rs, or the loop/spec
   doctrine itself; optional for docs/tests-only items): META-SPEC §6
   verbatim — VERDICT: PASS/FAIL + numbered findings, mutation-testing where
   feasible. The low-stakes lane (T189) is decided per item BEFORE dispatch
   and is MECHANICAL — gates-only when ALL hold: (a) the diff touches NO
   core-list file (the list this step opens with — src/driver.rs,
   src/api.rs, src/tools.rs, src/events.rs, or the loop/spec doctrine
   itself; any LOOP-SPEC/META-SPEC/META-META-SPEC/SELF-SPEC edit stays FULL
   validation); (b) ≤ ~150 changed lines (added+deleted, `git diff
   --numstat`); (c) no new tool/command surface (a schema enum, a CLI
   flag, an MCP tool, a hook event); (d) no CI/workflow or spec `check:`
   line change (a `.github/` path, or any diff line whose payload starts
   `check:`). Any single one flipped → full adversarial validation. The
   four inputs are computed from the diff (`git diff --numstat` + `git
   diff`), never the model's say-so, and the routing record (class
   `validation-routing`) names all four inputs plus the verdict. The lane
   changes review depth, never the gates or the quality floor: a
   gates-only item still runs build + clippy + the full suite in its
   worktree, byte-clean review, and the scope check — the lane skips the
   kimi child, never the gates — and a lane-eligible diff the gates catch
   red still gets a fix-up child (the FAIL arc below). Auto-spec'd runs
   (T188) default to the lane predicate; `--validate` forces full
   adversarial and `--no-validate` forces gates-only — either operator
   override is recorded. A validator on a docs-only round keeps its
   judgment: it may
   shrink its own gate run to step 3's guard floor, and it retains the
   right to run the full suite anyway when the doc diff quotes commands or
   check lines (the T67 class — a spec `check:` line IS executable text).
   The validation child launches exactly like step 2 — a `delegate`
   launch with model `anthropic-system.ai.kimi-k3` — ALWAYS kimi whatever
   model orchestrates (T81 family independence: glm implements, so a
   glm-orchestrated cycle never lets glm validate glm — a routine cycle
   still launches this child on kimi) — with
   `max_iters: 60`, `max_minutes: 50` (§6's budgets, passed explicitly —
   minutes is 50, still ABOVE delegate's 35 default. 60/50, not 50/30:
   4 of the last 4 validator children died at budget in cycles 66–69 —
   T134/T138/T142 at 50/50 with work in flight, T137 minutes-bound at
   30m08s with its verdict already written but unannounced — and the
   cycles-76–78 census found 3 of 5 validators finishing within ~1–4.5
   min of the 40-minute wall (t167-val 39m16s, t168-val 38m22s, t162-val
   35m46s of 40), one slow mutant from the same unannounced-verdict
   death — the raise's reason. Measure: if
   >1 of the next 8 validator runs still dies at 60/50 with the verdict
   unannounced, the next eval considers trimming default mutation-leg
   counts instead of further raises), and §6's goal text
   verbatim except its export line, which becomes
   `export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-validate-a`
   before every cargo command — ALWAYS, never conditionally (T52 role-keyed
   target dirs, slot-keyed per T194: the validator's
   mutate→test→revert→re-test cycle builds into its own persistent slot
   cache — `target-shared-validate-a` is validator slot a, the solo
   default — so its mutant binaries
   can never occupy an artifact slot another checkout's gates or impl builds
   read — same artifact-name mechanism as step 3). When a SECOND validator
   is in flight (the Pipeline overlap rule's pattern iv — two items
   simultaneously past gates, never two validators on the same item), it
   is slot b and its export becomes
   `export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-validate-b`
   — two validators sharing one dir would serialize on cargo's build lock
   and erase the parallelism the slot exists to create, and the T47
   invariant holds: no two cargo processes share a target dir. T183: the
   launch ALSO passes the slot's dir via `env`
   (`env: {"CARGO_TARGET_DIR": "/Users/jadams/workspace/chug/target-shared-validate-a"}`,
   or `env: {"CARGO_TARGET_DIR": "/Users/jadams/workspace/chug/target-shared-validate-b"}`
   for the slot-b validator)
   — the goal's export line STAYS, the step-2 defense-in-depth rule. The
   slot dirs persist across cycles — warm after first use; the first use
   is a cold build, the accepted one-time cost per role. The validator's
   own gate runs carry the T195 touch guard the same way —
   `touch src/*.rs tests/*.rs;` immediately prefixes its cargo gate
   commands (§6's "run the T82 gate runner yourself" legs) — because its
   slot dir persists across cycles and can hold a previous validator
   checkout's artifacts mtime-fresh (the same T55 mechanism step 3's guard
   closes). Wrap gates keep
   `target-shared-gates` (step 3's overlap-window rule, unchanged) and
   post-merge gates keep `target-shared-main` (step 5) — the slot split
   is validator-only. This
   paragraph is a LOOP-SPEC override of §6's launch
   mechanics only, and META-SPEC.md is not edited.
   Parallel mutants (T79, operator-approved 2026-09-26): after the gates
   pass on the clean tree, the validator MAY run its mutation legs in
   parallel — one throwaway worktree per mutant
   (`/tmp/chug-mut-<item>-<k>`), each with its own role-keyed target dir
   `CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-mut-<k>`
   (the T52 lesson per leg: a mutant's binaries must never share a target
   dir with another checkout's builds), each running its targeted test,
   results collected by the validator; cap legs in flight at 3 (the host
   has other work; validators are already capped — at most 2 of the
   max-3 children, T194). Serial
   stays the default when mutants touch overlapping files — the validator
   declares that overlap judgment in its verdict notes. Tree-restored
   semantics unchanged: main worktree byte-clean before the verdict;
   throwaway worktrees removed after results are collected; findings
   reference mutant names, not leg dirs. META-SPEC §6's goal template
   carries the same T79 mandate (that edit, not this paragraph's launch
   override, is what touches META-SPEC.md). On a validator budget death
   or wedge, read `.chug/verdict.md` in the worktree BEFORE spending a
   T63 resume — a written verdict IS the verdict (provenance note in the
   ledger; the resume is then only for missing mutation legs, named in
   the routing record). FAIL → fix-up child
   with the findings pasted into its goal — and when a finding names one
   instance of a class (a vacuous pin, a missing reset, an unchecked error
   leg), the goal ALSO names the class and requires
   EVERY instance swept with its own RED-proven killing test — the
   cycle-33 lesson (T69's run_start latch resets took three rounds one leg
   at a time; the sweep-the-family goal closed it in one) — then re-validate.
   The per-item
   validation routing call (REQUIRED / optional / skipped + why) and the
   validator verdict received (PASS/FAIL + findings count + survivor
   count) are each logged via `decision_log` (classes `validation-routing`
   / `validation-verdict`).
5. **Harvest, then merge + close — you own the books.** Before any
   `git worktree remove` (which deletes the worktree's untracked `.chug/`
   silently — the cycle-5 T18 loss), harvest ALL of the worktree's
   `.chug/events*.jsonl` files — the live stream AND every rotated
   `events-<ts>.jsonl` segment, ONE harvested file per source file: a
   FRESH child launched into a reused worktree ROTATES its predecessor's
   `.chug/events.jsonl` to `.chug/events-<ts>.jsonl` (T10/T7), so
   copying only the live stream silently drops the rotated segment(s) —
   the cycle-84 evidence: the t183 impl child's two glm segments and the
   t181 impl child's glm segment were never harvested and died with
   their removed worktrees, t181's even filed under one combined name
   while the harvested file held a single kimi segment (`runs: 1`). Name
   each harvested file per the run segment(s) it ACTUALLY contains
   (inspect its `run_start` model/spec: `events-t<N>-impl-<ts>.jsonl`
   for the impl segment, `events-t<N>-validate-<ts>.jsonl` for the
   validator; NEVER a combined `impl-validate` name for a
   single-segment file; impl and validator alike; precedents
   `events-t17-impl-20260925-170831.jsonl` and
   `events-t17-validate3-20260925.jsonl` — cycle-83's kimi harvest of
   t180 did it right, `events-t180-impl-…` (`runs: 2`) AND
   `events-t180-validate-…` as separate files), plus the child's
   `LEDGER.md` as `LEDGER-t<N>-<role>-<ts>.md` when it carried a verdict
   or non-trivial findings; transcript harvest is the operator's choice
   (size). The remove carries a SECOND precondition, exact in BOTH
   directions: verify NO child pid launched in that worktree is still
   alive — liveness comes from `delegate status` or `kill -0 <pid>`,
   NEVER a truncated `ps … | head` read (cycle-84 seg-1 declared the
   t183 validator dead on a bare ps pipeline read, removed its worktree
   while pid 6260 lived, and the zombie's cargo suites raced the wrap
   goal-gate into a ~30-minute "check command failed" diagnosis) — and a
   bare `ps -p <pid>` / `kill -0` hit must ALSO exclude the
   defunct-zombie state (`ps -p <pid> -o stat=` showing `Z`, which
   satisfies kill -0 while holding no files — the cycle-85 inverse: two
   defunct validator pids read ALIVE on a bare ps -p moments before a
   safe removal; the safe direction, but the check must be exact both
   ways). A live child blocks the removal: harvest what exists, leave
   the worktree, note the row for the next cycle. Only then merge to
   main, re-run gates in main —
   `CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-main`,
   ALWAYS, never conditionally on the T44 overlap (this is NOT step 3's
   role-keyed rule; step 3's worktree-review gates keep their T52
   `target-shared`/`target-shared-gates` split, scoped to worktree-review
   gates). The gate command is step 3's T82 runner — `cargo nextest run --release` when
   `cargo nextest` is on PATH, else
   `cargo test --release -- --test-threads=4` under the bounded cap (step
   3's templates: `alarm 280` below the `CHUG_BASH_TIMEOUT=300` bash cap) —
   same tradeoff as step 3, and the dir is warm after its first release build.
   Docs-only rounds (step 3's classification — every changed file ends
   `.md`) shrink the post-merge gate the same way: the guard floor
   replaces the full suite here too, and the main-dedicated-dir rule
   above still governs every cargo run that does happen.
   Why a main-dedicated dir (T57):
   cargo's artifact filename excludes the checkout path, so a shared dir's
   artifact slots are last-builder-wins — and only a dir whose builders are
   ALWAYS main checkouts keeps post-merge artifacts identical to main
   content (the T55 false red: the step-3 worktree gates had compiled the
   worktree's PRE-T53 `tests/loopd_reexec.rs` into `target-shared`; the
   merge didn't touch that file, so its main-checkout mtime stayed OLDER
   than the artifact and cargo ran the stale 4-test binary as fresh — 3/4
   false red, touch + rebuild recovered; the symmetric false-GREEN leg — a
   stale passing binary masking a real main failure — is silent). The dir
   persists across cycles like the others (warm after a first cold
   build). Then flip
   the TODO row to `done` **with the merge commit ref in the same commit**
   (or an immediately following `todo:` commit; bundled rows may share one
   such `todo:` commit naming every row + its ref — Trivial-row bundling).
   The row-flip commit appends an `outcome` record via `decision_log` per
   decision id logged for the item — `landed-clean`, or `fixed-up` when a
   fix-up arc ran; a later revert appends `reverted`.
   When editing TODO.md (row flips, notes annotations), the notes cell
   must contain no `|` — the T8 guard splits every row on it, so a stray
   pipe splits one cell into two and fails the guard (cycle-16's
   goal-gate death: a recipe pipe in the T37 notes cell rejected
   `goal_complete` at ~118/120 and the run aborted 120/120) — and the
   orchestrator runs `cargo test --test todo_consistency` (seconds)
   after every TODO.md edit, before committing.
   When editing repo files with `sed` or other in-place bash edits
   (bookkeeping, harvest, gates scripting), grep-verify the intended
   needle in the same command line or the immediately following one —
   sed exits 0 on no-match, so a typo'd anchor is a SILENT no-op (the
   cycle-61 anchor-typo incident: a `sed -i` bookkeeping edit exited 0
   changing nothing; only a later read-back caught it). Prefer
   `edit_file` (which errors on no-match) for repo files, and when sed
   is necessary, assert.
   Update README.md in the
   merge commit when the item is user-visible. **Outcomes are per-item
   too**: in the same commit as the row flip (or an immediately following
   `eval:` commit), append the item's Outcomes entry to EVALUATION.md —
   what landed, what the validators caught — so a mid-cycle death loses
   no narrative. The cycle-12 bite: it deferred the narrative to wrap,
   died mid-arc at budget, and cycle-13's wrap had to reconstruct the
   entry from the git record ("RECONSTRUCTED at cycle-13 wrap"; eval
   commit `697a6b6` records the lesson: "deferred-wrap loss lesson:
   write Outcomes per-item"). This ordering is the fix
   for the T10/T12 failure mode: children die between the code commit and
   the row flip, so children never own the row. **Push after each item
   lands green** (`git push` once the todo: commit is in) — the operator
   watches origin; don't hold a batch hostage to the wrap.
6. **Budget check.** Fewer than 30 iterations left → stop dispatching, go to
   wrap. Unworked rows stay `todo` — that is a fine outcome. The 30 is
   sized to the wrap tail's measured ~15–25 iteration cost — final gates,
   row flips, Outcomes, harvest, release check, push, goal gate — at the
   200-iteration / 360-minute orchestrator budget loopd now launches; the
   old 15 was calibrated when orchestrator budgets were 120 iterations, and
   cycle-94 seg-3 died 200/200 with the merge committed but the wrap
   unwritten after dispatching a validation round inside the margin.

**Trivial-row bundling (T45) — when one child may take up to 3 rows.** The
per-row arc above carries a fixed cost — a worktree, a fresh build, a
dispatch and a validation cycle ≈ 15–30 min even when the change is a
one-line const pin or a doc sentence (the T38–T43 class). The orchestrator
MAY therefore dispatch ONE impl child for up to 3 rows in a single round
(one worktree, one launch, one review pass) when ALL of the following
hold — the eligibility predicate is CONJUNCTIVE, every condition must
hold, no weighing; when any one is in doubt, run the rows separately:
- (a) each row's spec estimates ≤ ~30 changed lines (doc sentences, const
  pins, description text);
- (b) all rows touch the same 1–2 files or are docs/doctrine-only;
- (c) none touches src/driver.rs or src/api.rs core loop logic;
- (d) every row's pri ≤ 3.
Never bundled: features, rows across different file areas, bundles >3
rows. The child's goal is the step-2 template with the row list made
explicit — "Implement TODO items t<a>, t<b>, t<c> ONLY", followed by each
row's spec path — and it requires ONE commit PER ROW, in queue order
(never one squashed commit), so each row's flip references its own
commit. Validation: ONE kimi round covers the whole bundle,
mutation-testing per row where feasible; if ANY bundled row is
core-adjacent (step 4's REQUIRED list — src/tools.rs, src/events.rs, or
the loop/spec doctrine), that REQUIRED validation covers the set, while a
bundle of docs/pins-only rows may skip the kimi round and rely on the
orchestrator gates of step 4. At merge time the bundled rows may flip in
ONE `todo:` commit naming every row + its ref (step 5). For the Pipeline
overlap rule below, a bundle counts as ONE impl child under the same cap
(at most 3 children, of which at most 2 validators) — and a bundle
containing a doctrine row still never overlaps (it runs alone).

**Pipeline overlap (T44, T161) — when a second and third child may fly
(T194).** Steps 1–6
remain the per-row arc; adjacent rows may overlap in four patterns — (i)
and (ii) are both gated by the SAME disjointness check: read both specs,
list the files
each names as its targets, and overlap only when
no file appears on both lists; (iii) by the three-way PAIRWISE gate; (iv)
by the two-past-gates rule. (i) T44's {1 impl + 1 validator} overlap:
after impl child N completes (its step-3 review passed) and its validator
(step 4) has launched, you MAY create item N+1's worktree (step 1) and
launch impl child N+1 (step 2). (ii) T161's 2-impl overlap: impl child
N+1 MAY launch while impl child N is STILL FLYING, iff the disjointness
gate passes. (iii) T194's 3rd impl slot: impl child N+2 MAY launch while
N AND N+1 are STILL FLYING, iff the PAIRWISE gate passes — all three
items' spec-named target-file lists are pairwise disjoint (no file
appears on any two of the three lists); a 3rd impl whose spec-named files
overlap ANY in-flight impl's files is blocked. (iv) T194's 2nd validator:
it launches ONLY when two items are simultaneously past gates — item N's
validator is in flight and item N+1 has passed its step-3 review — never
two validators on the SAME item (correlated verdicts add nothing; family
independence unchanged — validators are always kimi). The
safety basis: each impl writes only its own worktree and a validator
reads main read-only, so no two processes ever write the same file — the
merge into main is the one shared surface, and it stays serial (below).
Concurrent impls also never share a build slot (the T52 lesson one level
down — cargo's artifact filename excludes the checkout path, so one
shared dir is last-builder-wins): the impl launched INTO an overlap
exports `CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-impl-a`
before every cargo command — unless the impl already in flight holds
impl-a, in which case
`CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-impl-b` — and its
step-1 warm build goes to the SAME dir; the impl already flying keeps the
slot it launched with (the T47/T52 default `target-shared` when it
launched solo). A third impl child (pattern iii) takes
`CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-impl-c` when
impl-a AND impl-b are both held — always a slot no in-flight impl holds,
so the T47 invariant survives the wider fleet: no two cargo processes
share a target dir. Validators are slot-keyed the same way (T194): the
validator in flight holds
`CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-validate-a`
(step 4's solo default); the pattern-(iv) second validator takes
`CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-validate-b` —
two validators sharing one dir would serialize on cargo's build lock and
defeat the slot. At dispatch into the overlap the orchestrator ALSO
re-keys the spec's `check:` line export to the SAME role-keyed slot
before launch (sed + grep-verify per the sed-assertion rule, and commit
the re-keyed spec on the branch): the goal's export and the check's
export must always name one dir, because T144's scrub makes the check's
own export the only target dir the goal gate sees, and the T52
artifact-name class makes a foreign dir a correctness hazard, not only
contention — spec authors keep writing check lines against the default
`target-shared`, the re-key is a dispatch-time act, only when slotting an
impl child into impl-a/impl-b/impl-c or a validator into
validate-a/validate-b (the cycle-77 bite: t165's gate ran its
check against the default `target-shared` while T162's impl child
built into it and was rejected on green work, fixed mid-flight by
607e877). Doctrine items NEVER overlap: a spec touching
LOOP-SPEC.md, META-SPEC.md, META-META-SPEC.md, SELF-SPEC.md, TODO.md's
row format, or loopd.sh runs alone, with NO other child in flight (a
mid-flight doctrine change would govern work that was launched under the
old rules). Hard cap (T194 amendment): at most 3 children in flight — of
which at most 2 validators; 2 impls only under the disjointness gate; a
3rd impl child only under the PAIRWISE gate (pattern iii); a 2nd
validator only when two items are simultaneously past gates (pattern
iv); the resource governor (Hard rules) caps the cargo-heavy fleet at 4
and, under memory pressure, validators win. When the two queued items are NOT
disjoint, the orchestrator
falls back to pattern (i) — the {1 impl + 1 validator} overlap — T161
widens T44, never narrows it, and never forces overlap. Merges stay
STRICTLY serial in queue order: N merges (step 5) before N+1 even if N+1
finished first; a FAIL verdict on N pauses N+1's MERGE — never N+1's
impl, which may keep flying — until the fix-up arc resolves; N+1's branch
may need a rebase onto the updated main, and you own that. Harvest still
precedes removal for BOTH children (step 5, T19): no worktree — N's or
N+1's — is removed until every child run it hosted (impl and validator
alike) has been harvested.

## Phase 3 — Wrap

- **Wrap-state note at the boundary (T207, hard rule).** When the
  orchestrator crosses the stop-dispatch boundary (step 6), its
  NEXT ledger write must carry a wrap-state note naming: every
  merged-but-unflipped row, every unharvested worktree, every
  unpushed commit count, and every missing decision record — so a
  mid-wrap death is recoverable from the ledger alone with
  zero git reconstruction (the seg-3 death left a ledger ~150
  iterations stale and cost cycle-95 ~6 iterations of git archaeology
  before any productive work). Write it before the wrap duties below.
- TODO.md truthful (every `done` row has a commit ref).
- Child harvests landed in the main repo's `.chug/`: each worked item's
  `events-t<N>-<role>-*.jsonl` — ALL of the segments the worktree held
  (the live stream AND each rotated segment — §2 step 5's harvest-all),
  never the live `.chug/events.jsonl` alone (plus
  `LEDGER-t<N>-<role>-*.md` where non-trivial) — nothing died with a
  removed worktree.
- `.chug/decisions.jsonl` carries the cycle's records — `eval-triage` at
  eval time (filed rows AND rejected candidates), `recovery-routing` /
  `model-fallback` at dispatch, `validation-routing` +
  `validation-verdict` per item, `outcome` backfills at row flips;
  a cycle that worked items with zero `decision_log` records
  is an incomplete wrap (the T23→T24 zero-calls lesson; cycles 34+35
  shipped nine routing/verdict decisions with none recorded).
- **Release trigger (T100): the loop cuts tags.** If ≥3 items landed
  since the newest `v*` tag OR any FEATURES.md check-off landed: bump
  Cargo.toml AND Cargo.lock (minor for a feature, patch otherwise;
  `cargo check` regenerates the lock — a manifest-only bump fails
  `--locked` builds, the v0.2.0 lesson), `chore: release
  vX.Y.Z` commit, `git tag vX.Y.Z`, push commit + tag. HARD RULES: tags
  are immutable — never re-tag, never move, never force-push; tag only
  with gates green at HEAD; ONE tag per wrap; tag message = generated
  notes since the previous tag. A failed release workflow files a row —
  never delete a published tag.
- EVALUATION.md's **Outcomes** section is complete and truthful — per-item
  entries were written at each landing (§2 step 5); wrap adds the
  skipped/deferred rows, the cycle-level notes (what the validators
  caught, final state), and fills any gap a mid-arc death left.
  Outcomes keeps the last 6 cycles in full; older entries are compacted
  at wrap time to one line each (`### Cycle N (date) — items landed +
  refs, one-line verdict`) — the full narrative lives in git (row-flip
  commits + TODO done rows carry the refs), so compaction drops nothing
  that isn't one `git log` away.
- Final gates green in main (build + clippy + step 5's T82 gate runner —
  `cargo nextest run --release` when `cargo nextest` is on PATH, else
  `cargo test --release`) → push anything
  remaining (eval commits, Outcomes) → `goal_complete` with the cycle
  summary. Final gates run in main under the T57 main-dedicated cache:
  `CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-main`, the
  same ALWAYS rule as step 5's post-merge re-run. Never force-push; a
  rejected push means the remote moved — stop
  and note it, don't reconcile mid-cycle.
- **Tag at wrap (T100 — operator override 2026-09-28, reversing the earlier
  never-self-tag guardrail).** After final gates are green at HEAD, the
  orchestrator cuts ONE release tag per wrap max: when ≥3 items landed
  since the last `v*` tag OR any FEATURES.md check-off landed since it,
  bump Cargo.toml's version (minor for a feature item, patch otherwise),
  commit `chore: release vX.Y.Z`, verify the pairing
  (`scripts/check-tag-version.sh vX.Y.Z` — req 3's divergence check), write
  the generated notes to a temp file (`scripts/release-notes.sh` — grouped
  feat/fix/chore/docs since the previous tag), tag `git tag -a vX.Y.Z -F
  <notes-file>`, and push the commit + tag together with the wrap push
  (the tag-triggered release workflow in `.github/workflows/release.yml`
  picks it up). HARD RULES: never re-tag or move a tag; never force-push
  tags; tag ONLY with gates green at HEAD (the wrap's final gates count);
  one tag per wrap max; tag message = the generated notes since the
  previous tag. If the release workflow later fails on a pushed tag, file
  a TODO row naming the tag — a published tag is immutable history, never
  deleted or moved. BOOTSTRAP: the FIRST tag is operator-cut — until a
  `v*` tag exists, the loop never tags (the trigger counts "since the
  last tag" against a tag that does not exist yet, so the loop must not
  invent one); once the operator's first tag lands, this doctrine is
  active from the next wrap on.
- **Your wrap is the next cycle's input.** `loopd.sh` relaunches this spec
  back-to-back with no human in the loop — TODO.md, EVALUATION.md and
  specs/ are the handoff. Leave them such that a cold next cycle needs zero
  human words: every `todo` row has a ready spec, every deferred item says
  why, every open question is written down.

## Hard rules

- ONE WRITER per file set, serial merges, bounded gates (T44, T161; the
  3-child fleet per T194):
  **at most 3 children, ≤2 validators, disjoint-gated** — at most 3
  children in flight, of which at most 2 validators; two impl children
  may fly together iff the two items' spec-named target files are disjoint
  — both specs read, file lists compared; a third impl child may fly only
  when all three items' spec-named target-file lists are PAIRWISE disjoint
  (no file appears on any two of the three lists); a second validator may
  fly only when two items are simultaneously past gates — never two
  validators on the same item (a Trivial-row bundle counts as ONE impl
  child). An impl may overlap a
  validator only when the two items' spec-named target files are disjoint;
  concurrent impls never share a build slot (target-shared-impl-a /
  target-shared-impl-b / target-shared-impl-c, the T52 lesson); validators
  never share one either (target-shared-validate-a /
  target-shared-validate-b, keyed by validator slot — two validators on
  one dir would serialize on cargo's build lock); doctrine items (LOOP/META/
  META-META/SELF-SPEC, TODO.md row format, loopd.sh) never overlap with
  anything; merges stay strictly serial in queue order — a FAIL on N's
  validator pauses N+1's MERGE (never its impl) until the fix-up arc
  resolves, and the orchestrator owns any rebase; when the queued items
  are NOT disjoint the orchestrator falls back to the {1 impl + 1
  validator} overlap — T161 widens T44, never narrows, and never forces
  overlap. Resource governor (T194): at most 4 cargo-heavy children total
  (impls + validators) — when memory pressure forces a choice between
  dispatching an impl child and dispatching a validator, validators win
  (a validator verdict unblocks a merge and the queue behind it) and the
  orchestrator records the degradation via decision_log. This overrides
  META-SPEC's "ONE child at a time" for launch
  concurrency ONLY; all of META-SPEC's other hard rules apply (never
  reset/remove worktrees with unmerged work, never force-push, wedge
  protocol); `delegate` (launch + status) is each child's
  launch/observation surface (step 2).
- **Anti-sprint-burn guard (T81, model-agnostic — the M2/M3 muse
  sprint-burn lesson made structural).** An orchestrator that has spent
  MORE than 5 consecutive iterations without a child launch (`delegate`
  launch — impl, validator, or resume — or a bash-spawned helper) MUST act
  this iteration: launch the next dispatchable child, merge a reviewed
  one, or wrap. Reading, planning, re-reading the queue, and instant
  (non-blocking) status polls are not acting — collapse idle waits with
  `wait_secs` (step 2) instead of spending iterations on them. The cap
  binds kimi and glm alike; it never authorizes breaking the ONE-WRITER
  caps above — when the caps forbid a launch and nothing is mergeable, the
  honest act is wrap.
- Single-driver invariant: another **`chug run`** (autonomous driver) with
  `/Users/jadams/workspace/chug` as its cwd blocks the cycle. When tripped:
  do NO mutating work, verify the untouched tree's gates once, then
  `goal_complete` immediately with a BLOCKED report — do not watch-and-wait
  (a paced 2h watch burned 712k tokens in cycle 3; the fast exit is the
  doctrine). An interactive **`chug chat`** does NOT block: note it and
  proceed (shared `.chug/` appends interleave harmlessly; the operator is
  trusted not to run chat turns in the repo mid-cycle).
- Children never edit TODO.md or LEDGER.md in the main tree.
- README gate before `goal_complete`: it must document everything the cycle
  landed — INTEGRATED into the existing structure, not a bullet appended to
  the nearest section. If the structure fights the addition, that's a docs
  finding for the next evaluation (META-META-SPEC §6), not a reason to
  force-fit it.
