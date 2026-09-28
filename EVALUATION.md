# EVALUATION — chug, assessed by chug-loop (2026-09-28, cycle 64)

**MANDATORY fresh eval** — the queue is EMPTY again (T1–T123 all done
with refs; cycle 63 landed the last row), so the freshness predicate's
queue half fails and loopd routed an eval cycle (`todo_rows=0
eval_fresh=yes -> eval cycle on kimi-k3`, loopd.log 15:51:30Z). The
cycle-63 carry executed first, per its own wrap addendum: origin's
operator commit (06d23d1 install.sh banner) rebased under the three
local commits conflict-free (disjoint file sets — install.sh vs
site-sync/TODO/EVALUATION), targeted nextest 31/31 green
(install_sh + site_sync) on the combined tree, pushed
(06d23d1..0300cb4). The headline of this eval: **the release pipeline
went live operator-side between evals (v0.2.1 published green with
all six assets), and the README still says no release exists** —
front-door staleness (T127) — while the era's one real child death
(T117 mid-impl at 80/80) traces to ESTIMATE undershoot, not the T110
ceiling value (T125).

## 1. What chug does well — be brief

- **T121's raise is already boring** (the good kind): cycle 63 ran
  49/200 iterations, goal accepted first try, 51 min wall — routine
  cycles sit far from the new ceiling; this cycle is the first
  eval-shaped test of 200 (the cycle-61/62 shape — eval + items —
  died at 160 twice).
- **FOUR consecutive all-PASS validation cycles** (58, 61, 62, 63):
  cycle 63's kimi round on T122 PASSed 33/50 with 4/4 mutants killed,
  zero blocking findings — adversarial validation as a gate is calm,
  and the T79 parallel-mutant batches are routine validator practice.
- **glm orchestrated a full routine cycle cleanly** (cycle 63): the
  T122 arc end-to-end — launch, terminal long-polls, review,
  EXERCISED optional validation, harvest, row flip with refs,
  Outcomes entry, five decision records, goal accepted. The T81
  two-model routing works in both directions.
- **The release pipeline is LIVE**: operator bootstrap
  v0.1.0 → v0.2.0 → v0.2.1 in one afternoon; two workflow failures
  (aarch64 cross libc, `--locked` Cargo.lock desync) diagnosed and
  fixed operator-side in minutes (3cd7b7e, 9ce7565+1e84357); run
  36438268858 green, six assets published. T100's wrap-time tag
  doctrine is wired (80b365c) — this is the first wrap operating
  under an active tag trigger.
- **T122's pure-awk Fliegel–Van Flandern JDN** day-key is
  TZ-independent with zero new dependencies; the kimi validator
  independently re-derived it against Python including leap/century
  edges. Craft-level work from both families.

## 2. Incidents worth fixing

### I1 — estimate undershoot killed a child mid-impl (T117 280→603) → T125

The T110 filing-time ceiling (~500 lines) only works when estimates
are honest, and the era's landed actuals show SYSTEMATIC undershoot
on feature rows, where test+doc density multiplies the src diff
(cycle-61/62 Outcomes records): T113 ~455→+583 (1.3x), T115
~130→+398 (3.0x), T116 ~30→+116 (3.9x), **T117 ~280→+603 (2.2x) —
whose glm impl child DIED mid-impl at 80/80** (uncommitted, t1 of 6
done; events-t117-impl-20260928-132236.jsonl), costing T63 resume
#25. The true size was over the ceiling the estimate claimed to be
under; an accurate filing estimate would have split the row.
Cycle-62's eval absorbed the overshoots as "calibration, not a
defect" — the t117 death converts it: a mid-impl budget death is
exactly the cost the ceiling exists to prevent, and the T110 measure
clause's premise (deaths at ≤500 estimates = ceiling failure) has
now fired with the ceiling itself innocent. Filed as **T125**
(doctrine, pri 3): estimates count ALL changed lines with the four
named data points; a novel-logic row estimated over ~400 SHOULD
split (the hard ceiling stays ~500; the band absorbs the observed
undershoot); each eval re-calibrates against landed actuals in the
eval text, never by editing the threshold in passing; pin legs
beside T114's i-j / T120's k-l in tests/loop_spec_recovery.rs
(~45 lines; runs ALONE; kimi REQUIRED — loop doctrine).

### I2 — validators keep rediscovering the /tmp write sandbox (10 fires) → T126

