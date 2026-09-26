# EVALUATION — chug, assessed by chug-loop (2026-09-26, cycle 24)

Corpus: `.chug/eval-digest.md` FIRST (regenerated 08:58:33Z at eval start —
it was stale, generated 07:29:57Z before cycles 22–23 ran; now 101 event
streams / 4,137 iterations), then the two orchestrator streams since the
cycle-22 eval: `.chug/events-20260926-080814.jsonl` (**cycle 22**: 119/120
iters, 37m40s, mandatory fresh eval + T48 landed + T52 filed at wrap; 0 tool
errors; budget_low@8; goal accepted) and
`.chug/events-20260926-085745.jsonl` (**cycle 23**: 118/120, 48m29s,
freshness-skip, T49+T52+T50+T51 landed — third T44 overlap; 1 benign tool
error — a git-add-ignored refusal, self-recovered; budget_low@8; goal
accepted), the 10 child streams t48–t52 (glm impls 25–37 iters, all
first-try goal-accepted; kimi validators 17–41 iters, all first-round
PASS), the anomalous 2-iteration validator stream
`.chug/events-t48-validate0-20260926-074801.jsonl` (I2 below),
`.chug/loopd/loopd.log` (cycles 22–23 OK unattended; this cycle started
08:57:45Z), `TODO.md` (T1–T52 all done with refs — queue EMPTY at cycle
start → this eval mandatory), git log `c7d608b` (cycle-23 wrap), `src/`
(22,060 lines, +34 since cycle 22), `README.md` (294 lines — §6 audit
below), and the cycle-23 wrap's three written carries (dispositions in §2:
(i) → I1/T53, investigated live and root-caused; (ii) → I3/T54; (iii)
re-checked, no action, closed).

## 1. What chug does well

- **The queue emptied itself honestly.** 52 rows landed with refs since
  2026-09-20, zero lost-work deaths since cycle 16, and the freshness
  rule's "cannot fire → mandatory eval" branch executed exactly as
  designed — this eval exists because of it.
- **Child quality is at a local maximum.** Every t48–t52 child, impl and
  validator alike, finished goal-accepted on its FIRST attempt: 5/5 glm
  impls first-try green (25–37/50 iters), 4/4 kimi validators first-round
  PASS (T49 17/50, T52 35/50, T50 41/50, T48 28/50). Mutation tallies:
  4/4, 10/12 (2 spec-sanctioned doc survivors), 8/8, 11/11.
- **Doctrine composed without friction.** Cycle 23 ran the third T44
  overlap (T51 impl ∥ T50 validator, disjoint files, strictly serial
  merges) and the T50 validator was the first consumer of
  `target-shared-validate` under T52's ALWAYS rule — doctrine written one
  cycle, dogfooded the next.
- **The wrap margin held.** Both orchestrators wrapped at budget_low@8
  with every artifact committed — T18's margin has now absorbed eleven
  consecutive wraps.
- **The digest carried the corpus read again** (second dogfood): this
  eval reached the writing phase in ~16 iterations against the 45–55
  pre-T46 estimate.

## 2. Incidents worth fixing

**I1 — loopd's single-driver check is DEAD on this host: pgrep
persistently cannot see the launchd-spawned loopd tree (NEW ROW T53, pri
1).** The cycle-23 T50 validator observed host pgrep missing the
long-running chug; this eval reproduced and characterized it LIVE (I am
pid 37073, `./target/debug/chug run --spec LOOP-SPEC.md`, child of loopd
pid 90114, parented to launchd pid 1):

- `pgrep -f "chug run --spec LOOP-SPEC.md"` → no match (exit 1);
  `pgrep -f "chug run"`, `pgrep -f "target/debug/chug"`, `pgrep -l chug`
  (name match), and even `pgrep -P 90114` (parent match — pattern-free)
  ALL miss pid 37073. A full-list diff (`pgrep -f .` vs `ps -ax`) shows
  37073 AND 90114 absent — **9/9 invocations over ~10 minutes**, while
  `ps -ax` sees both every time (argv intact, 592 bytes).
- The blindness is process-specific and persistent for this tree, not
  pattern-specific: a `nohup sleep 45` spawned from my own shell (itself
  inside the loopd tree) matched `pgrep -f "sleep 45"` instantly.
- pgrep's enumeration is also UNSTABLE for other processes: the visible
  count flapped 1298→1295→1293 across minutes, with boot-time daemons
  (pids 343/344) absent from one listing and present in the next.
