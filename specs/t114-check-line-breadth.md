# T114 — META-META-SPEC spec bar: check: lines must see tests the change can BREAK

check: grep -q 'every test the change can BREAK' META-META-SPEC.md && grep -q 'never the `tests/` integration binaries' META-META-SPEC.md && cargo test --test loop_spec_recovery

## Repo context

Cycle-60 evidence (the T111 arc): the spec's `check: cargo test --bin
chug` ran the bin unit tests only, so the child's own goal gate never
executed the `tests/readme_layout.rs` integration pin (T95) its README
edit broke — the REAL RED was caught by the ORCHESTRATOR's review gates
instead (orchestrator trivial fix fc1d691; cycle-60 Outcomes T111 entry
and wrap notes: "the next eval weighs whether src-touching specs' check:
lines should name the full suite or a broad stem").

META-META-SPEC's spec quality bar today covers the ADD side ("A
cargo-test `check:` filter MUST be broad enough to run every test the
change adds — prefer the module stem over a narrower substring") but
says nothing about the tests a change can BREAK without adding — the
integration pins over files the change touches (README layout, doctrine
carriers). `--bin chug` never runs any `tests/` binary, so a
README/doctrine-touching spec with a `--bin chug` check is blind to
exactly the pins most likely to break.

This is a DOCTRINE row (touches META-META-SPEC.md): it runs ALONE (no
other child in flight) and validation is kimi REQUIRED (loop/spec
doctrine). House precedent: T110's pin legs live in
`tests/loop_spec_recovery.rs` — this row's pin joins them there.

estimate: ~50 changed lines (≈12 doctrine + ≈35 pin legs + comments)

## Requirements

1. META-META-SPEC.md's spec quality bar (the paragraph beginning "A
   cargo-test `check:` filter MUST be broad enough…") gains the
   BREAK-side rule, carrying these two needles verbatim:
   - `every test the change can BREAK`
   - `never the `tests/` integration binaries`
   The rule: a `check:` line must run every test the change can BREAK,
   not only every test the change ADDS; `cargo test --bin chug` runs
   the bin unit tests only and never the `tests/` integration binaries
   (README layout pins, doctrine carriers), so a spec touching
   README.md, tests/, or pinned doctrine files writes plain
   `cargo test` or names the integration targets explicitly (the T111
   readme_layout escape named as the evidence).
2. Existing bar text (the `--lib` prohibition, the module-stem
   guidance, the estimate-ceiling sentence) stays intact — this is an
   insertion, not a rewrite.

## Tests

3. New pin legs in `tests/loop_spec_recovery.rs` beside T110's leg h:
   - both needles present exactly-once in META-META-SPEC.md;
   - the insertion sits in the spec-quality-bar window (anchored: in or
     immediately after the sentence carrying the ADD-side rule `every
     test the change adds`, and before the estimate-ceiling sentence
     `Every spec carries an `estimate: ~N changed lines` line`) —
     ordering pinned;
   - RED-proven: each leg fails when its needle/ordering is broken
     (prove by temporary mutation before finalizing, per house
     RED-prove practice).
4. All existing legs in `tests/loop_spec_recovery.rs` (and the other
   doctrine-pin families) stay green UNTOUCHED.

## Acceptance

- The spec's own `check:` line green (greps + the named integration
  target — this spec eats its own cooking: content checks are
  worktree-relative greps, and the pin family is named explicitly
  rather than hidden behind a bin-only filter).
- Full `cargo test` run by the orchestrator at review (doctrine rows
  get full gates regardless).

## Out of scope

- Mechanical linting of existing specs' check: lines (the corpus of
  live specs is worked once each; the rule binds NEW filings — a
  retro-sweep of all 112 spec files is noise).
- Changes to LOOP-SPEC's gate-runner rules (T82 nextest-first is
  untouched; this row governs what a spec's OWN check: line may say).
