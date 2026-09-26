# T69 — `delegate` gains a `collect` action: structured child result

check: cargo test --bin chug delegate

## Why (evidence)

FEATURES.md **F1** (Tier 1, top unworked), pulled per META-META-SPEC §4's
mandatory ROADMAP PULL (operator directive `259b5ed` + `dc18a7a`: "next
fresh eval MUST pull F1 then F13"). Today, learning WHAT a finished child
accomplished is git/files archaeology: the orchestrator (or a chat user)
must `git -C <worktree> log` for commit refs, jq the child's
`.chug/events.jsonl` for the `goal` line's summary, and scroll
`delegate.log` for the gates run — three hand-rolled surfaces re-derived
every time. `status` answers only "is it alive and where is it", never
"what did it produce". Benchmark: Claude Code's Task tool returns the
subagent's final report to the caller as one structured result. This is
the gap that makes `delegate` a real Task-tool equivalent for chat AND
loops (FEATURES.md F1 wording).

The structured sources already exist — this item READS them, it does not
create new child behavior:

- the child's `.chug/events.jsonl` `goal` line carries
  `"outcome":"accepted"` + `"summary"` (`src/eventlog.rs:212`) — the
  `goal_complete` summary verbatim; rejected verdicts carry
  `outcome:"rejected"` + `reason` (`:218`);
- the `verifying` line carries `"cmd"` (`src/eventlog.rs:207`) — the
  check command that gated the verdict;
- the `abort` line carries `"reason"`;
- commit refs are one bounded `git log` away in the child's cwd (T20's
  `build_info::resolve_head` is the in-binary runtime-git precedent:
  best-effort, every failure leg degrades, never fatal).

Adoption lesson this spec MUST honor (cycle-9 eval N5): T23 shipped
`delegate` and it saw ZERO calls for two full cycles because no doctrine
named it — the T24 follow-up row existed solely to adopt it. A capability
without a doctrine sentence is a capability that doesn't exist, so this
item carries its own one-sentence LOOP-SPEC adoption leg.

## Scope (one concern)

A third `delegate` action — **`collect`** — that returns a finished (or
in-progress) child's structured result in ONE bounded, non-blocking tool
response:

- **verdict**: the LATEST run-segment's terminal state —
  `goal-accepted` / `goal-rejected` / `aborted` (with `reason`) /
  `running` / `starting` (segment-scoped per T58: a `run_start` resets
  the prior segment's verdict latches);
- **goal summary**: the full text of the accepted `goal` line's
  `summary` (the child's own account of what it did), present only on
  `goal-accepted`;
- **check cmd**: the latest `verifying` line's `cmd` in the segment
  (which gates gated the verdict);
- **commit refs**: best-effort bounded `git log --oneline` of the child
  cwd — optional `base` string param (`<base>..HEAD`) with a bounded
  default range when absent; EVERY git failure leg (no git binary, not
  a repo, bad ref, nonzero exit) degrades to a one-line note, never a
  tool error (T20 resolve_head precedent).

**Explicitly out of scope:** gates STDOUT capture (the events stream
carries the check cmd + verdict, not the check's output — rejected
verdicts carry the reason, which is the failure surface that exists);
transcript mining; LEDGER collection (the harvest doctrine already owns
it); killing children; waiting (`collect` NEVER blocks — the caller
long-polls with `status` + `wait_secs` first); any change to the events
schema, the child, or the `status`/`launch` render.

## Repo context

- `src/tools.rs`:
  - `delegate()` dispatcher (`:628`) — the unknown-action error at
    `:643` names the two actions verbatim ("expected \"launch\" or
    \"status\"") and has a live pin; adding `collect` must update both
    the message and the pin.
  - `read_events()` (`:899`) + `read_tail_lines()` (bounded tail reader)
    — collect's events source; reuse, never re-read unboundedly.
  - `summarize_events()` (`:1019`) + `DelegateSummary` (`:1080`) — the
    segment-scoped verdict latches. Collect needs `outcome`, `summary`,
    and the `verifying` cmd, which the summary does not currently
    retain. **T68 constraint:** `significant_ne` is pinned as EXACTLY
    the six-field wake set (`max_iters`, `last_iteration`,
    `budget_low_seen`, `goal_seen`, `abort_seen`, `abort_reason`) with
    non-vacuousness mutants; collect's new retained fields MUST NOT
    enter that comparison and MUST NOT change any `status` render byte
    (T29/T58/T64/T68 pins stay green untouched). A separate collect-side
    parse or additive non-compared fields are both acceptable; a wider
    wake set is not.
  - `reap_and_alive()` (`:1163`) — optional `pid` param gets the same
    liveness leg `status` has.
  - Launch-arg parse precedents for the optional `base` string:
    `delegate_max_tokens` (`:725`) / `delegate_resume` (`:745`) style —
    wrong JSON type → tool error naming the constraint, never a silent
    ignore.
- `LOOP-SPEC.md` §2 step 3 (Review) — gains ONE sentence: after the
  child exits, the review's first look is
  `delegate{action:"collect", cwd, pid}` (verdict + summary + check cmd
  + commit refs), then the diff/bounded-gates legs as today. No other
  LOOP-SPEC byte changes; step numbering untouched (t13's §2.5 ref).
- `README.md` — the delegate paragraph (Tools section) gains the
  `collect` action in the same integrated style T51 set (user-facing
  semantics only; mechanism stays here). User-visible item → README
  update rides the impl commit.
- Tool schema (`tool_schemas()` `:156`) — `action` enum/description
  gains `collect`; the description's action sentence must name all
  three actions truthfully (T22-class doc honesty: the live schema is
  the pinned surface).

## Requirements

1. `delegate{action:"collect", cwd}` returns one ToolResult whose text
   renders the four-field result: verdict line (with abort reason when
   `aborted`), the accepted-goal summary when accepted, the segment's
   latest check cmd when a `verifying` line exists, and the commit-refs
   block (or its degrade note). Bounded reads only; never blocks; never
   waits.
2. Verdict is the LATEST segment's: `goal-accepted` ONLY on
   `outcome:"accepted"`; `goal-rejected` on a rejected verdict with no
   later accepted verdict (the child's loop continues after a
   rejection); `aborted` (+reason); `running` when events exist without
   a verdict; `starting` when nothing was read. A `run_start` line
   resets verdict/check/summary state exactly like T58's latch reset.
3. Optional `pid` (u64) adds the same liveness line `status` renders
   (via `reap_and_alive`); absent → no liveness claim. Optional `base`
   (string) scopes the git log range; non-string `base` → tool error.
4. `wait_secs` passed with `action:"collect"` → tool error naming that
   the wait knob is status-only (launch-leg parity: naming beats
   silently ignoring).
5. Every git leg degrades to a note, never an error and never a panic;
   every events-parse leg tolerates torn/malformed lines (skip, never
   fatal) — collecting must be safe at ANY child lifecycle moment,
   including mid-run and post-harvest-cleanup.
6. Unknown-action error text now names all three actions; existing
   launch/status behavior byte-identical (all pre-T69 pins green).
7. `significant_ne` and every `status` render stay byte-identical —
   collect's retained fields never join the T68 wake set.
8. LOOP-SPEC §2 step 3 gains exactly one collect-adoption sentence;
   README delegate paragraph names `collect`; tool schema description
   names all three actions.

## Tests

- Collect parse legs through a pure seam (no I/O): accepted →
  verdict+summary+cmd; rejected → `goal-rejected` (and a LATER accepted
  verdict in the same segment flips it); abort → `aborted`+reason;
  events-no-verdict → `running`; empty/missing → `starting`; `run_start`
  segment reset (pre-resume abort + post-resume acceptance →
  `goal-accepted`); torn last line skipped; multiple `verifying` lines →
  LATEST cmd wins.
- Git legs (temp dirs, T20 precedent): real temp repo with commits on a
  branch → refs rendered, `base` range honored; not-a-repo → degrade
  note, `is_error: false`; unresolvable `base` ref → degrade note.
- Dispatch legs: `collect` routes; `wait_secs` + collect → the
  status-only error; non-string `base` → tool error; unknown-action
  message names all three actions (update the pinned message).
- Non-regression: the existing T29/T58/T64/T68 delegate pins pass
  UNCHANGED (byte-identical status renders, six-field wake set); a
  contamination check — adding collect's retained fields to
  `significant_ne` — is the mutation the validator will try, so the
  six-field pin must keep teeth (it already has field-by-field legs).

## Acceptance

- `check:` passes in the impl worktree (`cargo test --bin chug delegate`
  — covers all delegate tests; worktree-relative, no `--lib`).
- Full `cargo test` + `cargo clippy --all-targets -- -D warnings` green.
- Manual smoke (recorded in the commit message): `collect` against a
  real finished child worktree returns verdict/summary/check/commits in
  one call; against a live child returns `running` without blocking.
- LOOP-SPEC sentence, README clause, and schema description all present
  and truthful.
