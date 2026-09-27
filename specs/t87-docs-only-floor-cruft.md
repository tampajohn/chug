# T87 — Docs-only guard floor: remove the unreachable `bash -n` leg + fix the go-red overclaim

check: ! grep -q 'bash -n` on any' LOOP-SPEC.md && grep -q 'pinned' LOOP-SPEC.md && cargo test

## Repo context

- Cycle-44 carry (the T80 validator's two non-blocking observations,
  restated by the cycle-47 eval §2):
  (a) LOOP-SPEC §2 step 3's docs-only rule shrinks gates to "the guard
  floor `cargo test --test todo_consistency` …, plus `bash -n` on any
  `.sh` in the diff" — but the md-only classification (every changed file
  ends `.md`) means an eligible diff contains NO `.sh` by construction:
  the leg is dead text.
  (b) Step 3's rationale overclaims: "a README clause or a doctrine
  sentence cannot go red in the full suite — for those diffs the full
  runs are pure latency" (LOOP-SPEC.md:194–196). FALSE for doctrine text
  pinned by the doctrine pin files (tests/loop_spec_*.rs,
  tests/shared_target_dir.rs, tests/nextest_gate_runner.rs): an md-only
  edit to a PINNED carrier paragraph breaks a pin the guard floor does
  not run — red merges are possible under the floor today.
- The `bash -n` leg IS pinned: tests/loop_spec_docs_only_gates.rs carries
  `const BASH_N` + a count pin + module-doc quotes — so the pin file must
  be updated in the SAME commit (the T72/T75 in-place-hunk + pin-file
  pattern). UNRELATED `bash -n` uses MUST NOT move:
  tests/nextest_gate_runner.rs:188–191 runs `bash -n` on loopd.sh as a
  syntax check (a real gate, not doctrine text).

## Requirements

1. **LOOP-SPEC §2 step 3**: delete the ", plus `bash -n` on any `.sh` in
   the diff" leg from the guard-floor sentence (keep the bounded-cap /
   env-prefix parenthetical and the "skipped at review AND post-merge"
   scope byte-identical).
2. **Same paragraph**: replace the overclaim with the honest risk model,
   to the effect of: todo_consistency covers the docs-only table-format
   risk; an md-only edit to UNPINNED docs text cannot go red in the full
   suite, so for those diffs the full runs are pure latency — but an
   md-only edit touching a PINNED doctrine carrier (a paragraph pinned by
   tests/loop_spec_*.rs, tests/shared_target_dir.rs, or
   tests/nextest_gate_runner.rs) can break a pin the floor does not run:
   the editor (orchestrator or child) ALSO runs the affected pin file's
   tests (seconds), and any doubt defaults to full gates under the
   existing ambiguity rule.
3. **tests/loop_spec_docs_only_gates.rs (same commit)**: remove the
   BASH_N const + its needle pair + every assertion referencing it;
   update the module-doc comment (quotes the leg at line ~10) and the
   test-doc comments that name the leg (lines ~58, ~198); add ONE new pin
   leg asserting the replacement risk-model text exists (e.g. a
   `pinned doctrine carrier`-class needle occurring exactly once in
   LOOP-SPEC.md). Keep the file's other pins byte-identical (FLOOR,
   FLOOR_CMD counts, POST_MERGE_SHRINK, predicate, ambiguity legs).
4. Verify no other doctrine surface quotes the removed leg
   (`grep -rn 'bash -n' LOOP-SPEC.md META-SPEC.md README.md` — after the
   edit only unrelated/live uses may remain) and that
   tests/nextest_gate_runner.rs is untouched.
5. One commit: `T87 — docs-only floor: drop the unreachable bash -n leg,
   pin the honest go-red risk model`.

## Tests

- Full `cargo build` + `cargo clippy --all-targets -- -D warnings` +
  `cargo test` green (`.rs` in the diff → NOT an md-only round: full
  gates at review and post-merge).
- RED legs the impl demonstrates before wrapping: (1) re-insert the old
  `bash -n` leg into LOOP-SPEC.md → the updated pin file has no pin
  blessing it and the check: line's `! grep` fails; (2) delete the new
  risk-model sentence → the new pin leg fails.
- The spec `check:` line above is non-vacuous per leg (negated grep for
  the dead leg, positive grep for the risk-model token, full suite).

## Acceptance

- Dead leg gone; risk model honest and pinned; pin file updated in the
  same commit; nextest_gate_runner.rs byte-identical; full gates green.
- Validation: REQUIRED (kimi) — LOOP-SPEC is the loop doctrine itself
  (step-4 REQUIRED list).

## Out of scope

- Changing the md-only classification itself; re-pinning other doctrine
  sentences; META-SPEC edits.
