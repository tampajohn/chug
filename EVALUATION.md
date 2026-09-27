# EVALUATION — chug, assessed by chug-loop (2026-09-27, cycle 53)

**MANDATORY fresh eval** — the queue is EMPTY again (T1–T90 all done with
refs; cycle 52's flip landed the last row), so the freshness predicate's
queue half fails and loopd routed an eval cycle (`todo_rows=0 → eval cycle
on kimi-k3`, loopd.log 23:34:40Z). The roadmap pull is **F5 (Image input)
→ T91, SPLIT**: phase 1 (`read_file` image support) filed now; phase 2
(chat paste/drag) deferred with a written reason (§4). This is a DELTA eval
over cycle 51's same-day eval: the new corpus is exactly the T90 arc plus
the cycle-52 orchestrator stream, so findings concentrate on the T90
landing, the watch items the cycle-51 eval owed a judgment, and the
impl-child ceiling census that T90's run1 death completed.

Corpus: `.chug/eval-digest.md` FIRST (FRESH at eval start — regenerated
23:34:28Z, 69 s after the newest pre-cycle stream; the only
post-generation events are THIS eval's own appends, so the mechanical
staleness trip is self-inflicted and no regeneration is warranted), then
cycle 51's eval stream (`events-20260927-230725.jsonl`, kimi 160/160 DIED
— §2 I4), cycle 52's glm stream (rotated to `...-233440.jsonl`, 62/160,
goal accepted), the t90 child streams (impl run1 + T63 resume #19,
validate), `.chug/loopd/loopd.log` (routing lines 22:17Z–23:34Z),
`TODO.md` (T1–T90 all done), `.chug/decisions.jsonl` (155 records — was
131 at the cycle-47 eval), `src/` (34,001 lines; driver.rs 5,309 post-T90;
permissions.rs 730 new; delegate.rs 3,992), `README.md` (full cold read,
§6 — TWO findings: §6(b) density judged, §6(c) staleness bite).

## 1. What chug does well

- **The T63 resume machinery is the era's workhorse — 19/19 all-time.**
  T90's impl run1 died 50/50 uncommitted (+511/−2 staged); resume #19
  accepted 10/50 in ~2 min and the arc closed clean. Every glm ceiling
  death since T63 has been recovered in-worktree, zero losses.
- **Family-independent validation keeps paying.** t90-validate (kimi,
  38/50) PASS 0 blocking with 6/6 mutants RED in 2 parallel T79 waves
  under role-keyed mut dirs — and its 4 non-blocking findings were written
  well enough that TWO became this eval's rows (T93 canary, T96
  check-filter breadth) with zero re-investigation.
- **Cycle 52 was the leanest item arc yet measured:** 62/160 iterations
  for the cycle-51 wrap remainder + a full T90 arc (worktree, impl,
  resume, validation, merge, flip, push, wrap) — the first cycle on a
  post-T89 binary (terminal waits), child-watching collapsed as predicted
  (§2 I4).
- **decision_log adoption is structural.** 155 records; cycle 52 shipped
  7 with zero omissions, and T88's corrective errors caught a real
  malformed call in-cycle (`options must be a string, got missing` — one
  diagnostic, fixed next call).
- **The T90 policy chain landed in the designed order.** permissions →
  hooks → plan/MCP/risk gate on one `blocked` flag, zero hook fires on a
  deny, plan-mode parity, 24 tests incl. ZERO-hook-fire ordering pins —
  the F3/F4 composition worked exactly as the cycle-51 spec wrote it.

## 2. Incidents worth fixing

### I1 — glm impl children keep dying at the 50/50 ceiling with the work done → T92

