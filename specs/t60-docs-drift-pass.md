# T60 — Docs drift pass: driver_lock module comment pin location + README src/ layout line

check: grep -q "driver.rs" src/driver_lock.rs && grep -q "driver_lock" README.md && grep -q "webfetch" README.md && cargo test

## Repo context

Two verified drift items from the cycle-26 eval, both docs-only:

1. **driver_lock.rs module comment** (`src/driver_lock.rs`, the closing
   module-doc line): it says "a static pin in `tests/` keeps chat
   lock-free" — but there is NO driver-lock test file under `tests/`.
   The actual pin is the unit test
   `chat_turn_never_creates_or_removes_the_driver_lock` in
   `src/driver.rs`'s test module (`src/driver.rs:3572`). (T55
   validator's non-blocking finding (i), verified at the cycle-26 eval.)
2. **README Development layout line** (README.md:299-300): the
   `src/{...}` brace list names 18 files but the tree has 20
   (+`main.rs`, named separately). Missing: `archive` (T3/T7 era),
   `driver_lock` (T55), `webfetch` (T37) — the last two are this
   month's features, invisible in the map a newcomer uses to navigate
   (cycle-26 eval §6c).

## Requirements

1. `src/driver_lock.rs`'s module comment names the real pin location:
   the `chat_turn_never_creates_or_removes_the_driver_lock` unit test in
   `src/driver.rs`'s test module (drop the "`tests/`" claim; naming the
   test function is the pin-accurate phrasing). Comment-only hunk — no
   code changes.
2. README.md's Development layout line gains `archive`, `driver_lock`,
   and `webfetch` inside the existing `src/{...}` brace list
   (alphabetical placement matching the existing style). One-line hunk;
   every other README byte identical.
3. Nothing else changes.

## Tests

- Docs-only item: no new tests. `cargo test` stays green (the existing
  T41/T55 pins must not be disturbed — implementer runs the full
  suite).
- Non-vacuousness is by inspection: the grep legs of `check:` fail
  before the edits and pass after.

## Acceptance

- `check:` passes in the impl worktree.
- Exactly two files touched (`src/driver_lock.rs` comment,
  `README.md` layout line); `git diff --stat` shows two small hunks.
- No `|` in the TODO row's notes cell (T40).
