# EVALUATION — chug, assessed by chug-loop (2026-10-01, cycle 85)

**MANDATORY fresh eval** — the queue is EMPTY (cycle-84 wrap `fe02689`
declared it drained — the first drain since cycle 82 — so the freshness
predicate's queue half fails and loopd routed kimi per T81, the third
consecutive correct routing prediction). The delta corpus since the
cycle-83 eval (same UTC day) is cycles 83(phase 2)–84: orchestrator
streams `events-20261001-102829` (kimi phase 2, 177/200 iters, 4h01m,
1.06M non-cached input tokens), `-121517` (glm, 102/200), `-132550`
(glm, 53/200) plus six child streams (t180 impl+validate, t182-impl,
t183 impl-validate, t181 impl-validate), read via the fresh digest (428
files, 24,438 iterations, FRESH per its self-exclusion check) with
targeted drills into `src/tools.rs`'s exec path, `src/trim.rs`,
`src/events.rs`, `src/eventlog.rs`, `.chug/decisions.jsonl`, and the
git record. The headline: **4/4 queued rows landed (T180 — the F12
web_search feature, T183 delegate env map, T182 decision_log
always-reminder, T181 BSD-sed sentence), zero reverts, release v0.11.0
self-tagged — and the delta's costs are two ORCHESTRATION-hygiene gaps
(two lost child event streams, one zombie-raced gate) plus one real
tool-output data-loss bug class that fired 13 times.**

## 1. What chug does well — be brief

- **4/4 landed incl. a 1050-line feature** (T180 web_search, F12 phase
  1: provider seam + zero-config DuckDuckGo HTML provider, kimi PASS
  8/8 mutants) — and the recovery machinery absorbed FOUR budget deaths
  with 100% correct routing (t180-impl 50-min wall → ONE resume →
  accepted; t180-validate 60/60 → ONE resume → PASS; t183-impl 80/80
  uncommitted → ONE resume → committed+accepted; t183-validate 60/60
  mid-mutant → ONE resume + verdict recovered from transcript +
  orchestrator gap-closers 6/6).
- **The small-row prediction verified 2/2**: post-T178/T179, both
  small rows landed first-try clean (T182 29/80 in ~7 min child wall,
  T181 20/80 in ~10 min) — small-row minutes-deaths → 0 as predicted.