TEN `path escapes cwd … cross-tree paths go through bash` tool errors
across three cycle-61/62 validators, every one the same shape —
writing the mutation helper script to /tmp with write_file/edit_file:
t112-validate ×7 (`/tmp/t112-mutate-*.py`, `/tmp/t112-leg.sh`),
t113-validate ×2 (`/tmp/chug-mut-apply.py`,
`/tmp/chug-mut-t113-run.sh`), t115-validate ×1
(`/tmp/t115-mut-leg.sh`). T79's parallel-mutant legs — routine since
cycle 61 — multiply exactly this write (3 legs ≈ 3 apply/run script
pairs per validator), so the class is structurally growing. The
cycle-62 eval weighed it at 6 fires and rejected ("working as
designed; the error names the remedy") — the T79-routinization datum
postdates that rejection and supersedes it (not ignores it). Each
fire still costs only ~1 validator iteration, and the fix is one
sentence in the META-SPEC §6 goal template every validator already
reads, adjacent to the existing cross-tree-READS sentence (which
covers reads only). Filed as **T126** (doctrine, pri 4): the write
half — /tmp helper scripts go through bash heredocs — plus the first
META-SPEC pin leg (new `meta_spec()` loader in
tests/loop_spec_recovery.rs) (~25 lines; runs ALONE; kimi REQUIRED).

### I3 — README Install: "no release published yet" is now FALSE → T127

README.md:16-17 still warns "Until the first tag is cut there is **no
release published yet** — the one-liner and tarball links below 404
until then". True through the cycle-62 audit (tags landed 14:38Z,
after that eval's filing); FALSE since the operator's bootstrap:
**v0.2.1 published green** (run 36438268858, six assets —
macos-arm64/linux-x86_64/linux-aarch64 tarballs + sha256s) and
`https://chug.sh/install.sh` serves (verified this eval: redirect
stub to the canonical installer). The worst seat in the README for a
stale claim — §6(c)'s superseded-behavior class, at the front door,
steering new users away from the working path. Filed as **T127**
(docs, pri 2): delete the stale sentence pair, versionless durable
phrasing (no version literal to re-stale), Gatekeeper + from-source
byte-identical; docs-only T80-floor gates; ~8 lines. (v0.1.1 note:
`gh run list` shows a 22s failed run on a v0.1.1 tag push that left
no release and no remote tag — an unpublished operator tag outside
T100's immutability scope; recorded here so the next eval doesn't
chase it.)

### I4 — cycle 62's goal rejection was the operator's ungated v0.2.0 bump (assessed, REJECTED)

Cycle 62's goal gate failed `cargo test` at 14:43:55Z and the cycle
died at 160/160 before re-attempting. The failure was NOT cycle-62
work: the operator's v0.2.0 release bump (aa5ed2e, pushed ungated —
wrap gates ran pre-bump) broke three hard-coded `0.1.0` build_info
banner expectations (9ce7565's commit message names the class). The
structural fixes landed operator-side within the hour: 9ce7565
(banner pins now DERIVE from the VERSION const — self-healing across
bumps), 1e84357 (lock-sync doctrine + v0.2.1), 3cd7b7e (CI cross
libc). The T123-carried "post-bump targeted gate" candidate is
**weighed and REJECTED**: both failure legs got structural fixes
(self-healing pins + lock-sync doctrine + `check-tag-version.sh`
pairing); the residual — a FUTURE version-coupled test added without
the self-healing pattern — is speculative; a third bump-failure class
re-files it. eval-triage record filed.

### I5 — validator ceiling zone: seventh consecutive zone-free cycle (watch)

Cycle-62/63 validators: t117 44/50 (budget_low fired INSIDE the T18
margin), t118 42/50, t120 38/50, t121 14/50, t122 33/50 — zero
deaths, zero resumes. The T32 sizing (50) holds; the 50→65 raise
stays rejected for the sixth+seventh cycles.

### I6 — post-commit death class: silent (fourth assessment, rejection stands)

Cycle 62–63: one mid-impl death (t117 — the I1 estimate class, a
different mechanism) and zero post-commit deaths. T63's resume
remains the answer when the class fires; the rejection stands.

## 3. Friction hot spots — fix assessment

- **decision_log schema fumbles (T88)**: ~18 events files era-wide
  now carry `invalid decision_log call` (the cycle-62 count of 12
  plus t115 `options`, t116 `options`, t119 `choice`, t120-validate
  `confidence`-as-string, cycle-63 orchestrator `options`,
  t100-fixup `options`). All self-corrected within one iteration,
  zero lost records (356 landed). REJECTED again with the updated
  count — the confidence-as-string leg is the same
  fresh-context-schema class; the per-fumble cost stays ≤1 iteration,
  below the filing bar.
- **120s bash-cap kills**: ~7 fires across the t112–t118
  impl/validate streams (unbounded cargo invocations against the
  shared cache; all self-corrected with bounded retries). Baseline
  rate; the T22-era tool-description remedy stands; watch only.
- **digest render nuance (assessed, NOT filed)**: eval-digest.sh
  renders budget_low fires as "first at remaining_iters=N" even when
  the MINUTES side tripped (t113-impl: remaining_iters=22,
  remaining_secs=177 — WARN_REMAINING_SECS=300 fired). The event
  carries both fields; the render is a cosmetic mislabel that could
  send a future evaluator chasing a phantom latch bug. Below bar;
  named here as the correction.
- **edit_file `old` not found**: baseline (orchestrators ×3,
  children ×5 era-wide across TODO.md/spec/src targets) — the T5
  disambiguation surface exists; no new class.
- **stream_fallback organic fires**: ZERO event-typed fires era-wide
  (`"type":"stream_fallback"` over all events files; the 14 raw
  string hits are payload quotes inside goals/specs). The endpoint
  serves SSE; watch stands.
- **Anti-sprint-burn guard**: cycle-62/63 orchestrators carried all
  waits as terminal long-polls (35 and 9 delegate calls; no
  idle-iteration runs). Working.
- **T122's five non-blocking validator findings** (carried from the
  cycle-63 verdict): impossible-date rollover in day_key (cosmetic
  ordering on a 2026-02-30-class input — no user surface), no
  absolute-epoch unit pin (test hygiene — the child verified vs
  `date -u` incl leap/century and kimi re-derived independently), one
  comment overreach (nit), in-day placement rationale (design note —
  the day-claim-vs-second-claim ordering is deliberate and pinned),
  redundant seq tiebreak (harmless belt-and-braces). ALL rejected
  individually; eval-triage records filed.
