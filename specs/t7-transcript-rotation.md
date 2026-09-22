# T7 — Fresh run rotates .chug/transcript.jsonl

check: cargo test

## Concern

`.chug/transcript.jsonl` is append-only forever. A fresh (non-`--resume`)
`chug run` starts with empty in-memory messages but **appends** its first goal
message to whatever transcript file already exists (`src/driver.rs:196-207`),
and `--resume` loads the entire file (`src/driver.rs:249-255`,
`src/transcript.rs`). The repo's own transcript currently splices ≥7 sessions
(SPEC-4 goal at line 1, smoke runs at 175/177, SPEC-5 meta at 176/198, SPEC-9
implementation at 279, SPEC-9 meta at 292, META-META at 509). A `--resume` in
such a repo silently feeds foreign goals/sessions into the model's context.

## Repo context

- `src/transcript.rs`: `transcript_path`, `append` (OpenOptions append),
  `load`, `rewrite`. JSONL, one message per line.
- `src/driver.rs` `run_loop`: seeding + first-message append at startup;
  `resume_messages` loads + trims on `--resume`.
- `src/chat.rs`: chat mode intentionally loads the existing transcript at
  session start (see `resume_loads_existing_transcript` test) — **unchanged**.
- Companion row T3 (ledger mismatch/reseed) touches the same startup path;
  the two are independent fixes but should land in the same session if both
  are in flight. Do not break T3's assumptions about `ensure_seeded`.

## Requirements

1. On a fresh autonomous run (`chug run` without `--resume`): if
   `<cwd>/.chug/transcript.jsonl` exists and is non-empty, rename it to
   `<cwd>/.chug/transcript-<YYYYMMDD-HHMMSS>.jsonl` BEFORE the first append,
   then start a new `transcript.jsonl`. Print one stderr line naming the
   archive path.
2. `--resume` behavior is unchanged: load `transcript.jsonl` as-is, never
   archive.
3. Chat mode (`chug chat`) is unchanged: it keeps loading the existing
   transcript across turns/sessions by design.
4. Best-effort: if the rename fails (permissions), warn on stderr and continue
   appending — never abort a run over housekeeping.
5. Empty (0-byte) or absent transcript: no archive, no warning.

## Tests

- Fresh run with a pre-existing non-empty transcript → archive file exists
  with the old content; new `transcript.jsonl` begins with the new goal
  message and nothing else.
- Fresh run with absent / empty transcript → no archive file created.
- `resume_messages` path → no archive created; existing content loaded.
- Rename failure (e.g. `.chug` read-only) → run proceeds, warning emitted.

## Acceptance

- `cargo build`, `cargo clippy --all-targets -- -D warnings`, `cargo test` all
  green.
- Manual: two consecutive fresh `chug run`s in one cwd → one archive file plus
  a current transcript containing only session 2.
