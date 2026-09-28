# T115 — goal-integrity surface: delegate launch echoes + run_start goal_sha256

check: cargo test

## Repo context

The display-artifact watch's escalated criterion has TRIPPED. Cycle-60's
eval (EVALUATION.md I3, carried in the handoff watch list) set: "any
artifact in a transmitted CODE or DOCTRINE payload, or a SECOND
payload-level sighting anywhere → immediate row." Cycle 60 recorded the
second-ever payload-level sighting — and the first in an
ORCHESTRATOR-AUTHORED payload: the T111 validator's `delegate launch`
goal arrived with a DUPLICATED TAIL (the composed goal text repeated its
own trailing block). It was caught by the orchestrator eyeballing its
own payload at send time, the child was killed at ~25s, and the
relaunched goal was clean (cycle-60 Outcomes T111 entry + wrap notes;
harvested stream `.chug/events-t111-validate-killed-20260928-090720.jsonl`).
First sighting: the U+FFFD pair in the t108-validate2 ledger (cycle 59).

Detection today is LUCK, not mechanism: `delegate launch` returns only
pid/log/events paths, and the child's `run_start` event records spec,
model, budgets, cwd, head — but NO goal field at all (verified in the
killed validator's stream: no goal key). Nothing anywhere records what
goal text a child actually received, so a garble that the sender doesn't
happen to re-read runs to budget death on a corrupted objective.

Two garble classes, honestly separated (the spec pins must not overclaim):

- **Composition class** (the observed class): the parent model garbles
  its own tool-call composition (duplicated tail). Parent-side and
  child-side hashes would MATCH the garble — hashes do NOT detect this
  class. What detects it: a mechanical re-read surface in the launch
  RESULT (byte count + tail preview) that the sender's next iteration
  actually reads, turning the eyeball catch into a designed glance.
- **Transmission class** (zero sightings): the argv/pipe between parent
  and child corrupts. Detected by comparing the parent's hash of the
  goal it passed against the child's hash of the goal it received.

estimate: ~130 changed lines (≈45 production + ≈70 tests + ≈15 README)
plus ONE new dependency (`sha2`, justified in req 2)

## Requirements

1. `delegate` `launch` result gains three fields, computed by the parent
   over the exact goal string it will pass to the child argv:
   - `goal_bytes: <n>` (byte length),
   - `goal_sha256: "<hex>"`,
   - `goal_tail: "<tail-anchored ≤120-char preview>"` (same
     tail-anchoring rule as T25's error previews; the duplicated-TAIL
     class is visible in a tail).
2. Hash primitive: SHA-256 via the `sha2 = "0.10"` crate (new
   dependency — justified: ecosystem standard, hex output comparable
   with external `shasum -a 256` for out-of-band verification; a
   bespoke FNV/xx hash would invent a one-off primitive to save one
   small pure-Rust dep). Hash the goal's UTF-8 bytes verbatim.
3. Child side: the `run_start` event gains `goal_sha256` — hex string
   when the run has a `--goal`, `null` when it doesn't (chat without a
   goal; the field is always present in the serialized line, matching
   how `max_tokens` serializes null). Both run-mode call sites
   (`src/driver.rs` fresh + resume/plan paths) pass it through;
   computing is a pure helper (`goal_sha256(&str) -> String`) so the
   eventlog call sites stay thin.
4. README: the `delegate` launch bullet gains the three returned fields
   (with the two-class honesty sentence — composition vs transmission);
   the events-log bullet's `run_start` field list gains `goal_sha256`.

## Tests

5. `goal_sha256` against a known vector (sha256 of a fixed string,
   cross-checkable with `shasum -a 256`).
6. Launch-result fields: a launch with a known goal renders
   `goal_bytes`/`goal_sha256`/`goal_tail` matching the input; tail
   preview is tail-anchored and ≤120 chars (long-goal leg + short-goal
   verbatim leg).
7. `run_start` serialization: `goal_sha256` present and correct in run
   mode with a goal; `null` when goal-less; the other run_start fields
   byte-identical (existing pins stay green).
8. The delegate schema itself is UNCHANGED (result-text-only change —
   pin the schema surface so a future edit doesn't silently widen it).

## Acceptance

- `cargo test` green (FULL suite — this touches README and src/, so
  integration pins must run), clippy clean, `cargo build --release`
  green with the new dep.
- Orchestrator at review: launch one real delegate child and compare
  the launch-result `goal_sha256` against the child's `run_start`
  `goal_sha256` — equal (the transmission leg live-smoked).

## Out of scope

- Automatic rejection/confirmation loops (the orchestrator compares;
  chug reports, it does not adjudicate — same philosophy as the banner
  head= field).
- Hashing `spec` file content (the spec is a PATH the child re-reads;
  its drift class is covered by the run_start head_branch/head_commit
  pair).
- Retroactive detection of the render-only splice class (T108-era
  discipline: read-back verification — unchanged).
