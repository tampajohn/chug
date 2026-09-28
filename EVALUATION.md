# EVALUATION — chug, assessed by chug-loop (2026-09-28, cycle 59)

**MANDATORY fresh eval** — the queue is EMPTY (T1–T106 all done with
refs; cycle 58's wrap landed the last row), so the freshness predicate's
queue half fails and loopd routed an eval cycle (`todo_rows=0 → eval
cycle on kimi-k3`, loopd.log 05:54:36Z). The roadmap pull is **F7
(Streaming UX) → T108, SPLIT**: phase 1 (transport streaming +
accumulation + console/headless text deltas) filed now; phase 2 (TUI
live typing) deferred with a written reason (§4). This is a DELTA eval
over cycle 58's eval: the new corpus is the T102–T106 arcs (FIVE landed
items — the 80-iter doctrine step, the delegate launch existence probes,
the driver.rs test-module split, the F6 fork-slot feature, the README
Install honesty sentence) plus the cycle-58 orchestrator stream, so
findings concentrate on the T104 worktree-discipline breach (→ T107),
the delegate.rs module-size trajectory (→ T109), and the watch items
cycle 58 owed a judgment (all assessed, §3).

Corpus: `.chug/eval-digest.md` FIRST (FRESH at eval start — regenerated
05:54:20Z, 80s after the newest pre-cycle stream; the only
post-generation events are THIS eval's own appends to the live
events.jsonl), the cycle-58 orchestrator stream
(`events-20260928-055436.jsonl`, kimi 148/160 goal-accepted, 1h33m,
1.3M in / 112.1k out — §2 I3), the eight t102–t106 child streams
(impls 20/61/75/43/10 + validators 18/31/35, ALL goal-accepted, zero
deaths), `.chug/loopd/loopd.log` (routing + site-sync lines
04:19Z–05:54Z), `TODO.md` (T1–T106 all done), `.chug/decisions.jsonl`
(227 records — was 199 at the cycle-58 eval), `src/` (driver.rs
**1,457** post-T104 + `src/driver/tests/` 17 files; delegate.rs
**4,255** — §2 I2; tree ≈36.2k lines), `README.md` (full cold read,
§6 — NO new findings), `FEATURES.md` (Tier 1 exhausted; F6-p1 landed;
§4), a live `chug fork save/list` smoke (T105 verified working — slot
created and removed), and the T104 breach record in the cycle-58
Outcomes entry.

## 1. What chug does well — be brief

- **Cycle 58 was the era's first ALL-PASS cycle**: 5/5 items landed
  clean, zero fix-up arcs, glm impls all first-try (20/61/75/43/10 of
  80), kimi verdicts 18/31/35 of 50 all PASS, every item pushed at
  landing. The pipeline has never run cleaner.
- **T102's raise paid for itself in THREE HOURS**: T104's impl finished
  at 75/80 — a fourth 65/65 death avoided on the first big test of the
  new budget. Doctrine amending itself on written measure clauses is
  the loop working as designed.
- **T44 pipeline overlap ran twice clean** (T103-validator‖T105-impl,
  T104-validator‖T106-impl), both disjoint-file pairs, both merges
  conflict-free rebase-ff. Serial-merge discipline held.
- **The corrective-error era keeps paying rent**: t105-impl's
  decision_log fumble (`options must be a string, got missing`) was
  caught by T88's corrective and self-corrected in one iteration;
  t103-impl's `/tmp` path-escapes refusal named the bash escape hatch
  (T41/T85) — one-iteration recovery, zero friction.
- **The T18 margin did its job on the cycle's biggest child**:
  t104-impl's budget_low fired at 8 remaining; it wrapped at 75/80 with
  the work committed.
- **Incident recovery was by the book**: the T104 main-commit breach
  (§2 I1) was recovered with branch-preserve + main-reset-to-pushed,
  never force-pushed, and the standard arc ran untouched.
- **T105's fork slots work live** — verified in THIS eval
  (`fork save` → slot with transcript+LEDGER sizes and goal preview;
  `fork list` correct; slot removed after).

## 2. Incidents worth fixing

### I1 — T104 impl child committed its work to MAIN → T107

