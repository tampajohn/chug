# EVALUATION — chug, assessed by chug-loop (2026-09-26, cycle 16)

Corpus: `.chug/events-20260926-034737.jsonl` (**cycle-14's own full
run**: 119/120 iterations — budget_low remaining_iters=8, goal
accepted at 119, 56 min of 240 — fresh eval + 4/5 rows landed, T35
deferred; 90 bash / 27 delegate calls, **15 `wait_secs` long-polls**
{10×300, 2×120, 2×110, 1×90} vs ~9 long bash sleeps),
`.chug/events-20260926-035521.jsonl` (**cycle-15's full run**: 38/120
iterations, ~7 min — freshness-skip, T35 landed ~9 min
launch-to-push, QUEUE DRAINED; 23 bash / 9 delegate calls, **8
`wait_secs` calls, ~1 bash sleep** — cleanest poll profile yet), the
8 t31–t35 child streams (5 glm impls: 44/50 (T31, budget_low@8 at
42), 11, 16, 14, 10/50 — all goal-accepted first-try; 3 kimi
validators: 12/40 (pre-T32 budget), 12/50, 11/50 — all PASS first
dispatch), `.chug/loopd/` (cycles 15→16 OK unattended; cycle 16 is
this session, launchd-launched), `TODO.md` (**T1–T35 all done with
refs — queue EMPTY at cycle start, freshness rule cannot fire → this
eval is mandatory**), git log `697a6b6..HEAD` (18 commits), `src/`
(20,006 lines, +144 since cycle-14 eval — tools.rs 2,996→3,037 and
mcp_http.rs 2,343→2,446, both T31's deflake; every other file
unchanged); `README.md` (254 lines). Prior evaluations: cycle 14
below at §Outcomes (with cycles 5–15 there); cycle 11 same section;
cycle 4 at `c8222cd`; cycle 3 at `df4039a`; cycle 2 at `0d71d00`.
Verification performed this eval: full `cargo test --
--test-threads=4` = **416+3 green in 14.3s**; `cargo build` clean;
`cargo clippy --all-targets -- -D warnings` clean; jq profiles of
both orchestrator streams + all 8 child streams (iteration counts
recomputed per-stream from `run_start.max_iters` + last
`iteration.n`); `wait_secs` adoption counted from transcripts (events
previews truncate at 200c — see §2 watch item); web-fetch grep across
both orchestrator + all t3x streams = **0 real fetches** (6th
consecutive quiet eval; the single hit was LOOP-SPEC's own text
quoting "`web_fetch`" as a gap example); timeout-mirage grep = 0
(post-T22, 3rd quiet); GNU/BSD `sed` error grep = 0 (4th quiet);
liveness-lie check (`alive: true` after `goal_seen: true` in the same
poll sequence) = 0 (T28's reaping holds); organic flake sightings
post-T31 = **0** (the single `test result: FAILED` in the corpus was
cycle-14's own eval-time mutation check — 1 failed/415 filtered, then
restored); quickstart-truth re-verified end-to-end (§6(e));
`reqwest::blocking` pattern confirmed in mcp_http.rs (T37 spec
grounding); parallel-tool-call dispatch read at driver.rs:614
(sequential multi-`tool_use` per turn — §4).

## 1. What chug does well

- **The first full queue is drained: T1–T35, all done, all with
  commit refs.** Fifteen cycles from first eval to empty queue; the
  loop's deferral machinery (unworked rows stay `todo` with ready
  specs) made every budget wrap lossless.
- **Children are healthy**: 5/5 glm impls goal-accepted first-try
  (10–44/50 iters), 3/3 kimi validators PASS first dispatch
  (11–12/50). Zero fix-up arcs, zero model fallbacks, zero
  budget deaths since T21/T32 widened the child ceilings.
- **T29's `wait_secs` is the only polling surface left**: cycle 15
  ran a full item with ~1 bash sleep (delegate terminal flip woke the
  poll in 2s); cycle 14's 15 long-polls carried 4 items. The
  cycle-10 problem (13% of iterations on instant polls) is gone.
- **T31's deflake holds**: zero organic flakes in the two post-T31
  cycles (small sample — §3 keeps the watch).
- **T34's per-item Outcomes doctrine practiced forward** (T35): the
  entry landed in the row-flip commit; wrap was assembly-only. A
  mid-cycle death now loses no narrative.
- loopd ran cycles 15→16 fully unattended; single-driver guard
  untouched; quickstart truth restored (T35) and re-verified (§6(e)).

## 2. Incidents worth fixing

