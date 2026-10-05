# T233 — Socket-teardown gate flakes: daemon UDS transient-success leg + mcp_http probe-retry exhaustion (tests-only, robustness)

check: set -o pipefail; export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-validate-b; touch src/*.rs tests/*.rs; cargo test --bin chug

estimate: ~200 changed lines all-in (pin-closure row kind whose
remedy seams PRE-EXIST — T151 tri-state + T214 scale — so the
calibration risk is UNDER-shoot and is priced up deliberately from
the ~80-line narrative: two leg rewrites + pure-seam pins +
RED-proofs + commit-message sweep verdicts)

## Concern

Two remaining un-hardened socket-classification test legs false-red
goal gates under host load — BOTH observed organically in ONE arc
(cycle 106, the T232 children), each green on identical-bytes
re-run, each in a module the diff did not touch:

- **Leg (a) — daemon UDS stale-socket.**
  `daemon::tests::stale_socket_connects_refused`
  (src/daemon.rs:1802-1841) failed the t232-VALIDATE goal gate
  1231/1 — "connect to the dead socket SUCCEEDED — something is
  listening" (src/daemon.rs:1831) — then passed 5/5 isolated and
  1232/1232 on the full re-run against the SAME binary hash
  (`.chug/verdict-t232-20261005.md` finding 9, with the
  diff-untouched proof). The leg's transient classification is
  ASYMMETRIC: transient ERRORS (EMFILE os error 24, ENOENT os error
  2 — both named in its doc comment) get a bounded retry to the 10s
  `STALE_CLASSIFY_DEADLINE`, but a transient connect SUCCESS panics
  on the spot. The doc comment itself names the mechanism ("the
  close-vs-connect teardown burst reproduced in the scratch
  bind/drop/connect racer"), and T151 proved the kernel shape for
  TCP (src/mcp_http.rs's `Invalidation::TeardownArtifact`): on
  macOS a connect to a just-closed listener completes from the
  kernel's pending-accept backlog, then refuses once the backlog
  drains. A transient success is a NON-VERDICT, and today it is an
  instant red.
- **Leg (b) — mcp_http probe-retry exhaustion.**
  `mcp_http::tests::dead_port_probe_retry_recovers_after_scripted_theft`
  (src/mcp_http.rs:3014) failed the t232-IMPL goal gate on the
  EXHAUSTION leg — "port-theft persisted across all 3 attempts
  (… T31 residual race, retried per T66)" — green in isolation
  (0.00s) and on the identical-bytes full re-run 1232/1232 (the
  impl child's accepted summary,
  `.chug/events-t232-impl-20261005-034529.jsonl`). The cycle-16 eval
  recorded the first organic T31-residual sighting (~1/8 gate
  runs) as a WATCH item; this second organic sighting — inside a
  child's goal gate, costing a false rejection plus a loaded
  re-verify inside a 50-minute budget — meets the "indictable once
  more" condition the cycle-106 wrap named.

Cost class: the T152/T172/T225 false-red lineage — red gates the
child must diagnose and re-run inside its budget, and flake fatigue
that teaches children to dismiss red in this family, which is how a
real regression gets waved through.

## Repo context

- Leg (a): src/daemon.rs `stale_socket_connects_refused` (test,
  1802-1841) and its consts `STALE_CLASSIFY_DEADLINE` (10s,
  src/daemon.rs:1551) / `STALE_CLASSIFY_BACKOFF` (25ms, 1553). The
  shipping classifier `connect_refused` is PRODUCTION code and is
  NOT this row's surface — the flake is the TEST's poll loop, whose
  `Ok(_) => panic!(...)` arm is the only unbounded-verdict leg.
- Leg (b): the T66/T151 machinery is all in mcp_http.rs's
  `#[cfg(test)] mod tests` (module opens at src/mcp_http.rs:1198):
  `DEAD_PORT_RETRY_ATTEMPTS = 3` (1551), the tri-state
  `Invalidation::{Theft, TeardownArtifact}` classifier
  (`invalidation_or_regression`, 1671; `confirm_connect`, 1409;
  `drain_pending_accepts`), and the two drivers
  (`dead_port_retry_with` 1718, `dead_port_probe_retry_with` 1796).
  The scripted pins assert exact attempt counts
  (`bind_calls == 2` success leg, `== DEAD_PORT_RETRY_ATTEMPTS`
  exhaustion leg). The exhaustion mechanism is a HYPOTHESIS per the
  T228 filing bar: cross-process ephemeral-port churn — T151's
  `timing_guard` is a PROCESS-LOCAL static mutex, and the host runs
  several concurrent chug loops/daemons plus parallel test binaries
  that do not share it, so a just-freed ephemeral port can be
  claimed inside the drop→probe window repeatedly. What is verified
  (not hypothesis): the bound is 3, the scripted-theft test consumes
  attempt 1 by construction, and the organic invalidation classes
  (theft + teardown artifact) exhausted the remaining 2.
- The T214 load machinery exists and is pure-seamed
  (src/testsupport.rs:137-166): `load_scaled_deadline(base)` =
  base × `scale_factor(loadavg_1m, cores)` clamped to [1.0, 4.0],
  every seam failure fail-SAFE to factor 1.0 (byte-identical
  quiet-host behavior), `scale_factor` itself pure.
- Precedent for the leg-(a) remedy shape: `confirm_connect`'s
  bounded live/refused/unresolved tri-state (src/mcp_http.rs:1409)
  and the T151 fix-up's "first-succeeds-then-refuses = artifact,
  retried like theft; sustained-live = genuine listener" rule.

## Requirements

1. **Leg (a): the transient-SUCCESS arm becomes a bounded
   re-verify, same shape as the error arms.** In
   `stale_socket_connects_refused`'s poll loop, an `Ok(_)` connect
   is no longer an instant panic: it is re-verified by a short
   bounded confirmation (success-then-REFUSED = teardown artifact —
   record it and keep polling to the existing
   `STALE_CLASSIFY_DEADLINE`; SUSTAINED success across the
   confirmation window = a genuine listener answers — panic naming
   that distinction: a real rogue listener still dies, fast). The
   ECONNREFUSED-breaks-green and transient-error-retry legs are
   byte-unchanged in behavior; the deadline and backoff constants
   are unchanged; the confirmation window is a NEW const whose
   magnitude rides the existing backoff scale (no unscaled waits,
   no timeout bumps to existing consts). The panic message for the
   sustained case names BOTH classes (what a sustained listener
   means vs what a transient backlog artifact was) so a future red
   is self-diagnosing.
2. **Leg (b): the retry bound becomes load-scaled through a pure
   seam.** The organic-invalidation attempt budget for the two
   dead_port retry drivers is computed by a pure seam — base
   `DEAD_PORT_RETRY_ATTEMPTS` (= 3, UNCHANGED as the factor-1
   value) scaled by `scale_factor(read_loadavg_1m(), cores)` with
   the T214 clamp [1.0, 4.0] (so 3..=12 attempts, hard-capped —
   never unbounded, and a pathological host still fails in bounded
   time naming itself). Every seam failure fails SAFE to the base
   (exactly today's behavior). The scripted unit pins keep their
   determinism BY CONSTRUCTION: they drive the pure seam (or a
   factor-1 path) so `bind_calls == 2` and the exhaustion-at-base
   assertions stay byte-exact; new pure legs pin the scaled
   arithmetic (factor 4.0 → 12; failed reads → 3). The per-class
   exhaustion wording (theft vs teardown-artifact, distinct
   messages) is preserved and gains the scaled attempt count so an
   exhaustion red names the budget it exhausted.
3. **Sweep-the-family (T69)**: inspect the adjacent
   socket-classification legs — `assert_dead_port`'s panicking form
   (webfetch's T31 leg keeps it by design), the `Confirm` consumers,
   and daemon.rs's other connect-classification tests — and name
   the per-leg verdict (hardened / already bounded / deliberately
   panicking and why) in the commit message. Legs outside the two
   named ones are NAMED, not changed — one concern per row.
4. **RED-proofs, one named mutant per leg**: (a) a sustained rogue
   listener on the stale socket path must still FAIL the daemon leg
   (non-vacuousness — the re-verify must not mask a real listener
   into green); a success-then-refused scripted artifact must PASS
   without exhausting; (b) dropping the scale seam (base always)
   must die on the new pure scaled-arithmetic pin; a mutant
   returning factor 4.0 unconditionally must die on a
   factor-1-path pin. RED outputs go in the commit message.
5. **Scope: src/daemon.rs and src/mcp_http.rs `#[cfg(test)]`
   modules ONLY** (plus testsupport reuse — NO changes to
   testsupport.rs itself; `scale_factor`/`read_loadavg_1m` are
   imported, not modified). No production code, no doctrine, no
   other test file. `connect_refused`, the `Confirm` enum, and both
   drivers' SIGNATURES stay compatible with their existing callers.

## Tests

- The rewritten legs green: single-threaded
  (`cargo test --bin chug -- --test-threads=1` for the two
  modules) AND the default threaded run.
- The new pure-seam pins + scripted-determinism pins green; the
  RED-proof log per req 4 in the commit message.
- The check line (`cargo test --bin chug`) green — daemon and
  mcp_http are both bin cfg(test) modules, so this filter covers
  every surface the change can break.

## Acceptance

- No instant-panic-on-transient-success leg remains in the daemon
  stale-socket test; the sustained-listener RED-proof dies and the
  artifact leg passes, both named in the commit message.
- The dead_port retry bound is load-scaled through a pure seam with
  factor-1 byte-identical behavior; the exhaustion message names
  the scaled budget; the scripted attempt-count pins are unchanged
  in shape.
- Sweep verdicts (req 3) named per inspected leg in the commit
  message.
- Check line green; clippy `-D warnings` clean.

## Out of scope

- Production classification behavior (`connect_refused`, daemon
  ensure/unlink logic), the mcp_http production client, any
  timeout/deadline constant's VALUE (STALE_CLASSIFY_DEADLINE stays
  10s; DEAD_PORT_PROBE_TIMEOUT stays 1s; only the ATTEMPT COUNT
  scales), cross-process serialization of the whole suite (the
  process-local lock limit is named, not fixed), testsupport.rs
  edits.