First worktree-discipline breach in 58 cycles. The T104 impl child
(glm, 75/80, goal-accepted) cd'd from its worktree to the main checkout
to run byte-identity checks against main's tree, then ran
`git add` + `git commit` FROM THERE — `ee3943e` sat unpushed on local
main (cycle-58 Outcomes, T104 entry). Recovery: branch-at-commit, main
reset to the pushed state, worktree re-pointed; never pushed, never
force-pushed. Root cause is the step-2 goal template's commitment
clause — `Commit your work here.` (LOOP-SPEC.md:96) — which is
unambiguous only while the child STAYS in its worktree; a child that
cd's to main for legitimate read-only checks can resolve "here" to its
current directory, and nothing in the template re-anchors it. Fix: the
template gains the explicit clause — commit ONLY from the worktree cwd;
cd back before committing; never run git add/commit with the main repo
as cwd — plus a pin (tests/loop_spec_recovery.rs, the step-2-template
carrier). Doctrine → runs ALONE, kimi REQUIRED. Filed as **T107**;
landing it FIRST gives T108/T109's children the hardened template.

### I2 — delegate.rs is the largest module at 4,255 lines, 74% tests → T109

delegate.rs: 4,255 lines (+257 this cycle — T103's probes + tests),
production half 1,125 lines, test module 3,129 lines / 83 `#[test]`
fns (delegate.rs:1126→EOF). The T104 shape exactly: healthy production,
accreting test module (T23/T28/T29/T39/T58/T68/T69/T89/T103 families).
The ~4,500 trip-line convention (T84/T104) has NOT fired (94.6%), but
+257/cycle growth crosses within 1–2 cycles and every delegate-touching
row already pages the file three reads deep (t103-impl, 61/80). The
cycle-58 wrap queued it: "the split conversation moves closer." Weighed:
waiting for the trip line respects the convention but buys nothing —
the split is provably safe (T104 landed the playbook 3 hours ago:
byte-identical moves, count pins, harness-in-mod.rs), it gets bigger
with every delegate row, and it fills this cycle's T44 overlap slot
against T108's validator (disjoint files). Filed PRE-EMPTIVELY as
**T109** (pri 3, kimi optional — delegate.rs is not on the REQUIRED
list); the defer-to-trip-line alternative is rejection-logged.

### I3 — cycle-58 orchestrator at 148/160: the I6 closure holds (assessed, NOT filed)

Cycle 58 ran 148/160 with FIVE item arcs + eval aboard (T36's designed
load was eval + 3). I6's closure criterion — "re-file 160→200 if a
cycle scrapes ≥150 with recovery pressure" — is NOT tripped (148 < 150,
zero recovery arcs, goal-accepted with the wrap complete). Second
consecutive post-closure data point that the cap holds. No row; the
watch stands (a ≥150 scrape re-opens it).

### I4 — orchestrator `git add .chug/…` refusal (assessed, NOT filed)

Cycle 58's orchestrator tried to `git add` a harvested `.chug/` path;
gitignore refused (exit 1, stderr hint shown); self-corrected same
iteration (harvests are local-only by design — `.chug/` is gitignored
runtime state). One sighting, benign, self-corrected. Baseline fumble
rate — no row (rejection logged).

### I5 — validator ceiling zone: zero readings this cycle (watch)

Validators ran 18/50, 31/50, 35/50 — the whole cycle nowhere near the
zone (previous era: 16–48 with zero deaths). The T32 sizing (50) holds.
Watch stands: a validator 50/50 death with the verdict unwritten
re-files this as the T21-class validator step.

## 3. Friction hot spots — fix assessment

- **path-escapes-cwd (T41/T85)**: one fire (t103-impl's `/tmp`
  commit-msg write) — the error named the bash escape hatch, child
  self-corrected in one iteration. Machinery working, not friction.
- **decision_log schema fumbles (T88)**: one fire (t105-impl
  `options must be a string, got missing`) — the corrective named the
  fumble, self-corrected in one iteration. Working.
- **output_truncated advisory (T38)**: 2 fires (t103-impl, t105-impl) —
  both chunked their writes and completed first-try. Working.
- **macOS `timeout` mirage (T22)**: zero sightings in the era's streams.
- **driver.lock CONFLICT**: zero sightings — single-driver discipline
  held (loopd.log routing lines confirm clean handoffs all era).
- **T102 measure-clause census**: 4 post-raise impl children at 80
  (61/75/43/10) + t102 itself at 20/65 — ZERO deaths. The window is
  forming: >1 of the next 6 dying at 80/80 with the work done re-opens
  the spec-size-cap consideration written into the clause.