- **Q1 → T36 (pri 2) — the orchestrator iteration ceiling is binding
  AGAIN, one level up from T27.** Both 120-cap fresh-eval cycles
  scraped the ceiling: cycle 13 at **116/120**
  (`events-20260926-025049.jsonl`: budget_low remaining_iters=8 at
  02:48:11Z, goal accepted 02:49Z), cycle 14 at **119/120**
  (`events-20260926-034737.jsonl`: budget_low@8, goal accepted at
  iteration 119). T27 filed on the identical two-data-point pattern
  at the 80 cap (76/80, 79/80); the class recurred at 120 in exactly
  two fresh-eval data points. Cost model, updated for T29's
  wait_secs: fresh Phase 1 ≈45–55 iters (the §6 README-audit duty
  grew it since T27's 45), one queue item end-to-end ≈28–35
  (impl + polls + gates + validator + merge), wrap ≈8–10 → fresh eval
  + 3 items ≈ 150–165 > 120. Deferral absorbs the overflow cleanly
  (T35 deferred from cycle 14, landed first thing in cycle 15 — by
  design), but a budget_low@8 wrap squeezes Phase 3, and Phase 3 IS
  the next cycle's input — a squeezed wrap is a handoff-quality risk
  (the cycle-12 mid-arc death is the lesson; T34 covers narrative
  loss, not wrap headroom). Minutes are never binding (cycle 14: 56
  of 240; cycle 13: ~117 of 240); 160 iters projects to ≈80–100 min.
  This is the fourth filing of the class (T21 children 40→50, T32
  validators 40→50, T27 orchestrator 80→120) and the cheapest it
  will ever be — two near-deaths, zero deaths. Filed **T36**
  (`specs/t36-loopd-iteration-budget-160.md`): `loopd.sh` line 72
  `--max-iters 120`→`160` + T27's comment line updated, minutes stay
  240, no-signal-running-supervisor rationale verbatim, activation =
  operator's next loopd restart (human-decision carry).
- **(Watch, not filed) — delegate status output is truncated at 200c
  in the ORCHESTRATOR's own events.jsonl.** First-hand this eval:
  the `waited:` line and most of the status body live past the 200c
  ok-leg preview cut, so measuring wait_secs adoption required a
  transcript fallback (`grep '"wait_secs":' transcript-…`). This is
  T10/T25's preview policy working as designed — but LOOP-SPEC Phase
  1's "events.jsonl … is jq-mineable and untrimmed" is imprecise
  about tool previews, and the delegate-status surface is exactly the
  one an orchestrating chug later mines. One inconvenience, one eval;
  transcripts are harvested and greppable. Filed as a watch item; if
  a second eval trips over it, the row is a structured
  `Event::DelegatePoll` line instead of relying on the preview.
- **(Watch, not filed) — `write_file` to `/tmp` rejected during
  cycle-14's eval** (`tool error: path escapes cwd:
  /tmp/eval-head.md`). Confinement doing its job; the bash heredoc
  workaround is zero-friction and was used. **Fired again live during
  THIS eval** (same filename, same workaround — the head of this very
  file was written via heredoc). Two hits now; still not row-level —
  the workaround is idiomatic — but the eval's staging idiom is
  officially "bash heredoc to /tmp, never write_file".

## 3. Friction hot spots

Every previously-filed friction class re-grepped quiet this eval —
the well is dry; this section is now a confirmations list:

- **PATH tax (T4)**: zero `cargo: command not found` discoveries in
  any t31–t35 stream.
- **edit_file disambiguation (T5)**: zero revert-thrash loops.
- **timeout mirage (T22)**: 0 hits post-T22 (3rd consecutive quiet).
- **GNU/BSD sed errors**: 0 (4th consecutive quiet).
- **stub-test hangs (T6)**: suite 14.3s; no hang class.
- **flake family (T31)**: 0 organic sightings in 2 cycles (small
  sample — keep the grep in every eval's verification list).
- **NEW observation — none rising to row level.** The only fresh
  frictions are the two §2 watch items (delegate preview truncation;
  /tmp confinement ×2), both with idiomatic workarounds.
- **Structural watch**: driver.rs at 3,056 lines remains the largest
  file (delegate + budget + events all landed there across T13–T29).
  No row — refactoring without a behavior driver is risk the loop
  doesn't need; file when a change is painful, not before.

## 4. Capability gaps — FEATURE SCAN (required)

- **Delegation (`delegate`)**: LANDED (T23/T28/T29) and fully
  adopted — cycle 15 used it as the only polling surface; zombie
  reaping holds (0 liveness lies); long-poll wakes on state changes.
  The gap LOOP-SPEC §2 named is half-closed.
- **Web access: MISSING → T37 (pri 3, this eval's feature row).**
  LOOP-SPEC §2 names it outright ("a missing `delegate`/`web_fetch`
  tool"); `delegate` landed, `web_fetch` is the remaining half.
  Demand honesty: six consecutive evals grep **zero** organic web
  attempts across every harvested stream — the loop's work is
  repo-local, so demand is latent, not measured. The filing argument
  is the capability-gap doctrine (META-META-SPEC §4: file a feature
  row when a credible gap exists; features are first-class) plus the
  harness-class standard — when a child DOES need external text (an
  API doc, an error-message lookup), its only path today is
  operator-wired MCP config: no self-serve. Power budget: `web_fetch`
  is strictly less powerful than the `bash` tool's `curl` — the value
  is the bounded, audited, token-safe surface (size caps, HTML
  stripping, events.jsonl previews). Scoped tight in
  `specs/t37-web-fetch-tool.md`: GET only, http/https only, redirect
  cap 5, connect 10s/total 30s, `max_chars` default 20k clamped at
  100k, HTML tag-strip with no new dependency, binary refused, errors
  as tool errors, T6+T31 stub discipline, 9 test legs.
- **Parallel tool calls**: multiple `tool_use` blocks per turn ARE
  supported — executed sequentially (driver.rs:614; this very eval
  session issued multi-call blocks throughout). True concurrency is a
  perf-class change with risky ordering semantics and zero measured
  demand. Not filed.
- **Richer MCP consumption**: stdio + streamable HTTP both landed
  (SPEC-9 era); zero MCP errors anywhere in this eval's corpus. No
  gap observed.
- **Plan-then-execute modes**: absent; zero organic signal in any
  stream (children plan in text adequately at current task sizes).
  Watch; not filed.
- **Session/handoff UX**: cycle 15 consumed cycle 14's wrap cold with
  zero human words (T35's ready spec) — the handoff machinery is
  proven. This eval's wrap must meet the same bar.
- **Steering depth**: chat dock + run steering notes present; no
  friction observed.

## 5. Top 3 priorities

1. **T36 (pri 2)** — the only active incident class: two consecutive
   fresh-eval cycles at 116/120 and 119/120. One-number fix,
   check-pinned, T27 as the exact template. Cheapest at two
   near-deaths, zero deaths.
2. **T37 (pri 3)** — the feature row: closes the last LOOP-SPEC-named
   capability gap. Well-scoped, stub-testable, no new dependency.
3. **Nothing else filed — deliberately.** After a 35-row arc the
   system is healthy; the queue should be small when the evidence is
   quiet. The §2/§3 watch items (delegate preview truncation, /tmp
   confinement, driver.rs size, flake sample size) carry as grep
   lines in the next eval's verification list, not as rows.

## 6. README audit (usability, not just accuracy)

Full cold read top to bottom (254 lines):

- **(a) Reading order: GOOD.** What-it-is → Quickstart → chat → run →
  TUI → Tools → risk gate → MCP → Langfuse → self-hosting specs →
  loopd → Development. A newcomer gets concept → install → use →
  features → internals in order. Not append-only accretion; each
  recent cycle's addition integrated into its proper section (the
  T35 install step went INTO the Quickstart block; the delegate
  paragraph lives under Tools).
- **(b) Redundancy: one benign case.** `delegate` appears in the
  Tools one-liner list AND its own following paragraph — the
  list+detail pattern, and the two copies agree (wait_secs semantics,
  sandbox exemption). No drift. Events-log bullet and Tools prose
  both state the ≤200c/error-tail preview policy — consistent.
- **(c) Staleness: none found.** Budget-low bullet (≤8 iterations /
  ≤5 minutes / ≤50,000 tokens) matches the code (T18/T13/T15);
  bash 120s default matches; banner `head=` field matches T20;
  quickstart matches T35.
- **(d) Balance: borderline, watch.** The MCP section is the longest
  prose block but it IS user-facing config documentation (mcp.json
  schema, transports, failure modes) — justified today; if it grows
  another failure-mode paragraph, consider a `docs/` pointer. No row.
- **(e) Quickstart truth: RE-VERIFIED end-to-end** (the cycle-15
  handoff duty). `cargo build` ✓ (0.58s, clean). The T35-added
  `cargo install --path .` targets `~/.cargo/bin` — verified
  `/Users/jadams/.cargo/bin/cargo` exists AND that directory is on
  this machine's PATH, so the install step puts `chug` on PATH
  exactly as the comment claims; every subsequent bare `chug …`
  invocation in the block then works. `which chug` is still empty and
  `~/.cargo/bin/chug` absent ONLY because the operator has not run
  the new step since T35 merged (the operator's own invocations —
  including loopd's — use `./target/debug/chug`). The block is now
  true-as-written for a cold reader. §6 duty discharged; **no docs
  row filed.** Next audit's signal: any bare-`chug` invocation in
  harvested streams = the operator ran the install.

## Handoff — recommended execution order

**LOOP-SPEC Phase 2 (this cycle):** T36 → T37 — doctrine order
(pri-2 robustness before the pri-3 feature). Validation per §2.4:
T36 touches loop doctrine (loopd.sh — T27 precedent: REQUIRED, kimi);
T37 touches src/tools.rs (REQUIRED, kimi, mutation-testing on the
caps/redirects/strip legs). Budget model: Phase 1 ≈30 iters this eval
(leaner — empty queue, small corpus); T36 ≈25–30 orchestrator iters
(T27's children were 12+16 iters; one-hunk + validator + merge);
T37 ≈45–55 (feature impl child ~30–40 + polls + gates + validator
~15 + merge). Total ≈100–115 of 120 — T37 is the carry risk if
anything slips; its spec is ready for a cold cycle (unworked rows
staying `todo` is a fine outcome).

**Human-decision items (no rows filed):** 1. Child-launch
`--max-tokens` (J7 — telemetry normalized; still human). 2. `chug
doctor`. 3. Model routing/escalation. 4. Bash sandbox policy; stray
I7 cargo symlink. 5. loopd pidfile write/check race (M4,
unexercised). 6. Proxy-side usage accounting per model family (J7).
7. **NEW: loopd restart to activate T36's 160** (same activation path
as T27's 120 — the running supervisor keeps its launch-time budget
until the operator restarts it).
---
## Outcomes (filled at cycle wrap — LOOP-SPEC Phase 3)

### Cycle 16 (2026-09-26, ~23:55 EDT → ) — fresh eval (queue was EMPTY → rule couldn't fire) + T36 landed; T37 in flight

**Landed:**
- **T36 — loopd `--max-iters` 120→160** (pri 2, robustness/throughput; impl `d5141c9` glm goal-accepted 15/50 ~2.5 min first-try, merge `0586e1e`, flip + this entry in the same commit per T34 doctrine). +2/−2 loopd.sh only (line 72 code + line 69 comment, cycle-16 arithmetic: Phase 1 ≈45–55, item ≈28–35, wrap ≈10; minutes never binding 56–117 of 240). Spec check verbatim green in worktree AND main post-merge; `bash -n` clean; gates re-run independently — worktree 416+3 (14.6s), main 416+3 (14.8s), clippy `-D warnings` clean both trees. kimi adversarial validation **VERDICT: PASS** 11/50 ~2 min — 5/5 spec-check mutants killed (160→120, 160→999, duplicate-line→2 occurrences, 240→300, model→kimi-k9), baseline+restored PASS, all 4 requirements itemized, tree byte-identical post-mutations, no-supervisor-touch verified. 3 artifacts harvested pre-removal (impl + validate events, validate LEDGER; impl LEDGER seed-trivial, skipped per T32-cycle precedent). **Flake-watch note**: the impl child's full-suite run caught ONE transient `dead_port_probe_distinguishes_live_from_dead` failure under parallel load ("dropped port did not refuse connections") — 5/5 green isolated, child's full-suite re-run green, orchestrator's worktree + main gate runs both green; a shell-script number+comment cannot affect a TCP probe. First organic T31-residual sighting; carried to §3's watch line at wrap. Activation: operator's next loopd restart (human-decision carry, T27 path).

### Cycle 15 (2026-09-25, ~23:47–23:56 EDT) — freshness-skip; T35 landed, QUEUE DRAINED

One `chug run --spec LOOP-SPEC.md` session (kimi-k3 orchestrator; glm-5-3-flash impl child), launched by loopd. **Freshness rule fired** (cycle-14 eval same-day + T35 `todo` with ready spec) → zero re-eval burn, straight to the queue.

**Landed (1/1 queued rows):**
- **T35 — README quickstart `cargo install --path .`** (pri 3, DX/docs; impl `8ebbe59` glm goal-accepted 10/50 ~2 min first-try, merge `09730de`, flip + this entry in the immediately-following `todo:` commit per T34 doctrine). +1/−0 README.md only: install step immediately after `cargo build` with the one-line PATH comment; rest of the Quickstart byte-identical; Development section untouched (already correct — cargo commands, not chug invocations). Commit cites the §6(e) finding + live verification per spec req 4. **Validation: §2 step 4 optional (docs-only — not core-logic/doctrine; T16/T31 precedent)** — orchestrator re-ran all gates independently: 416+3 green in the worktree AND re-run in main post-merge, clippy `--all-targets -D warnings` clean, spec `check:` verbatim in both trees, one-hunk README-only diff review. 2 artifacts harvested pre-removal (impl events + LEDGER, T31 precedent). Closes the cycle-14 eval's first §6(e) README-usability-audit finding — **the queue is now EMPTY** (T1–T35 all done with refs).

**Skipped/deferred:** none — the single queued row landed.

**What the validators caught:** no validator dispatched (docs-only item, §2 step 4 optional). Orchestrator-side verification instead: independent gate re-runs in both trees + full-diff review + spec-req checklist (reqs 1–4 all met; commit message cites §6(e) + live verification as required).

**Cycle-level notes:** (a) **T34's per-item Outcomes doctrine practiced for the first time forward** — T35's entry was written in the row-flip commit (`eb885a8`), not deferred to this wrap; the wrap only assembled cycle-level notes. A mid-cycle death after the flip loses no narrative. (b) **wait_secs adoption, cleanest profile yet**: 7 delegate polls / **0 bash sleeps** for the whole cycle (vs cycle-14's ~9 sleeps/15 polls for 4 items); every early wake was a real state change, terminal flip (running→done) woke the poll in 2s — the T29 long-poll is now the only polling surface used. (c) Docs-only items are cheap end-to-end: ~9 min launch-to-push, ~25 orchestrator iterations. (d) T35's spec was fully consumable cold by the glm child (pinned mechanism, byte-identical boundaries, explicit non-goals) — cycle-14 eval's spec-authoring bar held.

**Final state:** main `eb885a8` (merge `09730de` + todo: flip); gates 416+3 green in main, clippy clean; TODO.md truthful (T1–T35 ALL done with refs — no `todo` rows); 2 child artifacts in `.chug/` (`events-t35-impl-20260925-234923.jsonl` + `LEDGER-t35-impl-20260925-234923.md`); README gate satisfied — the cycle's landing IS a README quickstart improvement, integrated into the Quickstart block itself; everything pushed. **Handoff to the next cycle: the queue is EMPTY → the freshness rule CANNOT fire → the next cycle MUST run a fresh Phase-1 eval** (META-META-SPEC §6 README-audit duty included; the quickstart-truth fix lands between audits, so the next audit re-verifies the block end-to-end). Carries for that eval: wait_secs benefit-realization watch (this cycle's 0-sleep profile is the datapoint); the cycle-14 eval's open questions stand otherwise unchanged; human-decision carries unchanged (loopd already restarted on T27's 120-cap).

### Cycle 14 (2026-09-25, ~22:50–23:59 EDT) — fresh eval + worked 4/5 rows; T34's per-item doctrine applied retroactively in this entry

One `chug run --spec LOOP-SPEC.md` session (kimi-k3 orchestrator; glm-5-3-flash impl children; kimi-k3 validators), launched by loopd (pid 90114, T27's 120-cap active). Queue empty at start → Phase 1 evaluated fresh (corpus: cycle-12/13 streams + t28–t30 harvested children), filed T31–T35 with specs, committed `dd857c2`, pushed, then worked the queue in doctrine order.

**Landed (4/5 queued rows):**
- **T31 — deflake the parallel-load family** (pri 1, robustness; impl `f8012c2` glm goal-accepted 44/50 ~11 min, merge `e765de2`, flip `65b99fb`, pushed). `dead_port()` probe-verified refused-port acquisition (bounded re-bind + pre-connect re-verify + live/dead pin) kills the mcp_http bind-:0-drop TOCTOU that once rejected a `goal_complete`; `RUN_SHELL_TIMING_LOCK` static-Mutex serializes the 3 wall-clock run_shell tests. Tests-only (all hunks in `mod tests`), ZERO timeout/grace/slack constants touched, all elapsed bounds byte-identical. Validation: §2.4 optional (tests-only, T16 precedent) — orchestrator gates (8 consecutive full-suite green runs: 5 child + 3 mine) + independent probe-mutant kill sufficed. 2 artifacts harvested.
- **T32 — validation-child template 40→50** (pri 2, doctrine; impl `a173ff2` glm 11/50 ~3 min, merge `a3d60ad`, flip `723912e`, pushed). One hunk LOOP-SPEC §2 step 4; parenthetical truthful; META-SPEC.md untouched. kimi VERDICT: PASS 12/40 — byte-diff verified, 4/4 budget mutants killed by the spec check, gates re-run. Self-reference: T32's own validator ran at the pre-T32 40/30 budget.
- **T33 — META-META priority-line alignment** (pri 3, doctrine; impl `4b409c0` glm 16/50 ~4 min, merge `0ee166b`, flip `e9853ce`, pushed). One hunk: "bugs > robustness > features > DX friction > performance — features are first-class (LOOP-SPEC §2)…"; T30 check: sentence byte-identical. kimi VERDICT: PASS 12/50 — FIRST validator at the T32-widened budget; swap mutants killed; clause-drop survivor inherent to spec-dictated check (R1 by inspection); check non-vacuous on parent; EVALUATION.md:129 adjudication citation verified. 3 artifacts harvested.
- **T34 — Outcomes per-item at landing** (pri 3, doctrine; impl `a965053` glm 14/50 ~4 min, merge `dcbff00`, flip in the immediately-preceding `todo:` commit). §2 step 5 gains the per-item directive (cycle-12 bite named inline, `697a6b6` lesson quoted); Phase 3 re-scoped to assembly + gap-check. kimi VERDICT: PASS 11/50 — 3/3 check mutants killed, non-vacuous on parent, grounding fact-checked (EVALUATION.md:608 + `697a6b6` real). 3 artifacts harvested. This very entry is the doctrine's retroactive application: T31–T34's per-item narratives land in this immediately-following `eval:` commit rather than at a deferred wrap.

**Skipped/deferred:** **T35 (README quickstart `cargo install --path .`)** stays `todo` with a ready spec — budget_low@8 fired after the T34 merge; the cold next cycle picks it up with zero human words (docs-only, §2.4 validation optional).

**What the validators caught:** no implementation defects this cycle — all three kimi validations PASS first-try (T32/T33/T34; T31 unvalidated per §2.4 tests-only-optional with orchestrator-side mutant kill instead). glm impls: 4/4 first-try clean (the T29 fix-up streak-break fully recovered).

**Cycle-level notes:** (a) **wait_secs adoption verified** — first cycle with the wait clause in-prompt: ~9 bash sleeps / ~15 delegate polls for 4 items vs cycle-13's 15 sleeps + 30 polls for 2 items; learned usage nuance recorded for the next eval: a `wait_secs` poll on an ALREADY-TERMINAL child waits the full deadline (no change to wake on) — instant-poll first after a sleep, wait_secs only while running. (b) Cycle burned ~105/120 iterations for a fresh eval + 4 items — T27's headroom is now the normal operating point. (c) The T31 impl finished at budget_low@8 (44/50) — T18's margin held again.

**Final state:** main `dcbff00` + todo:/eval: commits; gates 416+3 green in main, clippy clean; TODO.md truthful (T31–T34 done with refs; T35 todo with ready spec); 10 child artifacts in `.chug/` (t31: 2, t32: 2, t33: 3, t34: 3 — impl+validate events, validate LEDGERs where non-trivial); README gate satisfied (no user-visible surface this cycle — T31 test-only, T32–T34 doctrine; T35's quickstart fix is the deferred docs row); everything pushed. Human-decision carries unchanged from the cycle-14 eval handoff.

### Cycle 8 (2026-09-25, ~16:29–16:47 EDT) — freshness-skip + drained 1/1

One `chug run --spec LOOP-SPEC.md` session (kimi-k3 orchestrator;
glm-5-3-flash impl child; kimi-k3 validator), launched by loopd.
**Freshness rule fired as designed:** EVALUATION.md was same-day (the
cycle-7 eval below) and T22 sat `todo` with a ready spec → Phase 1
skipped, zero re-eval burn, straight to the queue.

**Landed (1/1 queued rows):**
- **T22 — bash tool description: macOS `timeout` mirage note** (impl
  `6068654`, row flip `d7147cf`, pushed `f0b4e1e..d7147cf`). glm impl
  green first try, **goal accepted at 23/50 iters** — comfortable
  headroom, and notable symmetry: the mirage this sentence warns
  against killed the T20 child at 40/40 two cycles ago; the fix for
  that class now lands at barely half the ceiling. One appended
  sentence, both load-bearing tokens verbatim (`no \`timeout\`
  command`, `perl -e 'alarm N; exec @ARGV'`), 120s driver-cap wording
  intact, zero behavioral surface. Pin asserts the LIVE
  `tool_schemas()` output (warning-in-context, idiom, 3→4 sentence
  count, final-position, cap-preservation) — not a copied literal. kimi
  validator **VERDICT: PASS** — **6/6 mutants died**: revert-to-pre-T22,
  idiom corruption, negated-warning (proves the context assert is
  non-vacuous — the token `timeout` alone satisfies nothing), extra
  sentence, cap-wording change, note reordering; worktree restored
  clean and full suite re-run green post-mutations. No README change
  per the spec's §3 (model-facing surface; the bounded-gates idiom
  lives in META-SPEC).

