# EVALUATION — chug, assessed by chug-loop (2026-09-28, cycle 62)

**MANDATORY fresh eval** — the queue is EMPTY again (T1–T116 all done
with refs; cycle 61's wrap landed the last row), so the freshness
predicate's queue half fails and loopd routed an eval cycle
(`todo_rows=0 eval_fresh=yes -> eval cycle on kimi-k3`, loopd.log
12:12:33Z). The headline is the loopd-160 watch criterion TRIPPED:
**cycle 61 died at 160/160 iterations post-wrap, pre-goal_complete**
(wrap commit ccd4b0a pushed 12:06:36Z; loopd logged "cycle ended
WITHOUT goal complete (consecutive failures: 1)" 21s later) — the
raise candidate rejected twice at 104/160 is now filed as **T121**.
The roadmap pull is **F9 phase 2 → T117 + T118, SPLIT a/b** under the
T110 ceiling (phase 2's sequencing dependency — T115's goal-integrity
surface — landed in cycle 61 and is verified LIVE in this run's own
`run_start`). Two more rows from cycle-61 incidents: the orchestrator's
self-inflicted healthy-validator SIGKILL + sed silent no-op (→ **T120**
doctrine) and T115's two carried weak-test findings (→ **T119**).
This is a DELTA eval over cycle 61's: the new corpus is cycle 61's work
phase (the T112–T116 child arcs), its orchestrator stream, and the
loopd handoff into this run.

Corpus: `.chug/eval-digest.md` FIRST (FRESH — regenerated 12:12:09Z by
loopd pre-launch, 282 events files / 14,124 iterations; the only newer
stream is this run's own), the archived cycle-61 orchestrator stream
(`events-20260928-121234.jsonl` — **1 run_start, 1 abort, last
iteration 160: the I1 smoking gun**), the eleven t112–t116 child
streams (5 glm impls 26/62/44/63/26 of 80 ALL first-try goal-accepted;
4 kimi validators 21/26/19/44 of 50 all PASS + T116 skipped per
precedent; 2 killed-validator segments from the garble incidents),
`.chug/loopd/loopd.log` (routing 09:15:12Z → cycle end 12:06:57Z →
routing 12:12:33Z), `.chug/decisions.jsonl` (309 records — was 271 at
the cycle-61 eval, +38), `TODO.md` (T1–T116 all done), `src/` (api.rs
2,921 = 65% of the ~4,500 trip line, watch; tgrep.rs 2,531; mcp_http.rs
2,793; driver.rs 1,549 + split tests; commands.rs 291 new post-T113),
`README.md` (full cold read, §6 — fifth consecutive clean audit),
`FEATURES.md` (F9-p1 checked; **F9-p2 open, dependency met**; §4),
a live verification leg (this run's `run_start.goal_sha256` present —
T115's surface ACTIVE as the cycle-61 wrap predicted), `stream_fallback`
jq count 0 era-wide, and the layad deferral gate re-verified
(`~/.claude/settings.json` has no layad endpoint — phases-2 deferrals
stand).

## 1. What chug does well — be brief

- **The F8 todo tools went organic in ONE cycle**: cycle-61 children
  used `todo_add`/`todo_update` 60+ times across five streams
  (t112-validator 12, t113-impl 10, t114-impl 15, t115-impl 16,
  t116-impl 9) with ZERO todo-tool errors — the structured plan surface
  is now the children's default step-tracker, exactly the F8 bet.
- **The era's healthiest impl census**: ZERO budget deaths, ZERO T63
  resumes across five glm impls (26/62/44/63/26 of 80) — the
  T102+T110 sizing era is holding; the post-commit-death class went
  silent (§2 I6).
- **T112's fix is live-verified in production**: this run's own
  iterations report real `input_tokens` (8,975 → 11,830 and climbing)
  — the streaming accounting bug is dead on the default path, T15's
  budget axis real again.
- **T115's integrity surface activated exactly as predicted**: this
  run's `run_start` carries `goal_sha256` (loopd rebuilt pre-launch);
  the composition/transmission check pair is now mechanical, not luck.
- **kimi ran 4 rounds, all first-try PASS** (3 REQUIRED + 1
  optional-EXERCISED; 31 mutants, T79 parallel-mutant batches now
  routine) — and the orchestrator's review gates + row-flip discipline
  kept the second consecutive all-PASS cycle clean end-to-end.

## 2. Incidents worth fixing

### I1 — loopd 160 criterion TRIPPED: cycle 61 died 160/160 post-wrap, pre-goal_complete → T121

The archived orchestrator stream (`events-20260928-121234.jsonl`) ends
at iteration 160 with one abort ("iteration budget exceeded"); the wrap
commit ccd4b0a is timestamped 12:06:36Z and loopd logged the
goal-less cycle end 12:06:57Z — the cycle completed ALL work (eval +
five item arcs + wrap pushed, gates 924/924) and lost only the
`goal_complete` call, at 1/3 of loopd's HALT budget (3 consecutive
failures halt). Minutes had headroom (171 of 240); iterations did not.
The raise was weighed and rejected at the last two evals ("104/160 —
the ≥150 criterion nowhere near") — the criterion (a hard death, not
the ≥150 proxy) is now TRIPPED, and the recurring shape (a 5-item eval
cycle ≈195-240 iters by the loopd.sh:206 sizing model) guarantees
recurrence: this very cycle is a 5-item eval cycle. Filed as **T121**
(robustness, pri 2): `--max-iters 160 → 200` in loopd.sh + the sizing
comment + the `tests/loopd_model_routing.rs:73` needle (~25 lines;
the T27/T36 raise mechanism — watch criterion, death, raise).

### I2 — orchestrator self-inflicted: healthy-validator SIGKILL + sed silent no-op → T120

Two cycle-61 incidents, one doctrine gap (both recorded in the
cycle-61 Outcomes entry; the lesson currently lives ONLY in
EVALUATION.md, which orchestrators do not re-read mid-cycle — LOOP-SPEC
is the re-read surface). (a) Reading a RENDER-ONLY garble in the
delegate-status console view, the orchestrator SIGKILLed a HEALTHY
T112 validator 35s in — in the same breath as the check, before any
read-back; transcript read-back afterwards proved the 2166-byte payload
intact; relaunch cost a full validator spin-up (~11 min). (b) A
`sed -i` bookkeeping edit with a typo'd anchor exited 0 changing
nothing (sed never fails on no-match); only a later read-back caught
it. Filed as **T120** (doctrine, pri 4): LOOP-SPEC gains the
verify-then-kill SEQUENTIAL rule (step 2 — read the payload back from
disk in a SEPARATE completed step, kill only on proof; never kill in
the same breath as the check) and the assert-after-sed rule (step 5 —
grep-verify the needle; prefer edit_file, which errors on no-match),
with windowed exactly-once pin legs k-l beside T114's legs i-j in
tests/loop_spec_recovery.rs (~55 lines; runs ALONE; kimi REQUIRED —
loop doctrine).

### I3 — T115's carried weak-test findings (3 validator mutants survived) → T119

The T115 kimi round PASSed with 3/8 mutants surviving → 2 non-blocking
weak-test findings (cycle-61 Outcomes; validator ledger harvested):
(1) no multibyte-goal leg — `goal_tail`'s chars()-based rule is
invisible to ASCII-only fixtures, so a byte-slicing mutant (panic /
split char on multibyte input) survives; (2) `run_start` call-site
wiring unpinned — the driver `Some→None` and chat `None→Some` mutants
survive the full suite (eventlog-level legs pin the field's shape, not
the call sites). The carry-then-eval loop (T116 precedent) converts
them into **T119** (robustness/tests, pri 4): the multibyte boundary
leg (2- and 4-byte scalars at the 120-char cut) + run/plan/chat
call-site wiring pins at the session level (~70 lines, tests-only,
bin-internal modules only — `--bin chug` check justified in-spec;
kimi optional per the T16/T31/T106 tests-only precedent).

### I4 — harvest-fidelity gap: 2 of T113's 7 validator findings are unrecoverable (assessed, NOT filed)

The T113 kimi verdict compressed its 7 non-blocking findings to "~5
named + all documented" — but the 2 unnamed ones lived only in the
validator's worktree scratch (its two attempts to write them to /tmp
hit the cwd sandbox; the worktree is harvested-and-removed). The 5
named findings are assessed below (§3, rejected individually); the 2
lost ones were non-blocking by definition, so the practical loss is
nil — but META-SPEC §6's verdict template already requires a numbered
findings list, and a verdict that says "documented" elsewhere is not
self-contained. Weighed and REJECTED as a doctrine row this eval (one
occurrence; the template text already exists; the fix is validator
behavior, not doctrine). Watch item: a second compressed-verdict loss
re-files it as a META-SPEC §6 self-containedness line.

### I5 — validator ceiling zone: fifth consecutive zone-free era-cycle (watch)

Cycle-61 validators ran 21/26/19/44 of 50 (T116 skipped) — zero
deaths, inside the T18 margin. The T32 sizing (50) holds; rejection of
the 50→65 raise stands for the fifth eval.

### I6 — post-commit death class: silent (third assessment, rejection stands)

Cycle 61 had ZERO impl-child budget deaths of any class (after
cycle-60's two post-commit deaths). T63's near-free recovery remains
the system's answer when the class fires; the T110 ceiling keeps
targeting the mid-impl class (its failure measure still UNMET — zero
mid-impl deaths at ≤500-line estimates; cycle-61 estimates overshot on
test+doc density: T113 ~455→+583, T115 ~130→+398, T116 ~30→+116 —
absorbed as calibration, not a defect).

## 3. Friction hot spots — fix assessment

- **decision_log schema fumbles (T88)**: 12 events files era-wide
  carry `invalid decision_log call` (8 `options` missing, 2 `class`,
  2 `choice`), all self-corrected within one iteration, zero lost
  records (309 landed). The error names the exact missing field; the
  per-fumble cost is ≤1 iteration. Weighed and REJECTED again with the
  era-wide count — a fuller-schema error message would save ~half an
  iteration per fumble, below the filing bar; the class is per-child
  (fresh glm contexts re-derive the schema), not per-orchestrator.
- **path-escapes-cwd (T41/T85)**: 6 fires across the t1xx streams
  (validators writing mutant scripts to /tmp; the error names the
  remedy "cross-tree paths go through bash"; all self-corrected).
  Working as designed.
- **120s bash-cap kills in impl children**: 1 fire each in t112/t113/
  t114/t116 impls (`timed out after Ns (process group killed)` —
  unbounded cargo invocations against the shared cache; all
  self-corrected with bounded retries). Baseline rate; the tool
  description carries the perl-alarm remedy (T22 era). Watch only.
- **stream_fallback organic fires**: ZERO era-wide (jq-verified over
  all events files) — the endpoint serves SSE; watch stands.
- **Anti-sprint-burn guard**: cycle-61 orchestrator's 16–29 delegate
  calls per stream carried all waits as terminal long-polls; no
  idle-iteration runs. Working.
- **T80 docs-only floor**: exercised ZERO times in cycle 61 (no
  md-only diffs among five landed items — T120 this cycle is the
  first candidate, and its spec already defaults to full gates as a
  pinned-doctrine carrier).
- **Site-sync / SSH pushurl**: green through the wrap (loopd.log);
  five per-item pushes + wrap push, zero friction.
- **T113's 5 named carried findings** (the validator's non-blocking
  set): silent dir-entry skip (fail-open by T83-hooks precedent; the
  unreadable-FILE leg gets an eprintln, the dir-ENTRY error leg
  silently `continue`s — exotic permission-race territory, below bar),
  append-leg newline-collapse (DELIBERATE — documented in
  commands.rs's `substitute` comment + pinned by the
  token_absent_with_args leg), mid-turn queue-vs-steering asymmetry
  (design-level; zero consumer pain in the corpus; carried as a watch
  note), self-referential legacy help assertion (test hygiene, below
  bar), ~455→+583 overshoot (calibration, §2 I6). ALL rejected
  individually with reasons; eval-triage records filed.

## 4. Capability gaps — ROADMAP PULL: F9 phase 2 → T117 + T118, SPLIT

Tier 1's deferrals stand (F13/F2/F3/F4/F5 — layad endpoint absent,
re-verified this eval; F5-p2's TUI surface has no loop consumer).
Tier 2: F6/F7/F8 phase-2 deferrals stand (F8-p2 note: phase 1 went
organic THIS cycle — §1 — but the observed pattern is add/update/list
as designed; zero organic demand for p2's remove/deps/chat-command
scope; the gate stays closed on evidence, reaffirmed not
re-litigated). **F9 phase 2 is OPEN and its sequencing dependency is
MET** — "sequenced after T115" (cycle-61 §4), T115 landed cycle 61,
verified live in this run's `run_start`. The top unworked roadmap item
is therefore **F9 phase 2**: run-side goal expansion, Tab completion,
frontmatter. SPLIT per the working rules (combined estimate ~540-620,
over the T110 ceiling): **phase 2a → T117** (run-side expansion —
`chug run|plan --goal "/name args"` expands at the CLI boundary,
`goal_sha256` hashes the EXPANDED text, `run_start` gains the
always-present `goal_pack`, unknown/empty packs are hard errors naming
the remedy, and the spec pins the integrity-surface honesty line: a
non-null `goal_pack` means the child's hash legitimately differs from
a parent's launch echo — expansion, not garble; ~280 lines; kimi
REQUIRED — eventlog.rs + driver.rs call sites). **Phase 2b → T118**
(frontmatter `description:` parse+strip, `/help` descriptions, Tab
completion of pack names via a pure merge helper — the chat-UX pair;
~260 lines; the deferrable remainder: chat-only surface, zero organic
requests in the corpus — an unworked carry is a fine outcome).
Demand honesty stands from cycle 61 (zero organic requests; the loop
consumer is named — run-side packs let the loop itself invoke
`/review`-class objectives; completion/frontmatter are the
benchmark-parity leg).

Beyond the pull: **no new roadmap appends this eval.** F10–F12 stand
as ordered.

## 5. Top 3 priorities

1. **T121** (loopd 160→200) — the only criterion-TRIPPED item: a real
   death consumed 1/3 of loopd's HALT budget, and this cycle's own
   shape (5-item eval cycle) is the recurrence case. Tiny (~25 lines),
   mechanical, activates automatically via loopd's self re-exec.
2. **T117** (F9 phase 2a run-side expansion) — the mandatory roadmap
   pull's main piece; features are first-class, and this is the leg
   with a loop consumer. Largest row (~280), sized under the ceiling
   by the split; closes F9's run/chat parity story with the T115
   surface it was sequenced against now live.
3. **T120** (verify-then-kill + assert-after-sed doctrine) — small
   (~55), lands the two self-inflicted-incident rules where
   orchestrators actually re-read them before the next
   suspected-garble moment; doctrine, runs alone.

(T119 pri 4 and T118 pri 3 fill the tail — see Handoff for the work
order; T118 is the declared deferrable remainder if budget binds.)

## 6. README audit (usability)

(a) **Reading order**: correct — Install → Quickstart → chat (+ packs
subsection) → run → forks → plan → TUI → Tools → risk gate → hooks →
permissions → MCP → Langfuse → specs → loopd → Development; the
newcomer path reads top-down without backtracking. (b) **Redundancy**:
the two sandbox exceptions are stated at the Tools intro and inside
each tool's paragraph — deliberate cross-reference, no drift (checked
both copies). (c) **Staleness**: none — the no-release-yet honesty
line is still true (`git tag -l 'v*'` empty; the bootstrap holds); the
streaming bullet's `stream_fallback` claim matches the jq-verified
zero-fire reality; the packs subsection's "Phase 2 (deferred…)"
paragraph is TRUE today and is rewritten by T117/T118's merge commits
(this row-pair's README surface, spec'd). (d) **Balance**: fine — the
T115 integrity-surface text landed at reference density in both the
delegate paragraph and the events bullet; no section carries
spec-grade detail. (e) **Quickstart truth**: commands work as written,
in order. **Fifth consecutive clean audit — no docs row.**

## Handoff

- **Work order** (with reasons): **T121** (robustness, pri 2 — the
  criterion-tripped cap; first) → **T117** (feature, pri 2 — the
  mandatory pull; strict class order would put it second anyway, so no
  deviation this cycle) → **T119** (robustness/tests, pri 4 — cheap,
  closes the T115 carry) → **T120** (doctrine, pri 4 — runs ALONE,
  never overlaps) → **T118** (feature, pri 3 — the declared deferrable
  remainder; an unworked carry is a fine outcome). Bundle check (T45
  conjunctive): T117/T118 are features (never bundled), T120 is
  doctrine (runs alone), T119 touches src test modules (not
  docs/pins-only in the T38-43 sense — wiring pins at session level),
  T121 has no trivial same-area companion — NO bundles. Overlap check
  (T44): T121's files (loopd.sh, tests/loopd_model_routing.rs) are
  DISJOINT from T117's (src/main.rs, src/commands.rs, src/eventlog.rs,
  src/driver.rs, src/plan.rs, README.md) — T117's impl MAY overlap
  T121's validator; T118 SHARES src/commands.rs + README.md with T117
  → serial after T117; T119 (src/driver/tests/, src/delegate/tests/,
  src/chat.rs tests) shares src surface with T117's call-site area →
  serial; T120 never overlaps.