- **T113's mid-turn queue-vs-steering asymmetry** (carried watch):
  zero new evidence in the delta corpus; the watch stands.

## 4. Capability gaps — ROADMAP PULL: F10 phase 1 → T124, SPLIT

Tier 1's deferrals stand (F13/F2/F3/F4/F5 — layad endpoint re-verified
ABSENT this eval: `curl http://127.0.0.1:8420/` → 000). Tier 2's
stand (F6/F7/F8 phase 2s — F8-p1 adoption is now the children's
default step-tracker, zero organic demand for p2's remove/deps/
chat-command scope; the gate stays closed on evidence). F9 CLOSED
cycle 62 (phases 1/2a/2b landed). The top unworked roadmap item is
therefore **F10 — chug as MCP server** (Tier 3 head). SPLIT per the
working-rules shrink clause (combined phase-1+2 estimate ~750+, over
the T110 ceiling): **phase 1 → T124** (feature, pri 2): `chug
mcp-serve` — the stdio JSON-RPC 2.0 server skeleton (ndjson framing
matching the client's own shape, initialize/ping/tools-list/
tools-call, the -32700/-32600/-32601/-32602 taxonomy, stdout purity,
EOF lifecycle) plus ONE read-only tool `chug_status` (events summary
of any chug cwd, built on delegate.rs's `read_events`/
`summarize_events` seams with a compact additive renderer —
`render_status` byte-identical). Read-only by design: no spawning, no
writes. **Phase 2 deferred** (`chug_collect` + `chug_launch`, the
write leg — the permissions/hooks interaction of a remote-spawned run
must be thought through, and the leg rides phase 1's skeleton:
dependency, the written reason), **phase 3 deferred** (server log
file, listChanged notifications, cancellation, resources). ~485
lines, under the ceiling BY the split; kimi routing decision at work
time (not on the REQUIRED list — new module + main.rs + delegate.rs
visibility, no driver/api/tools/events core).

Beyond the pull: **no new roadmap appends this eval.** The delta
corpus surfaced no credible new capability gap; F11 (MCP
resources+prompts consume) and F12 (web search) stand as ordered —
F11 gains adjacency value once F10's server side exists (chug will
then speak both MCP directions), noted for the next eval's ordering
consideration, not filed.

## 5. Top 3 priorities

1. **T127** (docs, pri 2) — a falsehood at the front door while the
   release is live and green; ~8 lines, clears in one short arc;
   bug-class staleness (docs correctness at the install path).
2. **T125** (doctrine, pri 3) — the mid-impl-death preventive; tiny,
   runs alone, and every future eval's filing quality depends on it.
3. **T124** (feature, pri 2) — the mandatory roadmap pull and the
   fleet primitive's first leg; the biggest row, sized under the
   ceiling by the split.

(T126 pri 4 fills the tail — cheap doctrine; an unworked carry is a
fine outcome this cycle.)

## 6. README audit (usability)

(a) **Reading order**: correct — Install → Quickstart → chat (+
packs) → run → forks → plan → TUI → Tools → risk gate → hooks →
permissions → MCP → Langfuse → specs → loopd → Development; the
newcomer path reads top-down. (b) **Redundancy**: the delegate
launch/status/collect bullets are dense but single-sourced (checked
against the tool schemas); no drift. (c) **Staleness**: FOUND →
**T127** (§2 I3 — the first non-clean audit after five consecutive
clean ones; the audit's value is precisely catching the environment
moving under a true claim). (d) **Balance**: fine — hooks and
permissions carry reference-grade detail that IS the config docs for
those surfaces; the MCP section correctly scopes client-side only
(T124 adds the server subsection at merge). (e) **Quickstart truth**:
`chug.sh/install.sh` serves the redirect stub (curl-verified); the
run/chat/plan commands work as written; the plan-mode "Not yet" list
(`/plan` chat command, `--approve`, web_fetch-in-plan) remains
accurate. **One docs row filed (T127); no structural debt.**

## Handoff