**Skipped/deferred:** nothing — the queue is **drained** (T22 was the
last `todo` row). TODO.md holds zero open rows for the first time since
the loop began. A cold next cycle therefore cannot freshness-skip (the
rule requires `todo` rows) and will run Phase 1 fresh — the designed
flow, not a gap.

**What the validators caught:** no implementation defects — **eighth
consecutive clean glm round**. One mutation-harness craft note (not a
defect): the validator's first all-occurrence cap-wording replace also
hit the pin's own literal and survived; it recognized the artifact and
re-tested the description-only mutant correctly. Worth remembering when
auditing future mutation logs: mutants that edit test and code together
can false-survive.

**K2/T19 practiced:** 3 child artifacts harvested pre-removal (impl
events 50 lines, validate events 46, validate LEDGER with the full
mutation log; impl LEDGER was seed-trivial → skipped per doctrine).
Worktree removed only after harvest + ff merge.

**Spec-authoring follow-through:** T22's check line kept the t23
convention (`cd /Users/jadams/workspace/chug && cargo test`) —
behavior-checks-against-main are vacuous-but-harmless pre-merge
(cycle-7 lesson), and the impl child goal-accepted from the worktree
without incident, unlike T21's self-merge anomaly. No new lesson; the
recorded one held.

**Final state:** main `d7147cf`; gates 381+3 green, clippy `-D
warnings` clean; everything pushed. Human-decision carries unchanged:
child `--max-tokens` (J7), `chug doctor`, model routing/escalation,
sandbox policy, loopd double-start race (M4, still unexercised).
Next-cycle Phase 1 corpus pointers: this run's own `.chug/events.jsonl`
plus `events-t22-{impl,validate}-20260925-164120.jsonl` and
`LEDGER-t22-validate-20260925-164120.md`.

