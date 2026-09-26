# EVALUATION — chug, assessed by chug-loop (2026-09-26, cycle 22)

Corpus: `.chug/eval-digest.md` FIRST (T46's pre-digest, regenerated
07:29:57Z covering 89 event streams / 3,626 iterations — **it was missing
at cycle start**: the running loopd predates T46's refresh line; see S3),
then the three orchestrator streams since the cycle-18 eval:
`.chug/events-20260926-052326.jsonl` (**cycle 19**: 116/120 iters, 40m42s,
freshness-skip, T46+T45+T39 landed; 76 bash / 39 delegate, 1 tool error,
budget_low@8, goal accepted), `.chug/events-20260926-061713.jsonl`
(**cycle 20**: 118/120, 52m45s, freshness-skip, T40+T41+T42+T47 landed —
first live T44 pipeline overlap; 5 tool errors, budget_low@8, goal
accepted), `.chug/events-20260926-071302.jsonl` (**cycle 21**: 119/120,
54m47s, freshness-skip, T43 landed + Outcomes compaction practiced; 3 tool
errors, budget_low@8, goal accepted), the 13 child streams t39–t47 (glm
impls 13–50 iters incl. one 50/50 post-commit abort; kimi validators
15–48 iters incl. T47's FAIL→fixup→PASS arc), `.chug/loopd/loopd.log`
(cycles 19–21 OK unattended; cycle-22 start 07:29:30Z), `TODO.md` (T1–T47
all done with refs — queue EMPTY at cycle start, freshness rule cannot
fire → this eval is mandatory), git log `c8a8382` (T43 close), `src/`
(22,026 lines, +1,020 since cycle 18 — webfetch.rs now 1,054), `README.md`
(293 lines — §6 audit below), and the cycle-21 wrap's six written carries
(CARGO_MANIFEST_DIR × target-shared incident, connect-timeout const-pin,
glm truncated-write watch, T31-residual flake watch, web_fetch adoption
grep, README §6 audit — disposition for each below).

## 1. What chug does well

- **The budget margin is now a managed surface, not a death zone.** All
  three orchestrators hit budget_low@8 (116/118/119 of 120) and all three
  wrapped with goal accepted, every artifact committed, zero mid-arc
  losses. T18's margin has absorbed nine consecutive wraps.
- **Recovery doctrine keeps converting deaths into non-events.** The one
  50/50 child (t47-impl) died *post-commit*; its own spec's recovery
  instructions executed cleanly (T15-class, third consecutive clean
  recovery). Zero lost-work deaths since cycle 16.
- **The operator's speed initiative measurably worked.** Cycle 20 ran the
  first live T44 overlap (T41 validator ∥ T42 impl, disjoint files) and
  landed 4 items in 52m47s; T45's bundling doctrine is filed for the
  trivial-row class. Cycle 14's serial baseline was 4 items in 56 min
  *with* a fresh eval.
- **Adversarial validation caught a fleet-wide bug pre-merge.** T47's kimi
  round-1 FAIL *proved* the running-loopd-stale-binary mechanism (export
  persistence across cycles) with a live demonstration, forcing the
  env-prefix fix — exactly the class that ships silently without a second
  model family reading the diff.
- **T46 dogfooded immediately**: this eval read one 67 KB digest over 89
  streams instead of ad-hoc jq ETL; every drill-down below started from a
  digest anomaly.

## 2. Incidents worth fixing

**S1 — env!(CARGO_MANIFEST_DIR) × target-shared staleness false-reds gates
(NEW ROW T48, pri 1).** Cycle-21's post-merge gates failed 2/452 — the
shared-cache test binary had been compiled in the T43 worktree, baking
`/tmp/chug-loop-t43` into `env!("CARGO_MANIFEST_DIR")`; after `git
worktree remove` the path was dead and tests reading
`$MANIFEST/README.md` etc. failed against a *green* tree. Recovery was
touch+rebuild (cheap, once diagnosed). Five sites carry the compile-time
path: `src/tools.rs:3583` (test), `src/build_info.rs:194` (test),
`tests/todo_consistency.rs:135`, `tests/shared_target_dir.rs:21`,
`tests/eval_digest.rs:19` — all test code. Exposure grows at the next
loopd restart: with T47 active at supervisor level, *every* gate runs the
shared cache and *every* removed worktree becomes a stale-path landmine —
and the spec's own `check:` (`cargo test`) is exposed, so a false red can
reject a truthful `goal_complete` at the iteration margin, the exact
cycle-16 death class. Fix: resolve the repo root at *runtime*
(`std::env::current_dir()` — cargo executes test binaries with
cwd = package root, both unit and integration tests), making test binaries
worktree-agnostic and cache-safe. Plus a static pin so no
`env!("CARGO_MANIFEST_DIR")` re-enters test code.

**S2 — api.rs has no connect timeout (NEW ROW T49, pri 2).** Disposition
of the connect-timeout carry: T42 pinned webfetch's consts; auditing the
remaining clients — `mcp_http` has `CONNECT_TIMEOUT = 10s` (const pinned,
T16); `src/api.rs:616-617`'s reqwest builder sets only
`.timeout(READ_TIMEOUT_SECS=600)`. reqwest has **no default connect
timeout**: a blackholed endpoint (dropped SYNs — firewall, wedged NAT, not
a clean refusal) stalls on the OS TCP timeout (~75 s macOS, ~2 min+ Linux)
*per attempt*, and T1's 8-attempt retry loop multiplies that to a ~16-min
stall worst case. T2's activity timeout covers only the post-connect read
phase. Add `CONNECT_TIMEOUT_SECS = 10` (mirroring both siblings — fail
fast, retry sooner is strictly better for T1's restart-survival goal) and
pin all three api consts (T42 pattern). Small, one file, validation
REQUIRED (src/api.rs).

**S3 — The running loopd executes three-generations-stale doctrine
(NEW ROW T50, pri 2 — the cycle's feature row).** Activation lag is now
observable in the corpus, not just theory: loopd pid 90114 started
00:43Z, *before* the T36 (max-iters 160), T46 (digest refresh) and T47
(target-shared env prefix) merges. This cycle therefore launched with
`--max-iters 120` (on-disk loopd.sh says 160 — the cycle-16 Q1 fix for
margin deaths), the digest was absent at eval start (regenerated manually,
0.2 s), and the supervisor hands no shared-cache env. Three landed,
validated, pushed changes have been dormant for 7 cycles. T27's commit
recorded "activation = operator's next loopd restart" as a deliberate
tradeoff (the bash incremental-read hazard makes touching a *running*
supervisor unsafe), and T47's validator proved the mechanism live — but
nobody restarted. Fix per design rule #1 (the loop is code, not
conversation — applied one level up): loopd fingerprints itself at start
(POSIX `cksum`), re-compares at each while-top, and `exec`s itself
*between* cycles when the file changed — never mid-cycle; the STOP file
wins naturally; the pidfile guard gains a same-pid pass (exec preserves
$$, so today's guard would refuse the re-exec'd self). This converts the
standing human-decision carry into code — the last activation-lag carry
the loop should ever need.

**Watch items (disposition of the cycle-21 wrap's carries):**
- **glm truncated-write watch** → T38's advisory is live and surviving
  hits: t46-impl took 1 `output_truncated` and finished 49/50
  goal-accepted; t47-impl took 1 and died 50/50 but *post-commit*
  (recovered by its own instructions). No lethal recurrence of T37's
  987-line saga. Watch continues; not a row.
- **T31-residual flake watch** → 0 organic sightings across cycles 19–21
  (the t47-fixup stream's FAILED line was its own intentional mutation
  control). Watch continues; not a row.
- **web_fetch adoption grep** → 0 organic calls in all 3 orchestrator
  streams (`jq` tool_result count, not file mentions). Repo-local work
  stands; the capability row was filed demand-honest (T37). Watch
  continues.
- **connect-timeout const-pin** → PROMOTED to row T49 (api.rs gap found;
  mcp_http already pinned).
- **bash unquoted-paren syntax errors** — 2nd sighting (cycle 20; cycle 19
  clean; 1 iteration each). Watch; not yet a row.
- **`write_ledger` tool-name hallucination** — 1 sighting (cycle 21,
  `unknown tool: write_ledger`, 1 iteration). Watch; not a row.
- **`path escapes cwd` friction** — cycle 19 orchestrator ×1, cycle 20 ×1,
  t43-impl ×1 (reaching for main's doctrine — legitimate need, bash
  escape works). Cycle 21's orchestrator (first full post-T41 cycle):
  ZERO. The candor pass works at the orchestrator level. Watch; not a row.

## 3. Friction hot spots

**F1 — README "one exception" drift moved but didn't die (NEW ROW T51,
pri 4).** The cycle-18 audit found the Tools intro calling delegate "the
one documented exception" and folded the fix into T41 — which landed the
intro's "two documented exceptions" phrasing. But the delegate
*paragraph* still says "Delegate paths are the one exception to cwd
sandboxing" — three statements of the sandbox exception in one section,
one of them stale. While there: the delegate paragraph has accreted to 15
lines and carries spec-grade mechanism detail (process-group/SIGHUP/nohup
parity, bounded-tail reads) that lives in `specs/t23-delegate-tool.md` —
trim to user-facing semantics. Docs-only, ~10 lines, validation-optional
per T16/T31.

**Verified already-fixed (not re-filed):** budget-margin wraps (T18 —
§1); child 40/40 deaths (T21 — zero since); Outcomes accretion (T43 —
practiced at cycle-21 wrap, 627→319 lines); TODO-pipe goal-gate death
(T40 — three consecutive clean goal gates); eval ETL cost (T46 — this
eval); delegate polling profile (T24/T29 — zero bash sleeps, wait_secs
throughout); history trimming (discovered `src/driver.rs:1016` already
collapses old tool_use payloads — the context-growth concern is managed).

## 4. Capability gaps — FEATURE SCAN (required)

Standard classes interrogated against this corpus:

- **Delegation** — CLOSED: delegate + max_tokens passthrough (T39) +
  zombie reaping (T28) + wait_secs (T29). The entire child interface is
  now the delegate surface; hand-rolled nohup unused since T24.
- **Web access** — CLOSED: web_fetch (T37); 0 organic calls in 3 cycles —
  capability present, demand honest.
- **Parallel tool calls** — serial-in-turn execution of multiple tool_use
  blocks (driver.rs:631). Deterministic ordering is a *feature* for a
  self-driving loop; no corpus incident traceable to serial execution.
  Not filed.
- **Context/history management** — ALREADY PRESENT: driver.rs:1016 trims
  tool_use payloads past 20 messages; ledger carries durable state;
  late-cycle input curves (~14.5k/call at iter 116) are sustainable. Not
  filed.
- **Plan-then-execute modes** — spec+goal+ledger discipline externalizes
  planning; children's failures are budget/truncation class, not
  plan-absence class. Not filed.
- **MCP consumption** — stdio+HTTP shipped (SPEC-9), zero MCP incidents
  in the corpus. Not filed.
- **Session/handoff UX** — resume + rotation trilogy + 9.5-min recoveries.
  Not filed.
- **Steering depth** — chat dock + steering notes + budget-low/truncation
  injections. Sufficient.
- **Supervisor self-refresh (meta-loop class)** — REAL GAP, **NEW ROW
  T50** (the cycle's feature row): the fleet cannot pick up its own
  landed doctrine without a human restart — three changes dormant 7
  cycles is the evidence. See S2/S3.

## 5. Top 3 priorities

1. **T48 (pri 1, bug-class)** — runtime repo-root in test code. Kills the
   target-shared false-red landmine before the loopd restart exposes
   every gate to it; the goal gate itself is the exposed surface.
   Validation REQUIRED (tests/ + src test modules — touches the guard
   suite itself).
2. **T49 (pri 2, robustness)** — api.rs connect timeout + const pins.
   Closes the last timeout-asymmetry across the three HTTP clients; one
   file. Validation REQUIRED (src/api.rs).
3. **T50 (pri 2, feature)** — loopd self-exec on self-change. Ends the
   activation-lag carry class; supervisor tooling → validation REQUIRED
   (T47 precedent: loopd core flow).

(T51, pri 4 docs, follows if budget allows — trivial, validation-optional.)

## 6. README audit (usability, not just accuracy)

Cold-read of all 293 lines:

(a) **Reading order** — sound: what-it-is → quickstart → interactive →
autonomous → TUI → tools → risk gate → MCP → observability → self-hosting
specs → loopd → development. T37/T39/T41/T46/T47 additions are integrated
paragraphs, not glued bullets. No structural accretion debt.

(b) **Redundancy/drift — ONE finding:** the delegate paragraph's "the one
exception to cwd sandboxing" is stale (T37 made it two; the intro already
says "two documented exceptions" post-T41) and the exception is stated
three times in one section. The paragraph also carries spec-grade detail.
→ T51.

(c) **Staleness** — none beyond (b). Spot-verified live: `chug ledger`
subcommand exists (`src/main.rs:136` CliCommand::Ledger); `--bash-timeout`
flag exists (`src/main.rs:78`); budget-low ≤8 (T18), truncation advisory
(T38), token budget (T15/T39), events-log fields (T17/T20/T25/T38) all
match the code. The loopd section documents on-disk doctrine — correct
for a README (it documents the code, not the running process; S3 covers
the process).

(d) **Balance** — the delegate paragraph (15 lines) is at the over-detail
edge (part of T51); MCP's length remains justified (user-facing config
surface). Events-log bullet is long but it is the telemetry reference.

(e) **Quickstart truth** — `cargo build` → `cargo install --path .` →
`chug run … --max-iters 40 --max-minutes 120`: true as written (flags
verified against main.rs; auth chain comment matches SPEC-6).

## Handoff — recommended execution order

Queue (all rows have ready specs; priority doctrine bugs > robustness >
features > DX > perf, features first-class at equal pri):

1. **T48** (pri 1) — glm impl + kimi validation REQUIRED (touches the
   guard-suite tests themselves). Est. 25–30 orchestrator iterations.
2. **T49** (pri 2) — glm impl + kimi validation REQUIRED (src/api.rs).
   Est. 25–30. T44-overlappable with T48's validation (disjoint files:
   api.rs vs test files).
3. **T50** (pri 2) — glm impl + kimi validation REQUIRED (loopd.sh core
   flow + static-pin tests; watch tests/shared_target_dir.rs count pins
   when adding lines to loopd.sh). Est. 30–35. NOT overlappable with T48
   (both may touch tests/shared_target_dir.rs).
4. **T51** (pri 4) — glm impl; validation OPTIONAL (docs-only, T16/T31
   precedent). Est. 15–20.

Budget arithmetic at the 120-iteration cap (the running loopd still
predates T36 — see S3): Phase 1 ≈ 45–55, one item ≈ 25–35, wrap ≈ 10 →
this cycle realistically lands **2–3 rows** (T48 + T49, + T50 if lean);
the rest stay `todo` with ready specs — a fine outcome per doctrine, and
the freshness rule lets the next cycle skip Phase 1 and start straight on
the queue. If T50 lands, every future cycle gets the 160-iter budget
within one cycle of a loopd.sh merge, and this arithmetic stops being a
carry.

Human-decision items (for the operator): **restart loopd** — now doubly
urgent (activates T36/T46/T47 immediately; if T50 lands first, it becomes
the last restart the loop ever asks for). web_fetch adoption remains
watch-only. `du -sh target-shared` reclaim policy remains the operator's
call (T47: nothing cleans it automatically).

## Outcomes (filled at cycle wrap — LOOP-SPEC Phase 3)

### Cycle 23 (2026-09-26, in progress) — freshness-skip; T49 validated + merged

**Landed:**
- **T49 — api.rs LLM client gains a 10s connect timeout const + value pins** (pri 2; cycle-22 impl `56ecf72` glm 34/50 + orchestrator review; this cycle: kimi validation, merge `9a73b12`, flip + this entry per T34). `CONNECT_TIMEOUT_SECS = 10` const with the class doc + `.connect_timeout` builder leg + 3-value pin test; `classify_reqwest` unmodified (is_timeout/is_connect → retryable, verified incl. reqwest 0.12.28 from Cargo.lock). kimi VERDICT: **PASS** 17/50 — all 5 reqs verified, gates independently re-run (468 passed / 0 failed), 4/4 value mutants KILLED with the panic naming the correct const; the `.connect_timeout`-leg deletion mutant SURVIVES as the spec-sanctioned evidence level (T16/T42 precedent: unobservable without a SYN-dropping endpoint; pins pin values, not wiring); tree restored byte-clean. Post-merge main gates 453+6+1+6+3 + clippy green. 2 validator artifacts harvested pre-removal.
- **T50 — loopd re-execs itself between cycles when its own script changed** (pri 2, FEATURE — closes the activation-lag gap: T36/T46/T47 sat dormant 7 cycles in the running supervisor; impl `ca11c6b` glm 37/50 goal-accepted; merge `078cb77`; flip + this entry per T34). `SELF_CKSUM` cksum fingerprint before the cycle loop (content-based — `touch`/content-preserving checkout never re-execs); compare→log→`exec "$ROOT/loopd.sh" run` as the FIRST while-body statement (re-exec only BETWEEN cycles, zero incremental-read exposure; STOP wins at the while condition, never cleared by this path); pidfile guard refuses only a LIVE pid ≠ `$$` (same-pid pass for the exec'd self; stale-pid fall-through and foreign-supervisor refusal kept); tradeoffs documented in-script; tests/loopd_reexec.rs +144 (4 positional static pins — before-the-loop, first-statement ordering, log-before-exec, guard shape + liveness). Impl ran the spec's FULL optional live smoke: re-exec line observed at the next while-top ~58s after a comment append, same pid, no refusal, STOP leg clean, scratch removed; 2 non-vacuousness mutants killed. kimi VERDICT: **PASS** 41/50 (first validator on `target-shared-validate` per T52's ALWAYS rule) — gates independently re-run 476 + `bash -n`, 8/8 spec-shaped mutants killed (exec deletion, guard revert, `!=`→`=`, fingerprint deletion, log deletion, `kill -0` drop, block relocation, log/exec swap), live smoke independently re-demonstrated (re-exec 103s after append, same pid 29636, STOP landed clean), reqs 1–6 verified, tree clean. Non-blocking: (a) hardening suggestion — an additive duplicate-`SELF_CKSUM` mutant at the while-body end silently defeats the mechanism with all pins green (exact-count pin candidate, T47-carrier doctrine); (b) observation carried to the next eval — the host's `pgrep` could not see the long-running real chug during validation while `ps` showed it (pre-existing single-driver-check exposure; loopd's skip line uses `pgrep -f`), and the live supervisor still launches `--max-iters 120` (T50's premise confirmed in the wild). Post-merge main gates 453+6+4+1+9+3=476 + clippy green, `bash -n` clean. 4 artifacts harvested pre-removal.
- **T52 — role-keyed target dirs (T44-overlap × target-shared race fix)** (pri 2, doctrine, ran alone; impl `bdba301` glm 28/50 first-try; merge `4d4c383`; flip + this entry per T34). LOOP-SPEC: step-3 gates dir conditional (`target-shared` no-child / `target-shared-gates` in the overlap window incl. N+1's impl during N's post-merge gates) + the mechanism sentence (metadata hash excludes checkout path → same artifact name → last-builder-wins); step-4 validator export `target-shared-validate` ALWAYS; step-5 pointer. `.gitignore` gains both siblings contiguous (position-pinned); README T47 paragraph gains the integrated sibling-cache clause; loopd.sh + META-SPEC untouched; tests/shared_target_dir.rs +3 pins (6→9 — exact-count carriers are strictly stronger than the spec's `≥2`: a duplicated-carrier mutant dies). kimi VERDICT: **PASS** 35/50 — all 6 reqs verified, gates independently re-run 453+6+1+9+3, 10/12 mutants killed (2 survivors spec-sanctioned doc carriers: step-5 reminder sentence + README clause, same class as T44's prose survivors), tree byte-clean; the validator dogfooded the rule under test by building into `target-shared-validate` itself (auto-created, cold 27s, now warm). Post-merge main gates 453+6+1+9+3 + clippy green. 5 artifacts harvested pre-removal (impl stream was T7-rotated at the validator's reseed — both recovered).

### Cycle 22 (2026-09-26, ~03:29–04:15 EDT) — MANDATORY fresh eval (queue EMPTY); T48 landed green; T49 impl done, validation deferred at budget; T44-overlap × target-shared race observed live → T52 filed

Single `chug run` (kimi-k3 orchestrator; glm-5-3-flash impls; kimi-k3 validators), loopd-launched — the RUNNING loopd still predates T36/T46/T47 (this cycle: 120 iters, digest regenerated manually at eval start, no T47 env prefix; restart-loopd carry stands, T50 is its coded fix). Queue was EMPTY at start → freshness rule could not fire → mandatory Phase 1: digest-dogfooded eval (T46's artifact carried the whole corpus read; Phase 1 ≈30 orchestrator iterations vs the 45–55 estimate), 5 rows filed with ready specs (T48 pri 1, T49 pri 2, T50 pri 2 feature, T51 pri 4 at eval `0553a5b`; T52 at wrap).

**Landed (1 of 5 filed):**
- **T48 — runtime repo-root resolution in test code** (pri 1; impl `1cf0b64` glm goal-accepted 25/50 ~8 min first-try, self-recovered one git-checkout-index scar mid-mutation-check; merge `d000b72`; flip + this entry per T34). All 5 `env!("CARGO_MANIFEST_DIR")` test sites → `std::env::current_dir` with the cargo-cwd invariant commented; new tests/no_compile_time_manifest_dir.rs static pin (concatenated needle, no self-exemption, empty-scan vacuity guard, byte-level search); zero production changes; suite 467→468. kimi VERDICT: **PASS** 28/50 — 11/11 mutants killed (pin self-mutations, corrupted needle, empty-scan, flipped assert, all 5 converted-site bogus-path legs), tree byte-clean; also sighted 1 organic T31-residual dead_port_probe flake (watch stays open — 2nd organic sighting after cycle 16).

**Deferred at budget (all with ready state):**
- **T49 — api.rs connect timeout** (pri 2): impl `56ecf72` DONE on branch `loop-t49` (glm 34/50) + orchestrator review passed (deterministic own-content gates 453+6+6+3, clippy); REQUIRED kimi validation never launched → NOT merged; row `in-progress` with the recovery recipe (worktree-from-branch → validate → merge).
- **T50** (loopd self-reexec) and **T51** (README delegate paragraph): untouched, ready specs.
- **T52 — overlap×cache race** (pri 2, doctrine, runs alone): filed at wrap with the mechanism verified against the cache (cycle notes below).

**Cycle-level notes:** (a) **NEW CONFIRMED INCIDENT CLASS — T44-overlap × target-shared artifact-identity race.** Mid-validation the orchestrator's t49 review gates executed the T48 validator's leftover MUTANT binary (one 6-test suite red 6/6; self-healed on rebuild): cargo's metadata hash excludes the checkout path (same artifact filename across worktrees), dep-info paths are relative, mtime-freshness called the fresh mutant "fresh", and the target-dir lock serializes compile but not interleaved build→run. The validator's PASS was uncontaminated (it finished before the orchestrator's builds); T49's gates were re-run deterministic (touch-rebuild → own content, 453 tells the pin test is present). The false-green leg is the silent one → T52's role-keyed dirs. (b) **Orchestrator scar:** one mangled delegate launch (validator goal corrupted with an impl-goal fragment) — killed at ~1 iteration, relaunched verbatim-clean; harvested as events-t48-validate0-*. (c) T44 overlap again landed a clean pipeline: T48 validated while T49 implemented; merges serial. (d) Budget: wrap fired at 8 iterations remaining again — 120 is still the ceiling (T36's 160 dormant with the stale loopd; T50 removes the activation-lag class itself).

**Final state:** main = wrap commit (T48 merge `d000b72` + flip; T52 filed); gates 452+6+1+6+3 green in main, clippy clean; TODO truthful (T48 done w/ ref; T49 in-progress w/ recipe; T50/T51/T52 todo w/ specs); 7 child artifacts harvested (t48 impl/validate0/validate events + impl/validate LEDGERs; t49 impl events + LEDGER); README gate satisfied (T48 is test-internal — no user-visible surface); pushed. **Handoff: FIRST ACT next cycle = T49's validation+merge (recipe on the row); then T52 (doctrine, runs alone) or T50 by pri; the freshness rule fires (todo rows exist + same-day eval).**

### Cycle 21 (2026-09-26, ~03:13–03:40 EDT) — freshness-skip; T43 landed; CARGO_MANIFEST_DIR × target-shared staleness bit the post-merge gates

One `chug run --spec LOOP-SPEC.md` session (kimi-k3 orchestrator; glm-5-3-flash impl child; kimi-k3 validator), launched by loopd. **Freshness rule fired** (cycle-18 eval same-day + T43 `todo` with a ready spec) → straight to the queue. Single todo row → no bundling, no overlap candidates; single-driver verified (only this `chug run`).

**Landed (1/1 queued rows):**
- **T43 — EVALUATION.md Outcomes pruning rule + first compaction** (pri 4, doctrine; spec amended pre-dispatch `67fa2b8` — the check's 200-line bound was unreachable: authored at cycle 18 on a "cycles 14–17 full ≈150" estimate, cycles 18–20 then added ~256 full-entry lines; bound raised to 400 with the arithmetic recorded in req 3, 1d6780d precedent; impl `4ed7bdd` glm goal-accepted 20/50 ~4 min first-try, merge `8694003`, flip + this entry in the same `todo:` commit per T34 doctrine). LOOP-SPEC Phase 3 gains the wrap-time rule (+5 lines, all else byte-identical): Outcomes keeps the last 6 cycles in full; older entries compact to one-liners; the narrative lives in git. First compaction: cycles 5–13 (311 lines) → 9 one-liners, all 30 cited refs cat-file-verified; EVALUATION lines 1–536 cmp-verified byte-identical; Outcomes 627→333 lines; file 80,908→51,888 bytes. kimi VERDICT: **PASS** 17/50 — all 4 reqs hand-verified (byte-identity cmp, ref existence + subject/date matching against parent entries), 5/5 mutants killed (rule-token corruption, ≥400 padding, Cycle 5/13/14 heading corruption/deletion), gates re-run 452+6+6+3 + clippy, tree restored clean; one nit (commit-message "35 refs" vs actual 30) fixed by orchestrator amend pre-merge. 6 artifacts harvested pre-removal (impl events/LEDGER/delegate.log copied BEFORE the validator launch per the T39 reseed lesson, then validator events/LEDGER/delegate.log).

**Skipped/deferred:** none — the single queued row landed; the queue is **EMPTY** → the freshness rule cannot fire → the next cycle MUST run a fresh Phase-1 eval.

**What the validators caught:** no implementation defects. The kimi validator's ref-by-ref subject/date matching against the parent full entries is the deepest truthfulness check a compaction has received; its 5-mutant set proved every check leg kills something.

**Cycle-level notes:** (a) **NEW INCIDENT CLASS — `env!("CARGO_MANIFEST_DIR")` × target-shared staleness.** Post-merge gates in main FAILED 2/452 (`build_info::resolve_head_in_the_real_repo…` panicked "checkout resolves"; `tools::readme_tools_intro_names_both_sandbox_exceptions`) — the shared-cache test binary had last been compiled in the worktree (validator's gate run), cargo's fingerprint accepted it as fresh in main, and the compile-time `env!` had baked `/tmp/chug-loop-t43` — deleted at worktree removal seconds earlier. Recovery: `touch src/*.rs build.rs` → rebuild → 452+6+6+3 green. **Five test sites use the compile-time env var** (src/tools.rs README pin, src/build_info.rs real-repo resolve, tests/todo_consistency.rs, tests/shared_target_dir.rs, tests/eval_digest.rs) — each reads the WRONG checkout when the shared cache serves a binary compiled elsewhere: false RED if that checkout is deleted (this cycle), false GREEN if it still exists (the scarier leg — gates could pass on stale file contents). Fix candidates for the next eval to weigh: runtime path resolution in the tests (env::var/current_exe/dir-diff rebuild discipline), or a LOOP-SPEC gate rule (touch a src file before main-tree gates after any worktree removal). Filed as a written carry, not a row — Phase 1 owns row-filing and the queue-empty eval is mandatory next cycle. (b) **T43's own rule practiced at this wrap:** adding cycle 21 pushed cycles 14–15 outside the last-6-in-full window → both compacted to one-liners in this same entry edit (refs cat-file-verified) — the rule's first wrap-time application; Outcomes holds at 319 lines (627 at this cycle's start). (c) The pre-dispatch spec-amendment discipline (1d6780d → 67fa2b8) held again: measure the spec's stale assumptions before dispatch, amend with arithmetic, cite in the flip. (d) The glm child caught the 65,802→80,908 byte drift unprompted and cited both figures in its commit — spec-quality × model discipline compounding.

**Final state:** main = this `todo:` commit (merge `8694003` + flip); gates 452+6+6+3 green in main post-touch-rebuild, clippy `--all-targets -D warnings` clean; TODO.md truthful (T1–T47 all done with refs — zero `todo` rows); 6 t43 child artifacts in `.chug/` (impl+validate events, impl+validate LEDGERs, 2 delegate logs); README gate satisfied (no user-visible surface this cycle — doctrine + handoff-doc compaction only); everything pushed. **Handoff:** next cycle MUST fresh-eval (queue empty); written carries: the CARGO_MANIFEST_DIR staleness finding + fix candidates in note (a); standing human-decision carries unchanged.

### Cycle 20 (2026-09-26, ~02:17– EDT) — freshness-skip; T40 recovered, T47+T41+T42 landed; T43 deferred

**T41 (sandbox-candor tool descriptions — DX friction) → done 7170fb4.**
glm impl 37/50 first-try: all five filesystem tools (read_file, write_file,
edit_file, glob, list_dir) now name the `path escapes cwd` refusal and the
`bash` escape hatch in both the tool description and the path property;
README's stale "one documented exception" line now names delegate AND
web_fetch. Five live-schema pin tests (T22 pattern — assert tool_schemas()
output, not copied literals). kimi VERDICT: PASS 36/50 — 10/10 mutants
killed including a resolve_safe error-string corruption and a surgical
refusal-naming drop; the T22/T26/delegate/path_safety pins all pass
unmodified. Landed under the FIRST live T44 overlap: T42's impl ran
concurrently (disjoint files — tools.rs+README vs webfetch.rs), both
children finished within 7 minutes, merges stayed serial.

**T42 (webfetch timeout const pins — robustness nit) — done 7ff8bfa.**
glm impl 9ebd55d 28/50 first-try under the first live T44 overlap (ran
concurrent with T41's kimi validator — disjoint files, tools.rs+README vs
webfetch.rs; both children finished ~7 min; merges stayed serial). Pins
WEB_FETCH_CONNECT_TIMEOUT == 10s and WEB_FETCH_TOTAL_TIMEOUT == 30s in
webfetch.rs's test module so a const-only edit now fails the suite (T37
validator's non-blocking observation, closed). Tests-only item —
adversarial validation optional per T16/T31 precedent; orchestrator gates
(452+6+6+3 in main) + diff review sufficed. 1 artifact harvested
pre-removal.

**T47 (shared CARGO_TARGET_DIR for worktree builds — operator speed
initiative) → done 2f4cefc.** The cycle's adversarial-validation showcase:
glm impl c3e8942 died 50/50 post-commit (T15-class recovery, warm-cache
first dogfood 28.6s), kimi round-1 returned VERDICT: FAIL with a PROVEN
cross-cycle loopd.sh bug — the bare `export CARGO_TARGET_DIR` inside the
while loop persists into cycle 2+, redirecting the supervisor's own build
into target-shared and leaving ./target/debug/chug permanently stale
(faithful 2-cycle simulation) — plus the M6 mutant (both META-SPEC nohup
prefixes dropped) surviving the weak >=3 carrier pin. glm fix-up a65904f
(env-prefix on the chug invocation, bare-export ban pin, per-carrier
count_eq pins, self-mutation-tested) then kimi re-validation VERDICT: PASS
42/50 — both findings verified resolved with the required mutants, 8/8
mutants killed, non-vacuousness proven on the parent, tree sha-restored,
acceptance evidence recorded (3.0s warm build vs the 43s–6min cold range).
The loopd restart is pending operator action, so the export reaches cycles
only after the next supervisor start.

**T40 (LOOP-SPEC step-5 pipe doctrine) → done 75c21c8.** The cycle-19
mid-arc handoff, recovered per the T29/T37 precedent: diff reviewed (+7
lines LOOP-SPEC.md, one sentence woven into step 5's row-flip directive,
exactly per spec), gates re-run in the preserved worktree (444+6+3 +
clippy), kimi validation (REQUIRED — doctrine) VERDICT: PASS at 15/50 —
non-vacuousness proven (all 3 check tokens absent on the parent commit),
M1/M2/M3 token mutants killed, M4 prose-weakening survivor adjudicated
by-design per the T44 precedent, tree restored sha256-verified, commit
citations grounded (75da192 + cycle-18 eval). Impl events harvested BEFORE
the validator launch (the T39 reseed lesson), validator events + verdict
LEDGER harvested pre-removal; gates re-run in main post-merge. The new
doctrine was practiced at its own row flip: no pipe in the notes cell,
`cargo test --test todo_consistency` run before committing.

**Cycle-level notes.** Four items landed (T40 recovery, T47, T41, T42);
T43 (Outcomes pruning, pri 4, doctrine) deferred on the step-6 budget rule
— its spec is ready and its check is self-contained; work it FIRST next
cycle (it bounds the very carry-forward cost this cycle's four Outcomes
entries just added to). Validators earned their budget all cycle: T47's
round-1 FAIL caught a proven cross-cycle loopd bug simulation-verified;
T41's killed 10/10 mutants. First live T44 overlap ran clean (T41 validator
+ T42 impl, ~7 min wall for both). T47's warm cache dogfooded immediately:
every child build this cycle was seconds, not minutes. Watch items carried:
T46 golden-section pin test (cycle-19), T36/T47 loopd restart still pending
operator (the env-prefix reaches cycles only after the next supervisor
start), README gate: all four items integrated (T40/T47 doctrine is in-spec,
T41 README two-exceptions line + T47 loopd paragraph landed in their merge
commits; T42 is tests-only, no user surface).

### Cycle 19 (2026-09-26, ~01:23–02:20 EDT) — freshness-skip; T46 + T45 + T39 landed

**T39 (delegate launch max_tokens passthrough — FEATURE) → done 52ec8ab.**
The queue's feature row, worked third after the two pri-2 speed rows.
glm impl 30/50 first-try. `delegate` launch gains optional integer
`max_tokens` (schema `minimum: 1`): present → `--max-tokens N` appended at
the child argv tail (T15 parity); absent → argv byte-identical to pre-T39
(children keep no-ceiling behavior unless the orchestrator opts in).
Implementation shape: `delegate_child_argv` pure seam (whole-list pinned
in tests, plus end-to-end argv dumps through the `CHUG_DELEGATE_BIN`
stub), `delegate_max_tokens` parse (`< 1` → tool error naming the
constraint — 0 would read as "unlimited" on the child CLI, the
silent-unbounded-launch failure the guard exists to prevent), return-text
echo `max_tokens: N` only when configured (absent → byte-identical),
status ignores it exactly as it ignores max_iters today (pinned), README
launch sentence gains one hyphenated clause matching how
`--max-iters`/`--max-minutes` are presented. 8 new tests (444+6+3 total).
kimi validation VERDICT: PASS 38/50 — all 5 reqs hand-verified (incl. the
child CLI's u64/0=unlimited semantics confirming the reject-0 guard is
correct, not a behavior change), gates re-run on the pristine tree, both
spec-named acceptance mutants KILLED (drop-argv-append, accept-0) plus
drop-return-echo and drop-schema-minimum; M3 (as_u64 for as_i64) survives
adjudicated spec-neutral (both error legs name the constraint; 0/neg
still rejected pre-spawn); 3 non-blocking findings (negative-leg message
not uniquely pinned, >i64::MAX rejects as "non-integer" fail-closed, stub
`sleep 60` linger matches pre-existing delegate test pattern). 3
artifacts harvested pre-removal (impl events, validate events, verdict
LEDGER — the impl's own LEDGER was overwritten by the validator's
fresh-run reseed before archiving; its substance survives in the impl
events stream + commit message). Gates re-run in main green.
LOOP-SPEC's child-launch templates deliberately do NOT pass max_tokens
yet — spec's out-of-scope: adoption is a doctrine decision for a later
evaluation (the T37-validate 169k-token datum is the motivating case).

**T45 (trivial-row bundling doctrine) → done 3efb98d.** Second speed row,
LOOP-SPEC-only (+39/−5). glm impl 28/50 first-try — the doctrine-item
landing class holds (T32/T33/T34/T44/T45 all ≤28 iters). §2 gains
"Trivial-row bundling (T45)": ONE impl child may take up to 3 rows under a
CONJUNCTIVE four-condition eligibility gate ((a) ≤ ~30 changed lines per
row, (b) same 1–2 files or docs/doctrine-only, (c) no driver.rs/api.rs
core loop, (d) every pri ≤ 3 — "when any one is in doubt, run the rows
separately"); never bundled: features, cross-area, >3 rows. Child goal =
step-2 template + explicit row list + spec paths, ONE commit PER ROW in
queue order; ONE kimi round covers a bundle (mutation-test per row where
feasible; any core-adjacent row → REQUIRED validation covers the set;
docs/pins-only bundles may rely on orchestrator gates per T16/T31/T35);
bundled rows may flip in ONE `todo:` commit naming every row + ref; T44
interplay written in (bundle = ONE impl child under the 2-in-flight cap;
a doctrine-containing bundle never overlaps). kimi validation VERDICT:
PASS 20/50 — all 4 reqs verified against the on-disk spec, gates
independently re-run (436+6+3 + clippy), M3 mutant (TODO spec-cell
corruption) KILLED by todo_consistency (the T8 guard the doctrine leans
on is live and toothy), M1 (ALL→ANY conjunctivity flip) / M2
(squash-commit inversion) prose mutants by-design survivors per the T44
precedent (the spec's Tests leg designates review as the check), tree
restored byte-identical hash-verified. One non-blocking finding, recorded
for the first bundle dispatch: delegate's mandatory `spec:` field takes
exactly one path — use the first row's spec, the child reads the rest
from its worktree checkout. 4 artifacts harvested pre-removal; gates
re-run in main green. Acceptance is forward-looking: a later cycle lands
a ≥2-row bundle with per-row commits under one verdict.

**T46 (eval digest) → done 6a3fa98.** Freshness rule fired (EVALUATION.md
same-day + 8 todo rows) → Phase 1 skipped, straight to the queue. Queue
order T46 → T45 → T39: the cycle-18 handoff recommended T45 first, but
T46 is the same pri and its script is the more mechanical land; both are
doctrine items (no overlap either way). glm impl 49/50 goal-accepted
(budget_low@8 fired — wrapped inside T18's margin):
`scripts/eval-digest.sh` (293 lines, jq/awk-only, `LC_ALL=C`, pinned-clock
`CHUG_DIGEST_NOW`, BSD/GNU stat+date fallbacks, ~0.2s on the real corpus)
writes `.chug/eval-digest.md`: per-archive iterations/wall/tool
distribution/error classes/cumulative token totals (T14
last-value-not-summed semantics, hand-verified by the validator against
raw jq)/quartile input-context curve/aborts/budget_low/output_truncated,
plus corpus-wide TODO counts, EVALUATION.md age, and a self-flagging
staleness block (generated-at vs newest-events-mtime + write-race flag +
reader-side `find -newer` check). loopd.sh refreshes it before every
cycle (set -u-only → digest failure non-fatal). META-META-SPEC corpus
item 1 = digest-FIRST, raw events only for incident drill-down
(renumbered 1–6; the §6 cross-ref is to the EVALUATION section list,
unaffected). tests/eval_digest.rs: 6 legs — spec-pinned jq baseline on 3
sampled archives, empty + missing `.chug/`, malformed-line tolerance
(digest=4 where the naive pipeline dies), pinned-clock byte-determinism,
TODO/EVAL parsing. kimi validation VERDICT: PASS 31/50 — 5/5 reqs
hand-verified, gates re-run twice (436+6+3 + clippy), 6/6 kill-mutants
died; 7/7 survivors (unpinned spec-required output fields: tools top-N,
quartile divisor, iters-ceil source, abort budget label, `dur()` hours
branch, reader-check text, budget_low detail) adjudicated non-blocking
per the T44 precedent (spec Tests scoped iteration-counts + empty-digest
only) — **watch item for a future eval: one golden-section pin test per
archive would close the survivor class**; 4 cosmetic observations (one —
the corpus-item-3 backticks — orchestrator nit-fixed pre-merge, 6a3fa98).
4 artifacts harvested pre-removal; gates re-run in main green.
Acceptance is forward-looking per the spec: the next FRESH evaluation's
events should show the digest read early and Phase-1 iterations visibly
below the ~45–55 baseline — to be recorded in that cycle's Outcomes.

**Cycle-19 wrap.** Three rows landed (T46, T45, T39 — every impl glm
first-try, every validation a first-arc PASS, zero fix-up rounds; each
full arc cost ~13–15 orchestrator iterations with sleep-paced status
polling). **T40 mid-arc handoff:** impl child goal-accepted 13/50,
committed 311d305 on branch loop-t40 in the PRESERVED worktree
/tmp/chug-loop-t40 (LOOP-SPEC.md +7, one file) — the orchestrator's
budget-low (8 iterations left) hit before review/validation/merge. The
child is dead (state: done), its cwd was the worktree, so no
single-driver conflict. Recovery recipe is written on the T40 TODO row
(T29/T37 precedent); the worktree must NOT be removed before its
.chug/events.jsonl is harvested. **Deferred, all todo with ready specs:**
T47 (pri 3, shared CARGO_TARGET_DIR), T41 (pri 4, sandbox-candor
descriptions — NOT bundle-compatible with T42 under T45's same-1–2-files
clause: T41 is tools.rs + README, T42 is webfetch tests), T42 (pri 4,
webfetch timeout pins), T43 (pri 4, Outcomes pruning — increasingly
load-bearing: this section grows every landing). **Doctrine landed this
cycle, available but not yet exercised:** T44 pipeline overlap and T45
trivial-row bundling (this cycle ran under the pre-T44/T45 launched spec;
their acceptance markers are forward-looking). **Watch items for the next
evaluation:** (a) T46 validator's golden-section pin recommendation (7/7
survivor class: unpinned digest output fields); (b) T39 validator nits
(negative-leg error message not uniquely pinned; delegate stub sleep-60
linger, pre-existing pattern); (c) T46 acceptance — next FRESH eval must
show the digest read early with Phase-1 iterations visibly below the
~45–55 baseline; (d) T36's 160-iteration loopd budget is STILL pending an
operator loopd restart — this cycle ran at 120 and fit only because
Phase 1 was skipped; a fresh-eval cycle at 120 lands ~2 items; (e) small
mystery for the corpus: T39's impl LEDGER.md was overwritten by the
validator's fresh-run reseed without a .chug/LEDGER-\* archive (T3 should
have archived a non-seed ledger — worth one jq query next eval).
Freshness rule fires again next cycle (EVALUATION.md same-day, todo rows
remain): skip Phase 1, recover T40 first.

### Cycle 18 (2026-09-26, ~00:41–01:30 EDT, continued) — T44 (operator pri-1 speed initiative) landed

**T44 (pipeline overlap doctrine) → done 41f61a1.** Operator filed
T44–T47 mid-cycle (58be922, "feels awfully slow"); priority doctrine
re-planned the queue to T44 first (pri 1). glm impl 23/50 FIRST-TRY (the
doctrine-item class: T32/T33/T34/T44 all land 11–23 iters) — §2 opening +
new "Pipeline overlap (T44)" paragraph + Hard-rules rewrite (ONE WRITER
per file set, serial merges; META-SPEC override scoped to launch
concurrency only; stale "foreground" dropped). kimi validation VERDICT:
PASS 19/50 — all 5 reqs verified, gates re-run independently (436+3 +
clippy), guard-liveness mutant (removed t44 spec) killed by
todo_consistency, prose-mutant survival adjudicated by-design (the suite
cannot police prose; detection = review + next-cycle events per spec
Tests), tree restored byte-identical (hash-object verified). 4
non-blocking observations (empty-target-list edge, fix-up-child cap
classification implicit, 2-validator sequencing implicit, prose
unpoliceable). 4 artifacts harvested. Acceptance is forward-looking:
observable when a later multi-item cycle's events.jsonl shows impl N+1's
run_start before validator N's verdict. **Deferred this cycle (budget
step-6 rule, ~10 iters left at T44 landing): T39, T40, T41, T42, T43,
T45, T46, T47 — all todo with ready specs; the freshness rule lets the
next cycle skip Phase 1 and start on the queue. Recommended order for the
next cycle per priority doctrine: T45 (pri 2 bundling — compounds with
T44; T40/T41/T42/T43 are its first bundle candidates), T46 (pri 2), T39
(pri 3 feature), T47 (pri 3), then the pri-4 set.**

### Cycle 18 (2026-09-26, ~00:41–01:10 EDT) — fresh eval (queue EMPTY at start) + T38 landed; TWO 50/50 child deaths, both recovered

**T38 (truncated-response advisory) → done a686522.** Fresh evaluation filed
6 rows (T38–T43); T38 worked first (pri 2). glm impl died 50/50 mid-wrap
(T18's budget_low@42 fired and was acknowledged — 5th of the 50/50 class;
227k/28.6k tokens) with the work complete-but-uncommitted (396 insertions,
exactly the right 6 files, README done). Orchestrator harvest-completed one
fix the dying child never got to run: its run_t38 scripted harness used
max_iters=10, so the T13 budget-low notice fired on turn 3 and landed AFTER
the second advisory, breaking the no-latch test's last-message assertion
(1/436 red as harvested; 10→20, 436+3 green, clippy clean → 364b95c). kimi
adversarial validation then died 50/50 ITSELF — post-mutations, pre-verdict:
10/10 mutants killed on record (condition delete/flip, token corrupt,
event/transcript/chat-injection drop, type-string corrupt, latch, ordering,
sink-silence), tree restored clean. Orchestrator re-verified (clean tree,
436+3) and RESUMED the validator via `chug run --resume` over bash nohup —
**delegate cannot express `--resume` (capability gap, watch item for the
next eval)** — which emitted VERDICT: PASS, goal accepted (all 7 reqs
verified, gates green twice consecutive post-restore; 3 non-blocking
observations: chat goal-accepted edge gets no advisory by design, TODO row
flip pending = orchestrator's job, one transient unnamed flake during
mutation runs). 4 artifacts harvested pre-removal (impl+validate events,
impl+validate LEDGERs). Also noted live: delegate status misreports a
RESUMED child — its events stream holds the prior run's abort, so status
said `state: aborted` while the resumed validator ran healthy (watch item;
ps fallback used). T41 sighting #3: orchestrator's own edit_file to the /tmp
worktree refused (bash python3 workaround).

### Cycle 17 (2026-09-26, ~00:31–00:45 EDT) — freshness-skip; T37 recovered + landed, QUEUE DRAINED

One `chug run --spec LOOP-SPEC.md` session (kimi-k3 orchestrator, loopd-launched; kimi-k3 validator child via delegate). **Freshness rule fired as designed** (cycle-16 eval same-day + T37 `todo` with ready spec + preserved worktree) → zero re-eval burn, straight to the queue's single row. T37's per-item entry is written here at landing per T34 doctrine (this commit IS the row-flip commit's companion).

**Landed (1/1 queued rows):**
- **T37 — `web_fetch` tool: read-only HTTP(S) GET, bounded** (pri 3, **feature** — the second half of LOOP-SPEC §2's named capability gap; impl `1432a2e` harvest-completed by the cycle-16 orchestrator from a 50/50-budget-aborted glm child, merge `1e82ffb`, row flip + this entry in the companion commits). Recovery per the T29 precedent: worktree `/tmp/chug-loop-t37` preserved at `1432a2e` → gates re-run (430+3 in 14.35s, clippy `-D warnings` clean) → kimi adversarial validation (REQUIRED — src/tools.rs) launched via delegate at the T32-widened 50/30 budget → **VERDICT: PASS 48/50** (budget_low@8 fired at 42, wrapped inside T18's margin — third consecutive child to finish in the ceiling zone this week, all surviving on the margin). Validator: all 7 spec reqs verified (schema, transport bounds, content handling, errors-as-tool-errors, sandbox candor, README integration, no new dep); risk gate judges bash only (driver.rs:621-623); events.jsonl ToolResult preview on every fetch (driver.rs:670); **10/10 mutants killed** — cap-20k→50k, ceiling-clamp removal, redirect `>=`→`>`, scheme-gate drop, binary-refusal→accept, script/style-strip off, truncation-branch removal, non-2xx-arm removal, marker-text corrupt, whitespace-collapse break — each died to exactly the expected test(s); gates green pre- AND post-mutations, tree restored clean. 3 non-blocking observations: (1) req-7 out-of-scope list not enumerated in the impl commit message (the merge commit records it); (2) connect-timeout constant not value-pinned by a test; (3) `skip_raw_block` prefix-matches a closing `</scriptx>` tag (harmless — swallows to the real close or EOF, no content leak). Merged `1e82ffb`; gates re-run independently in main post-merge (**430+3 green in 15.12s**, clippy clean); todo_consistency guard green post-flip. 2 artifacts harvested pre-removal (`events-t37-validate-20260926-043809.jsonl` + `LEDGER-t37-validate-20260926-043809.md`; impl events harvested in cycle 16). **The capability gap named in LOOP-SPEC §2 is now CLOSED — both halves shipped (delegate T23, web_fetch T37).**

**Skipped/deferred:** none — the queue is **drained** (T1–T37 all done with refs). A cold next cycle therefore cannot freshness-skip (the rule requires `todo` rows) and MUST run a fresh Phase-1 eval — the designed flow.

**What the validators caught:** no implementation defects — the kimi validator's 10/10 mutant kill confirmed the suite has teeth; its 3 observations are documentation/pinning nits, recorded above for the next eval's consideration (a const-value pin for the connect timeout is a candidate micro-row if the eval judges it worth filing).

**Cycle-level notes:** (a) **The recovery recipe worked exactly as written** — cycle-16's deferred-with-ready-state handoff (T29 precedent) consumed cold: worktree intact, gates re-run, validation dispatched, merged, flipped, pushed in ~15 min wall-clock and ~20 orchestrator iterations. Deferred-with-ready-state is now 2-for-2 (T29, T37). (b) **The two-eyes doctrine held across a child death**: the dying glm child shipped 2 real production bugs (every-fetch `read_body_capped` panic; `<!DOCTYPE` leak) that the cycle-16 orchestrator caught in harvest-completion review and the cycle-17 kimi validator then confirmed fixed — orchestrator-review + adversarial-validation is the load-bearing pair for the harvest-completion path. (c) **delegate+wait_secs polling profile**: validator lifecycle (48 iters, ~5 min) observed with 4 instant/wait_secs status calls + 3 bounded bash sleeps; the 2s early-wake pattern on a churning stream noted again — instant polls after sleeps remain the cheap cadence for active children. (d) Budget: wrap begins at ~25/120 iterations used — the freshness-skip + single-recovery-item cycle is the cheapest profile the loop has; T36's 160-cap (awaiting the operator's loopd restart) is sized for the fresh-eval + 3-item cycles, not this one.

**Final state:** main at the `todo:`/`eval:` companion commits atop merge `1e82ffb`; gates 430+3 green in main, clippy `-D warnings` clean; TODO.md truthful (T1–T37 ALL done with refs — zero `todo` rows); 3 t37 artifacts in `.chug/` (impl events from cycle 16 + validate events + validate LEDGER); README gate satisfied — T37's Tools bullet + prose sentence landed integrated in the Tools section with its merge (verified in the diff, req 6); everything pushed. **Handoff to the next cycle: queue EMPTY → freshness rule CANNOT fire → the next cycle MUST run a fresh Phase-1 eval** (META-META-SPEC §6 README-audit duty included — the Tools section gained a tool this cycle, so the audit re-verifies that section's integration). Carries for that eval: (1) the validator's 3 T37 observations (connect-timeout const pin candidate); (2) glm truncated-write death class watch (cycle-16 note — if it recurs, a write_file chunking hint row); (3) T31-residual flake watch (1 sighting / ~8 runs at cycle 16; 0 new sightings this cycle — 2 suite runs, both green); (4) the web_fetch organic-demand watch (the 6-quiet-eval grep now has a tool to measure adoption of — the eval's grep should now look for `web_fetch` tool calls in child streams); (5) human-decision carries unchanged (loopd restart activates T36's 160-cap).

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