- **T182's success criterion measuring well**: the two `options … got
  missing` fumbles in the cycle-83 stream are at 06:28Z, PRE-merge
  (81ddb6c ~09:45Z); ZERO fumbles in all four post-merge streams.
- **T183's defense-in-depth paid off immediately**: the new delegate
  `env` param was silently dropped at both validator launches (the
  orchestrator binary predated the merge) and the goal-carried export
  carried the target dir anyway — exactly the layering the spec pins.
- **Evidence-first under confusion**: glm seg-1's wrap goal was
  REJECTED on a load flake; it root-caused a LIVE "zombie" validator
  racing the gates, proved the unpatched tree green on a quiet machine
  (1056/1056 debug, nextest 1310/1310 release, module bisections),
  reverted an unneeded patch byte-identical, and made no code change
  (triage d1790856539-8). Verify-then-kill held (read-back first).
- **The decision corpus is complete**: ~39 records in the delta window
  — every recovery routing, validation routing/verdict, outcome
  backfill, and the cycle-84 phase-1 skip — zero gaps.

## 2. Incidents worth fixing

1. **Harvest gap ×2 — two impl event streams LOST (→ T186, pri 3,
   doctrine SOLO).** The t183 impl's two glm segments (80/80 death +
   resume) and t181's glm impl segment (20/80) were never harvested;
   both worktrees were removed. Mechanism: a FRESH child launched into
   a reused worktree ROTATES the predecessor's `.chug/events.jsonl`
   (T10/T7), so copying only `.chug/events.jsonl` — the natural reading
   of LOOP-SPEC §2 step 5's "harvest every child run's
   `.chug/events.jsonl`" — silently drops the rotated segments, and
   `git worktree remove` deletes them (untracked). Cycle-84's glm
   orchestrator even named the t181 harvest
   `events-t181-impl-validate-…` believing BOTH segments were inside;
   the file holds `runs: 1`, kimi only (jq-verified). t183's loss
   happened DESPITE the cycle-83 wrap note's explicit correct recipe
   ("cp the worktree's .chug/events*.jsonl … both segments live in the
   worktree .chug/") — a written recipe is not a mechanism; cycle-83's
   kimi harvest of t180 (two separate files, impl runs:2 + validate)
   shows the right shape. Cost: the eval corpus loses exactly the
   impl-death-and-resume streams F13/F14 feed on; this eval could not
   reconstruct t183-impl's per-iteration evidence (its abort kind
   survives only via the wrap note's prose).
2. **Zombie-collision: a live validator raced the wrap gates
   (doctrine half → T186).** Cycle-84 seg-1 declared the t183 validator
   (pid 6260) dead via a TRUNCATED `ps` read and removed its worktree
   while it was alive; the zombie's cargo suites raced seg-1's
   goal-gate `cargo test` → "check command failed" rejection. Diagnosed
   (read-back first per the kill rule), zombie + orphaned check tree
   killed, quiet-machine green proven; code change weighed and REJECTED
   (d1790856539-8). Cost: ~30 min + one rejection cycle + a near-miss
   on tree integrity (removing a worktree under a live child is the
   T19 class in reverse). The two lessons — never remove a worktree
   while any child pid launched in it lives; liveness via `delegate
   status` or `kill -0`, never a truncated `ps … | head` — are
   doctrine, filed in T186.
3. **bash reader-grace DISCARDS already-read output when a grandchild
   holds the pipe (→ T185, pri 2, bug).** 13
   `(output truncated: reader did not drain)` fires in the delta
   (cycle-83 kimi ×1, cycle-84 glm ×6, t181-validate ×3, t183-validate
   ×3) — escalated from the cycle-83 watch-list's single instance
   (d1790836694-10). Mechanism (src/tools.rs, read in full): reader
   threads `read_to_end` then send ONCE at EOF (~line 1135); the
   caller's `recv_capped` returns `(Vec::new(), false)` on the 5s
   READER_GRACE timeout (~line 1253) — the side's ENTIRE accumulated
   buffer is discarded; the doc comment's "whatever output was captured
   is returned with a truncation note" is FALSE in the timeout leg.
   Cause class: an orphaned grandchild (alarm-killed cargo's rustc, a
   command's own backgrounded process) inherits the pipe write-end, so
   EOF never comes. Cost per fire: 5s stall + total loss of that side's
   output + usually a re-run iteration. The orphan-holds-pipe shape is
   already a known test scenario (~line 1947), so the fix has a ready
   harness.
4. **T180: the filing-time discipline failure the doctrine already
   names.** Filed at est ~420 WITH an explicit growth-stop clause;
   landed 1050/-58 (2.5x); impl died at the 50-minute wall 74/80; FIVE
   goal-gate rejections before acceptance — one REAL (the T95 README
   module-list miss), four diagnosed and honestly attributed (T31
   port-theft residual, a self-inflicted background-run load collision,
   the T82 stub-spawn family ×2). The T173 measure clause TRIPPED at
   the cycle-83 wrap (3/8 minutes-deaths: t176, t177, t180); the remedy
   stays filing-time splits — practiced here: all four of this eval's
   rows file at ≤ ~230 estimated lines. No new row (the doctrine
   suffices; this text is the calibration record per META-META-SPEC).
5. **Validator iteration-ceiling deaths ×2 — measure watch, NOT yet
   tripped.** t180-validate 60/60 (verdict announced post-resume) and
   t183-validate 60/60 with the verdict WRITTEN-but-unannounced (a
   load-flake check rejection killed the driver post-verdict;
   recovered from its transcript). The clause trips at >1 of the next
   8 dying with the verdict unannounced → trim default mutation-leg
   counts. This window: 1 of 3 (t181-validate 32/60 clean). Watch.
6. **Weighed and REJECTED** (each logged eval-triage): (a) **F14 as
   originally written** (a compaction/summarization pass) — the
   driver-managed context window ALREADY EXISTS (T77's src/trim.rs: 5
   markers in the cycle-83 kimi transcript, 4 in this eval's own live
   transcript by iteration ~12); no measured context-ceiling death in
   the delta; a summarization pass would break the trim design's
   byte-stable prompt-cache prefix. REFRAMED to telemetry → T184 (§4);
   summarization deferred on FEATURES.md. (b) **F12 phase 2
   keyed-provider pull** (the cycle-84 wrap note's suggestion) —
   FEATURES.md's deferral stands: zero consumers have needed a keyed
   provider; the zero-config seam covers observed need; F14 outranks
   it on measured cost. (c) **stub-deadline widening** for the
   load-flake family — already rejected (d1790856539-8: the zombie was
   the cause; quiet-machine green proven). (d) **driver post-abort
   orphan reaping** — mechanism unproven (the zombie was a LIVE
   validator, not an aborted one's orphan); T186's liveness discipline
   covers the actionable surface. (e) **T179 stale-hash follow-up** —
   cycle-83 §2.6(c) rejected (T20's runtime `resolve_head` fields carry
   worktree identity; the baked hash is the BUILD commit, correct
   semantics); not re-filed. (f) **estimate-threshold number edits** —
   META-META-SPEC: re-calibrate in eval text, never edit the number in
   passing. (g) **edit_file old-not-found fumbles** (kimi ×2 on t182's
   spec path during eval-artifact writing; self-corrected in ~1 iter;
   the error names counts) — watch, no row.

## 3. Friction hot spots — fix assessment

- **T182 (decision_log always-reminder): criterion HOLDING** — 0
  post-merge fumbles across 4 streams (both pre-merge fumbles at
  06:28Z, §1).
- **T176 (clippy `-D warnings` in the impl goal): HOLDING** — zero
  clippy findings in this delta's four validations.
- **Small-row minutes-deaths → ~0: 2/2** (T182, T181 — §1).
- **T173 clause: TRIPPED** (3/8 at the cycle-83 wrap) — the remedy
  (filing-time spec-size discipline) is in force; this eval's rows all
  file ≤ ~230 est (§2.4).
- **reader-drain class: ESCALATED watch → row** (T185; 1 → 13 fires).
- **Load-flake families (T172): one explained fire, no unexplained
  ones** — the cycle-84 wrap-gate red was the zombie's duplicate load
  (§2.2), not a latent flake; quiet-machine full green. The T31/T82
  families fired INSIDE t180's goal-gate checks (wrap-note evidence) —
  watch continues.
- **Estimate calibration (required re-check)**: T180 ~420 → 1050
  (2.5x — network-tool feature rows join the MCP-protocol class at
  2–2.5x naive); T183 ~360 → +723/-39 (2.0x); T182 ~30 → +57/-16
  (2.4x on a tiny base — small-base noise); T181 ~45 → +45/-3
  (1.1x ✓). Re-calibration: feature rows with a network/protocol
  surface price 2–2.5x naive and MUST file as two rows when naive
  exceeds ~200; sentence+pin rows land ~1x when the pin file already
  exists (T181) vs ~6x when it must be created (T179). The ceiling's
  failure measure stands: rows landing >500 actual needed rescue 3-for-3.
- **Orchestrator iteration economy**: kimi phase-2 177/200 in 4h01m
  (serial validation arcs dominate); glm 102 + 53. The terminal
  long-poll keeps per-arc cost ~15–25 iterations. Kimi's 1.06M input
  figure is NON-CACHED ONLY — true per-call context size is invisible
  (§4, T184).

## 4. Capability gaps — ROADMAP PULL: F14 → T184 (context-economy telemetry), REFRAMED

The cycle-83 eval appended F14 (context compaction) on the evidence of
0.8–1.2M cumulative input tokens per orchestrator stream. This eval's
code read REFRAMES it: **the driver-managed context window ALREADY
EXISTS** — T77's trim machinery (src/trim.rs: 120k-est-token trigger,
80k target, 16k frozen segments, `[trimmed:]` markers, byte-stable
prefix for prompt caching) FIRES in real streams (5 markers in
`.chug/transcript-20261001-102829.jsonl`; 4 in this eval's own live
transcript by iteration ~12), and BOTH call sites persist the markers
via `transcript::rewrite` (src/driver.rs ~689 resume path, ~1498 loop
path) — so the F14 entry's "transcript stays complete" constraint is
stale: T77 chose record-rewrite by design (re-segment-identical on
resume). The "1M input tokens" evidence is also misread: the events
stream records only NON-CACHED input — `cache_read_input_tokens` is
parsed by the API layer (src/api.rs ~1227) but dropped before the
events stream (`Event::Usage` carries `{ input, output }` only), so the
loop cannot see its own per-call context size, and the digest's "input
context curve" measures a partial quantity. **The real phase-1 gap is
OBSERVABILITY**: trim fires silently (no event), cache reads invisible,
context size invisible. → **T184**: cumulative cache fields on
serialized iteration lines, a `Trim` event at both call sites
(before/after estimates, segments collapsed), digest surfacing, README
honesty. The summarization-quality half is DEFERRED with a written
reason (no measured ceiling death; prefix-stability cost) — FEATURES.md
F14 annotated accordingly. Ordering notes (written reasons per
META-META-SPEC §4): F12 phase 2's deferral stands (§2.6(b)); F13 phases
2–3 stay layad-blocked; F9 `allowed-tools` / F11 prompts-surface /
F10 phase 3b keep their later-phase annotations — no new consumer has
appeared.

## 5. Top 3 priorities

1. **T185** (pri 2, bug) — tool-result data loss fired 13× in the
   delta with a nailed mechanism and a ready test harness; touches
   src/tools.rs → kimi REQUIRED.
2. **T184** (pri 2, feature — the mandatory roadmap pull) — the loop
   is blind to its own context economy; every future eval's cost
   analysis rides on it; touches src/events.rs + src/driver.rs → kimi
   REQUIRED.
3. **T186** (pri 3, doctrine SOLO) — two lost streams + one zombie
   race in ONE delta; the cheapest fix with the highest corpus value.
   (T187 slots fourth — trivial doctrine drift, same SOLO rule.)

## 6. README audit (usability)

The cycle-83 full cold-read stands (structure sound); this eval
delta-checks the four landings. 962 lines (+19). (a) **Reading order:
sound** — no accretion: T180's web_search bullet integrated into the
tools list (~line 484), the two→three network-exceptions sentence
updated in place (~line 380), the Development-layout line carries the
new module (the T95 guard caught the impl's miss — one of its four
check rejections); T183's delegate env bullet folded into the existing
delegate lines. (b) **Redundancy: none new.** (c) **Staleness: none
found** — the stale "seventeen tools" comment was fixed in T180's flip
commit; the tools list carries `mcp_resource` and `web_search`. (d)
**Balance: watch continues** — 962 lines; the MCP section remains the
densest but still earns its place. (e) **Quickstart truth: accurate** —
untouched this delta. No docs row filed.

## Handoff

- **Queue order**: T185 (pri 2, bug — src/tools.rs → kimi REQUIRED) →
  T184 (pri 2, feature — src/events.rs + src/driver.rs → kimi
  REQUIRED) → T186 (pri 3, doctrine — SOLO, kimi REQUIRED) → T187
  (pri 4, doctrine trivial — SOLO, kimi REQUIRED).
- **Overlap notes**: T185 (src/tools.rs only) and T184
  (src/events.rs / eventlog.rs / driver.rs / trim.rs-call-sites +
  scripts/eval-digest.sh + tests/eval_digest.rs + README) are
  FILE-DISJOINT → eligible for the T161 2-impl overlap (re-key the
  second spec's `check:` export per the T175 doctrine); validators
  stay serial (cap 1); merges strictly serial in queue order. T186 and
  T187 are doctrine → NEVER overlap anything; they run alone, serially.
- **Validation routing preview**: all four REQUIRED (tools.rs /
  events.rs+driver.rs / doctrine / doctrine).
- **Human-decision items**: none new. Standing carries: the layad
  fine-tune pipeline (F13 2–3 precondition), the deferred chat/TUI
  phase 2s, F12 phase 2's no-consumer deferral.
- **Next-eval measures**: (1) T182's criterion holds at 0 fumbles;
  (2) reader-drain fires → 0 post-T185 with bytes surviving; (3)
  harvest completeness: every worked item's impl AND validator streams
  present in `.chug/` post-T186 (this delta: 2 of 3 multi-child items
  incomplete); (4) T184 lands → the digest reports cache-read +
  trim-fires and the next eval can finally see per-call context size;
  (5) the validator 60/60-with-unannounced-verdict clause (trips at
  >1 of 8; stands 1 of 3); (6) estimate calibration: this eval's rows
  all est ≤ ~230 — the filing-discipline practice test.

## Outcomes (filled at cycle wrap — LOOP-SPEC Phase 3)

### Cycle 88 (2026-10-02, 00:54 UTC–, continued as cycle-89 run starting 05:03 UTC) — glm routine freshness-skip cycle (freshness held: EVALUATION.md touched 00:37 UTC same day + 7 todo rows) — COMPLETE

- **T194 landed** (merge 0fad6c7, impl f9f5340). The 3-child fleet + parallel validators doctrine: LOOP-SPEC Phase 2 cap amended T161's ≤2-children/≤1-validator to ≤3-children/≤2-validators with Pipeline-overlap pattern (iii) — a 3rd impl child flies only when all three items' spec-named target-file lists are PAIRWISE disjoint — and pattern (iv) — a 2nd validator flies only when two items are simultaneously past gates, never two on the same item, always kimi. Validator target dirs split into slot dirs target-shared-validate-a/b (two validators sharing one dir would serialize on cargo's build lock); a third impl slot impl-c added beyond the spec's letter (the ≤2-slot rule ignored a held impl-b on relaunch — impl-c keeps 'always a slot no in-flight impl holds' true for the fleet). Resource governor as a Hard rule: ≤4 cargo-heavy children, validators win under memory pressure, degradation recorded via decision_log. Pins: tests/loop_spec_fanout.rs (10 tests) + shared_target_dir.rs re-key. kimi validator: all 5 spec reqs verified at every carrier, 10/10 T79-parallel mutants killed by targeted pins, 5 non-blocking findings (worst: governor ≤4 is spec-mirrored and vacuous under the ≤3 cap — it binds only if the child cap later rises). Impl child died 80/80 wrapping up with work committed → T63 orchestrator-finish (no resume burned). Post-merge gates 1362/1362 + clippy clean.

- **T189 landed** (merge 7074afe, impl 5ca8a43). The low-stakes validation lane: LOOP-SPEC §2 step 4 gains the mechanical gates-only predicate — gates-only when ALL four hold: (a) no core-list file touched (driver/api/tools/events.rs; any LOOP/META/META-META/SELF-SPEC edit stays FULL), (b) ≤ ~150 changed lines by `git diff --numstat`, (c) no new tool/command surface (schema enum, CLI flag, MCP tool, hook event), (d) no CI/workflow or `check:` line change; any one flipped → full adversarial. Inputs are diff-computed (src/valroute.rs: parse_numstat incl. rename rows, surface markers on added lines only, check:-header exclusion); the routing record names all four inputs; the lane skips the kimi child NEVER the gates; gates-red lane-eligible diffs still get fix-up children. `--validate`/`--no-validate` operator overrides wired into `run --auto-spec` + `quick`, recorded. kimi PASS: m1-m4 mutants killed (core-direction flip, 150 boundary, surface marker, fix-up needle), m5 survivor LOW (cmd_run override-record call site lacks an e2e test; function-level pins hold — carried, noted for a future eval), core-list scope finding (doctrine's clause (a) uses the 4-file list; the spec's repo-context named a longer list — internally consistent, fails toward FULL, blessing deferred to next eval). Impl arc: 80/80 death uncommitted → T63 resume (5ca8a43) — resume count now 2 this cycle. Post-merge 1379/1379 + clippy clean.

- **T186 landed** (merge 634c205, impl 051e447 + orchestrator clippy fixup bff6630). LOOP-SPEC step 5 sharpened per the cycle-84 losses: (1) harvest-ALL mechanics — copy every `.chug/events*.jsonl` in the worktree (live stream AND each rotated segment, T10/T7 rotation named), one harvested file per source file, each named per the segment(s) it actually contains, `impl-validate` combined-name ban for single-segment files; (2) removal precondition exact in BOTH directions — no child pid launched in the worktree may be alive (liveness from `delegate status`/`kill -0`, truncated-`ps` ban, pid 6260 named) AND a bare ps/kill-0 hit must exclude defunct-Z (`ps -p <pid> -o stat=`); (3) Phase-3 wrap bullet matches. 3 new exactly-once pin legs in loop_spec_recovery.rs, RED-proofed in-commit. Impl died at the 50-min wall 60/80 committed-complete → orchestrator-finish (d1790921090-3); the independent clippy gate found `useless_concat` on TRUNCATED_PS_BAN (the T166 class AGAIN — 3rd sighting; fixed on-branch bff6630, string byte-equivalent). kimi REQUIRED PASS (d1790921297-4 routing, d1790922047-5 verdict): gates 1405/1405 zero flakes + clippy clean re-run, all 17 needles independently grep-verified exactly-once, 5/6 mutants killed serially (M5 drop-a-test uncaught — informational, the spec mandates redness on sentence reverts which M1–M3 prove), tree byte-clean. Validator process WEDGED post-goal_complete (14 min silent, verdict fully rendered in log) → read-back-first kill (the T186 kill rule exercised); its own doctrine then governed the worktree removal (defunct-Z check exact both ways). The 50-min wall: 1 of the next-8 census so far (T173 measure clause live).

- **T192 landed** (merge da366df, impl 004b6cb — flipped at cycle-89 tail). F14 PHASE 2 CLOSED: live-context editing, the CLM port. The driver mirrors the post-system-prompt message list to `.chug/LIVE_CTX.md` as `[[CTX_TURN i role=...]]` blocks pre-call each iteration; the model edits it with its ordinary file tools; an accepted parse-back — whole blocks, pinned content untouched, strictly smaller in tokens — splices the transcript and records `[ctx-edit: ~Nk tokens, M turns]` markers (T77's resume-identity discipline); edit-only turns are FREE (≤3 consecutive, 4th counts, `--max-tokens` always binds); one-shot `--ctx-warn-at-tokens` nudge (default 0 = off); `Event::CtxEdit` lines for the T184 digest. src/live_ctx.rs 1142 lines + driver/tests/live_ctx.rs 653. ARC: impl died 80/80 twice (fast-proxy ~7.5-min run 1; ONE T63 resume died 80/80 with the feature functionally complete) → orchestrator-finish with three gate fixes (README T95 module-list guard + the spec's req-6 bullet; the nudge test's position asserts vs the T13 budget-low co-fire; the free-turn test's CtxEditorLlm script bug — the drops queue is served BEFORE the responses queue, so `.then_read_big_file` was unreachable and call 6 re-read instead of calling goal_complete). kimi REQUIRED PASS (d1790924978-9 routing, d1790926020-10 verdict): gates re-run 1433/1433 + clippy clean, ALL 7 mutants caught (shrink-gate-flip, pinned-drop, free-turn-cap-4, parse-order-drop, pairing-drop, classify-bash-readonly, nudge-unlatched), tree byte-clean; the validator died 60/60 post-verdict pre-announcement — verdict recovered from the log (4th of the unannounced-verdict class; the validator measure census now reads: 2 of last 9 at 60/60 unannounced). Findings: 1 weak-test (non-blocking — `token_budget_binds_on_free_edit_turns`'s scripted edits are actually rejected, so no genuinely-free turn exercises the usage-accumulation skip; carried for the next eval) + 3 informational (chat-mode mirror in-scope-but-out-of-spec, plan-mode never mirrors, MIRROR_WARNED process-static). F14's value claim is NOT asserted — T184's cache-token telemetry is the arbiter.

- **T191 landed** (merge 69c4b4d, impl 7b10652 — flipped at cycle-89 start after a mid-arc segment death). Runbooks/: five one-page operator runbooks (quick-task, feature, repo-eval, loop-ops, adversarial-review) + index + a README Runbooks section integrated between Validation and Interactive mode. Docs-only (+304/-1, 8 .md files) → kimi SKIPPED per the docs-only optional rule (routing d1790914438-12), but the T67 duty was done for real: every load-bearing command executable-text-verified — quick/ledger/--validate/--no-validate against a CURRENT-source build (the repo ./target binary was stale vs the T189 merge — caught and avoided), loop-ops HALT/3-strikes/goal-less claims against loopd.sh lines 124/150/279/318/369. Reduced gates 81 green in the worktree (todo_consistency 19, shared_target_dir 33, loop_spec_docs_only_gates 8, loop_spec_validation_lane 6, install_sh 15), post-merge guard floor green in target-shared-main. The 2-impl overlap (T190 solo-default + T191 impl-a, d1790911785-10) worked: disjoint files, no build-slot contention, T190's merge landed first per serial-merge order.

- **T190 landed** (merge 75158e8, impl d7843c2 — flipped at cycle-89 start after a mid-arc segment death). Completion notifications (operator ask 2026-10-01: async-first needs a nudge): opt-in `.chug/notify.json` (absent/disabled = ZERO behavior change; malformed = off + one T90-style warn), fire-and-forget machinery mirroring SPEC-8 — bounded channel, drop-on-full, one flusher thread, drain-capped Drop so the terminal notice lands without stalling exit. Sinks: osascript `display notification` + layad POST (push/silent judgment stays layad's). Fires from the CHILD chug process in run_loop (loopd forwards nothing; plan/chat unwired by design): goal-complete w/ summary, abort w/ cause+model+budget, validation verdict w/ item+PASS/FAIL; short-run goal suppression via min_duration_secs (aborts/verdicts never suppressed); delivery failure counted + noted EXACTLY ONCE per run as a `notify_error` events line, run never affected. New `Event::ValidationVerdict {item, passed}` emitted at the verify() legs (blocked check = FAIL — a blocked check verified nothing), console/TUI silent. `release` event name accepted + Kind::Release plumbed end-to-end with NO in-repo emit point (the release tag push lives in the orchestrator's release flow — documented deviation, validator-confirmed). Arc: impl died 80/80 (~8 min, unusually fast proxy) UNCOMMITTED → ONE T63 resume → died at the 50-minute wall 49/80 with work COMPLETE+COMMITTED → orchestrator-finish per T55/T63 (resume cap burned, no second resume). kimi REQUIRED PASS (d1790915606-14 routing, d1790916846-15 verdict): gates re-run 1402/1402 + clippy -D clean, 5/5 T79-parallel mutants KILLED (disabled-path-fires, unbounded-channel, abort-suppression, note-once-latch, blocked-check-PASS), test-quality leg 23 discriminating legs no vacuous greens; 2 minor findings (dual-cause notify lines latch, cross-thread events interleave theoretical in degraded path) + 1 cosmetic carried as hardening candidates. Post-merge nextest release green in target-shared-main.

**Cycle-88/89 wrap notes.** Worked in queue order: T194 → T189 → T190+T191 (the 2-impl overlap, serial merges) → T186 (doctrine SOLO) → T192 (the F14 phase-2 feature pull). **Deferred with reason**: T187 (META-SPEC alarm 280 — pri 4, doctrine SOLO, ~105-min arc vs 73 min left at the 09:03 deadline; spec cold-start-ready, carried 3x; the next cycle should run it FIRST). Queue is handoff-clean: one open row, spec ready, SOLO. **Release v0.13.0 tagged at wrap** (e66f2e6 bump + tag; pairing verified scripts/check-tag-version.sh; notes = scripts/release-notes.sh HEAD v0.12.0 — 6 items since v0.12.0: T194, T189, T190, T191, T186, T192-minor-feature + the F14 PHASE 2 check-off in FEATURES.md). Final gates at tagged HEAD under target-shared-main: nextest release 1433/1433, clippy -D clean, build OK. **Child-economy census (the T173-era measure clauses)**: impl children 6 dispatched (T194, T189, T190, T191, T186, T192-run1) + 3 resumes — deaths: T194 80/80 committed (orch-finish), T189 80/80 uncommitted (resume→landed), T190 80/80 uncommitted → resume → 50-min wall committed (orch-finish), T186 60/80 committed (orch-finish), T192 run1 80/80 uncommitted (~7.5 min, fast-proxy) → resume died 80/80 functionally-complete (orch-finish). The 50-min wall caught T190 (1 of the next-8 census: 1). **Validators**: 4 REQUIRED kimi runs (T194 10/10, T190 5/5, T186 5/6, T192 7/7 — 27/28 mutants era-wide, zero blocking findings, all first-round PASSes) + 1 docs-only skip (T191). T192's validator died 60/60 POST-verdict (recovered from the log, no resume burned) — the unannounced-verdict class census: 2 of the last 9 (t173 measure: >1 of next 8 → trim mutation legs next eval; NOT yet tripped at 2). **Incidents**: T186's validator WEDGED post-goal_complete (14 min silent, verdict fully rendered) → read-back-first kill — the T186 kill rule exercised before its own doctrine even landed; T192's fast-proxy produced 80 iterations in ~7.5 min (twice) — the iteration ceiling, not minutes, was binding. **Harvest inventory** (all in .chug/): events-t194-* (2 segments), events-t189-* (2), events-t190-* (2), events-t191-impl, events-t186-impl-2seg + events-t186-validate + LEDGER-t186-validate, events-t192-impl-2seg + events-t192-validate + LEDGER-t192-validate — the T186 harvest-all doctrine was LIVE-TESTED on its own worktree mid-cycle and held (both segments of every multi-segment file recovered).

### Cycle 87 (2026-10-02, 20:52–00:50 UTC) — glm routine freshness-skip cycle (freshness held: EVALUATION.md cycle-85 same UTC day + 7 todo rows; skip record d1790888022-1) — 1 landed (T188 after a 4-round arc), T186/T187/T189/T190/T191/T192 unworked (specs ready; budget went to the T188 validation tail)



**T188 done 61aa90c (merge of loop-t188: 4e0a66c impl + 7377425 docs + fe584fb r2-fixup + 5c81faa r3-fixup + e2779c6 r4-drain-pin; routing/recovery records d1790888022-2, d1790890361-3, d1790896112-6; validation records d1790894090-4, d1790897479-7, d1790897512-8).** The auto-spec feature is LANDED: `chug run --goal ... --auto-spec` / `chug quick` drafts spec+check from a bare goal (one read-only LLM call reusing plan-mode machinery, PlanKind::SpecDraft), gates the draft with a real dry-run of its check (vacuous checks rejected, redraft once, abort honestly on a second failure — never loosened), runs against `.chug/auto-spec.md`, plus chat `/auto-spec` + `/auto-spec-approve` and the task-class doctrine sentence in README/SPEC. Cycle-87 arcs: r3 fixup died 80/80 UNCOMMITTED → ONE T63 resume with a noise-strip addendum (the child had mass cargo-fmt'd the whole repo — 83 files +12.6k/−9.2k uncommitted; the resumed child stripped it to 775 real insertions; NEW incident class for the next eval: impl-child tree-wide formatters need a goal-template ban) closing F1 (e2e mock-Llm draft→gate→run→goal_complete + m6 killers), F2 (SpecDraft wiring pins, m7 killed), F7 (parse_slash/tui/CLI entry pins). Kimi narrowed verdict (d1790897479-7): FAIL 1 — the en-route chat drain fix (ui_gone latch + one drain pass so a queued auto-spec request survives UI quit) was UNPINNED (m8 byte-exact revert survives 21/21; session-level tests race). Round-4 fixup e2779c6 factored the idle poll into `poll_step<P: IdlePollSources>` + `PollStep` with a deterministic scripted-source test; the kimi r4 round was skipped on wall budget (routing d1790897512-8, confidence 0.72 — the deviation to watch) and the ORCHESTRATOR RED-proved m8 in lieu: Idle→Drained on the disconnect leg made the new test RED, revert green, nextest 1351/1351 branch + 1352/1352 main (target-shared-main), clippy -D zero. Harvest: events-t188-round34 + LEDGER-t188-round34 on disk; worktree /tmp/chug-loop-t188 kept at wrap (T186 rule) — cleanup is next cycle's first mechanical act. Also this cycle: the 74d3331 class bit AGAIN (branch-side check re-key invisible to the delegate goal gate — check_cmd ran target-shared-impl-a; dispatch fix 82804fb puts the re-key in MAIN's spec copy).

### Cycle 86 (2026-10-01) — glm routine freshness-skip cycle (freshness held: 8 todo rows T186–T193 + EVALUATION.md same-UTC-day; skip record d1790875941-1) — 1 landed (T193), T188 IN-FLIGHT on a kept branch (round-2 validator FAIL, wall-budget stop), T186/T189/T190/T191/T192/T187 unworked (specs ready)

- **T193** (pri 3, bug) — site-sync in-flight classifier counts done TODO
  rows: landed `a3acaac` (impl `a9f631b`, glm first-try 43/80 clean,
  +112/-7 across exactly the spec's 3 target files). Classifier now
  pre-filters OPEN rows only before the F-id grep; F6/F12 FEATURES
  annotations brought to the uppercase-LANDED convention; 5 new
  site_sync pins. Gates-only routing (d1790876650-3 — no core-list file,
  ~119 lines ≤ 150, no new surface, no check: change; mirrors the T189
  predicate its sibling will codify): nextest 1324/1324 independently
  re-run in worktree AND main, clippy `-D warnings` clean. Child verified
  end-to-end on a throwaway site fixture: F6 landed, F12 landed, F14
  in-flight via open T192. Acceptance VERIFIED at this wrap: a
  CHUG_SITE_SYNC_NO_PUSH=1 dry run against the real site dir renders
  Session fork (F6) landed, Web search (F12) landed, Context compaction
  (F14) in-flight — site commit 2c5efb5 local, the post-cycle loopd sync
  pushes it. Region regenerates at this wrap's site sync —
  acceptance confirmed in this entry's release section.
- **T188** (pri 2, feature) — auto-spec: impl died 80/80 twice (fresh arc
  + ONE T63 resume, d1790876863-6) → orchestrator-finish amendment
  (d1790878543-7): closed a real vacuous-class bug (bare `cd` = $HOME
  always-0 = vacuous; `cd src` can fail = real — the child's impl
  classified `cd src` vacuous), approve_gate fixture, 4 clippy legs,
  README layout + req-4 docs (README Quickstart paragraph + SPEC.md CLI
  rows), commits 4e0a66c + 7377425, 1330/1330. kimi round-1 verdict FAIL
  (7 findings, d1790882387-9): m6 (draft_and_gate 1-attempt) and m7
  (SpecDraft kick flip) mutants SURVIVED the full suite — headless
  orchestration + wiring zero-coverage. Fixup child (pid 68910) died at
  the 50-MINUTE wall 76/80 (first minutes-wall death in the census —
  measure-clause data) with work complete-but-uncommitted → T55
  orchestrator-finish commit fe584fb (F3–F6 closed, 1335/1335). kimi
  round-2 verdict FAIL (3 of 7 open): F1 e2e mock-Llm test absent (m6
  still survives), F2 SpecDraft wiring pins absent (m7 still survives),
  F7 parse_slash/CLI pins absent; F3–F6 closed. Wall budget (three
  children hit budget walls this cycle) forced the stop: branch
  loop-t188 KEPT in /tmp/chug-loop-t188, recipe on the TODO row, all six
  child segments + three child ledgers harvested. One process lesson:
  the first fixup dispatch landed while the round-1 validator was still
  verifying — the child correctly refused to start on the held
  driver.lock (2-min loss, no damage); dispatch only after the previous
  child in the same worktree is fully dead.

### Cycle 85 (2026-10-01) — kimi mandatory fresh-eval cycle (queue drained at cycle-84 wrap fe02689; T81 routing correct 3-for-3) — 4 rows filed (T184–T187 + F14 reframe), 2 landed (T185, T184), T186/T187 DEFERRED to cycle 86 (specs cold-start-ready; T186 gained the defunct-zombie liveness amendment from this cycle's inverse incident) — no tag (2 items < 3 since v0.11.0, F14 phase 1 of 2 not a check-off)

- **T185** (pri 2, bug) — bash reader-grace keeps already-read output when a
  grandchild holds the pipe: landed `068e223` (impl `a0051b2`, glm 58/80
  first-try clean, +208/-23 src/tools.rs; orchestrator gap-closer `44faf3b`).
  kimi REQUIRED PASS (routing d1790865466-14, verdict d1790870511-16): a
  6-mutant study killed 5; the m6 survivor (`acc = chunk` — a >64KiB
  multi-chunk coverage gap the whole pre-existing suite missed) was
  discharged PRE-merge by the orchestrator's RED-proven gap-closer (the
  mutant fails the new byte-count assertion; revert green). The arc fought
  TWO T55-class foreign-binary poisonings: (1) the T184 children's goal
  gates built into `target-shared` because MY branch-side check re-key was
  invisible — the delegate `spec:` path reads MAIN's copy — dispatch fix
  `74d3331` + validator-diagnosed; (2) a Sep-29 stale `eval_digest` binary
  false-red on my own T184 review gate, fixed by the T55 touch-rebuild.
  Validator died at the 50-minute wall with its final clean run in flight →
  ONE T63 resume → verdict on a quiet provenance-pure run (1061 lib = its
  exact content). What the validator caught beyond m6: the delegate-launch
  stub family's EXTERNAL-load blindness (T151's timing_guard is in-process
  only) — fed to the next eval's flake watch.

- **T184** (pri 2, feature — F14 phase 1 reframed to telemetry) —
  context-economy telemetry: landed `198d664` (impl `7c77d85`, +507/-19,
  9 files — Event::Trim 4-field events-sink-only + cache_read/
  cache_creation on Event::Usage + trim.rs caller-side TrimStats seam
  keeping transcript_trim's bool signature + eventlog serialization
  incl. the resume-path log_trim + driver both call sites + digest
  cache counters and "trim fires: N" + README bullet). Impl glm died at
  80/80 with the work uncommitted-but-nearly-complete → ONE T63 resume →
  accepted; the child HONORED the growth-stop clause (dropped an
  over-cap scripted end-to-end leg itself — the filing-time discipline
  working). kimi REQUIRED PASS (routing d1790870669-21): 4 reqs traced
  incl. the api.rs→driver data chain; m1-m3 mutants CAUGHT in T79
  parallel legs; m4 (loop-path Trim emission leg) unrun — the spec's own
  120k-token-run prohibition made it review-asserted (sanctioned gap);
  the validator ALSO caught a stale pre-T184 release binary in
  target-shared-validate silently skipping all 6 new legs on its first
  green run — the THIRD T55-class artifact event this cycle. Cycle-level
  lesson: role-keyed dirs bound the class but do not end it — a dir
  shared across TIME by different checkouts stays last-builder-wins;
  touch-the-target-file before review/validation gates is the cheap
  guard (feeds the next eval).

### Cycle 84 (2026-10-01) — glm routine freshness-skip cycle (freshness held: EVALUATION.md same UTC day, 1 todo row T181; skip record d1790856978-1) — 1 landed (T181) — queue DRAINED

**Wrap note.** Routine glm freshness-skip cycle (skip record d1790856978-1; T81 routing — glm never evaluates). The whole cycle was the T181 arc, end to end in ~70 min wall: worktree+warm build 49s → impl glm first-try goal-accepted 20/80 (~10 min child wall) → kimi REQUIRED PASS 4/4 mutants (32/60, ~44 min wall, zero blocking findings) → merge, flip, push. Queue DRAINED — TODO.md has zero todo rows for the first time since the cycle-82 wrap; the next cycle (fresh-eval, kimi-routed per T81 since the queue half of the predicate fails) files from the delta corpus + FEATURES.md (F12 phase 2 keyed providers is the standing roadmap pull). Bookkeeping: 4 decision_log records (skip, routing, verdict, outcome); harvest events-t181-impl-validate + LEDGER-t181-validate in .chug/ (local, gitignored by design); leftover /tmp/chug-loop-t183 worktree from the cycle-83 mid-arc handoff removed post-harvest-check. **No release tag:** 1 item since v0.11.0 < 3, no FEATURES check-off. Final gates at HEAD (target-shared-main): build ✓, clippy -D ✓, nextest --release 1311/1311 ✓, todo_consistency 19/19, eval_outcomes_carry 4/4.

**T181 done 0656f65 (impl first-try goal-accepted 20/80; validation-routing d1790857703-2, verdict d1790859693-3).** The bash tool description gains ONE sentence for the BSD-sed `,+N` trap (the T22 class one level down — two orchestrator streams, both model families, hit `sed: N: ",+Np` on macOS and recovered 1–2 iters each): "macOS sed is BSD sed: GNU range forms like `,+N` do not exist — use `awk` or `sed -n 'N,Mp'` with absolute line numbers." Placed immediately after the 120s driver-cap wording and BEFORE T22's timeout note — the sole placement keeping T22's unmodified `ends_with("instead.")` final-position assertion (the validator named this placement as its one non-blocking observation, consistent with the spec's Tests section); T22's sentence-count pin extended 4→5 exactly as the spec sanctions; the 120s/`CHUG_BASH_TIMEOUT` wording untouched. New sibling pin `bash_description_pins_bsd_sed_no_gnu_plus_n_range_form` asserts the LIVE `tool_schemas()` bash description carries `,+N`, BSD, `awk`, `sed -n 'N,Mp'`, "absolute line numbers" (T22 non-vacuousness shape); RED-proof in the commit message (sentence revert → pin fails on `,+N` while T22's pin passes). Impl glm first-try clean 20/80 (second post-T178/T179 small row in a row after T182's 29/80 — the §2.1 prediction now 2-for-2). kimi REQUIRED (src/tools.rs letter) PASS: gates independently re-run 1311/1311 nextest --release + clippy -D, **4/4 mutants killed** incl. the RED-proof revert mutant, tree byte-clean. Orchestrator gates wt+main: 1311/1311 nextest --release, clippy -D clean, todo_consistency 19/19. Harvest: events-t181-impl-validate-20261001-130140.jsonl (shared worktree, both segments) + LEDGER-t181-validate-20261001-130140.md. Land: fast-forward at 0656f65.

