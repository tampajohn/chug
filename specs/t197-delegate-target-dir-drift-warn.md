# T197 — delegate launch warns on CARGO_TARGET_DIR drift (the 74d3331 dispatch-rekey class)

check: touch src/*.rs tests/*.rs; export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared && cargo test

## Repo context

The dispatch re-key fumble class — three bites in the cycle 85–87
delta, each self-caught and fixed mid-flight at ~5–15 min cost:
`74d3331` (cycle 85), `98f6f6b` and `82804fb` (cycle 87). The
mechanism every time: the orchestrator re-keys the check-line
`CARGO_TARGET_DIR` export into the BRANCH-side spec copy, but
`delegate launch`'s `spec:` path reads MAIN's copy — so the child
launches with goal/env pointing at the role-keyed slot while its goal
gate runs the check against a different dir (the cycle-77 origin
`607e877`, T144's scrub makes the check's own export the only target
dir the gate sees). Three surfaces must agree — the spec's check line
(MAIN's copy), the goal's `export CARGO_TARGET_DIR=…`, and the
launch `env` map (T183) — and nothing checks the agreement
mechanically; it currently depends on the orchestrator remembering a
two-surface manual act.

The fix point with the best leverage: `delegate launch` itself. It
already reads the spec file (to spawn the child) and holds the goal
text and env map in hand. A launch-time advisory that compares the
three surfaces and warns on drift fires exactly at the fumble moment
and can never be skipped by a distracted orchestrator — unlike a
script, it rides the one launch path every child takes.

src/delegate.rs is NOT on the LOOP-SPEC §2-step-4 REQUIRED-validation
core list (driver/api/tools/events/doctrine); the lane predicate is
computed at dispatch from the diff.

estimate: ~180 changed lines (lint fn + launch integration + ~8 test
legs + README clause).

## Requirements

1. At `action: "launch"`, after the existing argv/env assembly and
   BEFORE the spawn returns, compute an advisory (never a refusal):
   - Read the spec file at the `spec:` path (best-effort — unreadable
     → no advisory, launch proceeds).
   - If its text carries a `check:` line containing
     `CARGO_TARGET_DIR=<dir>`, extract that dir (first occurrence).
   - Extract the goal text's `export CARGO_TARGET_DIR=<dir>` (first
     occurrence, absent = none) and the launch `env` map's
     `CARGO_TARGET_DIR` (absent = none).
   - If ≥2 of the three are present AND any present pair disagrees,
     the launch return text gains a `WARN target-dir drift:` block
     naming every present surface and its dir. Absent surfaces never
     warn (a spec with no check export is legitimate).
2. The advisory is a pure seam: a `fn target_dir_drift(spec_text:
   &str, goal: &str, env_dir: Option<&str>) -> Option<String>` (or
   equivalent) returning the rendered warning, unit-tested without
   spawning anything.
3. Launch behavior is otherwise byte-identical: the spawn, the log
   paths, the returned pid, and the no-drift return text are
   unchanged (pinned).
4. README's delegate `launch` bullet gains one clause naming the
   drift warning and the three surfaces it compares.

## Tests

- Unit legs on the pure seam: three-way agree → None; spec-vs-goal
  drift → Some naming both; spec-vs-env drift → Some; goal-vs-env
  drift → Some; only one surface present → None; no surfaces → None;
  multiple `CARGO_TARGET_DIR` in check text → first wins (documented
  in the leg name).
- One dispatch-level leg: a launch with a drifting fixture spec
  spawns normally AND its return text carries the WARN block (the
  existing CHUG_DELEGATE_BIN stub harness or the argv seam — child's
  choice, named in the commit).
- Non-vacuousness: revert the comparison (always-None mutant) → at
  least two legs RED, stated in the commit message.

## Acceptance

- Check line green; `cargo clippy --all-targets -- -D warnings`
  green.
- The warning fires on a hand-staged drift rehearsal (evidence in the
  commit message).

## Out of scope

- Auto-correction (the launch never rewrites the spec or goal —
  advisory only; the orchestrator owns the re-key).
- Refusing to launch on drift (a hard gate could deadlock legitimate
  mid-flight re-keys; weighed and rejected at filing).