- Consequence: `loopd.sh:98` (`pgrep -f "chug run --spec LOOP-SPEC.md"`)
  has matched exactly once in the log's history (00:44:00Z, ~20s after
  the supervisor started — plausibly a terminal-launched manual run,
  outside the invisible tree). Against any driver inside the launchd
  tree the guard is dead code → the single-driver guard **fails open**
  (duplicate supervisors/drivers possible). No collision has fired; the
  deadness is the finding. LOOP-SPEC's orchestrator-level hard-rule check
  is unaffected in practice (it is a model-run `ps` inspection, and `ps`
  works — verified this eval).
- Candidate fix (specced as T53): a `ps`-based check —
  `ps -ax -o command= | grep -q "[c]hug run --spec LOOP-SPEC.md"` — ps is
  proven to see the tree; the `[c]hug` idiom excludes the grep itself;
  `tests/loopd_reexec.rs:80`'s positional pin currently pins the pgrep
  string's existence and must be updated, not deleted.

**I2 — kimi validator natural-stopped after 2 iterations with no verdict
(watch item, not a row).** `events-t48-validate0-20260926-074801.jsonl`:
2 iterations, 4 seconds, 2 bash calls (git status/log displaying the impl
commit), then end_turn with no `goal_complete`. The cycle-22 orchestrator
detected it via `delegate status` (state done, no goal flag) and
relaunched — the T24/T28/T29 observability stack worked as designed; cost
≈ 2 orchestrator iterations + one 4s launch. First sighting of this
class; the recovery path is already doctrine. If it recurs, the fix is a
one-line LOOP-SPEC step-2 note ("a done child with no goal flag is a
no-verdict natural stop — relaunch, never count it as a review"), not
code.

**I3 — duplicate-`SELF_CKSUM` additive mutant survives all four T50 pins
(NEW ROW T54, pri 3, hardening).** The T50 validator's one non-blocking
finding: an additive second `SELF_CKSUM="$(cksum …)"` assignment inside
the while body silently defeats the re-exec mechanism (the fingerprint
refreshes every cycle, so the while-top comparison never fires) with all
4 positional pins green — the pins assert existence + relative order,
never counts. Exact-count pins per the T47-carrier doctrine close it:
`SELF_CKSUM=` assignments == 1, `!= "$SELF_CKSUM"` comparisons == 1,
total `SELF_CKSUM` occurrences == 2. Baseline verified this eval
(loopd.sh:75 assignment, loopd.sh:93 comparison — exactly one each).

**Carries disposition (cycle-23 wrap):** (i) pgrep-blindness → I1/T53 —
investigated live, root-caused, row filed; (ii) duplicate-`SELF_CKSUM`
pin → I3/T54; (iii) T51 intro-reflow liberty → re-read req 4 and the
landed text: the liberty is confined to one intro clause, no drift
introduced — no action, closed.

**Watch items (all continue, none promoted):** glm truncated-write — 2
`output_truncated` hits this cycle (t50-impl 08:31:18Z, t52-impl
08:17:58Z), both children recovered to goal-acceptance, T38's advisory
working; T31-residual `dead_port_probe` flake — ZERO new organic
sightings (t48-validate's was the 2nd, already counted); bash
unquoted-paren syntax slips — none new (3 cumulative); `write_ledger`
hallucination — none new (1 cumulative); `web_fetch` organic adoption — 0
calls in both new orchestrator streams (capability demand-honest per
T37's filing; watch continues).

## 3. Friction hot spots

The digest's top recurring class across all 101 streams remains `tool
error: path escapes cwd: /tmp/…` (7 cumulative variants; t50-impl added
`/tmp/t50-commit-msg.txt` this cycle) — children in /tmp worktrees keep
reaching for /tmp scratch. Every occurrence self-recovered in one
iteration via the bash escape, and the tool descriptions already name
that remedy verbatim (T41 candor pass; verified `src/tools.rs:61–86`
this eval). Bounded cost + remedy-naming message → not a row; if it
climbs past ~1/child, the next lever is echoing the cwd in the error
itself. `timed out after 120s` (3 cumulative) — t50-impl's live-smoke
wait hit the bash cap mid-smoke and recovered; the perl-alarm idiom is
pinned in the bash description (T22). `sh: syntax error near unexpected
token '('` (3 cumulative) — unquoted-paren model slips, one-iteration
recoveries, watch continues. **No friction class crosses the row
threshold this cycle.**

## 4. Capability gaps — FEATURE SCAN (required)

Audited against the META-META-SPEC candidate classes: **parallel tool
calls — PRESENT** (`src/driver.rs:631` iterates every `tool_use` block in
the assistant content and collects all results into one user message;
verified this eval — the previously-open question closes). `delegate` —
present and heavily used (35/53/31 calls in the last three orchestrator
streams). `web_fetch` — present (T37; organic adoption 0, demand-honest).
MCP — stdio + streamable HTTP present (SPEC-7/9). Plan-then-execute — the
LEDGER is the plan surface. Session/handoff — `--resume` + rotation
(T3/T7). Steering — chat dock + `[operator]` notes.

**F1 — No harness-level same-cwd mutual exclusion: two `chug run`s in one
cwd corrupt each other's state files, and the only guards are doctrine +
one (dead, I1) supervisor line (NEW ROW T55, pri 2, FEATURE).**
META-SPEC's "Children MUST NOT share your cwd" is prose; the driver's
transcript/ledger/events appends have no in-harness protection when two
drivers share a cwd (operator error, a manual run racing loopd, or —
after I1 — a duplicate supervisor spawning cycles). A coding harness of
this class should enforce its own state-file exclusivity: a
`.chug/driver.lock` (pid + start time), acquired by `chug run` before the
first transcript append, refusing when the holder is alive AND still a
chug process (ps-argv check — the I1 lesson applies twice: pid-liveness
alone false-refuses on PID reuse, and the argv read must use `ps`, not
`pgrep`), stale locks (dead pid, argv mismatch, malformed file) reclaimed
by the acquirer, released best-effort on exit; `chug chat` never
acquires and never refuses (LOOP-SPEC's chat-does-not-block hard rule);
`--resume` transparently reclaims its dead predecessor's lock. T50's
pidfile liveness guard is the in-repo pattern precedent; T28's
`reap_and_alive` and T20's `resolve_head` are the seam-style precedents.
This composes the single-driver stack into defense-in-depth: operator
discipline → loopd ps-check (T53) → in-harness enforcement (T55).

Other candidates weighed and rejected: delegate-status no-verdict note
(I2 — status already surfaces state + goal flag; the inference is one
step; watch instead); MCP resources/prompts beyond tools (zero demand
signal in the corpus); web_fetch POST/HEAD (no demand); a plan-file mode
(LEDGER covers it); notifications/hooks (operator-UX sugar, no failure
demand).

## 5. Top 3 priorities

1. **T53 (pri 1, bug)** — the single-driver guard is dead code on the
   production host RIGHT NOW and fails open silently. One-line ps-based
   fix + pin update; the cheapest real-bug fix on the board.
2. **T55 (pri 2, feature)** — closes the mutual-exclusion capability gap
   this eval's feature scan mandates hunting; composes with T53 into a
   defense-in-depth stack; in-repo pattern precedents (T50/T28/T20) are
   fresh.
3. **T54 (pri 3, robustness)** — exact-count pins close the one known
   survivor class in T50's mutation testing; bundle-eligible with T53
   (same 2 files, each ≤ ~30 lines, pri ≤ 3, no core-loop touch).

## 6. README audit (usability, not just accuracy)

Cold read top-to-bottom (294 lines). **(a) Reading order:** what-it-is →
quickstart → interactive → autonomous → TUI → tools → risk gate → MCP →
Langfuse → self-hosting → loopd → development — the newcomer arc holds;
no append-only accretion visible post-T51. **(b) Redundancy:** the
events-log bullet re-explains `output_truncated` ("one `output_truncated`
line per injected advisory — a response hit the API output-token ceiling
and the loop named the chunking remedy") ~30 lines after the
Truncated-output advisory bullet defined the same mechanism — stated
twice with paraphrase-drift risk (NEW ROW T56, pri 4, with (c)). **(c)
Staleness:** the loopd section documents the pidfile refusal, HALTED, the
digest refresh and target-shared caching but NEVER mentions T50's
self-re-exec — an operator reading "Continuous mode" cannot learn that
script edits now activate between cycles without a restart, or why the
supervisor logged a re-exec (the T50 merge explicitly deferred README to
"in-script + spec", leaving the user-facing section silent on
user-visible supervisor behavior). **(d) Balance:** acceptable — post-T51
the delegate paragraph carries user-facing semantics only. **(e)
Quickstart truth:** commands verified against main (`cargo build`,
`cargo install --path .`, all `chug run` flags current; the `check:` line
behavior is accurately described).

## Handoff — recommended execution order

1. **T53 + T54 — ONE impl child under the T45 trivial-row bundle rule**
   (conjunctive eligibility verified: (a) each ≤ ~30 changed lines — T53:
   one loopd.sh line + ~3 pin lines; T54: ~8 test lines; (b) same 2 files
   — `loopd.sh` + `tests/loopd_reexec.rs`; (c) neither touches
   src/driver.rs or src/api.rs; (d) pri 1 and pri 3, both ≤ 3). ONE
   commit per row, in queue order. kimi validation REQUIRED (loopd.sh is
   supervisor tooling — T50 precedent) covering the bundle, with
   mutation-testing of both the new check line and the count pins.
2. **T55 — feature, runs alone** (touches driver startup — never bundles;
   REQUIRED kimi validation with mutation testing of the refuse/reclaim
   legs). Its impl MAY overlap the bundle's validator under T44 (files
   disjoint: src/+tests/+README vs loopd.sh+tests/loopd_reexec.rs);
   merges stay strictly serial in queue order.
3. **T56 — docs, orchestrator gates only** (kimi skipped per
   T16/T31/T35); MAY overlap the bundle's validator under T44 (README.md
   disjoint from the bundle's files) but NOT T55 (both touch README.md).
Human-decision items: none new. The standing restart-loopd carry is now
self-terminating: T50's re-exec is merged (`078cb77`), so the NEXT
restart is the last manual one — every supervisor started after that
merge self-updates between cycles.

## Outcomes (filled at cycle wrap — LOOP-SPEC Phase 3)

### Cycle 25 (2026-09-26, ~05:21–05:45 EDT) — freshness-skip; T55 recovered + landed (driver.lock feature) + T56 landed (README) — queue DRAINED

**Landed:**
- **T56 — README: loopd self-re-exec documented + output_truncated gloss deduped** (pri 4, docs-only; impl `34554e4` glm 17/50 goal-accepted ~2.5 min first-try — recovered instantly from 2 edit_file errors caused by an ASCII-vs-Unicode-arrow mismatch; merge `611af46`; flip + this entry per T34). Continuous-mode section gains one integrated sentence in the state/pidfile/HALTED paragraph: between cycles the supervisor fingerprints its own script and re-execs itself when the file changed, so merged loopd edits activate without an operator restart — a pending stop still wins (no cksum/same-pid mechanism detail, per spec). Events-log bullet drops the repeated `output-token ceiling` mechanism gloss — the Truncated-output advisory bullet stays the single definition (the string now occurs EXACTLY once); everything else byte-identical (2 hunks, README only). Docs-only → kimi round skipped per T16/T31/T35/T51 + the cycle-24 handoff plan; orchestrator gates independently re-run 501 + clippy, spec check verbatim PASS (ceiling-count==1, re-exec present, 'one exception' absent). 2 artifacts harvested pre-removal (impl events + delegate log; LEDGER trivial).
- **T55 — .chug/driver.lock: same-cwd mutual exclusion for concurrent chug runs** (pri 2, FEATURE — in-harness layer of the defense-in-depth: doctrine → loopd skip → this lock; recovered per its own row recipe from the cycle-24 preserved worktree: glm impl pid 55328 had died 50/50 with the work complete but UNCOMMITTED and 3 compile/clippy fixes short — `drive_turn`→`drive_loop` + the `initial_spec: None` arg in the chat-exemption test, clippy `single_match`→`if let` in acquire, one needless borrow; orchestrator finished, impl `cb5906f`; merge `2e7945b`; flip + this entry per T34). New `src/driver_lock.rs` (495 lines): `holder_status` pure decision matrix behind injectable `pid_alive`/`argv_names_chug` probe seams (T28/T20 style); refusal ONLY on the alive+chug double-positive (kill(pid,0) AND `ps -o command= -p <pid>` names chug — NEVER pgrep, cycle-24 eval I1); every other leg (dead pid, argv mismatch = PID-reuse, malformed/empty/unreadable, probe error) degrades to reclaim, never aborts (T20 never-fail); RAII Guard compare-then-delete release on every normal exit, SIGKILL-stale by design; acquire at run_loop top BEFORE the T3/T7/T10 rotations; chat structurally exempt (never reaches run_loop). 20 new tests (16 module + 4 run-path incl. real-sleep reclaim, SIGKILL-stale reclaim, release-on-goal-acceptance + same-cwd successor, chat never-creates-never-removes). kimi VERDICT: **PASS** 45/50 (target-shared-validate per T52 ALWAYS; budget_low@8 fired at 42, verdict delivered inside the T18 margin) — all 9 reqs hand-verified, gates independently re-run green, 6 mutants: 4 KILLED (drop-argv-leg incl. the run-path pin, drop-compare-then-delete, remove-acquire-call → 2 run-path tests, acquire-in-drive_loop → chat pin has teeth), 2 SURVIVORS in the refuse-wiring/refusal-message cluster that the spec explicitly delegates to hand-verification — which the validator then performed with the REAL binary: live chug-argv holder → second run exit 1 + pid/remedy on stderr + zero transcript/events writes + holder unaffected; stale dead-pid lock → reclaimed <1s. 3 non-blocking findings carried to the next eval: (i) module-comment says a static pin in `tests/` guards chat, the pin is actually a unit test (doc drift); (ii) acquire's read-then-write window is non-atomic (latent, spec-silent — bounded 3-attempt re-verification mitigates); (iii) PRE-EXISTING mcp_http dead_port_probe flake under parallel load (passes isolated 3/3 — the T31 watch item). **Post-merge gates incident:** first main gate run showed loopd_reexec 3/4 FAILED — a T52-class FALSE RED: the step-3 worktree gates (target-shared, no child in flight) had compiled the worktree's PRE-T53/T54 4-test loopd_reexec.rs into the shared artifact slot; the merge didn't touch that file (old mtime) so cargo reused the stale binary against main's ps-based loopd.sh (needle mismatch). touch+rebuild recovered (T43's incident class, second sighting — now SEQUENTIAL not just concurrent): main 501 (473+6+9+1+9+3) green + clippy clean. 3 validate artifacts harvested pre-removal (impl's 3 harvested cycle-24).

**Cycle notes:** (a) both queued rows landed, queue DRAINED — the freshness rule CANNOT fire next cycle (no todo rows): next cycle runs a MANDATORY fresh Phase-1 eval. (b) T55's post-merge false red is a NEW VARIANT of the T52/T43 stale-artifact class — SEQUENTIAL, not concurrent: step-3 worktree gates compiled the worktree's pre-T53/T54 4-test loopd_reexec.rs into the shared slot; the merge didn't touch that file (old mtime) so cargo's mtime freshness reused the stale binary against main's ps-based loopd.sh. Role-keyed dirs govern CONCURRENT builds only. Carry to next eval: post-merge main gates need a staleness flush (touch tests/ or a rebuild pin) whenever the worktree gates ran first in the same shared dir. (c) T55's kimi validator found the refuse-wiring/message mutant cluster untestable from the suite — spec-sanctioned hand-verification, which it performed with the real binary (exit 1 + pid/remedy, zero appends, holder unaffected; stale reclaim <1s); its 3 non-blocking findings (module-comment tests/-pin drift, non-atomic acquire window spec-silent, PRE-EXISTING mcp_http dead_port_probe flake — T31 watch item, passes isolated 3/3) are carried to the next eval. (d) T56's full arc cost ~7 min wall (worktree → impl 2.5 min → review/gates → merge → flip) — the T45-class fixed cost on a 2-hunk docs row remains the queue's dominant per-row overhead. (e) T43 cycle-18 double-heading compaction DEFERRED again (budget wrap): two `### Cycle 18` headings in Outcomes need one careful manual merge — hygiene, not loss, full narrative in git. (f) Final main gates 501 (473+6+9+1+9+3) + clippy green at the T56 flip; todo_consistency 3/3 green pre-commit.

### Cycle 24 (2026-09-26, ~04:57–05:19 EDT) — MANDATORY fresh eval (queue was EMPTY); T53+T54 bundle landed; T55 mid-arc at budget (recovery recipe on its row); T56 deferred unworked

**Deferred:** **T55** (pri 2 feature) — glm impl died 50/50 with the work UNCOMMITTED in the preserved worktree /tmp/chug-loop-t55 (README.md, api.rs, driver.rs, main.rs modified + new src/driver_lock.rs); 3 artifacts harvested (events/LEDGER/delegate.log 20260926-091931); full recovery recipe on the TODO row. **T56** (pri 4 docs) — never dispatched (budget); row stays `todo` with its ready spec. **Cycle notes:** (a) the eval's live I1 reproduction is the cycle's payload — pgrep's blindness is persistent (9/9), process-specific (in-tree nohup'd sleep matched), and its enumeration flaps — the ps fix landed in T53 within the hour; (b) T45's bundle rule executed for the first time: one child, two commits in queue order, one kimi round, one flip commit — arc cost ~7 min wall vs two full serial arcs; (c) T44 overlap executed for the fourth time (T55 impl during the bundle validator, disjoint files); (d) the bundle validator's live smoke doubled as independent confirmation that the new ps pipeline sees the exact process class pgrep missed (the orchestrator itself, pid 37073, ONLY match); (e) wrap fired at budget_low@8 with the T55 impl's abort arriving mid-merge — the per-item Outcomes discipline (T34) meant T53+T54's narrative was already committed before the wrap; (f) T56's deferral leaves the README loopd section silent on re-exec for one more cycle — harmless, the spec is ready. **Final state:** main pushed through the wrap commit; gates 481 (453+6+9+1+9+3) green at the T53+T54 merge, `bash -n` clean; todo_consistency 3/3 green pre-commit; QUEUE: T55 in-progress (recipe on row), T56 todo — next cycle skips Phase 1 (this eval is same-day) and resumes T55 from the worktree. The standing restart-loopd carry is now self-terminating (T50 merged; the next restart is the last manual one — and T53's ps-based guard rides with it).

**Landed:**
- **T53 — loopd single-driver check goes ps-based (pgrep dead on this host)** (pri 1, BUG; T45 bundle with T54 — conjunctive eligibility verified at eval: ≤30 lines each, same 2 files, no core loop, pri ≤3; impl `5c842a6` glm 24/50 first-try goal-accepted; merge `8624078`; flip + this entry per T34). The guard (`pgrep -f "chug run --spec LOOP-SPEC.md"`) was dead code on the production host — the cycle-24 eval reproduced the blindness LIVE 9/9 (pgrep -f/-l/-P all miss the launchd-spawned loopd tree: the orchestrator pid 37073 and supervisor pid 90114 absent from pgrep's full list while `ps -ax` showed both, argv intact; enumeration also flapped for other processes; the skip line fired exactly once in supervisor history, against an out-of-tree driver) — so the single-driver guard failed OPEN. Fix: `ps -ax -o command= | grep -q "[c]hug run --spec LOOP-SPEC.md"` (bracket idiom excludes the grep's own argv; supervisor argv holds no needle; chat doesn't match by design), skip body (log line/sleep 120/continue) byte-identical, why-comment citing eval I1; tests/loopd_reexec.rs: driver_check anchor re-pinned (T50 positional assertion preserved verbatim) + 2 pins (pgrep form GONE — a revert fails the suite; bracket idiom present + needle exactly-once). kimi VERDICT: **PASS** 24/50 (target-shared-validate per T52 ALWAYS) — gates independently re-run 481 (453+6+9+1+9+3) + clippy + `bash -n`, 5/5 T53 mutants killed (restore-pgrep, drop-bracket, corrupt-needle, delete-pipeline, duplicate-block), live smoke independently re-demonstrated (real driver pid 37073 matched as the ONLY match; negative leg exit 1), worktree restored byte-clean, both spec checks PASS verbatim. Post-merge main gates 481 + `bash -n` green. Bundle artifacts harvested under `t53t54` names pre-removal.
- **T54 — loopd_reexec.rs exact-count SELF_CKSUM pins (additive-mutant survivor closed)** (pri 3, robustness; T45 bundle with T53; impl `6587b22`; merge `8624078`; flip + this entry per T34). The T50 validator's non-blocking finding (cycle-23 carry ii): an additive second `SELF_CKSUM=` assignment inside the while body silently defeats the re-exec (fingerprint refreshes every cycle → while-top comparison never fires) with all 4 positional pins green — they asserted existence + order, never counts. Three exact-count pins per the T47-carrier doctrine: `SELF_CKSUM=` assignments ==1, `!= "$SELF_CKSUM"` comparisons ==1, literal occurrences ==2, each failure message naming the count + the defeat mechanism; tests-only, loopd.sh untouched. kimi VERDICT: **PASS** 24/50 covering the bundle — the ORIGINAL SURVIVOR now dies (additive 2nd assignment → pins 1+3), additive 2nd comparison → pins 2+3, delete-assignment → pin 1 + the pre-existing fingerprint pin; 8/8 bundle mutants killed total; the four pre-existing T50 pins stayed green throughout. Post-merge main gates 481 green.

### Cycle 23 (2026-09-26, ~04:08–04:56 EDT) — freshness-skip; T49+T52+T50+T51 landed (4/4 queued rows); QUEUE EMPTY → next cycle MANDATORY eval

**Landed:**
- **T49 — api.rs LLM client gains a 10s connect timeout const + value pins** (pri 2; cycle-22 impl `56ecf72` glm 34/50 + orchestrator review; this cycle: kimi validation, merge `9a73b12`, flip + this entry per T34). `CONNECT_TIMEOUT_SECS = 10` const with the class doc + `.connect_timeout` builder leg + 3-value pin test; `classify_reqwest` unmodified (is_timeout/is_connect → retryable, verified incl. reqwest 0.12.28 from Cargo.lock). kimi VERDICT: **PASS** 17/50 — all 5 reqs verified, gates independently re-run (468 passed / 0 failed), 4/4 value mutants KILLED with the panic naming the correct const; the `.connect_timeout`-leg deletion mutant SURVIVES as the spec-sanctioned evidence level (T16/T42 precedent: unobservable without a SYN-dropping endpoint; pins pin values, not wiring); tree restored byte-clean. Post-merge main gates 453+6+1+6+3 + clippy green. 2 validator artifacts harvested pre-removal.
- **T50 — loopd re-execs itself between cycles when its own script changed** (pri 2, FEATURE — closes the activation-lag gap: T36/T46/T47 sat dormant 7 cycles in the running supervisor; impl `ca11c6b` glm 37/50 goal-accepted; merge `078cb77`; flip + this entry per T34). `SELF_CKSUM` cksum fingerprint before the cycle loop (content-based — `touch`/content-preserving checkout never re-execs); compare→log→`exec "$ROOT/loopd.sh" run` as the FIRST while-body statement (re-exec only BETWEEN cycles, zero incremental-read exposure; STOP wins at the while condition, never cleared by this path); pidfile guard refuses only a LIVE pid ≠ `$$` (same-pid pass for the exec'd self; stale-pid fall-through and foreign-supervisor refusal kept); tradeoffs documented in-script; tests/loopd_reexec.rs +144 (4 positional static pins — before-the-loop, first-statement ordering, log-before-exec, guard shape + liveness). Impl ran the spec's FULL optional live smoke: re-exec line observed at the next while-top ~58s after a comment append, same pid, no refusal, STOP leg clean, scratch removed; 2 non-vacuousness mutants killed. kimi VERDICT: **PASS** 41/50 (first validator on `target-shared-validate` per T52's ALWAYS rule) — gates independently re-run 476 + `bash -n`, 8/8 spec-shaped mutants killed (exec deletion, guard revert, `!=`→`=`, fingerprint deletion, log deletion, `kill -0` drop, block relocation, log/exec swap), live smoke independently re-demonstrated (re-exec 103s after append, same pid 29636, STOP landed clean), reqs 1–6 verified, tree clean. Non-blocking: (a) hardening suggestion — an additive duplicate-`SELF_CKSUM` mutant at the while-body end silently defeats the mechanism with all pins green (exact-count pin candidate, T47-carrier doctrine); (b) observation carried to the next eval — the host's `pgrep` could not see the long-running real chug during validation while `ps` showed it (pre-existing single-driver-check exposure; loopd's skip line uses `pgrep -f`), and the live supervisor still launches `--max-iters 120` (T50's premise confirmed in the wild). Post-merge main gates 453+6+4+1+9+3=476 + clippy green, `bash -n` clean. 4 artifacts harvested pre-removal.
- **T51 — README delegate paragraph: stale "one exception" killed + spec-grade trim** (pri 4, docs-only; impl `dd024de` glm 26/50 first-try; merge `372f920`; flip + this entry per T34). The stale "Delegate paths are the one exception to cwd sandboxing" sentence (drift that moved rather than died at T41) is replaced by the short consistent clause (`cwd`/`spec` absolute, may target child worktrees outside your own); process-group/SIGHUP/nohup-parity spawn semantics and the bounded-tail-read mechanism trimmed (canonical home: specs/t23-delegate-tool.md); every user-facing semantic in req 2's keep-list verified present (two actions, args + 40/35 defaults, `--max-tokens` passthrough, returns-at-spawn, status liveness/events/log-tail, wait_secs early-return + 600s max + `waited:` line, bash owns the worktree lifecycle); README only, net −3 lines. Spec check PASS in main (`! grep "one exception" && grep "two documented exceptions"`). Orchestrator gates 453+6+1+9+3 + clippy + full diff review; kimi round skipped per T16/T31/T35 (docs-only). Landed under T44 overlap with T50's validator (disjoint files: README.md vs loopd.sh/tests); merge held strictly serial after T50. 2 artifacts harvested pre-removal.
- **T52 — role-keyed target dirs (T44-overlap × target-shared race fix)** (pri 2, doctrine, ran alone; impl `bdba301` glm 28/50 first-try; merge `4d4c383`; flip + this entry per T34). LOOP-SPEC: step-3 gates dir conditional (`target-shared` no-child / `target-shared-gates` in the overlap window incl. N+1's impl during N's post-merge gates) + the mechanism sentence (metadata hash excludes checkout path → same artifact name → last-builder-wins); step-4 validator export `target-shared-validate` ALWAYS; step-5 pointer. `.gitignore` gains both siblings contiguous (position-pinned); README T47 paragraph gains the integrated sibling-cache clause; loopd.sh + META-SPEC untouched; tests/shared_target_dir.rs +3 pins (6→9 — exact-count carriers are strictly stronger than the spec's `≥2`: a duplicated-carrier mutant dies). kimi VERDICT: **PASS** 35/50 — all 6 reqs verified, gates independently re-run 453+6+1+9+3, 10/12 mutants killed (2 survivors spec-sanctioned doc carriers: step-5 reminder sentence + README clause, same class as T44's prose survivors), tree byte-clean; the validator dogfooded the rule under test by building into `target-shared-validate` itself (auto-created, cold 27s, now warm). Post-merge main gates 453+6+1+9+3 + clippy green. 5 artifacts harvested pre-removal (impl stream was T7-rotated at the validator's reseed — both recovered).

**Cycle-level notes:** (a) **T52's role-keyed dirs dogfooded immediately** — the T50 validator was the first launched under the new ALWAYS rule (`target-shared-validate` — auto-created cold at T52's own validation, warm since); T44 overlap practiced for the third time (T51 impl during T50's validator; disjoint files README.md vs loopd.sh/tests; merges strictly serial — T50 merged before T51 despite T51 finishing first). (b) **What the validators caught:** nothing blocking — three first-round PASSes (T49 17/50; T52 35/50; T50 41/50). Mutation tallies: T49 4/4 killed (+1 spec-sanctioned wiring survivor), T52 10/12 (2 doc-carrier survivors), T50 8/8 (+1 additive duplicate-`SELF_CKSUM` survivor → hardening candidate: exact-count pin per the T47-carrier doctrine). (c) **T50 live leg, honestly recorded:** ZERO re-exec lines in `.chug/loopd/loopd.log` at wrap — expected: the running supervisor (started 00:43Z, predates T50) executes its while-loop from an already-parsed AST, so the merged re-exec code is inert in the current process; it activates at the operator's next loopd restart, and every supervisor started after this merge carries it. The cycle-22 premise stayed confirmed in the wild: this cycle still ran 120 iters, not T36's dormant 160. (d) **Carries for the next (mandatory) eval:** (i) T50-validator observation — the host's `pgrep` could not see the long-running real chug while `ps` showed it; loopd's single-driver skip uses `pgrep -f "chug run --spec LOOP-SPEC.md"`, so if the blindness is real and persistent the skip fails open (duplicate drivers) — investigate with `pgrep -fl` vs `ps` in a live window; (ii) duplicate-`SELF_CKSUM` exact-count pin hardening; (iii) T51's intro-line reflow liberty (req 4's "no other section" read strictly) — noted, no action. (e) Budget: wrap fired at budget_low@8 again — 4 items + one T44 overlap in ~48 min inside the 120 ceiling; T36's 160 stays dormant (see (c)).

**Final state:** main = `9301652` (merges: T49 `9a73b12`, T52 `4d4c383`, T50 `078cb77`, T51 `372f920`; flips `8079516`/`e50d1dd`/`abcc174`/`9301652`); gates 476 (453+6+4+1+9+3) + clippy --all-targets + `bash -n loopd.sh` green in main; TODO truthful — **QUEUE EMPTY** (T1–T52 all done with refs) → the freshness rule cannot fire → the next cycle MUST run a fresh Phase-1 eval; 13 child artifacts harvested into `.chug/` (t49 validate ×2, t52 ×5, t50 ×4, t51 ×2); all five loop worktrees removed and branches deleted (all merged, `git branch --merged` verified); README gate satisfied (T50's behavior change is documented in-script + its spec; T51's trim kept the Tools section truthful); pushed through `9301652`. **Handoff: mandatory fresh eval; carries in (d); T50 activates at the operator's next loopd restart.**


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
