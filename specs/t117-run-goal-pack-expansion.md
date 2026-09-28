# T117 — F9 phase 2a: run-side goal expansion (`chug run|plan --goal "/name args"`)

check: cargo test

## Repo context

F9 phase 1 (T113, 55fe207) landed chat-side slash-command packs:
`src/commands.rs` discovers `<cwd>/.chug/commands/*.md` (fail-open,
T83-hooks precedent) and `expand()` maps a name + args to
`Expansion::{Body, Unknown, Empty}` with `$ARGUMENTS` substitution
(built-ins win in chat by parser precedence). Phase 2 was deferred
"sequenced after T115" (EVALUATION.md cycle-61 §4) because run-side
expansion rewrites the goal text and its semantics had to be defined
against T115's `goal_sha256` integrity surface. **T115 landed in cycle
61** (ed6c96f): `eventlog::write_run_start` records `goal_sha256`
(hex when the mode has a goal, `null` otherwise, always present;
src/eventlog.rs:110) and delegate launch echoes the parent-side hash.
The sequencing dependency is satisfied; this row is the run side.

Call sites that matter: `main.rs` run path (goal parsed at
src/main.rs:59, flows into `driver::run_loop` ~:330-440) and plan path
(~:390+); `chat.rs` writes `run_start` with `goal_sha256: null`
(src/chat.rs:185). `commands::expand(cwd, name, args)` is reusable
as-is; its `Expansion` enum is exactly the run side's outcome set.

estimate: ~280 changed lines (≈90 production + ≈160 tests + ≈30 README)

## Requirements

1. **Invocation parse.** New pure helper in `src/commands.rs`
   (`parse_invocation(goal) -> Option<(name, Option<args>)>` shape):
   a goal starting with `/` immediately followed by a non-whitespace
   name is an invocation — name runs to the first whitespace, args are
   the trimmed remainder (`None` when empty). A goal that is `/` alone,
   `/`+whitespace, or not `/`-prefixed is NOT an invocation
   (passthrough, byte-identical behavior — zero-cost leg, pinned).
2. **Run + plan expansion.** Both `chug run` and `chug plan` resolve
   the goal through `commands::expand` at the CLI boundary (before the
   driver/plan loop starts, before any `.chug/` writes):
   - `Body(expanded)` → run with the EXPANDED text as the goal; one
     stderr line: `chug: goal expanded from pack '<name>'`.
   - `Unknown(packs)` → hard error, non-zero exit: stderr names the
     received `/name` and the available packs (or "no packs discovered"
     + the pack dir path) — the remedy-naming doctrine, never a silent
     literal run of a typo'd pack name.
   - `Empty` → hard error, non-zero exit naming the pack and that it
     expands to empty.
3. **`run_start` gains `goal_pack`** — the pack name when expansion
   fired, `null` otherwise, the field ALWAYS present (the T115
   always-present pattern). `goal_sha256` hashes the EXPANDED goal
   (what the model actually received — transmission truth);
   `goal_pack` records the transformation. Chat passes `null`
   (chat-side expansion is per-turn, T113 — unchanged).
4. **Integrity-surface honesty (spec-pinned doc line).** When
   `goal_pack` is non-null the child's `run_start` `goal_sha256` will
   NOT match a parent's delegate-launch echo (parent hashes the
   literal `"/name args"` argv; child hashes the expanded body) — that
   is the expansion, not a transmission garble; the parent's
   composition check (`goal_tail`) remains its garble surface. README
   says this verbatim-ish next to both surfaces.
5. **README.** The packs subsection's "Phase 2 (deferred…)" paragraph
   is rewritten: phase 2a behavior documented (invocation, expansion,
   `goal_pack`, the hash-difference honesty line), remainder
   (frontmatter, Tab completion — T118) named as still-pending.
   `readme_layout` set-equality stays satisfied.

## Tests

- `parse_invocation` legs: with args / without args / `/` alone /
  `/`+space / leading-space (not an invocation) / plain text.
- Resolution legs (helper or main-level): hit expands + names pack;
  unknown errors naming available packs; empty-body errors;
  non-invocation passes through byte-identical.
- `run_start`: `goal_pack` present-with-name when expanded; `null` but
  key-present on a plain goal (both at the write_run_start level AND
  wiring legs at the run + plan + chat call sites — the T115
  Some↔None survivor class applied to the new field).
- `goal_sha256` over the expanded text: a known-vector leg (expanded
  body's sha matches the recorded hash; the literal `"/name args"`
  sha does NOT).
- Existing T113/T115 pins green unmodified.

## Acceptance

- `cargo test` green (plain — the README touch makes the BREAK side
  include `tests/readme_layout.rs`; T114's rule).
- Live smoke (orchestrator or validator): a temp cwd with
  `.chug/commands/smoke.md`, `chug run --goal "/smoke hello"
  --max-iters 2` → the run's `run_start` shows `goal_pack: "smoke"`
  and `goal_sha256` == `shasum -a 256` of the expanded body.

## Out of scope

- Frontmatter parsing and Tab completion (T118, same F9-p2 split).
- Chat-side changes (T113's surface is done).
- Loop-side adoption (LOOP-SPEC invoking packs) — zero doctrine edits.
