# EVALUATION — chug, assessed by chug-loop (2026-09-27, cycle 51)

**MANDATORY fresh eval** — cycle 50 drained the queue (T1–T87 all done with
refs), so the freshness-skip rule cannot fire; loopd's routing probe agreed
at launch (`todo_rows=0 → eval cycle on kimi-k3`, loopd.log 22:17:02Z). The
roadmap pull is **F4 (Permissions policy) → T90** — F1 landed T69, F13
phase 1 landed T70, F2 phase 1 landed T73, F3 phase 1 landed T83 (cycle 49;
all three phase-2s deferred with written reasons carried). This cycle is
also the **T81 acceptance leg's eval-kimi half** — the cycle-47 eval that
was supposed to record it DIED (§2 I1) and cycles 48–50 were glm routine
legs; the wall-time/quality record for THIS eval lands in Outcomes at wrap.

Corpus: `.chug/eval-digest.md` FIRST (FRESH at eval start — regenerated
22:16:51Z, 104 s after the newest pre-cycle stream; the only post-generation
events are THIS eval's own appends), then the orchestrator streams since the
cycle-47 eval: cycle 47 (the died-at-44 eval itself,
`events-20260927-201956.jsonl`), cycle 48 (`...-210537.jsonl`, glm 157/160),
cycle 49 (`...-215944.jsonl`, glm 158/160), cycle 50
(`...-221702.jsonl` segment, glm 78/160), plus the cycle-44/45/46 streams
the dead eval never narrated (`...-183054/-192331/-200748.jsonl`), the 9
child streams t83–t87 (impl/validate/fixup/resume segments),
`.chug/loopd/loopd.log` (the 19:23Z + 20:07Z T50 re-execs, six routing
lines), `TODO.md` (T1–T87 all done), `.chug/decisions.jsonl` (131 records —
was 100 at the cycle-47 eval), `src/` (31,890 lines; driver.rs 4,898
post-T84; hooks.rs 977 new; delegate.rs 3,498), `README.md` (cold read,
§6 — zero findings).

## 1. What chug does well

- **T81's routing is the quiet triumph of the era.** Four consecutive glm
  routine cycles (46, 48, 49, 50) + this kimi eval, ALL launched by loopd's
  mechanical freshness predicate, zero model judgment. The
  glm-never-evaluates boundary HELD under stress: cycle 48 recovered the
  dead cycle-47 eval by landing its artifacts verbatim and reconstructing
  its records — explicitly honoring "glm never runs this phase" rather than
  re-evaluating (EVALUATION.md cycle-48 Outcomes).
- **Cross-cycle recovery is now routine machinery.** Cycle 49 budget-wrapped
  with the T85-bundle impl in flight; cycle 50 found it, read its handoff
  LEDGER, T63-resumed it (#17, accepted 20/50), validated, merged, flipped,
  pushed — a mechanical recipe execution end-to-end. T63 resumes: **17/17
  all-time.**
- **The adversarial pipeline caught a REAL semantic bug, not another weak
  test.** T83 kimi R1 FAIL: PostToolUse fired on PreToolUse-vetoed calls
  (tool never executed; phantom advisory + phantom fire line). The fix-up
  swept the CLASS (the sibling risk-gate-block path had the same hole —
  T72 doctrine) with 2 RED-proven killing tests; R2 PASS 0 blocking. This
  is exactly the bug shape the pipeline exists for.
- **decision_log corpus is compounding.** 131 records (51 outcome, 22
  eval-triage, 22 validation-verdict, 19 validation-routing, 16
  recovery-routing, 1 overlap-routing) vs 100 at the cycle-47 eval — incl.
  cycle-48's 11-record reconstruction of the dead eval (the recovery recipe
  proving the corpus is reconstructable from git + eval text when the
  worst happens).
- **T45 bundling handled a doctrine bundle honestly.** The T85+T86+T87
  round: ONE impl child (3 commits in queue order), ONE kimi round, and the
  T80 md-only classification correctly did NOT fire (T87 carried a `.rs`
  pin file → full gates, 704/704 nextest-release) — the ambiguity-default
  and pin-carrier rules practiced exactly as written.
- **T84's extraction held.** driver.rs is 4,898 (5,488 → 4,509 by T84;
  +~390 of T83 hooks integration since). trim.rs carries 1,007; the ~4,500
  trip line stays uncrossed-with-extraction; no new monolith alarm.
  delegate.rs (3,498) and tgrep.rs (2,527) are the next-largest modules —
  watch, no trip line declared.

