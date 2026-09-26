# EVALUATION — chug, assessed by chug-loop (2026-09-25, cycle 14)

Corpus: `.chug/events-20260926-005848.jsonl` (**cycle-12's full LOOP-SPEC
run**: 64/80 iterations — the last pre-T27 80-cap cycle — 46 bash /
17 delegate calls, **14 bash sleeps ≥80s**, goal accepted — freshness-skip,
T28 recovered+landed, T29 mid-arc at wrap),
`.chug/events-20260926-025049.jsonl` (**cycle-13's full run**: 116/120
iterations — the FIRST T27-activated 120-cap cycle — 80 bash / 30 delegate
calls, **15 bash sleeps ≥80s, 0 `waited:` lines**, budget_low@8 at
02:48:11, goal accepted 02:49 — freshness-skip, QUEUE DRAINED 2/2: T29
FAIL→fix-up→PASS arc + T30; operator spec commit `9b7e904` absorbed
mid-cycle), the t28/t29/t30 harvested child streams + LEDGERs (8 T29
artifacts incl. FAIL+PASS verdict ledgers, 4 T30), `.chug/loopd/`
(supervisor pid 90114, cycles 11→13 OK unattended), `TODO.md` (T1–T30
done with refs — **queue EMPTY at cycle start**, freshness rule cannot
fire), git log through `697a6b6`; `src/` (20,045 lines, +847 since cycle
11 — delegate T23/T28/T29: driver.rs 3,056; tools.rs 2,996; mcp_http.rs
2,343; tui.rs 2,163; observ.rs 1,318; api.rs 1,122); `README.md` (253
lines). Prior evaluations: cycle 11 (with cycle-5→13 Outcomes) below at
§Outcomes; cycle 7 same section; cycle 4 at `c8222cd`; cycle 3 at
`df4039a`; cycle 2 at `0d71d00`.
Verification performed this eval: full `cargo test -- --test-threads=4`
= **415+3 green in 14.5s**; `cargo build` clean; flake-family evidence
re-grepped across all harvested streams (4 sightings, 3 named tests —
§2 P1); validator iteration counts recomputed per-stream from
`run_start.max_iters` + last `iteration.n` (§2 P2 table); cycle-12/13
bash-sleep + delegate-poll profiles counted by jq (§3); web-fetch
evidence grep (docs.rs/stackoverflow/google/fetch) across cycles 11–13
+ all t29/t30 streams = 0 hits (5th consecutive eval); `timeout`-mirage
grep across all post-T22 streams = 0; GNU/BSD `sed` error grep = 0 (3rd
consecutive quiet eval — carry RETIRED, §4); wedge-grep across ledgers =
0 (T2's activity timeout: zero firings, 9th eval); README §6 cold-read
usability audit performed in full (first cycle carrying the `9b7e904`
duty — §6 below); quickstart-truth bug verified live (`which chug`
empty, `~/.cargo/bin/chug` absent — even the operator runs
`./target/debug/chug`, per the loopd launch line and `ps`).

## 1. What chug does well

- **T27 activated, and the headroom converted directly into recovered
  work.** Cycle 13 ran 116/120 — the widened budget held a two-item
  cycle that included a full FAIL→fix-up→PASS arc (T29) without a
  ceiling death; cycle 12 (the last 80-cap) wrapped at 64 with an arc
  stranded mid-flight. The T27 arithmetic (80 − 45 ≈ 1 item) is
  confirmed by construction: 120 − ~45 eval ≈ 2 items + wrap.
- **The T29 FAIL arc is adversarial validation doing exactly its job.**
  Validator #1 (kimi) FAILED T29 on a real defect pair — the
  liveness-flip leg untested (mutant M7 survived) and the deadline leg
  rendering a stale entry snapshot; a glm fix-up child fixed both with
  self-mutant-verified tests (real `sleep 30` child SIGKILLed mid-wait;
  revert-mutant dies 2.08s); validator #2 PASSed with all three mutants
  killed. Real defects caught pre-merge, fix verified adversarially —
  the loop's reason to exist, working end-to-end inside one cycle.
- **T28's zombie-reap proved itself live during its own validation**:
  the exited validator read `STAT Z` while a pre-T28 `status` said
  `alive: true`. Field-fixed, field-proven.
- **delegate dogfood total:** cycles 12+13 ran ALL child plumbing
  through the tool (17 + 30 calls, launches + polls), zero hand-rolled
  nohup/ps/tail/jq — and cycle 13 absorbed the first-ever mid-cycle
  operator upstream commit (`9b7e904`) cleanly (fetch-before-push +
  disjoint-section ort merge; no doctrine change needed).
- **loopd boring, 13 cycles:** supervisor pid 90114, `cycle OK`
  streak, HALT never fired, single-driver guard held every cycle
  (incl. this one — only my own run at start).
- **Freshness rule both legs still boringly legal:** skip fired cycles
  12/13 (todo rows + same-day eval); this cycle MUST evaluate fresh
  (empty queue) — the designed alternation, zero ambiguity in any log.

## 2. Incidents worth fixing

- **P1 → T31 (pri 1) — the parallel-load flake family now red-gates
  real work; it has rejected a `goal_complete`.** Four sightings, three
  named tests, root causes visible in code:
  1. `mcp_http::tests::dead_server_retries_then_tool_error_without_sleeping`
     — the T28 validator's **`goal_complete` was REJECTED** by this
     flake (its verdict ledger: "pre-existing flaky test … bind
     :0→drop port TOCTOU under parallel test threads"; passes isolated
     0.01s). `src/mcp_http.rs:1484` binds `:0`, drops the listener, and
     assumes the port stays free — a classic TOCTOU: under
     `--test-threads=4` another test's stub can claim it between drop
     and connect.
  2. `tools::tests::run_shell_returns_when_setsid_grandchild_holds_pipe`
     — failed once in cycle-13's MAIN gates under parallel load
     (passes isolated, 6.10s both trees); the T29 fix-up child's
     verdict now calls it "**known flake**" BY NAME. A flake with a
     nickname is a filing, not a watch.
  3. `tools::tests::run_shell_normal_path_unchanged` — failed once in
     T29-validate2's suite run (414/415, 7.83s; isolated pass 0.13s).
  4. The original T23-impl mystery failure (cycle-9 eval N1) is almost
     certainly this family — same shape (suite red once, named
     post-T25, never reproduces isolated).
  Mechanism: the `run_shell` pair assert wall-clock `elapsed <
  timeout + READER_GRACE + slack` (`src/tools.rs:1744,1782`) — under
  parallel load the scheduler stretches wall time and the assertion
  lies; `dead_server_retries` has the port TOCTOU. T25's tail-window
  did its job — every sighting arrived NAMED. Filed **T31**
  (`specs/t31-deflake-parallel-suite.md`): mechanism-over-timeout
  (load-insensitive assertion bases and/or serialization of the
  timing-sensitive set; port TOCTOU removed by construction), pri 1 —
  a suite that lies red and rejects goal_completes is bug-class, not
  friction.
- **P2 → T32 (pri 2) — the validator iteration ceiling is the next
  T21/T27; file BEFORE the first died-mid-verdict.** Last six
  substantive validators, recomputed per-stream this eval:

  | validator | iters | budget_low@8 |
  |---|---|---|
  | t26-validate | 38/40 | YES |
  | t27-validate | 16/40 | no |
  | t28-validate | 34/40 | YES |
  | t29-validate1 (FAIL) | **39/40** | YES |
  | t29-validate2 (PASS) | 36/40 | YES |
  | t30-validate | 19/40 | no |

  4 of 6 at ≥34/40; t29-validate1 delivered its FAIL verdict **one
  iteration from dying mid-verdict** — a death there strands a
  merge-decision arc exactly like T29's own cycle-12 stranding. Suite
  grows ~10 tests/item (403→415 across 3 items) and mutation count
  scales with diff surface; the trend only goes one way. T21 fixed
  impl children (40→50 after three 40/40 deaths); T27 fixed the
  orchestrator (80→120 after two near-deaths); the validator template
  gets the same arithmetic at 4-of-6 near-deaths, zero deaths — the
  cheapest the class will ever be. Filed **T32**
  (`specs/t32-validator-iteration-headroom.md`): LOOP-SPEC §2 step 4
  `max_iters: 40`→`50` only (minutes stays 30 — never binding:
  t29-validate2 used ~13 of 30), META-SPEC.md untouched per the
  T21/T24 override pattern (human spec; LOOP-SPEC overrides §6 launch
  mechanics).
- **P3 → T33 (pri 3) — META-META-SPEC's priority line contradicts
  LOOP-SPEC (handed over by cycle-13's wrap for adjudication;
  adjudicated here).** `META-META-SPEC.md:69`: "Priority doctrine:
  bugs > robustness > DX friction > performance > features."
  LOOP-SPEC §2: "bugs > robustness > **features** > DX friction >
  performance" plus "Features are first-class … work the feature first
  [at equal pri]". The evaluator (filing order) and the orchestrator
  (work order) are the SAME loop session reading two disagreeing
  doctrines. Adjudication: LOOP-SPEC wins — it post-dates, carries the
  explicitly amended doctrine, and its own preamble asserts override
  ("They apply in full except where this spec overrides"). Notably the
  README's continuous-mode paragraph already matches LOOP-SPEC — only
  META-META-SPEC drifted. T30 precedent sanctions a loop item editing
  META-META-SPEC.md (validated adjudication: the hard-rule list binds
  the evaluator role and does not enumerate it). Filed **T33**
  (`specs/t33-metameta-priority-line.md`) — one-sentence alignment,
  everything else byte-identical.
- **P4 → T34 (pri 3) — wrap-time Outcomes authorship is a
  single-point-of-loss (the cycle-12 lesson, same class as the
  T10/T12 row-flip fix).** Cycle 12 ended mid-arc at budget; its
  Outcomes entry was never written by cycle 12 — cycle-13's wrap
  reconstructed it from the git record (its entry is titled
  "RECONSTRUCTED at cycle-13 wrap"; the eval commit `697a6b6` records
  the lesson: "deferred-wrap loss lesson: write Outcomes per-item").
  The T10/T12 fix was orchestrator-owns-the-row AT LANDING; the same
  logic applies to the narrative: write each item's Outcomes entry in
  the row-flip commit (or immediately after), so a mid-cycle death
  loses nothing. Wrap-time Phase 3 becomes assembly + gap-check, not
  authorship. Filed **T34** (`specs/t34-per-item-outcomes.md`) —
  LOOP-SPEC §2 step 5 gains the per-item directive + the Phase 3
  bullet is re-scoped; cites cycle-12.
- **Assessed, carried (below the bar this eval):**
  (a) **Harvest `cp` fumbles** — cycle 13: two errors (`cp:
  events-20260925-233130.jsonl: No such file`, `cp: LEDGER.md: No such
  file`) — the orchestrator guessed ROTATED filenames instead of the
  worktree's live `.chug/events.jsonl` / root `LEDGER.md`; ~2 recovery
  iterations. Second cycle with harvest friction (cycle 9's was the
  `.chug` git-add fumble). Watch: a third sighting → one exact-command
  clause in §2 step 5 (the T34 doctrine touch is NOT expanded — one
  concern per spec).
  (b) **O4 giant-TODO-row edit misses** — 1/cycle (`edit_file: 'old'
  not found` on a multi-KB done-row; cycle 13's TODO row flip hit it).
  T26 pagination makes the fresh-read mitigation free; below the bar,
  carried 3rd eval.
  (c) **T29 validate2's two informational edge-leg nits**
  (empty-file-creation disjunct + req-5 mid-wait-failure leg lack
  unique pins) — predicted surviving mutants, edge-case-only;
  informational disposition stands; carried.
  (d) **README delegate sandbox-exception stated twice** (Tools list +
  delegate paragraph end — §6(b)); one-line drift risk; nit for the
  next docs pass, not a row.

## 3. Friction hot spots

- **The polling sink — measured across three cycles, fix shipped
  mid-cycle-13, benefit lands THIS cycle.** bash sleeps ≥80s per
  cycle: cycle 11: 5/78 iters; cycle 12: **14/64 (22%)**; cycle 13:
  **15/116 (13%)** — plus 17–30 instant delegate status polls per
  cycle, each a full-context LLM round trip (~325k input tokens
  late-cycle). T29's `wait_secs` shipped mid-cycle-13 and cycle-13's
  stream shows **0 `waited:` lines** — expected, not a defect: the
  orchestrator's system prompt is fixed at launch, so LOOP-SPEC edits
  land NEXT cycle. Cycle 14 is the first with the wait clause
  in-prompt; expect sleeps→0 and polls→≈1/state-change. Watch item for
  the next eval: verify the collapse in this cycle's stream.
- **Fixed classes holding (9th eval):** PATH tax, revert-thrash,
  edit-thrash, stub hangs, stale-ledger, watch-and-wait, `timeout`
  mirage (0 post-T22 across all t23–t30 + three cycle streams),
  2000-line read cap (T26; zero sed-chunking since), GNU/BSD `sed` (0
  new, 3rd quiet eval — **carry retired**, §4).
- **Validator burn — graduated from watch to incident:** see §2 P2
  (was cycle-11 eval O3; the "one died-at-ceiling incident away"
  condition is met in spirit at 39/40 + 4-of-6 near-deaths).

## 4. Capability gaps — FEATURE SCAN (required)

Judged against the human specs' direction (meta loops, adversarial
validation, observability, fleet-driving) and harness-class norms:

- **No new feature row filed this eval — the scan is documented, not
  quota-filling.** The features-first mandate was satisfied one cycle
  ago (T29 `wait_secs` — a feature worked ahead of same-pri friction);
  its benefit realization is THIS cycle's job (§3). Candidate classes
  interrogated:
  - **web_fetch — NOT filed (5th consecutive eval).** 0 external-info
    signals across cycles 11–13 + all t29/t30 streams. The bar stands:
    a cycle stalls on external information.
  - **delegate stop/kill action — NOT filed.** Wedge protocol: 0
    firings post-T2 (9th eval); `kill <pid>` via bash is one call when
    it does. Bar: the first post-T2 wedge.
  - **Parallel tool calls — closed class** (cycle 13: 121 results /
    116 iterations ≈ 1.04 + batched polls throughout).
  - **Child-token telemetry in `delegate status`** — the events
    summary already reads the stream (which has per-iteration
    cumulative tokens); surfacing last-known tokens would let an
    orchestrator spot a runaway child. No incident has demanded it
    (glm telemetry normalized since cycle 11: 92k/25, 74k/34, 33k/27).
    Below the bar; carried.
  - **MCP consumption depth / plan-then-execute / session UX — no
    gap.** The loop is the proof: fourteenth consecutive cold start
    with zero human words.
- **Carried human-decision items (unchanged unless noted):**
  child-launch `--max-tokens` (J7 — telemetry normalized;
  trust-in-proxy-accounting stays human), `chug doctor`, model
  routing/escalation (12 of last 13 glm rounds clean; the T29 fix-up
  reset the first-try streak but needed NO fallback), bash sandbox
  policy + stray I7 cargo symlink, loopd pidfile race (M4,
  unexercised), proxy-side usage accounting per model family (J7).
  **Retired this eval:** GNU/BSD `sed` note (N3 — 0 occurrences, 3rd
  quiet eval), loopd-T27-activation carry (DONE — 120-cap live under
  pid 90114).

## 5. Top 3 priorities

1. **T31 — deflake the parallel-suite family (robustness, pri 1).**
   The suite has rejected a `goal_complete` and red-gated two
   validators + the main gates once each; every sighting is named and
   both root causes are in-code (wall-clock assertions under load;
   bind-:0-drop port TOCTOU). Bug-class reliability, not polish.
2. **T32 — validator template 40→50 (robustness, pri 2).** 4 of the
   last 6 validators at ≥34/40, one FAIL verdict delivered at 39/40.
   The T21/T27 arithmetic says fix the ceiling before the first death,
   not after; the edit is one number in LOOP-SPEC §2 step 4.
3. **T33 — META-META-SPEC priority-line alignment (doctrine, pri
   3).** A one-sentence contradiction between the loop's two
   most-read doctrines; adjudicated this eval (LOOP-SPEC wins); the
   longest-blocked of the three pri-3 rows.

## 6. README audit (usability, not just accuracy — first cycle carrying the §6 duty)

Cold-read top to bottom as someone who has never seen chug:

- **(a) Reading order — GOOD.** What-it-is → quickstart → chat → run →
  TUI → tools → risk gate → MCP → Langfuse → self-hosting specs →
  continuous mode → development is a correct newcomer progression.
  Sections are NOT append-only accretion. Within the autonomous-mode
  section the bullets are chronologically accreted (banner →
  anti-stall → ledger → verification → stuck → budgets → abort →
  transcript → events), but each is self-contained and the order
  roughly matches ascending sophistication; not structural debt.
- **(b) Redundancy — one nit, carried.** The delegate cwd-sandbox
  exception is stated twice (the Tools list's parenthetical AND the
  delegate paragraph's closing sentence) with slight wording drift —
  ~2 lines of drift risk, below the bar (§2 carried (d)).
- **(c) Staleness — none found.** `wait_secs`, banner `head=`, token
  budgets, failure-aware previews are all documented at correct
  prominence with current semantics; the continuous-mode priority line
  already matches the AMENDED LOOP-SPEC doctrine (the README got it
  right while META-META-SPEC drifted — §2 P3).
- **(d) Balance — acceptable.** The events-log bullet carries
  schema-level detail (field names, null semantics, preview windows),
  but it is the events stream's only reference and jq-mining is a
  documented loop workflow — moving it to a spec would strand the
  newcomer. No offload filed.
- **(e) Quickstart truth — ONE BUG, filed T35.** The quickstart runs
  `cargo build` and then invokes **bare `chug`** — which fails as
  written for a cold reader: `cargo build` produces
  `target/debug/chug` and nothing puts `chug` on PATH. Verified live
  this eval: `which chug` empty, `~/.cargo/bin/chug` absent — even the
  operator's own supervisor launches `./target/debug/chug`. Every
  quickstart invocation (`run`, `--tui`, `--resume`, `chat`,
  `ledger`) is bare. §6(e)'s exact question — "do the commands work
  as written, in the order given?" — answers NO. Filed **T35**
  (`specs/t35-readme-quickstart-path.md`), docs, pri 3: add a
  `cargo install --path .` step (or `./target/debug/chug` prefixes —
  the impl child picks with justification) so the block runs cold in
  order.

## Handoff — recommended execution order

**LOOP-SPEC Phase 2 (this cycle):** T31 → T32 → T33 → T34 → T35 —
doctrine order (bug-class robustness, then robustness, then the three
pri-3 doctrine/docs rows longest-blocked-first). Validation per §2.4:
T31 is expected tests-only (validation OPTIONAL — orchestrator's call
at review; if the diff grows a production seam, validate); T32, T33,
T34 all touch loop/spec doctrine (REQUIRED, kimi); T35 docs-only
(optional). Budget model: ~20 iterations spent on Phase 1; T31 ≈
30–35, T32 ≈ 25–30; T33/T34/T35 are one-hunk doctrine/docs items
(~20–25 each) — T31 + T32 targeted, T33 if headroom; T34/T35 carry
with ready specs if the budget says stop (unworked rows staying `todo`
is a fine outcome — the cold next cycle picks them up with zero human
words).

**Human-decision items (no rows filed):** 1. Child-launch
`--max-tokens` (J7 — telemetry normalized; still human). 2. `chug
doctor`. 3. Model routing/escalation. 4. Bash sandbox policy; stray
I7 cargo symlink. 5. loopd pidfile write/check race (M4,
unexercised). 6. Proxy-side usage accounting per model family (J7).
No new carries this eval; two retired (§4).

---
## Outcomes (filled at cycle wrap — LOOP-SPEC Phase 3)

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
