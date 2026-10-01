# T183 — `delegate` launch gains an optional allowlisted `env` map (+ LOOP-SPEC adoption)

check: export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared && cargo test

## Repo context

`delegate` has no env parameter — the child inherits the
ORCHESTRATOR's process env (src/delegate.rs:332, the T144 comment).
The T47/T161 role-keyed target-dir discipline therefore rides a
GOAL SENTENCE ("export CARGO_TARGET_DIR=… before every cargo
command") that every impl child must read and honor on every call;
T144's scrub makes a forgotten export SAFE (the child builds cold
into its own worktree target) but slow, and glm demonstrably drops
goal sentences (the cycle-83 decision_log fumble census — 5 dropped-
field calls in 3 cycles — is the same instruction-following class).
Post-T179 a missed export costs one cold worktree build (~2–4 min),
not a rebuild per call — so this row is a ROBUSTNESS CLEANUP, not an
incident fix: move an operational invariant from prompt text to
process spawn. The goal sentence stays as defense-in-depth.

estimate: ~360 changed lines (delegate.rs parse+validate+apply ~120,
tools.rs schema+description ~30, delegate tests ~150, LOOP-SPEC
template + pin updates ~50, README ~8). Touches LOOP-SPEC.md →
doctrine class: runs ALONE, kimi validation REQUIRED.

## Requirements

1. `delegate` launch gains an optional `env` object: string→string
   map. Validation (fail-closed, tool error naming the offending
   key): ≤ 16 entries; keys must match `^(CARGO_|CHUG_|RUST)[A-Z0-9_]*$`
   (the allowlist exists so a model-influenced goal cannot rewrite
   PATH/HOME/DYLD_* on the child); values ≤ 4 KiB, no NUL bytes.
2. Application order at spawn (src/delegate.rs launch): existing
   inherited env → `scrub_target_dir_vars` (T144, unchanged) → apply
   `env` entries (explicit WINS over the scrub — a launch that names
   CARGO_TARGET_DIR in `env` means it). Add the comment naming this
   ordering and why (the scrub guards the ABSENT case; `env` is the
   explicit case).
3. The launch return payload names the applied env keys (keys only,
   never values — values may carry paths the model should treat as
   opaque; keeping values out also keeps status/log previews small).
4. Schema + description (tools.rs): the `env` field documented with
   the allowlist regex, the caps, the explicit-wins-over-scrub
   ordering, and one sentence that the goal-carried `export` remains
   the fallback when `env` is absent (byte-identical behavior when
   the field is missing — pinned).
5. **LOOP-SPEC adoption (same row)**: the step-2 impl-child launch
   template gains `env: {"CARGO_TARGET_DIR":
   "/Users/jadams/workspace/chug/target-shared"}` in the delegate
   block, with one sentence that the goal's export line STAYS
   (defense-in-depth — the goal gate's T144 scrub makes the check
   line's own export the only target dir the gate sees regardless);
   the T161 overlap slots (impl-a/impl-b) pass the role-keyed dir via
   `env` the same way; the step-4 validator launch gains
   `target-shared-validate` via `env`. Update the affected doctrine
   pins IN THE SAME COMMIT (tests/shared_target_dir.rs and any
   template-shape legs that assert the launch block).
6. README: the `delegate` bullet in Tools gains the `env` field
   (allowlist + explicit-wins ordering + fallback sentence).

## Tests

- Parse/validation: absent → byte-identical spawn (pin via the argv/
  env seam); well-formed map applies; bad key (PATH, lowercase,
  empty) → tool error naming the key; 17th entry → error; 4 KiB+1
  value → error; NUL in value → error.
- Ordering leg: inherited env carries a decoy `CARGO_TARGET_DIR`, the
  scrub removes it, `env` re-adds the explicit value → the spawned
  command's env has EXACTLY the explicit value (assert on the
  constructed `Command`'s envs via the existing test seam — do not
  spawn a real child for this leg).
- Schema pins: description carries the allowlist token `CARGO_`,
  the explicit-wins sentence, and the fallback sentence.
- The launch-payload keys-only leg (req 3): value bytes absent from
  the returned text.
- LOOP-SPEC pin legs updated in-commit pass; reverting the template
  env line fails the named pin (RED-proof in the commit message).

## Acceptance

- Plain `cargo test` green (the row touches README + LOOP-SPEC pin
  carriers — check-breadth doctrine); clippy `-D warnings` clean.
- One commit; no TODO.md/LEDGER.md edits.

## Out of scope

- `delegate status`/`collect` changes; passing env to an ALREADY-
  RUNNING child (impossible by construction); widening the allowlist
  (PATH etc.) — a future row with its own risk analysis; removing the
  goal-carried export sentences (they stay as defense-in-depth —
  revisit only if the dual surface drifts).