## 2. Incidents worth fixing

**I1 — decision_log one-field-at-a-time validation errors are not
corrective; the class turned FATAL → T88 (pri 2, robustness).** Nine
schema-fumble sightings across BOTH model families and every role:
t74-impl ×2 (`choice`, `options`), t85-impl ×1 (`options`), cycle-50
orchestrator ×1 (`options`) — all self-corrected — and the FATAL one: the
**cycle-47 eval itself** (kimi, `events-20260927-201956.jsonl`) announced
"Logging the eval-triage records (5 filed + 4 weighed-and-rejected)" then
made FIVE consecutive identical `missing or non-string field: class`
errors → stuck tripwire abort at 44/160, evaluation written but
uncommitted, zero records logged. Cycle 48 landed the artifacts verbatim
(`89d1b5a`) and reconstructed 11 records. The cycle-47 eval had filed the
glm fumbles as "minor, no row" HOURS before the same class killed it —
the new data flips that call. The contract stays ONE-record-per-call
(batch ids + outcome subjects depend on it; batching WEIGHED AND REJECTED);
the fix is the error text: list EVERY bad field at once, name the received
top-level keys when none match (the batched/aliased shape), name the JSON
type for non-object input, and append the full required list. Success path
byte-identical. Spec: `specs/t88-decision-log-corrective-errors.md`.

**I2 — the orchestrator's delegate-poll loop is the loop's dominant
iteration tax, and the 160 cap is now binding → T89 (pri 2, robustness +
doctrine).** Measured: cycle 48 = **116 delegate calls of 157 iterations**,
cycle 49 = **123 of 158** (~75%). Each poll is one orchestrator iteration =
one full-context LLM round trip (265–470k cumulative input late-cycle).
T29/T68 collapsed the WALL cost of waiting; the ITERATION cost is
structural: the significant-change wake set includes `last_iteration`, so
even `wait_secs: 600` wakes on EVERY child iteration advance (~15–90 s for
glm) — waiting on a 40-iteration child costs ~40 orchestrator iterations.
Ceiling pressure at the T36 cap (120→160, cycle 16): cycle 44 ran 158/160
(budget_low fired), **cycle 45 died 160/160 with goal never claimed**
(wrap lost; cycle 46 reconstructed the T81/T82 mid-arc state), cycles 48
and 49 both 157–158/160 with budget_low-forced wraps that left children in
flight. Cycle 50's 78/160 was a pure mechanical recovery — the exception
that proves the tax is work-proportional. Fix: opt-in `terminal` wait mode
(wake only on goal/abort/liveness-flip/events-creation/deadline — the four
facts an orchestrator acts on) + LOOP-SPEC adoption as the default wait
posture. A 40-iteration child then costs ~3–5 orchestrator iterations
(launch + terminal waits + collect), not ~40. Spec:
`specs/t89-delegate-terminal-wait.md`. **Doctrine item — runs alone.**