- **SELF-SPEC**: none. **Human items**: (1) the FIRST `v*` tag stays
  the operator's (bootstrap holds — `git tag -l 'v*'` empty at filing;
  F7-p1 + F8-p1 + F9-p1 landed make it triply feature-worthy, and
  cutting it activates T100's wrap-time tag doctrine). (2) `gh auth
  refresh -s workflow` remains the proper fix for the workflow-scope
  class; the SSH pushurl workaround held all era (carried). (3)
  `com.tampajohn.chug-loopd.plist` stays untracked — operator's
  launchd unit (carried).
- **Watch items carried**: loopd cap (RESOLVED this eval → T121; the
  watch resets to: a ≥180-iteration cycle or a second death re-opens);
  validator ceiling zone (I5 — fifth zone-free cycle); display-artifact
  watch (post-T115 reset — the surface is LIVE as of this run; any
  FURTHER payload-level garble sighting is a finding against T115's
  fix, and any kill-before-verify is a finding against T120's rule
  once landed); compressed-verdict harvest fidelity (I4 — second loss
  re-files the META-SPEC §6 line); module sizes (api.rs 2,921 — 65% of
  the ~4,500 trip line, watch only); stream_fallback organic-fire
  watch (zero, jq-verified); T110 ceiling failure measure (UNMET —
  zero mid-impl deaths at ≤500 estimates; density-overshoot absorbed
  as calibration); post-commit death class (I6 — silent; rejection
  stands, third eval); T113 mid-turn queue-vs-steering asymmetry
  (design-level, carried); F13/F2/F3/F4/F5/F6/F7/F8 phase-2 deferrals
  (layad absent re-verified; F8-p2's adoption data point noted —
  §1/§4).
- **Weighed and REJECTED this eval** (eval-triage records in
  `.chug/decisions.jsonl`): loopd cap raise at the ≥150 proxy (filed
  instead on the death criterion — T121; the proxy rejections are
  history, not this eval's records); post-commit-death-class row (I6 —
  silent this cycle; T63 covers); validator budget 50→65 (I5 — fifth
  zone-free cycle); decision_log schema-fumble row (§3 — 12 files,
  ≤1-iteration self-corrections); path-escapes-cwd row (§3 — 6 fires,
  remedy-named, self-corrected); T113 carried-findings rows (§3 — five
  assessed individually: by-design, deliberate-and-pinned,
  design-level watch, test-hygiene below bar, calibration); F8 phase 2
  filing (§4 — adoption noted, p2-scope demand absent); new roadmap
  appends (§4 — none credible in the delta corpus); validator-verdict
  self-containedness doctrine (I4 — one occurrence; template exists);
  F9-p2 filed as ONE row (~540-620 estimate — split per the T110
  ceiling; the split IS the resolution).

## Outcomes (filled at cycle wrap — LOOP-SPEC Phase 3)

### Cycle 62 (2026-09-28) — fresh eval (kimi, loopd-routed: queue empty) — T117–T121 filed (F9-p2 SPLIT a/b → T117+T118; loopd-160 death → T121; SIGKILL/sed doctrine → T120; T115 weak-test carry → T119). Items in flight — per-item entries land with each row flip.


### Cycle 61 (2026-09-28) — ALL FIVE landed clean (T112 0325a36 streaming-usage fix live-verified, T113 55fe207 F9-p1 packs, T114 63119e6 check:-breadth doctrine, T115 ed6c96f goal-integrity surface, T116 eb55003 todo symmetry) — the era\'s second all-PASS cycle after 58. Cycle notes: kimi ran 4 rounds (3 REQUIRED + 1 optional-exercised T113, T116 skipped per precedent), 31 mutants total, zero blocking findings era-wide second cycle running; T44 overlap ran twice clean (T112-val||T113-impl, T115-val||T116-impl — disjoint file sets, strictly serial merges, clean rebase-ffs). ZERO impl-child budget deaths and zero T63 resumes (26/62/44/63/26 of 80 — the T102+T110 era\'s healthiest census). Display-artifact watch: ~11 render-only sightings in the ORCHESTRATOR\'s own stream (incl. one big duplicated-block render), every payload verified intact by transcript/disk read-back — plus ONE real self-inflicted incident: SIGKILLed a healthy T112 validator at 35s misreading a render garble (verify-then-kill must be SEQUENTIAL — read first, kill after; the kill was issued in the same breath as the check) and one anchor-typo silent-replace no-op caught by read-back (assert replacements). Validator zone watch 21/26/19/44 of 50 — inside T18 margin. Estimate calibration: T115 ~130→+398 (3.0x), T113 ~455→+583, T116 ~30→+116 — test+doc density beats estimates ~2-3x, ceiling\'s failure measure still unmet. T115\'s integrity surface activates NEXT cycle (loopd rebuilds before launch). Carried to next eval: T113\'s 7 + T114\'s 1 + T115\'s 2 non-blocking (multibyte-goal leg, run_start call-site wiring pin, append-leg newline-collapse deviation). Tag bootstrap holds (no v* — operator\'s first, now triply feature-worthy F7+F8+F9-p1). Untracked com.tampajohn.chug-loopd.plist appeared mid-cycle — operator\'s launchd plist, left untracked. Queue EMPTY -> next cycle eval-routes kimi. Final gates 924/924 + clippy + build at HEAD under target-shared-main.

- **T112 landed (0325a36, fast-forward)** — streaming path reported
  `input_tokens: 0` on every run: the accumulator's `message_delta` leg
  now merges the delta's `input_tokens` / cache fields over the
  `message_start` skeleton when present (delta wins; a no-op on the
  real-API shape, so all T108 parity pins pass unmodified). glm impl
  26/80 first-try with RED-proven mutation claim (merge disabled → the
  proxy-shape pin fails left:0 right:257); orchestrator review verified
  the diff (+121/-11, src/api.rs only), nextest 894/894 + clippy
  first-hand. kimi REQUIRED PASS 21/50: 6/6 mutants RED in two parallel
  batches of 3 throwaway worktrees (drop-loop, drop-input-field,
  drop-cache-read, drop-cache-creation, skeleton-wins,
  output-only-when-null — M6 also killed by the untouched T108 parity
  pin), 2 non-blocking observations (no dual-message_delta fixture; the
  +121 diff vs the ~80 estimate — test density again). Post-merge gates
  894/894 under target-shared-main. **Live smoke (the spec's acceptance
  leg): a fresh run of the post-fix binary reports `input_tokens` 4232 →
  4431 across iterations — the accounting is real again.** Incident of
  the arc: the orchestrator SIGKILLed a HEALTHY first validator (35s in)
  after misreading a RENDER-ONLY goal garble — transcript read-back
  proved the 2166-byte payload intact; verify-then-kill must be
  sequential (cycle notes). Harvested: 3 streams (impl, killed segment,
  clean validator) + validator ledger + 2 child decision records.
  Routing d1790589928-16 verdict d1790589928-17 outcome landed-clean.

- **T113 landed (55fe207, rebased-ff)** — F9 phase 1, slash-command
  packs (the cycle's mandatory roadmap pull): `src/commands.rs`
  discovers `.chug/commands/*.md` in the run cwd (fail-open, T83-hooks
  precedent; per-checkout by construction) and expands `$ARGUMENTS`;
  `chat.rs`'s `SlashCommand::Unknown` now carries `{name, args}`;
  `tui.rs` tries the packs on unknown slash lines (a hit sends the
  expanded body as a normal turn behind a one-line note; a miss names
  the available packs) and `/help` gains the pack line; built-ins win
  by construction (parser precedence — the goal.md shadow pin).
  glm impl 62/80 first-try; orchestrator review verified the diff
  (+583/−16 across 5 files) and re-ran gates first-hand (911/911 +
  clippy). kimi validation optional-EXERCISED (routing d1790591375-21 —
  feature on the message-flow path, none of the REQUIRED five): PASS
  26/50, 12/12 mutants RED in batches of 3 with slot-exclusive target
  dirs (sort-reversed, token-append-only, append-dropped, ext-filter,
  unknown-empty, corrupt-kept, empty-guard, help-wrong-count,
  note-format, unknown-suffix, args-dropped, goal-shadow), 7
  non-blocking observations carried (silent dir-entry skip; append-leg
  newline-collapse deviation; mid-turn queue-vs-steering asymmetry; one
  self-referential legacy help assertion; ~455→+583 estimate overshoot
  — the test-density class again; 2 more documented). Post-merge
  914/914 under target-shared-main. FEATURES.md F9 phase-1 checked off;
  phase 2 stays deferred per §4 (sequenced after T115). Harvested:
  impl + validator streams, both ledgers (impl's rotated), child
  decision records. Verdict d1790591988-22 outcome landed-clean.

- **T114 landed (63119e6, fast-forward)** — the cycle-60 wrap's
  handed-over check:-breadth weighing resolved filing-side:
  META-META-SPEC's spec bar gains the BREAK-side rule as a pure
  insertion between T96's ADD-side sentence and T110's
  estimate-ceiling sentence (both needles verbatim; the T111
  readme_layout escape + fc1d691 named as evidence; all pre-existing
  bar text byte-intact), with pin legs (i) exactly-once + (j)
  window/ordering beside T110's leg h in tests/loop_spec_recovery.rs.
  glm impl 44/80 first-try, RED-proven via three temporary mutations.
  Orchestrator review: insertion exact, 916/916 + greps + clippy
  first-hand. kimi REQUIRED (doctrine): PASS 19/50, five mutants
  correctly SERIAL (overlap declared — all touch META-META-SPEC.md):
  M1 delete → both greps + both legs RED, M2 duplicate → leg (i) only,
  M3 reorder past the estimate sentence → leg (j) only, M4 rewrap →
  check-line grep + legs RED, M5 out-of-window move → leg (j) only; 8
  pre-existing legs green through every mutant; 1 non-blocking
  (needle-pin idiom leaves the remedy/evidence prose unpinned —
  as-specced per req 3). Post-merge 916/916. The rule binds NEW
  filings from this point — the retro-sweep alternative was rejected
  at eval (d1790587936-7). Harvested: impl + validator streams,
  validator ledger, child decision records. Routing d1790593212-26
  verdict d1790593600-27 outcome landed-clean.

- **T115 landed (ed6c96f, fast-forward)** — the goal-integrity surface:
  `delegate` launch results gain `goal_bytes` / `goal_sha256` /
  `goal_tail` (computed over the exact goal string passed to the child
  argv; T25's chars()-based tail rule, <=120 chars, no ellipsis), and
  the child's `run_start` events line gains `goal_sha256` (hex when the
  mode has a goal — run + plan call sites pass `Some`; `null` in chat;
  field always present). One new dependency (`sha2`, justified for
  `shasum -a 256` cross-checkability); delegate tool schema frozen by a
  new pin; the 83-to-86 delegate-test count pin. glm impl 63/80
  first-try. Orchestrator review: diff +398/-17 across 10 files
  (~130-line estimate — the test+doc-density overshoot class again,
  3.0x this time), 922/922 + clippy first-hand. kimi REQUIRED
  (driver.rs + events substrate): PASS 44/50, 8 mutants in isolated
  worktrees cap-3 — 5 caught (map-none, sha-corrupt, tail-head,
  drop-sha-line, schema-prop), 3 survived -> **2 non-blocking weak-test
  findings carried**: (1) no multibyte-goal leg (a bytes-vs-chars
  mutant is invisible to ASCII-only fixtures); (2) `run_start`
  call-site wiring unpinned (driver `Some`->`None` and chat
  `None`->`Some` mutants survive the full suite). The validator closed
  the spec's live-acceptance leg itself: a real T115-binary delegate
  child's `run_start.goal_sha256` == the external `shasum -a 256` of
  the goal (transmission leg green). Post-merge 922/922. The surface
  activates for the NEXT cycle's launches (loopd rebuilds the release
  binary before each cycle — this cycle's own launches predate it).
  Harvested: impl + validator streams, validator ledger, child
  decision records. Routing d1790595612-31 verdict d1790596580-32
  outcome landed-clean.

- **T116 landed (eb55003, rebased-ff)** — the T111 validator's
  actionable carried finding, closed: `todo_update` now rejects
  empty/whitespace-only titles through ONE shared
  `ensure_title_non_empty` helper in src/todos.rs (trim-then-check +
  the error shape live together so the two paths cannot drift;
  `todo_add`'s error stays byte-identical — pinned; the update-side
  remedy names the rule; the check fires in the pre-lookup phase so
  failed calls never write). 7 new test legs. glm impl 26/80
  first-try. kimi SKIPPED (routing d1790596944-36 — src/todos.rs
  only, none of the REQUIRED five, pri-4 trivial, T16/T31/T106
  precedent, budget-low window). Review 918/918, post-rebase and
  post-merge 924/924. Harvested: impl stream + child decisions.
  Routing d1790596944-36 outcome landed-clean.

### Cycle 60 (2026-09-28) — fresh eval (kimi, loopd-routed: queue empty) — T110 + T111 filed (F8 SPLIT phase 1 → T111); BOTH landed (T110 5a16ce5, T111 d8fdea0+fc1d691)

**T111 F8 phase 1 structured todo tool — LANDED, fast-forward.** The
mandatory roadmap pull: `.chug/todos.json` (lazy creation, missing/empty
= empty list, corrupt = tool error naming file+remedy, never panic) +
`todo_add`/`todo_update`/`todo_list` (statuses enforced — invalid status
names the valid set, unknown id names the existing ids) + `## Todos`
system-prompt section after `## Ledger` in run+chat (absent when empty;
plan mode renders nothing, exclusion sweep legs 8→11 with file-existence
asserts) + README Tools integration beside `update_ledger`. glm impl
died 80/80 POST-COMMIT (the spec-mandated RED-prove legs consumed the
tail — both required legs RED-proven: status-enforcement and unknown-id)
→ T63 resume accepted 1/80 in 18s. **Review gates caught a REAL RED the
child's check: line could not see**: the readme_layout T95 pin
(`todos` missing from the Development layout line) — the spec's
`check: cargo test --bin chug` runs only bin unit tests, never the
tests/ integration binaries (the T90 filter-breadth class, filing-side);
orchestrator trivial fix fc1d691, suite 891/891 first-hand. kimi
REQUIRED validation PASS 30/50 first-try: 6/6 mutants RED in PARALLEL
throwaway worktrees (T79's first exercise this era — cap 3, role-keyed
target dirs, overlap declared safe): status-accept-any,
unknown-id-matches-any, corrupt-swallow, heading-always, id-off-by-one,
neither-field-accepted; gates + commit claims verified exact; 3
non-blocking findings carried to the next eval (prompt_text
corrupt-swallow is by design; no deny_unknown_fields; todo_update
accepts empty-string title while todo_add rejects). Validator logged
its own record (d1790586305-1). Routing d1790585650-14, verdict
d1790586370-15, recovery d1790585385-13, outcome landed-clean; 3 streams
(impl both segments + killed-validator + clean validator) + validator
ledger + 6 child decision records harvested. Orchestrator note: the
first validator launch carried a garbled goal (duplicated tail in the
transmitted payload — caught on send, killed at ~25s, relaunched clean);
3 further splice sightings this cycle were render-only, all payloads
verified intact by read-back.

**Cycle-60 wrap notes.** Both filed rows landed, zero fix-up arcs (two
first-try PASS validations — T110 16/50, T111 30/50; T111's is the
era's fourth consecutive zone-free validator). What the gates caught:
the cycle's one REAL RED was caught by the ORCHESTRATOR's review gates,
not the validator — the readme_layout T95 pin the child's spec
`check: cargo test --bin chug` could not see (bin-unit-tests-only never
runs the tests/ integration binaries; the T90 filter-breadth class,
filing-side — the next eval weighs whether src-touching specs' check:
lines should name the full suite or a broad stem). kimi's contribution
was 6/6 parallel-mutant rigor (T79's first era exercise) + 3
non-blocking findings carried to the next eval (prompt_text
corrupt-swallow by design; no deny_unknown_fields; todo_update/todo_add
empty-title asymmetry). T63 resume #24 (post-commit class, 18s — the
RED-prove tail consumed the child's last iterations; two of the last
three impl children died post-commit, a class T63 covers near-free).
Estimate calibration data point for the T110 ceiling: T111 was filed at
~400 and landed +932 (validator: the delta is test density) — the
ceiling watches ESTIMATES, so filing-time estimates must count tests
honestly; zero mid-impl deaths at ≤500-line estimates so far (the
ceiling's failure measure is unmet). Artifact watch: the cycle's one
REAL payload garble was in MY OWN delegate goal composition (duplicated
tail on the wire — caught on send, validator killed at ~25s, relaunched
clean); 4 further sightings were render-only, all payloads verified
intact by read-back (the T108-era discipline holds; the watch's
payload-level criterion recorded its second-ever sighting — first in an
ORCHESTRATOR-authored payload). Final state: queue EMPTY → next cycle
eval-routed kimi (loopd freshness rule); tag bootstrap holds (no v*
tag — with F7-p1 AND F8-p1 landed the operator's first tag is doubly
feature-worthy); final gates 891/891 + clippy at HEAD under
target-shared-main.

**T110 filing-time spec-size estimate ceiling — LANDED, fast-forward.**
The T102 measure clause RESOLVED: the census tripped in cycle 59 (2 of
the last 4 impl children died 80/80 — t108-impl mid-impl at 141 total
iterations, t108-fixup post-commit, both on the one ~700–900-line row
whose sizing lived in eval prose, not the spec) and the remedy is
filing-time, not runtime. META-META-SPEC's spec quality bar now REQUIRES
an `estimate: ~N changed lines` line in every spec and splits any row
estimated above ~500 lines of new/modified logic at filing time
(mechanical byte-identical move rows — T104/T109 class — exempt, the
estimate line says so); LOOP-SPEC's step-2 parenthetical keeps its T102
history and carries the resolution needle; FEATURES.md's stale "50-iter
child budget" shrink reference now names the ceiling. glm impl 31/80
first-try; pin leg (h) in tests/loop_spec_recovery.rs pins the needle
exactly-once inside step 2 AFTER the Measure census sentence (RED-proven
count 0→1). kimi REQUIRED validation PASS 16/50: 6/6 mutants killed (M1
delete re-proved the RED leg, M2 duplicate→count-2, M3
moved-before-Measure→ordering, M4a–c check-line grep breaks on
META-META/FEATURES), serial in-tree legs correctly declared
(runtime-read markdown → no mutant binaries), tree byte-clean, two
non-blocking observations. Orchestrator review + post-merge nextest
871/871 + clippy clean under target-shared / target-shared-main.
Routing d1790584043-10, verdict d1790584497-11, outcome landed-clean;
2 streams + validator ledger + 2 child decision records harvested. The
ceiling's own failure measure is written in the eval's watch list: if
the next two post-T110 evals file rows that die 80/80 mid-impl at
≤500-line estimates, the ceiling failed and sizing doctrine re-opens.

### Cycle 59 (2026-09-28) — fresh eval (kimi, loopd-routed: queue empty) — T107–T109 filed (F7 SPLIT phase 1 → T108); ALL THREE landed (T107 793a0fc, T108 53e4aed+2a51cc5, T109 75025c9)

**T109 delegate.rs test-module family split — LANDED, rebased
fast-forward.** The pre-emptive T104-shaped split at 94.6% of the
~4,500 trip line: delegate.rs 4,255 → 1,127 (production half
byte-identical + the one-line `mod tests;` declaration — orchestrator
diff-verified, not just child-claimed), the 3,127-line test module →
`src/delegate/tests/` (201-line shared harness + 10 family files:
summary 13, status 10, dispatch 4, wait 9, wait_terminal 7, schema 8,
launch 11, argv 2, parse 2, collect 17 = 83 tests + the req-4 count pin
whose mod-drop leg is RED-proven at compile time and whose
`include_str!` legs cross-check each family's pinned count against real
`#[test]` lines). glm impl 47/80 first-try — the first child launched
under T107's hardened template, and it committed from the worktree cwd
with main untouched: the clause's first live exercise held. kimi
SKIPPED per routing d1790580571-17 (T16/T31 precedent — orchestrator
byte-identity + count-equality review on a non-idle queue). Ran as the
T44 overlap impl against T108's validator (disjoint files, 1 validator +
1 impl cap); merges stayed serial — T108 landed first, T109 rebased
onto it conflict-free. Review + post-merge nextest 870/870 + clippy.
Outcome landed-clean; 1 stream + 1 child decision record harvested.

**T108 F7 phase 1 streaming + console text deltas — LANDED via
FAIL→fix-up→PASS arc (the full T100 shape).** glm impl died 80/80
mid-implementation (design settled, 5 files dirty, uncommitted) → T63
resume accepted at 61/80 (141 total — the T91-class big arc; first
post-raise 80/80 death). kimi REQUIRED round 1 (37/50): **FAIL** — 1
blocking: the stream_fallback latch re-fired per downgraded response
(the validator's own two-downgrade probe reproduced the req-4
first-per-run violation on shipped code at api.rs:2587 — all 5 spec-named
mutants died, yet the probe caught what the mutation menu missed: the
adversarial gate earning its keep) + 4 non-blocking (hook-clear leak on
the T91 `?` path, a vacuous cleared-hook test, cosmetic, observation);
live smoke by the validator: the configured endpoint SERVES SSE (no
fallback line, goal accepted — deltas live in delegate.log). glm fix-up
(died 80/80 POST-COMMIT → T63 resume accepted 1/80 — the T100-run1
shape; SECOND post-raise death → **the T102 >1-of-6 measure clause is
TRIPPED: the next eval weighs a spec-size cap**): 3-state FallbackLatch
(first-per-run, and unreachable on the CHUG_STREAM=0 leg), hook cleared
on all three driver exit paths incl. T91's `?`, the vacuous test made
live (arm→prove-live→clear→re-call), latch-cardinality sweep
(fallback / accumulator-error / console-prefix each pinned, T91's
images_degraded correctly out of scope). kimi round 2 (44/50):
**PASS** — six verify mutants all die (M-LATCH-REVERT proves the shipped
behavior goes RED; M-DRIVER-SKIP-CLEAR, M-HOOK-NO-CLEAR,
M-TAKE-CONSUMES-ALWAYS, M-ACC-ERROR-GUARD, M-PREFIX-CLOSE), clean-tree
gates 869/869, byte-clean. README gained the streaming bullet
(integrated into the driver-loop behavior list). Routing
d1790578544-15, verdicts d1790579546-16 (FAIL) / d1790581800-19 (PASS),
recoveries d1790577666-14 / d1790581072-18, outcome **fixed-up**; 4
streams + 3 ledgers + 3 child decision records harvested. One display
artifact: the r2 ledger carries a real U+FFFD pair (first payload-level
mangling sighting — cosmetic, no information loss; watch updated).

**T107 LOOP-SPEC child-goal worktree-discipline clause — LANDED,
fast-forward.** The cycle-58 I1 breach fix: the step-2 goal template's
`Commit your work here.` gains the explicit clause — commit ONLY from
the worktree cwd, cd back before committing, never run git add/commit
with the main repo as cwd — so every child from T108 on launches under
the hardened template. glm impl 31/80 first-try clean (clause after the
intact sentence, both check: needles verbatim, three pin legs in
tests/loop_spec_recovery.rs RED-proven pins-first 5-pass/2-FAIL). kimi
REQUIRED PASS 16/50: 6/6 mutants killed (M1 = the commit's own RED
proof independently reproduced; M4 proved the T64 loose-heading step-2
scope leg independently load-bearing), zero blocking findings, zero
survivors, tree byte-clean, serial overlap declared. Review gates +
post-merge nextest 845/845 + clippy under target-shared /
target-shared-main. Routing d1790576171-11, verdict d1790576657-12,
outcome landed-clean; 2 streams + validator ledger + 3 child decision
records harvested.

Cycle notes: **T102's measure clause TRIPPED** — 2 of 4 post-raise
impl children died at 80/80 with the work done (t108-impl
mid-implementation at 141 total, t108-fixup post-commit; both recovered
by T63 resumes, 61/80 and 1/80): the clause's window is >1 of the next
6, and it fired inside 4 — **the next eval weighs the ~500-line
spec-size cap** (the split-forcing estimate ceiling) per the clause's
own text, no further iteration raises. **The validators earned their
keep**: T108 round 1's SELF-ADDED two-downgrade probe caught the req-4
latch violation that all 5 spec-named mutants missed — the adversarial
gate's best moment this cycle; the fix-up's sweep closed the whole
latch-cardinality family in one round (cycle-33 doctrine held).
**T107's clause got its first live exercise same-cycle**: T109's impl —
the first child launched under the hardened template — committed from
its worktree cwd with main untouched (impl summary stated it
explicitly). **T44 overlap ran clean again**: T108-validator ‖
T109-impl (disjoint files), serial merges, conflict-free rebase-ff.
**Display-artifact watch**: ~six render-garble sightings in
orchestrator tool-call renders this cycle (delegate goals, edit_file
parameters, a commit message, a delegate launch spec) — every
transmitted payload verified INTACT on read-back; ONE real
payload-level artifact: the T108-r2 validator's ledger carries a U+FFFD
pair (cosmetic, no information loss) — the first payload sighting; the
watch continues. Validator ceiling watch: r2 wrapped at 44/50 —
budget_low fired, inside T18's margin, verdict delivered; no zone
breach. T81: eval-kimi leg = THIS cycle's Phase 1 (~25 min eval, cycle
wall ~2h at wrap — consistent with I6's closure). SSH pushurl held
(five pushes, zero friction). Tag doctrine: `git tag -l 'v*'` still
EMPTY — the bootstrap holds; the first tag stays the operator's (and
F7-p1's landing makes the first minor bump feature-worthy). Wrap gates:
nextest 870/870 + build + clippy under target-shared-main. Next cycle:
queue EMPTY → eval-routed kimi; candidates already queued: **the
T102-trip spec-size cap (MANDATORY weigh)**, F7 phase 2 TUI streaming
(deferred, §4), F13/F2/F3/F4 phase-2 deferrals, display-artifact watch
(payload sighting recorded), validator 50/50 watch, module sizes
(mcp_http.rs 2,793 now the largest module; delegate.rs CLOSED at 1,127
— T109).

### Cycle 58 (2026-09-28) — fresh eval (kimi, loopd-routed: queue empty) — T102–T106 filed (F6 SPLIT phase 1 → T105); ALL FIVE landed clean (T102 ffebdaf, T103 80d4a14, T105 dc29137, T104 ee3943e, T106 e5cdebd)

**T102 LOOP-SPEC impl-child template 65→80 — LANDED, fast-forward.** The
T92 measure clause fired (3 of 12 post-T92 impl children died 65/65 with
the work done — T91/T99/T100 run1s, totals 84/93/67; a rolling 6-window
over children 5–10 holds two deaths) and the doctrine took its written
next step: glm impl 20/65 first-try clean (three byte-exact LOOP-SPEC
edits + the pin at tests/loop_spec_recovery.rs:141 RED-proven first),
kimi REQUIRED PASS 18/50 zero blocking (M1 resume-budget revert KILLED
by the pin, M5 full-spec revert KILLED; M2 check-grep-only and M3
unpinned-prose survivors both by design). Review gates + post-merge
nextest 820/820 + clippy under target-shared / target-shared-main.
Every impl child from here launches at 80/35 (validators stay 50/30 —
zero validator deaths era-wide). New measure clause: >1 of the next 6
impl children dying at 80/80 with the work done → the next eval
considers a spec-size cap (~500-line estimate ceiling forcing a split)
instead of further iteration raises. Routing d1790570262-13, verdict
d1790570426-14, outcome landed-clean.

**T103 delegate launch asserts spec + cwd exist — LANDED, fast-forward.**
The 4th field-corruption sighting filed the row and the fix is live:
`delegate` launch now refuses a spec that is not an existing readable
file and a cwd that is not an existing directory, errors naming the
received path verbatim, no spawn, no `.chug/` created — the
corrupted-launch class (cycle-53 goal bleed, cycle-55 pid 60756, T95
wrong slug, cycle-57 duplicate key) now fails fast at the orchestrator's
face in one iteration instead of spawning a doomed child. glm impl
61/80 first-try clean (the first impl child on the post-T102 80 budget —
61 would have been in the old 65 zone). kimi optional-TAKEN (cycle-55
precedent — launch admission is loop-wide) PASS 31/50: 4/5 mutants
killed independently re-proving the RED legs (probe-drop, is_file-drop,
both message corruptions), M2 readability-leg survivor expected
(chmod-000 fixtures are flaky under root); ONE non-blocking finding —
an inverted rationale comment on the probe legs — fixed by the
orchestrator as a trivial comment commit (80d4a14) per §7's
fix-trivially prerogative. The child's disclosed spec-conflict
resolution (7 pre-existing launch tests with fictional spec paths each
gained one `ensure_spec_file` setup line, asserts byte-identical) was
verified against the diff and accepted — the spec's "happy-path
unmodified" bullet was literally unsatisfiable, a spec-writing lesson
carried. Routing d1790571195-16, verdict d1790571562-17, outcome
landed-clean.

**T105 F6 phase 1: session fork slots — LANDED (rebased ff).** The
mandatory roadmap pull landed: `chug fork save/list/restore` — named
slots over `.chug/transcript.jsonl` + `LEDGER.md` under
`.chug/sessions/<name>/`, giving the serial explore-two-approaches
shape (run A, save, keep going, restore, run B). The safety design is
the point: restore refuses a live driver lock (naming the pid, T55
interlock), rotates the live session aside with the EXISTING T7/T3
archive machinery before copying (a Failed rotation aborts the restore
— nothing is ever overwritten un-archived), and slots are copy-only
(idempotent restores). glm impl 43/80 first-try clean: fork.rs (656
lines) + main.rs CLI wiring + README `## Session forks` + the one-line
archive.rs seam; 18 fork tests with both dangerous legs (overwrite
refusal, lock check) RED-proven and the readme_layout guard RED-then-
green as the spec required. kimi SKIPPED (routing d1790571949-19): the
subcommand is isolated from the run loop (no driver/tools/dispatch
interaction) and has no loop consumer yet — the orchestrator's deep
review plus a live binary smoke of the full leg set (save → mutate →
list → restore → idempotent restore → overwrite refusal, all exit
codes correct) substituted for the optional round. Merge needed a
rebase onto main (T44 overlap with T103's flip — disjoint files,
conflict-free, dc29137). Post-merge 842/842 + clippy under
target-shared-main. FEATURES.md F6 carries the phase-1 check-off and
the phase-2 deferral reason. Child decision record d1790571815-1
harvested into main's decisions.jsonl. Outcome landed-clean.

**T104 driver.rs test-module family split — LANDED, fast-forward.**
The pre-declared trip line (~4,500) fired for the second time at 5,717
and the T84-shaped extraction ran: driver.rs is now **1,457 lines**
(production 1–1,452 + the `#[cfg(test)]` header + a one-line
`pub(crate) mod tests;`) and the 4,259-line test body lives in
`src/driver/tests/` as the shared harness (mod.rs, 236 lines) + 16
family files. glm impl 75/80 — **the first child that would have DIED
under the old 65 cap** (75 > 65 with the work done): T102's raise,
landed three hours earlier, paid for itself on its very first big
test. Orchestrator-verified byte-identity (production half + harness
header diffed; moved-body multiset zero-loss with 92 accounted glue
lines), 90/90 driver::tests, 710/710 bin, 842/842 + clippy,
readme_layout green UNMODIFIED (the T95 non-recursive guard absorbed
the submodule exactly as spec'd). kimi REQUIRED PASS 35/50:
byte-identity independently re-proved THREE ways (multiset + in-order
subsequence + count bijection), fn-token multiset 137==137 (no
renames), and three PARALLEL T79 mutation legs all killed — M1
mod-drop (842→837 count-pin), M2 assertion-flip RED from the new home,
M3 `pub(crate)`-strip → E0603 at trim.rs:210 (the T84 seam is
load-bearing). INCIDENT of the cycle: the impl child **committed its
work to MAIN** (cd'd to the main repo for byte-identity checks, then
committed from there — first worktree-discipline breach; ee3943e sat
unpushed on local main). Recovery: branch created at the commit, main
reset to the pushed state e2b7d1a, worktree re-pointed — the standard
arc then ran untouched. Goal-text hardening candidate carried to the
next eval ("Commit your work here" proved ambiguous once a child cd's
out). Routing d1790573602-21, verdict d1790574301-22, outcome
landed-clean.

**T106 README Install names the pending first release — LANDED (rebased
ff).** The cycle's trivial row: one honesty sentence in the Install
section's first paragraph ("Until the first tag is cut there is **no
release published yet** — the one-liner and tarball links below 404
until then; use the from-source install at the bottom of this
section.") — conditional wording true both before and after the first
tag lands, so no follow-up edit is needed when the operator cuts it.
glm impl 10/80; docs-only classification (exit 1) → guard floor
(todo_consistency 5/5) at review AND post-merge; kimi SKIPPED per the
T16/T31/T97 README precedent (routing d1790574440-24). Outcome
landed-clean.

**Cycle notes (wrap).** The full queue drained — 5/5 landed, every item
pushed at landing, ZERO fix-up arcs (the era's first all-PASS cycle:
kimi verdicts 18/31/35 of 50, all PASS; glm impls all first-try clean:
20, 61, 43, 75, 10 of 80). **T102's raise paid for itself the same
cycle**: T104's impl finished at 75/80 — a fourth 65/65 death avoided.
**T44 pipeline overlap ran twice, clean**: T103-validator‖T105-impl and
T104-validator‖T106-impl, both disjoint-file pairs, both merges
rebased fast-forward (conflict-free). INCIDENT (first sighting):
**T104's impl committed its work to MAIN** after cd'ing to the main
repo for byte-identity checks — recovered by branching the commit,
resetting local main to the pushed state (ee3943e never pushed), and
re-pointing the worktree; the next eval weighs a goal-text hardening
("commit ONLY in your worktree cwd" — "Commit your work here" proved
ambiguous). Display-artifact watch: FOUR render-garble sightings this
cycle (two validator-goal assemblies, two edit_file parameter renders)
— every transmitted payload verified INTACT in the transcript/file;
the read-back habit is the whole game and it held. T81 acceptance:
eval-kimi leg = THIS cycle, ~1h33m wall, ~150/160 projected at
goal_complete (consistent with the I6 closure — the 160 cap holds at
5 items + eval; no re-measure alarm). SSH pushurl held all cycle (six
pushes, zero auth friction); the gh-workflow-scope class is retired at
host level. Wrap gates: nextest 842/842 + build + clippy under
target-shared-main; README gate: T105's fork section + T106's Install
sentence (child-integrated) + a Sandbox/cwd clause for T103's launch
refusal (orchestrator-integrated at wrap). Tag doctrine: `git tag -l
'v*'` still EMPTY — bootstrap holds, the first tag stays the
operator's (cutting it also exercises T100's release workflow
end-to-end). Next cycle: queue EMPTY → eval-routed kimi; candidates
already queued: worktree-discipline goal hardening, T102 measure
clause census (4 children at 80, zero deaths — window forming),
validator 50/50 watch, T91 survivors, F13/F2/F3/F4 deferrals, module
sizes (driver.rs now 1,457 — watch closed), delegate.rs 4,255 (+257
this cycle — the split conversation moves closer).

### Cycle 57 (2026-09-28) — eval-routed kimi turned routine after reconciling cycle-56's origin divergence (T101 row appeared post-rebase, predicate held — cycle-56 precedent repeated); T101 LANDED (bcd0b66, fast-forward)

**T101 site-sync timeline ordering + curation bugs — LANDED, fast-forward
bcd0b66 (scripts/site-sync.sh +205/-65, tests/site_sync.rs +104).** The
2026-09-28 user report ("timeline looks out of order") named four bugs in
T99's TIMELINE generation, all fixed: (a) entries sorted by row order /
date-string desc — the page's day-one→latest design wants COMMIT TIME
ascending, so T99 (d13a253) rendered before T98 (8721c83) and a 09-26
entry after a wall of 09-27s; now every entry (curated + machine) sorts
by `%ct` ascending via `git show -s --format=%ct`; (b) the newest-20 cap
+ collapse never fired — it now does, pinned N=5 with 25 rows (curated
entries never collapse; the collapse line sits where the oldest collapsed
row sat); (c) raw done-row titles dumped AFTER the 16 curated entries —
now merged BY REF: a done row whose commit matches any curated hash span
disappears into the curated entry (prose wins, one entry not two), so the
"chug.sh — this site" crescendo anchors its commit-time slot instead of
being buried; (d) same-day entries order by %ct, not row id. Undatable
curated entries (site-side/foreign hashes like the launchd plist) sort
AFTER dated neighbors, never before. The cat-file facts-only audit (req
5) is unchanged; idempotence preserved (machine output carries no <p>, so
the next run re-classifies and rebuilds byte-identically).

Arc: glm impl 53/65 goal-accepted FIRST TRY (~13 min — no budget death,
no resume; the T92 65-iter headroom never came into play). Orchestrator
review gates: nextest 820/820 release + clippy + bash -n. Kimi validation
(optional-TAKEN, routing d1790566862-2: scripts are outside the REQUIRED
list but the user-reported live-site-visible bug plus T99's own
FAIL→fix-up history in this region earned the adversarial round) VERDICT:
PASS 30/50 (d1790568828-3) — all 5 reqs verified, clean-tree gates
re-run, 6 T79 parallel mutants: cap-flip / merge-drop / undatable-key /
sort-direction all KILLED by the round's tests; two survivors carried
non-blocking: M5 collapse-PLACEMENT (position of the collapse line is
unpinned by spec — cosmetic) and M6 %ct→%at (fixtures pin
author==committer time so the swap is invisible to the suite; the code
visibly uses %ct per spec — a fixture gap, not a code bug). Worktree
byte-clean post-mutation. 4 artifacts harvested (impl+validate events,
impl+validate LEDGERs). Acceptance leg (spec Tests §4): the live chug.sh
timeline reads chronologically after the next wrap's site-sync run —
recorded at wrap.

Cycle notes: (1) CYCLE ROUTING — eval-routed kimi turned routine exactly
as cycle 56 did: launch saw todo_rows=0 (loopd → kimi), the open-time
rebase onto origin 8a5b958 brought the operator's T101 row in, and the
predicate on the reconciled files (todo_rows=1, EVALUATION.md mtime
same-UTC-day) held → Phase 1 skipped, bugs-first per the cycle-56 wrap
directive (routing d1790566862-1). (2) PUSH INCIDENT (operator action
needed): the first push of the rebased T100 arc was REJECTED — the https
OAuth token lacks the `workflow` scope and the arc creates
.github/workflows/release.yml; the gh token lacks it too. Workaround used
all cycle: `git push git@github.com:tampajohn/chug.git main` (operator's
SSH key, unscoped). Permanent fix: `gh auth refresh -s workflow` or
`git config remote.origin.pushurl git@github.com:tampajohn/chug.git`.
Until then, any push touching .github/workflows fails over https.
(3) NEAR-MISS (4th sighting of the goal/spec field-corruption class,
first non-fatal): the validator's delegate launch carried a malformed
duplicate `spec` key (goal text fragment); the spawn resolved
last-key-wins to the correct spec path and the full goal arrived intact
(verified in the child transcript: 2213 chars, head+tail+export line
checked) — the delegate assertion candidate from cycle 55 stands.
(4) glm impl 53/65 FIRST-TRY — no ceiling death, no resume (no new
census point; the T92 65-iter headroom plus a genuinely scoped spec).
(5) Queue EMPTY at wrap → next cycle routes eval (kimi) by the predicate.
(6) Tag doctrine bootstrap still held: no v* tag exists — the operator's
first tag now also exercises T100's release workflow end-to-end.
(7) Acceptance: loopd runs site-sync.sh after this cycle's goal-complete
— the next cycle verifies the live chug.sh timeline reads
chronologically (site commit in the chug-site clone).

### Cycle 56 (2026-09-28) — eval-routed kimi turned routine after reconciling cycle-55's origin divergence (T100 row appeared post-rebase, predicate held); T100 LANDED (9d1182a, fast-forward)

**T100 GitHub releases: tag-triggered prebuilt binaries + generated notes —
LANDED, fast-forward 9d1182a (impl 67b7702 + fix-up 9d1182a, +2008/-12
across 12 files).** The operator directive ("start doing releases of chug
in github") ships the whole release surface: `.github/workflows/release.yml`
(v* tag push -> macos-14 arm64 + ubuntu-22.04 x86_64 + cross aarch64,
`--release --locked`, per-platform tar.gz + sha256, GH Release via the
preinstalled `gh --verify-tag` — no third-party release action — every
`uses:` pinned by live-fetched 40-hex SHA, `contents: write` only),
`scripts/check-tag-version.sh` (tag/Cargo.toml divergence fails the release
before any build), `scripts/release-notes.sh` (mechanical feat/fix/docs/chore
grouping of semantic commits since the previous tag, whole-history fallback
for the first release), `install.sh` (POSIX, OS-aware platform detect,
sha256-verify-before-install, ~/.local/bin, failures name the fix), a README
Install section (one-liner + per-platform tarballs + Gatekeeper note), and
the LOOP-SPEC Phase-3 tag-at-wrap doctrine (trigger >=3 items or a FEATURES
check-off since last tag, immutable tags, one per wrap, gates green,
BOOTSTRAP: first tag is operator-cut). Tests: workflow YAML-parse + SHA-pin
grep leg (no actionlint on host), 12 fixture-repo script legs, 15 install.sh
legs, 4 doctrine pins.

Arc: glm impl run1 committed the complete 67b7702 then died 65/65 BEFORE
goal_complete (census point 6 — the ceiling-death-after-commit pattern
again); T63 resume #21 accepted 2/65 in 47s after auditing the spec (21/21
all-time). Review gates independently green (nextest 811/811 release +
clippy + sh -n x3). Kimi r1 (32/50) FAIL — and earned it: the BLOCKING
finding was install.sh mapping arch before OS (`arm64|aarch64) arch=arm64`),
so every linux-aarch64 host collapsed to the nonexistent `linux-arm64`
asset and was rejected at the allowlist even though the workflow publishes
`chug-linux-aarch64.tar.gz`; the second finding proved the uname-mapping
had ZERO test coverage (tests only exercised the CHUG_INSTALL_PLATFORM
override) via a surviving platform-map-flip mutant. Glm fix-up (41/65):
joint `osname/mach` mapping (linux/aarch64 AND the linux/arm64 kernel alias
both -> aarch64), CHUG_INSTALL_OS/MACH overrides making the uname path
testable, 7 RED-proven killing tests sweeping every leg. Kimi r2 (35/50)
PASS: r1's survivor class dead to 4 tests, a fresh osname-swap mutant dead
to 6, live smokes on the real repo (tag-version match/diverge/shape, notes
grouping, file:// end-to-end install, dash -n). One non-blocking survivor
carried: amd64-alias-drop (a defensive alias no Linux host reports, outside
req 5's mapping set). Review + post-merge nextest 818/818 under
target-shared / target-shared-main. Validator finding 3 (site get-started
block + chug.sh/install.sh serving + verify.sh legs) was orchestrator wrap
scope by design — the impl children's goals excluded the live site repo
(T98/T99 precedent); done at this cycle's wrap. Cycle notes, carry-overs,
and the wrap-time record of the site edit land at wrap.

**Cycle notes (wrap).**
1. **Reconciliation at open (the cycle-55 carry)**: rebased the 4 unpushed
   cycle-55 commits onto origin f930df7 — one TODO.md conflict (kept
   T99-done + T100-todo; todo_consistency 5/5) — pushed f930df7..86e4982.
   The freshness predicate then held on the post-rebase tree (todo_rows=1
   + EVALUATION.md mtime today, verified with loopd's own functions), so
   Phase 1 was skipped per the cycle-55 precedent; no eval-triage records
   were owed. Launch routing (eval/kimi on the stale todo_rows=0) was the
   same artifact class cycle 55 recorded.
2. **THIRD mid-cycle origin divergence**: the operator's T101 (user report
   — site-sync timeline ordering + curated-merge + cap enforcement;
   8a5b958, row + ready spec) landed during the T100 arc and rejected the
   per-item push. Per the no-reconcile-mid-cycle rule the local T100 arc
   (67b7702, 9d1182a, 50189fd + this wrap) is UNPUSHED. NEXT CYCLE: rebase
   onto origin (TODO.md-only overlap — T101 row append vs the T100 row
   flip; resolve exactly as this cycle did: keep both), todo_consistency,
   push, then work T101 (pri 2 user-reported bug — bugs outrank everything
   in the priority order; ready spec on origin).
3. **Site-repo edit landed at wrap (validator F3 — orchestrator scope by
   design, the impl children's goals excluded the live site per the
   T98/T99 precedent)**: chug-site 8eee10b pushed live — get-started gains
   step 1 INSTALL (`curl -fsSL https://chug.sh/install.sh | sh`; existing
   steps renumbered 2-5, content verbatim), install.sh served at the site
   root (the post-fix 9d1182a content), verify.sh gains check 4 (one-liner
   present, script linked, served + sh -n clean) and an /install.sh href
   allow-leg mirroring /llms.txt; `bash verify.sh` all green. Untracked
   runtime droppings in the site clone (.chug/, LEDGER.md,
   scripts/gen_assets.py) left alone.
4. **Tag doctrine active, bootstrap held**: T100's LOOP-SPEC Phase-3
   tag-at-wrap bullet is live from this wrap, but no `v*` tag exists and
   the first tag is operator-cut — this wrap tags nothing. The trigger
   (>=3 items or a FEATURES check-off since the last tag) activates from
   the wrap after the operator's first tag lands.
5. **Display-artifact watch — 3 benign sightings this cycle**: the
   orchestrator's own tool-call parameter rendering glitched mid-stream
   (two delegate launches, two heredoc edits); in every case the
   TRANSMITTED payload verified intact (child transcript carried the
   complete goal; the decision record's subject field complete; the
   site edits verified by rereading). Same surface class as cycle-55's
   "goal/spec field corruption killed at spawn (3rd sighting, delegate
   assertion candidate)" — the next eval should judge whether a delegate
   argv-assertion row is due; this cycle's sightings were display-side
   only, no child impact.
6. **glm ceiling census, point 6**: the impl run1 died 65/65 AFTER
   committing complete work (resume #21 accepted 2/65 in 47s; 21/21
   all-time). The T92 measure rule: >1 of the next 6 impl children dying
   at 65/65 with the work done -> the next eval considers 80 or a
   work-splitting doctrine. This cycle: 1 of 2 impl-class children (the
   fix-up finished 41/65).
7. **Books**: decisions.jsonl — d1790563558-1 recovery, d1790563713-2
   routing, d1790564470-3 + d1790566001-4 verdicts, d1790566100-5 outcome
   fixed-up. 6 artifacts harvested pre-removal (4 event streams + 2
   verdict ledgers); worktree removed, branch deleted. Queue at wrap:
   T101 todo (on origin; joins local at the next cycle's rebase). Final
   gates: build + clippy -D warnings + nextest 818/818 under
   target-shared-main. Non-blocking carried: amd64-alias-drop mutant
   survivor (defensive alias no Linux host reports, outside req-5's
   mapping set).

### Cycle 55 (2026-09-28) — eval-routed kimi cycle turned routine (operator's T99 directive filled the empty queue at launch); T99 LANDED (d13a253, fast-forward)

**T99 site-sync v2: TIMELINE + FEATURES deterministic regions — LANDED,
fast-forward d13a253 (impl a284065 + fix-up d13a253, +897/-10).** The
operator directive ("keep the site up to date / reflective of the work /
timeline etc") extends T98's stats block with two more machine-owned
regions of chug.sh: TIMELINE (one fact-only entry per TODO done row —
date from the commit's git author date, title from the row, short ref —
deduped against the 16 hand-built entries by cited commit ref, verified
with `git cat-file` before render (req 5), newest-first, latest 20 full +
"…and N earlier milestones (T1–T<n>)" collapse, machine entries rebuilt
from scratch each run so re-runs re-collapse without dupes) and FEATURES
(FEATURES.md-derived grid: landed = checked-off row (strikethrough or
LANDED annotation, legs pinned independently), in-flight = an active TODO
row references the F-id (F1-vs-F13 / F2-vs-F21 boundaries pinned), else
queued; existing cards keep position + markup with badges normalized to
the machine vocabulary, uncovered F-items get synthesized cards, and NO
card is ever dropped — now true). Req-4 bootstrap: when the live page
lacks the markers, the script wraps the existing `<div class="tl">` /
`<div class="grid">` blocks' INNER content verbatim in its own commit
(pinned INSIDE the container — an outer-wrap mutant dies). glm impl run1
died **65/65 uncommitted** (+816/-10 — the 5th ceiling death of the T92
census); **T63 resume #20 accepted 28/65 (20/20 all-time)**. kimi
validation (routing: optional→RUN, idle queue + new bootstrap surface
outweighing the T98-skip precedent) paid immediately: **round 1 FAIL** —
blocking finding: `flush_card`'s `<p` gate silently dropped any card
without a `<p>` (h3-only/ul-body cards), contradicting req 2 and the
code's own "No card is ever dropped" comment (the orchestrator's named
probe, fixture-demonstrated by the validator); survivors M6 (bootstrap
containment unpinned) + M7 (LANDED-leg removal vacuous). glm fix-up
(28/65) swept the class with RED-proven pins (unconditional print,
containment pin, split-leg landed fixtures, F21-boundary test), and
**kimi round 2 PASS 42/50** re-ran every mutant to death (M6/M7a/M7b/M8
all DEAD, meta-legs prove the pins are load-bearing; fresh sweep clean
but one infinite-ladder non-blocker: the h3-less card shape is unpinned
— zero live exposure, all 14 live cards have h3). Orchestrator gates:
nextest 782/782 review + post-merge (main-dedicated cache), clippy,
bash -n. Live site clone untouched by every child (fixtures only) — the
first LIVE bootstrap fires at the next loopd wrap; NOTE for the
operator: the site clone currently holds uncommitted WIP (index.html,
verify.sh) and site-sync commits the whole index.html file, so that WIP
rides the bootstrap commit (T98 design property — the loop owns the
file's commits; hand-edit between syncs at your own risk). 8 artifacts
harvested pre-removal (5 event streams incl. the killed misfire, 3
LEDGERs).

**Cycle-level notes (wrap).** Launch routing was an artifact: loopd saw
`todo_rows=0` locally (cycle-54's divergence had left the T99 row
unpushed on origin) and launched an eval cycle on kimi; this cycle's
first act reconciled (clean rebase of the 3 local commits onto ed4476e,
todo_consistency 5/5, pushed ee8a508/eca240a/53cdb4c), after which the
freshness predicate HELD (queue=T99, eval same-day) so Phase 1 skipped
per doctrine — kimi evaluated nothing, per the skip rule (the glm-never-
evaluates clause never bound). **INCIDENT 1 (orchestrator):** the first
re-validation launch went out with its spec field corrupted by goal text
(pid 60756) — caught at spawn, killed at iteration 3 pre-work, relaunched
clean (pid 60933); same class as cycle-53's goal-corrupted validator
launch and T95's wrong-spec-slug misfire — three sightings now, all
caught at spawn; a launch-time spec-path existence assertion in delegate
would kill the class (candidate row for the next eval). **INCIDENT 2
(divergence, second cycle running):** origin moved mid-cycle again — the
operator pushed 7058143/bdb013e/3332873 (T100 GitHub releases: tag-
triggered binaries + chug.sh/install.sh + wrap-time self-tagging
override) while the T99 arc ran; the push of the row-flip commit was
rejected, so per the hard rule (never force-push, never reconcile
mid-cycle) local commits a284065 + d13a253 + d2207ea (+ this wrap) are
LANDED LOCALLY, UNPUSHED. **Next cycle's first act:** reconcile
(`git rebase origin/main` — overlap is TODO.md only: origin's T100-row
addition vs local T99-row flip are adjacent lines, auto-resolvable or a
one-hunk keep-both; run todo_consistency after, then push), then work
**T100 (pri, operator directive, ready spec — note 3332873's operator
override: the loop cuts its own immutable tags at wrap as the release
trigger)**. T98/T99 site acceptance: the next wrap's site-sync run fires
the FIRST live TIMELINE/FEATURES bootstrap (2 bootstrap commits + the
sync commit on chug.sh) — record it in that cycle's Outcomes (T99's
acceptance criterion), and remember the live clone's dirty WIP rides
along (flagged to the operator above). T92 measure: 2-of-6 census points
now (t91 + t99 run1 deaths at 65 with the work done — the impl-child
iteration ceiling still binds the big-feature class; if >1 of the next
4 impl children dies at 65/65, the next eval considers 80 or
work-splitting). Validators this cycle: 2/2 rounds returned verdicts
with full mutant evidence (1 FAIL with 1 blocking + 2 survivors → fix-up
→ 1 PASS with zero survivors but the infinite-ladder carry); the
optional→RUN routing call (against the T98-skip precedent) caught a real
spec violation — the idle-queue weighing is recorded at d1790558329-1
for the future classifier.

### Cycle 54 (2026-09-28) — routine glm freshness-skip: T96 META-META-SPEC check-filter-breadth (merge 457720d, mid-arc recovery per row recipe, kimi REQUIRED PASS zero findings) + T97 README delegate sub-bullets (b0c6041 fast-forward, flip eca240a, kimi SKIPPED per T16/T31) landed; T97 push-divergence incident (operator moved origin mid-cycle, reconciled next cycle per no-force-push rule) — full narrative in git: `git log --grep "T9[67]"`.

### Cycle 53 (2026-09-27) — six landed: T92 9db86bc (impl 50→65), T91 ae7ff5f (F5-p1 images), T93 aeb12ea (mcp__ canary), T94 06f3b9e (get_str diag), T95 e80b3c5 (readme_layout), T98 8721c83 (site-sync); T96 mid-arc→c54, T97→c54 — one-line compaction; full narrative in git (row-flip commits + TODO done rows)
### Cycle 52 (2026-09-27) — routine glm freshness-skip; T90 LANDED (e9afed9, fast-forward merge) — F4 permissions phase 1: the deny-only fail-closed policy layer

- **T90 LANDED** (e9afed9): `.chug/permissions.json` deny-list — new `src/permissions.rs` (363 prod lines, at the ~350 guard's edge) holds all policy logic: absent/empty config = zero rules + zero cost; malformed config fails OPEN (one stderr warn + one `permission_error` line, T83 parity); per-rule malformed legs (unknown key, two matchers, non-string value, missing tool, matcher-that-cannot-fit) are skipped in place with valid siblings still denying; deny rules are a tool glob + at most one `command`/`path`/`url` arg matcher, first-match-wins, missing/non-string arg under an arg rule fails toward execution; deny text `[permission denied] <rule summary>`. Driver gates dispatch FIRST (permissions → PreToolUse hooks → plan/MCP/risk gate) riding the T83 `blocked` flag in run+chat+plan, so a denied call fires no hooks and never executes. `hooks::glob_matches` made pub(crate), reused byte-identically. README Permissions section integrated after Hooks; FEATURES F4 row annotated at flip. 24 tests (17 unit + 7 driver integration incl. ZERO-hook-fires ordering pins + plan-mode leg + fail-open-exactly-once).
- **Arc shape**: glm impl run1 aborted 50/50 uncommitted (ceiling-zone, +511/−2 staged) → T63 resume #19 (d1790550858-1) ACCEPTED 10/50 in ~2 min — the resume continues the transcript and finished clean. kimi REQUIRED validation (driver.rs + events.rs on the §2 step-4 list; routing d1790551117-2) pid 83896 VERDICT PASS 0 blocking 38/50 (verdict d1790551730-3): gates independently re-run under target-shared-validate (build + clippy + 745/745 fallback-release + spec check 19/19), 6/6 mutants RED in 2 parallel T79 waves with role-keyed mut dirs. Review + post-merge gates 745/745 fallback-release (nextest absent per T82) + clippy under target-shared / target-shared-main.
- **Validator's 4 non-blocking findings (carried for next eval)**: (1) the load-time matcher-fit `mcp__` canary over-rejects server-specific globs like `mcp__fs__*` + path (fail-open, operator-visible — a skipped-rule warn, never a silent dead deny; phase-2 candidate: canary should accept any `mcp__*`-prefixed tool glob); (2) the spec-check filter `cargo test --bin chug permissions` misses the 5 strongest driver-integration legs (a mutant survived under the plural filter, killed under `permission` — spec-authoring lesson: check filters should be the broader stem); (3) the stderr warn-once latch is not test-observable (events line is, stderr isn't); (4) size at the guard's edge (363 vs ~350).
- Cycle notes: freshness-skip per predicate (1 todo row + same-day EVALUATION.md; loopd routed routine glm, d1790550858-1 era). Opened by landing cycle-51's wrap remainder (21a76a6: T90 not-started note + cycle-level notes + cycle-44 compaction) — the previous cycle died at goal_complete with its wrap uncommitted; the eval commit was durable but the final EVALUATION.md push wasn't. Queue is now EMPTY (T1–T90 all done): next cycle's freshness predicate will NOT hold on the queue half → eval cycle on kimi unless rows are filed. `com.tampajohn.chug-loopd.plist` remains untracked at repo root (operator's launchd unit — left alone, operator's call).

### Cycle 51 (2026-09-27) — fresh eval (kimi: queue empty) filed T88+T89+T90; T89 delegate terminal-wait + LOOP-SPEC adoption LANDED (merge d2b402a, impl 6e95df9 via T63 resume #18; kimi PASS) + T88 decision_log corrective validation errors LANDED (c5a4f9e). Verdict: 2/2 landed

### Cycle 50 (2026-09-27) — T85+T86+T87 docs bundle LANDED (merge 2440520: cross-tree bash escape-hatch doctrine on both review surfaces, README layout tgrep/plan/hooks, docs-only floor honest risk model + pin leg); cycle-49 mid-arc recovery executed via T63 resume #17; kimi PASS 0 blocking 18/50, 2/2 mutants RED. Verdict: 3/3 landed

### Cycle 49 (2026-09-27) — T83 hooks phase 1 LANDED (merge ccb828a, F3: .chug/hooks.json PreToolUse veto + PostToolUse advisory; 2 budget-aborts -> resumes #15/#16; kimi R1 FAIL PostToolUse-on-vetoed-calls -> class sweep incl. the risk-gate sibling leg -> R2 PASS 0 blocking); T85-87 bundle MID-ARC (landed cycle 50). Verdict: 1/1 landed after 2 resumes

### Cycle 48 (2026-09-27) — cycle-47 dead eval recovered (artifacts landed verbatim 89d1b5a + 11 triage records reconstructed); T84 trim.rs extraction LANDED (745f8ff, 981 moved lines byte-identical, kimi PASS 4/4 mutants); T83 MID-ARC (landed cycle 49). Verdict: recovery + 1/1 landed

### Cycle 46 (2026-09-27) — FIRST loopd-routed cycle (routine glm, predicate honored end-to-end); T82 nextest gates LANDED (6edc5ee — 2.5x faster measured post-merge) + T75 decision-records wrap-checklist LANDED (b4c159f), both first-round kimi PASS, 12/12 mutants RED; queue EMPTY. Verdict: 2/2 landed

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