Census of the last six glm impl children (digest-verified): T83 run1 50/50
abort (→ 2 resumes, 136 iters total), T84 48/50 (one from death),
T85-bundle run1 50/50 abort (resume #17), T88 33/50 clean, T89 run1 50/50
abort (resume #18), T90 run1 50/50 abort (resume #19). **4 of 6 hit the
ceiling, 5 of 6 were in the ceiling zone** — the exact "3 of the last 5"
shape T21 was filed on at the 40 cap. Completed-work totals: 57 (t89), 60
(t90), 70 (t85), 136 (t83 grinder). Resume recovers every death (~2 min +
an orchestrator relaunch arc), but the death is now the MEDIAN outcome for
a feature-sized row, and each one spends an orchestrator recovery arc and
leaves an uncommitted-worktree window. Fix: T21-class one-step widening of
the LOOP-SPEC §2 step-2 impl-child template `max_iters: 50 → 65` (+30%,
the T21 proportional step), the T63 resume-budgets sentence and fix-up
children with it, validator budget UNCHANGED (validators finish 12–48/50).
Measure-clause (written into the row): if >1 of the next 6 impl children
still dies at 65/65, the next eval considers 80 or a work-splitting
doctrine instead. Pin surface: `tests/loop_spec_recovery.rs:141` pins
`"50/35 impl, 50/30 validate"`; LOOP-SPEC lines 99/102–105/135 carry the
template digits + rationale parenthetical; the step-4 validator template
(50/30) is untouched.

### I2 — t90 validator finding (1): the `mcp__` matcher-fit canary over-rejects server-specific globs → T93

`src/permissions.rs:352–354`: matcher-fit validation uses ONE canary
candidate `"mcp__server__tool"`; a rule `{"tool": "mcp__fs__*", "path":
…}` fails the canary (the glob's literal `mcp__fs__` prefix never matches
the canary string) and is SKIPPED as matcher-that-cannot-fit — the
operator's deny never enforces (warned-once, fail-open by design, so
severity is bounded — but it is a policy-intent hole in a just-landed
security surface: the glob is valid, the canary is wrong). Fix: any
`mcp__`-prefixed tool glob fits any matcher (MCP tools carry arbitrary
args — the canary's own comment says exactly that), with a RED-proven
killing test. permissions.rs only → kimi optional per §2 step 4.

### I3 — t88-impl's false self-diagnosis: the `old_string` alias class → T94

The glm impl hit `missing or non-string field: old` ×3, self-reported a
tool BUG in its own decision record, and routed around via bash heredoc —
transcript forensics proved it sent `old_string` (the Anthropic-canonical
alias): the model fumbled and the error text gave it nothing to
self-correct with. The cycle-51 eval named the generalized fix a next-eval
candidate; this eval files it. `src/tools.rs:217` `get_str` — the ONE
extraction helper behind the tools' string params — names the received
keys on a miss (`missing or non-string field: old (received keys: path,
old_string, new_string)`), so the model corrects in ONE iteration instead
of three. Success path byte-identical; the T88 decisions.rs change is the
shape template (error-text-only failure legs, pre-existing pins
unmodified). tools.rs is on the REQUIRED list → kimi validation.

### I4 — cycle-51's eval died at 160/160 — assessed, NOT filed (T89 re-measure says wait)

The kimi eval stream: 160/160, 45m3s wall, 1.4M input tokens, abort
mid-T90-dispatch after landing T88+T89. Pre-T89 binaries paid ~10–15
orchestrator poll iterations per child arc (cycle-51 Outcomes measured
~15 across three arcs even with the paced cadence). Cycle 52 — the FIRST
post-T89 cycle — ran a wrap remainder + a full item arc in 62/160 with
terminal waits: child-watching collapsed to ~3–4 iterations per arc
(launch + terminal wait + collect). The 160 cap's T36 sizing (eval ≈45–55
+ item ≈28–35 + wrap ≈10) now has real slack. Verdict: the death was real
but its cause (pre-T89 polling overhead on an eval+2-item+dispatch cycle)
is already fixed by T89's adoption; no 160→200 row. Watch item: the next
TWO eval cycles' iteration counts close the question — re-file if either
scrapes ≥150 without a recovery arc.

### I5 — t90 validator finding (2): the spec `check:` filter was narrower than the change's own test surface → T96

`cargo test --bin chug permissions` missed the 5 strongest
driver-integration legs (a mutant survived under the plural filter, killed
under the broader `permission` stem). Third spec-authoring lesson in the
quality-bar's class (worktree-relative checks, T30; no `--lib`, T67-era),
and lessons in this class are per-author, not per-spec — the bar grows a
third sentence: a cargo-test `check:` filter MUST be broad enough to run
every test the change adds (prefer the module stem over a narrower
substring), verified by running the filter and confirming the new tests
are in the run set. META-META-SPEC edit (T30/T33 precedent: not a
human-spec file) → T96, kimi REQUIRED (doctrine).

## 3. Friction hot spots — fix assessment

- **path-escapes-cwd (T85)**: EARLY PASS, window stays open — the T89 and
  T90 validators both used bash cross-tree reads with zero
  `path escapes cwd` errors. Two more clean cycles close it.
- **edit_file / alias fumbles**: the t88-impl ×3 is the error-TEXT gap,
  filed as T94 (I3). T5's disambiguation shows no regression.
- **macOS `timeout` mirage (T22)**: zero sightings in the cycle-51/52
  streams (cycle 52's `head: illegal line count` is a flag-syntax fumble,
  not the timeout class).
- **cargo-nextest absent**: uninstalled ~22:16Z before cycle 51; every
  gate since runs the unconditional fallback (745/745 in ~21s vs nextest's
  ~9s). T82's doctrine absorbs it cleanly; host tooling is the operator's
  call, no row. Watch: if still absent at cycle 56, re-measure whether the
  runner-comparison debt closes by doctrine edit.
- **T80 md-only floor**: 5th cycle without a true md-only landing (the
  T85 bundle carried a `.rs` pin file → full gates, correctly). Telemetry
  watch stands.
- **decision_log malformed-call leg**: cycle 52's one `options must be a
  string, got missing` corrective did its job in one iteration. Closed.

## 4. Capability gaps — ROADMAP PULL: F5 (Image input) → T91, SPLIT

F1, F13-p1, F2-p1, F3-p1, F4-p1 all landed; the top unworked roadmap item
is **F5 (Image input)**. Split per the FEATURES.md working rules (full
scope blows one child budget): **phase 1 → T91: `read_file` image
support** — png/jpg/jpeg/gif/webp under the cwd sandbox return a base64
image content block plus a short text note, instead of today's
lossy-mojibake text read that silently poisons context; `ToolResult` gains
an image channel, api.rs gains `KnownBlock::Image`, the driver wraps
tool_result content as a block array when images ride along; a size guard
refuses oversized images with a tool error naming the cap; and the
load-bearing safety leg: **endpoint-rejection degrade** — if the API 400s
on image content, ONE retry with image blocks replaced by a text
placeholder + an events note, so a non-vision endpoint degrades instead of
poisoning every subsequent request in the run. Events/tool previews never
carry base64. Phase 2 (chat paste/drag of screenshots) DEFERRED with
written reason: terminal clipboard/inline-image input is TUI +
terminal-capability work with no loop consumer — the loop's need (children
reading screenshots and image fixtures) is fully served by phase 1 — and
it rides the F7 streaming-UX surface more than the tool surface.
FEATURES.md F5 carries the SPLIT annotation from this eval; the phase-1
check-off lands in T91's row-flip commit. REQUIRED kimi validation
(api.rs + driver.rs + tools.rs are ALL on the §2 step-4 list).

Beyond the pull: **no new roadmap appends this eval.** F6–F12 stand; the
F2/F3/F4 phase-2 deferrals stand (layad endpoint absent; risk-gate
plumbing; chat ask-mode UX) — reaffirmed, not re-litigated.

## 5. Top 3 priorities

1. **T92** (impl-child 50→65) — the median feature-row death is the
   loop's largest standing recovery tax; landing it FIRST also gives every
   later child this cycle the wider budget.
2. **T93** (mcp__ canary) — a policy-intent hole in a just-landed
   security surface; small, mechanical, RED-provable.
3. **T91** (F5 phase 1) — the mandatory roadmap pull; biggest row of the
   queue, sized for the post-T92 budget.

## 6. README audit (usability)

(a) **Reading order**: correct — what-it-is → quickstart → chat → run →
plan → TUI → tools → risk gate → hooks → permissions → MCP → Langfuse →
specs → loopd → development; T90's Permissions section landed INTEGRATED
after Hooks, not appended. (b) **Redundancy/density**: the `delegate`
paragraph in ## Tools is ONE ~24-line paragraph now carrying five
sub-behaviors (launch / status / collect / wait_secs+terminal / the
sandbox exception) — the 7-eval density watch is JUDGED: it crosses the
readability threshold → T97 (per-action sub-bullets, restructure-only,
zero behavior-text change, pin needles intact). No drift-duplication
found. (c) **Staleness**: ONE bite — the Development layout line lists 27
modules; `src/` has 28: **`permissions` is missing** (T90 added the module
and its README section but not the layout line — the T86 class, second
sighting) → T95 fixes the line AND adds the mechanical set-equality guard
the class has earned (two sightings = automate). (d) **Balance**: fine —
reference detail sits in reference sections; loopd carries the
runner/cache rationale at the right altitude. (e) **Quickstart truth**:
the commands work as written in the order given (T35's `cargo install
--path .` step holds; this host's operator runs `./target/release/chug`
directly per the ps record — a built-binary alternative the Development
section's commands also cover).

## Handoff

- **Work order** (bugs > robustness > features > DX > docs): **T92**
  (doctrine — runs ALONE, kimi REQUIRED) → **T93** (permissions.rs only;
  kimi OPTIONAL per §2 step 4 — not on the REQUIRED list; orchestrator
  gates + the child's RED proofs suffice, T16/T31 precedent) → **T91**
  (feature, kimi REQUIRED, budget one fix-up round) → **T94** (tools.rs
  error text, kimi REQUIRED; the T88 arc is the shape template) →
  **T95** (README word + new guard test — NOT md-only (a `.rs` lands), full
  gates, kimi optional) → **T96** (META-META-SPEC doctrine, kimi REQUIRED,
  T30/T33-shaped) → **T97** (README restructure, md-only floor, kimi
  skipped per T16/T31). T95/T96/T97 are bundle-eligible ONLY pairwise per
  T45's conjunctive predicate — T95's guard test likely breaks condition
  (a), so expect them separate.
- **SELF-SPEC**: none. **Human items**: none new — the untracked
  `com.tampajohn.chug-loopd.plist` stays the operator's call (carried).
- **Watch items carried**: T89 re-measure (I4 — closes after two more
  eval cycles); T85 cross-tree window (two more clean cycles); T80 md-only
  telemetry (6th cycle); nextest-absence re-measure (cycle 56); F13
  phases 2–3 + F2/F3/F4 phase-2 deferrals (layad endpoint absent, written
  reasons carried); delegate.rs 3,992 / tgrep.rs 2,527 / driver.rs 5,309
  module sizes (delegate.rs +494 since cycle 51 — the next delegate
  feature lands the split conversation).
- **Weighed and REJECTED this eval** (eval-triage records in
  `.chug/decisions.jsonl`): loopd cap 160→200 (I4 — T89 re-measure first);
  t90 finding (3) stderr warn-latch test-observability (the events line IS
  observable; stderr adds no decision-relevant signal); t90 finding (4)
  permissions.rs 363-vs-~350 size split (soft convention, at-edge is
  acceptable — split when it grows, not before); F13 phases 2–3 now
  (layad endpoint absent — standing deferral reaffirmed); delegate.rs /
  tgrep.rs module-size splits now (no incident; watch item covers it).

## Outcomes (filled at cycle wrap — LOOP-SPEC Phase 3)

### Cycle 52 (2026-09-27) — routine glm freshness-skip; T90 LANDED (e9afed9, fast-forward merge) — F4 permissions phase 1: the deny-only fail-closed policy layer

- **T90 LANDED** (e9afed9): `.chug/permissions.json` deny-list — new `src/permissions.rs` (363 prod lines, at the ~350 guard's edge) holds all policy logic: absent/empty config = zero rules + zero cost; malformed config fails OPEN (one stderr warn + one `permission_error` line, T83 parity); per-rule malformed legs (unknown key, two matchers, non-string value, missing tool, matcher-that-cannot-fit) are skipped in place with valid siblings still denying; deny rules are a tool glob + at most one `command`/`path`/`url` arg matcher, first-match-wins, missing/non-string arg under an arg rule fails toward execution; deny text `[permission denied] <rule summary>`. Driver gates dispatch FIRST (permissions → PreToolUse hooks → plan/MCP/risk gate) riding the T83 `blocked` flag in run+chat+plan, so a denied call fires no hooks and never executes. `hooks::glob_matches` made pub(crate), reused byte-identically. README Permissions section integrated after Hooks; FEATURES F4 row annotated at flip. 24 tests (17 unit + 7 driver integration incl. ZERO-hook-fires ordering pins + plan-mode leg + fail-open-exactly-once).
- **Arc shape**: glm impl run1 aborted 50/50 uncommitted (ceiling-zone, +511/−2 staged) → T63 resume #19 (d1790550858-1) ACCEPTED 10/50 in ~2 min — the resume continues the transcript and finished clean. kimi REQUIRED validation (driver.rs + events.rs on the §2 step-4 list; routing d1790551117-2) pid 83896 VERDICT PASS 0 blocking 38/50 (verdict d1790551730-3): gates independently re-run under target-shared-validate (build + clippy + 745/745 fallback-release + spec check 19/19), 6/6 mutants RED in 2 parallel T79 waves with role-keyed mut dirs. Review + post-merge gates 745/745 fallback-release (nextest absent per T82) + clippy under target-shared / target-shared-main.
- **Validator's 4 non-blocking findings (carried for next eval)**: (1) the load-time matcher-fit `mcp__` canary over-rejects server-specific globs like `mcp__fs__*` + path (fail-open, operator-visible — a skipped-rule warn, never a silent dead deny; phase-2 candidate: canary should accept any `mcp__*`-prefixed tool glob); (2) the spec-check filter `cargo test --bin chug permissions` misses the 5 strongest driver-integration legs (a mutant survived under the plural filter, killed under `permission` — spec-authoring lesson: check filters should be the broader stem); (3) the stderr warn-once latch is not test-observable (events line is, stderr isn't); (4) size at the guard's edge (363 vs ~350).
- Cycle notes: freshness-skip per predicate (1 todo row + same-day EVALUATION.md; loopd routed routine glm, d1790550858-1 era). Opened by landing cycle-51's wrap remainder (21a76a6: T90 not-started note + cycle-level notes + cycle-44 compaction) — the previous cycle died at goal_complete with its wrap uncommitted; the eval commit was durable but the final EVALUATION.md push wasn't. Queue is now EMPTY (T1–T90 all done): next cycle's freshness predicate will NOT hold on the queue half → eval cycle on kimi unless rows are filed. `com.tampajohn.chug-loopd.plist` remains untracked at repo root (operator's launchd unit — left alone, operator's call).

### Cycle 51 (2026-09-27) — fresh eval (kimi, loopd-routed: queue empty) — T88 + T89 + T90 filed; T88 + T89 LANDED

**T89 delegate terminal-wait + LOOP-SPEC adoption — LANDED, merge d2b402a.**
glm impl run1 died 50/50 ceiling-zone (work done, gates+commit unfinished —
the T15/T17/T20 shape); T63 **resume #18** (pid 49989) accepted 7/50
(commit 6e95df9, +532/−23: delegate.rs `terminal` flag + 8 RED-proven
tests incl. two named mutant RED runs, tools.rs schema property +
description, LOOP-SPEC §2 step 2 default-posture adoption with the
iteration-economics clause, README integrated clause; pin sweep verified
loop_spec_recovery.rs STEP2_ANCHOR bytes intact). Review 721/721
fallback-release + clippy (target-shared). **kimi REQUIRED** (doctrine +
tools.rs; pid 53370) **VERDICT: PASS 0 blocking** at 48/50 — gates
independently re-run under target-shared-validate (721/721 fallback +
79/79 spec check), **8/8 mutants killed** in 2 parallel T79 waves with
role-keyed mut dirs (passthrough, never-wake, file-creation-drop,
rejection-==999, budget_low-wake, liveness-off, schema-rename,
launch-rejection-removal), tree byte-clean; 3 non-blocking notes carried
(presence-vs-flip semantics documented; `terminal: false` launch leg
untested; the one-call shorthand is bounded by the 600s cap). Recovery
routing d1790549083-14, verdict d1790549953-15. **Adoption note:** THIS
cycle's own binary predates the merge, so its `terminal` field would be
silently ignored (unknown-key tolerance) — the cycle used the paced
sleep+instant-poll cadence instead (~90s spacing cut validator-arc poll
cost to ~10 orchestrator iterations vs the measured ~40–60 wake-per-
advance pattern); the next cycle's rebuilt binary gets the real terminal
waits, and T89's re-measurement (does the 160-cap pressure lift?) lands
next eval.

**T88 decision_log corrective validation errors — LANDED, merge c5a4f9e.**
glm impl (pid 39838) accepted 33/50 CLEAN first-try (commit e127d96):
src/decisions.rs ONLY (+457/−27), all five reqs — one error lists every
invalid field in schema order (req 1), received-keys + one-record reminder
on unknown shapes (req 2 — trigger honestly widened to
no-required-key-OR-unrecognized-key so the alias leg with 5/6 valid keys
still diagnoses), JSON-type naming for non-object input (req 3), success
path byte-identical (req 4 — 8 pre-existing pins green unmodified),
confidence range message preserved (req 5); 9 new RED-proven legs. Review
gates **713/713 fallback-release** (704 baseline + 9) + clippy under
target-shared; post-merge 713/713 under target-shared-main. kimi SKIPPED
per routing (d1790548521-11 — decisions.rs not REQUIRED; error-text-only
failure legs; T16/T31 precedent). **T82 runner note:** cargo-nextest was
uninstalled from the host at ~22:16Z (loopd re-exec probe at 22:16:48Z
logged the fallback; cycles 48–50 genuinely ran nextest 0.9.146) — every
gate this cycle runs the unconditional fallback, named in each commit.
**Forensic bonus (filing-verified):** the glm impl hit `missing or
non-string field: old` ×3 on edit_file and self-reported a tool bug in its
own decision record — the harvested transcript proves it sent `old_string`
(the Anthropic-canonical alias), NOT `old`: model fumble, tool contract
correct, child's self-diagnosis FALSE (its bash-heredoc route-around cost 0
iterations). Same alias class as T88's decision_log findings — generalized
received-keys diagnosis for tools.rs `get_str` is a next-eval candidate
(touches REQUIRED-listed tools.rs; pin sweep needed; out of T88's scope).
Arc: 1 impl, 0 resumes, 0 fix-ups, 0 validator rounds — the T88-sized
shape the queue wants.

**T90 permissions deny-list — NOT STARTED (budget), ready for next cycle.**
The orchestrator hit budget_low with 8 iterations left after the T89 wrap;
the honest act under the anti-sprint-burn guard was wrap, not a third arc.
The row stays `todo` with a complete spec (deny-only phase 1, policy order
permissions → hooks → risk gate, fail-open config / fail-closed match,
size guard per the T83 grinder lesson, REQUIRED kimi routing pre-named).

**Cycle-level notes.** Two items landed of three filed (T88 clean arc;
T89 one ceiling-zone resume + one clean validation). The eval itself ran
~40 iterations; two full item arcs + wrap fit in ~110 — the 160 cap held
THIS time, but only because the paced sleep+instant-poll cadence (~90 s
spacing, the written 60–110 s cadence) cut the measured ~75%-poll share
to ~15 poll iterations across three child arcs. **T81 acceptance leg,
eval-kimi half (owed since cycle 47):** this fresh-eval cycle on kimi
filed 3 well-formed rows with specs, landed 2, and wrapped truthful in one
run with zero recovery — quality bar met; both halves of the T81
acceptance are now on record (routine-glm: cycle 46; eval-kimi: this
cycle). **Gate-runner environment:** cargo-nextest was uninstalled from
the host at ~22:16Z (loopd re-exec probe logged the fallback at
22:16:48Z); cycles 48–50 genuinely ran nextest 0.9.146 (their commit
narratives stand); every gate this cycle ran the unconditional fallback
(~2.4× slower wall, same coverage), named in each commit. **decision_log
adoption this cycle:** 18 records (10 eval-triage incl. rejects, 1
validation-routing, 1 validation-verdict, 1 recovery-routing, 5 outcomes).
**Watch items carried:** T80 md-only telemetry (4th cycle unexercised);
T85 cross-tree doctrine measurement (one data point: the T89 validator
read META-SPEC §6's escape-hatch line and used bash cross-tree with zero
path-escapes-cwd errors — early pass); T89 re-measurement (160-cap
pressure vs terminal waits — next eval); delegate-paragraph density (7th
eval — the T89 clause lands it at the split threshold, next eval judges);
t89 validator's 3 non-blocking notes; t88-impl's FALSE self-diagnosis
(its own `old_string` alias fumble recorded as a tool bug — F13 corpus
will need label cleaning; generalized get_str received-keys diagnosis is
a next-eval row candidate, REQUIRED-listed tools.rs).

### Cycle 50 (2026-09-27) — routine glm freshness-skip; T85+T86+T87 bundle LANDED (merge 2440520) — cycle-49's mid-arc recovery executed end-to-end

- **T85 (128a4b3)** — the bash escape hatch is now doctrine in both review and validation surfaces: LOOP-SPEC §2 step 3's Review paragraph gains the parenthetical after "read the child's ledger if ambiguous" (file tools are cwd-confined and refuse `/tmp/chug-loop-*` paths with `path escapes cwd`; read worktree files via bash) and META-SPEC §6's validator goal template gains the same sentence after "weak tests." kimi (pid 14318) VERDICT PASS 0 blocking at 18/50: token verified in both required positions, §6 confirmed single edit point, pin-safety verified (zero pinned needles in inserted lines, all pin files green), gates re-run under target-shared-validate (build + clippy + nextest 704/704 + spec check), M1 loop-spec-insertion-removed + M2 meta-spec-insertion-removed both RED via parallel T79 throwaway worktrees (removed; byte-clean restore). Origin: cycle-47 eval I3.
- **T86 (da1c51a)** — README Development `src/{...}` list gains tgrep, plan, hooks; set-equality verified 27/27 against `ls src/*.rs` minus main.rs by both the child and the orchestrator independently. Origin: cycle-47 eval §6(c) — first staleness finding in four audits.
- **T87 (ba5a9a0)** — the unreachable `bash -n` leg is gone from the docs-only guard floor (an all-`.md` diff cannot contain a `.sh`) and the "cannot go red" overclaim is replaced with the honest risk model: an md-only edit touching a PINNED doctrine carrier (`tests/loop_spec_*.rs`, `shared_target_dir.rs`, `nextest_gate_runner.rs`) CAN go red, so the editor ALSO runs the affected pin file's tests (seconds) before shrinking gates, with doubt keeping the existing ambiguity default. Same commit updates `tests/loop_spec_docs_only_gates.rs`: BASH_N const/entry/docs removed, new pin leg (i) asserts RISK_CARRIER + RISK_EDITOR exactly-once whole-file AND inside step 3's window (wrap-insensitive flat idiom); `nextest_gate_runner.rs` byte-untouched. Both RED legs proven by the impl child: re-inserting the leg fails the negated grep while the pin file stays green (nothing blesses it), deleting the risk-model sentence turns the new pin RED. Origin: cycle-44 T80 validator carry (cycle-47 eval §2).
- **Recovery shape (cycle-49 → 50 handoff worked as designed)**: cycle-49 budget-wrapped with the bundle impl child (glm pid 90007) still flying in /tmp/chug-loop-t85; this cycle found it alive at 43–45/50 with T85+T86 already committed in queue order and an exact T87 execution plan written into its worktree LEDGER, then it aborted 50/50 (iteration budget) at 22:01Z. T63 resume #17 (pid 9030, same worktree, same 3-row goal) ACCEPTED at 20/50 — executed the handoff plan verbatim (both edits, both RED-proofs, gates 704/704 + clippy, commit ba5a9a0, both spec check-lines verbatim). Second consecutive cycle where preserve-the-worktree + a written row recipe made the next cycle's recovery mechanical. Recovery-routing d1790546502-1.
- **Validation coverage note (non-blocking)**: the ONE kimi round covered the bundle per T45 with the spec field on T85 (first-row precedent); its deep mutation pass targeted T85's insertions; T87's killing tests were RED-proven by the implementer and ran green in the validator's full 704/704 suite; T86's set-equality was orchestrator-verified. Validation-routing d1790546766-2, verdict d1790547036-3. Review gates 704/704 nextest-release 9.4s + clippy under target-shared; post-merge gates 704/704 nextest 8.97s + clippy under target-shared-main. 5 artifacts harvested pre-removal (impl events 2-segment, validate events, impl archived handoff LEDGER, validate verdict LEDGER, child decisions). Freshness predicate held at launch (3 todo rows + same-day eval) → Phase 1 skipped per T81 routine routing.

### Cycle 49 (2026-09-27) — routine glm freshness-skip; T83 LANDED (merge ccb828a) — hooks phase 1, the F3 top Tier-1 row
- **T83 (merge ccb828a; impl fd7f97a + fix-up 94ede2e)** — F3 hooks phase 1: `.chug/hooks.json` PreToolUse veto + PostToolUse advisory. Impl arc was a budget grinder: run1 50/50 abort uncommitted → T63 resume #15 (pid 32727) 50/50 abort uncommitted → T63 resume #16 (pid 45954) ACCEPTED 36/50 (row recipe leg (2) sanctioned the second resume). Landed: src/hooks.rs 977 new (per-checkout config, absent/empty = zero hooks at zero cost, malformed = fail-open with ONE stderr warn + one hook_error event line; glob matchers `*`/`?`/`mcp__*`; `sh -c` in its own process group, 10s wall cap, JSON stdin; veto = `[hook veto] <stderr-tail>` tool error the model routes around, risk-gate shape; PostToolUse advisory appended as `\n\n[hook] <text>`, never touching ok/is_error; `type:"hook"` + `hook_error` event lines; plan mode structurally zero-hooks; README `## Hooks` section) + driver integration with SPEC-8-safe tool-duration capture + 24 new tests + live smoke end-to-end (real binary, fake server: MARKER bash vetoed, file never created, model routed around; PostToolUse echo landed in the tool result).
- **What the validators caught**: kimi R1 (pid 56561) VERDICT FAIL — 1 blocking: **PostToolUse fired on PreToolUse-vetoed calls** (tool never executed; model received `[hook veto] …\n\n[hook] …`, violating req 2's exact veto-text contract + req 3's after-execution lifecycle + a phantom fire line in events.jsonl); 6/6 mutants RED (vetoflip, unbounded-wait 60.2s, no-process-group, plan-exclusion 3-site, no-advisory-append, no-error-line). Fix-up (glm, 32/50) ran the **class sweep** (cycle-33 lesson): the class "PostToolUse fires on a call whose tool never executed" has TWO instances — the veto AND the sibling **risk-gate block** path — one `!blocked` guard now covers both, with 2 RED-proven killing tests (each fails pre-fix AND with the guard reverted; veto result asserted byte-exact) + the trivial EOF-newline restore. kimi R2 (pid 79800) VERDICT PASS 0 blocking at 24/50: gates 703/703 nextest-release + clippy under target-shared-validate, both priority mutants RED (guard-revert reproduces the round-1 bug shape; risk-gate-leg break is surgical), req-2 veto text byte-exact, executed-path behavior unchanged, tree byte-clean.
- **Recovery/budget shape**: 3 impl runs to acceptance (2 budget-aborts uncommitted, both recovered in-worktree by resume — the row's standing recipe carried the exact next-cycle instructions and this cycle executed it verbatim). Minutes never binding; iterations were. Post-merge gates 703/703 nextest 8.8s + clippy under target-shared-main. 9 artifacts harvested pre-removal (4 event streams: impl/validate1/fixup/validate2; 3 LEDGERs incl. both validation reports; child decisions; console log).
- **Deferred MID-ARC**: T85/T86/T87 docs-only bundle — impl child (glm pid 90007) IN FLIGHT at ~29/50 in /tmp/chug-loop-t85 (preserved; T85 LOOP-SPEC+META-SPEC edits uncommitted, T86/T87 pending) at this cycle's budget wrap; full next-cycle recipe on the T85 row (collect → review + FULL gates — T87 has .rs → NOT md-only → kimi REQUIRED validation → harvest → merge → 3 flips → push). Non-blocking validator observations carried: signal-death pre/post asymmetry (" (exit none)"), chat-mode per-turn hooks reload (documented per-invocation semantics).

### Cycle 48 (2026-09-27) — routine glm freshness-skip; cycle-47 dead eval recovered; T84 landed (745f8ff)
- **Cycle-47 eval RECOVERED at cycle open.** The kimi fresh-eval (launched 20:07:48Z per the todo_rows=0 probe) completed the corpus read, filed 5 rows (T83–T87 with specs), rewrote EVALUATION.md — then died at ~20:14Z (6.5-min run) BEFORE its git commit and BEFORE any of its eval-triage decision_log calls landed (decisions.jsonl still at the 100 pre-cycle records). Cycle 48 (routine glm leg — freshness predicate held: 5 ready todo rows + same-day eval mtime; glm-never-evaluates honored) landed the artifacts verbatim (`89d1b5a`, todo_consistency 5/5) and reconstructed 11 records from the eval's own text: d1790540514-1..6 (5 row filings + cycle-47's fresh-eval routing) and d1790540528-7..11 (4 rejected candidates — outage retry, max-iters raise re-affirmation, RED-proof mandate, cross-tree read release — plus cycle-48's freshness-skip and the recovery routing). Lesson for the next evaluator: **the eval commit is the eval's FIRST durable artifact** — write-then-die loses both the commit and the records.
- **T84 LANDED** (driver.rs trim-machinery extraction to src/trim.rs; fast-forward merge `745f8ff`, branch loop-t84). glm impl 48/50 goal-accepted FIRST-TRY (pid 95988, ~15 min wall): 981 moved lines byte-identical to 89d1b5a verified mechanically — sole delta the disclosed pub(crate) seam on transcript_trim; 17/17 trim tests relocated count-exact (17 before in driver::tests, 17 after in trim::tests); driver.rs 5488→4509, src/trim.rs +1007 (12 header + 3 consts + 189 prod + 803 tests); `mod trim;` in main.rs alphabetical + README layout line gains trim; RED-proofs demonstrated and reverted (mod-drop → E0432 compile-RED; one-token trim_marker_text mutant → moved pin RED). Review gates 677/677 nextest-release 9.2s + clippy under target-shared. kimi VERDICT PASS 0 blocking 38/50 (pid 14329): byte-identity independently re-proven (two-pointer bounded-lookahead walk after difflib blew up on repetitive content), gates re-run under target-shared-validate — build + clippy + nextest-release 677/677 + cargo test 12/12 suites, 4/4 mutants RED in ONE parallel T79 batch (M1 mod-deletion compile-RED, M2 rounding flip → trim_segment_walk pin, M3 pairing_unsafe drop → both trim_refuses_ pins, M4 call-site drop → byte-stable-assemblies pin), 1 non-blocking finding: the spec's "driver.rs ≤ ~3,600" was arithmetic-impossible (the trim family is 981 movable lines; 5488−981=4507 ≈ the landed 4,509) — honestly disclosed by the impl and confirmed by the validator; filing-hygiene note for the next eval (estimate the movable mass before pinning a target figure). Post-merge gates 677/677 nextest 8.9s + clippy under target-shared-main. 5 artifacts harvested pre-merge (impl+validate events, impl+validate LEDGERs, validate decisions.jsonl).
- Outcome backfills: T84 validation-routing d1790541593-12 + validation-verdict d1790542412-13 → landed-clean.
- Queue after T84: T83 (pri 2 feature, hooks p1 — driver.rs again, strictly serial), T85+T86 docs bundle then T87 (pri 3).

- **T83 MID-ARC** (F3 hooks phase 1, the feature row): impl child IN FLIGHT at wrap — glm run1 (pid 31121) died 50/50 on the iteration budget with the work complete-but-uncommitted (src/hooks.rs written + driver.rs/eventlog.rs/events.rs/main.rs/tui.rs edits +92/−3 at last read, README + tests pending — the ceiling-zone pattern, 15th sighting) → T63 FIFTEENTH resume (pid 32727, same worktree /tmp/chug-loop-t83, same budgets 50/35) in flight at ~20/50 appending tests when the orchestrator's own budget-low (8 iters) forced the wrap. Worktree PRESERVED (T19 — harvest precedes any removal). Full next-cycle recovery recipe on the TODO row: collect → review + gates → kimi REQUIRED (driver.rs + tools registry) → harvest → merge → flip → push; a run2 budget-death next-cycles to the same recipe.
- **Deferred, reason budget**: T85 + T86 (docs bundle, pri 3) and T87 (docs-only floor cruft, pri 3) — untouched this cycle, ready specs, next cycle dispatches after T83's arc closes (docs trio runs alone — doctrine).
- Cycle notes: routine glm freshness-skip cycle (d1790540528-10). Opened by RECOVERING cycle-47's dead eval (see top bullet). T84 landed end-to-end first-try on every round (impl 48/50, kimi PASS 0 blocking, 4/4 mutants RED — zero fix-ups, zero review-findings). T83 consumed the rest: one budget-abort + resume, in flight at wrap. Decision records this cycle: 11 reconstructed eval-triage/routing (cycle-47 recovery) + T84 validation-routing d1790541593-12 + validation-verdict d1790542412-13 + 2 outcome backfills + T83 recovery-routing d1790542703-16 (T63 resume #15) + T83 validation-routing deferral at wrap. Filing-hygiene note for the next eval (from T84's non-blocking): estimate the movable mass before pinning a target figure in a spec (T84's "≤ ~3,600" was arithmetic-impossible). Watch items carried: delegate-paragraph density (5th eval); T80 md-only telemetry (the docs trio remains the first md-only round — still unexercised); T76/T77 Langfuse telemetry legs; T83's resume #15 acceptance (16th datapoint for the T63 pattern).

### Cycle 46 (2026-09-27) — freshness-skip (glm routine — FIRST loopd-routed cycle); T82 + T75 both landed (6edc5ee, b4c159f); queue EMPTY
- **T82 LANDED** (cargo-nextest for gates; merge `6edc5ee`, branch loop-t82 at `c9ef37b`). Recipe from the cycle-45 row executed exactly: step-3 review gates FULL non-md diff under target-shared — 673/673 nextest-release 9.8s + clippy clean; REQUIRED kimi validation ALONE (pid 25121, goal-accepted 33/50): gates independently re-run under target-shared-validate — build --release + clippy green, `cargo nextest run --release` 673/673 in 9.107s (T31 dead_port + RUN_SHELL_TIMING_LOCK families green under nextest), spec-check `cargo test` debug 673/673, fallback leg `cargo test --release -- --test-threads=4` 673/673; mutation testing 6/6 RED in 2 parallel T79 batches (M1 LOOP-SPEC fallback drop → count pin, M2 META-SPEC §5 fallback drop → count pin, M3 loopd else-branch drop → branch pin, M4 delegate argv-stub wrong-name publish → 4 delegate_launch tests, M5 README fallback strip → block-pairing, M6 loopd cargo-install hard-require → installs-nothing pin); 3 non-blocking observations (prose fallback shorthand in wrap/README carriers, coarse block-pairing granularity in META-SPEC steps 1-7, literal cargo-install-only install pin); tree byte-clean, 6 throwaway worktrees removed. 5 artifacts harvested pre-merge (impl two-segment events + impl archived LEDGER + validate events + validate LEDGER + delegate console log — the worktree carried NO child decisions.jsonl: validator made no decision_log calls and the impl made none; orchestrator-side T82 records live in main's decisions.jsonl). **req-3 acceptance measured in main post-merge under target-shared-main: nextest 673/673 summary 8.613s (wall 8.9s) vs fallback 673/673 wall 21.9s — 2.5x faster, zero red-only-under-nextest families** (in-worktree impl datapoint was 9.0s vs 19.7s). First cycle whose gates actually ran nextest-first per the landed doctrine (review + post-merge + acceptance all nextest).
- **T75 LANDED** (decision-records wrap-checklist sentence; fast-forward `b4c159f`, branch loop-t75). impl glm 19/50 first-try goal-accepted (pid 47832): ONE +7-line in-place insertion in LOOP-SPEC.md's Phase-3 wrap checklist — `.chug/decisions.jsonl` carries the cycle's records (all six classes named), a worked-item cycle shipping zero decision_log records is an INCOMPLETE wrap (T23→T24 zero-calls lesson; cycles 34+35 receipt) — every other Phase-3 byte unchanged; new pin file tests/loop_spec_decision_records.rs +195 (exactly-once legs for carries-records/incomplete-wrap/T23→T24-citation tokens + T64 loose-heading Phase-3-window scope leg, T48 runtime resolution, T67 LOOP-SPEC-only grep honored); deletion hand-check in the commit message (line removed → 4 legs RED 0/4, restored → 4/4). The impl also logged the corpus's FIRST ORGANIC decision_log call (d1790538306-1, class validation-verdict, subject "T75 pin-file needle design") — the adoption dogfooded itself the same arc that landed it. kimi VERDICT PASS 0 blocking 25/50 (pid 58272): gates independently re-run under target-shared-validate — build + clippy green, nextest --release 677/677 in 8.9s, spec check verbatim green (4/4 pins + 677/677), fallback 677/677; 6/6 mutants RED in 2 parallel T79 batches (M1 drop 0/4, M2 move-out-of-window scope-only RED, M3 duplicate 0/4, M4 reword, M5 arrow-flatten, M6 corrupt-needle — each died on exactly the expected legs); 3 non-blocking observations; tree byte-clean, 6 throwaway worktrees removed. Post-merge gates 677/677 nextest 8.8s + clippy under target-shared-main. 5 artifacts harvested pre-merge (impl+validate events, validate LEDGER, child decisions.jsonl carrying the first organic record, delegate console log; impl ledger seed-trivial skipped per T32). Doctrine row ran ALONE (T82's arc fully closed first — validator zombie reaped via delegate status per T28 before worktree removal).
- **T81 acceptance leg PARTIAL (routine half)**: this cycle is the FIRST loopd-routed cycle — the mechanical freshness predicate held at launch (2 todo rows + EVALUATION.md same-UTC-day mtime from cycle-45's wrap 5 minutes prior) and loopd routed the cycle to glm-5-3-flash; the routing decision was honored end-to-end: glm skipped Phase 1 (glm-never-evaluates boundary), worked the full queue (T82 + T75), and stayed off every judgment call — both validations were launched on kimi. Routine-leg wall time + outcome quality: cycle start 19:23Z; T82 arc complete ~19:39Z (~16 min, mid-arc recovery, one validation round); T75 arc complete ~19:53Z (~14 min); both first-round kimi PASS, zero fix-up rounds, zero T63 resumes this cycle. The eval-kimi half of the acceptance leg still needs the next FRESH-EVAL cycle (predicate must break: queue empty after this wrap, so the next cycle evaluates → kimi).
- Outcome backfills this cycle: T82 d1790536134-8 + d1790537126-2 + d1790537771-3 → landed-clean (recorded d1790537950-4/5/6); T75 d1790538538-7 + d1790539368-8 → landed-clean (recorded d1790539412-9/10).
- Queue state at wrap: EMPTY (T75 + T82 were the only todo rows — both landed this cycle). Next cycle is a FRESH-EVAL cycle by construction (Phase 1 will run, routed to kimi).

### Cycle 45 (2026-09-27) — freshness-skip; T81 landed (merge c1daaca)
- **T81 LANDED** (per-phase model routing: glm routine, kimi judgment; merge `c1daaca`, branch loop-t81 at `f1f58a2`). Recovered from cycle-44 wrap state: impl child (pid 53420, glm 50/35) aborted 50/50 on the iteration budget AFTER committing f1f58a2 with the full suite green (cold test binaries pushed the final suite past the 120s tool cap at iter 45; budget-low at 42 per T18) → T63 THIRTEENTH resume (pid 72502) goal-accepted at iteration 1. loopd.sh: LOOP_ORCH_MODEL (default kimi-k3) + LOOP_ROUTINE_MODEL (default glm-5-3-flash) knobs; mechanical freshness predicate (todo_rows awk + eval_fresh same-UTC-day mtime, CHUG_ROUTINE_TODAY test pin, macOS+GNU stat legs) in route(); `routing` subcommand probe; per-cycle routing line in loopd.log + cycle-log first line; `--model "$orch_model"` (hardcoded kimi gone); one-env-var rollback. LOOP-SPEC: header per-cycle routing (never model judgment), Phase-1 glm-never-evaluates wrap boundary, step-4 validation ALWAYS kimi (family independence), Hard-rules model-agnostic anti-sprint-burn guard (>5 consecutive iterations without a child launch MUST act: launch/merge/wrap — the M2/M3 lesson made structural, binds kimi and glm alike, never authorizes breaking the ONE-WRITER caps). README documents knobs + rollback. tests/loopd_model_routing.rs 10 tests (exact-count static pins + behavioral `loopd.sh routing` fixture runs, both freshness legs load-bearing, safe defaults, rollback env). Orchestrator gates 666/666 release + clippy under target-shared. kimi VERDICT PASS 0 blocking 39/50 (pid 77595): gates independently re-run 666/666 release + clippy under target-shared-validate, live routing probes on the real repo both legs, 9 mutants in 3 parallel T79 batches — 8 RED-killed (arms-swap 5 tests, ge0, or, always-fresh, hardcode, no-guard, guard-50, drop-step4), 1 survivor split-swap (run-path `orch_model=${routing#* }` split unpinned — LOUD launch failure `--model routine`, not a silent misroute), 4 non-blocking findings (split-swap pin gap; todo_rows awk `/^[|]/` anchor misses 3 legacy leading-space TODO rows — all done, safe-side misroute toward eval/kimi only; GNU stat leg degrades safe; LOOP_ORCH_MODEL operator-settable off kimi backstopped by the Phase-1 wrap boundary). Post-merge gates 666/666 release under target-shared-main. 6 artifacts harvested pre-merge (impl+validate events two-segment pair, both LEDGERs, validate decisions, delegate console log). **Acceptance leg owed by later cycles (spec Tests section): one routine-glm cycle + one eval-kimi cycle recorded in Outcomes with per-cycle wall time + outcome quality notes** — the loopd routing switch fires for real from the next cycle onward.
- **T82 MID-ARC** (cargo-nextest for gates): impl committed + goal-accepted in /tmp/chug-loop-t82 (branch loop-t82 at `c9ef37b`: `d84c3cb` nextest-first gate templates in LOOP-SPEC/META-SPEC with UNCONDITIONAL fallback to `cargo test --release -- --test-threads=4`, loopd.sh runner probe + log line, tests/nextest_gate_runner.rs 7 pins + shared_target_dir.rs re-pins; `c9ef37b` fixes a PRE-EXISTING delegate argv-stub TOCTOU — atomic publish argv.tmp+mv — that the spec check caught at default parallelism). glm run1 50/50 abort uncommitted → T63 FOURTEENTH resume (pid 2841) accepted 22/50; 673/673 green under BOTH runners in release (nextest 9.0s vs fallback 19.7s in-worktree; orchestrator baseline in main: nextest 666/666 in 8.867s summary / 26.35s real, zero red-only-under-nextest families — req-4 vacuous). Host: cargo-nextest 0.9.146 installed at ~/.cargo/bin via get.nexte.st prebuilt (from-source install fails: usdt_probes compile errors on rustc 1.95.0). Full recovery recipe on the TODO row (review gates → REQUIRED kimi alone → harvest → merge → flip → backfills → req-3 both-runners wall times). Pending outcome backfill: recovery-routing d1790536134-8.
- **T75 deferred** (decision-records wrap-checklist sentence, pri 3 doctrine, runs alone): ready spec; not dispatched — cycle budget died after two recovery arcs + one full arc (T81) + T82 impl; doctrine items never overlap so no parallel launch was possible.
- Cycle notes: freshness-skip (d1790533899-1 — queue full with ready specs, cycle-44 had skipped 2 min prior; EVALUATION.md mtime stayed today via cycle-44 wrap). T81 landed end-to-end (recovery + full arc: review 666/666 + kimi PASS 0 blocking first round — 9 mutants 8 RED, 1 loud-failure survivor). TWO T63 resumes this cycle (#13 T81 accepted 1/50, #14 T82 accepted 22/50) — 14/14 all-time; both were the ceiling-zone pattern again (50/50 aborts at the wrap boundary: cold-binary full-suite push at iter ~45). The T82 spec check caught a real pre-existing flake class (test stub truncate-then-read race at default parallelism) — fixed by the impl in c9ef37b, scrutinize at validation. Decision records this cycle: 1 eval-triage + 2 recovery-routing + 1 validation-routing + 1 validation-verdict + 3 outcome backfills (T75's adoption sentence remains relevant — records now exist but the wrap-time accounting surface is still unwritten). Acceptance legs owed: T81 (one routine-glm + one eval-kimi cycle w/ wall times — NEXT cycle is the first routed-by-loopd cycle), T82 (both runners' wall times post-merge), T76/T77 telemetry carry. Compaction of cycle-38-and-older Outcomes carried to next wrap (budget). Outcome backfills through d1790535671-7; d1790536134-8 pending T82's landing.

### Cycle 44 (2026-09-27) — T80 docs-only gate slimming (9c4221d) + T79 parallel mutants (660f652) landed; T81 MID-ARC (impl in flight, recovered cycle 45); morning tools-proxy outage halted cycles 42/43 fast (halt-is-the-pager shape); full narrative: git log 9c4221d..660f652 + TODO done rows
### Cycle 40 (2026-09-26) — T78 release builds landed (merge 23fd276)
- **T78 LANDED**: loopd.sh builds + launches target/release/chug; LOOP-SPEC/META-SPEC launch paths and all review/merge/validation gates are now cargo test --release with the cold-build tradeoff named; spec check: convention stays debug (pinned); README documents it. impl a74769f (T63 resume #12 after 50/50 abort uncommitted) + fix-up f5a3ebc (tests-only, 11 granular count_eq pins, each RED-proven) after kimi R1 FAIL weak-tests (M8-M11 unpinned release carriers — single-line count needles could not see line-wrapped carriers; the class sweep pinned wrapped perl/timeout examples, comment restatements, README wrap-insensitively) then R2 PASS 0 blocking (fresh sweep F1-F8 RED, F9 prose survivor non-blocking). Post-merge gates 646/646 RELEASE under target-shared-main (migration half 1 paid; loopd pays half 2 next supervisor iteration). Acceptance telemetry (release gate wall time vs the 6-10s debug baseline) is next cycle's Outcomes item per the spec.
- Cycle notes: freshness-skip cycle (cycle-36 eval same-day). T63 resumes 12/12 all-time. R1 was the vacuous-pin family again — fixed by class sweep per T72 doctrine. Queue carries T80/T79/T81 (pri 2), T75 (pri 3 doctrine, runs alone), T82 (pri 3) — all with ready specs. Compaction of cycle-36-and-older Outcomes carried to next wrap (budget). Outcome backfills carried: d1790471401-24 (dispatch), d1790472177-1 (recovery), d1790472560-2 (routing).

### Cycle 39 (2026-09-26, ~20:1x-21:2x EDT) — freshness-skip; T76 + T77 both recovered and LANDED (merges 355f253, fedb9ef; pushed 26b547d, 09d9f03); T78 MID-ARC (impl in flight)

### Cycle 38 (2026-09-26) — freshness-skip; T76 + T77 MID-ARC at budget wrap, nothing merged

### Cycle 37 (2026-09-26, ~13:22–18:5x EDT) — freshness-skip; T73 recovered mid-arc + landed (merge 87fe53f); T76 MID-ARC (fix-up-2 in flight); I1 RESOLVED
### Cycle 36 (2026-09-26) — MANDATORY fresh eval (F2 SPLIT → T73/T74/T75 filed); T74 landed (merge ef0c3eb, impl 0f1475e, kimi PASS 24/50 3 mutants RED — body_watchdog flake margins re-classified mechanism-not-timeouts); T73 landed cycle 37 (87fe53f), T75 landed cycle 46 (b4c159f). Verdict: eval + 1/1 landed
### Cycle 35 (2026-09-26, ~10:42 EDT) — T70 recovered mid-arc + landed (merge 572ec5a: decision_log tool + LOOP-SPEC adoption at 4 named points, F13 phase 1); T71 (e1e940c) + T72 (db74a99) landed cycle 36. Verdict: 1/1 landed
### Cycle 34 (2026-09-26, ~09:55 EDT) — MANDATORY fresh eval (F13 SPLIT → T70/T71/T72 filed); T70 MID-ARC at budget wrap (validator-2 in flight; landed cycle 35 merge 572ec5a). Verdict: eval + staging
### Cycle 33 (2026-09-26, ~09:20 EDT) — T69 (F1 delegate collect) recovered + landed (merge c6ce238; impl ccd9923 + 2 fix-up rounds 50dcbe7/88227ab, kimi R3 PASS 0 findings, latch-reset class swept); QUEUE DRAINED. Verdict: 1/1 landed after 3 validation rounds
### Cycle 32 (2026-09-26, ~08:51 EDT) — delta eval + ROADMAP PULL (T69 = F1 delegate collect); T69 MID-ARC at budget wrap (validated-in-flight; landed cycle 33 merge c6ce238). Verdict: eval + staging
### Cycle 31 (2026-09-26, ~08:06 EDT) — MANDATORY fresh eval (queue EMPTY); T67 landed (spec check lines never cargo test --lib + todo_consistency lint; 0d3c88b) + T68 landed (delegate wait_secs wakes on significant change only; 362e9b9) — QUEUE DRAINED. Verdict: 2/2 landed
### Cycle 30 (2026-09-26, ~07:28 EDT) — freshness-skip; T64 (validator survivor pins; 1859b21) + T65 (README target-cache de-accretion; 458751c) + T66 (dead_port_probe drop-leg theft-retry; ac2ea36) landed — QUEUE DRAINED. Verdict: 3/3 landed, live goal-gate flake filed + landed same cycle
### Cycle 29 (2026-09-26, ~07:10 EDT) — MANDATORY fresh eval (queue EMPTY); T63 landed (LOOP-SPEC adopts delegate resume:true as standard budget-death recovery; merge db539c9, kimi PASS 5/5 mutants). Verdict: eval + 1/1 landed
### Cycle 28 (2026-09-26, ~06:39 EDT) — freshness-skip; ALL 4 queued rows landed green: T58 (delegate resume + latest-segment status; 5145536), T62 (eval-digest golden-section pins; f4e3bc3) resumed from cycle-27 wrap, T61 (resolve_safe error names bash; c176321), T60 (docs drift pass; 2ce4f8c) — QUEUE DRAINED. Verdict: 4/4 landed
### Cycle 27 (2026-09-26) — freshness-skip; T59 (mcp_http dead_port bounded theft-retry, tests-only; glm impl 27/50 first-try, ff-merge 062c175, scripted-theft pins exactly-attempt-2 + exhaustion-at-3 naming) landed — last known organic flake closed; T58 impl died 50/50 complete-but-UNCOMMITTED (5th occurrence: t15/t17/t20/t55/t58 — the demand signal for T58's own resume feature) → T55-precedent orchestrator finish b189517, T58 validator + T62 impl IN-FLIGHT at budget wrap with recovery recipes (both landed cycle 28); T60/T61 deferred with ready specs; T44 planning catch (disjointness must enumerate BOTH specs' FULL file lists — T58+T60 share README.md). Verdict: 1/3 landed, two clean recoveries staged

### Cycle 26 (2026-09-26) — MANDATORY fresh eval (queue empty; T46 digest-first acceptance HELD, filed T57–T62) + T57 landed (main-dedicated gates dir target-shared-main, doctrine; impl 1c2beee glm 32/50 first-try, merge 2f8bbe4, kimi PASS 18/50 13/14 mutants, 1 weak-pin survivor watch-item) — no future post-merge/final gate can execute a non-main binary by construction; cycle-19 T39 LEDGER mystery CLOSED (zero update_ledger calls, archive_stale correctly skipped seed); stale-loopd human carry PROMOTED. Verdict: eval + 1/1 landed

### Cycle 25 (2026-09-26) — freshness-skip; T55 (driver.lock same-cwd mutual exclusion, FEATURE; impl cb5906f, merge 2e7945b, kimi PASS 45/50 with real-binary hand-verification of the refusal cluster) + T56 (README loopd re-exec + ceiling dedupe; 611af46) landed — queue drained; SEQUENTIAL variant of the T52/T43 stale-artifact false-red observed at post-merge gates (touch+rebuild recovered, led to T57). Verdict: 2/2 landed, staleness class fully mapped

### Cycle 24 (2026-09-26) — MANDATORY fresh eval (queue EMPTY); T53 (loopd single-driver check ps-based, pgrep dead on this host — live 9/9 reproduction; impl 5c842a6) + T54 (loopd_reexec.rs exact-count SELF_CKSUM pins, additive-mutant survivor closed; impl 6587b22) landed as the FIRST T45 bundle (merge 8624078, kimi PASS 24/50, 8/8 bundle mutants); T55 mid-arc at budget (recovery recipe on its row, landed cycle 25); T56 deferred. Verdict: bundle arc ~7 min wall, pgrep blindness fixed within the hour

### Cycle 23 (2026-09-26) — freshness-skip; T49 (api.rs connect-timeout const + pins; 9a73b12), T52 (role-keyed target dirs; 4d4c383), T50 (loopd self-re-exec between cycles; 078cb77), T51 (README delegate paragraph trim; 372f920) landed 4/4 queued rows (kimi PASS x3: 17/50, 35/50, 41/50); T44 overlap #3 (T51 impl during T50 validator); QUEUE EMPTY → next cycle mandatory eval. Verdict: 4/4 landed, loopd-restart carry first stated


### Cycle 22 (2026-09-26) — MANDATORY fresh eval (queue EMPTY); T48 landed (test code resolves repo root at runtime, not env!(CARGO_MANIFEST_DIR); d000b72); T49 impl done, kimi validation deferred at budget (landed cycle 23); T44-overlap × target-shared race observed live (gates executed a validator's leftover mutant) → T52 filed pri-1. Verdict: 1/2 rows landed, race lesson became next cycle's doctrine

### Cycle 21 (2026-09-26) — freshness-skip; T43 landed (EVALUATION.md Outcomes pruning rule — last 6 cycles full — + first compaction of cycles 5-13; 8694003); CARGO_MANIFEST_DIR × target-shared staleness bit the post-merge gates → T48 filed. Verdict: 1/1 landed, staleness class identified

### Cycle 20 (2026-09-26) — freshness-skip; T40 recovered + landed (TODO notes-cell pipe doctrine + todo_consistency-after-edits; 75c21c8), T47 (shared CARGO_TARGET_DIR for worktree builds; 2f4cefc), T41 (sandbox-candor descriptions + README Tools intro; 7170fb4), T42 (webfetch timeout const pins; 7ff8bfa) landed; T43 deferred. Verdict: 4/5 landed

### Cycle 19 (2026-09-26) — freshness-skip; T46 (eval digest: pre-computed Phase-1 corpus summary; 6a3fa98), T45 (trivial-row bundling doctrine; 3efb98d), T39 (delegate launch optional max_tokens passthrough; 52ec8ab) landed. Verdict: 3/3 landed

### Cycle 18 (2026-09-26) — fresh eval (queue EMPTY at start); T38 landed (truncated-response advisory names the chunking remedy; a686522) + T44 landed (pipeline overlap, operator pri-1 speed initiative; 41f61a1); TWO 50/50 child deaths, both recovered. Verdict: 2/2 landed, child-death recovery proven twice

### Cycle 17 (2026-09-26) — freshness-skip; T37 (web_fetch tool) recovered from the cycle-16 goal-gate pipe death + landed (refs: 1432a2e, 1e82ffb) — queue drained

### Cycle 16 (2026-09-26) — fresh eval + T36 landed (loopd --max-iters 120→160: impl d5141c9, merge 0586e1e, kimi PASS 5/5 check-mutants); T37 web_fetch impl-complete on branch loop-t37 @ 1432a2e (430+3) with kimi validation deferred at budget; first organic T31-residual dead_port_probe sighting (~1/8); glm truncated-write death class first seen

### Cycle 15 (2026-09-25, ~23:47–23:56 EDT) — T35 landed (README quickstart cargo install --path .; impl 8ebbe59, merge 09730de, flip eb885a8) — queue drained; docs-only validation-optional per T16/T31; first forward practice of T34 per-item doctrine

### Cycle 14 (2026-09-25, ~22:50–23:59 EDT) — fresh eval filed T31-T35 (dd857c2); T31 landed (deflake parallel-load family; impl f8012c2, merge e765de2, flip 65b99fb) + T32 (validation-child template 40→50; a173ff2, merge a3d60ad, flip 723912e) + T33 (META-META priority line; 4b409c0, merge 0ee166b, flip e9853ce) + T34 (Outcomes per-item at landing; a965053, merge dcbff00); T35 deferred to cycle 15; all three kimi validators PASS first-try

### Cycle 8 (2026-09-25, ~16:29–16:47 EDT) — T22 landed (bash tool macOS timeout-mirage note; impl 6068654, flip d7147cf) — queue drained, zero open rows; eighth consecutive clean glm round

### Cycle 7 (2026-09-25, ~14:39–16:35 EDT) — T21 landed (impl-child template --max-iters 40→50; cac4649, flip e73b66d) + T23 landed (delegate tool; 1012dca); T21 self-merge anomaly (check cd'd to main) → worktree-relative check lesson; T22 deferred to cycle 8

### Cycle 5 (2026-09-25, ~13:29–14:05 EDT) — T18 landed (WARN_REMAINING_ITERS 5→8; impl 9056c78, flip 019cfa8): first-try PASS, fire-time 7→8 derived by impl/validator/orchestrator; T19/T20 filed for the next queue

### Cycle 6 (2026-09-25, ~14:03–14:40 EDT) — T19 landed (harvest-before-removal; impl 1e26acc, flip 8360ba4) + T20 landed (banner/run_start name worktree HEAD; f08ea04, flip 09496eb) — queue drained; J6 budget_low fired first time (T20 impl committed before ceiling)

### Cycle 9 (2026-09-25, ~16:44–17:20 EDT) — T25 landed (failure-aware event previews; impl f1f7946, flip 75d9c90); T24 impl committed 3334743, left unmerged in preserved worktree (recovered cycle 10); T26 spec ready

### Cycle 10 (2026-09-25, ~17:16–17:45 EDT) — T24 recovered + landed (LOOP-SPEC adopts delegate; merge e913cf9, flip ad80910) + T26 landed (read_file offset/limit pagination; merge 179e1aa, flip 4b77d6b) — queue drained; first full delegate-dogfood cycle

### Cycle 11 (2026-09-25, ~17:41–18:55 EDT) — T27 landed (loopd cycle budget --max-iters 80→120; merge 759c45e, flip 414acf3); hibernate-mid-child incident survived by design (awake-time budgets); T28 impl left running in preserved worktree

### Cycle 12 (2026-09-25, ~18:55–20:57 EDT) — T28 landed (delegate status reaps zombies; impl a0a7c97, merge fcaa7c2, flip 07dd3af); T29 mid-arc at exit — validator #1 FAIL, wrap deferred (RECONSTRUCTED at cycle-13 wrap; deferred-wrap lesson)

### Cycle 13 (2026-09-25, ~20:58–22:55 EDT) — T29 recovered + landed (delegate wait_secs long-poll; impl 549d250 + fix-up e36b30c, merge 47dfcaf, flip cd7d43c) + T30 landed (worktree-relative check: rule; merge 00ce617, flip a1dde80) — queue drained; operator spec 9b7e904 absorbed mid-cycle; flake-family sightings handed to next eval

