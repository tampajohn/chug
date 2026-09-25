# T24 — LOOP-SPEC adopts the `delegate` tool for child launches + polling

check: grep -q "delegate" LOOP-SPEC.md && cargo test

## Why (evidence)

Cycle-9 evaluation finding **N5**: the `delegate` tool (T23, landed
`1012dca`) has recorded **zero calls** in the cycle-7 and cycle-8
LOOP-SPEC streams (jq-verified:
`jq -rc 'select(.type=="tool_result" and .name=="delegate")'
.chug/events-20260925-{202932,204432}.jsonl` → empty). Cycle 8's
orchestrator made 37 of 47 tool calls in bash, the plurality of them
child plumbing by hand — `git worktree add`, the 6-line nohup launch
incantation, `ps -p <pid>` liveness polls, `tail` log reads, jq on the
child's `.chug/events.jsonl` — exactly the mechanics T23 internalized
and pinned. LOOP-SPEC §2 still teaches the hand-rolled template, so the
loop's newest capability is invisible to its own core mechanic. This is
an adoption gap, not an implementation gap: the tool is shipped,
tested (18 pins), README-documented, and its design brief was the
loop's own plumbing pain.

## Repo context

- `LOOP-SPEC.md` §2 step 2 — the implementation-child launch: a
  `cd <worktree> && nohup …/chug run --spec … --goal … --model …
  --max-iters 50 --max-minutes 35 > /tmp/chug-loop-t<N>.log 2>&1 &`
  code block, plus the "HARD 120s cap — launch backgrounded and poll,
  never foreground-and-wait" paragraph and the "Poll every ~60–110s
  (`ps -p <pid>` + `tail` the log + watch the worktree's
  `.chug/events.jsonl` mtime)" paragraph.
- `LOOP-SPEC.md` §2 step 4 — adversarial validation: "META-SPEC §6
  verbatim" for the goal text; launch mechanics therefore currently
  inherit META-SPEC §6's nohup template (validator budgets there:
  `--max-iters 40 --max-minutes 30`).
- The `delegate` tool (live schema, `src/tools.rs:146`):
  `action=launch` takes `cwd` (absolute, NOT confined to the caller's
  cwd — the one documented sandbox exemption), `spec` (absolute),
  `goal`, `model`, optional `max_iters`/`max_minutes` (**defaults
  40/35** — the loop's impl children need an explicit `max_iters: 50`
  per T21), returns the child pid + log/events paths at spawn and never
  blocks; `action=status` takes `cwd` + `pid` and reports liveness, a
  summary of the child's events.jsonl (state, last_iteration,
  budget-low/goal/abort flags), and the console-log tail — one tool
  call replaces the ps+tail+jq poll.
- `META-SPEC.md` is **human-owned doctrine** (META-META-SPEC hard
  rule): this spec must not edit it. LOOP-SPEC already overrides
  META-SPEC where stated ("except where this spec overrides") — the
  launch-mechanics change lives entirely in LOOP-SPEC.md as such an
  override.
- Cross-reference to preserve: `specs/t13-budget-low-warning.md:20`
  cites "LOOP-SPEC §2.5" — step numbering must not change; edits stay
  inside steps 2 and 4 (T19's fold-don't-renumber precedent).
- Check-line lesson applied (cycle-7 T21 anomaly): the `check:` above
  greps the **worktree-relative** `LOOP-SPEC.md` (the child's own copy
  on its branch) and never `cd`s to the main repo; `cargo test` rides
  along as the vacuous-but-harmless behavior leg.

## Requirements

1. **§2 step 2 — launch via delegate.** Replace the nohup code block
   with a `delegate` tool-call example: `action: "launch"`,
   `cwd: "/tmp/chug-loop-t<N>"`, `spec` (absolute path, as today),
   `goal` (the same goal wording as today, verbatim — children must not
   touch TODO.md/LEDGER.md etc.), `model:
   "anthropic-system.ai.glm-5-3-flash"`, **`max_iters: 50`,
   `max_minutes: 35` explicit** (delegate's defaults are 40/35; T21's
   headroom must survive the migration). Keep the glm→kimi fallback
   rule and the "(50, not 40: …)" rationale paragraph.
2. **§2 step 2 — polling via delegate status.** Rewrite the polling
   guidance: polls are `delegate{action: "status", cwd, pid}` calls —
   each is non-blocking by construction, so the "HARD 120s bash cap /
   never foreground-and-wait" warning is reframed (the cap is why the
   tool, not nohup, is the mechanism; status replaces ps+tail+jq;
   liveness-unknown ⇒ fall back to `ps -p`). Keep the ~60–110s cadence
   and "exit of the pid = child done; then review."
3. **Fallback preserved.** One sentence: if `delegate` itself errors
   persistently (not the child — the tool), the hand-rolled nohup
   template from META-SPEC §4 remains the fallback launch path; note
   the fallback in the ledger.
4. **§2 step 4 — validator launch via delegate.** State that the
   validation child launches the same way (`delegate` launch; model
   `anthropic-system.ai.kimi-k3`; `max_iters: 40`, `max_minutes: 30`
   per META-SPEC §6's budgets; the §6 goal text stays verbatim), as a
   LOOP-SPEC override of §6's nohup mechanics; META-SPEC.md is not
   edited.
5. **No renumbering, no scope creep.** Steps stay numbered as today;
   §1, §3, hard rules, and the goal section are untouched except that
   the hard-rule about ONE child at a time may gain a trailing clause
   naming delegate as the launch surface (optional, one line).
6. README: no change (internal loop doctrine).

## Tests

Docs/doctrine item — no cargo tests apply. Acceptance greps (run by
implementer, then independently by the validator):

- `grep -c 'delegate' LOOP-SPEC.md` increases and the step-2 template
  contains `max_iters: 50` (T21's number survives);
- the goal wording inside the new template is byte-identical to today's
  (`DO NOT touch TODO.md or LEDGER.md` present verbatim);
- `grep -n "§2.5" specs/t13-budget-low-warning.md` still resolves to
  the harvest step (numbering unchanged);
- `git diff --stat` shows LOOP-SPEC.md only;
- `grep -c nohup LOOP-SPEC.md` may remain >0 only in the
  fallback sentence (requirement 3) — the main template is gone.

## Acceptance

- `check:` green from the worktree (content leg on the branch's own
  LOOP-SPEC.md + behavior leg), and again in main post-merge.
- Adversarial validation (REQUIRED — loop/spec doctrine, LOOP-SPEC
  §2.4): confirm (a) the launch template's parameters match T21/T23
  ground truth (50/35 impl, 40/30 validate, delegate defaults not
  relied on); (b) the goal text is verbatim; (c) no renumbering and
  t13's §2.5 ref resolves; (d) META-SPEC.md untouched; (e) the
  fallback path is named exactly once; (f) the spec's own `check:` does
  not `cd` to the main repo (T21 lesson).