**I3 — T83 grinder: 136 child iterations across three runs for one feature
item → no row, spec-size guard adopted.** hooks phase 1 (977-line
hooks.rs + driver integration + 24 tests) blew TWO 50-iteration budgets
uncommitted before resume #16 accepted at 36/50; the row recipe sanctioned
the second resume (T63's cap is ONE — the deviation worked and is recorded,
but the cap's text and the practice now diverge). Root cause is upstream:
the spec was ~2.5 child-budgets of work. Adopted fix (not a row): feature
specs now carry an explicit size guard with a split seam — T90's spec has
one ("≤ ~350 production lines or split"). Watch: if the next feature spec
with a size guard still grinds, file the doctrine row then.

**Minor — macOS `timeout` mirage, 3rd sighting (t84-validate, kimi) —
rejected, no row.** T22's bash-description note holds severity at ~1
iteration self-correct; incidence is 3 all-time across both families.

**Minor — BSD/GNU grep dialect** (`repetition-operator operand invalid`,
t84-validate ×1, self-corrected) — first sighting, below the bar. Watch.

**Carried rejection — path-escapes-cwd friction: T85 LANDED, measurement
window opens NOW.** t83-validate1 hit the refusal ×3 (21:16Z) writing
T79 mutant-leg scripts — but that validator RAN BEFORE T85's doctrine
landed (22:10Z, cycle 50). Every post-T85 stream is the measurement; one
cycle of evidence is owed before judging the fix.

**Carried rejections re-affirmed (eval-triage records logged):**
longer-API-retry-for-outages (stands from cycle 47 — the halt is the
pager); cross-tree read-tools release (stands — pinned contract, doctrine
fix landed instead); RED-proof-mandate-in-every-spec (stands — ceiling-zone
headroom is the binding child budget); transcript-archive forensics gap
(operator prunes `transcript-*.jsonl` by doctrine — "transcript harvest is
the operator's choice (size)"; the events stream + console log carried
enough to diagnose I1 anyway); hooks micro-polish (PostToolUse
signal-death `(exit none)` asymmetry; chat per-turn config reload —
documented behavior, polish below the bar).

**Weighed-and-rejected THIS eval — raise loopd `--max-iters` 160→200.**
The T36 pattern says raise when scrapes accumulate, and we have 4 scrapes
+ 1 death. But the same doctrine fixed BURN before raising CAPS (T27
followed T13/T18; T36 followed T29/T68): T89 attacks the burn directly and
should cut routine cycles to well under the cap; the T18 budget_low margin
+ preserve-worktree + row-recipe recovery absorbed 4 of 5 scrapes with zero
lost work (the cycle-45 wrap loss was reconstructed in minutes). Re-measure
next eval: if post-T89 cycles still scrape, the raise is due.
(eval-triage record logged.)

## 3. Friction hot spots

- **decision_log schema fumbles** — GRADUATED to T88 (§2 I1): from
  cycle-47's "minor watch" to the fatal-incident class in one cycle.
- **delegate polling** — GRADUATED to T89 (§2 I2): the T68 watch closes
  superseded; wake-on-advance IS the tax.
- **edit_file `old` not-found** — 5 sightings (t83-impl ×3:
  README/driver.rs/hooks.rs; cycle-48 orchestrator ×2: EVALUATION.md), all
  self-corrected at ~1 iter each via re-read. T5's disambiguation holds;
  steady frequency, below the bar. Watch.
- **path-escapes-cwd** — §2 carried: 3 sightings, all pre-T85; measuring
  from this cycle.
- **Prior fixes assessed, not re-filed:** T74 body_watchdog — zero
  sightings since landing; T31 parallel-load family — quiet; T79 parallel
  mutants — exercised again (t85-validate M1/M2 in 2 throwaway worktrees);
  T80 md-only gate slimming — STILL UNEXERCISED (the T85 bundle carried
  T87's `.rs` pin file → full gates correctly; no pure-md round has
  occurred since T80 landed — third cycle carrying this telemetry leg);
  T82 nextest — every gate this era ran nextest-first (~9 s vs ~22 s
  fallback), zero red-only families.

## 4. Capability gaps — ROADMAP PULL (required)

**PULL: F4 → T90 (pri 2, feature), SPLIT per the FEATURES.md working
rules.** F4 (Permissions policy) is the top unworked Tier-1 item. Phase 1
(this row, `specs/t90-permissions-deny-list.md`): `.chug/permissions.json`
deny-list — in-process per-tool rules (`{"tool": glob}` + optional
`command`/`path`/`url` arg glob) evaluated BEFORE the T83 hook seam; a
match denies with the veto-shaped `[permission denied] …` tool error the
model routes around; config problems fail OPEN (warn-once + one
`permission_error` line, T83 parity); a rule match fails CLOSED (there is
no spawn to fail — the differentiator from hooks); policy order
permissions → hooks → risk gate established; run + chat + plan surfaces
(plan can only be restricted further — e.g. deny `read_file *.key` inside a
plan session); events telemetry per the T83 two-line pattern. **Phase 2
DEFERRED with written reasons:** allow-rules short-circuiting the risk
gate (the F4 "one policy source among several" leg — needs risk-gate
plumbing of its own), ask-mode (chat interactive prompt — TUI surface
item), settings.json unification, `--permissions` CLI flag. The size guard
from I3 is in the spec.

**FEATURES.md bookkeeping (this eval commit):** F3's row gains its
phase-1 annotation (`T83 LANDED ccb828a, cycle 49; phase 2 deferred`) —
the check-off was missed at cycle-49's merge (orchestrator duty per
META-META-SPEC §4; the evaluator repairs the roadmap surface it owns).

