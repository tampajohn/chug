# EVALUATION — chug, assessed by chug-loop (2026-09-28, cycle 65)

**MANDATORY fresh eval** — the queue is EMPTY again (cycle 64 landed
all four of its rows, T124–T127, and cut v0.3.0 at wrap), so the
freshness predicate's queue half fails and loopd routed an eval cycle
(`todo_rows=0 eval_fresh=yes -> eval cycle on kimi-k3`, loopd.log
17:48:11Z). The delta corpus since the cycle-64 eval text
(~15:5xZ) is cycle 64's own working phase: eight child streams
(t127/t125/t124/t126 impl+validate), the 135-iteration cycle-64
orchestrator stream, and the v0.3.0 release evidence. The headline:
**the loop's first self-cut tag went green end-to-end** (release run
36460136826, six assets, `gh release list` shows v0.3.0 Latest), and
the era's only validator survivor (T124's M3 weak pin) gets its
carried triage → **T130**. The mandatory roadmap pull is **F10
phase 2, SPLIT per T125 → T128 (chug_collect) + T129 (chug_launch)**.
Four rows filed: T128/T129/T130/T131.

## 1. What chug does well — be brief

- **T100's wrap-time tag doctrine worked on its first firing**:
  v0.3.0 cut at cycle-64 wrap (4 items since v0.2.1, feature → minor
  bump), tag message = generated notes, check-tag-version pairing
  verified, release workflow green in 2m18s, six assets published.
  The loop now ships its own releases.
- **Fourth all-PASS validation cycle** (58/61/62/64): 3 kimi rounds,
  15 mutants, ONE survivor (triaged this eval), zero blocking
  findings. Adversarial validation is a calm gate.
- **T124's 78/80 first-try feature landing** (+1106 lines): the
  biggest single-row diff of the era; budget_low fired INSIDE the
  T18 margin at remaining_iters=8 and the child still wrapped and
  committed — the margin did its job under maximum load.
- **T120's verify-then-kill is fully internalized**: exercised ×3 in
  cycle 64 (all render artifacts, read-backs intact, zero kills) and
  once already this eval (a mangled spec-file view in the
  orchestrator's own context — read-back proved the payload intact,
  no action). The rule costs one read and saves children.
- **T121's 200-cap sized right**: cycle 64 ran eval + 4 items +
  release in 135/200 iterations; cycle 63's routine cycle was 49/200.
- **T81 two-model routing boring in both directions**: glm
  orchestrated cycle 63 cleanly, kimi cycle 64 cleanly, routing
  decisions logged every cycle by loopd, not by model judgment.

## 2. Incidents worth fixing

### I1 — T124's M3 survivor: the missing-`.chug/` pin is substring-shadowed → T130 (FILED)

The one carrying defect of the era. T124's kimi verdict
(d1790615724-24): deleting the `.chug/`-existence check
(src/mcp_serve.rs:228-234) goes undetected because
`chug_status_missing_chug_dir_names_received_path`
(src/mcp_serve.rs:605) asserts only `is_error` +
`contains(<tmp path>)` + `contains(".chug")` — and the downstream
"events file unreadable: `<tmp>/.chug/events.jsonl`" message the
mutant falls through to satisfies ALL THREE (the path embeds both
substrings). Production behavior is correct; the PIN is broken — the
T119 weak-test class, explicitly carried from the cycle-64 wrap.
Filed as **T130** (robustness, pri 2): distinctive-phrase assertion +
negative shadow guard + sibling-leg sweep, every touched leg
RED-proven against its mutant; production untouched; ~20 lines.

### I2 — eval-digest staleness check trips on the evaluator's own stream → T131 (FILED)

Observed live at this eval's first minute: the digest's rendered
reader check (`find .chug … -newer .chug/eval-digest.md … && echo
STALE`) flagged STALE because loopd regenerated the digest at
17:47:52Z and this cycle's own driver rotated `.chug/events.jsonl`
at 17:48:12Z — the newest events file at any eval is ALWAYS the
evaluating cycle's own live stream, so the check is guaranteed to
trip and META-META-SPEC's remedy ("regenerate it") cannot help (the
stream keeps appending). The question the check should answer is
"does corpus OTHER than your own stream postdate the digest?"
Filed as **T131** (DX, pri 4): exclude the newest file from the
`-newer` test, render definite STALE/FRESH verdicts + one
explanatory sentence, one shape-pinned `tests/eval_digest.rs` leg
RED-proven against the old render; ~30 lines.

### I3 — decision_log schema fumbles: +2 (era count ~20) — weighed, REJECTED again

t127-impl ×1 and t125-impl ×1 (`options must be a string, got
missing`), both self-corrected within one iteration, zero lost
records (360+ landed records era-wide). Same fresh-context-schema
class as the ~18 counted at cycle 64; the per-fumble cost stays ≤1
iteration, below the filing bar. Rejection stands with the updated
count; eval-triage record filed.

### I4 — 120s bash-cap timeouts: 4 fires across t124/t125 streams — watch, no row

`timed out after Ns (process group killed)`: t125-impl ×2,
t124-impl ×1, t124-validate ×1 — unbounded cargo invocations against
the shared cache, all self-corrected with bounded retries. Baseline
rate; the T22-era tool-description remedy stands; the
perl-alarm idiom is in every template that needs it. Watch only.

