# T119 — T115 weak-test pins: multibyte `goal_tail` leg + `run_start` call-site wiring pins

check: cargo test --bin chug

## Repo context

T115 (ed6c96f, cycle 61) landed the goal-integrity surface: delegate
launch echoes `goal_bytes`/`goal_sha256`/`goal_tail`
(src/delegate.rs:354-369; `goal_tail` is the T25 tail-anchored
≤120-CHAR preview) and `run_start` records `goal_sha256`
(src/eventlog.rs:110, `goal.map(goal_sha256)` — hex when the mode has
a goal, `null` in chat, always present). The kimi validator PASSed
with 3 mutants SURVIVING → 2 non-blocking weak-test findings carried
to this eval (LEDGER-t115-validate + cycle-61 Outcomes):

1. **No multibyte-goal leg** — `goal_tail` is chars()-based by spec
   (T25's rule), but every fixture is ASCII, so a bytes-vs-chars
   mutant (e.g. slicing `&goal[goal.len()-120..]` — panics or splits a
   char on multibyte input) is invisible to the suite.
2. **`run_start` call-site wiring unpinned** — the eventlog-level legs
   pin the field's shape given `Some`/`None`, but nothing kills the
   CALL-SITE mutants: driver run/plan passing `None` instead of
   `Some(goal)`, or chat passing `Some(...)` instead of `None`
   (chat.rs:185 carries the intentional-null comment).

All legs land in bin-internal test modules (src/delegate/tests/,
src/driver/tests/, src/chat.rs tests) — no `tests/` integration or
README surface, so `--bin chug` runs every test the change adds AND
every test it can break (T114 BREAK-side analysis: no integration pin
reads these modules).

estimate: ~70 changed lines (tests-only)

## Requirements

1. **Multibyte `goal_tail` leg(s)** (delegate launch echo tests):
   a goal whose tail crosses a multibyte UTF-8 boundary past the
   120-char cut — assert the returned tail is exactly the last ≤120
   CHARS (char-boundary-safe: no panic, no replacement char, no split
   sequence), with at least one 4-byte scalar (emoji) and one 2-byte
   scalar in the boundary window; assert `goal_bytes` stays the UTF-8
   BYTE length on the same input (bytes-vs-chars is pinned in BOTH
   directions on one fixture). The leg must kill a byte-slicing
   mutant: RED-prove by temporarily replacing the chars()-based tail
   with a byte slice (or document the equivalent mutation).
2. **Driver call-site wiring pin** (src/driver/tests/ — the
   scripted-provider integration shape already used for run_start
   assertions): a minimal `run` records `goal_sha256` equal to
   `eventlog::goal_sha256(<the exact goal>)` — non-null, correct
   value. Kills the driver `Some→None` mutant. A plan-mode session
   pins the same leg if the plan path has its own call site.
3. **Chat call-site wiring pin**: a chat session's `run_start` has
   `goal_sha256: null` AND the key present — asserted at the chat
   session level (not just the eventlog helper level), killing the
   chat `None→Some` mutant.
4. Every existing T115 pin green unmodified (eventlog known vectors,
   delegate schema freeze, count pin — update the count pin only by
   the exact delta of legs added, with the pin's own RED leg
   re-verified).

## Tests

The legs above ARE the tests; each names the mutant it kills (per
the era's RED-prove discipline).

## Acceptance

- `cargo test --bin chug` green; the full suite (`cargo test`) also
  green at review (orchestrator runs it regardless).
- Mutation evidence in the child's goal summary: each new leg shown
  RED against its named mutant (temporary, reverted).

## Out of scope

- `goal_pack` wiring (T117's own spec carries its call-site legs).
- Any production-code change — this row is tests-only by construction;
  if a leg cannot be written without a production change, the child
  stops and reports instead of expanding scope.
