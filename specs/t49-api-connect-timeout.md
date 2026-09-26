# T49 — api.rs LLM client gains a connect timeout const + value pins

check: cargo test

## Context

Three HTTP clients exist, and their timeout posture is asymmetric:

- `src/mcp_http.rs:37` — `CONNECT_TIMEOUT: Duration = Duration::from_secs(10)`
  (const pinned by T16).
- `src/webfetch.rs:54` — `WEB_FETCH_CONNECT_TIMEOUT = 10s` (values pinned by
  T42, cycle 20).
- `src/api.rs:616-617` — the reqwest blocking client for LLM calls sets only
  `.timeout(Duration::from_secs(READ_TIMEOUT_SECS))` (600s). **No connect
  timeout at all.**

reqwest has no default connect timeout: a connection attempt to a
*blackholed* endpoint (SYNs dropped — firewall rule, wedged NAT, a host that
is up but not refusing) blocks until the OS TCP stack gives up (~75 s on
macOS, ~2 min+ on Linux). T1's retry loop (8 attempts) then multiplies that
stall per attempt — worst case ~16 minutes of dead time before the run
aborts, and T2's `ACTIVITY_TIMEOUT_SECS` (180s) does not help because it
arms only after the connection exists (`src/api.rs:253-301` read watchdog).

A 10s connect timeout is strictly better for T1's original goal (survive
endpoint restarts): a refused or stalled connect fails fast and the retry
gets to a recovered endpoint sooner. It also makes the LLM client consistent
with both sibling clients. The tools-proxy endpoints this fleet talks to
connect in milliseconds; 10s is generous headroom, matching mcp_http and
webfetch.

Cycle-22 eval S2; the last open half of the "connect-timeout const-pin"
carry (T42 closed webfetch; mcp_http was already pinned by T16).

## Requirements

1. New private const in `src/api.rs` near the existing two (lines 11-15):
   `CONNECT_TIMEOUT_SECS: u64 = 10;` with a doc comment naming the class it
   closes (blackholed-endpoint connect stall; reqwest has no default;
   mirrors mcp_http CONNECT_TIMEOUT and WEB_FETCH_CONNECT_TIMEOUT; T1's
   retry loop multiplies per-attempt stall without it).
2. Add `.connect_timeout(Duration::from_secs(CONNECT_TIMEOUT_SECS))` to the
   client builder at `src/api.rs:616-617`, keeping the existing
   `.timeout(READ_TIMEOUT_SECS)` leg untouched (total-read semantics
   unchanged).
3. Retry classification unchanged: a connect timeout surfaces as a
   reqwest error for which `is_timeout()` or `is_connect()` is true, so it
   lands in the existing retryable class (`src/api.rs:202-206`) — verify by
   reading that classifier and state the verification in the commit message;
   do not modify the classifier.
4. Pin all three api.rs timeout const VALUES in api.rs's test module
   (T42 pattern — a const-only edit must fail the suite):
   `CONNECT_TIMEOUT_SECS == 10`, `READ_TIMEOUT_SECS == 600`,
   `ACTIVITY_TIMEOUT_SECS == 180`.
5. No behavior change to success paths, streaming, the activity watchdog,
   or error messages.

## Tests

- Three const-value pins in `src/api.rs`'s `#[cfg(test)]` module (one test
  or three — impl's choice; each const named so a mutation points at the
  right one).
- Non-vacuousness: the pin for `CONNECT_TIMEOUT_SECS` must fail if the
  const is changed to 9 or 11 (impl child verifies once, reverts, records
  in ledger).
- Full gates green: `cargo build`, `cargo clippy --all-targets --
  -D warnings`, `cargo test` — with the T47
  `CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared` export the
  goal carries.

## Acceptance

- `cargo test` (the check above) passes from the worktree.
- `grep -n "connect_timeout" src/api.rs` shows the builder leg.
- The three pins exist and the connect pin is non-vacuous.
- No diff outside `src/api.rs` except (optionally) this spec's row
  bookkeeping, which the impl child must NOT touch (orchestrator-owned).

## Notes

- A behavioral integration test of the 10s connect stall is impractical in
  the suite (fabricating a SYN-dropping endpoint needs a firewall rule or a
  non-routable address with multi-second OS-dependent latency; T6's
  dead-port patterns exercise fast refusals, not stalls). Const pins plus
  classifier review is the accepted evidence level for this change — the
  same level T16/T42 used for the sibling clients. The adversarial
  validator is asked to probe this decision, not to demand the integration
  test.
- Out of scope: making the timeout configurable (no CLI knob — YAGNI;
  both siblings are const-only too).