- **Work order** (with reasons): **T127** (docs-staleness, pri 2 —
  smallest, front-door) → **T125** (doctrine, pri 3 — runs ALONE) →
  **T124** (feature, pri 2 — the mandatory pull) → **T126**
  (doctrine, pri 4 — runs ALONE; the declared deferrable tail).
  Bundle check (T45 conjunctive): T125's ~45 estimate EXCEEDS (a)'s
  ≤30 → no doctrine bundle; T124 is a feature (never bundled); T127
  is alone in its file area — NO bundles. Overlap check (T44): T125
  and T126 are doctrine — NEVER overlap, each runs alone; T127
  (README.md) and T124 (src + tests + README.md) SHARE README.md →
  serial; effectively a strictly serial queue this cycle.
- **SELF-SPEC**: none. **Human items**: (1) `gh auth refresh -s
  workflow` remains the proper fix for the workflow-scope class
  (carried; the SSH pushurl workaround held again this era). (2)
  `com.tampajohn.chug-loopd.plist` stays untracked — operator's
  launchd unit (carried).
- **Watch items carried**: loopd cap (T121 landed and boring — watch
  resets to: a ≥180-iteration cycle or another iteration-death
  re-opens); validator ceiling zone (I5 — seventh zone-free cycle);
  display-artifact watch (ZERO payload-level garbles this era
  post-T115/T120 — the verify-then-kill SEQUENTIAL rule was not
  exercised, which is the outcome it wants); compressed-verdict
  harvest fidelity (zero new losses; the re-file trigger is a SECOND
  loss); module sizes (api.rs 2,921 = 65% of the ~4,500 trip line,
  mcp_http.rs 2,793 = 62%, tgrep.rs 2,531, tui.rs 2,527 — watch
  only); stream_fallback organic-fire watch (zero, jq-verified);
  T113 mid-turn queue-vs-steering asymmetry (zero new evidence);
  T110 ceiling measure (SUPERSEDED by T125 — the measure fired with
  the ceiling innocent; the remedy moves from ceiling-value to
  estimate-calibration); F13/F2/F3/F4/F5/F6/F7/F8 phase-2 deferrals
  (layad absent re-verified; F10 phases 2–3 join the deferred set
  with the phase-1 dependency named).
- **Weighed and REJECTED this eval** (eval-triage records in
  `.chug/decisions.jsonl`): post-bump targeted release gate (I4 —
  structural fixes landed: self-healing pins 9ce7565 + lock-sync
  doctrine 1e84357 + check-tag-version.sh pairing; a third
  bump-failure class re-files); decision_log schema-fumble row
  (§3 — ~18 files, ≤1-iteration self-corrections, zero lost
  records); validator budget 50→65 (I5 — seventh zone-free cycle);
  post-commit-death-class row (I6 — silent; T63 covers); T122's five
  non-blockings (§3 — each rejected individually with reasons);
  digest budget_low render mislabel (§3 — cosmetic, named as the
  correction); /tmp sandbox move (I2 — the boundary stays; T126
  teaches the remedy instead); F10 filed as ONE row (~750+ estimate —
  split per the working-rules shrink clause; the split IS the
  resolution); new roadmap appends beyond the F10 pull (§4 — none
  credible in the delta corpus; F11 adjacency noted, not filed).


## Outcomes (filled at cycle wrap — LOOP-SPEC Phase 3)

### Cycle 64 (2026-09-28) — fresh eval (kimi, loopd-routed: queue empty) — T124–T127 filed — IN PROGRESS

**T126** (META-SPEC §6 validator /tmp-heredoc doctrine, pri 4):
landed eef7a29 (fast-forward) — §6's validator goal template gains
the cross-tree WRITE half immediately after the byte-identical READ
sentence: "Write /tmp helper scripts (the mutant apply/run legs) with
bash heredocs — write_file and edit_file are cwd-confined the same way
and refuse /tmp paths with `path escapes cwd`." The first META-SPEC
pin: `meta_spec()` loader + leg (o) (needles exactly-once, windowed
inside §6, read-sentence byte-identical guard + adjacency guard, T48
self-checks). glm impl 35/80 first-try, RED-proven. kimi REQUIRED
**PASS** 16/50: independent gates (build/clippy/nextest 996/996,
loop_spec_recovery 15/15) + 5 parallel mutants ALL killed on distinct
assert paths — M1's RED-proof failed ONLY leg (o) of 15 (specificity),
M2 duplicate-count, M3 read-sentence guard, M4 relocation window, M5
adjacency break. Zero blocking findings. Estimate ~25 → actual +192
(7.7x — the pin-density pattern; validator logged it as T125
calibration data). Review + post-merge nextest 996/996. Both streams
+ validator ledger harvested.

