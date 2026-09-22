# T3 — Fresh run detects spec/ledger mismatch

check: cargo test

## Concern

`LEDGER.md` is run-scoped working memory, but `ledger::ensure_seeded`
(`src/ledger.rs`) seeds it only when the file is **absent**. A fresh
(non-`--resume`) `chug run` in a repo where a previous session left a ledger
inherits that ledger verbatim into every system prompt (`src/driver.rs`
`run_loop`, ledger re-read each iteration). Real incident (TODO T3 note,
EVALUATION.md I5): a fresh `chug run --spec SPEC-9` inherited the old SPEC-5
meta's LEDGER.md claiming "goal met" — state from a different project
poisoning a new run.

## Repo context

- `src/ledger.rs`: `SEED`, `ledger_path`, `ensure_seeded` (idempotent),
  `read` (seed fallback).
- `src/driver.rs` `run_loop`: calls `ledger::ensure_seeded(&cfg.cwd)?` first;
  `cfg.resume` selects `resume_messages` vs a fresh first goal message.
- `src/chat.rs` `run_chat_with`: chat sessions keep the ledger across
  turns/sessions by design — **unchanged**.
- Companion row T7 rotates `.chug/transcript.jsonl` on the same startup
  path with the same archive-not-delete semantics; land in the same
  session, separate commits.
- `.chug/` and `LEDGER.md` are gitignored; archives under `.chug/` match the
  existing operator convention (`.chug/LEDGER-spec5-archive.md`).

## Design decision

The TODO note offers two fixes: heuristic goal-vs-ledger comparison, or
always seeding a run-scoped ledger. This spec takes the deterministic
option: **a fresh run never inherits a previous session's ledger**. Any
ledger whose content differs from the pristine seed is archived (never
deleted) and the seed is re-installed. Continuity across a crash is what
`--resume` is for; `--resume` never archives. This mirrors T7's transcript
rotation exactly and needs no fragile free-text heuristics.

## Requirements

1. On a fresh autonomous run (`chug run` without `--resume`), before the
   first transcript append: if `LEDGER.md` exists and its content differs
   from `ledger::SEED`, rename it to
   `.chug/LEDGER-<YYYYMMDD-HHMMSS>.md` (UTC; numeric `-2`, `-3`, … suffix
   on same-second collision) and print one stderr line naming the archive
   path. Then `ensure_seeded` runs as today, so the run starts on the seed.
2. `--resume` behavior is unchanged: keep the existing ledger, never
   archive.
3. Chat mode is unchanged (keeps the ledger across turns/sessions).
4. Best-effort: if the rename fails (e.g. permissions), warn on stderr and
   continue with the existing ledger — never abort a run over housekeeping.
5. Absent ledger, or one byte-identical to the seed: no archive, no
   warning.

## Tests

- Fresh run with a stale (non-seed) ledger → archive exists under `.chug/`
  with the old content; `LEDGER.md` equals the seed afterwards.
- Fresh run with a pristine seed ledger or no ledger → no archive created.
- `--resume` with a stale ledger → ledger untouched, no archive.
- Rename failure (`.chug` or cwd read-only) → run proceeds, warning
  emitted, existing ledger kept.
- Collision: two archives within the same second get distinct names.

## Acceptance

- `cargo build`, `cargo clippy --all-targets -- -D warnings`, `cargo test`
  all green.
- Manual: `chug run` to completion, then a fresh `chug run` in the same cwd
  → `.chug/LEDGER-<ts>.md` holds the first run's ledger and the second run
  starts on the seed.
