# SPEC 5 — chat input UX: @files, /help layout, autocomplete

Three cohesive upgrades to the `chug chat` input dock (tui.rs, chat mode
only — run-mode TUI and headless behavior unchanged). Read SPEC-4 and
src/tui.rs / src/chat.rs first.

## 1. `@file` attachments in chat input

Users reference files inline; contents ride along with the objective/steering
message.

- Syntax: `@<path>` where path is non-whitespace chars (allow quoted
  `@"path with spaces"`). Tokenized from the submitted text; resolved against
  chat cwd with the SAME path-safety as tools (no `..` escapes).
- On submit, each valid `@path` expands; the LLM-bound user message becomes:
  ```
  <typed text with @mentions left inline>

  <file path="src/main.rs">
  <contents, capped at 2000 lines / 50_000 chars, truncation noted>
  </file>
  ```
  Multiple files append in order. A directory expands to its `list_dir`-style
  listing (same 500 cap). A missing/unreadable path becomes an inline
  `[file not found: <path>]` note in the message — never an error, never an
  extra LLM round-trip.
- The activity stream shows the typed text only (not the expanded contents),
  plus a dim `attached: a.rs, b.py` line when expansion occurred.
- The transcript stores the EXPANDED message (resume-safe, model-visible
  history stays true).

## 2. `/help` as a real list

Current: one long wrapped line. Fix: each command its own activity-stream
entry (or one entry with `\n`s — whichever the draw path already wraps
correctly; pick the one that renders as separate rows). Include the new
`@file` syntax and Tab completion in the output. Format:

```
/help — commands
  /spec <path>   load/replace spec file (/spec alone clears)
  /goal <text>   set persistent goal (/goal alone clears)
  /check <cmd>   verification command for goal_complete (/check clears)
  /model <id>    switch model
  /budget <i> <m> per-turn iteration/minute budgets
  /ledger        focus ledger pane
  /quit          exit
  @path          attach a file to your message
  Tab            complete /commands and @paths
```

## 3. Tab autocomplete in the input dock

- Trigger: Tab pressed in the chat input dock.
- Token: the whitespace-delimited word ending at the cursor.
- `/`-token → candidates = slash command names (prefix match, case
  sensitive). `@`-token → candidates = file paths under chat cwd:
  - Source: `git ls-files` when inside a git repo (fast, gitignore-aware);
    else walk the tree skipping `.git`, `target`, `node_modules`, depth ≤ 6,
    capped at 5000 entries. Build the index LAZILY on first `@`-Tab; cache it
    (no rebuild per keystroke; rebuild if index older than 30s).
  - Match: case-insensitive SUBSTRING match on the path (not just prefix —
    `@driv` should find `src/driver.rs`), ranked: basename matches first,
    then shorter paths. Cap 20 candidates.
- Behavior: single candidate → complete inline, done. Multiple → complete the
  longest common prefix; if unchanged, open a one-row candidate strip
  directly above the input dock showing up to ~8 (with scroll indicator);
  Tab/Shift-Tab cycles the strip, typing any non-Tab key closes it, the
  highlighted candidate replaces the token on the next Tab.
- Esc closes the strip without completing. Enter while strip open = accept
  highlighted candidate (does NOT submit); Enter with strip closed = submit
  as today.

## Files

- `src/attach.rs` — new: @mention tokenizer, expansion (file/dir/missing),
  with the message-format constants.
- `src/complete.rs` — new: token-at-cursor, slash completion, file index
  (build/cache/refresh), substring rank, common-prefix.
- `src/tui.rs` — candidate strip draw, Tab/Esc/Enter handling in chat input,
  /help multi-line, `attached:` indicator line.
- `src/chat.rs` — submit path routes through attach expansion before the
  message reaches the driver.

## Tests (no terminal)

- Tokenizer: inline mentions, quoted paths, punctuation adjacency
  (`@a.rs,`), `@` alone, email-like `a@b` NOT a mention.
- Expansion: file contents + cap note, directory listing, missing → inline
  note, multiple in order, path escape rejected (`@../x`).
- Complete: common-prefix, single-candidate inline, substring ranking
  (basename first), git-index fallback to walk, 30s cache expiry logic,
  cycle wraparound.
- /help reducer: emits multi-row activity entries.

## Acceptance

- Gates clean: build / clippy -D warnings / test.
- Existing chat behavior unchanged except where specified.
- pty smoke: type `/he` Tab → `/help`; Enter → multi-row help renders; type
  `@SPEC-4` Tab → completes to `@SPEC-4-interactive.md`; submit objective
  with it → transcript message contains `<file path="SPEC-4-interactive.md">`.