**T124** (F10 phase 1 — `chug mcp-serve` + `chug_status`, feature,
pri 2, THE ROADMAP PULL): landed d6264be (rebased a4e1bea onto main
post-T125, fast-forward) — the fleet primitive's first leg. A stdio
JSON-RPC 2.0 server: newline framing matching the client's own shape,
initialize (PROTOCOL_VERSION `2025-06-18` reused `pub(crate)` from
mcp.rs, capabilities.tools, serverInfo), ping, tools/list, tools/call,
notifications never answered, the -32700/-32600/-32601/-32602
taxonomy with errors that never kill the loop, EOF exit 0, and stdout
purity BY CONSTRUCTION (single `writeln!` protocol writer, grep-pinned
by the `stdout_purity_module_has_no_stdout_writers` leg). One
read-only tool `chug_status`: fail-fast cwd validation (absolute /
exists / has `.chug/`, each naming the received path verbatim) over
the delegate `read_events`/`summarize_events` seams (visibility-only
`pub(crate)`; `render_status` byte-identical) with a compact additive
renderer. 28 bin-internal legs + 2 deadline-bounded e2e spawn legs
(full client conversation with a notification-silence probe; errors
keep the loop alive over the real wire). glm impl 78/80 first-try
(budget_low inside the T18 margin). kimi optional-EXERCISED **PASS**
37/50: static review clean (delegate.rs line-by-line visibility-only;
dispatch bypasses banner/ledger/lock/events), independent gates
(build/clippy/nextest 993/993), manual wire smoke, and 6 mutants in 2
T79 parallel batches — M1/M2/M4/M5/M6 caught by exactly their targeted
tests; **M3 survived**: the `.chug/`-existence check can be deleted
undetected because the missing-.chug-dir leg's assertions are
substring-satisfied by the downstream events-unreadable message (one
low-severity weak-test finding, carried to next-eval triage — the
T119-class pin-strengthening pattern). Review + post-merge nextest
995/995. Estimate ~485 → actual +1106 (2.3x, tests ≈ 65% of the diff)
— **the T125 ~400 band's first proving case, landed hours after the
band did**: under the new rule this row (est ~485 > ~400) would have
been split at filing; its true size was over even the 500 ceiling.
T125's recalibration counter: T127 bullseye (+8/-8), T125 itself 3.5x
(+159, all pins), T124 2.3x. FEATURES.md F10 annotated phase-1 LANDED;
phases 2–3 deferred (phase 2 now READY to file next eval). Both
streams + validator ledger harvested. Orchestrator note: the T120
verify-then-kill rule was exercised twice this arc on suspected
goal/edit garbles — both read-backs proved the payloads INTACT
(render artifacts in the orchestrator's own context view), no kills,
no rework — the rule paying for itself.

**T127** (README Install staleness, docs, pri 2): landed 5dbab0d
(fast-forward) — the pre-release honesty pair deleted, replaced with
versionless durable claims (releases cut on `v*` tags and published;
three platform tarballs + sha256s `--release --locked` from the tagged
commit; one-liner installs latest; tarball links ride GitHub's
`releases/latest` redirect). glm impl 13/80 first-try green; the child
live-verified chug.sh/install.sh + the latest-tarball 302→200 + its
sha256 at edit time. kimi SKIPPED per docs-only precedent (routing
d1790612463-18). Review + post-merge floor: todo_consistency 5/5 +
readme_layout 1/1. Estimate ~8 → actual +8/-8 EXACT — the calibration
era's first bullseye (T125's counter starts here). Impl stream
harvested (7.7 KB).

**T125** (estimate-calibration doctrine, pri 3): landed 532c403
(fast-forward) — META-META-SPEC's spec bar gains the calibration rule
immediately after the T110 ceiling sentence: estimates count ALL
changed lines (src + tests + docs), feature-row test/doc density
~1.5–3x with the four named data points (T113/T115/T116/T117, the
last's mid-impl 80/80 death as the cost), a novel-logic row estimated
over ~400 SHOULD split under the unchanged ~500 hard ceiling and
unchanged move-row exemption, and each eval re-calibrates from the
Outcomes records instead of editing the threshold in passing. Legs
(m)–(n) pin the three tokens exactly-once + windowed placement with
T48 needle self-checks. glm impl 37/80 first-try, RED-proven against
pre-edit doctrine. kimi REQUIRED **PASS** 21/50: independent gates
(14/14, clippy, nextest 966/966) + R1 RED-proof + M1–M4 mutants
(corrupted ~400/T117/en-dash each kill (m); reorder kills (n) while
(m) stays green); tree byte-clean. What the validator caught: no
defects — and it logged the row's OWN estimate undershoot (~45 →
+159, 3.5x, all in the pins) as the rule's first re-calibration data
point. Review + post-merge nextest 966/966. Both streams + validator
ledger harvested (a validator mid-worktree launch rotates the impl
stream aside — the harvest reads the rotated file, verified by
run_start model + goal_sha256 prefix).

### Cycle 63 (2026-09-28) — routine glm freshness-skip — T122 LANDED (93dae89 fast-forward)

