# T113 — F9 phase 1: slash-command packs (.chug/commands/*.md, chat invocation)

check: cargo test

## Repo context

FEATURES.md **F9 (Slash-command packs)** — the top unworked roadmap item
(Tier 2; F8-p1 landed in cycle 60): "`.chug/commands/*.md` repo-local
commands invocable from chat (`/review`, `/triage`) and as run goals.
Community-extensible without code." Benchmark: Claude Code's
`.claude/commands/` skills. chug's mission twist: packs must ALSO serve
the autonomous loop.

SPLIT per the working rules (the ~500-line filing-time ceiling — chat +
run surfaces together estimate over it): **this is phase 1, the chat
surface** — discovery, expansion, invocation, `/help` discoverability.
Phase 2 is DEFERRED with a written reason in EVALUATION.md cycle-61 §4:
run-side invocation (`chug run --goal "/triage …"` expansion in
`main.rs`), tab-completion integration (`src/complete.rs`), and
frontmatter — phase 2's goal-rewriting must define itself against
T115's `goal_sha256` run_start field (raw vs expanded goal), so it is
sequenced after T115 lands.

Directory semantics (a named choice, following the T83 hooks / T90
permissions precedent): `.chug/commands/` lives in the run cwd's
gitignored `.chug/` — per-checkout, no search chain, no CLI flag; a
worktree child has its own (or none). "Community-extensible" = drop a
markdown file in, no code, no rebuild. The committed-repo-pack
alternative (un-gitignoring one subdirectory) is rejected for phase 1:
`.chug/` gitignore uniformity is a documented invariant (hooks,
permissions, todos all live there), and distribution can ride phase 2
if organic use wants it.

Existing seams (read first):

- `src/chat.rs` `parse_slash` (~line 81) parses `/name args` into
  `SlashCommand`; unknown names yield `SlashCommand::Unknown(String)`,
  rendered today as an activity-stream "unknown command" line. Built-ins:
  spec/goal/check/ledger/model/budget/quit/help.
- `src/todos.rs` is the in-crate precedent for a small `.chug/`-backed
  store module with lazy load + corrupt-error behavior.
- Tool-style errors name the remedy (T41/T85/T88 doctrine).

estimate: ~455 changed lines (≈200 production + ≈220 tests + ≈35 README)

## Requirements

1. **`src/commands.rs`** (new module): discovery of `.chug/commands/*.md`
   in the process cwd. Name = file stem (exact, case-sensitive); non-`.md`
   files ignored; names enumerate sorted. Missing directory = empty set
   (zero cost, like hooks). Unreadable/invalid UTF-8 file = skipped with
   one stderr note at load (fail-open, hooks precedent).
2. **Expansion**: `expand(name, args)` loads the file body and
   substitutes `$ARGUMENTS` with the args string (empty string when no
   args). When the body contains no `$ARGUMENTS` token and args are
   present, append `\n\n<args>`. Unknown name → error naming the
   available pack commands (remedy-naming doctrine).
3. **Chat invocation**: where the chat dispatch today renders
   `SlashCommand::Unknown(name)`, first try `expand(name, args)`; a hit
   sends the expanded body as the user message (a normal turn) with a
   one-line activity-stream note (`/<name> → .chug/commands/<name>.md`);
   a miss keeps today's unknown-command line, extended to name the
   available pack commands when any exist. **Built-ins always win**: a
   pack file named `goal.md` is shadowed by `/goal` (pinned test).
4. **`/help`** gains one line naming the pack directory and how many
   packs were discovered (discoverability; count from the same
   discovery call).
5. **README**: new subsection under "Interactive mode (`chug chat`)"
   documenting the pack dir, `$ARGUMENTS`, built-in precedence, and the
   per-checkout semantics (hooks/permissions precedent named) — integrated,
   not appended to a random section; the Development layout line gains
   `commands.rs` (readme_layout pin — run the FULL suite, not
   `--bin chug`).

## Tests

6. commands.rs unit tests: discovery (missing dir, non-md ignored,
   sorted enumeration); expansion (`$ARGUMENTS` present/absent/no-args
   legs); unknown-name error lists available names; corrupt file
   skipped-with-note.
7. Precedence pin: a pack file named `goal.md` does not shadow `/goal`.
8. Chat-dispatch test at the tightest seam the code allows: `/review
   the diff` with a pack present produces a user message containing the
   expanded body; an unknown `/nosuch` names available packs.
9. `/help` renders the pack line (count asserted).

## Acceptance

- `cargo test` green (FULL suite — this touches README, so the
  readme_layout integration pins must run; `--bin chug` alone is
  insufficient), clippy clean.
- Manual smoke leg (child records in its ledger): a real `chug chat`
  is NOT required; the dispatch-seam tests carry the behavior. The
  orchestrator's review gates re-run the suite.

## Out of scope (phase 2, deferred — EVALUATION.md cycle-61 §4)

- Run-side invocation (`chug run --goal "/triage …"` expansion),
  tab-completion integration in `src/complete.rs`, frontmatter
  (description/allowed-tools), MCP exposure, committed (un-gitignored)
  pack distribution.