**New finds beyond the roadmap:** none appended. T89 is loop-internal
iteration economics (a benchmark-neutral capability on an existing tool);
T88 is robustness. Neither is a benchmark capability gap.

## 5. Top 3 priorities

1. **T88 decision_log corrective errors** (robustness; the FATAL class) —
   smallest diff of the three, error-text-only on failure legs, kills the
   stuck-abort-by-malformed-bookkeeping class at the corrective surface.
2. **T89 delegate terminal-wait** (robustness + doctrine) — the loop's
   iteration economics; expected to lift the 160-cap pressure without
   raising it (re-measure next eval).
3. **T90 F4 permissions deny-list** (roadmap pull, feature) — the
   fail-closed policy layer; REQUIRED kimi validation (driver.rs dispatch
   + events.rs).

## 6. README audit (usability, not just accuracy)

Cold read, top to bottom, as a newcomer. (a) **Reading order** — quickstart
→ chat → run → plan → TUI → tools → risk gate → **hooks** → MCP → Langfuse
→ self-hosting → continuous → development: T83's Hooks section landed IN
ORDER between risk gate and MCP (policy surfaces together), not appended;
the integration doctrine held. (b) **Redundancy** — the delegate paragraph
remains the densest block (SIXTH consecutive eval watch — still 3 actions;
wait_secs + resume + collect documented inline at reference density; a
fourth action or the terminal-wait clause T89 adds tips it to a split
row); the continuous-mode section (routing + gate-runner + digest + four
caches + mutant caches + re-exec) is the second-densest (2nd watch) —
both still accurate. (c) **Staleness** — ZERO findings: T86's layout fix
landed (27/27 set-equality re-verified this eval against `ls src/*.rs`
minus main.rs); plan mode's "Not yet" tail is accurate; hooks phase-2
deferrals named. (d) **Balance** — the hooks section carries config shape
+ semantics appropriate for a user surface; continuous-mode rationale
paragraphs name their specs. (e) **Quickstart truth** — install/run
commands verified against the live tree (T35's `cargo install --path .`;
the `check:` convention intact). **Zero findings this audit** — the docs
row bar is not met; the two density watches carry.

## Handoff — recommended execution order

Strictly serial (T89 is doctrine → never overlaps; T90 touches the dispatch
chain → no disjoint-files case against anything):

1. **T88** (robustness, pri 2) — src/decisions.rs + its test module ONLY;
   NOT on step-4's REQUIRED list → kimi validation OPTIONAL (T16/T31
   precedent: orchestrator gates + the spec's RED-proven error legs
   suffice). Estimate: one short child arc.
2. **T89** (robustness + doctrine, pri 2) — src/delegate.rs + LOOP-SPEC §2
   step 2 + README + any wrapped pins; doctrine → runs ALONE; LOOP-SPEC.md
   is on the REQUIRED list → kimi REQUIRED. Estimate: one arc, one
   validation round.
3. **T90** (feature, pri 2) — new src/permissions.rs + driver.rs dispatch +
   events.rs/eventlog.rs + README; REQUIRED kimi; budget for one fix-up
   round (the T83 grinder lesson — the spec's size guard should prevent a
   second).

**Human items:** none this eval. **Owed at THIS wrap:** the T81 acceptance
leg's eval-kimi half (this cycle's wall time + outcome quality — the
routine-glm half was recorded cycle 46). **Watch items carried:** T80
md-only telemetry (3rd cycle unexercised); T85 cross-tree doctrine
measurement (window opened at its landing); delegate-paragraph density
(6th eval); continuous-mode density (2nd); T83's two non-blocking
validator observations (signal-death `(exit none)` asymmetry; chat
per-turn hooks reload); edit_file not-found frequency; F13 phases 2–3 +
F2 phase 2 + F3 phase 2 deferrals stand (layad endpoint absent; written
reasons carried); delegate.rs (3,498) / tgrep.rs (2,527) module sizes.