**T122** (site-sync timeline curated sort, pri 2, user report): T101's "+inf for undatable" clause pinned ref-less curated entries BELOW newer generated entries — the live 09-27 K7/chug.sh milestones rendered after 09-28 items. Fix: key precedence per entry = commit %ct (ref resolves, unchanged) → the entry's own `tl-date` parsed at day precision (`day_key` = that day's 23:59:59Z via Fliegel–Van Flandern civil→JDN arithmetic in pure awk, TZ-independent, no date(1) dialects) → +inf sentinel only when NEITHER a ref NOR a parseable tl-date exists; same-day ties keep original region position (SEQ). Day-precision entries sort INSIDE their day (after same-day exact-%ct entries — a day claim cannot beat a second claim — before the next day's); T101's guarantees untouched (%ct primary, curated-text-wins merge, 20+collapse cap, cat-file audit). glm impl 38/80 first-try RED-proven (both new fixture tests + the flipped T99 bootstrap leg). kimi EXERCISED PASS 33/50: 4/4 mutants killed zero survivors (fallback-drop / precedence-flip / start-of-day / sentinel-zero), serial declared overlap, day_key arithmetic independently re-verified vs Python incl leap/epoch, tree byte-clean sha-match, 5 non-blocking informational findings (impossible-date rollover in day_key, no absolute-epoch unit pin, one comment overreach, in-day placement rationale, redundant seq tiebreak) — carried to next eval, none blocking. Review + post-merge nextest 964/964 under target-shared-main. Routing d1790609328-1 (optional→exercised: SECOND ordering bug in timeline_generate, user-visible on the live site), verdict d1790610212-2, outcome landed-clean ×2. 2 streams harvested (impl + validate).

**Cycle-level notes (wrap).** (1) loopd routed glm routine at 14:58:38Z (`todo_rows=1 eval_fresh=yes`) after cycle-62's kimi run died 14:53:09Z post-wrap without goal_complete (consecutive failures: 2 — its release-saga tail is recorded in T123's row + the cycle-62 notes; this cycle started clean from 5a9e099). Phase 1 skipped per the freshness predicate; glm never evaluates (T81). (2) Single-child cycle: T122 solo, no T44 overlap, no T63 resume, zero budget deaths (impl 38/80, validator 33/50 — both inside T18 margin). (3) Queue now EMPTY again → next cycle eval-routes kimi (fresh-eval); the carried post-bump targeted gate (T123's notes) + T122's 5 non-blocking findings are its input. (4) Anti-sprint-burn guard never tripped (dispatch → collect → merge → wrap, ~20 orchestrator iterations). (5) Release trigger NOT tripped: 1 item landed since v0.2.1, no FEATURES check-off — no tag this wrap. (6) PUSH DIVERGENCE at wrap (unresolved by doctrine): `git push` rejected non-fast-forward — origin gained operator commit 06d23d1 (install.sh banner glyph fix, 2 lines) while this cycle ran; local holds 93dae89 + 740f453 (+ this addendum), file sets fully DISJOINT from 06d23d1 (scripts/tests/TODO/EVALUATION vs install.sh only). Per the no-force-push / no-mid-cycle-reconcile rule (T97 precedent, cycle 54) the divergence is NOTED, not reconciled: next cycle's first act is fetch + rebase the local chain onto origin/main (conflict-free) + push, then Phase 1 (queue is EMPTY after T122 → eval-routes kimi; cycle-57 precedent for reconcile-then-route).

### Cycle 62 (2026-09-28) — ALL FIVE landed clean (T121 cbc21e0 loopd 160→200, T117 3579d9d F9-p2a run-side expansion, T119 43f427d weak-test pins, T120 c173c2f kill/sed doctrine, T118 4c96414 F9-p2b frontmatter+completion — **F9 phase 2 CLOSED**) — the era's THIRD all-PASS cycle after 58/61. Cycle notes below.

**Cycle-level notes (wrap).** (1) **Operator burst mid-wrap (14:36–14:38Z)**: T122 filed WITH ready spec (site timeline curated-sort, user report — stays `todo`; next cycle's predicate holds: queue non-empty + eval fresh today → loopd routes GLM routine, T122 dispatch-ready), the T100 release trigger wired into Phase 3 (80b365c — the rule was spec-only since cycle 57), and **the bootstrap tag v0.1.0 operator-cut → the tag doctrine is ACTIVE from this wrap**; a 30/30 site-layout `chug run` (pid 63474) observed co-active — single-driver invariant NOT tripped (its cwd can't be this repo: my driver.lock is held and it stayed alive; pushes reconciled by rebase 80b365c→9d891bd, no force-push, no mid-arc reconcile). (2) **Composition-artifact watch: 6 sightings in the ORCHESTRATOR's own invoke stream** (split/garbled parameters on delegate launches ×4, edit_file ×2) — every payload verified INTACT by read-back (transcript first-line probes, goal_sha256 comparison across resume, post-edit greps); zero kills, zero bad landings — the T120 sequential verify-then-kill rule was written THIS cycle and exercised 6 times in it (also 2 outcome-id typos caught by my own read-back + fixed pre-commit). Filed to next eval as a watch item, not a row: the tool layer reassembled correctly in all 6 cases, but the composition side is producing artifacts at a rising rate (1 in cycle 60 → ~11 render-only in 61 → 6 invoke-level in 62). (3) kimi ran 4 rounds (REQUIRED ×2: T117 44/50, T120 38/50; optional-EXERCISED ×2: T121 14/50, T118 42/50) — all inside the T18 margin, zero blocking findings, 29/29 mutants killed era-wide-third-cycle; T119 SKIPPED per tests-only precedent. (4) T44 overlap once clean (T121-val ‖ T117-impl, disjoint files, serial merges). (5) T63 resume #25 (T117 impl died 80/80 MID-IMPL uncommitted; resume accepted 23/80) — T110 census: 1-of-4 recent impls died mid-impl, census NOT tripped. (6) Estimate calibration: T117 ~280→+603 (2.2x), T118 ~260→+438 (1.7x), T121 ~25→+10/-4 (spot-on), T120 ~55→+184 (pins inflate), T119 ~70→+127 — test+doc density band holds; ceiling failure measure unmet. (7) Carried to next eval: T117's 1 non-blocking (expansion-order live-pinned not unit — no observable reorder mutant) + T120's 2 (leg-k ordering pin stronger than req; loose-anchor future-heading risk = accepted T64 pattern) + the artifact-rate watch. (8) **Release: v0.2.0 cut at this wrap** (trigger: FEATURES check-off since v0.1.0 — T118's F9-2b; minor bump; T100's own release.yml — landed cycle 56 — fires on it: the loop's first self-cut tag exercises the cycle-56 workflow). **Release-procedure holes found post-tag**: (a) the wrap's final gates ran BEFORE the version-bump commit (the wire's "final gates count" blessing), so the bump itself was never gated — and it BROKE three `build_info` banner pins that hard-coded "0.1.0" (caught by the goal-gate's `cargo test`, not by my wrap gates); fixed post-tag by sourcing the expected strings from the existing `VERSION` constant (the file's fourth banner test already did), making future bumps self-healing. (b) The bump commit ALSO never synced Cargo.lock — cargo auto-synced it locally on my next build (uncommitted), and v0.2.0's release workflow FAILED on its `cargo build --release --locked` leg (a second pre-existing failure: the aarch64 cross build missing libc6-dev-arm64-cross for ring, v0.1.0 run 36437458891, failed v0.1.0 too). The operator remedied both within minutes: 3cd7b7e (CI cross-build libc) + 1e84357 (v0.2.1 with the lock synced + the LOOP-SPEC doctrine "release bumps must sync Cargo.lock"). Tags v0.2.0/v0.2.1 both stand (immutable); **T123 filed post-hoc naming v0.2.0 per the Phase-3 wire**. Next eval weighs adding a post-bump targeted gate (`cargo test --bin chug build_info` + `cargo build --locked` check, seconds) to the release procedure before the tag. Final gates 962/962 + clippy + build at HEAD under target-shared-main; spec-check `cargo test` re-green post-fix.

