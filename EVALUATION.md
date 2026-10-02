# EVALUATION — chug, assessed by chug-loop (2026-10-02, cycle 91)

MANDATORY fresh eval — the cycle-90 wrap drained the queue (0 `todo`
rows; digest confirms `todo=0 done=189`), so the freshness predicate's
queue half fails and this cycle was loopd-routed to kimi per T81 (the
fourth consecutive correct routing prediction: kimi has orchestrated
every queue-empty cycle since T81 landed). Delta corpus since the
cycle-85 eval (`a0c44c1`): cycles 85(part 2)–90 — six orchestrator
streams (kimi ×2, glm ×4, 512 iterations / 24.0h wall) plus ~20 child
streams — read via the fresh digest (461 files, 27,213 iterations,
FRESH per the digest's own T148 self-exclusion) with drills into
`src/tools.rs` (recv_capped, T185 verify), `src/delegate.rs`,
`src/decisions.rs`, `scripts/eval-digest.sh`, `.chug/decisions.jsonl`,
and the full git record (`a0c44c1..HEAD`, 84 commits). Headline: the
delta landed 11 of 11 queued rows (T184, T185, T193, T188, T194,
T189, T190, T191, T186, T192, T187 — four of them cycle-85 filings,
seven operator-batch), zero reverts, two self-tagged releases
(v0.12.0, v0.13.0), and CLOSED the F14 roadmap item (telemetry +
live-context editing). The delta's costs cluster in three mechanical
classes — gate artifact-integrity (3 fires), dispatch re-key fumbles
(3), and impl budget deaths on feature rows (6 of 10 impl arcs) —
each with a row below.

## 1. What chug does well

- **The queue fully converts.** 11/11 landed including two ~2k-line
  features (T188 auto-spec 2573 all-in, T192 live-context 2099) and
  the T194 orchestration fleet itself — and two releases self-tagged
  with generated notes and pairing checks.
- **The recovery machinery is the load-bearing surface, and it held
  100%.** ~8 T63 resume/orchestrator-finish routings across the
  delta, all correct (committed-complete → orchestrator-finish,
  uncommitted → resume, cap ONE); zero bad merges, zero lost work
  (T186's harvest-all doctrine live-tested on its own worktree).
- **T185 verified holding in production** (fix assessment §3).
- **T184's telemetry is immediately load-bearing:** the digest now
  shows cache-read 13–17.2M vs 0.3–1.3M non-cached input per
  orchestrator stream — prompt caching carries ~95% of the context
  economy — and exactly 1 trim fire across 6 cycles: the F14
  summarization deferral is now MEASURED-correct, not asserted.
- **New machinery dogfooded in-cycle:** the T194 2-impl overlap ran
  T190∥T191 with serial merges; T193 was routed gates-only on the
  T189 lane predicate BEFORE the lane landed; validators killed
  27/28 mutants era-wide per the cycle-88/89 census with zero
  blocking findings in 4 REQUIRED rounds.

## 2. Incidents worth fixing

1. **T55-class gate poisonings ×3 in one cycle — the promised
   touch-guard was never filed (→ T195, pri 1, doctrine SOLO).**
   Commit `ee2ff37` (cycle-85 wrap bookkeeping) records "T55-class
   artifact events ×3 this cycle … touch-before-gates guard to next
   eval" — a handoff the next two evals never received (both were
   freshness-skips; this is the first fresh eval since). The fires:
   (a) `74d3331` — t184's goal gate built the 9-test eval_digest into
   `target-shared`; t185's validator check then ran that foreign
   binary against its 8-test sources (2 false reds, diagnosed by the
   t184 validator); (b) a Sep-29 stale eval_digest binary false-red
   on the t184 review gate (T55 touch-rebuild recovered); (c) a third
   recorded in the same wrap note. Role-keyed dirs (T52/T57) bound
   the class per role but do NOT close it — within one role dir,
   sequential checkouts poison each other whenever the gated
   checkout's sources are OLDER than a previous tenant's artifacts.
   Measured this eval: `touch src/main.rs` forces an 18s release bin
   rebuild (stable cargo has no content-hash skip for local crates);
   a full `touch src/*.rs tests/*.rs` + all-targets release rebuild
   is 29.7s — the guard costs ~30s per gate run, against ~30 min per
   false-red diagnosis and a SILENT false-green leg. T195 wires the
   touch into every non-main gate template + the check-line
   convention, main-dedicated dir exempt by T57 construction.
2. **T188 fmt-noise: an impl child mass-formatted the repo (→ T196,
   pri 2, doctrine SOLO).** The round-3 glm impl ran a tree-wide
   `cargo fmt` — 83 files, +12.6k/−9.2k uncommitted noise on top of
   the real change; the T63 resume spent its first act stripping it
   to the 775 real insertions. The cycle-87 wrap handed the class to
   this eval verbatim ("impl-child tree-wide formatters need a
   goal-template ban"); the step-2 goal template currently licenses
   the gesture by silence.
3. **Dispatch re-key fumbles ×3 — the three-surface agreement is
   manual (→ T197, pri 2).** `74d3331` (cycle 85), `98f6f6b` and
   `82804fb` (cycle 87): the check-line `CARGO_TARGET_DIR` re-key
   landed in the BRANCH-side spec copy while `delegate launch`'s
   `spec:` reads MAIN's — the child's goal gate then ran a different
   dir than its goal/env carried (the T144-scrub + T52-artifact-name
   composition makes that a correctness hazard, not just contention).
   Each bite was self-caught and fixed mid-flight (~5–15 min each).
   The mechanical fix point is the launch itself: `delegate` holds
   spec, goal, and env in hand — an advisory WARN on drift fires
   exactly at the fumble moment and can never be skipped.
4. **loopd's cycle wall is the new binding budget (→ T198, pri 2,
   doctrine SOLO).** Delta cycle walls: 3h55m, 3h23m, 3h56m,
   **4h01m — cycle 88 DIED at the 240-minute cap mid-arc** (wrap
   finished by cycle 89), 2h47m, 5m42s. Three of six cycles ran
   within 5 minutes of the cap; T194's fleet lands MORE items per
   cycle. 360 = p95 (~4h) × 1.5; iterations stay 200 (cycle 89's
   195/200 is a watch item, not a trip). The T27/T36 precedent,
   one lever up.
5. **Validator verdicts still die inside dying contexts (→ T201,
   pri 3, doctrine SOLO).** Unannounced-verdict census this delta:
   t185-validate (50-min wall, verdict via T63 resume), t188-round4
   (60/60, verdict from transcript), t192-validate (60/60, verdict
   from the log) — 2 of the last 10 strictly-unannounced, so the
   T173-era mutation-leg-trim clause is NOT tripped (no leg trim).
   But the t186 validator's post-goal_complete WEDGE (14 min silent,
   verdict fully rendered in its log) is the same shape one step
   further. One sentence — verdict lands in `.chug/verdict.md` at
   decision time — makes every future death mode lossless for the
   price of a heredoc.