## Outcomes (filled at cycle wrap — LOOP-SPEC Phase 3)

### Cycle 51 (2026-09-27) — fresh eval (kimi, loopd-routed: queue empty) — T88 + T89 + T90 filed; T88 LANDED

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

### Cycle 44 (2026-09-27) — freshness-skip; T80 + T79 landed (merges 9c4221d, 660f652); T81 MID-ARC (impl in flight)
- **T79 LANDED**: validators MAY run mutation legs in parallel after clean-tree gates — one throwaway worktree per mutant (/tmp/chug-mut-<item>-<k>) with its own role-keyed target-shared-mut-<k> (the T52 lesson per leg), cap 3 in flight, serial default on overlapping files with the overlap judgment declared in verdict notes, tree-restored semantics unchanged, findings reference mutant names. Carried in BOTH surfaces: LOOP-SPEC §2 step 4 (full path) + META-SPEC §6's executed goal text (elided form); .gitignore target-shared-mut-*/ glob + README cache-paragraph clause; 3 pin tests in shared_target_dir.rs. impl d942103 (glm 45/50 goal-accepted, self-RED-proven 6 mutants incl 2 catches of its own non-applying mutant commands) + orchestrator review-fix b7d9794 (the impl inserted the T79 mandate INTO the T52 launch-mechanics paragraph, leaving 'META-SPEC §6's goal template carries the same T79 mandate' adjacent to 'META-SPEC.md is not edited' — relocated to its own paragraph naming its own META-SPEC footprint; all wrap-sensitive pin carriers kept single-line; the kimi validator verified the fix accurate). kimi VERDICT PASS 34/50: gates independently re-run 656/656 release x2 under target-shared-validate, 8 mutations RED-proven (cap flip, MAY→MUST, wrap-spanning overlap corrupt, gitignore drop, both dir carriers, worktree path, byte-clean leg) with shasum-verified restores, 3 survivors all outside the spec's named review-check surface (targeted-test element, README paragraph, in-parallel wording), 5 non-blocking findings. Post-merge gates 656/656 under target-shared-main. Pushed. Acceptance telemetry (a later validator's events showing >=2 mutant legs overlapping in wall time) is a future Outcomes item per the spec.
- **T80 LANDED**: LOOP-SPEC §2 docs-only rounds (*.md-only diffs, file-extension-exact) skip full build/clippy/test at review AND post-merge — gates shrink to the guard floor (cargo test --test todo_consistency + bash -n on any .sh), mechanical grep -qvE '\.md$' classification template, validator escape clause (T67 executable-text class), ambiguity defaults to full gates; pinned by tests/loop_spec_docs_only_gates.rs (7 pins). Recovery arc: cycle-41 impl committed 860ec34 then died on the 35-min minutes budget pre-gate (machine slept ~7h mid-run); cycle-42 re-created review state, re-ran build green, died on the tools-proxy outage mid-test; the worktree was externally removed pre-harvest (impl artifacts lost — T18-class, no one's harvest omission); cycle-44 re-created the worktree from the branch, review found the committed pins RED against the committed doctrine (FLOOR + POST_MERGE_SHRINK needles spanned mid-phrase wrap points — the T78 wrapped-carrier class inside T80's own pin file), orchestrator fix-up 388a4a6 (T78 flat idiom, 3 mutation legs RED-proven), kimi VERDICT PASS 24/50 (gates independently re-run 653/653 release under target-shared-validate, 9 mutation legs ALL RED, predicate empirically verified on 6 file-shape cases, 3 non-blocking observations: rationale overclaim in step 3, the bash -n leg is unreachable under the md-only predicate — a spec-vs-doctrine nuance carried into the doctrine faithfully — impl-child gates scope). Post-merge gates 653/653 under target-shared-main. Pushed.
- **T81 MID-ARC** (per-phase model routing): impl child launched 18:27Z (glm, 50/35, pid 53420 in /tmp/chug-loop-t81), at 5/50 uncommitted when the budget-low directive (8 iters left) stopped new work; child left in flight per the cycle-39 wrap precedent. Full next-cycle recovery recipe on the T81 TODO row (collect-or-resume → review+gates → REQUIRED kimi alone → harvest-all → merge → flip → push → backfills).
- Cycle notes: freshness-skip (EVALUATION.md same-UTC-day, 5 ready rows; third consecutive cycle making the call, d1790531285-1). Morning lost to a tools-proxy outage: cycles 42/43 died mid-LLM-call, loopd HALTED after 3 consecutive failures, restarted 17:46Z. Two items landed end-to-end this cycle (one recovery, one clean arc); the T80 validator noted for next eval: the bash -n leg is unreachable under the md-only predicate + a rationale overclaim in step 3. Acceptance telemetry owed by later items: T80 docs-only gate wall time, T79 parallel-mutant leg overlap. Queue carries T81 (in flight), T75, T82 — all with ready specs. Outcome backfills through d1790533626-10. Compaction of cycle-37-and-older Outcomes carried to next wrap (budget).