- **T118 LANDED 4c96414** (fast-forward) — **F9 phase 2 CLOSED** (2b, the deferrable remainder, worked anyway with budget to spare): lenient frontmatter (`---` first line only, strip-before-storage, unclosed fence → whole body + None, empty value → None, unknown keys ignored silently) + `/help` `/name — description` lines (bare-name fallback) + pack-name Tab completion via the pure `slash_candidates_with_packs` merge (built-ins first in `/help` order, packs sorted after, shadow-rule dedup, empty pack set byte-identical to built-ins-only) + README/FEATURES (`allowed-tools` semantics remain a later phase, noted in both). glm impl 68/80 first-try (self-recovered from a 120s bash-timeout on a duplicate test run); kimi optional-EXERCISED (T113 feature precedent) PASS 42/50 — 9/9 mutants RED serial in-tree with declared overlap judgment (mut-strip/unclosed/empty-desc/late-fence/shadow/unsorted/pack-prefix/help-fallback/tab-nopacks); the impl child's recorded spec-interpretation call (spec file over the goal summary's stale fallback phrasing, d1790605202-1) was ENDORSED — mut-help-fallback proves the rejected alternative goes RED — and the validator backfilled the child's own decision record (d1790606020-2). Review + post-merge nextest 962/962. Routing d1790605365-31, verdict d1790606202-32, outcome landed-clean (d1790606264-33/-34). Estimate ~260 → +438 (1.7x — inside the era's density band).