6. **T192's carried loose ends (→ T202, pri 3).** (a) kimi finding 1
   (explicitly carried): `token_budget_binds_on_free_edit_turns`
   scripts only REJECTED edits — no genuinely-free turn ever
   exercises the usage-skip it claims to pin (vacuous for its name);
   (b) `ctx_edit` events land (0 fires measured across ALL 545
   harvested streams — the "2 fires in the cycle-89 orchestrator
   stream" claim was an eval misread of a combined grep, corrected at
   the cycle-92 merge per validator finding 1) but
   `scripts/eval-digest.sh` never surfaces them — the
   F14 adoption metric is invisible in the loop's own telemetry
   (T192's spec named the lines "for the T184-class digest").
7. **Weighed and REJECTED** (each logged eval-triage):
   (a) **mutation-leg trim** — census 2/10, clause not tripped; the
   T201 verdict-first covers the actual loss mode.
   (b) **glm `decision_log` fumbles ×2** (cycle-86 stream:
   options-must-be-a-string + missing-fields) — the tool error names
   the constraint; T182's reminder holds at 12/14; ~2-iteration cost,
   self-correcting.
   (c) **cycle-88 python Tracebacks ×3** — the glm orchestrator's own
   jq/glue heredocs (`substring not found` ×2, one assertion);
   self-corrected within seconds each; no harness involvement.
   (d) **`path escapes cwd` ×2** (cycles 86/89, worktree reads via
   file tools) — the T41/T61 surfaces (description + fire-time
   remedy) are working; ~1/cycle residual is first-touch discovery.
   (e) **validator-exit-wedge root-cause row** — one sighting (t186),
   mechanism unknown (post-goal_complete exit hang); T186's exact
   removal precondition covers the actionable surface; watch.
   (f) **check-line clippy leg for the T166 class** (3rd sighting:
   `bff6630` useless_concat under a "clippy clean" claim) — the class
   fires only on orchestrator-finish arcs, where the orchestrator's
   own clippy gate catches 100%, and completing children are caught
   by validators; the net exists at two layers and the cost is one
   on-branch fixup. Not filed.
   (g) **loopd iteration raise (200→)** — cycle 89's 195/200 is the
   first >95% sighting; one data point, no row; the next eval
   re-checks.
   (h) **`edit_file` old-not-found ×3** (cycle-85 eval stream) —
   self-corrected, the error names counts; carried watch.

## 3. Friction hot spots — fix assessment

- **T185 (reader-grace): VERIFIED HOLDING.** Post-merge validator
  streams still fire the note ~2–3× each (`reader did not drain`),
  but the partial output is now KEPT and the exit code stays 0
  (spot-checked t187-validate: `GATE-PID=18311\n\n(output truncated:
  reader did not drain)\n[exit code: 0]`); zero drain-induced
  re-run iterations observed in the delta. The residual fire rate is
  the mechanism (backgrounded grandchildren holding pipes) occurring
  benignly, exactly as the fix intended.
- **T182 (decision_log reminder): holding** — kimi and the glm
  orchestrators recorded 80+ decision records across the delta with
  only the 2 glm fumbles in §2.7(b).
- **T184 (telemetry): load-bearing immediately** — see §1; the
  digest's `trim fires` lines gave this eval its context-ceiling
  census in one grep.
- **`git add` on gitignored paths ×2** (cycles 86/89 — `.chug`
  harvest paths): self-correcting friction, ~1 iteration each; no
  row (the error names the ignore rule).
- **Estimate recalibration** (the META-META duty — text, no threshold
  edit). Filed estimate → actual all-in lines for the delta's 11
  landed rows: T184 230→526 (2.3×), T185 130→257 (2.0×), T186
  70→372 (5.3×), T187 25→30 (1.2×), T188 400→2573 (6.4×), T189
  200→1171 (5.9×), T190 300→1297 (4.3×), T191 250→304 (1.2×), T192
  450→2099 (4.7×), T193 120→119 (1.0×), T194 300→879 (2.9×).
  Pattern: docs/small-doctrine rows land at 1.0–1.2×; feature/src
  rows at 2.0–6.4× — the doctrine's "~1.5–3× test/doc density"
  guidance under-reads features ~2×. Correlation with budget deaths
  is sharp: every row ≥879 actual lines took ≥1 impl budget death;
  every row ≤526 landed clean or with ≤1 resume. The ~500-line
  ceiling remains right — the failure is the ESTIMATES, which
  undercount deliverables at filing time (each enumerated requirement
  costs its test legs × ~15 lines + pins + docs). Calibration for
  future filings: count bottom-up from the requirements list, and
  treat any estimate as a floor when the spec lists >8 test legs.
  This text is the calibration record; the threshold number is
  untouched (a number edit would need its own row — none filed).
- **T173 minutes-measure clause: TRIPPED, adjudicated.** Deaths at
  the 50-minute wall with the goal unaccepted in the delta's impl
  children: t186 (50m39s), t188-fixup (52m15s), t190-seg2 (~50-min
  wall) — 3 of the last ~10, clause trips at >2 of 8. T173's remedy
  is "spec-size discipline instead of further raises": adopted — the
  discipline is the §3 calibration above (filing-time) plus T195's
  gate-touch (which removes a whole class of mid-arc diagnosis that
  eats child minutes); NO budget raises (the 80/50 budgets are not
  the failure — estimate honesty is).
- **Impl budget-death census:** 6 of 10 impl arcs had ≥1 death
  (t184 80/80→resume✓; t186 50-min committed→orch-finish; t188
  80/80×2 + 50-min; t189 80/80→resume✓; t190 80/80 + 50-min→
  orch-finish; t192 80/80×2→orch-finish; t194 80/80 committed→
  orch-finish). Zero bad merges; the recovery surface held 100%.

## 4. Capability gaps — ROADMAP PULL: F13 phase 2a → T199 + T200 (decision-corpus distillation readiness)

The pull doctrine fires on F13 (tier 1, top unworked). Its deferral
("pending corpus + layad endpoint") has half-expired: **the corpus
half is now satisfied** — 791 records: 237 eval-triage, 132
validation-routing, 123 validation-verdict, 92 recovery-routing, 200
outcome backfills (153 landed-clean, 40 fixed-up). And it is NOT
distillation-clean — measured this eval: 7 outcome records carry
free-text prose in `choice` (polluting the closed label set the
classifier trains against; `decision_log` validates required fields
but accepts any string), and one correction record exists precisely
because an outcome's `subject` misattributed its decision id. The
layad FINE-TUNE endpoint remains absent (T190's notify sink proves a
layad HTTP endpoint exists for notifications — inference-shaped, not
a training pipeline). So phase 2 splits per the established pattern:
**2a-i → T199** (write-time outcome-choice enum + subject lint +
`scripts/decisions-audit.sh` corpus-health summary; corpus history
immutable — the 7 violations are grandfathered, counted, never
rewritten) and **2a-ii → T200** (`scripts/decisions-export.sh`:
decisions joined with their outcome labels to deterministic training
JSONL — the operator-consumable artifact and the thing an eval can
eyeball). **2b stays deferred** (fine-tune pipeline + confidence-gated
first-pass routing) — endpoint absent; FEATURES.md re-annotates.
Every other deferral stands with its written reason: F2-2b
(chat-only), F3 phase 2 (Stop-hook surface; layad consumer absent),
F4/F5/F6/F7/F8 phase-2s, F10-3b (no MCP-notification consumer), F11
later phases (no consumer for model-facing prompts), F12-2 (no keyed
consumer), F14 summarization (NOW MEASURED: 1 trim in 6 cycles, cache
carrying 13–17M reads/stream). **New finds: NONE** — benchmark
coverage is phase-1 complete across FEATURES.md; the loop's marginal
value has moved to integrity/observability, which is exactly what
this eval files.

## 5. Top 3 priorities

1. **T195 (pri 1)** — gate artifact-integrity. The false-green leg is
   silent: a stale PASSING binary masking a real failure lands bad
   merges with green gates. Highest blast radius per line in the
   queue.
2. **T199 (pri 2)** — F13 phase 2a-i. Features are first-class, and
   this is the distillation precondition: the corpus is measurably
   dirty (7 label violations) exactly where the future classifier
   reads.
3. **T198 (pri 2)** — the throughput lever for the T194-fleet era:
   the 240-minute wall already killed one 6-item cycle.

## 6. README audit

(a) **Reading order: sound.** Install → Quickstart (with the T188
auto-spec and T189 lane woven in) → Runbooks (T191, landed this
delta — the newcomer path is now guided end-to-end) → chat → run →
fork → plan → TUI → Tools → policy surfaces (risk gate, hooks,
permissions) → MCP (client + serve) → observability → self-hosting →
Continuous mode → Development. Order is by reader need, not accretion.
(b) **Redundancy:** `target-shared-validate-b/` is introduced twice
in Continuous mode (cache-list paragraph + T194 overlap paragraph) —
the two carriers hold complementary content (existence vs when it
flies); tolerated, no edit. (c) **Staleness:** none found at
prominence — the delegate/notify/live-ctx/auto-spec surfaces all
document their landed forms (verified against §411–560 and the
Continuous section). (d) **Balance:** the delegate paragraph runs
spec-dense but T51's trim discipline holds (agent-facing depth lives
in specs/). (e) **Quickstart truth:** the load-bearing commands were
executable-verified during T191's arc this delta (against a
current-source build). **Two micro-findings FIXED in this eval
commit:** the malformed `- - **Events log**` bullet at line ~300
(stray double dash — renders as a nested bullet) and "four gitignored
build caches" naming FIVE dirs in Continuous mode. No structural
debt → no docs row filed.