### Cycle 40 (2026-09-26) — T78 release builds landed (merge 23fd276)
- **T78 LANDED**: loopd.sh builds + launches target/release/chug; LOOP-SPEC/META-SPEC launch paths and all review/merge/validation gates are now cargo test --release with the cold-build tradeoff named; spec check: convention stays debug (pinned); README documents it. impl a74769f (T63 resume #12 after 50/50 abort uncommitted) + fix-up f5a3ebc (tests-only, 11 granular count_eq pins, each RED-proven) after kimi R1 FAIL weak-tests (M8-M11 unpinned release carriers — single-line count needles could not see line-wrapped carriers; the class sweep pinned wrapped perl/timeout examples, comment restatements, README wrap-insensitively) then R2 PASS 0 blocking (fresh sweep F1-F8 RED, F9 prose survivor non-blocking). Post-merge gates 646/646 RELEASE under target-shared-main (migration half 1 paid; loopd pays half 2 next supervisor iteration). Acceptance telemetry (release gate wall time vs the 6-10s debug baseline) is next cycle's Outcomes item per the spec.
- Cycle notes: freshness-skip cycle (cycle-36 eval same-day). T63 resumes 12/12 all-time. R1 was the vacuous-pin family again — fixed by class sweep per T72 doctrine. Queue carries T80/T79/T81 (pri 2), T75 (pri 3 doctrine, runs alone), T82 (pri 3) — all with ready specs. Compaction of cycle-36-and-older Outcomes carried to next wrap (budget). Outcome backfills carried: d1790471401-24 (dispatch), d1790472177-1 (recovery), d1790472560-2 (routing).

### Cycle 39 (2026-09-26, ~20:1x-21:2x EDT) — freshness-skip; T76 + T77 both recovered and LANDED (merges 355f253, fedb9ef; pushed 26b547d, 09d9f03); T78 MID-ARC (impl in flight)

- **T76 landed** (tgrep token-budgeted ranked context retrieval, 13th
  tool; merge `355f253`, branch loop-t76 at `30dacb3`). Recovered from
  cycle-38 wrap state: kimi R5 validator (pid 74446) was in flight,
  delivered VERDICT PASS 0 blocking (47/50): 8 fresh mutants ALL caught
  RED incl. MARKER_RESERVE 96→32 subtle shrink (band-calibrated reserve
  tests genuinely straddle the marker band) and packing break→continue;
  worktree verified byte-exact post-revert; spec 5/5 requirements
  confirmed incl. the 300-hit scripted driver integration. 4 non-blocking
  observations carried for a future eval (glob arm lacks the walk arm's
  64 MiB aggregate cap; symbols_skeleton no 1 MiB per-file cap; degenerate
  <40-token budgets floor at 1; decl_kind extern word-boundary nit).
  FIVE validation rounds total: R1 merge-radius contradiction + vacuous
  basename, R2 symbols declaration-dropping class, R3 MARKER_RESERVE band
  + symbols resumption (3 survivors), R4 flaky perf pin straddling the
  70-140ms load band (21/21 mutants RED), R5 PASS. Orchestrator
  post-merge gates independently re-run 627/627 + clippy under
  target-shared-main. 16 artifacts harvested pre-merge (10 event streams
  incl. 2 abort+resume two-segment files = T63 resume pairs, 5 validator
  LEDGERs, child decisions.jsonl). Acceptance-telemetry leg is a
  later-cycle Outcomes item per the R5 report.
- **T77 mid-arc**: fix-up-1 (pid 74447) goal accepted 38/50 → `a34d0c0`
  tests-only (+204 lines, zero production changes vs `a5de407`): all 3
  R1 surviving mutants killed with individually RED-proven tests
  (pairing_unsafe chooser guard 2 fixtures, SEGMENT_TOKENS 16k pin
  24-message granularity fixture, !seg.complete young-remainder leg) +
  sweep-the-family. Orchestrator gates independently re-run 594/594 +
  clippy under target-shared. Kimi R2 validator IN FLIGHT (pid 7916,
  REQUIRED driver.rs, routing d1790468299-1, verdict record pending).
- **T77 landed** (cache-stable transcript trimming — segment-frozen
  prefix; merge `fedb9ef`, branch loop-t77 at `f83a9e7`). Recovered from
  cycle-38 wrap state: fix-up-1 (pid 74447) accepted 38/50 → `a34d0c0`
  tests-only (+204/−0), orchestrator gates independently 594/594. Kimi R2
  FAIL ONE blocking (S1 stop-at-target break deletable — over-collapse
  unpinned; R1's 3 mutants confirmed RED; S10 `seg.collapsed` disjunct
  proven equivalent non-blocking) → fix-up-2 run1 50/50 abort with work
  done uncommitted → T63 ELEVENTH resume accepted 7/50 → `f83a9e7`
  tests-only (+237/−0, 6 mutants RED-proven: M1 stop-at-target, M2
  pool-exhaustion, M3/M3b KEEP_LAST both directions, M4 segment-advance,
  M5 engage-gate est∈(80k,120k] band — the child caught its own
  first-surviving M5 fixture and rewrote it into the killing band).
  Kimi R3 VERDICT PASS 0 blocking (49/50): S1 CLOSED RED by
  `trim_stops_at_target_exact_collapse_extent` (6 markers vs pinned
  exactly-4), production byte-identical f83a9e7 vs a5de407, 599/599 +
  clippy on shasum-verified clean tree, 2 non-blocking benign
  single-token boundary survivors (measure-zero vs the spec's own ~16k
  estimate). Merge clean (no driver.rs conflicts vs T76). Post-merge
  gates 641/641 + clippy under target-shared-main. 12 artifacts
  harvested (6 event streams incl. 3 abort+resume two-segment T63 pairs,
  5 LEDGERs, child decisions.jsonl). Spec repo-context line corrected
  (trimming lives in driver.rs, not transcript.rs). Langfuse telemetry
  acceptance leg is a later-cycle Outcomes item.
- T44 overlap #6: T77 R2 validator (reads/mutates its own worktree) ran
  concurrently with the T76 main-checkout merge + post-merge gates —
  disjoint write surfaces, merges stayed serial (T76 first).
- **T78 MID-ARC at budget wrap** (8 iters left): impl child pid 20743 IN
  FLIGHT (glm 50/35, /tmp/chug-loop-t78, loop-t78 at 09d9f03 base,
  worktree preserved). Full recovery recipe on the T78 row. Dispatch
  record d1790471401-24 (mis-classed validation-routing in wrap haste;
  it was the solo-dispatch routing — doctrine never overlaps).
- Cycle-39 ledger: T76 + T77 both recovered from cycle-38 MID-ARC state
  and LANDED with pushes (26b547d, 09d9f03). TWO double-recoveries
  closed. T63 resume 2x this cycle (T77 fix-up-2 run1 abort → resume
  accepted 7/50; career 11/11) — plus one NOT needed (T76 R5 validator
  finished on its own). Validators caught: nothing new post-recovery —
  R5 (T76) PASS first-try this cycle, T77 R2's S1 over-collapse survivor
  was the cycle's one blocking catch (collapse-EXTENT pin class: assert
  exact marker counts + verbatim survivors, not ≤bounds — closed by
  f83a9e7 and proven RED in R3). T44 overlap #6 (T77 R2 validator ∥ T76
  main-merge, disjoint surfaces, serial merges kept). Host external load
  eased mid-cycle (mutant cycles back to seconds by ~01:00 UTC).