- **T120 LANDED c173c2f** (fast-forward) — the cycle-61 incidents are now LAW: step 2 gains the kill rule ("verify-then-kill is SEQUENTIAL — read first, kill after"; "Never kill in the same breath"; the T112-validator SIGKILL named with its numbers — 2166-byte payload proven intact post-kill, ~11 min relaunch) and step 5 gains the edit-assertion rule ("sed exits 0 on no-match" → grep-verify the needle; `edit_file` preferred; anchor-typo incident named). Legs k–l pin both (windowed exactly-once beside T114's i–j). glm impl 36/80 first-try; kimi REQUIRED PASS 38/50 — 8/8 mutants RED serial in-tree with the overlap judgment correctly declared (both hunks in LOOP-SPEC.md; delete/duplicate/move/reword all fail), 2 non-blocking observations (leg-k ordering pin intentionally stronger than req; loose anchors carry the accepted T64-pattern future-heading risk — carried to next eval). Review + post-merge nextest 947/947. Routing d1790603455-27, verdict d1790603847-28, outcome landed-clean (d1790603906-29/-30). **This doctrine governed THIS cycle immediately**: three orchestrator composition-artifact sightings (T117/T119/T120 launch invokes) were each resolved by read-back (transcript first-line + launch-echo hash comparison) with zero kills — the rule pays for itself in the cycle that wrote it.

- **T119 LANDED 43f427d** (fast-forward) — T115's carried weak-test findings CLOSED, tests-only (+127/-2, zero production change): the multibyte `goal_tail` leg crosses the 120-char cut with a 2-byte `é` and 4-byte `🌍` in the boundary window with the byte-cut asserted `!is_char_boundary` — the byte-slicing mutant PANICS (the exact cycle-61 gap, proven: mutant FAILED while the ASCII leg survived); `goal_bytes` pins the BYTE length on the same fixture (bytes-vs-chars both directions); sha pinned to an external `shasum` vector. The chat None→Some wiring mutant dies to a new session-level null-but-present leg; the driver run/plan Some→None mutants die to T117's own wiring legs (RED-proven here, not duplicated — the child's call, logged d1790602675-1). Count pin 86→87 with its own RED leg re-verified. glm impl 46/80 first-try; kimi SKIPPED per routing d1790602767-25 (T16/T31/T106/T116 tests-only precedent, RED-prove spec-required and documented). Review + post-merge nextest 945/945. Outcome landed-clean (d1790602855-26).

- **T121 LANDED cbc21e0** (fast-forward) — loopd `--max-iters 160 → 200` + sizing-comment rewrite (cycle-61 evidence + HALT-guard tradeoff named) + `loopd_model_routing` needle. glm impl 25/80 first-try (RED-proven both ways: 200-script×160-needle and 160-script×200-needle both fail); kimi optional-EXERCISED (T36 raise precedent + loop-own-budget surface) PASS 14/50 — 3/3 parallel mutants (flag160, needle160, minutes241) killed by the exact-count pin, sweep verified (`160` survives only as comment evidence), zero findings. Review + post-merge nextest 924/924. Routing d1790598904-15, verdict d1790599296-16, outcome landed-clean (d1790599377-17/-18). The raise activates at the NEXT cycle via loopd's self re-exec — this cycle still runs on 160.

- **T117 LANDED 3579d9d** (rebased-ff, orig 07fb88f) — **F9 phase 2a, the roadmap pull's main piece**: run-side goal expansion at the CLI boundary (`chug run/plan --goal "/name args"` → `commands::parse_invocation` + `resolve_goal`; Unknown/Empty = remedy-naming exit-1 BEFORE any `.chug` write; non-invocation = byte-identical zero-cost passthrough), `run_start` gains always-present `goal_pack` with `goal_sha256` over the EXPANDED body, chat stays null, README phase-2a + the hash-difference honesty line at both surfaces (parent echo hashes literal argv ≠ child's expanded-body hash — expansion, not garble). glm impl died **80/80 MID-IMPL** (t1 of 6 committed-less; T110 watch: 1-of-4 recent impls died mid-impl, census NOT tripped) → **T63 resume #25** accepted 23/80 (recovery d1790599841-19). kimi REQUIRED PASS 44/50 (budget_low fired inside the T18 margin): 8/8 mutants killed in parallel legs incl. the T115 Some↔None wiring class at run/plan/chat call sites, live smoke first-hand (goal_pack "smoke", sha≠literal, exit-1 legs zero `.chug` writes); ONE non-blocking note (expansion-before-writes ordering live-pinned, no observable reorder mutant expressible — carried to next eval). Review + post-merge nextest 943/943. Routing d1790600649-20, verdict d1790601740-21, outcome landed-clean (d1790601855-22/-23/-24). FEATURES.md F9 line updated (phase 2a landed; 2b = T118 open). Estimate ~280 → +603 (2.2x, the test+doc density calibration class again — ceiling failure measure still unmet). T44 overlap ran clean (T121-val ‖ T117-impl, disjoint file sets, strictly serial merges).


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

### Cycle 56 (2026-09-28) — T100 LANDED (9d1182a fast-forward: GitHub-releases surface — tag-triggered release.yml + install.sh + notes generator, +2008/-12 across 12 files) via FAIL→fix-up→PASS arc (kimi r1 caught the aarch64→linux-arm64 collapse); cycle-55 divergence reconciled; live chug-site edit 8eee10b; wrap 419afbc

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