### Cycle 83 (2026-10-01) — kimi mandatory fresh-eval cycle (queue drained at cycle-82 wrap 7646250; T81 routing per the cycle-82 handoff) — 4 rows filed (T180–T183 + F14), 2 landed (T180, T182), T183 MID-ARC at wrap (recovery recipe below), T181 unworked — no tag (2 items < 3 since v0.10.1)

**Wrap note.** Cycle 83 ran the mandatory fresh eval (queue empty at the cycle-82 wrap) and filed T180 (the F12 roadmap pull), T181, T182, T183 plus F14 on FEATURES.md (12 eval-triage records). **Landed and pushed:** T180 (web_search, F12 phase 1 — kimi REQUIRED PASS 8/8 mutants; impl AND validator each recovered by exactly ONE resume) and T182 (decision_log always-reminder — first post-T178/T179 small row, first-try 29/80; the §2.1 prediction 1-for-1; T175's re-key doctrine exercised into the validator overlap). **T183 MID-ARC at the wrap wall:** impl run1 died 80/80 in ~9 min wall (glm's fastest observed burn, ~9 iters/min) with +596/-18 code+tests UNCOMMITTED → ONE resume (recovery d1790847576-22) still FLYING at wrap (pid 6074, 42/80 at last poll, goal_seen latched = ≥1 goal_complete call, branch loop-t183, worktree /tmp/chug-loop-t183). **Recovery recipe for the next cycle (T183):** (a) collect the child — if goal-accepted: review the diff (expect src/delegate.rs env-map + delegate tests + tools.rs schema desc + LOOP-SPEC step-1/2 template adoption + README delegate bullet), run worktree gates (nextest release + clippy -D), kimi REQUIRED validation (touches LOOP-SPEC doctrine + src/delegate.rs launch mechanics), merge, flip, push; (b) if it died at budget with work COMMITTED: orchestrator-finish per T150 (same review+gates+kimi arc, no second resume — the ONE resume is used); (c) if died UNCOMMITTED and committable: finish in the worktree (the run1 diff built cleanly per the child's own clippy claim mid-resume — verify independently); (d) harvest STILL OWED: cp the worktree's .chug/events*.jsonl as events-t183-impl-* before any worktree removal (the run1 segment rotated at the resume launch; both segments live in the worktree .chug/). **T181 stays todo with its spec cold-start-ready** (kimi REQUIRED by the tools.rs letter; description-string-only diff). **No release tag:** 2 items landed since v0.10.1 < 3 and no FEATURES.md check-off (F12 is phase 1 of 2 — not checked). Final-state gates: main was green at ddb71c7 (nextest release 1303/1303 post-T182; the cycle's three bg main gates all green: 1291 → 1302 → 1303). Cycle-level notes: the T173 measure clause now TRIPS literally (3/8 minutes-deaths: t176, t177, t180 — remedy stays filing-time spec-size discipline, no further raises); the T82 stub-spawn + T31 port-theft flake families each fired inside t180's goal-gate checks (next-eval T172-residual watch evidence); the '(output truncated: reader did not drain)' class fired a SECOND time (this orchestrator's own nohup bg launch — watch-list per triage d1790836694-10); T182's success criterion (options-got-missing fumbles → 0) starts measuring next delta.

**T183 done 786bc2d (impl e6cfff0; impl-recovery d1790847576-22; validation-routing d1790850871-1, validator-recovery d1790853121-2, verdict d1790854940-3).** `delegate` launch now takes an optional allowlisted `env` map: string→string entries (keys hand-rolled to exactly `^(CARGO_|CHUG_|RUST)[A-Z0-9_]*$` — no regex dep for one pattern; ≤16 entries; values ≤4 KiB, no NUL), applied at spawn as inherited env → T144 scrub → explicit entries (explicit WINS over the scrub — the scrub guards the ABSENT case, `env` is the EXPLICIT case, ordering commented at both the fn and the call site), launch payload echoes applied KEYS ONLY (absent → byte-identical); tools.rs schema + description carry the regex/caps/ordering/fallback; LOOP-SPEC adopted in the same commit (step-2 delegate block env line, the goal's export STAYS as defense-in-depth, T161 impl-a/impl-b env carriers, step-4 validator target-shared-validate via env); tests/shared_target_dir.rs gained per-carrier count_eq pins (validate carrier 2→3 + new `t183_delegate_launch_templates_carry_the_env_map`); README delegate bullet updated. Impl glm: run1 died 80/80 at ~9 iters/min with the code+tests UNCOMMITTED → ONE resume (d1790847576-22) committed e6cfff0, finished the doctrine adoption, and its goal-accepted check ran 6.5 min past the resume's own arrival — it was ALIVE at the cycle-83 wrap (the mid-arc handoff), accepted 10:32Z, exited clean. kimi REQUIRED validator (routing d1790850871-1): died 60/60 mid-mutation with the M2 mutant APPLIED → ONE resume (d1790853121-2, restore-mutant-first instruction) re-verified 1056/1056 bin + 33/33 pins byte-clean, but its goal was rejected on a LOAD FLAKE (spec check cargo test at 222s under its own mutation builds — the T82/timing-flake family; quiet re-runs: 1056/1056 in 41s, release 1310/1310) and the driver process died post-rejection with the verdict unannounced — verdict recovered intact from the transcript's goal_complete summary: **PASS, reqs 1–6 traced, 4/4 mutants CAUGHT** (order-flip, value-leak, cap 16→15, LOOP-SPEC env-line revert — independently replicating the commit's RED-proof). The verdict's two declared coverage gaps were closed by the orchestrator: clippy -D + the release T82 gate (my worktree run: 1310/1310 nextest --release + clippy clean), and the two prepared-but-unrun mutants RED-proven personally (allowlist always-true → parse test FAILED; NUL-check dropped → parse test FAILED; both byte-restored, re-run green) — **6/6 mutants total**. Post-merge main gates 1310/1310 + clippy -D under target-shared-main. Non-blocking findings: commit header +316/-27 vs actual +723/-39 (2× the ~360 estimate — the estimate-band class again); collect.rs timing assert 5s→30s (test-only, justified in-comment — a 5.63s scheduler stall caught by the assertion, not a contract break); pre-existing T144 test-hygiene corner (early-assert failure leaves CHUG_DELEGATE_BIN + a sleeping stub); one vacuous `err.contains("")` half (covered by sibling legs). **T183-meta note:** the delegate `env` param was passed at BOTH this validator's launches and was silently dropped both times — the orchestrator binary predates the merge, so the field is unknown and ignored (the missing `env_keys:` echo is the tell); the goal-carried export line carried the dir instead, exactly the defense-in-depth the spec pins. The env param becomes operative for the NEXT cycle's children (post-merge binary). T173 measure clause: this impl was a minutes-wall arrival but goal-ACCEPTED (no death) — census stays 3/8.

**T182 done 81ddb6c (impl 33a39af, first-try goal-accepted 29/80; validation-routing d1790845110-16).** decision_log's validation errors now ALWAYS carry the full contract: `validation_message` appends `CONTRACT_REMINDER` unconditionally — the 5-of-6-fields fumble shape (all keys recognized, one missing) previously took the legs-only path, so the error named the ONE missing field but never re-stated the contract and the retry could drop a DIFFERENT field (the cycle-83 census: 5 instances in 3 cycles; T88 built the reminder, T168 the description, the gap was the conditional). The received-keys echo stays conditional on unrecognized keys; the success path is byte-identical; new exact-pin leg `decision_log_five_of_six_fields_still_carries_the_contract_reminder` plus the multi-missing/each-field legs extended to the always-reminder shape. Launched INTO the T180-validator overlap under pattern (i) (disjoint: decisions.rs vs websearch/tools/README; spec check re-keyed to target-shared-impl-a at dispatch, T175's doctrine exercised — ed99464). kimi SKIPPED (decisions.rs not in step-4's REQUIRED list) — orchestrator personal RED-proof: dropping the unconditional reminder failed exactly 5 decision_log tests (the new leg + the exact pins), byte-identical restore, 19/19 green re-run. Gates: nextest 1292/1292 (+1 = exact) + clippy -D in worktree and post-merge main. **T178/T179 effect note: the first small-row impl ran first-try clean at 29/80 in ~7 min of child wall — the §2.1 prediction (small-row minutes-deaths → ~0) is 1-for-1 so far.** Measure next delta: `options … got missing` fumbles → 0 (T182's success criterion). Harvest: events-t182-impl-20261001-092626.

**T180 done 770487d (impl 35262d3; recovery-routing d1790840438-13; validation-routing d1790843483-14, verdict d1790846385-17, validator decision d1790844854-1).** F12 phase 1 landed: the builtin `web_search` in new src/websearch.rs — per-call provider seam (`CHUG_WEB_SEARCH_PROVIDER`; unknown value → error naming received+valid; never latched), zero-config DuckDuckGo HTML provider (class-token anchors tolerant of attribute order/quoting, `uddg` redirector percent-decode, snippet-less-block scoping), honesty legs (non-2xx preview mirroring web_fetch; a zero-block page says "0 results for <q>" vs a byte-count markup-drift note — never a silent empty list), `max_results` default 5 clamp 1..=10, fixed 20k output cap, the test-only `CHUG_WEB_SEARCH_BASE_URL` seam hidden from the description. webfetch's transport factored into shared `fetch_raw`/`bounded_get_body`; tools.rs schema+dispatch; README tool list + two→three-exceptions sentence + Development-layout line (the T95 guard caught the miss — one of the impl's four check rejections and the only REAL one); plan.rs run-mode tool-list pin and the T41 three-exceptions pin updated in-commit. Impl glm died at the 50-minute wall 74/80 (b3abb2a committed + cap-test fixups uncommitted) → ONE resume → goal-accepted; the other three rejections were diagnosed and honestly attributed (T31 port-theft residual, a self-inflicted background-run load collision, the T82 stub-spawn family — the latter two are flake-family evidence for the next eval's T172-residual watch). **The T173 measure clause now TRIPS literally: 3 minutes-deaths in the clause's 8-child window (t176, t177, t180)** — the remedy stays filing-time spec-size discipline (T180's own 1050/-58 actual = 2.5x its ~420 estimate is the instance). kimi REQUIRED validator PASS (died 60/60 with the final mutant batch in flight → ONE resume → verdict): all 7 reqs traced, gates independently re-run (release build, clippy -D, nextest 1302/1302 under target-shared-validate), 8/8 mutants CAUGHT (uddg, snippet-scope, clamp, honesty, provider-env, readme-revert RED, cap, non-2xx) via T79 parallel legs ≤3 after the validator self-caught its invalid `--lib`-on-binary-only batch and re-ran — tree byte-clean, throwaway worktrees removed. Non-blocking: the 2.2-2.5x estimate overshoot (cycle-83 §3 calibration confirmed again), a stale "seventeen tools" comment (fixed in the flip commit), two spec-silent parser/env edge notes (non-unicode provider env falls to the unset leg; find_tag_with_class walk-back edge). Orchestrator gates: nextest 1302/1302 (+11 = exact) + clippy -D in worktree and post-merge main. Harvests: events-t180-impl-20261001-083136, events-t180-validate-20261001-091454, LEDGER-t180-{impl,validate}-*.

**T178 done 0c0ba88 (impl 875fc50; validation-routing d1790832928-3, verdict d1790834339-4).** loopd now exports `CHUG_BASH_TIMEOUT=300` for the whole fleet — one line beside the launch block, with the comment naming the cycles-76-78 death census (3-4 `timed out after Ns` bash deaths per stream: cold/warm nextest gates and cold cargo builds exceed 120s; every death buys a retry whose wall feeds the minutes-death census), the precedence chain (`--bash-timeout` flag > env > 120 default), and why the persisted export is harmless (affects only chug bash-tool calls, never the supervisor's own build). Children inherit it via the orchestrator's env (delegate has no env parameter — the T47 lesson). LOOP-SPEC's step-3 gate templates and the step-5 post-merge reference now carry `alarm 280` — below the 300s cap so the inner bound fires first; the cargo-culted `alarm 600` could never fire under the 120s default and is gone file-wide. No src/ changes: interactive/chat users keep 120. README Continuous-mode gains the fleet bullet. Pins updated in-commit: a new `loopd_exports_bash_timeout_300_before_the_launch` leg plus the alarm-280 and README-bullet carriers. **The census row fired at EVERY level while landing it:** this cycle's orchestrator RED-proof leg died at the 120s cap mid-rebuild (T179's), and the impl child's own spec check died twice mid-compile before going green warm. kimi REQUIRED PASS (routing d1790832928-3, verdict d1790834339-4): 47/47 spec check + 1291/1291 nextest + clippy -D independently under target-shared-validate; 4/4 parallel T79 mutants (export-drop, comment-drop, alarm-revert, readme-drop) each failed exactly the pin the commit message names; overlap judgment declared; tree byte-clean; one disclosed non-blocking observation (META-SPEC's illustrative alarm-600 T6 example intentionally untouched — out of scope per the spec).

### Cycle 82 (2026-10-01) — glm routine freshness-skip cycle (freshness held: cycle-81 wrap note c4d778c landed 03:39 UTC) — Phase 1 skipped per skip-rule record d1790826770-1 — 2 landed (T179, T178), QUEUE DRAINED, release v0.10.1 tagged

**Wrap note.** Both queued rows landed and pushed; the queue is EMPTY. Release v0.10.1 tagged at this wrap (trigger: 4 items since v0.10.0 — T175, T176 in cycle 81 + T179, T178 here; patch bump: T179 is a fix, T178 is doctrine/DX, no FEATURES.md check-off; pairing verified via check-tag-version.sh, tag message = generated notes since v0.10.0; gates green at HEAD 483b05d: nextest --release 1291/1291 + clippy -D + release build under target-shared-main). **Handoff for cycle 83 (cold-start, zero human words): the queue is EMPTY, so the freshness predicate's queue half FAILS — a mandatory fresh eval on kimi per T81** (exactly what the cycle-78/81 wraps predicted each time; loopd will route it). Next-eval inputs ready: the delta corpus is cycles 80-82 (the digest covers it); open watch items — (a) the T173 minutes-wall measure clause TRIPPED (T176 was the 3rd ~8-impl-children 50-minute wall death; the remedy named there is spec-size discipline, not further raises), (b) the T179 in-comment known gap (a worktree commit no longer retriggers the build script → baked hash can go stale — the gitdir-resolving rerun target follow-up), (c) META-SPEC's disclosed alarm-600 illustrative drift (the validator named it non-blocking; fold into the next doctrine touch or leave pinned as-is), (d) the T178 effect is unobserved in a live child until the next impl arc runs under loopd's re-exec (the running supervisor has the old env; the export lands from the next loopd restart). Harvests this cycle: events-t179-impl-20261001-044900, events-t178-impl-20261001-053526, events-t178-validate-20261001-055800 (both worktrees removed post-harvest; child LEDGERs were seed shells, not harvested). Records: skip d1790826770-1, routing d1790830102-2 + d1790832928-3, verdict d1790834339-4, outcomes d1790834480-5/-6.

**T179 done c90776c (impl feb9c80; validation-routing d1790830102-2).** build.rs's worktree always-stale rebuild tax fixed: `emit_git_rerun_hints` now reads `.git/HEAD` FIRST and emits `cargo:rerun-if-changed=.git/HEAD` (and the ref hint) only on read success — the cycle-81 gate-enablement shape. In a worktree `.git` is a FILE so `.git/HEAD` never exists as a path and cargo treated the missing hint target as ALWAYS stale (`StaleItem(MissingFile)`, CARGO_LOG fingerprint=trace) → the build script + whole crate rebuilt on EVERY cargo invocation in every worktree (~40-95s release — the hidden tax behind slow worktree gates, eaten gate windows, and child budget deaths). Worktree builds now watch nothing and stay cache-clean; the main checkout's behavior is unchanged (`.git/HEAD` exists and is still watched). Doc-comment carries the discovery, the evidence command, and the CHUG_GIT_HASH stub-test finding (documented, not changed); the known gap (a worktree commit no longer retriggers the build script → baked hash can go stale) is honestly named in-comment as out of scope. tests/build_script_shape.rs pins the ordering (read < else-return < emit, T48 windowed needles, runtime-cwd resolution per T47) + the doc-comment evidence carriers. RED-proofs: the child's revert leg AND the orchestrator's independent mutant (pre-T179 shape → ordering pin FAILED rc=101, read 97 < emit 32), byte-identical restore, green re-run. Effect verified by both: two consecutive `cargo build --release` → 1 then 0 Compiling lines (child measured 48.43s → 3.23s; orchestrator re-verified 0/0). kimi SKIPPED per routing d1790830102-2 — build.rs is not in step 4's REQUIRED list and every acceptance criterion was exercised hands-on by the orchestrator (nextest 1290/1290 + clippy -D in worktree and post-merge main). **Live taxonomy datum: the orchestrator's own RED-proof leg died at the bash tool's hard 120s cap mid-rebuild** (the T178 census row firing for the orchestrator, not just children) — backgrounded via nohup and completed, but T178's CHUG_BASH_TIMEOUT=300 fleet export would have covered it.

### Cycle 81 (2026-10-01) — kimi routine freshness-skip cycle — 2 landed (T175, T176); T178 deferred to the next cycle (4-minute wrap wall hit first — its spec is cold-start-ready)

**Wrap note.** T178 (loopd CHUG_BASH_TIMEOUT=300) never dispatched: two serial doctrine arcs (each impl + kimi validation) consumed the budget; the row stays `todo` with its spec ready — a cold next cycle can dispatch immediately (solo, loopd + LOOP-SPEC step-3/5 alarm-600→280 re-key). Release check: 2 items since v0.10.0 (< 3) and no FEATURES check-off → no tag this wrap. T179 (pri 1, build.rs worktree rebuild tax) is the queue's top row for next cycle.

**T176 done b46b730 (impl 9ed1246; recovery-routing d1790822838-6, validation-routing d1790823305-7, verdict d1790825494-8).** LOOP-SPEC step-2's impl goal template now runs clippy with `-D warnings` (the bare "clippy + test green" let an impl child honestly claim clippy-green while warnings stood — the cycle-77 T166 needless_lifetimes instance is named as evidence) plus a semantics sentence after the launch block (`cargo clippy --all-targets -- -D warnings`, zero warnings, not merely exit-0); sweep-the-family verified exactly ONE goal-template surface carried the bare phrase (step 4's FAIL arc reuses step 2's template with findings pasted in; lines 259/532 are gate descriptions); META-SPEC untouched. Pin leg (z): the sentence exactly-once in step-2's window with a wrap-insensitive revert detector (the OLD bare phrase must occur ZERO times file-wide). Impl glm died at the 50-MINUTE wall 67/80 with work complete+committed → orchestrator-finish (third minutes-wall death in ~8 impl children — the T173 measure clause is tripping). kimi validator PASS 8/8 mutants killed — every assert leg of the pin independently proven (count-0, count-2, bare-phrase, ordering, flag window, evidence window). Gates 1288/1288 + clippy -D in worktree and post-merge main.

**T175 done b7a113a (impl d4663c7; recovery-routing d1790814515-2, validation-routing d1790816416-3, verdict d1790819131-4).** LOOP-SPEC's Pipeline-overlap paragraph gains ONE sentence: at dispatch into the 2-impl overlap the orchestrator ALSO re-keys the spec's `check:` line export to the SAME role-keyed slot before launch — the goal's export and the check's export must name one dir (T144's scrub makes the check's own export the only target dir the goal gate sees; T52 artifact-name class makes a foreign dir a correctness hazard); cycle-77 evidence (t165's green-work rejection, fixed mid-flight 607e877) named in the doctrine; solo default unchanged. Pin leg (y) in tests/loop_spec_recovery.rs: load-bearing tokens exactly-once inside the paragraph window, ordered after the T161 slot sentence and before the Doctrine-items rule, with T48 needle self-checks. Impl glm died 80/80 with work complete+committed → orchestrator-finish (no resume burned, T63/T150). kimi validator PASS 5/5 mutants RED (delete/duplicate/move/placement-flip/token-corrupt), gates independently re-run 1287/1287 + clippy -D. **Cycle-81 DISCOVERY: build.rs's `rerun-if-changed=.git/HEAD` makes EVERY cargo invocation in ANY git worktree a full crate rebuild** (`.git` is a file in a worktree → the watched path never exists → `StaleItem(MissingFile)` always-dirty, confirmed via CARGO_LOG) — the hidden tax behind slow worktree gates and child budget deaths; orchestrator carried a 2-line uncommitted gate-enablement patch in the worktree (hint emitted only when readable; reverted byte-identical pre-merge, disclosed to the validator in its goal); T179 filed for the real fix. Also: exporting CHUG_GIT_HASH in the gate env breaks the delegate-launch stub tests (empirical — unset it).

### Cycle 80 (2026-09-30) — glm routine freshness-skip cycle — 4 landed (T177 af4f4b4, T170 88119dc, T174 ab421d3, T171 7b9b2bf — F11 phase 1b CLOSED), doctrine bundle T175+T176+T178 deferred with reason — full narrative in git (row-flip commits + todo: flips).


### Cycle 79 (2026-09-30) — fresh eval (kimi; 8e832d2: 10 rows + 16 triage records) — 3 landed (T173 minutes-budgets 553928a, T172 load-lock deflake bae88d5, T169 F11-1b-i pull 3796983), 7 deferred with reason (minutes binding; iteration-ceiling deaths the new census class; T63 resume textbook; kill-rule confirmed x4) — one-line backfill at the cycle-84 wrap (entry absent since an earlier wrap's compaction; full text in git 8b4db7d and the todo: commits).

### Cycle 67 (2026-09-28/29) — routine (freshness-skip) — codex-intake pri-2 queue — T135 LANDED 611916e (merge + stale-flip cleanup), T136 arc mid-flight at wrap (recovered + landed cycle 68; its crash-safety class completed by T145 in cycle 70) — compacted at the cycle-73 wrap (last-6 rule; the full text lived in git until the cycle-72 eval commit dropped it — restored verbatim for 69/70/71 below, 67/66 re-compacted from git).

### Cycle 68 (2026-09-29) — routine glm freshness-skip (queue non-empty, eval fresh) — codex-intake pri-2 queue (T137 merge pickup + T138 resume + T139)

**T137 — loopd launches stale binary after a failed build (pri 2, landed
08f0a39+46ec1a9 rebased ff; orig 3c0e403+f827659, cbfa952+f827659 pre-cycle-68-rebase).** The codex review's
§1 HIGH supervisor item: loopd.sh never checked `cargo build`'s status,
so a broken merge relaunched the previous release binary all cycle. The
round-1 validator FAILED it on the real class the fix created: with
`set -euo pipefail`, the pipelined `ps | grep -q` single-driver guard
flipped FAIL-OPEN under a SIGPIPE'd ps leg (the 83KB-ps
EARLY-MATCH-FLIPPED-FALSE proof) — a live driver would no longer block a
second launch. Fix-up 8a823ae de-pipelines that guard (rc-latched ps
capture + herestring grep; probe failure fails CLOSED) and sweeps the
class (all remaining pipelines enumerated). Round-2 kimi PASS
d1790651023-2: F1 re-probed byte-exact BOTH directions (fixed shape
skips a live driver under the 2.6MB early-needle attack; reverting the
shape re-opens it — attack teeth confirmed), class sweep independently
re-enumerated complete (:127/:344 masked, probe+verdict de-pipelined),
full pipefail leg audit clean incl. a SHELLOPTS non-export probe, 4/4
prescribed mutants died (M-A pipeline-revert killed twice rc 101, M-B
|| true-swallow 0.87s, M-C bare-capture set -e death signature, M-D
HALT-threshold — covering round-1's unexecuted M6 path). 3 non-blocking
findings carried (validator leg-harness rc-echo inversion; bash≥4
SHELLOPTS inheritance note; hardcoded sleep 120 observation).
Post-merge nextest 1064/1064 (target-shared-main). Cross-cycle lesson
confirmed: a fix that hardens one guard can silently break another —
adversarial round 2 on the FIX-UP (not just the feature) is what caught
F1.

**T138 — mcp.json executes before permission enforcement (pri 2, landed
37f8edc rebased ff from 0712c61).** The codex review's §2 HIGH
supply-chain item: a repo-controlled `mcp.json` spawned its stdio
command during startup — BEFORE permissions loaded — so denying
`bash`/`mcp__*` never prevented the checkout's own code from executing
(mcp.rs:366/469, driver.rs:295/658). Fix (impl 0712c61, glm, 96/80
across two segments after a T63 resume): `McpRegistry::new` is now
parse-only; a new `start(&Permissions)` does spawn+handshake and is
called by drive_loop immediately after `Permissions::load` (run AND
chat; plan mode structurally never starts MCP). The spawn gate
`Permissions::mcp_spawn_block_reason` judges deny rules alone — no tool
list exists pre-spawn — with three blocking shapes: a whole-namespace
deny (probed with TWO distinct synthetic servers so a server-specific
glob never reads as deny-everything), a whole-server deny, and (stdio
only) a `bash` deny — arg-matcher rules exempt. Credentials leg: stdio
spawns env_clear + fixed baseline (PATH/HOME/TMPDIR/LANG/LC_ALL +
windows extras) + the entry's env map only. RED proven with flag-file
driver tests (the command ran despite denies pre-fix; reproduced at
parent ae257da by the validator). kimi REQUIRED PASS d1790652558-6:
gate legs verified by mutation (6 mutants, M2–M6 killed; M1 drop-2nd-
probe SURVIVED — non-blocking, availability-only delta — plus
plan-guard test gap and doc nits, all carried); validator's 1st segment
budget-died 50/50 mid-mutant-batch, resumed per T63 and finished at
iter 4 (5th consecutive resume success). Post-merge nextest 1077/1077
(target-shared-main).

**T143 — per-request max_tokens too small for thinking models (pri 1,
operator intake, landed 00adb88 rebased ff from 67c4913).** A live
operator session died at iteration 9: the request-side `max_tokens` was
hardcoded 8192 in src/api.rs, and GLM thinking blocks consume the SAME
budget as the response content — a ~7KB `write_file` JSON overflowed
the cap and truncated mid-stream (the REQUEST-side sibling of T141's
stream-side detection). Fix (impl 67c4913, glm, 140/80 across two
segments after a T63 resume — the first segment died 80/80 in 4m45s of
fast small steps): `DEFAULT_MAX_TOKENS = 32768` (the operator-proven
value, not their stashed diff verbatim), `$CHUG_MAX_TOKENS` env +
`--max-tokens-per-request` CLI on run/plan/chat, resolution flag > env
> default via a pure main.rs helper; the cap flows through
RunConfig/PlanConfig/ChatConfig into `Client::new(model, cap)` and out
the single request-body site (api.rs:1350); T38's budget-abort advisory
gains a `raise CHUG_MAX_TOKENS` remedy line ONLY when
stop_reason=max_tokens AND the cap is below 32768 (byte-identical
otherwise); eventlog run_start records the cap. T15's cumulative
`--max-tokens` independence pinned (clap introspection + both-set
test). kimi REQUIRED PASS d1790655550-10: RED re-proven at parent
(body==8192, assert vs 32768 fails), remedy boundary + run_start
field-drop mutants killed, 8/9 mutants (m9 observ-sink survivor
non-blocking, outside spec surface, carried) + 3 minor coverage
observations. Post-merge nextest 1084/1084 (target-shared-main).

**T139 — editable spec check bypassed bash deny + risk gate (pri 2,
landed b5f1fa0+bb02823 ff-merge).** The codex review's §2 HIGH
self-modification item: goal_complete's spec `check:` command executed
via run_shell DIRECTLY (driver.rs:822/1390), outside permissions, hooks,
and the risk gate — deny bash + permit file editing let the model edit
its own spec to an arbitrary `check:` shell command and execute it the
next iteration. Fix (impl b5f1fa0, glm, 101/80 across two segments after
a T63 resume): verify() now routes the check through the SAME gate chain
as an ordinary bash call, in dispatch order — permissions.check deny →
PreToolUse hook veto → risk gate (fail-open judge + allow-destructive
override unchanged) → run_shell — and fails CLOSED: a blocked check
never executes AND never verifies (new VerifyOutcome::Blocked, the goal
rejected with "check command blocked by policy", blocked≠failed in the
model message); PostToolUse intentionally not fired (T83's advisory
follows dispatched tools only); the legit path is preserved (default
permissions still gate `check: cargo test`). Class swept: exactly two
production run_shell callers exist (tools.rs:582 the gated bash tool,
driver.rs verify()), and every sibling candidate (hooks = operator
config, mcp argv = T138-gated, delegate/git/ps = fixed argv) carries no
model-controlled shell string. RED proven END-TO-END (deny bash + edited
spec + goal_complete executed the attacker command; the trigger test
failed at the right assert on parent e56979d). kimi round-1 FAIL
d1790658499-14 — NOT a code bug: the production chain verified correct
in every leg, but the REORDER mutants M4 (risk-gate-before-permissions)
and M11 (hooks-before-permissions) survived the family AND the full
suite, violating the spec's own "a mutant that reorders or drops a leg
must be caught" bar. Fix-up bb02823 (test-only, +246, driver.rs
byte-identical) pins ALL THREE adjacent gate-chain pairs with
RED-proven killing tests — the sweep-the-class remedy from the cycle-33
lesson closed in ONE round what one-leg-at-a-time would have spread over
three. kimi round-2 PASS d1790660325-15: M4/M11/M12 all die on their
order-signal pins (RecordingAllowJudge consultation flag), gates
1092/1092 zero flakes. Post-merge nextest 1092/1092
(target-shared-main).

**CYCLE-68 CYCLE-LEVEL NOTES (wrap).** FOUR items landed (T137,
T138, T143, T139 — three codex-review pri-2 HIGHs + one operator pri-1
report), three through full kimi REQUIRED rounds and T139 through the
era's cleanest FAIL→fix-up→PASS arc (round-1 FAIL was a TEST gap, the
code verified correct; the sweep-the-class fix-up closed all three
gate-order adjacencies in ONE round). Release: v0.4.1 tagged at wrap
(4 items since v0.4.0, no FEATURES check-off → PATCH). T63 resumes went
4-for-4 this cycle (T138 impl, T138 validator, T143 impl, T139 impl —
all resumed children finished their arcs; era total 8-for-8). Iteration
economics: 2 of the last 4 impl children died at 80/80 with the work
done (T143 seg-1 in 4m45s of fast small steps; T139 seg-1 pre-commit
with green tests) — the T110 measure clause trips again; the remedy
stands (filing-time ~500-line estimate ceiling, META-META spec-quality
bar), no further iteration raises. Harvest hygiene: one mislabel caught
BEFORE worktree removal by the T19 read-back practice (T137's "024850
validate2" copy was the fix-up stream dup; the real round-2 stream was
the worktree's live events.jsonl — re-harvested correctly; both old
files removed). Origin divergence reconciled at cycle start (operator
intake 1257b05 rebased cleanly; T137's flip re-hashed
3c0e403+8a823ae→08f0a39+46ec1a9 — row refs corrected in c3f2568).
T129 (feature pri 2) + T131 (pri 4) deferred on wall-clock — bugs
outrank features, queue order held; T129's spec is ready, next cycle
works it FIRST. Carried to the next eval: T138 M1 (drop-2nd-probe
mutant, availability-only delta), T143 m9 (observ-sink generation
maxTokens unpinned at non-default caps) + 3 coverage observations,
T137's 3 non-blocking (leg-harness rc-echo inversion, bash≥4 SHELLOPTS
note, hardcoded sleep 120), T135 M2 + F1, T136's 3 non-blocking
(update_ledger wholesale write, legacy chat-shape repair gap,
write_atomic dir-fsync/tmp-reap nits), T128 M4, T141 M1.

### Cycle 71 (2026-09-29) — routine glm freshness-skip (queue non-empty, eval fresh) — ALL FOUR landed (T147+T148+T149+T150), queue EMPTY

**T150 — todo-row spec estimate pin (92f987a, merge of loop-t150:
3f5a99a).** The T110 filing-time estimate ceiling now has a mechanical
guard: every `todo`-status TODO row's named spec must carry an
`estimate: ~<number>` line (filing time only — 121/141 legacy specs
predate the rule and are exempt history). Tests-only +195/-25 in
tests/todo_consistency.rs with the T8 row parsing lifted into shared
helpers (behavior-identical). Pure function over (table text,
spec-reader closure) — repo-live leg plus 4 synthetic legs + a
fail-closed unreadable leg. The child RED-proved 4 mutants (flag-arm
suppressed, always-fires, status-filter dropped, ~N requirement dropped)
with sha-verified byte restores; the orchestrator independently re-ran
the vacuous-guard mutant — both must-flag legs RED. kimi SKIPPED per the
spec's own routing (tests-only tooling guard). Process note: the child
ABORTED on the 35-min TIME budget at 48/80 AFTER committing the work —
the final full-suite gates chased the documented default-parallelism
load-flake class; the committed-work variant of the budget-death recipe
applied (orchestrator review + gates + merge, routing
d1790691515-11, no resume burned). Gates 1135/1135 nextest --release
worktree + main. QUEUE NOW EMPTY.

**T149 — validator budgets 60/40, doctrine (8699a70, merge of loop-t149:
10b6f6a).** The validator-side T102: after 4 validator budget deaths in 4
cycles (T134/T138/T142 at 50/50, T137 MINUTES-bound at 30m08s with the
verdict written but unannounced), LOOP-SPEC step 4 now launches validators
at max_iters 60 / max_minutes 40 with the census in the rationale and the
measure clause (>1 of the next 8 dying at 60/40 unannounced → next eval
considers trimming default mutation-leg counts, not further raises). Step
2's T63-resume echo aligned; META-SPEC §6's template (which the grep found
DOES name validator budgets) aligned in the same commit — supersedes T32's
do-not-touch stance for this row only; src/tools.rs's delegate description
correctly untouched (its 40/35 is delegate's own default, census-listed).
Exactly one pin fixed the old numbers (tests/loop_spec_recovery.rs) —
updated and RED-proven against pre-edit text. kimi REQUIRED (routing
d1790687293-8, doctrine row, ran ALONE), launched under the row's own new
60/40 budgets, verdict PASS (d1790688874-9): 3 parallel mutants —
echo-revert sent the pin RED (RED-proof independently reproduced),
weakened-needle still RED (pin non-vacuous); the step-4-sentence and
META-SPEC text-revert mutants went full-suite GREEN — pre-existing
unpinned-text gaps, non-blocking observations (candidate pin-breadth rows
for a future eval). Gates 1129/1129 nextest --release worktree + main.

**T148 — chug_launch wire e2e (8cd59ac, merge of loop-t148: 14d1bcd).**
T129's descoped follow-up closed the M7+M8 survivor gap with a real-wire
e2e: the REAL `chug mcp-serve` binary over REAL stdio (T124 harness,
+425 tests-only + one README line), `CHUG_DELEGATE_BIN` stub child
recording argv+env — real server, real spawn, no model endpoint. Three
legs: happy path (advertised ⇔ callable, pid/log/events payload, stub
record byte-equals the wire's spec/goal/model + budgets 7/9 — distinct
from the 40/35 delegate defaults), above-ceiling refusal (isError:true
naming "got 201" + the 200 ceiling, 1s bounded absence probe, loop alive
after refusal), default-deny (unadvertised, unknown-tool -32602). Carried
nit honored — zero sub-ms ordering assertions. kimi SKIPPED (routing
d1790684833-5, tests-only precedent); the orchestrator independently
re-ran the 3 canonical mutants — ALL RED (verdict d1790684835-6):
flag-drop → happy-path FAIL, isError:false → ceiling-leg FAIL,
advertise-unconditional → default-deny FAIL. Gates 1129/1129 nextest
--release worktree + main. Child's gate note: default-parallelism
`cargo test` bin-test flakes reproduce on the CLEAN base (dc6c11d,
tree stashed) under current machine load — pre-existing, nextest-based
gates unaffected; watch item for the next eval (threading or
isolation candidate).

**T147 — 4 carried survivor pins, tests-only (6b217c3, merge of loop-t147:
b08e909).** The cycle-70 80/80 budget death recovered by ONE T63 resume in
the standing worktree: child finished at 26/80 with the goal accepted,
gates green (build, clippy -D warnings, 1126/1126). All four carried
survivors now RED-proven pinned: T128 M4 (both collect-liveness arms + None
leg), T135 M2 (release must spend its 50×10ms budget on a held .chug flock)
+ F2 (lock absence + observe-seam first-attempt acquire), T138 plan-guard
(pending flag-writing mcp.json registry through the REAL plan loop — no
flag, no server; the prod empty-registry construction was the mask), T141
M1 (message_stop-masked open-block rejector — the mutant synthesized an
executable goal_complete). kimi SKIPPED per routing d1790681505-2
(tests-only, T130 precedent); the orchestrator independently re-ran all 5
mutants at review (T79 throwaway worktrees + role-keyed target dirs) — ALL
5 RED, 0 survivors (verdict d1790681871-3); post-merge nextest 1126/1126 in
target-shared-main. One launch slip caught by read-back: the first mutant
legs ran with cwd=main (no per-leg cd) — killed, relaunched with subshell
cds, main tree untouched. Carried weak-pin debt (cycles 65-70) CLOSED.

**Cycle-level notes.** Four for four, zero goal-rejections, one budget
death per arc (T147 resumed via T63; T150 was the committed-work variant
— orchestrator-finish, no resume burned). kimi ran ONE round (T149
REQUIRED, doctrine) and its PASS carried two non-blocking observations:
the step-4-sentence and META-SPEC text-revert mutants stay full-suite
GREEN — pre-existing unpinned-text gaps, candidate pin-breadth rows for
the next eval. The orchestrator independently re-ran mutants on every
tests-only row (T147: 5/5 RED; T148: 3/3 RED; T150: vacuous-guard RED) —
the no-kimi rounds lost no teeth. Recurring machine-load watch: TWO
children (T148 impl, T150 impl) reproduced default-parallelism
`cargo test` bin-test flakes on the CLEAN base (spawn-timing tests,
T144/T148 class) and fell back to `--test-threads=4` per the doctrine's
gate posture; nextest-based gates were unaffected both times — next eval
should consider an isolation/threading row. Iteration census: impl
children used 26 (T147-resume), 52 (T148), 43 (T149), 48-of-80 (T150,
minutes-bound) of 80 — no 80/80 deaths; the T110 estimate ceiling held
(T150's guard now enforces it at filing time). Release v0.5.1 cut at wrap
(4 items since v0.5.0, tests/doctrine/guards only → patch). Next cycle:
queue EMPTY → fresh eval routes kimi (loopd).

### Cycle 70 (2026-09-29) — kimi fresh-eval cycle (queue was empty) — 7 rows filed, T144 landed

Eval commit c802934: T144-T150 filed (bug T144 pri 1; T145 crash-safety;
T146 F2-2a roadmap pull; T147 survivor pins; T148 wire e2e; T149
validator budgets doctrine; T150 estimate pin), 13 rejected candidates,
20 eval-triage records, README audit third consecutive clean.

**T144 — goal-gate check + driver-spawned shells must not inherit
CARGO_TARGET_DIR (pri 1 bug, landed 8cabc79 ff-merge).** The cycle-66/67
infra finding promoted to a row: `goal_complete` checks executed a
FOREIGN worktree's test binary twice (d1790632587-1, d1790640546-3) —
and the fix's own validator reproduced it a THIRD time when its first
gate attempt (running the pre-T144 main binary, which still inherited
`target-shared`) hit the 600s check timeout under its own mutation-build
load with zero test failures. Fix: `tools::scrub_target_dir_vars`
removes both spellings at every driver-side spawn passing the inherited
env — `run_shell` (bash tool + goal-gate check), `hooks::run_hook`,
`delegate_launch`; explicit in-command prefixes keep the warm role-keyed
path (pinned green). Spawn-site sweep in the commit message (mcp N/A via
T138 env_clear; git/ps/rg fixed-argv). glm impl 73/80 one segment. kimi
REQUIRED PASS (d1790668684-21/d1790670698-22): gates 1110/1110
independent; mutants M1 (run_shell-scrub revert), M2 (alias dropped),
M3 (delegate-scrub revert), M6b (over-scrub breaking in-command prefix)
ALL DIED on named legs; non-blocking survivors M4 (general env
passthrough unpinned — broader isolation class, out of scope) + M5
(the beyond-spec hooks.rs scrub has no test of its own) carried to the
next eval. The `delegate_launch_stub_then_status_reports_summary_and_
liveness` full-suite load flake (one sighting) assessed by the
validator: 2 O(1) env_remove calls pre-spawn, timing window not widened.
Post-merge nextest 1110/1110. Watch for next cycle: children now run
the scrubbed binary — first goal gates build worktree-local
(impl-measured 85s vs the 600s cap).

**T145 — update_ledger writes through fsatomic::write_atomic (pri 2
robustness, landed 52a0abf ff-merge).** T136's crash-safety sweep had
left the ledger — the file every child reads first — on a truncating
`fs::write` (validator carry d1790643172-7). Now routed through the
shared same-dir-temp+fsync+rename primitive; happy path byte-identical,
a failed write errors the tool call with the previous ledger intact.
RED leg obstructs the pid-suffixed temp path (chosen over a second
RLIMIT_FSIZE leg to dodge process-wide rlimit flake against
transcript.rs's T136 test in the same binary — the validator assessed
the choice SOUND). Sweep verdicts in the commit message: update_ledger
CONVERTED; ensure_seeded / fork save+restore / archive::rotate /
generic write tools EXCLUDED with reasons. glm impl 62/80; its two
goal-gate rejections were the pre-existing load-sensitive mcp_http
dead-port race (T31/T59/T66 class, both legs 8/8 isolated, different
test each run) — aggravated by a 9.8-HOUR orphaned spinning test binary
from the t134 validation era (99% CPU, 590 min burned), which the
orchestrator found and killed mid-arc; see the cycle notes for the
orphaned-process finding. kimi REQUIRED PASS (d1790673125-26 /
d1790673939-27): independent gates 1113/1113; mutants M1
(revert-to-direct-write) DIED on the RED leg, M3 (wrong-dir temp) no
silent pass, M4 (error-swallow) DIED on both failure legs; fsync-drop
indistinguishable, skipped per spec; two non-blocking nits (124 vs ~60
estimate — informational; rename-replaces-inode symlink nit). Post-merge
nextest 1113/1113.

**T146 — F2 phase 2a: `chug run --approve plan.md` + web_fetch in plan
mode (pri 2 FEATURE, the mandatory roadmap pull; landed 55d59c3 merge of
8ea88e8 + d7d7aa0).** Retires the oldest Tier-1 deferral (34 cycles —
cycle-36's 50-iteration budget reason obsolete). Surface 1: the clap
flag exists on `run` only (plan/chat are clap errors);
`driver::load_approved_plan` refuses missing/unreadable/empty/escaping
legs with named-leg messages BEFORE any `.chug/` write (the T117
ordering); on success the approval sentence + plan text prepend the
first message with the goal undisplaced; `run_start` gains
always-present null-able `approve` + `plan_sha256` (new
`eventlog::sha256_hex`, the T117 honesty shape). Surface 2: web_fetch
joins plan mode's read-only set across all five enumeration sites
(six-tool contract, cardinality pinned). glm impl died 80/80 at a NEW
pace class (~5s/iteration, 6m41s for the whole budget — flash models
make iteration counts, not minutes, the binding constraint) and finished
on the T63 resume (impl recovery d1790674063-31; validator recovery
d1790677504-33). kimi REQUIRED PASS across two segments (validator died
50/50 mid mutation wave 2 — the 5th validator budget death in 5 cycles,
more T149 census — resumed and goal-accepted at iteration 4; verdict
d1790677901-34): independent gates 1118/1118; all 4 spec-named mutants
(before-.chug ordering, null-fields shape, approve-on-plan clap,
web_fetch enumeration) killed + 2 extras (injection-order, empty-check);
2 stale five-tool comments fixed on-branch pre-merge (d7d7aa0), 4
cosmetic nits carried. Post-merge nextest 1121/1121. FEATURES.md F2
annotated phase-2a landed, 2b deferred with refreshed reason.

**Cycle-70 wrap notes (budget-low at 240-min orchestrator ceiling).**
Landed 3/7 rows: T144 (bug), T145 (robustness), T146 (feature). DEFERRED
to next cycle: T147 (impl died 80/80 at the glm-flash ~5s/iter pace
class — 613k input tokens in ~10 min; worktree /tmp/chug-loop-t147 left
STANDING with uncommitted work + a recovery pointer on the row),
T148/T150 (not dispatched), T149 (doctrine — never dispatched; its
census GREW this cycle: t146-validate was the 5th validator budget death
in 5 cycles). Cycle-level findings: (1) a 9.8-hour orphaned spinning
t134-era test binary (99% CPU, 590 min burned) was found and killed
mid-T145 — a mutation-leg test process outlived its validator by ~11h;
the port-test flakes that cost T145 two gate rejections trace to its
load; watch for recurrence, candidate row next eval if the T6 bounded
harness needs a reaper. (2) The mcp_http dead-port race (T31/T59/T66
class) fired twice under load this cycle — third era sighting; next
eval should weigh a deflake row. (3) glm-flash pace class: iteration
budgets, not minutes, bind (T146 impl 80 iters in 6m41s; T147 impl same
death) — next eval weighs whether glm children need higher iteration
budgets or tighter specs. (4) Tool-result RENDER garbles (7 sightings)
proved to be artifacts on every disk read-back — verify-then-act held
every time; no action needed beyond the standing doctrine. Validators
this cycle: 3 REQUIRED PASSes (T144/T145/T146), 4+6+3 mutants killed,
survivors carried (M4 env-passthrough, M5 hooks-scrub pin, T146
cosmetics). Release: v0.5.0 (minor — T146 feature) cut at this wrap per
the T100 trigger (3 items + FEATURES phase annotation since v0.4.1).

### Cycle 69 (2026-09-29) — routine glm freshness-skip (queue non-empty: T129/T131 deferred from cycle 68, eval fresh) — the deferrals worked first

**T129 — F10 phase 2b: chug_launch MCP write leg (pri 2 feature, landed
2b4490b ff-merge).** The fleet primitive's actual verb, held one cycle by
the cycle-68 wall-clock defer, landed first-try: `chug mcp-serve
--allow-launch` (default OFF — the flag is the policy boundary; the
flagless server is byte-identical read-only and a `chug_launch` call gets
the unknown-tool `-32602`, so a read-only deployment cannot probe the
flag into revealing the tool exists). One boolean feeds both `tools/list`
and `tools/call` (advertised ⇔ callable by construction); the full
validation chain (shared `validate_chug_cwd` with error-text parity,
absolute readable-file spec, non-empty-trim goal, pass-through model,
budgets 1..=200/240 reject-above naming the received value + ceiling);
spawn hands off to the ONE `delegate_launch` path (`pub(crate)`
visibility-only — no second spawner, `CHUG_DELEGATE_BIN` seam reused,
`DELEGATE_ENV_LOCK` shared for the stub legs); launch failures are
`isError` results and no error kills the server loop; 13 bin-internal
legs; README params + safety paragraph + phase 2 CLOSED. The estimate
line's descope clause FIRED (all-in diff 814 > ~500): the optional
flag-ON wire e2e dropped to a follow-up, zero validation legs trimmed
(child descope record d1790662565-1). glm impl 77/80 first-try
(budget-low@8 fired, accepted before the ceiling — T21 headroom held).
kimi REQUIRED (routing d1790663037-1) VERDICT PASS (verdict
d1790664201-2): 8 mutants, M1–M6 killed (flag guard, advertise gate,
ceiling boundary, goal trim, spec-absolute, budget argv pass-through);
M7 (`--allow-launch` CLI plumbing) + M8 (launch-failure isError arm)
SURVIVED as non-blocking "correct code, test gap" findings — M7 is
exactly the descoped e2e's pin, M8 wants a spawn-failure leg; carried to
the next eval. Gates 1105/1105 + clippy clean independently; post-merge
nextest 1105/1105 (target-shared-main). Pipeline overlap ran once clean
(T129-validator ‖ T131-impl — disjoint file sets, strictly serial
merges). FEATURES.md F10 updated at the row flip (phase 2b LANDED, phase
2 CLOSED).

**T131 — eval-digest reader staleness check excludes the reader's own
live stream (pri 4, landed b52e62c rebased ff from 26b3ec6).** The
cycle-65 eval's live-observed annoyance (digest 17:47:52Z vs own-stream
rotation 17:48:12Z → STALE within the first minute, "regenerate if
stale" unsatisfiable mid-cycle) fixed at the render: the newest events
file (`ls -t | head -1`) is dropped from the `-newer` candidate set via
`grep -vx`, both verdicts render (`&& echo STALE || echo FRESH`), and
the rendered rationale sentence says why the newest file is excluded and
what STALE now means (foreign corpus). Pre/post-scan machinery,
corpus-age field, and the four pinned staleness labels byte-identical;
one pin leg RED-proven on the parent tree (died at the check-line pin,
old render lacked exclusion + FRESH) with a live 3-leg tempdir demo of
the extracted check (digest-newest→FRESH, own-stream→FRESH,
foreign-second→STALE) recorded in the commit message. glm impl 27/80
first-try; kimi SKIPPED per routing d1790664201-3 (T16/T116
tests+tooling precedent — the child's RED proof + orchestrator gates
carried it; a kimi round on a ~30-line script render is the optional
tier the routing call declined). Review + post-merge nextest 1106/1106
(target-shared-main). Est ~30 → actual +73/−4 (test density again beats
the line estimate; within the no-action band).

**CYCLE-69 CYCLE-LEVEL NOTES (wrap).** BOTH deferred rows landed — the
cycle-68 deferrals were honored in queue order (feature T129 first, then
small T131) — and the queue is now EMPTY. Zero budget deaths, zero T63
resumes (77/27 and 41/50 of their ceilings — the healthiest census in
the post-T110 era); minutes never binding. Pipeline overlap ran once
clean (T129-validator ‖ T131-impl, disjoint file sets, strictly serial
merges, clean rebase-ff for T131). Two decision-log hygiene notes for
the record: (a) one outcome record (d1790664534-4) was appended with
T131's inputs under T129's routing subject — corrected by the two
following records (d1790664540-5/-6); the corpus is append-only so the
stray stays readable; (b) child decision records (T129's descope call,
T131's RED-first verdict) were merged into the main decisions.jsonl at
harvest. Harvest: 3 impl/validate streams + 1 verdict LEDGER into
`.chug/` (untracked by design — .gitignore carries .chug/; commit
messages name the harvest). NO release tag this wrap: 2 items since
v0.4.1 (< 3) and no FEATURES.md check-off (F10 phase 2 CLOSED is a
progress annotation on a still-open row — phase 3 deferred — matching
the T128 precedent). Carried to the next eval (queue empty → fresh
eval routes kimi): T129's M7 (--allow-launch CLI plumbing pin = the
descoped wire e2e) + M8 (launch-failure isError arm pin) survivors, the
stub-spawn events-ordering nit, and the optional follow-up row for the
flag-ON wire e2e. Final gates at HEAD: build + clippy clean, nextest
1106/1106 (target-shared-main).

### Cycle 66 (2026-09-28) — routine (reconciled cycle-65 divergence first) — codex-intake queue T134–T142 — T140 9b36a2e (goal_complete denial bypass), T141 70ec4a7 (SSE truncation acceptance, rebased), T142 3bc3169 + fix-up dd184d4 (loopd grep spoofing), T134 d5d9c28 + fix-up bfd316a (symlink sandbox escape + doctrine drift) — compacted at the cycle-73 wrap (last-6 rule); full narrative in git.

### Cycle 65 (2026-09-28) — fresh eval (kimi, loopd-routed: queue empty) — T128–T131 filed (F10 phase 2 PULLED + SPLIT); T130 + T128 LANDED (976e4ae, 85ca4c1); T129/T131 DEFERRED by mid-cycle operator intake (codex adversarial review, 9 HIGH rows T134–T142); cycle ended on a push divergence (doctrine: no mid-cycle reconcile). Cycle notes below the per-item entries.
### Cycle 64 (2026-09-28) — fresh eval (kimi, loopd-routed: queue empty) — T124–T127 filed; ALL FOUR landed clean (T127 5dbab0d README install truth, T125 532c403 estimate calibration, T124 d6264be F10-p1 mcp-serve+chug_status, T126 eef7a29 validator /tmp-heredoc doctrine) — the era's FOURTH all-PASS cycle after 58/61/62. Cycle notes below the per-item entries.
### Cycle 63 (2026-09-28) — routine glm freshness-skip — T122 LANDED (93dae89 fast-forward)
### Cycle 62 (2026-09-28) — ALL FIVE landed clean (T121 cbc21e0 loopd 160→200, T117 3579d9d F9-p2a run-side expansion, T119 43f427d weak-test pins, T120 c173c2f kill/sed doctrine, T118 4c96414 F9-p2b frontmatter+completion — **F9 phase 2 CLOSED**) — the era's THIRD all-PASS cycle after 58/61. Cycle notes below.
### Cycle 61 (2026-09-28) — ALL FIVE landed clean (T112 0325a36 streaming-usage fix live-verified, T113 55fe207 F9-p1 packs, T114 63119e6 check:-breadth doctrine, T115 ed6c96f goal-integrity surface, T116 eb55003 todo symmetry) — the era\'s second all-PASS cycle after 58. Cycle notes: kimi ran 4 rounds (3 REQUIRED + 1 optional-exercised T113, T116 skipped per precedent), 31 mutants total, zero blocking findings era-wide second cycle running; T44 overlap ran twice clean (T112-val||T113-impl, T115-val||T116-impl — disjoint file sets, strictly serial merges, clean rebase-ffs). ZERO impl-child budget deaths and zero T63 resumes (26/62/44/63/26 of 80 — the T102+T110 era\'s healthiest census). Display-artifact watch: ~11 render-only sightings in the ORCHESTRATOR\'s own stream (incl. one big duplicated-block render), every payload verified intact by transcript/disk read-back — plus ONE real self-inflicted incident: SIGKILLed a healthy T112 validator at 35s misreading a render garble (verify-then-kill must be SEQUENTIAL — read first, kill after; the kill was issued in the same breath as the check) and one anchor-typo silent-replace no-op caught by read-back (assert replacements). Validator zone watch 21/26/19/44 of 50 — inside T18 margin. Estimate calibration: T115 ~130→+398 (3.0x), T113 ~455→+583, T116 ~30→+116 — test+doc density beats estimates ~2-3x, ceiling\'s failure measure still unmet. T115\'s integrity surface activates NEXT cycle (loopd rebuilds before launch). Carried to next eval: T113\'s 7 + T114\'s 1 + T115\'s 2 non-blocking (multibyte-goal leg, run_start call-site wiring pin, append-leg newline-collapse deviation). Tag bootstrap holds (no v* — operator\'s first, now triply feature-worthy F7+F8+F9-p1). Untracked com.tampajohn.chug-loopd.plist appeared mid-cycle — operator\'s launchd plist, left untracked. Queue EMPTY -> next cycle eval-routes kimi. Final gates 924/924 + clippy + build at HEAD under target-shared-main.
### Cycle 60 (2026-09-28) — fresh eval (kimi, loopd-routed: queue empty) — T110 + T111 filed (F8 SPLIT phase 1 → T111); BOTH landed (T110 5a16ce5, T111 d8fdea0+fc1d691)
### Cycle 59 (2026-09-28) — fresh eval (kimi, loopd-routed: queue empty) — T107–T109 filed (F7 SPLIT phase 1 → T108); ALL THREE landed (T107 793a0fc, T108 53e4aed+2a51cc5, T109 75025c9)
### Cycle 58 (2026-09-28) — fresh eval (kimi, loopd-routed: queue empty) — T102–T106 filed (F6 SPLIT phase 1 → T105); ALL FIVE landed clean (T102 ffebdaf, T103 80d4a14, T105 dc29137, T104 ee3943e, T106 e5cdebd)
### Cycle 57 (2026-09-28) — eval-routed kimi turned routine after reconciling cycle-56's origin divergence (T101 row appeared post-rebase, predicate held — cycle-56 precedent repeated); T101 LANDED (bcd0b66, fast-forward)
### Cycle 56 (2026-09-28) — T100 LANDED (9d1182a fast-forward: GitHub-releases surface — tag-triggered release.yml + install.sh + notes generator, +2008/-12 across 12 files) via FAIL→fix-up→PASS arc (kimi r1 caught the aarch64→linux-arm64 collapse); cycle-55 divergence reconciled; live chug-site edit 8eee10b; wrap 419afbc
### Cycle 55 (2026-09-28) — eval-routed kimi cycle turned routine (operator's T99 directive filled the empty queue at launch); T99 LANDED (d13a253, fast-forward)
### Cycle 54 (2026-09-28) — routine glm freshness-skip: T96 META-META-SPEC check-filter-breadth (merge 457720d, mid-arc recovery per row recipe, kimi REQUIRED PASS zero findings) + T97 README delegate sub-bullets (b0c6041 fast-forward, flip eca240a, kimi SKIPPED per T16/T31) landed; T97 push-divergence incident (operator moved origin mid-cycle, reconciled next cycle per no-force-push rule) — full narrative in git: `git log --grep "T9[67]"`.
### Cycle 53 (2026-09-27) — six landed: T92 9db86bc (impl 50→65), T91 ae7ff5f (F5-p1 images), T93 aeb12ea (mcp__ canary), T94 06f3b9e (get_str diag), T95 e80b3c5 (readme_layout), T98 8721c83 (site-sync); T96 mid-arc→c54, T97→c54 — one-line compaction; full narrative in git (row-flip commits + TODO done rows)
### Cycle 52 (2026-09-27) — routine glm freshness-skip; T90 LANDED (e9afed9, fast-forward merge) — F4 permissions phase 1: the deny-only fail-closed policy layer
### Cycle 51 (2026-09-27) — fresh eval (kimi: queue empty) filed T88+T89+T90; T89 delegate terminal-wait + LOOP-SPEC adoption LANDED (merge d2b402a, impl 6e95df9 via T63 resume #18; kimi PASS) + T88 decision_log corrective validation errors LANDED (c5a4f9e). Verdict: 2/2 landed
### Cycle 50 (2026-09-27) — T85+T86+T87 docs bundle LANDED (merge 2440520: cross-tree bash escape-hatch doctrine on both review surfaces, README layout tgrep/plan/hooks, docs-only floor honest risk model + pin leg); cycle-49 mid-arc recovery executed via T63 resume #17; kimi PASS 0 blocking 18/50, 2/2 mutants RED. Verdict: 3/3 landed
### Cycle 49 (2026-09-27) — T83 hooks phase 1 LANDED (merge ccb828a, F3: .chug/hooks.json PreToolUse veto + PostToolUse advisory; 2 budget-aborts -> resumes #15/#16; kimi R1 FAIL PostToolUse-on-vetoed-calls -> class sweep incl. the risk-gate sibling leg -> R2 PASS 0 blocking); T85-87 bundle MID-ARC (landed cycle 50). Verdict: 1/1 landed after 2 resumes
### Cycle 48 (2026-09-27) — cycle-47 dead eval recovered (artifacts landed verbatim 89d1b5a + 11 triage records reconstructed); T84 trim.rs extraction LANDED (745f8ff, 981 moved lines byte-identical, kimi PASS 4/4 mutants); T83 MID-ARC (landed cycle 49). Verdict: recovery + 1/1 landed
### Cycle 46 (2026-09-27) — FIRST loopd-routed cycle (routine glm, predicate honored end-to-end); T82 nextest gates LANDED (6edc5ee — 2.5x faster measured post-merge) + T75 decision-records wrap-checklist LANDED (b4c159f), both first-round kimi PASS, 12/12 mutants RED; queue EMPTY. Verdict: 2/2 landed
### Cycle 45 (2026-09-27) — freshness-skip; T81 landed (merge c1daaca)
### Cycle 44 (2026-09-27) — T80 docs-only gate slimming (9c4221d) + T79 parallel mutants (660f652) landed; T81 MID-ARC (impl in flight, recovered cycle 45); morning tools-proxy outage halted cycles 42/43 fast (halt-is-the-pager shape); full narrative: git log 9c4221d..660f652 + TODO done rows
### Cycle 40 (2026-09-26) — T78 release builds landed (merge 23fd276)
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

### Cycle 72 (2026-09-29) — kimi fresh-eval (queue was EMPTY) — 6 rows filed + T158 mid-cycle (filed as T157; renumbered at the cycle-73 origin reconcile — the operator's MCP-write-verbs filing took the T157 id)

**T151 done c60ed3a (e9d6d72 + 5a58e4f).** One shared test-timing serialization domain (src/testsupport.rs, ONE domain by construction via fixed export symbol) + full serialization sweep of the six sighted families + dead_port ROOT fix (drain_pending_accepts — the test's own probe left a nonempty accept backlog; macOS completes connects to just-closed listeners). Recovery: impl 80/80 uncommitted → T63 resume → minutes-bound COMMITTED → orchestrator-finish; review caught the theft_or_regression single-shot misclassification (2/7 reds) → fix-up child (minutes-bound, committed) — tri-state confirm_connect + Invalidation{Theft,TeardownArtifact} + scripted-seam pins. Gates: nextest release 1136/1136; vacuousness mutant (fresh mutex per call) RED on the lock-domain pin. The acceptance matrix (8 runs) exposed the residual third family — spawn-failure-under-resource-pressure (26 red legs: loopd fixtures ×16, driver scripts ×3, mcp_serve stub ×3; ZERO in T151's closed classes) — filed mid-cycle as T158 with reproduction. Minutes-death census this era: 4 (t150, t151-resume, t151-fixup, t152-impl) — impl-minutes raise measure DECISIVELY tripped for next eval. Live catch: the t151 fix-up's bg hammer loop survived its death 1h49m (reaped manually) → T152 spec gained the cwd needle leg.
**T152 in flight:** base 81f43be (+1163: orphan-reaper.sh 248 + tests 891) committed; goal-gate red at its minutes-death (T158 flakes under double load, orchestrator-finish); spec amended post-hoc with the cwd leg (evidence above); fix-up child (glm) in flight at wrap — next cycle: check fix-up → gates → kimi REQUIRED → merge.
**T153/T154/T155/T156/T158 unworked** — specs ready; queue order per the cycle-72 handoff (T154 before T153).
**Wrap-truncated:** budget-low at 54 iters/4 min — post-merge target-shared-main gates + T151 ledger-harvest deferred to cycle 73 (T151 events already harvested into .chug/; worktrees /tmp/chug-loop-t151 + -t152 NOT removed).

### Cycle 73 (2026-09-29) — resumed mid-arc (prior segment truncated at wrap); origin reconcile + T152 landed

**Origin reconcile (b4e1d1a, pushed).** The cycle-72 wrap's rejected push resolved: origin had gained the operator's T157 filing (MCP write verbs — chug_abort + chug_steer, F10 phase 3) while the loop had filed its own T157 mid-cycle (spawn-pressure test family). Operator's filing keeps the id (shared remote, authoritative); the loop's row renumbered **T158** (row + spec mv + EVALUATION refs + T151 notes ref), todo_consistency 11/11.

**T152 done a59cf97 (81f43be + 39c8a26).** loopd pre-cycle orphan-process reaper — identity-based needle, fail-closed, SIGTERM-only, sweep at the one safe instant (post-single-driver, pre-build), LOOP_REAPER=0 opt-out. The fix-up child landed the spec-amended **cwd third leg** (`/tmp/chug-loop-t*`//tmp/chug-mut*` cwd via a PATH-independent probe, /private/tmp forms included, fail-closed on unresolvable cwd) driven by the cycle-72 live evidence (the T151 fix-up's hammer loop survived its child's death by 1h49m identified only by cwd). **kimi validator: PASS** — gates build+clippy+bash -n clean, worktree nextest 1152/1152, live verification on the real process table, **6/6 collected mutants killed** (lsof-fail-posture, non-abs-cwd, argv-needle, opt-out, SIGKILL, own-group-gate); 3 extra legs (m7-wiring, m8-commgate, m9-direxists) launched but **uncollected at a 60/60 budget death** — verdict survived in the child's ledger (T137 class: written-but-unannounced); m7-wiring killed by orchestrator read-back (loopd.sh:246 invokes the reaper behind the best-effort guard), m8/m9 noted as residual watch. **Post-merge gate finding:** `loopd_orphan_reaper` red ONLY under nextest parallelism — the three through-loopd tests (real loopd spawn → real sweep over the real process table) bust their 30s wait deadline at 17-way load; solo 3.8s green; T82 fallback serial run green **1155/1155, 0 failed** — family named in the landing commit. Load-flake filed as **T159** (bring the reaper family into T151's shared timing-lock domain). Harvests: events-t152-{impl-fixup,validate}-*.jsonl + LEDGER-t152-{children,validate}-*.md into .chug/.

**T154 done efa287a (400642f).** The mcp_serve.rs split — the era's fastest grower settled BEFORE T153 lands in it (the spec's dependency ordering). 1,168-line `#[cfg(test)] mod tests` body + 49 fns → `src/mcp_serve/tests.rs`; production keeps the T104/T109 shape; novel lines named in the commit (6-line provenance tail + the forced `include_str!("../mcp_serve.rs")` path fix — include paths are relative to the containing file). **Validation routed to the ORCHESTRATOR** (kimi skipped: mechanical move outside the REQUIRED file list; the spec's own acceptance names validator-or-orchestrator): worktree nextest **1138/1138**; **list-identity diff EMPTY** (1138 pre = 1138 post, identical sha256); **move-mutant RED** (dropping the `mod tests;` declaration → exactly the 49 moved tests vanish, `mcp_serve::` count 0). The impl child died **minutes-bound 48/80 AFTER committing with a clean tree** — the T156 committed variant (5th minutes-death this era; the raise measure is already tripped for the next eval). Post-merge serial fallback gates **1155/1155** (the reaper family's documented runner, per the T152 finding). Harvests: events-t154-impl-*.jsonl into .chug/.

**T153 MID-ARC at the earlier segment's wrap (driver wall) — since RESOLVED; the worktree resumed this same day and the row landed (see the T153 done 675076d entry below).** chug_cancel (the second flag-gated MCP write leg) is fully implemented and COMMITTED on `loop-t153` in `/tmp/chug-loop-t153` (3ae17a1, +1197/−23: schema + advertised⇔callable under --allow-launch, fail-closed ownership re-derivation per call, TERM-group → 5s grace → SIGKILL, 12 unit tests + 3 wire legs, README/FEATURES/main.rs) plus ae0a684 (the spec's `check:` line now embeds the T47 shared cache — the goal-gate check had run env-starved, cold-built into `./target`, and hit the 600s wrapper: an infrastructure kill, not a test failure). Child ledger gates: build + clippy green; per-binary green; two full `cargo test` runs green. Two load-flakes observed at the edges (both documented, neither a product red): the new `chug_cancel_happy_path_over_the_real_wire` dead-poll leg failed ONCE under nextest 17-way (passes solo 1.13s — T151 family) and the pre-existing `delegate_collect_mid_run_reports_running` 5s wall assert (passes solo 0.93s). Known risk carried in the child ledger: the full suite is ~800–1000s warm vs the 600s check wall — remedy candidate for the next eval. Impl arc burned TWO budget deaths (80/80 + the resumed leg at 35:00 mid-check); T63's one-resume cap reached → orchestrator-finish attempted, superseded by the wall. Harvests: events-t153-impl-resume-*.jsonl + transcript-t153-impl-*.jsonl in .chug/.

**T153 done 675076d (3ae17a1 + ae0a684 + 6e92f84, cycle 73 resumed).** chug_cancel — the second flag-gated MCP write leg (F10 phase 3a): advertised ⇔ callable under `--allow-launch`, stateless fail-closed ownership re-derivation (alive → pgid==pid → ps argv names `chug run`), TERM to the whole process group → 5s grace → one SIGKILL escalation, payload `{pid, signaled, waited_ms}`; 12 unit legs + 3 wire legs on the T148 harness. **The resume arc's forensics found a REAL product bug, not load:** the cancel dead-poll failure reproduced SOLO 12/30 and the instrumented tick caught `kill(-pgid,0)` answering **-1/EPERM** over a group whose sole member was the server's own unreaped zombie child (macOS killpg-on-zombie; per-pid `kill(zombie,0)` returns 0 — the group and pid probes disagree) — `group_gone`'s `rc != 0` read EPERM as "group empty" and returned `signaled: term` with the leader an unreaped zombie. Fix-up 6e92f84: `group_gone_rc` seam reads ONLY ESRCH as empty (rc==0 and every other rc/errno mean not-gone; the next tick's waitpid reaps the zombie and the probe turns ESRCH — deterministic convergence), predicate-table + real-zombie + convergence RED proofs, wire happy path 15× solo green (was 12/30), and a kill(2) class sweep in the commit message (`process_alive` adjudicated already-compliant EPERM→alive, pinned empirically via `kill(1,0)`=EPERM on live root launchd; driver_lock/loopd.sh/reaper/delivery-rc sites justified). **kimi REQUIRED PASS** (routing d1790721266-2, verdict d1790722238-3): clean gates nextest 1170/1170 run independently in the role-keyed validate cache, 7/7 spec requirements verified, T79 parallel mutation round 9 mutants — 7 RED (incl. M4 re-proving the EPERM quirk live), **2 weak-test survivors carried as T160** (M5: needle table lacks a `--special` near-miss row; M6: the escalation fixture leg asserts the payload but never proves the group emptied — FixtureGuard cleanup masks a no-SIGKILL mutant), 2 observations (needle-without-`chug`-token is by design and documented). The validator's goal-gate rejected on the spec check line vs the 600s wrapper wall — the row's documented KNOWN RISK (warm full debug suite ~800–1000s); verdict accepted per the T137 written-before-rejection class, and the check-wall remedy stays a next-eval candidate. Worktree + post-merge gates 1170/1170 (target-shared-main). Harvests: events-t153-fixup/validate + LEDGER-t153-fixup/validate into .chug/.

**T155 done 00d4f7e (6780191 rebased, cycle 73).** The two T149 kimi non-blocking unpinned-text gaps closed: `tests/loop_spec_recovery.rs` +248 — (p) step-4's budget RATIONALE tokens (the 4-of-the-last-4 census clause, the >1-of-8 measure tripwire, the trim-mutation-leg-counts remedy) each pinned EXACTLY once inside step 4's T64 window, AFTER the 60/40 numbers pair; (q) META-SPEC §6's validator argv `--max-iters 60 --max-minutes 40` pinned as a contiguous exactly-once fragment. glm impl child died minutes-bound 49/80 AFTER committing (the T150 committed-variant class, 6th minutes-death this era) — orchestrator-finish per routing d1790723435-6: no resume, review + gates + merge. kimi SKIPPED per the tests-only T16/T31/T130/T131 precedent with the orchestrator RED-proving both text-revert mutants personally (census 4→9 → pin panicked at the count leg; argv 40→30 → pin panicked at the fragment leg; pristine restore 17/17). Worktree (rebased onto the T153 merge) + post-merge nextest 1172/1172.

**T156 done (merge of loop-t156, cycle 73).** The T63 doctrine now names the complete+committed budget-death variant — the routing discriminator sentence sits in LOOP-SPEC §2 step 2 right after the resume-first rule: work INCOMPLETE takes the ONE resume; work complete and committed with the goal unaccepted goes to orchestrator-finish directly (no resume burned; T150-impl named alongside T55); resume-exhausted goes to next-cycle with a recipe (T28). Pins (r)(s)(t) in tests/loop_spec_recovery.rs (+227): 9 routing tokens exactly-once inside step 2's window, byte-exact adjacency/ordering chain, standing-recipes byte-pin. The doctrine row validated its own thesis in practice: the impl child died minutes-bound 47/80 AFTER committing (4th consecutive committed-variant — T150, T154, T155, T156) and the orchestrator-finish routing applied cleanly. kimi REQUIRED PASS (routing d1790726950-9, verdict d1790729228-10): all 3 spec requirements verified, own gates 1175/1175, serial mutants budget-truncated with RED corroborated by pre-edit zero-occurrence analysis and the commit's documented M0–M4 matrix (restore sha independently verified) — plus the orchestrator's own sentence-revert RED proof (both routing pins died; pristine 20/20). Gates wt+main 1175/1175.

**Cycle-73 wrap notes (2026-09-29/30, resumed mid-arc → full wrap).** FIVE rows landed: T152 (a59cf97, prior segment), T154 (efa287a), T153 (675076d — the feature + the forensics-driven EPERM/zombie fix-up), T155 (00d4f7e), T156 (merge of loop-t156) — plus the b4e1d1a + df042ad origin reconciles (operator filings T157/T161 kept their ids). **Release: v0.6.0 tagged 622cc79** (5 items since v0.5.1 + the F10 phase-3a check-off; minor bump; notes generated; check-tag-version ok; commit+tag pushed together). Validators: kimi REQUIRED ×2 (T153 PASS 7/9 mutants, 2 weak-test survivors → T160 filed; T156 PASS, mutants budget-truncated + RED corroborated), kimi SKIPPED ×1 tests-only (T155, orchestrator RED-proofs both pins), orchestrator-verified ×2 (T152 prior segment, T154 list-identity). **Minutes-death census now 6-8 this era — ALL committed-variant** (T150, T151-resume+fixup, T152, T154, T155, T156 + T153's arc): the T63 three-way routing (T156, landed this cycle) named the variant and the orchestrator-finish path executed cleanly four times in one cycle. T63 resume used ONCE (T153's mid-arc). Queue for cycle 74: T157 (operator MCP write verbs — chug_abort + chug_steer, F10 phase 3), T158 (spawn-pressure family), T159 (reaper timing-lock), T160 (T153 weak pins), T161 (operator two-impl overlap lever) — all with ready specs. **Bookkeeping incident found + fixed at this wrap:** the cycle-72 eval commit (1206d93) dropped the Cycle 69/70/71 Outcomes sections that existed at the cycle-71 wrap (regenerated file, lost carry-over) — restored verbatim from 9c1d6bd:EVALUATION.md; 67/66 re-compacted to one-liners (outside the last-6 window); the eval-commit template must carry forward ALL existing Outcomes sections (next-eval candidate). Final gates at HEAD: build + clippy + nextest 1175/1175 (target-shared-main).

### Cycle 74 (2026-09-30) — freshness-skip routine cycle; T161 (the operator's parallelization lever) landed first

**T161 done 6d1ae1d (impl 7e7c2c9, spec check pre-scoped 5309d25).** Two-impl overlap — the T44 extension the operator asked for ("let's do 1", 2026-09-29): LOOP-SPEC §2 + Hard rules now read **at most 2 children, of which at most 1 validator, disjoint-gated**; the Pipeline overlap rule gains pattern (ii) — impl child N+1 MAY launch while impl child N is STILL FLYING iff the two specs' named target files are disjoint (both specs read, file lists compared); concurrent impls never share a build slot — the impl launched into an overlap takes the role-keyed slot `target-shared-impl-a` (or `-impl-b` when a flying impl holds a), goal-export swap + step-1 warm build to the SAME dir, solo impls keep `target-shared` (the T52 lesson one level down); a FAIL on N's validator pauses N+1's MERGE — never N+1's impl; not-disjoint → falls back to pattern (i) — T161 widens T44, never narrows, never forces overlap; doctrine items (incl. loopd.sh, added this round) NEVER overlap. The old "never 2 impls" clause is gone from all three carriers (§2 opening, Pipeline rule, Hard rules); the invariant text exists exactly once. 9 new pins in `tests/shared_target_dir.rs` (+382, incl. gitignore/README slot pins); README + .gitignore document the two slot caches. glm impl child goal-accepted 79/80 (right at the wire — the T92-class 80-cap watch continues; 1 of last 1, census stays open). **kimi REQUIRED PASS** (routing d1790732675-1): clean gates 1185/1185 nextest + clippy -D clean + spec check 32/32, **10/10 mutants killed each by exactly its targeted pin** (M1 invariant, M2 old-clause, M3 fallback, M4 FAIL-clause, M5 slot-collision, M6 gate-drop, M7 loopd.sh, M8 never-3+, M9 gitignore, M10 README), T79 parallel legs cap 3, worktree byte-clean. Post-merge nextest 1185/1185 (target-shared-main, 26s — the T159 reaper family green under nextest this run; its nextest-parallelism flake remains intermittent, row open). Harvests: events-t161-{impl,validate}-20260930-021348.jsonl + LEDGER-t161-validate-20260930-021348.md. **First live exercise of the landed rule is queued: T157 (mcp_serve.rs feature) + T158 (events.rs/hooks_policy.rs/mcp_serve tests) are disjoint-candidate for a 2-impl overlap this same cycle.**

**T157 done e0253e0 (impl e735f2b + fix-up 545be87; routing d1790736135-7, verdict d1790737941-9 FAIL → re-validation PASS).** F10 phase 3 complete: `chug_abort` (run-level cancel of a chug_launch'd child — same fail-closed ownership re-derivation + TERM→~5s grace→SIGKILL group discipline refactored byte-identically out of chug_cancel; terminal states aborted|already-done|not-found, verdict-first idempotent; the abort is recorded in the child's events.jsonl) and `chug_steer` (operator note into a running child via the driver's EXISTING [operator] mechanism over a new cross-process queue `.chug/steer.jsonl` — rename-away atomic drain at the iteration boundary, channel-first FIFO, ≤4000 chars reject-above; queued vs undeliverable). Both behind a new default-deny `--allow-control` flag (same advertised⇔callable policy family as --allow-launch, separate boundary; T90 permissions mcp__ rules apply). `chug_delegate` EVALUATED and DEFERRED with the written reason in the commit (spec req 4): steer-as-text + the existing delegate surface cover the consumer's needs today; revisit when a consumer outgrows it. +1814/−178 across 9 files; 15 new tests (13 mcp_serve unit + honest stdout-purity update, 7 driver steering fixtures, 2 wire e2e incl. the launch→steer→abort→re-abort arc over real stdio). **The kimi arc caught a REAL correctness bug class (validator routing d1790736135-7, verdict d1790737941-9 FAIL after one T63 resume — run1 died 60/60 with the verdict unwritten, the resumed run finished it): both control verbs derived terminal state from summarize_events().state() alone, whose "done" latches on goal_seen — which fires on ANY goal line INCLUDING outcome "rejected", after which the driver KEEPS RUNNING — so a LIVE goal-rejected child reported `already-done` with NOTHING signalled, and steering notes went unqueued; empirically demonstrated (d1-demo: goal-rejected fixture + live child).** Fix-up 545be87 (fresh child on loop-t157, committed-variant 80/80 → orchestrator-finish routing d1790738925-11): liveness-first legs — `reap_and_alive(pid)` (the T28 seam) consulted BEFORE any events-derived terminal state in every control-verb site (class-swept: abort, steer, chug_cancel reads no events, status/collect read-only), an events terminal state only shortens the path when the process is actually gone; the m6-orderflip weak order pin strengthened. **Re-validation PASS** (d1790739041-12): 1208/1208 nextest + clippy clean + spec check 76/76 on the clean tree; the three finding-mode mutants all KILLED (m1 events_terminal-sufficient = exact finding-1 mode, m2 steer done-leg-first = finding-2, m3 orderflip); regression legs green (graceful TERM+record, SIGKILL escalation, dead-pid not-found, non-group-leader refusal, already-done gone-child, steer dead/done undeliverable). Post-merge nextest 1208/1208 (target-shared-main). Budget-note: the impl arc burned run1 80/80 UNCOMMITTED → T63 resume (accepted 66 iters later); the fix-up died 80/80 COMMITTED → orchestrator-finish — both T63 discriminators exercised in one arc. Harvests: events-t157-{impl,validate,fixup,validate2}-20260930-041200.jsonl + LEDGER-t157-{validate,validate2,fixup}-20260930-041200.md.

**T159 done 5e26cc5 (impl 534dfd5; kimi skipped tests-only, routing d1790735929-6).** The three through-loopd reaper tests join T151's shared timing-lock domain — via `#[path = "../src/testsupport.rs"] mod testsupport;` (compiling THE T151 file into the integration binary: one domain by construction, same static + poison-tolerant timing_guard(); src/ untouched, +129/-0 tests-only). Zero timeout bumps; each test takes the guard as its FIRST acquisition, held spawn → assertion → cleanup. The spec's killing-check branch was attempted honestly and the stretch is NOT reproducible on this host (the tests are wait-dominated: unlocked trio 4.29s under a 16-worker hammer vs 9.24s solo-serial; full binary 17-way under 24-worker hammer, load 26.1 on 18 CPUs: 4.45s green) → the spec's second branch: a lock-scope membership pin (the include IS src/testsupport.rs, no second in-file lock, every real-loopd test takes the guard before the spawn — a future unguarded through-loopd test is RED by construction), RED-proven by the child (guard deleted → pin dies; guard moved after spawn → ordering leg dies) and independently by the orchestrator (guard-deletion mutant → pin FAILED, restore clean). glm child goal-accepted 51/80 (its half of the first live 2-impl overlap, see the T161 entry + the cycle notes). Worktree spec check 19/19; post-merge nextest 1210/1210 — the through-loopd trio at 3.6–8.1s under the untouched 30s caps. Harvests: events-t159-impl-20260930-023849.jsonl + LEDGER-t159-impl-20260930-023849.md.


**T158 done 0f8a4d9 (loop-t158: 82396ba seam+pins, ffbc613 dedent, 9fb8736 conversions+visibility+spoof_guard seam, 2af0fbe stale_binary finish; cycle-74 arc completed by the resumed segment 2026-09-30, recipe-per-row T28/T63 routing d1790746621-1).** The spawn-failure-under-resource-pressure family (cycle-72 matrix's residual red face) closed by MECHANISM per T31/T59/T66 doctrine, zero timeout bumps: `drive_attempt_with_spawn_retry` in the driver harness — 5 enumerated invalidation markers (`spawning sh -c `, hook twin, `timed out after `, `spawning mcp server `, ` failed to start: `), 3 bounded attempts on a fresh tempdir, marker-free reds resume BYTE-DISTINCT un-retried; named legs + class sweep converted (events, hooks_policy ×5, permissions ×3, preview ×2, mcp, plan, delegate twins); `wait_for_stub_dump`/argv/env polls now name the observed launch outcome (10s/5s fences stand); loopd fixture seam — spoof_guard ×7 (9fb8736) + stale_binary ×5 (2af0fbe, the run-4 red face) wrapped on the same "loopd never reached" marker, orphan_reaper descoped HONESTLY (protected by T151's timing-lock domain since T159, its own doctrine pins the deadline as a quiescence cap). Req-5 non-vacuousness: near-miss ×5 (flip/loosen a marker → pin dies), behavioral retry vs byte-distinct un-retried (attempt-count asserted = 1), exhaustion wording names class + count, static production-text ties. **Acceptance matrix (orchestrator, worktree 2af0fbe): 9 full-suite runs GREEN — 7 default-parallelism (run 6 under 8× yes-spinner load; run 6b under 8× yes + a concurrent COLD release build — cycle-72 run-4's exact double-load shape), 2 at --test-threads=4.** Zero invalidation retries fired: the family did not invalidate on this host state (18 logical cores; T151's lock + dead_port fix already removed the manufactured contention) — so the "retries observed" clause is satisfied by the req-5 behavioral pins, which exercise the real retry/exhaustion paths in every gate run; the seams stand as bounded insurance for the next genuinely hostile host. kimi SKIPPED (tests-only — src/*/tests + tests/loopd_*; no production file touched; T16/T31/T155 precedent; routing d1790746621-2) with the orchestrator reviewing the seam, markers, pins and running all gates: clippy -D clean, worktree nextest 1213/1213, post-merge main nextest 1213/1213 (target-shared-main). Harvests: events-t158-impl-20260930-013704.jsonl + LEDGER-t158-impl-20260930-013704.md (the prior segment's resume harvest events-t158-impl-resume-050139.jsonl already in .chug/).

**Cycle-74 wrap (2026-09-30, completed by the resumed segment).** FOUR rows landed: T161 (6d1ae1d, the operator's two-impl overlap lever), T157 (e0253e0, the F10 phase-3 feature — chug_abort + chug_steer behind --allow-control; the kimi arc caught a REAL goal-rejected-latching bug class and the fix-up closed it liveness-first), T159 (5e26cc5, the reaper family joins T151's lock domain), T158 (0f8a4d9, the spawn-pressure invalidation seams + a 9-run acceptance matrix — the cycle-72 residual red face closed by mechanism). T158's arc exercised the T28/T63 mid-arc recipe handoff end to end: budget wall → recipe on the row → resumed segment finished the partial half (stale_binary ×5), descoped orphan_reaper honestly, ran the matrix, merged. Validators: kimi REQUIRED ×2 (T161 PASS 10/10; T157 FAIL→fix-up→re-PASS), kimi SKIPPED ×2 tests-only (T159, T158 — orchestrator RED-proofs/gates personally). **Release: v0.7.0 tagged at this wrap** (4 items since v0.6.0 + the F10 phase-3 FEATURES check-off; minor bump for the T157 feature; check-tag-version ok; notes generated since v0.6.0; commit+tag pushed together). Final gates at HEAD: build + clippy -D + nextest release (target-shared-main). Queue for cycle 75: **T160** (the T153 validator's 2 weak-test survivors — M5 needle near-miss row, M6 escalation-fixture group-emptied proof) is the only open `todo` row, spec ready; the T158 matrix's zero-retry observation (family deflated on this host) is a standing datum, not a new row; the check-wall remedy (warm full debug suite ~800–1000s vs the 600s wrapper) remains the top next-eval candidate alongside the eval-template Outcomes carry-forward pin.

### Cycle 75 (2026-09-30) — freshness-skip routine cycle; the queue drained: T160 landed, zero rows remain

**T160 done bad8b89 (impl f625418, glm child 32/80 first-try goal-accepted; kimi skipped tests-only, routing d1790748367-2).** The T153 validator's two weak-test survivors pinned, both in src/mcp_serve/tests.rs (+43/-3, zero product edits, zero timeout bumps): **M5** — the argv-needle table gains the mirror near-miss row `("tool run --special cfg", false)`, killing a needle relaxed to bare `starts_with("--spec")` (the comment always claimed `--special` must not match); **M6** — the TERM-ignoring escalation fixture leg now probes the group AFTER the cancel call and BEFORE the FixtureGuard drops, reusing the escalation loop's own `group_gone` (bounded 20×10 ms reap-then-ESRCH, the T153 fix-up convergence shape) plus the probe's reap post-condition (per-pid kill → ESRCH, never a zombie for cleanup), killing the report-only mutant that ships `signaled: kill` while the group stays alive (the guard's cleanup kill used to mask it). **The orchestrator personally RED-proved both named mutants** (the exact two the T153 verdict carried): M5 relaxed → new row FAILED (true vs false on `--special`); M6 `kill_rc = 0` report-only → payload still said `signaled: kill`, probe FAILED at tests.rs:1633 with the intended message; tree restored + green after each (worktree nextest release 1213/1213 + clippy -D clean; post-merge main nextest 1213/1213, target-shared-main). Tests-only → validation optional per T16/T147/T155/T158/T159 precedent; kimi round skipped. **Process note (worktree-mutation lesson):** `edit_file` is cwd-confined to the orchestrator's tree — a mutant applied through it edited the MAIN repo's src/mcp_serve.rs while the gates ran the worktree's copy (mutant "survived" trivially); caught on the first read-back, main restored byte-identical, worktree mutants re-applied via bash perl in the worktree cwd. Next-eval candidate: the check-wall remedy (spec check lines that embed the 600s wrapper vs warm full-suite walls) and the eval-template Outcomes carry-forward pin remain the top two; **the TODO queue is now EMPTY — cycle 76 will fail the freshness predicate's queue half and MUST evaluate** (mandatory fresh eval, kimi per T81). Harvests: events-t160-impl-20260930-060607.jsonl (child LEDGER was a seed shell, not harvested).

### Cycle 76 (2026-09-30) — MANDATORY fresh eval (queue empty at cycle-75 wrap; kimi, loopd-routed) — 7 rows filed (T162–T168); T163 landed; cycle in progress

**T163 done 195204c (impl 8062a2f; routing d1790752429-13, verdict d1790754986-15, outcome d1790755157-16).** The goal-gate check-wall closed: `CHECK_TIMEOUT_SECS` 600→1200 (src/tools.rs, rationale comment naming the t153-fixup double rejection), META-META-SPEC's spec quality bar gains the warm-wall budgeting sentence (targeted binaries ~300s preferred; >~600s walls must be measured and stated in the spec), and tests/loop_spec_check_wall.rs pins all three legs (const parsed not string-matched, doctrine tokens exactly-once in the bar window, rejection message still interpolating the const — non-vacuous). The impl child died at the 35-minute budget at 46/80 with the work COMMITTED (goal unaccepted) — the T63/T156 committed-variant discriminator routed to orchestrator-finish, no resume burned. Kimi VERDICT: PASS (42/60): gates independently re-run under target-shared-validate (1217/1217 release nextest, clippy -D, goal 40/40, check_wall 4/4, loop_spec_recovery 20/20), 4 mutants ALL RED (M1 const→600, M2 compile-clean hardcode in the message, M3 doctrine-sentence deletion, M4 ~600s→~500s reword — each killing its pin leg); one non-blocking nit (the commit's 600-sweep omits LOOP-SPEC.md:140/146 — the delegate wait_secs max, same justified-unchanged class as README's). Post-merge nextest 1217/1217 under target-shared-main. **Kill-rule note:** the validator's launch goal READ as garbled in the orchestrator's own tool-call echo — the cycle-61 verify-then-kill discipline ran SEQUENTIALLY (transcript read-back in a separate step) and the 2513-byte payload proved fully intact (sha matched the launch return); no kill, no relaunch — the render-artifact class caught exactly as the doctrine intends. Harvests: events-t163-{impl,validate}-20260930-0756*.jsonl + LEDGER-t163-validate-20260930-075633.md.

**T164 done c96255d (impl 48a1ca1 via T63 resume #1-of-cycle; routing d1790757559-17, verdict d1790761073-19, outcome d1790761210-20).** The honor-system check-line bans are now a mechanical three-leg lint in tests/todo_consistency.rs (+294): (a) `--lib` rejected in ANY flag position (token/segment-scoped, quote-opaque — closes the t160 `--release --lib` escape the T67 literal-substring lint missed); (b) cargo piped through tail/head/grep requires `pipefail` on the line; (c) no `cd` to an absolute path (the T21 anomaly class). The lint's first corpus run found 17 problems across 13 spec files — the impl's run-1 died at the 35-min budget at 50/80 with the work UNCOMMITTED (T63 incomplete-variant → ONE resume, goal-accepted 11 iters later); the sweep dropped 13 legacy `cd` prefixes, two cd+tail shapes, and rewrote t160's line to `--bin chug mcp_serve` (verified by --list to run the t160 pins). META-META-SPEC gains the one-sentence pipe-masking ban beside the `--lib` ban. Kimi VERDICT: PASS (43/60): gates independently re-run under target-shared-validate (nextest release 1222/1222, clippy ×2, todo_consistency 16/16), 5/5 mutants killed — each lint leg neutered → its fixture + sample-spec tests fail, the doctrine sentence reverted → its pin fails, t160's banned line restored → the corpus lint fails naming t160:41 on all three rules. Non-blocking observations (carried): latent slice-panic on a malformed `&&&` in shell_segments; leg-(b) pipefail check is line-wide substring; `||` treated as pipe (defensible). Eval-authoring note: my spec's offender attribution named t152/t153-chug-cancel-mcp (already clean on main) — the lint caught the real corpus regardless; attribute-by-lint, not by memory, next time. Post-merge nextest 1222/1222 under target-shared-main. Harvests: events-t164-{impl,validate}-20260930-*.jsonl + LEDGER-t164-validate-*.md.

**Cycle-76 wrap notes (2026-09-30, ~09:47 UTC).** TWO rows landed: T163 (195204c — the goal-gate check-wall: CHECK_TIMEOUT_SECS 600→1200 + warm-wall budgeting doctrine; kimi PASS 4/4) and T164 (c96255d — the spec check-line lint: any-position --lib, pipe-masking-needs-pipefail, no absolute cd; corpus sweep 17 problems/13 files; kimi PASS 5/5). Both impl children died at the 35-MINUTES budget mid-arc (T163 at 46/80 committed → orchestrator-finish; T164 at 50/80 uncommitted → ONE T63 resume, accepted 11 iters later) — the T63/T156 family absorbed both, running the era's minutes-death census to ~10 and its routing accuracy to 100%. Validators: kimi REQUIRED ×2, both first-round PASS, 9/9 mutants RED. **Kill-rule render-artifact class CONFIRMED twice this cycle:** both validator launch goals READ garbled in the orchestrator's own tool-call echo; sequential transcript read-back proved both payloads intact (sha-matched); zero kills, zero relaunches — the cycle-61 doctrine doing exactly its job. **NO release tag this wrap:** 2 items since v0.7.0 (< 3) and no FEATURES.md check-off landed (F11 phase 1a is T162, unworked). **README gate: no change needed** — T163's const is not a README-documented surface (the only README 600 is delegate wait_secs, unrelated) and T164 is spec-authoring doctrine. **Queue for cycle 77 (all specs ready, priority order):** T162 (pri 2, FEATURE — F11 phase 1a MCP resources consume legs, the mandatory-pull row; biggest item ~350 est, give it a full arc) → T165 (pri 3, T129 M7/M8 survivor pins, tests-only) → T166+T167 (pri 3, T45 bundle, doctrine — runs alone, kimi REQUIRED) → T168 (pri 4, decision_log description). **Deferred with reason:** none dropped; the cycle simply ran out of minutes with the queue staged. Watch items for the next eval: T164's three non-blocking validator observations (shell_segments `&&&` slice-panic, leg-b line-wide substring, `||`-as-pipe); the T163 commit's sweep nit (LOOP-SPEC wait_secs lines unlisted, same justified class); the orchestrator's own 120s-bash × long-gate dance (nextest release cold-compile needs a retry cadence — SIGALRM at 507/1222 once this cycle, warm retry green in 25s). Final gates at HEAD (c64cc40): build + clippy -D release + nextest 1222/1222 + todo_consistency 16/16, all under target-shared-main. Harvests: events-t163-{impl,validate}-*.jsonl, LEDGER-t163-validate-*.md, events-t164-{impl,validate}-*.jsonl, LEDGER-t164-validate-*.md — no worktree removed unharvested.

### Cycle 77 (2026-09-30) — freshness-skip routine cycle (EVALUATION.md fresh, 5 todo rows); T162 landed; cycle in progress

**T162 done 8120b00 (impl 54334c2; recovery routing d1790764331-2, validation routing d1790764643-3, verdict d1790766799-4).** F11 phase 1a landed — chug now consumes MCP resources, not only tools: a shared `McpCapabilities` catalog parsed at initialize (stdio keeps it; HTTP captures the same struct for phase 1b — no fork), `resources/list` + `resources/read` stdio legs (tools/list framing discipline, LIST/CALL timeout consts, JSON-RPC error replies surfaced with the server's message), `MAX_MCP_RESOURCES=200` warn-and-cap mirroring `MAX_MCP_TOOLS` into the per-server .chug log, and `McpRegistry::list_resources()` / `read_resource(server, uri)` returning structured per-server outcomes where non-capable/down servers are named errors, never panics or hangs. Internal surface only (no model-facing tool — phase 1b); README states the tools+resources consumption and the phasing honestly. Tests (a)-(e) as hermetic python stubs including the stub-side never-queried witness. Impl child died at the 35-min budget at 54/80 with the work COMMITTED (goal unaccepted) → the T63/T156 committed-variant discriminator routed to orchestrator-finish, no resume burned. Kimi VERDICT: PASS (57/60): gates independently re-run under target-shared-validate (nextest release 1233/1233, clippy -D, spec check 162/162); 5 mutants in parallel-3 batches over disjoint hunks — 4 CAUGHT ((b)-uncapped: 250 resources vs the 200 cap; (e)-always-query: the tools-only stub actually received resources/list and replied method-not-found; read-err-drop; presence-rule), 1 informational SURVIVOR carried (list-err-drop: no test pins the resources/list error-reply message surface — the fallback error still names the server, so the Req-3 named-error contract holds; a future eval may file the one-test pin). Post-merge nextest 1233/1233 under target-shared-main. Harvests: events-t162-{impl,validate}-20260930-*.jsonl + LEDGER-t162-validate-20260930-*.md.

**T165 done 12699d9 (impl 2b89172; validation routing d1790767714-6; check-line recovery d1790763842-1).** The T129 validator's two weak-test survivors are pinned, tests-only (+82/-5 across src/mcp_serve/tests.rs + tests/mcp_serve.rs; src/main.rs + src/mcp_serve.rs byte-clean, verified after the RED-proof cycle). M8 (launch-failure isError arm): NEW bin leg `chug_launch_spawn_failure_is_an_is_error_result_and_the_loop_survives` — CHUG_DELEGATE_BIN pointed at a never-created absolute path so delegate_launch fails at Command::spawn (real seam-side ENOENT, not a validation refusal) and pins isError == Some(true) with the tool-named payload plus loop-survives. M7 (CLI plumbing flag-OFF): the T148 default-deny wire leg strengthened into the named pin — refusal pinned byte-exact (code -32602, message verbatim `unknown tool: chug_launch`), replacing contains-only; the drop direction is covered by the T148/T153/T157 flag-ON e2e legs. The spec's stretch (req 4) was already closed by T148's happy-path e2e — noted, nothing deferred. **The first 2-impl overlap ran here (T161 pattern (ii), disjointness gate passed: mcp.rs/mcp_http.rs vs mcp_serve tests)**, and it bit twice through the goal-gate: the spec's check line hardcoded the DEFAULT target-shared dir while T162's child built into it (cargo lock contention + a starved stub-spawn test) — fixed mid-flight by role-keying the check line to target-shared-impl-a (T153 check-line precedent, routing d1790763842-1); the second rejection was a pure load-flake on a pre-existing wire test (`chug_cancel_happy_path` — green solo 1.45s). The child then died at the 35-min budget at 65/80 with the work committed → committed-variant orchestrator-finish (third this cycle — the minutes-death era continues). Kimi SKIPPED per tests-only precedent (T16/T130/T147/T158/T159/T160); the orchestrator independently RED-proved BOTH mutants in the worktree cwd (T167 discipline): M8 isError true→false → the spawn-failure pin FAILED; M7 allow_launch:true hardcode → the default-deny pin FAILED at the tool-set assertion; both restored byte-clean (0 porcelain). Worktree gates nextest 1223/1223 (pre-T162 base), post-merge nextest 1234/1234 under target-shared-main. Harvests: events-t165-impl-20260930-*.jsonl + LEDGER-t165-impl-20260930-*.md.

**T166 done fafbe7f (impl bc813d3 + clippy fix 7911b3c; validation routing d1790771111-8, verdict d1790773587-9, outcome d1790773780-10).** The cycle-72/73/75-named Outcomes-amputation hazard is now pinned: META-META-SPEC.md's Write-EVALUATION.md section gains ONE sentence — a regenerated EVALUATION.md MUST carry forward every existing `## Outcomes` content verbatim (per-cycle sections and per-item entries); the eval rewrites the assessment body only, never the Outcomes ledger; before committing, verify the newest pre-existing cycle's section is still present. New pin file tests/eval_outcomes_carry.rs (4 tests, loop_spec_* conventions): exactly-once load-bearing tokens with a window-scoped Outcomes census (==2 in the Write-EVALUATION section — the Extend-TODO bar legitimately carries one), contiguous core needle + preamble-placement leg, cheap EVALUATION.md `## Outcomes` heading sanity leg. Impl child first-try goal-accepted at 48/80 (the cycle's only non-budget-death child). Kimi VERDICT: PASS (32/60): gates independently re-run under target-shared-validate (nextest 1238/1238, spec check 4/4); 3 parallel T79 mutants RED (M1 delete-sentence, M2 verbatim→unchanged — each flipping exactly the 3 sentence legs; M3 placement — flipping ONLY the placement leg), M4 (heading rename) covered by the commit-msg hand-check under budget. Validator finding 1 (needless_lifetimes in the new pin file — the commit's "clippy clean" claim inaccurate) fixed trivially on-branch (7911b3c, clippy -D clean restored, pin 4/4) before merge. **Gate-family note (T82 fallback exercised):** nextest worktree gates hit the KNOWN reaper-family nextest-parallelism flake (`the_reaper_terms_an_orphan_through_loopd_before_the_build` 30.8s > 30s deadline at 17-way load — the exact T152 signature; solo green 3.36s) AND the T129 stub-spawn flake (`chug_launch_stub_spawn_pins_exact_argv_cwd_and_return_paths` under the threads=4 fallback — solo green 0.55s, family/re-run green); fallback full run green 1238/1238, families named here and in the merge commit. Post-merge nextest 1238/1238 under target-shared-main. Harvests: events-t166-{impl,validate}-20260930-*.jsonl + LEDGER-t166-{impl,validate}-20260930-*.md. **Watch items for the next eval:** the T129 stub-spawn 10s family has now flaked THREE times this cycle under different load shapes (T165 check ×1, T166 fallback ×1, plus T165's second-check starve) — a T151-style serialization row candidate; needless_lifetimes class-check: children claiming clippy-clean without -D (validator caught it — keep the -D invocation in child goals).

**Cycle-77 wrap notes (2026-09-30, ~13:45 UTC).** THREE rows landed: T162 (8120b00 — the cycle's FEATURE, F11 phase 1a MCP resources consume legs; kimi PASS 4/5), T165 (12699d9 — T129 M7/M8 survivor pins, tests-only, both mutants orchestrator-RED-proven), T166 (fafbe7f — the Outcomes carry-forward doctrine + pin; kimi PASS 3 mutants + M4 hand-check; needless_lifetimes fix on-branch). **The first 2-impl overlap (T161 pattern (ii)) ran and worked** — T162 (default slot) + T165 (impl-a) flew together under the disjointness gate — and immediately surfaced a NEW hazard: a spec check line hardcoding the DEFAULT target-shared dir inside an overlap makes the goal-gate race the other impl's builds (T165's check failed once on contention, once on a coincident load-flake; fixed mid-flight by role-keying the check line to impl-a, d1790763842-1, committed post-hoc 607e877). Watch item: eval-time check lines should role-key OR the goal-gate could scrub/pin the dir; next eval candidate. **Budget economics:** 3 of 4 impl children died at the 35-MINUTES budget (T162 at 54/80 committed, T165 at 65/80 committed → both committed-variant orchestrator-finish, no resumes burned; T166's child the sole first-try acceptance at 48/80) — the minutes-death era continues, routing accuracy 100% (4/4 lifetime committed-variant routings correct). Validators: kimi REQUIRED ×2 (T162 feature, T166 doctrine), both first-round PASS; T165 tests-only → kimi skipped with orchestrator RED-proofs (T160 precedent). T82 fallback exercised once (T166: reaper family nextest-parallel flake + stub-spawn flake — both solo-green, families named in the merge commit). **Deferred with reason:** T167 (pri 3, LOOP-SPEC worktree-mutation-cwd sentence + pin) and T168 (pri 4, decision_log description fields) — specs ready and handoff-clean, deferred on wall-clock budget (~27 min left at the wrap decision, routing d1790774023-11; a doctrine arc's validator alone takes ~30 min). **Release: v0.8.0 tagged** (trigger: 6 items since v0.7.0 ≥ 3; minor bump — T162 is a feature; pairing verified via check-tag-version.sh; tag message = generated notes since v0.7.0; final gates at HEAD 1238/1238 nextest + clippy -D clean under target-shared-main). **README gate: satisfied** — T162 landed its own MCP-section bullet; T165/T166 are tests/doctrine with no user surface. **Handoff for cycle 78 (cold-start, zero human words):** queue = T167 → T168 (specs ready; T167 is doctrine — runs ALONE, kimi REQUIRED; T168 touches src/tools.rs — kimi REQUIRED); next eval candidates (watch items): the T129 stub-spawn 10s family (3 flakes this cycle under different load shapes — T151-style serialization row), spec-check-line role-keying under overlap (this cycle's new hazard), the T162 informational list-err survivor (one-test pin), needless_lifetimes/-D class note (children claiming clippy-clean without -D), T164's three carried lint observations. Harvests this cycle: events-t162-{impl,validate}-*, LEDGER-t162-validate-*, events-t165-impl-*, LEDGER-t165-impl-*, events-t166-{impl,validate}-*, LEDGER-t166-{impl,validate}-* — no worktree removed unharvested. Final gates at HEAD (814f12a): nextest 1238/1238 + clippy -D clean + todo_consistency 16/16, all under target-shared-main.

### Cycle 78 (2026-09-30) — freshness-skip routine cycle (eval fresh 09:17); BOTH queued rows landed (T167, T168), queue drained

**T167 done af94cc7 (impl 51f3136; recovery routing d1790777031-1, validation routing d1790778534-2, verdict d1790780904-3).** The cycle-75 T160 hazard is now pinned in doctrine: LOOP-SPEC.md §2 step 3 (Review) gains ONE sentence in the RED-proof / docs-only-guard neighborhood — orchestrator mutation legs for RED-proofs MUST be applied inside the worktree via bash (e.g. `perl -i` with cwd `/tmp/chug-loop-t<N>`), because `edit_file`/`write_file` are main-tree-confined and silently produce false survivors when the gates under proof run the worktree copy, and after any main-tree edit during a round main must be verified restored byte-identical before merging. New pin file tests/loop_spec_worktree_mutants.rs (5 tests, T48/T64/T78/T19/T30 conventions): the whole sentence as ONE flat needle exactly-once; `byte-identical` whole-file exactly-once (0 pre-existing) + step-3 window placement; `edit_file` per-window (step 3's new sentence + step 5's pre-existing sed rule); `worktree` census inside the sentence's own span (exactly 2 — 28 pre-existing whole-file occurrences saturate the file, so no whole-file count can read exactly-once; head/tail anchors token-free of the counted token); placement leg (after the docs-only ambiguity default, no step renumbering). Impl child died at the 35-MIN budget at 47/80 with the work COMMITTED (goal unaccepted) → T63 committed-variant discriminator routed to orchestrator-finish, no resume burned (fourth consecutive minutes-death, routing 5/5 lifetime correct). Kimi VERDICT: PASS (49/60): 5/5 mutants RED in parallel throwaway worktrees with role-keyed target dirs (delete-sentence, byte-identical→byte-clean, drop-MUST, soften-bash-cwd, pin-needle-corrupt), clean gates independently re-run under target-shared-validate (build, clippy -D, spec check 5/5, nextest --release green); 1 cosmetic finding CARRIED (the pin's comment says 27 pre-existing `worktree` occurrences, actual 28 — the test counts within its span, so the census is unaffected; a comment-count fix would re-trigger full gates for zero pin behavior). Worktree gates nextest 1240/1240 (the loopd_stale_binary + loopd_spoof_guard flake family red-only-under-nextest — 90s timeouts at parallel load, exact T152 signature — solo-green under the T82 fallback 3/3, family named in the merge commit); post-merge nextest 1240/1240 + todo_consistency 16/16 under target-shared-main. Harvests: events-t167-impl-validate-20260930-*.jsonl (both runs, one file) + LEDGER-t167-validate-20260930-*.md.

**T168 done 43cb484 (impl 3f0ce66, first-try goal-accepted at 37/80; validation routing d1790783464-7, verdict d1790785783-8).** The decision_log tool description — the surface glm reads every stream (the T22 lesson) — now NAMES its six required fields explicitly: one sentence in `src/decisions.rs::schema()` ("Required in every call: `class`, `subject`, `inputs`, `options`, `choice`, and `confidence` — all strings except `confidence` (a number in 0..=1) — with no field omitted."), verified against the real required-array (names, order, confidence's number type) with no schema shape change (T168 out of scope). Pin `decision_log_description_pins_required_fields_sentence` asserts the LIVE `tool_schemas()` output (T22 precedent) with sentence-unique needles — the load-bearing one is the joined backticked six-field list in required-array order (bare tokens `options`/`choice` already existed in the record-shape parenthetical and would not be RED-provable, the child correctly reasoned) — plus a 5→6 sentence-count leg; RED-proven by reverting the sentence with the pin kept (commit message states the proof). Kimi VERDICT: PASS (46/60): 5/5 mutants RED serial (delete-sentence, drop-options, soften-required, typing-clause, corrupt-pin-needle; serial DECLARED — all legs touch src/decisions.rs so parallel throwaway worktrees would contend one file), clean gates independently re-run under target-shared-validate (build, clippy -D 0, spec check 18/18 incl. pin, nextest --release green). The child self-logged its validation verdict to the worktree decisions file (d1790785494-1, harvested). Branch `loop-t168` was cut from 4a456be before T167 landed → main merged INTO the branch pre-validation (0c62dd4, the orchestrator-owned rebase-equivalent per the overlap doctrine) so the kimi gates ran on the combined tree carrying T167's new pins (1241 tests). Worktree gates nextest 1241/1241 + clippy -D clean + spec check 18/18; post-merge nextest 1241/1241 under target-shared-main. Harvests: events-t168-impl-validate-20260930-162943.jsonl (both runs, one file) + LEDGER-t168-impl-20260930-162943.md.

**Cycle-78 wrap notes (2026-09-30, ~17:00 UTC).** Phase 1 SKIPPED per the freshness predicate (EVALUATION.md fresh from the cycle-76 eval, 2 todo rows pending — the routine routing; the cycle-77 wrap had deferred both rows with reasons). TWO rows landed, both arcs complete: T167 (af94cc7, pri 3 LOOP-SPEC doctrine — the T160 worktree-mutation-cwd hazard pinned + 5-test pin file) and T168 (43cb484, pri 4 src/decisions.rs — the decision_log description names its six required fields + a live-schema pin). **Queue DRAINED** — the freshness predicate's queue half now fails, so cycle 79 is a MANDATORY fresh eval (kimi-routed per T81). **Validators:** kimi REQUIRED ×2 (T167 doctrine, T168 schema surface), both first-round PASS, 5/5 mutants RED each — T167's in parallel throwaway worktrees (disjoint tests), T168's serial with the overlap judgment declared (all legs touch src/decisions.rs). **Budget economics:** 1 of 2 impl children died at the 35-MIN budget with work committed (T167 at 47/80 → committed-variant orchestrator-finish, sixth consecutive correct committed-variant routing lifetime); T168's child first-try accepted at 37/80. **T82 fallback exercised again** for the loopd_stale_binary + loopd_spoof_guard flake family (90s timeouts under nextest parallel load — the T152 signature, fourth cycle running; solo-green 3/3 under the fallback each time; families named in merge af94cc7) — a T151-style serialization row candidate stays on the next eval's desk, alongside the T129 stub-spawn family. **New pattern worth the doctrine (worked first try):** T168's branch predated T167's merge, so main was merged INTO loop-t168 pre-validation (0c62dd4) and kimi's gates ran on the combined tree — the orchestrator-owned rebase-equivalent the overlap doctrine names, first live exercise; candidates for a next-eval note: branch-creation order (create N+1's worktree AFTER N's merge lands, when queue order allows) would remove the extra merge. **No release tag:** 2 rows since v0.8.0 (< 3) and no FEATURES.md check-off — trigger does not fire. **README gate: satisfied** — both items are model/doctrine-facing (a LOOP-SPEC sentence; a tool-description sentence), no user-visible surface; README's decision_log entry stays behaviorally accurate. **Deferred/carry-forward for the next eval's corpus:** T167's cosmetic comment count (27 vs 28 in the pin's header doc — informational, carried), the loopd + stub-spawn nextest-parallelism flake families (4th cycle), spec-check-line role-keying under overlap (cycle-77 watch item, unchanged), the T162 informational list-err survivor (one-test pin), needless_lifetimes/-D class note, T164's carried lint observations. Housekeeping this wrap: 59 stale /tmp/chug-mut-* throwaway worktrees from prior cycles' validators removed (unregistered orphans, no children flying). Harvests: events-t167-impl-validate-20260930-150824.jsonl, LEDGER-t167-validate-20260930-150824.md, events-t168-impl-validate-20260930-162943.jsonl, LEDGER-t168-impl-20260930-162943.md — no worktree removed unharvested. Final gates at HEAD (4bcf898): nextest 1241/1241 (+3 known-flake skips, fallback-covered) + clippy -D clean + todo_consistency 16/16, all under target-shared-main.


**Deferred with reason: T175 + T176 + T178 (doctrine bundle, all pri 3, specs ready).** Wall-clock: the four landed rows + two budget-death recoveries (T170's repeated-error resume, T171's iteration-ceiling resume) + two kimi rounds consumed the cycle; the bundle is solo-only by doctrine (no child may overlap it) and its impl+REQUIRED-validation needs ~70-80 min that did not exist after T171's merge. Handoff is cold-start-ready: specs/t175-overlap-check-rekey.md, t176-impl-goal-clippy-deny.md, t178-loopd-bash-timeout-300.md are complete and the dispatch-time check re-key + clippy -D goal text are ALREADY being practiced (9d5bc55 + this cycle's T170/T174 dispatches) — T178's fix is the highest-value one (the 120s bash cap vs the 600s alarm idiom cost this cycle 5+ killed gate/compile calls; every stream shows the same tax). Next cycle: fresh-eval (kimi) routes, then the bundle works first among pri-3s.

### Cycle 79 (2026-09-30) — MANDATORY fresh eval (queue drained at cycle-78 wrap; kimi, loopd-routed) — 10 rows filed (T169–T178); 3 landed (T173, T172, T169 + the F11 1b-i roadmap pull); 7 deferred with reason

**T173 done 553928a (impl e32829f, first-try goal-accepted 65/80 in ~26 min; validation routing d1790787764-17, verdict d1790791665-18).** Minutes are no longer the binding child budget on paper: LOOP-SPEC's three surfaces moved 35/40→50/50 (step-2 delegate template + explicitness sentence + a new T21-class impl-side measure clause carrying the cycles-76–78 census 5-of-7 and a >2-of-8 tripwire; the T63 echo "(80/50 impl, 60/50 validate)"; step-4's validator pair + measure re-key "dies at 60/50" + the cycle-79 validator census evidence). tests/loop_spec_recovery.rs moved in the same commit — re-keyed needles plus NEW legs (u)–(x) pinning the step-2 template fragment for the first time (+458/-45 total; the eval's sentence+pin-file calibration predicted exactly this shape). Kimi VERDICT: PASS (44/60, budget_low fired on minutes): 6/6 requirements verified, acceptance greps 11/11 needles unique, gates independently re-run under target-shared-validate (spec check 24/24, clippy -D, nextest 1248/1248), M1 step-2-template-revert + M2 T63-echo-revert both RED serial (declared overlap — all mutants touch LOOP-SPEC.md), tree byte-clean at verdict. **Kill-rule render-artifact class confirmed a THIRD time**: the validator's launch tool-call echo rendered its `spec` parameter garbled ("/Users/jadams MAY run the mutation legs…"); sequential read-back of the child's on-disk artifacts (ps argv + events run_start) in a SEPARATE step proved the spec path and the 2425-byte goal payload fully intact (sha-matched the delegate return) — no kill, no relaunch, validator healthy at iteration 4. Post-merge nextest 1248/1248 under target-shared-main. Harvests: events-t173-impl-20260930-180745.jsonl, events-t173-validate-20260930-180745.jsonl, LEDGER-t173-impl-20260930-180745.md.

**T172 done 7d2f7e6 (impl d7ccd1b + orchestrator-finish df47d6b; recovery routing d1790794250-20, validation routing d1790794368-21).** The 4-cycle flake era gets its mechanism: a NEW `tests/support/load_lock.rs` cross-binary advisory-flock harness (the gap in T31's static-Mutex doctrine — nextest runs each test as its own PROCESS, so per-process mutexes cannot serialize the deadline families across binaries; a flock on `chug-t172-load-locks/<family>.lock` under the shared target dir can, under BOTH runners) applied as the FIRST acquisition in 24 sites across the loopd supervisor harnesses + the mcp_serve stub-spawn family. Fail-open everywhere (unopenable lockfile / expired 45s bounded wait / no creatable dir → stderr note + proceed unlocked, never a hang; kernel releases on holder death; lockfile never unlinked). **ZERO second-valued constants bumped** — the acceptance grep `from_secs\([0-9]` is byte-clean, and the harness's own new bounds are expressed in millis to keep it that way (the child's final note + df47d6b polish). The impl died at the ITERATION ceiling (80/80 in ~35 min — glm ran fast on tests-only; the new 50-min budget was NOT binding, first data point for T173's measure clause) with work complete+committed → committed-variant orchestrator-finish, no resume burned. Kimi SKIPPED (tests-only, no REQUIRED-list file) with orchestrator personal RED-proofs: M1 (zero the bounded wait) and M2 (bypass try_lock) each kill the exclusivity + bounded-wait semantics legs, reverts green 3/3. **First full-parallelism nextest green for the loopd families in 4 cycles**: 1255/1255 in the worktree (target-shared-gates) AND post-merge in main (target-shared-main, 22.5s — serialization costs no meaningful wall when not starved).

**T169 done af4ff7a (impl 7fc1e07 via T63 resume d1790794686-23; validation routing d1790795778-24, verdict d1790798730-25).** The mandatory roadmap pull lands — F11 phase 1b-i gives the model its MCP resource surface: one builtin `mcp_resource` (list/read actions) in the LIVE tool_schemas(), dispatching beside the mcp__ branch in the driver (read-only risk-gate bypass, same posture, T90 deny proven to block pre-dispatch). The capability gate stayed single-source (T162's legs moved verbatim into a private shared helper; public signatures untouched); T88-shaped param errors via the newly pub(crate) received_hint; list renders `server uri — description (mimeType)` lines with name fallbacks; read renders text inline / blob base64 char-capped pinned to web_fetch's constant. 11 new tests including the capability-gate stub never-queried witness and a run_turn e2e. **T63 resume textbook case**: the impl died at ITERATION 80/80 in ~18 min (glm at ~4.4 iters/min — the MINUTES raise didn't bind; the ITERATION ceiling did) with +811/-27 uncommitted across 6 files → ONE resume → the child committed 7fc1e07 and goal-accepted on the resume segment (20 iters). Kimi VERDICT: PASS (52/60, ~49 min — the 50-min validator budget JUST covered it; at 40 this is the unannounced-verdict death class the T173 census predicted): gates independently re-run (spec check 174/174+11/11, nextest 1259/1259, fallback clean, clippy -D), 6/6 mutants RED via T79 parallel throwaway worktrees (gate-removal, description-drop, is_error-flip, truncation-drop, branch-rename, header-drop), 2 informational notes carried (no two-server witness pin; zero-server named-list message), tree byte-clean. **First full T161 2-impl overlap cycle** (T172‖T169, disjointness declared: loopd/mcp_serve tests vs mcp.rs/tools.rs/driver.rs): T169 ran its warm build + all cargo in target-shared-impl-a with the spec check line role-keyed at dispatch (9d5bc55, the T175 doctrine practiced pre-landing); merges stayed strictly serial (T172 then T169); zero cross-slot artifacts observed. Post-merge nextest 1266/1266 under target-shared-main. FEATURES.md F11 annotated 1b-i LANDED.

**Cycle-79 wrap notes.** Worked in queue order: T173 (doctrine, alone, kimi REQUIRED) → T172+T169 (the first full T161 2-impl overlap cycle, disjointness declared: T172's loopd/mcp_serve test files vs T169's mcp.rs/tools.rs/driver.rs; T169 held impl-a with its check line role-keyed at dispatch per the T175 doctrine practiced pre-landing; merges strictly serial). **Deferred with reason (all pri-3/4; cycle minutes binding, not iterations)**: T177 (&&& lint panic — spec ready, ~40-line fix), T170 (prompts legs — spec ready), T171 (HTTP legs — spec ready, after T170), T178 (loopd CHUG_BASH_TIMEOUT — doctrine, runs alone), T175+T176 (doctrine bundle, runs alone), T174 (list-err pin — tests-only). Queue is handoff-clean: every todo row has a ready spec and the eval's Handoff section carries the work order + validation preview. **Cycle-level lessons**: (1) T173's minutes raise was validated the moment it mattered — the T169 validator finished at ~49 min (an unannounced-verdict death at 40); BOTH impls this cycle died at the ITERATION ceiling instead (80/80, glm at ~4.4 iters/min on well-spec'd rows — the measure clause's next data point: if iteration-ceiling deaths with uncommitted work become the norm, the ceiling or spec-size is the next lever). (2) The T63 resume is textbook-proven end to end (die uncommitted → resume → commit → goal-accepted on the resume segment). (3) The kill-rule render-artifact class confirmed a third AND fourth time (two garbled launch echoes this cycle; on-disk read-back proved both payloads intact; zero kills). (4) T172's load-lock gave the loopd families their first full-parallelism nextest green in 4 cycles, in main. Validators caught: nothing blocking this cycle (3 first-round PASSes) — the era's findings are now informational pins (T174 carries one).
