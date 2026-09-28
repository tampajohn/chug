# T129 — F10 phase 2b: `chug_launch` MCP write leg, flag-gated `--allow-launch`

check: cargo test

## Repo context

F10 phase 2's write half (the T125 split; T128 is the read half).
`chug mcp-serve` today is read-only by construction (T124): no
spawning, no writes. This row lets a remote MCP client (Claude Code,
the bridge fleet, another chug) LAUNCH a bounded detached chug run in
any chug working directory — the fleet primitive's actual verb.

The permissions/hooks interaction the cycle-64 eval demanded be
thought through, resolved at filing:

1. **The server flag is the policy boundary.** `chug mcp-serve`
   gains `--allow-launch` (default OFF). Without it the server is
   byte-for-byte the phase-1/T128 read-only server: `chug_launch` is
   NOT advertised in `tools/list`, and a `tools/call` for it gets the
   taxonomy's unknown-tool error. The operator who starts the server
   decides whether writes exist — a read-only deployment cannot be
   surprised into spawning.
2. **The spawned child runs its OWN full policy chain.** The server
   adds no bypass: the child is an ordinary `chug run` process in the
   target cwd, subject to that cwd's `.chug/permissions.json` deny
   rules, `.chug/hooks.json` vetoes, and risk gate exactly as if a
   human typed the command. Single-driver safety is the child's own
   `.chug/driver.lock` (a conflicting launch fails fast child-side
   and surfaces via `chug_status`/`chug_collect`).

The spawn machinery exists delegate-side (T23): detached spawn,
`process_group(0)` + SIGHUP-ignore nohup parity, `<cwd>/.chug/
delegate.log`, the `CHUG_DELEGATE_BIN` env seam (tests substitute a
stub), pid/log/events returned at spawn. This row reuses those seams
— it does not invent a second spawner.

estimate: ~400 changed lines (src ~180, tests ~220) — this sits IN
the T125 ~400 should-split band and is filed as one row because the
split boundary is already drawn (collect vs launch) and the remaining
scope is one concern; the estimate line names the band per the
calibration rule. If the impl child measures the diff blowing past
~500, the named descope boundary is the e2e wire leg (req 6) — drop
it to a follow-up, do NOT trim validation legs.

## Requirements

1. `src/main.rs`: `chug mcp-serve --allow-launch` flag (clap,
   default false) plumbed into `mcp_serve::serve(allow_launch:
   bool)`. Flag-absent behavior byte-identical to pre-T129.
2. `tools/list` advertises `chug_launch` ONLY when the flag is set;
   without it, `tools/call chug_launch` returns the unknown-tool
   error (`-32602`, consistent with phase 1's taxonomy). Capability
   honesty: advertised ⇔ callable.
3. `chug_launch` params (JSON schema):
   - `cwd` (required string): absolute, exists, has `.chug/` — the
     SAME fail-fast validator as `chug_status`/`chug_collect`, error
     text parity, each violation naming the RECEIVED value verbatim.
   - `spec` (required string): absolute path to an existing FILE;
     rejected otherwise naming the received path.
   - `goal` (required string): non-empty after trim.
   - `model` (required string): non-empty; passed through — the
     spawned child's own auth/settings chain validates it.
   - `max_iters` (optional integer): 1..=200 — a value above 200 is
     REJECTED (not clamped) naming the received value and the 200
     ceiling (loopd's own ceiling).
   - `max_minutes` (optional integer): 1..=240 — same reject-above
     semantics naming the 240 ceiling.
4. Spawn: a detached `chug run --spec <spec> --goal <goal> --model
   <model> [--max-iters N] [--max-minutes M]` in `cwd` through the
   delegate launch seams (detached, `process_group(0)`, SIGHUP-ignore
   parity, log at `<cwd>/.chug/delegate.log`); the binary resolves
   through the existing `CHUG_DELEGATE_BIN` seam (documented reuse —
   tests point it at a stub). The goal text passes exactly one shell
   layer with correct quoting (the delegate launch path's own
   mechanism — no new quoting code).
5. Result: a text block carrying the spawned `pid`, the events path,
   and the log path (mirroring delegate launch's return shape).
   Launch failures (spawn error, lock conflict surfacing at spawn
   time) are `isError` results; NOTHING writes to the server's
   stdout (purity: the single protocol writer only) and no error
   kills the server loop.
6. README "chug as MCP server" subsection in the same commit:
   `chug_launch` described with its params, the `--allow-launch`
   flag, and a short safety paragraph (default-deny; the spawned
   child runs its cwd's own permissions/hooks/risk-gate chain;
   budget ceilings 200/240; single-driver via the child's lock).
   Phase language updated (phase 2 CLOSED; phase 3 unchanged). No
   version literals.
7. No changes to LOOP/META/META-META/SELF doctrine, delegate's tool
   schema, TODO.md, FEATURES.md, LEDGER.md.

## Tests

Bin-internal legs in `src/mcp_serve.rs`'s test module plus at most
one e2e:

- Flag OFF: `tools/list` shows only the read-only tools;
  `tools/call chug_launch` → `-32602`.
- Flag ON: `tools/list` advertises `chug_launch` with the full
  schema (required list exact: cwd, spec, goal, model).
- Validation matrix (flag ON, each an `isError`/`-32602` naming the
  received value): relative `cwd`; missing `.chug/`; nonexistent
  `spec`; relative `spec`; empty/whitespace `goal`; empty `model`;
  `max_iters: 201` and `max_minutes: 241` rejected naming the
  ceiling; boundary legs 200/240 ACCEPTED.
- Stub-spawn leg: `CHUG_DELEGATE_BIN` pointed at a stub (a tiny
  shell script written via bash heredoc in the test's tempdir — the
  T126 idiom — that records its argv+cwd to a file and sleeps as a
  fake child): assert the exact argv order/values, the cwd, and that
  the returned pid/log/events paths name real paths; the stub is
  reaped/killed at leg end.
- At most one wire e2e (flag-ON server over real stdio): initialize
  → tools/list honesty → `chug_launch` against the stub → result
  carries pid/log; deadline-bounded. THIS is the descope boundary
  named in the estimate line.
- Phase-1/T128 surfaces byte-identical with the flag OFF (existing
  tests unchanged and green).

## Acceptance

- `cargo test` green (full suite — README.md is touched).
- A read-only (flagless) `chug mcp-serve` is observably identical to
  before: same tools/list, same purity, same taxonomy.
- A flagged server launches a real detached chug run whose argv,
  cwd, and budgets match the request and whose policy chain is the
  target cwd's own.

## Out of scope

`resume`/`max_tokens` pass-through (a later row if organic demand
fires); concurrent-launch policy beyond the child's own
`driver.lock`; notifications/cancellation/resources (phase 3);
server log file; FEATURES.md check-off (orchestrator's row-flip
commit).
