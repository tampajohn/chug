# T103 — delegate launch asserts spec + cwd exist (fail-fast on corrupted fields)

check: cargo test --bin chug delegate

## Repo context

Cycle-58 eval §2 I2 (EVALUATION.md): the delegate-launch field-corruption
class has FOUR sightings in five cycles — cycle 53 (goal text duplicated
into a validator launch's goal field, killed pre-work, relaunched), cycle
55 (spec field corrupted by goal text, pid 60756 killed at iteration 3
pre-work), cycle 53's T95 dispatch (orchestrator hallucinated spec slug
`t95-transcript-dir-pin.md` — the child failed at spawn with
spec-not-found), cycle 57 (a DUPLICATE `spec` key — last-key-wins
resolved to the correct path silently; verified harmless THIS time).
Cost so far: two killed children, one loud spawn failure, one silent
near-miss. Today `src/delegate.rs` validates only SHAPE: the launch path
(~line 137) bails unless `spec.is_absolute()`; a corrupted-but-absolute
spec (goal text glued to a path, a hallucinated slug, a truncated path)
spawns a doomed child that burns setup iterations before dying — or, the
cycle-57 leg, resolves silently and nobody notices the payload was
malformed. The T94 thesis applies: name the problem at the call site so
the caller self-corrects in ONE iteration instead of spawning a corpse.

Where: `src/delegate.rs` launch action, immediately after the existing
absoluteness bails (~lines 132–140). The launch already stats the cwd
for the log/events paths and holds the deliberate `resolve_safe`
exemption for these absolute paths (~line 1440) — the existence probes
ride the same exemption, no new surface.

## Requirements

1. After the absoluteness bail, `delegate` launch refuses a `spec` whose
   path does not exist as a readable file: tool error
   `delegate: spec does not exist or is not a readable file: <path>` —
   it names the path it received verbatim (a corrupted payload is
   visible in the error). Same leg for `cwd`: must exist as a directory,
   error `delegate: cwd does not exist or is not a directory: <path>`.
2. Fail-fast means fail-fast in the CALLER: no child is spawned, no
   `<cwd>/.chug/delegate.log` is created, the error returns in the
   launch tool result (not a spawned child that dies at iteration 1).
3. Probe-then-launch TOCTOU is accepted and documented in a comment:
   the path can vanish between the probe and the child's own read; the
   probe exists to catch MALFORMED payloads, not to guarantee the child
   succeeds (the child's own spec-read error remains the backstop).
4. Every pre-existing launch-validation leg (absoluteness, required
   fields, budget clamps, resume semantics) byte-identical.

## Tests

- New tests in delegate.rs's test module (the existing launch-validation
  family): nonexistent spec → the req-1 error naming the path; spec is a
  DIRECTORY → same error class; nonexistent cwd → its error; both-valid
  launches unaffected (one pre-existing happy-path launch test stays
  green unmodified).
- RED proof: each new test fails against the pre-edit code (the old code
  spawns or attempts the launch instead of erroring at the call site) —
  demonstrate at least one leg in the commit message or ledger.
- Full `cargo build` + `cargo clippy --all-targets -- -D warnings` +
  `cargo test` green in the worktree.

## Acceptance

- The `check:` line above passes in the worktree (the `delegate` stem
  runs every delegate test incl. the new legs — T96 breadth rule).
- Diff is src/delegate.rs ONLY.
- Validation routing: delegate.rs is NOT on the LOOP-SPEC §2 step-4
  REQUIRED list → kimi OPTIONAL (T16/T31 precedent); the orchestrator
  may take the optional round on an idle queue.