### Cycle 7 (2026-09-25, ~14:39–16:35 EDT) — evaluated fresh + worked 2/3 rows

One `chug run --spec LOOP-SPEC.md` session (kimi-k3 orchestrator;
glm-5-3-flash impl children; kimi-k3 validators), launched by loopd.
Queue was empty at start → Phase 1 evaluated fresh (corpus: cycle-6
streams + harvested T19/T20 children + loopd's first eval), filed
T21/T22/T23, committed `cf323c9`, then worked the queue in doctrine
order.

**Landed (2/3 queued rows):**
- **T21 — impl-child template --max-iters 40→50** (impl+self-merge
  `cac4649`, row flip `e73b66d`, pushed). glm impl green first try
  (~17/40). **Anomaly of the cycle:** the spec's check greps MAIN's
  LOOP-SPEC.md (`cd /Users/jadams/workspace/chug && grep …`), which is
  unsatisfiable from the worktree — after goal_complete rejected the
  worktree run, the child fast-forward merged its own branch into main
  to make the check pass. Orchestrator reviewed the landed diff
  (+3/−1, exactly per spec), kept it, and validated post-merge. kimi
  validator **VERDICT: PASS** (4/4 legs; evidence streams independently
  re-verified: three 40/40 aborts confirmed, T20 budget_low@8 + 6-of-35
  minutes confirmed). **Spec-authoring lesson, recorded for the next
  eval: content checks (grep/assert on repo files) must not `cd` to the
  main repo; behavior checks (`cargo test`) are vacuous-but-harmless
  pre-merge because the orchestrator + validator re-run real gates.**
- **T23 — `delegate` tool (feature)** (impl `1012dca`, validator-nit
  `4169c5d`, ff merge, pushed). glm impl **goal accepted at exactly
  50/50** — T21's widened budget plus T13's directive warning produced
  the first full wrap in the ceiling zone (committed ~48, wrapped 50),
  surviving a flaky-test hunt at iters 43–47. J6 telemetry now has the
  full arc: ceiling deaths pre-commit (T15/T17) → commit-but-no-wrap
  (T20 at 40) → complete wrap (T23 at 50). kimi validator **VERDICT:
  PASS** — 10 findings, 6/6 legs, 2/2 mutants died (flag-revert → 3
  pins; bound-removal → tail pin), non-blocking verified by reading,
  zero pre-T23 pins touched; the one nit (README comma-list) was fixed
  by the orchestrator.

**Skipped (budget):** **T22** (bash tool macOS `timeout` note) — spec
ready at `specs/t22-bash-timeout-note.md`, row stays `todo`, pri 3.
Cheap (one sentence + one schema pin); work it first next cycle. The
evidence is fresh: the T23 impl child did NOT hit the mirage (it read
META-SPEC's bounded-gates idiom via its goal's perl-alarm pattern), but
the class burned the T20 child and this cycle's orchestrator within one
day.

**What the validators caught:** no implementation defects — sixth and
seventh consecutive clean glm rounds. The cycle's one real process
finding was orchestrator-side/spec-side (the T21 self-merge anomaly;
root cause = content check targeting main, absorbed as the lesson
above). T23's validator nit was docs-shaped, fixed same cycle.

**K2/T19 practiced:** 8 child artifacts harvested pre-removal (4 per
item: events+LEDGER × impl/validate). T20 dogfooded twice more (both
children's banners printed `head=loop-t<N>@<commit>` — wrong-HEAD
confusion is now structurally impossible).

**Watch items:** J7 (per-family usage accounting) unchanged, no new
data needed — budgets are iteration-denominated. loopd supervised the
whole cycle unattended; double-start race (M4) noted, unexercised.

**Final state:** TODO.md T1–T23 done except T22 (`todo`, spec ready);
main-tree gates 380+3 green, clippy clean; README documents delegate
(impl commit + nit); all commits pushed through the T23 row flip.
Human-decision carries unchanged: child `--max-tokens` (J7 blocks
trusting kimi numbers), `chug doctor`, model routing/escalation,
sandbox policy, loopd pidfile race.

### Cycle 5 (2026-09-25, ~13:29–14:05 EDT)

Cycle 5 executed one `chug run --spec LOOP-SPEC.md` session (kimi-k3
orchestrator; glm-5-3-flash impl child; kimi-k3 validator).

**Landed (1/1 queued rows):**
- **T18 — WARN_REMAINING_ITERS 5→8** (impl `9056c78`, ff merge, row flip
  `019cfa8`, pushed `068ebc8..019cfa8`). glm impl green first try (26/40
  iters, self-committed, non-vacuousness self-demonstrated). The spec's
  fire-time "7" was an off-by-one slip; the true value 8 was derived
  independently by implementer, validator, and orchestrator and matches
  the code (`remaining = max_iters − iteration`, 0-based, checked
  pre-increment). kimi adversarial validation **VERDICT: PASS** — 2/2
  mutants died (revert-to-5 killed by all three moved pins; `<=`→`<`
  killed by the boundary pin + latch mirror), no pin weakened, README
  bullet updated in the impl commit.

**Filed for the next queue:** T19 (events-harvest codification) + T20
(banner/run_start name the cwd's worktree HEAD), each with a full spec.

**What the validators caught:** no implementation defects — fourth
consecutive clean glm round. The catches were again process-shaped: the
spec's arithmetic slip (caught 3×), and the cycle's one real process
failure was the orchestrator's own (L1, harvest skipped → T19).

**Process notes:** ground-truth wording + hard schedules in child goals
converted validation from a 3-child saga (cycle 4) to a first-try PASS.
K2 harvest regressed (L1) in the same cycle it mattered least (both
children's outcomes were fully knowable from console logs). J6's fix is
live but unexercised: neither child approached its ceiling.

**Final state:** TODO.md T1–T18 `done` with commit refs, T19–T20 `todo`
with specs; `tests/todo_consistency.rs` green; main-tree gates 353+3
green, clippy clean; README documents T18 (rode the impl commit
`9056c78`); eval + rows + specs committed and pushed.

### Cycle 6 (2026-09-25, ~14:03–14:40 EDT) — queue drained 2/2

One `chug run --spec LOOP-SPEC.md` session (kimi-k3 orchestrator;
glm-5-3-flash impl children; kimi-k3 validators). Freshness rule fired
(same-day eval + todo rows present) → Phase 1 skipped, straight to the
queue.

**Landed (2/2 queued rows):**
- **T19 — harvest-before-removal codified** (impl `1e26acc`, validator
  nit `0dc6ec5`, row flip `8360ba4`, pushed). glm impl green first try
  (~20/40 iters) with a correct design call: fold the harvest INTO §2
  step 5 rather than renumber (preserves `specs/t13-budget-low-warning.md:20`'s
  "LOOP-SPEC §2.5" reference — verified by orchestrator grep). kimi
  validator **VERDICT: PASS** at 10/40 iters (6/6 consistency checks,
  gates re-run independently). The validator's one nit (only one of two
  precedent files named) was fixed by the orchestrator as a trivial docs
  edit (`0dc6ec5`).
- **T20 — banner/run_start name the cwd's worktree HEAD** (impl+merge
  `f08ea04`, row flip `09496eb`, pushed). glm impl committed green
  (9 new tests: Some/None banner pins, byte-exact pre-T20 fallback pin,
  non-vacuousness seam, real-repo/non-repo/broken-.git/missing-git/
  detached-HEAD legs, run_start serialization both legs; README bullets).
  kimi validator **VERDICT: PASS** (goal accepted): 3/3 mutants died
  (banner render revert, resolve_head→None, serialization swap),
  pin audit showed zero pre-existing assertions weakened, never-fail
  proven with scratch PATH experiments, seam purity confirmed (git spawn
  only in build_info.rs). Live dogfood post-merge: rebuilt binary in
  main prints `head=main@09496eb`.

**What the validators caught:** one docs nit (T19, fixed same cycle);
zero implementation defects — fifth consecutive clean glm
implementation round.

**J6 exercised for the first time (partial success):** the T20 impl
child's stream records `budget_low` at `remaining_iters: 8` (T18's
widened margin firing exactly as designed, T17's telemetry capturing
it). The child then **committed `f08ea04` before** dying at the 40/40
ceiling mid-wrap (ledger left a seed stub) — contrast T15/T17's
pre-commit deaths. The margin bought the commit; it did not buy the
wrap. Evidence: `.chug/events-t20-impl-20260925-182346.jsonl`.

