# T235 — pin `cargo clippy --all-targets -- -D warnings` at the orchestrator gate surfaces (LOOP-SPEC doctrine + carrier pin)

check: set -o pipefail; export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-validate-a; touch src/*.rs tests/*.rs; cargo test --test loop_spec_recovery

## Repo context

Doctrine row (LOOP-SPEC.md edit + one carrier pin leg in
tests/loop_spec_recovery.rs). estimate: ~80 changed lines (the
doctrine-sentence family; the carrier pin is one leg beside T176's).

The cycle-107 wrap's final gates ran
`cargo clippy --all-targets -- -D warnings` in main and went RED on
an unused-`mut` in T233's NEW cfg(test) code (src/daemon.rs test
module; the orchestrator's 1-keyword fix landed in 61b0f5e and was
verified green). Both the t233 impl child AND the t233 kimi
validator had passed the same code. The escape path, verified by
reading the repo at cycle-108 eval time (I1):

1. Spec check lines carry NO clippy leg at all (t233/t234 check
   lines verified — test-only), so the goal gate never lints.
2. nextest compiles cfg(test) code but never LINTS it.
3. The LOOP-SPEC impl goal demands the `--all-targets` form in
   prose (the T176 paragraph), but the goal's short-form sentence
   is "clippy `-D warnings`" and a child can — and the t233 child
   did — run a narrower form that never compiles cfg(test) code.
4. The orchestrator's OWN gate surfaces name clippy with NO pinned
   form: step 3's review gates ("run bounded gates yourself …
   build + clippy + test"), step 5's post-merge re-run, and Phase
   3's final gates ("build + clippy + step 5's T82 gate runner").
   Whether cfg(test) gets linted pre-merge is orchestrator
   discretion, not doctrine — the cycle-107 catch happened only
   because that wrap orchestrator chose `--all-targets`.

Cost this instance: ~1 keyword + one wrap detour. The class cost: a
lint-red main at HEAD between merge and wrap whenever a child's
clippy form slips.

## Requirements

1. **Pin the exact form at all three orchestrator gate surfaces in
   LOOP-SPEC.md** — `cargo clippy --all-targets -- -D warnings`
   (zero warnings, not exit-0 clippy — the T176 semantics):
   (a) step 3's review-gate paragraph (the bounded-gates template
       sentence): the clippy leg names the exact form — this is the
       systematic PRE-merge catch, the row's point;
   (b) step 5's post-merge re-run: the gate enumeration names the
       same form (backstop);
   (c) Phase 3's final-gates bullet: the same form (the catch that
       fired in cycle 107, now guaranteed rather than chosen).
   Each edit is a minimal in-place sentence amendment, not a
   restructure; every surrounding sentence byte-identical.
2. **One carrier pin leg in tests/loop_spec_recovery.rs** beside
   the T176 clippy-bar pin section: assert the exact string
   `cargo clippy --all-targets -- -D warnings` appears in each of
   the three surfaces (step 3's review paragraph, step 5's
   post-merge gate text, Phase 3's final-gates bullet) — locate the
   surfaces by their existing stable anchors (the pin style the
   file already uses), and assert the form string's total count in
   LOOP-SPEC.md matches the pin's expectation so a future
   fourth-surface mention or a silent removal both go red.
3. **The check-line surface is deliberately UNCHANGED** — spec
   check lines stay test-only (the goal gate's job is test-green;
   lint is the orchestrator gates' job). The row does NOT add
   clippy to any spec `check:` line, does NOT edit the impl goal
   template (its prose already demands the form — T176), and does
   NOT edit META-META-SPEC.
4. Zero production-code changes; zero changes to any other doctrine
   file; no TODO.md/LEDGER.md edits (orchestrator bookkeeping).

## Tests

The one new pin leg in tests/loop_spec_recovery.rs (req 2). Run the
check line; the pin must be RED on the pre-row LOOP-SPEC.md (the
form is absent from at least one of the three surfaces today — the
child verifies and names which) and GREEN after the edit.

## Acceptance

- The check line green: `cargo test --test loop_spec_recovery`.
- `grep -c 'cargo clippy --all-targets -- -D warnings' LOOP-SPEC.md`
  matches the pin's expected count, with the three surfaces named
  above each carrying it.
- Full doctrine pin set green (the loop_spec_recovery binary covers
  the LOOP-SPEC carriers); `cargo test --test todo_consistency`
  green (TODO table untouched but cheap to confirm).
- The T176 goal-template paragraph and the goal's short-form
  clippy sentence byte-identical.

## Out of scope

- Adding clippy legs to spec check lines (deliberately unchanged —
  req 3).
- CI/workflow clippy legs (no .github/ change; the loop's own gates
  are the surface).
- `clippy.toml` or lint-level configuration changes.