## Handoff — the next cycle reads this

Queue (8 rows, all spec'd): **T195** (pri 1, doctrine SOLO — run it
FIRST and alone) → **T197** (src/delegate.rs; NOT disjoint from T199
— both touch README.md — so serial) → **T199** (feature pull;
src/decisions.rs + scripts/ + tests) → **bundle T196+T198+T201**
(T45-eligible: each ≤30 lines, doctrine-only, pri ≤3 — ONE child,
one commit per row in queue order, ONE kimi round REQUIRED for the
doctrine set; the bundle runs SOLO) → **T200** (sequences after T199;
pairwise-disjoint files if overlapped with a non-doctrine row) →
**T202** (tests/live_ctx.rs + scripts/eval-digest.sh + goldens;
kimi-skip expected per the lane predicate — compute the four inputs
at dispatch). Watch items for the next eval: loopd iterations
195/200 (one sighting), the fmt ban's adoption (first post-T196
feature arc), verdict.md adoption (first post-T201 validator death),
the validator-wedge class (one sighting), post-360 cycle walls. No
SELF-SPEC-shaped rows; no human-decision items.

## Outcomes (filled at cycle wrap — LOOP-SPEC Phase 3)

### Cycle 94 (2026-10-02, ~19:40 UTC–) — kimi routine freshness-skip cycle (predicate held post-reconcile: eval cycle-91 same UTC day + operator T203–T205 rows merged in) — opened with the ahead-10/behind-6 reconcile (85b63a0)

