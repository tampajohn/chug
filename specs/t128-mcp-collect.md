# T128 — F10 phase 2a: `chug_collect` MCP tool (read-only structured child result)

check: cargo test

## Repo context

F10 phase 1 landed d6264be (cycle 64): `chug mcp-serve`, a stdio
JSON-RPC 2.0 server skeleton with ONE read-only tool, `chug_status`
(src/mcp_serve.rs, 783 lines; README "chug as MCP server"
subsection). The cycle-64 eval deferred phase 2
(`chug_collect` + `chug_launch`) with one written reason — the
write leg's permissions/hooks interaction needed thought — and marked
phase 2 READY to file at this eval. The T125 calibration rule splits
phase 2 at filing: this row is the READ half (collect); T129 is the
write half (launch).

The collect machinery already exists delegate-side and is exactly
what an MCP observer wants: `CollectSummary` (src/delegate.rs:898),
`summarize_collect` (:935 — latest-segment verdict, accepted-goal
summary, check cmd), `commit_range` (:1021) + `collect_git_commits`
(:1035 — bounded commit refs, `base` scopes `<base>..HEAD`, every git
failure degrading to a note), rendered by `render_collect` (:1115)
for the `delegate{action:"collect"}` tool (:1080). All of it is
private today except the `read_events`/`summarize_events`/
`render_status` seams T124 already lifted. This row exposes the SAME
latest-segment result over MCP — the fleet observer's structured
"did the child finish, what did it claim, what commits landed" answer
— with zero new collection logic.

estimate: ~250 changed lines (src ~120, tests ~130 — feature-row
test density per T125 calibration)

## Requirements

1. `tools/list` gains `chug_collect` beside `chug_status`: input
   schema with `cwd` (required string — absolute chug working
   directory), `pid` (optional integer — liveness line, mirroring
   delegate collect's pid semantics), `base` (optional string — git
   ref scoping the commit-refs range as `<base>..HEAD`; absent = the
   bounded default range over HEAD). Fail-fast `cwd` validation
   IDENTICAL to `chug_status`'s legs (absolute / exists / has
   `.chug/` — factor the shared validator if clean, error text
   byte-identical either way; each violation names the RECEIVED path
   verbatim as an `isError` result).
2. The result is a single text block carrying the latest run
   segment's verdict, the accepted goal's summary when accepted, the
   check cmd, the liveness line when `pid` was passed, and the
   commit refs (bounded default or `base`-scoped) — built on
   delegate's collect seams: visibility-only `pub(crate)` lifts of
   `CollectSummary`/`summarize_collect`/`collect_git_commits` (and
   `commit_range` if needed); delegate's own `render_collect` output
   stays BYTE-IDENTICAL (the MCP side gets its own compact renderer —
   same relationship as `render_compact_status` vs `render_status`
   in phase 1).
3. Read-only by construction: no spawning, no writes anywhere; the
   git calls are read-only; every failure leg (missing/unreadable
   events, git absent/failing, malformed final line) is an `isError`
   result or a degraded note — never a panic, never a killed server
   loop (the -32700/-32600/-32601/-32602 taxonomy and stdout purity
   are untouched).
4. README "chug as MCP server" subsection updated in the same
   commit: the phase-1 "ONE read-only tool" sentence and the deferred
   parenthetical are updated — `chug_collect` described (one
   sentence + params), phase 2's remaining half named `chug_launch`
   (T129), phase 3 unchanged. No version literals.
5. No changes to delegate's tool schema, LOOP/META/META-META/SELF
   doctrine, TODO.md, FEATURES.md, or LEDGER.md (bookkeeping is the
   orchestrator's).

## Tests

Bin-internal legs in `src/mcp_serve.rs`'s test module (the phase-1
pattern, `tempfile`-fabricated `.chug/` trees):

- `tools/list` advertises `chug_collect` with the three params and
  `cwd` required (alongside `chug_status`, not replacing it).
- Happy path: a fabricated events stream holding a goal-accepted
  segment → result names the verdict, carries the summary text and
  check cmd; commit-refs section degrades to a note (tempdir is not
  a git repo) without failing the call.
- A segment with NO goal verdict (aborted run) → verdict rendered
  with the abort reason.
- Missing/unreadable events file → `isError` naming the events path
  (parity with `chug_status`'s leg).
- `cwd` validation legs: relative path and missing `.chug/` each
  refused naming the received string (proves the shared validator on
  the new tool — one leg per violation class, not a re-run of
  `chug_status`'s whole matrix).
- `base` is forwarded into the commit-range seam (assert via a seam
  or a git-fixture leg — cheap option wins; the git plumbing itself
  stays pinned delegate-side).
- One wire e2e leg (the phase-1 spawn pattern): real server, full
  `initialize` → `tools/call chug_collect` conversation over stdin/
  stdout, deadline-bounded.

The delegate-side collect tests must pass byte-unchanged (the lift
is visibility-only).

## Acceptance

- `cargo test` green (full suite — README.md is touched, so the
  `tests/` integration pins ride).
- `chug_collect` answers the structured-result question for any chug
  cwd with zero writes and zero new failure modes.
- Delegate's own `collect` behavior byte-identical (its test module
  untouched and green).

## Out of scope

`chug_launch` (T129 — the write leg, flag-gated); `resume`/
`max_tokens` surfaces; notifications/resources/cancellation
(phase 3); server log file; FEATURES.md check-off (orchestrator's
row-flip commit).