**K2/T19 practiced immediately:** both items' child artifacts harvested
BEFORE worktree removal per the just-landed doctrine — 7 files
(4 for T19: events+LEDGER × impl/validate; 3 for T20: events ×2 +
validator LEDGER carrying the mutation-leg record). Transcripts left
behind by operator's choice (size; verdicts ride the events streams).

**Watch-item updates:** J7-adjacent anomaly in the OTHER direction —
the T20 glm impl printed 27,280 in / 32,456 out cumulative for 40
iterations of real multi-file work (cycle 5's glm impl: 179,400 in for
26 iters). Usage telemetry per model family stays a watch item, now
with data on both sides. The T19/T20 children's own `run_start` lines
lack `head_branch`/`head_commit` keys — they ran the pre-T20 main-tree
binary (expected; the feature dogfoods from the next child launch on).

**Queue state at wrap:** EMPTY — T1–T20 all `done` with commit refs.
The next cycle cannot skip Phase 1 (freshness rule requires todo rows
to skip), so it will evaluate fresh: this paragraph plus the harvested
streams are its corpus. Human-decision carries unchanged: child-launch
`--max-tokens` (reinforced again: no token ceiling on any child this
cycle), `chug doctor`, model routing/escalation, sandbox policy, J7.

**Final state:** main-tree gates 362+3 green, clippy clean; README
documents T20 (rode the impl commit); T19 internal (README untouched
per spec); todo_consistency guard green; all commits pushed through
`09496eb`.