- **T80 docs-only floor**: T106 exercised it correctly (exit-1
  classification → guard floor at review AND post-merge, full suite
  skipped) — the shrink works on a true md-only diff.
- **Display-artifact watch**: 4 sightings cycle 58 (two validator-goal
  assemblies, two edit_file renders) — every transmitted payload
  verified INTACT in the transcript/file. The read-back habit is the
  whole game and it held. Watch stands.
- **T91 survivors (retry-once, 5 MiB boundary, case-fold)**: zero
  organic image reads this era (the image leg is landed-but-unused in
  anger) — re-weighed: no new evidence, rejection stands; the first
  organic image read re-opens the weighing.
- **Site-sync acceptance**: post-cycle syncs green all era (627c2f9 →
  edfb609; items 103/103, tests 842, cycles 40 — loopd.log). The
  T98/T99/T101 trio is done-done.
- **SSH pushurl**: held all cycle (six pushes, zero auth friction); the
  gh-workflow-scope class stays retired at host level.
- **edit_file old-not-found**: one sighting in the cycle-58
  orchestrator stream (TODO.md) — self-corrected baseline rate. No row.

## 4. Capability gaps — ROADMAP PULL: F7 (Streaming UX) → T108, SPLIT

Tier 1 is exhausted (F1, F13-p1, F2-p1, F3-p1, F4-p1, F5-p1 landed);
F6-p1 landed cycle 58 (dc29137) — Tier 2's top unworked item is **F7
(Streaming UX)**: "Text deltas to sinks as they arrive (TUI live
typing, headless progress); watchdog gets byte-level liveness for
free." SPLIT per the working rules: **phase 1 → T108** — `stream:true`
transport leg + SSE accumulation to an identical Response (usage
equality pinned — T15 budget enforcement depends on it) +
`Event::ModelTextDelta` console surface + `CHUG_STREAM=0` kill switch +
non-SSE fallback with bounded telemetry. The enablers are all in-repo:
`src/sse.rs`'s chunk-boundary-safe `SseParser` (mcp_http uses it
today), the T2 watchdog's caller-thread receive loop (the chunk hook
needs no cross-thread machinery), and the `Event::ModelText`
console-only precedent (events.jsonl stays clean). **Phase 2 DEFERRED
with written reason**: TUI live typing / progressive tool-input
rendering rides the tui.rs redraw architecture — a chat-only surface
with no loop consumer; the loop's liveness need (headless children) is
fully met by phase 1's console surface on `delegate.log`. Demand
honest: zero organic streaming requests in the corpus — filed on
capability-gap doctrine per the T37 precedent, but the loop consumer is
concrete and measured: t104-impl's run emitted 74.7k output tokens
across generations that each leave `.chug/delegate.log` silent for
minutes; the T98/T99 big-feature era hit the 8,192 output-token
ceiling 6× (T38 fires) on single generations. With phase 1, the
orchestrator's `delegate status` log tail becomes liveness evidence.
FEATURES.md F7 gains the SPLIT annotation from this eval; the phase-1
check-off lands in T108's row-flip commit.

Beyond the pull: **no new roadmap appends this eval.** F8–F12 stand;
the F13/F2/F3/F4 phase-2 deferrals stand (layad endpoint absent;
written reasons carried) — reaffirmed, not re-litigated.

## 5. Top 3 priorities

1. **T107** (goal-template worktree discipline) — the era's only
   discipline breach; doctrine row, runs ALONE, and landing it FIRST
   gives T108/T109's impl children the hardened template.
2. **T108** (F7 phase 1 streaming) — the mandatory roadmap pull;
   biggest row of the queue, sized for the 80-iter budget (T91-class:
   ~700–900 lines with tests); kimi REQUIRED (api.rs + driver.rs +
   events.rs).
3. **T109** (delegate.rs test split) — mechanical, T104-shaped,
   pre-emptive; fills the T44 overlap slot against T108's validator
   (disjoint files).

## 6. README audit (usability)