- Cycle-39 records: d1790468299-1 (T77 R2 routing), d1790468338-2 (T76
  R5 PASS), d1790468491-3..-10 (T76 backfills x8), d1790469659-11 (T77
  R2 FAIL), d1790470425-12 (fixup-2 T63 recovery), d1790470664-13 (T77
  R3 routing), d1790471194-14 (T77 R3 PASS), d1790471336-15..-23 (T77
  backfills x9), d1790471401-24 (T78 dispatch).
- Carried with ready specs: T78 (MID-ARC, recipe on row), T80/T81
  (pri 2 doctrine), T75/T79/T82 (pri 3 doctrine) — all doctrine, all run
  alone, no overlap possible. Final main gates this cycle: 641/641 +
  clippy under target-shared-main at fedb9ef (only doc/bookkeeping
  commits since).

### Cycle 38 (2026-09-26) — freshness-skip; T76 + T77 MID-ARC at budget wrap, nothing merged

- Freshness rule fired (cycle-36 eval same-day, todo rows present). Worked T76 (pri 1 feature, cycle-37 mid-arc recovery) and T77 (pri 2) under T44 overlap #5 (disjoint files: tgrep.rs/tools.rs/main.rs/driver.rs vs driver.rs-transcript/api.rs; overlap record d1790450841-1; T78 skipped for the window — doctrine never overlaps).
- T76 arc advanced FOUR validation rounds this cycle: fix-up-2 accepted 46/50 (f837e31) → kimi R3 FAIL (3 survivors, 2 vacuous-pin classes: MARKER_RESERVE band unexercised; symbols test-mod resumption unpinned) → fix-up-3 via T63 resume (146b89e, M1-M4 RED-proven) → kimi R4 FAIL (ONE finding: perf pin straddles load band = flaky goal gate; 21/21 mutants RED, zero survivors) → fix-up-4 (30dacb3: median-of-5 vs 1500ms, determinism byte-strong, RED both directions, 10/10 under 12-way CPU load). Orchestrator gates 627/627 + clippy at 30dacb3. Kimi R5 validator IN FLIGHT at wrap (pid 74446; full recovery recipe on the T76 row).
- T77: impl via T63 resume accepted 12/50 (a5de407 — segment-frozen 16k trim; SPEC.md/README updated; 590/590 + clippy) → kimi R1 FAIL weak-tests-not-correctness (3/6 mutants survived: deletable pairing guard, unpinned SEGMENT_TOKENS, deletable completeness skip; tree restored pristine after validator budget-died mid-mutant and was itself T63-resumed) → fix-up IN FLIGHT at wrap (pid 74447; recipe on the T77 row).
- T63 resume exercised 4x this cycle (T76 fixup-3, T77 impl, T77 validator mid-mutant, plus cycle-37's fixup-2 completed) — 10/10 career resumes accepted. First validator-resume with a live mutant: resume reverted and re-verified pristine.
- Validators caught the vacuous-pin class twice more (T76 R3, T77 R1) plus a flaky-gate class (T76 R4): the T72 sweep-the-family + T54/T62 survivor-to-pin pipeline keeps paying; mutation leg counts 18 (R3) and 21 (R4) all-RED-but-named.
- INCIDENTS: host under heavy EXTERNAL load mid-cycle (operator VM 564% CPU, load avg 57+) — orchestrator gates split per-suite to fit the 120s bash cap; multi-hour host sleep observed (minute budgets are awake-time, unaffected).
- Cycle-38 decision records: 7 orchestrator records logged (overlap-routing, 2 validation-verdicts, validation-routing, 2 recovery-routings, T77 verdict) + child-side records (T76 validator d1790451351-1, T76 R4 validator d1790454196-1, fixup-4 child logs) — the T75 zero-call adoption gap is closed in practice this cycle (the T75 doctrine row itself carries, ready spec).
- Carried with ready specs: T78 (pri 1), T75/T80/T79/T81/T77-in-flight, T82.


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

