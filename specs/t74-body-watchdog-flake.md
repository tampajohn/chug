# T74 — api.rs body_watchdog cold-parallel timing flake (2 sightings, one day)

check: cargo test --bin chug body_watchdog && cargo test

## Repo context

`read_body_with_watchdog` (src/api.rs, T2) aborts a streaming response body
that goes silent: an injected per-chunk ACTIVITY timeout (production 180s,
tests inject ~1s) distinct from the 600s total read timeout. Two inline
tests pin the contract (src/api.rs `mod tests`):

- `body_watchdog_aborts_silent_stream` (~line 1082): a reader that never
  sends must error at the activity timeout. Asserts the error class AND
  `elapsed >= 900ms` (not instant) AND `elapsed < 1900ms` (fired at the
  ~1s activity timeout, not the 600s total).
- `body_watchdog_allows_slow_steady_stream` (~line 1130): 5 chunks at
  250ms intervals (1.25s total > 1s activity timeout, each gap < timeout)
  completes — proves the watchdog is per-chunk activity, not a total
  deadline.

**Evidence (two sightings, 2026-09-26, promoted from the twice-escalated
watch item):**

1. t70-impl's first goal-gate REJECTION — `check command failed` on a
   cold-build full-suite run, diagnosed by the child as api.rs
   body_watchdog timing; the check re-ran green 4x (commit b6b22c7 row
   notes; .chug/events-t70-impl-20260926-140700.jsonl `goal: rejected 1`).
   The rejection burned the resumed child's iterations and could mask real
   red.
2. T71 impl's 518/1 parallel run — same signature, re-ran green 3x (commit
   e1e940c / 2a8fe30 row notes: "2nd sighting api.rs body_watchdog
   cold-parallel flake").

Both sightings were COLD default-parallel full-suite runs (first `cargo
test` after a big diff, all cores saturated). Gate-blocker class (T66 row
text): every cycle's goal gate and orchestrator review gate is a
default-parallel `cargo test`, so a timing-upper-bound flake false-reds any
future arc.

**Root-cause hypothesis (verify, don't cargo-cult):** the 1900ms upper
bound assumes the watchdog thread fires within ~900ms of the injected 1s
deadline; under a cold parallel build the watchdog's sleep/poll scheduling
can exceed that. The upper bound's PURPOSE is discriminating
activity-timeout (~1s) from total-read-timeout (600s) — that discrimination
survives a much wider bound (e.g. 15s is still 40x under 600s). The
steady-stream test carries the mirror-image hazard: a `sleep(250ms)` can
stretch past the 1s activity timeout under the same load, false-redding
the green leg.

Precedents: T59/T66 (dead_port_probe bounded theft-retry + margin audit),
T31 (deflake parallel-load family).

## Requirements

1. **Keep both discriminations, absorb the load.** Rework the two tests'
   timing assertions so each keeps its load-bearing meaning under cold
   default-parallel load:
   - aborts_silent: keep a not-instant lower bound (~900ms class) and an
     activity-NOT-total upper bound that stays << 600s while absorbing
     scheduler delay (e.g. 15s) — with a comment naming the discrimination
     the bound preserves (the cycle-14 T31 comment convention).
   - allows_slow_steady: re-audit the margins so every inter-chunk gap has
     generous headroom under the activity timeout AND the total stream
     time still exceeds the activity timeout by a safe factor (the
     per-chunk-not-total point is the test's reason to exist). Numbers are
     the child's choice; the invariant is not.
2. **Sweep the family (T72 doctrine).** Audit every OTHER timing-margin
   assertion in src/api.rs's test module for the same cold-parallel class
   (there is at least the error-classification test's 1s injection —
   class-only, likely safe, but LOOK). Any leg sharing the class gets the
   same treatment in the same diff, each with its own non-vacuousness
   proof.
3. **Tests-only change expected.** If the investigation shows the
   production watchdog itself (not the test margins) is wrong, STOP and
   say so in the LEDGER with evidence — a production change is a different
   row; do not mix it in.
4. **Non-vacuousness RED-proven both directions** and recorded in the
   commit message: (a) gut the watchdog (never fire) → aborts_silent must
   go RED; (b) make the watchdog fire on TOTAL elapsed time instead of
   per-chunk activity → allows_slow_steady must go RED. Revert after
   proving.

## Tests

- The two modified tests green under DEFAULT parallelism (no
  `--test-threads=1` crutch — that would defeat the point).
- **Acceptance gate: 10 consecutive full-suite `cargo test` runs at
  default parallelism green in the worktree** (the T66 4x pattern raised —
  this flake fired ~1/hundreds of runs, so 10 is a smoke gate, not a
  proof; the margin redesign is the proof). Record the 10/10 in the commit
  message.
- `cargo clippy --all-targets -- -D warnings` clean.

## Acceptance

- `cargo build && cargo clippy --all-targets -- -D warnings && cargo test`
  green in the worktree (export
  `CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared` first).
- `check:` line passes verbatim from the worktree root.
- `git diff` confined to src/api.rs `mod tests` (plus its comments).
- Commit message records: the chosen bounds + the discrimination each
  preserves, the family-sweep result (even if "no other leg"), both
  RED-proofs, and the 10/10 default-parallel acceptance.

## Out of scope

- Production changes to read_body_with_watchdog (see requirement 3).
- Other files' timing tests (mcp_http etc.) — T59/T66 already swept those;
  this row is api.rs only.
- Validation routing: src/api.rs is on LOOP-SPEC §2 step 4's REQUIRED
  list, so kimi validation applies even though the change is tests-only.