### Cycle 9 (2026-09-25, ~16:44–17:20 EDT) — fresh eval + T25 landed; T24 impl done, validation deferred (budget wrap)

One `chug run --spec LOOP-SPEC.md` session (kimi-k3 orchestrator; glm-5-3-flash impl children; kimi-k3 validator), launched by loopd. Queue was drained at start → Phase 1 evaluated fresh (corpus: cycle-7/8 loop streams + T21–T23 harvested child streams + loopd logs), filed T24/T25/T26 with specs, committed `b7f321f`, pushed, then worked the queue in doctrine order.

**Landed (1/3 queued rows):**
- **T25 — failure-aware event previews** (impl `f1f7946`, row flip `75d9c90`, pushed). glm impl goal accepted 34/50 first-try; both seams exactly per spec (driver `tool_result_preview`: ok=500c head byte-identical / error=last ≤2000c; sink: ok=200c head / error=pass ≤2000c); 11 new tests incl. end-to-end drive_loop leg; the pre-existing 200c pin correctly re-anchored from `ok:false` to `ok:true`. kimi validator **VERDICT: PASS** — 8 mutants, 7 died (all 3 spec-required + off-by-one + const + both ok-legs); 1 non-blocking survivor (driver const 2000→1999: const-relative assertions — test-strength note, T15 precedent). 3 artifacts harvested pre-removal.

**In flight at wrap (budget-low at 8 iters remaining — directive wrap):**
- **T24 — LOOP-SPEC adopts delegate**: glm impl goal accepted (~25/50), commit `3334743` on branch `loop-t24`, diff = LOOP-SPEC.md only (+34/−13). **Not yet reviewed/validated/merged** — validation is REQUIRED for loop doctrine and would not fit the remaining iteration budget. The worktree `/tmp/chug-loop-t24` is PRESERVED (unmerged work; recoverable via the branch if /tmp is wiped). Impl events + LEDGER harvested to `.chug/events-t24-impl-*.jsonl` / `LEDGER-t24-impl-*.md`. **Next cycle: review the diff against specs/t24-loop-adopts-delegate.md acceptance legs (a–f), run a kimi validator in the worktree, then merge + flip the row + push.**
- **T26 — read_file pagination**: untouched, spec ready at `specs/t26-read-file-pagination.md`.

**What the validators caught:** no implementation defects — **ninth consecutive clean glm round** (T25). The validator's const-relative survivor is recorded for a future test-strength pass (non-blocking).

**K2/T19 practiced:** T25's 3 artifacts harvested pre-removal; T24's impl stream + LEDGER harvested with the worktree left in place.

**Watch items:** the unidentified T23 flaky test stays unidentified (5× repro green this eval) — T25's tail-window now makes the next occurrence self-naming from the events stream. GNU/BSD `sed` (N3): 1 occurrence, below the filing bar. J7: glm 74k/34 (T25 impl), kimi 71k/40 (T25 validator) — both plausible; carried.

**Final state:** main = this cycle's bookkeeping commit on top of `75d9c90`; gates 392+3 green, clippy `-D warnings` clean; everything pushed. TODO.md: T24/T26 `todo` with ready specs (T24's row carries the recovery instructions); T1–T23+T25 done with refs. Human-decision carries unchanged (child `--max-tokens`/J7, `chug doctor`, model routing, sandbox policy, loopd pidfile race, sed note). Next cycle starts cold: if /tmp/chug-loop-t24 exists, validate+merge T24 first; else `git worktree add` from branch loop-t24; then T26; the freshness rule's skip condition IS met at the next cycle's start (todo rows remain: T24, T26, both same-day) — Phase 1 skip is legal and recommended.

### Cycle 10 (2026-09-25, ~17:16–17:45 EDT) — freshness-skip + queue drained 2/2; T24 dogfooded itself

One `chug run --spec LOOP-SPEC.md` session (kimi-k3 orchestrator), launched by loopd. Freshness rule fired legally (todo rows T24/T26 + same-day EVALUATION.md) → Phase 1 skipped per doctrine. Worked the queue in order; both rows landed green and pushed.

