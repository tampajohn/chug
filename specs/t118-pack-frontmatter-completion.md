# T118 — F9 phase 2b: pack frontmatter (`description:`) + `/help` descriptions + Tab completion of pack names

check: cargo test

## Repo context

F9 phase 1 (T113, 55fe207) landed `.chug/commands/*.md` discovery +
`$ARGUMENTS` expansion + chat invocation; phase 2a (T117, this queue)
lands run-side expansion. Remaining phase-2 scope is the chat-UX pair:
**frontmatter** (a metadata header so packs can describe themselves —
the Claude Code skills shape, where frontmatter carries
description/allowed-tools) and **Tab completion** of pack names.

Surfaces: `src/commands.rs` (`Pack{name, body}` — discovery, sorted,
fail-open); `src/tui.rs` (`help_text` ~:831 renders the pack line with
a live discovery count; `complete_tab` ~:593-631 completes `/`-tokens
against `complete::slash_candidates` over the const `SLASH_COMMANDS`);
`src/complete.rs` (pure completion engine: `token_at_cursor`,
`slash_candidates`, `decide` — terminal-free by design). Completion
is already wired into the TUI (Tab handling at src/tui.rs:530/593), so
this row extends candidate sources, not key handling.

estimate: ~260 changed lines (≈90 production + ≈150 tests + ≈20 README)

## Requirements

1. **Frontmatter parse (commands.rs).** A pack file whose FIRST line
   is exactly `---` carries frontmatter: lines up to the next line
   that is exactly `---` are metadata and are STRIPPED from the body
   before storage/expansion. The one supported key is
   `description: <text>` (single line, trimmed); unknown keys are
   ignored silently (forward-compat — allowed-tools et al. may follow
   in a later phase). An unclosed fence (no closing `---` line) is NOT
   frontmatter: the file is body-as-written, no metadata (pinned).
   An empty description value yields `None`.
2. **`Pack` gains `description: Option<String>`** (derived Eq stays).
3. **`/help` descriptions.** The pack line lists each pack as
   `/name — description` when a description exists, bare `/name`
   otherwise. The unknown-command remedy line stays names-only
   (remedy lines stay short) — pinned.
4. **Tab completion of pack names.** `/`-token candidates become
   built-ins (existing `SLASH_COMMANDS`, `/help` order) followed by
   discovered pack names (sorted), EXCLUDING any pack name that
   collides with a built-in (the shadow rule: built-ins always win —
   a `goal.md` pack never appears as a second `/goal` candidate).
   A pure merge helper in `complete.rs` keeps the engine testable;
   the `tui.rs` call site passes `commands::names(&chat.cwd)` (a
   `read_dir` per Tab is trivially cheap — the 30s `FileIndex` TTL
   precedent covers the expensive tree walk, not this).
5. **README.** The packs subsection documents frontmatter (format,
   description key, strip semantics, unknown-key posture) and pack
   Tab completion; the phase-2 paragraph is rewritten to name only
   what remains open after this row (if anything). `readme_layout`
   set-equality stays satisfied.

## Tests

- Frontmatter legs: parse+strip (body excludes the block at expansion
  time too), absent (byte-identical body), unclosed fence → whole file
  is body, unknown key ignored, empty description → `None`,
  description containing colons keeps everything after the first `:`,
  frontmatter after a leading blank line is NOT frontmatter (first
  line must be exactly `---`).
- `/help` rendering legs: with descriptions, mixed (some bare), zero
  packs unchanged.
- Completion merge legs: packs appended sorted after built-ins,
  shadow-collision dedup, empty pack set = byte-identical current
  behavior, prefix filtering still applies to pack names.
- T113 expansion pins green with a frontmatter-carrying pack
  (expansion sees the stripped body).

## Acceptance

- `cargo test` green (plain — README touch; T114's BREAK-side rule).
- Manual smoke (orchestrator or validator): a pack with frontmatter
  renders its description in `/help` and Tab-completes by name in a
  real `chug chat` session (or the closest TUI-fixture demonstration).

## Out of scope

- `allowed-tools` or any other frontmatter key's SEMANTICS (ignored
  keys are stored nowhere; a later phase gives them meaning).
- Run-side surfaces (T117 covers them).
- Completion caching (a `read_dir` per Tab is not a hotspot).
