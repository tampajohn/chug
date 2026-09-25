# T29 — delegate `status` gains optional `wait_secs` long-poll

check: cargo test

## One concern

`delegate{action:"status"}` returns instantly, so a waiting
orchestrator burns one full-context LLM iteration per poll interval
just to ask "done yet?". Give `status` a bounded server-side wait so
one tool call spans a whole wait window.

## Repo context

- `src/tools.rs`: `delegate_status` at ~729–756 builds its answer from
  three non-blocking reads: `summarize_events` (bounded ≤64 KiB tail
  parse of `<cwd>/.chug/events.jsonl`), `process_alive`
  (`kill(pid, 0)`; T28 lands the waitpid-reap leg first — coordinate
  or rebase if both are in flight), and the delegate.log tail. The
  schema entry for `delegate` is at ~148 in `tool_schemas()`.
- Evidence (cycle-11 eval O7/§3): cycle 10 spent **9 of 68 iterations
  (~13%)** on status polls for two items; each poll is a full-context
  round trip (late-cycle input ≈ 325k tokens). A `wait_secs` leg
  collapses each idle wait window into one tool call.
- Tool execution inside the driver is synchronous; long BLOCKING tool
  calls are precedented (bash runs up to the 120 s driver cap). The
  wait must be strictly bounded and never abort the run: every
  internal error leg degrades to the instant behavior.
- T24 doctrine: LOOP-SPEC §2 step 2's polling paragraph teaches
  "poll every ~60–110s with delegate status … each poll is a single
  non-blocking tool call". This item amends that sentence to teach the
  wait option (ship+adopt in one commit, the T23→T24 pattern folded
  together) — LOOP-SPEC.md IS in scope for the impl child here.

## Requirements

1. Schema: `delegate` input gains optional integer `wait_secs`
   (minimum 0, maximum 600). `action`/`cwd` unchanged; required list
   unchanged. Absent/`0` = current instant behavior — **byte-identical
   output for identical state** (pin this).
2. `wait_secs > 0` on the `status` action: block until the FIRST of
   (a) the child's events-derived state changes vs. the state
   summarized at entry (new last_event, goal_seen/abort_seen flip,
   last_iteration advance, or file creation when missing at entry),
   (b) the observed liveness flips (alive→dead), or
   (c) the deadline (`wait_secs`, hard-capped at 600) elapses.
   Internal poll cadence 2–5 s; then render the SAME status payload as
   the instant leg, plus one line `waited: <n>s` naming the actual
   elapsed seconds (instant leg prints no such line).
3. `wait_secs` on the `launch` action → tool error naming that it
   applies to `status` only.
4. Negative/non-integer/>600 values → tool error (or clamp with a
   note — pick ONE and pin it in tests).
5. Never aborts the run: missing events file, unreadable log,
   vanished cwd → immediate instant-style answer (existing notes
   legs), not a hang.
6. LOOP-SPEC §2 step 2's polling sentence gains the wait option, one
   clause, e.g. "— or pass `wait_secs: 90` to collapse each idle wait
   window into one blocking status call". The existing cadence
   guidance, fallback clauses, and step numbering stay byte-identical.
7. README delegate bullets gain `wait_secs` in one clause.

## Tests

In `src/tools.rs`'s test module (real-process fixtures already exist):

1. **Instant leg byte-identical**: `wait_secs` absent vs. pre-T29
   render on a fixed fake `.chug/` — same string.
2. **Wait returns early on state change**: against a temp dir whose
   `events.jsonl` grows mid-wait (writer thread/appender), `wait_secs:
   30` returns in well under 30 s with the new state and a `waited:`
   line.
3. **Wait returns at deadline unchanged**: static temp `.chug/`,
   `wait_secs: 2` returns after ~2 s (assert 1 ≤ waited ≤ 10 for CI
   slack) with unchanged state fields.
4. **Wait on missing events file**: returns immediately with the
   starting/note leg, no panic.
5. **Schema pins**: optional integer, min 0, max 600, required list
   unchanged; `wait_secs` rejected on `launch` with the naming error.
6. **Boundary pins**: 0 = instant; 600 accepted; 601 rejected/clamped
   per req 4's choice; negative rejected.
7. Non-vacuousness: test 2 must fail if the wait loop is gutted to a
   fixed sleep (state-change detection, not just early return) —
   comment the technique (e.g. assert on the returned last_event
   content, not just timing).

## Acceptance

- build + clippy `-D warnings` + full suite green in the worktree.
- The seven test legs present; instant leg byte-identical pinned.
- LOOP-SPEC.md diff = the one clause; steps unrenumbered; t13's §2.5
  reference still resolves.
- README truthful. `cargo test` is the check (worktree-relative).