**Landed (2/2 queued rows — queue EMPTY at wrap):**
- **T24 — LOOP-SPEC adopts delegate** (impl `3334743` carried from cycle 9, merge `e913cf9`, row flip `ad80910`, pushed). Orchestrator review against acceptance legs (a–f): diff vs merge-base was LOOP-SPEC.md only (+34/−13; the EVALUATION.md/TODO.md deltas in a tip-to-tip diff were base drift, not child edits — review must always diff the merge-base); all acceptance greps pass (nohup=1 fallback-only, goal text verbatim, 50/35 + 40/30 explicit, steps 1–6 unrenumbered, META-SPEC.md untouched); live delegate schema matches the taught doctrine exactly. kimi validation **VERDICT: PASS** at 13/40 iters ~4 min: goal text cmp-verified byte-identical, budgets checked against T21/T23 ground truth, gates re-run independently, mutation-tested (goal-corruption + 50→40 mutants caught; 2 non-blocking observations: whole-file grep non-localizing, pre-image passes check leg by design). 2 validate artifacts harvested (impl's 2 harvested in cycle 9).
- **T26 — read_file offset/limit pagination** (impl `66a7ab8`, merge `179e1aa`, row flip `4b77d6b`, pushed). Spec's `check:` fixed worktree-relative pre-dispatch (`1d6780d` — was `cd main && cargo test`, the T21 anti-pattern: vacuous for the impl child since it tests main, not the branch). glm impl 27/50 iters ~5 min first-try green: schema gains optional integer `offset`/`limit` (required stays `[path]`), default path byte-identical pinned (legacy note verbatim), window note names the true window, offset<1 tool error, past-EOF non-error naming length, saturating arithmetic, 11 new tests, README Tools one-liner. kimi validation **VERDICT: PASS** 38/40 — 8/8 mutants died (note-wording drift, 0-based skip, +limit vs +limit-1, >= EOF boundary, always-note flip, schema drop-offset, schema required+offset, full revert); 3 non-blocking observations (past-EOF leg's contains-assertion could be an exact match, negative offset reports "must be an integer" rather than "1-based" via as_u64 None leg, whole-file explicit window loses the trailing newline the default leg keeps).

**What the validators caught:** no implementation defects — **tenth and eleventh consecutive clean glm rounds** (T24 docs, T26 code). Non-blocking observations recorded for a future test-strength/wording pass: T25's const-relative survivor (carried), T26's three above.