- **T203** (DEPENDENCIES.md audit doc) landed `c3c5975` (merge of loop-t203: 8ef6ce9) — the operator's 2026-10-02 dependency audit codified: 117-line DEPENDENCIES.md with the inventory table (9 runtime crates + 2 dev-deps + LLM proxy + 5 services + 6 spawned-process rows — dep / kind / required-for / env knobs / failure mode / first-added-by row), the "one hard dependency: the LLM proxy" fail-closed section, the exhaustive env surface (every `std::env::var`/`var_os` literal in `src/` + `build.rs` — verified mechanically in BOTH directions at review: every source literal appears in the doc, every doc-listed var appears in source), the three-polarity failure-semantics spine (fail-open / fail-closed / blocking), and the adding-a-dependency rules (check the doc first; fail-open unless policy surface; default-features off; record the row) + README Development-section link (integrated, no append-sprawl). Impl arc: glm child committed 8ef6ce9 then died at the 50-minute wall 57/80 (the T173 census ticks again — minutes binding on a docs row whose spec `check:` is the full `cargo test`: each check run costs minutes of the child's wall) → orchestrator-finish per T55/T150 (committed-complete, no resume burned). Docs-only classification (+121/-1, every file .md): guard floor green worktree + main (todo_consistency 19/19 + readme_layout 1/1 — README is a pinned carrier, its pin ran with the floor). kimi skipped per the T189 gates-only lane, all four mechanical inputs diff-computed (routing d1790974130-2). Sequencing note: T203 worked first despite pri 3 < T204's 2 — T204 req 6 edits this file (dependency-order decision d1790970794-1).

- **T204** (baked-in Laya judge daemon, F15 phase 1) DEFERRED mid-arc to next-cycle recovery — three glm child deaths, all budget (80/80 iteration ceiling ×2 at sprint pace ~6 iters/min, one 50-min wall), with real committed progress each round: the item is structurally ~2× a single child budget (the T110 spec-size class — the operator-filed ~800-line estimate needed a split the row never got). Orchestrator pre-staging is the cycle's force multiplier and it held: venv (torch 2.14.1 + transformers 5.18.0), both HF snapshots cached (base @55cf4c4e 842MB, stop-judge @c1931d4f 1.68GB), reference Python sources mirrored to /Users/jadams/models/laya/ref/, golden vectors GENERATED from the Python SDK cpu/fp32 (8 fixtures — readme billing observed 0.9653 vs the spec's named 0.9607, dtype/device drift noted for the pin), candle-transformers 0.11 modernbert availability verified pre-dispatch (no encoder port needed), candle dep tree pre-compiled into target-shared (1m58s) so no child ever cold-compiled it. Landed on the unmerged loop-t204 branch: bed129e (judge_pack.rs 784-line packing/answer-assembly port, byte-exact state serialization via an order-preserving OValue pair-list, token-exact weightless fixtures replayed through the SDK internals incl. raw logits), 235ea0d (README module-list pin fix — the part-1 commit's real defect, misattributed by its child to load: caught by the orchestrator's quiet-host gate), and the parser checkpoint (parse_ordered — Python-json-faithful ordered parser, surrogate pairs, dup-key first-position-last-value; to_internal port; 3 unit tests; round-2 child died pre-commit, orchestrator fixed 2 clippy lints, lib 1179/1179 + clippy -D green). Routing records: T63 resume d1790975423-4 (run 1 → resume), fresh-round-2 d1790979472-5 (post-resume-exhaustion, T188 multi-round pattern) — a third death made a fourth dispatch this cycle a wall-clock loss (~2h50m left vs ~3h20m needed). Recovery recipe written on the row: worktree persists, split into child A (inference core) / child B (daemon + lifecycle + client + docs), kimi REQUIRED before merge. Process lesson for the next eval: at ~6 iters/min sprint pace, an 80-iteration ceiling is ~13 minutes of wall — for big features the ITERATION budget now binds harder than minutes; consider the census.

- **T205** (HF org hosting) DEFERRED untouched — sequences after T204 (its req 1 extends the daemon surface); org-side steps stay operator-blocked pending the Monday 2026-10-05 hosting decision per the spec's own interim posture; loop-side scope (revision pinning, HF_TOKEN passthrough + 401 honesty, CHUG_HF_ENDPOINT, publish contract, no-secret pin) noted ready-to-dispatch on the row.

