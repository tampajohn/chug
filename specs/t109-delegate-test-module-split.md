# T109 — delegate.rs test-module family split (T104-shaped, pre-emptive)

check: cargo test --bin chug

## Repo context

Cycle-59 eval §2 (EVALUATION.md): delegate.rs is **4,255 lines** — the
largest module in the tree — and +257 lines this cycle (T103's probes +
tests). Its shape is the T104 one: the PRODUCTION half is healthy
(1,125 lines, delegate.rs:1–1125) and the growth is the TEST MODULE
(`#[cfg(test)] mod tests` at delegate.rs:1126 → EOF: **3,129 lines,
83 `#[test]` fns** — launch/status/collect/wait/argv/schema families
from T23/T28/T29/T39/T58/T68/T69/T89/T103 all landed here). The ~4,500
trip-line convention (T84/T104) has not fired (4,255 = 94.6%), but the
growth rate makes the crossing certain within 1–2 cycles and every
delegate-touching row already pages a 4,255-line file three reads deep
(t103-impl did, 61/80). This is the T104 split executed PRE-EMPTIVELY
while the playbook is fresh — the split only gets bigger from here.
The weighed-and-rejected alternative (wait for the trip line) is
recorded in the eval's decision log.

The split (identical rules to T104, spec t104-driver-test-module-split.md
— the Rust-2018 file+directory module pair):
- `src/delegate.rs` keeps its `#[cfg(test)] mod tests;` DECLARATION
  (one line, attributes intact — note: delegate.rs's module is
  `mod tests`, visibility as written today) and the module BODY moves
  to `src/delegate/tests/mod.rs`.
- `src/delegate/tests/mod.rs` holds the shared harness (the
  `delegate_ctx` / `write_argv_stub` / `ensure_spec_file` /
  `wait_for_argv_dump` / `spawn_pid_of` helper family,
  delegate.rs:1136–3384 approx) plus one `mod <family>;` line per
  extracted family file.
- Each cohesive test family moves to its own
  `src/delegate/tests/<family>.rs` — the impl picks the families by
  test-name prefixes (launch / argv-builder / schema pins / summary
  state machine / wait_secs+wake-set / collect / resume / max_tokens /
  spec-existence probes — the exact cut is the impl's judgment under
  the ONE rule: every test lands in exactly one family file; the
  harness and only the harness stays in mod.rs).
- T104's rules verbatim: byte-identical MOVES (no logic edits, no
  renames, no signature changes; test count before == after, both
  numbers in the commit message); `use super::*;` per family file
  adjusted to the module path as the cut requires; anything a
  cross-module caller uses stays reachable exactly as today.

Guard interplay (verified at filing): `tests/readme_layout.rs` walks
`std::fs::read_dir(src)` NON-recursively (T104 verified the same) —
subdirectory modules do NOT enter its set, so the README layout line
needs NO edit and the guard must stay green UNTOUCHED. `src/main.rs`
needs no new `mod` line (the tests module hangs off delegate.rs
itself; delegate.rs is already declared there).

## Requirements

1. The split above: delegate.rs shrinks to ~1,130 lines (production +
   the one-line tests declaration); every moved line byte-identical.
2. Test count before == after (83 `#[test]` fns at filing — recount at
   implementation and use the fresh number), both numbers in the
   commit message; full suite green from the new layout.
3. RED-proof (T104 shape): drop one `mod <family>;` line → that
   family's tests vanish and a count pin fails; revert one moved test
   to a stub → it fails. Record the legs in the commit message.
4. A count pin test (new, in the moved module) asserting the total
   delegate test count so a dropped `mod` line fails LOUDLY (the T104
   M1 mutant lesson: mod-drop was caught by the count pin — without
   one it is silent).
5. Non-goals: no production-code moves, no behavior change, no README
   edit, no guard edit, no renames.

## Tests

- `cargo test --bin chug` green in the worktree; count equality proven
  (req 2); `cargo clippy --all-targets -- -D warnings` clean.
- `cargo test --test readme_layout` green UNMODIFIED.
- Non-vacuousness: the split is a MOVE — the validator's angle is
  byte-identity + count equality + the req-3/req-4 loud legs (a mutant
  dropping a family `mod` line must fail the count pin; a mutant
  editing moved logic dies against the moved tests).

## Acceptance

- The `check:` line passes in the worktree.
- Diff is src/delegate.rs + new src/delegate/tests/** ONLY.
- Validation: delegate.rs is NOT on the LOOP-SPEC §2 step-4 REQUIRED
  list (driver.rs/api.rs/tools.rs/events.rs/doctrine) → kimi OPTIONAL
  per the T16/T31 precedent; take the round only on an idle queue
  (cycle-55 T99 precedent). The orchestrator's byte-identity +
  count-equality review gates substitute.