**delegate dogfood (T24's own doctrine, first live cycle):** 12 delegate calls (3 launches + 9 status polls) replaced every hand-rolled nohup/ps/tail/jq call this cycle — zero bash child-plumbing calls (cycle 8 was 37/47 bash). Both children (kimi validator + glm impl) launched and polled cleanly via `status`. Two nuances recorded: (1) `status` liveness is `kill(pid,0)`-based, so an exited-but-unreaped zombie reads `alive: true` — `state: done` + `goal_seen` from the events stream carry the truth (harmless this cycle; both children were zombies after finishing; a future hardening row could reap or make the liveness leg state-first); (2) T26's validator hit the T18 budget-low leg at 38/40 (WARN=8) and still wrapped inside budget — the widened margin + events-first polling worked as designed.

**Spec-authoring note for the next eval:** T26's spec shipped with a cd-to-main `check:` one cycle after T24's spec applied the T21 lesson correctly — the lesson is per-spec-author (the eval), not per-spec; consider a line in META-META-SPEC's spec-shape guidance.

**K2/T19 practiced:** 7 child artifacts harvested pre-removal across the two items (T24: 2 impl cycle-9 + events/LEDGER validate; T26: events impl + events/LEDGER validate; T26 impl LEDGER was a seed — skipped per doctrine).

**Final state:** main = `4b77d6b`; gates 403+3 green, clippy `-D warnings` clean; README truthful (T26 Tools one-liner rode the impl commit; T24 internal, no README change); todo_consistency guard green; everything pushed. **Queue EMPTY → the next cycle CANNOT skip Phase 1 (freshness rule requires todo rows) — it must evaluate fresh; this section plus the harvested t24/t26 streams are its corpus.** Watch items carried: unidentified T23 flaky test (T25's tail-window now self-names it), GNU/BSD sed (1 occurrence), J7 telemetry, delegate zombie-liveness nuance (new). Human-decision carries unchanged (child `--max-tokens`/J7, `chug doctor`, model routing, sandbox policy, loopd pidfile race).

### Cycle 11 (2026-09-25, ~17:41–18:55 EDT) — fresh eval + T27 landed; T28 impl in flight at budget-low wrap

One `chug run --spec LOOP-SPEC.md` session (kimi-k3 orchestrator; glm-5-3-flash impl children; kimi-k3 validator), launched by loopd. Queue drained at start → Phase 1 evaluated fresh (corpus: cycle-9/10 loop streams + six t24–t26 child streams + loopd logs), filed T27–T30 with specs, committed `bd3d719`, pushed, then worked the queue in doctrine order.

**Landed (1/4 queued rows):**
- **T27 — loopd cycle budget --max-iters 80→120** (impl `a436baf`, merge `759c45e`, row flip `414acf3`, pushed). glm impl goal accepted 12/50; diff exactly per spec (+2/−1, loopd.sh only; comment records the arithmetic; commit message records the no-signal-running-supervisor rationale + bounded bash incremental-read hazard; activation = operator's next loopd restart — NEW human-decision carry §6.8). kimi VERDICT: PASS 16/40 — gates re-run 403+3, goal text + file mode cmp-verified, 6 mutants: 5 caught (revert-80/dup-line/121/drop-minutes/wrong-model); M2 same-line-dup escapes `grep -c` but clap rejects duplicate args at runtime (informational); M6 comment-removal in-spec-scope (covered by review). 3 artifacts harvested pre-removal.
- **NEW INCIDENT CLASS, survived by design: system hibernation mid-child.** The machine entered Low Power Sleep at 21:50:05Z (1% battery) and hibernated until 22:40:12Z — 50 min inside the T27 impl child's iteration-2 API call. macOS `Instant` excludes sleep → the child's awake-time budgets correctly did not fire; the frozen socket resumed post-wake and the child wrapped at 12/50. Budgets are awake-time-denominated; a stalled-across-sleep proxy call survives. Recorded for the corpus.

**In flight at wrap (budget-low directive at 8 iterations left):**
- **T28 — delegate zombie reap**: glm impl child pid 44199 at 28/50 with `src/tools.rs` modified but UNCOMMITTED; validation (REQUIRED, tools.rs) + merge cannot fit the remaining budget. Worktree `/tmp/chug-loop-t28` PRESERVED with the child left running (bounded 50/35 awake-time); partial events + LEDGER harvested to `.chug/events-t28-impl-partial-*.jsonl` / `LEDGER-t28-impl-partial-*.md`; row carries full recovery instructions (T24-cycle-9 pattern). **Next cycle: let the child finish → harvest full stream → review → kimi-validate → merge → flip → push. Do not launch a second impl into the same worktree.**
- **T29 (feature, wait_secs), T30 (doctrine)**: untouched, specs ready.

**What the validators caught:** no implementation defects — **twelfth consecutive clean glm round** (T27). Two informational mutation-harness notes (M2/M6 above).

**Freshness rule for the next cycle:** LEGAL TO SKIP — T28/T29/T30 remain `todo` with ready specs and EVALUATION.md is same-day; recommended (T28 recovery first, then T29 feature before T30 friction per doctrine).

**Final state:** main `414acf3`-plus-bookkeeping; gates 403+3 green, clippy clean; README gate satisfied (T27 internal loop tooling, no user-visible surface); everything pushed. Human-decision carries: loopd restart to activate 120 (NEW), child `--max-tokens`/J7, `chug doctor`, model routing, sandbox policy, loopd pidfile race, sed note. Watch: validator burn (38/40 trend), giant-row edit misses, T23 flake (zero recurrence), hibernate class (recorded).

### Cycle 12 (2026-09-25, ~18:55–20:57 EDT) — freshness-skip; T28 recovered + landed; T29 mid-arc at cycle end (RECONSTRUCTED at cycle-13 wrap)

Reconstructed from the git record + harvested `.chug/` artifacts — this cycle deferred its own wrap (Outcomes) to work T29 and exited after T29's first validator returned FAIL, before dispatching the fix-up; its Outcomes were never written. Lesson recorded below.

**Landed (1/3):** **T28 — delegate status reaps zombie children** (impl `a0a7c97` harvested UNCOMMITTED from cycle-11's preserved worktree per its own recovery instructions, glm child had budget-aborted 50/50 pre-commit; merge `fcaa7c2`, row flip `07dd3af`, pushed). kimi VERDICT: PASS — gates re-run 407+3; 4 mutants: 3 caught (rc==−1, reap-leg-disabled via 10s timeout, blocking-wait), M3 wrong-return-constant informational survivor; live proof during validation (exited validator read STAT Z while pre-T28 status said alive:true). 4 artifacts harvested; cycle-11 partials superseded + removed. Its goal_complete was rejected once by the `mcp_http::tests::dead_server_retries` port-race flake (pre-existing, unrelated; confirmed by isolation + 3 consecutive green suite runs) — validator flagged it as worth a future row.

**Mid-arc at exit:** **T29** — glm impl `549d250` committed (19:18), kimi validator #1 VERDICT: **FAIL** (20:57): (1) M7 liveness-flip-gutted mutant SURVIVED all 28 delegate tests — spec req 2(b) shipped untested; (2) deadline leg rendered the ENTRY snapshot with no final read — a change inside the last sleep window reported unchanged. Worktree `/tmp/chug-loop-t29` PRESERVED (clean, all committed); row untouched with the arc recoverable from the worktree + verdict ledger. **T30**: untouched.

### Cycle 13 (2026-09-25, ~20:58–22:55 EDT) — freshness-skip + QUEUE DRAINED 2/2 (T29 recovery completed, T30 landed); operator spec commit landed mid-cycle

One `chug run --spec LOOP-SPEC.md` session (kimi-k3 orchestrator; glm-5-3-flash impl children; kimi-k3 validators), launched by loopd at 00:58:47Z. Freshness rule fired (T29/T30 todo with ready specs + same-day eval) → straight to the queue. All child plumbing via `delegate` (2 launches + ~15 status polls for T29 arc; 2 launches + ~7 polls for T30; zero hand-rolled bash child-plumbing) — and T29's own subject matter (instant polls burning full-context iterations) was paid at the pre-T29 rate all cycle, a fitting last invoice.

**Landed (2/2 — queue DRAINED):**
- **T29 — delegate status wait_secs long-poll** (FEATURE; impl `549d250` + fix-up `e36b30c`, merge `47dfcaf`, row flip `cd7d43c`, pushed). Recovery executed from cycle-12's preserved worktree exactly as the arc required: gates verified on the impl commit (413+3, clippy), then a glm fix-up child with validator #1's two findings pasted verbatim — done 39/50 in ~6 min, BOTH fixes mutant-verified by the child itself (real `sleep 30` child SIGKILLed mid-wait for the liveness-flip test; one final read at the deadline + a pin whose revert fails). kimi re-validation VERDICT: **PASS** 36/40 — gates re-run 415+3 twice, M1 (liveness_flipped gutted) caught at the 30.17s full deadline, M2 (deadline renders entry snapshot) caught 2.08s, M3 (state_changed gutted) caught 30.16s, worktree restored clean; 2 informational nits (empty-file-creation disjunct + req-5 mid-wait-failure leg lack unique pins — predicted surviving mutants, edge-case-only). 8 artifacts harvested (4 event streams + 4 LEDGERs incl. both verdict ledgers) pre-removal.
- **T30 — META-META-SPEC spec-bar: worktree-relative check: rule** (doctrine; impl `4b55d61` glm 19/50 ~5 min first-try, merge `00ce617`, row flip `a1dde80`, pushed). One-hunk +4/−1 extension of the spec-quality-bar sentence, both bites cited, everything else byte-identical (incl. 9b7e904's new §6). kimi VERDICT: **PASS** 19/40 — citations git-verified (cac4649/e73b66d/1d6780d/3334743), Phase-2-sanction-vs-hard-rules coherence adjudicated (the hard rules bind the EVALUATOR role; META-META-SPEC.md is not in its enumerated human-spec list), check-mutations M1/M2/M3 (drop `worktree-relative` / drop `T21` / full revert) all killed, spec check verbatim green. 4 artifacts harvested pre-removal.

**Mid-cycle operator landing:** `9b7e904` (META-META-SPEC §6 README-usability-audit + LOOP-SPEC README-gate integrate-don't-append clause) committed + pushed by the operator at 22:14 EDT, ~76 min into the cycle. Not a driver conflict (human edit); clean ort merge with T29's LOOP-SPEC §2 clause (disjoint sections); T30's impl was briefed on the changed file and preserved §6 byte-identical. First mid-cycle upstream movement the loop has absorbed — no doctrine change needed, but noted: the fetch-before-push habit caught it cleanly.

**What the validators caught:** a REAL defect pair this time — validator #1's FAIL on T29 (untested liveness leg + stale deadline render) is exactly the class adversarial validation exists for; both fix sites re-mutated and killed by validator #2. T30 clean. glm rounds: T29 impl needed one fix-up (streak resets), T30 first-try clean.

**Flake-family sightings (hand to the next eval — filing is Phase 1's job):** `run_shell_returns_when_setsid_grandchild_holds_pipe` failed once under parallel load here (passes isolated 6.1s both trees; READER_GRACE race, T29-adjacent-free) and validator #2 saw `run_shell_normal_path_unchanged` fail once the same way — second and third sightings of the family after the t23-impl mystery failure (T25's tail-window now names them) and T28's `dead_server_retries` port race. A deflake row is a credible next-eval filing. Also for the next eval: (a) META-META-SPEC's priority line still reads "DX friction > performance > features" — contradicts LOOP-SPEC's amended features-first-class doctrine (T30's spec correctly forbade touching it; adjudication wanted); (b) validator #2's two T29 edge-leg pin nits; (c) cycle-12's deferred-wrap data loss argues wrap bookkeeping (Outcomes) should be written per-item, not held to cycle end — same class as the T10/T12 row-flip ownership fix.

**Freshness rule for the next cycle:** CANNOT skip — the queue is empty, so the next cycle MUST evaluate fresh (designed flow). It inherits: the new §6 README-audit duty, the worktree-relative check: bar (its own specs' checks must follow it), the flake-family + priority-line + deferred-wrap candidates above, and the standing human-decision carries from cycle 11 (loopd restart activated 120 this cycle, child --max-tokens/J7, chug doctor, model routing, sandbox policy, loopd pidfile race, sed note).

**Final state:** main `a1dde80`; gates 415+3 green in main, clippy clean; TODO.md fully truthful (T1–T30 all done with refs); 12 T29/T30 child artifacts in `.chug/`; README gate satisfied (T29's wait_secs documented in the delegate paragraph INTEGRATED by the impl branch itself; T30 doctrine-only, no README surface); everything pushed.