### I5 — t125-validate's 3 /tmp-sandbox fires PREDATE the T126 fix — the fix's measure starts now

t125-validate hit `path escapes cwd: /tmp/tN-slotN.sh` ×3 at
16:37–16:44Z writing mutant helper scripts — BEFORE T126 merged
(17:39Z). These are the old baseline, not evidence against the fix.
T126's measure: validators launched with the heredoc sentence in
their goal template should produce ZERO /tmp write-sandbox fires;
the next eval counts them.

### I6 — validator ceiling zone: eighth consecutive zone-free cycle (watch)

Cycle-64 validators: 21/37/16 of 50 — zero deaths, zero resumes,
budget_low never fired validator-side. The T32 sizing (50) holds;
the 50→65 raise stays rejected for the eighth cycle.

### I7 — post-commit death class: silent (fifth assessment, rejection stands)

Cycle 64: ZERO budget deaths, ZERO T63 resumes — impl children
13/37/78/35 of 80, all first-try goal-accepted. T63's resume remains
the answer when the class fires; no row.

## 3. Friction hot spots — fix assessment

- **edit_file `old` not found**: +2 impl-side (t124, t125) +1
  orchestrator-side (cycle-64 EVALUATION.md edit) — baseline rate;
  the T5 disambiguation surface exists; no new class.
- **web_fetch refusing `application/x-sh`** (t127-impl + cycle-64
  orchestrator): working as designed — binaries/scripts are refused,
  children fall back to `curl` via bash (the correct division of
  labor for a text-only tool). No row.