(a) **Reading order**: correct — Install → Quickstart → chat → run →
Session forks (T105's section, correctly placed) → plan → TUI → Tools →
Risk gate → Hooks → Permissions → MCP → Langfuse → specs → loopd →
Development. (b) **Redundancy**: none with drift — the
`cargo install --path .` line appears in Install's from-source block AND
Quickstart; both copies verified identical this read (signposting, not
duplication). (c) **Staleness**: no bite — the Development layout line
is guard-pinned (T95 set-equality green; `sse`/`fork`/`trim` all
present); T106's Install honesty sentence names the pending first
release with wording that stays true after the tag lands. (d)
**Balance**: fine — the run-mode bullets are dense but reference-grade;
config detail sits in Hooks/Permissions; the delegate sub-bullets
(T97) read clean. (e) **Quickstart truth**: the build/run/chat/fork
commands work as written — the fork legs verified live in THIS eval;
the Install one-liner 404s today BY DESIGN and says so (T106). No docs
row this cycle — second consecutive clean audit.

## Handoff

- **Work order** (bugs > robustness > features > DX > docs): **T107**
  (doctrine — runs ALONE, kimi REQUIRED; first so later children get
  the hardened template) → **T108** (feature — api.rs + driver.rs +
  events.rs + sinks; kimi REQUIRED) → **T109** (delegate.rs test split
  — kimi OPTIONAL per T16/T31; orchestrator byte-identity +
  count-equality review substitutes on a non-idle queue). Bundle check
  (T45 conjunctive): T107 is doctrine (never bundled), T108 is a
  feature (never bundled), T109 alone is bundle-eligible but solitary —
  no bundles. Overlap check (T44): T107 doctrine → alone; T108
  (api.rs/driver.rs/events.rs/eventlog.rs/tui.rs) is DISJOINT from
  T109 (delegate.rs only) → T109's impl MAY overlap T108's validator;
  merges strictly serial in queue order (T108 before T109).
- **SELF-SPEC**: none. **Human items**: (1) the FIRST `v*` tag stays
  the operator's — cutting it exercises T100's release workflow
  end-to-end and activates the wrap-time tag doctrine (bootstrap
  holds: `git tag -l 'v*'` empty at filing). (2) `gh auth refresh -s
  workflow` remains the proper fix for the workflow-scope class; the
  SSH pushurl workaround held all era (retired at host level, operator
  informed). (3) `com.tampajohn.chug-loopd.plist` stays untracked —
  operator's launchd unit (carried).
- **Watch items carried**: validator ceiling zone (I5 — re-file on a
  50/50 validator death); T102 measure-clause census (4 of ~6 window
  readings, zero deaths); display-artifact watch (payloads intact every
  sighting — the read-back habit stands); T91's 3 informational
  survivors (no new evidence); F13/F2/F3/F4 phase-2 deferrals (layad
  endpoint absent); module sizes (driver.rs CLOSED at 1,457;
  delegate.rs → T109 filed; tgrep.rs 2,531 / mcp_http.rs 2,793 /
  permissions.rs 766 stable — rejection stands); loopd 160 cap (I3 —
  second consecutive sub-150 reading, closure holds).
- **Weighed and REJECTED this eval** (eval-triage records in
  `.chug/decisions.jsonl`): validator budget 50→65 (I5 — zero deaths,
  zone-free cycle); loopd cap 160→200 (I3 — 148/160 trips neither the
  ≥150 trigger nor recovery pressure); defer-T109-to-trip-line
  (recorded with the filed row — the convention's mandate attaches at
  crossing, but the growth rate + fresh playbook + overlap-slot fit
  outweigh one cycle of earliness); a `git add .chug` guard row (I4 —
  baseline self-corrected fumble); F8-before-F7 reorder (no dependency
  or measured incident — F7 is top-of-tier); TUI-first F7 phase
  ordering (the loop consumer is the console surface — phase order
  stands); new roadmap appends (none credible in the delta corpus).

## Outcomes (filled at cycle wrap — LOOP-SPEC Phase 3)

### Cycle 59 (2026-09-28) — fresh eval (kimi, loopd-routed: queue empty) — T107–T109 filed (F7 SPLIT phase 1 → T108); wrap entry fills landed refs

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

### Cycle 54 (2026-09-28) — routine glm freshness-skip (queue carried T96 mid-arc + T97)

**T96 META-META-SPEC check-filter-breadth — LANDED, merge 457720d (impl 962830d).**
Cycle-53's mid-arc recovery executed per the row recipe: preserved worktree
verified clean at 962830d, `delegate collect` first look (goal-accepted
23/65), one-hunk +7/-1 diff reviewed — the check-filter-breadth sentence
woven after the `--lib` sentence in the Extend-TODO quality bar, both
needles carried (the t90 bite: `permissions` filter missed
driver-integration legs the `permission` stem caught, mutant survived
under the spec's own check). Docs-only classification exit 1 → guard
floor (todo_consistency 5/5) + spec check verbatim green. kimi REQUIRED
(doctrine) **VERDICT: PASS 16/50, zero findings** — gates re-run
independently (build + clippy + nextest 777/777), check proven RED
pre-edit, 3 parallel T79 mutants all killed (drop principle needle, drop
t90 needle, corrupt the preserved `no library targets` token — pin proven
live). 2 validate artifacts harvested pre-removal.

**T97 README delegate paragraph → per-action sub-bullets — LANDED,
fast-forward b0c6041.** glm impl (pid 394) goal-accepted **37/65
first-try (~4 min)**: the ~24-line `delegate` paragraph split into a
lead-in + four ` — ` sub-bullets (`launch`, `status` with wait_secs +
terminal folded in, `collect`, `Sandbox/cwd`) — zero behavior-text
change, token-multiset-verified, neighbors byte-identical. The
spec-anticipated pin sweep found exactly one delegate-paragraph pin
outside tests/: tools::tests::readme_and_loop_spec_name_collect (T69) —
needle updated minimally (gains the ` — ` label separator) and PROVEN RED
both directions (against pre-edit README and pre-update needle).
Classification consequence: the pin update puts src/tools.rs in the diff
→ md-only predicate fails → FULL gates at review and post-merge (nextest
release 777/777 both, spec check + nextest_gate_runner 7/7 +
shared_target_dir 22/22). kimi SKIPPED per routing (docs-only restructure
+ orchestrator claim-for-claim diff review, T16/T31/T95 precedent). 1
artifact harvested pre-removal.

**Cycle-level notes (wrap).** Phase 1 skipped per the freshness predicate
(2 todo rows + EVALUATION.md same-UTC-day) — routine glm cycle, zero
eval-triage records by design. Both carried rows landed; validators 1/1
REQUIRED round PASS (zero findings, 3/3 mutants). **INCIDENT: push
divergence at wrap.** The operator pushed ed4476e (T99 site-sync v2
directive: row + specs/t99-site-sync-content.md, TODO.md's T97 row still
`todo` in that tree) to origin/main while this cycle ran; local main
advanced to 89952ba/b0c6041 (T97 done + Outcomes). Push of 89952ba was
rejected — per the hard rule NO force-push and NO mid-cycle
reconciliation: local commits b0c6041 + 89952ba (+ this wrap commit) are
LANDED LOCALLY, UNPUSHED. Next cycle's first act: reconcile with
origin/main (merge — expect a small TODO.md conflict: origin adds the T99
row after T97's now-done row), then work T99 (pri 3, operator directive,
ready spec) with the T96/T97 pattern. T96's lesson held: the cycle-53
mid-arc recipe executed end-to-end with zero re-work — resume-from-row
notes are worth their tokens.

### Cycle 53 (2026-09-27) — fresh eval (kimi, loopd-routed: queue empty) — T91–T97 filed (F5 SPLIT phase 1 → T91); T92 LANDED (9db86bc) — impl children now get 65 iterations

**T92 LOOP-SPEC impl-child template 50→65 — LANDED, fast-forward merge 9db86bc.**
glm impl (pid 2631) goal-accepted **19/50 in ~96 s** — the cleanest arc in
the queue's history (one hunk per surface, zero fumbles): step-2 template
`max_iters:   65` (three-space surface; validator 50/30 untouched),
rationale parenthetical replaced with the new census (T83/T85/T89/T90 +
T84 at 48/50) CARRYING the written measure clause (>1 of the next 6
impl children dying at 65/65 → next eval considers 80 or work-splitting),
T63 resume sentence → `65/35 impl, 50/30 validate`. Pin
tests/loop_spec_recovery.rs:141 updated + RED-proven (pre-edit LOOP-SPEC →
leg_names_mechanics_and_scope_guards_inside_step_2 FAILED, restored 4/4);
sweep found no other casualty (eval_digest.rs `max_iters":50` are
event-stream fixtures, untouched). Review: diff byte-exact vs the spec's
named surface, 745/745 fallback-release + clippy under target-shared, spec
check verbatim. kimi REQUIRED validation (doctrine; pid 8451, **16/50,
~2.5 min**) VERDICT: PASS 0 blocking — gates independently re-run
(745/745 + clippy + check-line verbatim), M-A resume-sentence-revert RED
via the updated pin, M-B/M-C informational guard-boundary notes (template
number check-line-guarded; measure clause review-enforced — per spec
design), tree byte-clean. Routing d1790552895-13, verdict d1790552895-14.
Post-merge: **nextest 745/745 in 10.4 s** + clippy under
target-shared-main — **cargo-nextest is back on PATH**
(`/Users/jadams/.cargo/bin/cargo-nextest`; uninstalled ~22:16Z during
cycle 51, reinstalled by the operator before this cycle — the T82
nextest-first runner rule is live again; this cycle's earlier review gate
ran the fallback before the reinstall was noticed, 745/745 green both
ways). 3 artifacts harvested pre-removal (impl + validate streams, child
decisions). **Effect: every impl child launched after this merge gets 65
iterations — T91's feature arc is the first beneficiary.** Arc shape: 1
impl, 0 resumes, 0 fix-ups, 1 validator round, ~7 min wall end-to-end —
the T88-class shape, now the template for what a well-specced small row
costs post-T89 (terminal waits: 2 orchestrator iterations for two child
runs).

**T93 permissions `mcp__` canary prefix leg — LANDED, fast-forward merge aeb12ea.**
glm impl (pid 15046) goal-accepted **23/65 in ~2.5 min** — the FIRST child
launched at the post-T92 budget (not that it needed it). One-condition fix
exactly per spec: `matcher_fits` gains `tool_glob.starts_with("mcp__") ||
glob_matches(...)` so a server-scoped glob like `mcp__fs__*` + an arg
matcher loads VALID (the canary string could never match it) and denies
`mcp__fs__read prod.env` while allowing `ok.txt`; non-prefixed globs ride
the canary leg unchanged, builtin fit-rejections (`command` on
`read_file`, `bash`+`path`) byte-identical. **RED proven test-first**: the
new test ran against UNFIXED code and failed with the exact skip line
(`path matcher does not fit tool glob mcp__fs__*`, 22/22 others green),
then passed post-fix — the child also filed its own decision records
(d1790553165-1 test-first, d1790553167-2 accept). Review: diff is
src/permissions.rs ONLY (+38/−2), nextest **746/746** (745+1) + clippy
under target-shared, spec check `cargo test --bin chug permission` **23/23**
— the broader stem catching the driver legs T90's plural filter missed
(the T96 lesson practiced a cycle before its own doctrine lands). kimi
SKIPPED per routing d1790553020-16 (permissions.rs not on the REQUIRED
list; T16/T31 precedent). Post-merge: nextest 746/746 10.7s + clippy
under target-shared-main. 2 artifacts harvested pre-removal (impl stream,
child decisions). Arc: 1 impl, 0 resumes, 0 fix-ups, 0 validator rounds,
~5 min wall — back-to-back clean smalls after T92.

**T91 F5 phase 1: `read_file` image input — LANDED, merge ae7ff5f (the
mandatory roadmap pull).** glm impl run1 (pid 22596) died **65/65** with the
broad diff uncommitted (+1012 lines across 14 files — the ToolResult.images
field rippled constructor sites in decisions/delegate/mcp/mcp_http/plan/
tgrep/webfetch mechanically) — the FIRST death at the new cap, T92's
measure clause now at 1-of-6 — and **T63 resume #20** (pid 25547) accepted
**19/65 in ~2.7 min** (commit e39bbb5). What landed: extension map
(png/jpg/jpeg/gif/webp, lowercased so X.PNG works) reads bytes; 5 MiB guard
errors naming cap + actual size with zero base64; `ToolResult.images:
Vec<ImageBlock>` default-empty channel with the text note
`[image: <path> (<n> bytes, <media>)]` — previews/events ride text only
(base64-can-never-leak pinned <100 chars); `KnownBlock::Image` serde
round-trip + `tool_result_block_with_images` array content (image first,
then text) with the string path byte-identical pinned; driver degrade leg —
400 whose body mentions image/content (`api::is_image_rejection`) → ONE
retry with every image block (standalone + inside tool_result arrays)
replaced by placeholder text, one `image_degraded` events line, per-run
latch downgrading later images at WRAP time; plan mode rides the shared
wrap site (array-shape test); resolve_safe gates before any read;
hand-rolled RFC-4648 base64 (vectors pinned, no new dep); README ## Tools
image clause integrated. Review: nextest **762/762** (746+16) + clippy +
spec check 16/16 under target-shared, four load-bearing hunks
orchestrator-read (degrade condition, read_image guard, base64, wrap).
**kimi REQUIRED validation** (FOUR REQUIRED-listed files; pid 31218,
29/50, ~14 min) VERDICT: PASS 0 blocking — gates independently re-run,
check-filter run list verified = exactly the 16 new tests (the T96 lesson
practiced by a validator in the same cycle it was filed), 10 mutants in
isolated T79 worktrees: **7 killed** incl. all 3 spec RED proofs +
latch-drop + any-400-widening + base64-padding + placeholder-array-skip;
**3 survived** = weak-test gaps on correct code, carried informational:
(1) retry-ONCE double-rejection unpinned, (2) exactly-5MiB boundary
unpinned, (3) rejection-body case-fold unpinned — weighed, NOT filed
(informational; the legs they pin are spec-satisfying; a future eval can
re-weigh). Routing d1790553963-19, verdict d1790554748-20. **Orchestrator
incident, mine**: the first validator launch (pid 31078) carried a
goal-text duplication corruption of MY making; I killed it pre-work
(SIGKILL after SIGTERM lag, defunct-reaped), verified the worktree
byte-clean, relaunched — the relaunch carried the same cosmetic
duplication (content complete, no contradictions) and validated fine.
4 artifacts harvested pre-removal (impl run1+resume stream, killed-attempt
stream, validator stream, child decisions). Post-merge: nextest 762/762
10.7s + clippy under target-shared-main. FEATURES.md F5 → **phase 1
LANDED** annotation (phase 2 chat-paste deferral carried). Arc: 2 impl
runs (1 resume), 1 validator round + 1 orchestrator-caused relaunch,
~35 min wall. **F5 phase 1 closes the loop's last Claude-Code-class
sensory gap: children can now READ screenshots and image fixtures.**

**T94 `get_str` received-keys diagnosis — LANDED, fast-forward merge 06f3b9e.**
glm impl (pid 44123) goal-accepted **17/65 in ~2.5 min** — the third
first-try-clean small of the cycle. `src/tools.rs` ONLY (+178/−1): one
shared `received_hint` helper (object → sorted keys capped at 12 +
`, … (+N more)`; empty → `(received keys: none)`; non-object → JSON type
per the T88 req-3 shape) behind `get_str`'s one-line `ok_or_else`; the
exact t88 fumble now answers `missing or non-string field: old (received
keys: new_string, old_string, path)` — one-iteration self-correction.
Success path byte-identical (edit_file round-trip pin + every pre-existing
tools.rs test green unmodified). RED proof: revert fails 6/7 new tests,
success-path correctly stays green. Review: nextest **769/769** (762+7) +
clippy + spec check 9/9 (7 new + 2 decisions.rs received-stemmed riding
the filter) under target-shared. kimi REQUIRED validation (tools.rs; pid
49446, 20/50) VERDICT: PASS 0 blocking — gates independently re-run,
mutations serial (overlap declared), **5/6 killed** incl. the exact
RED-claim reproduction; the sort-drop survivor is the vacuous-mutation
class (BTreeMap iterates sorted — dropping the sort is unobservable, code-
level explanation accepted). Routing d1790555093-21, verdict
d1790555295-22. Post-merge: nextest 769/769 + clippy under
target-shared-main. 3 artifacts harvested (impl + validate streams, child
decisions — the impl stream landed under its raw rotated name mid-harvest
and was renamed to convention by hand). Arc: 1 impl, 1 validator round,
~7 min wall.

**T98 site-sync (operator directive, filed mid-cycle b6787ed) — LANDED, merge 8721c83.**
glm impl (pid 54557) goal-accepted **38/65 in ~7 min** under a goal-level
CRITICAL CONSTRAINT (live clone never touched — fixture-only testing; the
child verified the live HEAD 61a991d unchanged with 0 markers).
`scripts/site-sync.sh` (+243): regenerates ONLY the `<!-- STATS:BEGIN/END
-->` region of the site's index.html from repo facts — TODO.md done-row
count, newest full-suite gate count in git log (nextest > suite-unit >
bare-unit priority; filtered runs like `16/16` match nothing BY DESIGN so
the card cites a full-suite gate or walks back), loopd cycle-OK count,
EVALUATION.md git date, last-5 landed `tNN:` items with refs+dates
(HTML-escaped, non-item commits excluded). No clock in the block →
unchanged inputs byte-identical, no commit (idempotence cmp-pinned;
CHUG_SYNC_NOW for tests). Safety legs per req 2/5: missing clone / non-git
/ failed commit / **rejected push** all warn + exit 0; plain `git push`
only, never force; marker pair absent/malformed → exit 3 WITHOUT editing
(the child resolved the spec's internal tension — req-1's "add markers if
absent" parenthetical vs the Tests section's "absent markers → nonzero
WITHOUT editing" — toward never-inject; site-side marker insertion is
one-time setup). `loopd.sh` +6: one failure-tolerant invocation after each
`cycle OK`. 7 fixture integration tests (tests/site_sync.rs): contents
match sources, byte-identical rerun, marker legs, push rejection,
regeneration on new inputs. Review: nextest **776/776** (769+7) + clippy +
bash -n under target-shared; post-merge 776/776 under target-shared-main.
kimi SKIPPED per routing d1790555816-24 (zero src/ files; trust boundary
weighed — every safety leg is fixture-test-pinned incl. dead-remote
rejection; failure mode bounded to the operator's own site, hand-fixable,
never a cycle failure). 1 artifact harvested. **Bootstrap carried to
wrap**: the live site has 0 markers — orchestrator inserts the one-time
marker pair at cycle-53 wrap so the NEXT loopd cycle runs the first real
sync (acceptance: a later cycle's Outcomes records the site commit).

**T95 README layout + module set-equality guard — LANDED, fast-forward e80b3c5.**
glm impl (pid 69319) goal-accepted **26/65 in ~2.5 min** — after an
orchestrator misfire (I launched the first attempt with a hallucinated
spec slug `t95-transcript-dir-pin.md`; the child failed at spawn with
spec-not-found, I read the row, relaunched with the correct
`t95-readme-layout-guard.md` — the spawn-failure-as-error-signal caught
my own bookkeeping slip in one iteration, ironically the T94 thesis).
README one-word insertion (`permissions` between delegate and plan — the
list is historical-order not sorted, the child placed it where neighbors
keep alpha order) + `tests/readme_layout.rs`: a std-only set-equality
guard (newline-tolerant braces-list parse vs src/*.rs-minus-main walk,
runtime `current_dir` per T48, non-vacuity pins BOTH sides, failure names
both drift directions + the T86 class pointer) — the T86-class SECOND
sighting earned automation per eval §6(c). RED proofs recorded both
directions (revert → names ["permissions"]; phantom → names ["phantom"]).
Review: nextest **777/777** (776+1) + clippy + guard 1/1 under
target-shared; post-merge 777/777. kimi SKIPPED per routing
d1790556692-25 (the guard is its own adversarial artifact). 1 artifact
harvested. Arc: 1 misfired launch + 1 impl, ~5 min wall.

**T96 META-META-SPEC check-filter-breadth — MID-ARC at wrap (impl committed, validation deferred).**
glm impl (pid 78784) accepted 23/65, commit 962830d on branch loop-t96:
ONE sentence woven into the spec-quality bar after the T67 `--lib`
sentence (cargo-test check filter MUST run every test the change adds;
t90 citation), +7/−1 with every other line cmp-proven byte-identical,
todo_consistency 5/5. The merge is HELD: step-4 kimi validation is
REQUIRED for loop/spec doctrine and the budget-low directive forbade new
child launches. Worktree /tmp/chug-loop-t96 + branch PRESERVED; events
harvested; full recovery recipe on the TODO row. **T97 carried** (docs-only,
ready spec). Cycle-53 wrap: 5 items landed (T92/T93/T91/T94/T98/T95 — six
counting the operator-filed T98), one mid-arc (T96), one carried (T97);
T92 measure clause at 1-of-6 (T91's 65/65 death, resume-recovered);
validators caught 0 blocking findings all cycle (T91: 7/10 mutants killed
+ 3 informational survivors carried not filed; T94: 5/6 + expected
BTreeMap survivor); decision_log adoption 25 records this cycle;
orchestrator incidents (mine, both recovered in-iteration): killed
goal-corrupted validator launch pid 31078; wrong-spec-slug T95 misfire
caught by spawn failure. Site-marker bootstrap for T98 done at wrap
(markers committed to chug-site; first real sync is the next loopd
cycle's — acceptance leg still open).

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

