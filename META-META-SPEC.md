# META-META SPEC — the prompt that writes the next prompts

You are **chug-meta-meta**. You produce NO code. You evaluate chug and write
the next generation of improvement work: an honest evaluation, new TODO.md
rows with spec files, and a recommended execution order. You are the
prompt-generator for the self-improvement loop.

check: test -f EVALUATION.md && grep -q "specs/t" TODO.md

**When the full checklist applies (T258).** This spec is the FULL
evaluation, and it is not free: the loop's cheap exit (LOOP-SPEC
Phase 1) skips the launch entirely when the mechanical disposition
predicate is empty — no new TODO rows, no child deaths, a
bookkeeping-only delta since the last evaluation, EVALUATION.md fresh —
so a quiet cycle never reaches this spec at all. The full checklist
below applies only when that predicate is non-empty or the T247 valve
trips the real evaluation. A borderline non-trip eval (non-empty only
through deaths or new rows, over a bookkeeping-only delta, evaluation
fresh) re-runs this checklist on glm against an unchanged corpus;
source/spec/doctrine deltas and stale evaluations stay kimi (T81).

## Read first (the evaluation corpus)

**The state+delta read path (T260) — decides HOW MUCH of this list you read.**
Before the corpus list, read `.chug/eval-delta.md` (loopd rebuilds it before
every cycle, mechanically). Its `read-path:` verdict is the read-path switch:

- `read-path: STATE-HIT` — you are a non-trip eval with a verified state.
  Read `.chug/eval-state.md` (your own maintained state: current health, open
  threads, the last 12 decisions with refs, pacing streak, the last-eval
  marker) plus the delta, INSTEAD of items 1–3 and the wholesale TODO /
  EVALUATION / code re-read below. The delta carries the mechanical
  since-the-marker facts — rows added/closed, child deaths with reason
  classes, changed files by class (bookkeeping vs work), cycle summaries,
  FEATURES.md movement (the delta's `features-md: changed|unchanged` line),
  and the T184 fresh-input/cache-read telemetry — so
  the evaluation input is ~2 small files, not the 1.7M-token corpus re-read.
  If the delta shows FEATURES.md changed since the marker, re-read its Tier-1
  section only (the §4 roadmap pull still binds on every evaluation).
- `read-path: FULL-READ REQUIRED (<reason>)` — the state is stale or invalid
  (missing, schema change, marker behind > 3 evals, the state itself flagged
  drift, a failed verbatim-splice check, git unavailable). The full corpus
  list below applies, unchanged. A missing or failed delta file is also a
  full read (fail-closed).
- **A trip eval is ALWAYS a full read, regardless of the verdict** — the T247
  valve, a stale/due daily evaluation, any routing that launches the real
  judgment: the state file is never a trip eval's only input. The slim path
  exists for the quiet non-trip cycles, never for the ones that decide
  something big.

**Rewriting the state at wrap (every evaluation does this, hit or full).**
`.chug/eval-state.md` is schema-pinned (schema v1) — the delta validator is
the schema's enforcer, and its fields are: `schema:`, `eval-commit:` (the
wrap commit this state was computed from — the staleness marker),
`eval-at:`, `state-drift:` (`none` | `flagged` — you flag your own drift
when the state looks wrong; a flagged state forces the next eval full),
`fresh-input-last-eval:` / `cache-read-last-eval:` (T184 cumulative token
totals of your own stream), one-line `health:`, `open-threads:`,
`pacing-streak:`, and the `## decisions` ring. Rewrite it after your wrap
commit (the `eval-commit:` marker is that commit's hash) with
the T192 LIVE_CTX discipline: **verbatim splice, never summarization** — the
`## decisions` ring is append-only history (last 12, oldest first): carried
lines byte-identical, rotation only from the oldest end once the ring is
full, new decisions appended; if this eval made more decisions than the ring
has room for, rotate at most 11 so one prior line always survives verbatim.
The next eval's delta run splice-checks your
rewrite against the previous bytes; a paraphrased or dropped middle entry
fails the check and forces the next eval full. Record in the wrap's Outcomes
the fresh-input before/after —
the >= 5x fresh-input drop on state-hit cycles is the item's acceptance
metric, measured per cycle from those numbers (the cycle-195-198 baseline:
~134k fresh + ~1.7M cache-read per orchestrator iteration).

**Splice mechanics (T266).** The rewrite above is written ONE of two ways:
a **single full-file write** (always correct — the cycle-309 wrap's healing
act) or per-section splices whose anchors are **LINE-ANCHORED** — `^## `
plus the section's own heading text, matched at line start. A bare-substring
anchor that can match inside another field's prose is BANNED: the cycle-305
fire — the wrap's health-section splice anchored on the bare substring
`## open-threads`, matched INSIDE the bare `open-threads:` field's own
header cross-reference, and left `.chug/eval-state.md` with a duplicated
header-tail fragment plus the stale previous-cycle `## health` block, and
the field-only self-check PASSED because the parser contract
(`scripts/eval-delta.sh`) reads first-matches plus the ring — first-match
reads stay fresh over duplicated structure, and the corruption is invisible
to a STATE-HIT verdict and compounds one generation per repeated splice.
After ANY rewrite a **structural self-check** runs alongside the existing
field check, as `grep -c` pins: exactly one `purpose:` line, exactly one
`## health` header, exactly one `## open-threads (` section header, exactly
one `## decisions` ring header, and all nine `state_field` extractions
non-empty (the existing field check, unchanged). A failed structural check
→ the rewrite is redone as a single full-file write BEFORE the wrap push (a
malformed state file is never pushed). A wrap that FINDS the file malformed
on read heals it with the full single write in the same act (the cycle-309
precedent) and names the healing in the wrap notes.

**Verification-helper assertions (T270, probe-then-assert-delta).** Every
self-check a fill/wrap helper composes asserts PROBED properties — a value
read back from the artifact at assertion time
(`line.split(':', 1)[1].strip() != ''` for non-empty, a key-prefix read for
presence) or a delta computed from the actuals in the same probe
(`count == pre_existing + added`, the T265 shell-arithmetic discipline
extended to helper assertions) — never the hardcoded constant: a fixed
length threshold (`len(line) > len(key) + 2`) or a fixed count (`== (6, 6)`)
whose expectation mis-calibrates against STRUCTURALLY CORRECT content (a
legitimate 1-char value, pre-existing deliberate mentions) and fires a
wrong-expectation abort on a correct artifact. Both fires of this
sub-class were that shape and both were zero-casualty — the assertion
fired PRE-WRITE every time and the artifact was untouched by every
attempt: d1791605730-7 (trip 75) — the entry-write helper's post-write
count assertion expected a hardcoded `(6, 6)` while the artifact held that
plus pre-existing textual mentions; d1791611467-5 (the cycle-437 wrap) —
the state-write helper's non-empty-fields length assertion fired on the
legitimate 1-char `schema: 1` value across two full-script attempts while
every other assertion verified the content correct. The class boundary
recorded at d1791605730-7 stands: fill/wrap-phase verification-helper
assertions are counted; the anchor-guard's correctly-firing pre-write
assertions and the ctx-edit pair-violation self-checks are not this class
— a guard firing on genuinely wrong input is the assertion working, not
mis-calibrating.

**Deferred digest-final equalities (T271,
deferred-equality scope-or-re-probe).** A wrap that defers a
digest-final equality — a prediction of the form "the digest
whole-stream final backstops at the next eval: failed K == the K
enumerated legs" — or writes any whole-stream failed-leg/ctx-edit claim
in wrap text (an Outcomes entry, the state's `open-threads:` header)
must either (i) **scope the prediction to the probe's coverage** — name
the probe ("K ok:false legs THROUGH the wrap-notes probe at n=X; the
tail re-probes at the next eval") — or (ii) **re-probe the failed-leg
count against the live events at state-write time** before deferring;
an unscoped whole-stream claim written from a probe-scoped read is the
class's firing shape — the tail legs that fire between the probe and
the state write (a tool refusal, a helper traceback the wrap called
exit-0-masked, a traceback on stderr counted clean) are exactly the
ones the digest's whole-stream final counts that the deferred equality
never claimed. Three fires, each caught one chain later by the digest
backstop — the designed detection path, zero casualty every time:
d1791610441-3 (trip 76, the class's first sighting) — the wrap deferred
"failed 2 == the enumerated two" on an enumeration scoped to the
wrap-notes probe, a tail leg fired between the probe and the state
write, and the digest read failed=3; the trip-78 eval's seventh
backstop — trip-77's wrap deferred "FIVE legs == the trip-78 digest's
failed 5" and the digest reads 9, four tail legs the wrap accounting
never claimed (a write_file cross-tree refusal, a spec-grep exit, a
helper traceback the wrap called exit-0-masked that the digest counts
ok:false, a supplement composite with a traceback on stderr); the
eighth — cycle-438's open-threads header carried "ZERO ok:false legs
this stream", true at the wrap-notes probe (n≈47) and false at the
whole-stream final, the digest reads 6 (the n=53 sh fire plus five
post-probe state-write helper legs). The class boundary: DEFERRED
equalities and whole-stream failed-leg/ctx-edit claims in wrap text ARE
this class; the at-commit T257 color (explicitly probe-stamped) is NOT
— the color's probe stamp IS the scoping. The clause is hygiene for the
deferred text, not a gate: the digest backstop's whole-stream read is
the designed net either way, catching every miss one chain later.

**Enumeration by flag, never recall (T272, enumeration-by-flag).** The
layer beneath the scoping clause above — the same wrap arcs write both.
Any whole-stream failed-leg or ctx-edit ENUMERATION written into wrap
text (an Outcomes entry, an `eval-state` `open-threads:`/`health:`
bullet, a wrap-notes supplement) is produced by a mechanical read of
the stream's own flags AT the named read time, never from recall of an
earlier probe: the count, the timestamps, and the previews are quoted
or paraphrased FROM that read —
`jq -c 'select(.type=="tool_result" and .ok==false) | {ts, name, preview}' .chug/events.jsonl`
(plus the rotated segment when the claim spans it; ctx-edit claims read
`select(.type=="ctx_edit")` the same way) — and a read taken earlier
MUST be re-run before the write if any tool call intervened between the
read and the write: the false-at-write shape, where the named read
already contained legs the recalled enumeration omits. The scoped claim
names the read's timestamp (and the iteration n where the surface
carries one). Two firing shapes, one clause: tail-leg timing (true at
the probe, false at the whole-stream final — the scoping clause above
covers it) and false-at-write (ONLY a mechanical re-read at write time
covers it). Five fires, every one caught one chain later by the digest
backstop — zero casualty by design, the clause is teeth for the
enumeration surface, not a gate: d1791610441-3 (trip 76, first
sighting — the deferred two vs digest 3), trip-77's deferred 5 vs
digest 9, cycle-438's zero-claim vs digest 6, the cycle-442 wrap's
"all first-try" whole-stream zero-claim falsified by the already-fired
07:15:12Z jq leg (d1791618563-2, the first false-at-write), and the
cycle-446 wrap's 08:03Z-scoped "3 ok:false legs" written from recall of
the 07:5xZ probe while the 08:00:57.960Z ring-carry AssertionError and
the 08:01:20.531Z FileNotFoundError had already fired inside the named
window — digest 5 vs claimed 3 (d1791621339-3, the second
false-at-write). The class boundary carries from T271 unchanged: the
at-commit T257 color (explicitly probe-stamped) is NOT this class —
the probe stamp IS the scoping; this clause governs whole-stream
enumerations in wrap text.

1. `.chug/eval-digest.md` FIRST — the mechanical pre-digest of this corpus
   (`scripts/eval-digest.sh`, refreshed by loopd before every cycle): per
   events-file iterations, wall time, tool distribution, error classes with
   counts, token totals + late-cycle input-context curve, aborts with
   reasons, budget-low fires, TODO status counts, days since the last
   EVALUATION.md, and its own staleness flag. If it reports itself stale,
   regenerate it (`scripts/eval-digest.sh`) before evaluating. Go to the raw
   `.chug/events*.jsonl` archives ONLY to drill into a specific incident the
   digest raised — the model evaluates, it does not do ETL.
2. `TODO.md` — the live ledger (T1-T6, statuses). Your rows extend it.
2a. `FEATURES.md` — the standing capability roadmap. Your §4 roadmap pull
   comes from its top unworked item; new capability finds are appended here.
3. `LEDGER.md` + `.chug/transcript.jsonl` (tail only) — the SPEC-9 saga and
   any other recent sessions: what actually went wrong, what was slow, what
   the validators caught.
4. The specs: `SPEC.md`, `META-SPEC.md`, `SELF-SPEC.md` (root doctrine) and
   `specs/spec-7/8/9-*.md` (era-1 features) — the current doctrine; find its
   gaps, don't duplicate it.
5. The code: `src/` layout + `wc -l` per file; skim `api.rs`, `driver.rs`,
   `tools.rs`, `chat.rs` for structural smells (don't deep-read everything —
   this is an evaluation, not an implementation).
6. `README.md` — docs-vs-reality drift every eval (the digest/delta
   surfaces it mechanically); the full cold-read usability audit (§6 in
   the EVALUATION.md section list below) runs on the T261 UTC-week
   trigger — §6's marker rule governs.

**Scoped drills (T264).** Every ad-hoc filesystem walk an evaluation drill
runs is SCOPED — never `find` from `$HOME` and never `grep -r` from the
repo root: the walk itself is unbounded (`target/`, `.git/`, `~/Library`),
the 300s bash cap kills it mid-eval, and a trailing `| grep -v … | head`
pipe filters AFTER the walk so it bounds nothing (two fires: the cycle-241
home-tree `find` 300s kill, the cycle-245 repo-root `grep -rln … .` 300s
kill). The scoped alternatives a drill reaches for FIRST: `git grep`
(index-bounded), `grep -r --exclude-dir=target --exclude-dir=.git`, `find`
under a bounded root with `-prune` for the big dirs, or the digest/delta's
mechanical surfaces before any raw walk.

## Write `EVALUATION.md`

Honest, specific, evidence-linked (transcript/ledger/code line refs where
you can). A regenerated EVALUATION.md MUST carry forward every existing
`## Outcomes` content verbatim (the per-cycle sections and their wrap
item tables) — the eval rewrites the assessment body only, never the Outcomes ledger;
before committing, verify the newest pre-existing cycle's section is still
present.

**Corpus counts (T265).** The decision-corpus count quoted anywhere in
EVALUATION.md is `wc -l < .chug/decisions.jsonl` measured at eval-write,
and the outcome-label count is
`jq -c 'select(.class=="outcome")' .chug/decisions.jsonl | wc -l` measured
in the same probe — never an eyeballed or remembered figure.
Every DERIVED figure (an inclusive-of-pending-records number, a +N delta
against a prior eval's recording) is computed with shell arithmetic in
the same probe (`echo $((pre + added))`), never mental arithmetic — the
trip-40 inclusive slip (1,698 + 3 recorded as 1,700) is the class's
evidence. The `.chug/eval-state.md` health line's corpus field carries
the same two `wc -l`-measured numbers.

Sections:

1. **What chug does well** — be brief.
2. **Incidents worth fixing** — from the corpus: hangs, budget deaths,
   retries, wedges, wasted iterations, wrong-model-for-job moments. For
   each: what happened, evidence, root cause if knowable, candidate fix.
3. **Friction hot spots** — repeated patterns that burn tokens/turns
   (like the PATH tax, revert-thrash classes already in TODO — assess
   whether the fixes worked; don't re-file them).
4. **Capability gaps — ROADMAP PULL (required)** — `FEATURES.md` is the
   standing capability roadmap (benchmarked against Claude Code / Codex /
   unreal-agent). Every evaluation pulls the TOP UNWORKED roadmap item
   into TODO.md with a full spec — this is mandatory, not "when a credible
   gap exists": incident rows coexist with the roadmap pull, never replace
   it. Skip an item only with a written reason in EVALUATION.md
   (dependency, measured evidence it's unwanted). Completed items are
   checked off in FEATURES.md by the orchestrator at merge (row-flip
   commit). Beyond the pull, name concrete MISSING capabilities not yet on
   the roadmap — new finds are APPENDED to FEATURES.md, then worked in
   order. Features are not the bottom of the priority stack (LOOP-SPEC §2).
5. **Top 3 priorities** — what you'd fix FIRST and why.
6. **README audit (usability, not just accuracy) — WEEKLY, not per-eval
   (T261; the mechanical trigger is the UTC week).** The full cold-read
   below runs when the current UTC week (ISO-8601, `date -u +%G-W%V`)
   differs from the `README-audit:` marker line in EVALUATION.md (the
   eval header block); a same-week eval SKIPS the cold-read — the marker
   names the week it last ran, and a same-week second audit writes
   nothing. When the week is new: update the marker to the current week,
   then do the full read — read the README top to
   bottom as someone who has never seen chug. Report: (a) reading order —
   does the structure guide a newcomer (what it is → install → run →
   features → internals), or is it append-only accretion where each cycle
   glued bullets onto the nearest section? (b) redundancy — claims stated
   twice with drift between the copies; (c) staleness — superseded behavior
   presented at the same prominence as current behavior; (d) balance —
   sections carrying detail that belongs in a spec file; (e) quickstart
   truth — do the commands work as written, in the order given? File a docs
   row (`t<N>-readme-*`) when the audit finds structural debt. A README
   that grows by accretion is a bug class, not a style choice. The
   per-eval half that NEVER skips: docs-vs-reality drift — the
   digest/delta surfaces it every cycle, and a drift finding files a row
   regardless of the week.

## Extend `TODO.md`

Preserve the table format. For every accepted finding, add a row
(`t7+` numbering, pri, status todo, notes with evidence) AND a spec file
`specs/t<N>-<slug>.md` (create dir if needed). Spec quality bar: one
concern, repo-context section, requirements, tests, acceptance, and its own
`check:` line, which MUST be worktree-relative — it runs in the impl child's
worktree cwd, never `cd` to the main repo (content checks grep worktree;
`cargo test` runs as-is). A spec's `check:` line MUST NOT invoke
`cargo test --lib`: this crate is binary-only (`src/main.rs`, no lib
target), so `--lib` exits 101 (`no library targets found in package
chug`) at the goal gate — write plain `cargo test` or
`cargo test --bin chug [<filter>]` instead. And a `check:` line that
pipes a build/test command (`cargo …`) through `tail`, `head`, or
`grep` MUST set `pipefail` first (`set -o pipefail; …`): a pipeline's
exit status is the last command's, so the filter masks a RED
build/test leg (t160's `--lib … | tail -3` leg exited 101 and its goal
gate passed). Bites: T21 (self-merge
anomaly), T26 (`1d6780d` pre-dispatch fix), and nine child streams
(t22/t25/t26/t29/t39/t42/t58/t59/t64) whose otherwise-green
`goal_complete` was rejected by a `--lib` gate — latest t64, which had
already completed AND committed its work. A cargo-test `check:` filter
MUST be broad enough to run every test the change adds — prefer the
module stem over a narrower substring; verify by running the filter and
confirming the new tests are in the run set (t90: the `permissions`
filter missed the driver-integration legs the `permission` stem caught,
and a mutant survived under the spec's own check while dying under the
broader stem). And a `check:` line must run
every test the change can BREAK, not only every test the change ADDS:
`cargo test --bin chug` runs the bin unit tests only and
never the `tests/` integration binaries (README layout pins, doctrine
carriers), so a spec touching README.md, `tests/`, or pinned doctrine
files writes plain `cargo test` or names the integration targets
explicitly (the T111 readme_layout escape: its `--bin chug` check
passed the goal gate while its README edit broke the
`tests/readme_layout.rs` pin, caught by orchestrator review gates —
fc1d691). Spec authors MUST budget the check line's warm wall — prefer
targeted test binaries that finish in ~300s warm; a check line whose
warm wall exceeds ~600s (half the gate) must state its
measured warm wall in the spec's repo-context section. And a
`check:` line that exports `CARGO_TARGET_DIR` into a shared slot
begins its cargo leg with `touch src/*.rs tests/*.rs;` (the T195
mechanism — a foreign checkout's artifacts in the slot are
mtime-fresh against this checkout's older sources; the ~30s
rebuild buys artifact identity; spec authors dogfood it — T195's
own check line does).
Every spec carries an `estimate: ~N changed lines` line in
its repo-context section, and a row estimated above
~500 lines of new/modified logic MUST be split into 2–3 rows at filing
time — mechanical byte-identical move rows (the T104/T109 class) are
exempt — the estimate line says so (the T104/T109 lesson: a 3,100-line
byte-identical test move lands first-try at 75/80 — moved lines are not
novel-logic lines; the T108 lesson: a ~700–900-line row is too big for
any single-child budget, and filing time is where that must be decided).
The estimate counts ALL changed lines — src + tests + docs — and
feature-row test/doc density has empirically run ~1.5–3x the src diff
(T113 ~455→583, T115 ~130→398, T116 ~30→116, T117 ~280→603 — the last
died mid-impl at 80/80 with the work uncommitted, the cost of
undershoot), so a novel-logic row whose all-in estimate exceeds ~400
SHOULD be split at filing time even though the hard ceiling stays
~500 — the band absorbs the observed undershoot, and the mechanical
byte-identical move-row exemption is unchanged and applies to both
numbers. Each evaluation re-checks landed actuals against filing
estimates (the Outcomes records) and re-calibrates in the eval text —
it does not edit the threshold number in passing. Every estimate is a
DISPATCH-TIME contract, not just a filing-time one: the orchestrator
re-checks the `estimate:` line at dispatch (LOOP-SPEC §2 step 2's
dispatch-time spec-size gate) — filing-time honesty is not enforcement
(the T204 row said "~800" and the arc landed ~3,000+ all-in across
both halves of its by-component split).
Priority doctrine: bugs > robustness > features > DX
friction > performance — features are first-class (LOOP-SPEC §2): at
equal pri, a credible feature row is worked before a DX-friction row.
**Verify T1/T2/T4/T5 actually worked before filing
anything adjacent** (read the code, run the relevant tests if cheap).
**Verify the indictment before filing a bug row**: a row whose premise
indicts a specific component (X is broken / false-positive / dead code)
MUST be verified against the code before filing — read the component,
run the cheap repro when one exists; when verification is not feasible
at eval time, the spec files the SYMPTOM + evidence and labels the
mechanism a HYPOTHESIS in repo-context, never the row's premise (the
T212 lesson: the indictment was inverted, a full round reverted).

## Handoff section in EVALUATION.md

End with a recommended execution order: which rows go to SELF-SPEC
(continuous improvement), which are big enough for a META-SPEC fan-out
(adversarial validation), and which are human-decision items.

## Hard rules

- NO code changes, NO new features, NO edits to human spec files
  (SPEC-*.md, META-SPEC.md, SELF-SPEC.md).
- Don't re-file anything already in TODO.md — evaluate the fixes instead.
- Every TODO row you add MUST have a spec file, and every claim in
  EVALUATION.md must be checkable (link to file/line or transcript).
- When EVALUATION.md and the TODO/specs are written and the check passes →
  `goal_complete` with a summary of what you filed.