- **Orchestrator-side /tmp write fire ×1** (cycle-64,
  `/tmp/eval-new-body.md`): the class's 11th fire, first
  orchestrator-side; the error names the remedy and the T126
  sentence covers validators (the class's structural center). One
  fire is below the bar; noted so the next eval has the count.
- **stream_fallback organic fires**: ZERO (jq `"type":"stream_fallback"`
  over today's streams; watch stands).
- **T113 mid-turn queue-vs-steering asymmetry**: zero new evidence
  in the delta corpus; the watch stands.
- **Anti-sprint-burn guard**: cycle-64 orchestrator made 25 delegate
  calls (terminal long-polls), zero idle-iteration runs; this eval
  has launched nothing yet by design. Working.
- **digest budget_low render nuance** (carried rejection): t124-impl's
  budget_low fired at remaining_iters=8 — verified genuinely
  iters-side (iter 72 at ~1230s of 2100s wall ⇒ remaining ~870s >
  300s), so no NEW mislabel evidence; the cycle-64 rejection
  (cosmetic, below bar) stands.

## 4. Capability gaps — ROADMAP PULL: F10 phase 2 → T128 + T129 (SPLIT)

Tier 1 deferrals stand (F13/F2/F3/F4/F5 — layad endpoint re-verified
ABSENT this eval: `curl http://127.0.0.1:8420/` → 000). Tier 2
deferrals stand (F6/F7/F8 phase 2s — zero organic-demand evidence in
the delta corpus). F9 closed (cycle 62). F10 phase 1 LANDED
(cycle 64, d6264be) and the cycle-64 eval marked phase 2 READY.

**PULLED: F10 phase 2, SPLIT per the T125 calibration rule** — the
combined collect+launch estimate (~650 all-in) sits over the ~400
should-split band, and the collect/launch boundary is the natural
cleavage:

- **T128** (feature, pri 2): `chug_collect` — the read half. The
  delegate collect machinery (CollectSummary/summarize_collect/
  collect_git_commits, all private today) exposed as a read-only MCP
  tool with visibility-only lifts and its own compact renderer
  (render_collect byte-identical). Est ~250.
- **T129** (feature, pri 2): `chug_launch` — the write half,
  flag-gated by `chug mcp-serve --allow-launch` (default OFF). The
  permissions/hooks interaction the cycle-64 deferral demanded,
  resolved at filing in two moves: (1) the server flag IS the policy
  boundary — a flagless server is byte-identical read-only,
  `chug_launch` unadvertised and `-32602` if called; (2) the spawned
  child runs its OWN cwd's full policy chain (permissions/hooks/risk
  gate) — the server adds no bypass, and the child's driver.lock
  keeps the single-driver invariant. Budget ceilings 200/240,
  reject-above. Spawns through the delegate launch seams
  (CHUG_DELEGATE_BIN reuse — no second spawner). Est ~400, the band
  named in the spec with a descope boundary (the e2e wire leg).

**Phase 3 stays deferred** (notifications/cancellation/resources/
server log — no consumer until phase 2 proves the fleet shape).
**Beyond the pull: no new roadmap appends.** F11's adjacency note
stands (after phase 2, chug speaks MCP in both directions — F11
remains queued behind F10 phase 3, not filed).

## 5. Top 3 priorities

1. **T130** (robustness, pri 2) — the era's only validator survivor;
   ~20 lines; closes the weak-pin class in chug's newest module
   before T128/T129 build on that same file.
2. **T128** (feature, pri 2) — the pull's read half; proven seams,
   moderate size; makes the fleet observable.
3. **T129** (feature, pri 2) — the pull's write half; the fleet
   primitive's verb; the cycle's largest row, band-named.

(T131 pri 4 is the declared-deferrable tail — cheap tooling truth,
overlap-eligible during any validator window.)

## 6. README audit (usability)

(a) **Reading order**: correct — Install → Quickstart → chat → run →
forks → plan → TUI → Tools → risk gate → hooks → permissions → MCP →
Langfuse → specs → loopd → Development; the new "chug as MCP server"
subsection sits INSIDE the MCP section (README.md:580-606) —
integrated, not appended. (b) **Redundancy**: the mcp-serve
subsection's wire-protocol detail does not duplicate the Tools
section (checked — Tools covers the in-loop delegate surface, the
subsection covers the server surface; no drift). (c) **Staleness**:
none found — T127's Install rewrite reads true against `gh release
list` (v0.3.0 Latest, six assets) and carries no version literal.
(d) **Balance**: fine — the subsection's taxonomy/framing detail IS
the config doc for that surface. (e) **Quickstart truth**: install
one-liner serves (curl-verified at cycle-64 filing; it now resolves
v0.3.0); run/chat commands work as written. 721 lines. **No docs row
filed** — the second consecutive clean audit after T127's catch.

## Handoff

- **Work order** (with reasons): **T130** (robustness, pri 2 —
  smallest, clears the mcp_serve.rs file first) → **T128** (feature,
  pri 2) → **T129** (feature, pri 2 — shares src/mcp_serve.rs with
  T128, strictly serial) → **T131** (DX, pri 4 — tail).
  Bundle check (T45 conjunctive): T130 (~20) + T131 (~30) both pass
  (a) ≤30, but (b) FAILS (src/mcp_serve.rs vs
  scripts/eval-digest.sh + tests/eval_digest.rs — different areas) →
  NO bundle. Overlap check (T44): T130/T128/T129 all share
  src/mcp_serve.rs → serial with each other; **T131 (scripts/ +
  tests/) is disjoint from every src row and MAY overlap one
  validator window**; no doctrine rows this cycle, so the
  never-overlap rule does not bind.
- **HANDOFF ADDENDUM (cycle-65 wrap): the operator's codex-review
  intake (origin commits 2b6dad2/cc94925/5a7485b, landed mid-cycle)
  SUPERSEDES this work order.** Nine HIGH rows (T140/T141/T142/T134
  pri 1; T135–T139 pri 2) now outrank T129/T131 under bugs >
  features. The next cycle's first act is the rebase recipe in the
  cycle-65 Outcomes notes (expected conflict-free), then the queue
  in the NEW order: T140/T141/T142/T134 → T135–T139 → T129/T131.
  The release trigger is PENDING (T128's FEATURES check-off →
  v0.4.0 minor candidate, post-reconciliation only).
- **Expected kimi routing at work time** (the calls themselves are
  logged then): T130 — optional (src/mcp_serve.rs not on the
  REQUIRED list; tests-only) — skip per T16/T31 precedent, the
  RED-proofs are the evidence; T128 — exercise (T124
  optional-EXERCISED precedent; young MCP wire surface); T129 —
  exercise (spawn safety surface — the flag-gate and budget ceilings
  are mutation-worthy); T131 — skip (tooling script + pin leg).
- **SELF-SPEC**: none. **Human items**: (1) `gh auth refresh -s
  workflow` remains the proper fix for the workflow-scope class
  (carried; the SSH pushurl workaround held again). (2)
  `com.tampajohn.chug-loopd.plist` stays untracked — operator's
  launchd unit (carried since cycle 61).
- **Watch items carried**: loopd cap (cycle 64 at 135/200
  eval-shaped — healthy; re-open on a ≥180-iteration cycle or an
  iteration death); validator ceiling zone (eighth zone-free cycle);
  display-artifact watch (cycle 64 ×3 + this eval ×1, all
  render-side, payloads intact, zero kills post-T120);
  compressed-verdict harvest fidelity (cycle-64 rotation lesson
  recorded; zero losses); module sizes (api.rs 2,921, mcp_http.rs
  2,793, tgrep.rs 2,531, tui.rs 2,527 — all UNCHANGED from the
  cycle-64 census; mcp_serve.rs new at 783; watch only);
  stream_fallback (zero organic, jq-verified); T113 asymmetry (zero
  new evidence); /tmp-sandbox fires in validators (I5 — T126's
  measure: next eval counts post-fix fires, target zero);
  decision_log fumbles (I3 — ~20, rejected); digest budget_low
  render mislabel (§3 — no new evidence, rejection stands).
- **T125 recalibration counter** (the calibration rule's running
  record): cycle-64 actuals — T127 +8/-8 bullseye; T125 ~45→+159
  (3.5x, all pins); T124 ~485→+1106 (2.3x, tests ≈65% of diff);
  T126 ~25→+192 (7.7x, pin density). Pattern: pin/test density is
  THE multiplier; doctrine+pin rows undershoot hardest in ratio,
  feature rows in absolute lines. This eval's filings apply it:
  T128 ~250 (density-adjusted from ~120 src lines), T129 ~400 (the
  band named in the spec instead of pretended under), T130 ~20 and
  T131 ~30 (pin-class ratio risk acknowledged — even 3x lands ≤90,
  trivially safe).
- **Weighed and REJECTED this eval** (eval-triage records in
  `.chug/decisions.jsonl`): decision_log schema-fumble row (I3 —
  ~20 fires era-wide, ≤1-iteration self-corrections, zero lost
  records); 120s bash-cap row (I4 — 4 fires, all self-corrected,
  baseline); validator budget 50→65 (I6 — eighth zone-free cycle);
  post-commit-death-class row (I7 — silent; T63 covers); /tmp
  sandbox move (I5's class — the boundary stays; T126 teaches the
  remedy and its measure is now running); orchestrator /tmp fire row
  (§3 — one fire, below bar); web_fetch script-refusal change (§3 —
  working as designed); module-split rows (§6 watch — sizes
  unchanged); new roadmap appends beyond the F10-p2 pull (§4 — none
  credible; F11 adjacency noted, not filed); F10 phase 3
  (deferred — no consumer until phase 2 proves the fleet shape);
  F10 phase 2 filed as ONE row (~650 estimate — split per T125; the
  split IS the resolution).

## Outcomes (filled at cycle wrap — LOOP-SPEC Phase 3)

### Cycle 67 (2026-09-28/29) — routine glm freshness-skip (queue non-empty, eval fresh) — codex-review pri-2 rows

### Cycle 67 (2026-09-28/29) — routine (freshness-skip) — codex-intake pri-2 queue

**T136 — crash mid-tool-batch leaves an unresumable transcript (pri 2,
landed bf672b6, d67d434 rebased ff).** The review's second §1 HIGH
crash-safety item: tool results were persisted only after a whole tool
batch finished, so a kill mid-batch left the transcript's last assistant
message holding unanswered `tool_use` blocks the endpoint rejects on
resume — and earlier tools in the batch may already have changed files
with no persisted record. Fix (impl d67d434, glm, 98/80 iters across
two segments): `resume_messages` — the single choke point shared by run
and chat resume — calls new `repair_interrupted_tools`, which appends
one user message with an is_error "[interrupted] … effects unknown"
`tool_result` per unanswered id, in memory AND on disk, idempotently
(the exact shape the loop writes after an intact batch, so the endpoint
pairing rule is satisfied and the model is routed at re-verifying real
state instead of trusting a result that never landed). Swept legs:
`transcript::load_with_torn` drops a torn trailing JSONL line (crash
mid-append) and physically truncates it so later appends can't merge
into the torn bytes (mid-file corruption still errors loudly);
`transcript::rewrite` (the transcript.rs:77 truncating-rewrite destroy
leg) and the same-shape `todos::save` now go through new
`fsatomic::write_atomic` (same-dir pid-suffixed temp + fsync + atomic
rename, temp cleaned on failure). Validation: 6 RED-proven tests
pre-fix incl. a deterministic RLIMIT_FSIZE destroy-leg test and a
recorded-transport test simulating the endpoint's tool_use/tool_result
pairing rule over resumed request bodies. kimi REQUIRED PASS round 1
(d1790643172-7): 6/6 mutants killed with zero survivors, the 6 RED legs
independently reproduced at base, gates re-run 1056/1056 + clippy -D
warnings clean, and the load-bearing invariant verified (unanswered
tool_use only ever sits at the transcript tail, so append-at-end repair
IS the immediately-following user message; `transcript::load` demoted
to cfg(test) so no production bypass compiles). Non-blocking findings
carried for the next eval: `update_ledger` (tools.rs:749) still does a
wholesale truncating write of LEDGER.md (same shape, bounded impact);
append-at-end repair cannot rescue pre-fix-damaged chat transcripts
where user messages piled up after the unanswered assistant;
`write_atomic` doesn't fsync the parent directory after rename and
never reaps crash-orphaned `*.tmp` siblings. Recovery note: the impl
child died 80/80 mid-work (iteration budget) and the ONE T63 resume
(d1790641323-5) finished in 18 more iterations — the resume recipe's
third consecutive success (T134 fix-up, T135 impl error-death, T136
impl budget-death). Post-merge nextest 1057/1057.

**T135 — driver.lock acquisition race (pri 2, landed 611916e ff-merge).**
The cycle's first pri-2 codex-review item: acquire's read-absent →
write → verify sequence was not atomic, so two simultaneous starters
could both decide from the same absent lock and both proceed into
transcript housekeeping. Fix: the whole critical section (loop
iterations included) now runs under an exclusive advisory `flock` on
the `.chug` directory fd — a stable inode, unlike `driver.lock` itself,
which Guard::drop unlinks (a file-mutex would swap inodes under
unlink+recreate and stop excluding). LOCK_NB + bounded retries
(300×10ms acquire / 50×10ms release); unobtainable mutex degrades to
the pre-T135 best-effort path with a stderr warning — T20 never-fail,
no new abort/hang legs. Guard::drop's compare-then-delete swept under
the same mutex (same read-then-act shape). The review's missing test
leg pinned: `simultaneous_starters_only_one_decides_from_an_absent_lock`
— two racers, injectable pids beyond every pid_max, synchronized AFTER
both read the absent lock via an observe seam on the new
`acquire_with`; RED pre-fix (both decided absent), GREEN post-fix,
20/20 stable. glm impl 66/80 — first try died at iter 7 on a
SERVER-side failure (truncated SSE → malformed tool_use input JSON,
T141-adjacent; transcript clean) → T63 resume d1790637275-1 (mechanism
extended to error-deaths, not just budget) finished in 59 more iters.
kimi REQUIRED PASS: routing d1790639671-2, verdict d1790640546-3 —
M1/M3/M8 killed (M1 mutex-removal = the exact RED proof), M2
release-side sweep mutant SURVIVED (no drop-side regression test —
carried), F1 latent non-unix compile break carried (cfg(unix) consts
used ungated; project ships unix-only), F3 informational
(threads-vs-processes leg shape). The validator's chug-level
goal_complete was REJECTED — its `cargo test` check ran without a
role-keyed target dir and collided with the in-flight T136 impl
build's artifacts: the cycle-66 infra finding d1790632587-1
REPRODUCED live; verdict taken from the validator LEDGER read-back.
Post-merge nextest 1044/1044 (target-shared-main).

### Cycle 66 (2026-09-28) — routine (reconciled cycle-65 divergence first) — codex-intake queue T134–T142

**CYCLE-66 CYCLE-LEVEL NOTES (wrap).** FOUR items landed (T140,
T141, T142, T134 — every pri-1 codex-review row), two of them through
full FAIL→fix-up→re-validate arcs. kimi ran FOUR validator rounds (T140
PASS 4/4 mutants; T141 PASS 5/6, M1 weak-pin carried; T142 round-1
FAIL 2 findings → round-2 PASS 10/10 after fix-up; T134 round-1 FAIL
2 findings → round-2 PASS M1–M5 all killed after fix-up) plus one
round-1 re-run after a T63 resume (T134 validator died 50/50
mid-battery, resume d1790631787-10 finished in 17 more iterations).
Budget deaths: T141 impl #1 (13/80, stuck:repeated-error on its OWN
malformed grep regex — recovery d1790626727-4 fresh relaunch), T134
fix-up #1 (80/80 mid-gates with +456/-18 UNCOMMITTED — T63 resume
d1790634951-13 finished the gates + committed in 13 iters), T134
validator round-1 (50/50 mid-battery — resume). The T63 resume
recipe (ONE relaunch, same worktree/spec/model/budgets) went 2-for-2
this cycle — both resumed children finished their arcs. Routing ids:
T140 d1790625539-1/d1790626612-2; T141 d1790627696-5/d1790628970-6;
T142 d1790629235-7/d1790630340-8/d1790634233-12; T134
d1790630798-9/d1790632841-11/d1790636105-14; recoveries
d1790626727-4, d1790631787-10, d1790634951-13. INFRA finding for the
next eval (d1790632587-1, logged by the T134 validator and
independently re-hit by the T142 re-validator): the goal_complete
check harness runs cargo test WITHOUT the T52 role-keyed target dir,
so two loops sharing `target-shared` collide on same-named
integration-test binaries — a foreign suite's failure gets reported
against the wrong tree. Remedy candidate: role-key the check env
(validate dir for validators) or serialize checks across loops.
Carried survivors: T128 M4 (alive-render weak pin, cycle-65), T141 M1
(open-block-check mutant masked by the message_stop guard —
behavior-preserving). Release v0.4.0 cut at this wrap: 4 items + the
T128 FEATURES check-off since v0.3.0, minor bump, tags immutable.
T135–T139 (pri 2) + T129/T131 remain `todo` with specs ready — the
next cycle's cold input needs zero human words.

**T140 — goal_complete denial bypass (pri 1, landed 9b36a2e).** The
cycle's first codex-review item: a permission-denied or hook-vetoed
`goal_complete` still completed the run because `goal_summary` was
populated from the tool name alone, without the `result.is_error`
check `submit_plan` has always applied. Fix is the one-line latch
guard at driver.rs:1084 (`!result.is_error`) — complete by
construction for all three block classes (permission deny, PreToolUse
veto, risk-gate block), each of which yields an is_error result
without executing the tool. glm impl 61/80 first-try; 4 new tests RED
pre-fix (permissions_policy + hooks_policy mods, +267/-1). kimi
REQUIRED PASS (routing d1790625539-1, verdict d1790626612-2): 4/4
mutants killed by named tests (revert, negate, is_error-only,
assertion-drop), positive control proves legitimate completions still
accept, class sweep confirmed goal_summary + plan_submitted are the
only tool-name-keyed run exits and both guarded, tree byte-clean.
Worktree gates nextest 1010/1010 + clippy clean; post-merge
target-shared-main nextest 1010/1010. Outcome landed-clean
d1790626687-3. T44 overlap: T141 impl (api.rs, disjoint) flew during
T140 validation.

**T141 — SSE truncation acceptance (pri 1, landed 70ec4a7, rebased
ff from impl 20fd96a).** The second codex-review item: a proxy
returning HTTP 200 SSE that ends the body before
`content_block_stop`/`message_stop` was accepted as success —
`finish()` closed the open block, missing input became `{}`, and an
executable `goal_complete` could be synthesized from a truncated
stream. Fix in api.rs `finish()`: open block at EOF now errors
"truncated SSE stream: body ended before content_block_stop" and
`message_stop` is tracked; the error rides the retryable Connection
class (a fresh accumulator per attempt — a real transport mid-body
EOF surfaces identically). 5 new tests RED pre-fix, including the
client-level test that reproduces the review's exact symptom
(executable goal_complete synthesized from a truncated stream).
First impl child died 13/80 stuck:repeated-error on its OWN malformed
grep regex (no orchestrator fault); recovery routing d1790626727-4
chose fresh relaunch over resume (13/80 with no committed work —
resume's continuation value nil); glm impl 51/80 second-try. kimi
REQUIRED PASS (routing d1790627696-5, verdict d1790628970-6): 6
mutants, 5 killed on named tests, M1 open-block-check mutant SURVIVES
masked by the message_stop guard — behavior-preserving under the
T141 threat model, carried to the next eval alongside T128's M4;
positive control proves legitimate closed zero-arg tools still
synthesize `{}`; class-sweep claims (mcp_http SSE POST EOF→Failed,
listen EOF→reconnect, non-streaming JSON bail) verified by code read.
Post-merge target-shared-main nextest 1015/1015. T44 overlap: T142
impl (loopd.sh+events.rs, disjoint from api.rs) flew during T141
validation.

**T142 — loopd grep spoofing (pri 1, landed 3bc3169 + fix-up dd184d4,
rebased ff).** The third codex-review item: the supervisor decided
cycle OK/fail by grepping the mixed cycle log for `chug: goal
complete` — but raw model text reaches that log verbatim (events.rs
stderr deltas, the F7 raw-bytes doctrine), so a run that died on
verification or budget while SAYING the marker recorded OK, reset the
failure counter, and ran site sync. Fix (3341658, glm impl 56/80):
the verdict is the child's EXIT STATUS (driver.rs run-mode exit 0 ⟺
accepted goal; budget/abort 1, stuck 2) and the child's stdout is
captured apart from the stderr log — forged lines can neither satisfy
the marker grep nor shadow the recorded summary; the supervisor
stamps an rc-based `verdict:` line for downstream consumers. kimi
round 1 VERDICT FAIL (d1790630340-8): the primary fix verified
correct + RED-proven, but F1 the site-sync cycle_count fallback just
RENAMED the forgeable marker (`verdict: goal complete (rc=0)` still
greppable in child bytes — real probe published "cycles 2" for a
failed cycle) and F2 M5 stamp-branch inversion passed the entire
suite. Fix-up (0b643be, glm 53/80, findings + class sweep in goal per
the cycle-33 lesson): fallback counts a log only when its LAST
verdict line stamps goal-complete (the supervisor writes nothing
after child death → unforgeable), behavioral stamp↔rc ties both ways
(inversion mutant now dead), observation 3 kept behavioral
(rc-gate-drop killed only by the abort-path stdout-ledger test).
kimi re-validation PASS (d1790634233-12): 10 mutants each killed on
named tests, the original forged-marker attack re-probed end-to-end
(3 forged lines in a FAILED cycle's bytes moved neither primary nor
fallback count). Post-merge target-shared-main nextest 1027/1027.
Two infra lessons logged: (1) T134-validator's note d1790632587-1 —
the goal_complete check harness runs cargo test without the T52
role-keyed dir, so concurrent loops sharing target-shared collide on
same-named test binaries (first re-validation check rejected by
exactly this while the T134 fix-up flew; retried clean); (2) T142
re-validator hit the same class. Both are check-harness findings for
the next eval, not row blockers.

**T134 — symlink sandbox escape + doctrine drift (pri 1, landed
d5d9c28 + fix-up bfd316a, rebased ff).** The fourth codex-review item:
resolve_safe was lexical-only, so an in-tree symlink to an external
path made every file tool follow it on disk — no race needed. Fix
(8785a2a, glm impl 48/80): two-stage resolve_safe — stage 1 the
unchanged lexical pass, stage 2 walks every component against the real
filesystem (lstat sees dangling links), expands symlinks under a hop
budget (loops fail closed), refuses when resolution escapes the
canonical sandbox root (macOS /tmp→/private/tmp handled), nonexistent
tails stay legal, returned path stays lexical; one fix point sweeps
all 9 call sites (get_path read/write/edit/image, grep, glob, list_dir,
tgrep, @attach, submit_plan --out); the false README/SPEC claims
replaced with the accurate contract. kimi round 1 VERDICT FAIL
(d1790632841-11): F1 MEDIUM — glob-metachar paths bypass stage 2
(resolve_safe treated `*`/`**` as inert-missing, the glob crate follows
symlinked dirs in expansion, tgrep's corpus arm read external
CONTENTS; /etc/passwd exfiltrated end-to-end via `path="**/passwd"`
through an in-tree etcdir→/etc link; external names leaked via the
glob tool); F2 LOW — SPEC's corrected claim had no pin test. Fix-up
(f28f38c→bfd316a after T63 resume d1790634951-13 — the first fix-up
child died 80/80 mid-gates with +456/-18 uncommitted): metachar
components refused fail-closed in the model-supplied relative part +
resolve_glob_pattern confines the literal prefix of both glob-expanding
surfaces + confine_glob_match re-passes EVERY concrete match through
the full two-stage check before its name is reported or bytes read;
SPEC.md pin added, stale "resolved lexically" line corrected. kimi
re-validation PASS (d1790636105-14): all three layers verified wired
at every call site, M1–M5 all killed (round 2 finished the fix-up
child's expired M4/M5 legs), the round-1 attack re-proven dead
byte-exact with glob 0.3.4. 8 RED proofs; post-merge
target-shared-main nextest 1043/1043.

### Cycle 65 (2026-09-28) — fresh eval (kimi, loopd-routed: queue empty) — T128–T131 filed (F10 phase 2 PULLED + SPLIT); T130 + T128 LANDED (976e4ae, 85ca4c1); T129/T131 DEFERRED by mid-cycle operator intake (codex adversarial review, 9 HIGH rows T134–T142); cycle ended on a push divergence (doctrine: no mid-cycle reconcile). Cycle notes below the per-item entries.

**CYCLE-65 CYCLE-LEVEL NOTES (wrap).** TWO items landed clean
(T130, T128 — entries below). kimi ran ONE round (T128
optional-EXERCISED PASS 43/50, 6 mutants 5 killed 1 survivor); T130
SKIPPED per tests-only precedent with an orchestrator independent
M3 re-kill. Routing/verdict/outcome ids: T130 d1790620124-13 +
d1790620197-14; T128 d1790622226-15 + d1790623655-16 +
d1790623731-17. ZERO budget deaths, ZERO T63 resumes (impl 32/57 of
80, validator 43/50 — budget_low fired inside the T18 margin on the
validator's wrap leg, harmless). T120 verify-then-kill exercised
×3 by the orchestrator on its own context-view garbles (spec-file
view, decision_log view, EVALUATION edit view) — every read-back
proved the payload INTACT, zero kills, zero rework. Estimate
recalibration counter: T130 ~20→+58 (2.9x, pin density); T128
~250→+580/-84 (2.3x, feature test density) — the T125 pattern holds
(pins multiply by ratio, features by absolute lines); doctrine+
feature rows keep landing at 2.3-3x, the ~400 band's guidance
unchanged.

**The mid-cycle intake (the cycle's defining event).** At
~19:22–19:27Z the operator pushed three commits (2b6dad2/cc94925/
5a7485b): a 199-line codex adversarial review
(reviews/CODEX-REVIEW-20260928.md) + NINE rows — T140
goal_complete denial bypass (pri 1), T141 SSE truncation acceptance
(pri 1), T142 loopd grep spoofing (pri 1), T134 symlink sandbox
escape (pri 1), T135 driver.lock race (pri 2), T136 crash
mid-tool-batch unresumable transcript (pri 2), T137 loopd stale
binary (pri 2), T138 mcp.json pre-permission execution (pri 2),
T139 editable-spec-check bypass (pri 2). The operator's renumber
(cc94925) already resolved the collision with this cycle's T131.
Per bugs > robustness > features > DX, ALL NINE outrank T129
(feature, pri 2) and T131 (DX, pri 4) — so this cycle stopped
dispatching rather than work a feature above four pri-1 security/
correctness rows. T129/T131 stay `todo` with ready specs; a
deferred item is a fine outcome.

**The divergence and the handoff recipe.** The T128 flip's push was
rejected (remote moved): local main = origin/main + 2 commits
(85ca4c1 impl, b74eb26 flip) plus the wrap bookkeeping commit;
origin/main = +3 intake commits. Per LOOP-SPEC (never force-push;
no mid-cycle reconcile — T97/cycle-63 precedent) the reconciliation
belongs to the NEXT cycle's first act: `git rebase origin/main` —
expected conflict-free or a trivial TODO.md region adjacency (their
rows append at the tail; my flips edit rows T128/T130 mid-file;
EVALUATION/FEATURES/src untouched by the intake) — then
`cargo test --test todo_consistency` + the full gate runner + push.
The next cycle is a ROUTINE cycle (queue non-empty, eval fresh →
glm): after reconciling it works the queue in the NEW priority
order — T140/T141/T142/T134 (pri 1; T140 touches driver/tools →
kimi REQUIRED; T141 api.rs → kimi REQUIRED) before T135–T139
(pri 2) before T129/T131.

**Release trigger PENDING, deliberately untagged.** T128's
FEATURES.md check-off (F10 phase-2a LANDED) fires the T100 trigger
(plus 2 items since v0.3.0) — a v0.4.0 minor candidate. NOT cut:
tags are immutable and local HEAD is unreconciled-divergent; the
next cycle cuts it post-reconciliation with gates green at the
reconciled HEAD (one tag per wrap). **M4 survivor carried to
next-eval triage** (T128's alive-render weak pin — the
substring-shadow class's second sighting, both in mcp_serve.rs;
T130's sweep pattern is the remedy template). Render-artifact count
this cycle: 3, all read-back intact, zero kills. Final gates at
local HEAD: nextest 1006/1006 + build + clippy green under
target-shared-main. 5 event streams + 1 validator LEDGER harvested.

**T128** (F10 phase 2a — `chug_collect` MCP tool, feature, pri 2,
THE PULL's read half): landed 85ca4c1 (fast-forward) — `chug mcp-serve`
now serves two read-only tools. `chug_collect` answers the fleet
observer's structured-result question for any chug cwd: latest
segment verdict, accepted-goal summary, check cmd, pid-gated
liveness, bounded/`base`-scoped commit refs — built on
visibility-only `pub(crate)` lifts of delegate's T69 collect seams
(every delegate.rs hunk a signature line with a T128 doc note;
`render_collect` byte-untouched; the shared `validate_chug_cwd` keeps
`chug_status`'s error texts byte-identical so the T130 pins pass
unchanged). `base` honors the T69 no-silent-default rule. 9 new bin
legs + one wire e2e. glm impl 57/80 first-try. kimi EXERCISED
**PASS** 43/50 (routing d1790622226-15, verdict d1790623655-16):
independent gates (build, clippy -D warnings, nextest 1006/1006), 6
mutants in 2 T79 parallel batches — M1/M2/M3/M5/M6 killed; **M4
survived**: an `alive:true`→render flip on the liveness arm goes
unguarded (weak pin, low severity — the SAME substring-shadow class
T130 just swept on `chug_status`; **carried to next-eval triage** —
the class now has two sightings, both in mcp_serve.rs). Review +
post-merge nextest 1006/1006. Estimate ~250 → actual +580/-84 (2.3x
— feature-row test density, T125 calibration confirmed again).
FEATURES.md F10 annotated phase-2a LANDED. Outcome landed-clean.
Both streams + validator LEDGER harvested (the rotation lesson
recurred: the validator's fresh launch rotated the impl stream —
identified by goal_sha256 prefix, both kept).

**T130** (chug_status weak-pin strengthening, robustness, pri 2):
landed 976e4ae (fast-forward) — the era's only validator survivor
closed. All six `chug_status` fail-fast legs now assert their
DISTINCTIVE error phrase plus a negative guard against the shadow
message; the sweep (the spec's sweep-the-family clause) found a
SECOND real shadow beyond M3: the nonexistent-cwd leg's path
assertion was substring-satisfied by the no-`.chug/` message for the
same bogus path. glm impl 32/80 first-try, 7 mutants RED-proved
(each died in 6-7s to its named leg; reword-mutants proved the new
phrase assertions are unique killers). kimi SKIPPED (routing
d1790620124-13 — tests-only, non-core, T16/T31 precedent);
orchestrator independently re-killed the M3 mutant in the worktree
(neutered `.chug` check → leg FAILED → revert → 28/28 green).
Review + post-merge nextest 996/996. Estimate ~20 → actual +58
(2.9x — pin density again, safely inside the trivial band). Outcome
landed-clean. Impl stream harvested; LEDGER seed-trivial, skipped
per doctrine.

### Cycle 64 (2026-09-28) — fresh eval (kimi, loopd-routed: queue empty) — T124–T127 filed; ALL FOUR landed clean (T127 5dbab0d README install truth, T125 532c403 estimate calibration, T124 d6264be F10-p1 mcp-serve+chug_status, T126 eef7a29 validator /tmp-heredoc doctrine) — the era's FOURTH all-PASS cycle after 58/61/62. Cycle notes below the per-item entries.

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

**Cycle-64 notes.** kimi ran 3 rounds (T125 REQUIRED, T124
optional-EXERCISED, T126 REQUIRED; T127 SKIPPED per docs-only
precedent) — ALL PASS, 15 mutants total, ONE survivor (T124's M3
weak-test, low severity — the `.chug/`-existence check inert under
substring-satisfied assertions; **carried to next eval**, T119-class
pin-strengthening), zero blocking findings: the era's fourth all-PASS
cycle. Routing/verdict/outcome ids: T127 d1790612463-18 +
d1790612498-19; T125 d1790613425-20/d1790613867-21/d1790613947-22;
T124 d1790614884-23/d1790615724-24/d1790615847-25; T126
d1790616483-26/d1790617141-27/d1790617196-28 (known typo: commit
04281f9's message cites d1790613974-22 — the true record is
d1790613947-22). T44 overlap ran once clean (T125-validator ∥
T124-impl — disjoint spec-named files, strictly serial merges, one
clean rebase-ff d6264be). ZERO budget deaths, ZERO T63 resumes
(impl 13/37/78/35 of 80 — t124-impl's 78 with budget_low INSIDE the
T18 margin, first-try; validators 21/37/16 of 50). T120
verify-then-kill exercised ×3 (T124 validator goal + FEATURES edit +
EVALUATION edit all LOOKED garbled in the orchestrator's context view;
read-backs proved all three payloads INTACT; zero kills, zero rework —
the render-artifact class is confined to the orchestrator's own
viewing of long parameters; the composition-to-tool pipeline is
intact — cycle-61's lesson fully internalized). Estimate recalibration
counter (T125's rule, first cycle): T127 ~8→+8/-8 EXACT bullseye;
T125 ~45→+159 (3.5x, all pins); T124 ~485→+1106 (2.3x — over BOTH the
~400 band and the 500 ceiling: the band's proving case, filed hours
before the band existed); T126 ~25→+192 (7.7x, pin density). Pattern
for the next eval's recalibration text: pin/test density is THE
multiplier; doctrine+pin rows undershoot hardest in ratio, feature
rows in absolute lines. Harvest mechanics lesson: a validator launched
fresh into a used worktree ROTATES the impl child's events.jsonl
aside (standard rotation) — the harvest copies the current file as
the validator stream and the rotated file as the impl stream,
identified by run_start model + goal_sha256 prefix (impl LEDGER
archives were seed/routine; their substance lives in the streams +
these entries). 9 event streams + 3 validator ledgers harvested.
F10: phase 1 LANDED, phase 2 (chug_collect + chug_launch write leg —
permissions/hooks interaction) READY to file at next eval, phase 3
deferred. T121 watch: reset — zero T81-tagged fumbles this cycle.
Release: v0.3.0 cut at this wrap (4 items since v0.2.1 ≥ 3; T124 is
a feature → minor bump) — the loop's FIRST self-cut tag (T100
trigger's first firing). Queue EMPTY → next cycle eval-routes kimi.
Final gates green at HEAD under target-shared-main.

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

