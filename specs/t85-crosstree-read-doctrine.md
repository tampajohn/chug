# T85 — Cross-tree read friction: name the bash escape hatch in the review/validator doctrine

check: grep -q 'path escapes cwd' LOOP-SPEC.md && grep -q 'path escapes cwd' META-SPEC.md && cargo test --test todo_consistency

## Repo context

- Cycle-47 eval I3: `path escapes cwd` tool errors fired ×9 in ten cycles
  across BOTH model families and every role — orchestrators (cycles
  37/42/44/45 reading worktree files mid-review), validators
  (t76-validate3, t78-validate2, t81-validate), and the evaluator itself
  (a /tmp write during the cycle-47 eval). Every tool description carries
  the escape hatch ("cross-tree reads go through `bash`"), but the
  surfaces the roles actually READ — LOOP-SPEC §2 step 3's review
  paragraph and META-SPEC §6's validator goal template — never mention
  it. Each sighting costs ~1 iteration of self-correction.
- The code-side alternative (release the read tools cross-tree) was
  weighed and REJECTED at the eval: the confinement is a pinned,
  deliberate contract (tools.rs tests assert the refusal text) and the
  cost does not justify weakening a core tool contract.

## Requirements

1. **LOOP-SPEC §2 step 3** (the "Review." paragraph): after the
   "read the child's ledger if ambiguous" clause, add ONE parenthetical
   sentence to the effect of: "(read worktree files via `bash` — the
   file tools are cwd-confined and refuse `/tmp/chug-loop-*` paths with
   `path escapes cwd`)". Placement and exact wording are the impl's,
   subject to the pin-safety constraints below.
2. **META-SPEC §6** (the validator goal template, the
   "VALIDATION ONLY — do not implement" block): add ONE sentence to the
   goal text to the effect of: "Read worktree files via bash —
   read_file/grep/glob/list_dir/edit_file are cwd-confined and refuse
   /tmp paths with `path escapes cwd`; cross-tree reads go through bash."
   (LOOP-SPEC step 4 consumes §6's goal text verbatim except the export
   line, so §6 is the single edit point.)
3. **Pin-safety constraints (HARD)**: the inserted text must NOT contain
   any count-pinned needle: no `bash -n` (pinned exactly-once in
   LOOP-SPEC by tests/loop_spec_docs_only_gates.rs), no
   `cargo nextest run --release` / `HOST tool` /
   `command -v cargo-nextest` (tests/nextest_gate_runner.rs), no
   `CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-share…` carrier
   strings or `/tmp/chug-mut-<item>-<k>` (tests/shared_target_dir.rs).
   The clause tokens (`path escapes cwd`, cross-tree, bash) are unpinned.
4. **No pin weakening**: run the full `cargo test` after the edits. If a
   doctrine pin breaks, do NOT edit the pin file — stop and report the
   collision in the goal summary (the orchestrator re-scopes).
5. Pure-`.md` diff (two files, two insertions, ≤ ~20 lines total) — the
   round is md-only under the step-3 classification (the first live T80
   round). One commit: `T85 — name the bash escape hatch in the
   review/validator doctrine (path-escapes-cwd friction)`.

## Tests

- `cargo test` green (the doctrine pin files + todo_consistency are the
  regression surface for a doctrine-text edit; the impl runs the full
  suite once, cheap).
- The spec `check:` line above greps both files for the inserted token.
- Deletion hand-check: remove either insertion → the corresponding grep
  fails (the check is non-vacuous per file).

## Acceptance

- Both doctrine surfaces name the bash escape hatch; suite green; zero
  pin edits; diff is `.md`-only.

## Out of scope

- Releasing the read tools cross-tree (rejected at the eval);
  error-message wording changes; any code.