### Cycle 93 (2026-10-02, 14:42–18:15 UTC) — glm routine freshness-skip cycle (predicate held: eval 10:15Z same UTC day + 2 todo rows T199/T200) — 2/2 landed, QUEUE DRAINED; operator pushed 6 rows (T203–T205) mid-cycle → push deferred to next cycle per the no-mid-cycle-reconcile doctrine

- **T200** (distillation export, F13 2a-ii) landed `48bd6c9` (merge of loop-t200 impl 5a2e45c) — the join half of the F13 pull, landing one cycle after T199: `scripts/decisions-export.sh` emits one training row per non-outcome record in file order (`{id, ts, class, subject, inputs, options, choice, confidence, outcome}`), first-outcome-wins via an object-indexed label map (`//=` — chronology = file order under append-only), outcome records appear ONLY as labels never rows, grandfathered out-of-set choices pass through verbatim AND count on the stderr summary `export: N rows, M labeled, K grandfathered choice-violations` — K deliberately counts ALL out-of-set outcomes including unresolved subjects, so the audit's violation section and K agree by construction. 10 golden legs (byte-level field-order pins — the validator's M4 mutant proved the Value-level `is_null()` assertion alone would MISS a dropped key: serde reads a missing key as Null, so the byte pins carry the leg). Deterministic (sha256-identical across runs), pure filter (corpus sha stable through the whole validator review), 0.433s at 10x corpus. glm impl child goal-accepted 74/80 first-try — the FIRST solo first-try child of the recent cycles (T199's needed a resume + orchestrator-finish; T200's clean run is the counter-example the minutes-wall census needed). kimi REQUIRED PASS (routing d1790964114-8, criterion b fails at +625/-2): 6/7 mutants CAUGHT, M7 (`sort_by(.id)`) SURVIVES non-blocking — every fixture emits lexicographic ids so no leg distinguishes file order from sorted order; the script is correct (verified live, head/tail + mutant changes output bytes); hardening for the next eval: one same-epoch unpadded-seq inversion fixture (`...-10` before `...-2`). Live acceptance: 626 rows / 183 labeled / 7 violations, delta-0 vs an independent raw-jq recount of the audit's bfset join; per-class decomposition exact (81/52/34 + 16 in unaudited classes). Process: spec pre-keyed to validate-a BEFORE the validator launch (df49c78) — zero drift warnings, the T197 advisory's first prevention (vs T199's mid-flight catch). Cycle-level: 2/2 queued rows landed, queue drained; the operator pushed 6 operator-filed rows (T203 DEPENDENCIES.md audit, T204 laya judge daemon phase 1, T205 HF org hosting) to origin/main mid-cycle → pushes deferred (ba89166, 48bd6c9 unpushed) per the no-mid-cycle-reconcile doctrine — next cycle reconciles FIRST, then triages the operator rows (the F15/F13 roadmap just got its endpoint-side dependencies).

- **T199** (decision-corpus integrity, F13 2a-i) landed `cd0e020` (merge of loop-t199 impl 2d77c83) — the label half of the F13 distillation-readiness pull: `choice` is now a CLOSED set {landed-clean, fixed-up, reverted} enforced at write time for outcome records only (tool error names the set + the `inputs` provenance home, layered after the T88 type legs, never a silent clamp, no write on refusal) + an advisory subject lint (exact-equality id resolution — the same definition the audit applies, `note: subject <id> not found in <path>` on the return text, silent skip on an unreadable corpus, append-only T70 invariant intact) + `scripts/decisions-audit.sh` (five shape-stable sections — class counts, choice violations, unresolved subjects, missing backfills by routing/verdict class, duplicate ids — single jq pass, LC_ALL=C, BSD/GNU clean, ~0.24s live) + `tests/decisions_audit.rs` (7-vio fixture, clean zeros, missing-corpus, dup-ids, malformed-line tolerance, determinism, 10x bound) + README clause. Corpus history immutable: the 7 grandfathered prose-choice records unrewritten; the audit ALSO surfaced 25 grandfathered prose subjects (report-only, advisory — a bigger grandfather set than the eval's "1 misattributed subject" framing). Impl arc: glm child died 80/80 iter-abort mid-work → T63 resume (d1790955031-2) committed 2d77c83, then died at the 50-min wall ~20 min after calling goal_complete (its check ran long; verdict unannounced — the T137 class) → orchestrator-finish (d1790959170-3; one-resume cap burned, T55/T150 precedent). The child's goal-rejection was self-diagnosed: a self-inflicted load flake (its own backgrounded suite under the check = the T82/T152 argv-stub class), no code change. kimi REQUIRED PASS (routing d1790959350-4 — criterion b fails at ~800 actual lines vs the ~260 spec estimate; verdict d1790960580-5): 6/6 mutants CAUGHT (drop-enum, enum-for-all-classes, lint-equality-substring, lint-invert, audit-backfill-join, audit-subject-join — serial, shared files), gates independently re-run (nextest 1457/1457, clippy -D warnings, spec check green), live acceptance independently re-counted with raw jq (7 violations, 0 dups — script and recount agree). Process note: the T197 drift warning fired at validator launch (I had skipped the dispatch-time re-key) → spec check line re-keyed to validate-a mid-flight (8bd6e15, 5dc1fcf precedent) — the T197 advisory caught its first real fumble within one cycle of landing.

### Cycle 92 (2026-10-02, 12:38–14:30 UTC) — glm routine freshness-skip cycle (predicate held: eval fresh same UTC day + 4 todo rows) — 2 landed (T202, T197), T199/T200 deferred (wall)

- **T197** (delegate target-dir drift warning) landed `0649f12` (merge of loop-t197: f7e6a94) — the dispatch re-key fumble class (74d3331/98f6f6b/82804fb) gets a mechanical launch-time check: `delegate launch` compares the three CARGO_TARGET_DIR carriers (spec check-line export / goal export / env map) and appends a `WARN target-dir drift:` block naming every present surface when ≥2 are present and any pair disagrees — advisory-only, never a refusal, launch byte-identical when clean (drift_note empty, pinned), README env bullet integrated. Impl arc: glm child died at the 50-minute wall mid-commit (54/80 — minutes were the binding budget again, the T173 measure clause's 2-of-8 census tick 1) with gates green and work complete-but-uncommitted → orchestrator-finish d1790948048-6 committed f7e6a94 (no T63 resume: a 50-min budget for a 2-iter finish collides with wrap). kimi REQUIRED PASS (routing d1790948332-7 — one mechanical leg flipped: 359 actual lines > ~150; verdict d1790949657-8): always-none mutant 5 legs RED (spec-required ≥2), flip-agreement 7 RED, drop-env-surface exactly the 2 env legs RED, terminator CAUGHT; 2 qualifier mutants SURVIVED (dropping the `check:`-line qualifier / the `export ` prefix) — coverage note for the next eval, not a defect (impl matches spec wording); drift-rehearsal dispatch leg (CHUG_DELEGATE_BIN stub) passes. Gates: nextest 1445/1445 worktree + 1446/1446 main load-split + clippy -D warnings. Wrap lesson: two adjudicated gate-reds were an ORPHANED DETACHED CHECK — the dead impl child's detached `cargo test` kept compiling the full debug tree into target-shared, pegging the host (fseventsd 100%) and tripping the loopd_orphan_reaper deadline class twice; kill-verified (ps read-back) then the quiet-host load-split run went 1446/1446. The orphan problem is the T197 advisory's sibling: a child that dies with a detached check running leaves an unwatched cargo process — candidate row for the next eval (a delegate/loop-level orphan sweep or a check-runner that dies with its parent).

- **T202** (T192 completion) landed `3127b9e` (merge of loop-t202: 1d98253 legs 1-2 + 169fad3 leg 3 + 5dc1fcf dispatch re-key) — the cycle-91 deferred handoff, closed by orchestrator-finish: the resumed child (pid 63649) was found ALIVE 4.5h past its 50-min budget (it had detected a SIGTERM'd inner run and self-relaunched detached) chasing its known-unsatisfiable goal gate (300s bash cap vs the touch-rebuild the check line triggers — 3 failed attempts logged, then a detached-shim workaround whose test HANGS); killed after transcript read-back (routing d1790945038-1), the uncommitted in-flight move (tests/live_ctx.rs integration file + spec tweak) adjudicated DROP (bin-only crate: integration tests cannot see the pub(crate) internals — validator independently reproduced the filed check's "no test target named live_ctx" failure, legitimizing the on-branch check-line amendment to `--bin chug live_ctx`). Legs: (1) genuinely-free-turn budget leg reworked — the carried weak test scripted only REJECTED edits; the rework drives an ACCEPTED edit through the real gate with a two-budget abort-reason discriminator (max_iters=2 convicts a counted turn via iteration-death; token-death at 320k≥300k proves the edit turn was free AND its usage accumulated) — M1/M2/M6 RED-proven; (2) digest gains `- ctx-edit fires: N` immediately after trim fires (adjacency-pinned, M3/M4/M5 RED-proven); (3) eval_digest golden pins (GOLDEN_BLOCK_FIELDS 11→12, real T192 serialization shapes, exact-count + zero-shape-stable legs). kimi REQUIRED PASS (verdict d1790945675-3): 6/6 mutants, nextest 1437/1437, 0 blocking findings — finding 1 adjudicated the spec's false "2 fires" premise with an independent 545-stream census (0 everywhere) and recommended the EVALUATION.md §2.3 correction, applied in the flip commit. Lane call: FULL validation, two mechanical legs flipped from the actual diff (179 lines > ~150; on-branch `check:` line change — the wrap's ~120-line estimate was wrong; the mechanical rule reads the diff). Post-merge: 1437/1437 (one mcp_serve stub-spawn flake re-run green; family untouched by the merge, zero src/ production deltas). Post-merge hygiene: the dispatch-time validate-a re-key reverted to canonical target-shared in the flip commit.

### Cycle 91 (2026-10-02, ~09:00–12:45 UTC) — kimi MANDATORY fresh-eval cycle (queue drained at cycle-90 wrap; T81 routing correct 4-for-4) — 8 rows filed (T195–T202) + the F13 phase-2a roadmap pull; 4 landed (T195, T196, T198, T201 — all doctrine), T202 impl-complete-but-unmerged (recipe on the row), T197/T199/T200 deferred (wall)

- **T195 landed** (merge c634a6c, impl df2c285 — glm 67/80 first-try,
  ~32 min, zero budget deaths). The gate source-touch guard: every
  bounded-gate template running in a non-main checkout against a shared
  role-keyed dir now carries the immediate `touch src/*.rs tests/*.rs;`
  prefix (LOOP-SPEC steps 1/3/4, META-SPEC §5/§7, the guard-floor clause,
  META-META-SPEC's check-line convention sentence); main-dedicated gates
  exempt by T57 construction, impl inner loop exempt — both exemptions
  stated once at the first carrier. Measured grounding (this eval):
  touch forces an 18s release bin rebuild / 29.7s all-targets — the
  guard's per-gate cost against the ~30-min false-red diagnoses it kills
  (cycle-84 precedent) and the silent false-green leg. Pins amended
  T187-style + ONE new structural pin (adjacency-exact, RED-proven).
  kimi REQUIRED PASS (routing d1790935661-17, verdict d1790936624-18):
  gates re-run 1434/1434 + clippy, 7 serial mutants ALL RED-as-expected
  (incl. the spec-mandated M1 + both structural-pin legs), 0 survivors,
  4 minor non-blocking observations. Post-merge main gates 1434/1434
  under target-shared-main. The arc dogfooded its own doctrine: the
  review gate and the validator both ran the touch prefix.

- **T196 + T198 + T201 landed (T45 doctrine bundle)** (merge 1136ade;
  per-row commits d4229bf / 96ccd9f / ca82fd9 in queue order). T196: the
  step-2 impl goal template bans tree-wide formatters (one woven
  sentence + exactly-once pin, RED-proven). T198: loopd cycle budget
  `--max-minutes 240→360` with the walls arithmetic in-comment (360 =
  p95 × 1.5; iters stay 200) + the argv pin amended in-commit. T201:
  validator verdict-first — VERDICT + findings land in
  `.chug/verdict.md` at decision time (META-SPEC §6) and step 4 reads
  it before spending a T63 resume (LOOP-SPEC) — the deviation
  ("heredoc" → "one bash command") verified justified by the T126
  exactly-once pin. ONE kimi round covered the bundle (routing
  d1790939775-22, verdict d1790940890-23): PASS first round, 5/5
  mutants killed across two T79 waves (per-row deletions + needle
  corruption), 1 non-blocking observation (loopd.sh:314's stale
  171/240 comment — comment-hygiene candidate). Arc: impl died 80/80
  with t201 written-uncommitted → ONE T63 resume (routing
  d1790939069-21) finished in 10 iters / ~9.5 min. DISCLOSED INCIDENT:
  the impl's first t196 commit heredoc mangled and the shell EXECUTED
  the prose's backticked `cargo fmt` — a tree-wide reformat (89 files)
  the child stripped T63-style before recommitting; the row's own
  incident class via a different vector (heredoc-execution, not
  intent) — noted for the next eval. The validator DOGFOODED T201:
  its verdict.md was written at decision time and harvested
  (verdict-t196-bundle-20261002.md). Post-merge main gates 1436/1436.

- **T202 deferred at the wrap wall** (recipe on the row). Full impl
  landed in `loop-t202` as two commits (1d98253 legs 1–2, 169fad3 leg
  3) via an 80/80 death + ONE T63 resume; the resume's first
  goal_complete was REJECTED — the child's 300s bash cap kills the
  T195 touch-forced all-targets rebuild mid-check (the goal GATE's
  1200s would pass; a new friction interaction for the next eval:
  children observing a touch-guarded check end-to-end need the split
  build-then-test run). At the wrap wall with the resume still in
  flight, the arc hands off per T28/T19 (routing d1790944041-27).
  SPEC-ACCURACY MISS (mine): the filed acceptance claimed "ctx-edit
  fires: 2" on the cycle-89 stream — re-measured at wrap: ZERO
  `ctx_edit` events in every delta stream (the emission site
  driver.rs:1651 is live; no accepted live-ctx edit occurred in the
  sample). My eval misread a combined grep output; the acceptance
  evidence becomes 0-on-delta-streams + fixture-pinned rendering,
  adjudicated at next cycle's review.
- **T197, T199, T200 deferred — unworked** (wall-clock; T197's
  ~70–90-min arc vs ~75 min left after the bundle was a coin flip
  that risked a mid-validation orchestrator death, and dispatching it
  would ALSO have trapped T202's serial merge behind it — the
  cycle-88 T187-deferral precedent; T199/T200 sequence behind T197 —
  all three specs are dispatch-ready: worktree + warm build + glm
  80/50 + gates + the lane call from the diff).
- **Cycle-level notes.** 4 rows landed (T195 + the T196/T198/T201
  bundle), all doctrine, 2 kimi rounds both PASS-first-round
  (12/12 mutants killed, 0 survivors, 2 justified deviations).
  Validator census: 2/2 announced verdicts — and T201 was dogfooded
  in-cycle (the bundle validator wrote verdict.md at decision time).
  Impl budget deaths: 2 of 3 impl arcs (bundle 80/80, t202 80/80) —
  both recovered via ONE T63 resume each (the T63 machinery held).
  The T195 review gate + both validators ran the touch prefix
  in-cycle (adoption immediate). The disclosed fmt-execution incident
  (bundle impl's mangled heredoc EXECUTED the prose's backticked
  `cargo fmt`, 89 files, stripped pre-commit) is the T188 class via
  the heredoc-execution vector — the ban covers intent, not shell
  mangling; flagged for the next eval (commit-message quoting
  discipline). Release trigger check at wrap: 4 items since v0.13.0
  (T195, T196, T198, T201) ≥3 → patch bump (no feature check-off:
  all doctrine) → v0.13.1 pending final gates.

### Cycle 90 (2026-10-02, 08:06–09:00 UTC) — glm routine freshness-skip cycle (freshness held: EVALUATION.md 03:41Z same UTC day + 1 todo row T187) — 1 landed (T187), QUEUE DRAINED

- **T187 landed** (merge ae9ea36, impl d3d4da5). The 3x-deferred doctrine row landed first-try clean: META-SPEC.md's bounded-gates (T6) hard rule re-keys the illustrative caps from the dead form (`alarm 600` / `timeout 600` — under any bash cap below 600s the cap's process-group SIGKILL lands before the inner alarm can fire) to `alarm 280` / `timeout 280`, matching T178's LOOP-SPEC templates verbatim, and gains the rule in one sentence: the inner bound must sit BELOW the driver's bash cap (280 under the fleet's CHUG_BASH_TIMEOUT=300, ≤ ~110 under the 120s default). Both pins that ASSERTED the 600 form were amended in-commit with the justification named (nextest_gate_runner::bounded_caps_wrap_the_nextest_form, shared_target_dir::meta_spec_release_carriers_are_pinned_per_carrier — the pin follows the re-keyed carrier, T178's LOOP-SPEC half already pinned 280). kimi REQUIRED PASS (23/60 iters, ~10 min): gates independently re-run (spec check line green incl. both greps, clippy -D warnings, nextest --release 1433/1433 under the doctrine's own alarm-280 wrap), 4 mutants serial in-tree (overlap declared — all touch the same META-SPEC paragraph): M1 alarm-600 restore KILLED (both pins + both greps), M2 timeout-600 restore KILLED (both pins; grep legs alone blind — the check line's timeout leg is covered by the cargo-test leg, observation), M3 rule-sentence drop SURVIVED (observation — the sentence is unpinned; the pins protect the cap VALUES, not the rule statement), M4 alarm-300 corrupt-value KILLED. Tree byte-clean post-mutants. Child economy: impl 21/80 in ~20 min, validator 23/60 in ~10 min — zero budget deaths, zero resumes; three deferrals (cycle-82 handoff, cycle-83 eval, cycle-89 wrap) were pure queue position, not difficulty. Post-merge gates 1433/1433 in target-shared-main + clippy clean.

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

### Cycle 83 (2026-10-01) — kimi mandatory fresh-eval cycle (queue drained at cycle-82 wrap 7646250; T81 routing per the cycle-82 handoff) — 4 rows filed (T180–T183 + F14), 2 landed (T180, T182), T183 MID-ARC at wrap (recovery recipe below), T181 unworked — no tag (2 items < 3 since v0.10.1)

### Cycle 82 (2026-10-01) — glm routine freshness-skip cycle (freshness held: cycle-81 wrap note c4d778c landed 03:39 UTC) — Phase 1 skipped per skip-rule record d1790826770-1 — 2 landed (T179, T178), QUEUE DRAINED, release v0.10.1 tagged

### Cycle 81 (2026-10-01) — kimi routine freshness-skip cycle — 2 landed (T175, T176); T178 deferred to the next cycle (4-minute wrap wall hit first — its spec is cold-start-ready)

### Cycle 80 (2026-09-30) — glm routine freshness-skip cycle — 4 landed (T177 af4f4b4, T170 88119dc, T174 ab421d3, T171 7b9b2bf — F11 phase 1b CLOSED), doctrine bundle T175+T176+T178 deferred with reason — full narrative in git (row-flip commits + todo: flips).

### Cycle 79 (2026-09-30) — fresh eval (kimi; 8e832d2: 10 rows + 16 triage records) — 3 landed (T173 minutes-budgets 553928a, T172 load-lock deflake bae88d5, T169 F11-1b-i pull 3796983), 7 deferred with reason (minutes binding; iteration-ceiling deaths the new census class; T63 resume textbook; kill-rule confirmed x4) — one-line backfill at the cycle-84 wrap (entry absent since an earlier wrap's compaction; full text in git 8b4db7d and the todo: commits).

### Cycle 67 (2026-09-28/29) — routine (freshness-skip) — codex-intake pri-2 queue — T135 LANDED 611916e (merge + stale-flip cleanup), T136 arc mid-flight at wrap (recovered + landed cycle 68; its crash-safety class completed by T145 in cycle 70) — compacted at the cycle-73 wrap (last-6 rule; the full text lived in git until the cycle-72 eval commit dropped it — restored verbatim for 69/70/71 below, 67/66 re-compacted from git).

### Cycle 68 (2026-09-29) — routine glm freshness-skip (queue non-empty, eval fresh) — codex-intake pri-2 queue (T137 merge pickup + T138 resume + T139)

### Cycle 71 (2026-09-29) — routine glm freshness-skip (queue non-empty, eval fresh) — ALL FOUR landed (T147+T148+T149+T150), queue EMPTY

### Cycle 70 (2026-09-29) — kimi fresh-eval cycle (queue was empty) — 7 rows filed, T144 landed

### Cycle 69 (2026-09-29) — routine glm freshness-skip (queue non-empty: T129/T131 deferred from cycle 68, eval fresh) — the deferrals worked first

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

### Cycle 73 (2026-09-29) — resumed mid-arc (prior segment truncated at wrap); origin reconcile + T152 landed

### Cycle 74 (2026-09-30) — freshness-skip routine cycle; T161 (the operator's parallelization lever) landed first

### Cycle 75 (2026-09-30) — freshness-skip routine cycle; the queue drained: T160 landed, zero rows remain

### Cycle 76 (2026-09-30) — MANDATORY fresh eval (queue empty at cycle-75 wrap; kimi, loopd-routed) — 7 rows filed (T162–T168); T163 landed; cycle in progress

### Cycle 77 (2026-09-30) — freshness-skip routine cycle (EVALUATION.md fresh, 5 todo rows); T162 landed; cycle in progress

### Cycle 78 (2026-09-30) — freshness-skip routine cycle (eval fresh 09:17); BOTH queued rows landed (T167, T168), queue drained

### Cycle 79 (2026-09-30) — MANDATORY fresh eval (queue drained at cycle-78 wrap; kimi, loopd-routed) — 10 rows filed (T169–T178); 3 landed (T173, T172, T169 + the F11 1b-i roadmap pull); 7 deferred with reason

